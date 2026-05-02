use crate::canvas::vello::{Scene, peniko::kurbo::Affine};
use crate::runtime::capture::WidgetCapture;
use crate::runtime::qt_error;
use anyrender::PaintScene;
use vello_cpu::{Pixmap, RenderContext};

pub(crate) fn render_scene_to_capture(
    width_px: u32,
    height_px: u32,
    scale_factor: f64,
    scene: &Scene,
) -> napi::Result<WidgetCapture> {
    let width_u16 = u16::try_from(width_px)
        .map_err(|_| qt_error("scene width exceeds vello_cpu range".to_owned()))?;
    let height_u16 = u16::try_from(height_px)
        .map_err(|_| qt_error("scene height exceeds vello_cpu range".to_owned()))?;

    let mut painter =
        anyrender_vello_cpu::VelloCpuScenePainter(RenderContext::new(width_u16, height_u16));
    painter.append_scene(scene.clone(), Affine::scale(scale_factor));

    let mut pixmap = Pixmap::new(width_u16, height_u16);
    painter.0.flush();
    painter.0.render_to_pixmap(&mut pixmap);

    WidgetCapture::from_premul_rgba_pixels(
        width_px,
        height_px,
        width_px as usize * 4,
        scale_factor,
        pixmap.data().to_vec(),
    )
    .map_err(|error| qt_error(error.to_string()))
}
