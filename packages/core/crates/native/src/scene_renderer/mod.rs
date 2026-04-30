use napi::Result;
use crate::canvas::vello::Scene;
use crate::runtime::capture::WidgetCapture;

pub(crate) mod cpu;
#[cfg(target_os = "macos")]
mod macos_cpu;
pub(crate) mod effect_pass;
#[cfg(not(target_os = "macos"))]
mod wgpu_hybrid;

pub(crate) fn render_scene_to_capture(
    target: qt_compositor::QtCompositorTarget,
    node_id: u32,
    width_px: u32,
    height_px: u32,
    scale_factor: f64,
    scene: &Scene,
) -> Result<WidgetCapture> {
    #[cfg(target_os = "macos")]
    {
        macos_cpu::render_scene_to_capture(target, node_id, width_px, height_px, scale_factor, scene)
    }

    #[cfg(not(target_os = "macos"))]
    {
        wgpu_hybrid::render_scene_to_capture(
            target,
            node_id,
            width_px,
            height_px,
            scale_factor,
            scene,
        )
    }
}
