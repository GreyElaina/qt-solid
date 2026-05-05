mod accessibility;
#[cfg(target_os = "macos")]
mod accessibility_bridge;
mod api;
mod canvas;
mod image;
// Re-export for fragment-derive proc macro which generates `crate::fragment::decl::*`
// and `crate::fragment::*` paths.
pub use canvas::{fragment, vello};
mod layout;
mod qt;
mod renderer;
mod runtime;
mod trace;

pub use api::{
    FocusPolicy, QtApp, QtHostEvent, QtNode, qt_solid_trace_clear,
    qt_solid_trace_enter_interaction, qt_solid_trace_exit_interaction, qt_solid_trace_record_js,
    qt_solid_trace_set_enabled, qt_solid_trace_snapshot,
};

#[napi_derive::module_init]
fn module_init() {}
