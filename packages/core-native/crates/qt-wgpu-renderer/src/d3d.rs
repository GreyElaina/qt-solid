use std::{
    any::Any,
    collections::HashMap,
    sync::{Arc, Mutex},
};

use once_cell::sync::Lazy;
use windows::{
    Win32::{
        Foundation::{GENERIC_ALL, HANDLE, LUID},
        Graphics::{
            Direct3D11, Direct3D12,
            Dxgi::{self, Common::DXGI_FORMAT_R8G8B8A8_UNORM},
        },
    },
    core::{Free as _, Interface as _, PCWSTR},
};
use wgpu::hal::Instance as _;

use crate::{
    context::{QtWgpuContext, QtWgpuContextHandle, QtWgpuTextureEntry, TextureCacheKey},
    error::{QtWgpuRendererError, Result},
    lease::{
        QT_RHI_BACKEND_D3D11, QT_RHI_BACKEND_D3D12, QT_TEXTURE_FORMAT_RGBA8_UNORM,
        QtNativeTextureLease, QtNativeTextureLeaseInfo, QtRhiD3d11InteropInfo,
        QtRhiD3d12InteropInfo,
    },
};

#[derive(Debug)]
struct SharedNtHandle(u64);

impl Drop for SharedNtHandle {
    fn drop(&mut self) {
        let mut handle = HANDLE(self.0 as *mut core::ffi::c_void);
        unsafe {
            HANDLE::free(&mut handle);
        }
    }
}

#[derive(Debug, Clone)]
struct NativeTextureLeaseOwner<T> {
    texture: T,
}

#[derive(Debug)]
struct NativeTextureLeaseOwnerD3d11 {
    texture: wgpu::Texture,
    d3d11_texture: Direct3D11::ID3D11Texture2D,
    shared_handle: SharedNtHandle,
}

type D3d12ContextHandle = Arc<Mutex<QtWgpuContext>>;
type D3d11ContextKey = (u32, i32);

static D3D12_CONTEXTS: Lazy<Mutex<HashMap<u64, D3d12ContextHandle>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static D3D11_CONTEXTS: Lazy<Mutex<HashMap<D3d11ContextKey, D3d12ContextHandle>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub fn load_or_create_context_d3d12(
    rhi_interop: QtRhiD3d12InteropInfo,
) -> Result<QtWgpuContextHandle> {
    let key = rhi_interop.device_object;
    let mut contexts = D3D12_CONTEXTS
        .lock()
        .expect("qt-wgpu D3D12 contexts mutex poisoned");
    if let Some(existing) = contexts.get(&key) {
        return Ok(Arc::clone(existing));
    }

    let context = Arc::new(Mutex::new(create_context_d3d12(rhi_interop)?));
    contexts.insert(key, Arc::clone(&context));
    Ok(context)
}

pub fn load_or_create_context_d3d11(
    rhi_interop: QtRhiD3d11InteropInfo,
) -> Result<QtWgpuContextHandle> {
    let key = (rhi_interop.adapter_luid_low, rhi_interop.adapter_luid_high);
    let mut contexts = D3D11_CONTEXTS
        .lock()
        .expect("qt-wgpu D3D11 contexts mutex poisoned");
    if let Some(existing) = contexts.get(&key) {
        return Ok(Arc::clone(existing));
    }

    let context = Arc::new(Mutex::new(create_context_d3d11(rhi_interop)?));
    contexts.insert(key, Arc::clone(&context));
    Ok(context)
}

pub fn cached_rgba8_texture_entry_d3d12<'a>(
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
        let lease = texture_lease_d3d12(
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

pub fn cached_rgba8_texture_entry_d3d11<'a>(
    context: &'a mut QtWgpuContext,
    rhi_interop: QtRhiD3d11InteropInfo,
    key: TextureCacheKey,
    label: &'static str,
) -> Result<&'a QtWgpuTextureEntry> {
    context.texture_entry_with(key, |device, key| {
        create_shared_texture_entry_d3d11(device, rhi_interop, key, label)
    })
}

pub fn finish_texture_render_d3d12(context: &QtWgpuContext) -> Result<()> {
    context
        .device()
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|error| {
            QtWgpuRendererError::new(format!(
                "failed to wait for D3D12 texture rendering: {error}",
            ))
        })?;
    Ok(())
}

pub fn finish_texture_render_d3d11(context: &QtWgpuContext) -> Result<()> {
    context
        .device()
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|error| {
            QtWgpuRendererError::new(format!(
                "failed to wait for D3D11 shared texture rendering: {error}",
            ))
        })?;
    Ok(())
}

fn create_context_d3d12(rhi_interop: QtRhiD3d12InteropInfo) -> Result<QtWgpuContext> {
    let qt_device = retain_d3d12_device(rhi_interop.device_object)?;
    let qt_luid = unsafe { qt_device.GetAdapterLuid() };

    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::DX12,
        ..Default::default()
    });
    let hal_instance = unsafe { instance.as_hal::<wgpu::hal::api::Dx12>() }.ok_or_else(|| {
        QtWgpuRendererError::new("failed to expose D3D12 HAL instance from wgpu instance")
    })?;
    let exposed_adapter = select_matching_adapter(hal_instance, qt_luid)?;
    let required_features = wgpu::Features::empty();
    let required_limits = exposed_adapter.capabilities.limits.clone();
    let adapter = unsafe { instance.create_adapter_from_hal::<wgpu::hal::api::Dx12>(exposed_adapter) };
    let hal_adapter = unsafe { adapter.as_hal::<wgpu::hal::api::Dx12>() }.ok_or_else(|| {
        QtWgpuRendererError::new("failed to expose D3D12 HAL adapter from wgpu adapter")
    })?;
    let open_device = unsafe {
        <wgpu::hal::dx12::Adapter as wgpu::hal::Adapter>::open(
            &*hal_adapter,
            required_features,
            &required_limits,
            &wgpu::MemoryHints::Performance,
        )
    }
    .map_err(|error| {
        QtWgpuRendererError::new(format!(
            "failed to create wgpu D3D12 device from Qt adapter: {error}",
        ))
    })?;

    let opened_device_ptr = open_device.device.raw_device().as_raw() as u64;
    let qt_device_ptr = qt_device.as_raw() as u64;
    if opened_device_ptr != qt_device_ptr {
        return Err(QtWgpuRendererError::new(
            "qt-wgpu-renderer could not reuse Qt D3D12 device; raw queue import is still missing",
        ));
    }

    let (device, queue) = unsafe {
        adapter.create_device_from_hal::<wgpu::hal::api::Dx12>(
            open_device,
            &wgpu::DeviceDescriptor {
                label: Some("qt-wgpu-renderer-d3d12-device"),
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
            "failed to open wgpu D3D12 device from Qt handles: {error}",
        ))
    })?;

    Ok(QtWgpuContext::new(device, queue))
}

fn create_context_d3d11(rhi_interop: QtRhiD3d11InteropInfo) -> Result<QtWgpuContext> {
    let qt_luid = LUID {
        LowPart: rhi_interop.adapter_luid_low,
        HighPart: rhi_interop.adapter_luid_high,
    };
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::DX12,
        ..Default::default()
    });
    let hal_instance = unsafe { instance.as_hal::<wgpu::hal::api::Dx12>() }.ok_or_else(|| {
        QtWgpuRendererError::new("failed to expose D3D12 HAL instance from wgpu instance")
    })?;
    let exposed_adapter = select_matching_adapter(hal_instance, qt_luid)?;
    let required_features = wgpu::Features::empty();
    let required_limits = exposed_adapter.capabilities.limits.clone();
    let adapter = unsafe { instance.create_adapter_from_hal::<wgpu::hal::api::Dx12>(exposed_adapter) };
    let hal_adapter = unsafe { adapter.as_hal::<wgpu::hal::api::Dx12>() }.ok_or_else(|| {
        QtWgpuRendererError::new("failed to expose D3D12 HAL adapter from wgpu adapter")
    })?;
    let open_device = unsafe {
        <wgpu::hal::dx12::Adapter as wgpu::hal::Adapter>::open(
            &*hal_adapter,
            required_features,
            &required_limits,
            &wgpu::MemoryHints::Performance,
        )
    }
    .map_err(|error| {
        QtWgpuRendererError::new(format!(
            "failed to create wgpu D3D12 device for Qt D3D11 interop: {error}",
        ))
    })?;

    let (device, queue) = unsafe {
        adapter.create_device_from_hal::<wgpu::hal::api::Dx12>(
            open_device,
            &wgpu::DeviceDescriptor {
                label: Some("qt-wgpu-renderer-d3d11-via-d3d12-device"),
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
            "failed to open wgpu D3D12 device for Qt D3D11 interop: {error}",
        ))
    })?;

    Ok(QtWgpuContext::new(device, queue))
}

fn create_shared_texture_entry_d3d11(
    device: &wgpu::Device,
    rhi_interop: QtRhiD3d11InteropInfo,
    key: TextureCacheKey,
    label: &'static str,
) -> Result<QtWgpuTextureEntry> {
    let hal_device = unsafe { device.as_hal::<wgpu::hal::api::Dx12>() }.ok_or_else(|| {
        QtWgpuRendererError::new("failed to expose D3D12 HAL device from wgpu device")
    })?;

    let raw_desc = Direct3D12::D3D12_RESOURCE_DESC {
        Dimension: Direct3D12::D3D12_RESOURCE_DIMENSION_TEXTURE2D,
        Alignment: 0,
        Width: u64::from(key.width_px),
        Height: key.height_px,
        DepthOrArraySize: 1,
        MipLevels: 1,
        Format: DXGI_FORMAT_R8G8B8A8_UNORM,
        SampleDesc: Dxgi::Common::DXGI_SAMPLE_DESC {
            Count: 1,
            Quality: 0,
        },
        Layout: Direct3D12::D3D12_TEXTURE_LAYOUT_UNKNOWN,
        Flags: Direct3D12::D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS,
    };
    let heap_props = Direct3D12::D3D12_HEAP_PROPERTIES {
        Type: Direct3D12::D3D12_HEAP_TYPE_DEFAULT,
        CPUPageProperty: Direct3D12::D3D12_CPU_PAGE_PROPERTY_UNKNOWN,
        MemoryPoolPreference: Direct3D12::D3D12_MEMORY_POOL_UNKNOWN,
        CreationNodeMask: 0,
        VisibleNodeMask: 0,
    };
    let mut resource = None;
    unsafe {
        hal_device.raw_device().CreateCommittedResource(
            &heap_props,
            Direct3D12::D3D12_HEAP_FLAG_SHARED,
            &raw_desc,
            Direct3D12::D3D12_RESOURCE_STATE_COMMON,
            None,
            &mut resource,
        )
    }
    .map_err(|error| {
        QtWgpuRendererError::new(format!(
            "failed to create shared D3D12 texture for Qt D3D11 interop: {error}",
        ))
    })?;
    let resource = resource.ok_or_else(|| {
        QtWgpuRendererError::new("D3D12 shared texture creation returned a null resource")
    })?;

    let shared_handle = unsafe {
        hal_device.raw_device().CreateSharedHandle(
            &resource,
            None,
            GENERIC_ALL.0,
            PCWSTR::null(),
        )
    }
    .map_err(|error| {
        QtWgpuRendererError::new(format!(
            "failed to create shared handle for Qt D3D11 interop texture: {error}",
        ))
    })?;

    let d3d11_device = retain_d3d11_device1(rhi_interop.device_object)?;
    let d3d11_texture = unsafe {
        d3d11_device.OpenSharedResource1::<Direct3D11::ID3D11Texture2D>(shared_handle)
    }
    .map_err(|error| {
        QtWgpuRendererError::new(format!(
            "failed to open shared D3D12 texture on Qt D3D11 device: {error}",
        ))
    })?;

    let hal_texture = unsafe {
        wgpu::hal::dx12::Device::texture_from_raw(
            resource,
            key.format(),
            wgpu::TextureDimension::D2,
            wgpu::Extent3d {
                width: key.width_px,
                height: key.height_px,
                depth_or_array_layers: 1,
            },
            1,
            1,
        )
    };
    let texture = unsafe {
        device.create_texture_from_hal::<wgpu::hal::api::Dx12>(
            hal_texture,
            &wgpu::TextureDescriptor {
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
            },
        )
    };

    let owner = Arc::new(NativeTextureLeaseOwnerD3d11 {
        texture,
        d3d11_texture,
        shared_handle: SharedNtHandle(shared_handle.0 as u64),
    });
    let lease = texture_lease_d3d11(
        Arc::clone(&owner),
        key.format(),
        key.width_px,
        key.height_px,
    )?;
    let view = owner
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());
    Ok(QtWgpuTextureEntry::new(lease, view))
}

fn select_matching_adapter(
    hal_instance: &wgpu::hal::dx12::Instance,
    qt_luid: LUID,
) -> Result<wgpu::hal::ExposedAdapter<wgpu::hal::api::Dx12>> {
    let mut fallback = None;
    for exposed_adapter in unsafe { hal_instance.enumerate_adapters(None) } {
        if fallback.is_none() {
            fallback = Some(exposed_adapter.info.name.clone());
        }
        let desc = unsafe { exposed_adapter.adapter.as_raw().GetDesc2() }.map_err(|error| {
            QtWgpuRendererError::new(format!(
                "failed to query D3D12 adapter descriptor from wgpu-hal: {error}",
            ))
        })?;
        if luid_eq(desc.AdapterLuid, qt_luid) {
            return Ok(exposed_adapter);
        }
    }

    match fallback {
        Some(name) => Err(QtWgpuRendererError::new(format!(
            "failed to find a matching D3D12 adapter for Qt interop; first adapter was {name}",
        ))),
        None => Err(QtWgpuRendererError::new(
            "wgpu did not enumerate any D3D12 adapters",
        )),
    }
}

fn retain_d3d11_device1(device_object: u64) -> Result<Direct3D11::ID3D11Device1> {
    if device_object == 0 {
        return Err(QtWgpuRendererError::new(
            "qt-wgpu-renderer is missing a D3D11 device handle",
        ));
    }
    let raw = device_object as *mut core::ffi::c_void;
    let borrowed = unsafe { Direct3D11::ID3D11Device::from_raw_borrowed(&raw) }.ok_or_else(|| {
        QtWgpuRendererError::new("Qt passed an invalid D3D11 device handle")
    })?;
    borrowed.cast().map_err(|error| {
        QtWgpuRendererError::new(format!(
            "Qt D3D11 device does not support ID3D11Device1 shared-resource import: {error}",
        ))
    })
}

fn retain_d3d12_device(device_object: u64) -> Result<Direct3D12::ID3D12Device> {
    if device_object == 0 {
        return Err(QtWgpuRendererError::new(
            "qt-wgpu-renderer is missing a D3D12 device handle",
        ));
    }
    let raw = device_object as *mut core::ffi::c_void;
    let borrowed = unsafe { Direct3D12::ID3D12Device::from_raw_borrowed(&raw) }.ok_or_else(|| {
        QtWgpuRendererError::new("Qt passed an invalid D3D12 device handle")
    })?;
    Ok(borrowed.clone())
}

fn texture_lease_d3d12(
    owner: Arc<NativeTextureLeaseOwner<wgpu::Texture>>,
    format: wgpu::TextureFormat,
    width_px: u32,
    height_px: u32,
) -> Result<QtNativeTextureLease> {
    let raw_texture = unsafe {
        owner
            .texture
            .as_hal::<wgpu::hal::api::Dx12>()
            .ok_or_else(|| QtWgpuRendererError::new("failed to expose wgpu D3D12 texture"))?
    };
    let object = unsafe { raw_texture.raw_resource() }.as_raw() as u64;
    if object == 0 {
        return Err(QtWgpuRendererError::new(
            "qt-wgpu-renderer produced a null D3D12 resource handle",
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
            backend_tag: QT_RHI_BACKEND_D3D12,
            format_tag,
            width_px,
            height_px,
            object,
            layout: 0,
        },
        owner,
    ))
}

fn texture_lease_d3d11(
    owner: Arc<NativeTextureLeaseOwnerD3d11>,
    format: wgpu::TextureFormat,
    width_px: u32,
    height_px: u32,
) -> Result<QtNativeTextureLease> {
    std::hint::black_box(owner.shared_handle.0);
    let object = owner.d3d11_texture.as_raw() as u64;
    if object == 0 {
        return Err(QtWgpuRendererError::new(
            "qt-wgpu-renderer produced a null D3D11 texture handle",
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
            backend_tag: QT_RHI_BACKEND_D3D11,
            format_tag,
            width_px,
            height_px,
            object,
            layout: 0,
        },
        owner,
    ))
}

fn luid_eq(lhs: LUID, rhs: LUID) -> bool {
    lhs.LowPart == rhs.LowPart && lhs.HighPart == rhs.HighPart
}
