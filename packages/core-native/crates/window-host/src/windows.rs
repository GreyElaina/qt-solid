use crate::{
    WindowHostError,
    host::{BackendKind, HostCapabilities, HostIntegration, PumpResult, WaitBridgeKind},
    wake_flag::WakeFlagHost,
};

#[derive(Debug, Default)]
pub(crate) struct WindowsWindowHost {
    wake_host: WakeFlagHost,
}

impl WindowsWindowHost {
    pub(crate) const INTEGRATION: HostIntegration = HostIntegration {
        backend_kind: BackendKind::Windows,
        supports_zero_timeout_pump: true,
        supports_external_wake: true,
        wait_bridge_kind: WaitBridgeKind::None,
    };

    pub(crate) const CAPABILITIES: HostCapabilities = Self::INTEGRATION.capabilities();

    pub(crate) fn new() -> Result<Self, WindowHostError> {
        Ok(Self::default())
    }

    pub(crate) fn backend_kind(&self) -> BackendKind {
        Self::INTEGRATION.backend_kind
    }

    pub(crate) fn integration(&self) -> HostIntegration {
        Self::INTEGRATION
    }

    pub(crate) fn capabilities(&self) -> HostCapabilities {
        self.integration().capabilities()
    }

    pub(crate) fn pump_zero_timeout(&self) -> Result<PumpResult, WindowHostError> {
        Ok(self.wake_host.pump_zero_timeout())
    }

    pub(crate) fn request_wake(&self) {
        self.wake_host.request_wake();
    }
}

#[cfg(test)]
mod tests {
    use super::WindowsWindowHost;
    use crate::host::{BackendKind, HostCapabilities};

    #[test]
    fn windows_backend_reports_supported_capabilities() {
        let host = WindowsWindowHost::new().expect("windows backend should construct");

        assert_eq!(host.backend_kind(), BackendKind::Windows);
        assert_eq!(
            host.capabilities(),
            HostCapabilities {
                backend_kind: BackendKind::Windows,
                supports_zero_timeout_pump: true,
                supports_external_wake: true,
                supports_fd_bridge: false,
            }
        );
        assert!(
            !host
                .pump_zero_timeout()
                .expect("pump should succeed")
                .pumped_native
        );
        host.request_wake();
        assert!(
            host.pump_zero_timeout()
                .expect("wake pump should succeed")
                .pumped_native
        );
    }
}
