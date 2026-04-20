#pragma once

#include <cstdint>

struct MacosDisplayLinkHandle;
using MacosDisplayLinkCallback = void (*)(void *, void *);

#ifdef __cplusplus
extern "C" {
#endif

MacosDisplayLinkHandle *qt_macos_display_link_create(
    void *metal_layer, void *context, MacosDisplayLinkCallback callback);
bool qt_macos_display_link_start(MacosDisplayLinkHandle *handle);
void qt_macos_display_link_stop(MacosDisplayLinkHandle *handle);
void qt_macos_display_link_destroy(MacosDisplayLinkHandle *handle);

#ifdef __cplusplus
}
#endif
