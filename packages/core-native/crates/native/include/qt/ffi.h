#pragma once

#include "rust/cxx.h"

#include <cstddef>
#include <cstdint>

class QPainter;

namespace qt_solid_spike::qt {

struct QtRealizedNodeState;
struct QtNodeBounds;
struct QtMethodValue;
struct QtRect;
struct QtWidgetCaptureLayout;
struct QtWindowCompositorPartMeta;

bool qt_host_started();
std::uint8_t qt_runtime_wait_bridge_kind_tag();
std::int32_t qt_runtime_wait_bridge_unix_fd();
std::uint64_t qt_runtime_wait_bridge_windows_handle();
void start_qt_host(std::uintptr_t uv_loop_ptr);
void shutdown_qt_host();
void qt_create_widget(std::uint32_t id, std::uint8_t kind_tag);
void qt_insert_child(std::uint32_t parent_id, std::uint32_t child_id,
                     std::uint32_t anchor_id_or_zero);
void qt_remove_child(std::uint32_t parent_id, std::uint32_t child_id);
void qt_destroy_widget(std::uint32_t id);
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
bool qt_paint_window_compositor(std::uint32_t node_id, std::uint32_t width_px,
                                std::uint32_t height_px, std::size_t stride,
                                double scale_factor, std::uint8_t dirty_flags,
                                bool interactive_resize,
                                rust::Slice<std::uint8_t> bytes);
void qt_apply_string_prop(std::uint32_t id, std::uint16_t prop_id,
                          std::uint64_t trace_id, rust::Str value);
void qt_apply_i32_prop(std::uint32_t id, std::uint16_t prop_id,
                       std::uint64_t trace_id, std::int32_t value);
void qt_apply_f64_prop(std::uint32_t id, std::uint16_t prop_id,
                       std::uint64_t trace_id, double value);
void qt_apply_bool_prop(std::uint32_t id, std::uint16_t prop_id,
                        std::uint64_t trace_id, bool value);
QtMethodValue qt_call_host_slot(std::uint32_t id, std::uint16_t slot,
                                const rust::Vec<QtMethodValue> &args);
QtMethodValue qt_qpainter_call(::QPainter &painter, std::uint16_t slot,
                               const rust::Vec<QtMethodValue> &args);
rust::String qt_read_string_prop(std::uint32_t id, std::uint16_t prop_id);
std::int32_t qt_read_i32_prop(std::uint32_t id, std::uint16_t prop_id);
double qt_read_f64_prop(std::uint32_t id, std::uint16_t prop_id);
bool qt_read_bool_prop(std::uint32_t id, std::uint16_t prop_id);
QtRealizedNodeState qt_debug_node_state(std::uint32_t id);
QtNodeBounds debug_node_bounds(std::uint32_t id);
std::uint32_t debug_node_at_point(std::int32_t screen_x, std::int32_t screen_y);
void debug_set_inspect_mode(bool enabled);
void schedule_debug_event(std::uint32_t delay_ms, rust::Str name);
void debug_click_node(std::uint32_t id);
void debug_close_node(std::uint32_t id);
void debug_input_insert_text(std::uint32_t id, rust::Str value);
void debug_highlight_node(std::uint32_t id);
void debug_clear_highlight();
std::uint64_t trace_now_ns();

} // namespace qt_solid_spike::qt
