use std::{
    any::Any,
    collections::HashMap,
    sync::{Arc, Mutex},
};

use ash::vk::{self, Handle as _};
use once_cell::sync::Lazy;
use wgpu::hal::Instance as _;

use crate::{
    context::{QtWgpuContext, QtWgpuContextHandle, QtWgpuTextureEntry, TextureCacheKey},
    error::{QtWgpuRendererError, Result},
    lease::{
        QT_RHI_BACKEND_VULKAN, QT_TEXTURE_FORMAT_RGBA8_UNORM, QtNativeTextureLease,
        QtNativeTextureLeaseInfo, QtRhiVulkanInteropInfo,
    },
};

#[derive(Debug, Clone)]
struct NativeTextureLeaseOwner<T> {
    texture: T,
}

type VulkanContextHandle = Arc<Mutex<QtWgpuContext>>;

static VULKAN_CONTEXTS: Lazy<Mutex<HashMap<(u64, u64, u32, u32), VulkanContextHandle>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub fn load_or_create_context(rhi_interop: QtRhiVulkanInteropInfo) -> Result<QtWgpuContextHandle> {
    let key = (
        rhi_interop.physical_device_object,
        rhi_interop.device_object,
        rhi_interop.queue_family_index,
        rhi_interop.queue_index,
    );
    let mut contexts = VULKAN_CONTEXTS
        .lock()
        .expect("qt-wgpu Vulkan contexts mutex poisoned");
    if let Some(existing) = contexts.get(&key) {
        return Ok(Arc::clone(existing));
    }

    let context = Arc::new(Mutex::new(create_context(rhi_interop)?));
    contexts.insert(key, Arc::clone(&context));
    Ok(context)
}

pub fn cached_rgba8_texture_entry<'a>(
    context: &'a mut QtWgpuContext,
    key: TextureCacheKey,
    label: &'static str,
) -> Result<&'a QtWgpuTextureEntry> {
    context.texture_entry_with(key, |device, key| {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: key.width_px,
                height: key.height_px,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: key.format(),
            usage: key.usage(),
            view_formats: &[],
        });
        let owner = Arc::new(NativeTextureLeaseOwner { texture });
        let lease = texture_lease(
            Arc::clone(&owner),
            key.format(),
            key.width_px,
            key.height_px,
        )?;
        let view = owner
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        Ok(QtWgpuTextureEntry::new(lease, view))
    })
}

fn create_context(rhi_interop: QtRhiVulkanInteropInfo) -> Result<QtWgpuContext> {
    if rhi_interop.physical_device_object == 0 || rhi_interop.device_object == 0 {
        return Err(QtWgpuRendererError::new(
            "qt-wgpu-renderer is missing Vulkan physical device or device handles",
        ));
    }

    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::VULKAN,
        ..Default::default()
    });
    let hal_instance = unsafe { instance.as_hal::<wgpu::hal::api::Vulkan>() }.ok_or_else(|| {
        QtWgpuRendererError::new("failed to expose Vulkan HAL instance from wgpu instance")
    })?;
    let exposed_adapter =
        select_matching_adapter(hal_instance, rhi_interop.physical_device_object)?;
    let required_features = wgpu::Features::empty();
    let required_limits = exposed_adapter.capabilities.limits.clone();
    let enabled_extensions = exposed_adapter
        .adapter
        .required_device_extensions(required_features);
    let adapter_hal = &exposed_adapter.adapter;
    let raw_device = unsafe {
        ash::Device::load(
            adapter_hal.shared_instance().raw_instance().fp_v1_0(),
            vk::Device::from_raw(rhi_interop.device_object),
        )
    };
    let open_device = unsafe {
        adapter_hal.device_from_raw(
            raw_device,
            Some(Box::new(|| {})),
            &enabled_extensions,
            required_features,
            &wgpu::MemoryHints::Performance,
            rhi_interop.queue_family_index,
            rhi_interop.queue_index,
        )
    }
    .map_err(|error| {
        QtWgpuRendererError::new(format!(
            "failed to create wgpu Vulkan device from Qt handles: {error}",
        ))
    })?;

    let adapter =
        unsafe { instance.create_adapter_from_hal::<wgpu::hal::api::Vulkan>(exposed_adapter) };
    let (device, queue) = unsafe {
        adapter.create_device_from_hal::<wgpu::hal::api::Vulkan>(
            open_device,
            &wgpu::DeviceDescriptor {
                label: Some("qt-wgpu-renderer-vulkan-device"),
                required_features,
                required_limits,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            },
        )
    }
    .map_err(|error| {
        QtWgpuRendererError::new(format!(
            "failed to open wgpu Vulkan device from Qt handles: {error}",
        ))
    })?;

    Ok(QtWgpuContext::new(device, queue))
}

fn select_matching_adapter(
    hal_instance: &wgpu::hal::vulkan::Instance,
    qt_physical_device_object: u64,
) -> Result<wgpu::hal::ExposedAdapter<wgpu::hal::api::Vulkan>> {
    let target = vk::PhysicalDevice::from_raw(qt_physical_device_object);
    let mut fallback = None;
    for exposed_adapter in unsafe { hal_instance.enumerate_adapters(None) } {
        if fallback.is_none() {
            fallback = Some(exposed_adapter.info.name.clone());
        }
        if exposed_adapter.adapter.raw_physical_device() == target {
            return Ok(exposed_adapter);
        }
    }

    match fallback {
        Some(name) => Err(QtWgpuRendererError::new(format!(
            "failed to find a matching Vulkan adapter for Qt physical device; first adapter was {name}",
        ))),
        None => Err(QtWgpuRendererError::new(
            "wgpu did not enumerate any Vulkan adapters",
        )),
    }
}

fn texture_lease(
    owner: Arc<NativeTextureLeaseOwner<wgpu::Texture>>,
    format: wgpu::TextureFormat,
    width_px: u32,
    height_px: u32,
) -> Result<QtNativeTextureLease> {
    let raw_texture = unsafe {
        owner
            .texture
            .as_hal::<wgpu::hal::api::Vulkan>()
            .ok_or_else(|| QtWgpuRendererError::new("failed to expose wgpu Vulkan texture"))?
    };
    let object = unsafe { raw_texture.raw_handle() }.as_raw();
    if object == 0 {
        return Err(QtWgpuRendererError::new(
            "qt-wgpu-renderer produced a null Vulkan image handle",
        ));
    }

    let format_tag = match format {
        wgpu::TextureFormat::Rgba8Unorm => QT_TEXTURE_FORMAT_RGBA8_UNORM,
        other => {
            return Err(QtWgpuRendererError::new(format!(
                "qt-wgpu-renderer cannot export texture format {other:?}",
            )));
        }
    };

    let owner: Arc<dyn Any + Send + Sync> = owner;
    Ok(QtNativeTextureLease::new(
        QtNativeTextureLeaseInfo {
            backend_tag: QT_RHI_BACKEND_VULKAN,
            format_tag,
            width_px,
            height_px,
            object,
            layout: vk::ImageLayout::GENERAL.as_raw(),
        },
        owner,
    ))
}
