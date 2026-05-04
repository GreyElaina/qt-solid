use std::time::Instant;

use crate::canvas::vello::{Scene, peniko::kurbo::Affine};
use crate::image::{ImageCache, sweep_stale_images};
#[cfg(not(target_os = "macos"))]
use crate::runtime::capture::{WidgetCapture, WidgetCaptureFormat};
use anyrender::PaintScene;
use vello::wgpu;
use vello_hybrid::{RenderSize, Renderer, Scene as GpuScene};

use crate::runtime::qt_error;

fn gpu_scene_from_logical_scene(
    width_px: u32,
    height_px: u32,
    scale_factor: f64,
    scene: &Scene,
    renderer: &mut Renderer,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    image_cache: &mut ImageCache,
) -> napi::Result<GpuScene> {
    let started = Instant::now();
    let width_u16 = u16::try_from(width_px).map_err(|_| {
        qt_error("scene width exceeds vello_hybrid range")
    })?;
    let height_u16 = u16::try_from(height_px).map_err(|_| {
        qt_error("scene height exceeds vello_hybrid range")
    })?;
    let mut gpu_scene = GpuScene::new(width_u16, height_u16);
    let image_manager =
        anyrender_vello_hybrid::ImageManager::new(renderer, device, queue, encoder, image_cache);
    let mut painter =
        anyrender_vello_hybrid::VelloHybridScenePainter::new(&mut gpu_scene, image_manager);
    painter.append_scene(scene.clone(), Affine::scale(scale_factor));
    record_append_scene_metric(started.elapsed());
    Ok(gpu_scene)
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
) -> napi::Result<Vec<u8>> {
    let bytes_per_row = width_px as usize * 4;
    let padded_bytes_per_row = align_copy_stride(bytes_per_row);
    let readback_size = padded_bytes_per_row
        .checked_mul(height_px as usize)
        .ok_or_else(|| qt_error("qt gpu readback size overflow"))?;
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("qt-solid-gpu-readback-buffer"),
        size: readback_size as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("qt-solid-gpu-readback-encoder"),
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
        .map_err(|error| qt_error(error.to_string()))?;
    receiver
        .recv()
        .map_err(|_| {
            qt_error("qt gpu readback map channel closed")
        })?
        .map_err(|error| qt_error(error.to_string()))?;
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
    target: crate::renderer::types::SurfaceTarget,
    node_id: u32,
    width_px: u32,
    height_px: u32,
    scale_factor: f64,
    scene: &Scene,
) -> napi::Result<WidgetCapture> {
    crate::renderer::compositor::surface::with_window_compositor_device_queue(
        node_id, &target, width_px, height_px,
        |device, queue, renderer, image_cache| {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("qt-solid-gpu-capture-texture"),
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
                label: Some("qt-solid-gpu-capture-encoder"),
            });
            // Clear render target to transparent — vello uses LoadOp::Load so
            // stale GPU memory would bleed through without an explicit clear.
            {
                let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("qt-solid-gpu-capture-clear"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                        depth_slice: None,
                    })],
                    ..Default::default()
                });
            }
            let gpu_scene = gpu_scene_from_logical_scene(
                width_px,
                height_px,
                scale_factor,
                scene,
                renderer,
                device,
                queue,
                &mut encoder,
                image_cache,
            )?;
            sweep_stale_images(
                scene,
                renderer,
                device,
                queue,
                &mut encoder,
                image_cache,
            );
            renderer
                .render(
                    &gpu_scene,
                    device,
                    queue,
                    &mut encoder,
                    &RenderSize {
                        width: width_px,
                        height: height_px,
                    },
                    &view,
                )
                .map_err(|error| qt_error(error.to_string()))?;
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
            .map_err(|error| qt_error(error.to_string()))?;
            capture.bytes_mut().copy_from_slice(&bytes);
            Ok(capture)
        },
    )
}
