#include "rust_widget_binding_host.h"

#include <functional>
#include <vector>

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

      tick_frame();
      focus->update();
      update();
      qt_solid_spike::qt::window_host_request_wake();
    });
  }

  void add_close_requested_handler(CloseRequestedHandler handler) {
    close_requested_handlers_.push_back(std::move(handler));
  }

  void bind_rust_widget(std::uint32_t node_id, std::uint8_t kind_tag) override {
    rust_node_id_ = node_id;
    kind_tag_ = kind_tag;
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

  void mark_compositor_scene_dirty() { update(); }

  void mark_compositor_pixels_dirty() { update(); }

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
    tick_frame();
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
    autonomous_repaint_timer_.start();
  }

  void hideEvent(QHideEvent *event) override {
    autonomous_repaint_timer_.stop();
    QWidget::hideEvent(event);
  }

private:
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
  QTimer autonomous_repaint_timer_;
  std::vector<CloseRequestedHandler> close_requested_handlers_;
};
