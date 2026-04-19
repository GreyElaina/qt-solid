use napi::Result;
use qt_solid_widget_core::{
    runtime::{self as widget_runtime, WidgetCapture},
    vello::{FrameTime as VelloFrameTime, VelloDirtyRect, VelloFrame},
};

use crate::{
    qt,
    runtime::{
        NodeHandle, debug_node_bounds, ensure_live_node, node_by_id, qt_error,
        request_overlay_next_frame_exact, widget_instance_for_node_id,
    },
    vello_wgpu,
};

use super::frame_clock::{node_frame_time, window_ancestor_id_for_node};
use super::prepare::{pixel_rect_to_qt_rect, vello_dirty_rects_to_local_pixel_rects};
use super::{
    capture_qt_widget_exact_with_children, compositor_target_to_renderer,
    load_window_compositor_target, mark_window_compositor_dirty_region,
};

pub(crate) struct PreparedVelloWidgetScene {
    pub(crate) scene: vello::Scene,
    pub(crate) next_frame_requested: bool,
    pub(crate) dirty_rects: Vec<VelloDirtyRect>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextureWidgetLayerRenderResult {
    pub(crate) rendered: bool,
    pub(crate) next_frame_requested: bool,
    pub(crate) local_dirty_rects_px: Vec<qt::QtRect>,
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
        dirty_rects: logical_dirty_rects,
    })
}

fn local_pixel_rect_to_window_logical_rect(
    layout: &qt::QtWidgetCaptureLayout,
    offset_x: i32,
    offset_y: i32,
    rect: qt::QtRect,
) -> Option<qt::QtRect> {
    let scale_factor = layout.scale_factor.max(f64::EPSILON);
    let left = offset_x + (f64::from(rect.x) / scale_factor).floor() as i32;
    let top = offset_y + (f64::from(rect.y) / scale_factor).floor() as i32;
    let right = offset_x + (f64::from(rect.x + rect.width) / scale_factor).ceil() as i32;
    let bottom = offset_y + (f64::from(rect.y + rect.height) / scale_factor).ceil() as i32;
    (right > left && bottom > top).then_some(qt::QtRect {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    })
}

fn window_dirty_rects_from_local_pixel_rects(
    layout: &qt::QtWidgetCaptureLayout,
    offset_x: i32,
    offset_y: i32,
    local_dirty_rects_px: &[qt::QtRect],
    next_frame_requested: bool,
) -> Vec<qt::QtRect> {
    let mut dirty_rects = local_dirty_rects_px
        .iter()
        .copied()
        .filter_map(|rect| {
            local_pixel_rect_to_window_logical_rect(layout, offset_x, offset_y, rect)
        })
        .collect::<Vec<_>>();

    if dirty_rects.is_empty() && next_frame_requested {
        let scale_factor = layout.scale_factor.max(f64::EPSILON);
        dirty_rects.push(qt::QtRect {
            x: offset_x,
            y: offset_y,
            width: (f64::from(layout.width_px) / scale_factor).ceil() as i32,
            height: (f64::from(layout.height_px) / scale_factor).ceil() as i32,
        });
    }

    dirty_rects
}

fn is_texture_paint_host_node(generation: u64, node_id: u32) -> Result<bool> {
    let node = node_by_id(generation, node_id)?;
    Ok(node.inner().binding().host.class == "TexturePaintHostWidget")
}

pub(crate) fn render_texture_widget_part_into_compositor_layer(
    generation: u64,
    target: qt_wgpu_renderer::QtCompositorTarget,
    node_id: u32,
) -> Result<TextureWidgetLayerRenderResult> {
    if !is_texture_paint_host_node(generation, node_id)? {
        return Ok(TextureWidgetLayerRenderResult {
            rendered: false,
            next_frame_requested: false,
            local_dirty_rects_px: Vec::new(),
        });
    }

    let node = node_by_id(generation, node_id)?;
    let Some(window_id) = window_ancestor_id_for_node(generation, node_id)? else {
        return Ok(TextureWidgetLayerRenderResult {
            rendered: false,
            next_frame_requested: false,
            local_dirty_rects_px: Vec::new(),
        });
    };
    let window = node_by_id(generation, window_id)?;
    let window_bounds = debug_node_bounds(window_id)?;
    let bounds = debug_node_bounds(node_id)?;
    let layout =
        qt::qt_capture_widget_layout(node_id).map_err(|error| qt_error(error.what().to_owned()))?;
    let time = node_frame_time(&window)?;
    let prepared_scene = prepare_vello_widget_scene(&node, &layout, time)?;
    let local_dirty_rects_px =
        vello_dirty_rects_to_local_pixel_rects(&layout, &prepared_scene.dirty_rects)?
            .into_iter()
            .map(pixel_rect_to_qt_rect)
            .collect::<Vec<_>>();
    let offset_x = bounds.screen_x - window_bounds.screen_x;
    let offset_y = bounds.screen_y - window_bounds.screen_y;
    let window_dirty_rects = window_dirty_rects_from_local_pixel_rects(
        &layout,
        offset_x,
        offset_y,
        &local_dirty_rects_px,
        prepared_scene.next_frame_requested,
    );
    for rect in &window_dirty_rects {
        mark_window_compositor_dirty_region(
            window_id,
            node_id,
            rect.x,
            rect.y,
            rect.width,
            rect.height,
        );
    }
    if prepared_scene.next_frame_requested {
        request_overlay_next_frame_exact(&window, node_id)?;
    }
    vello_wgpu::render_vello_scene_into_compositor_layer(
        target,
        node_id,
        layout.width_px,
        layout.height_px,
        layout.scale_factor,
        &prepared_scene.scene,
    )?;
    Ok(TextureWidgetLayerRenderResult {
        rendered: true,
        next_frame_requested: prepared_scene.next_frame_requested,
        local_dirty_rects_px,
    })
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
        request_overlay_next_frame_exact(&window, node.inner().id)?;
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
    use super::{
        local_pixel_rect_to_window_logical_rect, window_dirty_rects_from_local_pixel_rects,
    };
    use crate::qt::{QtRect, QtWidgetCaptureLayout};

    fn layout() -> QtWidgetCaptureLayout {
        QtWidgetCaptureLayout {
            width_px: 200,
            height_px: 120,
            stride: 800,
            scale_factor: 2.0,
            format_tag: 2,
        }
    }

    #[test]
    fn local_pixel_rect_maps_to_window_logical_rect() {
        let rect = local_pixel_rect_to_window_logical_rect(
            &layout(),
            10,
            20,
            QtRect {
                x: 4,
                y: 6,
                width: 12,
                height: 10,
            },
        )
        .expect("mapped rect");

        assert_eq!(
            rect,
            QtRect {
                x: 12,
                y: 23,
                width: 6,
                height: 5,
            }
        );
    }

    #[test]
    fn next_frame_without_dirty_rects_falls_back_to_full_widget() {
        let dirty_rects = window_dirty_rects_from_local_pixel_rects(&layout(), 5, 7, &[], true);

        assert_eq!(
            dirty_rects,
            vec![QtRect {
                x: 5,
                y: 7,
                width: 100,
                height: 60,
            }]
        );
    }
}
