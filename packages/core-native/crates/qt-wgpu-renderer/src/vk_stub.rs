use crate::{
    context::{QtWgpuContext, QtWgpuContextHandle, QtWgpuTextureEntry, TextureCacheKey},
    error::{QtWgpuRendererError, Result},
    lease::QtRhiVulkanInteropInfo,
};

pub fn load_or_create_context(_rhi_interop: QtRhiVulkanInteropInfo) -> Result<QtWgpuContextHandle> {
    Err(QtWgpuRendererError::new(
        "qt-wgpu-renderer Vulkan interop is not available on this target",
    ))
}

pub fn cached_rgba8_texture_entry<'a>(
    _context: &'a mut QtWgpuContext,
    _key: TextureCacheKey,
    _label: &'static str,
) -> Result<&'a QtWgpuTextureEntry> {
    Err(QtWgpuRendererError::new(
        "qt-wgpu-renderer Vulkan textures are not available on this target",
    ))
}

