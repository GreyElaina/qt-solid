mod host;
#[cfg(any(target_os = "linux", test))]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(any(target_os = "linux", target_os = "windows", test))]
mod wake_flag;
#[cfg(any(target_os = "windows", test))]
mod windows;

use std::{error::Error, fmt};

pub use host::{BackendKind, HostCapabilities, HostIntegration, WaitBridgeKind};
#[cfg(target_os = "macos")]
pub use macos::NativeFrameNotifier;

#[cfg(target_os = "linux")]
type Backend = linux::LinuxWindowHost;
#[cfg(target_os = "macos")]
type Backend = macos::MacosWindowHost;
#[cfg(target_os = "windows")]
type Backend = windows::WindowsWindowHost;

#[derive(Debug, Clone)]
pub struct WindowHostError {
    message: String,
}

impl WindowHostError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for WindowHostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(f)
    }
}

impl Error for WindowHostError {}

pub struct WindowHost {
    inner: Backend,
}

pub fn detected_backend_kind() -> BackendKind {
    detect_backend_kind()
}

pub fn detected_integration() -> HostIntegration {
    detect_integration()
}

pub fn detected_capabilities() -> HostCapabilities {
    detected_integration().capabilities()
}

impl WindowHost {
    pub fn new() -> Result<Self, WindowHostError> {
        Ok(Self {
            inner: create_backend()?,
        })
    }

    pub fn backend_kind(&self) -> BackendKind {
        self.inner.backend_kind()
    }

    pub fn backend_name(&self) -> &'static str {
        self.backend_kind().into()
    }

    pub fn integration(&self) -> HostIntegration {
        self.inner.integration()
    }

    pub fn capabilities(&self) -> HostCapabilities {
        self.integration().capabilities()
    }

    pub fn pump_zero_timeout(&self) -> Result<bool, WindowHostError> {
        Ok(self.inner.pump_zero_timeout()?.pumped_native)
    }

    pub fn request_wake(&self) {
        self.inner.request_wake();
    }


    #[cfg(target_os = "macos")]
    pub fn native_frame_notifier(&self) -> NativeFrameNotifier {
        self.inner.native_frame_notifier()
    }

    /// Return the raw Win32 event HANDLE for the wait bridge.
    /// Only meaningful when `wait_bridge_kind` is `WindowsHandle`.
    #[cfg(target_os = "windows")]
    pub fn bridge_event_handle(&self) -> u64 {
        self.inner.bridge_event_handle()
    }
}

#[cfg(target_os = "macos")]
fn create_backend() -> Result<Backend, WindowHostError> {
    macos::MacosWindowHost::new()
}

#[cfg(target_os = "macos")]
fn detect_backend_kind() -> BackendKind {
    BackendKind::Macos
}

#[cfg(target_os = "macos")]
fn detect_integration() -> HostIntegration {
    macos::MacosWindowHost::INTEGRATION
}

#[cfg(target_os = "windows")]
fn create_backend() -> Result<Backend, WindowHostError> {
    windows::WindowsWindowHost::new()
}

#[cfg(target_os = "windows")]
fn detect_backend_kind() -> BackendKind {
    BackendKind::Windows
}

#[cfg(target_os = "windows")]
fn detect_integration() -> HostIntegration {
    windows::WindowsWindowHost::INTEGRATION
}

#[cfg(target_os = "linux")]
fn create_backend() -> Result<Backend, WindowHostError> {
    linux::LinuxWindowHost::new()
}

#[cfg(target_os = "linux")]
fn detect_backend_kind() -> BackendKind {
    linux::detected_backend_kind()
}

#[cfg(target_os = "linux")]
fn detect_integration() -> HostIntegration {
    linux::detected_integration()
}
