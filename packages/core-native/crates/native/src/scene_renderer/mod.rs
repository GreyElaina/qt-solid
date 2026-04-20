use napi::Result;
use qt_solid_widget_core::{
    runtime::WidgetCapture,
    vello::Scene,
};

#[cfg(target_os = "macos")]
mod macos_cpu;
mod wgpu_hybrid;

pub(crate) fn render_scene_to_capture(
    target: qt_wgpu_renderer::QtCompositorTarget,
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

pub(crate) fn render_scene_into_compositor_layer(
    target: qt_wgpu_renderer::QtCompositorTarget,
    node_id: u32,
    width_px: u32,
    height_px: u32,
    scale_factor: f64,
    scene: &Scene,
) -> Result<()> {
    wgpu_hybrid::render_scene_into_compositor_layer(
        target,
        node_id,
        width_px,
        height_px,
        scale_factor,
        scene,
    )
}
