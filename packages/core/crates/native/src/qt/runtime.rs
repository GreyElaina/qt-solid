pub(crate) fn emit_app_event(name: &str) {
    crate::runtime::emit_app_event(name);
}

pub(crate) fn emit_debug_event(name: &str) {
    crate::runtime::emit_debug_event(name);
}

pub(crate) fn emit_inspect_event(node_id: u32) {
    crate::runtime::emit_inspect_event(node_id);
}

pub(crate) fn emit_canvas_pointer_event(node_id: u32, event_tag: u8, x: f64, y: f64) {
    crate::runtime::emit_canvas_pointer_event(node_id, event_tag, x, y);
}

pub(crate) fn emit_window_typed_event(node_id: u32, export_name: &str) {
    crate::runtime::emit_window_typed_event(node_id, export_name);
}

pub(crate) fn qt_canvas_key_event(
    node_id: u32,
    event_tag: u8,
    qt_key: i32,
    modifiers: u32,
    text: &str,
    repeat: bool,
    native_scan_code: u32,
    native_virtual_key: u32,
) {
    crate::runtime::qt_canvas_key_event(
        node_id, event_tag, qt_key, modifiers, text, repeat, native_scan_code, native_virtual_key,
    );
}

pub(crate) fn qt_canvas_wheel_event(
    node_id: u32,
    delta_x: f64,
    delta_y: f64,
    pixel_dx: f64,
    pixel_dy: f64,
    x: f64,
    y: f64,
    modifiers: u32,
    phase: u32,
) {
    crate::runtime::qt_canvas_wheel_event(node_id, delta_x, delta_y, pixel_dx, pixel_dy, x, y, modifiers, phase);
}

pub(crate) fn qt_window_event_focus_change(node_id: u32, gained: bool) {
    crate::runtime::qt_window_event_focus_change(node_id, gained);
}

pub(crate) fn qt_window_event_resize(node_id: u32, width: f64, height: f64) {
    crate::runtime::qt_window_event_resize(node_id, width, height);
}

pub(crate) fn qt_window_event_state_change(node_id: u32, state: u8) {
    crate::runtime::qt_window_event_state_change(node_id, state);
}

pub(crate) fn qt_system_color_scheme_changed(scheme: u8) {
    crate::runtime::qt_system_color_scheme_changed(scheme);
}

pub(crate) fn qt_screen_dpi_changed(dpi: f64) {
    crate::runtime::qt_screen_dpi_changed(dpi);
}

pub(crate) fn qt_file_dialog_result(request_id: u32, paths: Vec<String>) {
    crate::runtime::qt_file_dialog_result(request_id, paths);
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
