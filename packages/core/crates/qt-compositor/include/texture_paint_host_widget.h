#pragma once

#include "rust_widget_binding_host.h"

#include <QtCore/QSize>
#include <QtWidgets/QWidget>

#include <algorithm>
#include <cstdint>

class TexturePaintHostWidgetPrivate;

class TexturePaintHostWidget : public QWidget, public RustWidgetBindingHost {
  Q_DECLARE_PRIVATE(TexturePaintHostWidget)

public:
  explicit TexturePaintHostWidget(QWidget *parent = nullptr);
  ~TexturePaintHostWidget() override;

  void bind_rust_widget(std::uint32_t node_id, std::uint8_t kind_tag) override;

  void set_preferred_width(int width);
  void set_preferred_height(int height);
  QSize sizeHint() const override;
  QSize minimumSizeHint() const override;
  void mark_frame_dirty();

protected:
  bool event(QEvent *event) override;
  void paintEvent(QPaintEvent *event) override;
  void resizeEvent(QResizeEvent *event) override;

private:
  explicit TexturePaintHostWidget(TexturePaintHostWidgetPrivate &dd,
                                  QWidget *parent = nullptr,
                                  Qt::WindowFlags flags = {});

  int resolve_width_hint(const QSize &fallback) const;
  int resolve_height_hint(const QSize &fallback) const;
  void sync_preferred_size();
};
