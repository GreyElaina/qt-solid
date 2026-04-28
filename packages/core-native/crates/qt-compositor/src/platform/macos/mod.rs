mod compositor;
mod display_link;
mod owner;
mod presenter;
mod state;
mod trace;

pub use compositor::load_or_create_compositor;

pub fn destroy_compositor(target: crate::types::QtCompositorTarget) {
    compositor::remove_compositor(target.surface_key());
    crate::surface::destroy_window_compositor(target);
}

pub fn release_metal_drawable(drawable_handle: u64) {
    presenter::drop_retained_metal_drawable(drawable_handle);
}

pub fn present_compositor_frame(
    target: crate::types::QtCompositorTarget,
    base: &crate::types::QtCompositorBaseUpload<'_>,
    layers: &[crate::types::QtCompositorLayerUpload<'_>],
) -> crate::types::Result<()> {
    load_or_create_compositor(target)?.present_frame(target, base, layers, None).map(|_| ())
}

pub fn present_compositor_frame_async(
    window_id: u32,
    target: crate::types::QtCompositorTarget,
    base: &crate::types::QtCompositorBaseUpload<'_>,
    layers: &[crate::types::QtCompositorLayerUpload<'_>],
) -> crate::types::Result<()> {
    load_or_create_compositor(target)?.present_frame(target, base, layers, Some(window_id)).map(|_| ())
}

pub fn compositor_frame_is_busy(target: crate::types::QtCompositorTarget) -> bool {
    load_or_create_compositor(target)
        .map(|compositor| compositor.is_busy())
        .unwrap_or(false)
}

pub fn compositor_frame_is_initialized(target: crate::types::QtCompositorTarget) -> bool {
    load_or_create_compositor(target)
        .map(|compositor| compositor.is_initialized(target))
        .unwrap_or(false)
}
