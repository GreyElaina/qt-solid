use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use vello::wgpu;
use vello_hybrid::{RenderTargetConfig, Renderer};

use crate::image::ImageCache;
use crate::renderer::types::SurfaceTarget;
use crate::runtime::qt_error;

pub(crate) struct RendererCache {
    pub width_px: u32,
    pub height_px: u32,
    pub renderer: Renderer,
    pub image_cache: ImageCache,
}

struct WindowCompositorContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter_pci_bus_id: Option<String>,
    renderer_cache: Option<RendererCache>,
}

static WINDOW_COMPOSITORS: LazyLock<Mutex<HashMap<u32, WindowCompositorContext>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

impl WindowCompositorContext {
    fn new(target: &SurfaceTarget) -> napi::Result<Self> {
        let backends = if cfg!(target_os = "windows") {
            wgpu::Backends::VULKAN
        } else {
            wgpu::Backends::default()
        };
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends,
            ..Default::default()
        });

        #[cfg(target_os = "macos")]
        let surface = {
            let crate::renderer::types::SurfaceHandle::AppKit(ns_view) = target.handle else {
                return Err(qt_error("expected AppKit surface handle"));
            };
            let layer_ptr = super::resolve_metal_layer_for_ns_view(ns_view);
            unsafe {
                instance
                    .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(
                        layer_ptr.as_ptr(),
                    ))
                    .map_err(|error| qt_error(error.to_string()))?
            }
        };

        #[cfg(not(target_os = "macos"))]
        let surface = unsafe {
            instance
                .create_surface_unsafe(super::compositor_surface_target(&target)?)
                .map_err(|error| qt_error(error.to_string()))?
        };

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }))
        .map_err(|error| qt_error(error.to_string()))?;

        let adapter_pci_bus_id = {
            let id = adapter.get_info().device_pci_bus_id;
            if id.is_empty() { None } else { Some(id) }
        };

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("qt-solid-wgpu-compositor-device"),
            ..Default::default()
        }))
        .map_err(|error| qt_error(error.to_string()))?;

        Ok(Self {
            device,
            queue,
            adapter_pci_bus_id,
            renderer_cache: None,
        })
    }

    fn ensure_renderer(&mut self, width_px: u32, height_px: u32) {
        let needs_recreate = self
            .renderer_cache
            .as_ref()
            .map_or(true, |c| c.width_px != width_px || c.height_px != height_px);
        if needs_recreate {
            self.renderer_cache = Some(RendererCache {
                width_px,
                height_px,
                renderer: Renderer::new(
                    &self.device,
                    &RenderTargetConfig {
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        width: width_px,
                        height: height_px,
                    },
                ),
                image_cache: ImageCache::default(),
            });
        }
    }
}

fn ensure_compositor<'a>(
    compositors: &'a mut HashMap<u32, WindowCompositorContext>,
    node_id: u32,
    target: &SurfaceTarget,
) -> napi::Result<&'a mut WindowCompositorContext> {
    if !compositors.contains_key(&node_id) {
        let ctx = WindowCompositorContext::new(target)?;
        compositors.insert(node_id, ctx);
    }
    Ok(compositors.get_mut(&node_id).unwrap())
}

pub(crate) fn destroy_window_compositor_by_node(node_id: u32) {
    WINDOW_COMPOSITORS
        .lock()
        .expect("qt wgpu compositor registry mutex poisoned")
        .remove(&node_id);
}

pub(crate) fn with_window_compositor_device_queue<T, F>(
    node_id: u32,
    target: &SurfaceTarget,
    width_px: u32,
    height_px: u32,
    run: F,
) -> napi::Result<T>
where
    F: FnOnce(&wgpu::Device, &wgpu::Queue, &mut Renderer, &mut ImageCache) -> napi::Result<T>,
{
    let mut compositors = WINDOW_COMPOSITORS
        .lock()
        .expect("qt wgpu compositor registry mutex poisoned");
    let ctx = ensure_compositor(&mut compositors, node_id, target)?;
    ctx.ensure_renderer(width_px, height_px);
    let rc = ctx.renderer_cache.as_mut().unwrap();
    run(&ctx.device, &ctx.queue, &mut rc.renderer, &mut rc.image_cache)
}

pub(crate) fn window_compositor_adapter_pci_bus_id_by_node(node_id: u32) -> Option<String> {
    WINDOW_COMPOSITORS
        .lock()
        .expect("qt wgpu compositor registry mutex poisoned")
        .get(&node_id)?
        .adapter_pci_bus_id
        .clone()
}
