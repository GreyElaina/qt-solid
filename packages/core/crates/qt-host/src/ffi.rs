/// CXX bridge for qt-host ↔ C++ host code.
///
/// The extern "Rust" block exposes window-host functions to C++ (LibuvQtPump).
#[cxx::bridge(namespace = "qt_solid::host")]
pub(crate) mod bridge {
    extern "Rust" {
        fn window_host_pump_zero_timeout() -> bool;
        fn window_host_supports_zero_timeout_pump() -> bool;
        fn window_host_supports_external_wake() -> bool;
        fn window_host_wait_bridge_windows_handle() -> u64;
        fn window_host_request_wake();
    }
}

pub(crate) fn window_host_pump_zero_timeout() -> bool {
    crate::pump_zero_timeout()
}

pub(crate) fn window_host_supports_zero_timeout_pump() -> bool {
    crate::supports_zero_timeout_pump()
}

pub(crate) fn window_host_supports_external_wake() -> bool {
    crate::supports_external_wake()
}

pub(crate) fn window_host_wait_bridge_windows_handle() -> u64 {
    crate::wait_bridge_windows_handle()
}

pub(crate) fn window_host_request_wake() {
    crate::request_wake();
}
