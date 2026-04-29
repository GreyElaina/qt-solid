pub(crate) fn window_host_pump_zero_timeout() -> bool {
    crate::window_host::ffi_pump_zero_timeout()
}

pub(crate) fn window_host_supports_zero_timeout_pump() -> bool {
    crate::window_host::ffi_supports_zero_timeout_pump()
}

pub(crate) fn window_host_supports_external_wake() -> bool {
    crate::window_host::ffi_supports_external_wake()
}

pub(crate) fn window_host_wait_bridge_kind_tag() -> u8 {
    crate::window_host::ffi_wait_bridge_kind_tag()
}

pub(crate) fn window_host_wait_bridge_unix_fd() -> i32 {
    crate::window_host::ffi_wait_bridge_unix_fd()
}

pub(crate) fn window_host_wait_bridge_windows_handle() -> u64 {
    crate::window_host::ffi_wait_bridge_windows_handle()
}

pub(crate) fn window_host_request_wake() {
    crate::window_host::ffi_request_wake();
}