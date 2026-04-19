#include "qt_wgpu_platform.h"

#include "texture_paint_host_widget.h"
#include "native/src/qt/ffi.rs.h"

#include <QtCore/QByteArray>
#include <QtCore/QCborArray>
#include <QtCore/QCborMap>
#include <QtCore/QCborValue>
#include <QtCore/QDebug>
#include <QtCore/QPointer>
#include <QtCore/QVariant>
#include <QtGui/QGuiApplication>
#include <QtGui/QImage>
#include <QtGui/QOpenGLContext>
#include <QtGui/QPaintDevice>
#include <QtGui/QPlatformSurfaceEvent>
#include <QtGui/QWindow>
#include <QtWidgets/QWidget>
#include <qpa/qplatformbackingstore.h>
#include <qpa/qplatformintegration.h>
#include <qpa/qplatformintegrationplugin.h>
#include <qpa/qplatformnativeinterface.h>

#include "qplatformintegrationfactory_p.h"
#include "qplugin_p.h"

#if defined(Q_OS_LINUX)
#include "qplatformwindow_p.h"
#endif

#include <memory>
#include <cstdint>
#include <mutex>
#include <optional>
#include <utility>

namespace qt_wgpu_renderer {

namespace {

constexpr char kRootNodeIdProperty[] = "_qt_solid_root_node_id";
constexpr char kUnifiedCompositorActiveProperty[] =
    "_qt_solid_wgpu_compositor_active";
constexpr char kCocoaPlatformKey[] = "cocoax";
constexpr char kWindowsPlatformKey[] = "windowsx";
constexpr char kXcbPlatformKey[] = "xcbx";
constexpr char kWaylandPlatformKey[] = "waylandx";

enum class LinuxBackendKind {
  X11,
  Wayland,
};

struct PlatformPluginDescriptor {
  const char *wrapper_key;
  const char *delegate_key;
};

struct BackingStorePixels {
  std::uint32_t width_px = 0;
  std::uint32_t height_px = 0;
  std::size_t stride = 0;
  rust::Slice<const std::uint8_t> bytes =
      rust::Slice<const std::uint8_t>(nullptr, 0);
  QImage image;
};

bool is_supported_texture_source(QWidget *widget) {
  return widget == nullptr || dynamic_cast<TexturePaintHostWidget *>(widget) != nullptr;
}

rust::Vec<qt_solid_spike::qt::QtRect>
collect_scaled_dirty_rects(const QRegion &region, const QPoint &offset,
                           qreal source_device_pixel_ratio,
                           qreal source_transform_factor) {
  const qreal transform_factor =
      source_transform_factor != 0.0 ? source_transform_factor
                                     : source_device_pixel_ratio;
  rust::Vec<qt_solid_spike::qt::QtRect> dirty_rects;
  for (const QRect &rect : region) {
    const QRect scaled_rect(rect.translated(offset).topLeft() * transform_factor,
                            rect.size() * transform_factor);
    if (scaled_rect.isEmpty()) {
      continue;
    }
    dirty_rects.push_back(qt_solid_spike::qt::QtRect{
        scaled_rect.x(),
        scaled_rect.y(),
        scaled_rect.width(),
        scaled_rect.height(),
    });
  }
  return dirty_rects;
}

std::optional<BackingStorePixels>
read_backingstore_pixels(QPlatformBackingStore *delegate) {
  QImage image = delegate->toImage();
  if (image.isNull()) {
    return std::nullopt;
  }
  if (image.format() != QImage::Format_ARGB32_Premultiplied) {
    image = image.convertToFormat(QImage::Format_ARGB32_Premultiplied);
  }

  BackingStorePixels pixels;
  pixels.width_px = static_cast<std::uint32_t>(image.width());
  pixels.height_px = static_cast<std::uint32_t>(image.height());
  pixels.stride = static_cast<std::size_t>(image.bytesPerLine());
  pixels.bytes = rust::Slice<const std::uint8_t>(
      reinterpret_cast<const std::uint8_t *>(image.constBits()),
      static_cast<std::size_t>(image.sizeInBytes()));
  pixels.image = std::move(image);
  return pixels;
}

#if defined(Q_OS_LINUX)
std::optional<LinuxBackendKind> parse_linux_backend_kind(const QString &value) {
  QString normalized = value;
  const qsizetype colon = normalized.indexOf(u':');
  if (colon >= 0) {
    normalized.truncate(colon);
  }

  if (normalized.startsWith(u"wayland", Qt::CaseInsensitive)) {
    return LinuxBackendKind::Wayland;
  }
  if (normalized.startsWith(u"xcb", Qt::CaseInsensitive) ||
      normalized.startsWith(u"x11", Qt::CaseInsensitive)) {
    return LinuxBackendKind::X11;
  }
  return std::nullopt;
}

const char *linux_platform_key_for_backend(LinuxBackendKind backend) {
  switch (backend) {
  case LinuxBackendKind::X11:
#if QT_CONFIG(xcb)
    return kXcbPlatformKey;
#elif QT_CONFIG(wayland)
    return kWaylandPlatformKey;
#else
    return nullptr;
#endif
  case LinuxBackendKind::Wayland:
#if QT_CONFIG(wayland)
    return kWaylandPlatformKey;
#elif QT_CONFIG(xcb)
    return kXcbPlatformKey;
#else
    return nullptr;
#endif
  }

  return nullptr;
}

const char *selected_linux_platform_key() {
  if (const auto configured =
          parse_linux_backend_kind(qEnvironmentVariable("QT_QPA_PLATFORM"));
      configured.has_value()) {
    return linux_platform_key_for_backend(*configured);
  }

  if (qEnvironmentVariableIsSet("WAYLAND_DISPLAY")) {
    return linux_platform_key_for_backend(LinuxBackendKind::Wayland);
  }

  return linux_platform_key_for_backend(LinuxBackendKind::X11);
}
#endif

const char *selected_unified_platform_key() {
#if defined(Q_OS_MACOS)
  return kCocoaPlatformKey;
#elif defined(Q_OS_WIN)
  return kWindowsPlatformKey;
#elif defined(Q_OS_LINUX)
  return selected_linux_platform_key();
#else
  return nullptr;
#endif
}

void append_registered_platform_keys(QCborArray &keys) {
#if defined(Q_OS_MACOS)
  keys.append(QStringLiteral("cocoax"));
#elif defined(Q_OS_WIN)
  keys.append(QStringLiteral("windowsx"));
#elif defined(Q_OS_LINUX)
#if QT_CONFIG(xcb)
  keys.append(QStringLiteral("xcbx"));
#endif
#if QT_CONFIG(wayland)
  keys.append(QStringLiteral("waylandx"));
#endif
#endif
}

std::optional<PlatformPluginDescriptor> resolve_plugin_descriptor(
    const QString &system) {
#if defined(Q_OS_MACOS)
  if (system.compare(QLatin1StringView(kCocoaPlatformKey), Qt::CaseInsensitive) == 0) {
    return PlatformPluginDescriptor{kCocoaPlatformKey, "cocoa"};
  }
#elif defined(Q_OS_WIN)
  if (system.compare(QLatin1StringView(kWindowsPlatformKey), Qt::CaseInsensitive) ==
      0) {
    return PlatformPluginDescriptor{kWindowsPlatformKey, "windows"};
  }
#elif defined(Q_OS_LINUX)
#if QT_CONFIG(xcb)
  if (system.compare(QLatin1StringView(kXcbPlatformKey), Qt::CaseInsensitive) == 0) {
    return PlatformPluginDescriptor{kXcbPlatformKey, "xcb"};
  }
#endif
#if QT_CONFIG(wayland)
  if (system.compare(QLatin1StringView(kWaylandPlatformKey), Qt::CaseInsensitive) ==
      0) {
    return PlatformPluginDescriptor{kWaylandPlatformKey, "wayland"};
  }
#endif
#else
  Q_UNUSED(system);
#endif

  return std::nullopt;
}

bool is_unified_platform_name(const QString &platform_name) {
  if (platform_name.compare(QLatin1StringView(kCocoaPlatformKey), Qt::CaseInsensitive) ==
          0 ||
      platform_name.compare(QLatin1StringView(kWindowsPlatformKey),
                            Qt::CaseInsensitive) == 0 ||
      platform_name.compare(QLatin1StringView(kXcbPlatformKey), Qt::CaseInsensitive) ==
          0 ||
      platform_name.compare(QLatin1StringView(kWaylandPlatformKey),
                            Qt::CaseInsensitive) == 0) {
    return true;
  }

  return false;
}

#if defined(Q_OS_MACOS)
std::optional<qt_solid_spike::qt::QtCompositorTarget>
resolve_macos_target(QWindow *window, std::uint32_t width_px,
                     std::uint32_t height_px, qreal scale_factor) {
  auto *native_interface = QGuiApplication::platformNativeInterface();
  void *ns_view = native_interface != nullptr
                      ? native_interface->nativeResourceForWindow("nsview", window)
                      : nullptr;
  if (ns_view == nullptr) {
    qWarning("qt wgpu compositor failed to resolve NSView for top-level window");
    return std::nullopt;
  }

  return qt_solid_spike::qt::QtCompositorTarget{
      qt_solid_spike::qt::QtCompositorSurfaceKind::AppKitNsView,
      reinterpret_cast<std::uint64_t>(ns_view),
      0,
      width_px,
      height_px,
      scale_factor,
  };
}
#endif

#if defined(Q_OS_WIN)
std::optional<qt_solid_spike::qt::QtCompositorTarget>
resolve_windows_target(QWindow *window, std::uint32_t width_px,
                       std::uint32_t height_px, qreal scale_factor) {
  auto *native_interface = QGuiApplication::platformNativeInterface();
  void *native_handle = native_interface != nullptr
                            ? native_interface->nativeResourceForWindow("handle", window)
                            : nullptr;
  const quintptr hwnd_value = native_handle != nullptr
                                  ? reinterpret_cast<quintptr>(native_handle)
                                  : static_cast<quintptr>(window->winId());
  if (hwnd_value == 0) {
    qWarning("qt wgpu compositor failed to resolve HWND for top-level window");
    return std::nullopt;
  }

  return qt_solid_spike::qt::QtCompositorTarget{
      qt_solid_spike::qt::QtCompositorSurfaceKind::Win32Hwnd,
      static_cast<std::uint64_t>(hwnd_value),
      0,
      width_px,
      height_px,
      scale_factor,
  };
}
#endif

#if defined(Q_OS_LINUX)
std::optional<qt_solid_spike::qt::QtCompositorTarget>
resolve_x11_target(QWindow *window, std::uint32_t width_px,
                   std::uint32_t height_px, qreal scale_factor) {
#if QT_CONFIG(xcb)
  auto *app = qGuiApp;
  if (app == nullptr) {
    return std::nullopt;
  }

  auto *native = app->nativeInterface<QNativeInterface::QX11Application>();
  if (native == nullptr) {
    qWarning("qt wgpu compositor failed to resolve X11 native interface");
    return std::nullopt;
  }

  auto *connection = native->connection();
  const WId window_id = window->winId();
  if (connection == nullptr || window_id == 0) {
    qWarning("qt wgpu compositor failed to resolve X11 window handles");
    return std::nullopt;
  }

  return qt_solid_spike::qt::QtCompositorTarget{
      qt_solid_spike::qt::QtCompositorSurfaceKind::XcbWindow,
      static_cast<std::uint64_t>(window_id),
      reinterpret_cast<std::uint64_t>(connection),
      width_px,
      height_px,
      scale_factor,
  };
#else
  Q_UNUSED(window);
  Q_UNUSED(width_px);
  Q_UNUSED(height_px);
  Q_UNUSED(scale_factor);
  return std::nullopt;
#endif
}

std::optional<qt_solid_spike::qt::QtCompositorTarget>
resolve_wayland_target(QWindow *window, std::uint32_t width_px,
                       std::uint32_t height_px, qreal scale_factor) {
#if QT_CONFIG(wayland)
  auto *app = qGuiApp;
  if (app == nullptr) {
    return std::nullopt;
  }

  auto *display_native = app->nativeInterface<QNativeInterface::QWaylandApplication>();
  if (display_native == nullptr) {
    qWarning("qt wgpu compositor failed to resolve Wayland application interface");
    return std::nullopt;
  }

  auto *wayland_window =
      window->nativeInterface<QNativeInterface::Private::QWaylandWindow>();
  if (wayland_window == nullptr) {
    qWarning("qt wgpu compositor failed to resolve Wayland window interface");
    return std::nullopt;
  }

  auto *display = display_native->display();
  auto *surface = wayland_window->surface();
  if (display == nullptr || surface == nullptr) {
    qWarning("qt wgpu compositor failed to resolve Wayland surface handles");
    return std::nullopt;
  }

  return qt_solid_spike::qt::QtCompositorTarget{
      qt_solid_spike::qt::QtCompositorSurfaceKind::WaylandSurface,
      reinterpret_cast<std::uint64_t>(surface),
      reinterpret_cast<std::uint64_t>(display),
      width_px,
      height_px,
      scale_factor,
  };
#else
  Q_UNUSED(window);
  Q_UNUSED(width_px);
  Q_UNUSED(height_px);
  Q_UNUSED(scale_factor);
  return std::nullopt;
#endif
}

std::optional<qt_solid_spike::qt::QtCompositorTarget>
resolve_linux_target(QWindow *window, std::uint32_t width_px,
                     std::uint32_t height_px, qreal scale_factor) {
  const auto backend = parse_linux_backend_kind(QGuiApplication::platformName());
  if (!backend.has_value()) {
    qWarning("qt wgpu compositor could not classify Linux QPA backend");
    return std::nullopt;
  }

  switch (*backend) {
  case LinuxBackendKind::X11:
    return resolve_x11_target(window, width_px, height_px, scale_factor);
  case LinuxBackendKind::Wayland:
    return resolve_wayland_target(window, width_px, height_px, scale_factor);
  }

  return std::nullopt;
}
#endif

std::optional<qt_solid_spike::qt::QtCompositorTarget>
resolve_compositor_target(QWindow *window, std::uint32_t width_px,
                          std::uint32_t height_px, qreal scale_factor) {
#if defined(Q_OS_MACOS)
  return resolve_macos_target(window, width_px, height_px, scale_factor);
#elif defined(Q_OS_WIN)
  return resolve_windows_target(window, width_px, height_px, scale_factor);
#elif defined(Q_OS_LINUX)
  return resolve_linux_target(window, width_px, height_px, scale_factor);
#else
  Q_UNUSED(window);
  Q_UNUSED(width_px);
  Q_UNUSED(height_px);
  Q_UNUSED(scale_factor);
  return std::nullopt;
#endif
}

class QtWgpuBackingStore final : public QPlatformBackingStore {
public:
  QtWgpuBackingStore(QWindow *window,
                     std::unique_ptr<QPlatformBackingStore> delegate)
      : QPlatformBackingStore(window), delegate_(std::move(delegate)) {}

  QPaintDevice *paintDevice() override { return delegate_->paintDevice(); }

  void flush(QWindow *window, const QRegion &region,
             const QPoint &offset) override {
    delegate_->flush(window, region, offset);
  }

  FlushResult rhiFlush(QWindow *window, qreal source_device_pixel_ratio,
                       const QRegion &region, const QPoint &offset,
                       QPlatformTextureList *textures,
                       bool translucent_background,
                       qreal source_transform_factor) override {
    Q_UNUSED(translucent_background);

    if (!unified_compositor_active()) {
      return delegate_->rhiFlush(window, source_device_pixel_ratio, region, offset,
                                 textures, translucent_background,
                                 source_transform_factor);
    }

    const QVariant node_id_value = window->property(kRootNodeIdProperty);
    if (!node_id_value.isValid()) {
      return delegate_->rhiFlush(window, source_device_pixel_ratio, region, offset,
                                 textures, translucent_background,
                                 source_transform_factor);
    }

    if (textures != nullptr) {
      for (int index = 0; index < textures->count(); ++index) {
        auto *widget = static_cast<QWidget *>(textures->source(index));
        if (!is_supported_texture_source(widget)) {
          qWarning()
              << "qt wgpu compositor does not support mixed render-to-texture widgets:"
              << widget->metaObject()->className();
          return FlushFailed;
        }
      }
    }

    try {
      auto base_dirty_rects = collect_scaled_dirty_rects(
          region, offset, source_device_pixel_ratio, source_transform_factor);
      const auto present_plan = qt_solid_spike::qt::qt_plan_present_window_with_wgpu(
          node_id_value.toUInt(), base_dirty_rects);
      if (!present_plan.must_present) {
        return FlushSuccess;
      }
      rust::Slice<const std::uint8_t> base_bytes(nullptr, 0);
      std::size_t base_stride = present_plan.cached_stride;
      std::uint32_t target_width_px = present_plan.cached_width_px;
      std::uint32_t target_height_px = present_plan.cached_height_px;
      if (present_plan.needs_base_upload) {
        const auto pixels = read_backingstore_pixels(delegate_.get());
        if (!pixels.has_value()) {
          qWarning("qt wgpu compositor received null backingstore pixels");
          return FlushFailed;
        }
        base_bytes = pixels->bytes;
        base_stride = pixels->stride;
        target_width_px = pixels->width_px;
        target_height_px = pixels->height_px;
      } else if (target_width_px == 0 || target_height_px == 0) {
        qWarning("qt wgpu compositor missing cached surface size for texture-only flush");
        return FlushFailed;
      }
      const auto target = resolve_compositor_target(window, target_width_px,
                                                    target_height_px,
                                                    source_device_pixel_ratio);
      if (!target.has_value()) {
        qWarning("qt wgpu compositor failed to resolve raw handles for platform");
        return FlushFailed;
      }
      if (qt_solid_spike::qt::qt_present_window_with_wgpu(
              node_id_value.toUInt(), *target, base_stride,
              source_device_pixel_ratio, present_plan.needs_base_upload,
              std::move(base_dirty_rects),
              base_bytes)) {
        return FlushSuccess;
      }
    } catch (const rust::Error &error) {
      qWarning() << "qt wgpu compositor present failed:" << error.what();
      return FlushFailed;
    }

    return FlushFailed;
  }

  QImage toImage() const override { return delegate_->toImage(); }

  QPlatformGraphicsBuffer *graphicsBuffer() const override {
    return delegate_->graphicsBuffer();
  }

  void resize(const QSize &size, const QRegion &static_contents) override {
    delegate_->resize(size, static_contents);
  }

  bool scroll(const QRegion &area, int dx, int dy) override {
    return delegate_->scroll(area, dx, dy);
  }

  void beginPaint(const QRegion &region) override { delegate_->beginPaint(region); }

  void endPaint() override { delegate_->endPaint(); }

private:
  std::unique_ptr<QPlatformBackingStore> delegate_;
};

class QtWgpuIntegration final : public QPlatformIntegration {
public:
  explicit QtWgpuIntegration(std::unique_ptr<QPlatformIntegration> delegate)
      : delegate_(std::move(delegate)) {}

  bool hasCapability(Capability cap) const override {
    return delegate_->hasCapability(cap);
  }

  QPlatformPixmap *createPlatformPixmap(
      QPlatformPixmap::PixelType type) const override {
    return delegate_->createPlatformPixmap(type);
  }

  QPlatformWindow *createPlatformWindow(QWindow *window) const override {
    return delegate_->createPlatformWindow(window);
  }

  QPlatformWindow *createForeignWindow(QWindow *window,
                                       WId native_handle) const override {
    return delegate_->createForeignWindow(window, native_handle);
  }

  QPlatformBackingStore *createPlatformBackingStore(QWindow *window) const override {
    auto delegate = std::unique_ptr<QPlatformBackingStore>(
        delegate_->createPlatformBackingStore(window));
    if (!delegate) {
      return nullptr;
    }
    return new QtWgpuBackingStore(window, std::move(delegate));
  }

#ifndef QT_NO_OPENGL
  QPlatformOpenGLContext *createPlatformOpenGLContext(
      QOpenGLContext *context) const override {
    return delegate_->createPlatformOpenGLContext(context);
  }

  QOpenGLContext::OpenGLModuleType openGLModuleType() override {
    return delegate_->openGLModuleType();
  }
#endif

  QAbstractEventDispatcher *createEventDispatcher() const override {
    return delegate_->createEventDispatcher();
  }

  void initialize() override { delegate_->initialize(); }

  void destroy() override { delegate_->destroy(); }

  QPlatformFontDatabase *fontDatabase() const override {
    return delegate_->fontDatabase();
  }

#ifndef QT_NO_CLIPBOARD
  QPlatformClipboard *clipboard() const override {
    return delegate_->clipboard();
  }
#endif

#if QT_CONFIG(draganddrop)
  QPlatformDrag *drag() const override { return delegate_->drag(); }
#endif

  QPlatformInputContext *inputContext() const override {
    return delegate_->inputContext();
  }

#if QT_CONFIG(accessibility)
  QPlatformAccessibility *accessibility() const override {
    return delegate_->accessibility();
  }
#endif

  QPlatformNativeInterface *nativeInterface() const override {
    return delegate_->nativeInterface();
  }

  QPlatformServices *services() const override { return delegate_->services(); }

  QVariant styleHint(StyleHint hint) const override {
    return delegate_->styleHint(hint);
  }

  Qt::WindowState defaultWindowState(Qt::WindowFlags flags) const override {
    return delegate_->defaultWindowState(flags);
  }

  QPlatformKeyMapper *keyMapper() const override {
    return delegate_->keyMapper();
  }

  QStringList themeNames() const override { return delegate_->themeNames(); }

  QPlatformTheme *createPlatformTheme(const QString &name) const override {
    return delegate_->createPlatformTheme(name);
  }

  QPlatformOffscreenSurface *createPlatformOffscreenSurface(
      QOffscreenSurface *surface) const override {
    return delegate_->createPlatformOffscreenSurface(surface);
  }

#ifndef QT_NO_SESSIONMANAGER
  QPlatformSessionManager *createPlatformSessionManager(
      const QString &id, const QString &key) const override {
    return delegate_->createPlatformSessionManager(id, key);
  }
#endif

  void sync() override { delegate_->sync(); }

  void setApplicationIcon(const QIcon &icon) const override {
    delegate_->setApplicationIcon(icon);
  }

  void setApplicationBadge(qint64 number) override {
    delegate_->setApplicationBadge(number);
  }

  void beep() const override { delegate_->beep(); }

  void quit() const override { delegate_->quit(); }

#if QT_CONFIG(vulkan)
  QPlatformVulkanInstance *createPlatformVulkanInstance(
      QVulkanInstance *instance) const override {
    return delegate_->createPlatformVulkanInstance(instance);
  }
#endif

private:
  std::unique_ptr<QPlatformIntegration> delegate_;
};

class QtWgpuIntegrationPlugin final : public QPlatformIntegrationPlugin {
public:
  QPlatformIntegration *create(const QString &system, const QStringList &param_list,
                               int &argc, char **argv) override {
    const auto descriptor = resolve_plugin_descriptor(system);
    if (!descriptor.has_value()) {
      return nullptr;
    }

    auto delegate = std::unique_ptr<QPlatformIntegration>(
        QPlatformIntegrationFactory::create(descriptor->delegate_key, param_list, argc,
                                            argv));
    if (!delegate) {
      return nullptr;
    }

    return new QtWgpuIntegration(std::move(delegate));
  }
};

QObject *qt_wgpu_plugin_instance() {
  static QPointer<QObject> instance;
  if (instance.isNull()) {
    instance = new QtWgpuIntegrationPlugin();
  }
  return instance;
}

QPluginMetaData qt_wgpu_plugin_metadata() {
  static const QByteArray data = [] {
    QCborMap top_level;
    top_level.insert(int(QtPluginMetaDataKeys::IID),
                     QString::fromLatin1(QPlatformIntegrationFactoryInterface_iid));
    top_level.insert(int(QtPluginMetaDataKeys::ClassName),
                     QStringLiteral("QtWgpuIntegrationPlugin"));

    QCborMap meta_data;
    QCborArray keys;
    append_registered_platform_keys(keys);
    meta_data.insert(QStringLiteral("Keys"), keys);
    top_level.insert(int(QtPluginMetaDataKeys::MetaData), meta_data);

    QPluginMetaData::Header header;
    QByteArray payload(reinterpret_cast<const char *>(&header),
                       sizeof(QPluginMetaData::Header));
    payload.append(QCborValue(top_level).toCbor());
    return payload;
  }();

  return {data.constData(), static_cast<std::size_t>(data.size())};
}

} // namespace

void register_static_platform_plugins() {
  static std::once_flag once;
  std::call_once(once, []() {
#if defined(Q_OS_MACOS) || defined(Q_OS_WIN) || defined(Q_OS_LINUX)
    qRegisterStaticPluginFunction(
        QStaticPlugin(qt_wgpu_plugin_instance, qt_wgpu_plugin_metadata));
#endif
  });
}

bool unified_compositor_requested() {
#if defined(Q_OS_MACOS)
  return true;
#else
  return qEnvironmentVariableIntValue("QT_SOLID_WGPU_COMPOSITOR") != 0;
#endif
}

void configure_unified_compositor_platform() {
  if (!unified_compositor_requested() || !qEnvironmentVariableIsEmpty("QT_QPA_PLATFORM")) {
    return;
  }

  const char *platform_key = selected_unified_platform_key();
  if (platform_key == nullptr) {
    return;
  }

  qputenv("QT_QPA_PLATFORM", QByteArray(platform_key));
}

void sync_unified_compositor_active_state() {
  if (qApp == nullptr) {
    return;
  }

  qApp->setProperty(kUnifiedCompositorActiveProperty,
                    is_unified_platform_name(QGuiApplication::platformName()));
}

bool unified_compositor_active() {
  if (qApp != nullptr) {
    return qApp->property(kUnifiedCompositorActiveProperty).toBool();
  }
  return unified_compositor_requested();
}

} // namespace qt_wgpu_renderer
