#pragma once

#include <cstdint>

class QWindow;

namespace qt_wgpu_renderer {

enum class UnifiedCompositorDriveStatus {
  Idle,
  Presented,
  Busy,
  NeedsQtRepaint,
};

void register_static_platform_plugins();
bool unified_compositor_requested();
void configure_unified_compositor_platform();
void sync_unified_compositor_active_state();
bool unified_compositor_active();
bool unified_compositor_window_frame_ready(QWindow *window,
                                           double source_device_pixel_ratio);
void *unified_compositor_window_metal_layer(QWindow *window,
                                            double source_device_pixel_ratio);
bool unified_compositor_window_request_frame(QWindow *window,
                                             double source_device_pixel_ratio);
bool unified_compositor_window_display_link_should_run(
    QWindow *window, double source_device_pixel_ratio);
bool unified_compositor_window_note_metal_display_link_drawable(
    QWindow *window, double source_device_pixel_ratio,
    std::uint64_t drawable_handle);
void release_unified_compositor_metal_drawable(std::uint64_t drawable_handle);
UnifiedCompositorDriveStatus
drive_unified_compositor_window_frame_from_display_link(
    QWindow *window, std::uint32_t node_id, double source_device_pixel_ratio,
    std::uint64_t drawable_handle);
UnifiedCompositorDriveStatus drive_unified_compositor_window_frame(
    QWindow *window, std::uint32_t node_id, double source_device_pixel_ratio);

} // namespace qt_wgpu_renderer
