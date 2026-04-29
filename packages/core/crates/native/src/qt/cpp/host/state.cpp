class QtHostState {
public:
  explicit QtHostState(uv_loop_t *loop) : loop_(loop) {}

  bool started() const { return started_; }

  void start() {
    if (started_) {
      return;
    }

    qt_wgpu_renderer::register_static_platform_plugins();
    qt_wgpu_renderer::configure_unified_compositor_platform();

    argv_storage_ = "qt-solid-spike";
    argv_[0] = argv_storage_.data();
    argv_[1] = nullptr;

    if (!app_) {
      if (QCoreApplication::instance() != nullptr) {
        throw_error(
            "QCoreApplication already exists before qt-solid host startup");
      }

      app_ = std::make_unique<QApplication>(argc_, argv_);
      app_->setApplicationName(QStringLiteral("qt-solid-spike"));
      app_->setQuitOnLastWindowClosed(false);
      qt_wgpu_renderer::sync_unified_compositor_active_state();
      QObject::connect(app_.get(), &QGuiApplication::applicationStateChanged,
                       app_.get(), [](Qt::ApplicationState state) {
                         if (state == Qt::ApplicationActive) {
                           qt_solid_spike::qt::emit_app_event(
                               ::rust::Str("activate"));
                         }
                       });
#if defined(__APPLE__)
      wait_bridge_ = std::make_unique<MacosEventBufferBridge>();
      const auto dispatcher_probe = probe_cocoa_dispatcher_private_prefix();
      if (dispatcher_probe.dispatcher_private == nullptr) {
        throw_error(dispatcher_probe.error_message);
      }
#endif
    } else {
      throw_error("Qt host cannot restart in the same process yet");
    }

    try {
      pump_ = std::make_unique<LibuvQtPump>(loop_);
      pump_->start();
      pump_->pump_events();
      registry_.install_motion_mouse_filter(app_.get());
      started_ = true;
    } catch (...) {
      if (pump_) {
        if (pump_->close_async()) {
          pump_.release();
        } else {
          pump_.reset();
        }
      }
#if defined(__APPLE__)
      wait_bridge_.reset();
#endif
      app_.reset();
      throw;
    }
  }

  void request_pump() {
    if (!started_ || !pump_) {
      return;
    }

    pump_->request_pump();
  }
  void shutdown() {
    if (!started_) {
      return;
    }

    started_ = false;

    if (pump_) {
      pump_->request_shutdown([this]() {
        execute_teardown();
      });
    } else {
      execute_teardown();
    }
  }

  QtRegistry &registry() {
    if (!started_) {
      throw_error("Qt host is not started");
    }
    return registry_;
  }

  int runtime_wait_bridge_unix_fd() const noexcept {
#if defined(__APPLE__)
    if (wait_bridge_) {
      return wait_bridge_->read_fd();
    }
#endif
    return -1;
  }

  void drain_runtime_wait_bridge() noexcept {
#if defined(__APPLE__)
    if (wait_bridge_) {
      wait_bridge_->drain();
    }
#endif
  }

  std::optional<std::uint64_t> runtime_wait_bridge_timer_delay_ms()
      noexcept {
#if defined(__APPLE__)
    if (!app_) {
      return std::nullopt;
    }

    const auto dispatcher_probe = probe_cocoa_dispatcher_private_prefix();
    if (dispatcher_probe.dispatcher_private == nullptr) {
      return std::nullopt;
    }

    if (auto delay = dispatcher_probe.dispatcher_private->timerInfoList.timerWait()) {
      const auto delay_ms = std::chrono::duration_cast<std::chrono::milliseconds>(
          *delay);
      return delay_ms.count() > 0
                 ? static_cast<std::uint64_t>(delay_ms.count())
                 : std::uint64_t{0};
    }

    return std::nullopt;
#else
    return std::nullopt;
#endif
  }

private:
  void execute_teardown() {
#if defined(__APPLE__)
    wait_bridge_.reset();
#endif
    registry_.clear();
    if (QCoreApplication::instance() != nullptr) {
      for (int index = 0; index < 4; ++index) {
        QCoreApplication::sendPostedEvents(nullptr, QEvent::DeferredDelete);
        QCoreApplication::processEvents(QEventLoop::AllEvents);
        QCoreApplication::sendPostedEvents(nullptr);
      }
    }
    app_.reset();
  }

  uv_loop_t *loop_ = nullptr;
  int argc_ = 1;
  std::string argv_storage_;
  char *argv_[2] = {nullptr, nullptr};
  std::unique_ptr<QApplication> app_;
  std::unique_ptr<LibuvQtPump> pump_;
#if defined(__APPLE__)
  std::unique_ptr<MacosEventBufferBridge> wait_bridge_;
#endif
  QtRegistry registry_;
  bool started_ = false;
};

QtHostState *g_host = nullptr;

void request_qt_pump() {
  if (!g_host || !g_host->started()) {
    return;
  }

  record_request_qt_pump();
  g_host->request_pump();
}
int current_runtime_wait_bridge_unix_fd() noexcept {
  if (!g_host) {
    return -1;
  }

  return g_host->runtime_wait_bridge_unix_fd();
}

void drain_runtime_wait_bridge_notifications() noexcept {
  if (!g_host) {
    return;
  }

  g_host->drain_runtime_wait_bridge();
}

#if !defined(__APPLE__)
std::optional<std::uint64_t> current_runtime_wait_bridge_timer_delay_ms()
    noexcept {
  if (!g_host) {
    return std::nullopt;
  }

  return g_host->runtime_wait_bridge_timer_delay_ms();
}
#endif
