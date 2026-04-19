mod api;
mod bootstrap;
mod qt;
mod runtime;
mod trace;
mod vello_wgpu;
mod window_compositor;
mod window_host;

#[macro_export]
macro_rules! prop {
    ($root:ident $(. $field:ident)+) => {
        $crate::bootstrap::widget_registry()
            .prop_id_by_symbol(concat!(stringify!($root) $(, ".", stringify!($field))+))
            .expect("schema prop symbol id")
    };
}

pub use api::{
    FocusPolicy, QtApp, QtHostEvent, QtNode, ping, qt_solid_debug_capture_window_frame,
    qt_solid_debug_click_node, qt_solid_debug_close_node, qt_solid_debug_emit_app_event,
    qt_solid_debug_input_insert_text, qt_solid_debug_schedule_timer_event, qt_solid_trace_clear,
    qt_solid_trace_enter_interaction, qt_solid_trace_exit_interaction, qt_solid_trace_record_js,
    qt_solid_trace_set_enabled, qt_solid_trace_snapshot, qt_solid_window_host_info,
};

#[napi_derive::module_init]
fn module_init() {}
