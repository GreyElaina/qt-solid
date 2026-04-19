#pragma once

namespace qt_wgpu_renderer {

void register_static_platform_plugins();
bool unified_compositor_requested();
void configure_unified_compositor_platform();
void sync_unified_compositor_active_state();
bool unified_compositor_active();

} // namespace qt_wgpu_renderer
