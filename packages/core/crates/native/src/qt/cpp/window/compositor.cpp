bool HostWindowWidget::request_compositor_frame() {
  if (render_suppressed()) {
    return false;
  }
  if (!qt_wgpu_renderer::unified_compositor_active()) {
    return false;
  }
  if (windowHandle() == nullptr) {
    return false;
  }
  const bool compositor_already_active =
      driving_compositor_frame_ || compositor_frame_requested_
#if defined(Q_OS_MACOS)
      || compositor_display_link_running_
#endif
      ;
#if defined(Q_OS_MACOS) || defined(Q_OS_WIN)
  // macOS / Windows: always consider frame ready. The wgpu compositor
  // initializes the surface lazily on first drive_compositor_frame.
  const bool frame_ready = true;
#else
  const bool frame_ready = qt_wgpu_renderer::unified_compositor_window_frame_ready(
      windowHandle(), capture_device_pixel_ratio());
#endif
  if (!frame_ready && !compositor_already_active) {
    return false;
  }
  autonomous_repaint_timer_.stop();
  record_compositor_frame_request();
#if defined(Q_OS_MACOS)
  const bool requested = qt_wgpu_renderer::unified_compositor_window_request_frame(
      windowHandle(), capture_device_pixel_ratio());
  qt_solid_wgpu_trace("request-frame node=%u requested=%d active=%d visible=%d",
                      rust_node_id_, requested ? 1 : 0,
                      compositor_already_active ? 1 : 0, isVisible() ? 1 : 0);
  if (requested || compositor_already_active) {
    start_compositor_display_link();
    request_qt_pump();
  }
#else
  const bool requested = qt_wgpu_renderer::unified_compositor_window_request_frame(
      windowHandle(), capture_device_pixel_ratio());
  qt_solid_wgpu_trace("request-frame node=%u requested=%d active=%d visible=%d",
                      rust_node_id_, requested ? 1 : 0,
                      compositor_already_active ? 1 : 0, isVisible() ? 1 : 0);
  compositor_frame_requested_ = true;
  if (frame_ready || compositor_already_active) {
    post_compositor_frame_drive();
  }
#endif
  return true;
}

void HostWindowWidget::notify_compositor_frame_complete() {
#if defined(Q_OS_MACOS)
  if (windowHandle() == nullptr) {
    return;
  }
  if (qt_wgpu_renderer::unified_compositor_window_display_link_should_run(
          windowHandle(), capture_device_pixel_ratio())) {
    start_compositor_display_link();
    if (compositor_display_link_running_) {
      request_qt_pump();
    }
  } else {
    stop_compositor_display_link();
  }
#else
  if (windowHandle() == nullptr) {
    return;
  }
  if (qt_wgpu_renderer::unified_compositor_window_display_link_should_run(
          windowHandle(), capture_device_pixel_ratio())) {
    post_compositor_frame_drive();
  }
#endif
}

void HostWindowWidget::drive_compositor_frame_from_signal() {
  if (!isVisible() || driving_compositor_frame_ || rust_node_id_ == 0 ||
      windowHandle() == nullptr) {
    return;
  }
#if !defined(Q_OS_MACOS)
  compositor_frame_requested_ = true;
#endif
  drive_compositor_frame();
}

void HostWindowWidget::post_compositor_frame_drive() {
  if (render_suppressed()) {
    return;
  }
  if (compositor_drive_posted_ || driving_compositor_frame_ || !isVisible() ||
      rust_node_id_ == 0 || windowHandle() == nullptr) {
    return;
  }

  compositor_drive_posted_ = true;
  record_compositor_frame_post();
  QPointer<HostWindowWidget> deferred_host(this);
  QTimer::singleShot(0, this, [deferred_host]() {
    if (deferred_host == nullptr) {
      return;
    }
    deferred_host->compositor_drive_posted_ = false;
    deferred_host->drive_compositor_frame();
  });
}

void HostWindowWidget::drive_compositor_frame() {
  if (render_suppressed()) {
#if !defined(Q_OS_MACOS)
    compositor_frame_requested_ = false;
#endif
    return;
  }
  if (rust_node_id_ == 0 || windowHandle() == nullptr) {
#if !defined(Q_OS_MACOS)
    compositor_frame_requested_ = false;
#endif
    return;
  }
#if !defined(Q_OS_MACOS)
  // Window not yet exposed — DWM has not acknowledged the surface yet, so
  // DX12 swap chain creation will fail. Defer; the PlatformSurface/Show event
  // handler will re-trigger request_compositor_frame() when ready.
  if (!windowHandle()->isExposed()) {
    post_compositor_frame_drive();
    return;
  }
#endif
  if (!isVisible()) {
    // Window not yet visible — defer until next event loop turn.
    // This happens when WinIdChange/PlatformSurface fires before show completes.
    compositor_drive_posted_ = false;
    post_compositor_frame_drive();
    return;
  }
#if !defined(Q_OS_MACOS)
  if (!compositor_frame_requested_) {
    return;
  }
  compositor_frame_requested_ = false;
#endif
  driving_compositor_frame_ = true;
  const auto status = qt_wgpu_renderer::drive_unified_compositor_window_frame(
      windowHandle(), rust_node_id_, capture_device_pixel_ratio());
  driving_compositor_frame_ = false;
  record_compositor_frame_status(status);

  switch (status) {
  case qt_wgpu_renderer::UnifiedCompositorDriveStatus::Presented:
    break;
  case qt_wgpu_renderer::UnifiedCompositorDriveStatus::Busy:
#if defined(Q_OS_MACOS)
    if (windowHandle() != nullptr) {
      const bool should_continue =
          qt_wgpu_renderer::unified_compositor_window_request_frame(
              windowHandle(), capture_device_pixel_ratio());
      if (should_continue) {
        start_compositor_display_link();
      }
    }
#else
    if (windowHandle() != nullptr) {
      qt_wgpu_renderer::unified_compositor_window_request_frame(
          windowHandle(), capture_device_pixel_ratio());
    }
    compositor_frame_requested_ = true;
#endif
    break;
  case qt_wgpu_renderer::UnifiedCompositorDriveStatus::Idle:
#if defined(Q_OS_MACOS)
    if (!qt_wgpu_renderer::unified_compositor_window_display_link_should_run(
            windowHandle(), capture_device_pixel_ratio())) {
      stop_compositor_display_link();
    }
#else
    compositor_frame_requested_ = false;
    if (isVisible() && !autonomous_repaint_timer_.isActive()) {
      autonomous_repaint_timer_.start();
    }
#endif
    break;
  case qt_wgpu_renderer::UnifiedCompositorDriveStatus::NeedsQtRepaint:
#if defined(Q_OS_MACOS)
    stop_compositor_display_link();
#else
    compositor_frame_requested_ = false;
    if (isVisible() && !autonomous_repaint_timer_.isActive()) {
      autonomous_repaint_timer_.start();
    }
#endif
    update();
    break;
  }
}

#if defined(Q_OS_MACOS)
void HostWindowWidget::compositor_display_link_callback(void *context, void *drawable) {
  auto *host = static_cast<HostWindowWidget *>(context);
  if (host == nullptr) {
    if (drawable != nullptr) {
      qt_wgpu_renderer::release_unified_compositor_metal_drawable(
          reinterpret_cast<std::uint64_t>(drawable));
    }
    return;
  }
  if (host->thread() == QThread::currentThread()) {
    qt_solid_wgpu_trace("display-link-callback direct node=%u drawable=%p",
                        host->rust_node_id_, drawable);
    host->post_compositor_frame_drive_from_display_link(drawable);
    return;
  }

  qt_solid_wgpu_trace("display-link-callback queued node=%u drawable=%p host_thread=%p current_thread=%p",
                      host->rust_node_id_, drawable,
                      static_cast<void *>(host->thread()),
                      static_cast<void *>(QThread::currentThread()));

  QPointer<HostWindowWidget> deferred_host(host);
  const bool invoked = QMetaObject::invokeMethod(
      host,
      [deferred_host, drawable]() {
        if (deferred_host == nullptr) {
          if (drawable != nullptr) {
            qt_wgpu_renderer::release_unified_compositor_metal_drawable(
                reinterpret_cast<std::uint64_t>(drawable));
          }
          return;
        }
        deferred_host->post_compositor_frame_drive_from_display_link(drawable);
      },
      Qt::QueuedConnection);
  if (!invoked && drawable != nullptr) {
    qt_wgpu_renderer::release_unified_compositor_metal_drawable(
        reinterpret_cast<std::uint64_t>(drawable));
    return;
  }
  request_qt_pump();
}

void HostWindowWidget::start_compositor_display_link() {
  if (compositor_display_link_handle_ != nullptr && compositor_display_link_running_) {
    qt_solid_wgpu_trace("start-link skip-running node=%u", rust_node_id_);
    return;
  }

  if (compositor_display_link_handle_ == nullptr) {
    if (windowHandle() == nullptr) {
      qt_solid_wgpu_trace("start-link no-window node=%u", rust_node_id_);
      post_compositor_frame_drive();
      return;
    }
    void *metal_layer = qt_wgpu_renderer::unified_compositor_window_metal_layer(
        windowHandle(), capture_device_pixel_ratio());
    if (metal_layer == nullptr) {
      qt_solid_wgpu_trace("start-link no-layer node=%u", rust_node_id_);
      post_compositor_frame_drive();
      return;
    }
    compositor_display_link_handle_ = ::qt_macos_display_link_create(
        metal_layer, this,
        &HostWindowWidget::compositor_display_link_callback,
        ::qt_solid_native_frame_notifier());
    if (compositor_display_link_handle_ == nullptr) {
      qt_solid_wgpu_trace("start-link create-failed node=%u", rust_node_id_);
      post_compositor_frame_drive();
      return;
    }
  }

  if (::qt_macos_display_link_start(compositor_display_link_handle_)) {
    compositor_display_link_running_ = true;
    qt_solid_wgpu_trace("start-link ok node=%u", rust_node_id_);
  } else {
    qt_solid_wgpu_trace("start-link start-failed node=%u", rust_node_id_);
    post_compositor_frame_drive();
  }
}

void HostWindowWidget::stop_compositor_display_link() {
  compositor_display_link_tick_posted_.store(false);
  if (compositor_display_link_handle_ == nullptr) {
    compositor_display_link_running_ = false;
    return;
  }
  qt_solid_wgpu_trace("stop-link node=%u", rust_node_id_);
  ::qt_macos_display_link_stop(compositor_display_link_handle_);
  compositor_display_link_running_ = false;
}

void HostWindowWidget::shutdown_compositor_display_link() {
  stop_compositor_display_link();
  if (compositor_display_link_handle_ != nullptr) {
    qt_solid_wgpu_trace("shutdown-link node=%u", rust_node_id_);
    ::qt_macos_display_link_destroy(compositor_display_link_handle_);
    compositor_display_link_handle_ = nullptr;
  }
  if (windowHandle() != nullptr) {
    qt_wgpu_renderer::destroy_unified_compositor_window(
        windowHandle(), capture_device_pixel_ratio());
  }
}

void HostWindowWidget::post_compositor_frame_drive_from_display_link(void *drawable) {
  if (compositor_display_link_tick_posted_.exchange(true)) {
    if (drawable != nullptr) {
      qt_wgpu_renderer::release_unified_compositor_metal_drawable(
          reinterpret_cast<std::uint64_t>(drawable));
    }
    return;
  }

  compositor_display_link_tick_posted_.store(false);
  handle_compositor_display_link_tick(drawable);
}

void HostWindowWidget::handle_compositor_display_link_tick(void *drawable) {
  if (!isVisible()) {
    if (drawable != nullptr) {
      qt_wgpu_renderer::release_unified_compositor_metal_drawable(
          reinterpret_cast<std::uint64_t>(drawable));
    }
    stop_compositor_display_link();
    return;
  }
  if (driving_compositor_frame_) {
    if (drawable != nullptr) {
      qt_wgpu_renderer::release_unified_compositor_metal_drawable(
          reinterpret_cast<std::uint64_t>(drawable));
    }
    return;
  }
  if (windowHandle() == nullptr) {
    if (drawable != nullptr) {
      qt_wgpu_renderer::release_unified_compositor_metal_drawable(
          reinterpret_cast<std::uint64_t>(drawable));
    }
    stop_compositor_display_link();
    return;
  }
  if (drawable == nullptr) {
    return;
  }
  driving_compositor_frame_ = true;
  const auto status =
      qt_wgpu_renderer::drive_unified_compositor_window_frame_from_display_link(
          windowHandle(), rust_node_id_, capture_device_pixel_ratio(),
          reinterpret_cast<std::uint64_t>(drawable));
  qt_solid_wgpu_trace("tick node=%u status=%d", rust_node_id_,
                      static_cast<int>(status));
  driving_compositor_frame_ = false;
  record_compositor_frame_status(status);

  switch (status) {
  case qt_wgpu_renderer::UnifiedCompositorDriveStatus::Presented:
    if (qt_wgpu_renderer::unified_compositor_window_display_link_should_run(
            windowHandle(), capture_device_pixel_ratio())) {
      request_qt_pump();
    } else {
      stop_compositor_display_link();
    }
    break;
  case qt_wgpu_renderer::UnifiedCompositorDriveStatus::Busy:
    start_compositor_display_link();
    break;
  case qt_wgpu_renderer::UnifiedCompositorDriveStatus::Idle:
    stop_compositor_display_link();
    break;
  case qt_wgpu_renderer::UnifiedCompositorDriveStatus::NeedsQtRepaint:
    stop_compositor_display_link();
    update();
    break;
  }
}

void HostWindowWidget::set_display_link_frame_rate(float fps) {
  if (compositor_display_link_handle_ == nullptr) {
    return;
  }
  ::qt_macos_display_link_set_preferred_fps(compositor_display_link_handle_, fps);
}
#endif
