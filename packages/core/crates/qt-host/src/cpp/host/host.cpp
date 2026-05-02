// Amalgam translation unit for Qt host lifecycle, libuv pump, and wait bridge.
// Compiles as a separate TU from ffi.cpp in namespace qt_solid::host.

#include "qt_host/host.h"

// CXX bridge declarations for extern "Rust" functions called by host code.
#include "qt-host/src/ffi.rs.h"

#include <QtCore/QCoreApplication>
#include <QtCore/QEventLoop>
#include <QtCore/QThread>
#include <QtCore/QVersionNumber>
#include <QtGui/QGuiApplication>
#include <QtGui/QStyleHints>
#include <QtWidgets/QApplication>

#if defined(Q_OS_WIN)
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>
#endif

#if defined(Q_OS_LINUX)
#include <dlfcn.h>
#endif

#if defined(Q_OS_MACOS) || defined(__APPLE__)
#include "qt_host/macos/event_buffer.h"
#include "qt_host/macos/cocoa_dispatcher_shim.h"
#include <CoreFoundation/CoreFoundation.h>
#include <pthread.h>
#endif

#include <atomic>
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <functional>
#include <memory>
#include <optional>
#include <stdexcept>
#include <string>

#include <uv.h>

// Private Qt headers for Cocoa dispatcher shim
#if defined(Q_OS_MACOS) || defined(__APPLE__)
#include <private/qabstracteventdispatcher_p.h>
#endif

namespace qt_solid::host {

// ---------------------------------------------------------------------------
// Helper definitions (shared by all host files)
// ---------------------------------------------------------------------------

[[noreturn]] void throw_error(const char *message) {
  throw std::runtime_error(message);
}

#if defined(Q_OS_WIN)
const char *qt_solid_uv_strerror(int status);
#endif

[[noreturn]] void throw_uv_error(const char *operation, int status) {
  std::string message(operation);
  message += ": ";
#if defined(Q_OS_WIN)
  message += qt_solid_uv_strerror(status);
#else
  message += uv_strerror(status);
#endif
  throw std::runtime_error(message);
}

bool on_required_qt_host_thread() {
#if defined(__APPLE__)
  return pthread_main_np() == 1;
#else
  return true;
#endif
}

WaitBridgeKind wait_bridge_kind_from_tag(std::uint8_t tag) {
  switch (tag) {
  case static_cast<std::uint8_t>(WaitBridgeKind::None):
    return WaitBridgeKind::None;
  case static_cast<std::uint8_t>(WaitBridgeKind::UnixFd):
    return WaitBridgeKind::UnixFd;
  case static_cast<std::uint8_t>(WaitBridgeKind::WindowsHandle):
    return WaitBridgeKind::WindowsHandle;
  default:
    throw_error("received unknown window-host wait bridge kind tag");
  }
}

// Forward declarations for functions defined in state.cpp (used by uv.cpp/wait_bridge.cpp)
int current_runtime_wait_bridge_unix_fd() noexcept;
void drain_runtime_wait_bridge_notifications() noexcept;
#if !defined(__APPLE__)
std::optional<std::uint64_t> current_runtime_wait_bridge_timer_delay_ms()
    noexcept;
#endif

// Forward declarations for functions defined in wait_bridge.cpp (used by uv.cpp)
WaitBridgeKind qt_runtime_wait_bridge_kind_impl();
int qt_runtime_wait_bridge_unix_fd_impl();

// ---------------------------------------------------------------------------
// Include host source files in dependency order
// ---------------------------------------------------------------------------

#include "wait_bridge.cpp"
#include "uv.cpp"
#include "state.cpp"

// ---------------------------------------------------------------------------
// Impl functions (declared in host.h, used by ffi.cpp)
// ---------------------------------------------------------------------------

void qt_host_start_impl(uv_loop_t *loop, QtHostCallbacks callbacks) {
  if (!on_required_qt_host_thread()) {
    throw_error("QtApp.start must run on macOS main thread");
  }
  if (!g_host) {
    g_host = new QtHostState(loop, callbacks);
  }
  try {
    g_host->start();
  } catch (...) {
    delete g_host;
    g_host = nullptr;
    throw;
  }
}

void qt_host_shutdown_impl() {
  if (!on_required_qt_host_thread()) {
    throw_error("QtApp.shutdown must run on macOS main thread");
  }
  if (!g_host) {
    return;
  }
  // g_host is intentionally never deleted. QtHostState does not support
  // restart (start() throws if app_ already exists), and QApplication
  // cleanup relies on process exit. The deferred teardown via
  // pump_->request_shutdown fires on the next uv loop turn, but only
  // performs C++-side cleanup (registry clear, QApplication drop) — no
  // Rust WindowHost calls happen during that phase, so the Rust side
  // can safely drop its WindowHost immediately after this returns.
  g_host->shutdown();
}

bool qt_host_started_impl() {
  return g_host != nullptr && g_host->started();
}

} // namespace qt_solid::host
