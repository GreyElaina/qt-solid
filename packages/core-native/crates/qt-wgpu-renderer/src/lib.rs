mod context;
#[cfg(any(
    target_os = "windows",
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd"
))]
pub mod gles;
#[cfg(not(any(
    target_os = "windows",
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd"
)))]
#[path = "gles_stub.rs"]
pub mod gles;
mod error;
mod lease;
#[cfg(target_os = "windows")]
pub mod d3d;
#[cfg(not(target_os = "windows"))]
#[path = "d3d_stub.rs"]
pub mod d3d;
#[cfg(target_os = "macos")]
pub mod metal;
#[cfg(not(target_os = "macos"))]
#[path = "metal_stub.rs"]
pub mod metal;
#[cfg(any(
    target_os = "windows",
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_vendor = "apple"
))]
pub mod vk;
#[cfg(not(any(
    target_os = "windows",
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_vendor = "apple"
)))]
#[path = "vk_stub.rs"]
pub mod vk;

pub use context::{
    QtWgpuContext, QtWgpuContextHandle, QtWgpuTextureEntry, TextureCacheKey, load_or_create_context,
};
pub use error::{QtWgpuRendererError, Result};
pub use lease::{
    QT_RHI_BACKEND_D3D11, QT_RHI_BACKEND_D3D12, QT_RHI_BACKEND_METAL, QT_RHI_BACKEND_OPENGLES2,
    QT_RHI_BACKEND_VULKAN, QT_TEXTURE_FORMAT_BGRA8_UNORM_PREMULTIPLIED,
    QT_TEXTURE_FORMAT_RGBA8_UNORM, QtNativeTextureLease, QtNativeTextureLeaseInfo,
    QtRhiD3d11InteropInfo, QtRhiD3d12InteropInfo, QtRhiInteropInfo, QtRhiMetalInteropInfo,
    QtRhiVulkanInteropInfo, QtRhiGles2InteropInfo,
};
