#include "rust_widget_binding_host.h"
#include "qt_wgpu_platform.h"

#include "native/src/qt/ffi.rs.h"

#if defined(Q_OS_MACOS)
#include "qt/macos_display_link_bridge.h"
#endif

#include <QtCore/QEvent>
#include <QtCore/QMetaObject>
#include <QtCore/QPointer>
#include <QtCore/QThread>
#include <QtCore/QTimer>
#include <QtCore/QVariant>
#include <QtGui/QMouseEvent>
#include <QtGui/QWheelEvent>
#include <QtGui/QPlatformSurfaceEvent>
#include <QtWidgets/QApplication>
#include <atomic>
#include <cstdio>
#include <functional>
#include <vector>

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

void request_qt_pump();

// ---------------------------------------------------------------------------
// TextEditSession — wraps QWidgetLineControl for a focused TextInput fragment
// ---------------------------------------------------------------------------

class TextEditSession {
public:
  TextEditSession() = default;
  ~TextEditSession() { deactivate(); }

  bool active() const { return control_ != nullptr; }
  std::uint32_t canvas_node_id() const { return canvas_node_id_; }
  std::uint32_t fragment_id() const { return fragment_id_; }

  void activate(std::uint32_t canvas_node_id, std::uint32_t fragment_id,
                const QString &text, double font_size, int cursor_pos,
                int sel_start, int sel_end) {
    if (control_ != nullptr && canvas_node_id_ == canvas_node_id &&
        fragment_id_ == fragment_id) {
      return;
    }
    deactivate();

    canvas_node_id_ = canvas_node_id;
    fragment_id_ = fragment_id;
    font_size_ = font_size;

    control_ = new QWidgetLineControl(text);
    control_->setCursorPosition(cursor_pos);
    if (sel_start >= 0 && sel_end > sel_start) {
      control_->setSelection(sel_start, sel_end - sel_start);
    }

    QObject::connect(control_, &QWidgetLineControl::updateNeeded, [this](const QRect &) {
      if (control_ == nullptr) {
        return;
      }
      const bool visible = control_->cursorBlinkStatus();
      qt_solid_spike::qt::qt_text_edit_set_caret_visible(
          canvas_node_id_, fragment_id_, visible);
      request_qt_pump();
    });

    control_->setBlinkingCursorEnabled(true);
  }

  void deactivate() {
    if (control_ == nullptr) {
      return;
    }
    delete control_;
    control_ = nullptr;
    canvas_node_id_ = 0;
    fragment_id_ = 0;
  }

  bool process_key_event(QKeyEvent *event) {
    if (control_ == nullptr) {
      return false;
    }
    control_->processKeyEvent(event);
    sync_to_rust();
    return true;
  }

  bool process_input_method_event(QInputMethodEvent *event) {
    if (control_ == nullptr) {
      return false;
    }
    control_->processInputMethodEvent(event);
    sync_to_rust();
    return true;
  }

  QVariant input_method_query(Qt::InputMethodQuery query) const {
    if (control_ == nullptr) {
      return QVariant();
    }
    switch (query) {
    case Qt::ImCursorRectangle: {
      const qreal x = control_->cursorToX();
      return QRectF(x, 0.0, 1.0, font_size_);
    }
    case Qt::ImCursorPosition:
      return control_->cursor();
    case Qt::ImSurroundingText:
      return control_->text();
    case Qt::ImCurrentSelection:
      return control_->selectedText();
    case Qt::ImAnchorPosition: {
      const int sel_start = control_->selectionStart();
      const int cursor = control_->cursor();
      if (sel_start >= 0) {
        return (sel_start == cursor) ? control_->selectionEnd() : sel_start;
      }
      return cursor;
    }
    case Qt::ImEnabled:
      return true;
    default:
      return QVariant();
    }
  }

  void click_to_cursor(double local_x) {
    if (control_ == nullptr) {
      return;
    }
    const QTextLine line = control_->textLayout()->lineAt(0);
    if (!line.isValid()) {
      return;
    }
    const int pos = line.xToCursor(local_x, QTextLine::CursorBetweenCharacters);
    control_->setCursorPosition(pos);
    sync_to_rust();
  }

  void drag_to_cursor(double local_x) {
    if (control_ == nullptr) {
      return;
    }
    const QTextLine line = control_->textLayout()->lineAt(0);
    if (!line.isValid()) {
      return;
    }
    const int pos = line.xToCursor(local_x, QTextLine::CursorBetweenCharacters);
    control_->moveCursor(pos, true);
    sync_to_rust();
  }

private:
  void sync_to_rust() {
    if (control_ == nullptr) {
      return;
    }

    const QString text = control_->text();
    const QByteArray utf8 = text.toUtf8();

    // Re-shape text via existing Qt shaping FFI.
    auto shaped = qt_solid_spike::qt::qt_shape_text_with_cursors(
        rust::Str(utf8.constData(), utf8.size()), font_size_, rust::Str("", 0), 0, false);

    const int cursor = control_->cursor();
    const int sel_start = control_->selectionStart();
    const int sel_end = control_->selectionEnd();

    rust::Slice<const qt_solid_spike::qt::QtShapedPathEl> elements(
        shaped.elements.data(), shaped.elements.size());
    rust::Slice<const double> cursor_positions(
        shaped.cursor_x_positions.data(), shaped.cursor_x_positions.size());

    qt_solid_spike::qt::qt_text_edit_sync(
        canvas_node_id_, fragment_id_,
        rust::Str(utf8.constData(), utf8.size()),
        cursor, sel_start, sel_end,
        elements, cursor_positions,
        shaped.ascent, shaped.descent, shaped.total_width);
  }

  QWidgetLineControl *control_ = nullptr;
  std::uint32_t canvas_node_id_ = 0;
  std::uint32_t fragment_id_ = 0;
  double font_size_ = 14.0;
};

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
      if (!isVisible()) {
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
#if defined(Q_OS_MACOS)
    shutdown_compositor_display_link();
#else
    if (windowHandle() != nullptr) {
      qt_wgpu_renderer::destroy_unified_compositor_window(
          windowHandle(), capture_device_pixel_ratio());
    }
#endif
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

  bool request_compositor_frame() {
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
#if defined(Q_OS_MACOS)
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

  void notify_compositor_frame_complete() {
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

  void drive_compositor_frame_from_signal() {
    if (!isVisible() || driving_compositor_frame_ || rust_node_id_ == 0 ||
        windowHandle() == nullptr) {
      return;
    }
#if !defined(Q_OS_MACOS)
    compositor_frame_requested_ = true;
#endif
    drive_compositor_frame();
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
    if (rust_node_id_ == 0) {
      return;
    }

    try {
      qt_solid_spike::qt::qt_window_frame_tick(rust_node_id_);
    } catch (const rust::Error &error) {
      qWarning() << "failed to tick host window frame for node" << rust_node_id_
                 << ":" << error.what();
    }
  }

  QWidget *widget_at_screen_point(const QPoint &screen_pos) const {
    if (!isVisible()) {
      return nullptr;
    }

    const QPoint local = mapFromGlobal(screen_pos);
    return deepest_child_at(const_cast<HostWindowWidget *>(this), local);
  }

protected:
  void paintEvent(QPaintEvent *event) override {
#if !defined(Q_OS_MACOS)
    tick_frame();
#endif
    QWidget::paintEvent(event);
  }

  void closeEvent(QCloseEvent *event) override {
    if (close_requested_handlers_.empty()) {
      QWidget::closeEvent(event);
      return;
    }

    event->ignore();
    auto handlers = close_requested_handlers_;
    QPointer<HostWindowWidget> guard(this);
    QTimer::singleShot(0, this, [guard, handlers = std::move(handlers)]() {
      if (guard == nullptr) {
        return;
      }
      for (const auto &handler : handlers) {
        handler();
      }
    });
  }

  void showEvent(QShowEvent *event) override {
    if (window_kind_ != 0) {
      adjustSize();
    }
    QWidget::showEvent(event);
    sync_window_metadata();
    request_compositor_frame();
#if !defined(Q_OS_MACOS)
    if (!autonomous_repaint_timer_.isActive()) {
      autonomous_repaint_timer_.start();
    }
#endif
  }

  void hideEvent(QHideEvent *event) override {
    autonomous_repaint_timer_.stop();
    compositor_frame_requested_ = false;
    compositor_drive_posted_ = false;
#if defined(Q_OS_MACOS)
    stop_compositor_display_link();
#endif
    QWidget::hideEvent(event);
  }

  bool event(QEvent *event) override {
    const bool handled = QWidget::event(event);
    if (event != nullptr &&
        (event->type() == QEvent::WinIdChange ||
         event->type() == QEvent::PlatformSurface ||
         event->type() == QEvent::Show)) {
      sync_window_metadata();
    }
    if (event != nullptr &&
        event->type() == QEvent::PlatformSurface) {
      auto *surface_event = static_cast<QPlatformSurfaceEvent *>(event);
      if (surface_event->surfaceEventType() ==
          QPlatformSurfaceEvent::SurfaceAboutToBeDestroyed) {
#if defined(Q_OS_MACOS)
        shutdown_compositor_display_link();
#else
        if (windowHandle() != nullptr) {
          qt_wgpu_renderer::destroy_unified_compositor_window(
              windowHandle(), capture_device_pixel_ratio());
        }
#endif
      }
    }
    return handled;
  }

  void mousePressEvent(QMouseEvent *event) override {
    const QPointF pos = event->position();
    qt_solid_spike::qt::qt_canvas_pointer_event(rust_node_id_, 1, pos.x(), pos.y());
  }

  void mouseReleaseEvent(QMouseEvent *event) override {
    const QPointF pos = event->position();
    qt_solid_spike::qt::qt_canvas_pointer_event(rust_node_id_, 2, pos.x(), pos.y());
  }

  void mouseMoveEvent(QMouseEvent *event) override {
    const QPointF pos = event->position();
    if (text_edit_session_.active() && (event->buttons() & Qt::LeftButton)) {
      qt_solid_spike::qt::qt_canvas_pointer_event(rust_node_id_, 4, pos.x(), pos.y());
    } else {
      qt_solid_spike::qt::qt_canvas_pointer_event(rust_node_id_, 3, pos.x(), pos.y());
    }
  }

  void keyPressEvent(QKeyEvent *event) override {
    if (rust_node_id_ == 0) {
      QWidget::keyPressEvent(event);
      return;
    }
    if (text_edit_session_.process_key_event(event)) {
      return;
    }
    // Forward to fragment dispatch (event_tag 1 = keydown).
    forward_key_event(event, 1);
  }

  void keyReleaseEvent(QKeyEvent *event) override {
    if (rust_node_id_ == 0) {
      QWidget::keyReleaseEvent(event);
      return;
    }
    // Forward to fragment dispatch (event_tag 2 = keyup).
    forward_key_event(event, 2);
  }

  void wheelEvent(QWheelEvent *event) override {
    if (rust_node_id_ == 0) {
      QWidget::wheelEvent(event);
      return;
    }
    const QPoint angle = event->angleDelta();
    const QPoint pixel = event->pixelDelta();
    const QPointF pos = event->position();
    const auto mods = static_cast<std::uint32_t>(event->modifiers().toInt());
    // Phase: 0=NoScroll, 1=Begin, 2=Update, 3=End, 4=Momentum
    uint32_t phase = 0;
    switch (event->phase()) {
      case Qt::ScrollBegin:   phase = 1; break;
      case Qt::ScrollUpdate:  phase = 2; break;
      case Qt::ScrollEnd:     phase = 3; break;
      case Qt::ScrollMomentum: phase = 4; break;
      default: phase = 0; break;
    }
    qt_solid_spike::qt::qt_canvas_wheel_event(
        rust_node_id_,
        static_cast<double>(angle.x()),
        static_cast<double>(angle.y()),
        static_cast<double>(pixel.x()),
        static_cast<double>(pixel.y()),
        pos.x(), pos.y(), mods, phase);
  }

  void mouseDoubleClickEvent(QMouseEvent *event) override {
    if (rust_node_id_ == 0) {
      QWidget::mouseDoubleClickEvent(event);
      return;
    }
    const QPointF pos = event->position();
    // Qt's double-click replaces the second press; forward tag 1 to restore
    // pointer down/up symmetry, then tag 5 for the double-click itself.
    qt_solid_spike::qt::qt_canvas_pointer_event(rust_node_id_, 1, pos.x(), pos.y());
    qt_solid_spike::qt::qt_canvas_pointer_event(rust_node_id_, 5, pos.x(), pos.y());
  }

  void inputMethodEvent(QInputMethodEvent *event) override {
    if (text_edit_session_.process_input_method_event(event)) {
      return;
    }
    QWidget::inputMethodEvent(event);
  }

  QVariant inputMethodQuery(Qt::InputMethodQuery query) const override {
    if (text_edit_session_.active()) {
      return text_edit_session_.input_method_query(query);
    }
    return QWidget::inputMethodQuery(query);
  }

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
    QWidget::focusOutEvent(event);
    if (rust_node_id_ != 0) {
      qt_solid_spike::qt::qt_window_event_focus_change(rust_node_id_, false);
    }
  }

  void resizeEvent(QResizeEvent *event) override {
    QWidget::resizeEvent(event);
    if (rust_node_id_ == 0) {
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

#if defined(Q_OS_MACOS)
    void *wgpu_layer = reinterpret_cast<void *>(static_cast<quintptr>(
        qt_solid_spike::qt::qt_surface_renderer_metal_layer_ptr(rust_node_id_)));
    qt_wgpu_renderer::set_metal_layer_presents_with_transaction(wgpu_layer, true);
#endif
    drive_compositor_frame();
#if defined(Q_OS_MACOS)
    qt_wgpu_renderer::set_metal_layer_presents_with_transaction(wgpu_layer, false);
#endif
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
  void forward_key_event(QKeyEvent *event, std::uint8_t event_tag) {
    const auto qt_key = static_cast<std::int32_t>(event->key());
    const auto mods = static_cast<std::uint32_t>(event->modifiers().toInt());
    const QByteArray text_utf8 = event->text().toUtf8();
    const bool repeat = event->isAutoRepeat();
    const auto scan_code = static_cast<std::uint32_t>(event->nativeScanCode());
    const auto virtual_key = static_cast<std::uint32_t>(event->nativeVirtualKey());
    qt_solid_spike::qt::qt_canvas_key_event(
        rust_node_id_, event_tag, qt_key, mods,
        rust::Str(text_utf8.constData(), text_utf8.size()),
        repeat, scan_code, virtual_key);
  }

#if defined(Q_OS_MACOS)
  static void compositor_display_link_callback(void *context, void *drawable) {
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

  void start_compositor_display_link() {
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

  void stop_compositor_display_link() {
    compositor_display_link_tick_posted_.store(false);
    if (compositor_display_link_handle_ == nullptr) {
      compositor_display_link_running_ = false;
      return;
    }
    qt_solid_wgpu_trace("stop-link node=%u", rust_node_id_);
    ::qt_macos_display_link_stop(compositor_display_link_handle_);
    compositor_display_link_running_ = false;
  }

  void shutdown_compositor_display_link() {
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

  void post_compositor_frame_drive_from_display_link(void *drawable) {
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

  void handle_compositor_display_link_tick(void *drawable) {
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
      request_qt_pump();
      break;
    case qt_wgpu_renderer::UnifiedCompositorDriveStatus::Busy:
      start_compositor_display_link();
      break;
    case qt_wgpu_renderer::UnifiedCompositorDriveStatus::Idle:
      if (!qt_wgpu_renderer::unified_compositor_window_display_link_should_run(
              windowHandle(), capture_device_pixel_ratio())) {
        stop_compositor_display_link();
      }
      break;
    case qt_wgpu_renderer::UnifiedCompositorDriveStatus::NeedsQtRepaint:
      stop_compositor_display_link();
      update();
      break;
    }
  }
#endif


  void sync_window_metadata() {
    constexpr auto property_name = "_qt_solid_root_node_id";
    if (windowHandle() == nullptr) {
      return;
    }
    windowHandle()->setProperty(property_name,
                                QVariant::fromValue<qulonglong>(rust_node_id_));
  }

  void post_compositor_frame_drive() {
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

  void drive_compositor_frame() {
    if (!isVisible() || rust_node_id_ == 0 || windowHandle() == nullptr) {
#if !defined(Q_OS_MACOS)
      compositor_frame_requested_ = false;
#endif
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

    if (window_kind_ != 0) {
      setAttribute(Qt::WA_TranslucentBackground, true);
    }

    if (!saved_geometry.isNull()) {
      setGeometry(saved_geometry);
    }
    if (visible) {
      show();
    }
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
  std::atomic_bool compositor_display_link_tick_posted_ = false;
#endif
  QTimer autonomous_repaint_timer_;
  std::vector<CloseRequestedHandler> close_requested_handlers_;
  TextEditSession text_edit_session_;

};
