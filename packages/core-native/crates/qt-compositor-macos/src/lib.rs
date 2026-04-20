mod compositor;
mod display_link;
mod owner;
mod presenter;
mod state;
mod trace;

pub use compositor::load_or_create_compositor;
pub use qt_compositor_core::CompositorBackendKind;
pub use qt_compositor_surface::{
    with_window_compositor_device_queue, with_window_compositor_layer_texture,
    with_window_compositor_layer_texture_handle,
};
pub use qt_compositor_types::{
    QtCompositorBaseUpload, QtCompositorImageFormat, QtCompositorLayerUpload,
    QtCompositorLayerSourceKind, QtCompositorTarget, QtCompositorUploadKind, Result,
};

pub fn backend_kind() -> CompositorBackendKind {
    CompositorBackendKind::Macos
}

pub fn release_metal_drawable(drawable_handle: u64) {
    presenter::drop_retained_metal_drawable(drawable_handle);
}

pub fn present_compositor_frame(
    target: QtCompositorTarget,
    base: &QtCompositorBaseUpload<'_>,
    layers: &[QtCompositorLayerUpload<'_>],
) -> Result<()> {
    load_or_create_compositor(target)?.present_frame(target, base, layers, None)
}

pub fn present_compositor_frame_async(
    window_id: u32,
    target: QtCompositorTarget,
    base: &QtCompositorBaseUpload<'_>,
    layers: &[QtCompositorLayerUpload<'_>],
) -> Result<()> {
    load_or_create_compositor(target)?.present_frame(target, base, layers, Some(window_id))
}

pub fn compositor_frame_is_busy(target: QtCompositorTarget) -> bool {
    load_or_create_compositor(target)
        .map(|compositor| compositor.is_busy())
        .unwrap_or(false)
}

pub fn compositor_frame_is_initialized(target: QtCompositorTarget) -> bool {
    load_or_create_compositor(target)
        .map(|compositor| compositor.is_initialized(target))
        .unwrap_or(false)
}
