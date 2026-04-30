use crate::canvas::vello::{Scene, peniko::kurbo::Affine};
use anyrender::PaintScene;
use vello_cpu::{Pixmap, RenderContext};

pub(crate) fn logical_scene_to_cpu_pixmap(
    _target: qt_compositor::QtCompositorTarget,
    _node_id: u32,
    width_px: u32,
    height_px: u32,
    scale_factor: f64,
    scene: &Scene,
) -> qt_compositor::Result<Pixmap> {
    let width_u16 = u16::try_from(width_px)
        .map_err(|_| qt_compositor::QtCompositorError::new("scene width exceeds vello_cpu range"))?;
    let height_u16 = u16::try_from(height_px)
        .map_err(|_| qt_compositor::QtCompositorError::new("scene height exceeds vello_cpu range"))?;

    let mut painter = anyrender_vello_cpu::VelloCpuScenePainter(
        RenderContext::new(width_u16, height_u16),
    );
    painter.append_scene(scene.clone(), Affine::scale(scale_factor));

    let mut pixmap = Pixmap::new(width_u16, height_u16);
    painter.0.flush();
    painter.0.render_to_pixmap(&mut pixmap);
    Ok(pixmap)
}
