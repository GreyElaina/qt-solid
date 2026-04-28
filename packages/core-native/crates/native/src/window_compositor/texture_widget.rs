use napi::Result;
use crate::canvas::vello::Scene;
use crate::runtime::capture::WidgetCapture;

use crate::{
    qt,
    runtime::{
        NodeHandle, ensure_live_node, node_by_id, qt_error,
    },
    scene_renderer,
};

use super::frame_clock::{node_frame_time, window_ancestor_id_for_node};
use super::capture_qt_widget_exact_with_children;
use super::load_window_compositor_target;
use super::compositor_target_to_renderer;

use crate::canvas::fragment as fragment_store;
use crate::canvas::vello::peniko::kurbo::Affine;

fn compositor_trace_enabled() -> bool {
    std::env::var_os("QT_SOLID_WGPU_TRACE").is_some()
}

fn compositor_trace(args: std::fmt::Arguments<'_>) {
    if !compositor_trace_enabled() {
        return;
    }
    println!("[qt-texture-widget] {args}");
}

pub(crate) fn capture_vello_widget_exact(node: &impl NodeHandle) -> Result<Option<WidgetCapture>> {
    ensure_live_node(node)?;
    if !node.inner().is_window() {
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
    let render_target = compositor_target_to_renderer(target)?;

    let node_id = node.inner().id;
    let now = crate::qt::trace_now_ns() as f64 / 1_000_000_000.0;
    let (_still_animating, completed) =
        fragment_store::fragment_store_tick_motion(node_id, now);
    for fid in completed {
        crate::runtime::emit_js_event(crate::api::QtHostEvent::CanvasMotionComplete {
            canvas_node_id: node_id,
            fragment_id: fid.0,
        });
    }

    let mut scene = Scene::new();
    fragment_store::fragment_store_paint(node_id, &mut scene, Affine::IDENTITY);

    compositor_trace(format_args!(
        "capture-vello node={} target={}x{} layout={}x{} scale={:.3} elapsed_ms={:.3} delta_ms={:.3}",
        node_id,
        render_target.width_px,
        render_target.height_px,
        layout.width_px,
        layout.height_px,
        layout.scale_factor,
        time.elapsed.as_secs_f64() * 1000.0,
        time.delta.as_secs_f64() * 1000.0,
    ));
    let capture = scene_renderer::render_scene_to_capture(
        render_target,
        node_id,
        layout.width_px,
        layout.height_px,
        layout.scale_factor,
        &scene,
    )
    .map_err(|error| qt_error(error.to_string()))?;
    compositor_trace(format_args!(
        "capture-vello-done node={} bytes={} format={:?} size={}x{} stride={}",
        node_id,
        capture.bytes().len(),
        capture.format(),
        capture.width_px(),
        capture.height_px(),
        capture.stride()
    ));
    Ok(Some(capture))
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
