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
    let shader_source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/shaders/inner_shadow.wgsl"));
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

    EffectPipelineState {
        pipeline,
    }
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
    let shader_source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/shaders/backdrop_blur.wgsl"));
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
    let scratch_mutex = ensure_scratch_texture(device, texture_size.0, texture_size.1);
    let scratch_guard = scratch_mutex.lock().unwrap();
    let scratch = scratch_guard.as_ref().unwrap();

    // Create a separate view of the target texture for sampling.
    let target_sample_view =
        target_texture.create_view(&wgpu::TextureViewDescriptor::default());

    for effect in effects {
        // Horizontal uniform: direction = (1, 0)
        let h_uniform = {
            let data = make_blur_uniform(effect, texture_size, [1.0, 0.0]);
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("backdrop-blur-h-uniform"),
                contents: &data,
                usage: wgpu::BufferUsages::UNIFORM,
            })
        };

        // Vertical uniform: direction = (0, 1)
        let v_uniform = {
            let data = make_blur_uniform(effect, texture_size, [0.0, 1.0]);
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
