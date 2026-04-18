use std::sync::Arc;

use napi::{
    Env, Result,
    bindgen_prelude::{Buffer, Function},
    threadsafe_function::UnknownReturnValue,
};

use crate::runtime::{self, NodeHandle, QtNodeInner};

#[napi_derive::napi(string_enum)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexDirection {
    #[napi(value = "column")]
    Column,
    #[napi(value = "row")]
    Row,
}

#[napi_derive::napi(string_enum)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignItems {
    #[napi(value = "flex-start")]
    FlexStart,
    #[napi(value = "center")]
    Center,
    #[napi(value = "flex-end")]
    FlexEnd,
    #[napi(value = "stretch")]
    Stretch,
}

#[napi_derive::napi(string_enum)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JustifyContent {
    #[napi(value = "flex-start")]
    FlexStart,
    #[napi(value = "center")]
    Center,
    #[napi(value = "flex-end")]
    FlexEnd,
}

#[napi_derive::napi(string_enum)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPolicy {
    #[napi(value = "no-focus")]
    NoFocus,
    #[napi(value = "tab-focus")]
    TabFocus,
    #[napi(value = "click-focus")]
    ClickFocus,
    #[napi(value = "strong-focus")]
    StrongFocus,
}

#[napi_derive::napi(discriminant = "type", discriminant_case = "lowercase")]
#[derive(Debug, Clone)]
pub enum QtHostEvent {
    App {
        name: String,
    },
    Debug {
        name: String,
    },
    Inspect {
        node_id: u32,
    },
    Listener {
        node_id: u32,
        listener_id: u16,
        trace_id: Option<i64>,
        values: Vec<QtListenerValue>,
    },
    ListenerBatch {
        node_id: u32,
        listener_ids: Vec<u16>,
        trace_id: Option<i64>,
        values: Vec<QtListenerValue>,
    },
}

#[napi_derive::napi(object)]
#[derive(Debug, Clone)]
pub struct QtListenerValue {
    pub path: String,
    pub kind_tag: u8,
    pub string_value: Option<String>,
    pub bool_value: Option<bool>,
    pub i32_value: Option<i32>,
    pub f64_value: Option<f64>,
}

#[napi_derive::napi(object)]
#[derive(Debug, Clone)]
pub struct QtInitialProp {
    pub key: String,
    pub string_value: Option<String>,
    pub bool_value: Option<bool>,
    pub i32_value: Option<i32>,
    pub f64_value: Option<f64>,
}

#[napi_derive::napi(object)]
#[derive(Debug, Clone)]
pub struct QtDebugNodeSnapshot {
    pub id: u32,
    pub kind: String,
    pub parent_id: Option<u32>,
    pub children: Vec<u32>,
    pub text: Option<String>,
    pub title: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub min_width: Option<i32>,
    pub min_height: Option<i32>,
    pub flex_grow: Option<i32>,
    pub flex_shrink: Option<i32>,
    pub enabled: Option<bool>,
    pub placeholder: Option<String>,
    pub checked: Option<bool>,
    pub flex_direction: Option<FlexDirection>,
    pub justify_content: Option<JustifyContent>,
    pub align_items: Option<AlignItems>,
    pub gap: Option<i32>,
    pub padding: Option<i32>,
    pub value: Option<f64>,
}

#[napi_derive::napi(object)]
#[derive(Debug, Clone)]
pub struct QtWindowHostCapabilities {
    pub backend_kind: String,
    pub supports_zero_timeout_pump: bool,
    pub supports_external_wake: bool,
    pub supports_fd_bridge: bool,
}

#[napi_derive::napi(object)]
#[derive(Debug, Clone)]
pub struct QtWindowHostInfo {
    pub enabled: bool,
    pub backend_name: String,
    pub capabilities: QtWindowHostCapabilities,
}

#[napi_derive::napi(object)]
#[derive(Debug, Clone)]
pub struct QtDebugSnapshot {
    pub host_runtime: String,
    pub window_host_backend: Option<String>,
    pub window_host_capabilities: Option<QtWindowHostCapabilities>,
    pub root_id: u32,
    pub nodes: Vec<QtDebugNodeSnapshot>,
}

#[napi_derive::napi(object)]
#[derive(Debug, Clone)]
pub struct QtDebugNodeBounds {
    pub visible: bool,
    pub screen_x: i32,
    pub screen_y: i32,
    pub width: i32,
    pub height: i32,
}

#[napi_derive::napi(string_enum)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QtWidgetCaptureFormat {
    #[napi(value = "argb32-premultiplied")]
    Argb32Premultiplied,
    #[napi(value = "rgba8-premultiplied")]
    Rgba8Premultiplied,
}

#[napi_derive::napi(string_enum)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QtWindowCaptureGrouping {
    #[napi(value = "segmented")]
    Segmented,
    #[napi(value = "whole-window")]
    WholeWindow,
}

#[napi_derive::napi(object)]
pub struct QtWidgetCapture {
    pub format: QtWidgetCaptureFormat,
    pub width_px: u32,
    pub height_px: u32,
    pub stride: u32,
    pub scale_factor: f64,
    pub bytes: Buffer,
}

#[napi_derive::napi(object)]
#[derive(Debug, Clone, Copy)]
pub struct QtWindowFrameState {
    pub seq: f64,
    pub elapsed_ms: f64,
    pub delta_ms: f64,
}

#[napi_derive::napi(object)]
#[derive(Debug, Clone)]
pub struct QtCapturedWidgetComposingPart {
    pub node_id: u32,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub width_px: u32,
    pub height_px: u32,
    pub stride: u32,
    pub scale_factor: f64,
    pub byte_length: u32,
}

#[napi_derive::napi(object)]
#[derive(Debug, Clone)]
pub struct QtWindowCaptureFrame {
    pub window_id: u32,
    pub grouping: QtWindowCaptureGrouping,
    pub frame_seq: f64,
    pub elapsed_ms: f64,
    pub delta_ms: f64,
    pub parts: Vec<QtCapturedWidgetComposingPart>,
}

#[napi_derive::napi(object)]
#[derive(Debug, Clone)]
pub struct QtTraceRecord {
    pub trace_id: i64,
    pub ts_us: i64,
    pub lane: String,
    pub stage: String,
    pub node_id: Option<u32>,
    pub listener_id: Option<u16>,
    pub prop_id: Option<u16>,
    pub detail: Option<String>,
}

#[napi_derive::napi]
pub struct QtApp {
    generation: u64,
}

#[napi_derive::napi]
#[derive(Clone)]
pub struct QtNode {
    inner: Arc<QtNodeInner>,
}

impl QtNode {
    pub(crate) fn from_inner(inner: Arc<QtNodeInner>) -> Self {
        Self { inner }
    }

    pub(crate) fn inner(&self) -> &Arc<QtNodeInner> {
        &self.inner
    }
}

impl QtApp {
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }
}

impl runtime::NodeHandle for QtNode {
    fn inner(&self) -> &Arc<QtNodeInner> {
        &self.inner
    }
}

#[napi_derive::napi]
pub fn ping() -> &'static str {
    runtime::ping()
}

#[napi_derive::napi]
impl QtApp {
    #[napi(factory)]
    pub fn start(env: Env, on_event: Function<QtHostEvent, UnknownReturnValue>) -> Result<Self> {
        let generation = runtime::start_app(env, on_event)?;
        Ok(Self { generation })
    }

    #[napi]
    pub fn shutdown(&mut self) -> Result<()> {
        runtime::shutdown_app(self.generation)
    }

    #[napi(getter)]
    pub fn root(&self) -> Result<QtNode> {
        runtime::root_node(self.generation)
    }

    #[napi]
    pub fn create_text_node(&self, value: String) -> Result<QtNode> {
        runtime::create_text_node(self.generation, value)
    }

    #[napi]
    pub fn debug_snapshot(&self) -> Result<QtDebugSnapshot> {
        runtime::debug_snapshot(self.generation)
    }

    #[napi(js_name = "__qtSolidCreateWidget")]
    pub fn qt_solid_create_widget(&self, spec_key: String) -> Result<QtNode> {
        runtime::create_widget_by_spec_key(self.generation, &spec_key)
    }

    #[napi(js_name = "__qtSolidGetNode")]
    pub fn qt_solid_get_node(&self, node_id: u32) -> Result<QtNode> {
        runtime::node_by_id(self.generation, node_id)
    }
}

#[napi_derive::napi]
impl QtNode {
    #[napi(getter)]
    pub fn id(&self) -> u32 {
        self.inner.id
    }

    #[napi(getter)]
    pub fn parent(&self) -> Result<Option<QtNode>> {
        runtime::node_parent(self)
    }

    #[napi(getter)]
    pub fn first_child(&self) -> Result<Option<QtNode>> {
        runtime::node_first_child(self)
    }

    #[napi(getter)]
    pub fn next_sibling(&self) -> Result<Option<QtNode>> {
        runtime::node_next_sibling(self)
    }

    #[napi]
    pub fn is_text_node(&self) -> bool {
        runtime::node_is_text_node(self)
    }

    #[napi]
    pub fn insert_child(&self, child: &QtNode, anchor: Option<&QtNode>) -> Result<()> {
        runtime::insert_child(self, child, anchor)
    }

    #[napi]
    pub fn remove_child(&self, child: &QtNode) -> Result<()> {
        runtime::remove_child(self, child)
    }

    #[napi]
    pub fn destroy(&self) -> Result<()> {
        runtime::destroy_node(self)
    }

    #[napi(js_name = "__qtSolidApplyStringProp")]
    pub fn qt_solid_apply_string_prop(&self, prop_id: u16, value: String) -> Result<()> {
        runtime::apply_string_prop_by_id(self, prop_id, value)
    }

    #[napi(js_name = "__qtSolidApplyStringPropByName")]
    pub fn qt_solid_apply_string_prop_by_name(&self, js_name: String, value: String) -> Result<()> {
        runtime::apply_string_prop_by_name(self, &js_name, value)
    }

    #[napi(js_name = "__qtSolidApplyBoolProp")]
    pub fn qt_solid_apply_bool_prop(&self, prop_id: u16, value: bool) -> Result<()> {
        runtime::apply_bool_prop_by_id(self, prop_id, value)
    }

    #[napi(js_name = "__qtSolidApplyBoolPropByName")]
    pub fn qt_solid_apply_bool_prop_by_name(&self, js_name: String, value: bool) -> Result<()> {
        runtime::apply_bool_prop_by_name(self, &js_name, value)
    }

    #[napi(js_name = "__qtSolidApplyI32Prop")]
    pub fn qt_solid_apply_i32_prop(&self, prop_id: u16, value: i32) -> Result<()> {
        runtime::apply_i32_prop_by_id(self, prop_id, value)
    }

    #[napi(js_name = "__qtSolidApplyI32PropByName")]
    pub fn qt_solid_apply_i32_prop_by_name(&self, js_name: String, value: i32) -> Result<()> {
        runtime::apply_i32_prop_by_name(self, &js_name, value)
    }

    #[napi(js_name = "__qtSolidApplyF64Prop")]
    pub fn qt_solid_apply_f64_prop(&self, prop_id: u16, value: f64) -> Result<()> {
        runtime::apply_f64_prop_by_id(self, prop_id, value)
    }

    #[napi(js_name = "__qtSolidApplyF64PropByName")]
    pub fn qt_solid_apply_f64_prop_by_name(&self, js_name: String, value: f64) -> Result<()> {
        runtime::apply_f64_prop_by_name(self, &js_name, value)
    }

    #[napi(js_name = "__qtSolidApplyEnumProp")]
    pub fn qt_solid_apply_enum_prop(&self, prop_id: u16, value: String) -> Result<()> {
        runtime::apply_enum_prop_by_id(self, prop_id, &value)
    }

    #[napi(js_name = "__qtSolidApplyEnumPropByName")]
    pub fn qt_solid_apply_enum_prop_by_name(&self, js_name: String, value: String) -> Result<()> {
        runtime::apply_enum_prop_by_name(self, &js_name, &value)
    }

    #[napi(js_name = "__qtSolidReadStringPropByName")]
    pub fn qt_solid_read_string_prop_by_name(&self, js_name: String) -> Result<String> {
        runtime::read_string_prop_by_name(self, &js_name)
    }

    #[napi(js_name = "__qtSolidReadBoolPropByName")]
    pub fn qt_solid_read_bool_prop_by_name(&self, js_name: String) -> Result<bool> {
        runtime::read_bool_prop_by_name(self, &js_name)
    }

    #[napi(js_name = "__qtSolidReadI32PropByName")]
    pub fn qt_solid_read_i32_prop_by_name(&self, js_name: String) -> Result<i32> {
        runtime::read_i32_prop_by_name(self, &js_name)
    }

    #[napi(js_name = "__qtSolidReadF64PropByName")]
    pub fn qt_solid_read_f64_prop_by_name(&self, js_name: String) -> Result<f64> {
        runtime::read_f64_prop_by_name(self, &js_name)
    }

    #[napi(js_name = "__qtSolidRequestRepaint")]
    pub fn qt_solid_request_repaint(&self) -> Result<()> {
        runtime::request_repaint_exact(self)
    }

    #[napi(js_name = "__qtSolidRequestNextFrame")]
    pub fn qt_solid_request_next_frame(&self) -> Result<()> {
        runtime::request_next_frame_exact(self)
    }

    #[napi(js_name = "__qtSolidReadWindowFrameState")]
    pub fn qt_solid_read_window_frame_state(&self) -> Result<QtWindowFrameState> {
        runtime::read_window_frame_state_exact(self)
    }

    #[napi(js_name = "__qtSolidCaptureWidget")]
    pub fn qt_solid_capture_widget(&self) -> Result<QtWidgetCapture> {
        let capture = runtime::capture_widget_exact(self)?;
        let format = match capture.format() {
            qt_solid_widget_core::runtime::WidgetCaptureFormat::Argb32Premultiplied => {
                QtWidgetCaptureFormat::Argb32Premultiplied
            }
            qt_solid_widget_core::runtime::WidgetCaptureFormat::Rgba8Premultiplied => {
                QtWidgetCaptureFormat::Rgba8Premultiplied
            }
        };
        let stride = u32::try_from(capture.stride()).map_err(|_| {
            napi::Error::new(
                napi::Status::InvalidArg,
                "widget capture stride exceeds u32".to_owned(),
            )
        })?;

        Ok(QtWidgetCapture {
            format,
            width_px: capture.width_px(),
            height_px: capture.height_px(),
            stride,
            scale_factor: capture.scale_factor(),
            bytes: Buffer::from(capture.into_bytes()),
        })
    }
}

include!(concat!(env!("OUT_DIR"), "/qt_widget_entities.rs"));

include!(concat!(env!("OUT_DIR"), "/qt_node_methods.rs"));

#[napi_derive::napi(js_name = "__qtSolidDebugScheduleTimerEvent")]
pub fn qt_solid_debug_schedule_timer_event(delay_ms: u32, event: String) -> Result<()> {
    runtime::schedule_debug_event(delay_ms, event)
}

#[napi_derive::napi(js_name = "__qtSolidDebugClickNode")]
pub fn qt_solid_debug_click_node(node_id: u32) -> Result<()> {
    runtime::debug_click_node(node_id)
}

#[napi_derive::napi(js_name = "__qtSolidDebugCloseNode")]
pub fn qt_solid_debug_close_node(node_id: u32) -> Result<()> {
    runtime::debug_close_node(node_id)
}

#[napi_derive::napi(js_name = "__qtSolidDebugInputInsertText")]
pub fn qt_solid_debug_input_insert_text(node_id: u32, value: String) -> Result<()> {
    runtime::debug_input_insert_text(node_id, value)
}

#[napi_derive::napi(js_name = "__qtSolidDebugHighlightNode")]
pub fn qt_solid_debug_highlight_node(node_id: u32) -> Result<()> {
    runtime::debug_highlight_node(node_id)
}

#[napi_derive::napi(js_name = "__qtSolidDebugGetNodeBounds")]
pub fn qt_solid_debug_get_node_bounds(node_id: u32) -> Result<QtDebugNodeBounds> {
    runtime::debug_node_bounds(node_id)
}

#[napi_derive::napi(js_name = "__qtSolidDebugGetNodeAtPoint")]
pub fn qt_solid_debug_get_node_at_point(screen_x: i32, screen_y: i32) -> Result<Option<u32>> {
    runtime::debug_node_at_point(screen_x, screen_y)
}

#[napi_derive::napi(js_name = "__qtSolidDebugCaptureWindowFrame")]
pub fn qt_solid_debug_capture_window_frame(window_id: u32) -> Result<QtWindowCaptureFrame> {
    runtime::debug_capture_window_frame(window_id)
}

#[napi_derive::napi(js_name = "__qtSolidDebugSetInspectMode")]
pub fn qt_solid_debug_set_inspect_mode(enabled: bool) -> Result<()> {
    runtime::debug_set_inspect_mode(enabled)
}

#[napi_derive::napi(js_name = "__qtSolidDebugClearHighlight")]
pub fn qt_solid_debug_clear_highlight() -> Result<()> {
    runtime::debug_clear_highlight()
}

#[napi_derive::napi(js_name = "__qtSolidDebugEmitAppEvent")]
pub fn qt_solid_debug_emit_app_event(name: String) -> Result<()> {
    runtime::debug_emit_app_event(name)
}

#[napi_derive::napi(js_name = "__qtSolidWindowHostInfo")]
pub fn qt_solid_window_host_info() -> QtWindowHostInfo {
    runtime::window_host_info()
}

#[napi_derive::napi(js_name = "__qtSolidTraceSetEnabled")]
pub fn qt_solid_trace_set_enabled(enabled: bool) {
    crate::trace::set_enabled(enabled);
}

#[napi_derive::napi(js_name = "__qtSolidTraceClear")]
pub fn qt_solid_trace_clear() {
    crate::trace::clear();
}

#[napi_derive::napi(js_name = "__qtSolidTraceSnapshot")]
pub fn qt_solid_trace_snapshot() -> Vec<QtTraceRecord> {
    crate::trace::snapshot()
}

#[napi_derive::napi(js_name = "__qtSolidTraceEnterInteraction")]
pub fn qt_solid_trace_enter_interaction(trace_id: i64) {
    crate::trace::enter_interaction(trace_id as u64);
}

#[napi_derive::napi(js_name = "__qtSolidTraceExitInteraction")]
pub fn qt_solid_trace_exit_interaction() {
    crate::trace::exit_interaction();
}

#[napi_derive::napi(js_name = "__qtSolidTraceRecordJs")]
pub fn qt_solid_trace_record_js(
    trace_id: i64,
    stage: String,
    node_id: Option<u32>,
    listener_id: Option<u16>,
    prop_id: Option<u16>,
    detail: Option<String>,
) {
    crate::trace::record_dynamic(
        trace_id as u64,
        "js".to_owned(),
        stage,
        node_id,
        listener_id,
        prop_id,
        detail,
    );
}
