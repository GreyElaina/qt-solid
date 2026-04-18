#include "texture_paint_host_widget.h"
#include "qt_wgpu_rhi.h"

#include "native/src/qt/ffi.rs.h"

#include <QtGui/QImage>
#include <QtGui/QPaintEvent>
#include <QtGui/QResizeEvent>
#include <QtCore/QByteArray>
#include <QtWidgets/QWidget>
#include <private/qguiapplication_p.h>
#include <private/qwidget_p.h>
#include <qpa/qplatformintegration.h>
#include <rhi/qrhi.h>

namespace {

enum class TextureWidgetSourceKind : std::uint8_t {
  CpuBytes = 0,
  ImportedNativeTexture = 1,
};

std::optional<QPlatformBackingStoreRhiConfig::Api>
preferred_backing_store_api() {
  const QByteArray forced_backend =
      qgetenv("QT_WIDGETS_RHI_BACKEND").trimmed().toLower();
  if (!forced_backend.isEmpty()) {
#if defined(Q_OS_WIN)
    if (forced_backend == "d3d12") {
      return QPlatformBackingStoreRhiConfig::D3D12;
    }
    if (forced_backend == "d3d11" || forced_backend == "d3d") {
      return QPlatformBackingStoreRhiConfig::D3D11;
    }
#endif
#if QT_CONFIG(metal)
    if (forced_backend == "metal") {
      return QPlatformBackingStoreRhiConfig::Metal;
    }
#endif
#if QT_CONFIG(vulkan)
    if (forced_backend == "vulkan") {
      return QPlatformBackingStoreRhiConfig::Vulkan;
    }
#endif
#if QT_CONFIG(opengl)
    if (forced_backend == "opengl" || forced_backend == "gl") {
      return QPlatformBackingStoreRhiConfig::OpenGL;
    }
#endif
    if (forced_backend == "null") {
      return QPlatformBackingStoreRhiConfig::Null;
    }
    qWarning("TexturePaintHostWidget: unsupported QT_WIDGETS_RHI_BACKEND override '%s'",
             forced_backend.constData());
  }

#if defined(Q_OS_WIN)
  return QPlatformBackingStoreRhiConfig::D3D12;
#elif QT_CONFIG(metal)
  return QPlatformBackingStoreRhiConfig::Metal;
#elif QT_CONFIG(vulkan)
  return QPlatformBackingStoreRhiConfig::Vulkan;
#elif QT_CONFIG(opengl)
  return QPlatformBackingStoreRhiConfig::OpenGL;
#else
  return std::nullopt;
#endif
}

} // namespace

class TexturePaintHostWidgetPrivate : public QWidgetPrivate {
  Q_DECLARE_PUBLIC(TexturePaintHostWidget)

public:
  struct SourceTextureState {
    QRhiTexture *texture = nullptr;
    QSize pixel_size;
    std::uint8_t format_tag = 0;
    TextureWidgetSourceKind source_kind = TextureWidgetSourceKind::CpuBytes;
    std::uint64_t native_object = 0;
    int native_layout = 0;
  };

  explicit TexturePaintHostWidgetPrivate(
      decltype(QObjectPrivateVersion) version = QObjectPrivateVersion)
      : QWidgetPrivate(version) {}

  TextureData texture() const override;
  QPlatformTextureList::Flags textureListFlags() override;
  QPlatformBackingStoreRhiConfig rhiConfig() const override;
  void endCompose() override;

  void init();
  void ensure_rhi();
  SourceTextureState *ensure_source_texture(const QSize &pixel_size,
                                            std::uint8_t format_tag,
                                            bool *recreated);
  SourceTextureState *ensure_imported_source_texture(
      const qt_solid_spike::qt::QtNativeTextureLeaseInfo &texture_info,
      bool *recreated);
  void queue_source_texture_for_delete(QRhiTexture *texture);
  void release_resources();
  void release_source_resources();
  bool update_prepared_frame(
      const qt_solid_spike::qt::QtPreparedTextureWidgetFrame &prepared_frame);

  std::uint32_t node_id = 0;
  std::uint8_t kind_tag = 0;
  bool no_size = false;
  bool texture_invalid = false;
  int preferred_width = -1;
  int preferred_height = -1;
  QRhi *rhi = nullptr;
  QPlatformBackingStoreRhiConfig config;
  SourceTextureState source_texture;
  mutable QVector<QRhiResource *> pending_deletes;
};

QWidgetPrivate::TextureData TexturePaintHostWidgetPrivate::texture() const {
  qDeleteAll(pending_deletes);
  pending_deletes.clear();

  TextureData data;
  if (!texture_invalid)
    data.textureLeft = source_texture.texture;
  return data;
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

  const auto api = preferred_backing_store_api();
  config.setEnabled(api.has_value());
  if (api.has_value()) {
    config.setApi(*api);
  } else {
    qWarning("TexturePaintHostWidget: no supported RHI backend is available");
  }
}

void TexturePaintHostWidgetPrivate::ensure_rhi() {
  Q_Q(TexturePaintHostWidget);
  QRhi *current_rhi = QWidgetPrivate::rhi();
  if (current_rhi &&
      current_rhi->backend() !=
          QBackingStoreRhiSupport::apiToRhiBackend(config.api())) {
    qWarning(
        "TexturePaintHostWidget: top-level window uses incompatible graphics API "
        "'%s'",
        current_rhi->backendName());
    return;
  }

  if (current_rhi && current_rhi != rhi) {
    if (rhi != nullptr) {
      rhi->removeCleanupCallback(q);
      release_resources();
    }

    current_rhi->addCleanupCallback(q, [q, this](QRhi *registered_rhi) {
      if (!QWidgetPrivate::get(q)->data.in_destructor && rhi == registered_rhi) {
        release_resources();
        rhi = nullptr;
      }
    });
  }

  rhi = current_rhi;
}

void TexturePaintHostWidgetPrivate::release_source_resources() {
  delete source_texture.texture;
  source_texture.texture = nullptr;
  source_texture.pixel_size = {};
  source_texture.format_tag = 0;
  source_texture.source_kind = TextureWidgetSourceKind::CpuBytes;
  source_texture.native_object = 0;
  source_texture.native_layout = 0;
}

void TexturePaintHostWidgetPrivate::queue_source_texture_for_delete(
    QRhiTexture *texture) {
  if (texture != nullptr)
    pending_deletes.append(texture);
}

void TexturePaintHostWidgetPrivate::release_resources() {
  release_source_resources();
  qDeleteAll(pending_deletes);
  pending_deletes.clear();
  texture_invalid = true;
}

TexturePaintHostWidgetPrivate::SourceTextureState *
TexturePaintHostWidgetPrivate::ensure_source_texture(const QSize &pixel_size,
                                                     std::uint8_t format_tag,
                                                     bool *recreated) {
  const auto texture_format = qt_wgpu_renderer::texture_format_for_tag(format_tag);
  if (!texture_format.has_value() || rhi == nullptr) {
    return nullptr;
  }

  const bool needs_recreate = source_texture.texture == nullptr ||
                              source_texture.pixel_size != pixel_size ||
                              source_texture.format_tag != format_tag ||
                              source_texture.source_kind !=
                                  TextureWidgetSourceKind::CpuBytes;
  if (recreated != nullptr) {
    *recreated = needs_recreate;
  }
  if (!needs_recreate) {
    return &source_texture;
  }

  auto *next_texture = rhi->newTexture(*texture_format, pixel_size);
  if (!next_texture->create()) {
    qWarning("TexturePaintHostWidget: failed to create source texture");
    delete next_texture;
    return source_texture.texture != nullptr ? &source_texture : nullptr;
  }

  queue_source_texture_for_delete(source_texture.texture);
  source_texture.texture = next_texture;
  source_texture.pixel_size = pixel_size;
  source_texture.format_tag = format_tag;
  source_texture.source_kind = TextureWidgetSourceKind::CpuBytes;
  source_texture.native_object = 0;
  source_texture.native_layout = 0;
  texture_invalid = false;
  return &source_texture;
}

TexturePaintHostWidgetPrivate::SourceTextureState *
TexturePaintHostWidgetPrivate::ensure_imported_source_texture(
    const qt_solid_spike::qt::QtNativeTextureLeaseInfo &texture_info,
    bool *recreated) {
  if (rhi == nullptr) {
    return nullptr;
  }

  const auto texture_format =
      qt_wgpu_renderer::texture_format_for_tag(texture_info.format_tag);
  const auto backend =
      qt_wgpu_renderer::texture_backend_for_tag(texture_info.backend_tag);
  if (!texture_format.has_value() || !backend.has_value()) {
    return nullptr;
  }
  if (rhi->backend() != *backend) {
    qWarning() << "texture widget frame backend does not match current QRhi"
               << texture_info.backend_tag << "!=" << int(rhi->backend());
    return nullptr;
  }

  const QSize pixel_size(static_cast<int>(texture_info.width_px),
                         static_cast<int>(texture_info.height_px));
  const bool needs_recreate =
      source_texture.texture == nullptr ||
      source_texture.source_kind != TextureWidgetSourceKind::ImportedNativeTexture ||
      source_texture.pixel_size != pixel_size ||
      source_texture.format_tag != texture_info.format_tag ||
      source_texture.native_object != texture_info.object;
  if (recreated != nullptr) {
    *recreated = needs_recreate;
  }

  if (!needs_recreate) {
    if (source_texture.native_layout != texture_info.layout) {
      source_texture.texture->setNativeLayout(texture_info.layout);
      source_texture.native_layout = texture_info.layout;
    }
    return &source_texture;
  }

  auto *next_texture = rhi->newTexture(*texture_format, pixel_size);
  if (!next_texture->createFrom(
          QRhiTexture::NativeTexture{texture_info.object, texture_info.layout})) {
    qWarning("TexturePaintHostWidget: failed to import native source texture");
    delete next_texture;
    return source_texture.texture != nullptr ? &source_texture : nullptr;
  }

  queue_source_texture_for_delete(source_texture.texture);
  source_texture.texture = next_texture;
  source_texture.pixel_size = pixel_size;
  source_texture.format_tag = texture_info.format_tag;
  source_texture.source_kind = TextureWidgetSourceKind::ImportedNativeTexture;
  source_texture.native_object = texture_info.object;
  source_texture.native_layout = texture_info.layout;
  texture_invalid = false;
  return &source_texture;
}

bool TexturePaintHostWidgetPrivate::update_prepared_frame(
    const qt_solid_spike::qt::QtPreparedTextureWidgetFrame &prepared_frame) {
  Q_Q(TexturePaintHostWidget);
  if (rhi == nullptr || node_id == 0) {
    return false;
  }

  const auto layout =
      qt_solid_spike::qt::qt_texture_widget_frame_layout(prepared_frame);
  const auto source_kind = static_cast<TextureWidgetSourceKind>(
      qt_solid_spike::qt::qt_texture_widget_frame_source_kind(prepared_frame));

  const QSize capture_size(static_cast<int>(layout.width_px),
                           static_cast<int>(layout.height_px));
  bool recreated = false;
  SourceTextureState *source_state = nullptr;
  switch (source_kind) {
  case TextureWidgetSourceKind::CpuBytes: {
    const auto image_format = qt_wgpu_renderer::image_format_for_tag(layout.format_tag);
    if (!image_format.has_value()) {
      qWarning() << "texture widget frame uses unsupported format tag"
                 << layout.format_tag;
      return false;
    }

    const auto bytes =
        qt_solid_spike::qt::qt_texture_widget_frame_bytes(prepared_frame);
    source_state = ensure_source_texture(capture_size, layout.format_tag, &recreated);
    if (source_state == nullptr || source_state->texture == nullptr) {
      return false;
    }

    QImage source_image(reinterpret_cast<const uchar *>(bytes.data()),
                        capture_size.width(), capture_size.height(),
                        static_cast<qsizetype>(layout.stride), *image_format);

    QRhiResourceUpdateBatch *resource_updates = rhi->nextResourceUpdateBatch();
    if (resource_updates == nullptr) {
      qWarning("TexturePaintHostWidget: failed to allocate resource update batch");
      return false;
    }

    const std::uint8_t upload_kind =
        qt_solid_spike::qt::qt_texture_widget_frame_upload_kind(prepared_frame);
    bool has_updates = false;
    if (recreated || upload_kind == 1) {
      resource_updates->uploadTexture(source_state->texture, source_image);
      has_updates = true;
    } else if (upload_kind == 2) {
      auto dirty_rects =
          qt_solid_spike::qt::qt_texture_widget_frame_dirty_rects(prepared_frame);
      std::vector<QRhiTextureUploadEntry> upload_entries;
      upload_entries.reserve(static_cast<std::size_t>(dirty_rects.size()));
      for (const auto &dirty_rect : dirty_rects) {
        const QRect rect(dirty_rect.x, dirty_rect.y, dirty_rect.width,
                         dirty_rect.height);
        if (!rect.isValid() || !source_image.rect().contains(rect)) {
          continue;
        }

        QRhiTextureSubresourceUploadDescription subresource(
            source_image.copy(rect));
        subresource.setDestinationTopLeft(rect.topLeft());
        upload_entries.emplace_back(0, 0, subresource);
      }
      if (!upload_entries.empty()) {
        QRhiTextureUploadDescription upload_description;
        upload_description.setEntries(upload_entries.begin(),
                                      upload_entries.end());
        resource_updates->uploadTexture(source_state->texture,
                                        upload_description);
        has_updates = true;
      }
    }

    if (has_updates) {
      QRhiCommandBuffer *command_buffer = nullptr;
      if (rhi->beginOffscreenFrame(&command_buffer) != QRhi::FrameOpSuccess) {
        resource_updates->release();
        return false;
      }
      command_buffer->resourceUpdate(resource_updates);
      rhi->endOffscreenFrame();
    } else {
      resource_updates->release();
    }
    break;
  }
  case TextureWidgetSourceKind::ImportedNativeTexture: {
    const auto native_texture_info =
        qt_solid_spike::qt::qt_texture_widget_frame_native_texture_info(
            prepared_frame);
    source_state = ensure_imported_source_texture(native_texture_info, &recreated);
    if (source_state == nullptr || source_state->texture == nullptr) {
      return false;
    }
    break;
  }
  default:
    qWarning() << "texture widget frame uses unsupported source kind"
               << static_cast<int>(source_kind);
    return false;
  }

  texture_invalid = false;

  if (qt_solid_spike::qt::qt_texture_widget_frame_next_frame_requested(
          prepared_frame)) {
    q->update();
    qt_solid_spike::qt::window_host_request_wake();
  }

  return true;
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

TexturePaintHostWidget::~TexturePaintHostWidget() {
  Q_D(TexturePaintHostWidget);
  if (d->rhi != nullptr) {
    d->rhi->removeCleanupCallback(this);
  }
  d->release_resources();
}

void TexturePaintHostWidget::bind_rust_widget(std::uint32_t node_id,
                                              std::uint8_t kind_tag) {
  Q_D(TexturePaintHostWidget);
  d->node_id = node_id;
  d->kind_tag = kind_tag;
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
  update();
  qt_solid_spike::qt::window_host_request_wake();
}

bool TexturePaintHostWidget::event(QEvent *event) {
  Q_D(TexturePaintHostWidget);
  switch (event->type()) {
  case QEvent::WindowAboutToChangeInternal:
    d->texture_invalid = true;
    if (d->rhi != nullptr) {
      d->rhi->removeCleanupCallback(this);
      d->release_resources();
      d->rhi = nullptr;
    }
    break;
  case QEvent::Show:
    if (isVisible()) {
      d->sendPaintEvent(QRect(QPoint(0, 0), size()));
    }
    break;
  default:
    break;
  }

  return QWidget::event(event);
}

void TexturePaintHostWidget::paintEvent(QPaintEvent *event) {
  Q_UNUSED(event);

  Q_D(TexturePaintHostWidget);
  if (!updatesEnabled() || d->no_size) {
    return;
  }

  d->ensure_rhi();
  if (d->rhi == nullptr) {
    return;
  }

  const QSize pixel_size =
      (size() * devicePixelRatioF()).expandedTo(QSize(1, 1));
  const auto interop = qt_wgpu_renderer::texture_widget_rhi_interop(d->rhi);
  const bool use_gles_context =
      interop.backend_tag ==
      qt_wgpu_renderer::texture_backend_tag(QRhi::OpenGLES2);
  if (use_gles_context &&
      !qt_wgpu_renderer::prepare_gles_context(interop.gles2.context_object)) {
    qWarning() << "failed to make OpenGL/GLES interop context current for node"
               << d->node_id;
    return;
  }
  try {
    auto prepared_frame = qt_solid_spike::qt::qt_prepare_texture_widget_frame(
        d->node_id, static_cast<std::uint32_t>(pixel_size.width()),
        static_cast<std::uint32_t>(pixel_size.height()),
        static_cast<std::size_t>(pixel_size.width()) * 4, devicePixelRatioF(), interop);
    d->update_prepared_frame(*prepared_frame);
  } catch (const rust::Error &error) {
    if (use_gles_context) {
      qt_wgpu_renderer::done_gles_context(interop.gles2.context_object);
    }
    qWarning() << "failed to prepare texture widget frame for node" << d->node_id
               << ":" << error.what();
    return;
  }
  if (use_gles_context) {
    qt_wgpu_renderer::done_gles_context(interop.gles2.context_object);
  }
}

void TexturePaintHostWidget::resizeEvent(QResizeEvent *event) {
  Q_D(TexturePaintHostWidget);
  if (event->size().isEmpty()) {
    d->no_size = true;
    return;
  }

  d->no_size = false;
  d->sendPaintEvent(QRect(QPoint(0, 0), size()));
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
