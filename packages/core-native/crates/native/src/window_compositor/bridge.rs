use napi::Result;

use crate::qt::{QtRect, QtWindowCompositorPartMeta, ffi::QtCompositorTarget};

use super::{
    QtPreparedWindowCompositorFrame, frame_clock, mark_window_compositor_dirty_node,
    mark_window_compositor_dirty_region, mark_window_compositor_geometry_node,
    mark_window_compositor_scene_node, pipeline, upload_kind_tag,
};

pub(crate) fn qt_paint_window_compositor(
    node_id: u32,
    width_px: u32,
    height_px: u32,
    stride: usize,
    scale_factor: f64,
    dirty_flags: u8,
    interactive_resize: bool,
    bytes: &mut [u8],
) -> Result<bool> {
    pipeline::paint_window_compositor(
        node_id,
        width_px,
        height_px,
        stride,
        scale_factor,
        dirty_flags,
        interactive_resize,
        bytes,
    )
}

pub(crate) fn qt_prepare_window_compositor_frame(
    node_id: u32,
    width_px: u32,
    height_px: u32,
    stride: usize,
    scale_factor: f64,
    dirty_flags: u8,
    interactive_resize: bool,
) -> Result<Option<Box<QtPreparedWindowCompositorFrame>>> {
    pipeline::prepare_window_compositor_frame(
        node_id,
        width_px,
        height_px,
        stride,
        scale_factor,
        dirty_flags,
        interactive_resize,
    )
}

pub(crate) fn qt_present_window_with_wgpu(
    node_id: u32,
    target: QtCompositorTarget,
    stride: usize,
    scale_factor: f64,
    interactive_resize: bool,
    base_dirty_rects: Vec<QtRect>,
    bytes: &[u8],
) -> Result<bool> {
    pipeline::present_window_with_wgpu(
        node_id,
        target,
        stride,
        scale_factor,
        interactive_resize,
        base_dirty_rects,
        bytes,
    )
}

pub(crate) fn qt_window_compositor_frame_part_count(
    frame: &QtPreparedWindowCompositorFrame,
) -> usize {
    frame.part_count()
}

pub(crate) fn qt_window_compositor_frame_part_meta(
    frame: &QtPreparedWindowCompositorFrame,
    index: usize,
) -> Result<QtWindowCompositorPartMeta> {
    Ok(frame.part(index)?.meta)
}

pub(crate) fn qt_window_compositor_frame_part_visible_rects(
    frame: &QtPreparedWindowCompositorFrame,
    index: usize,
) -> Result<Vec<QtRect>> {
    Ok(frame.part(index)?.visible_rects.clone())
}

pub(crate) fn qt_window_compositor_frame_part_upload_kind(
    frame: &QtPreparedWindowCompositorFrame,
    index: usize,
) -> Result<u8> {
    Ok(upload_kind_tag(frame.part(index)?.upload_kind))
}

pub(crate) fn qt_window_compositor_frame_base_upload_kind(
    frame: &QtPreparedWindowCompositorFrame,
) -> u8 {
    upload_kind_tag(frame.base_upload_kind())
}

pub(crate) fn qt_window_compositor_frame_part_dirty_rects(
    frame: &QtPreparedWindowCompositorFrame,
    index: usize,
) -> Result<Vec<QtRect>> {
    Ok(frame.part(index)?.dirty_rects.clone())
}

pub(crate) fn qt_window_compositor_frame_part_bytes<'a>(
    frame: &'a QtPreparedWindowCompositorFrame,
    index: usize,
) -> Result<&'a [u8]> {
    frame
        .part(index)?
        .capture
        .as_deref()
        .map(|capture| capture.bytes())
        .ok_or_else(|| crate::runtime::qt_error("window compositor frame part has no CPU bytes"))
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
