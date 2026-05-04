use std::{
    collections::HashMap,
    sync::Mutex,
};
#[cfg(target_os = "macos")]
use std::{ffi::c_void, ptr::NonNull};

use once_cell::sync::Lazy;
pub(crate) mod effects;
#[cfg(not(target_os = "macos"))]
pub(crate) mod surface;
use crate::canvas::fragment::{FragmentId, FragmentLayerKey, RenderPlan};
use crate::image::{ImageCache, sweep_stale_images};
use vello::wgpu;
use vello_hybrid::{AtlasConfig, RenderSettings, RenderSize, RenderTargetConfig, Renderer, Scene as GpuScene};
use anyrender_vello_hybrid::Recording;

use crate::canvas::vello::Scene;
use crate::canvas::vello::peniko::kurbo::Affine;
use anyrender::PaintScene;
use crate::runtime::qt_error;

/// Per-promoted-layer GPU texture state.
struct LayerTextureState {
    view: wgpu::TextureView,
    width: u32,
    height: u32,
    /// Retained uniform buffer for composite shader (UNIFORM | COPY_DST).
    uniform_buffer: wgpu::Buffer,
    /// Cached bind group: sampler + texture view + uniform buffer.
    composite_bind_group: wgpu::BindGroup,
}

/// Per-window GPU surface state: wgpu device/queue/surface + vello renderer.
struct WindowSurface {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    image_cache: ImageCache,
    /// Retained base texture — vello renders here. Persists across frames for
    /// partial rendering (LoadOp::Load retains last frame pixels).
    base_texture: wgpu::Texture,
    base_view: wgpu::TextureView,
    base_bind_group: wgpu::BindGroup,
    /// Output texture — effects (backdrop blur, inner shadow) are applied here.
    /// When effects are active: base is copied to output, effects run on output,
    /// output is blitted to surface. When no effects: base is blitted directly.
    output_texture: wgpu::Texture,
    output_view: wgpu::TextureView,
    output_bind_group: wgpu::BindGroup,
    blit_pipeline: wgpu::RenderPipeline,
    blit_bind_group_layout: wgpu::BindGroupLayout,
    blit_sampler: wgpu::Sampler,
    /// Composite pipeline for drawing promoted layer textures with transform/opacity.
    composite_pipeline: wgpu::RenderPipeline,
    composite_bind_group_layout: wgpu::BindGroupLayout,
    /// Per-promoted-layer retained textures.
    layer_textures: HashMap<FragmentLayerKey, LayerTextureState>,
    /// Retained GPU Scene (vello_hybrid) to avoid per-frame alloc/dealloc.
    /// Reused via `reset()` when viewport size hasn't changed.
    retained_gpu_scene: Option<GpuScene>,
    /// Retained zero buffer for dirty rect clearing. Avoids per-frame
    /// staging buffer allocation in `write_texture`.
    zero_buffer: Option<(wgpu::Buffer, usize)>,
    /// Per-subtree cached Recordings for strip caching.
    subtree_recordings: HashMap<FragmentId, Recording>,
}

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

const BLIT_SHADER: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/shaders/blit_shader.wgsl"));
const COMPOSITE_LAYER_SHADER: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/shaders/composite_layer.wgsl"));

/// Like `render_and_present` but accepts per-subtree scenes with dirty flags
/// for Recording-based strip caching. GPU path only (CPU falls back to merged).
pub(crate) fn render_and_present_subtrees(
    node_id: u32,
    target: crate::renderer::types::SurfaceTarget,
    scale_factor: f64,
    subtrees: Vec<(crate::canvas::fragment::FragmentId, Scene, bool)>,
    backdrop_blurs: &[effects::BackdropBlurEffect],
    inner_shadows: &[effects::InnerShadowEffect],
    dirty_rects: Option<&[(u32, u32, u32, u32)]>,
) -> napi::Result<bool> {
    let width_px = target.width_px.max(1);
    let height_px = target.height_px.max(1);

    let mut surfaces = WINDOW_SURFACES
        .lock()
        .expect("surface_renderer mutex poisoned");

    if matches!(surfaces.get(&node_id), Some(WindowRenderMode::Destroyed)) {
        return Ok(true);
    }

    if !surfaces.contains_key(&node_id) {
        if cfg!(target_os = "macos") || crate::renderer::with_renderer(|r| r.gpu_enabled(node_id)) {
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

    // CPU path: merge subtrees into single scene, use existing render path.
    let is_cpu = matches!(surfaces.get(&node_id), Some(WindowRenderMode::Cpu(_)));
    if is_cpu {
        let mut merged = Scene::new();
        for (_, sub, _) in &subtrees {
            merged.append_scene(sub.clone(), Affine::IDENTITY);
        }
        let cached = match surfaces.get_mut(&node_id) {
            Some(WindowRenderMode::Cpu(state)) => state.take(),
            _ => None,
        };
        drop(surfaces);
        let cached = render_cpu_and_present(node_id, target, scale_factor, &merged, cached)?;
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
        recreate_render_textures(ws, width_px, height_px);
    }
    render_gpu_and_present_subtrees(ws, width_px, height_px, scale_factor, &subtrees, backdrop_blurs, inner_shadows, dirty_rects)
}

fn render_cpu_and_present(
    node_id: u32,
    target: crate::renderer::types::SurfaceTarget,
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

fn render_gpu_and_present_subtrees(
    ws: &mut WindowSurface,
    width_px: u32,
    height_px: u32,
    scale_factor: f64,
    subtrees: &[(FragmentId, Scene, bool)],
    backdrop_blurs: &[effects::BackdropBlurEffect],
    inner_shadows: &[effects::InnerShadowEffect],
    dirty_rects: Option<&[(u32, u32, u32, u32)]>,
) -> napi::Result<bool> {
    let mut encoder = ws.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("qt-solid-surface-renderer-encoder"),
    });

    let is_noop = matches!(dirty_rects, Some(rects) if rects.is_empty());
    let partial_rects = dirty_rects.filter(|r| !r.is_empty());

    if is_noop {
        // Re-present retained content. Use output_texture when effects are
        // active — it already contains base + backdrop blur / inner shadow
        // from the most recent full render. Without this, the noop path
        // would blit the raw base_texture, dropping all post-process effects.
        let has_effects = !backdrop_blurs.is_empty() || !inner_shadows.is_empty();

        let surface_texture = ws
            .surface
            .get_current_texture()
            .map_err(|e| qt_error(format!("surface acquire failed: {e}")))?;
        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let noop_bind_group = if has_effects { &ws.output_bind_group } else { &ws.base_bind_group };
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("qt-solid-surface-blit-noop"),
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
            pass.set_bind_group(0, noop_bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        ws.queue.submit([encoder.finish()]);
        surface_texture.present();
        return Ok(true);
    }

    if partial_rects.is_none() {
        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("qt-solid-surface-clear-base-texture"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &ws.base_view,
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
    } else {
        for &(dx, dy, dw, dh) in partial_rects.unwrap() {
            let dx = dx.min(width_px);
            let dy = dy.min(height_px);
            let dw = dw.min(width_px.saturating_sub(dx));
            let dh = dh.min(height_px.saturating_sub(dy));
            clear_texture_rect(
                &ws.device, &mut encoder, &ws.base_texture,
                &mut ws.zero_buffer, dx, dy, dw, dh,
            );
        }
    }

    // Build GPU scene using per-subtree Recording cache.
    let retained = ws.retained_gpu_scene.take();
    let gpu_scene = build_gpu_scene_from_subtrees(
        width_px, height_px, scale_factor, subtrees,
        &mut ws.renderer, &ws.device, &ws.queue, &mut encoder,
        &mut ws.image_cache,
        retained,
        &mut ws.subtree_recordings,
    )?;

    // Sweep stale images across all subtrees.
    for (_, sub_scene, _) in subtrees {
        sweep_stale_images(
            sub_scene, &mut ws.renderer, &ws.device, &ws.queue, &mut encoder,
            &mut ws.image_cache,
        );
    }

    ws.renderer
        .render(
            &gpu_scene,
            &ws.device,
            &ws.queue,
            &mut encoder,
            &RenderSize {
                width: width_px,
                height: height_px,
            },
            &ws.base_view,
        )
        .map_err(|e| qt_error(format!("vello render failed: {e}")))?;

    let has_effects = !backdrop_blurs.is_empty() || !inner_shadows.is_empty();
    let tex_size = (width_px, height_px);

    let has_effects = if has_effects {
        ws.queue.submit([encoder.finish()]);
        let mut fx_encoder = ws.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("qt-solid-surface-effect-encoder"),
        });
        fx_encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &ws.base_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &ws.output_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: width_px,
                height: height_px,
                depth_or_array_layers: 1,
            },
        );
        effects::apply_backdrop_blurs(
            &ws.device, &ws.queue, &mut fx_encoder,
            &ws.output_texture, &ws.output_view,
            tex_size, backdrop_blurs,
        );
        effects::apply_inner_shadows(
            &ws.device, &ws.queue, &mut fx_encoder,
            &ws.output_view, tex_size, inner_shadows,
        );
        encoder = fx_encoder;
        true
    } else {
        false
    };

    // Determine blit source bind group for wgpu Surface path.
    let blit_bind_group = if has_effects {
        &ws.output_bind_group
    } else {
        &ws.base_bind_group
    };

    // --- wgpu Surface present path ---
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
        pass.set_bind_group(0, blit_bind_group, &[]);
        pass.draw(0..3, 0..1);
    }

    ws.queue.submit([encoder.finish()]);
    ws.retained_gpu_scene = Some(gpu_scene);
    surface_texture.present();
    Ok(true)
}

/// Render a partitioned RenderPlan: inline scene to base_texture, promoted
/// layers to per-layer textures, then composite everything onto the surface.
pub(crate) fn render_composited_and_present(
    node_id: u32,
    target: crate::renderer::types::SurfaceTarget,
    scale_factor: f64,
    render_plan: RenderPlan,
    backdrop_blurs: &[effects::BackdropBlurEffect],
    inner_shadows: &[effects::InnerShadowEffect],
    dirty_rects: Option<&[(u32, u32, u32, u32)]>,
) -> napi::Result<bool> {
    let width_px = target.width_px.max(1);
    let height_px = target.height_px.max(1);

    let mut surfaces = WINDOW_SURFACES
        .lock()
        .expect("surface_renderer mutex poisoned");

    if matches!(surfaces.get(&node_id), Some(WindowRenderMode::Destroyed)) {
        return Ok(true);
    }

    // Ensure GPU surface exists (same as render_and_present).
    if !surfaces.contains_key(&node_id) {
        if cfg!(target_os = "macos") || crate::renderer::with_renderer(|r| r.gpu_enabled(node_id)) {
            match create_window_surface(target) {
                Ok(ws) => { surfaces.insert(node_id, WindowRenderMode::Gpu(ws)); }
                Err(SurfaceCreationError::NotReady(e)) => {
                    eprintln!("[qt-solid] surface not ready: {e}");
                    return Ok(false);
                }
                Err(SurfaceCreationError::NoGpu(e)) => {
                    eprintln!("[qt-solid] GPU unavailable for composited render: {e}");
                    // Compositor layers require GPU — can't fallback to CPU.
                    return Ok(false);
                }
            }
        }
    }

    let Some(WindowRenderMode::Gpu(ws)) = surfaces.get_mut(&node_id) else {
        return Ok(false);
    };

    // Handle resize.
    if ws.config.width != width_px || ws.config.height != height_px {
        ws.config.width = width_px;
        ws.config.height = height_px;
        ws.surface.configure(&ws.device, &ws.config);
        recreate_render_textures(ws, width_px, height_px);
    }

    // Clean up stale layer textures.
    for key in &render_plan.stale_keys {
        ws.layer_textures.remove(key);
    }

    let pose_only = render_plan.pose_only;

    let mut encoder = ws.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("qt-solid-composited-encoder"),
    });

    // --- Step 1: Render base scene into base_texture (unless pose-only) ---
    if !pose_only {
        let is_noop = matches!(dirty_rects, Some(rects) if rects.is_empty());
        let partial_rects = dirty_rects.filter(|r| !r.is_empty());

        if is_noop && render_plan.composited_layers.is_empty() {
            // Nothing dirty, no layers — just re-present.
            let surface_texture = ws.surface.get_current_texture()
                .map_err(|e| qt_error(format!("surface acquire: {e}")))?;
            let surface_view = surface_texture.texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("qt-solid-composited-noop-blit"),
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
                pass.set_bind_group(0, &ws.base_bind_group, &[]);
                pass.draw(0..3, 0..1);
            }
            ws.queue.submit([encoder.finish()]);
            surface_texture.present();
            return Ok(true);
        }

        // Clear or partial-clear base_texture.
        if partial_rects.is_none() {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("qt-solid-composited-clear-base"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &ws.base_view,
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
        } else {
            for &(dx, dy, dw, dh) in partial_rects.unwrap() {
                let dx = dx.min(width_px);
                let dy = dy.min(height_px);
                let dw = dw.min(width_px.saturating_sub(dx));
                let dh = dh.min(height_px.saturating_sub(dy));
                clear_texture_rect(
                    &ws.device, &mut encoder, &ws.base_texture,
                    &mut ws.zero_buffer, dx, dy, dw, dh,
                );
            }
        }

        // Vello render base scene into base_texture.
        let retained = ws.retained_gpu_scene.take();
        let gpu_scene = build_gpu_scene(
            width_px, height_px, scale_factor, &render_plan.base_scene,
            &mut ws.renderer, &ws.device, &ws.queue, &mut encoder,
            &mut ws.image_cache,
            retained,
        )?;
        sweep_stale_images(
            &render_plan.base_scene, &mut ws.renderer, &ws.device, &ws.queue, &mut encoder,
            &mut ws.image_cache,
        );
        ws.renderer.render(
            &gpu_scene, &ws.device, &ws.queue, &mut encoder,
            &RenderSize { width: width_px, height: height_px },
            &ws.base_view,
        ).map_err(|e| qt_error(format!("vello base render: {e}")))?;
        ws.retained_gpu_scene = Some(gpu_scene);
    }

    // --- Step 2: Render each composited layer into its own texture ---
    for layer in &render_plan.composited_layers {
        if !layer.content_dirty && ws.layer_textures.contains_key(&layer.layer_key) {
            continue; // Reuse retained texture.
        }

        // Determine layer texture size from bounds (in device pixels).
        let lw = ((layer.bounds.width() * scale_factor).ceil() as u32).max(1);
        let lh = ((layer.bounds.height() * scale_factor).ceil() as u32).max(1);

        // Recreate texture if size changed or new layer.
        let needs_recreate = ws.layer_textures.get(&layer.layer_key)
            .map_or(true, |t| t.width != lw || t.height != lh);
        if needs_recreate {
            let texture = ws.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("qt-solid-layer-texture"),
                size: wgpu::Extent3d { width: lw, height: lh, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let uniform_buffer = ws.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("qt-solid-layer-uniform"),
                size: 64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let composite_bind_group = ws.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("qt-solid-layer-composite-bg"),
                layout: &ws.composite_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Sampler(&ws.blit_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                ],
            });
            ws.layer_textures.insert(layer.layer_key, LayerTextureState {
                view, width: lw, height: lh,
                uniform_buffer, composite_bind_group,
            });
        }

        let lt = ws.layer_textures.get(&layer.layer_key).unwrap();

        // Clear layer texture.
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("qt-solid-layer-clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &lt.view,
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

        // Vello render layer scene into layer texture.
        let gpu_scene = build_gpu_scene(
            lw, lh, scale_factor, &layer.scene,
            &mut ws.renderer, &ws.device, &ws.queue, &mut encoder,
            &mut ws.image_cache,
            None,
        )?;
        sweep_stale_images(
            &layer.scene, &mut ws.renderer, &ws.device, &ws.queue, &mut encoder,
            &mut ws.image_cache,
        );
        ws.renderer.render(
            &gpu_scene, &ws.device, &ws.queue, &mut encoder,
            &RenderSize { width: lw, height: lh },
            &lt.view,
        ).map_err(|e| qt_error(format!("vello layer render: {e}")))?;
    }

    // --- Step 3: Effects pass on base_texture → output_texture ---
    let has_effects = !backdrop_blurs.is_empty() || !inner_shadows.is_empty();
    let blit_bind_group = if has_effects && !pose_only {
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &ws.base_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &ws.output_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d { width: width_px, height: height_px, depth_or_array_layers: 1 },
        );
        effects::apply_backdrop_blurs(
            &ws.device, &ws.queue, &mut encoder,
            &ws.output_texture, &ws.output_view,
            (width_px, height_px), backdrop_blurs,
        );
        effects::apply_inner_shadows(
            &ws.device, &ws.queue, &mut encoder,
            &ws.output_view, (width_px, height_px), inner_shadows,
        );
        &ws.output_bind_group
    } else {
        &ws.base_bind_group
    };

    // --- Step 4: Composite pass → surface ---
    let surface_texture = ws.surface.get_current_texture()
        .map_err(|e| qt_error(format!("surface acquire: {e}")))?;
    let surface_view = surface_texture.texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    {
        // Blit base_texture (or output_texture if effects) to surface.
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("qt-solid-composited-blit-base"),
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
        pass.set_bind_group(0, blit_bind_group, &[]);
        pass.draw(0..3, 0..1);
    }

    // Overdraw each composited layer.
    if !render_plan.composited_layers.is_empty() {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("qt-solid-composited-layers-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &surface_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&ws.composite_pipeline);

        for layer in &render_plan.composited_layers {
            let Some(lt) = ws.layer_textures.get(&layer.layer_key) else {
                continue;
            };

            // Update retained uniform buffer (zero alloc).
            let coeffs = layer.transform.as_coeffs();
            let uniform_data = make_layer_uniform(
                &coeffs, layer.bounds.x0, layer.bounds.y0,
                layer.bounds.width(), layer.bounds.height(),
                width_px as f64 / scale_factor, height_px as f64 / scale_factor,
                layer.opacity,
            );
            ws.queue.write_buffer(&lt.uniform_buffer, 0, &uniform_data);

            pass.set_bind_group(0, &lt.composite_bind_group, &[]);
            pass.draw(0..6, 0..1);
        }
    }

    ws.queue.submit([encoder.finish()]);
    surface_texture.present();
    Ok(true)
}
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
        recreate_render_textures(ws, width_px, height_px);
    }
}

/// Convert a SurfaceTarget to wgpu's SurfaceTargetUnsafe via raw-window-handle.
#[cfg(not(target_os = "macos"))]
pub(crate) fn compositor_surface_target(
    target: &crate::renderer::types::SurfaceTarget,
) -> napi::Result<wgpu::SurfaceTargetUnsafe> {
    use crate::renderer::types::SurfaceHandle;
    use raw_window_handle::*;

    match target.handle {
        SurfaceHandle::Win32(hwnd) => Ok(wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: RawDisplayHandle::Windows(WindowsDisplayHandle::new()),
            raw_window_handle: RawWindowHandle::Win32(Win32WindowHandle::new(hwnd)),
        }),
        SurfaceHandle::Xcb { window, connection } => Ok(wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: RawDisplayHandle::Xcb(XcbDisplayHandle::new(
                Some(connection),
                0,
            )),
            raw_window_handle: RawWindowHandle::Xcb(XcbWindowHandle::new(window)),
        }),
        SurfaceHandle::Wayland { surface, display } => Ok(wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle: RawDisplayHandle::Wayland(WaylandDisplayHandle::new(display)),
            raw_window_handle: RawWindowHandle::Wayland(WaylandWindowHandle::new(surface)),
        }),
        _ => Err(qt_error("unsupported surface handle for compositor_surface_target")),
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn resolve_metal_layer_for_ns_view(ns_view: NonNull<c_void>) -> NonNull<c_void> {
    use objc2::msg_send;
    use objc2::runtime::AnyClass;
    use objc2_foundation::NSObjectProtocol;
    use objc2_quartz_core::CALayer;

    let view: &objc2::runtime::AnyObject = unsafe { ns_view.cast().as_ref() };
    let root_layer: Option<objc2::rc::Retained<CALayer>> = unsafe { msg_send![view, layer] };
    if let Some(root_layer) = root_layer {
        let sublayers: Option<objc2::rc::Retained<objc2_foundation::NSArray<CALayer>>> =
            unsafe { msg_send![&*root_layer, sublayers] };
        if let Some(sublayers) = sublayers {
            let metal_cls =
                AnyClass::get(c"CAMetalLayer").expect("CAMetalLayer class not found");
            for i in 0..sublayers.count() {
                let sub: &CALayer = unsafe { msg_send![&*sublayers, objectAtIndex: i] };
                if sub.isKindOfClass(metal_cls) {
                    let ptr = sub as *const CALayer as *mut c_void;
                    return NonNull::new(ptr).unwrap();
                }
            }
        }
    }
    let layer = unsafe { raw_window_metal::Layer::from_ns_view(ns_view) };
    let ptr = layer.as_ptr().as_ptr() as *mut c_void;
    std::mem::forget(layer);
    NonNull::new(ptr).unwrap()
}

fn create_window_surface(
    target: crate::renderer::types::SurfaceTarget,
) -> Result<WindowSurface, SurfaceCreationError> {
    if cfg!(target_os = "windows") {
        // Vulkan has lower per-device memory overhead than GL or DX12 on
        // Windows (explicit API = less driver-side implicit state).
        // Fallback to DX12 if Vulkan is unavailable.
        create_window_surface_with_backends(target, wgpu::Backends::VULKAN)
            .or_else(|_| create_window_surface_with_backends(target, wgpu::Backends::DX12))
    } else {
        create_window_surface_with_backends(target, wgpu::Backends::default())
    }
}

fn create_window_surface_with_backends(
    target: crate::renderer::types::SurfaceTarget,
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
    let surface = {
        use crate::renderer::types::SurfaceHandle;
        let SurfaceHandle::AppKit(ns_view) = target.handle else {
            return Err(SurfaceCreationError::NoGpu("expected AppKit handle on macOS".into()));
        };
        let layer_ptr = resolve_metal_layer_for_ns_view(ns_view).as_ptr();
        let surface = unsafe {
            instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(
                    layer_ptr,
                ))
                .map_err(|e| SurfaceCreationError::NoGpu(format!("create surface: {e}")))?
        };
        surface
    };

    #[cfg(not(target_os = "macos"))]
    let surface = {
        let surface_target = compositor_surface_target(&target)
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
    // Pick the first non-Opaque alpha mode so surface.configure() sets
    // CAMetalLayer.opaque = NO. On macOS Metal this is typically PostMultiplied.
    // `Auto` resolves to `Opaque`, which makes the layer opaque regardless of
    // pixel alpha — causing black backgrounds on transparent windows (popups).
    let alpha_mode = capabilities
        .alpha_modes
        .iter()
        .copied()
        .find(|m| *m != wgpu::CompositeAlphaMode::Opaque && *m != wgpu::CompositeAlphaMode::Auto)
        .unwrap_or(wgpu::CompositeAlphaMode::Auto);
    let config = wgpu::SurfaceConfiguration {
        format: surface_format,
        present_mode: wgpu::PresentMode::AutoVsync,
        desired_maximum_frame_latency: 2,
        alpha_mode,
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
            "surface.configure failed for target={:?} adapter={:?}",
            target.handle, adapter.get_info().name,
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

    let (base_texture, base_view, base_bind_group) =
        create_render_texture(&device, &blit_bind_group_layout, &blit_sampler, width_px, height_px, "base");
    let (output_texture, output_view, output_bind_group) =
        create_render_texture(&device, &blit_bind_group_layout, &blit_sampler, width_px, height_px, "output");

    // Composite pipeline: draw promoted layer textures with transform/opacity.
    let composite_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("qt-solid-composite-layer-shader"),
        source: wgpu::ShaderSource::Wgsl(COMPOSITE_LAYER_SHADER.into()),
    });
    let composite_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("qt-solid-composite-layer-bgl"),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(64),
                    },
                    count: None,
                },
            ],
        });
    let composite_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("qt-solid-composite-layer-pl"),
        bind_group_layouts: &[&composite_bind_group_layout],
        immediate_size: 0,
    });
    let composite_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("qt-solid-composite-layer-pipeline"),
        layout: Some(&composite_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &composite_shader,
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
            module: &composite_shader,
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

    Ok(WindowSurface {
        surface,
        device,
        queue,
        config,
        renderer,
        image_cache: ImageCache::default(),
        base_texture,
        base_view,
        base_bind_group,
        output_texture,
        output_view,
        output_bind_group,
        blit_pipeline,
        blit_bind_group_layout,
        blit_sampler,
        composite_pipeline,
        composite_bind_group_layout,
        layer_textures: HashMap::new(),
        retained_gpu_scene: None,
        zero_buffer: None,
        subtree_recordings: HashMap::new(),
    })
}

fn create_render_texture(
    device: &wgpu::Device,
    bgl: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    width_px: u32,
    height_px: u32,
    label_suffix: &str,
) -> (wgpu::Texture, wgpu::TextureView, wgpu::BindGroup) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(Box::leak(format!("qt-solid-surface-{label_suffix}-texture").into_boxed_str())),
        size: wgpu::Extent3d {
            width: width_px.max(1),
            height: height_px.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(Box::leak(format!("qt-solid-surface-{label_suffix}-blit-bg").into_boxed_str())),
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

fn recreate_render_textures(ws: &mut WindowSurface, width_px: u32, height_px: u32) {
    let (texture, view, bind_group) = create_render_texture(
        &ws.device,
        &ws.blit_bind_group_layout,
        &ws.blit_sampler,
        width_px,
        height_px,
        "base",
    );
    ws.base_texture = texture;
    ws.base_view = view;
    ws.base_bind_group = bind_group;

    let (texture, view, bind_group) = create_render_texture(
        &ws.device,
        &ws.blit_bind_group_layout,
        &ws.blit_sampler,
        width_px,
        height_px,
        "output",
    );
    ws.output_texture = texture;
    ws.output_view = view;
    ws.output_bind_group = bind_group;

    // Viewport size changed — cached recordings have stale strip data.
    ws.subtree_recordings.clear();
}

fn build_gpu_scene(
    width_px: u32,
    height_px: u32,
    scale_factor: f64,
    scene: &Scene,
    renderer: &mut Renderer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    image_cache: &mut ImageCache,
    retained: Option<GpuScene>,
) -> napi::Result<GpuScene> {
    let width_u16 = u16::try_from(width_px)
        .map_err(|_| qt_error("scene width exceeds vello_hybrid range"))?;
    let height_u16 = u16::try_from(height_px)
        .map_err(|_| qt_error("scene height exceeds vello_hybrid range"))?;
    // Reuse retained scene if viewport size matches, else allocate new.
    let mut gpu_scene = match retained {
        Some(mut s) if s.width() == width_u16 && s.height() == height_u16 => {
            s.reset();
            s
        }
        _ => GpuScene::new(width_u16, height_u16),
    };
    let image_manager = anyrender_vello_hybrid::ImageManager::new(
        renderer, device, queue, encoder, image_cache,
    );
    let mut painter = anyrender_vello_hybrid::VelloHybridScenePainter::new(
        &mut gpu_scene, image_manager,
    );
    painter.append_scene(scene.clone(), Affine::scale(scale_factor));
    Ok(gpu_scene)
}

/// Build GPU scene from per-subtree scenes, using Recording cache for clean subtrees.
/// Dirty subtrees get record+prepare+execute; clean ones replay cached recordings.
fn build_gpu_scene_from_subtrees(
    width_px: u32,
    height_px: u32,
    scale_factor: f64,
    subtrees: &[(FragmentId, Scene, bool)],
    renderer: &mut Renderer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    image_cache: &mut ImageCache,
    retained: Option<GpuScene>,
    recordings: &mut HashMap<FragmentId, Recording>,
) -> napi::Result<GpuScene> {
    use vello_common::recording::Recordable;

    let width_u16 = u16::try_from(width_px)
        .map_err(|_| qt_error("scene width exceeds vello_hybrid range"))?;
    let height_u16 = u16::try_from(height_px)
        .map_err(|_| qt_error("scene height exceeds vello_hybrid range"))?;
    let mut gpu_scene = match retained {
        Some(mut s) if s.width() == width_u16 && s.height() == height_u16 => {
            s.reset();
            s
        }
        _ => GpuScene::new(width_u16, height_u16),
    };

    let scale_transform = Affine::scale(scale_factor);

    for (id, scene, is_dirty) in subtrees {
        let has_cached = recordings.contains_key(id);

        if !is_dirty && has_cached {
            // Clean subtree with cached recording — skip strip generation.
            let recording = recordings.get(id).unwrap();
            gpu_scene.execute_recording(recording);
        } else {
            // Dirty or first-time: record, prepare strips, execute, cache.
            let mut recording = recordings.remove(id).unwrap_or_else(Recording::new);
            recording.clear();

            let mut image_manager = anyrender_vello_hybrid::ImageManager::new(
                renderer, device, queue, encoder, image_cache,
            );
            anyrender_vello_hybrid::record_anyrender_scene(
                &mut gpu_scene,
                &mut recording,
                scene,
                scale_transform,
                &mut image_manager,
            );
            gpu_scene.prepare_recording(&mut recording);
            gpu_scene.execute_recording(&recording);
            recordings.insert(*id, recording);
        }
    }

    // Prune stale recordings for subtrees no longer present.
    let active_ids: std::collections::HashSet<FragmentId> =
        subtrees.iter().map(|(id, _, _)| *id).collect();
    recordings.retain(|id, _| active_ids.contains(id));

    Ok(gpu_scene)
}

/// Build 64-byte uniform for composite_layer.wgsl LayerUniforms.
fn make_layer_uniform(
    affine_coeffs: &[f64; 6],
    bounds_x: f64, bounds_y: f64,
    bounds_w: f64, bounds_h: f64,
    viewport_w: f64, viewport_h: f64,
    opacity: f32,
) -> [u8; 64] {
    let mut data = [0u8; 64];
    // transform_ab: vec4<f32>(a, b, c, d)
    data[0..4].copy_from_slice(&(affine_coeffs[0] as f32).to_le_bytes());
    data[4..8].copy_from_slice(&(affine_coeffs[1] as f32).to_le_bytes());
    data[8..12].copy_from_slice(&(affine_coeffs[2] as f32).to_le_bytes());
    data[12..16].copy_from_slice(&(affine_coeffs[3] as f32).to_le_bytes());
    // transform_ef: vec4<f32>(e, f, viewport_w, viewport_h)
    data[16..20].copy_from_slice(&(affine_coeffs[4] as f32).to_le_bytes());
    data[20..24].copy_from_slice(&(affine_coeffs[5] as f32).to_le_bytes());
    data[24..28].copy_from_slice(&(viewport_w as f32).to_le_bytes());
    data[28..32].copy_from_slice(&(viewport_h as f32).to_le_bytes());
    // bounds: vec4<f32>(x, y, w, h)
    data[32..36].copy_from_slice(&(bounds_x as f32).to_le_bytes());
    data[36..40].copy_from_slice(&(bounds_y as f32).to_le_bytes());
    data[40..44].copy_from_slice(&(bounds_w as f32).to_le_bytes());
    data[44..48].copy_from_slice(&(bounds_h as f32).to_le_bytes());
    // opacity_pad: vec4<f32>(opacity, 0, 0, 0)
    data[48..52].copy_from_slice(&opacity.to_le_bytes());
    data
}

/// Clear a sub-region of a texture to transparent black using a retained
/// zero buffer + copy_buffer_to_texture. Avoids per-frame staging buffer
/// allocation that `write_texture` would cause.
fn clear_texture_rect(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    texture: &wgpu::Texture,
    zero_buffer: &mut Option<(wgpu::Buffer, usize)>,
    dx: u32, dy: u32, dw: u32, dh: u32,
) {
    if dw == 0 || dh == 0 { return; }
    let row_bytes = (dw as usize) * 4;
    // wgpu requires bytes_per_row aligned to 256 for copy_buffer_to_texture.
    let aligned_row = ((row_bytes + 255) & !255) as u32;
    let total_bytes = (aligned_row as usize) * (dh as usize);

    // Grow retained zero buffer if needed.
    let needs_new = zero_buffer.as_ref().map_or(true, |&(_, sz)| sz < total_bytes);
    if needs_new {
        let alloc_size = (total_bytes * 2).next_power_of_two().max(4096);
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("qt-solid-zero-buffer"),
            size: alloc_size as u64,
            usage: wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: true,
        });
        buffer.unmap();
        *zero_buffer = Some((buffer, alloc_size));
    }

    let (buf, _) = zero_buffer.as_ref().unwrap();
    encoder.copy_buffer_to_texture(
        wgpu::TexelCopyBufferInfo {
            buffer: buf,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(aligned_row),
                rows_per_image: None,
            },
        },
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d { x: dx, y: dy, z: 0 },
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::Extent3d { width: dw, height: dh, depth_or_array_layers: 1 },
    );
}

