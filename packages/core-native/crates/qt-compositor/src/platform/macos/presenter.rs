use std::{
    collections::{HashMap, HashSet},
    ffi::c_void,
    ptr::NonNull,
};

use foreign_types::ForeignType;
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_core_foundation::CGSize;
use objc2_foundation::NSString;
use objc2_metal::{
    MTLBlendFactor, MTLBlendOperation, MTLClearColor, MTLCommandBuffer, MTLCommandEncoder,
    MTLCommandQueue, MTLCompileOptions, MTLDevice, MTLDrawable, MTLFunction, MTLLibrary,
    MTLLoadAction, MTLOrigin, MTLPixelFormat, MTLPrimitiveType, MTLRegion, MTLRenderCommandEncoder,
    MTLRenderPassDescriptor, MTLRenderPipelineDescriptor, MTLRenderPipelineState,
    MTLSamplerAddressMode, MTLSamplerDescriptor, MTLSamplerMinMagFilter, MTLSamplerMipFilter,
    MTLSamplerState, MTLScissorRect, MTLSize, MTLStorageMode, MTLStoreAction, MTLTexture,
    MTLTextureDescriptor, MTLTextureType, MTLTextureUsage,
};
use objc2_quartz_core::{CAMetalDrawable, CAMetalLayer as ObjcCAMetalLayer};
use crate::types::{QtCompositorAffine, QtCompositorError, QtCompositorTarget, Result};
use raw_window_metal::Layer;

use super::{
    state::{MacosCompositorState, OwnedCompositorSnapshot, PendingMetalDisplayLinkDrawable},
    trace::trace,
};
use crate::surface::with_window_compositor_layer_texture_handle;
use crate::types::{QtCompositorImageFormat, QtCompositorLayerSourceKind, QtCompositorUploadKind};

const RAW_METAL_SHADER_SOURCE: &str = r#"
#include <metal_stdlib>
using namespace metal;

struct QuadVertex {
    float px;
    float py;
    float u;
    float v;
    float opacity;
    float padding;
};

struct VertexOut {
    float4 position [[position]];
    float2 uv;
    float opacity;
};

vertex VertexOut quad_vertex(uint vid [[vertex_id]],
                             constant QuadVertex *vertices [[buffer(0)]]) {
    VertexOut out;
    out.position = float4(float2(vertices[vid].px, vertices[vid].py), 0.0, 1.0);
    out.uv = float2(vertices[vid].u, vertices[vid].v);
    out.opacity = vertices[vid].opacity;
    return out;
}

fragment float4 quad_fragment(VertexOut in [[stage_in]],
                              texture2d<float> color_texture [[texture(0)]],
                              sampler color_sampler [[sampler(0)]]) {
    return color_texture.sample(color_sampler, in.uv) * in.opacity;
}

float srgb_channel_to_linear(float value) {
    if (value <= 0.04045) {
        return value / 12.92;
    }
    return pow((value + 0.055) / 1.055, 2.4);
}

fragment float4 quad_fragment_cached_texture(VertexOut in [[stage_in]],
                                             texture2d<float> color_texture [[texture(0)]],
                                             sampler color_sampler [[sampler(0)]]) {
    float4 sample = color_texture.sample(color_sampler, in.uv);
    return float4(
        srgb_channel_to_linear(sample.r),
        srgb_channel_to_linear(sample.g),
        srgb_channel_to_linear(sample.b),
        sample.a
    ) * in.opacity;
}
"#;

#[repr(C)]
#[derive(Clone, Copy)]
struct QuadVertex {
    px: f32,
    py: f32,
    u: f32,
    v: f32,
    opacity: f32,
    padding: f32,
}

pub(crate) struct MetalPresenter {
    pub(crate) raw_device: Retained<ProtocolObject<dyn MTLDevice>>,
    pub(crate) layer: Layer,
    pub(crate) present_queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    pub(crate) raw_pipeline: RawPipelineState,
    pub(crate) raw_textures: RawTextureState,
}

pub(crate) trait Presenter: Send {
    fn configure_for_target(
        &mut self,
        target: QtCompositorTarget,
        present_format: wgpu::TextureFormat,
    ) -> Result<()>;

    fn layer_handle(&self) -> u64;

    fn render_snapshot(
        &mut self,
        state: &mut MacosCompositorState,
        pending_drawable: PendingMetalDisplayLinkDrawable,
        snapshot: &OwnedCompositorSnapshot,
    ) -> Result<()>;
}

pub(crate) struct RawPipelineState {
    pub(crate) pipeline: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    pub(crate) cached_texture_pipeline: Retained<ProtocolObject<dyn MTLRenderPipelineState>>,
    pub(crate) sampler: Retained<ProtocolObject<dyn MTLSamplerState>>,
}

#[derive(Clone)]
pub(crate) struct RawImageTexture {
    pub(crate) format: QtCompositorImageFormat,
    pub(crate) width_px: u32,
    pub(crate) height_px: u32,
    pub(crate) texture_handle: usize,
}

#[derive(Default)]
pub(crate) struct RawTextureState {
    pub(crate) base: Option<RawImageTexture>,
    pub(crate) layers: HashMap<u32, RawImageTexture>,
}

impl Presenter for MetalPresenter {
    fn configure_for_target(
        &mut self,
        target: QtCompositorTarget,
        present_format: wgpu::TextureFormat,
    ) -> Result<()> {
        configure_metal_layer(
            target,
            &self.layer,
            self.raw_device.as_ref(),
            present_format,
        )
    }

    fn layer_handle(&self) -> u64 {
        self.layer.as_ptr().as_ptr() as u64
    }

    fn render_snapshot(
        &mut self,
        state: &mut MacosCompositorState,
        pending_drawable: PendingMetalDisplayLinkDrawable,
        snapshot: &OwnedCompositorSnapshot,
    ) -> Result<()> {
        render_snapshot_to_drawable(self, state, pending_drawable, snapshot)
    }
}

pub(crate) fn create_raw_pipeline_state(
    device: &ProtocolObject<dyn MTLDevice>,
) -> Result<RawPipelineState> {
    let options = MTLCompileOptions::new();
    let shader_source = NSString::from_str(RAW_METAL_SHADER_SOURCE);
    let library = device
        .newLibraryWithSource_options_error(&shader_source, Some(&options))
        .map_err(|error| {
            QtCompositorError::new(format!("qt macos raw metal shader compile failed: {error}"))
        })?;
    let vertex_fn = library
        .newFunctionWithName(&NSString::from_str("quad_vertex"))
        .ok_or_else(|| QtCompositorError::new("qt macos raw metal missing vertex fn"))?;
    let fragment_fn = library
        .newFunctionWithName(&NSString::from_str("quad_fragment"))
        .ok_or_else(|| QtCompositorError::new("qt macos raw metal missing fragment fn"))?;
    let cached_fragment_fn = library
        .newFunctionWithName(&NSString::from_str("quad_fragment_cached_texture"))
        .ok_or_else(|| QtCompositorError::new("qt macos raw metal missing cached fragment fn"))?;

    let create_pipeline = |label: &str, fragment: &ProtocolObject<dyn MTLFunction>| {
        let descriptor = MTLRenderPipelineDescriptor::new();
        descriptor.setLabel(Some(&NSString::from_str(label)));
        descriptor.setVertexFunction(Some(vertex_fn.as_ref()));
        descriptor.setFragmentFunction(Some(fragment));
        let color_attachments = descriptor.colorAttachments();
        let color_attachment = unsafe { color_attachments.objectAtIndexedSubscript(0) };
        color_attachment.setPixelFormat(MTLPixelFormat::BGRA8Unorm_sRGB);
        color_attachment.setBlendingEnabled(true);
        color_attachment.setRgbBlendOperation(MTLBlendOperation::Add);
        color_attachment.setAlphaBlendOperation(MTLBlendOperation::Add);
        color_attachment.setSourceRGBBlendFactor(MTLBlendFactor::One);
        color_attachment.setSourceAlphaBlendFactor(MTLBlendFactor::One);
        color_attachment.setDestinationRGBBlendFactor(MTLBlendFactor::OneMinusSourceAlpha);
        color_attachment.setDestinationAlphaBlendFactor(MTLBlendFactor::OneMinusSourceAlpha);
        device
            .newRenderPipelineStateWithDescriptor_error(&descriptor)
            .map_err(|error| {
                QtCompositorError::new(format!("qt macos raw metal pipeline failed: {error}"))
            })
    };
    let pipeline = create_pipeline("qt-solid-raw-metal-compositor", fragment_fn.as_ref())?;
    let cached_texture_pipeline = create_pipeline(
        "qt-solid-raw-metal-compositor-cached-texture",
        cached_fragment_fn.as_ref(),
    )?;

    let sampler_descriptor = MTLSamplerDescriptor::new();
    sampler_descriptor.setMinFilter(MTLSamplerMinMagFilter::Linear);
    sampler_descriptor.setMagFilter(MTLSamplerMinMagFilter::Linear);
    sampler_descriptor.setMipFilter(MTLSamplerMipFilter::NotMipmapped);
    sampler_descriptor.setSAddressMode(MTLSamplerAddressMode::ClampToEdge);
    sampler_descriptor.setTAddressMode(MTLSamplerAddressMode::ClampToEdge);
    sampler_descriptor.setRAddressMode(MTLSamplerAddressMode::ClampToEdge);
    let sampler = device
        .newSamplerStateWithDescriptor(&sampler_descriptor)
        .ok_or_else(|| QtCompositorError::new("qt macos raw metal sampler creation failed"))?;

    Ok(RawPipelineState {
        pipeline,
        cached_texture_pipeline,
        sampler,
    })
}

pub(crate) fn drop_retained_metal_drawable(drawable_handle: u64) {
    if drawable_handle == 0 {
        return;
    }
    let _ =
        unsafe { Retained::from_raw(drawable_handle as *mut ProtocolObject<dyn CAMetalDrawable>) };
}

pub(crate) fn borrowed_metal_drawable_ref(
    drawable_handle: u64,
) -> Option<&'static ProtocolObject<dyn CAMetalDrawable>> {
    borrowed_protocol_object(drawable_handle as *mut ProtocolObject<dyn CAMetalDrawable>)
}

pub(crate) fn retain_protocol_object<P: ?Sized>(
    ptr: *mut ProtocolObject<P>,
    label: &str,
) -> Result<Retained<ProtocolObject<P>>> {
    unsafe { Retained::retain(ptr) }
        .ok_or_else(|| QtCompositorError::new(format!("{label} is null")))
}

fn metal_texture_format(format: QtCompositorImageFormat) -> MTLPixelFormat {
    match format {
        QtCompositorImageFormat::Bgra8UnormPremultiplied => MTLPixelFormat::BGRA8Unorm_sRGB,
        QtCompositorImageFormat::Rgba8UnormPremultiplied => MTLPixelFormat::RGBA8Unorm_sRGB,
    }
}

fn ensure_raw_texture<'a>(
    device: &ProtocolObject<dyn MTLDevice>,
    slot: &'a mut Option<RawImageTexture>,
    format: QtCompositorImageFormat,
    width_px: u32,
    height_px: u32,
) -> Result<&'a ProtocolObject<dyn MTLTexture>> {
    let recreate = slot
        .as_ref()
        .map(|texture| {
            texture.format != format
                || texture.width_px != width_px
                || texture.height_px != height_px
        })
        .unwrap_or(true);
    if recreate {
        if let Some(previous) = slot.take() {
            drop_retained_mtl_texture(previous.texture_handle);
        }
        let descriptor = unsafe {
            MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(
                metal_texture_format(format),
                width_px.max(1) as usize,
                height_px.max(1) as usize,
                false,
            )
        };
        descriptor.setTextureType(MTLTextureType::Type2D);
        descriptor.setStorageMode(MTLStorageMode::Managed);
        descriptor.setUsage(MTLTextureUsage::ShaderRead);
        *slot = Some(RawImageTexture {
            format,
            width_px,
            height_px,
            texture_handle: Retained::into_raw(
                device
                    .newTextureWithDescriptor(&descriptor)
                    .ok_or_else(|| {
                        QtCompositorError::new("qt macos raw base texture creation failed")
                    })?,
            ) as usize,
        });
    }
    let texture_handle = slot.as_ref().expect("raw texture inserted").texture_handle;
    borrowed_mtl_texture(texture_handle)
        .ok_or_else(|| QtCompositorError::new("qt macos raw base texture handle is null"))
}

fn ensure_raw_layer_texture<'a>(
    device: &ProtocolObject<dyn MTLDevice>,
    textures: &'a mut HashMap<u32, RawImageTexture>,
    node_id: u32,
    format: QtCompositorImageFormat,
    width_px: u32,
    height_px: u32,
) -> Result<&'a ProtocolObject<dyn MTLTexture>> {
    let recreate = textures
        .get(&node_id)
        .map(|texture| {
            texture.format != format
                || texture.width_px != width_px
                || texture.height_px != height_px
        })
        .unwrap_or(true);
    if recreate {
        if let Some(previous) = textures.remove(&node_id) {
            drop_retained_mtl_texture(previous.texture_handle);
        }
        let descriptor = unsafe {
            MTLTextureDescriptor::texture2DDescriptorWithPixelFormat_width_height_mipmapped(
                metal_texture_format(format),
                width_px.max(1) as usize,
                height_px.max(1) as usize,
                false,
            )
        };
        descriptor.setTextureType(MTLTextureType::Type2D);
        descriptor.setStorageMode(MTLStorageMode::Managed);
        descriptor.setUsage(MTLTextureUsage::ShaderRead);
        textures.insert(
            node_id,
            RawImageTexture {
                format,
                width_px,
                height_px,
                texture_handle: Retained::into_raw(
                    device
                        .newTextureWithDescriptor(&descriptor)
                        .ok_or_else(|| {
                            QtCompositorError::new(format!(
                                "qt macos raw layer texture creation failed for node {}",
                                node_id
                            ))
                        })?,
                ) as usize,
            },
        );
    }
    let texture_handle = textures
        .get(&node_id)
        .expect("raw layer texture inserted")
        .texture_handle;
    borrowed_mtl_texture(texture_handle).ok_or_else(|| {
        QtCompositorError::new(format!(
            "qt macos raw layer texture handle is null for node {}",
            node_id
        ))
    })
}

fn upload_raw_texture(
    texture: &ProtocolObject<dyn MTLTexture>,
    width_px: u32,
    height_px: u32,
    stride: usize,
    bytes: &[u8],
) {
    let byte_ptr = NonNull::new(bytes.as_ptr() as *mut c_void)
        .expect("upload bytes pointer should not be null");
    unsafe {
        texture.replaceRegion_mipmapLevel_withBytes_bytesPerRow(
            MTLRegion {
                origin: MTLOrigin { x: 0, y: 0, z: 0 },
                size: MTLSize {
                    width: width_px.max(1) as usize,
                    height: height_px.max(1) as usize,
                    depth: 1,
                },
            },
            0,
            byte_ptr,
            stride,
        );
    }
}

fn quad_vertices_for_rect(
    target: QtCompositorTarget,
    transform: QtCompositorAffine,
    opacity: f32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    u0: f32,
    v0: f32,
    u1: f32,
    v1: f32,
) -> [QuadVertex; 6] {
    let map_position = |local_x: f64, local_y: f64| {
        let (dx, dy) = transform.map_point(local_x, local_y);
        let x = x as f64 + dx;
        let y = y as f64 + dy;
        [
            (x as f32 / target.width_px.max(1) as f32) * 2.0 - 1.0,
            1.0 - (y as f32 / target.height_px.max(1) as f32) * 2.0,
        ]
    };
    let top_left = map_position(0.0, 0.0);
    let top_right = map_position(width as f64, 0.0);
    let bottom_left = map_position(0.0, height as f64);
    let bottom_right = map_position(width as f64, height as f64);
    [
        QuadVertex {
            px: top_left[0],
            py: top_left[1],
            u: u0,
            v: v0,
            opacity,
            padding: 0.0,
        },
        QuadVertex {
            px: top_right[0],
            py: top_right[1],
            u: u1,
            v: v0,
            opacity,
            padding: 0.0,
        },
        QuadVertex {
            px: bottom_left[0],
            py: bottom_left[1],
            u: u0,
            v: v1,
            opacity,
            padding: 0.0,
        },
        QuadVertex {
            px: bottom_left[0],
            py: bottom_left[1],
            u: u0,
            v: v1,
            opacity,
            padding: 0.0,
        },
        QuadVertex {
            px: top_right[0],
            py: top_right[1],
            u: u1,
            v: v0,
            opacity,
            padding: 0.0,
        },
        QuadVertex {
            px: bottom_right[0],
            py: bottom_right[1],
            u: u1,
            v: v1,
            opacity,
            padding: 0.0,
        },
    ]
}

fn render_snapshot_to_drawable(
    presenter: &mut MetalPresenter,
    state: &mut MacosCompositorState,
    pending_drawable: PendingMetalDisplayLinkDrawable,
    snapshot: &OwnedCompositorSnapshot,
) -> Result<()> {
    let drawable = unsafe {
        Retained::from_raw(
            pending_drawable.drawable_handle as *mut ProtocolObject<dyn CAMetalDrawable>,
        )
    }
    .ok_or_else(|| QtCompositorError::new("qt macos drawable handle is null"))?;
    let drawable_texture = drawable.texture();
    let base_texture = {
        let texture = ensure_raw_texture(
            presenter.raw_device.as_ref(),
            &mut presenter.raw_textures.base,
            snapshot.base.format,
            snapshot.base.width_px,
            snapshot.base.height_px,
        )?;
        if !matches!(snapshot.base.upload_kind, QtCompositorUploadKind::None) {
            upload_raw_texture(
                texture,
                snapshot.base.width_px,
                snapshot.base.height_px,
                snapshot.base.stride,
                &snapshot.base.bytes,
            );
            state.base_initialized = true;
            trace(format_args!(
                "base-initialized from snapshot window={} bytes={}",
                snapshot.window_id,
                snapshot.base.bytes.len()
            ));
        }
        retain_protocol_object(
            texture as *const ProtocolObject<dyn MTLTexture> as *mut ProtocolObject<dyn MTLTexture>,
            "qt macos raw base texture",
        )?
    };

    let mut active_cpu_layers = HashSet::new();
    for layer in &snapshot.layers {
        if matches!(layer.source_kind, QtCompositorLayerSourceKind::CpuBytes) {
            active_cpu_layers.insert(layer.node_id);
            let texture = ensure_raw_layer_texture(
                presenter.raw_device.as_ref(),
                &mut presenter.raw_textures.layers,
                layer.node_id,
                layer.format,
                layer.width_px,
                layer.height_px,
            )?;
            if !matches!(layer.upload_kind, QtCompositorUploadKind::None) {
                upload_raw_texture(
                    texture,
                    layer.width_px,
                    layer.height_px,
                    layer.stride,
                    &layer.bytes,
                );
            }
        }
    }
    presenter.raw_textures.layers.retain(|node_id, texture| {
        if active_cpu_layers.contains(node_id) {
            true
        } else {
            drop_retained_mtl_texture(texture.texture_handle);
            false
        }
    });

    let command_buffer = presenter.present_queue.commandBuffer().ok_or_else(|| {
        QtCompositorError::new("qt macos raw metal could not allocate command buffer")
    })?;
    let render_pass_descriptor = MTLRenderPassDescriptor::renderPassDescriptor();
    let color_attachments = render_pass_descriptor.colorAttachments();
    let color_attachment = unsafe { color_attachments.objectAtIndexedSubscript(0) };
    color_attachment.setTexture(Some(drawable_texture.as_ref()));
    color_attachment.setLoadAction(MTLLoadAction::Clear);
    color_attachment.setStoreAction(MTLStoreAction::Store);
    color_attachment.setClearColor(MTLClearColor {
        red: 0.0,
        green: 0.0,
        blue: 0.0,
        alpha: 0.0,
    });
    let encoder = command_buffer
        .renderCommandEncoderWithDescriptor(&render_pass_descriptor)
        .ok_or_else(|| {
            QtCompositorError::new("qt macos raw metal could not allocate render encoder")
        })?;
    encoder.setRenderPipelineState(presenter.raw_pipeline.pipeline.as_ref());
    unsafe {
        encoder.setFragmentSamplerState_atIndex(Some(presenter.raw_pipeline.sampler.as_ref()), 0);
        encoder.setScissorRect(MTLScissorRect {
            x: 0,
            y: 0,
            width: snapshot.target.width_px.max(1) as usize,
            height: snapshot.target.height_px.max(1) as usize,
        });
    }

    let base_vertices = quad_vertices_for_rect(
        snapshot.target,
        QtCompositorAffine::IDENTITY,
        1.0,
        0,
        0,
        snapshot.target.width_px as i32,
        snapshot.target.height_px as i32,
        0.0,
        0.0,
        1.0,
        1.0,
    );
    unsafe {
        encoder.setVertexBytes_length_atIndex(
            NonNull::new(base_vertices.as_ptr() as *mut c_void)
                .expect("base quad vertices pointer should not be null"),
            std::mem::size_of_val(&base_vertices),
            0,
        );
        encoder.setFragmentTexture_atIndex(Some(base_texture.as_ref()), 0);
        encoder.drawPrimitives_vertexStart_vertexCount(MTLPrimitiveType::Triangle, 0, 6);
    }

    for layer in &snapshot.layers {
        match layer.source_kind {
            QtCompositorLayerSourceKind::CpuBytes => {
                let Some(texture) = presenter
                    .raw_textures
                    .layers
                    .get(&layer.node_id)
                    .and_then(|entry| borrowed_mtl_texture(entry.texture_handle))
                else {
                    continue;
                };
                for rect in &layer.visible_rects {
                    if rect.width <= 0 || rect.height <= 0 {
                        continue;
                    }
                    if let Some(clip_rect) = layer.clip_rect {
                        encoder.setScissorRect(MTLScissorRect {
                            x: clip_rect.x.max(0) as usize,
                            y: clip_rect.y.max(0) as usize,
                            width: clip_rect.width.max(0) as usize,
                            height: clip_rect.height.max(0) as usize,
                        });
                    } else {
                        encoder.setScissorRect(MTLScissorRect {
                            x: 0,
                            y: 0,
                            width: snapshot.target.width_px.max(1) as usize,
                            height: snapshot.target.height_px.max(1) as usize,
                        });
                    }
                    let texture_width = layer.width_px.max(1) as f32;
                    let texture_height = layer.height_px.max(1) as f32;
                    let u0 = rect.x as f32 / texture_width;
                    let u1 = (rect.x + rect.width) as f32 / texture_width;
                    let v0 = rect.y as f32 / texture_height;
                    let v1 = (rect.y + rect.height) as f32 / texture_height;
                    let quad_vertices = quad_vertices_for_rect(
                        snapshot.target,
                        layer.transform,
                        layer.opacity,
                        layer.x + rect.x,
                        layer.y + rect.y,
                        rect.width,
                        rect.height,
                        u0,
                        v0,
                        u1,
                        v1,
                    );
                    unsafe {
                        encoder.setVertexBytes_length_atIndex(
                            NonNull::new(quad_vertices.as_ptr() as *mut c_void)
                                .expect("layer quad vertices pointer should not be null"),
                            std::mem::size_of_val(&quad_vertices),
                            0,
                        );
                        encoder.setFragmentTexture_atIndex(Some(texture), 0);
                        encoder.drawPrimitives_vertexStart_vertexCount(
                            MTLPrimitiveType::Triangle,
                            0,
                            6,
                        );
                    }
                }
            }
            QtCompositorLayerSourceKind::CachedTexture => {
                encoder.setRenderPipelineState(
                    presenter.raw_pipeline.cached_texture_pipeline.as_ref(),
                );
                with_window_compositor_layer_texture_handle(
                    snapshot.target,
                    layer.node_id,
                    layer.format,
                    layer.width_px,
                    layer.height_px,
                    |_, _, texture, _| {
                        let hal_texture = unsafe { texture.as_hal::<wgpu_hal::metal::Api>() }
                            .ok_or_else(|| {
                                QtCompositorError::new(format!(
                                    "qt macos compositor cached layer {} is not backed by Metal",
                                    layer.node_id
                                ))
                            })?;
                        let raw_texture_ptr = unsafe {
                            hal_texture.raw_handle().as_ptr() as *mut ProtocolObject<dyn MTLTexture>
                        };
                        let raw_texture = retain_protocol_object(
                            raw_texture_ptr,
                            &format!("qt macos cached layer {}", layer.node_id),
                        )?;
                        for rect in &layer.visible_rects {
                            if rect.width <= 0 || rect.height <= 0 {
                                continue;
                            }
                            if let Some(clip_rect) = layer.clip_rect {
                                encoder.setScissorRect(MTLScissorRect {
                                    x: clip_rect.x.max(0) as usize,
                                    y: clip_rect.y.max(0) as usize,
                                    width: clip_rect.width.max(0) as usize,
                                    height: clip_rect.height.max(0) as usize,
                                });
                            } else {
                                encoder.setScissorRect(MTLScissorRect {
                                    x: 0,
                                    y: 0,
                                    width: snapshot.target.width_px.max(1) as usize,
                                    height: snapshot.target.height_px.max(1) as usize,
                                });
                            }
                            let texture_width = layer.width_px.max(1) as f32;
                            let texture_height = layer.height_px.max(1) as f32;
                            let u0 = rect.x as f32 / texture_width;
                            let u1 = (rect.x + rect.width) as f32 / texture_width;
                            let v0 = rect.y as f32 / texture_height;
                            let v1 = (rect.y + rect.height) as f32 / texture_height;
                            let quad_vertices = quad_vertices_for_rect(
                                snapshot.target,
                                layer.transform,
                                layer.opacity,
                                layer.x + rect.x,
                                layer.y + rect.y,
                                rect.width,
                                rect.height,
                                u0,
                                v0,
                                u1,
                                v1,
                            );
                            unsafe {
                                encoder.setVertexBytes_length_atIndex(
                                    NonNull::new(quad_vertices.as_ptr() as *mut c_void).expect(
                                        "cached layer quad vertices pointer should not be null",
                                    ),
                                    std::mem::size_of_val(&quad_vertices),
                                    0,
                                );
                                encoder.setFragmentTexture_atIndex(Some(raw_texture.as_ref()), 0);
                                encoder.drawPrimitives_vertexStart_vertexCount(
                                    MTLPrimitiveType::Triangle,
                                    0,
                                    6,
                                );
                            }
                        }
                        Ok(())
                    },
                )?;
                encoder.setRenderPipelineState(presenter.raw_pipeline.pipeline.as_ref());
            }
        }
    }
    encoder.endEncoding();
    let drawable_protocol = drawable_mtl_drawable_ref(drawable.as_ref());
    command_buffer.presentDrawable(drawable_protocol);
    command_buffer.commit();
    Ok(())
}

fn configure_metal_layer(
    target: QtCompositorTarget,
    layer: &Layer,
    device: &ProtocolObject<dyn MTLDevice>,
    present_format: wgpu::TextureFormat,
) -> Result<()> {
    let layer_ref = metal_layer_ref(layer);
    layer_ref.setDevice(Some(device));
    layer_ref.setPixelFormat(metal_pixel_format(present_format)?);
    layer_ref.setDrawableSize(CGSize {
        width: target.width_px.max(1) as f64,
        height: target.height_px.max(1) as f64,
    });
    layer_ref.setFramebufferOnly(false);
    layer_ref.setPresentsWithTransaction(false);
    layer_ref.setDisplaySyncEnabled(!should_prefer_immediate_present_mode());
    Ok(())
}

fn should_prefer_immediate_present_mode() -> bool {
    std::env::var_os("QT_SOLID_WGPU_PRESENT_IMMEDIATE").is_some_and(|value| value == "1")
}

fn metal_pixel_format(format: wgpu::TextureFormat) -> Result<MTLPixelFormat> {
    let pixel_format = match format {
        wgpu::TextureFormat::Bgra8Unorm => MTLPixelFormat::BGRA8Unorm,
        wgpu::TextureFormat::Bgra8UnormSrgb => MTLPixelFormat::BGRA8Unorm_sRGB,
        wgpu::TextureFormat::Rgba8Unorm => MTLPixelFormat::RGBA8Unorm,
        wgpu::TextureFormat::Rgba8UnormSrgb => MTLPixelFormat::RGBA8Unorm_sRGB,
        other => {
            return Err(QtCompositorError::new(format!(
                "qt macos compositor does not support present format {other:?}",
            )));
        }
    };
    Ok(pixel_format)
}

fn metal_layer_ref(layer: &Layer) -> &ObjcCAMetalLayer {
    unsafe { &*(layer.as_ptr().as_ptr() as *const ObjcCAMetalLayer) }
}

fn drop_retained_mtl_texture(texture_handle: usize) {
    if texture_handle == 0 {
        return;
    }
    let _ = unsafe { Retained::from_raw(texture_handle as *mut ProtocolObject<dyn MTLTexture>) };
}

fn borrowed_mtl_texture(texture_handle: usize) -> Option<&'static ProtocolObject<dyn MTLTexture>> {
    borrowed_protocol_object(texture_handle as *mut ProtocolObject<dyn MTLTexture>)
}

fn drawable_mtl_drawable_ref(
    drawable: &ProtocolObject<dyn CAMetalDrawable>,
) -> &ProtocolObject<dyn MTLDrawable> {
    unsafe {
        &*(drawable as *const ProtocolObject<dyn CAMetalDrawable>
            as *const ProtocolObject<dyn MTLDrawable>)
    }
}

fn borrowed_protocol_object<P: ?Sized>(
    ptr: *mut ProtocolObject<P>,
) -> Option<&'static ProtocolObject<P>> {
    let ptr = NonNull::new(ptr)?;
    Some(unsafe { ptr.as_ref() })
}
