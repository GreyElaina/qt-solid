#pragma once

#include <cstdint>

struct MacosDisplayLinkHandle;
using MacosDisplayLinkCallback = void (*)(void *, void *);

#ifdef __cplusplus
extern "C" {
#endif

MacosDisplayLinkHandle *qt_macos_display_link_create(
    void *metal_layer, void *context, MacosDisplayLinkCallback callback,
    const void *notifier);
bool qt_macos_display_link_start(MacosDisplayLinkHandle *handle);
void qt_macos_display_link_stop(MacosDisplayLinkHandle *handle);
void qt_macos_display_link_destroy(MacosDisplayLinkHandle *handle);
void qt_macos_display_link_set_preferred_fps(MacosDisplayLinkHandle *handle,
                                              float fps);

/// Returns an opaque pointer to the NativeFrameNotifier. Valid for process lifetime.
const void *qt_solid_native_frame_notifier();

#ifdef __cplusplus
}
#endif
