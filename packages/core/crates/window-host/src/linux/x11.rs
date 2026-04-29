use crate::{
    WindowHostError,
    host::{BackendKind, HostCapabilities, HostIntegration, PumpResult, WaitBridgeKind},
    wake_flag::WakeFlagHost,
};

#[derive(Debug, Default)]
pub(super) struct X11WindowHost {
    // Qt/xcb keeps ownership of the actual native event queue.
    // We only preserve host-side wake intent here, so the outer runtime can
    // coalesce a pending pump request without introducing a second X11 loop.
    wake_host: WakeFlagHost,
}

impl X11WindowHost {
    pub(super) const INTEGRATION: HostIntegration = HostIntegration {
        backend_kind: BackendKind::LinuxX11,
        supports_zero_timeout_pump: true,
        supports_external_wake: true,
        // The actual X11 connection fd is owned by the Qt runtime and exported
        // by the outer native host layer once QGuiApplication is running.
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
