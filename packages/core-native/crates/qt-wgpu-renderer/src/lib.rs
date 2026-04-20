pub use qt_compositor_core::{Compositor, CompositorBackendKind, FrameReason};
pub use qt_compositor_gpu::{
    CompositorTimingStage, record_compositor_present_decision, record_compositor_timing,
};
pub use qt_compositor_types::{
    QT_COMPOSITOR_SURFACE_APPKIT_NS_VIEW, QT_COMPOSITOR_SURFACE_WAYLAND_SURFACE,
    QT_COMPOSITOR_SURFACE_WIN32_HWND, QT_COMPOSITOR_SURFACE_XCB_WINDOW, QtCompositorBaseUpload,
    QtCompositorError as QtWgpuRendererError, QtCompositorImageFormat, QtCompositorLayerSourceKind,
    QtCompositorLayerUpload, QtCompositorRect, QtCompositorTarget, QtCompositorUploadKind, Result,
};

#[cfg(target_os = "macos")]
pub use qt_compositor_macos::{
    compositor_frame_is_busy, compositor_frame_is_initialized, present_compositor_frame,
    present_compositor_frame_async, release_metal_drawable, with_window_compositor_device_queue,
    with_window_compositor_layer_texture, with_window_compositor_layer_texture_handle,
};

#[cfg(target_os = "windows")]
pub use qt_compositor_windows::{
    compositor_frame_is_busy, compositor_frame_is_initialized, present_compositor_frame,
    present_compositor_frame_async,
    with_window_compositor_device_queue,
    with_window_compositor_layer_texture, with_window_compositor_layer_texture_handle,
};

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub use qt_compositor_x11::{
    compositor_frame_is_busy, compositor_frame_is_initialized, present_compositor_frame,
    present_compositor_frame_async,
    with_window_compositor_device_queue,
    with_window_compositor_layer_texture, with_window_compositor_layer_texture_handle,
};

#[cfg(target_os = "macos")]
pub fn current_backend_kind() -> CompositorBackendKind {
    qt_compositor_macos::backend_kind()
}

#[cfg(target_os = "windows")]
pub fn current_backend_kind() -> CompositorBackendKind {
    qt_compositor_windows::backend_kind()
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub fn current_backend_kind() -> CompositorBackendKind {
    qt_compositor_x11::backend_kind()
}

#[cfg(target_os = "macos")]
pub fn load_or_create_compositor(
    target: QtCompositorTarget,
) -> Result<std::sync::Arc<dyn Compositor>> {
    qt_compositor_macos::load_or_create_compositor(target)
}

#[cfg(target_os = "windows")]
pub fn load_or_create_compositor(
    target: QtCompositorTarget,
) -> Result<std::sync::Arc<dyn Compositor>> {
    qt_compositor_windows::load_or_create_compositor(target)
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
pub fn load_or_create_compositor(
    target: QtCompositorTarget,
) -> Result<std::sync::Arc<dyn Compositor>> {
    qt_compositor_x11::load_or_create_compositor(target)
}
