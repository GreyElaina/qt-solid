use std::{
    any::{Any, TypeId},
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{
    d3d,
    error::Result,
    gles,
    lease::{QtNativeTextureLease, QtRhiInteropInfo},
    metal, vk,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextureCacheKey {
    pub slot_id: u64,
    pub width_px: u32,
    pub height_px: u32,
    format: wgpu::TextureFormat,
    usage_bits: u32,
}

impl TextureCacheKey {
    pub fn new(
        slot_id: u64,
        width_px: u32,
        height_px: u32,
        format: wgpu::TextureFormat,
        usage: wgpu::TextureUsages,
    ) -> Self {
        Self {
            slot_id,
            width_px,
            height_px,
            format,
            usage_bits: usage.bits(),
        }
    }

    pub fn rgba8_storage(slot_id: u64, width_px: u32, height_px: u32) -> Self {
        Self::new(
            slot_id,
            width_px,
            height_px,
            wgpu::TextureFormat::Rgba8Unorm,
            wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
        )
    }

    pub(crate) fn format(self) -> wgpu::TextureFormat {
        self.format
    }

    pub(crate) fn usage(self) -> wgpu::TextureUsages {
        wgpu::TextureUsages::from_bits_retain(self.usage_bits)
    }
}

#[derive(Debug)]
pub struct QtWgpuTextureEntry {
    lease: QtNativeTextureLease,
    view: wgpu::TextureView,
}

impl QtWgpuTextureEntry {
    pub(crate) fn new(lease: QtNativeTextureLease, view: wgpu::TextureView) -> Self {
        Self { lease, view }
    }

    pub fn texture_lease(&self) -> &QtNativeTextureLease {
        &self.lease
    }

    pub fn texture_view(&self) -> &wgpu::TextureView {
        &self.view
    }
}

pub struct QtWgpuContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    textures: HashMap<TextureCacheKey, QtWgpuTextureEntry>,
    states: HashMap<TypeId, Box<dyn Any + Send>>,
}

pub type QtWgpuContextHandle = Arc<Mutex<QtWgpuContext>>;

pub fn load_or_create_context(rhi_interop: QtRhiInteropInfo) -> Result<QtWgpuContextHandle> {
    match rhi_interop {
        QtRhiInteropInfo::Metal(info) => metal::load_or_create_context(info),
        QtRhiInteropInfo::Vulkan(info) => vk::load_or_create_context(info),
        QtRhiInteropInfo::D3d11(info) => d3d::load_or_create_context_d3d11(info),
        QtRhiInteropInfo::D3d12(info) => d3d::load_or_create_context_d3d12(info),
        QtRhiInteropInfo::OpenGles2(info) => gles::load_or_create_context(info),
    }
}

impl QtWgpuContext {
    pub(crate) fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        Self {
            device,
            queue,
            textures: HashMap::new(),
            states: HashMap::new(),
        }
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn state_or_insert_with<T>(
        &mut self,
        init: impl FnOnce(&wgpu::Device) -> Result<T>,
    ) -> Result<&mut T>
    where
        T: Any + Send + 'static,
    {
        let type_id = TypeId::of::<T>();
        if let std::collections::hash_map::Entry::Vacant(entry) = self.states.entry(type_id) {
            entry.insert(Box::new(init(&self.device)?));
        }
        Ok(self
            .states
            .get_mut(&type_id)
            .expect("typed state inserted")
            .downcast_mut::<T>()
            .expect("typed state downcast"))
    }

    pub(crate) fn texture_entry_with(
        &mut self,
        key: TextureCacheKey,
        create_entry: impl FnOnce(&wgpu::Device, TextureCacheKey) -> Result<QtWgpuTextureEntry>,
    ) -> Result<&QtWgpuTextureEntry> {
        self.textures
            .retain(|existing_key, _| existing_key.slot_id != key.slot_id || *existing_key == key);
        if !self.textures.contains_key(&key) {
            let entry = create_entry(&self.device, key)?;
            self.textures.insert(key, entry);
        }
        Ok(self.textures.get(&key).expect("texture entry inserted"))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::lease::{
        QT_RHI_BACKEND_METAL, QT_TEXTURE_FORMAT_RGBA8_UNORM, QtNativeTextureLease,
        QtNativeTextureLeaseInfo,
    };

    use super::TextureCacheKey;

    #[test]
    fn native_texture_lease_keeps_info() {
        let lease = QtNativeTextureLease::new(
            QtNativeTextureLeaseInfo {
                backend_tag: QT_RHI_BACKEND_METAL,
                format_tag: QT_TEXTURE_FORMAT_RGBA8_UNORM,
                width_px: 320,
                height_px: 180,
                object: 0x1234,
                layout: 7,
            },
            Arc::new(()),
        );

        assert_eq!(lease.info().object, 0x1234);
        assert_eq!(lease.info().width_px, 320);
    }

    #[test]
    fn rgba8_storage_key_uses_expected_descriptor_bits() {
        let key = TextureCacheKey::rgba8_storage(7, 640, 360);

        assert_eq!(key.slot_id, 7);
        assert_eq!(key.width_px, 640);
        assert_eq!(key.height_px, 360);
        assert_eq!(key.format(), wgpu::TextureFormat::Rgba8Unorm);
        assert_eq!(
            key.usage(),
            wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING
        );
    }
}
