use crate::{
    context::{QtWgpuContext, QtWgpuContextHandle, QtWgpuTextureEntry, TextureCacheKey},
    error::{QtWgpuRendererError, Result},
    lease::{QtRhiD3d11InteropInfo, QtRhiD3d12InteropInfo},
};

pub fn load_or_create_context_d3d11(
    _rhi_interop: QtRhiD3d11InteropInfo,
) -> Result<QtWgpuContextHandle> {
    Err(QtWgpuRendererError::new(
        "qt-wgpu-renderer D3D11 shared interop is only available on Windows",
    ))
}

pub fn load_or_create_context_d3d12(
    _rhi_interop: QtRhiD3d12InteropInfo,
) -> Result<QtWgpuContextHandle> {
    Err(QtWgpuRendererError::new(
        "qt-wgpu-renderer D3D12 interop is only available on Windows",
    ))
}

pub fn cached_rgba8_texture_entry_d3d12<'a>(
    _context: &'a mut QtWgpuContext,
    _rhi_interop: QtRhiD3d12InteropInfo,
    _key: TextureCacheKey,
    _label: &'static str,
) -> Result<&'a QtWgpuTextureEntry> {
    Err(QtWgpuRendererError::new(
        "qt-wgpu-renderer D3D12 textures are only available on Windows",
    ))
}

pub fn cached_rgba8_texture_entry_d3d11<'a>(
    _context: &'a mut QtWgpuContext,
    _rhi_interop: QtRhiD3d11InteropInfo,
    _key: TextureCacheKey,
    _label: &'static str,
) -> Result<&'a QtWgpuTextureEntry> {
    Err(QtWgpuRendererError::new(
        "qt-wgpu-renderer D3D11 shared textures are only available on Windows",
    ))
}

pub fn finish_texture_render_d3d12(
    _context: &QtWgpuContext,
    _texture: &wgpu::Texture,
) -> Result<()> {
    Err(QtWgpuRendererError::new(
        "qt-wgpu-renderer D3D12 sync is only available on Windows",
    ))
}

pub fn finish_texture_render_d3d11(_context: &QtWgpuContext) -> Result<()> {
    Err(QtWgpuRendererError::new(
        "qt-wgpu-renderer D3D11 shared sync is only available on Windows",
    ))
}
