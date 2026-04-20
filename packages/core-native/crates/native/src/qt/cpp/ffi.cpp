#include "qt/ffi.h"
#if defined(__APPLE__)
#include "qt/macos_event_buffer_bridge.h"
#include "qt/macos_display_link_bridge.h"
#include "qt_cocoa_dispatcher_private_shim.h"
#endif
#include "qt_wgpu_platform.h"
#include "native/src/qt/ffi.rs.h"
#include "rust_widget_binding_host.h"
#include "qt_widget_host_includes.inc"
#include "qt_widget_overrides.inc"

#include <QtCore/QCoreApplication>
#include <QtCore/QDebug>
#include <QtCore/QEvent>
#include <QtCore/QEventLoop>
#include <QtCore/QMetaMethod>
#include <QtCore/QMetaProperty>
#include <QtCore/QMetaType>
#include <QtCore/QObject>
#include <QtCore/QPoint>
#include <QtCore/QPointer>
#include <QtCore/QSignalBlocker>
#include <QtCore/QTimer>
#include <QtCore/QThread>
#include <QtCore/QVariant>
#include <QtCore/QVersionNumber>
#include <QtGui/QCloseEvent>
#include <QtGui/QCursor>
#include <QtGui/QExposeEvent>
#include <QtGui/QFont>
#include <QtGui/QGuiApplication>
#include <QtGui/QBackingStore>
#include <QtGui/QImage>
#include <QtGui/QKeyEvent>
#include <QtGui/QMouseEvent>
#include <QtGui/QPaintEvent>
#include <QtGui/QPainter>
#include <QtGui/QPlatformSurfaceEvent>
#include <QtGui/QWheelEvent>
#include <QtGui/QWindow>
#include <private/qbackingstorerhisupport_p.h>
#include <QtWidgets/QAbstractButton>
#include <QtWidgets/QApplication>
#include <QtWidgets/QBoxLayout>
#include <QtWidgets/QCheckBox>
#include <QtWidgets/QDoubleSpinBox>
#include <QtWidgets/QGroupBox>
#include <QtWidgets/QLabel>
#include <QtWidgets/QLayout>
#include <QtWidgets/QLineEdit>
#include <QtWidgets/QPushButton>
#include <QtWidgets/QSizePolicy>
#include <QtWidgets/QSlider>
#include <QtWidgets/QWidget>
#include <qpa/qplatformbackingstore.h>
#include <qpa/qplatformgraphicsbuffer.h>
#include <rhi/qrhi.h>
#include <rhi/qshader.h>

#include <uv.h>

#if defined(Q_OS_WIN)
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>
#endif

#if defined(Q_OS_LINUX)
#include <dlfcn.h>
#endif

#if defined(__APPLE__)
#include <pthread.h>
#endif

#include <algorithm>
#include <atomic>
#include <array>
#include <cstdio>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <functional>
#include <memory>
#include <optional>
#include <stdexcept>
#include <string>
#include <string_view>
#include <unordered_map>
#include <vector>

#include "qt_window_compositor_shaders.inc"

namespace qt_solid_spike::qt {
namespace {

constexpr std::uint32_t kRootNodeId = 1;

#if defined(Q_OS_WIN)
template <typename FunctionPointer>
FunctionPointer resolve_process_symbol(HMODULE process_module,
                                       const char *symbol_name) {
  auto *symbol = GetProcAddress(process_module, symbol_name);
  if (symbol == nullptr) {
    std::string message("resolve Node/libuv symbol failed: ");
    message += symbol_name;
    throw std::runtime_error(message);
  }

  return reinterpret_cast<FunctionPointer>(symbol);
}

struct WindowsUvSymbols {
  decltype(&::uv_ref) ref = nullptr;
  decltype(&::uv_unref) unref = nullptr;
  decltype(&::uv_strerror) strerror = nullptr;
  decltype(&::uv_close) close = nullptr;
  decltype(&::uv_is_closing) is_closing = nullptr;
  decltype(&::uv_poll_init) poll_init = nullptr;
  decltype(&::uv_poll_start) poll_start = nullptr;
  decltype(&::uv_poll_stop) poll_stop = nullptr;
  decltype(&::uv_prepare_init) prepare_init = nullptr;
  decltype(&::uv_prepare_start) prepare_start = nullptr;
  decltype(&::uv_prepare_stop) prepare_stop = nullptr;
  decltype(&::uv_async_init) async_init = nullptr;
  decltype(&::uv_async_send) async_send = nullptr;
  decltype(&::uv_timer_init) timer_init = nullptr;
  decltype(&::uv_timer_start) timer_start = nullptr;
  decltype(&::uv_timer_stop) timer_stop = nullptr;
  decltype(&::uv_hrtime) hrtime = nullptr;

  static WindowsUvSymbols load() {
    HMODULE process_module = GetModuleHandleW(nullptr);
    if (process_module == nullptr) {
      throw std::runtime_error("resolve Node process module failed");
    }

    WindowsUvSymbols symbols;
    symbols.ref =
        resolve_process_symbol<decltype(symbols.ref)>(process_module, "uv_ref");
    symbols.unref = resolve_process_symbol<decltype(symbols.unref)>(
        process_module, "uv_unref");
    symbols.strerror = resolve_process_symbol<decltype(symbols.strerror)>(
        process_module, "uv_strerror");
    symbols.close = resolve_process_symbol<decltype(symbols.close)>(
        process_module, "uv_close");
    symbols.is_closing =
        resolve_process_symbol<decltype(symbols.is_closing)>(
            process_module, "uv_is_closing");
    symbols.poll_init = resolve_process_symbol<decltype(symbols.poll_init)>(
        process_module, "uv_poll_init");
    symbols.poll_start = resolve_process_symbol<decltype(symbols.poll_start)>(
        process_module, "uv_poll_start");
    symbols.poll_stop = resolve_process_symbol<decltype(symbols.poll_stop)>(
        process_module, "uv_poll_stop");
    symbols.prepare_init =
        resolve_process_symbol<decltype(symbols.prepare_init)>(
            process_module, "uv_prepare_init");
    symbols.prepare_start =
        resolve_process_symbol<decltype(symbols.prepare_start)>(
            process_module, "uv_prepare_start");
    symbols.prepare_stop =
        resolve_process_symbol<decltype(symbols.prepare_stop)>(
            process_module, "uv_prepare_stop");
    symbols.async_init = resolve_process_symbol<decltype(symbols.async_init)>(
        process_module, "uv_async_init");
    symbols.async_send = resolve_process_symbol<decltype(symbols.async_send)>(
        process_module, "uv_async_send");
    symbols.timer_init = resolve_process_symbol<decltype(symbols.timer_init)>(
        process_module, "uv_timer_init");
    symbols.timer_start =
        resolve_process_symbol<decltype(symbols.timer_start)>(
            process_module, "uv_timer_start");
    symbols.timer_stop = resolve_process_symbol<decltype(symbols.timer_stop)>(
        process_module, "uv_timer_stop");
    symbols.hrtime = resolve_process_symbol<decltype(symbols.hrtime)>(
        process_module, "uv_hrtime");
    return symbols;
  }
};

const WindowsUvSymbols &windows_uv_symbols() {
  static const WindowsUvSymbols symbols = WindowsUvSymbols::load();
  return symbols;
}

void qt_solid_uv_ref(uv_handle_t *handle) { windows_uv_symbols().ref(handle); }

void qt_solid_uv_unref(uv_handle_t *handle) {
  windows_uv_symbols().unref(handle);
}

const char *qt_solid_uv_strerror(int status) {
  return windows_uv_symbols().strerror(status);
}

void qt_solid_uv_close(uv_handle_t *handle, uv_close_cb close_cb) {
  windows_uv_symbols().close(handle, close_cb);
}

int qt_solid_uv_is_closing(const uv_handle_t *handle) {
  return windows_uv_symbols().is_closing(handle);
}

int qt_solid_uv_poll_init(uv_loop_t *loop, uv_poll_t *handle, int fd) {
  return windows_uv_symbols().poll_init(loop, handle, fd);
}

int qt_solid_uv_poll_start(uv_poll_t *handle, int events, uv_poll_cb poll_cb) {
  return windows_uv_symbols().poll_start(handle, events, poll_cb);
}

int qt_solid_uv_poll_stop(uv_poll_t *handle) {
  return windows_uv_symbols().poll_stop(handle);
}

int qt_solid_uv_prepare_init(uv_loop_t *loop, uv_prepare_t *handle) {
  return windows_uv_symbols().prepare_init(loop, handle);
}

int qt_solid_uv_prepare_start(uv_prepare_t *handle,
                              uv_prepare_cb prepare_cb) {
  return windows_uv_symbols().prepare_start(handle, prepare_cb);
}

int qt_solid_uv_prepare_stop(uv_prepare_t *handle) {
  return windows_uv_symbols().prepare_stop(handle);
}

int qt_solid_uv_async_init(uv_loop_t *loop, uv_async_t *handle,
                           uv_async_cb async_cb) {
  return windows_uv_symbols().async_init(loop, handle, async_cb);
}

int qt_solid_uv_async_send(uv_async_t *handle) {
  return windows_uv_symbols().async_send(handle);
}

int qt_solid_uv_timer_init(uv_loop_t *loop, uv_timer_t *handle) {
  return windows_uv_symbols().timer_init(loop, handle);
}

int qt_solid_uv_timer_start(uv_timer_t *handle, uv_timer_cb timer_cb,
                            std::uint64_t timeout, std::uint64_t repeat) {
  return windows_uv_symbols().timer_start(handle, timer_cb, timeout, repeat);
}

int qt_solid_uv_timer_stop(uv_timer_t *handle) {
  return windows_uv_symbols().timer_stop(handle);
}

std::uint64_t qt_solid_uv_hrtime() { return windows_uv_symbols().hrtime(); }

#define uv_ref qt_solid_uv_ref
#define uv_unref qt_solid_uv_unref
#define uv_strerror qt_solid_uv_strerror
#define uv_close qt_solid_uv_close
#define uv_is_closing qt_solid_uv_is_closing
#define uv_poll_init qt_solid_uv_poll_init
#define uv_poll_start qt_solid_uv_poll_start
#define uv_poll_stop qt_solid_uv_poll_stop
#define uv_prepare_init qt_solid_uv_prepare_init
#define uv_prepare_start qt_solid_uv_prepare_start
#define uv_prepare_stop qt_solid_uv_prepare_stop
#define uv_async_init qt_solid_uv_async_init
#define uv_async_send qt_solid_uv_async_send
#define uv_timer_init qt_solid_uv_timer_init
#define uv_timer_start qt_solid_uv_timer_start
#define uv_timer_stop qt_solid_uv_timer_stop
#define uv_hrtime qt_solid_uv_hrtime
#endif

#if defined(__APPLE__)
bool runtime_supports_cocoa_dispatcher_shim() noexcept {
  const auto runtime_version =
      QVersionNumber::fromString(QLatin1StringView(qVersion()));
  return runtime_version.majorVersion() == QT_VERSION_MAJOR &&
         runtime_version.minorVersion() == QT_VERSION_MINOR;
}

struct CocoaDispatcherShimProbeResult {
  QtSolidQCocoaEventDispatcherPrivatePrefix *dispatcher_private = nullptr;
  const char *error_message = nullptr;
};

CocoaDispatcherShimProbeResult probe_cocoa_dispatcher_private_prefix()
    noexcept {
  if (!runtime_supports_cocoa_dispatcher_shim()) {
    return {.dispatcher_private = nullptr,
            .error_message = "macOS Qt wait bridge requires Qt 6.10.x runtime"};
  }

  auto *dispatcher =
      QAbstractEventDispatcher::instance(QThread::currentThread());
  if (dispatcher == nullptr) {
    return {.dispatcher_private = nullptr,
            .error_message = "Qt host expected main-thread event dispatcher"};
  }

  const QMetaObject *meta_object = dispatcher->metaObject();
  if (meta_object == nullptr ||
      std::strcmp(meta_object->className(), "QCocoaEventDispatcher") != 0) {
    return {.dispatcher_private = nullptr,
            .error_message =
                "Qt host expected QCocoaEventDispatcher on macOS main thread"};
  }

  auto *dispatcher_private =
      reinterpret_cast<QtSolidQCocoaEventDispatcherPrivatePrefix *>(
          QAbstractEventDispatcherPrivate::get(dispatcher));
  if (dispatcher_private == nullptr) {
    return {.dispatcher_private = nullptr,
            .error_message =
                "Qt host failed to access QCocoaEventDispatcher private state"};
  }

  return {.dispatcher_private = dispatcher_private, .error_message = nullptr};
}
#endif

enum class WidgetKind : std::uint8_t {
#include "qt_widget_kind_enum.inc"
};

enum class FlexDirectionKind : std::uint8_t {
  Column = 1,
  Row = 2,
};

enum class AlignItemsKind : std::uint8_t {
  FlexStart = 1,
  Center = 2,
  FlexEnd = 3,
  Stretch = 4,
};

enum class JustifyContentKind : std::uint8_t {
  FlexStart = 1,
  Center = 2,
  FlexEnd = 3,
};

enum class FocusPolicyKind : std::uint8_t {
  NoFocus = 1,
  TabFocus = 2,
  ClickFocus = 3,
  StrongFocus = 4,
};

[[noreturn]] void throw_error(const char *message);

QtMethodValue method_unit() {
  return QtMethodValue{.kind_tag = 0,
                       .string_value = rust::String(),
                       .bool_value = false,
                       .i32_value = 0,
                       .f64_value = 0.0};
}

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

enum class EventPayloadKind : std::uint8_t {
  Unit = 0,
  Scalar = 1,
  Object = 2,
};

enum class EventValueKind : std::uint8_t {
  String = 1,
  Bool = 2,
  I32 = 3,
  F64 = 4,
};

enum class PropPayloadKind : std::uint8_t {
  String = 1,
  Bool = 2,
  I32 = 3,
  Enum = 4,
  F64 = 5,
};

enum class PropLowerKind : std::uint8_t {
  MetaProperty,
  Custom,
};

enum class EventLowerKind : std::uint8_t {
  QtSignal,
  Custom,
};

struct CompiledPropBinding {
  std::uint16_t prop_id = 0;
  std::string js_name;
  PropPayloadKind payload_kind = PropPayloadKind::String;
  bool non_negative = false;
  PropLowerKind lower_kind = PropLowerKind::Custom;
  std::string lower_name;
  int property_index = -1;
  bool has_read_lowering = false;
  PropLowerKind read_lower_kind = PropLowerKind::Custom;
  std::string read_lower_name;
  int read_property_index = -1;
};

struct CompiledEventBinding {
  std::uint8_t event_index = 0;
  EventPayloadKind payload_kind = EventPayloadKind::Unit;
  bool has_scalar_kind = false;
  EventValueKind scalar_kind = EventValueKind::String;
  EventLowerKind lower_kind = EventLowerKind::QtSignal;
  std::string lower_name;
  struct PayloadField {
    std::string js_name;
    EventValueKind kind = EventValueKind::String;
  };
  std::vector<PayloadField> payload_fields;
  int signal_method_index = -1;
};

struct CompiledWidgetContract {
  std::uint8_t kind_tag = 0;
  std::vector<CompiledPropBinding> props;
  std::vector<CompiledEventBinding> events;
};

class QtHostState;

int current_runtime_wait_bridge_unix_fd() noexcept;
void drain_runtime_wait_bridge_notifications() noexcept;
std::optional<std::uint64_t> current_runtime_wait_bridge_timer_delay_ms()
    noexcept;

[[noreturn]] void throw_error(const char *message) {
  throw std::runtime_error(message);
}

[[noreturn]] void throw_uv_error(const char *operation, int status) {
  std::string message(operation);
  message += ": ";
  message += uv_strerror(status);
  throw std::runtime_error(message);
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

#if defined(Q_OS_LINUX)
void *open_linux_library(const char *primary_name, const char *fallback_name) {
  if (void *handle = dlopen(primary_name, RTLD_LAZY | RTLD_LOCAL)) {
    return handle;
  }
  return dlopen(fallback_name, RTLD_LAZY | RTLD_LOCAL);
}

template <typename FunctionPointer>
FunctionPointer resolve_linux_symbol(void *library_handle,
                                     const char *symbol_name) {
  if (library_handle == nullptr) {
    return nullptr;
  }

  return reinterpret_cast<FunctionPointer>(dlsym(library_handle, symbol_name));
}

#if QT_CONFIG(xcb)
int qt_runtime_x11_wait_bridge_unix_fd() {
  auto *app = qGuiApp;
  if (app == nullptr) {
    return -1;
  }

  auto *native = app->nativeInterface<QNativeInterface::QX11Application>();
  if (native == nullptr) {
    return -1;
  }

  auto *connection = native->connection();
  if (connection == nullptr) {
    return -1;
  }

  using XcbGetFileDescriptorFn = int (*)(xcb_connection_t *);
  static void *const xcb_library_handle =
      open_linux_library("libxcb.so.1", "libxcb.so");
  static const auto get_file_descriptor =
      resolve_linux_symbol<XcbGetFileDescriptorFn>(xcb_library_handle,
                                                   "xcb_get_file_descriptor");
  if (get_file_descriptor == nullptr) {
    return -1;
  }

  return get_file_descriptor(connection);
}
#endif

#if QT_CONFIG(wayland)
int qt_runtime_wayland_wait_bridge_unix_fd() {
  auto *app = qGuiApp;
  if (app == nullptr) {
    return -1;
  }

  auto *native = app->nativeInterface<QNativeInterface::QWaylandApplication>();
  if (native == nullptr) {
    return -1;
  }

  auto *display = native->display();
  if (display == nullptr) {
    return -1;
  }

  using WaylandDisplayGetFdFn = int (*)(wl_display *);
  static void *const wayland_client_library_handle =
      open_linux_library("libwayland-client.so.0", "libwayland-client.so");
  static const auto get_fd = resolve_linux_symbol<WaylandDisplayGetFdFn>(
      wayland_client_library_handle, "wl_display_get_fd");
  if (get_fd == nullptr) {
    return -1;
  }

  return get_fd(display);
}
#endif
#endif

int qt_runtime_wait_bridge_unix_fd_impl() {
#if defined(Q_OS_LINUX)
  auto *app = qGuiApp;
  if (app == nullptr) {
    return -1;
  }

  const QString platform_name = QGuiApplication::platformName();

#if QT_CONFIG(xcb)
  if (platform_name.startsWith("xcb") || platform_name.startsWith("x11")) {
    if (const int fd = qt_runtime_x11_wait_bridge_unix_fd(); fd >= 0) {
      return fd;
    }
  }
#endif

#if QT_CONFIG(wayland)
  if (platform_name.startsWith("wayland")) {
    if (const int fd = qt_runtime_wayland_wait_bridge_unix_fd(); fd >= 0) {
      return fd;
    }
  }
#endif

#if QT_CONFIG(xcb)
  if (const int fd = qt_runtime_x11_wait_bridge_unix_fd(); fd >= 0) {
    return fd;
  }
#endif

#if QT_CONFIG(wayland)
  if (const int fd = qt_runtime_wayland_wait_bridge_unix_fd(); fd >= 0) {
    return fd;
  }
#endif

  return -1;
#elif defined(__APPLE__)
  return current_runtime_wait_bridge_unix_fd();
#else
  return -1;
#endif
}

std::uint64_t qt_runtime_wait_bridge_windows_handle_impl() { return 0; }

WaitBridgeKind qt_runtime_wait_bridge_kind_impl() {
  if (qt_runtime_wait_bridge_unix_fd_impl() >= 0) {
    return WaitBridgeKind::UnixFd;
  }
  if (qt_runtime_wait_bridge_windows_handle_impl() != 0) {
    return WaitBridgeKind::WindowsHandle;
  }
  return WaitBridgeKind::None;
}

bool on_required_qt_host_thread() {
#if defined(__APPLE__)
  return pthread_main_np() == 1;
#else
  return true;
#endif
}

QString to_qstring(rust::Str value) {
  return QString::fromUtf8(value.data(), static_cast<qsizetype>(value.size()));
}

rust::String to_rust_string(const QString &value) {
  const auto utf8 = value.toUtf8();
  return rust::String(utf8.constData(), static_cast<std::size_t>(utf8.size()));
}

WidgetKind widget_kind_from_tag(std::uint8_t kind_tag) {
  switch (kind_tag) {
#include "qt_widget_kind_from_tag.inc"
  default:
    throw_error("received unknown widget kind tag");
  }
}

bool widget_kind_is_top_level(WidgetKind kind) {
  switch (kind) {
#include "qt_widget_top_level_cases.inc"
  }

  throw_error("received unknown widget kind for top-level lookup");
}

FlexDirectionKind flex_direction_from_tag(std::uint8_t direction_tag) {
  switch (direction_tag) {
  case static_cast<std::uint8_t>(FlexDirectionKind::Column):
    return FlexDirectionKind::Column;
  case static_cast<std::uint8_t>(FlexDirectionKind::Row):
    return FlexDirectionKind::Row;
  default:
    throw_error("received unknown flex direction tag");
  }
}

AlignItemsKind align_items_from_tag(std::uint8_t align_items_tag) {
  switch (align_items_tag) {
  case static_cast<std::uint8_t>(AlignItemsKind::FlexStart):
    return AlignItemsKind::FlexStart;
  case static_cast<std::uint8_t>(AlignItemsKind::Center):
    return AlignItemsKind::Center;
  case static_cast<std::uint8_t>(AlignItemsKind::FlexEnd):
    return AlignItemsKind::FlexEnd;
  case static_cast<std::uint8_t>(AlignItemsKind::Stretch):
    return AlignItemsKind::Stretch;
  default:
    throw_error("received unknown align items tag");
  }
}

JustifyContentKind justify_content_from_tag(std::uint8_t justify_content_tag) {
  switch (justify_content_tag) {
  case static_cast<std::uint8_t>(JustifyContentKind::FlexStart):
    return JustifyContentKind::FlexStart;
  case static_cast<std::uint8_t>(JustifyContentKind::Center):
    return JustifyContentKind::Center;
  case static_cast<std::uint8_t>(JustifyContentKind::FlexEnd):
    return JustifyContentKind::FlexEnd;
  default:
    throw_error("received unknown justify content tag");
  }
}

Qt::FocusPolicy focus_policy_from_tag(std::uint8_t focus_policy_tag) {
  switch (static_cast<FocusPolicyKind>(focus_policy_tag)) {
  case FocusPolicyKind::NoFocus:
    return Qt::NoFocus;
  case FocusPolicyKind::TabFocus:
    return Qt::TabFocus;
  case FocusPolicyKind::ClickFocus:
    return Qt::ClickFocus;
  case FocusPolicyKind::StrongFocus:
    return Qt::StrongFocus;
  }

  throw_error("received unknown focus policy tag");
}

EventPayloadKind event_payload_kind_from_tag(std::uint8_t payload_tag) {
  switch (payload_tag) {
  case static_cast<std::uint8_t>(EventPayloadKind::Unit):
    return EventPayloadKind::Unit;
  case static_cast<std::uint8_t>(EventPayloadKind::Scalar):
    return EventPayloadKind::Scalar;
  case static_cast<std::uint8_t>(EventPayloadKind::Object):
    return EventPayloadKind::Object;
  default:
    throw_error("received unknown event payload tag");
  }
}

EventValueKind event_value_kind_from_tag(std::uint8_t payload_tag) {
  switch (payload_tag) {
  case static_cast<std::uint8_t>(EventValueKind::String):
    return EventValueKind::String;
  case static_cast<std::uint8_t>(EventValueKind::Bool):
    return EventValueKind::Bool;
  case static_cast<std::uint8_t>(EventValueKind::I32):
    return EventValueKind::I32;
  case static_cast<std::uint8_t>(EventValueKind::F64):
    return EventValueKind::F64;
  default:
    throw_error("received unknown event payload tag");
  }
}

PropPayloadKind prop_payload_kind_from_tag(std::uint8_t payload_tag) {
  switch (payload_tag) {
  case static_cast<std::uint8_t>(PropPayloadKind::String):
    return PropPayloadKind::String;
  case static_cast<std::uint8_t>(PropPayloadKind::Bool):
    return PropPayloadKind::Bool;
  case static_cast<std::uint8_t>(PropPayloadKind::I32):
    return PropPayloadKind::I32;
  case static_cast<std::uint8_t>(PropPayloadKind::Enum):
    return PropPayloadKind::Enum;
  case static_cast<std::uint8_t>(PropPayloadKind::F64):
    return PropPayloadKind::F64;
  default:
    throw_error("received unknown prop payload tag");
  }
}

PropLowerKind prop_lower_kind_from_tag(std::uint8_t lower_kind_tag) {
  switch (lower_kind_tag) {
  case 1:
    return PropLowerKind::MetaProperty;
  case 2:
    return PropLowerKind::Custom;
  default:
    throw_error("received unknown prop lower kind tag");
  }
}

EventLowerKind event_lower_kind_from_tag(std::uint8_t lower_kind_tag) {
  switch (lower_kind_tag) {
  case 1:
    return EventLowerKind::QtSignal;
  case 2:
    return EventLowerKind::Custom;
  default:
    throw_error("received unknown event lower kind tag");
  }
}

bool qt_wgpu_timing_enabled() {
  static const bool enabled = qEnvironmentVariableIsSet("QT_SOLID_WGPU_TIMING");
  return enabled;
}

struct QtWgpuWakeStats {
  std::atomic<std::uint64_t> request_qt_pump_calls{0};
  std::atomic<std::uint64_t> libuv_request_pump_calls{0};
  std::atomic<std::uint64_t> libuv_pump_events{0};
  std::atomic<std::uint64_t> libuv_zero_timeout_pumps{0};
  std::atomic<std::uint64_t> compositor_present_completions{0};
  std::atomic<std::uint64_t> compositor_frame_requests{0};
  std::atomic<std::uint64_t> compositor_frame_posts{0};
  std::atomic<std::uint64_t> compositor_frame_runs{0};
  std::atomic<std::uint64_t> compositor_frame_presented{0};
  std::atomic<std::uint64_t> compositor_frame_busy{0};
  std::atomic<std::uint64_t> compositor_frame_idle{0};
  std::atomic<std::uint64_t> compositor_frame_needs_qt_repaint{0};
  std::atomic<std::uint64_t> last_logged_frame_runs{0};
};

QtWgpuWakeStats &qt_wgpu_wake_stats() {
  static QtWgpuWakeStats stats;
  return stats;
}

void maybe_log_qt_wgpu_wake_stats() {
  if (!qt_wgpu_timing_enabled()) {
    return;
  }

  auto &stats = qt_wgpu_wake_stats();
  const std::uint64_t frame_runs =
      stats.compositor_frame_runs.load(std::memory_order_relaxed);
  if (frame_runs == 0 || frame_runs % 240 != 0) {
    return;
  }

  std::uint64_t last_logged =
      stats.last_logged_frame_runs.load(std::memory_order_relaxed);
  while (last_logged < frame_runs) {
    if (stats.last_logged_frame_runs.compare_exchange_weak(
            last_logged, frame_runs, std::memory_order_relaxed,
            std::memory_order_relaxed)) {
      std::fprintf(
          stderr,
          "qt-wgpu wake request_qt_pump=%llu libuv_request_pump=%llu libuv_pump_events=%llu libuv_zero_timeout=%llu present_completions=%llu frame_requests=%llu frame_posts=%llu frame_runs=%llu presented=%llu busy=%llu idle=%llu repaint=%llu\n",
          static_cast<unsigned long long>(
              stats.request_qt_pump_calls.load(std::memory_order_relaxed)),
          static_cast<unsigned long long>(
              stats.libuv_request_pump_calls.load(std::memory_order_relaxed)),
          static_cast<unsigned long long>(
              stats.libuv_pump_events.load(std::memory_order_relaxed)),
          static_cast<unsigned long long>(
              stats.libuv_zero_timeout_pumps.load(std::memory_order_relaxed)),
          static_cast<unsigned long long>(stats.compositor_present_completions.load(
              std::memory_order_relaxed)),
          static_cast<unsigned long long>(
              stats.compositor_frame_requests.load(std::memory_order_relaxed)),
          static_cast<unsigned long long>(
              stats.compositor_frame_posts.load(std::memory_order_relaxed)),
          static_cast<unsigned long long>(frame_runs),
          static_cast<unsigned long long>(stats.compositor_frame_presented.load(
              std::memory_order_relaxed)),
          static_cast<unsigned long long>(
              stats.compositor_frame_busy.load(std::memory_order_relaxed)),
          static_cast<unsigned long long>(
              stats.compositor_frame_idle.load(std::memory_order_relaxed)),
          static_cast<unsigned long long>(stats.compositor_frame_needs_qt_repaint.load(
              std::memory_order_relaxed)));
      return;
    }
  }
}

void record_request_qt_pump() {
  qt_wgpu_wake_stats().request_qt_pump_calls.fetch_add(1, std::memory_order_relaxed);
  maybe_log_qt_wgpu_wake_stats();
}

void record_libuv_request_pump() {
  qt_wgpu_wake_stats().libuv_request_pump_calls.fetch_add(1, std::memory_order_relaxed);
  maybe_log_qt_wgpu_wake_stats();
}

void record_libuv_pump_events(bool zero_timeout_pumped) {
  auto &stats = qt_wgpu_wake_stats();
  stats.libuv_pump_events.fetch_add(1, std::memory_order_relaxed);
  if (zero_timeout_pumped) {
    stats.libuv_zero_timeout_pumps.fetch_add(1, std::memory_order_relaxed);
  }
  maybe_log_qt_wgpu_wake_stats();
}

void record_compositor_present_complete() {
  qt_wgpu_wake_stats().compositor_present_completions.fetch_add(
      1, std::memory_order_relaxed);
  maybe_log_qt_wgpu_wake_stats();
}

void record_compositor_frame_request() {
  qt_wgpu_wake_stats().compositor_frame_requests.fetch_add(
      1, std::memory_order_relaxed);
  maybe_log_qt_wgpu_wake_stats();
}

void record_compositor_frame_post() {
  qt_wgpu_wake_stats().compositor_frame_posts.fetch_add(1, std::memory_order_relaxed);
  maybe_log_qt_wgpu_wake_stats();
}

void record_compositor_frame_status(
    qt_wgpu_renderer::UnifiedCompositorDriveStatus status) {
  auto &stats = qt_wgpu_wake_stats();
  stats.compositor_frame_runs.fetch_add(1, std::memory_order_relaxed);
  switch (status) {
  case qt_wgpu_renderer::UnifiedCompositorDriveStatus::Presented:
    stats.compositor_frame_presented.fetch_add(1, std::memory_order_relaxed);
    break;
  case qt_wgpu_renderer::UnifiedCompositorDriveStatus::Busy:
    stats.compositor_frame_busy.fetch_add(1, std::memory_order_relaxed);
    break;
  case qt_wgpu_renderer::UnifiedCompositorDriveStatus::Idle:
    stats.compositor_frame_idle.fetch_add(1, std::memory_order_relaxed);
    break;
  case qt_wgpu_renderer::UnifiedCompositorDriveStatus::NeedsQtRepaint:
    stats.compositor_frame_needs_qt_repaint.fetch_add(1, std::memory_order_relaxed);
    break;
  }
  maybe_log_qt_wgpu_wake_stats();
}

#include "qt_widget_kind_values.inc"

#include "debug.cpp"

#include "registry/host.cpp"

#include "event.cpp"

#include "uv_pump.cpp"

#include "registry/core.cpp"

} // namespace

#include "qt_opaque_dispatch.inc"

bool qt_host_started() { return g_host != nullptr && g_host->started(); }

std::uint8_t qt_runtime_wait_bridge_kind_tag() {
  return static_cast<std::uint8_t>(qt_runtime_wait_bridge_kind_impl());
}

std::int32_t qt_runtime_wait_bridge_unix_fd() {
  return qt_runtime_wait_bridge_unix_fd_impl();
}

std::uint64_t qt_runtime_wait_bridge_windows_handle() {
  return qt_runtime_wait_bridge_windows_handle_impl();
}

void start_qt_host(std::uintptr_t uv_loop_ptr) {
  if (!on_required_qt_host_thread()) {
    throw_error("QtApp.start must run on macOS main thread");
  }

  if (uv_loop_ptr == 0) {
    throw_error("received null libuv loop pointer from N-API");
  }

  if (!g_host) {
    g_host = new QtHostState(reinterpret_cast<uv_loop_t *>(uv_loop_ptr));
  }

  try {
    g_host->start();
  } catch (...) {
    delete g_host;
    g_host = nullptr;
    throw;
  }
}

void shutdown_qt_host() {
  if (!on_required_qt_host_thread()) {
    throw_error("QtApp.shutdown must run on macOS main thread");
  }

  if (!g_host) {
    return;
  }

  g_host->shutdown();
  delete g_host;
  g_host = nullptr;
}

void qt_create_widget(std::uint32_t id, std::uint8_t kind_tag) {
  if (!g_host || !g_host->started()) {
    throw_error("call QtApp.start before creating Qt widgets");
  }

  g_host->registry().create_widget(id, kind_tag);
  request_qt_pump();
}

void qt_insert_child(std::uint32_t parent_id, std::uint32_t child_id,
                     std::uint32_t anchor_id_or_zero) {
  if (!g_host || !g_host->started()) {
    throw_error("call QtApp.start before inserting Qt widgets");
  }

  g_host->registry().insert_child(parent_id, child_id, anchor_id_or_zero);
  request_qt_pump();
}

void qt_remove_child(std::uint32_t parent_id, std::uint32_t child_id) {
  if (!g_host || !g_host->started()) {
    throw_error("call QtApp.start before removing Qt widgets");
  }

  g_host->registry().remove_child(parent_id, child_id);
  request_qt_pump();
}

void qt_destroy_widget(std::uint32_t id) {
  if (!g_host || !g_host->started()) {
    return;
  }

  g_host->registry().destroy_widget(id);
  request_qt_pump();
}

void qt_request_repaint(std::uint32_t id) {
  if (!g_host || !g_host->started()) {
    throw_error("call QtApp.start before requesting a repaint");
  }

  g_host->registry().request_repaint(id);
  request_qt_pump();
}

bool qt_request_window_compositor_frame(std::uint32_t id) {
  if (!g_host || !g_host->started()) {
    throw_error("call QtApp.start before requesting a compositor frame");
  }

  const bool requested = g_host->registry().request_window_compositor_frame(id);
#if !defined(Q_OS_MACOS)
  request_qt_pump();
#endif
  return requested;
}

void notify_window_compositor_present_complete(std::uint32_t id) {
  if (!g_host || !g_host->started()) {
    return;
  }

  record_compositor_present_complete();
  auto *context = QCoreApplication::instance();
  if (context == nullptr) {
    return;
  }

  QTimer::singleShot(0, context, [id]() {
    if (!g_host || !g_host->started()) {
      return;
    }

    g_host->registry().notify_window_compositor_frame_complete(id);
  });
#if defined(Q_OS_MACOS)
  request_qt_native_wait_once();
#else
  request_qt_pump();
#endif
}

QtWidgetCaptureLayout qt_capture_widget_layout(std::uint32_t id) {
  if (!g_host || !g_host->started()) {
    throw_error("call QtApp.start before capturing a Qt widget");
  }

  return g_host->registry().capture_widget_layout(id);
}

rust::Vec<QtRect> qt_capture_widget_visible_rects(std::uint32_t id) {
  if (!g_host || !g_host->started()) {
    throw_error("call QtApp.start before reading a Qt widget visible region");
  }

  return g_host->registry().capture_widget_visible_rects(id);
}

void qt_capture_widget_into(std::uint32_t id, std::uint32_t width_px,
                            std::uint32_t height_px, std::size_t stride,
                            bool include_children,
                            rust::Slice<std::uint8_t> bytes) {
  if (!g_host || !g_host->started()) {
    throw_error("call QtApp.start before capturing a Qt widget");
  }

  g_host->registry().capture_widget_into(id, width_px, height_px, stride,
                                         include_children, bytes);
}

void qt_capture_widget_region_into(std::uint32_t id, std::uint32_t width_px,
                                   std::uint32_t height_px, std::size_t stride,
                                   bool include_children, QtRect rect,
                                   rust::Slice<std::uint8_t> bytes) {
  if (!g_host || !g_host->started()) {
    throw_error("call QtApp.start before capturing a Qt widget");
  }

  g_host->registry().capture_widget_region_into(id, width_px, height_px, stride,
                                                include_children, rect, bytes);
}

void qt_apply_string_prop(std::uint32_t id, std::uint16_t prop_id,
                          std::uint64_t trace_id, rust::Str value) {
  if (!g_host || !g_host->started()) {
    throw_error("call QtApp.start before applying Qt string props");
  }

  g_host->registry().apply_string_prop(id, prop_id, trace_id, value);
  request_qt_pump();
}

void qt_apply_i32_prop(std::uint32_t id, std::uint16_t prop_id,
                       std::uint64_t trace_id, std::int32_t value) {
  if (!g_host || !g_host->started()) {
    throw_error("call QtApp.start before applying Qt i32 props");
  }

  g_host->registry().apply_i32_prop(id, prop_id, trace_id, value);
  request_qt_pump();
}

void qt_apply_f64_prop(std::uint32_t id, std::uint16_t prop_id,
                       std::uint64_t trace_id, double value) {
  if (!g_host || !g_host->started()) {
    throw_error("call QtApp.start before applying Qt f64 props");
  }

  g_host->registry().apply_f64_prop(id, prop_id, trace_id, value);
  request_qt_pump();
}

void qt_apply_bool_prop(std::uint32_t id, std::uint16_t prop_id,
                        std::uint64_t trace_id, bool value) {
  if (!g_host || !g_host->started()) {
    throw_error("call QtApp.start before applying Qt bool props");
  }

  g_host->registry().apply_bool_prop(id, prop_id, trace_id, value);
  request_qt_pump();
}

QtMethodValue qt_call_host_slot(std::uint32_t id, std::uint16_t slot,
                                const rust::Vec<QtMethodValue> &args) {
  if (!g_host || !g_host->started()) {
    throw_error("call QtApp.start before calling Qt host methods");
  }

  const auto value = g_host->registry().call_host_slot(id, slot, args);
  request_qt_pump();
  return value;
}

rust::String qt_read_string_prop(std::uint32_t id, std::uint16_t prop_id) {
  if (!g_host || !g_host->started()) {
    throw_error("call QtApp.start before reading Qt string props");
  }

  return g_host->registry().read_string_prop(id, prop_id);
}

std::int32_t qt_read_i32_prop(std::uint32_t id, std::uint16_t prop_id) {
  if (!g_host || !g_host->started()) {
    throw_error("call QtApp.start before reading Qt i32 props");
  }

  return g_host->registry().read_i32_prop(id, prop_id);
}

double qt_read_f64_prop(std::uint32_t id, std::uint16_t prop_id) {
  if (!g_host || !g_host->started()) {
    throw_error("call QtApp.start before reading Qt f64 props");
  }

  return g_host->registry().read_f64_prop(id, prop_id);
}

bool qt_read_bool_prop(std::uint32_t id, std::uint16_t prop_id) {
  if (!g_host || !g_host->started()) {
    throw_error("call QtApp.start before reading Qt bool props");
  }

  return g_host->registry().read_bool_prop(id, prop_id);
}

QtRealizedNodeState qt_debug_node_state(std::uint32_t id) {
  if (!g_host || !g_host->started()) {
    throw_error("call QtApp.start before reading a Qt debug snapshot");
  }

  return g_host->registry().debug_node_state(id);
}

QtNodeBounds debug_node_bounds(std::uint32_t id) {
  if (!g_host || !g_host->started()) {
    throw_error("call QtApp.start before reading debug node bounds");
  }

  return g_host->registry().debug_node_bounds(id);
}

std::uint32_t debug_node_at_point(std::int32_t screen_x,
                                  std::int32_t screen_y) {
  if (!g_host || !g_host->started()) {
    throw_error("call QtApp.start before reading debug node at point");
  }

  return g_host->registry().debug_node_at_point(screen_x, screen_y);
}

void debug_set_inspect_mode(bool enabled) {
  if (!g_host || !g_host->started()) {
    throw_error("call QtApp.start before toggling debug inspect mode");
  }

  g_host->registry().debug_set_inspect_mode(enabled);
  request_qt_pump();
}

void debug_click_node(std::uint32_t id) {
  if (!g_host || !g_host->started()) {
    throw_error("call QtApp.start before triggering debug clicks");
  }

  g_host->registry().debug_click_node(id);
  request_qt_pump();
}

void debug_close_node(std::uint32_t id) {
  if (!g_host || !g_host->started()) {
    throw_error("call QtApp.start before triggering debug close requests");
  }

  g_host->registry().debug_close_node(id);
  request_qt_pump();
}

void debug_input_insert_text(std::uint32_t id, rust::Str value) {
  if (!g_host || !g_host->started()) {
    throw_error("call QtApp.start before triggering debug text input");
  }

  g_host->registry().debug_input_insert_text(id, value);
  request_qt_pump();
}

void debug_highlight_node(std::uint32_t id) {
  if (!g_host || !g_host->started()) {
    throw_error("call QtApp.start before triggering debug highlight");
  }

  g_host->registry().debug_highlight_node(id);
  request_qt_pump();
}

void debug_clear_highlight() {
  if (!g_host || !g_host->started()) {
    throw_error("call QtApp.start before clearing debug highlight");
  }

  g_host->registry().debug_clear_highlight();
  request_qt_pump();
}

void schedule_debug_event(std::uint32_t delay_ms, rust::Str name) {
  if (!g_host || !g_host->started()) {
    throw_error("call QtApp.start before scheduling a debug event");
  }

  std::string event_name(name);
  auto *context = QCoreApplication::instance();
  QTimer::singleShot(static_cast<int>(delay_ms), context,
                     [event_name = std::move(event_name)]() {
                       qt_solid_spike::qt::emit_debug_event(
                           rust::Str(event_name));
                     });
  request_qt_pump();
}

std::uint64_t trace_now_ns() { return uv_hrtime(); }

} // namespace qt_solid_spike::qt

extern "C" void qt_solid_notify_window_compositor_present_complete(
    std::uint32_t id) {
  qt_solid_spike::qt::notify_window_compositor_present_complete(id);
}
