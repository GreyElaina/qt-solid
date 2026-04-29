use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use crate::compositor_actor::FrameSignalSource;
use crate::types::QtCompositorSurfaceKey;

const FALLBACK_INTERVAL: Duration = Duration::from_millis(16);

fn trace_enabled() -> bool {
    static ENABLED: std::sync::LazyLock<bool> =
        std::sync::LazyLock::new(|| std::env::var_os("QT_SOLID_WGPU_TRACE").is_some());
    *ENABLED
}

fn trace(args: std::fmt::Arguments<'_>) {
    if !trace_enabled() {
        return;
    }
    println!("[qt-frame-signal] {args}");
}

pub struct FrameSignal {
    surface_key: QtCompositorSurfaceKey,
    alive: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

// SAFETY: surface_key is Copy, atomics are Arc'd, thread handle is only joined on drop.
unsafe impl Send for FrameSignal {}

impl FrameSignal {
    pub fn new(surface_key: QtCompositorSurfaceKey) -> Self {
        Self {
            surface_key,
            alive: Arc::new(AtomicBool::new(true)),
            running: Arc::new(AtomicBool::new(false)),
            thread: None,
        }
    }

    fn ensure_thread(&mut self) {
        if self.thread.is_some() {
            return;
        }
        let alive = Arc::clone(&self.alive);
        let running = Arc::clone(&self.running);
        let key = self.surface_key;
        self.thread = Some(
            thread::Builder::new()
                .name("qt-frame-signal".into())
                .spawn(move || vsync_thread_main(key, alive, running))
                .expect("failed to spawn frame signal thread"),
        );
    }
}

impl FrameSignalSource for FrameSignal {
    fn start(&mut self) {
        self.ensure_thread();
        self.running.store(true, Ordering::Release);
        trace(format_args!(
            "start kind={} primary=0x{:x}",
            self.surface_key.surface_kind, self.surface_key.primary_handle,
        ));
    }

    fn stop(&mut self) {
        self.running.store(false, Ordering::Release);
        trace(format_args!(
            "stop kind={} primary=0x{:x}",
            self.surface_key.surface_kind, self.surface_key.primary_handle,
        ));
    }
}

impl Drop for FrameSignal {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::Release);
        self.running.store(false, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

unsafe extern "C" {
    fn qt_solid_post_frame_signal_for_node(node_id: u32);
}

fn vsync_thread_main(
    key: QtCompositorSurfaceKey,
    alive: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
) {
    trace(format_args!(
        "vsync thread started kind={} primary=0x{:x}",
        key.surface_kind, key.primary_handle,
    ));

    let pci_bus_id = crate::surface::window_compositor_adapter_pci_bus_id(
        crate::types::QtCompositorTarget {
            surface_kind: key.surface_kind,
            primary_handle: key.primary_handle,
            secondary_handle: key.secondary_handle,
            width_px: 1,
            height_px: 1,
            scale_factor: 1.0,
        },
    );
    let mut vblank = platform_vblank::VblankWaiter::new(pci_bus_id.as_deref());

    while alive.load(Ordering::Acquire) {
        if running.load(Ordering::Acquire) {
            if let Some(node_id) = crate::compositor_actor::lookup_surface_node_id(&key) {
                trace(format_args!(
                    "vsync fire node={node_id} kind={} primary=0x{:x}",
                    key.surface_kind, key.primary_handle,
                ));
                unsafe { qt_solid_post_frame_signal_for_node(node_id) };
            }
        }
        vblank.wait();
    }

    trace(format_args!(
        "vsync thread exiting kind={} primary=0x{:x}",
        key.surface_kind, key.primary_handle,
    ));
}

// ---------------------------------------------------------------------------
// Platform vblank implementations
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
mod platform_vblank {
    use super::{FALLBACK_INTERVAL, trace};
    use std::time::Instant;

    pub struct VblankWaiter {
        initialized: bool,
    }

    impl VblankWaiter {
        pub fn new(_pci_bus_id: Option<&str>) -> Self {
            Self { initialized: false }
        }

        pub fn wait(&mut self) {
            if !self.initialized {
                self.initialized = true;
                trace(format_args!("vblank-windows: using DwmFlush"));
            }
            let start = Instant::now();
            let hr = unsafe { windows_sys::Win32::Graphics::Dwm::DwmFlush() };
            let elapsed = start.elapsed();
            // DwmFlush returns S_OK (0) on success. If it fails or returns
            // suspiciously fast (< 1ms, e.g. monitor asleep), fall back to sleep.
            if hr != 0 || elapsed < std::time::Duration::from_millis(1) {
                std::thread::sleep(FALLBACK_INTERVAL);
            }
        }
    }
}

#[cfg(all(
    any(target_os = "linux", target_os = "freebsd"),
    any(feature = "x11", feature = "wayland"),
))]
mod platform_vblank {
    use super::{FALLBACK_INTERVAL, trace};
    use std::os::fd::AsFd;

    pub struct VblankWaiter {
        device: Option<DrmCard>,
    }

    struct DrmCard {
        fd: std::fs::File,
    }

    impl AsFd for DrmCard {
        fn as_fd(&self) -> std::os::fd::BorrowedFd<'_> {
            self.fd.as_fd()
        }
    }

    impl drm::Device for DrmCard {}

    impl DrmCard {
        fn open(pci_bus_id: Option<&str>) -> Option<Self> {
            // Preferred: match via sysfs using adapter PCI bus ID.
            if let Some(bus_id) = pci_bus_id {
                if let Some(card) = Self::open_from_sysfs(bus_id) {
                    return Some(card);
                }
                trace(format_args!(
                    "vblank-drm: sysfs lookup failed for {bus_id}, falling back to enumeration"
                ));
            }
            Self::open_by_enumeration()
        }

        fn open_from_sysfs(pci_bus_id: &str) -> Option<Self> {
            let drm_dir = format!("/sys/bus/pci/devices/{pci_bus_id}/drm");
            let dir = std::fs::read_dir(&drm_dir).ok()?;
            let card_name = dir
                .filter_map(|e| e.ok())
                .find(|e| {
                    e.file_name()
                        .to_str()
                        .is_some_and(|n| n.starts_with("card"))
                })?
                .file_name();
            let path = std::path::Path::new("/dev/dri").join(card_name);
            let fd = std::fs::File::options()
                .read(true)
                .write(true)
                .open(&path)
                .ok()?;
            trace(format_args!(
                "vblank-drm: matched {} via sysfs for PCI {pci_bus_id}",
                path.display()
            ));
            Some(Self { fd })
        }

        fn open_by_enumeration() -> Option<Self> {
            let dir = std::fs::read_dir("/dev/dri").ok()?;
            let mut entries: Vec<_> = dir
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.file_name()
                        .to_str()
                        .is_some_and(|n| n.starts_with("card"))
                })
                .collect();
            entries.sort_by_key(|e| e.file_name());
            for entry in entries {
                if let Ok(fd) = std::fs::File::options()
                    .read(true)
                    .write(true)
                    .open(entry.path())
                {
                    return Some(Self { fd });
                }
            }
            None
        }
    }

    impl VblankWaiter {
        pub fn new(pci_bus_id: Option<&str>) -> Self {
            let device = DrmCard::open(pci_bus_id);
            if device.is_some() {
                trace(format_args!("vblank-drm: opened DRM device"));
            } else {
                trace(format_args!("vblank-drm: no DRM device, falling back to timer"));
            }
            Self { device }
        }

        pub fn wait(&mut self) {
            if let Some(device) = &self.device {
                match drm::Device::wait_vblank(
                    device,
                    drm::VblankWaitTarget::Relative(1),
                    drm::VblankWaitFlags::NEXT_ON_MISS,
                    0,
                ) {
                    Ok(_) => return,
                    Err(_) => {
                        // DRM vblank failed — drop device and fall back permanently.
                        trace(format_args!("vblank-drm: ioctl failed, falling back to timer"));
                        self.device = None;
                    }
                }
            }
            std::thread::sleep(FALLBACK_INTERVAL);
        }
    }
}

// Wayland and other platforms: timer-based fallback.
#[cfg(not(any(
    target_os = "windows",
    all(
        any(target_os = "linux", target_os = "freebsd"),
        any(feature = "x11", feature = "wayland"),
    ),
)))]
mod platform_vblank {
    use super::FALLBACK_INTERVAL;

    pub struct VblankWaiter;

    impl VblankWaiter {
        pub fn new(_pci_bus_id: Option<&str>) -> Self {
            Self
        }

        pub fn wait(&self) {
            std::thread::sleep(FALLBACK_INTERVAL);
        }
    }
}
