#pragma once

#include <QtCore/QSize>
#include <QtWidgets/QWidget>

#include <algorithm>

class CustomPaintHostWidget : public QWidget {
public:
  using QWidget::QWidget;

  void set_preferred_width(int width) {
    preferred_width_ = std::max(0, width);
    sync_preferred_size();
  }

  void set_preferred_height(int height) {
    preferred_height_ = std::max(0, height);
    sync_preferred_size();
  }

  QSize sizeHint() const override {
    const QSize fallback = QWidget::sizeHint();
    return QSize(resolve_width_hint(fallback), resolve_height_hint(fallback));
  }

  QSize minimumSizeHint() const override {
    const QSize fallback = QWidget::minimumSizeHint();
    return QSize(std::max(fallback.width(), minimumWidth()),
                 std::max(fallback.height(), minimumHeight()));
  }

private:
  int preferred_width_ = -1;
  int preferred_height_ = -1;

  int resolve_width_hint(const QSize &fallback) const {
    if (preferred_width_ >= 0) {
      return preferred_width_;
    }
    return std::max(fallback.width(), minimumWidth());
  }

  int resolve_height_hint(const QSize &fallback) const {
    if (preferred_height_ >= 0) {
      return preferred_height_;
    }
    return std::max(fallback.height(), minimumHeight());
  }

  void sync_preferred_size() {
    updateGeometry();
    const QSize next = sizeHint().expandedTo(minimumSize());
    const int width = preferred_width_ >= 0 ? next.width() : QWidget::width();
    const int height = preferred_height_ >= 0 ? next.height() : QWidget::height();
    resize(width, height);
  }
};
