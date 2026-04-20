pub use qt_compositor_gpu::{
    CompositorTimingStage, record_compositor_present_decision, record_compositor_timing,
};
pub use qt_compositor_surface::{
    compositor_frame_is_busy, compositor_frame_is_initialized, present_compositor_frame,
    present_compositor_frame_async, with_window_compositor_device_queue,
    with_window_compositor_layer_texture, with_window_compositor_layer_texture_handle,
};
pub use qt_compositor_types::{
    QT_COMPOSITOR_SURFACE_APPKIT_NS_VIEW, QT_COMPOSITOR_SURFACE_WAYLAND_SURFACE,
    QT_COMPOSITOR_SURFACE_WIN32_HWND, QT_COMPOSITOR_SURFACE_XCB_WINDOW, QtCompositorBaseUpload,
    QtCompositorError, QtCompositorImageFormat, QtCompositorLayerSourceKind,
    QtCompositorLayerUpload, QtCompositorRect, QtCompositorTarget, QtCompositorUploadKind, Result,
};

pub fn backend_kind() -> qt_compositor_core::CompositorBackendKind {
    qt_compositor_core::CompositorBackendKind::Wayland
}

pub fn load_or_create_compositor(
    target: QtCompositorTarget,
) -> Result<std::sync::Arc<dyn qt_compositor_core::Compositor>> {
    qt_compositor_surface::load_or_create_compositor(target)
}
