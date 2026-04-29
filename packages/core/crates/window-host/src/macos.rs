use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

use objc2::MainThreadMarker;
use objc2_core_foundation::{CFRetained, CFRunLoop};

use crate::{
    WindowHostError,
    host::{BackendKind, HostIntegration, PumpResult, WaitBridgeKind},
};


/// Thread-safe handle for display-link callback to signal the main run loop.
/// Cloneable and `Send + Sync` — safe to store in display-link delegate ivars.
#[derive(Clone)]
pub struct NativeFrameNotifier {
    flag: Arc<AtomicBool>,
    run_loop: CFRetained<CFRunLoop>,
}

impl std::fmt::Debug for NativeFrameNotifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeFrameNotifier")
            .field("delivered", &self.flag.load(Ordering::Relaxed))
            .finish()
    }
}

// SAFETY: CFRunLoop is thread-safe (CFRunLoopWakeUp is documented as callable
// from any thread). AtomicBool + Arc are inherently Send+Sync.
unsafe impl Send for NativeFrameNotifier {}
unsafe impl Sync for NativeFrameNotifier {}

impl NativeFrameNotifier {
    /// Called from display-link thread to wake the main run loop.
    /// CFRunLoopWakeUp causes `CFRunLoopRunInMode` (called in uv_prepare)
    /// to return early, which is sufficient now that we no longer use
    /// `[NSApplication run]`.
    pub fn notify(&self) {
        self.flag.store(true, Ordering::Release);
        self.run_loop.wake_up();
    }

    /// Check if notified since last reset.
    pub fn delivered(&self) -> bool {
        self.flag.load(Ordering::Acquire)
    }

    fn reset(&self) {
        self.flag.store(false, Ordering::Release);
    }
}


fn trace_enabled() -> bool {
    static ENABLED: std::sync::LazyLock<bool> =
        std::sync::LazyLock::new(|| std::env::var_os("QT_SOLID_WGPU_TRACE").is_some());
    *ENABLED
}

fn trace(args: std::fmt::Arguments<'_>) {
    if !trace_enabled() {
        return;
    }
    println!("[qt-window-host] {args}");
}

#[derive(Debug)]
pub(crate) struct MacosWindowHost {
    notifier: NativeFrameNotifier,
    main_run_loop: CFRetained<CFRunLoop>,
}

impl MacosWindowHost {
    pub(crate) const INTEGRATION: HostIntegration = HostIntegration {
        backend_kind: BackendKind::Macos,
        supports_zero_timeout_pump: true,
        supports_external_wake: true,
        wait_bridge_kind: WaitBridgeKind::None,
    };

    pub(crate) fn new() -> Result<Self, WindowHostError> {
        let _mtm = MainThreadMarker::new()
            .ok_or_else(|| WindowHostError::new("window-host must start on macOS main thread"))?;
        let main_run_loop = CFRunLoop::main()
            .ok_or_else(|| WindowHostError::new("failed to get macOS main CFRunLoop"))?;

        let notifier = NativeFrameNotifier {
            flag: Arc::new(AtomicBool::new(false)),
            run_loop: main_run_loop.clone(),
        };

        Ok(Self {
            notifier,
            main_run_loop,
        })
    }

    pub(crate) fn backend_kind(&self) -> BackendKind {
        Self::INTEGRATION.backend_kind
    }

    pub(crate) fn integration(&self) -> HostIntegration {
        Self::INTEGRATION
    }

    /// No-op on macOS. Native source dispatch is handled by
    /// `CFRunLoopRunInMode` in the C++ `on_prepare` callback.
    pub(crate) fn pump_zero_timeout(&self) -> Result<PumpResult, WindowHostError> {
        self.notifier.reset();
        Ok(PumpResult {
            pumped_native: true,
        })
    }

    pub(crate) fn request_wake(&self) {
        trace(format_args!("request-wake"));
        self.main_run_loop.wake_up();
    }

    /// Return a cloned notifier handle for the display-link callback.
    pub(crate) fn native_frame_notifier(&self) -> NativeFrameNotifier {
        self.notifier.clone()
    }
}

#[cfg(test)]
mod tests {
    // Intentionally minimal — the heavy integration testing happens at the
    // uv_pump / display-link level. Unit tests here would require a running
    // CFRunLoop which is impractical in cargo test.
}
