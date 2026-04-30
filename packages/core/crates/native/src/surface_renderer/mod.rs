use std::{
    collections::HashMap,
    sync::Mutex,
};
#[cfg(target_os = "macos")]
use std::{ffi::c_void, ptr::NonNull};

use once_cell::sync::Lazy;
use crate::hybrid_image_cache::{HybridImageCache, sweep_stale_images};
use vello::wgpu;
use vello_hybrid::{AtlasConfig, RenderSettings, RenderSize, RenderTargetConfig, Renderer, Scene as HybridScene};

use crate::canvas::vello::Scene;
use crate::canvas::vello::peniko::kurbo::Affine;
use anyrender::PaintScene;
use crate::runtime::qt_error;

/// Per-window GPU surface state: wgpu device/queue/surface + vello renderer.
struct WindowSurface {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    image_cache: HybridImageCache,
    /// Intermediate Rgba8Unorm texture — vello renders here, then we blit to the
    /// sRGB surface texture.
    render_texture: wgpu::Texture,
    render_view: wgpu::TextureView,
    render_bind_group: wgpu::BindGroup,
    blit_pipeline: wgpu::RenderPipeline,
    blit_bind_group_layout: wgpu::BindGroupLayout,
    blit_sampler: wgpu::Sampler,
    /// Raw pointer to the CAMetalLayer (macOS). Used by the C++ side to
    /// install a displayLayer delegate for synchronous resize presentation.
    #[cfg(target_os = "macos")]
    metal_layer_ptr: SendPtr,
}

/// Wrapper to allow raw pointers in `Send` contexts.
/// Safety: the CAMetalLayer pointer is only accessed from the main thread.
#[cfg(target_os = "macos")]
#[derive(Clone, Copy)]
struct SendPtr(*mut c_void);
#[cfg(target_os = "macos")]
unsafe impl Send for SendPtr {}

/// Cached CPU render state to avoid per-frame allocation of RenderContext + Pixmap.
struct CpuRenderState {
    context: vello_cpu::RenderContext,
    pixmap: vello_cpu::Pixmap,
}

enum WindowRenderMode {
    Gpu(WindowSurface),
    Cpu(Option<CpuRenderState>),
    /// Window has been destroyed; pending frame drives should no-op.
    Destroyed,
}

/// Distinguishes hard GPU failures (no adapter/device — fallback to CPU) from
/// transient surface readiness issues (HWND not ready — skip frame, retry later).
enum SurfaceCreationError {
    /// GPU hardware is not available or incompatible. Permanent; use CPU fallback.
    NoGpu(String),
    /// Surface exists but is not ready yet (e.g. configure failed). Retry next frame.
    NotReady(String),
}

static WINDOW_SURFACES: Lazy<Mutex<HashMap<u32, WindowRenderMode>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

const BLIT_SHADER: &str = include_str!("../../shaders/blit_shader.wgsl");

/// Returns `Ok(true)` if frame was presented, `Ok(false)` if surface is not
/// ready yet (caller should preserve dirty state and retry later).
pub(crate) fn render_and_present(
    node_id: u32,
    target: qt_compositor::QtCompositorTarget,
    scale_factor: f64,
    scene: &Scene,
    backdrop_blurs: &[crate::scene_renderer::effect_pass::BackdropBlurEffect],
    inner_shadows: &[crate::scene_renderer::effect_pass::InnerShadowEffect],
) -> napi::Result<bool> {
    let width_px = target.width_px.max(1);
    let height_px = target.height_px.max(1);

    let mut surfaces = WINDOW_SURFACES
        .lock()
        .expect("surface_renderer mutex poisoned");

    // Already destroyed — late frame drive, silently ignore.
    if matches!(surfaces.get(&node_id), Some(WindowRenderMode::Destroyed)) {
        return Ok(true);
    }

    // Ensure an entry exists. On first call, try GPU if enabled; otherwise CPU.
    if !surfaces.contains_key(&node_id) {
        if crate::runtime::window_gpu_enabled(node_id) {
            eprintln!("[qt-solid] node {node_id}: GPU mode requested");
            match create_window_surface(target) {
                Ok(ws) => {
                    surfaces.insert(node_id, WindowRenderMode::Gpu(ws));
                }
                Err(SurfaceCreationError::NotReady(e)) => {
                    eprintln!("[qt-solid] surface not ready, will retry next frame: {e}");
                    return Ok(false);
                }
                Err(SurfaceCreationError::NoGpu(e)) => {
                    eprintln!("[qt-solid] GPU not available, using CPU fallback: {e}");
                    surfaces.insert(node_id, WindowRenderMode::Cpu(None));
                }
            }
        } else {
            eprintln!("[qt-solid] node {node_id}: CPU mode (default)");
            surfaces.insert(node_id, WindowRenderMode::Cpu(None));
        }
    }

    let is_cpu = matches!(surfaces.get(&node_id), Some(WindowRenderMode::Cpu(_)));
    if is_cpu {
        // Take cached state out of the map so we don't hold the lock during render.
        let cached = match surfaces.get_mut(&node_id) {
            Some(WindowRenderMode::Cpu(state)) => state.take(),
            _ => None,
        };
        drop(surfaces);

        let cached = render_cpu_and_present(node_id, target, scale_factor, scene, cached)?;

        // Put the state back.
        let mut surfaces = WINDOW_SURFACES.lock().expect("surface_renderer mutex poisoned");
        if let Some(WindowRenderMode::Cpu(slot)) = surfaces.get_mut(&node_id) {
            *slot = Some(cached);
        }
        return Ok(true);
    }

    let Some(WindowRenderMode::Gpu(ws)) = surfaces.get_mut(&node_id) else {
        unreachable!()
    };
    if ws.config.width != width_px || ws.config.height != height_px {
        ws.config.width = width_px;
        ws.config.height = height_px;
        ws.surface.configure(&ws.device, &ws.config);
        recreate_render_texture(ws, width_px, height_px);
    }
    render_gpu_and_present(ws, width_px, height_px, scale_factor, scene, backdrop_blurs, inner_shadows)?;
    Ok(true)
}

fn render_cpu_and_present(
    node_id: u32,
    target: qt_compositor::QtCompositorTarget,
    scale_factor: f64,
    scene: &Scene,
    cached: Option<CpuRenderState>,
) -> napi::Result<CpuRenderState> {
    let width_px = target.width_px.max(1);
    let height_px = target.height_px.max(1);
    let width_u16 = u16::try_from(width_px)
        .map_err(|_| qt_error("scene width exceeds vello_cpu range".to_owned()))?;
    let height_u16 = u16::try_from(height_px)
        .map_err(|_| qt_error("scene height exceeds vello_cpu range".to_owned()))?;

    // Reuse or recreate RenderContext + Pixmap based on size match.
    let mut state = match cached {
        Some(mut s) if s.context.width() == width_u16 && s.context.height() == height_u16 => {
            s.context.reset();
            s.pixmap.resize(width_u16, height_u16);
            s
        }
        _ => CpuRenderState {
            context: vello_cpu::RenderContext::new(width_u16, height_u16),
            pixmap: vello_cpu::Pixmap::new(width_u16, height_u16),
        },
    };

    let mut painter = anyrender_vello_cpu::VelloCpuScenePainter(state.context);
    painter.append_scene(scene.clone(), Affine::scale(scale_factor));
    painter.0.flush();
    painter.0.render_to_pixmap(&mut state.pixmap);
    state.context = painter.0;

    let pixels = state.pixmap.data_as_u8_slice();
    let stride = width_px * 4;
    crate::qt::ffi::bridge::qt_window_present_cpu_frame(node_id, pixels, width_px, height_px, stride)
        .map_err(|e| qt_error(e.to_string()))?;
    Ok(state)
}

pub(crate) fn destroy_window_renderer_state(node_id: u32) {
    WINDOW_SURFACES
        .lock()
        .expect("surface renderer mutex poisoned")
        .insert(node_id, WindowRenderMode::Destroyed);
}

fn render_gpu_and_present(
    ws: &mut WindowSurface,
    width_px: u32,
    height_px: u32,
    scale_factor: f64,
    scene: &Scene,
    backdrop_blurs: &[crate::scene_renderer::effect_pass::BackdropBlurEffect],
    inner_shadows: &[crate::scene_renderer::effect_pass::InnerShadowEffect],
) -> napi::Result<()> {
    let mut encoder = ws.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("qt-solid-surface-renderer-encoder"),
    });

    // Clear the persistent render texture before vello draws into it.
    // vello_hybrid renders with LoadOp::Load (no clear) on the output target,
    // so stale pixels from prior frames bleed through transparent regions.
    {
        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("qt-solid-surface-clear-render-texture"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &ws.render_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    }

    // Build vello_hybrid scene from logical scene.
    let hybrid_scene = build_hybrid_scene(
        width_px, height_px, scale_factor, scene,
        &mut ws.renderer, &ws.device, &ws.queue, &mut encoder,
        &mut ws.image_cache,
    )?;

    sweep_stale_images(
        scene, &mut ws.renderer, &ws.device, &ws.queue, &mut encoder,
        &mut ws.image_cache,
    );

    ws.renderer
        .render(
            &hybrid_scene,
            &ws.device,
            &ws.queue,
            &mut encoder,
            &RenderSize {
                width: width_px,
                height: height_px,
            },
            &ws.render_view,
        )
        .map_err(|e| qt_error(format!("vello render failed: {e}")))?;

    // Apply post-process effect passes onto the intermediate texture.
    let tex_size = (width_px, height_px);
    if !backdrop_blurs.is_empty() || !inner_shadows.is_empty() {
        // Submit the vello encoder first so effects read the finished scene.
        ws.queue.submit([encoder.finish()]);
        let mut fx_encoder = ws.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("qt-solid-surface-effect-encoder"),
        });
        crate::scene_renderer::effect_pass::apply_backdrop_blurs(
            &ws.device, &ws.queue, &mut fx_encoder,
            &ws.render_texture, &ws.render_view,
            tex_size, backdrop_blurs,
        );
        crate::scene_renderer::effect_pass::apply_inner_shadows(
            &ws.device, &ws.queue, &mut fx_encoder,
            &ws.render_view, tex_size, inner_shadows,
        );
        // Replace encoder for the blit pass.
        encoder = fx_encoder;
    }

    // Acquire surface texture and blit.
    let surface_texture = ws
        .surface
        .get_current_texture()
        .map_err(|e| qt_error(format!("surface acquire failed: {e}")))?;
    let surface_view = surface_texture
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("qt-solid-surface-blit-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &surface_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&ws.blit_pipeline);
        pass.set_bind_group(0, &ws.render_bind_group, &[]);
        pass.draw(0..3, 0..1);
    }

    ws.queue.submit([encoder.finish()]);
    surface_texture.present();
    Ok(())
}


/// Pre-configure the surface to match new window dimensions.
/// Must be called before CA commits the transaction so that drawableSize
/// and layer bounds change atomically (preventing stretched frames).
pub(crate) fn resize_surface(node_id: u32, width_px: u32, height_px: u32) {
    let width_px = width_px.max(1);
    let height_px = height_px.max(1);
    let mut surfaces = WINDOW_SURFACES
        .lock()
        .expect("surface_renderer mutex poisoned");
    let Some(WindowRenderMode::Gpu(ws)) = surfaces.get_mut(&node_id) else {
        return;
    };
    if ws.config.width != width_px || ws.config.height != height_px {
        ws.config.width = width_px;
        ws.config.height = height_px;
        ws.surface.configure(&ws.device, &ws.config);
        recreate_render_texture(ws, width_px, height_px);
    }
}

/// Cheap present: acquire surface texture, blit the existing render_texture
/// (which may contain stale content from last full render), and present.
/// Used during live resize throttling to avoid window server stretching.
pub(crate) fn blit_and_present(node_id: u32) -> napi::Result<()> {
    let mut surfaces = WINDOW_SURFACES
        .lock()
        .expect("surface_renderer mutex poisoned");
    let Some(WindowRenderMode::Gpu(ws)) = surfaces.get_mut(&node_id) else {
        return Ok(());
    };

    let surface_texture = ws
        .surface
        .get_current_texture()
        .map_err(|e| qt_error(format!("surface acquire failed: {e}")))?;
    let surface_view = surface_texture
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = ws.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("qt-solid-surface-blit-only-encoder"),
    });

    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("qt-solid-surface-blit-only-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &surface_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&ws.blit_pipeline);
        pass.set_bind_group(0, &ws.render_bind_group, &[]);
        pass.draw(0..3, 0..1);
    }

    ws.queue.submit([encoder.finish()]);
    surface_texture.present();
    Ok(())
}

/// Return the raw CAMetalLayer pointer for the given surface (macOS only).
/// Returns 0 if no surface exists for the node.
#[cfg(target_os = "macos")]
pub(crate) fn metal_layer_ptr(node_id: u32) -> u64 {
    let surfaces = WINDOW_SURFACES
        .lock()
        .expect("surface_renderer mutex poisoned");
    surfaces
        .get(&node_id)
        .and_then(|mode| match mode {
            WindowRenderMode::Gpu(ws) => Some(ws.metal_layer_ptr.0 as u64),
            _ => None,
        })
        .unwrap_or(0)
}

fn create_window_surface(
    target: qt_compositor::QtCompositorTarget,
) -> Result<WindowSurface, SurfaceCreationError> {
    // Try GL first (lower memory overhead, no DXGI swap chain conflict).
    // Fall back to DX12 (DComp visual) if GL is unavailable.
    create_window_surface_with_backends(target, wgpu::Backends::GL)
        .or_else(|_| create_window_surface_with_backends(target, wgpu::Backends::default()))
}

fn create_window_surface_with_backends(
    target: qt_compositor::QtCompositorTarget,
    backends: wgpu::Backends,
) -> Result<WindowSurface, SurfaceCreationError> {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends,
        backend_options: wgpu::BackendOptions {
            dx12: wgpu::Dx12BackendOptions {
                presentation_system: wgpu::Dx12SwapchainKind::DxgiFromVisual,
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    });

    #[cfg(target_os = "macos")]
    let (surface, metal_layer_ptr) = {
        let ns_view = NonNull::new(target.primary_handle as *mut c_void)
            .ok_or_else(|| SurfaceCreationError::NoGpu("NSView handle is null".into()))?;
        let layer = unsafe { raw_window_metal::Layer::from_ns_view(ns_view) };
        let layer_ptr = layer.as_ptr().as_ptr() as *mut c_void;
        let surface = unsafe {
            instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(
                    layer_ptr,
                ))
                .map_err(|e| SurfaceCreationError::NoGpu(format!("create surface: {e}")))?
        };
        // Leak the Layer so the CAMetalLayer stays alive for the surface lifetime.
        std::mem::forget(layer);
        (surface, SendPtr(layer_ptr))
    };

    #[cfg(not(target_os = "macos"))]
    let surface = {
        let surface_target = unsafe { qt_compositor::compositor_surface_target(target) }
            .map_err(|e| SurfaceCreationError::NoGpu(format!("resolve surface target: {e}")))?;
        unsafe {
            instance
                .create_surface_unsafe(surface_target)
                .map_err(|e| SurfaceCreationError::NoGpu(format!("create surface: {e}")))?
        }
    };
    let adapter = pollster::block_on(
        instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }),
    )
    .map_err(|e| SurfaceCreationError::NoGpu(format!("request adapter: {e}")))?;

    let (device, queue) = pollster::block_on(
        adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("qt-solid-surface-renderer-device"),
            ..Default::default()
        }),
    )
    .map_err(|e| SurfaceCreationError::NoGpu(format!("request device: {e}")))?;

    let capabilities = surface.get_capabilities(&adapter);
    if capabilities.formats.is_empty() {
        return Err(SurfaceCreationError::NotReady(
            "surface has no supported formats (adapter may not be compatible yet)".into(),
        ));
    }

    let width_px = target.width_px.max(1);
    let height_px = target.height_px.max(1);

    let config = surface
        .get_default_config(&adapter, width_px, height_px)
        .ok_or_else(|| SurfaceCreationError::NotReady(
            "surface.get_default_config returned None".into(),
        ))?;

    // Override format: vello outputs sRGB-encoded values into Rgba8Unorm.
    // Use a non-sRGB surface format so the blit pass copies verbatim without
    // an additional linear→sRGB conversion (double-gamma).
    let surface_format = capabilities
        .formats
        .iter()
        .find(|f| !f.is_srgb())
        .copied()
        .unwrap_or(capabilities.formats[0]);
    let config = wgpu::SurfaceConfiguration {
        format: surface_format,
        present_mode: wgpu::PresentMode::AutoVsync,
        desired_maximum_frame_latency: 2,
        alpha_mode: wgpu::CompositeAlphaMode::Auto,
        ..config
    };
    // Set a non-panicking error handler so surface.configure failures are
    // recoverable (default handler panics on validation errors).
    let configure_failed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let configure_failed_clone = configure_failed.clone();
    device.on_uncaptured_error(std::sync::Arc::new(move |error: wgpu::Error| {
        eprintln!("[qt-solid] wgpu uncaptured error: {error}");
        configure_failed_clone.store(true, std::sync::atomic::Ordering::SeqCst);
    }));
    device.set_device_lost_callback(|_reason, _message| {});

    surface.configure(&device, &config);

    if configure_failed.load(std::sync::atomic::Ordering::SeqCst) {
        return Err(SurfaceCreationError::NotReady(format!(
            "surface.configure failed for hwnd=0x{:x} adapter={:?}",
            target.primary_handle, adapter.get_info().name,
        )));
    }

    let renderer = Renderer::new_with(
        &device,
        &RenderTargetConfig {
            format: wgpu::TextureFormat::Rgba8Unorm,
            width: width_px,
            height: height_px,
        },
        RenderSettings {
            atlas_config: AtlasConfig {
                initial_atlas_count: 1,
                max_atlases: 4,
                atlas_size: (2048, 2048),
                ..Default::default()
            },
            ..Default::default()
        },
    );
    // Blit pipeline: Rgba8Unorm → surface format (sRGB).
    let blit_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("qt-solid-surface-blit-shader"),
        source: wgpu::ShaderSource::Wgsl(BLIT_SHADER.into()),
    });
    let blit_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("qt-solid-surface-blit-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
    let blit_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("qt-solid-surface-blit-pl"),
        bind_group_layouts: &[&blit_bind_group_layout],
        immediate_size: 0,
    });
    let blit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("qt-solid-surface-blit-pipeline"),
        layout: Some(&blit_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &blit_shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &blit_shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_format,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        multiview_mask: None,
        cache: None,
    });
    let blit_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("qt-solid-surface-blit-sampler"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    let (render_texture, render_view, render_bind_group) =
        create_render_texture(&device, &blit_bind_group_layout, &blit_sampler, width_px, height_px);

    Ok(WindowSurface {
        surface,
        device,
        queue,
        config,
        renderer,
        image_cache: HybridImageCache::default(),
        render_texture,
        render_view,
        render_bind_group,
        blit_pipeline,
        blit_bind_group_layout,
        blit_sampler,
        #[cfg(target_os = "macos")]
        metal_layer_ptr,
    })
}

fn create_render_texture(
    device: &wgpu::Device,
    bgl: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    width_px: u32,
    height_px: u32,
) -> (wgpu::Texture, wgpu::TextureView, wgpu::BindGroup) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("qt-solid-surface-render-texture"),
        size: wgpu::Extent3d {
            width: width_px.max(1),
            height: height_px.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("qt-solid-surface-blit-bg"),
        layout: bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&view),
            },
        ],
    });
    (texture, view, bind_group)
}

fn recreate_render_texture(ws: &mut WindowSurface, width_px: u32, height_px: u32) {
    let (texture, view, bind_group) = create_render_texture(
        &ws.device,
        &ws.blit_bind_group_layout,
        &ws.blit_sampler,
        width_px,
        height_px,
    );
    ws.render_texture = texture;
    ws.render_view = view;
    ws.render_bind_group = bind_group;
}

fn build_hybrid_scene(
    width_px: u32,
    height_px: u32,
    scale_factor: f64,
    scene: &Scene,
    renderer: &mut Renderer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    image_cache: &mut HybridImageCache,
) -> napi::Result<HybridScene> {
    let width_u16 = u16::try_from(width_px)
        .map_err(|_| qt_error("scene width exceeds vello_hybrid range"))?;
    let height_u16 = u16::try_from(height_px)
        .map_err(|_| qt_error("scene height exceeds vello_hybrid range"))?;
    let mut hybrid_scene = HybridScene::new(width_u16, height_u16);
    let image_manager = anyrender_vello_hybrid::ImageManager::new(
        renderer, device, queue, encoder, image_cache,
    );
    let mut painter = anyrender_vello_hybrid::VelloHybridScenePainter::new(
        &mut hybrid_scene, image_manager,
    );
    painter.append_scene(scene.clone(), Affine::scale(scale_factor));
    Ok(hybrid_scene)
}
