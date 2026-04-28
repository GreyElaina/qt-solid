use napi::Result;

use crate::qt::ffi::{
    QtCompositorTarget,
    bridge::{
        QtMotionMouseTarget, QtWindowCompositorDriveStatus,
    },
};

use super::{
    frame_clock, mark_window_compositor_dirty_node,
    mark_window_compositor_dirty_region, mark_window_compositor_geometry_node,
    mark_window_compositor_scene_node, pipeline,
};


pub(crate) fn qt_drive_window_compositor_frame(
    node_id: u32,
    target: QtCompositorTarget,
) -> Result<QtWindowCompositorDriveStatus> {
    pipeline::drive_window_compositor_frame(node_id, target)
}

pub(crate) fn qt_window_compositor_frame_is_initialized(
    target: QtCompositorTarget,
) -> Result<bool> {
    let render_target = super::compositor_target_to_renderer(target)
        .map_err(|error| crate::runtime::qt_error(error.to_string()))?;
    Ok(qt_compositor::compositor_frame_is_initialized(
        render_target,
    ))
}


pub(crate) fn qt_mark_window_compositor_pixels_dirty(window_id: u32, node_id: u32) {
    mark_window_compositor_dirty_node(window_id, node_id);
}

pub(crate) fn qt_mark_window_compositor_scene_dirty(window_id: u32, node_id: u32) {
    mark_window_compositor_scene_node(window_id, node_id);
}

pub(crate) fn qt_mark_window_compositor_geometry_dirty(window_id: u32, node_id: u32) {
    mark_window_compositor_geometry_node(window_id, node_id);
}

pub(crate) fn qt_window_frame_tick(node_id: u32) -> Result<()> {
    frame_clock::qt_window_frame_tick(node_id)
}

pub(crate) fn qt_window_take_next_frame_request(node_id: u32) -> Result<bool> {
    frame_clock::qt_window_frame_take_next_frame_request(node_id)
}

pub(crate) fn qt_mark_window_compositor_pixels_dirty_region(
    window_id: u32,
    node_id: u32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) {
    mark_window_compositor_dirty_region(window_id, node_id, x, y, width, height);
}

pub(crate) fn qt_window_motion_hit_test(
    window_id: u32,
    screen_x: i32,
    screen_y: i32,
) -> Result<QtMotionMouseTarget> {
    pipeline::window_motion_hit_test(window_id, screen_x, screen_y)
}

pub(crate) fn qt_window_motion_map_point_to_root(
    window_id: u32,
    root_node_id: u32,
    screen_x: i32,
    screen_y: i32,
) -> Result<QtMotionMouseTarget> {
    pipeline::window_motion_map_point_to_root(window_id, root_node_id, screen_x, screen_y)
}

pub(crate) fn qt_window_motion_hit_root_ids(window_id: u32) -> Result<Vec<u32>> {
    pipeline::window_motion_hit_root_ids(window_id)
}
