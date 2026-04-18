use std::sync::Arc;

pub(crate) fn emit_app_event(name: &str) {
    crate::runtime::emit_app_event(name);
}

pub(crate) fn emit_debug_event(name: &str) {
    crate::runtime::emit_debug_event(name);
}

pub(crate) fn emit_inspect_event(node_id: u32) {
    crate::runtime::emit_inspect_event(node_id);
}

pub(crate) fn qt_mark_window_compositor_scene_dirty(window_id: u32, node_id: u32) {
    crate::runtime::qt_mark_window_compositor_scene_dirty(window_id, node_id);
}

pub(crate) fn qt_mark_window_compositor_geometry_dirty(window_id: u32, node_id: u32) {
    crate::runtime::qt_mark_window_compositor_geometry_dirty(window_id, node_id);
}

pub(crate) fn qt_mark_window_compositor_pixels_dirty(window_id: u32, node_id: u32) {
    crate::runtime::qt_mark_window_compositor_pixels_dirty(window_id, node_id);
}

pub(crate) fn qt_window_frame_tick(node_id: u32) -> napi::Result<()> {
    crate::runtime::qt_window_frame_tick(node_id)
}

pub(crate) fn qt_window_take_next_frame_request(node_id: u32) -> napi::Result<bool> {
    crate::runtime::qt_window_take_next_frame_request(node_id)
}

pub(crate) fn qt_mark_window_compositor_pixels_dirty_region(
    window_id: u32,
    node_id: u32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) {
    crate::runtime::qt_mark_window_compositor_pixels_dirty_region(
        window_id, node_id, x, y, width, height,
    );
}

pub(crate) fn qt_paint_window_compositor(
    node_id: u32,
    width_px: u32,
    height_px: u32,
    stride: usize,
    scale_factor: f64,
    dirty_flags: u8,
    interactive_resize: bool,
    bytes: &mut [u8],
) -> napi::Result<bool> {
    crate::runtime::qt_paint_window_compositor(
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
) -> napi::Result<Option<Box<crate::runtime::QtPreparedWindowCompositorFrame>>> {
    crate::runtime::qt_prepare_window_compositor_frame(
        node_id,
        width_px,
        height_px,
        stride,
        scale_factor,
        dirty_flags,
        interactive_resize,
    )
}

pub(crate) fn qt_window_compositor_frame_part_count(
    frame: &crate::runtime::QtPreparedWindowCompositorFrame,
) -> usize {
    crate::runtime::qt_window_compositor_frame_part_count(frame)
}

pub(crate) fn qt_window_compositor_frame_part_meta(
    frame: &crate::runtime::QtPreparedWindowCompositorFrame,
    index: usize,
) -> napi::Result<super::ffi::QtWindowCompositorPartMeta> {
    crate::runtime::qt_window_compositor_frame_part_meta(frame, index)
}

pub(crate) fn qt_window_compositor_frame_part_visible_rects(
    frame: &crate::runtime::QtPreparedWindowCompositorFrame,
    index: usize,
) -> napi::Result<Vec<super::ffi::QtRect>> {
    crate::runtime::qt_window_compositor_frame_part_visible_rects(frame, index)
}

pub(crate) fn qt_window_compositor_frame_part_upload_kind(
    frame: &crate::runtime::QtPreparedWindowCompositorFrame,
    index: usize,
) -> napi::Result<u8> {
    crate::runtime::qt_window_compositor_frame_part_upload_kind(frame, index)
}

pub(crate) fn qt_window_compositor_frame_base_upload_kind(
    frame: &crate::runtime::QtPreparedWindowCompositorFrame,
) -> u8 {
    crate::runtime::qt_window_compositor_frame_base_upload_kind(frame)
}

pub(crate) fn qt_window_compositor_frame_part_dirty_rects(
    frame: &crate::runtime::QtPreparedWindowCompositorFrame,
    index: usize,
) -> napi::Result<Vec<super::ffi::QtRect>> {
    crate::runtime::qt_window_compositor_frame_part_dirty_rects(frame, index)
}

pub(crate) fn qt_window_compositor_frame_part_bytes<'a>(
    frame: &'a crate::runtime::QtPreparedWindowCompositorFrame,
    index: usize,
) -> napi::Result<&'a [u8]> {
    crate::runtime::qt_window_compositor_frame_part_bytes(frame, index)
}

pub(crate) fn qt_prepare_texture_widget_frame(
    node_id: u32,
    width_px: u32,
    height_px: u32,
    stride: usize,
    scale_factor: f64,
    rhi_interop: super::ffi::QtRhiInteropTransport,
) -> napi::Result<Box<crate::runtime::QtPreparedTextureWidgetFrame>> {
    crate::runtime::qt_prepare_texture_widget_frame(
        node_id,
        width_px,
        height_px,
        stride,
        scale_factor,
        rhi_interop,
    )
}

pub(crate) fn qt_texture_widget_frame_layout(
    frame: &crate::runtime::QtPreparedTextureWidgetFrame,
) -> super::ffi::QtPreparedTextureWidgetFrameLayout {
    crate::runtime::qt_texture_widget_frame_layout(frame)
}

pub(crate) fn qt_texture_widget_frame_source_kind(
    frame: &crate::runtime::QtPreparedTextureWidgetFrame,
) -> u8 {
    crate::runtime::qt_texture_widget_frame_source_kind(frame)
}

pub(crate) fn qt_texture_widget_frame_native_texture_info(
    frame: &crate::runtime::QtPreparedTextureWidgetFrame,
) -> napi::Result<super::ffi::QtNativeTextureLeaseInfo> {
    crate::runtime::qt_texture_widget_frame_native_texture_info(frame)
}

pub(crate) fn qt_texture_widget_frame_upload_kind(
    frame: &crate::runtime::QtPreparedTextureWidgetFrame,
) -> u8 {
    crate::runtime::qt_texture_widget_frame_upload_kind(frame)
}

pub(crate) fn qt_texture_widget_frame_next_frame_requested(
    frame: &crate::runtime::QtPreparedTextureWidgetFrame,
) -> bool {
    crate::runtime::qt_texture_widget_frame_next_frame_requested(frame)
}

pub(crate) fn qt_texture_widget_frame_dirty_rects(
    frame: &crate::runtime::QtPreparedTextureWidgetFrame,
) -> napi::Result<Vec<super::ffi::QtRect>> {
    crate::runtime::qt_texture_widget_frame_dirty_rects(frame)
}

pub(crate) fn qt_texture_widget_frame_bytes<'a>(
    frame: &'a crate::runtime::QtPreparedTextureWidgetFrame,
) -> napi::Result<&'a [u8]> {
    crate::runtime::qt_texture_widget_frame_bytes(frame)
}

pub(crate) fn emit_listener_event(
    node_id: u32,
    kind_tag: u8,
    event_index: u8,
    trace_id: u64,
    values: Vec<super::ffi::QtListenerValue>,
) {
    let values = values
        .into_iter()
        .map(|value| crate::api::QtListenerValue {
            path: value.path,
            kind_tag: value.kind_tag,
            string_value: (value.kind_tag == 1).then_some(value.string_value),
            bool_value: (value.kind_tag == 2).then_some(value.bool_value),
            i32_value: (value.kind_tag == 3).then_some(value.i32_value),
            f64_value: (value.kind_tag == 4).then_some(value.f64_value),
        })
        .collect::<Vec<_>>();
    crate::runtime::emit_listener_event(
        node_id,
        kind_tag,
        event_index,
        trace_id,
        Arc::from(values),
    );
}

pub(crate) fn next_trace_id() -> u64 {
    crate::trace::next_trace_id()
}

pub(crate) fn trace_cpp_stage(
    trace_id: u64,
    stage: &str,
    node_id: u32,
    prop_id: u16,
    detail: &str,
) {
    crate::trace::record_dynamic(
        trace_id,
        "cpp".to_owned(),
        stage.to_owned(),
        Some(node_id),
        None,
        if prop_id == 0 { None } else { Some(prop_id) },
        if detail.is_empty() {
            None
        } else {
            Some(detail.to_owned())
        },
    );
}

pub(crate) fn qt_invoke_qpainter_hook(
    node_id: u32,
    kind_tag: u8,
    hook_name: &str,
    painter: std::pin::Pin<&mut super::ffi::QPainter>,
) -> napi::Result<()> {
    crate::runtime::qt_invoke_qpainter_hook(node_id, kind_tag, hook_name, painter)
}
