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

static bool qt_solid_wgpu_trace_enabled() {
  static const bool enabled = qEnvironmentVariableIsSet("QT_SOLID_WGPU_TRACE");
  return enabled;
}

template <typename... Args>
static void qt_solid_wgpu_trace(const char *fmt, Args... args) {
  if (!qt_solid_wgpu_trace_enabled()) {
    return;
  }
  std::fprintf(stdout, "[qt-host] ");
  std::fprintf(stdout, fmt, args...);
  std::fprintf(stdout, "\n");
  std::fflush(stdout);
}

class LibuvQtPump final {
public:
  explicit LibuvQtPump(uv_loop_t *loop) : loop_(loop) {
    if (const char *value = std::getenv("QT_SOLID_AGGRESSIVE_PUMP")) {
      aggressive_poll_ = std::strcmp(value, "0") != 0;
    }

    supports_zero_timeout_pump_ =
        qt_solid_spike::qt::window_host_supports_zero_timeout_pump();
    supports_external_wake_ =
        qt_solid_spike::qt::window_host_supports_external_wake();
    wait_bridge_kind_ = wait_bridge_kind_from_tag(
        qt_solid_spike::qt::window_host_wait_bridge_kind_tag());
    if (wait_bridge_kind_ == WaitBridgeKind::UnixFd) {
      wait_bridge_unix_fd_ =
          qt_solid_spike::qt::window_host_wait_bridge_unix_fd();
    } else if (wait_bridge_kind_ == WaitBridgeKind::WindowsHandle) {
      wait_bridge_windows_handle_ =
          qt_solid_spike::qt::window_host_wait_bridge_windows_handle();
    }
    driver_mode_ = select_driver_mode();

    async_.data = this;
    prepare_.data = this;
    timer_.data = this;
    wait_bridge_poll_.data = this;

#if defined(__APPLE__)
    setup_cfrunloop_libuv_source();
#elif defined(Q_OS_WIN)
    setup_win32_wait_bridge();
#endif
  }

  ~LibuvQtPump() {
#if defined(Q_OS_WIN)
    teardown_win32_wait_bridge();
#endif
  }

  void start() {
    if (started_) {
      return;
    }

    int status = uv_async_init(loop_, &async_, &LibuvQtPump::on_async);
    if (status != 0) {
      throw_uv_error("uv_async_init", status);
    }
    async_initialized_ = true;

    status = uv_prepare_init(loop_, &prepare_);
    if (status != 0) {
      throw_uv_error("uv_prepare_init", status);
    }
    prepare_initialized_ = true;

    status = uv_timer_init(loop_, &timer_);
    if (status != 0) {
      throw_uv_error("uv_timer_init", status);
    }
    timer_initialized_ = true;

    if (driver_mode_ == PumpDriverMode::WaitBridge &&
        wait_bridge_kind_ == WaitBridgeKind::UnixFd) {
      status = uv_poll_init(loop_, &wait_bridge_poll_, wait_bridge_unix_fd_);
      if (status != 0) {
        throw_uv_error("uv_poll_init", status);
      }
      wait_bridge_poll_initialized_ = true;

      status = uv_poll_start(&wait_bridge_poll_, UV_READABLE,
                             &LibuvQtPump::on_wait_bridge_poll);
      if (status != 0) {
        throw_uv_error("uv_poll_start", status);
      }
    }

#if defined(__APPLE__)
    // On macOS, always run prepare to give CFRunLoop a chance to process
    // native sources (display-link, AppKit events) before libuv enters kevent.
    if (driver_mode_ == PumpDriverMode::WaitBridge ||
        driver_mode_ == PumpDriverMode::PollingFallback) {
#elif defined(Q_OS_WIN)
    // On Windows, run prepare in WaitBridge mode to PeekMessage before
    // libuv enters IOCP, and in PollingFallback as backstop.
    if (driver_mode_ == PumpDriverMode::WaitBridge ||
        driver_mode_ == PumpDriverMode::PollingFallback) {
#else
    if (driver_mode_ == PumpDriverMode::PollingFallback) {
#endif
      status = uv_prepare_start(&prepare_, &LibuvQtPump::on_prepare);
      if (status != 0) {
        throw_uv_error("uv_prepare_start", status);
      }
    }

    uv_unref(reinterpret_cast<uv_handle_t *>(&async_));
    uv_unref(reinterpret_cast<uv_handle_t *>(&prepare_));
    if (wait_bridge_poll_initialized_) {
      uv_unref(reinterpret_cast<uv_handle_t *>(&wait_bridge_poll_));
    }
    set_timer_referenced(false);

    started_ = true;
    reschedule_timer();
  }

  // uv_close completes on later loop turn, so callers must release ownership
  // when this returns true and let on_close delete the pump.
  bool close_async() {
    if (closing_) {
      return true;
    }

    if (timer_initialized_) {
      uv_timer_stop(&timer_);
      set_timer_referenced(false);
    }
    if (prepare_initialized_) {
      uv_prepare_stop(&prepare_);
    }
    if (wait_bridge_poll_initialized_) {
      uv_poll_stop(&wait_bridge_poll_);
    }

    started_ = false;
#if defined(__APPLE__)
    teardown_cfrunloop_libuv_source();
#elif defined(Q_OS_WIN)
    teardown_win32_wait_bridge();
#endif
    close_pending_count_ = 0;

    close_handle(&timer_, timer_initialized_);
    close_handle(&prepare_, prepare_initialized_);
    close_handle(&wait_bridge_poll_, wait_bridge_poll_initialized_);
    close_handle(&async_, async_initialized_);

    closing_ = close_pending_count_ > 0;
    return closing_;
  }

  void abandon_for_process_exit() {
    if (timer_initialized_) {
      uv_timer_stop(&timer_);
      set_timer_referenced(false);
    }
    if (prepare_initialized_) {
      uv_prepare_stop(&prepare_);
    }
    if (wait_bridge_poll_initialized_) {
      uv_poll_stop(&wait_bridge_poll_);
    }
    started_ = false;
  }

  void request_shutdown(std::function<void()> on_shutdown) {
    if (shutdown_requested_.exchange(true)) {
      return;
    }
    on_shutdown_ = std::move(on_shutdown);
    started_ = false;
    if (async_initialized_) {
      uv_async_send(&async_);
    }
  }

  void pump_events() {
    if (pumping_ || !started_ || shutdown_requested_.load()
        || QCoreApplication::instance() == nullptr) {
      return;
    }

    pumping_ = true;
    drain_runtime_wait_bridge_notifications();
    bool zero_timeout_pumped = false;
#if !defined(__APPLE__)
    // On macOS, native source dispatch (display-link, AppKit events) is
    // handled by CFRunLoopRunInMode in the uv_prepare callback. Other
    // platforms still need the Rust-side pump.
    if (supports_zero_timeout_pump_) {
      qt_solid_spike::qt::window_host_pump_zero_timeout();
      zero_timeout_pumped = true;
    }
#endif

    // Process Qt events. On macOS, CFRunLoop already fired native sources
    // during on_prepare; any resulting Qt events (paint, timer) are queued
    // and picked up here. Two iterations handle one level of cascading
    // posted events (e.g. paint triggers update triggers paint).
    const int iterations = 2;
    for (int index = 0; index < iterations; ++index) {
      QCoreApplication::sendPostedEvents(nullptr);
      QCoreApplication::processEvents(QEventLoop::AllEvents);
      if (shutdown_requested_.load()) {
        break;
      }
    }
    pumping_ = false;
    record_libuv_pump_events(zero_timeout_pumped);

#if defined(Q_OS_WIN)
    if (!shutdown_requested_.load()) {
      rearm_win32_wait_bridge();
    }
#endif
    reschedule_timer();
  }

  void request_pump(bool issue_external_wake = true) {
    if (!started_ || !async_initialized_) {
      return;
    }

    if (qEnvironmentVariableIsSet("QT_SOLID_WGPU_TRACE")) {
      std::fprintf(stdout, "[qt-uv-pump] request-pump external_wake=%d\n",
                   issue_external_wake ? 1 : 0);
      std::fflush(stdout);
    }
    record_libuv_request_pump();
    if (issue_external_wake && supports_external_wake_) {
      qt_solid_spike::qt::window_host_request_wake();
    }

    const int status = uv_async_send(&async_);
    if (status != 0) {
      throw_uv_error("uv_async_send", status);
    }
  }

private:
  PumpDriverMode select_driver_mode() const {
    if (wait_bridge_available()) {
      return PumpDriverMode::WaitBridge;
    }
    if (supports_external_wake_) {
      return PumpDriverMode::ExternalWake;
    }
    return PumpDriverMode::PollingFallback;
  }

  bool wait_bridge_available() const {
    switch (wait_bridge_kind_) {
    case WaitBridgeKind::None:
      return false;
    case WaitBridgeKind::UnixFd:
      return wait_bridge_unix_fd_ >= 0;
    case WaitBridgeKind::WindowsHandle:
      return wait_bridge_windows_handle_ != 0;
    }
  }

  std::uint64_t idle_poll_delay_ms() const {
    if (aggressive_poll_) {
      return 1;
    }

    switch (driver_mode_) {
    case PumpDriverMode::PollingFallback:
      return polling_fallback_idle_ms_;
    case PumpDriverMode::ExternalWake:
      return external_wake_backstop_ms_;
    case PumpDriverMode::WaitBridge:
      return 16;
    }
  }

  void execute_deferred_shutdown() {
    if (timer_initialized_) {
      uv_timer_stop(&timer_);
      set_timer_referenced(false);
    }
    if (prepare_initialized_) {
      uv_prepare_stop(&prepare_);
    }
    if (wait_bridge_poll_initialized_) {
      uv_poll_stop(&wait_bridge_poll_);
    }
#if defined(__APPLE__)
    teardown_cfrunloop_libuv_source();
#elif defined(Q_OS_WIN)
    teardown_win32_wait_bridge();
#endif

    if (on_shutdown_) {
      on_shutdown_();
      on_shutdown_ = nullptr;
    }
  }

  template <typename Handle>
  void close_handle(Handle *handle, bool initialized) {
    if (!initialized) {
      return;
    }

    auto *raw = reinterpret_cast<uv_handle_t *>(handle);
    if (uv_is_closing(raw)) {
      return;
    }

    ++close_pending_count_;
    uv_close(raw, &LibuvQtPump::on_close);
  }

  static void on_async(uv_async_t *handle) {
    auto *self = static_cast<LibuvQtPump *>(handle->data);
    if (self->shutdown_requested_.load()) {
      self->execute_deferred_shutdown();
      return;
    }
    self->pump_events();
  }

  static void on_prepare(uv_prepare_t *handle) {
    auto *self = static_cast<LibuvQtPump *>(handle->data);
    if (self->shutdown_requested_.load()) {
      return;
    }
#if defined(__APPLE__)
    if (self->driver_mode_ == PumpDriverMode::WaitBridge) {
      // Run CFRunLoop before libuv's kevent poll. This lets native macOS
      // sources (CAMetalDisplayLink, AppKit event ports) fire using the
      // time libuv would otherwise spend blocked in kevent.
      //
      // uv_backend_timeout tells us how long libuv would block. We give
      // that budget to CFRunLoop instead. After CFRunLoop returns, libuv
      // proceeds to its own kevent poll. External wakeups (display-link
      // via request_qt_pump, wait-bridge pipe, backstop timer) ensure
      // libuv wakes promptly when there is real work.
      const int timeout_ms = uv_backend_timeout(self->loop_);
      const double timeout_sec = timeout_ms < 0 ? 0.05 : timeout_ms / 1000.0;
      CFRunLoopRunInMode(kCFRunLoopDefaultMode, timeout_sec, false);
      return;
    }
#elif defined(Q_OS_WIN)
    if (self->driver_mode_ == PumpDriverMode::WaitBridge) {
      // Zero-timeout peek: detect pending Win32 messages before libuv
      // enters IOCP. If messages are queued, pump immediately so Qt
      // can process them without waiting for the backstop timer.
      MSG msg;
      if (PeekMessageW(&msg, nullptr, 0, 0, PM_NOREMOVE)) {
        self->pump_events();
      }
      return;
    }
#endif
    self->pump_events();
  }

  static void on_timer(uv_timer_t *handle) {
    auto *self = static_cast<LibuvQtPump *>(handle->data);
    self->set_timer_referenced(false);
    if (self->shutdown_requested_.load()) {
      return;
    }
    self->pump_events();
  }

  static void on_wait_bridge_poll(uv_poll_t *handle, int status, int events) {
    auto *self = static_cast<LibuvQtPump *>(handle->data);
    if (self == nullptr || status < 0 || (events & UV_READABLE) == 0) {
      return;
    }
    if (self->shutdown_requested_.load()) {
      return;
    }
    self->pump_events();
  }

  static void on_close(uv_handle_t *handle) {
    auto *self = static_cast<LibuvQtPump *>(handle->data);
    if (handle == reinterpret_cast<uv_handle_t *>(&self->async_)) {
      self->async_initialized_ = false;
    } else if (handle == reinterpret_cast<uv_handle_t *>(&self->prepare_)) {
      self->prepare_initialized_ = false;
    } else if (handle == reinterpret_cast<uv_handle_t *>(&self->timer_)) {
      self->timer_initialized_ = false;
    } else if (handle ==
               reinterpret_cast<uv_handle_t *>(&self->wait_bridge_poll_)) {
      self->wait_bridge_poll_initialized_ = false;
    }

    if (self->close_pending_count_ == 0) {
      return;
    }

    --self->close_pending_count_;
    if (self->close_pending_count_ == 0) {
      delete self;
    }
  }

  void set_timer_referenced(bool referenced) {
    if (timer_referenced_ == referenced) {
      return;
    }

    if (referenced) {
      uv_ref(reinterpret_cast<uv_handle_t *>(&timer_));
    } else {
      uv_unref(reinterpret_cast<uv_handle_t *>(&timer_));
    }

    timer_referenced_ = referenced;
  }

  void reschedule_timer() {
    if (!started_) {
      return;
    }

    std::optional<std::uint64_t> delay_ms;
#if defined(__APPLE__)
    // On macOS, CFRunLoopRunInMode in on_prepare handles native timer
    // dispatch. The libuv timer is a backstop to ensure periodic pumps.
    delay_ms = idle_poll_delay_ms();
#else
    if (driver_mode_ == PumpDriverMode::WaitBridge) {
      delay_ms = current_runtime_wait_bridge_timer_delay_ms();
      if (!delay_ms.has_value()) {
        delay_ms = idle_poll_delay_ms();
      }
    } else {
      delay_ms = idle_poll_delay_ms();
    }
#endif

    if (!delay_ms.has_value()) {
      if (timer_initialized_) {
        uv_timer_stop(&timer_);
      }
      set_timer_referenced(false);
      return;
    }

    const int status =
        uv_timer_start(&timer_, &LibuvQtPump::on_timer, *delay_ms, 0);
    if (status != 0) {
      throw_uv_error("uv_timer_start", status);
    }
    set_timer_referenced(true);
  }

#if defined(__APPLE__)
  void setup_cfrunloop_libuv_source() {
    const int backend_fd = uv_backend_fd(loop_);
    if (backend_fd < 0) {
      return;
    }

    CFFileDescriptorContext ctx{};
    ctx.info = this;
    cf_fd_ = CFFileDescriptorCreate(
        kCFAllocatorDefault, backend_fd, false,
        &LibuvQtPump::cf_fd_callback, &ctx);
    if (cf_fd_ == nullptr) {
      return;
    }
    CFFileDescriptorEnableCallBacks(cf_fd_, kCFFileDescriptorReadCallBack);
    cf_fd_source_ = CFFileDescriptorCreateRunLoopSource(
        kCFAllocatorDefault, cf_fd_, 0);
    if (cf_fd_source_ == nullptr) {
      CFRelease(cf_fd_);
      cf_fd_ = nullptr;
      return;
    }
    CFRunLoopAddSource(CFRunLoopGetMain(), cf_fd_source_,
                       kCFRunLoopDefaultMode);
  }

  void teardown_cfrunloop_libuv_source() {
    if (cf_fd_source_ != nullptr) {
      CFRunLoopRemoveSource(CFRunLoopGetMain(), cf_fd_source_,
                            kCFRunLoopDefaultMode);
      CFRelease(cf_fd_source_);
      cf_fd_source_ = nullptr;
    }
    if (cf_fd_ != nullptr) {
      CFFileDescriptorInvalidate(cf_fd_);
      CFRelease(cf_fd_);
      cf_fd_ = nullptr;
    }
  }

  /// Re-arm after each callback (CFFileDescriptor is one-shot).
  void rearm_cfrunloop_libuv_source() {
    if (cf_fd_ != nullptr) {
      CFFileDescriptorEnableCallBacks(cf_fd_, kCFFileDescriptorReadCallBack);
    }
  }

  static void cf_fd_callback(CFFileDescriptorRef /* fdref */,
                             CFOptionFlags /* callback_types */,
                             void *info) {
    auto *self = static_cast<LibuvQtPump *>(info);
    // Re-arm so we stay registered. The actual libuv→CFRunLoop
    // integration is handled by on_prepare's CFRunLoopRunInMode;
    // we must NOT call uv_async_send here — it would make the
    // kevent fd immediately readable again, causing a busy spin.
    self->rearm_cfrunloop_libuv_source();
  }
#endif  // __APPLE__

#if defined(Q_OS_WIN)
  void setup_win32_wait_bridge() {
    if (wait_bridge_kind_ != WaitBridgeKind::WindowsHandle ||
        wait_bridge_windows_handle_ == 0) {
      return;
    }

    auto *handle = reinterpret_cast<HANDLE>(wait_bridge_windows_handle_);
    BOOL ok = RegisterWaitForSingleObject(
        &win32_wait_handle_,
        handle,
        &LibuvQtPump::on_wait_bridge_win32_event,
        this,
        INFINITE,
        WT_EXECUTEONLYONCE);
    if (!ok) {
      // Non-fatal: fall back to polling. Driver mode stays WaitBridge
      // but the event-driven wake won't fire; backstop timer covers it.
      return;
    }
    win32_wait_registered_ = true;
  }

  void rearm_win32_wait_bridge() {
    if (!win32_wait_registered_ || wait_bridge_windows_handle_ == 0) {
      return;
    }
    if (shutdown_requested_.load()) {
      return;
    }
    // Unregister previous one-shot wait, then re-register.
    // INVALID_HANDLE_VALUE = block until any in-flight callback completes.
    UnregisterWaitEx(win32_wait_handle_, INVALID_HANDLE_VALUE);
    win32_wait_handle_ = nullptr;

    auto *handle = reinterpret_cast<HANDLE>(wait_bridge_windows_handle_);
    BOOL ok = RegisterWaitForSingleObject(
        &win32_wait_handle_,
        handle,
        &LibuvQtPump::on_wait_bridge_win32_event,
        this,
        INFINITE,
        WT_EXECUTEONLYONCE);
    if (!ok) {
      win32_wait_registered_ = false;
    }
  }

  void teardown_win32_wait_bridge() {
    if (!win32_wait_registered_) {
      return;
    }
    // INVALID_HANDLE_VALUE = wait for callback to complete before returning.
    UnregisterWaitEx(win32_wait_handle_, INVALID_HANDLE_VALUE);
    win32_wait_registered_ = false;
    win32_wait_handle_ = nullptr;
  }

  static void CALLBACK on_wait_bridge_win32_event(
      PVOID context, BOOLEAN /* timed_out */) {
    auto *self = static_cast<LibuvQtPump *>(context);
    if (self->shutdown_requested_.load()) {
      return;
    }
    // Wake libuv from IOCP sleep. The actual message processing
    // happens in on_prepare (PeekMessage) or pump_events.
    // This is a one-shot wait; rearm happens after pump_events().
    if (self->async_initialized_) {
      uv_async_send(&self->async_);
    }
  }
#endif  // Q_OS_WIN

  uv_loop_t *loop_ = nullptr;
  const std::uint64_t polling_fallback_idle_ms_ = 8;
  const std::uint64_t external_wake_backstop_ms_ = 64;
  uv_async_t async_{};
  uv_prepare_t prepare_{};
  uv_timer_t timer_{};
  uv_poll_t wait_bridge_poll_{};
#if defined(__APPLE__)
  CFFileDescriptorRef cf_fd_ = nullptr;
  CFRunLoopSourceRef cf_fd_source_ = nullptr;
#endif
#if defined(Q_OS_WIN)
  HANDLE win32_wait_handle_ = nullptr;
  bool win32_wait_registered_ = false;
#endif
  bool started_ = false;
  bool pumping_ = false;
  std::atomic<bool> shutdown_requested_{false};
  std::function<void()> on_shutdown_;
  bool timer_referenced_ = true;
  bool aggressive_poll_ = false;
  bool async_initialized_ = false;
  bool prepare_initialized_ = false;
  bool timer_initialized_ = false;
  bool wait_bridge_poll_initialized_ = false;
  bool closing_ = false;
  std::size_t close_pending_count_ = 0;
  bool supports_zero_timeout_pump_ = false;
  bool supports_external_wake_ = false;
  WaitBridgeKind wait_bridge_kind_ = WaitBridgeKind::None;
  PumpDriverMode driver_mode_ = PumpDriverMode::PollingFallback;
  int wait_bridge_unix_fd_ = -1;
  std::uint64_t wait_bridge_windows_handle_ = 0;
};
