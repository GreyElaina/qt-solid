#pragma once

#include "rust/cxx.h"

#include <cstddef>
#include <cstdint>

namespace qt_solid_spike::qt {

struct QtRealizedNodeState;
struct QtNodeBounds;
struct QtScreenGeometry;
struct QtRect;
struct QtWidgetCaptureLayout;
struct QtWindowCompositorPartMeta;
struct QtShapedPathEl;
struct QtShapedTextLine;
struct QtShapedTextResult;
struct QtShapedTextWithCursorsResult;
struct QtClipboardEntry;
struct QtScreenDpiInfo;
struct QtTextMeasurement;

bool qt_host_started();
std::uint8_t qt_runtime_wait_bridge_kind_tag();
std::int32_t qt_runtime_wait_bridge_unix_fd();
void start_qt_host(std::uintptr_t uv_loop_ptr);
void shutdown_qt_host();
void qt_create_widget(std::uint32_t id, std::uint8_t kind_tag);
void qt_insert_child(std::uint32_t parent_id, std::uint32_t child_id,
                     std::uint32_t anchor_id_or_zero);
void qt_remove_child(std::uint32_t parent_id, std::uint32_t child_id);
void qt_destroy_widget(std::uint32_t id,
                       rust::Slice<const std::uint32_t> subtree_ids);
void qt_request_repaint(std::uint32_t id);
bool qt_request_window_compositor_frame(std::uint32_t id);
QtWidgetCaptureLayout qt_capture_widget_layout(std::uint32_t id);
void qt_capture_widget_into(std::uint32_t id, std::uint32_t width_px,
                            std::uint32_t height_px, std::size_t stride,
                            bool include_children,
                            rust::Slice<std::uint8_t> bytes);
void qt_capture_widget_region_into(std::uint32_t id, std::uint32_t width_px,
                                   std::uint32_t height_px, std::size_t stride,
                                   bool include_children, QtRect rect,
                                   rust::Slice<std::uint8_t> bytes);
rust::Vec<QtRect> qt_capture_widget_visible_rects(std::uint32_t id);
void qt_set_widget_mouse_transparent(std::uint32_t id, bool transparent);
void qt_set_window_transient_owner(std::uint32_t window_id, std::uint32_t owner_id);
bool qt_paint_window_compositor(std::uint32_t node_id, std::uint32_t width_px,
                                std::uint32_t height_px, std::size_t stride,
                                double scale_factor, std::uint8_t dirty_flags,
                                bool interactive_resize,
                                rust::Slice<std::uint8_t> bytes);
QtRealizedNodeState qt_debug_node_state(std::uint32_t id);
QtNodeBounds debug_node_bounds(std::uint32_t id);
QtScreenGeometry get_screen_geometry(std::uint32_t id);
void focus_widget(std::uint32_t id);
QtScreenGeometry get_widget_size_hint(std::uint32_t id);
std::uint32_t debug_node_at_point(std::int32_t screen_x, std::int32_t screen_y);
void debug_set_inspect_mode(bool enabled);
void schedule_debug_event(std::uint32_t delay_ms, rust::Str name);
void debug_click_node(std::uint32_t id);
void debug_close_node(std::uint32_t id);
void debug_input_insert_text(std::uint32_t id, rust::Str value);
void debug_highlight_node(std::uint32_t id);
void debug_clear_highlight();
std::uint64_t trace_now_ns();
rust::String qt_clipboard_get_text();
void qt_clipboard_set_text(rust::Str text);
bool qt_clipboard_has_text();
rust::Vec<rust::String> qt_clipboard_formats();
rust::Vec<std::uint8_t> qt_clipboard_get(rust::Str mime);
void qt_clipboard_clear();
void qt_clipboard_set(rust::Vec<QtClipboardEntry> entries);
QtShapedTextResult qt_shape_text_to_path(rust::Str text, double font_size, rust::Str font_family, std::int32_t font_weight, bool font_italic, double max_width);
QtShapedTextWithCursorsResult qt_shape_text_with_cursors(rust::Str text, double font_size, rust::Str font_family, std::int32_t font_weight, bool font_italic);
struct QtTextStyleRun;
struct QtStyledShapedRun;
struct QtStyledShapedTextResult;
QtStyledShapedTextResult qt_shape_styled_text_to_path(rust::Str text, double default_font_size, rust::Str default_font_family, double max_width, rust::Slice<const QtTextStyleRun> style_runs);
std::uint8_t qt_system_color_scheme();

// Direct typed Window FFI — bypasses generic prop dispatch.
void qt_window_set_title(std::uint32_t id, rust::Str value);
void qt_window_set_width(std::uint32_t id, std::int32_t value);
void qt_window_set_height(std::uint32_t id, std::int32_t value);
void qt_window_set_min_width(std::uint32_t id, std::int32_t value);
void qt_window_set_min_height(std::uint32_t id, std::int32_t value);
void qt_window_set_visible(std::uint32_t id, bool value);
void qt_window_set_enabled(std::uint32_t id, bool value);
void qt_window_set_frameless(std::uint32_t id, bool value);
void qt_window_set_transparent_background(std::uint32_t id, bool value);
void qt_window_set_always_on_top(std::uint32_t id, bool value);
void qt_window_set_window_kind(std::uint32_t id, std::uint8_t value);
void qt_window_set_screen_position(std::uint32_t id, std::int32_t x,
                                   std::int32_t y);
void qt_window_wire_close_requested(std::uint32_t id);
void qt_window_wire_hover_enter(std::uint32_t id);
void qt_window_wire_hover_leave(std::uint32_t id);
void qt_canvas_set_cursor(std::uint32_t node_id, std::uint8_t cursor_tag);
void qt_text_edit_activate(std::uint32_t window_id, std::uint32_t canvas_node_id,
                           std::uint32_t fragment_id, rust::Str text,
                           double font_size, std::int32_t cursor_pos,
                           std::int32_t sel_start, std::int32_t sel_end);
void qt_text_edit_deactivate(std::uint32_t window_id);
void qt_text_edit_click_to_cursor(std::uint32_t window_id, double local_x);
void qt_text_edit_drag_to_cursor(std::uint32_t window_id, double local_x);

QtTextMeasurement qt_measure_text(rust::Str text, double font_size, rust::Str font_family, std::int32_t font_weight, bool font_italic, double max_width);

QtScreenDpiInfo qt_screen_dpi_info(std::uint32_t id);

void qt_window_minimize(std::uint32_t id);
void qt_window_maximize(std::uint32_t id);
void qt_window_restore(std::uint32_t id);
void qt_window_fullscreen(std::uint32_t id, bool enter);
bool qt_window_is_minimized(std::uint32_t id);
bool qt_window_is_maximized(std::uint32_t id);
bool qt_window_is_fullscreen(std::uint32_t id);

std::uint32_t qt_show_open_file_dialog(std::uint32_t window_id, rust::Str title, rust::Str filter, bool multiple);
std::uint32_t qt_show_save_file_dialog(std::uint32_t window_id, rust::Str title, rust::Str filter, rust::Str default_name);

} // namespace qt_solid_spike::qt
