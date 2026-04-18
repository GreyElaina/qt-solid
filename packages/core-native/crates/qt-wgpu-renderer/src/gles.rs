use std::{
    any::Any,
    collections::HashMap,
    ffi::{CString, c_char},
    sync::{Arc, Mutex},
};

use once_cell::sync::Lazy;

use crate::{
    context::{QtWgpuContext, QtWgpuContextHandle, QtWgpuTextureEntry, TextureCacheKey},
    error::{QtWgpuRendererError, Result},
    lease::{
        QT_RHI_BACKEND_OPENGLES2, QT_TEXTURE_FORMAT_RGBA8_UNORM,
        QtNativeTextureLease, QtNativeTextureLeaseInfo, QtRhiGles2InteropInfo,
    },
};

unsafe extern "C" {
    fn qt_wgpu_gles_get_proc_address(context_object: u64, name: *const c_char) -> u64;
}

#[derive(Debug, Clone)]
struct NativeTextureLeaseOwner<T> {
    texture: T,
}

type GlesContextHandle = Arc<Mutex<QtWgpuContext>>;

static GLES_CONTEXTS: Lazy<Mutex<HashMap<u64, GlesContextHandle>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub fn load_or_create_context(rhi_interop: QtRhiGles2InteropInfo) -> Result<QtWgpuContextHandle> {
    let key = rhi_interop.context_object;
    let mut contexts = GLES_CONTEXTS
        .lock()
        .expect("qt-wgpu GLES contexts mutex poisoned");
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

fn create_context(rhi_interop: QtRhiGles2InteropInfo) -> Result<QtWgpuContext> {
    if rhi_interop.context_object == 0 {
        return Err(QtWgpuRendererError::new(
            "qt-wgpu-renderer is missing a current OpenGL/GLES interop context",
        ));
    }

    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::GL,
        ..Default::default()
    });
    let exposed_adapter =
        unsafe {
            <wgpu::hal::api::Gles as wgpu::hal::Api>::Adapter::new_external(
                |name| {
                    let name = CString::new(name).expect("GL function name has interior NUL");
                    qt_wgpu_gles_get_proc_address(rhi_interop.context_object, name.as_ptr())
                        as *const _
                },
                wgpu::GlBackendOptions::default(),
            )
        }
        .ok_or_else(|| {
            QtWgpuRendererError::new(
                "failed to create wgpu GLES adapter from current external OpenGL context",
            )
        })?;
    let required_features = wgpu::Features::empty();
    let required_limits = exposed_adapter.capabilities.limits.clone();
    let adapter = unsafe { instance.create_adapter_from_hal::<wgpu::hal::api::Gles>(exposed_adapter) };
    let hal_adapter = unsafe { adapter.as_hal::<wgpu::hal::api::Gles>() }.ok_or_else(|| {
        QtWgpuRendererError::new("failed to expose GLES HAL adapter from wgpu adapter")
    })?;
    let open_device = unsafe {
        <wgpu::hal::gles::Adapter as wgpu::hal::Adapter>::open(
            &*hal_adapter,
            required_features,
            &required_limits,
            &wgpu::MemoryHints::Performance,
        )
    }
    .map_err(|error| {
        QtWgpuRendererError::new(format!(
            "failed to create wgpu GLES device from external context: {error}",
        ))
    })?;
    let (device, queue) = unsafe {
        adapter.create_device_from_hal::<wgpu::hal::api::Gles>(
            open_device,
            &wgpu::DeviceDescriptor {
                label: Some("qt-wgpu-renderer-gles-device"),
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
            "failed to open wgpu GLES device from external context: {error}",
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
            .as_hal::<wgpu::hal::api::Gles>()
            .ok_or_else(|| QtWgpuRendererError::new("failed to expose wgpu GLES texture"))?
    };

    let object = match &raw_texture.inner {
        wgpu::hal::gles::TextureInner::Texture { raw, .. } => raw.0.get() as u64,
        _ => {
            return Err(QtWgpuRendererError::new(
                "qt-wgpu-renderer cannot export non-texture GLES surfaces",
            ))
        }
    };

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
            backend_tag: QT_RHI_BACKEND_OPENGLES2,
            format_tag,
            width_px,
            height_px,
            object,
            layout: 0,
        },
        owner,
    ))
}
