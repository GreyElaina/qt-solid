use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BackendKind {
    Macos,
    LinuxX11,
    LinuxWayland,
    Windows,
}

impl From<BackendKind> for &'static str {
    fn from(value: BackendKind) -> Self {
        match value {
            BackendKind::Macos => "macos",
            BackendKind::LinuxX11 => "linux-x11",
            BackendKind::LinuxWayland => "linux-wayland",
            BackendKind::Windows => "windows",
        }
    }
}

impl fmt::Display for BackendKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str((*self).into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WaitBridgeKind {
    None,
    UnixFd,
    WindowsHandle,
}

impl WaitBridgeKind {
    pub const fn supports_fd_bridge(self) -> bool {
        matches!(self, Self::UnixFd)
    }
}

impl From<WaitBridgeKind> for &'static str {
    fn from(value: WaitBridgeKind) -> Self {
        match value {
            WaitBridgeKind::None => "none",
            WaitBridgeKind::UnixFd => "unix-fd",
            WaitBridgeKind::WindowsHandle => "windows-handle",
        }
    }
}

impl fmt::Display for WaitBridgeKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str((*self).into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HostIntegration {
    pub backend_kind: BackendKind,
    pub supports_zero_timeout_pump: bool,
    pub supports_external_wake: bool,
    pub wait_bridge_kind: WaitBridgeKind,
}

impl HostIntegration {
    pub const fn capabilities(self) -> HostCapabilities {
        HostCapabilities {
            backend_kind: self.backend_kind,
            supports_zero_timeout_pump: self.supports_zero_timeout_pump,
            supports_external_wake: self.supports_external_wake,
            supports_fd_bridge: self.wait_bridge_kind.supports_fd_bridge(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HostCapabilities {
    pub backend_kind: BackendKind,
    pub supports_zero_timeout_pump: bool,
    pub supports_external_wake: bool,
    pub supports_fd_bridge: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PumpResult {
    pub(crate) pumped_native: bool,
}

#[cfg(test)]
mod tests {
    use super::{BackendKind, HostCapabilities, HostIntegration, WaitBridgeKind};

    #[test]
    fn only_unix_fd_reports_fd_bridge_support() {
        assert!(!WaitBridgeKind::None.supports_fd_bridge());
        assert!(WaitBridgeKind::UnixFd.supports_fd_bridge());
        assert!(!WaitBridgeKind::WindowsHandle.supports_fd_bridge());
    }

    #[test]
    fn integration_derives_legacy_capabilities_view() {
        let integration = HostIntegration {
            backend_kind: BackendKind::LinuxX11,
            supports_zero_timeout_pump: true,
            supports_external_wake: false,
            wait_bridge_kind: WaitBridgeKind::UnixFd,
        };

        assert_eq!(
            integration.capabilities(),
            HostCapabilities {
                backend_kind: BackendKind::LinuxX11,
                supports_zero_timeout_pump: true,
                supports_external_wake: false,
                supports_fd_bridge: true,
            }
        );
    }
}
