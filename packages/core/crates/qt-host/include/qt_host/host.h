#pragma once

#include <cstdint>
#include <optional>

class QApplication;
struct uv_loop_s;
typedef struct uv_loop_s uv_loop_t;

namespace qt_solid::host {

enum class WaitBridgeKind : std::uint8_t {
  None = 0,
  UnixFd = 1,
  WindowsHandle = 2,
};

enum class PumpDriverMode : std::uint8_t {
  PollingFallback = 0,
  ExternalWake = 1,
  WaitBridge = 2,
};

struct QtHostCallbacks {
  void (*on_pre_start)() = nullptr;
  void (*on_started)(QApplication *) = nullptr;
  void (*on_shutdown)() = nullptr;
  void (*on_app_activate)() = nullptr;
};

// Lifecycle
void qt_host_start_impl(uv_loop_t *loop, QtHostCallbacks callbacks);
void qt_host_shutdown_impl();
bool qt_host_started_impl();

// Pump
void request_qt_pump();

// Wait bridge
std::int32_t qt_runtime_wait_bridge_unix_fd_impl();

// Helpers
[[noreturn]] void throw_error(const char *message);
[[noreturn]] void throw_uv_error(const char *operation, int status);
bool on_required_qt_host_thread();
WaitBridgeKind wait_bridge_kind_from_tag(std::uint8_t tag);

} // namespace qt_solid::host
