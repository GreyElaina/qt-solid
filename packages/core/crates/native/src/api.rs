use std::sync::Arc;

use napi::{
    bindgen_prelude::{Buffer, Function},
    threadsafe_function::UnknownReturnValue,
    Env, Result,
};

use crate::runtime::{self, QtNodeInner};

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
pub enum FlexWrap {
    #[napi(value = "nowrap")]
    Nowrap,
    #[napi(value = "wrap")]
    Wrap,
    #[napi(value = "wrap-reverse")]
    WrapReverse,
}

#[napi_derive::napi(string_enum)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignSelf {
    #[napi(value = "auto")]
    Auto,
    #[napi(value = "flex-start")]
    FlexStart,
    #[napi(value = "flex-end")]
    FlexEnd,
    #[napi(value = "center")]
    Center,
    #[napi(value = "stretch")]
    Stretch,
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

#[napi_derive::napi(discriminant = "prop", discriminant_case = "camelCase")]
#[derive(Debug, Clone)]
pub enum WindowPropUpdate {
    Title { value: String },
    Width { value: i32 },
    Height { value: i32 },
    MinWidth { value: i32 },
    MinHeight { value: i32 },
    Visible { value: bool },
    Enabled { value: bool },
    Frameless { value: bool },
    TransparentBackground { value: bool },
    AlwaysOnTop { value: bool },
    Gpu { value: bool },
    WindowKind { value: i32 },
    ScreenX { value: i32 },
    ScreenY { value: i32 },
    Text { value: String },
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
    },
    CanvasPointer {
        canvas_node_id: u32,
        fragment_id: i32,
        event_tag: u8,
        x: f64,
        y: f64,
    },
    CanvasContextMenu {
        canvas_node_id: u32,
        fragment_id: i32,
        x: f64,
        y: f64,
        screen_x: f64,
        screen_y: f64,
    },
    CanvasKeyboard {
        canvas_node_id: u32,
        fragment_id: i32,
        event_tag: u8,
        qt_key: i32,
        modifiers: u32,
        text: String,
        repeat: bool,
        native_scan_code: u32,
        native_virtual_key: u32,
    },
    CanvasWheel {
        canvas_node_id: u32,
        fragment_id: i32,
        delta_x: f64,
        delta_y: f64,
        pixel_dx: f64,
        pixel_dy: f64,
        x: f64,
        y: f64,
        modifiers: u32,
        phase: u32,
    },
    CanvasMotionComplete {
        canvas_node_id: u32,
        fragment_id: u32,
    },
    CanvasFocusChange {
        canvas_node_id: u32,
        old_fragment_id: i32,
        new_fragment_id: i32,
    },
    CanvasTextInputChange {
        canvas_node_id: u32,
        fragment_id: u32,
        text: String,
        cursor: i32,
        sel_start: i32,
        sel_end: i32,
    },
    FragmentLayout {
        canvas_node_id: u32,
        fragment_id: u32,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
    WindowFocusChange {
        node_id: u32,
        gained: bool,
    },
    WindowResize {
        node_id: u32,
        width: f64,
        height: f64,
    },
    WindowStateChange {
        node_id: u32,
        state: u8,
    },
    ColorSchemeChange {
        scheme: String,
    },
    ScreenDpiChange {
        dpi: f64,
    },
    FileDialogResult {
        request_id: u32,
        paths: Vec<String>,
    },
}

#[napi_derive::napi(object)]
#[derive(Debug, Clone)]
pub struct QtMotionTarget {
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub scale_x: Option<f64>,
    pub scale_y: Option<f64>,
    pub rotate: Option<f64>,
    pub opacity: Option<f64>,
    pub origin_x: Option<f64>,
    pub origin_y: Option<f64>,
    // Keyframe arrays (override scalar when set)
    pub x_keyframes: Option<Vec<f64>>,
    pub y_keyframes: Option<Vec<f64>>,
    pub scale_x_keyframes: Option<Vec<f64>>,
    pub scale_y_keyframes: Option<Vec<f64>>,
    pub rotate_keyframes: Option<Vec<f64>>,
    pub opacity_keyframes: Option<Vec<f64>>,
    pub origin_x_keyframes: Option<Vec<f64>>,
    pub origin_y_keyframes: Option<Vec<f64>>,
    // Paint (colors as 0.0-1.0 float channels)
    pub background_r: Option<f64>,
    pub background_g: Option<f64>,
    pub background_b: Option<f64>,
    pub background_a: Option<f64>,
    pub border_radius: Option<f64>,
    pub blur_radius: Option<f64>,
    pub shadow_offset_x: Option<f64>,
    pub shadow_offset_y: Option<f64>,
    pub shadow_blur_radius: Option<f64>,
    pub shadow_r: Option<f64>,
    pub shadow_g: Option<f64>,
    pub shadow_b: Option<f64>,
    pub shadow_a: Option<f64>,
}

#[napi_derive::napi(string_enum)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QtTransitionType {
    #[napi(value = "tween")]
    Tween,
    #[napi(value = "spring")]
    Spring,
    #[napi(value = "instant")]
    Instant,
}

#[napi_derive::napi(object)]
#[derive(Debug, Clone)]
pub struct QtTransitionSpec {
    pub r#type: QtTransitionType,
    // Tween fields
    pub duration: Option<f64>,
    pub ease: Option<Vec<f64>>,
    // Spring fields
    pub stiffness: Option<f64>,
    pub damping: Option<f64>,
    pub mass: Option<f64>,
    pub velocity: Option<f64>,
    pub rest_delta: Option<f64>,
    pub rest_speed: Option<f64>,
    // Repeat (tween only)
    pub repeat: Option<f64>,
    pub repeat_type: Option<String>,
    // Keyframe timing
    pub times: Option<Vec<f64>>,
}

#[napi_derive::napi(object)]
#[derive(Debug, Clone)]
pub struct QtPerPropertyTransition {
    pub default: Option<QtTransitionSpec>,
    pub x: Option<QtTransitionSpec>,
    pub y: Option<QtTransitionSpec>,
    pub scale_x: Option<QtTransitionSpec>,
    pub scale_y: Option<QtTransitionSpec>,
    pub rotate: Option<QtTransitionSpec>,
    pub opacity: Option<QtTransitionSpec>,
    pub origin_x: Option<QtTransitionSpec>,
    pub origin_y: Option<QtTransitionSpec>,
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

#[napi_derive::napi(object)]
#[derive(Debug, Clone)]
pub struct QtScreenGeometryInfo {
    pub x: i32,
    pub y: i32,
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

impl runtime::NodeHandle for QtNode {
    fn inner(&self) -> &Arc<QtNodeInner> {
        &self.inner
    }
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
    pub fn debug_snapshot(&self) -> Result<QtDebugSnapshot> {
        runtime::debug_snapshot(self.generation)
    }

    #[napi(js_name = "createWidget")]
    pub fn qt_solid_create_widget(&self) -> Result<QtNode> {
        runtime::create_widget(self.generation)
    }

    #[napi(js_name = "getNode")]
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

    #[napi(js_name = "wireEvent")]
    pub fn qt_solid_wire_event(&self, export_id: u16) -> Result<()> {
        runtime::wire_event(self, export_id)
    }

    #[napi(js_name = "applyProp")]
    pub fn qt_solid_apply_prop(&self, update: WindowPropUpdate) -> Result<()> {
        runtime::apply_prop(self, update)
    }


    #[napi(js_name = "requestRepaint")]
    pub fn qt_solid_request_repaint(&self) -> Result<()> {
        runtime::request_repaint(self)
    }

    #[napi(js_name = "requestNextFrame")]
    pub fn qt_solid_request_next_frame(&self) -> Result<()> {
        runtime::request_next_frame_exact(self)
    }

    #[napi(js_name = "readWindowFrameState")]
    pub fn qt_solid_read_window_frame_state(&self) -> Result<QtWindowFrameState> {
        runtime::read_window_frame_state_exact(self)
    }

    #[napi(js_name = "captureWidget")]
    pub fn qt_solid_capture_widget(&self) -> Result<QtWidgetCapture> {
        let capture = runtime::capture_widget_exact(self)?;
        let format = match capture.format() {
            crate::runtime::capture::WidgetCaptureFormat::Argb32Premultiplied => {
                QtWidgetCaptureFormat::Argb32Premultiplied
            }
            crate::runtime::capture::WidgetCaptureFormat::Rgba8Premultiplied => {
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

#[napi_derive::napi(js_name = "scheduleTimerEvent")]
pub fn qt_solid_schedule_timer_event(delay_ms: u32, event: String) -> Result<()> {
    runtime::schedule_debug_event(delay_ms, event)
}

#[napi_derive::napi(js_name = "clickNode")]
pub fn qt_solid_click_node(node_id: u32) -> Result<()> {
    runtime::debug_click_node(node_id)
}

#[napi_derive::napi(js_name = "closeNode")]
pub fn qt_solid_close_node(node_id: u32) -> Result<()> {
    runtime::debug_close_node(node_id)
}

#[napi_derive::napi(js_name = "inputInsertText")]
pub fn qt_solid_input_insert_text(node_id: u32, value: String) -> Result<()> {
    runtime::debug_input_insert_text(node_id, value)
}

#[napi_derive::napi(js_name = "highlightNode")]
pub fn qt_solid_highlight_node(node_id: u32) -> Result<()> {
    runtime::debug_highlight_node(node_id)
}

#[napi_derive::napi(js_name = "getNodeBounds")]
pub fn qt_solid_get_node_bounds(node_id: u32) -> Result<QtDebugNodeBounds> {
    runtime::debug_node_bounds(node_id)
}

#[napi_derive::napi(js_name = "getScreenGeometry")]
pub fn qt_solid_get_screen_geometry(node_id: u32) -> Result<QtScreenGeometryInfo> {
    if !crate::qt::qt_host_started() {
        return Err(napi::Error::from_reason(
            "call QtApp.start before reading screen geometry",
        ));
    }
    let geo = crate::qt::get_screen_geometry(node_id);
    Ok(QtScreenGeometryInfo {
        x: geo.x,
        y: geo.y,
        width: geo.width,
        height: geo.height,
    })
}

#[napi_derive::napi(js_name = "focusWidget")]
pub fn qt_solid_focus_widget(node_id: u32) -> Result<()> {
    crate::qt::focus_widget(node_id)
        .map_err(|e| napi::Error::from_reason(e.what().to_owned()))
}

#[napi_derive::napi(js_name = "getWidgetSizeHint")]
pub fn qt_solid_get_widget_size_hint(node_id: u32) -> Result<QtScreenGeometryInfo> {
    if !crate::qt::qt_host_started() {
        return Err(napi::Error::from_reason(
            "call QtApp.start before reading widget size hint",
        ));
    }
    let hint = crate::qt::get_widget_size_hint(node_id);
    Ok(QtScreenGeometryInfo {
        x: hint.x,
        y: hint.y,
        width: hint.width,
        height: hint.height,
    })
}

#[napi_derive::napi(js_name = "setWindowTransientOwner")]
pub fn qt_solid_set_window_transient_owner(window_id: u32, owner_id: u32) -> Result<()> {
    crate::qt::qt_set_window_transient_owner(window_id, owner_id)
        .map_err(|e| napi::Error::from_reason(e.what().to_owned()))
}

#[napi_derive::napi(js_name = "getNodeAtPoint")]
pub fn qt_solid_get_node_at_point(screen_x: i32, screen_y: i32) -> Result<Option<u32>> {
    runtime::debug_node_at_point(screen_x, screen_y)
}

#[napi_derive::napi(js_name = "captureWindowFrame")]
pub fn qt_solid_capture_window_frame(window_id: u32) -> Result<QtWindowCaptureFrame> {
    runtime::debug_capture_window_frame(window_id)
}

#[napi_derive::napi(js_name = "setInspectMode")]
pub fn qt_solid_set_inspect_mode(enabled: bool) -> Result<()> {
    runtime::debug_set_inspect_mode(enabled)
}

#[napi_derive::napi(js_name = "clearHighlight")]
pub fn qt_solid_clear_highlight() -> Result<()> {
    runtime::debug_clear_highlight()
}

#[napi_derive::napi(js_name = "emitAppEvent")]
pub fn qt_solid_emit_app_event(name: String) -> Result<()> {
    runtime::debug_emit_app_event(name)
}

#[napi_derive::napi(js_name = "windowHostInfo")]
pub fn qt_solid_window_host_info() -> QtWindowHostInfo {
    runtime::window_host_info()
}

#[napi_derive::napi(js_name = "traceSetEnabled")]
pub fn qt_solid_trace_set_enabled(enabled: bool) {
    crate::trace::set_enabled(enabled);
}

#[napi_derive::napi(js_name = "traceClear")]
pub fn qt_solid_trace_clear() {
    crate::trace::clear();
}

#[napi_derive::napi(js_name = "traceSnapshot")]
pub fn qt_solid_trace_snapshot() -> Vec<QtTraceRecord> {
    crate::trace::snapshot()
}

#[napi_derive::napi(js_name = "traceEnterInteraction")]
pub fn qt_solid_trace_enter_interaction(trace_id: i64) {
    crate::trace::enter_interaction(trace_id as u64);
}

#[napi_derive::napi(js_name = "traceExitInteraction")]
pub fn qt_solid_trace_exit_interaction() {
    crate::trace::exit_interaction();
}

#[napi_derive::napi(js_name = "traceRecordJs")]
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


fn lower_motion_target(target: &QtMotionTarget) -> Vec<(motion::PropertyKey, f64)> {
    let mut out = Vec::with_capacity(8);
    if let Some(v) = target.x { out.push((motion::PropertyKey::X, v)); }
    if let Some(v) = target.y { out.push((motion::PropertyKey::Y, v)); }
    if let Some(v) = target.scale_x { out.push((motion::PropertyKey::ScaleX, v)); }
    if let Some(v) = target.scale_y { out.push((motion::PropertyKey::ScaleY, v)); }
    if let Some(v) = target.rotate { out.push((motion::PropertyKey::Rotate, v)); }
    if let Some(v) = target.opacity { out.push((motion::PropertyKey::Opacity, v)); }
    if let Some(v) = target.origin_x { out.push((motion::PropertyKey::OriginX, v)); }
    if let Some(v) = target.origin_y { out.push((motion::PropertyKey::OriginY, v)); }
    // Paint
    if let Some(v) = target.background_r { out.push((motion::PropertyKey::BackgroundR, v)); }
    if let Some(v) = target.background_g { out.push((motion::PropertyKey::BackgroundG, v)); }
    if let Some(v) = target.background_b { out.push((motion::PropertyKey::BackgroundB, v)); }
    if let Some(v) = target.background_a { out.push((motion::PropertyKey::BackgroundA, v)); }
    if let Some(v) = target.border_radius { out.push((motion::PropertyKey::BorderRadius, v)); }
    if let Some(v) = target.blur_radius { out.push((motion::PropertyKey::BlurRadius, v)); }
    if let Some(v) = target.shadow_offset_x { out.push((motion::PropertyKey::ShadowOffsetX, v)); }
    if let Some(v) = target.shadow_offset_y { out.push((motion::PropertyKey::ShadowOffsetY, v)); }
    if let Some(v) = target.shadow_blur_radius { out.push((motion::PropertyKey::ShadowBlurRadius, v)); }
    if let Some(v) = target.shadow_r { out.push((motion::PropertyKey::ShadowR, v)); }
    if let Some(v) = target.shadow_g { out.push((motion::PropertyKey::ShadowG, v)); }
    if let Some(v) = target.shadow_b { out.push((motion::PropertyKey::ShadowB, v)); }
    if let Some(v) = target.shadow_a { out.push((motion::PropertyKey::ShadowA, v)); }
    out
}

fn lower_motion_target_keyframes(target: &QtMotionTarget) -> Vec<(motion::PropertyKey, Vec<f64>)> {
    let mut out = Vec::with_capacity(8);
    macro_rules! prop {
        ($scalar:expr, $kf:expr, $key:expr) => {
            if let Some(kf) = &$kf {
                if kf.len() >= 2 { out.push(($key, kf.clone())); }
            } else if let Some(v) = $scalar {
                out.push(($key, vec![v]));
            }
        };
    }
    prop!(target.x, target.x_keyframes, motion::PropertyKey::X);
    prop!(target.y, target.y_keyframes, motion::PropertyKey::Y);
    prop!(target.scale_x, target.scale_x_keyframes, motion::PropertyKey::ScaleX);
    prop!(target.scale_y, target.scale_y_keyframes, motion::PropertyKey::ScaleY);
    prop!(target.rotate, target.rotate_keyframes, motion::PropertyKey::Rotate);
    prop!(target.opacity, target.opacity_keyframes, motion::PropertyKey::Opacity);
    prop!(target.origin_x, target.origin_x_keyframes, motion::PropertyKey::OriginX);
    prop!(target.origin_y, target.origin_y_keyframes, motion::PropertyKey::OriginY);
    // Paint properties remain scalar-only
    if let Some(v) = target.background_r { out.push((motion::PropertyKey::BackgroundR, vec![v])); }
    if let Some(v) = target.background_g { out.push((motion::PropertyKey::BackgroundG, vec![v])); }
    if let Some(v) = target.background_b { out.push((motion::PropertyKey::BackgroundB, vec![v])); }
    if let Some(v) = target.background_a { out.push((motion::PropertyKey::BackgroundA, vec![v])); }
    if let Some(v) = target.border_radius { out.push((motion::PropertyKey::BorderRadius, vec![v])); }
    if let Some(v) = target.blur_radius { out.push((motion::PropertyKey::BlurRadius, vec![v])); }
    if let Some(v) = target.shadow_offset_x { out.push((motion::PropertyKey::ShadowOffsetX, vec![v])); }
    if let Some(v) = target.shadow_offset_y { out.push((motion::PropertyKey::ShadowOffsetY, vec![v])); }
    if let Some(v) = target.shadow_blur_radius { out.push((motion::PropertyKey::ShadowBlurRadius, vec![v])); }
    if let Some(v) = target.shadow_r { out.push((motion::PropertyKey::ShadowR, vec![v])); }
    if let Some(v) = target.shadow_g { out.push((motion::PropertyKey::ShadowG, vec![v])); }
    if let Some(v) = target.shadow_b { out.push((motion::PropertyKey::ShadowB, vec![v])); }
    if let Some(v) = target.shadow_a { out.push((motion::PropertyKey::ShadowA, vec![v])); }
    out
}

fn lower_transition_spec(spec: &QtTransitionSpec) -> motion::TransitionSpec {
    match spec.r#type {
        QtTransitionType::Instant => motion::TransitionSpec::Instant,
        QtTransitionType::Tween => {
            let duration = spec.duration.unwrap_or(0.3);
            let easing = match spec.ease.as_deref() {
                Some(&[x1, y1, x2, y2]) => motion::Easing::cubic(x1, y1, x2, y2),
                _ => motion::Easing::EASE_IN_OUT,
            };
            let repeat = spec.repeat.and_then(|r| {
                use motion::transition::{RepeatConfig, RepeatCount, RepeatType};
                let count = if r.is_infinite() {
                    RepeatCount::Infinite
                } else if r >= 1.0 {
                    RepeatCount::Finite(r as u32)
                } else {
                    return None;
                };
                let repeat_type = match spec.repeat_type.as_deref() {
                    Some("reverse") => RepeatType::Reverse,
                    _ => RepeatType::Loop,
                };
                Some(RepeatConfig { count, repeat_type })
            });
            motion::TransitionSpec::Tween {
                duration_secs: duration,
                easing,
                repeat,
                times: spec.times.clone(),
            }
        }
        QtTransitionType::Spring => {
            let params = motion::SpringParams {
                stiffness: spec.stiffness.unwrap_or(100.0),
                damping: spec.damping.unwrap_or(10.0),
                mass: spec.mass.unwrap_or(1.0),
                initial_velocity: spec.velocity.unwrap_or(0.0),
                rest_delta: spec.rest_delta.unwrap_or(0.01),
                rest_speed: spec.rest_speed.unwrap_or(0.01),
            };
            motion::TransitionSpec::Spring(params)
        }
    }
}

fn lower_per_property_transitions(
    t: &QtPerPropertyTransition,
) -> std::collections::HashMap<motion::PropertyKey, motion::TransitionSpec> {
    let mut map = std::collections::HashMap::new();
    if let Some(s) = &t.x { map.insert(motion::PropertyKey::X, lower_transition_spec(s)); }
    if let Some(s) = &t.y { map.insert(motion::PropertyKey::Y, lower_transition_spec(s)); }
    if let Some(s) = &t.scale_x { map.insert(motion::PropertyKey::ScaleX, lower_transition_spec(s)); }
    if let Some(s) = &t.scale_y { map.insert(motion::PropertyKey::ScaleY, lower_transition_spec(s)); }
    if let Some(s) = &t.rotate { map.insert(motion::PropertyKey::Rotate, lower_transition_spec(s)); }
    if let Some(s) = &t.opacity { map.insert(motion::PropertyKey::Opacity, lower_transition_spec(s)); }
    if let Some(s) = &t.origin_x { map.insert(motion::PropertyKey::OriginX, lower_transition_spec(s)); }
    if let Some(s) = &t.origin_y { map.insert(motion::PropertyKey::OriginY, lower_transition_spec(s)); }
    map
}

// ---------------------------------------------------------------------------
// Canvas fragment store FFI
// ---------------------------------------------------------------------------

use crate::canvas::fragment::{
    self as fragment_store, FragmentId,
    ShapedTextCache, ShapedTextLayout, ShapedRun, ShapedTextLine, RasterizedGlyph,
};
use crate::canvas::fragment::decl::FragmentValue;
use crate::canvas::vello::peniko::kurbo::{BezPath, PathEl, Point};
use crate::canvas::vello::peniko as peniko_crate;

fn build_rasterized_glyphs(
    rasterized: &[crate::qt::ffi::bridge::QtRasterizedGlyph],
    dy: f64,
) -> Vec<RasterizedGlyph> {
    rasterized
        .iter()
        .filter_map(|rg| {
            if rg.width == 0 || rg.height == 0 || rg.pixels.is_empty() {
                return None;
            }
            let blob = peniko_crate::Blob::new(std::sync::Arc::new(rg.pixels.clone()));
            let image = peniko_crate::ImageData {
                data: blob,
                format: peniko_crate::ImageFormat::Rgba8,
                alpha_type: peniko_crate::ImageAlphaType::AlphaPremultiplied,
                width: rg.width,
                height: rg.height,
            };
            Some(RasterizedGlyph {
                image,
                x: rg.x + rg.bearing_x,
                y: rg.y + rg.bearing_y + dy,
                scale_factor: rg.scale_factor,
            })
        })
        .collect()
}

fn reshape_text_fragment_if_needed(canvas_node_id: u32, fragment_id: u32) {
    // Rich text path: if text_runs are present, use styled shaping.
    if fragment_store::fragment_store_read_text_style_runs(canvas_node_id, FragmentId(fragment_id)).is_some() {
        reshape_styled_text_fragment(canvas_node_id, fragment_id);
        return;
    }

    let Some((text, font_size, font_family, font_weight, font_italic, text_max_width, text_overflow)) =
        fragment_store::fragment_store_read_text_props(canvas_node_id, FragmentId(fragment_id))
    else {
        return;
    };
    if text.is_empty() {
        return;
    }

    let elide_mode: u8 = match text_overflow.as_str() {
        "clip" => 1,
        "ellipsis" => 2,
        _ => 0,
    };
    let result = crate::qt::qt_shape_text_to_path(&text, font_size, &font_family, font_weight, font_italic, text_max_width, elide_mode);

    let mut path = BezPath::new();
    for el in &result.elements {
        match el.tag {
            0 => path.push(PathEl::MoveTo(Point::new(el.x0, el.y0))),
            1 => path.push(PathEl::LineTo(Point::new(el.x0, el.y0))),
            2 => path.push(PathEl::CurveTo(
                Point::new(el.x0, el.y0),
                Point::new(el.x1, el.y1),
                Point::new(el.x2, el.y2),
            )),
            _ => {}
        }
    }

    // Vertically center glyph content within the line-metrics box so that
    // flex centering aligns to visual center.  We keep line-metrics
    // (ascent+descent) as the Taffy measure — this preserves correct line
    // spacing for normal text — but shift the path so glyphs sit centered
    // inside that box rather than at the baseline.
    use crate::canvas::vello::peniko::kurbo::Shape;
    let bbox = path.bounding_box();
    let bbox_h = bbox.height();
    let dy = if bbox_h > 0.0 && result.total_height > bbox_h {
        (result.total_height - bbox_h) / 2.0 - bbox.y0
    } else {
        0.0
    };
    if dy != 0.0 {
        path.apply_affine(crate::canvas::vello::peniko::kurbo::Affine::translate((0.0, dy)));
    }

    let rasterized_glyphs = build_rasterized_glyphs(
        &result.rasterized_glyphs,
        dy,
    );

    let lines: Vec<ShapedTextLine> = result.lines.iter().map(|l| ShapedTextLine {
        y_offset: l.y_offset,
        width: l.width,
        height: l.height,
        ascent: l.ascent,
        descent: l.descent,
    }).collect();

    let cache = ShapedTextCache {
        path,
        width: result.total_width,
        height: result.total_height,
        ascent: result.ascent,
        lines,
        runs: Vec::new(),
        rasterized_glyphs,
    };

    fragment_store::fragment_store_set_text_shape_cache(
        canvas_node_id,
        FragmentId(fragment_id),
        cache,
    );
}

fn reshape_styled_text_fragment(canvas_node_id: u32, fragment_id: u32) {
    let Some((text_runs, default_font_size, default_font_family, text_max_width, text_overflow)) =
        fragment_store::fragment_store_read_text_style_runs(canvas_node_id, FragmentId(fragment_id))
    else {
        return;
    };

    if text_runs.is_empty() {
        return;
    }

    // Build full text and style run descriptors for C++ shaping.
    let mut full_text = String::new();
    let mut wire_runs: Vec<crate::qt::ffi::bridge::QtTextStyleRun> = Vec::new();
    let mut utf16_offset: i32 = 0;
    for run in &text_runs {
        let run_utf16_len = run.text.encode_utf16().count() as i32;
        full_text.push_str(&run.text);
        wire_runs.push(crate::qt::ffi::bridge::QtTextStyleRun {
            start: utf16_offset,
            length: run_utf16_len,
            font_size: run.font_size,
            font_family: run.font_family.clone(),
            font_weight: run.font_weight,
            font_italic: run.font_italic,
        });
        utf16_offset += run_utf16_len;
    }

    if full_text.is_empty() {
        return;
    }

    let elide_mode: u8 = match text_overflow.as_str() {
        "clip" => 1,
        "ellipsis" => 2,
        _ => 0,
    };
    let result = crate::qt::qt_shape_styled_text_to_path(
        &full_text,
        default_font_size,
        &default_font_family,
        text_max_width,
        elide_mode,
        &wire_runs,
    );

    // Build combined path for measurement/centering.
    let mut combined_path = BezPath::new();
    for el in &result.combined_elements {
        match el.tag {
            0 => combined_path.push(PathEl::MoveTo(Point::new(el.x0, el.y0))),
            1 => combined_path.push(PathEl::LineTo(Point::new(el.x0, el.y0))),
            2 => combined_path.push(PathEl::CurveTo(
                Point::new(el.x0, el.y0),
                Point::new(el.x1, el.y1),
                Point::new(el.x2, el.y2),
            )),
            _ => {}
        }
    }

    // Vertical centering (same logic as single-style).
    use crate::canvas::vello::peniko::kurbo::Shape;
    let bbox = combined_path.bounding_box();
    let bbox_h = bbox.height();
    let dy = if bbox_h > 0.0 && result.total_height > bbox_h {
        (result.total_height - bbox_h) / 2.0 - bbox.y0
    } else {
        0.0
    };

    // Build per-run shaped paths with same vertical adjustment.
    let shaped_runs: Vec<ShapedRun> = result.runs.iter().zip(text_runs.iter()).map(|(shaped_run, source_run)| {
        let mut path = BezPath::new();
        for el in &shaped_run.elements {
            match el.tag {
                0 => path.push(PathEl::MoveTo(Point::new(el.x0, el.y0))),
                1 => path.push(PathEl::LineTo(Point::new(el.x0, el.y0))),
                2 => path.push(PathEl::CurveTo(
                    Point::new(el.x0, el.y0),
                    Point::new(el.x1, el.y1),
                    Point::new(el.x2, el.y2),
                )),
                _ => {}
            }
        }
        if dy != 0.0 {
            path.apply_affine(crate::canvas::vello::peniko::kurbo::Affine::translate((0.0, dy)));
        }
        ShapedRun {
            path,
            color: source_run.color,
        }
    }).collect();

    if dy != 0.0 {
        combined_path.apply_affine(crate::canvas::vello::peniko::kurbo::Affine::translate((0.0, dy)));
    }

    let lines: Vec<ShapedTextLine> = result.lines.iter().map(|l| ShapedTextLine {
        y_offset: l.y_offset,
        width: l.width,
        height: l.height,
        ascent: l.ascent,
        descent: l.descent,
    }).collect();

    let rasterized_glyphs = build_rasterized_glyphs(
        &result.rasterized_glyphs,
        dy,
    );

    let cache = ShapedTextCache {
        path: combined_path,
        width: result.total_width,
        height: result.total_height,
        ascent: result.ascent,
        lines,
        runs: shaped_runs,
        rasterized_glyphs,
    };

    fragment_store::fragment_store_set_text_shape_cache(
        canvas_node_id,
        FragmentId(fragment_id),
        cache,
    );
}

/// If the given fragment is a Span, return its parent Text fragment id.
fn parent_text_id_for_span(canvas_node_id: u32, fragment_id: u32) -> Option<u32> {
    fragment_store::fragment_store_parent_text_for_span(canvas_node_id, FragmentId(fragment_id))
        .map(|id| id.0)
}

fn reshape_text_input_fragment_if_needed(canvas_node_id: u32, fragment_id: u32) {
    let Some((text, font_size, font_family, font_weight, font_italic)) =
        fragment_store::fragment_store_read_text_input_props(canvas_node_id, FragmentId(fragment_id))
    else {
        return;
    };
    reshape_text_input_with(canvas_node_id, fragment_id, &text, font_size, &font_family, font_weight, font_italic);
}

fn reshape_text_input_with(canvas_node_id: u32, fragment_id: u32, text: &str, font_size: f64, font_family: &str, font_weight: i32, font_italic: bool) {
    if text.is_empty() {
        fragment_store::fragment_store_set_text_input_layout_cache(
            canvas_node_id,
            FragmentId(fragment_id),
            ShapedTextLayout {
                path: BezPath::new(),
                cursor_x_positions: vec![0.0],
                width: 0.0,
                height: font_size,
                ascent: font_size,
            },
        );
        return;
    }

    let result = crate::qt::qt_shape_text_with_cursors(text, font_size, font_family, font_weight, font_italic);

    let mut path = BezPath::new();
    for el in &result.elements {
        match el.tag {
            0 => path.push(PathEl::MoveTo(Point::new(el.x0, el.y0))),
            1 => path.push(PathEl::LineTo(Point::new(el.x0, el.y0))),
            2 => path.push(PathEl::CurveTo(
                Point::new(el.x0, el.y0),
                Point::new(el.x1, el.y1),
                Point::new(el.x2, el.y2),
            )),
            _ => {}
        }
    }

    let layout = ShapedTextLayout {
        path,
        cursor_x_positions: result.cursor_x_positions,
        width: result.total_width,
        height: result.ascent + result.descent,
        ascent: result.ascent,
    };

    fragment_store::fragment_store_set_text_input_layout_cache(
        canvas_node_id,
        FragmentId(fragment_id),
        layout,
    );
}

#[napi_derive::napi(js_name = "canvasFragmentStoreEnsure")]
pub fn canvas_fragment_store_ensure(canvas_node_id: u32) {
    fragment_store::fragment_store_ensure(canvas_node_id);
}

#[napi_derive::napi(js_name = "canvasFragmentStoreRemove")]
pub fn canvas_fragment_store_remove(canvas_node_id: u32) {
    fragment_store::fragment_store_remove(canvas_node_id);
}

#[napi_derive::napi(js_name = "canvasFragmentCreate")]
pub fn canvas_fragment_create(canvas_node_id: u32, kind: String) -> Result<u32> {
    fragment_store::fragment_store_create_node(canvas_node_id, &kind)
        .map(|id| id.0)
        .ok_or_else(|| {
            napi::Error::from_reason(format!(
                "unknown fragment kind or canvas {canvas_node_id} not found: {kind}"
            ))
        })
}

#[napi_derive::napi(js_name = "canvasFragmentInsertChild")]
pub fn canvas_fragment_insert_child(
    canvas_node_id: u32,
    parent_fragment_id: i32,
    child_fragment_id: u32,
    before_fragment_id: Option<u32>,
) {
    let parent = if parent_fragment_id < 0 {
        None
    } else {
        Some(FragmentId(parent_fragment_id as u32))
    };
    fragment_store::fragment_store_insert_child(
        canvas_node_id,
        parent,
        FragmentId(child_fragment_id),
        before_fragment_id.map(FragmentId),
    );
    // Reshape parent text if a span was inserted.
    if parent_fragment_id >= 0 {
        reshape_text_fragment_if_needed(canvas_node_id, parent_fragment_id as u32);
    }
}

#[napi_derive::napi(js_name = "canvasFragmentDetachChild")]
pub fn canvas_fragment_detach_child(
    canvas_node_id: u32,
    parent_fragment_id: i32,
    child_fragment_id: u32,
) {
    let parent = if parent_fragment_id < 0 {
        None
    } else {
        Some(FragmentId(parent_fragment_id as u32))
    };
    fragment_store::fragment_store_detach_child(
        canvas_node_id,
        parent,
        FragmentId(child_fragment_id),
    );
    // Reshape parent text after span removal.
    if parent_fragment_id >= 0 {
        reshape_text_fragment_if_needed(canvas_node_id, parent_fragment_id as u32);
    }
}

#[napi_derive::napi(js_name = "canvasFragmentDestroy")]
pub fn canvas_fragment_destroy(canvas_node_id: u32, fragment_id: u32) {
    fragment_store::fragment_store_destroy(canvas_node_id, FragmentId(fragment_id));
}

#[napi_derive::napi(js_name = "canvasFragmentSetProp")]
pub fn canvas_fragment_set_prop(
    canvas_node_id: u32,
    fragment_id: u32,
    key: String,
    value: FragmentValue,
) {
    fragment_store::fragment_store_set_prop(
        canvas_node_id,
        FragmentId(fragment_id),
        &key,
        value,
    );
    if matches!(key.as_str(), "text" | "fontSize" | "fontFamily" | "fontWeight" | "fontStyle" | "textMaxWidth" | "color") {
        reshape_text_fragment_if_needed(canvas_node_id, fragment_id);
        reshape_text_input_fragment_if_needed(canvas_node_id, fragment_id);
    }
    // When a span prop changes, reshape the parent text fragment.
    if let Some(parent_id) = parent_text_id_for_span(canvas_node_id, fragment_id) {
        reshape_text_fragment_if_needed(canvas_node_id, parent_id);
    }
}

#[napi_derive::napi(js_name = "canvasFragmentSetF64Prop")]
pub fn canvas_fragment_set_f64_prop(
    canvas_node_id: u32,
    fragment_id: u32,
    key: String,
    value: f64,
) {
    fragment_store::fragment_store_set_prop(
        canvas_node_id,
        FragmentId(fragment_id),
        &key,
        FragmentValue::F64 { value },
    );
    if matches!(key.as_str(), "fontSize" | "fontWeight" | "textMaxWidth") {
        reshape_text_fragment_if_needed(canvas_node_id, fragment_id);
        reshape_text_input_fragment_if_needed(canvas_node_id, fragment_id);
    }
    if let Some(parent_id) = parent_text_id_for_span(canvas_node_id, fragment_id) {
        reshape_text_fragment_if_needed(canvas_node_id, parent_id);
    }
}

#[napi_derive::napi(js_name = "canvasFragmentSetStringProp")]
pub fn canvas_fragment_set_string_prop(
    canvas_node_id: u32,
    fragment_id: u32,
    key: String,
    value: String,
) {
    fragment_store::fragment_store_set_prop(
        canvas_node_id,
        FragmentId(fragment_id),
        &key,
        FragmentValue::Str { value },
    );
    if matches!(key.as_str(), "text" | "fontFamily" | "fontStyle" | "color") {
        reshape_text_fragment_if_needed(canvas_node_id, fragment_id);
        reshape_text_input_fragment_if_needed(canvas_node_id, fragment_id);
    }
    if let Some(parent_id) = parent_text_id_for_span(canvas_node_id, fragment_id) {
        reshape_text_fragment_if_needed(canvas_node_id, parent_id);
    }
}

#[napi_derive::napi(js_name = "canvasFragmentSetBoolProp")]
pub fn canvas_fragment_set_bool_prop(
    canvas_node_id: u32,
    fragment_id: u32,
    key: String,
    value: bool,
) {
    fragment_store::fragment_store_set_prop(
        canvas_node_id,
        FragmentId(fragment_id),
        &key,
        FragmentValue::Bool { value },
    );
    if let Some(parent_id) = parent_text_id_for_span(canvas_node_id, fragment_id) {
        reshape_text_fragment_if_needed(canvas_node_id, parent_id);
    }
}

#[napi_derive::napi(js_name = "canvasFragmentSetEncodedImage")]
pub fn canvas_fragment_set_encoded_image(
    canvas_node_id: u32,
    fragment_id: u32,
    data: Buffer,
) -> Result<()> {
    use image::GenericImageView;
    use crate::canvas::vello::peniko;

    let decoded = image::load_from_memory(&data)
        .map_err(|e| napi::Error::from_reason(format!("image decode failed: {e}")))?;
    let rgba = decoded.to_rgba8();
    let (w, h) = decoded.dimensions();
    let blob = peniko::Blob::new(std::sync::Arc::new(rgba.into_raw()));
    let image_data = peniko::ImageData {
        data: blob,
        format: peniko::ImageFormat::Rgba8,
        alpha_type: peniko::ImageAlphaType::Alpha,
        width: w,
        height: h,
    };

    fragment_store::fragment_store_set_image_data(
        canvas_node_id,
        FragmentId(fragment_id),
        image_data,
    );
    Ok(())
}

#[napi_derive::napi(js_name = "canvasFragmentClearImage")]
pub fn canvas_fragment_clear_image(
    canvas_node_id: u32,
    fragment_id: u32,
) {
    fragment_store::fragment_store_clear_image_data(
        canvas_node_id,
        FragmentId(fragment_id),
    );
}

#[napi_derive::napi(js_name = "canvasFragmentRequestRepaint")]
pub fn canvas_fragment_request_repaint(canvas_node_id: u32) -> Result<()> {
    let generation = runtime::current_app_generation()?;
    let node = runtime::node_by_id(generation, canvas_node_id)?;
    runtime::request_repaint(&node)
}

#[napi_derive::napi(js_name = "canvasFragmentSetDebugHighlight")]
pub fn canvas_fragment_set_debug_highlight(
    canvas_node_id: u32,
    fragment_id: Option<u32>,
) -> Result<()> {
    fragment_store::fragment_store_set_debug_highlight(
        canvas_node_id,
        fragment_id.map(FragmentId),
    );
    let generation = runtime::current_app_generation()?;
    let node = runtime::node_by_id(generation, canvas_node_id)?;
    runtime::request_repaint(&node)
}

#[napi_derive::napi(js_name = "canvasFragmentComputeLayout")]
pub fn canvas_fragment_compute_layout(
    canvas_node_id: u32,
    available_width: f64,
    available_height: f64,
) -> Result<()> {
    fragment_store::fragment_store_compute_layout(canvas_node_id, available_width, available_height);
    let generation = runtime::current_app_generation()?;
    let node = runtime::node_by_id(generation, canvas_node_id)?;
    runtime::request_repaint(&node)
}

#[napi_derive::napi(js_name = "canvasFragmentSetListener")]
pub fn canvas_fragment_set_listener(
    canvas_node_id: u32,
    fragment_id: u32,
    listener_bit: u32,
    enabled: bool,
) {
    fragment_store::fragment_store_set_listener(canvas_node_id, fragment_id, listener_bit, enabled);
}

#[napi_derive::napi(js_name = "canvasFragmentSetMotionTarget")]
pub fn canvas_fragment_set_motion_target(
    canvas_node_id: u32,
    fragment_id: u32,
    target: QtMotionTarget,
    transition: QtPerPropertyTransition,
    delay: Option<f64>,
) -> Result<bool> {
    let has_keyframes = target.x_keyframes.is_some()
        || target.y_keyframes.is_some()
        || target.scale_x_keyframes.is_some()
        || target.scale_y_keyframes.is_some()
        || target.rotate_keyframes.is_some()
        || target.opacity_keyframes.is_some()
        || target.origin_x_keyframes.is_some()
        || target.origin_y_keyframes.is_some();
    let default_transition = transition
        .default
        .as_ref()
        .map(lower_transition_spec)
        .unwrap_or_default();
    let per_property = lower_per_property_transitions(&transition);
    let now = crate::qt::trace_now_ns() as f64 / 1_000_000_000.0;
    let delay_secs = delay.unwrap_or(0.0);
    let animating = if has_keyframes {
        let targets = lower_motion_target_keyframes(&target);
        let times = transition.default.as_ref().and_then(|s| s.times.clone());
        fragment_store::fragment_store_set_motion_target_keyframes(
            canvas_node_id,
            FragmentId(fragment_id),
            targets,
            times,
            &default_transition,
            &per_property,
            delay_secs,
            now,
        )
    } else {
        let targets = lower_motion_target(&target);
        fragment_store::fragment_store_set_motion_target(
            canvas_node_id,
            FragmentId(fragment_id),
            &targets,
            &default_transition,
            &per_property,
            delay_secs,
            now,
        )
    };
    let generation = runtime::current_app_generation()?;
    let node = runtime::node_by_id(generation, canvas_node_id)?;
    runtime::request_repaint(&node)?;
    Ok(animating)
}

#[napi_derive::napi(object)]
pub struct QtWorldBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[napi_derive::napi(js_name = "canvasFragmentGetWorldBounds")]
pub fn canvas_fragment_get_world_bounds(
    canvas_node_id: u32,
    fragment_id: u32,
) -> Result<Option<QtWorldBounds>> {
    let rect = fragment_store::fragment_store_get_world_bounds(
        canvas_node_id,
        FragmentId(fragment_id),
    );
    Ok(rect.map(|r| QtWorldBounds {
        x: r.x0,
        y: r.y0,
        width: r.width(),
        height: r.height(),
    }))
}

#[napi_derive::napi(js_name = "canvasFragmentSetScrollOffset")]
pub fn canvas_fragment_set_scroll_offset(
    canvas_node_id: u32,
    fragment_id: u32,
    x: f64,
    y: f64,
) -> Result<()> {
    fragment_store::fragment_store_set_scroll_offset(
        canvas_node_id,
        FragmentId(fragment_id),
        x,
        y,
    );
    let generation = runtime::current_app_generation()?;
    let node = runtime::node_by_id(generation, canvas_node_id)?;
    runtime::request_repaint(&node)
}

#[napi_derive::napi(js_name = "canvasFragmentScrollDrive")]
pub fn canvas_fragment_scroll_drive(
    canvas_node_id: u32,
    fragment_id: u32,
    x: f64,
    y: f64,
) -> Result<()> {
    let now = crate::qt::trace_now_ns() as f64 / 1_000_000_000.0;
    fragment_store::fragment_store_drive_scroll_motion(
        canvas_node_id,
        FragmentId(fragment_id),
        x,
        y,
        now,
    );
    let generation = runtime::current_app_generation()?;
    let node = runtime::node_by_id(generation, canvas_node_id)?;
    runtime::request_repaint(&node)
}

#[napi_derive::napi(js_name = "canvasFragmentScrollRelease")]
pub fn canvas_fragment_scroll_release(
    canvas_node_id: u32,
    fragment_id: u32,
    clamped_x: f64,
    clamped_y: f64,
    stiffness: Option<f64>,
    damping: Option<f64>,
) -> Result<bool> {
    let now = crate::qt::trace_now_ns() as f64 / 1_000_000_000.0;
    let spring = motion::TransitionSpec::Spring(motion::SpringParams {
        stiffness: stiffness.unwrap_or(170.0),
        damping: damping.unwrap_or(26.0),
        mass: 1.0,
        initial_velocity: 0.0,
        rest_delta: 0.5,
        rest_speed: 0.5,
    });
    let animating = fragment_store::fragment_store_release_scroll_motion(
        canvas_node_id,
        FragmentId(fragment_id),
        clamped_x,
        clamped_y,
        spring,
        now,
    );
    let generation = runtime::current_app_generation()?;
    let node = runtime::node_by_id(generation, canvas_node_id)?;
    if animating {
        runtime::request_repaint(&node)?;
    }
    Ok(animating)
}

#[napi_derive::napi(js_name = "canvasFragmentGetContentSize")]
pub fn canvas_fragment_get_content_size(
    canvas_node_id: u32,
    fragment_id: u32,
) -> Option<QtWorldBounds> {
    let size = fragment_store::fragment_store_get_content_size(
        canvas_node_id,
        FragmentId(fragment_id),
    );
    size.map(|(w, h)| QtWorldBounds {
        x: 0.0,
        y: 0.0,
        width: w,
        height: h,
    })
}

#[napi_derive::napi(js_name = "canvasFragmentSetLayoutFlip")]
pub fn canvas_fragment_set_layout_flip(
    canvas_node_id: u32,
    fragment_id: u32,
    dx: f64,
    dy: f64,
    sx: f64,
    sy: f64,
    transition: QtTransitionSpec,
) -> Result<bool> {
    let spec = lower_transition_spec(&transition);
    let now = crate::qt::trace_now_ns() as f64 / 1_000_000_000.0;
    let animating = fragment_store::fragment_store_set_layout_flip(
        canvas_node_id,
        FragmentId(fragment_id),
        dx, dy, sx, sy,
        &spec,
        now,
    );
    let generation = runtime::current_app_generation()?;
    let node = runtime::node_by_id(generation, canvas_node_id)?;
    runtime::request_repaint(&node)?;
    Ok(animating)
}

// ---------------------------------------------------------------------------
// Clipboard
// ---------------------------------------------------------------------------

#[napi_derive::napi]
pub fn clipboard_get_text() -> Result<String> {
    Ok(crate::qt::qt_clipboard_get_text())
}

#[napi_derive::napi]
pub fn clipboard_set_text(text: String) -> Result<()> {
    crate::qt::qt_clipboard_set_text(&text);
    Ok(())
}

#[napi_derive::napi]
pub fn clipboard_has_text() -> Result<bool> {
    Ok(crate::qt::qt_clipboard_has_text())
}

#[napi_derive::napi]
pub fn clipboard_formats() -> Result<Vec<String>> {
    Ok(crate::qt::qt_clipboard_formats())
}

#[napi_derive::napi]
pub fn clipboard_get(mime: String) -> Result<Buffer> {
    let bytes = crate::qt::qt_clipboard_get(&mime);
    Ok(Buffer::from(bytes))
}

#[napi_derive::napi]
pub fn clipboard_clear() -> Result<()> {
    crate::qt::qt_clipboard_clear();
    Ok(())
}

#[napi_derive::napi(object)]
pub struct ClipboardEntry {
    pub mime: String,
    pub data: Buffer,
}

#[napi_derive::napi]
pub fn clipboard_set(entries: Vec<ClipboardEntry>) -> Result<()> {
    let cxx_entries: Vec<crate::qt::QtClipboardEntry> = entries
        .into_iter()
        .map(|e| crate::qt::QtClipboardEntry {
            mime: e.mime,
            data: e.data.to_vec(),
        })
        .collect();
    crate::qt::qt_clipboard_set(cxx_entries);
    Ok(())
}

// ---------------------------------------------------------------------------
// System theme
// ---------------------------------------------------------------------------

#[napi_derive::napi]
pub fn system_color_scheme() -> Result<String> {
    let tag = crate::qt::qt_system_color_scheme();
    let scheme = match tag {
        1 => "light",
        2 => "dark",
        _ => "unknown",
    };
    Ok(scheme.to_owned())
}

// ---------------------------------------------------------------------------
// Text measurement
// ---------------------------------------------------------------------------

#[napi_derive::napi(object)]
pub struct TextMeasurement {
    pub width: f64,
    pub height: f64,
    pub ascent: f64,
    pub descent: f64,
    pub line_count: i32,
}

#[napi_derive::napi]
pub fn measure_text(
    text: String,
    font_size: f64,
    font_family: Option<String>,
    font_weight: Option<i32>,
    font_italic: Option<bool>,
    max_width: Option<f64>,
) -> Result<TextMeasurement> {
    let family = font_family.unwrap_or_default();
    let weight = font_weight.unwrap_or(0);
    let italic = font_italic.unwrap_or(false);
    let max_w = max_width.unwrap_or(0.0);
    let m = crate::qt::qt_measure_text(&text, font_size, &family, weight, italic, max_w);
    Ok(TextMeasurement {
        width: m.width,
        height: m.height,
        ascent: m.ascent,
        descent: m.descent,
        line_count: m.line_count,
    })
}

// ---------------------------------------------------------------------------
// Screen DPI
// ---------------------------------------------------------------------------

#[napi_derive::napi(object)]
pub struct ScreenDpiInfo {
    pub dpi_x: f64,
    pub dpi_y: f64,
    pub device_pixel_ratio: f64,
    pub available_x: i32,
    pub available_y: i32,
    pub available_width: i32,
    pub available_height: i32,
}

#[napi_derive::napi]
pub fn screen_dpi_info(id: u32) -> Result<ScreenDpiInfo> {
    let info = crate::qt::qt_screen_dpi_info(id);
    Ok(ScreenDpiInfo {
        dpi_x: info.dpi_x,
        dpi_y: info.dpi_y,
        device_pixel_ratio: info.device_pixel_ratio,
        available_x: info.available_geometry.x,
        available_y: info.available_geometry.y,
        available_width: info.available_geometry.width,
        available_height: info.available_geometry.height,
    })
}

// ---------------------------------------------------------------------------
// Window state control
// ---------------------------------------------------------------------------

#[napi_derive::napi]
pub fn window_minimize(id: u32) -> Result<()> {
    crate::qt::qt_window_minimize(id).map_err(|e| napi::Error::from_reason(e.what().to_owned()))
}

#[napi_derive::napi]
pub fn window_maximize(id: u32) -> Result<()> {
    crate::qt::qt_window_maximize(id).map_err(|e| napi::Error::from_reason(e.what().to_owned()))
}

#[napi_derive::napi]
pub fn window_restore(id: u32) -> Result<()> {
    crate::qt::qt_window_restore(id).map_err(|e| napi::Error::from_reason(e.what().to_owned()))
}

#[napi_derive::napi]
pub fn window_fullscreen(id: u32, enter: bool) -> Result<()> {
    crate::qt::qt_window_fullscreen(id, enter).map_err(|e| napi::Error::from_reason(e.what().to_owned()))
}

#[napi_derive::napi]
pub fn window_is_minimized(id: u32) -> Result<bool> {
    crate::qt::qt_window_is_minimized(id).map_err(|e| napi::Error::from_reason(e.what().to_owned()))
}

#[napi_derive::napi]
pub fn window_is_maximized(id: u32) -> Result<bool> {
    crate::qt::qt_window_is_maximized(id).map_err(|e| napi::Error::from_reason(e.what().to_owned()))
}

#[napi_derive::napi]
pub fn window_is_fullscreen(id: u32) -> Result<bool> {
    crate::qt::qt_window_is_fullscreen(id).map_err(|e| napi::Error::from_reason(e.what().to_owned()))
}

// ---------------------------------------------------------------------------
// File dialogs
// ---------------------------------------------------------------------------

#[napi_derive::napi]
pub fn show_open_file_dialog(
    window_id: u32,
    title: String,
    filter: Option<String>,
    multiple: Option<bool>,
) -> Result<u32> {
    let f = filter.unwrap_or_default();
    let m = multiple.unwrap_or(false);
    Ok(crate::qt::qt_show_open_file_dialog(window_id, &title, &f, m))
}

#[napi_derive::napi]
pub fn show_save_file_dialog(
    window_id: u32,
    title: String,
    filter: Option<String>,
    default_name: Option<String>,
) -> Result<u32> {
    let f = filter.unwrap_or_default();
    let d = default_name.unwrap_or_default();
    Ok(crate::qt::qt_show_save_file_dialog(window_id, &title, &f, &d))
}

// ---------------------------------------------------------------------------
// Devtools: fragment tree snapshot & hit test
// ---------------------------------------------------------------------------

#[napi_derive::napi(object)]
pub struct QtFragmentSnapshot {
    pub id: u32,
    pub tag: String,
    pub parent_id: Option<u32>,
    pub child_ids: Vec<u32>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub clip: bool,
    pub visible: bool,
    pub opacity: f64,
    pub props: std::collections::HashMap<String, String>,
}

#[napi_derive::napi(js_name = "canvasFragmentTreeSnapshot")]
pub fn canvas_fragment_tree_snapshot(canvas_node_id: u32) -> Vec<QtFragmentSnapshot> {
    fragment_store::fragment_store_snapshot(canvas_node_id)
        .into_iter()
        .map(|s| QtFragmentSnapshot {
            id: s.id,
            tag: s.tag,
            parent_id: s.parent_id,
            child_ids: s.child_ids,
            x: s.x,
            y: s.y,
            width: s.width,
            height: s.height,
            clip: s.clip,
            visible: s.visible,
            opacity: s.opacity as f64,
            props: s.props,
        })
        .collect()
}

#[napi_derive::napi(js_name = "canvasFragmentHitTest")]
pub fn canvas_fragment_hit_test(canvas_node_id: u32, x: f64, y: f64) -> Option<u32> {
    fragment_store::fragment_store_hit_test(canvas_node_id, x, y).map(|id| id.0)
}

#[napi_derive::napi(object)]
pub struct QtLayerSnapshot {
    pub fragment_id: u32,
    pub layer_key: u32,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub opacity: f64,
    pub reasons: String,
}

#[napi_derive::napi(js_name = "canvasFragmentSnapshotLayers")]
pub fn canvas_fragment_snapshot_layers(canvas_node_id: u32) -> Vec<QtLayerSnapshot> {
    fragment_store::fragment_store_snapshot_layers(canvas_node_id)
        .into_iter()
        .map(|l| QtLayerSnapshot {
            fragment_id: l.fragment_id,
            layer_key: l.layer_key,
            x: l.x, y: l.y, width: l.width, height: l.height,
            opacity: l.opacity as f64,
            reasons: l.reasons,
        })
        .collect()
}

#[napi_derive::napi(object)]
pub struct QtAnimationChannelSnapshot {
    pub property: String,
    pub origin: f64,
    pub target: f64,
    pub state: String,
}

#[napi_derive::napi(object)]
pub struct QtAnimationSnapshot {
    pub fragment_id: u32,
    pub tag: String,
    pub channels: Vec<QtAnimationChannelSnapshot>,
}

#[napi_derive::napi(js_name = "canvasFragmentSnapshotAnimations")]
pub fn canvas_fragment_snapshot_animations(canvas_node_id: u32) -> Vec<QtAnimationSnapshot> {
    fragment_store::fragment_store_snapshot_animations(canvas_node_id)
        .into_iter()
        .map(|a| QtAnimationSnapshot {
            fragment_id: a.fragment_id,
            tag: a.tag,
            channels: a.channels.into_iter().map(|c| QtAnimationChannelSnapshot {
                property: c.property,
                origin: c.origin,
                target: c.target,
                state: c.state,
            }).collect(),
        })
        .collect()
}

#[napi_derive::napi(js_name = "captureCanvasRegion")]
pub fn capture_canvas_region(
    canvas_node_id: u32,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
) -> Option<napi::bindgen_prelude::Buffer> {
    let generation = runtime::current_app_generation().ok()?;
    let node = runtime::node_by_id(generation, canvas_node_id).ok()?;
    // Use vello capture path directly — capture_widget_exact routes window
    // nodes through Qt raster which doesn't include vello-rendered fragment content.
    let capture = crate::renderer::scheduler::capture_vello_widget_exact(&node)
        .ok()?
        .or_else(|| runtime::capture_widget_exact(&node).ok())?;

    let scale = capture.scale_factor();
    let img = capture.to_rgba_image();

    // Logical → pixel coordinates
    let px = (x * scale).round() as u32;
    let py = (y * scale).round() as u32;
    let pw = ((width * scale).round() as u32).min(img.width().saturating_sub(px));
    let ph = ((height * scale).round() as u32).min(img.height().saturating_sub(py));

    if pw == 0 || ph == 0 {
        return None;
    }

    let cropped = image::imageops::crop_imm(&img, px, py, pw, ph).to_image();
    let mut png_buf = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut png_buf);
    cropped.write_to(&mut cursor, image::ImageFormat::Png).ok()?;

    Some(png_buf.into())
}

#[napi_derive::napi(object, js_name = "QtCanvasSnapshot")]
pub struct QtCanvasSnapshot {
    pub format: String,
    pub width_px: u32,
    pub height_px: u32,
    pub stride: u32,
    pub scale_factor: f64,
    pub bytes: napi::bindgen_prelude::Buffer,
}

#[napi_derive::napi(js_name = "captureCanvasSnapshot")]
pub fn capture_canvas_snapshot(canvas_node_id: u32) -> Option<QtCanvasSnapshot> {
    use crate::runtime::capture::WidgetCaptureFormat;

    let generation = runtime::current_app_generation().ok()?;
    let node = runtime::node_by_id(generation, canvas_node_id).ok()?;
    let capture = crate::renderer::scheduler::capture_vello_widget_exact(&node)
        .ok()?
        .or_else(|| runtime::capture_widget_exact(&node).ok())?;

    let format_str = match capture.format() {
        WidgetCaptureFormat::Argb32Premultiplied => "argb32-premultiplied",
        WidgetCaptureFormat::Rgba8Premultiplied => "rgba8-premultiplied",
    };

    Some(QtCanvasSnapshot {
        format: format_str.to_owned(),
        width_px: capture.width_px(),
        height_px: capture.height_px(),
        stride: capture.stride() as u32,
        scale_factor: capture.scale_factor(),
        bytes: capture.into_bytes().into(),
    })
}

#[napi_derive::napi(js_name = "captureFragmentIsolated")]
pub fn capture_fragment_isolated(
    canvas_node_id: u32,
    fragment_id: u32,
) -> Option<napi::bindgen_prelude::Buffer> {
    use crate::canvas::fragment::{FragmentId, fragment_store_paint_at_origin, fragment_store_world_bounds};
    use crate::canvas::vello::{Scene, peniko::kurbo::Affine};

    let generation = runtime::current_app_generation().ok()?;
    let _ = runtime::node_by_id(generation, canvas_node_id).ok()?;

    let window_id = crate::renderer::scheduler::window_ancestor_id_for_node(
        generation,
        canvas_node_id,
    )
    .ok()?
    .unwrap_or(canvas_node_id);

    let target = crate::renderer::with_renderer(|r| r.scheduler.target(window_id))?;
    let render_target = crate::renderer::scheduler::compositor_target_to_renderer(target).ok()?;

    let layout = crate::qt::qt_capture_widget_layout(canvas_node_id).ok()?;
    let scale = layout.scale_factor;

    // Get the fragment's world AABB — this determines the render target size
    let bounds = fragment_store_world_bounds(canvas_node_id, FragmentId(fragment_id))?;
    let w = bounds.width();
    let h = bounds.height();
    if w <= 0.0 || h <= 0.0 {
        return None;
    }

    let pw = (w * scale).round() as u32;
    let ph = (h * scale).round() as u32;
    if pw == 0 || ph == 0 {
        return None;
    }

    // Paint fragment at viewport origin — paint_node_at_origin skips the
    // node's own local_transform so content lands at (0,0) regardless of
    // where the fragment sits in the tree.
    let mut scene = Scene::new();
    fragment_store_paint_at_origin(canvas_node_id, FragmentId(fragment_id), &mut scene, Affine::IDENTITY);

    // Render to a target sized exactly to the fragment
    let capture = crate::renderer::offscreen::render_scene_to_capture(
        render_target,
        canvas_node_id,
        pw,
        ph,
        scale,
        &scene,
    )
    .ok()?;

    let png_buf = capture.to_png_bytes()?;
    Some(png_buf.into())
}