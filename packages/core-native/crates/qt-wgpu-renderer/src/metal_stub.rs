use crate::{
    QtRhiMetalInteropInfo, QtWgpuContext, QtWgpuContextHandle, QtWgpuRendererError,
    QtWgpuTextureEntry, Result, TextureCacheKey,
};

pub fn load_or_create_context(rhi_interop: QtRhiMetalInteropInfo) -> Result<QtWgpuContextHandle> {
    let _ = rhi_interop;
    Err(QtWgpuRendererError::new(
        "qt-wgpu-renderer does not implement Metal interop on this platform",
    ))
}

pub fn cached_rgba8_texture_entry<'a>(
    context: &'a mut QtWgpuContext,
    key: TextureCacheKey,
    label: &'static str,
) -> Result<&'a QtWgpuTextureEntry> {
    let _ = (context, key, label);
    Err(QtWgpuRendererError::new(
        "qt-wgpu-renderer does not implement Metal texture caching on this platform",
    ))
}
