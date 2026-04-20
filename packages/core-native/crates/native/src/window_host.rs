use std::cell::RefCell;

use napi::{Error, Result, Status};
use window_host::{
    HostCapabilities, HostIntegration, WaitBridgeKind, WindowHost,
    detected_backend_kind as detected_backend_kind_impl,
    detected_capabilities as detected_capabilities_impl,
    detected_integration as detected_integration_impl,
};

thread_local! {
    static WINDOW_HOST: RefCell<Option<WindowHost>> = const { RefCell::new(None) };
}

fn runtime_wait_bridge_kind() -> Option<WaitBridgeKind> {
    match crate::qt::qt_runtime_wait_bridge_kind_tag() {
        0 => None,
        1 => Some(WaitBridgeKind::UnixFd),
        2 => Some(WaitBridgeKind::WindowsHandle),
        _ => None,
    }
}

fn integration_with_runtime_wait_bridge(
    integration: HostIntegration,
    runtime_wait_bridge_kind: Option<WaitBridgeKind>,
) -> HostIntegration {
    let Some(wait_bridge_kind) = runtime_wait_bridge_kind else {
        return integration;
    };

    HostIntegration {
        wait_bridge_kind,
        ..integration
    }
}

fn effective_integration(integration: HostIntegration) -> HostIntegration {
    integration_with_runtime_wait_bridge(integration, runtime_wait_bridge_kind())
}

pub(crate) fn enabled() -> bool {
    true
}

pub(crate) fn start() -> Result<()> {
    WINDOW_HOST.with(|slot| {
        if slot.borrow().is_some() {
            return Ok(());
        }

        let host = WindowHost::new().map_err(|error| {
            Error::new(
                Status::GenericFailure,
                format!("failed to create window-host pump: {error}"),
            )
        })?;
        *slot.borrow_mut() = Some(host);
        Ok(())
    })
}

pub(crate) fn stop() {
    WINDOW_HOST.with(|slot| {
        let Ok(mut slot) = slot.try_borrow_mut() else {
            return;
        };
        let host = slot.take();
        drop(slot);
        if let Some(host) = host.as_ref() {
            host.request_wake();
        }
    });
}

pub(crate) fn pump_zero_timeout() -> bool {
    WINDOW_HOST.with(|slot| {
        let Ok(slot) = slot.try_borrow() else {
            return false;
        };
        let Some(host) = slot.as_ref() else {
            return false;
        };

        host.pump_zero_timeout().unwrap_or(false)
    })
}

pub(crate) fn request_wake() {
    WINDOW_HOST.with(|slot| {
        let Ok(slot) = slot.try_borrow() else {
            return;
        };
        if let Some(host) = slot.as_ref() {
            host.request_wake();
        }
    });
}

pub(crate) fn request_native_wait_once() {
    WINDOW_HOST.with(|slot| {
        let Ok(slot) = slot.try_borrow() else {
            return;
        };
        if let Some(host) = slot.as_ref() {
            host.request_native_wait_once();
        }
    });
}

pub(crate) fn notify_native_frame_source() {
    WINDOW_HOST.with(|slot| {
        let Ok(slot) = slot.try_borrow() else {
            return;
        };
        if let Some(host) = slot.as_ref() {
            host.notify_native_frame_source();
        }
    });
}

pub(crate) fn backend_name() -> Option<String> {
    WINDOW_HOST.with(|slot| {
        let Ok(slot) = slot.try_borrow() else {
            return None;
        };
        slot.as_ref().map(|host| host.backend_name().to_owned())
    })
}

pub(crate) fn capabilities() -> Option<HostCapabilities> {
    integration().map(HostIntegration::capabilities)
}

pub(crate) fn integration() -> Option<HostIntegration> {
    WINDOW_HOST.with(|slot| {
        let Ok(slot) = slot.try_borrow() else {
            return None;
        };
        slot.as_ref()
            .map(|host| effective_integration(host.integration()))
    })
}

pub(crate) fn detected_backend_name() -> String {
    let backend: &'static str = detected_backend_kind_impl().into();
    backend.to_owned()
}

pub(crate) fn detected_capabilities() -> HostCapabilities {
    detected_capabilities_impl()
}

pub(crate) fn detected_integration() -> HostIntegration {
    effective_integration(detected_integration_impl())
}

pub(crate) fn supports_zero_timeout_pump() -> bool {
    integration()
        .unwrap_or_else(detected_integration)
        .supports_zero_timeout_pump
}

pub(crate) fn supports_external_wake() -> bool {
    integration()
        .unwrap_or_else(detected_integration)
        .supports_external_wake
}

pub(crate) fn wait_bridge_kind_tag() -> u8 {
    match integration()
        .unwrap_or_else(detected_integration)
        .wait_bridge_kind
    {
        WaitBridgeKind::None => 0,
        WaitBridgeKind::UnixFd => 1,
        WaitBridgeKind::WindowsHandle => 2,
    }
}

pub(crate) fn wait_bridge_unix_fd() -> i32 {
    match integration()
        .unwrap_or_else(detected_integration)
        .wait_bridge_kind
    {
        WaitBridgeKind::UnixFd => crate::qt::qt_runtime_wait_bridge_unix_fd(),
        WaitBridgeKind::None | WaitBridgeKind::WindowsHandle => -1,
    }
}

pub(crate) fn wait_bridge_windows_handle() -> u64 {
    match integration()
        .unwrap_or_else(detected_integration)
        .wait_bridge_kind
    {
        WaitBridgeKind::WindowsHandle => crate::qt::qt_runtime_wait_bridge_windows_handle(),
        WaitBridgeKind::None | WaitBridgeKind::UnixFd => 0,
    }
}

pub(crate) fn ffi_pump_zero_timeout() -> bool {
    pump_zero_timeout()
}

pub(crate) fn ffi_request_wake() {
    request_wake();
}

pub(crate) fn ffi_request_native_wait_once() {
    request_native_wait_once();
}

#[unsafe(no_mangle)]
pub extern "C" fn qt_solid_notify_native_frame_source() {
    notify_native_frame_source();
}

pub(crate) fn ffi_supports_zero_timeout_pump() -> bool {
    supports_zero_timeout_pump()
}

pub(crate) fn ffi_supports_external_wake() -> bool {
    supports_external_wake()
}

pub(crate) fn ffi_wait_bridge_kind_tag() -> u8 {
    wait_bridge_kind_tag()
}

pub(crate) fn ffi_wait_bridge_unix_fd() -> i32 {
    wait_bridge_unix_fd()
}

pub(crate) fn ffi_wait_bridge_windows_handle() -> u64 {
    wait_bridge_windows_handle()
}

#[cfg(test)]
mod tests {
    use super::{
        detected_capabilities, detected_integration, integration_with_runtime_wait_bridge,
        supports_external_wake, supports_zero_timeout_pump, wait_bridge_kind_tag,
        wait_bridge_unix_fd, wait_bridge_windows_handle,
    };
    use window_host::{BackendKind, HostIntegration, WaitBridgeKind};

    #[test]
    fn detected_capabilities_stay_derived_from_detected_integration() {
        assert_eq!(
            detected_capabilities(),
            detected_integration().capabilities()
        );
    }

    #[test]
    fn helper_flags_match_detected_integration_before_start() {
        let integration = detected_integration();

        assert_eq!(
            supports_zero_timeout_pump(),
            integration.supports_zero_timeout_pump
        );
        assert_eq!(supports_external_wake(), integration.supports_external_wake);
    }

    #[test]
    fn wait_bridge_tag_matches_detected_integration_before_start() {
        let expected = match detected_integration().wait_bridge_kind {
            WaitBridgeKind::None => 0,
            WaitBridgeKind::UnixFd => 1,
            WaitBridgeKind::WindowsHandle => 2,
        };

        assert_eq!(wait_bridge_kind_tag(), expected);
    }

    #[test]
    fn wait_bridge_payloads_are_sentinel_before_qt_runtime_starts() {
        let expected = match detected_integration().wait_bridge_kind {
            WaitBridgeKind::None => 0,
            WaitBridgeKind::UnixFd => 1,
            WaitBridgeKind::WindowsHandle => 2,
        };

        assert_eq!(wait_bridge_kind_tag(), expected);
        assert_eq!(wait_bridge_unix_fd(), -1);
        assert_eq!(wait_bridge_windows_handle(), 0);
    }

    #[test]
    fn runtime_wait_bridge_overlay_updates_legacy_capabilities_view() {
        let integration = HostIntegration {
            backend_kind: BackendKind::LinuxX11,
            supports_zero_timeout_pump: true,
            supports_external_wake: true,
            wait_bridge_kind: WaitBridgeKind::None,
        };

        let effective =
            integration_with_runtime_wait_bridge(integration, Some(WaitBridgeKind::UnixFd));

        assert_eq!(effective.wait_bridge_kind, WaitBridgeKind::UnixFd);
        assert!(effective.capabilities().supports_fd_bridge);
    }
}
