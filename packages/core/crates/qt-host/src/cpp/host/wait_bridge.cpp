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

std::uint64_t qt_runtime_wait_bridge_windows_handle_impl() {
#if defined(Q_OS_WIN)
  return window_host_wait_bridge_windows_handle();
#else
  return 0;
#endif
}

WaitBridgeKind qt_runtime_wait_bridge_kind_impl() {
  if (qt_runtime_wait_bridge_unix_fd_impl() >= 0) {
    return WaitBridgeKind::UnixFd;
  }
  if (qt_runtime_wait_bridge_windows_handle_impl() != 0) {
    return WaitBridgeKind::WindowsHandle;
  }
  return WaitBridgeKind::None;
}
