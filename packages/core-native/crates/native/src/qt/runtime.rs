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
