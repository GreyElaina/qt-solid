use crate::canvas::vello::Scene;
use crate::runtime::capture::WidgetCapture;

use crate::runtime::qt_error;

pub(crate) fn render_scene_to_capture(
    target: qt_compositor::QtCompositorTarget,
    node_id: u32,
    width_px: u32,
    height_px: u32,
    scale_factor: f64,
    scene: &Scene,
) -> napi::Result<WidgetCapture> {
    let pixmap = super::cpu::logical_scene_to_cpu_pixmap(target, node_id, width_px, height_px, scale_factor, scene)
        .map_err(|error| qt_error(error.to_string()))?;
    WidgetCapture::from_premul_rgba_pixels(
        width_px,
        height_px,
        width_px as usize * 4,
        scale_factor,
        pixmap.data().to_vec(),
    )
    .map_err(|error| qt_error(error.to_string()))
}
