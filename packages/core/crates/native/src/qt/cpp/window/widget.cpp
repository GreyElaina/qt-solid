#if defined(Q_OS_MACOS)
// Popup outside-click monitor (implemented in platform/popup_monitor.mm)
extern "C" void qt_popup_install_outside_click_monitor(
    std::uint32_t node_id,
    void *ns_view_ptr,
    void (*callback)(std::uint32_t));
extern "C" void qt_popup_remove_outside_click_monitor(std::uint32_t node_id);
static void popup_outside_click_callback(std::uint32_t node_id);
#endif

static QWidget *deepest_child_at(QWidget *root, const QPoint &point_in_root) {
  if (root == nullptr || !root->rect().contains(point_in_root)) {
    return nullptr;
  }

  auto *child = root->childAt(point_in_root);
  if (child == nullptr) {
    return root;
  }

  const QPoint point_in_child = child->mapFrom(root, point_in_root);
  if (auto *deepest = deepest_child_at(child, point_in_child)) {
    return deepest;
  }
  return child;
}

class HostWindowWidget final : public QWidget, public RustWidgetBindingHost {
public:
  using CloseRequestedHandler = std::function<void()>;

  explicit HostWindowWidget(QWidget *parent = nullptr) : QWidget(parent) {
    setWindowFlags(Qt::Window);
    setMouseTracking(true);
    setFocusPolicy(Qt::StrongFocus);
    autonomous_repaint_timer_.setSingleShot(false);
    autonomous_repaint_timer_.setInterval(
        std::max(120, QApplication::cursorFlashTime() / 2));
    QObject::connect(&autonomous_repaint_timer_, &QTimer::timeout, this, [this]() {
      if (render_suppressed() || !isVisible()) {
        return;
      }

      QWidget *focus = QApplication::focusWidget();
      if (focus == nullptr || focus->window() != this) {
        return;
      }

#if !defined(Q_OS_MACOS)
      tick_frame();
#endif
      focus->update();
      update();
    });
  }

  ~HostWindowWidget() override {
    begin_teardown();
  }

  void add_close_requested_handler(CloseRequestedHandler handler) {
    close_requested_handlers_.push_back(std::move(handler));
  }

  void bind_rust_widget(std::uint32_t node_id, std::uint8_t kind_tag) override {
    rust_node_id_ = node_id;
    kind_tag_ = kind_tag;
    sync_window_metadata();
  }

  void set_frameless(bool value) {
    if (frameless_ == value) {
      return;
    }

    frameless_ = value;
    apply_window_flags();
  }

  bool frameless() const { return frameless_; }

  void set_transparent_background(bool value) {
    if (transparent_background_ == value) {
      return;
    }

    transparent_background_ = value;
    setAttribute(Qt::WA_TranslucentBackground, value);
    update();
  }

  bool transparent_background() const { return transparent_background_; }

  void set_always_on_top(bool value) {
    if (always_on_top_ == value) {
      return;
    }

    always_on_top_ = value;
    apply_window_flags();
  }

  bool always_on_top() const { return always_on_top_; }

  void set_window_kind(std::uint8_t value) {
    if (window_kind_ == value) {
      return;
    }

    window_kind_ = value;
    apply_window_flags();
  }

  std::uint8_t window_kind() const { return window_kind_; }

  std::uint8_t kind_tag() const { return kind_tag_; }

  void set_screen_position(int x, int y) {
    move(x, y);
  }

  std::uint32_t rust_node_id() const { return rust_node_id_; }

  TextEditSession &text_edit_session() { return text_edit_session_; }

  void mark_compositor_scene_dirty() {
    update();
    request_compositor_frame();
  }

  void mark_compositor_pixels_dirty() {
    update();
    request_compositor_frame();
  }

  bool compositor_host_visible_for_capture(const QWidget *widget) const {
    return isVisible() && widget != nullptr && widget->isVisible();
  }

  qreal capture_device_pixel_ratio() const {
    if (windowHandle() != nullptr) {
      return windowHandle()->devicePixelRatio();
    }
    return devicePixelRatioF();
  }

  QtNodeBounds presenter_bounds_for_widget(QWidget *widget) const {
    return bounds_for_widget(widget);
  }

  void tick_frame() {
    if (render_suppressed()) {
      return;
    }

    try {
      qt_solid_spike::qt::qt_window_frame_tick(rust_node_id_);
    } catch (const rust::Error &error) {
      qWarning() << "failed to tick host window frame for node" << rust_node_id_
                 << ":" << error.what();
    }
  }

  bool render_suppressed() const {
    return tearing_down_ || close_pending_ || rust_node_id_ == 0;
  }

  void begin_teardown() {
    if (tearing_down_) {
      return;
    }
    tearing_down_ = true;
    close_pending_ = false;
    autonomous_repaint_timer_.stop();
    compositor_frame_requested_ = false;
    compositor_drive_posted_ = false;
    cpu_frame_ = QImage();
    rust_node_id_ = 0;
#if defined(Q_OS_MACOS)
    shutdown_compositor_display_link();
#else
    destroy_compositor_window_once();
#endif
  }

  void destroy_compositor_window_once() {
#if !defined(Q_OS_MACOS)
    if (compositor_window_destroyed_) {
      return;
    }
    if (windowHandle() == nullptr) {
      return;
    }
    compositor_window_destroyed_ = true;
    qt_wgpu_renderer::destroy_unified_compositor_window(
        windowHandle(), capture_device_pixel_ratio());
#endif
  }

  void present_cpu_frame(const unsigned char *data, int width, int height, int stride) {
    if (render_suppressed()) {
      return;
    }
    cpu_frame_ = QImage(data, width, height, stride, QImage::Format_RGBA8888_Premultiplied).copy();
    update();
  }

  QWidget *widget_at_screen_point(const QPoint &screen_pos) const {
    if (!isVisible()) {
      return nullptr;
    }

    const QPoint local = mapFromGlobal(screen_pos);
    return deepest_child_at(const_cast<HostWindowWidget *>(this), local);
  }

  // -- Compositor methods (out-of-line in compositor.cpp) --------------------
  bool request_compositor_frame();
  void notify_compositor_frame_complete();
  void drive_compositor_frame_from_signal();
  void post_compositor_frame_drive();
  void drive_compositor_frame();
#if defined(Q_OS_MACOS)
  static void compositor_display_link_callback(void *context);
  void start_compositor_display_link();
  void stop_compositor_display_link();
  void shutdown_compositor_display_link();
  void set_display_link_frame_rate(float fps);
#endif

  // -- Input methods (out-of-line in input.cpp) ------------------------------
  void forward_key_event(QKeyEvent *event, std::uint8_t event_tag);

protected:
  void paintEvent(QPaintEvent *event) override {
#if !defined(Q_OS_MACOS)
    if (!render_suppressed()) {
      tick_frame();
    }
#endif
    if (!cpu_frame_.isNull()) {
      QPainter painter(this);
      if (testAttribute(Qt::WA_TranslucentBackground)) {
        painter.setCompositionMode(QPainter::CompositionMode_Source);
        painter.fillRect(rect(), Qt::transparent);
        painter.setCompositionMode(QPainter::CompositionMode_SourceOver);
      }
      painter.setRenderHint(QPainter::SmoothPixmapTransform);
      painter.drawImage(rect(), cpu_frame_);
    }
    QWidget::paintEvent(event);
  }

  void closeEvent(QCloseEvent *event) override {
    if (close_requested_handlers_.empty()) {
      QWidget::closeEvent(event);
      return;
    }

    event->ignore();
    close_pending_ = true;
    autonomous_repaint_timer_.stop();
    compositor_frame_requested_ = false;
    compositor_drive_posted_ = false;

    auto handlers = close_requested_handlers_;
    QPointer<HostWindowWidget> guard(this);
    QTimer::singleShot(0, this, [guard, handlers = std::move(handlers)]() {
      if (guard == nullptr) {
        return;
      }
      for (const auto &handler : handlers) {
        handler();
      }
      // If close was cancelled (widget still visible and not tearing down), resume.
      if (guard != nullptr && !guard->tearing_down_ && guard->isVisible()) {
        guard->close_pending_ = false;
#if !defined(Q_OS_MACOS)
        if (!guard->autonomous_repaint_timer_.isActive()) {
          guard->autonomous_repaint_timer_.start();
        }
#endif
      }
    });
  }

  void showEvent(QShowEvent *event) override {
    // Only adjustSize for non-main windows that haven't been explicitly sized
    // from JS (e.g. popup intrinsic sizing sets width/height before show).
    if (window_kind_ != 0 && !testAttribute(Qt::WA_Resized)) {
      adjustSize();
    }
    QWidget::showEvent(event);
    sync_window_metadata();
    request_compositor_frame();
#if defined(Q_OS_MACOS)
    // macOS Cocoa does not dismiss Qt::Popup on clicks to other in-process
    // windows. Install a local event monitor to detect outside clicks.
    if (window_kind_ == 1) {
      qt_popup_install_outside_click_monitor(
          rust_node_id_,
          reinterpret_cast<void *>(winId()),
          popup_outside_click_callback);
    }
#else
    if (!autonomous_repaint_timer_.isActive()) {
      autonomous_repaint_timer_.start();
    }
#endif
  }

  void hideEvent(QHideEvent *event) override {
    cpu_frame_ = QImage();
    autonomous_repaint_timer_.stop();
    compositor_frame_requested_ = false;
    compositor_drive_posted_ = false;

#if defined(Q_OS_MACOS)
    if (window_kind_ == 1) {
      qt_popup_remove_outside_click_monitor(rust_node_id_);
    }
#endif

    // Qt::Popup outside click: Qt auto-hides without closeEvent.
    // Fire close_requested handlers so JS dismiss logic runs.
    // No re-entrancy risk: JS will setVisible(false) on an already-hidden
    // widget, which is a no-op in Qt (no second hideEvent).
    if (window_kind_ != 0 && !tearing_down_ &&
        !close_requested_handlers_.empty()) {
      auto handlers = close_requested_handlers_;
      for (const auto &handler : handlers) {
        handler();
      }
    }

#if defined(Q_OS_MACOS)
    stop_compositor_display_link();
#endif
    QWidget::hideEvent(event);
  }

  bool event(QEvent *event) override {
    // Handle SurfaceAboutToBeDestroyed before base class to ensure native
    // handle is still valid for compositor teardown.
    if (event != nullptr && event->type() == QEvent::PlatformSurface) {
      auto *surface_event = static_cast<QPlatformSurfaceEvent *>(event);
      if (surface_event->surfaceEventType() ==
          QPlatformSurfaceEvent::SurfaceAboutToBeDestroyed) {
#if defined(Q_OS_MACOS)
        shutdown_compositor_display_link();
#else
        destroy_compositor_window_once();
#endif
        return QWidget::event(event);
      }
    }

    const bool handled = QWidget::event(event);
    if (!render_suppressed() &&
        event != nullptr &&
        (event->type() == QEvent::WinIdChange ||
         event->type() == QEvent::PlatformSurface ||
         event->type() == QEvent::Show)) {
      sync_window_metadata();
      if (windowHandle() != nullptr) {
        request_compositor_frame();
      }
    }
    return handled;
  }

  // -- Input event overrides (out-of-line in input.cpp) ----------------------
  void mousePressEvent(QMouseEvent *event) override;
  void mouseReleaseEvent(QMouseEvent *event) override;
  void mouseMoveEvent(QMouseEvent *event) override;
  void keyPressEvent(QKeyEvent *event) override;
  void keyReleaseEvent(QKeyEvent *event) override;
  void wheelEvent(QWheelEvent *event) override;
  void mouseDoubleClickEvent(QMouseEvent *event) override;
  void inputMethodEvent(QInputMethodEvent *event) override;
  QVariant inputMethodQuery(Qt::InputMethodQuery query) const override;
  void contextMenuEvent(QContextMenuEvent *event) override;

  bool focusNextPrevChild(bool next) override {
    if (rust_node_id_ == 0) {
      return QWidget::focusNextPrevChild(next);
    }
    if (qt_solid_spike::qt::qt_canvas_focus_next(rust_node_id_, next)) {
      return true;
    }
    return QWidget::focusNextPrevChild(next);
  }

  void focusInEvent(QFocusEvent *event) override {
    QWidget::focusInEvent(event);
    if (rust_node_id_ != 0) {
      qt_solid_spike::qt::qt_window_event_focus_change(rust_node_id_, true);
    }
  }

  void focusOutEvent(QFocusEvent *event) override {
    // Reset the input method *before* the base class processes the focus-out.
    // On macOS, IMK tries to message a Mach port during focus transitions;
    // if WA_InputMethodEnabled is still set the framework attempts to wake a
    // CFRunLoop that is no longer servicing the port, producing the
    // "error messaging the mach port for IMKCFRunLoopWakeUpReliable" log.
    // Resetting here lets IMK finalize cleanly before the window loses focus.
    if (auto *im = QGuiApplication::inputMethod(); im != nullptr) {
      im->reset();
    }
    QWidget::focusOutEvent(event);
    if (rust_node_id_ != 0) {
      qt_solid_spike::qt::qt_window_event_focus_change(rust_node_id_, false);
    }
  }

  void resizeEvent(QResizeEvent *event) override {
    QWidget::resizeEvent(event);
    if (render_suppressed()) {
      return;
    }
    const QSize s = event->size();
    const qreal dpr = capture_device_pixel_ratio();
    const auto width_px = static_cast<std::uint32_t>(
        std::max(0, qRound(static_cast<qreal>(s.width()) * dpr)));
    const auto height_px = static_cast<std::uint32_t>(
        std::max(0, qRound(static_cast<qreal>(s.height()) * dpr)));

    qt_solid_spike::qt::qt_surface_renderer_resize(rust_node_id_, width_px, height_px);
    qt_solid_spike::qt::qt_window_event_resize(
      rust_node_id_,
      static_cast<double>(s.width()),
      static_cast<double>(s.height()));
    // Force a compositor frame on resize — drive_compositor_frame() on Windows
    // early-returns when compositor_frame_requested_ is false, but resize must
    // unconditionally produce a new frame at the updated dimensions.
    compositor_frame_requested_ = true;
    drive_compositor_frame();
    request_compositor_frame();
  }

  void changeEvent(QEvent *event) override {
    QWidget::changeEvent(event);
    if (rust_node_id_ != 0 && event->type() == QEvent::WindowStateChange) {
      std::uint8_t state = 0;
      const auto ws = windowState();
      if (ws & Qt::WindowMinimized) state = 1;
      else if (ws & Qt::WindowMaximized) state = 2;
      else if (ws & Qt::WindowFullScreen) state = 3;
      qt_solid_spike::qt::qt_window_event_state_change(rust_node_id_, state);
    }
  }

private:
  void apply_window_flags() {
    const bool visible = isVisible();
    const QRect saved_geometry = geometry();

    Qt::WindowFlags flags;
    switch (window_kind_) {
    case 1: // Popup
      flags = Qt::Popup | Qt::FramelessWindowHint;
      break;
    case 2: // ToolTip
      flags = Qt::ToolTip | Qt::FramelessWindowHint;
      break;
    default: // Normal
      flags = Qt::Window;
      flags.setFlag(Qt::FramelessWindowHint, frameless_);
      break;
    }
    flags.setFlag(Qt::WindowStaysOnTopHint, always_on_top_);
    setWindowFlags(flags);

    if (window_kind_ != 0 || transparent_background_) {
      setAttribute(Qt::WA_TranslucentBackground, true);
      // Qt Cocoa's QCocoaWindow::isOpaque() checks alphaBufferSize, not
      // WA_TranslucentBackground. Without alpha in the surface format the
      // NSWindow stays opaque → black background behind transparent content.
      if (windowHandle() != nullptr) {
        auto fmt = windowHandle()->format();
        if (fmt.alphaBufferSize() < 8) {
          fmt.setAlphaBufferSize(8);
          windowHandle()->setFormat(fmt);
        }
      }
    }

    if (!saved_geometry.isNull()) {
      setGeometry(saved_geometry);
    }
    if (visible) {
      show();
    }
  }

  void sync_window_metadata() {
    constexpr auto property_name = "_qt_solid_root_node_id";
    if (windowHandle() == nullptr) {
      return;
    }
    windowHandle()->setProperty(property_name,
                                QVariant::fromValue<qulonglong>(rust_node_id_));
  }

  std::uint32_t rust_node_id_ = 0;
  std::uint8_t kind_tag_ = 0;
  std::uint8_t window_kind_ = 0;
  bool frameless_ = false;
  bool transparent_background_ = false;
  bool always_on_top_ = false;
  bool driving_compositor_frame_ = false;
  bool compositor_drive_posted_ = false;
  bool compositor_frame_requested_ = false;
#if defined(Q_OS_MACOS)
  ::MacosDisplayLinkHandle *compositor_display_link_handle_ = nullptr;
  bool compositor_display_link_running_ = false;
#endif
  bool close_pending_ = false;
  bool tearing_down_ = false;
#if !defined(Q_OS_MACOS)
  bool compositor_window_destroyed_ = false;
#endif
  QTimer autonomous_repaint_timer_;
  std::vector<CloseRequestedHandler> close_requested_handlers_;
  TextEditSession text_edit_session_;
  QImage cpu_frame_;

};

#if defined(Q_OS_MACOS)
static void popup_outside_click_callback(std::uint32_t node_id) {
  qt_window_event_close_requested(node_id);
}
#endif
