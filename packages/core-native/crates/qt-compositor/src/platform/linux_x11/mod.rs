use std::sync::Arc;

use crate::compositor_actor;
use crate::compositor_core::Compositor;
use crate::types::{
    QT_COMPOSITOR_SURFACE_XCB_WINDOW, QtCompositorBaseUpload, QtCompositorLayerUpload,
    QtCompositorTarget, Result,
};

pub fn load_or_create_compositor(target: QtCompositorTarget) -> Result<Arc<dyn Compositor>> {
    compositor_actor::load_or_create_wgpu_compositor("x11", QT_COMPOSITOR_SURFACE_XCB_WINDOW, target)
}

pub fn destroy_compositor(target: QtCompositorTarget) {
    compositor_actor::remove_compositor(target.surface_key());
    crate::surface::destroy_window_compositor(target);
}

pub fn present_compositor_frame(
    target: QtCompositorTarget,
    base: &QtCompositorBaseUpload<'_>,
    layers: &[QtCompositorLayerUpload<'_>],
) -> Result<()> {
    load_or_create_compositor(target)?
        .present_frame(target, base, layers, None)
        .map(|_| ())
}

pub fn present_compositor_frame_async(
    window_id: u32,
    target: QtCompositorTarget,
    base: &QtCompositorBaseUpload<'_>,
    layers: &[QtCompositorLayerUpload<'_>],
) -> Result<()> {
    load_or_create_compositor(target)?
        .present_frame(target, base, layers, Some(window_id))
        .map(|_| ())
}

pub fn compositor_frame_is_busy(target: QtCompositorTarget) -> bool {
    load_or_create_compositor(target)
        .map(|c| c.is_busy())
        .unwrap_or(false)
}

pub fn compositor_frame_is_initialized(target: QtCompositorTarget) -> bool {
    load_or_create_compositor(target)
        .map(|c| c.is_initialized(target))
        .unwrap_or(false)
}
