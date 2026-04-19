use napi::Result;
use qt_solid_widget_core::{
    runtime::{self as widget_runtime, WidgetCapture},
    vello::{FrameTime as VelloFrameTime, VelloFrame},
};

use crate::{
    qt,
    runtime::{
        NodeHandle, ensure_live_node, node_by_id, qt_error, request_next_frame_exact,
        widget_instance_for_node_id,
    },
    vello_wgpu,
};

use super::frame_clock::{node_frame_time, window_ancestor_id_for_node};
use super::{
    capture_qt_widget_exact_with_children, compositor_target_to_renderer,
    load_window_compositor_target,
};

pub(crate) struct PreparedVelloWidgetScene {
    pub(crate) scene: vello::Scene,
    pub(crate) next_frame_requested: bool,
}

pub(crate) fn prepare_vello_widget_scene(
    node: &impl NodeHandle,
    layout: &qt::QtWidgetCaptureLayout,
    time: VelloFrameTime,
) -> Result<PreparedVelloWidgetScene> {
    let instance = widget_instance_for_node_id(node.inner().id)?;
    let mut scene = vello::Scene::new();
    let mut next_frame_requested = false;
    let mut logical_dirty_rects = Vec::new();
    let mut frame = VelloFrame::new(
        f64::from(layout.width_px) / layout.scale_factor.max(f64::EPSILON),
        f64::from(layout.height_px) / layout.scale_factor.max(f64::EPSILON),
        layout.scale_factor,
        time,
        &mut scene,
        &mut next_frame_requested,
        &mut logical_dirty_rects,
    );
    match instance.paint(widget_runtime::PaintDevice::Vello(&mut frame)) {
        Ok(()) => {}
        Err(error) if error.is_unsupported_paint_device() => {
            return Err(qt_error(format!(
                "node {} does not support texture widget rendering",
                node.inner().id
            )));
        }
        Err(error) => return Err(qt_error(error.to_string())),
    }

    Ok(PreparedVelloWidgetScene {
        scene,
        next_frame_requested,
    })
}

fn is_texture_paint_host_node(generation: u64, node_id: u32) -> Result<bool> {
    let node = node_by_id(generation, node_id)?;
    Ok(node.inner().binding().host.class == "TexturePaintHostWidget")
}

pub(crate) fn render_texture_widget_part_into_compositor_layer(
    generation: u64,
    target: qt_wgpu_renderer::QtCompositorTarget,
    node_id: u32,
) -> Result<bool> {
    if !is_texture_paint_host_node(generation, node_id)? {
        return Ok(false);
    }

    let node = node_by_id(generation, node_id)?;
    let Some(window_id) = window_ancestor_id_for_node(generation, node_id)? else {
        return Ok(false);
    };
    let window = node_by_id(generation, window_id)?;
    let layout =
        qt::qt_capture_widget_layout(node_id).map_err(|error| qt_error(error.what().to_owned()))?;
    let time = node_frame_time(&window)?;
    let prepared_scene = prepare_vello_widget_scene(&node, &layout, time)?;
    if prepared_scene.next_frame_requested {
        request_next_frame_exact(&window)?;
    }
    vello_wgpu::render_vello_scene_into_compositor_layer(
        target,
        node_id,
        layout.width_px,
        layout.height_px,
        layout.scale_factor,
        &prepared_scene.scene,
    )?;
    Ok(true)
}

pub(crate) fn capture_vello_widget_exact(node: &impl NodeHandle) -> Result<Option<WidgetCapture>> {
    ensure_live_node(node)?;
    if node.inner().binding().host.class != "TexturePaintHostWidget" {
        return Ok(None);
    }
    let Some(window_id) = window_ancestor_id_for_node(node.inner().generation, node.inner().id)?
    else {
        return Ok(None);
    };
    let Some(target) = load_window_compositor_target(window_id) else {
        return capture_qt_widget_exact_with_children(node, false).map(Some);
    };
    let window = node_by_id(node.inner().generation, window_id)?;
    let time = node_frame_time(&window)?;
    let layout = qt::qt_capture_widget_layout(node.inner().id)
        .map_err(|error| qt_error(error.what().to_owned()))?;
    let prepared_scene = prepare_vello_widget_scene(node, &layout, time)?;
    if prepared_scene.next_frame_requested {
        request_next_frame_exact(&window)?;
    }
    let render_target = compositor_target_to_renderer(target)?;
    vello_wgpu::render_vello_scene_to_capture(
        render_target,
        node.inner().id,
        layout.width_px,
        layout.height_px,
        layout.scale_factor,
        &prepared_scene.scene,
    )
    .map(Some)
}

pub(crate) fn capture_painted_widget_exact_with_children(
    node: &impl NodeHandle,
    include_children: bool,
) -> Result<WidgetCapture> {
    if let Some(capture) = capture_vello_widget_exact(node)? {
        return Ok(capture);
    }

    capture_qt_widget_exact_with_children(node, include_children)
}

#[cfg(test)]
mod tests {
    #[test]
    fn texture_widget_test_harness_placeholder() {}
}
