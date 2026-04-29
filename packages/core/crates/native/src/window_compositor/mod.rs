pub(crate) mod bridge;
pub(crate) mod frame_clock;
pub(crate) mod pipeline;
pub(crate) mod state;
pub(crate) mod texture_widget;

use napi::Result;
use crate::runtime::capture::{WidgetCapture, WidgetCaptureFormat};

use crate::{
    qt::{self, ffi::QtCompositorTarget},
    runtime::{NodeHandle, qt_error},
};

pub(crate) use bridge::{
    qt_drive_window_compositor_frame, qt_mark_window_compositor_geometry_dirty,
    qt_mark_window_compositor_pixels_dirty, qt_mark_window_compositor_pixels_dirty_region,
    qt_mark_window_compositor_scene_dirty,
    qt_window_compositor_frame_is_initialized, qt_window_frame_tick,
    qt_window_motion_hit_root_ids, qt_window_motion_hit_test,
    qt_window_motion_map_point_to_root, qt_window_take_next_frame_request,
};
pub(crate) use frame_clock::{
    read_frame_f64_prop, window_ancestor_id_for_node, write_frame_bool_prop,
};
pub(crate) use pipeline::{
    WindowCaptureGrouping, capture_window_frame_exact, capture_window_widget_exact,
};
pub(crate) use state::CompositorState;
pub(crate) use texture_widget::capture_painted_widget_exact_with_children;
pub(crate) use texture_widget::capture_vello_widget_exact;

fn snapshot_window_compositor_pending_state(window_id: u32) -> state::WindowCompositorPendingState {
    crate::runtime::with_compositor_state(|state| state.pending_state_snapshot(window_id))
}

fn store_window_compositor_target(window_id: u32, target: QtCompositorTarget) {
    crate::runtime::with_compositor_state_mut(|state| state.set_target(window_id, target));
}

pub(crate) fn load_window_compositor_target(window_id: u32) -> Option<QtCompositorTarget> {
    crate::runtime::with_compositor_state(|state| state.target(window_id))
}

fn mark_window_compositor_scene_node(window_id: u32, node_id: u32) {
    crate::runtime::with_compositor_state_mut(|state| state.mark_scene_node(window_id, node_id));
}

fn mark_window_compositor_geometry_node(window_id: u32, node_id: u32) {
    crate::runtime::with_compositor_state_mut(|state| state.mark_geometry_node(window_id, node_id));
}

pub(crate) fn mark_window_compositor_scene_subtree(window_id: u32, node_id: u32) {
    crate::runtime::with_compositor_state_mut(|state| state.mark_scene_subtree(window_id, node_id));
}

fn mark_window_compositor_dirty_node(window_id: u32, node_id: u32) {
    crate::runtime::with_compositor_state_mut(|state| state.mark_dirty_node(window_id, node_id));
}

pub(crate) fn mark_window_compositor_frame_tick_node(window_id: u32, node_id: u32) {
    crate::runtime::with_compositor_state_mut(|state| {
        state.mark_frame_tick_node(window_id, node_id)
    });
}

fn mark_window_compositor_dirty_region(
    window_id: u32,
    node_id: u32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) {
    if width <= 0 || height <= 0 {
        return;
    }

    crate::runtime::with_compositor_state_mut(|state| {
        state.mark_dirty_region(
            window_id,
            state::WindowCompositorDirtyRegion {
                node_id,
                x,
                y,
                width,
                height,
            },
        )
    });
}

fn clear_window_compositor_dirty_nodes(window_id: u32) {
    crate::runtime::with_compositor_state_mut(|state| state.clear_dirty_nodes(window_id));
}

fn compositor_surface_kind_to_renderer(
    kind: qt::ffi::QtCompositorSurfaceKind,
) -> Result<u8> {
    let surface_kind = match kind {
        qt::ffi::QtCompositorSurfaceKind::AppKitNsView => {
            qt_compositor::QT_COMPOSITOR_SURFACE_APPKIT_NS_VIEW
        }
        qt::ffi::QtCompositorSurfaceKind::Win32Hwnd => {
            qt_compositor::QT_COMPOSITOR_SURFACE_WIN32_HWND
        }
        qt::ffi::QtCompositorSurfaceKind::XcbWindow => {
            qt_compositor::QT_COMPOSITOR_SURFACE_XCB_WINDOW
        }
        qt::ffi::QtCompositorSurfaceKind::WaylandSurface => {
            qt_compositor::QT_COMPOSITOR_SURFACE_WAYLAND_SURFACE
        }
        _ => return Err(qt_error("unsupported qt compositor surface kind tag")),
    };
    Ok(surface_kind)
}

pub(crate) fn compositor_target_to_renderer(
    target: QtCompositorTarget,
) -> Result<qt_compositor::QtCompositorTarget> {
    Ok(qt_compositor::QtCompositorTarget {
        surface_kind: compositor_surface_kind_to_renderer(target.surface_kind)?,
        primary_handle: target.primary_handle,
        secondary_handle: target.secondary_handle,
        width_px: target.width_px,
        height_px: target.height_px,
        scale_factor: target.scale_factor,
    })
}

fn widget_capture_format_from_qt(tag: u8) -> Result<WidgetCaptureFormat> {
    match tag {
        1 => Ok(WidgetCaptureFormat::Argb32Premultiplied),
        2 => Ok(WidgetCaptureFormat::Rgba8Premultiplied),
        _ => Err(qt_error(format!(
            "unsupported Qt widget capture format tag {tag}",
        ))),
    }
}

fn capture_qt_widget_exact_with_children(
    node: &impl NodeHandle,
    include_children: bool,
) -> Result<WidgetCapture> {
    crate::runtime::ensure_live_node(node)?;

    let layout = qt::qt_capture_widget_layout(node.inner().id)
        .map_err(|error| qt_error(error.what().to_owned()))?;
    let format = widget_capture_format_from_qt(layout.format_tag)?;
    let mut capture = WidgetCapture::new_zeroed(
        format,
        layout.width_px,
        layout.height_px,
        layout.stride,
        layout.scale_factor,
    )
    .map_err(|error| qt_error(error.message().to_owned()))?;

    qt::qt_capture_widget_into(
        node.inner().id,
        layout.width_px,
        layout.height_px,
        layout.stride,
        include_children,
        capture.bytes_mut(),
    )
    .map_err(|error| qt_error(error.what().to_owned()))?;

    Ok(capture)
}

fn capture_widget_visible_rects(node_id: u32) -> Result<Vec<state::PartVisibleRect>> {
    let rects = qt::qt_capture_widget_visible_rects(node_id)
        .map_err(|error| qt_error(error.what().to_owned()))?;
    Ok(rects
        .into_iter()
        .filter(|rect| rect.width > 0 && rect.height > 0)
        .map(|rect| state::PartVisibleRect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        })
        .collect())
}
