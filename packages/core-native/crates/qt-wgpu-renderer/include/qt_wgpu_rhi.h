#pragma once

#include "native/src/qt/ffi.rs.h"

#include <QtGui/QImage>
#include <rhi/qrhi.h>

#include <cstddef>
#include <cstdint>
#include <optional>

namespace qt_wgpu_renderer {

std::optional<QRhiTexture::Format> texture_format_for_tag(
    std::uint8_t format_tag);

std::optional<QImage::Format> image_format_for_tag(std::uint8_t format_tag);

std::optional<QRhi::Implementation> texture_backend_for_tag(
    std::uint8_t backend_tag);

std::uint8_t texture_backend_tag(QRhi::Implementation backend);

qt_solid_spike::qt::QtRhiInteropTransport texture_widget_rhi_interop(QRhi *rhi);

bool prepare_gles_context(std::uint64_t context_object);
void done_gles_context(std::uint64_t context_object);

extern "C" std::uint64_t
qt_wgpu_gles_get_proc_address(std::uint64_t context_object,
                              const char *name) noexcept;

} // namespace qt_wgpu_renderer
