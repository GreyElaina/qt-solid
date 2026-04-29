use std::sync::atomic::{AtomicBool, Ordering};

use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE},
    System::Threading::{CreateEventW, ResetEvent, SetEvent},
};

use crate::{
    WindowHostError,
    host::{BackendKind, HostIntegration, PumpResult, WaitBridgeKind},
};

#[derive(Debug)]
pub(crate) struct WindowsWindowHost {
    bridge_event: HANDLE,
    wake_pending: AtomicBool,
}

// SAFETY: Win32 manual-reset events are thread-safe kernel objects.
// SetEvent / ResetEvent are safe to call from any thread.
unsafe impl Send for WindowsWindowHost {}
unsafe impl Sync for WindowsWindowHost {}

impl WindowsWindowHost {
    pub(crate) const INTEGRATION: HostIntegration = HostIntegration {
        backend_kind: BackendKind::Windows,
        supports_zero_timeout_pump: true,
        supports_external_wake: true,
        wait_bridge_kind: WaitBridgeKind::WindowsHandle,
    };

    pub(crate) fn new() -> Result<Self, WindowHostError> {
        // Manual-reset event, initially non-signaled.
        let handle =
            unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
        if handle == INVALID_HANDLE_VALUE || handle.is_null() {
            return Err(WindowHostError::new(
                "CreateEventW failed for wait bridge event",
            ));
        }

        Ok(Self {
            bridge_event: handle,
            wake_pending: AtomicBool::new(false),
        })
    }

    pub(crate) fn backend_kind(&self) -> BackendKind {
        Self::INTEGRATION.backend_kind
    }

    pub(crate) fn integration(&self) -> HostIntegration {
        Self::INTEGRATION
    }

    pub(crate) fn pump_zero_timeout(&self) -> Result<PumpResult, WindowHostError> {
        let had_pending = self.wake_pending.swap(false, Ordering::AcqRel);
        if had_pending {
            unsafe { ResetEvent(self.bridge_event) };
        }
        Ok(PumpResult {
            pumped_native: had_pending,
        })
    }

    pub(crate) fn request_wake(&self) {
        self.wake_pending.store(true, Ordering::Release);
        unsafe { SetEvent(self.bridge_event) };
    }

    /// Return the raw HANDLE for the wait bridge event.
    /// Valid for the lifetime of this host. Caller must not close it.
    pub(crate) fn bridge_event_handle(&self) -> u64 {
        self.bridge_event as usize as u64
    }
}

impl Drop for WindowsWindowHost {
    fn drop(&mut self) {
        if self.bridge_event != INVALID_HANDLE_VALUE && !self.bridge_event.is_null() {
            unsafe { CloseHandle(self.bridge_event) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::WindowsWindowHost;
    use crate::host::{BackendKind, WaitBridgeKind};

    #[test]
    fn windows_backend_reports_wait_bridge() {
        let host = WindowsWindowHost::new().expect("windows backend should construct");

        assert_eq!(host.backend_kind(), BackendKind::Windows);
        assert_eq!(
            host.integration().wait_bridge_kind,
            WaitBridgeKind::WindowsHandle
        );
        assert!(host.bridge_event_handle() != 0);
    }

    #[test]
    fn wake_then_pump_drains_event() {
        let host = WindowsWindowHost::new().expect("should construct");

        assert!(!host.pump_zero_timeout().unwrap().pumped_native);

        host.request_wake();
        host.request_wake();
        assert!(host.pump_zero_timeout().unwrap().pumped_native);
        assert!(!host.pump_zero_timeout().unwrap().pumped_native);
    }
}
