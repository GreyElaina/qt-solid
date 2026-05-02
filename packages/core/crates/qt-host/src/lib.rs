use std::sync::OnceLock;

mod ffi;

pub use window_host::{
    BackendKind, HostCapabilities, HostIntegration, WaitBridgeKind, WindowHost, WindowHostError,
};
#[cfg(target_os = "macos")]
pub use window_host::NativeFrameNotifier;

/// Process-global WindowHost instance. Initialized once by `start()`, never
/// dropped (symmetric with the C++ QtHostState which also outlives shutdown).
/// This is safe because WindowHost's methods (request_wake, pump_zero_timeout)
/// are internally thread-safe, and the instance holds OS resources (pipe fds,
/// event handles) that are reclaimed at process exit.
static WINDOW_HOST: OnceLock<WindowHost> = OnceLock::new();

pub fn start() -> Result<(), WindowHostError> {
    if WINDOW_HOST.get().is_some() {
        return Ok(());
    }
    let host = WindowHost::new()?;
    let _ = WINDOW_HOST.set(host);
    Ok(())
}

pub fn pump_zero_timeout() -> bool {
    WINDOW_HOST
        .get()
        .and_then(|host| host.pump_zero_timeout().ok())
        .unwrap_or(false)
}

pub fn request_wake() {
    if let Some(host) = WINDOW_HOST.get() {
        host.request_wake();
    }
}

pub fn backend_name() -> Option<String> {
    WINDOW_HOST
        .get()
        .map(|host| host.backend_name().to_owned())
}

pub fn capabilities() -> Option<HostCapabilities> {
    integration().map(HostIntegration::capabilities)
}

pub fn integration() -> Option<HostIntegration> {
    WINDOW_HOST
        .get()
        .map(|host| effective_integration(host.integration()))
}

pub fn detected_backend_name() -> String {
    let backend: &'static str = window_host::detected_backend_kind().into();
    backend.to_owned()
}

pub fn detected_capabilities() -> HostCapabilities {
    window_host::detected_capabilities()
}

pub fn detected_integration() -> HostIntegration {
    effective_integration(window_host::detected_integration())
}

pub fn supports_zero_timeout_pump() -> bool {
    integration()
        .unwrap_or_else(detected_integration)
        .supports_zero_timeout_pump
}

pub fn supports_external_wake() -> bool {
    integration()
        .unwrap_or_else(detected_integration)
        .supports_external_wake
}

pub fn wait_bridge_windows_handle() -> u64 {
    #[cfg(target_os = "windows")]
    {
        WINDOW_HOST
            .get()
            .map(|host| host.bridge_event_handle())
            .unwrap_or(0)
    }
    #[cfg(not(target_os = "windows"))]
    {
        0
    }
}

#[cfg(target_os = "macos")]
#[unsafe(no_mangle)]
pub extern "C" fn qt_solid_native_frame_notifier() -> *const std::ffi::c_void {
    native_frame_notifier()
}

#[cfg(target_os = "macos")]
pub fn native_frame_notifier() -> *const std::ffi::c_void {
    use std::ffi::c_void;
    static NOTIFIER_ADDR: OnceLock<usize> = OnceLock::new();
    let addr = *NOTIFIER_ADDR.get_or_init(|| {
        let host = WINDOW_HOST
            .get()
            .expect("native_frame_notifier called before WindowHost is initialized");
        let notifier = host.native_frame_notifier();
        Box::into_raw(Box::new(notifier)) as usize
    });
    addr as *const c_void
}

// -- Internal helpers --

fn runtime_wait_bridge_kind() -> Option<WaitBridgeKind> {
    // C++ side handles runtime wait bridge detection directly.
    None
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
