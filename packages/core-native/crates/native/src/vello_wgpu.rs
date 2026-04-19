use std::{
    collections::HashMap,
    num::NonZeroUsize,
    sync::{Mutex, mpsc},
    time::Instant,
};

use once_cell::sync::Lazy;
use qt_solid_widget_core::runtime::{WidgetCapture, WidgetCaptureFormat};
use qt_wgpu_renderer::{
    QtCompositorTarget, with_window_compositor_device_queue, with_window_compositor_layer_texture,
};
use vello::{
    AaConfig, AaSupport, RenderParams, Renderer, RendererOptions, Scene, peniko::Color, wgpu,
};

use crate::runtime::qt_error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct RendererCacheKey {
    surface_kind: u8,
    primary_handle: u64,
    secondary_handle: u64,
}

static VELLO_RENDERERS: Lazy<Mutex<HashMap<RendererCacheKey, Renderer>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

fn renderer_cache_key(target: QtCompositorTarget) -> RendererCacheKey {
    RendererCacheKey {
        surface_kind: target.surface_kind,
        primary_handle: target.primary_handle,
        secondary_handle: target.secondary_handle,
    }
}

fn renderer_options() -> RendererOptions {
    RendererOptions {
        antialiasing_support: AaSupport::area_only(),
        num_init_threads: NonZeroUsize::new(1),
        ..Default::default()
    }
}

fn with_cached_renderer<T>(
    target: QtCompositorTarget,
    device: &wgpu::Device,
    run: impl FnOnce(&mut Renderer) -> qt_wgpu_renderer::Result<T>,
) -> qt_wgpu_renderer::Result<T> {
    let key = renderer_cache_key(target);
    let mut renderers = VELLO_RENDERERS
        .lock()
        .expect("vello renderer cache mutex poisoned");
    let renderer = match renderers.entry(key) {
        std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
        std::collections::hash_map::Entry::Vacant(entry) => entry.insert(
            Renderer::new(device, renderer_options())
                .map_err(|error| qt_wgpu_renderer::QtWgpuRendererError::new(error.to_string()))?,
        ),
    };
    run(renderer)
}

fn scaled_scene_for_render(scene: &Scene, scale_factor: f64) -> Scene {
    let mut scaled_scene = Scene::new();
    scaled_scene.append(scene, Some(vello::kurbo::Affine::scale(scale_factor)));
    scaled_scene
}

fn align_copy_stride(bytes_per_row: usize) -> usize {
    let alignment = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT as usize;
    bytes_per_row.div_ceil(alignment) * alignment
}

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
        .ok_or_else(|| {
            qt_wgpu_renderer::QtWgpuRendererError::new("qt vello readback size overflow")
        })?;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("qt-solid-vello-readback-buffer"),
        size: readback_size as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("qt-solid-vello-readback-encoder"),
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
    let (sender, receiver) = mpsc::sync_channel(1);
    slice.map_async(wgpu::MapMode::Read, move |result| {
        let _ = sender.send(result);
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|error| qt_wgpu_renderer::QtWgpuRendererError::new(error.to_string()))?;
    receiver
        .recv()
        .map_err(|_| {
            qt_wgpu_renderer::QtWgpuRendererError::new("qt vello readback map channel closed")
        })?
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

pub(crate) fn render_vello_scene_to_capture(
    target: QtCompositorTarget,
    node_id: u32,
    width_px: u32,
    height_px: u32,
    scale_factor: f64,
    scene: &Scene,
) -> napi::Result<WidgetCapture> {
    with_window_compositor_device_queue(target, |device, queue| {
        let scaled_scene = scaled_scene_for_render(scene, scale_factor);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("qt-solid-vello-capture-texture"),
            size: wgpu::Extent3d {
                width: width_px.max(1),
                height: height_px.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        with_cached_renderer(target, device, |renderer| {
            renderer
                .render_to_texture(
                    device,
                    queue,
                    &scaled_scene,
                    &texture_view,
                    &RenderParams {
                        base_color: Color::from_rgba8(0, 0, 0, 0),
                        width: width_px,
                        height: height_px,
                        antialiasing_method: AaConfig::Area,
                    },
                )
                .map_err(|error| {
                    qt_wgpu_renderer::QtWgpuRendererError::new(format!(
                        "failed to render vello scene to compositor texture for node {node_id}: {error}",
                    ))
                })
        })?;
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

pub(crate) fn render_vello_scene_into_compositor_layer(
    target: QtCompositorTarget,
    node_id: u32,
    width_px: u32,
    height_px: u32,
    scale_factor: f64,
    scene: &Scene,
) -> napi::Result<()> {
    with_window_compositor_layer_texture(
        target,
        node_id,
        qt_wgpu_renderer::QtCompositorImageFormat::Rgba8UnormPremultiplied,
        width_px,
        height_px,
        |device, queue, texture_view| {
            let scaled_scene = scaled_scene_for_render(scene, scale_factor);
            let started = Instant::now();
            with_cached_renderer(target, device, |renderer| {
                renderer
                    .render_to_texture(
                        device,
                        queue,
                        &scaled_scene,
                        texture_view,
                        &RenderParams {
                            base_color: Color::from_rgba8(0, 0, 0, 0),
                            width: width_px,
                            height: height_px,
                            antialiasing_method: AaConfig::Area,
                        },
                    )
                    .map_err(|error| {
                        qt_wgpu_renderer::QtWgpuRendererError::new(format!(
                            "failed to render vello scene to compositor layer for node {node_id}: {error}",
                        ))
                    })
            })?;
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

#[cfg(test)]
mod tests {
    use super::renderer_options;

    #[test]
    fn renderer_options_only_enable_area_antialiasing() {
        let options = renderer_options();
        assert!(options.antialiasing_support.area);
        assert!(!options.antialiasing_support.msaa8);
        assert!(!options.antialiasing_support.msaa16);
    }
}
