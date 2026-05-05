use std::sync::{Mutex, OnceLock};

use bytemuck::{Pod, Zeroable};
use vello::wgpu;
use wgpu::util::DeviceExt;

/// Parameters for a single inner shadow effect, in device pixels.
#[derive(Debug, Clone, Copy)]
pub struct InnerShadowEffect {
    /// Rect position (top-left) in device pixels.
    pub rect_min: [f32; 2],
    /// Rect size in device pixels.
    pub rect_size: [f32; 2],
    /// Corner radius in device pixels.
    pub corner_radius: f32,
    /// Shadow offset in device pixels.
    pub offset: [f32; 2],
    /// Gaussian blur sigma in device pixels.
    pub blur_std_dev: f32,
    /// Shadow color, premultiplied RGBA.
    pub color: [f32; 4],
}

struct EffectPipelineState {
    pipeline: wgpu::RenderPipeline,
}

fn effect_pipeline(device: &wgpu::Device) -> &'static EffectPipelineState {
    static STATE: OnceLock<EffectPipelineState> = OnceLock::new();
    STATE.get_or_init(|| create_pipeline(device))
}

fn create_pipeline(device: &wgpu::Device) -> EffectPipelineState {
    let shader_source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/shaders/inner_shadow.wgsl"
    ));
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("inner-shadow-shader"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("inner-shadow-bind-group-layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: std::num::NonZeroU64::new(64),
            },
            count: None,
        }],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("inner-shadow-pipeline-layout"),
        bind_group_layouts: &[&bind_group_layout],
        immediate_size: 0,
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("inner-shadow-pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8Unorm,
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });

    EffectPipelineState { pipeline }
}

/// Apply inner shadow effects to the given render target.
///
/// Each effect is rendered as a fullscreen triangle with SDF-based alpha.
/// The uniform layout matches the WGSL `Params` struct:
///
/// ```text
/// rect_min:      vec2<f32>  offset  0
/// rect_size:     vec2<f32>  offset  8
/// corner_radius: f32        offset 16
/// blur_std_dev:  f32        offset 20
/// offset:        vec2<f32>  offset 24
/// color:         vec4<f32>  offset 32
/// texture_size:  vec2<f32>  offset 48
/// _padding:      vec2<f32>  offset 56
/// total: 64 bytes
/// ```
pub fn apply_inner_shadows(
    device: &wgpu::Device,
    _queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    target_view: &wgpu::TextureView,
    texture_size: (u32, u32),
    effects: &[InnerShadowEffect],
) {
    if effects.is_empty() {
        return;
    }

    let state = effect_pipeline(device);

    for effect in effects {
        let data = make_inner_shadow_uniform(effect, texture_size);
        let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("inner-shadow-uniform"),
            contents: &data,
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("inner-shadow-bind-group"),
            layout: &state.pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("inner-shadow-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });

        pass.set_pipeline(&state.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct InnerShadowUniforms {
    rect_min: [f32; 2],
    rect_size: [f32; 2],
    corner_radius: f32,
    blur_std_dev: f32,
    offset: [f32; 2],
    color: [f32; 4],
    texture_size: [f32; 2],
    _padding: [f32; 2],
}

fn make_inner_shadow_uniform(effect: &InnerShadowEffect, texture_size: (u32, u32)) -> [u8; 64] {
    bytemuck::cast(InnerShadowUniforms {
        rect_min: effect.rect_min,
        rect_size: effect.rect_size,
        corner_radius: effect.corner_radius,
        blur_std_dev: effect.blur_std_dev,
        offset: effect.offset,
        color: effect.color,
        texture_size: [texture_size.0 as f32, texture_size.1 as f32],
        _padding: [0.0; 2],
    })
}

// ---------------------------------------------------------------------------
// Backdrop blur
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct BackdropBlurEffect {
    pub rect_min: [f32; 2],
    pub rect_size: [f32; 2],
    pub corner_radius: f32,
    pub blur_radius: f32,
}

struct BlurPipelineState {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

fn blur_pipeline(device: &wgpu::Device) -> &'static BlurPipelineState {
    static STATE: OnceLock<BlurPipelineState> = OnceLock::new();
    STATE.get_or_init(|| create_blur_pipeline(device))
}

fn create_blur_pipeline(device: &wgpu::Device) -> BlurPipelineState {
    let shader_source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/shaders/backdrop_blur.wgsl"
    ));
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("backdrop-blur-shader"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("backdrop-blur-bind-group-layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: std::num::NonZeroU64::new(48),
                },
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
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("backdrop-blur-pipeline-layout"),
        bind_group_layouts: &[&bind_group_layout],
        immediate_size: 0,
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("backdrop-blur-pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8Unorm,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("backdrop-blur-sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        ..Default::default()
    });

    BlurPipelineState {
        pipeline,
        bind_group_layout,
        sampler,
    }
}

// ---------------------------------------------------------------------------
// Scratch texture cache for ping-pong blur passes
// ---------------------------------------------------------------------------

struct ScratchTexture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
}

static SCRATCH: OnceLock<Mutex<Option<ScratchTexture>>> = OnceLock::new();

fn ensure_scratch_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> &'static Mutex<Option<ScratchTexture>> {
    let mutex = SCRATCH.get_or_init(|| Mutex::new(None));
    let mut guard = mutex.lock().unwrap();
    let needs_recreate = match guard.as_ref() {
        Some(s) => s.width != width || s.height != height,
        None => true,
    };
    if needs_recreate {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("backdrop-blur-scratch"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        *guard = Some(ScratchTexture {
            texture,
            view,
            width,
            height,
        });
    }
    mutex
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn apply_backdrop_blurs(
    device: &wgpu::Device,
    _queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    target_texture: &wgpu::Texture,
    target_view: &wgpu::TextureView,
    texture_size: (u32, u32),
    effects: &[BackdropBlurEffect],
) {
    if effects.is_empty() {
        return;
    }

    let blur_state = blur_pipeline(device);

    // Create a separate view of the target texture for sampling.
    let target_sample_view = target_texture.create_view(&wgpu::TextureViewDescriptor::default());

    for effect in effects {
        // Use a downscaled scratch texture for large blur radii.
        // Bilinear sampling handles the up/downscaling; this reduces texture
        // samples proportionally to `downscale²`.
        let downscale = if effect.blur_radius > 20.0 {
            4u32
        } else if effect.blur_radius > 8.0 {
            2u32
        } else {
            1u32
        };
        let scratch_w = (texture_size.0 / downscale).max(1);
        let scratch_h = (texture_size.1 / downscale).max(1);

        let scratch_mutex = ensure_scratch_texture(device, scratch_w, scratch_h);
        let scratch_guard = scratch_mutex.lock().unwrap();
        let scratch = scratch_guard.as_ref().unwrap();

        // Horizontal uniform: source = full-res target, direction = (1, 0)
        let h_uniform = {
            let data = make_blur_uniform(effect, texture_size, [1.0, 0.0]);
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("backdrop-blur-h-uniform"),
                contents: &data,
                usage: wgpu::BufferUsages::UNIFORM,
            })
        };

        // Vertical uniform: source = downscaled scratch, direction = (0, 1).
        // SDF params and blur_radius are scaled to match the scratch resolution.
        let v_uniform = {
            let ds = downscale as f32;
            let v_effect = BackdropBlurEffect {
                rect_min: [effect.rect_min[0] / ds, effect.rect_min[1] / ds],
                rect_size: [effect.rect_size[0] / ds, effect.rect_size[1] / ds],
                corner_radius: effect.corner_radius / ds,
                blur_radius: effect.blur_radius / ds,
            };
            let data = make_blur_uniform(&v_effect, (scratch_w, scratch_h), [0.0, 1.0]);
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("backdrop-blur-v-uniform"),
                contents: &data,
                usage: wgpu::BufferUsages::UNIFORM,
            })
        };

        let h_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("backdrop-blur-h-bind-group"),
            layout: &blur_state.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: h_uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&target_sample_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&blur_state.sampler),
                },
            ],
        });

        // Horizontal pass: target → scratch
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("backdrop-blur-h-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &scratch.view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&blur_state.pipeline);
            pass.set_bind_group(0, &h_bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        let v_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("backdrop-blur-v-bind-group"),
            layout: &blur_state.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: v_uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&scratch.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&blur_state.sampler),
                },
            ],
        });

        // Vertical pass: scratch → target
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("backdrop-blur-v-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target_view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&blur_state.pipeline);
            pass.set_bind_group(0, &v_bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct BlurUniforms {
    rect_min: [f32; 2],
    rect_size: [f32; 2],
    corner_radius: f32,
    blur_radius: f32,
    texture_size: [f32; 2],
    direction: [f32; 2],
    _padding: [f32; 2],
}

fn make_blur_uniform(
    effect: &BackdropBlurEffect,
    texture_size: (u32, u32),
    direction: [f32; 2],
) -> [u8; 48] {
    bytemuck::cast(BlurUniforms {
        rect_min: effect.rect_min,
        rect_size: effect.rect_size,
        corner_radius: effect.corner_radius,
        blur_radius: effect.blur_radius,
        texture_size: [texture_size.0 as f32, texture_size.1 as f32],
        direction,
        _padding: [0.0; 2],
    })
}

// ---------------------------------------------------------------------------
// Outer drop shadow (rendered behind promoted layers on the surface pass)
// ---------------------------------------------------------------------------

/// Parameters for a single outer shadow effect, in local layer pixels.
#[derive(Debug, Clone, Copy)]
pub struct OuterShadowEffect {
    /// 4x4 column-major transform: local px → clip space (same as composite layer).
    pub transform: [f32; 16],
    /// Expanded shadow bounds in local pixels: (x, y, w, h).
    pub bounds: [f32; 4],
    /// Corner radius in local pixels.
    pub corner_radius: f32,
    /// Blur radius in local pixels.
    pub blur_radius: f32,
    /// Shadow offset relative to layer center, in local pixels.
    pub offset: [f32; 2],
    /// Shadow color (premultiplied RGBA).
    pub color: [f32; 4],
}

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct OuterShadowUniforms {
    transform: [f32; 16], // 64 bytes
    bounds: [f32; 4],     // 16 bytes
    shadow_params: [f32; 4], // corner_radius, blur_radius, offset_x, offset_y
    color: [f32; 4],      // 16 bytes
    _padding: [f32; 4],   // pad to 128
}

pub struct OuterShadowPipeline {
    pipeline: wgpu::RenderPipeline,
}

/// Create an outer-shadow pipeline targeting the given surface format.
/// Called once per window during `WindowSurface` init.
pub fn create_outer_shadow_pipeline(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
) -> OuterShadowPipeline {
    let shader_source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/shaders/outer_shadow.wgsl"
    ));
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("outer-shadow-shader"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("outer-shadow-bgl"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: std::num::NonZeroU64::new(128),
            },
            count: None,
        }],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("outer-shadow-pl"),
        bind_group_layouts: &[&bind_group_layout],
        immediate_size: 0,
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("outer-shadow-pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_format,
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::One,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });

    OuterShadowPipeline { pipeline }
}

/// Draw outer shadows into the given render pass.
///
/// The pass must already be begun with the appropriate surface color attachment.
/// Each shadow is drawn as a 6-vertex expanded quad transformed by the layer's
/// 4x4 matrix, so it follows 3D perspective.
pub fn draw_outer_shadows(
    device: &wgpu::Device,
    pass: &mut wgpu::RenderPass<'_>,
    pipeline: &OuterShadowPipeline,
    effects: &[OuterShadowEffect],
) {
    if effects.is_empty() {
        return;
    }

    pass.set_pipeline(&pipeline.pipeline);

    for effect in effects {
        let uniforms = OuterShadowUniforms {
            transform: effect.transform,
            bounds: effect.bounds,
            shadow_params: [
                effect.corner_radius,
                effect.blur_radius,
                effect.offset[0],
                effect.offset[1],
            ],
            color: effect.color,
            _padding: [0.0; 4],
        };
        let data: [u8; 128] = bytemuck::cast(uniforms);
        let uniform_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("outer-shadow-uniform"),
            contents: &data,
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("outer-shadow-bg"),
            layout: &pipeline.pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buf.as_entire_binding(),
            }],
        });

        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..6, 0..1);
    }
}

// ---------------------------------------------------------------------------
// Content filter (post-process for promoted layers)
// ---------------------------------------------------------------------------

/// Per-layer content filter parameters.
///
/// Identity values (no visual change): grayscale=0, saturate=1, brightness=1,
/// contrast=1, hue_rotate=0, invert=0, sepia=0.
#[derive(Debug, Clone, Copy)]
pub struct ContentFilterEffect {
    pub grayscale: f32,
    /// Multiplier; 1.0 = normal saturation.
    pub saturate: f32,
    /// Multiplier; 1.0 = normal brightness.
    pub brightness: f32,
    /// Multiplier; 1.0 = normal contrast.
    pub contrast: f32,
    /// Hue rotation in degrees.
    pub hue_rotate: f32,
    /// 0..1
    pub invert: f32,
    /// 0..1
    pub sepia: f32,
}

impl ContentFilterEffect {
    pub fn is_identity(&self) -> bool {
        self.grayscale == 0.0
            && self.saturate == 1.0
            && self.brightness == 1.0
            && self.contrast == 1.0
            && self.hue_rotate == 0.0
            && self.invert == 0.0
            && self.sepia == 0.0
    }
}

impl Default for ContentFilterEffect {
    fn default() -> Self {
        Self {
            grayscale: 0.0,
            saturate: 1.0,
            brightness: 1.0,
            contrast: 1.0,
            hue_rotate: 0.0,
            invert: 0.0,
            sepia: 0.0,
        }
    }
}

struct ContentFilterPipelineState {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

fn content_filter_pipeline(device: &wgpu::Device) -> &'static ContentFilterPipelineState {
    static STATE: OnceLock<ContentFilterPipelineState> = OnceLock::new();
    STATE.get_or_init(|| create_content_filter_pipeline(device))
}

fn create_content_filter_pipeline(device: &wgpu::Device) -> ContentFilterPipelineState {
    let shader_source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/shaders/content_filter.wgsl"
    ));
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("content-filter-shader"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("content-filter-bind-group-layout"),
        entries: &[
            // binding 0: FilterParams uniform
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: std::num::NonZeroU64::new(48),
                },
                count: None,
            },
            // binding 1: source texture
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
            // binding 2: sampler
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("content-filter-pipeline-layout"),
        bind_group_layouts: &[&bind_group_layout],
        immediate_size: 0,
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("content-filter-pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8Unorm,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("content-filter-sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        ..Default::default()
    });

    ContentFilterPipelineState {
        pipeline,
        bind_group_layout,
        sampler,
    }
}

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct ContentFilterUniforms {
    grayscale: f32,
    saturate: f32,
    brightness: f32,
    contrast: f32,
    hue_rotate: f32,
    invert: f32,
    sepia: f32,
    _pad: f32,
    texture_size: [f32; 2],
    _pad2: [f32; 2],
}

fn make_content_filter_uniform(
    effect: &ContentFilterEffect,
    texture_size: (u32, u32),
) -> [u8; 48] {
    bytemuck::cast(ContentFilterUniforms {
        grayscale: effect.grayscale,
        saturate: effect.saturate,
        brightness: effect.brightness,
        contrast: effect.contrast,
        hue_rotate: effect.hue_rotate,
        invert: effect.invert,
        sepia: effect.sepia,
        _pad: 0.0,
        texture_size: [texture_size.0 as f32, texture_size.1 as f32],
        _pad2: [0.0; 2],
    })
}

/// Apply a content filter post-process to a layer texture.
///
/// Reads from `target_texture` (via copy to scratch), writes filtered result
/// back to `target_view`. No-op when the filter is identity.
pub fn apply_content_filter(
    device: &wgpu::Device,
    _queue: &wgpu::Queue,
    encoder: &mut wgpu::CommandEncoder,
    target_texture: &wgpu::Texture,
    target_view: &wgpu::TextureView,
    texture_size: (u32, u32),
    effect: &ContentFilterEffect,
) {
    if effect.is_identity() {
        return;
    }

    let state = content_filter_pipeline(device);

    // Reuse the shared scratch texture (same one as backdrop blur).
    let scratch_mutex = ensure_scratch_texture(device, texture_size.0, texture_size.1);
    let scratch_guard = scratch_mutex.lock().unwrap();
    let scratch = scratch_guard.as_ref().unwrap();

    // Copy current layer content to scratch so we can sample it.
    encoder.copy_texture_to_texture(
        wgpu::TexelCopyTextureInfo {
            texture: target_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyTextureInfo {
            texture: &scratch.texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::Extent3d {
            width: texture_size.0,
            height: texture_size.1,
            depth_or_array_layers: 1,
        },
    );

    let data = make_content_filter_uniform(effect, texture_size);
    let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("content-filter-uniform"),
        contents: &data,
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("content-filter-bind-group"),
        layout: &state.bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&scratch.view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&state.sampler),
            },
        ],
    });

    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("content-filter-pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target_view,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    });

    pass.set_pipeline(&state.pipeline);
    pass.set_bind_group(0, &bind_group, &[]);
    pass.draw(0..3, 0..1);
}

// ---------------------------------------------------------------------------
// Layer mask
// ---------------------------------------------------------------------------

pub struct LayerMaskEffect {
    pub texture_size: (u32, u32),
}

struct MaskPipelineState {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

fn mask_pipeline(device: &wgpu::Device) -> &'static MaskPipelineState {
    static STATE: OnceLock<MaskPipelineState> = OnceLock::new();
    STATE.get_or_init(|| create_mask_pipeline(device))
}

fn create_mask_pipeline(device: &wgpu::Device) -> MaskPipelineState {
    let shader_source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/shaders/layer_mask.wgsl"
    ));
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("layer-mask-shader"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("layer-mask-bind-group-layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: std::num::NonZeroU64::new(16),
                },
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
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("layer-mask-pipeline-layout"),
        bind_group_layouts: &[&bind_group_layout],
        immediate_size: 0,
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("layer-mask-pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8Unorm,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("layer-mask-sampler"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    MaskPipelineState {
        pipeline,
        bind_group_layout,
        sampler,
    }
}

/// Apply alpha mask from `mask_view` onto content rendered to `target_view`.
/// Reads content from `content_view`, mask from `mask_view`, writes to `target_view`.
pub fn apply_layer_mask(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    target_texture: &wgpu::Texture,
    target_view: &wgpu::TextureView,
    mask_view: &wgpu::TextureView,
    texture_size: (u32, u32),
) {
    let state = mask_pipeline(device);

    // Copy target → scratch so we can sample content while writing back.
    let scratch_mutex = ensure_scratch_texture(device, texture_size.0, texture_size.1);
    let scratch_guard = scratch_mutex.lock().unwrap();
    let scratch = scratch_guard.as_ref().unwrap();

    encoder.copy_texture_to_texture(
        wgpu::TexelCopyTextureInfo {
            texture: target_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyTextureInfo {
            texture: &scratch.texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::Extent3d {
            width: texture_size.0,
            height: texture_size.1,
            depth_or_array_layers: 1,
        },
    );

    let uniform_data = [
        (texture_size.0 as f32).to_le_bytes(),
        (texture_size.1 as f32).to_le_bytes(),
        0.0f32.to_le_bytes(),
        0.0f32.to_le_bytes(),
    ]
    .concat();

    let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("layer-mask-uniform"),
        contents: &uniform_data,
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("layer-mask-bind-group"),
        layout: &state.bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&scratch.view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(mask_view),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::Sampler(&state.sampler),
            },
        ],
    });

    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("layer-mask-pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target_view,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    });
    pass.set_pipeline(&state.pipeline);
    pass.set_bind_group(0, &bind_group, &[]);
    pass.draw(0..3, 0..1);
}

// ---------------------------------------------------------------------------
// Vibrancy effect
// ---------------------------------------------------------------------------

/// Parameters for the vibrancy compositing pass.
///
/// Runs *after* backdrop blur has already produced a blurred texture.
/// Desaturates the blurred backdrop, applies a tint, then blends a
/// foreground layer on top with a configurable blend mode.
#[derive(Debug, Clone, Copy)]
pub struct VibrancyEffect {
    /// 0.0 = keep colour, 1.0 = fully desaturated.
    pub desaturation: f32,
    /// 0 = multiply, 1 = screen, 2 = overlay, 3 = plus-lighter.
    pub blend_mode: u32,
    /// Premultiplied RGBA tint applied to the desaturated backdrop.
    pub tint: [f32; 4],
}

struct VibrancyPipelineState {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
}

fn vibrancy_pipeline(device: &wgpu::Device) -> &'static VibrancyPipelineState {
    static STATE: OnceLock<VibrancyPipelineState> = OnceLock::new();
    STATE.get_or_init(|| create_vibrancy_pipeline(device))
}

fn create_vibrancy_pipeline(device: &wgpu::Device) -> VibrancyPipelineState {
    let shader_source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/shaders/vibrancy.wgsl"
    ));
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("vibrancy-shader"),
        source: wgpu::ShaderSource::Wgsl(shader_source.into()),
    });

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("vibrancy-bind-group-layout"),
        entries: &[
            // binding 0: VibrancyParams uniform (48 bytes)
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: std::num::NonZeroU64::new(48),
                },
                count: None,
            },
            // binding 1: backdrop texture (already blurred)
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
            // binding 2: foreground texture
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            // binding 3: sampler
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("vibrancy-pipeline-layout"),
        bind_group_layouts: &[&bind_group_layout],
        immediate_size: 0,
    });

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("vibrancy-pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba8Unorm,
                blend: Some(wgpu::BlendState::REPLACE),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("vibrancy-sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Linear,
        ..Default::default()
    });

    VibrancyPipelineState {
        pipeline,
        bind_group_layout,
        sampler,
    }
}

/// Uniform buffer layout matching the WGSL `VibrancyParams` struct (48 bytes).
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
struct VibrancyUniforms {
    desaturation: f32,
    blend_mode: f32,
    _align_pad: [f32; 2],
    tint: [f32; 4],
    texture_size: [f32; 2],
    _padding: [f32; 2],
}

fn make_vibrancy_uniform(effect: &VibrancyEffect, texture_size: (u32, u32)) -> [u8; 48] {
    bytemuck::cast(VibrancyUniforms {
        desaturation: effect.desaturation,
        blend_mode: effect.blend_mode as f32,
        _align_pad: [0.0; 2],
        tint: effect.tint,
        texture_size: [texture_size.0 as f32, texture_size.1 as f32],
        _padding: [0.0; 2],
    })
}

/// Composite a foreground layer over a desaturated+tinted blurred backdrop.
///
/// Both `backdrop_view` (already blurred) and `foreground_view` are sampled;
/// the blended result is written to `target_view`.
pub fn apply_vibrancy(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    target_view: &wgpu::TextureView,
    backdrop_view: &wgpu::TextureView,
    foreground_view: &wgpu::TextureView,
    texture_size: (u32, u32),
    effect: &VibrancyEffect,
) {
    let state = vibrancy_pipeline(device);

    let data = make_vibrancy_uniform(effect, texture_size);
    let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("vibrancy-uniform"),
        contents: &data,
        usage: wgpu::BufferUsages::UNIFORM,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("vibrancy-bind-group"),
        layout: &state.bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(backdrop_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(foreground_view),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::Sampler(&state.sampler),
            },
        ],
    });

    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("vibrancy-pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: target_view,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        })],
        ..Default::default()
    });

    pass.set_pipeline(&state.pipeline);
    pass.set_bind_group(0, &bind_group, &[]);
    pass.draw(0..3, 0..1);
}
