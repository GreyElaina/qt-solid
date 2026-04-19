use crate::window_compositor::QtPreparedWindowCompositorFrame;

#[cxx::bridge(namespace = "qt_solid_spike::qt")]
pub(crate) mod bridge {
    struct QtMethodValue {
        kind_tag: u8,
        string_value: String,
        bool_value: bool,
        i32_value: i32,
        f64_value: f64,
    }

    struct QtListenerValue {
        path: String,
        kind_tag: u8,
        string_value: String,
        bool_value: bool,
        i32_value: i32,
        f64_value: f64,
    }

    struct QtRealizedNodeState {
        has_text: bool,
        text: String,
        has_title: bool,
        title: String,
        has_width: bool,
        width: i32,
        has_height: bool,
        height: i32,
        has_min_width: bool,
        min_width: i32,
        has_min_height: bool,
        min_height: i32,
        has_flex_grow: bool,
        flex_grow: i32,
        has_flex_shrink: bool,
        flex_shrink: i32,
        has_enabled: bool,
        enabled: bool,
        has_placeholder: bool,
        placeholder: String,
        has_checked: bool,
        checked: bool,
        flex_direction_tag: u8,
        justify_content_tag: u8,
        align_items_tag: u8,
        has_gap: bool,
        gap: i32,
        has_padding: bool,
        padding: i32,
        has_value: bool,
        value: f64,
    }

    struct QtNodeBounds {
        visible: bool,
        screen_x: i32,
        screen_y: i32,
        width: i32,
        height: i32,
    }

    struct QtWidgetCaptureLayout {
        format_tag: u8,
        width_px: u32,
        height_px: u32,
        stride: usize,
        scale_factor: f64,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct QtRect {
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    }

    #[derive(Clone, Copy, Debug)]
    struct QtWindowCompositorPartMeta {
        node_id: u32,
        format_tag: u8,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        width_px: u32,
        height_px: u32,
        stride: usize,
        scale_factor: f64,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum QtCompositorSurfaceKind {
        AppKitNsView = 1,
        Win32Hwnd = 2,
        XcbWindow = 3,
        WaylandSurface = 4,
    }

    #[derive(Clone, Copy, Debug)]
    struct QtCompositorTarget {
        surface_kind: QtCompositorSurfaceKind,
        primary_handle: u64,
        secondary_handle: u64,
        width_px: u32,
        height_px: u32,
        scale_factor: f64,
    }

    extern "Rust" {
        type QtPreparedWindowCompositorFrame;

        fn emit_app_event(name: &str);
        fn emit_debug_event(name: &str);
        fn emit_inspect_event(node_id: u32);
        fn qt_mark_window_compositor_scene_dirty(window_id: u32, node_id: u32);
        fn qt_mark_window_compositor_geometry_dirty(window_id: u32, node_id: u32);
        fn qt_mark_window_compositor_pixels_dirty(window_id: u32, node_id: u32);
        fn qt_window_frame_tick(node_id: u32) -> Result<()>;
        fn qt_window_take_next_frame_request(node_id: u32) -> Result<bool>;
        fn qt_mark_window_compositor_pixels_dirty_region(
            window_id: u32,
            node_id: u32,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
        );
        fn qt_paint_window_compositor(
            node_id: u32,
            width_px: u32,
            height_px: u32,
            stride: usize,
            scale_factor: f64,
            dirty_flags: u8,
            interactive_resize: bool,
            bytes: &mut [u8],
        ) -> Result<bool>;
        fn qt_prepare_window_compositor_frame(
            node_id: u32,
            width_px: u32,
            height_px: u32,
            stride: usize,
            scale_factor: f64,
            dirty_flags: u8,
            interactive_resize: bool,
        ) -> Result<Box<QtPreparedWindowCompositorFrame>>;
        fn qt_present_window_with_wgpu(
            node_id: u32,
            target: QtCompositorTarget,
            stride: usize,
            scale_factor: f64,
            interactive_resize: bool,
            base_dirty_rects: Vec<QtRect>,
            bytes: &[u8],
        ) -> Result<bool>;
        fn qt_window_compositor_frame_part_count(frame: &QtPreparedWindowCompositorFrame) -> usize;
        fn qt_window_compositor_frame_part_meta(
            frame: &QtPreparedWindowCompositorFrame,
            index: usize,
        ) -> Result<QtWindowCompositorPartMeta>;
        fn qt_window_compositor_frame_part_visible_rects(
            frame: &QtPreparedWindowCompositorFrame,
            index: usize,
        ) -> Result<Vec<QtRect>>;
        fn qt_window_compositor_frame_part_upload_kind(
            frame: &QtPreparedWindowCompositorFrame,
            index: usize,
        ) -> Result<u8>;
        fn qt_window_compositor_frame_base_upload_kind(
            frame: &QtPreparedWindowCompositorFrame,
        ) -> u8;
        fn qt_window_compositor_frame_part_dirty_rects(
            frame: &QtPreparedWindowCompositorFrame,
            index: usize,
        ) -> Result<Vec<QtRect>>;
        unsafe fn qt_window_compositor_frame_part_bytes<'a>(
            frame: &'a QtPreparedWindowCompositorFrame,
            index: usize,
        ) -> Result<&'a [u8]>;
        fn emit_listener_event(
            node_id: u32,
            kind_tag: u8,
            event_index: u8,
            trace_id: u64,
            values: Vec<QtListenerValue>,
        );
        fn qt_widget_event_count(kind_tag: u8) -> usize;
        fn qt_widget_event_lower_kind(kind_tag: u8, index: usize) -> u8;
        fn qt_widget_event_lower_name(kind_tag: u8, index: usize) -> &'static str;
        fn qt_widget_event_payload_kind(kind_tag: u8, index: usize) -> u8;
        fn qt_widget_event_payload_scalar_kind(kind_tag: u8, index: usize) -> u8;
        fn qt_widget_event_payload_field_count(kind_tag: u8, index: usize) -> usize;
        fn qt_widget_event_payload_field_name(
            kind_tag: u8,
            index: usize,
            field_index: usize,
        ) -> &'static str;
        fn qt_widget_event_payload_field_kind(kind_tag: u8, index: usize, field_index: usize)
        -> u8;
        fn qt_widget_prop_count(kind_tag: u8) -> usize;
        fn qt_widget_prop_id(kind_tag: u8, index: usize) -> u16;
        fn qt_widget_prop_js_name(kind_tag: u8, index: usize) -> &'static str;
        fn qt_widget_prop_payload_kind(kind_tag: u8, index: usize) -> u8;
        fn qt_widget_prop_non_negative(kind_tag: u8, index: usize) -> bool;
        fn qt_widget_prop_lower_kind(kind_tag: u8, index: usize) -> u8;
        fn qt_widget_prop_lower_name(kind_tag: u8, index: usize) -> &'static str;
        fn qt_widget_prop_read_lower_kind(kind_tag: u8, index: usize) -> u8;
        fn qt_widget_prop_read_lower_name(kind_tag: u8, index: usize) -> &'static str;
        fn qt_invoke_qpainter_hook(
            node_id: u32,
            kind_tag: u8,
            hook_name: &str,
            painter: Pin<&mut QPainter>,
        ) -> Result<()>;
        fn next_trace_id() -> u64;
        fn trace_cpp_stage(trace_id: u64, stage: &str, node_id: u32, prop_id: u16, detail: &str);
        fn window_host_supports_zero_timeout_pump() -> bool;
        fn window_host_supports_external_wake() -> bool;
        fn window_host_wait_bridge_kind_tag() -> u8;
        fn window_host_wait_bridge_unix_fd() -> i32;
        fn window_host_wait_bridge_windows_handle() -> u64;
        fn window_host_pump_zero_timeout() -> bool;
        fn window_host_request_wake();
    }

    unsafe extern "C++" {
        include!("qt/ffi.h");

        #[namespace = ""]
        type QPainter;
        fn qt_host_started() -> bool;
        fn qt_runtime_wait_bridge_kind_tag() -> u8;
        fn qt_runtime_wait_bridge_unix_fd() -> i32;
        fn qt_runtime_wait_bridge_windows_handle() -> u64;
        fn start_qt_host(uv_loop_ptr: usize) -> Result<()>;
        fn shutdown_qt_host() -> Result<()>;
        fn qt_create_widget(id: u32, kind_tag: u8) -> Result<()>;
        fn qt_insert_child(parent_id: u32, child_id: u32, anchor_id_or_zero: u32) -> Result<()>;
        fn qt_remove_child(parent_id: u32, child_id: u32) -> Result<()>;
        fn qt_destroy_widget(id: u32) -> Result<()>;
        fn qt_request_repaint(id: u32) -> Result<()>;
        fn qt_capture_widget_layout(id: u32) -> Result<QtWidgetCaptureLayout>;
        fn qt_capture_widget_into(
            id: u32,
            width_px: u32,
            height_px: u32,
            stride: usize,
            include_children: bool,
            bytes: &mut [u8],
        ) -> Result<()>;
        fn qt_capture_widget_region_into(
            id: u32,
            width_px: u32,
            height_px: u32,
            stride: usize,
            include_children: bool,
            rect: QtRect,
            bytes: &mut [u8],
        ) -> Result<()>;
        fn qt_capture_widget_visible_rects(id: u32) -> Result<Vec<QtRect>>;
        fn qt_apply_string_prop(id: u32, prop_id: u16, trace_id: u64, value: &str) -> Result<()>;
        fn qt_apply_i32_prop(id: u32, prop_id: u16, trace_id: u64, value: i32) -> Result<()>;
        fn qt_apply_f64_prop(id: u32, prop_id: u16, trace_id: u64, value: f64) -> Result<()>;
        fn qt_apply_bool_prop(id: u32, prop_id: u16, trace_id: u64, value: bool) -> Result<()>;
        fn qt_call_host_slot(
            id: u32,
            slot: u16,
            args: &Vec<QtMethodValue>,
        ) -> Result<QtMethodValue>;
        fn qt_qpainter_call(
            painter: Pin<&mut QPainter>,
            slot: u16,
            args: &Vec<QtMethodValue>,
        ) -> Result<QtMethodValue>;
        fn qt_read_string_prop(id: u32, prop_id: u16) -> Result<String>;
        fn qt_read_i32_prop(id: u32, prop_id: u16) -> Result<i32>;
        fn qt_read_f64_prop(id: u32, prop_id: u16) -> Result<f64>;
        fn qt_read_bool_prop(id: u32, prop_id: u16) -> Result<bool>;
        fn qt_debug_node_state(id: u32) -> QtRealizedNodeState;
        fn schedule_debug_event(delay_ms: u32, name: &str) -> Result<()>;
        fn debug_click_node(id: u32) -> Result<()>;
        fn debug_close_node(id: u32) -> Result<()>;
        fn debug_input_insert_text(id: u32, value: &str) -> Result<()>;
        fn debug_highlight_node(id: u32) -> Result<()>;
        fn debug_node_bounds(id: u32) -> QtNodeBounds;
        fn debug_node_at_point(screen_x: i32, screen_y: i32) -> u32;
        fn debug_set_inspect_mode(enabled: bool) -> Result<()>;
        fn debug_clear_highlight() -> Result<()>;
        fn trace_now_ns() -> u64;
    }
}

pub(crate) use bridge::{
    QPainter, QtCompositorSurfaceKind, QtCompositorTarget, QtListenerValue, QtMethodValue,
    QtRealizedNodeState, QtRect, QtWidgetCaptureLayout, QtWindowCompositorPartMeta,
    debug_clear_highlight, debug_click_node, debug_close_node, debug_highlight_node,
    debug_input_insert_text, debug_node_at_point, debug_node_bounds, debug_set_inspect_mode,
    qt_apply_bool_prop, qt_apply_f64_prop, qt_apply_i32_prop, qt_apply_string_prop,
    qt_call_host_slot, qt_capture_widget_into, qt_capture_widget_layout,
    qt_capture_widget_region_into, qt_capture_widget_visible_rects, qt_create_widget,
    qt_debug_node_state, qt_destroy_widget, qt_host_started, qt_insert_child, qt_qpainter_call,
    qt_read_bool_prop, qt_read_f64_prop, qt_read_i32_prop, qt_read_string_prop, qt_remove_child,
    qt_request_repaint, qt_runtime_wait_bridge_kind_tag, qt_runtime_wait_bridge_unix_fd,
    qt_runtime_wait_bridge_windows_handle, schedule_debug_event, shutdown_qt_host, start_qt_host,
    trace_now_ns,
};

pub(crate) fn emit_app_event(name: &str) {
    super::runtime::emit_app_event(name);
}

pub(crate) fn emit_debug_event(name: &str) {
    super::runtime::emit_debug_event(name);
}

pub(crate) fn emit_inspect_event(node_id: u32) {
    super::runtime::emit_inspect_event(node_id);
}

pub(crate) fn qt_mark_window_compositor_scene_dirty(window_id: u32, node_id: u32) {
    crate::window_compositor::qt_mark_window_compositor_scene_dirty(window_id, node_id);
}

pub(crate) fn qt_mark_window_compositor_geometry_dirty(window_id: u32, node_id: u32) {
    crate::window_compositor::qt_mark_window_compositor_geometry_dirty(window_id, node_id);
}

pub(crate) fn qt_mark_window_compositor_pixels_dirty(window_id: u32, node_id: u32) {
    crate::window_compositor::qt_mark_window_compositor_pixels_dirty(window_id, node_id);
}

pub(crate) fn qt_window_frame_tick(node_id: u32) -> napi::Result<()> {
    crate::window_compositor::qt_window_frame_tick(node_id)
}

pub(crate) fn qt_window_take_next_frame_request(node_id: u32) -> napi::Result<bool> {
    crate::window_compositor::qt_window_take_next_frame_request(node_id)
}

pub(crate) fn qt_mark_window_compositor_pixels_dirty_region(
    window_id: u32,
    node_id: u32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) {
    crate::window_compositor::qt_mark_window_compositor_pixels_dirty_region(
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
    crate::window_compositor::qt_paint_window_compositor(
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
) -> napi::Result<Box<QtPreparedWindowCompositorFrame>> {
    crate::window_compositor::qt_prepare_window_compositor_frame(
        node_id,
        width_px,
        height_px,
        stride,
        scale_factor,
        dirty_flags,
        interactive_resize,
    )?
    .ok_or_else(|| napi::Error::from_reason("window compositor layout mismatch"))
}

pub(crate) fn qt_present_window_with_wgpu(
    node_id: u32,
    target: QtCompositorTarget,
    stride: usize,
    scale_factor: f64,
    interactive_resize: bool,
    base_dirty_rects: Vec<QtRect>,
    bytes: &[u8],
) -> napi::Result<bool> {
    crate::window_compositor::qt_present_window_with_wgpu(
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
    crate::window_compositor::qt_window_compositor_frame_part_count(frame)
}

pub(crate) fn qt_window_compositor_frame_part_meta(
    frame: &QtPreparedWindowCompositorFrame,
    index: usize,
) -> napi::Result<QtWindowCompositorPartMeta> {
    crate::window_compositor::qt_window_compositor_frame_part_meta(frame, index)
}

pub(crate) fn qt_window_compositor_frame_part_visible_rects(
    frame: &QtPreparedWindowCompositorFrame,
    index: usize,
) -> napi::Result<Vec<QtRect>> {
    crate::window_compositor::qt_window_compositor_frame_part_visible_rects(frame, index)
}

pub(crate) fn qt_window_compositor_frame_part_upload_kind(
    frame: &QtPreparedWindowCompositorFrame,
    index: usize,
) -> napi::Result<u8> {
    crate::window_compositor::qt_window_compositor_frame_part_upload_kind(frame, index)
}

pub(crate) fn qt_window_compositor_frame_base_upload_kind(
    frame: &QtPreparedWindowCompositorFrame,
) -> u8 {
    crate::window_compositor::qt_window_compositor_frame_base_upload_kind(frame)
}

pub(crate) fn qt_window_compositor_frame_part_dirty_rects(
    frame: &QtPreparedWindowCompositorFrame,
    index: usize,
) -> napi::Result<Vec<QtRect>> {
    crate::window_compositor::qt_window_compositor_frame_part_dirty_rects(frame, index)
}

pub(crate) fn qt_window_compositor_frame_part_bytes<'a>(
    frame: &'a QtPreparedWindowCompositorFrame,
    index: usize,
) -> napi::Result<&'a [u8]> {
    crate::window_compositor::qt_window_compositor_frame_part_bytes(frame, index)
}

pub(crate) fn emit_listener_event(
    node_id: u32,
    kind_tag: u8,
    event_index: u8,
    trace_id: u64,
    values: Vec<QtListenerValue>,
) {
    super::runtime::emit_listener_event(node_id, kind_tag, event_index, trace_id, values);
}

pub(crate) fn next_trace_id() -> u64 {
    super::runtime::next_trace_id()
}

pub(crate) fn trace_cpp_stage(
    trace_id: u64,
    stage: &str,
    node_id: u32,
    prop_id: u16,
    detail: &str,
) {
    super::runtime::trace_cpp_stage(trace_id, stage, node_id, prop_id, detail);
}

pub(crate) fn qt_widget_event_count(kind_tag: u8) -> usize {
    super::ffi_host::qt_widget_event_count(kind_tag)
}

pub(crate) fn qt_widget_event_lower_kind(kind_tag: u8, index: usize) -> u8 {
    super::ffi_host::qt_widget_event_lower_kind(kind_tag, index)
}

pub(crate) fn qt_widget_event_lower_name(kind_tag: u8, index: usize) -> &'static str {
    super::ffi_host::qt_widget_event_lower_name(kind_tag, index)
}

pub(crate) fn qt_widget_event_payload_kind(kind_tag: u8, index: usize) -> u8 {
    super::ffi_host::qt_widget_event_payload_kind(kind_tag, index)
}

pub(crate) fn qt_widget_event_payload_scalar_kind(kind_tag: u8, index: usize) -> u8 {
    super::ffi_host::qt_widget_event_payload_scalar_kind(kind_tag, index)
}

pub(crate) fn qt_widget_event_payload_field_count(kind_tag: u8, index: usize) -> usize {
    super::ffi_host::qt_widget_event_payload_field_count(kind_tag, index)
}

pub(crate) fn qt_widget_event_payload_field_name(
    kind_tag: u8,
    index: usize,
    field_index: usize,
) -> &'static str {
    super::ffi_host::qt_widget_event_payload_field_name(kind_tag, index, field_index)
}

pub(crate) fn qt_widget_event_payload_field_kind(
    kind_tag: u8,
    index: usize,
    field_index: usize,
) -> u8 {
    super::ffi_host::qt_widget_event_payload_field_kind(kind_tag, index, field_index)
}

pub(crate) fn qt_widget_prop_count(kind_tag: u8) -> usize {
    super::ffi_host::qt_widget_prop_count(kind_tag)
}

pub(crate) fn qt_widget_prop_id(kind_tag: u8, index: usize) -> u16 {
    super::ffi_host::qt_widget_prop_id(kind_tag, index)
}

pub(crate) fn qt_widget_prop_js_name(kind_tag: u8, index: usize) -> &'static str {
    super::ffi_host::qt_widget_prop_js_name(kind_tag, index)
}

pub(crate) fn qt_widget_prop_payload_kind(kind_tag: u8, index: usize) -> u8 {
    super::ffi_host::qt_widget_prop_payload_kind(kind_tag, index)
}

pub(crate) fn qt_widget_prop_non_negative(kind_tag: u8, index: usize) -> bool {
    super::ffi_host::qt_widget_prop_non_negative(kind_tag, index)
}

pub(crate) fn qt_widget_prop_lower_kind(kind_tag: u8, index: usize) -> u8 {
    super::ffi_host::qt_widget_prop_lower_kind(kind_tag, index)
}

pub(crate) fn qt_widget_prop_lower_name(kind_tag: u8, index: usize) -> &'static str {
    super::ffi_host::qt_widget_prop_lower_name(kind_tag, index)
}

pub(crate) fn qt_widget_prop_read_lower_kind(kind_tag: u8, index: usize) -> u8 {
    super::ffi_host::qt_widget_prop_read_lower_kind(kind_tag, index)
}

pub(crate) fn qt_widget_prop_read_lower_name(kind_tag: u8, index: usize) -> &'static str {
    super::ffi_host::qt_widget_prop_read_lower_name(kind_tag, index)
}

pub(crate) fn qt_invoke_qpainter_hook(
    node_id: u32,
    kind_tag: u8,
    hook_name: &str,
    painter: std::pin::Pin<&mut QPainter>,
) -> napi::Result<()> {
    super::runtime::qt_invoke_qpainter_hook(node_id, kind_tag, hook_name, painter)
}

pub(crate) fn window_host_pump_zero_timeout() -> bool {
    super::ffi_host::window_host_pump_zero_timeout()
}

pub(crate) fn window_host_supports_zero_timeout_pump() -> bool {
    super::ffi_host::window_host_supports_zero_timeout_pump()
}

pub(crate) fn window_host_supports_external_wake() -> bool {
    super::ffi_host::window_host_supports_external_wake()
}

pub(crate) fn window_host_wait_bridge_kind_tag() -> u8 {
    super::ffi_host::window_host_wait_bridge_kind_tag()
}

pub(crate) fn window_host_wait_bridge_unix_fd() -> i32 {
    super::ffi_host::window_host_wait_bridge_unix_fd()
}

pub(crate) fn window_host_wait_bridge_windows_handle() -> u64 {
    super::ffi_host::window_host_wait_bridge_windows_handle()
}

pub(crate) fn window_host_request_wake() {
    super::ffi_host::window_host_request_wake();
}
