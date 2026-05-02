#import <Cocoa/Cocoa.h>
#include <cstdint>
#include <unordered_map>

// ---------------------------------------------------------------------------
// Cocoa local event monitor for popup outside-click dismiss.
//
// macOS does not deliver mouse events to Qt::Popup widgets when the user
// clicks on another NSWindow in the same process. We install an
// NSEvent local monitor to detect this and fire the dismiss callback.
// ---------------------------------------------------------------------------

namespace {

struct PopupMonitor {
    id monitor = nil;
    NSWindow *popup_window = nullptr;
};

static std::unordered_map<std::uint32_t, PopupMonitor> g_monitors;

} // namespace

extern "C" {

void qt_popup_install_outside_click_monitor(
    std::uint32_t node_id,
    void *ns_view_ptr,
    void (*callback)(std::uint32_t)
) {
    // Remove existing monitor for this node if any.
    auto it = g_monitors.find(node_id);
    if (it != g_monitors.end()) {
        [NSEvent removeMonitor:it->second.monitor];
        g_monitors.erase(it);
    }

    auto *ns_view = (__bridge NSView *)ns_view_ptr;
    auto *popup_window = [ns_view window];
    if (popup_window == nil) {
        return;
    }

    NSEventMask mask = NSEventMaskLeftMouseDown | NSEventMaskRightMouseDown | NSEventMaskOtherMouseDown;

    id monitor = [NSEvent addLocalMonitorForEventsMatchingMask:mask handler:^NSEvent *(NSEvent *event) {
        auto mit = g_monitors.find(node_id);
        if (mit == g_monitors.end()) {
            return event;
        }

        NSWindow *pw = mit->second.popup_window;
        if (pw == nil || ![pw isVisible]) {
            return event;
        }

        // Click inside popup — let it through.
        if ([event window] == pw) {
            return event;
        }

        // Outside click — fire dismiss.
        callback(node_id);
        return event;
    }];

    g_monitors[node_id] = PopupMonitor{monitor, popup_window};
}

void qt_popup_remove_outside_click_monitor(std::uint32_t node_id) {
    auto it = g_monitors.find(node_id);
    if (it != g_monitors.end()) {
        [NSEvent removeMonitor:it->second.monitor];
        g_monitors.erase(it);
    }
}

} // extern "C"
