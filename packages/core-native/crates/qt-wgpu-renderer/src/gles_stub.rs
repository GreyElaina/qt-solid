use crate::{
    context::{QtWgpuContext, QtWgpuContextHandle, QtWgpuTextureEntry, TextureCacheKey},
    error::{QtWgpuRendererError, Result},
    lease::QtRhiGles2InteropInfo,
};

pub fn load_or_create_context(_rhi_interop: QtRhiGles2InteropInfo) -> Result<QtWgpuContextHandle> {
    Err(QtWgpuRendererError::new(
        "qt-wgpu-renderer OpenGL/GLES interop is not available on this target",
    ))
}

pub fn cached_rgba8_texture_entry<'a>(
    _context: &'a mut QtWgpuContext,
    _key: TextureCacheKey,
    _label: &'static str,
) -> Result<&'a QtWgpuTextureEntry> {
    Err(QtWgpuRendererError::new(
        "qt-wgpu-renderer OpenGL/GLES textures are not available on this target",
    ))
}
