mod wayland;
mod x11;

use std::env;

use crate::{
    WindowHostError,
    host::{BackendKind, HostCapabilities, HostIntegration, PumpResult},
};

#[derive(Debug)]
pub(crate) struct LinuxWindowHost {
    backend: LinuxBackend,
}

#[derive(Debug)]
enum LinuxBackend {
    X11(x11::X11WindowHost),
    Wayland(wayland::WaylandWindowHost),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinuxBackendKind {
    X11,
    Wayland,
}

impl LinuxWindowHost {
    pub(crate) fn new() -> Result<Self, WindowHostError> {
        let backend = match detect_backend_kind() {
            LinuxBackendKind::X11 => LinuxBackend::X11(x11::X11WindowHost::new()?),
            LinuxBackendKind::Wayland => LinuxBackend::Wayland(wayland::WaylandWindowHost::new()?),
        };

        Ok(Self { backend })
    }

    pub(crate) fn backend_kind(&self) -> BackendKind {
        match &self.backend {
            LinuxBackend::X11(host) => host.backend_kind(),
            LinuxBackend::Wayland(host) => host.backend_kind(),
        }
    }

    pub(crate) fn capabilities(&self) -> HostCapabilities {
        self.integration().capabilities()
    }

    pub(crate) fn integration(&self) -> HostIntegration {
        match &self.backend {
            LinuxBackend::X11(host) => host.integration(),
            LinuxBackend::Wayland(host) => host.integration(),
        }
    }

    pub(crate) fn pump_zero_timeout(&self) -> Result<PumpResult, WindowHostError> {
        match &self.backend {
            LinuxBackend::X11(host) => host.pump_zero_timeout(),
            LinuxBackend::Wayland(host) => host.pump_zero_timeout(),
        }
    }

    pub(crate) fn request_wake(&self) {
        match &self.backend {
            LinuxBackend::X11(host) => host.request_wake(),
            LinuxBackend::Wayland(host) => host.request_wake(),
        }
    }
}

pub(crate) fn detected_backend_kind() -> BackendKind {
    match detect_backend_kind() {
        LinuxBackendKind::X11 => BackendKind::LinuxX11,
        LinuxBackendKind::Wayland => BackendKind::LinuxWayland,
    }
}

pub(crate) fn detected_capabilities() -> HostCapabilities {
    detected_integration().capabilities()
}

pub(crate) fn detected_integration() -> HostIntegration {
    match detect_backend_kind() {
        LinuxBackendKind::X11 => x11::X11WindowHost::INTEGRATION,
        LinuxBackendKind::Wayland => wayland::WaylandWindowHost::INTEGRATION,
    }
}

fn detect_backend_kind() -> LinuxBackendKind {
    if let Some(kind) = env::var_os("QT_QPA_PLATFORM").and_then(|value| parse_qpa_backend(&value)) {
        return kind;
    }

    if env::var_os("WAYLAND_DISPLAY").is_some() {
        return LinuxBackendKind::Wayland;
    }

    LinuxBackendKind::X11
}

fn parse_qpa_backend(value: &std::ffi::OsStr) -> Option<LinuxBackendKind> {
    let value = value.to_string_lossy();
    let normalized = value
        .split_once(':')
        .map(|(kind, _)| kind)
        .unwrap_or(&value);

    if normalized.eq_ignore_ascii_case("wayland") || normalized.starts_with("wayland") {
        return Some(LinuxBackendKind::Wayland);
    }

    if normalized.eq_ignore_ascii_case("xcb")
        || normalized.eq_ignore_ascii_case("x11")
        || normalized.starts_with("xcb")
        || normalized.starts_with("x11")
    {
        return Some(LinuxBackendKind::X11);
    }

    None
}

#[cfg(test)]
mod tests {
    use std::{env, ffi::OsStr, sync::Mutex};

    use super::{
        LinuxBackendKind, LinuxWindowHost, detected_backend_kind, detected_capabilities,
        detected_integration, parse_qpa_backend, wayland, x11,
    };
    use crate::host::{BackendKind, HostCapabilities, HostIntegration, WaitBridgeKind};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn with_linux_env(
        qpa_platform: Option<&str>,
        wayland_display: Option<&str>,
        test: impl FnOnce(),
    ) {
        let _guard = ENV_LOCK.lock().expect("linux env lock");
        let original_qpa_platform = env::var_os("QT_QPA_PLATFORM");
        let original_wayland_display = env::var_os("WAYLAND_DISPLAY");

        unsafe {
            match qpa_platform {
                Some(value) => env::set_var("QT_QPA_PLATFORM", value),
                None => env::remove_var("QT_QPA_PLATFORM"),
            }
        }
        unsafe {
            match wayland_display {
                Some(value) => env::set_var("WAYLAND_DISPLAY", value),
                None => env::remove_var("WAYLAND_DISPLAY"),
            }
        }

        test();

        unsafe {
            match original_qpa_platform {
                Some(value) => env::set_var("QT_QPA_PLATFORM", value),
                None => env::remove_var("QT_QPA_PLATFORM"),
            }
        }
        unsafe {
            match original_wayland_display {
                Some(value) => env::set_var("WAYLAND_DISPLAY", value),
                None => env::remove_var("WAYLAND_DISPLAY"),
            }
        }
    }

    #[test]
    fn parses_wayland_qpa_variants() {
        assert_eq!(
            parse_qpa_backend(OsStr::new("wayland")),
            Some(LinuxBackendKind::Wayland)
        );
        assert_eq!(
            parse_qpa_backend(OsStr::new("wayland-egl")),
            Some(LinuxBackendKind::Wayland)
        );
        assert_eq!(
            parse_qpa_backend(OsStr::new("wayland:decoration=client")),
            Some(LinuxBackendKind::Wayland)
        );
    }

    #[test]
    fn parses_x11_qpa_variants() {
        assert_eq!(
            parse_qpa_backend(OsStr::new("xcb")),
            Some(LinuxBackendKind::X11)
        );
        assert_eq!(
            parse_qpa_backend(OsStr::new("x11")),
            Some(LinuxBackendKind::X11)
        );
        assert_eq!(
            parse_qpa_backend(OsStr::new("xcb:force-font-dpi=96")),
            Some(LinuxBackendKind::X11)
        );
    }

    #[test]
    fn linux_backend_capabilities_match_coalesced_wake_model() {
        assert_eq!(
            x11::X11WindowHost::CAPABILITIES,
            HostCapabilities {
                backend_kind: BackendKind::LinuxX11,
                supports_zero_timeout_pump: true,
                supports_external_wake: true,
                supports_fd_bridge: true,
            }
        );
        assert_eq!(
            wayland::WaylandWindowHost::CAPABILITIES,
            HostCapabilities {
                backend_kind: BackendKind::LinuxWayland,
                supports_zero_timeout_pump: true,
                supports_external_wake: true,
                supports_fd_bridge: true,
            }
        );
    }

    #[test]
    fn linux_integrations_report_unix_fd_wait_bridge_contract() {
        assert_eq!(
            x11::X11WindowHost::INTEGRATION,
            HostIntegration {
                backend_kind: BackendKind::LinuxX11,
                supports_zero_timeout_pump: true,
                supports_external_wake: true,
                wait_bridge_kind: WaitBridgeKind::UnixFd,
            }
        );
        assert_eq!(
            wayland::WaylandWindowHost::INTEGRATION,
            HostIntegration {
                backend_kind: BackendKind::LinuxWayland,
                supports_zero_timeout_pump: true,
                supports_external_wake: true,
                wait_bridge_kind: WaitBridgeKind::UnixFd,
            }
        );
    }

    #[test]
    fn x11_backend_constructs_and_drains_wake_flag() {
        with_linux_env(Some("xcb"), None, || {
            let host = LinuxWindowHost::new().expect("x11 backend should construct");

            assert_eq!(host.backend_kind(), BackendKind::LinuxX11);
            assert_eq!(detected_backend_kind(), BackendKind::LinuxX11);
            assert_eq!(host.capabilities(), x11::X11WindowHost::CAPABILITIES);
            assert_eq!(detected_capabilities(), x11::X11WindowHost::CAPABILITIES);
            assert_eq!(host.integration(), x11::X11WindowHost::INTEGRATION);
            assert_eq!(detected_integration(), x11::X11WindowHost::INTEGRATION);

            assert!(
                !host
                    .pump_zero_timeout()
                    .expect("initial pump should succeed")
                    .pumped_native
            );
            host.request_wake();
            assert!(
                host.pump_zero_timeout()
                    .expect("wake pump should succeed")
                    .pumped_native
            );
            assert!(
                !host
                    .pump_zero_timeout()
                    .expect("wake should drain")
                    .pumped_native
            );
        });
    }

    #[test]
    fn wayland_backend_constructs_and_drains_wake_flag() {
        with_linux_env(Some("wayland"), Some("wayland-0"), || {
            let host = LinuxWindowHost::new().expect("wayland backend should construct");

            assert_eq!(host.backend_kind(), BackendKind::LinuxWayland);
            assert_eq!(detected_backend_kind(), BackendKind::LinuxWayland);
            assert_eq!(
                host.capabilities(),
                wayland::WaylandWindowHost::CAPABILITIES
            );
            assert_eq!(
                detected_capabilities(),
                wayland::WaylandWindowHost::CAPABILITIES
            );
            assert_eq!(host.integration(), wayland::WaylandWindowHost::INTEGRATION);
            assert_eq!(
                detected_integration(),
                wayland::WaylandWindowHost::INTEGRATION
            );

            assert!(
                !host
                    .pump_zero_timeout()
                    .expect("initial pump should succeed")
                    .pumped_native
            );
            host.request_wake();
            assert!(
                host.pump_zero_timeout()
                    .expect("wake pump should succeed")
                    .pumped_native
            );
            assert!(
                !host
                    .pump_zero_timeout()
                    .expect("wake should drain")
                    .pumped_native
            );
        });
    }
}
