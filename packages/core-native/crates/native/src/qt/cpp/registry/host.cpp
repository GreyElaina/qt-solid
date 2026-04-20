#include "rust_widget_binding_host.h"
#include "qt_wgpu_platform.h"
#if defined(Q_OS_MACOS)
#endif

#include <QtCore/QEvent>
#include <QtCore/QMetaObject>
#include <QtCore/QPointer>
#include <QtCore/QThread>
#include <QtCore/QTimer>
#include <QtCore/QVariant>
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
void request_qt_native_wait_once();

class HostWindowWidget final : public QWidget, public RustWidgetBindingHost {
public:
  using CloseRequestedHandler = std::function<void()>;

  explicit HostWindowWidget(QWidget *parent = nullptr) : QWidget(parent) {
    apply_window_flags();
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
    stop_compositor_display_link();
      if (compositor_display_link_handle_ != nullptr) {
      ::qt_macos_display_link_destroy(compositor_display_link_handle_);
      compositor_display_link_handle_ = nullptr;
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

  std::uint32_t rust_node_id() const { return rust_node_id_; }

  void mark_compositor_scene_dirty() {
    update();
#if defined(Q_OS_MACOS)
    request_compositor_frame();
#endif
  }

  void mark_compositor_pixels_dirty() {
    update();
#if defined(Q_OS_MACOS)
    request_compositor_frame();
#endif
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
      if (compositor_display_link_running_) {
        request_qt_native_wait_once();
      } else {
        request_qt_pump();
      }
    }
#else
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
        request_qt_native_wait_once();
      }
    } else {
      stop_compositor_display_link();
    }
#else
    if (!compositor_frame_requested_) {
      return;
    }
    post_compositor_frame_drive();
#endif
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
    if (auto *context = QCoreApplication::instance()) {
      QTimer::singleShot(0, context, [handlers = std::move(handlers)]() {
        for (const auto &handler : handlers) {
          handler();
        }
      });
      return;
    }

    for (const auto &handler : handlers) {
      handler();
    }
  }

  void showEvent(QShowEvent *event) override {
    QWidget::showEvent(event);
    sync_window_metadata();
#if defined(Q_OS_MACOS)
    request_compositor_frame();
#else
    autonomous_repaint_timer_.start();
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
    return handled;
  }

private:
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
          &HostWindowWidget::compositor_display_link_callback);
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
#endif
#if !defined(Q_OS_MACOS)
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
#endif
#if !defined(Q_OS_MACOS)
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

    Qt::WindowFlags flags = Qt::Window;
    flags.setFlag(Qt::FramelessWindowHint, frameless_);
    flags.setFlag(Qt::WindowStaysOnTopHint, always_on_top_);
    setWindowFlags(flags);

    if (!saved_geometry.isNull()) {
      setGeometry(saved_geometry);
    }
    if (visible) {
      show();
    }
  }

  std::uint32_t rust_node_id_ = 0;
  std::uint8_t kind_tag_ = 0;
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
};
