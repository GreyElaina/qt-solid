use crate::{
    WindowHostError,
    host::{BackendKind, HostCapabilities, HostIntegration, PumpResult, WaitBridgeKind},
    wake_flag::WakeFlagHost,
};

#[derive(Debug, Default)]
pub(super) struct WaylandWindowHost {
    // Qt/Wayland keeps ownership of the compositor connection and dispatch.
    // This backend only tracks wake intent, avoiding a second calloop/wayland
    // stack inside the host bridge.
    wake_host: WakeFlagHost,
}

impl WaylandWindowHost {
    pub(super) const INTEGRATION: HostIntegration = HostIntegration {
        backend_kind: BackendKind::LinuxWayland,
        supports_zero_timeout_pump: true,
        supports_external_wake: true,
        // The actual Wayland display fd is runtime-owned and becomes available
        // only after the Qt host has constructed QGuiApplication.
        wait_bridge_kind: WaitBridgeKind::UnixFd,
    };

    pub(super) const CAPABILITIES: HostCapabilities = Self::INTEGRATION.capabilities();

    pub(super) fn new() -> Result<Self, WindowHostError> {
        Ok(Self::default())
    }

    pub(super) fn backend_kind(&self) -> BackendKind {
        Self::INTEGRATION.backend_kind
    }

    pub(super) fn integration(&self) -> HostIntegration {
        Self::INTEGRATION
    }

    pub(super) fn capabilities(&self) -> HostCapabilities {
        self.integration().capabilities()
    }

    pub(super) fn pump_zero_timeout(&self) -> Result<PumpResult, WindowHostError> {
        Ok(self.wake_host.pump_zero_timeout())
    }

    pub(super) fn request_wake(&self) {
        self.wake_host.request_wake();
    }
}
