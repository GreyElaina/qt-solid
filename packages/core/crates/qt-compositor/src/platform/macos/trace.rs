use std::fmt;

pub(crate) fn trace_enabled() -> bool {
    static ENABLED: std::sync::LazyLock<bool> =
        std::sync::LazyLock::new(|| std::env::var_os("QT_SOLID_WGPU_TRACE").is_some());
    *ENABLED
}

pub(crate) fn trace(args: fmt::Arguments<'_>) {
    if !trace_enabled() {
        return;
    }
    println!("[qt-macos] {args}");
}
