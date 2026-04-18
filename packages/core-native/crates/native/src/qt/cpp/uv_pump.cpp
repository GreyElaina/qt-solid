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
  }

  ~LibuvQtPump() = default;

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

    if (driver_mode_ == PumpDriverMode::PollingFallback) {
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

  void pump_events() {
    if (pumping_ || !started_ || QCoreApplication::instance() == nullptr) {
      return;
    }

    pumping_ = true;
    drain_runtime_wait_bridge_notifications();
    if (supports_zero_timeout_pump_) {
      qt_solid_spike::qt::window_host_pump_zero_timeout();
    }

    for (int index = 0; index < 8; ++index) {
      QCoreApplication::sendPostedEvents(nullptr);
      QCoreApplication::processEvents(QEventLoop::AllEvents);
    }
    pumping_ = false;

    reschedule_timer();
  }

  void request_pump() {
    if (!started_ || !async_initialized_) {
      return;
    }

    if (supports_external_wake_) {
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
    static_cast<LibuvQtPump *>(handle->data)->pump_events();
  }

  static void on_prepare(uv_prepare_t *handle) {
    static_cast<LibuvQtPump *>(handle->data)->pump_events();
  }

  static void on_timer(uv_timer_t *handle) {
    auto *self = static_cast<LibuvQtPump *>(handle->data);
    self->set_timer_referenced(false);
    self->pump_events();
  }

  static void on_wait_bridge_poll(uv_poll_t *handle, int status, int events) {
    auto *self = static_cast<LibuvQtPump *>(handle->data);
    if (self == nullptr || status < 0 || (events & UV_READABLE) == 0) {
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
    if (driver_mode_ == PumpDriverMode::WaitBridge) {
      delay_ms = current_runtime_wait_bridge_timer_delay_ms();
    } else {
      delay_ms = idle_poll_delay_ms();
    }
#else
    delay_ms = idle_poll_delay_ms();
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

  uv_loop_t *loop_ = nullptr;
  const std::uint64_t polling_fallback_idle_ms_ = 8;
  const std::uint64_t external_wake_backstop_ms_ = 64;
  uv_async_t async_{};
  uv_prepare_t prepare_{};
  uv_timer_t timer_{};
  uv_poll_t wait_bridge_poll_{};
  bool started_ = false;
  bool pumping_ = false;
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
