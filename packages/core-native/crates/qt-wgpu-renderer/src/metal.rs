use std::{
    any::Any,
    collections::HashMap,
    sync::{Arc, Mutex},
};

use metal::foreign_types::ForeignType;
use once_cell::sync::Lazy;
use wgpu::hal::Instance as _;

use metal::{CommandQueue, Device};

use crate::{
    context::{QtWgpuContext, QtWgpuContextHandle, QtWgpuTextureEntry, TextureCacheKey},
    error::{QtWgpuRendererError, Result},
    lease::{
        QT_TEXTURE_FORMAT_RGBA8_UNORM, QtNativeTextureLease, QtNativeTextureLeaseInfo,
        QtRhiMetalInteropInfo,
    },
};

#[derive(Debug, Clone)]
struct NativeTextureLeaseOwner<T> {
    texture: T,
}

type MetalContextHandle = Arc<Mutex<QtWgpuContext>>;

static METAL_CONTEXTS: Lazy<Mutex<HashMap<(u64, u64), MetalContextHandle>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub fn load_or_create_context(rhi_interop: QtRhiMetalInteropInfo) -> Result<QtWgpuContextHandle> {
    let key = (rhi_interop.device_object, rhi_interop.command_queue_object);
    let mut contexts = METAL_CONTEXTS
        .lock()
        .expect("qt-wgpu contexts mutex poisoned");
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

fn create_context(rhi_interop: QtRhiMetalInteropInfo) -> Result<QtWgpuContext> {
    if rhi_interop.device_object == 0 || rhi_interop.command_queue_object == 0 {
        return Err(QtWgpuRendererError::new(
            "qt-wgpu-renderer is missing Metal device or queue handles",
        ));
    }

    let qt_device = retain_device(rhi_interop.device_object)?;
    let qt_queue = retain_command_queue(rhi_interop.command_queue_object)?;
    let qt_device_name = qt_device.name().to_string();

    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::METAL,
        ..Default::default()
    });
    let hal_instance = unsafe { instance.as_hal::<wgpu::hal::api::Metal>() }.ok_or_else(|| {
        QtWgpuRendererError::new("failed to expose Metal HAL instance from wgpu instance")
    })?;

    let exposed_adapter = select_matching_adapter(hal_instance, rhi_interop.device_object)?;
    let required_features = wgpu::Features::empty();
    let required_limits = exposed_adapter.capabilities.limits.clone();
    let adapter =
        unsafe { instance.create_adapter_from_hal::<wgpu::hal::api::Metal>(exposed_adapter) };
    let timestamp_period = timestamp_period(&qt_device_name);
    let hal_device =
        unsafe { wgpu::hal::metal::Device::device_from_raw(qt_device, required_features) };
    let hal_queue = unsafe { wgpu::hal::metal::Queue::queue_from_raw(qt_queue, timestamp_period) };
    let (device, queue) = unsafe {
        adapter.create_device_from_hal::<wgpu::hal::api::Metal>(
            wgpu::hal::OpenDevice {
                device: hal_device,
                queue: hal_queue,
            },
            &wgpu::DeviceDescriptor {
                label: Some("qt-wgpu-renderer-metal-device"),
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
            "failed to create wgpu device from Qt Metal handles: {error}",
        ))
    })?;

    Ok(QtWgpuContext::new(device, queue))
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
            .as_hal::<wgpu::hal::api::Metal>()
            .ok_or_else(|| QtWgpuRendererError::new("failed to expose wgpu Metal texture"))?
    };
    let object = unsafe { raw_texture.raw_handle() }.as_ptr() as u64;
    if object == 0 {
        return Err(QtWgpuRendererError::new(
            "qt-wgpu-renderer produced a null Metal texture handle",
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
            backend_tag: crate::lease::QT_RHI_BACKEND_METAL,
            format_tag,
            width_px,
            height_px,
            object,
            layout: 0,
        },
        owner,
    ))
}

fn select_matching_adapter(
    hal_instance: &wgpu::hal::metal::Instance,
    qt_device_object: u64,
) -> Result<wgpu::hal::ExposedAdapter<wgpu::hal::api::Metal>> {
    let mut fallback = None;
    for exposed_adapter in unsafe { hal_instance.enumerate_adapters(None) } {
        if fallback.is_none() {
            fallback = Some(exposed_adapter.info.name.clone());
        }

        let opened = unsafe {
            <wgpu::hal::metal::Adapter as wgpu::hal::Adapter>::open(
                &exposed_adapter.adapter,
                wgpu::Features::empty(),
                &exposed_adapter.capabilities.limits,
                &wgpu::MemoryHints::Performance,
            )
        }
        .map_err(|error| {
            QtWgpuRendererError::new(format!("failed to open Metal adapter candidate: {error}"))
        })?;
        let opened_device_ptr = opened.device.raw_device().as_ptr() as u64;
        if opened_device_ptr == qt_device_object {
            return Ok(exposed_adapter);
        }
    }

    match fallback {
        Some(name) => Err(QtWgpuRendererError::new(format!(
            "failed to find a matching Metal adapter for Qt device; first adapter was {name}",
        ))),
        None => Err(QtWgpuRendererError::new(
            "wgpu did not enumerate any Metal adapters",
        )),
    }
}

fn retain_device(device_object: u64) -> Result<Device> {
    if device_object == 0 {
        return Err(QtWgpuRendererError::new(
            "Qt passed a null Metal device handle",
        ));
    }
    let raw = device_object as *mut metal::MTLDevice;
    Ok(unsafe { Device::from_ptr(raw) })
}

fn retain_command_queue(command_queue_object: u64) -> Result<CommandQueue> {
    if command_queue_object == 0 {
        return Err(QtWgpuRendererError::new(
            "Qt passed a null Metal command queue handle",
        ));
    }
    let raw = command_queue_object as *mut metal::MTLCommandQueue;
    Ok(unsafe { CommandQueue::from_ptr(raw) })
}

fn timestamp_period(device_name: &str) -> f32 {
    if device_name.starts_with("Intel") {
        83.333
    } else {
        1.0
    }
}
