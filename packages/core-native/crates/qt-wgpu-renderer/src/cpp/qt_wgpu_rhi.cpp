#include "qt_wgpu_rhi.h"

#include <QtGui/QOffscreenSurface>
#include <QtGui/QOpenGLContext>
#include <rhi/qrhi_platform.h>

#include <memory>
#include <unordered_map>

namespace qt_wgpu_renderer {

namespace {

struct QtWgpuGlesInteropContext {
  std::unique_ptr<QOffscreenSurface> surface;
  std::unique_ptr<QOpenGLContext> shared;
};

QtWgpuGlesInteropContext *gles_context_for(std::uint64_t context_object) {
  return reinterpret_cast<QtWgpuGlesInteropContext *>(context_object);
}

QtWgpuGlesInteropContext *ensure_gles_context(QOpenGLContext *source) {
  if (source == nullptr) {
    return nullptr;
  }

  static std::unordered_map<std::uint64_t,
                            std::unique_ptr<QtWgpuGlesInteropContext>>
      contexts;
  const std::uint64_t key = reinterpret_cast<std::uint64_t>(source);
  auto it = contexts.find(key);
  if (it != contexts.end()) {
    return it->second.get();
  }

  auto interop = std::make_unique<QtWgpuGlesInteropContext>();
  interop->surface = std::make_unique<QOffscreenSurface>();
  interop->surface->setFormat(source->format());
  if (source->screen() != nullptr) {
    interop->surface->setScreen(source->screen());
  }
  interop->surface->create();

  interop->shared = std::make_unique<QOpenGLContext>();
  interop->shared->setFormat(source->format());
  interop->shared->setShareContext(source);
  if (!interop->shared->create()) {
    return nullptr;
  }

  auto *raw = interop.get();
  contexts.emplace(key, std::move(interop));
  return raw;
}

} // namespace

std::optional<QRhiTexture::Format> texture_format_for_tag(
    std::uint8_t format_tag) {
  switch (format_tag) {
  case 1:
    return QRhiTexture::BGRA8;
  case 2:
    return QRhiTexture::RGBA8;
  default:
    return std::nullopt;
  }
}

std::optional<QRhi::Implementation> texture_backend_for_tag(
    std::uint8_t backend_tag) {
  switch (backend_tag) {
  case 1:
    return QRhi::Vulkan;
  case 2:
    return QRhi::OpenGLES2;
  case 3:
    return QRhi::D3D11;
  case 4:
    return QRhi::Metal;
  case 5:
    return QRhi::D3D12;
  default:
    return std::nullopt;
  }
}

std::uint8_t texture_backend_tag(QRhi::Implementation backend) {
  switch (backend) {
  case QRhi::Vulkan:
    return 1;
  case QRhi::OpenGLES2:
    return 2;
  case QRhi::D3D11:
    return 3;
  case QRhi::Metal:
    return 4;
  case QRhi::D3D12:
    return 5;
  default:
    return 0;
  }
}

qt_solid_spike::qt::QtRhiInteropTransport texture_widget_rhi_interop(QRhi *rhi) {
  qt_solid_spike::qt::QtRhiInteropTransport interop{};
  interop.backend_tag = 0;
  interop.metal.device_object = 0;
  interop.metal.command_queue_object = 0;
  interop.vulkan.physical_device_object = 0;
  interop.vulkan.device_object = 0;
  interop.vulkan.queue_family_index = 0;
  interop.vulkan.queue_index = 0;
  interop.d3d11.device_object = 0;
  interop.d3d11.context_object = 0;
  interop.d3d11.adapter_luid_low = 0;
  interop.d3d11.adapter_luid_high = 0;
  interop.d3d12.device_object = 0;
  interop.d3d12.command_queue_object = 0;
  interop.gles2.context_object = 0;
  if (rhi == nullptr) {
    return interop;
  }

  interop.backend_tag = texture_backend_tag(rhi->backend());
  const QRhiNativeHandles *native_handles = rhi->nativeHandles();
  if (native_handles == nullptr) {
    return interop;
  }

  switch (rhi->backend()) {
#if (QT_CONFIG(vulkan) && __has_include(<vulkan/vulkan.h>)) || defined(Q_QDOC)
  case QRhi::Vulkan: {
    const auto *vulkan_handles =
        static_cast<const QRhiVulkanNativeHandles *>(native_handles);
    interop.vulkan.physical_device_object =
        reinterpret_cast<std::uint64_t>(vulkan_handles->physDev);
    interop.vulkan.device_object =
        reinterpret_cast<std::uint64_t>(vulkan_handles->dev);
    interop.vulkan.queue_family_index = vulkan_handles->gfxQueueFamilyIdx;
    interop.vulkan.queue_index = vulkan_handles->gfxQueueIdx;
    return interop;
  }
#endif
  case QRhi::OpenGLES2: {
#if QT_CONFIG(opengl) || defined(Q_QDOC)
    const auto *gles2_handles =
        static_cast<const QRhiGles2NativeHandles *>(native_handles);
    auto *context = ensure_gles_context(gles2_handles->context);
    interop.gles2.context_object = reinterpret_cast<std::uint64_t>(context);
    return interop;
#else
    return interop;
#endif
  }
#if defined(Q_OS_WIN)
  case QRhi::D3D11: {
    const auto *d3d11_handles =
        static_cast<const QRhiD3D11NativeHandles *>(native_handles);
    interop.d3d11.device_object =
        reinterpret_cast<std::uint64_t>(d3d11_handles->dev);
    interop.d3d11.context_object =
        reinterpret_cast<std::uint64_t>(d3d11_handles->context);
    interop.d3d11.adapter_luid_low = d3d11_handles->adapterLuidLow;
    interop.d3d11.adapter_luid_high = d3d11_handles->adapterLuidHigh;
    return interop;
  }
  case QRhi::D3D12: {
    const auto *d3d12_handles =
        static_cast<const QRhiD3D12NativeHandles *>(native_handles);
    interop.d3d12.device_object =
        reinterpret_cast<std::uint64_t>(d3d12_handles->dev);
    interop.d3d12.command_queue_object =
        reinterpret_cast<std::uint64_t>(d3d12_handles->commandQueue);
    return interop;
  }
#endif
#if QT_CONFIG(metal)
  case QRhi::Metal: {
    const auto *metal_handles =
        static_cast<const QRhiMetalNativeHandles *>(native_handles);
    interop.metal.device_object =
        reinterpret_cast<std::uint64_t>(metal_handles->dev);
    interop.metal.command_queue_object =
        reinterpret_cast<std::uint64_t>(metal_handles->cmdQueue);
    return interop;
  }
#endif
  default:
    return interop;
  }
}

bool prepare_gles_context(std::uint64_t context_object) {
  auto *context = gles_context_for(context_object);
  if (context == nullptr || context->shared == nullptr || context->surface == nullptr) {
    return false;
  }
  return context->shared->makeCurrent(context->surface.get());
}

void done_gles_context(std::uint64_t context_object) {
  auto *context = gles_context_for(context_object);
  if (context != nullptr && context->shared != nullptr) {
    context->shared->doneCurrent();
  }
}

} // namespace qt_wgpu_renderer

extern "C" std::uint64_t
qt_wgpu_gles_get_proc_address(std::uint64_t context_object,
                              const char *name) noexcept {
  auto *context = qt_wgpu_renderer::gles_context_for(context_object);
  if (context == nullptr || context->shared == nullptr || name == nullptr) {
    return 0;
  }
  return reinterpret_cast<std::uint64_t>(
      context->shared->getProcAddress(QByteArray(name)));
}
