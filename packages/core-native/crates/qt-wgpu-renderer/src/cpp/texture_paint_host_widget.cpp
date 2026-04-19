#include "texture_paint_host_widget.h"
#include "qt_wgpu_platform.h"

#include "native/src/qt/ffi.rs.h"

#include <QtGui/QPaintEvent>
#include <QtGui/QResizeEvent>
#include <QtWidgets/QWidget>
#include <private/qguiapplication_p.h>
#include <private/qwidget_p.h>
#include <qpa/qplatformintegration.h>

namespace {

std::optional<std::uint32_t> host_window_node_id(const QWidget *widget) {
  if (widget == nullptr) {
    return std::nullopt;
  }

  auto *top_level = widget->window();
  if (top_level == nullptr) {
    return std::nullopt;
  }
  auto *window = top_level->windowHandle();
  if (window == nullptr) {
    return std::nullopt;
  }

  const QVariant node_id = window->property("_qt_solid_root_node_id");
  if (!node_id.isValid()) {
    return std::nullopt;
  }
  return node_id.toUInt();
}

void mark_unified_compositor_widget_dirty(const QWidget *widget,
                                          std::uint32_t node_id,
                                          const QRect &local_rect) {
  if (!qt_wgpu_renderer::unified_compositor_active() || widget == nullptr ||
      node_id == 0 || local_rect.isEmpty()) {
    return;
  }

  const auto window_id = host_window_node_id(widget);
  if (!window_id.has_value()) {
    return;
  }

  auto *top_level = widget->window();
  if (top_level == nullptr) {
    return;
  }

  const QPoint top_left_in_window = widget->mapTo(top_level, local_rect.topLeft());
  qt_solid_spike::qt::qt_mark_window_compositor_pixels_dirty_region(
      *window_id, node_id, top_left_in_window.x(), top_left_in_window.y(),
      local_rect.width(), local_rect.height());
}

QPlatformBackingStoreRhiConfig::Api preferred_backing_store_api() {
  return QPlatformBackingStoreRhiConfig::Null;
}

} // namespace

class TexturePaintHostWidgetPrivate : public QWidgetPrivate {
  Q_DECLARE_PUBLIC(TexturePaintHostWidget)

public:
  explicit TexturePaintHostWidgetPrivate(
      decltype(QObjectPrivateVersion) version = QObjectPrivateVersion)
      : QWidgetPrivate(version) {}

  TextureData texture() const override;
  QPlatformTextureList::Flags textureListFlags() override;
  QPlatformBackingStoreRhiConfig rhiConfig() const override;
  void endCompose() override;

  void init();

  std::uint32_t node_id = 0;
  std::uint8_t kind_tag = 0;
  bool no_size = false;
  int preferred_width = -1;
  int preferred_height = -1;
  QPlatformBackingStoreRhiConfig config;
};

QWidgetPrivate::TextureData TexturePaintHostWidgetPrivate::texture() const {
  return TextureData{};
}

QPlatformTextureList::Flags TexturePaintHostWidgetPrivate::textureListFlags() {
  QPlatformTextureList::Flags flags = QWidgetPrivate::textureListFlags();
  flags |= QPlatformTextureList::NeedsPremultipliedAlphaBlending;
  return flags;
}

QPlatformBackingStoreRhiConfig
TexturePaintHostWidgetPrivate::rhiConfig() const {
  return config;
}

void TexturePaintHostWidgetPrivate::endCompose() {}

void TexturePaintHostWidgetPrivate::init() {
  if (Q_UNLIKELY(!QGuiApplicationPrivate::platformIntegration()->hasCapability(
          QPlatformIntegration::RhiBasedRendering))) {
    qWarning("TexturePaintHostWidget: QRhi is not supported on this platform.");
  } else {
    setRenderToTexture();
  }

  config.setEnabled(true);
  config.setApi(preferred_backing_store_api());
}

TexturePaintHostWidget::TexturePaintHostWidget(QWidget *parent)
    : TexturePaintHostWidget(*new TexturePaintHostWidgetPrivate, parent) {}

TexturePaintHostWidget::TexturePaintHostWidget(TexturePaintHostWidgetPrivate &dd,
                                               QWidget *parent,
                                               Qt::WindowFlags flags)
    : QWidget(dd, parent, flags) {
  Q_D(TexturePaintHostWidget);
  d->init();
}

TexturePaintHostWidget::~TexturePaintHostWidget() = default;

void TexturePaintHostWidget::bind_rust_widget(std::uint32_t node_id,
                                              std::uint8_t kind_tag) {
  Q_D(TexturePaintHostWidget);
  d->node_id = node_id;
  d->kind_tag = kind_tag;
  mark_unified_compositor_widget_dirty(this, d->node_id, rect());
  update();
}

void TexturePaintHostWidget::set_preferred_width(int width) {
  Q_D(TexturePaintHostWidget);
  d->preferred_width = std::max(0, width);
  sync_preferred_size();
}

void TexturePaintHostWidget::set_preferred_height(int height) {
  Q_D(TexturePaintHostWidget);
  d->preferred_height = std::max(0, height);
  sync_preferred_size();
}

QSize TexturePaintHostWidget::sizeHint() const {
  const QSize fallback = QWidget::sizeHint();
  return QSize(resolve_width_hint(fallback), resolve_height_hint(fallback));
}

QSize TexturePaintHostWidget::minimumSizeHint() const {
  const QSize fallback = QWidget::minimumSizeHint();
  return QSize(std::max(fallback.width(), minimumWidth()),
               std::max(fallback.height(), minimumHeight()));
}

void TexturePaintHostWidget::mark_frame_dirty() {
  Q_D(TexturePaintHostWidget);
  mark_unified_compositor_widget_dirty(this, d->node_id, rect());
  update();
}

bool TexturePaintHostWidget::event(QEvent *event) {
  Q_D(TexturePaintHostWidget);
  if (event->type() == QEvent::Show && isVisible()) {
    mark_unified_compositor_widget_dirty(this, d->node_id, rect());
    update();
  }

  return QWidget::event(event);
}

void TexturePaintHostWidget::paintEvent(QPaintEvent *event) {
  Q_UNUSED(event);

  Q_D(TexturePaintHostWidget);
  if (!updatesEnabled() || d->no_size) {
    return;
  }

  if (!qt_wgpu_renderer::unified_compositor_active()) {
    qWarning("TexturePaintHostWidget requires unified compositor backingstore");
    return;
  }
}

void TexturePaintHostWidget::resizeEvent(QResizeEvent *event) {
  Q_D(TexturePaintHostWidget);
  if (event->size().isEmpty()) {
    d->no_size = true;
    return;
  }

  d->no_size = false;
  mark_unified_compositor_widget_dirty(this, d->node_id, rect());
  update();
}

int TexturePaintHostWidget::resolve_width_hint(const QSize &fallback) const {
  Q_D(const TexturePaintHostWidget);
  if (d->preferred_width >= 0) {
    return d->preferred_width;
  }
  return std::max(fallback.width(), minimumWidth());
}

int TexturePaintHostWidget::resolve_height_hint(const QSize &fallback) const {
  Q_D(const TexturePaintHostWidget);
  if (d->preferred_height >= 0) {
    return d->preferred_height;
  }
  return std::max(fallback.height(), minimumHeight());
}

void TexturePaintHostWidget::sync_preferred_size() {
  updateGeometry();
  const QSize next = sizeHint().expandedTo(minimumSize());
  Q_D(TexturePaintHostWidget);
  const int width = d->preferred_width >= 0 ? next.width() : QWidget::width();
  const int height =
      d->preferred_height >= 0 ? next.height() : QWidget::height();
  resize(width, height);
}
