use napi::Result;

use crate::{
    api::{
        AlignItems, FlexDirection, JustifyContent, QtDebugNodeBounds, QtDebugNodeSnapshot,
        QtDebugSnapshot, QtHostEvent, QtWindowCaptureFrame, QtWindowFrameState,
        QtWindowHostCapabilities, QtWindowHostInfo, WindowPropUpdate,
    },
    qt::{self, QtRealizedNodeState},
    renderer::scheduler,
};
#[rustfmt::skip]
use qt_host::HostCapabilities as RawWindowHostCapabilities;

use super::{
    capture::WidgetCapture, emit_js_event, ensure_live_node, invalid_arg, qt_error, NodeHandle,
    RUNTIME_STATE,
};

// ---------------------------------------------------------------------------
// Host info
// ---------------------------------------------------------------------------

fn api_window_host_capabilities(
    capabilities: RawWindowHostCapabilities,
) -> QtWindowHostCapabilities {
    QtWindowHostCapabilities {
        backend_kind: capabilities.backend_kind.to_string(),
        supports_zero_timeout_pump: capabilities.supports_zero_timeout_pump,
        supports_external_wake: capabilities.supports_external_wake,
        supports_fd_bridge: capabilities.supports_fd_bridge,
    }
}

fn current_window_host_backend_name() -> String {
    qt_host::backend_name().unwrap_or_else(qt_host::detected_backend_name)
}

fn current_window_host_capabilities() -> QtWindowHostCapabilities {
    api_window_host_capabilities(
        qt_host::capabilities().unwrap_or_else(qt_host::detected_capabilities),
    )
}

pub(crate) fn window_host_info() -> QtWindowHostInfo {
    QtWindowHostInfo {
        enabled: true,
        backend_name: current_window_host_backend_name(),
        capabilities: current_window_host_capabilities(),
    }
}

// ---------------------------------------------------------------------------
// Debug snapshot
// ---------------------------------------------------------------------------

fn flex_direction_from_tag(tag: u8) -> Option<FlexDirection> {
    match tag {
        1 => Some(FlexDirection::Column),
        2 => Some(FlexDirection::Row),
        _ => None,
    }
}

fn align_items_from_tag(tag: u8) -> Option<AlignItems> {
    match tag {
        1 => Some(AlignItems::FlexStart),
        2 => Some(AlignItems::Center),
        3 => Some(AlignItems::FlexEnd),
        4 => Some(AlignItems::Stretch),
        _ => None,
    }
}

fn justify_content_from_tag(tag: u8) -> Option<JustifyContent> {
    match tag {
        1 => Some(JustifyContent::FlexStart),
        2 => Some(JustifyContent::Center),
        3 => Some(JustifyContent::FlexEnd),
        _ => None,
    }
}

fn snapshot_from_realized_state(
    id: u32,
    parent_id: Option<u32>,
    children: Vec<u32>,
    realized: QtRealizedNodeState,
) -> QtDebugNodeSnapshot {
    QtDebugNodeSnapshot {
        id,
        kind: "window".to_owned(),
        parent_id,
        children,
        text: realized.has_text.then_some(realized.text),
        title: realized.has_title.then_some(realized.title),
        width: realized.has_width.then_some(realized.width),
        height: realized.has_height.then_some(realized.height),
        min_width: realized.has_min_width.then_some(realized.min_width),
        min_height: realized.has_min_height.then_some(realized.min_height),
        flex_grow: realized.has_flex_grow.then_some(realized.flex_grow),
        flex_shrink: realized.has_flex_shrink.then_some(realized.flex_shrink),
        enabled: realized.has_enabled.then_some(realized.enabled),
        placeholder: realized.has_placeholder.then_some(realized.placeholder),
        checked: realized.has_checked.then_some(realized.checked),
        flex_direction: flex_direction_from_tag(realized.flex_direction_tag),
        justify_content: justify_content_from_tag(realized.justify_content_tag),
        align_items: align_items_from_tag(realized.align_items_tag),
        gap: realized.has_gap.then_some(realized.gap),
        padding: realized.has_padding.then_some(realized.padding),
        value: realized.has_value.then_some(realized.value),
    }
}

pub(crate) fn debug_snapshot(generation: u64) -> Result<QtDebugSnapshot> {
    super::ensure_app_generation(generation)?;

    let nodes_to_snapshot = {
        let state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
        state.ensure_generation(generation)?;
        let mut nodes = Vec::new();
        for id in state.tree.all_handles() {
            let kind = state
                .tree
                .kind(id)
                .ok_or_else(|| invalid_arg(format!("node {id} not found")))?;
            let parent_id = state.tree.get_parent(id);
            let children = state.tree.children(id).unwrap_or(&[]).to_vec();
            nodes.push((id, kind, parent_id, children));
        }
        nodes
    };

    let mut nodes = Vec::new();
    for (id, kind, parent_id, children) in nodes_to_snapshot {
        if kind.is_root() {
            nodes.push(QtDebugNodeSnapshot {
                id,
                kind: kind.label().to_owned(),
                parent_id,
                children,
                text: None,
                title: None,
                width: None,
                height: None,
                min_width: None,
                min_height: None,
                flex_grow: None,
                flex_shrink: None,
                enabled: None,
                placeholder: None,
                checked: None,
                flex_direction: None,
                justify_content: None,
                align_items: None,
                gap: None,
                padding: None,
                value: None,
            });
            continue;
        }

        let realized = qt::qt_debug_node_state(id);
        let snapshot = snapshot_from_realized_state(id, parent_id, children, realized);
        nodes.push(snapshot);
    }

    let window_host_backend = Some(current_window_host_backend_name());
    let window_host_capabilities = Some(current_window_host_capabilities());

    Ok(QtDebugSnapshot {
        host_runtime: "nodejs".to_owned(),
        window_host_backend,
        window_host_capabilities,
        root_id: super::ROOT_NODE_ID,
        nodes,
    })
}

// ---------------------------------------------------------------------------
// Outbound FFI wrappers (Rust → Qt)
// ---------------------------------------------------------------------------

pub(crate) fn request_repaint(node: &impl NodeHandle) -> Result<()> {
    ensure_live_node(node)?;
    if let Some(window_id) =
        scheduler::window_ancestor_id_for_node(node.inner().generation, node.inner().id)?
    {
        crate::renderer::with_renderer_mut(|r| {
            r.scheduler.mark_dirty_node(window_id, node.inner().id)
        });
    }
    let _ = qt::qt_request_window_compositor_frame(node.inner().id);
    qt::qt_request_repaint(node.inner().id).map_err(|error| qt_error(error.what().to_owned()))
}

pub(crate) fn request_window_repaint_exact(window: &impl NodeHandle) -> Result<()> {
    ensure_live_node(window)?;
    qt::qt_request_repaint(window.inner().id).map_err(|error| qt_error(error.what().to_owned()))
}

pub(crate) fn request_overlay_next_frame_exact(
    window: &impl NodeHandle,
    overlay_node_id: u32,
) -> Result<()> {
    ensure_live_node(window)?;
    crate::renderer::with_renderer_mut(|r| {
        r.scheduler
            .mark_frame_tick_node(window.inner().id, overlay_node_id)
    });
    if qt::qt_request_window_compositor_frame(window.inner().id)
        .map_err(|error| qt_error(error.what().to_owned()))?
    {
        Ok(())
    } else {
        request_window_repaint_exact(window)
    }
}

pub(crate) fn capture_widget_exact(node: &impl NodeHandle) -> Result<WidgetCapture> {
    ensure_live_node(node)?;
    if node.inner().is_window() {
        return scheduler::capture_window_widget_exact(node);
    }

    scheduler::capture_painted_widget_exact_with_children(node, true)
}

pub(crate) fn wire_event(node: &impl NodeHandle, export_id: u16) -> Result<()> {
    ensure_live_node(node)?;

    let export_name = match export_id {
        1 => "onCloseRequested",
        2 => "onHoverEnter",
        3 => "onHoverLeave",
        _ => return Err(invalid_arg(format!("unknown event export id {export_id}"))),
    };

    if node.inner().is_window() {
        let id = node.inner().id;
        return match export_name {
            "onCloseRequested" => {
                qt::qt_window_wire_close_requested(id).map_err(|e| qt_error(e.what().to_owned()))
            }
            "onHoverEnter" => {
                qt::qt_window_wire_hover_enter(id).map_err(|e| qt_error(e.what().to_owned()))
            }
            "onHoverLeave" => {
                qt::qt_window_wire_hover_leave(id).map_err(|e| qt_error(e.what().to_owned()))
            }
            _ => Err(invalid_arg(format!(
                "unknown window event export {export_name}"
            ))),
        };
    }

    Err(invalid_arg(format!(
        "event export {export_name} not supported on widget kind {}",
        node.inner().kind.label()
    )))
}

pub(crate) fn apply_prop(node: &impl NodeHandle, update: WindowPropUpdate) -> Result<()> {
    ensure_live_node(node)?;
    let id = node.inner().id;
    match update {
        WindowPropUpdate::Title { value } => {
            qt::qt_window_set_title(id, &value).map_err(|e| qt_error(e.what().to_owned()))?;
        }
        WindowPropUpdate::Width { value } => {
            qt::qt_window_set_width(id, value).map_err(|e| qt_error(e.what().to_owned()))?;
        }
        WindowPropUpdate::Height { value } => {
            qt::qt_window_set_height(id, value).map_err(|e| qt_error(e.what().to_owned()))?;
        }
        WindowPropUpdate::MinWidth { value } => {
            qt::qt_window_set_min_width(id, value).map_err(|e| qt_error(e.what().to_owned()))?;
        }
        WindowPropUpdate::MinHeight { value } => {
            qt::qt_window_set_min_height(id, value).map_err(|e| qt_error(e.what().to_owned()))?;
        }
        WindowPropUpdate::Visible { value } => {
            qt::qt_window_set_visible(id, value).map_err(|e| qt_error(e.what().to_owned()))?;
        }
        WindowPropUpdate::Enabled { value } => {
            qt::qt_window_set_enabled(id, value).map_err(|e| qt_error(e.what().to_owned()))?;
        }
        WindowPropUpdate::Frameless { value } => {
            qt::qt_window_set_frameless(id, value).map_err(|e| qt_error(e.what().to_owned()))?;
        }
        WindowPropUpdate::TransparentBackground { value } => {
            qt::qt_window_set_transparent_background(id, value)
                .map_err(|e| qt_error(e.what().to_owned()))?;
        }
        WindowPropUpdate::AlwaysOnTop { value } => {
            qt::qt_window_set_always_on_top(id, value)
                .map_err(|e| qt_error(e.what().to_owned()))?;
        }
        WindowPropUpdate::Gpu { value } => {
            crate::renderer::with_renderer_mut(|r| r.set_gpu_mode(id, value));
        }
        WindowPropUpdate::WindowKind { value } => {
            let tag = u8::try_from(value).map_err(|_| invalid_arg("windowKind out of range"))?;
            qt::qt_window_set_window_kind(id, tag).map_err(|e| qt_error(e.what().to_owned()))?;
        }
        WindowPropUpdate::ScreenX { value } => {
            let y = read_window_screen_y(node)?;
            qt::qt_window_set_screen_position(id, value, y)
                .map_err(|e| qt_error(e.what().to_owned()))?;
        }
        WindowPropUpdate::ScreenY { value } => {
            let x = read_window_screen_x(node)?;
            qt::qt_window_set_screen_position(id, x, value)
                .map_err(|e| qt_error(e.what().to_owned()))?;
        }
        WindowPropUpdate::Text { .. } => {
            return Ok(());
        }
    }
    Ok(())
}

fn read_window_screen_x(node: &impl NodeHandle) -> Result<i32> {
    let bounds = qt::debug_node_bounds(node.inner().id);
    Ok(bounds.screen_x)
}

fn read_window_screen_y(node: &impl NodeHandle) -> Result<i32> {
    let bounds = qt::debug_node_bounds(node.inner().id);
    Ok(bounds.screen_y)
}

pub(crate) fn schedule_debug_event(delay_ms: u32, event: String) -> Result<()> {
    if !qt::qt_host_started() {
        return Err(invalid_arg(
            "call QtApp.start before scheduling a debug event",
        ));
    }

    qt::schedule_debug_event(delay_ms, event.as_str())
        .map_err(|error| qt_error(error.what().to_owned()))
}

pub(crate) fn debug_click_node(node_id: u32) -> Result<()> {
    if !qt::qt_host_started() {
        return Err(invalid_arg(
            "call QtApp.start before triggering debug clicks",
        ));
    }

    qt::debug_click_node(node_id).map_err(|error| qt_error(error.what().to_owned()))
}

pub(crate) fn debug_close_node(node_id: u32) -> Result<()> {
    if !qt::qt_host_started() {
        return Err(invalid_arg(
            "call QtApp.start before triggering debug close requests",
        ));
    }

    qt::debug_close_node(node_id).map_err(|error| qt_error(error.what().to_owned()))
}

pub(crate) fn debug_emit_app_event(name: String) -> Result<()> {
    if !qt::qt_host_started() {
        return Err(invalid_arg(
            "call QtApp.start before triggering debug app events",
        ));
    }

    emit_app_event(name.as_str());
    Ok(())
}

pub(crate) fn request_next_frame_exact(node: &impl NodeHandle) -> Result<()> {
    ensure_live_node(node)?;
    let window_id = node.inner().id;
    crate::renderer::with_renderer_mut(|r| r.scheduler.set_next_frame_requested(window_id, true));
    if qt::qt_request_window_compositor_frame(window_id)
        .map_err(|error| qt_error(error.what().to_owned()))?
    {
        Ok(())
    } else {
        request_window_repaint_exact(node)
    }
}

pub(crate) fn read_window_frame_state_exact(node: &impl NodeHandle) -> Result<QtWindowFrameState> {
    let window_id = node.inner().id;
    let clock = crate::renderer::with_renderer(|r| r.scheduler.frame_clock(window_id));
    Ok(QtWindowFrameState {
        seq: clock.seq,
        elapsed_ms: clock.elapsed_ms,
        delta_ms: clock.delta_ms,
    })
}

pub(crate) fn debug_input_insert_text(node_id: u32, value: String) -> Result<()> {
    if !qt::qt_host_started() {
        return Err(invalid_arg(
            "call QtApp.start before triggering debug text input",
        ));
    }

    qt::debug_input_insert_text(node_id, value.as_str())
        .map_err(|error| qt_error(error.what().to_owned()))
}

pub(crate) fn debug_highlight_node(node_id: u32) -> Result<()> {
    if !qt::qt_host_started() {
        return Err(invalid_arg(
            "call QtApp.start before triggering debug highlight",
        ));
    }

    qt::debug_highlight_node(node_id).map_err(|error| qt_error(error.what().to_owned()))
}

pub(crate) fn debug_node_bounds(node_id: u32) -> Result<QtDebugNodeBounds> {
    if !qt::qt_host_started() {
        return Err(invalid_arg(
            "call QtApp.start before reading debug node bounds",
        ));
    }

    let bounds = qt::debug_node_bounds(node_id);
    Ok(QtDebugNodeBounds {
        visible: bounds.visible,
        screen_x: bounds.screen_x,
        screen_y: bounds.screen_y,
        width: bounds.width,
        height: bounds.height,
    })
}

pub(crate) fn debug_node_at_point(screen_x: i32, screen_y: i32) -> Result<Option<u32>> {
    if !qt::qt_host_started() {
        return Err(invalid_arg(
            "call QtApp.start before reading debug node at point",
        ));
    }

    let node_id = qt::debug_node_at_point(screen_x, screen_y);
    Ok((node_id != 0).then_some(node_id))
}

pub(crate) fn debug_capture_window_frame(window_id: u32) -> Result<QtWindowCaptureFrame> {
    scheduler::capture_window_frame_exact(window_id, scheduler::WindowCaptureGrouping::Segmented)?
        .into_api_frame()
}

pub(crate) fn debug_set_inspect_mode(enabled: bool) -> Result<()> {
    if !qt::qt_host_started() {
        return Err(invalid_arg(
            "call QtApp.start before toggling debug inspect mode",
        ));
    }

    qt::debug_set_inspect_mode(enabled).map_err(|error| qt_error(error.what().to_owned()))
}

pub(crate) fn debug_clear_highlight() -> Result<()> {
    if !qt::qt_host_started() {
        return Err(invalid_arg(
            "call QtApp.start before clearing debug highlight",
        ));
    }

    qt::debug_clear_highlight().map_err(|error| qt_error(error.what().to_owned()))
}

// ---------------------------------------------------------------------------
// Inbound event emitters (C++ → Rust → JS)
// ---------------------------------------------------------------------------

pub(crate) fn emit_app_event(name: &str) {
    emit_js_event(QtHostEvent::App {
        name: name.to_owned(),
    });
}

pub(crate) fn emit_debug_event(name: &str) {
    emit_js_event(QtHostEvent::Debug {
        name: name.to_owned(),
    });
}

pub(crate) fn emit_inspect_event(node_id: u32) {
    emit_js_event(QtHostEvent::Inspect { node_id });
}

pub(crate) fn emit_canvas_pointer_event(canvas_node_id: u32, event_tag: u8, x: f64, y: f64) {
    use crate::canvas::fragment::{
        fragment_store_focus_fragment, fragment_store_get_cursor, fragment_store_hit_test,
    };

    let fragment_id_opt = fragment_store_hit_test(canvas_node_id, x, y);
    let fragment_id = fragment_id_opt.map(|id| id.0 as i32).unwrap_or(-1);

    // Update cursor on mouse move.
    if event_tag == 3 {
        let cursor_tag = fragment_id_opt
            .map(|id| fragment_store_get_cursor(canvas_node_id, id))
            .unwrap_or(0);
        let _ = crate::qt::ffi::qt_canvas_set_cursor(canvas_node_id, cursor_tag);
    }

    // Auto-focus on press.
    if event_tag == 1 {
        if let Some(fid) = fragment_id_opt {
            let (old, new) = fragment_store_focus_fragment(canvas_node_id, fid);
            if old != new {
                emit_js_event(QtHostEvent::CanvasFocusChange {
                    canvas_node_id,
                    old_fragment_id: old,
                    new_fragment_id: new,
                });
            }
            // Click-to-cursor: place caret in TextInput at click x position.
            crate::canvas::fragment::fragment_store_click_to_cursor(canvas_node_id, fid, x, y);
        }
        crate::qt::ffi::sync_text_edit_session_for_focus(canvas_node_id);
    }

    // Drag-select: extend selection while left button held (tag 4 from C++).
    if event_tag == 4 {
        crate::canvas::fragment::fragment_store_drag_to_cursor(canvas_node_id, x, y);
    }

    emit_js_event(QtHostEvent::CanvasPointer {
        canvas_node_id,
        fragment_id,
        event_tag,
        x,
        y,
    });
}

pub(crate) fn emit_canvas_context_menu_event(
    canvas_node_id: u32,
    x: f64,
    y: f64,
    screen_x: f64,
    screen_y: f64,
) {
    use crate::canvas::fragment::fragment_store_hit_test;

    let fragment_id = fragment_store_hit_test(canvas_node_id, x, y)
        .map(|id| id.0 as i32)
        .unwrap_or(-1);

    emit_js_event(QtHostEvent::CanvasContextMenu {
        canvas_node_id,
        fragment_id,
        x,
        y,
        screen_x,
        screen_y,
    });
}

pub(crate) fn qt_canvas_key_event(
    canvas_node_id: u32,
    event_tag: u8,
    qt_key: i32,
    modifiers: u32,
    text: &str,
    repeat: bool,
    native_scan_code: u32,
    native_virtual_key: u32,
) {
    let fragment_id = crate::canvas::fragment::fragment_store_focused(canvas_node_id);

    emit_js_event(QtHostEvent::CanvasKeyboard {
        canvas_node_id,
        fragment_id,
        event_tag,
        qt_key,
        modifiers,
        text: text.to_owned(),
        repeat,
        native_scan_code,
        native_virtual_key,
    });
}

pub(crate) fn qt_canvas_wheel_event(
    canvas_node_id: u32,
    delta_x: f64,
    delta_y: f64,
    pixel_dx: f64,
    pixel_dy: f64,
    x: f64,
    y: f64,
    modifiers: u32,
    phase: u32,
) {
    use crate::canvas::fragment::fragment_store_hit_test;

    let fragment_id = fragment_store_hit_test(canvas_node_id, x, y)
        .map(|id| id.0 as i32)
        .unwrap_or(-1);

    emit_js_event(QtHostEvent::CanvasWheel {
        canvas_node_id,
        fragment_id,
        delta_x,
        delta_y,
        pixel_dx,
        pixel_dy,
        x,
        y,
        modifiers,
        phase,
    });
}

pub(crate) fn emit_window_typed_event(node_id: u32, export_name: &str) {
    let export_id = match export_name {
        "onCloseRequested" => 1u16,
        "onHoverEnter" => 2,
        "onHoverLeave" => 3,
        _ => return,
    };
    emit_js_event(QtHostEvent::Listener {
        node_id,
        listener_id: export_id,
        trace_id: None,
    });
}

pub(crate) fn qt_window_event_focus_change(node_id: u32, gained: bool) {
    crate::accessibility::update_window_accessibility(node_id, gained);
    emit_js_event(QtHostEvent::WindowFocusChange { node_id, gained });
}

pub(crate) fn qt_window_event_resize(node_id: u32, width: f64, height: f64) {
    crate::canvas::fragment::fragment_store_request_full_repaint(node_id);
    crate::renderer::with_renderer_mut(|r| r.scheduler.mark_geometry_node(node_id, node_id));
    emit_js_event(QtHostEvent::WindowResize {
        node_id,
        width,
        height,
    });
}

pub(crate) fn qt_window_event_state_change(node_id: u32, state: u8) {
    emit_js_event(QtHostEvent::WindowStateChange { node_id, state });
}

pub(crate) fn qt_system_color_scheme_changed(scheme: u8) {
    let scheme_str = match scheme {
        1 => "light",
        2 => "dark",
        _ => "unknown",
    };
    emit_js_event(QtHostEvent::ColorSchemeChange {
        scheme: scheme_str.to_owned(),
    });
}

pub(crate) fn qt_screen_dpi_changed(dpi: f64) {
    emit_js_event(QtHostEvent::ScreenDpiChange { dpi });
}

pub(crate) fn qt_file_dialog_result(request_id: u32, paths: Vec<String>) {
    emit_js_event(QtHostEvent::FileDialogResult { request_id, paths });
}
