use std::{collections::HashMap, sync::Mutex, time::Instant};

use once_cell::sync::Lazy;
use qt_solid_widget_core::{
    vello::{PaintScene, Scene, peniko::kurbo::Affine},
};
#[cfg(not(target_os = "macos"))]
use qt_solid_widget_core::runtime::{WidgetCapture, WidgetCaptureFormat};
use vello::wgpu;
use vello_hybrid::{
    RenderSize,
    RenderTargetConfig,
    Renderer,
    Resources,
    Scene as HybridScene,
    api::HybridScenePainter,
};

use crate::runtime::qt_error;

#[derive(Default)]
struct TimingAggregate {
    count: u64,
    total_ms: f64,
}

impl TimingAggregate {
    fn add_sample(&mut self, elapsed: std::time::Duration) {
        self.count += 1;
        self.total_ms += elapsed.as_secs_f64() * 1000.0;
    }

    fn average_ms(&self) -> f64 {
        if self.count == 0 {
            return 0.0;
        }
        self.total_ms / self.count as f64
    }
}

#[derive(Default)]
struct HybridMetrics {
    frames: u64,
    append_scene: TimingAggregate,
    render_layer: TimingAggregate,
}

fn hybrid_metrics_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("QT_SOLID_WGPU_METRICS").is_some())
}

fn hybrid_metrics() -> &'static Mutex<HybridMetrics> {
    static METRICS: std::sync::OnceLock<Mutex<HybridMetrics>> = std::sync::OnceLock::new();
    METRICS.get_or_init(|| Mutex::new(HybridMetrics::default()))
}

fn record_append_scene_metric(elapsed: std::time::Duration) {
    if !hybrid_metrics_enabled() {
        return;
    }
    let mut metrics = hybrid_metrics()
        .lock()
        .expect("qt hybrid metrics mutex poisoned");
    metrics.append_scene.add_sample(elapsed);
}

fn record_render_layer_metric(elapsed: std::time::Duration) {
    if !hybrid_metrics_enabled() {
        return;
    }
    let mut metrics = hybrid_metrics()
        .lock()
        .expect("qt hybrid metrics mutex poisoned");
    metrics.frames += 1;
    metrics.render_layer.add_sample(elapsed);
    if metrics.frames % 120 == 0 {
        eprintln!(
            "qt-solid hybrid metrics frames={} append_scene_ms={:.3} render_layer_ms={:.3}",
            metrics.frames,
            metrics.append_scene.average_ms(),
            metrics.render_layer.average_ms(),
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct RendererCacheKey {
    surface_kind: u8,
    primary_handle: u64,
    secondary_handle: u64,
    width_px: u32,
    height_px: u32,
}

struct HybridRendererCacheEntry {
    renderer: Renderer,
    resources: Resources,
}

static HYBRID_RENDERERS: Lazy<Mutex<HashMap<RendererCacheKey, HybridRendererCacheEntry>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

fn renderer_cache_key(target: qt_wgpu_renderer::QtCompositorTarget, width_px: u32, height_px: u32) -> RendererCacheKey {
    RendererCacheKey {
        surface_kind: target.surface_kind,
        primary_handle: target.primary_handle,
        secondary_handle: target.secondary_handle,
        width_px,
        height_px,
    }
}

fn hybrid_scene_from_logical_scene(
    width_px: u32,
    height_px: u32,
    scale_factor: f64,
    scene: &Scene,
) -> qt_wgpu_renderer::Result<HybridScene> {
    let started = Instant::now();
    let width_u16 = u16::try_from(width_px)
        .map_err(|_| qt_wgpu_renderer::QtWgpuRendererError::new("scene width exceeds vello_hybrid range"))?;
    let height_u16 = u16::try_from(height_px)
        .map_err(|_| qt_wgpu_renderer::QtWgpuRendererError::new("scene height exceeds vello_hybrid range"))?;
    let mut painter = HybridScenePainter {
        scene: HybridScene::new(width_u16, height_u16),
    };
    painter
        .append(Affine::scale(scale_factor), scene)
        .map_err(|_| qt_wgpu_renderer::QtWgpuRendererError::new("failed to append scene into vello_hybrid painter"))?;
    record_append_scene_metric(started.elapsed());
    Ok(painter.scene)
}

fn with_cached_renderer<T>(
    target: qt_wgpu_renderer::QtCompositorTarget,
    width_px: u32,
    height_px: u32,
    device: &wgpu::Device,
    run: impl FnOnce(&mut HybridRendererCacheEntry) -> qt_wgpu_renderer::Result<T>,
) -> qt_wgpu_renderer::Result<T> {
    let key = renderer_cache_key(target, width_px, height_px);
    let mut renderers = HYBRID_RENDERERS
        .lock()
        .expect("vello_hybrid renderer cache mutex poisoned");
    let entry = renderers.entry(key).or_insert_with(|| HybridRendererCacheEntry {
        renderer: Renderer::new(
            device,
            &RenderTargetConfig {
                format: wgpu::TextureFormat::Rgba8Unorm,
                width: width_px,
                height: height_px,
            },
        ),
        resources: Resources::default(),
    });
    run(entry)
}

#[cfg(not(target_os = "macos"))]
fn align_copy_stride(bytes_per_row: usize) -> usize {
    let alignment = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;
    bytes_per_row.div_ceil(alignment) * alignment
}

#[cfg(not(target_os = "macos"))]
fn read_rgba_texture(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width_px: u32,
    height_px: u32,
) -> qt_wgpu_renderer::Result<Vec<u8>> {
    let bytes_per_row = width_px as usize * 4;
    let padded_bytes_per_row = align_copy_stride(bytes_per_row);
    let readback_size = padded_bytes_per_row
        .checked_mul(height_px as usize)
        .ok_or_else(|| qt_wgpu_renderer::QtWgpuRendererError::new("qt hybrid readback size overflow"))?;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("qt-solid-hybrid-readback-buffer"),
        size: readback_size as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("qt-solid-hybrid-readback-encoder"),
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row as u32),
                rows_per_image: Some(height_px),
            },
        },
        wgpu::Extent3d {
            width: width_px,
            height: height_px,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);
    let slice = readback.slice(..);
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|error| qt_wgpu_renderer::QtWgpuRendererError::new(error.to_string()))?;
    receiver
        .recv()
        .map_err(|_| qt_wgpu_renderer::QtWgpuRendererError::new("qt hybrid readback map channel closed"))?
        .map_err(|error| qt_wgpu_renderer::QtWgpuRendererError::new(error.to_string()))?;
    let mapped = slice.get_mapped_range();
    let mut bytes = vec![0; bytes_per_row * height_px as usize];
    for row in 0..height_px as usize {
        let source_offset = row * padded_bytes_per_row;
        let target_offset = row * bytes_per_row;
        bytes[target_offset..target_offset + bytes_per_row]
            .copy_from_slice(&mapped[source_offset..source_offset + bytes_per_row]);
    }
    drop(mapped);
    readback.unmap();
    Ok(bytes)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn render_scene_to_capture(
    target: qt_wgpu_renderer::QtCompositorTarget,
    _node_id: u32,
    width_px: u32,
    height_px: u32,
    scale_factor: f64,
    scene: &Scene,
) -> napi::Result<WidgetCapture> {
    qt_wgpu_renderer::with_window_compositor_device_queue(target, |device, queue| {
        let hybrid_scene = hybrid_scene_from_logical_scene(width_px, height_px, scale_factor, scene)?;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("qt-solid-hybrid-capture-texture"),
            size: wgpu::Extent3d {
                width: width_px.max(1),
                height: height_px.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("qt-solid-hybrid-capture-encoder"),
        });
        with_cached_renderer(target, width_px, height_px, device, |entry| {
            entry
                .renderer
                .render(
                    &hybrid_scene,
                    &mut entry.resources,
                    device,
                    queue,
                    &mut encoder,
                    &RenderSize {
                        width: width_px,
                        height: height_px,
                    },
                    &view,
                )
                .map_err(|error| qt_wgpu_renderer::QtWgpuRendererError::new(error.to_string()))
        })?;
        queue.submit([encoder.finish()]);
        let bytes = read_rgba_texture(device, queue, &texture, width_px, height_px)?;
        let stride = width_px as usize * 4;
        let mut capture = WidgetCapture::new_zeroed(
            WidgetCaptureFormat::Rgba8Premultiplied,
            width_px,
            height_px,
            stride,
            scale_factor,
        )
        .map_err(|error| qt_wgpu_renderer::QtWgpuRendererError::new(error.to_string()))?;
        capture.bytes_mut().copy_from_slice(&bytes);
        Ok(capture)
    })
    .map_err(|error| qt_error(error.to_string()))
}

pub(crate) fn render_scene_into_compositor_layer(
    target: qt_wgpu_renderer::QtCompositorTarget,
    _node_id: u32,
    width_px: u32,
    height_px: u32,
    scale_factor: f64,
    scene: &Scene,
) -> napi::Result<()> {
    qt_wgpu_renderer::with_window_compositor_layer_texture(
        target,
        _node_id,
        qt_wgpu_renderer::QtCompositorImageFormat::Rgba8UnormPremultiplied,
        width_px,
        height_px,
        |device, queue, texture_view| {
            let hybrid_scene = hybrid_scene_from_logical_scene(width_px, height_px, scale_factor, scene)?;
            let started = Instant::now();
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("qt-solid-hybrid-layer-encoder"),
            });
            with_cached_renderer(target, width_px, height_px, device, |entry| {
                entry
                    .renderer
                    .render(
                        &hybrid_scene,
                        &mut entry.resources,
                        device,
                        queue,
                        &mut encoder,
                        &RenderSize {
                            width: width_px,
                            height: height_px,
                        },
                        texture_view,
                    )
                    .map_err(|error| qt_wgpu_renderer::QtWgpuRendererError::new(error.to_string()))
            })?;
            queue.submit([encoder.finish()]);
            record_render_layer_metric(started.elapsed());
            qt_wgpu_renderer::record_compositor_timing(
                target,
                qt_wgpu_renderer::CompositorTimingStage::RenderOverlayLayer,
                started.elapsed(),
            );
            Ok(())
        },
    )
    .map_err(|error| qt_error(error.to_string()))
}
