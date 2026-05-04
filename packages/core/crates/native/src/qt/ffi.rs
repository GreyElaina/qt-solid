#[cxx::bridge(namespace = "qt_solid_spike::qt")]
pub(crate) mod bridge {
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

    struct QtScreenGeometry {
        x: i32,
        y: i32,
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

    #[derive(Clone, Copy, Debug)]
    struct QtMotionMouseTarget {
        found: bool,
        root_node_id: u32,
        local_x: f64,
        local_y: f64,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum QtWindowCompositorDriveStatus {
        Idle = 0,
        Presented = 1,
        Busy = 2,
        NeedsQtRepaint = 3,
    }

    struct QtShapedPathEl {
        tag: u8,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
    }

    struct QtShapedTextLine {
        y_offset: f64,
        width: f64,
        height: f64,
        ascent: f64,
        descent: f64,
    }

    struct QtRasterizedGlyph {
        x: f64,
        y: f64,
        width: u32,
        height: u32,
        bearing_x: f64,
        bearing_y: f64,
        scale_factor: f64,
        pixels: Vec<u8>,
        run_index: u32,
    }

    struct QtTextMeasurement {
        width: f64,
        height: f64,
        ascent: f64,
        descent: f64,
        line_count: i32,
    }

    struct QtShapedTextResult {
        elements: Vec<QtShapedPathEl>,
        lines: Vec<QtShapedTextLine>,
        ascent: f64,
        descent: f64,
        total_width: f64,
        total_height: f64,
        rasterized_glyphs: Vec<QtRasterizedGlyph>,
    }

    struct QtShapedTextWithCursorsResult {
        elements: Vec<QtShapedPathEl>,
        cursor_x_positions: Vec<f64>,
        ascent: f64,
        descent: f64,
        total_width: f64,
        rasterized_glyphs: Vec<QtRasterizedGlyph>,
    }

    struct QtTextStyleRun {
        start: i32,
        length: i32,
        font_size: f64,
        font_family: String,
        font_weight: i32,
        font_italic: bool,
    }

    struct QtStyledShapedRun {
        elements: Vec<QtShapedPathEl>,
    }

    struct QtStyledShapedTextResult {
        combined_elements: Vec<QtShapedPathEl>,
        runs: Vec<QtStyledShapedRun>,
        lines: Vec<QtShapedTextLine>,
        ascent: f64,
        descent: f64,
        total_width: f64,
        total_height: f64,
        rasterized_glyphs: Vec<QtRasterizedGlyph>,
    }

    struct QtClipboardEntry {
        mime: String,
        data: Vec<u8>,
    }

    struct QtScreenDpiInfo {
        dpi_x: f64,
        dpi_y: f64,
        device_pixel_ratio: f64,
        available_geometry: QtScreenGeometry,
    }

    extern "Rust" {

        fn emit_app_event(name: &str);
        fn emit_debug_event(name: &str);
        fn emit_inspect_event(node_id: u32);
        fn qt_canvas_pointer_event(node_id: u32, event_tag: u8, x: f64, y: f64);
        fn qt_canvas_context_menu_event(node_id: u32, x: f64, y: f64, screen_x: f64, screen_y: f64);
        fn qt_canvas_key_event(
            node_id: u32,
            event_tag: u8,
            qt_key: i32,
            modifiers: u32,
            text: &str,
            repeat: bool,
            native_scan_code: u32,
            native_virtual_key: u32,
        );
        fn qt_canvas_wheel_event(
            node_id: u32,
            delta_x: f64,
            delta_y: f64,
            pixel_dx: f64,
            pixel_dy: f64,
            x: f64,
            y: f64,
            modifiers: u32,
            phase: u32,
        );
        fn qt_canvas_focus_next(node_id: u32, forward: bool) -> bool;
        fn qt_text_edit_sync(
            canvas_node_id: u32,
            fragment_id: u32,
            text: &str,
            cursor: i32,
            sel_start: i32,
            sel_end: i32,
            elements: &[QtShapedPathEl],
            cursor_x_positions: &[f64],
            ascent: f64,
            descent: f64,
            width: f64,
            rasterized_glyphs: &[QtRasterizedGlyph],
        );
        fn qt_text_edit_set_caret_visible(canvas_node_id: u32, fragment_id: u32, visible: bool);
        fn qt_window_event_close_requested(node_id: u32);
        fn qt_window_event_hover_enter(node_id: u32);
        fn qt_window_event_hover_leave(node_id: u32);
        fn qt_window_event_focus_change(node_id: u32, gained: bool);
        fn qt_window_event_resize(node_id: u32, width: f64, height: f64);
        fn qt_surface_renderer_resize(node_id: u32, width_px: u32, height_px: u32);
        fn qt_surface_renderer_blit_and_present(node_id: u32);
        fn qt_surface_renderer_metal_layer_ptr(node_id: u32) -> u64;
        fn qt_window_event_state_change(node_id: u32, state: u8);
        fn qt_system_color_scheme_changed(scheme: u8);
        fn qt_screen_dpi_changed(dpi: f64);
        fn qt_file_dialog_result(request_id: u32, paths: Vec<String>);
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
        fn qt_drive_window_compositor_frame(
            node_id: u32,
            target: QtCompositorTarget,
        ) -> Result<QtWindowCompositorDriveStatus>;
        fn qt_drive_window_compositor_frame_from_display_link(
            node_id: u32,
            target: QtCompositorTarget,
            drawable_handle: u64,
        ) -> Result<QtWindowCompositorDriveStatus>;
        fn qt_window_compositor_frame_is_initialized(target: QtCompositorTarget) -> Result<bool>;
        fn qt_window_compositor_request_frame(target: QtCompositorTarget) -> Result<bool>;
        fn qt_window_compositor_display_link_should_run(target: QtCompositorTarget)
        -> Result<bool>;
        fn qt_window_compositor_metal_layer_handle(target: QtCompositorTarget) -> Result<u64>;
        fn qt_window_compositor_note_metal_display_link_drawable(
            target: QtCompositorTarget,
            drawable_handle: u64,
        ) -> Result<()>;
        fn qt_window_compositor_release_metal_drawable(drawable_handle: u64) -> Result<()>;
        fn qt_destroy_window_compositor(target: QtCompositorTarget) -> Result<()>;
        fn qt_window_motion_hit_test(
            window_id: u32,
            screen_x: i32,
            screen_y: i32,
        ) -> Result<QtMotionMouseTarget>;
        fn qt_window_motion_map_point_to_root(
            window_id: u32,
            root_node_id: u32,
            screen_x: i32,
            screen_y: i32,
        ) -> Result<QtMotionMouseTarget>;
        fn qt_window_motion_hit_root_ids(window_id: u32) -> Result<Vec<u32>>;
        fn next_trace_id() -> u64;
        fn trace_cpp_stage(trace_id: u64, stage: &str, node_id: u32, prop_id: u16, detail: &str);
    }

    unsafe extern "C++" {
        include!("qt/ffi.h");

        fn qt_host_started() -> bool;
        fn qt_host_start(uv_loop_ptr: usize) -> Result<()>;
        fn qt_host_shutdown() -> Result<()>;
        fn qt_create_widget(id: u32, kind_tag: u8) -> Result<()>;
        fn qt_insert_child(parent_id: u32, child_id: u32, anchor_id_or_zero: u32) -> Result<()>;
        fn qt_remove_child(parent_id: u32, child_id: u32) -> Result<()>;
        fn qt_destroy_widget(id: u32, subtree_ids: &[u32]) -> Result<()>;
        fn qt_request_repaint(id: u32) -> Result<()>;
        fn qt_request_window_compositor_frame(id: u32) -> Result<bool>;
        fn qt_capture_widget_layout(id: u32) -> Result<QtWidgetCaptureLayout>;
        fn qt_set_window_transient_owner(window_id: u32, owner_id: u32) -> Result<()>;
        fn qt_capture_widget_into(
            id: u32,
            width_px: u32,
            height_px: u32,
            stride: usize,
            include_children: bool,
            bytes: &mut [u8],
        ) -> Result<()>;
        fn qt_capture_widget_visible_rects(id: u32) -> Result<Vec<QtRect>>;
        fn qt_debug_node_state(id: u32) -> QtRealizedNodeState;
        fn schedule_debug_event(delay_ms: u32, name: &str) -> Result<()>;
        fn debug_click_node(id: u32) -> Result<()>;
        fn debug_close_node(id: u32) -> Result<()>;
        fn debug_input_insert_text(id: u32, value: &str) -> Result<()>;
        fn debug_highlight_node(id: u32) -> Result<()>;
        fn debug_node_bounds(id: u32) -> QtNodeBounds;
        fn get_screen_geometry(id: u32) -> QtScreenGeometry;
        fn focus_widget(id: u32) -> Result<()>;
        fn get_widget_size_hint(id: u32) -> QtScreenGeometry;
        fn debug_node_at_point(screen_x: i32, screen_y: i32) -> u32;
        fn debug_set_inspect_mode(enabled: bool) -> Result<()>;
        fn debug_clear_highlight() -> Result<()>;
        fn trace_now_ns() -> u64;
        fn qt_clipboard_get_text() -> String;
        fn qt_clipboard_set_text(text: &str);
        fn qt_clipboard_has_text() -> bool;
        fn qt_clipboard_formats() -> Vec<String>;
        fn qt_clipboard_get(mime: &str) -> Vec<u8>;
        fn qt_clipboard_clear();
        fn qt_clipboard_set(entries: Vec<QtClipboardEntry>);
        fn qt_shape_text_to_path(
            text: &str,
            font_size: f64,
            font_family: &str,
            font_weight: i32,
            font_italic: bool,
            max_width: f64,
            elide_mode: u8,
        ) -> QtShapedTextResult;
        fn qt_shape_text_with_cursors(
            text: &str,
            font_size: f64,
            font_family: &str,
            font_weight: i32,
            font_italic: bool,
        ) -> QtShapedTextWithCursorsResult;
        fn qt_shape_styled_text_to_path(
            text: &str,
            default_font_size: f64,
            default_font_family: &str,
            max_width: f64,
            elide_mode: u8,
            style_runs: &[QtTextStyleRun],
        ) -> QtStyledShapedTextResult;
        fn qt_measure_text(
            text: &str,
            font_size: f64,
            font_family: &str,
            font_weight: i32,
            font_italic: bool,
            max_width: f64,
        ) -> QtTextMeasurement;
        fn qt_system_color_scheme() -> u8;
        fn qt_screen_dpi_info(id: u32) -> QtScreenDpiInfo;
        fn qt_show_open_file_dialog(
            window_id: u32,
            title: &str,
            filter: &str,
            multiple: bool,
        ) -> u32;
        fn qt_show_save_file_dialog(
            window_id: u32,
            title: &str,
            filter: &str,
            default_name: &str,
        ) -> u32;

        fn qt_window_set_title(id: u32, value: &str) -> Result<()>;
        fn qt_window_set_width(id: u32, value: i32) -> Result<()>;
        fn qt_window_set_height(id: u32, value: i32) -> Result<()>;
        fn qt_window_set_min_width(id: u32, value: i32) -> Result<()>;
        fn qt_window_set_min_height(id: u32, value: i32) -> Result<()>;
        fn qt_window_set_visible(id: u32, value: bool) -> Result<()>;
        fn qt_window_set_enabled(id: u32, value: bool) -> Result<()>;
        fn qt_window_set_frameless(id: u32, value: bool) -> Result<()>;
        fn qt_window_set_transparent_background(id: u32, value: bool) -> Result<()>;
        fn qt_window_set_always_on_top(id: u32, value: bool) -> Result<()>;
        fn qt_window_set_window_kind(id: u32, value: u8) -> Result<()>;
        fn qt_window_set_screen_position(id: u32, x: i32, y: i32) -> Result<()>;
        fn qt_window_wire_close_requested(id: u32) -> Result<()>;
        fn qt_window_wire_hover_enter(id: u32) -> Result<()>;
        fn qt_window_wire_hover_leave(id: u32) -> Result<()>;
        fn qt_canvas_set_cursor(node_id: u32, cursor_tag: u8) -> Result<()>;
        fn qt_text_edit_activate(
            window_id: u32,
            canvas_node_id: u32,
            fragment_id: u32,
            text: &str,
            font_size: f64,
            cursor_pos: i32,
            sel_start: i32,
            sel_end: i32,
        ) -> Result<()>;
        fn qt_text_edit_deactivate(window_id: u32) -> Result<()>;
        fn qt_text_edit_click_to_cursor(window_id: u32, local_x: f64) -> Result<()>;
        fn qt_text_edit_drag_to_cursor(window_id: u32, local_x: f64) -> Result<()>;
        fn qt_window_present_cpu_frame(
            node_id: u32,
            pixels: &[u8],
            width: u32,
            height: u32,
            stride: u32,
        ) -> Result<()>;
        fn qt_macos_set_display_link_frame_rate(node_id: u32, fps: f32);
        fn qt_window_minimize(id: u32) -> Result<()>;
        fn qt_window_maximize(id: u32) -> Result<()>;
        fn qt_window_restore(id: u32) -> Result<()>;
        fn qt_window_fullscreen(id: u32, enter: bool) -> Result<()>;
        fn qt_window_is_minimized(id: u32) -> Result<bool>;
        fn qt_window_is_maximized(id: u32) -> Result<bool>;
        fn qt_window_is_fullscreen(id: u32) -> Result<bool>;
    }
}

pub(crate) use bridge::{
    QtClipboardEntry, QtCompositorSurfaceKind, QtCompositorTarget, QtRealizedNodeState,
    QtWindowCompositorDriveStatus, debug_clear_highlight, debug_click_node, debug_close_node,
    debug_highlight_node, debug_input_insert_text, debug_node_at_point, debug_node_bounds,
    debug_set_inspect_mode, focus_widget, get_screen_geometry, get_widget_size_hint,
    qt_canvas_set_cursor, qt_capture_widget_into, qt_capture_widget_layout,
    qt_capture_widget_visible_rects, qt_clipboard_clear, qt_clipboard_formats, qt_clipboard_get,
    qt_clipboard_get_text, qt_clipboard_has_text, qt_clipboard_set, qt_clipboard_set_text,
    qt_create_widget, qt_debug_node_state, qt_destroy_widget, qt_host_shutdown, qt_host_start,
    qt_host_started, qt_insert_child, qt_measure_text, qt_remove_child, qt_request_repaint,
    qt_request_window_compositor_frame, qt_screen_dpi_info, qt_set_window_transient_owner,
    qt_shape_styled_text_to_path, qt_shape_text_to_path, qt_shape_text_with_cursors,
    qt_show_open_file_dialog, qt_show_save_file_dialog, qt_system_color_scheme,
    qt_text_edit_click_to_cursor, qt_text_edit_drag_to_cursor, qt_window_fullscreen,
    qt_window_is_fullscreen, qt_window_is_maximized, qt_window_is_minimized, qt_window_maximize,
    qt_window_minimize, qt_window_restore, qt_window_set_always_on_top, qt_window_set_enabled,
    qt_window_set_frameless, qt_window_set_height, qt_window_set_min_height,
    qt_window_set_min_width, qt_window_set_screen_position, qt_window_set_title,
    qt_window_set_transparent_background, qt_window_set_visible, qt_window_set_width,
    qt_window_set_window_kind, qt_window_wire_close_requested, qt_window_wire_hover_enter,
    qt_window_wire_hover_leave, schedule_debug_event, trace_now_ns,
};

pub(crate) fn build_rasterized_glyphs(
    rasterized: &[bridge::QtRasterizedGlyph],
    dy: f64,
) -> Vec<crate::canvas::fragment::RasterizedGlyph> {
    use crate::canvas::vello::peniko as peniko_crate;

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
            Some(crate::canvas::fragment::RasterizedGlyph {
                image,
                x: rg.x + rg.bearing_x,
                y: rg.y + rg.bearing_y + dy,
                scale_factor: rg.scale_factor,
            })
        })
        .collect()
}

pub(crate) fn emit_app_event(name: &str) {
    super::runtime::emit_app_event(name);
}

pub(crate) fn emit_debug_event(name: &str) {
    super::runtime::emit_debug_event(name);
}

pub(crate) fn emit_inspect_event(node_id: u32) {
    super::runtime::emit_inspect_event(node_id);
}

pub(crate) fn qt_canvas_pointer_event(node_id: u32, event_tag: u8, x: f64, y: f64) {
    super::runtime::emit_canvas_pointer_event(node_id, event_tag, x, y);
}

pub(crate) fn qt_canvas_context_menu_event(
    node_id: u32,
    x: f64,
    y: f64,
    screen_x: f64,
    screen_y: f64,
) {
    super::runtime::emit_canvas_context_menu_event(node_id, x, y, screen_x, screen_y);
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
    super::runtime::qt_canvas_key_event(
        node_id,
        event_tag,
        qt_key,
        modifiers,
        text,
        repeat,
        native_scan_code,
        native_virtual_key,
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
    super::runtime::qt_canvas_wheel_event(
        node_id, delta_x, delta_y, pixel_dx, pixel_dy, x, y, modifiers, phase,
    );
}

pub(crate) fn qt_text_edit_sync(
    canvas_node_id: u32,
    fragment_id: u32,
    text: &str,
    cursor: i32,
    sel_start: i32,
    sel_end: i32,
    elements: &[bridge::QtShapedPathEl],
    cursor_x_positions: &[f64],
    ascent: f64,
    descent: f64,
    width: f64,
    rasterized_glyphs: &[bridge::QtRasterizedGlyph],
) {
    use crate::canvas::fragment::{
        FragmentId, ShapedTextLayout, fragment_store_set_text_input_state,
    };
    use crate::canvas::vello::peniko::kurbo::{BezPath, PathEl, Point};

    let mut path = BezPath::new();
    for el in elements {
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

    let height = ascent + descent;
    let layout = ShapedTextLayout {
        path,
        rasterized_glyphs: build_rasterized_glyphs(rasterized_glyphs, 0.0),
        cursor_x_positions: cursor_x_positions.to_vec(),
        width,
        height,
        ascent,
    };

    let sel_anchor = if sel_start >= 0 && sel_start != sel_end {
        let anchor = if cursor == sel_end {
            sel_start
        } else {
            sel_end
        };
        anchor as f64
    } else {
        -1.0
    };

    fragment_store_set_text_input_state(
        canvas_node_id,
        FragmentId(fragment_id),
        text.to_string(),
        cursor as f64,
        sel_anchor,
        layout,
    );

    // Notify JS of text input state change.
    crate::runtime::emit_js_event(crate::api::QtHostEvent::CanvasTextInputChange {
        canvas_node_id,
        fragment_id,
        text: text.to_owned(),
        cursor,
        sel_start,
        sel_end,
    });

    // Request repaint.
    if let Ok(generation) = crate::runtime::current_app_generation() {
        if let Ok(node) = crate::runtime::node_by_id(generation, canvas_node_id) {
            let _ = crate::runtime::request_repaint(&node);
        }
    }
}

pub(crate) fn qt_text_edit_set_caret_visible(canvas_node_id: u32, fragment_id: u32, visible: bool) {
    use crate::canvas::fragment::{FragmentId, fragment_store_set_caret_visible};
    fragment_store_set_caret_visible(canvas_node_id, FragmentId(fragment_id), visible);

    if let Ok(generation) = crate::runtime::current_app_generation() {
        if let Ok(node) = crate::runtime::node_by_id(generation, canvas_node_id) {
            let _ = crate::runtime::request_repaint(&node);
        }
    }
}

pub(crate) fn qt_canvas_focus_next(node_id: u32, forward: bool) -> bool {
    use crate::api::QtHostEvent;
    use crate::canvas::fragment::fragment_store_focus_next;

    let (old, new) = fragment_store_focus_next(node_id, forward);
    if new < 0 {
        // No more focusable fragments — let Qt handle Tab.
        sync_text_edit_session_for_focus(node_id);
        return false;
    }
    if old != new {
        crate::runtime::emit_js_event(QtHostEvent::CanvasFocusChange {
            canvas_node_id: node_id,
            old_fragment_id: old,
            new_fragment_id: new,
        });
    }
    sync_text_edit_session_for_focus(node_id);
    true
}

/// Activate or deactivate the C++ TextEditSession based on current focus.
///
/// When the focused fragment is not itself a TextInput (e.g. a focusable
/// `<rect>` wrapper), we search its subtree for the first TextInput child
/// and activate that instead.
pub(crate) fn sync_text_edit_session_for_focus(canvas_node_id: u32) {
    use crate::canvas::fragment::FragmentData;

    let info = crate::runtime::with_fragment_tree(canvas_node_id, |tree| {
        let focused_id = tree.focused()?;
        let node = tree.node(focused_id)?;
        if let FragmentData::TextInput(ref ti) = node.kind {
            return Some(extract_text_input_info(focused_id, ti));
        }
        // Focused node is not a TextInput — search children for one.
        let ti_id = find_first_text_input(tree, &node.children)?;
        let ti_node = tree.node(ti_id)?;
        if let FragmentData::TextInput(ref ti) = ti_node.kind {
            Some(extract_text_input_info(ti_id, ti))
        } else {
            None
        }
    })
    .flatten();

    match info {
        Some((fragment_id, text, font_size, cursor, sel_start, sel_end)) => {
            let _ = bridge::qt_text_edit_activate(
                canvas_node_id,
                canvas_node_id,
                fragment_id,
                &text,
                font_size,
                cursor,
                sel_start,
                sel_end,
            );
        }
        None => {
            let _ = bridge::qt_text_edit_deactivate(canvas_node_id);
        }
    }
}

fn extract_text_input_info(
    id: crate::canvas::fragment::FragmentId,
    ti: &crate::canvas::fragment::TextInputFragment,
) -> (u32, String, f64, i32, i32, i32) {
    let cursor = ti.cursor_pos as i32;
    let (sel_start, sel_end) = if ti.selection_anchor >= 0.0 {
        let anchor = ti.selection_anchor as i32;
        (cursor.min(anchor), cursor.max(anchor))
    } else {
        (-1, -1)
    };
    (
        id.0,
        ti.text.clone(),
        ti.font_size,
        cursor,
        sel_start,
        sel_end,
    )
}

fn find_first_text_input(
    tree: &crate::canvas::fragment::FragmentTree,
    children: &[crate::canvas::fragment::FragmentId],
) -> Option<crate::canvas::fragment::FragmentId> {
    use crate::canvas::fragment::FragmentData;
    for &child_id in children {
        let child = tree.node(child_id)?;
        if matches!(child.kind, FragmentData::TextInput(_)) {
            return Some(child_id);
        }
        if let Some(found) = find_first_text_input(tree, &child.children) {
            return Some(found);
        }
    }
    None
}

pub(crate) fn qt_window_event_close_requested(node_id: u32) {
    super::runtime::emit_window_typed_event(node_id, "onCloseRequested");
}

pub(crate) fn qt_window_event_hover_enter(node_id: u32) {
    super::runtime::emit_window_typed_event(node_id, "onHoverEnter");
}

pub(crate) fn qt_window_event_hover_leave(node_id: u32) {
    super::runtime::emit_window_typed_event(node_id, "onHoverLeave");
}

pub(crate) fn qt_window_event_focus_change(node_id: u32, gained: bool) {
    super::runtime::qt_window_event_focus_change(node_id, gained);
}

pub(crate) fn qt_window_event_resize(node_id: u32, width: f64, height: f64) {
    super::runtime::qt_window_event_resize(node_id, width, height);
}

pub(crate) fn qt_surface_renderer_resize(node_id: u32, width_px: u32, height_px: u32) {
    crate::renderer::compositor::resize_surface(node_id, width_px, height_px);
}

pub(crate) fn qt_surface_renderer_blit_and_present(node_id: u32) {
    let _ = crate::renderer::compositor::blit_and_present(node_id);
}

#[cfg(target_os = "macos")]
pub(crate) fn qt_surface_renderer_metal_layer_ptr(node_id: u32) -> u64 {
    crate::renderer::compositor::metal_layer_ptr(node_id)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn qt_surface_renderer_metal_layer_ptr(_node_id: u32) -> u64 {
    0
}

pub(crate) fn qt_window_event_state_change(node_id: u32, state: u8) {
    super::runtime::qt_window_event_state_change(node_id, state);
}

pub(crate) fn qt_system_color_scheme_changed(scheme: u8) {
    super::runtime::qt_system_color_scheme_changed(scheme);
}

pub(crate) fn qt_screen_dpi_changed(dpi: f64) {
    super::runtime::qt_screen_dpi_changed(dpi);
}

pub(crate) fn qt_file_dialog_result(request_id: u32, paths: Vec<String>) {
    super::runtime::qt_file_dialog_result(request_id, paths);
}

pub(crate) fn qt_mark_window_compositor_scene_dirty(window_id: u32, node_id: u32) {
    crate::renderer::with_renderer_mut(|r| r.scheduler.mark_scene_node(window_id, node_id));
}

pub(crate) fn qt_mark_window_compositor_geometry_dirty(window_id: u32, node_id: u32) {
    crate::renderer::with_renderer_mut(|r| r.scheduler.mark_geometry_node(window_id, node_id));
}

pub(crate) fn qt_mark_window_compositor_pixels_dirty(window_id: u32, node_id: u32) {
    crate::renderer::with_renderer_mut(|r| r.scheduler.mark_dirty_node(window_id, node_id));
}

pub(crate) fn qt_window_frame_tick(node_id: u32) -> napi::Result<()> {
    crate::renderer::scheduler::frame_clock::qt_window_frame_tick(node_id)
}

pub(crate) fn qt_window_take_next_frame_request(node_id: u32) -> napi::Result<bool> {
    crate::renderer::scheduler::frame_clock::qt_window_frame_take_next_frame_request(node_id)
}

pub(crate) fn qt_mark_window_compositor_pixels_dirty_region(
    window_id: u32,
    node_id: u32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) {
    crate::renderer::with_renderer_mut(|r| {
        r.scheduler.mark_dirty_region(
            window_id,
            crate::renderer::scheduler::state::WindowCompositorDirtyRegion {
                node_id,
                x,
                y,
                width,
                height,
            },
        )
    });
}

pub(crate) fn qt_drive_window_compositor_frame(
    node_id: u32,
    target: QtCompositorTarget,
) -> napi::Result<QtWindowCompositorDriveStatus> {
    let render_target = crate::renderer::scheduler::compositor_target_to_renderer(target)?;
    #[cfg(not(target_os = "macos"))]
    qt_compositor::compositor_actor::register_surface_node_id(render_target.surface_key(), node_id);
    qt_compositor::load_or_create_compositor(render_target)
        .and_then(|compositor| compositor.begin_drive(render_target))
        .map_err(|error| crate::runtime::qt_error(error.to_string()))?;
    crate::renderer::scheduler::frame_clock::qt_window_frame_tick(node_id)?;
    crate::renderer::scheduler::pipeline::drive_frame(node_id, target)
}

pub(crate) fn qt_drive_window_compositor_frame_from_display_link(
    node_id: u32,
    target: QtCompositorTarget,
    drawable_handle: u64,
) -> napi::Result<QtWindowCompositorDriveStatus> {
    let render_target = crate::renderer::scheduler::compositor_target_to_renderer(target)?;
    let compositor = qt_compositor::load_or_create_compositor(render_target)
        .map_err(|error| crate::runtime::qt_error(error.to_string()))?;
    compositor
        .begin_drive(render_target)
        .map_err(|error| crate::runtime::qt_error(error.to_string()))?;
    crate::renderer::scheduler::frame_clock::qt_window_frame_tick(node_id)?;
    let status = crate::renderer::scheduler::pipeline::drive_frame_with_drawable(
        node_id,
        target,
        drawable_handle,
    )?;

    if matches!(status, QtWindowCompositorDriveStatus::Busy) {
        let _ = compositor.request_frame(
            render_target,
            qt_compositor::FrameReason::OverlayInvalidated,
        );
    }
    Ok(status)
}

pub(crate) fn qt_window_compositor_frame_is_initialized(
    target: QtCompositorTarget,
) -> napi::Result<bool> {
    let render_target = crate::renderer::scheduler::compositor_target_to_renderer(target)
        .map_err(|error| crate::runtime::qt_error(error.to_string()))?;
    Ok(qt_compositor::compositor_frame_is_initialized(
        render_target,
    ))
}

pub(crate) fn qt_window_compositor_request_frame(target: QtCompositorTarget) -> napi::Result<bool> {
    let render_target = crate::renderer::scheduler::compositor_target_to_renderer(target)
        .map_err(|error| crate::runtime::qt_error(error.to_string()))?;
    qt_compositor::load_or_create_compositor(render_target)
        .and_then(|compositor| {
            compositor.request_frame(
                render_target,
                qt_compositor::FrameReason::OverlayInvalidated,
            )
        })
        .map_err(|error| crate::runtime::qt_error(error.to_string()))
}

pub(crate) fn qt_window_compositor_display_link_should_run(
    target: QtCompositorTarget,
) -> napi::Result<bool> {
    let render_target = crate::renderer::scheduler::compositor_target_to_renderer(target)
        .map_err(|error| crate::runtime::qt_error(error.to_string()))?;
    Ok(qt_compositor::load_or_create_compositor(render_target)
        .map(|compositor| compositor.should_run_frame_source())
        .map_err(|error| crate::runtime::qt_error(error.to_string()))?)
}

pub(crate) fn qt_window_compositor_metal_layer_handle(
    target: QtCompositorTarget,
) -> napi::Result<u64> {
    let render_target = crate::renderer::scheduler::compositor_target_to_renderer(target)
        .map_err(|error| crate::runtime::qt_error(error.to_string()))?;
    qt_compositor::load_or_create_compositor(render_target)
        .and_then(|compositor| compositor.layer_handle(render_target))
        .map_err(|error| crate::runtime::qt_error(error.to_string()))
}

pub(crate) fn qt_window_compositor_note_metal_display_link_drawable(
    _target: QtCompositorTarget,
    drawable_handle: u64,
) -> napi::Result<()> {
    // note_drawable is no longer used; release the drawable to avoid leaks.
    #[cfg(target_os = "macos")]
    qt_compositor::release_metal_drawable(drawable_handle);
    Ok(())
}

pub(crate) fn qt_window_compositor_release_metal_drawable(
    drawable_handle: u64,
) -> napi::Result<()> {
    #[cfg(target_os = "macos")]
    qt_compositor::release_metal_drawable(drawable_handle);
    Ok(())
}

pub(crate) fn qt_destroy_window_compositor(target: QtCompositorTarget) -> napi::Result<()> {
    let render_target = crate::renderer::scheduler::compositor_target_to_renderer(target)
        .map_err(|error| crate::runtime::qt_error(error.to_string()))?;
    #[cfg(target_os = "macos")]
    qt_compositor::destroy_compositor(render_target);
    Ok(())
}

pub(crate) fn qt_window_motion_hit_test(
    window_id: u32,
    screen_x: i32,
    screen_y: i32,
) -> napi::Result<bridge::QtMotionMouseTarget> {
    crate::renderer::scheduler::pipeline::window_motion_hit_test(window_id, screen_x, screen_y)
}

pub(crate) fn qt_window_motion_map_point_to_root(
    window_id: u32,
    root_node_id: u32,
    screen_x: i32,
    screen_y: i32,
) -> napi::Result<bridge::QtMotionMouseTarget> {
    crate::renderer::scheduler::pipeline::window_motion_map_point_to_root(
        window_id,
        root_node_id,
        screen_x,
        screen_y,
    )
}

pub(crate) fn qt_window_motion_hit_root_ids(window_id: u32) -> napi::Result<Vec<u32>> {
    crate::renderer::scheduler::pipeline::window_motion_hit_root_ids(window_id)
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
