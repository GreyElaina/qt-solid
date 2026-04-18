use std::{any::Any, fmt, sync::Arc};

pub const QT_RHI_BACKEND_VULKAN: u8 = 1;
pub const QT_RHI_BACKEND_OPENGLES2: u8 = 2;
pub const QT_RHI_BACKEND_D3D11: u8 = 3;
pub const QT_RHI_BACKEND_METAL: u8 = 4;
pub const QT_RHI_BACKEND_D3D12: u8 = 5;

pub const QT_TEXTURE_FORMAT_BGRA8_UNORM_PREMULTIPLIED: u8 = 1;
pub const QT_TEXTURE_FORMAT_RGBA8_UNORM: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QtRhiMetalInteropInfo {
    pub device_object: u64,
    pub command_queue_object: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QtRhiVulkanInteropInfo {
    pub physical_device_object: u64,
    pub device_object: u64,
    pub queue_family_index: u32,
    pub queue_index: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QtRhiD3d11InteropInfo {
    pub device_object: u64,
    pub context_object: u64,
    pub adapter_luid_low: u32,
    pub adapter_luid_high: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QtRhiD3d12InteropInfo {
    pub device_object: u64,
    pub command_queue_object: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QtRhiGles2InteropInfo {
    pub context_object: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QtRhiInteropInfo {
    Metal(QtRhiMetalInteropInfo),
    Vulkan(QtRhiVulkanInteropInfo),
    D3d11(QtRhiD3d11InteropInfo),
    D3d12(QtRhiD3d12InteropInfo),
    OpenGles2(QtRhiGles2InteropInfo),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QtNativeTextureLeaseInfo {
    pub backend_tag: u8,
    pub format_tag: u8,
    pub width_px: u32,
    pub height_px: u32,
    pub object: u64,
    pub layout: i32,
}

#[derive(Clone)]
pub struct QtNativeTextureLease {
    info: QtNativeTextureLeaseInfo,
    owner: Arc<dyn Any + Send + Sync>,
}

impl fmt::Debug for QtNativeTextureLease {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QtNativeTextureLease")
            .field("info", &self.info)
            .finish_non_exhaustive()
    }
}

impl QtNativeTextureLease {
    pub fn new(info: QtNativeTextureLeaseInfo, owner: Arc<dyn Any + Send + Sync>) -> Self {
        Self { info, owner }
    }

    pub fn info(&self) -> QtNativeTextureLeaseInfo {
        let owner_type_id = self.owner.as_ref().type_id();
        std::hint::black_box(owner_type_id);
        self.info
    }
}
