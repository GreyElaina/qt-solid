use std::{
    collections::HashMap,
    ffi::c_void,
    num::{NonZeroIsize, NonZeroU32},
    ptr::NonNull,
    sync::{Arc, Mutex},
};

use bytemuck::{Pod, Zeroable};
use once_cell::sync::Lazy;
use raw_window_handle::{
    AppKitDisplayHandle, AppKitWindowHandle, RawDisplayHandle, RawWindowHandle,
    WaylandDisplayHandle, WaylandWindowHandle, Win32WindowHandle, WindowsDisplayHandle,
    XcbDisplayHandle, XcbWindowHandle,
};
use wgpu::util::DeviceExt;

use crate::{QtWgpuRendererError, Result};

pub const QT_COMPOSITOR_SURFACE_APPKIT_NS_VIEW: u8 = 1;
pub const QT_COMPOSITOR_SURFACE_WIN32_HWND: u8 = 2;
pub const QT_COMPOSITOR_SURFACE_XCB_WINDOW: u8 = 3;
pub const QT_COMPOSITOR_SURFACE_WAYLAND_SURFACE: u8 = 4;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QtCompositorTarget {
    pub surface_kind: u8,
    pub primary_handle: u64,
    pub secondary_handle: u64,
    pub width_px: u32,
    pub height_px: u32,
    pub scale_factor: f64,
}

impl QtCompositorTarget {
    fn surface_key(self) -> SurfaceKey {
        SurfaceKey {
            surface_kind: self.surface_kind,
            primary_handle: self.primary_handle,
            secondary_handle: self.secondary_handle,
        }
    }

    unsafe fn surface_target(self) -> Result<wgpu::SurfaceTargetUnsafe> {
        match self.surface_kind {
            QT_COMPOSITOR_SURFACE_APPKIT_NS_VIEW => {
                let Some(ns_view) = NonNull::new(self.primary_handle as *mut c_void) else {
                    return Err(QtWgpuRendererError::new(
                        "qt compositor target is missing NSView handle",
                    ));
                };
                Ok(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: RawDisplayHandle::AppKit(AppKitDisplayHandle::new()),
                    raw_window_handle: RawWindowHandle::AppKit(AppKitWindowHandle::new(ns_view)),
                })
            }
            QT_COMPOSITOR_SURFACE_WIN32_HWND => {
                let Some(hwnd) = NonZeroIsize::new(self.primary_handle as isize) else {
                    return Err(QtWgpuRendererError::new(
                        "qt compositor target is missing HWND handle",
                    ));
                };
                Ok(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: RawDisplayHandle::Windows(WindowsDisplayHandle::new()),
                    raw_window_handle: RawWindowHandle::Win32(Win32WindowHandle::new(hwnd)),
                })
            }
            QT_COMPOSITOR_SURFACE_XCB_WINDOW => {
                let Some(window) = NonZeroU32::new(self.primary_handle as u32) else {
                    return Err(QtWgpuRendererError::new(
                        "qt compositor target is missing XCB window handle",
                    ));
                };
                let Some(connection) = NonNull::new(self.secondary_handle as *mut c_void) else {
                    return Err(QtWgpuRendererError::new(
                        "qt compositor target is missing XCB connection handle",
                    ));
                };
                Ok(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: RawDisplayHandle::Xcb(XcbDisplayHandle::new(
                        Some(connection),
                        0,
                    )),
                    // Qt bridge does not propagate screen yet. Use current/default screen.
                    raw_window_handle: RawWindowHandle::Xcb(XcbWindowHandle::new(window)),
                })
            }
            QT_COMPOSITOR_SURFACE_WAYLAND_SURFACE => {
                let Some(surface) = NonNull::new(self.primary_handle as *mut c_void) else {
                    return Err(QtWgpuRendererError::new(
                        "qt compositor target is missing Wayland surface handle",
                    ));
                };
                let Some(display) = NonNull::new(self.secondary_handle as *mut c_void) else {
                    return Err(QtWgpuRendererError::new(
                        "qt compositor target is missing Wayland display handle",
                    ));
                };
                Ok(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle: RawDisplayHandle::Wayland(WaylandDisplayHandle::new(
                        display,
                    )),
                    raw_window_handle: RawWindowHandle::Wayland(WaylandWindowHandle::new(surface)),
                })
            }
            other => Err(QtWgpuRendererError::new(format!(
                "unsupported qt compositor surface kind {other}",
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QtCompositorImageFormat {
    Bgra8UnormPremultiplied,
    Rgba8UnormPremultiplied,
}

impl QtCompositorImageFormat {
    fn texture_format(self) -> wgpu::TextureFormat {
        match self {
            Self::Bgra8UnormPremultiplied => wgpu::TextureFormat::Bgra8Unorm,
            Self::Rgba8UnormPremultiplied => wgpu::TextureFormat::Rgba8Unorm,
        }
    }

    fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Bgra8UnormPremultiplied | Self::Rgba8UnormPremultiplied => 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QtCompositorUploadKind {
    None,
    Full,
    SubRects,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QtCompositorLayerSourceKind {
    CpuBytes,
    CachedTexture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QtCompositorRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Copy)]
pub struct QtCompositorBaseUpload<'a> {
    pub format: QtCompositorImageFormat,
    pub width_px: u32,
    pub height_px: u32,
    pub stride: usize,
    pub upload_kind: QtCompositorUploadKind,
    pub dirty_rects: &'a [QtCompositorRect],
    pub bytes: &'a [u8],
}

#[derive(Debug, Clone, Copy)]
pub struct QtCompositorLayerUpload<'a> {
    pub node_id: u32,
    pub source_kind: QtCompositorLayerSourceKind,
    pub format: QtCompositorImageFormat,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub width_px: u32,
    pub height_px: u32,
    pub stride: usize,
    pub upload_kind: QtCompositorUploadKind,
    pub dirty_rects: &'a [QtCompositorRect],
    pub visible_rects: &'a [QtCompositorRect],
    pub bytes: &'a [u8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct SurfaceKey {
    surface_kind: u8,
    primary_handle: u64,
    secondary_handle: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CompositeVertex {
    position: [f32; 2],
    uv: [f32; 2],
}

impl CompositeVertex {
    fn layout() -> wgpu::VertexBufferLayout<'static> {
        use std::mem::size_of;

        wgpu::VertexBufferLayout {
            array_stride: size_of::<CompositeVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: size_of::<[f32; 2]>() as u64,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

struct CachedImageTexture {
    format: QtCompositorImageFormat,
    width_px: u32,
    height_px: u32,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
}

impl CachedImageTexture {
    fn ensure_descriptor_matches(
        &self,
        format: QtCompositorImageFormat,
        width_px: u32,
        height_px: u32,
    ) -> bool {
        self.format == format && self.width_px == width_px && self.height_px == height_px
    }
}

struct CompositorPipelineState {
    bind_group_layout: wgpu::BindGroupLayout,
    pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
}

#[derive(Debug, Clone, Copy)]
struct TextureUploadRect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

struct WindowCompositorContext {
    target: QtCompositorTarget,
    instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: CompositorPipelineState,
    base_texture: Option<CachedImageTexture>,
    layer_textures: HashMap<u32, CachedImageTexture>,
}

type WindowCompositorContextHandle = Arc<Mutex<WindowCompositorContext>>;

static WINDOW_COMPOSITORS: Lazy<Mutex<HashMap<SurfaceKey, WindowCompositorContextHandle>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

const COMPOSITOR_SHADER: &str = r#"
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.position = vec4<f32>(input.position, 0.0, 1.0);
    output.uv = input.uv;
    return output;
}

@group(0) @binding(0)
var layer_sampler: sampler;

@group(0) @binding(1)
var layer_texture: texture_2d<f32>;

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(layer_texture, layer_sampler, input.uv);
}
"#;

pub fn present_compositor_frame(
    target: QtCompositorTarget,
    base: &QtCompositorBaseUpload<'_>,
    layers: &[QtCompositorLayerUpload<'_>],
) -> Result<()> {
    if target.width_px == 0 || target.height_px == 0 {
        return Ok(());
    }

    let context_handle = load_or_create_window_compositor(target)?;
    let mut context = context_handle
        .lock()
        .expect("qt wgpu compositor mutex poisoned");
    context.present(base, layers)
}

pub fn with_window_compositor_device_queue<T, F>(target: QtCompositorTarget, run: F) -> Result<T>
where
    F: FnOnce(&wgpu::Device, &wgpu::Queue) -> Result<T>,
{
    let context_handle = load_or_create_window_compositor(target)?;
    let context = context_handle
        .lock()
        .expect("qt wgpu compositor mutex poisoned");
    run(&context.device, &context.queue)
}

pub fn with_window_compositor_layer_texture<T, F>(
    target: QtCompositorTarget,
    node_id: u32,
    format: QtCompositorImageFormat,
    width_px: u32,
    height_px: u32,
    run: F,
) -> Result<T>
where
    F: FnOnce(&wgpu::Device, &wgpu::Queue, &wgpu::Texture, &wgpu::TextureView) -> Result<T>,
{
    let context_handle = load_or_create_window_compositor(target)?;
    let mut context = context_handle
        .lock()
        .expect("qt wgpu compositor mutex poisoned");
    let needs_recreate = context
        .layer_textures
        .get(&node_id)
        .map(|entry| !entry.ensure_descriptor_matches(format, width_px, height_px))
        .unwrap_or(true);
    if needs_recreate {
        let next_entry = create_cached_texture(
            &context.device,
            &context.pipeline,
            format,
            width_px,
            height_px,
            &format!("qt-solid-wgpu-layer-{node_id}"),
        );
        context.layer_textures.insert(node_id, next_entry);
    }
    let entry = context.layer_textures.get(&node_id).ok_or_else(|| {
        QtWgpuRendererError::new(format!(
            "qt compositor cached layer {} could not be allocated",
            node_id
        ))
    })?;
    run(&context.device, &context.queue, &entry.texture, &entry.view)
}

fn load_or_create_window_compositor(
    target: QtCompositorTarget,
) -> Result<WindowCompositorContextHandle> {
    let mut compositors = WINDOW_COMPOSITORS
        .lock()
        .expect("qt wgpu compositor registry mutex poisoned");
    let key = target.surface_key();
    if let Some(existing) = compositors.get(&key) {
        return Ok(Arc::clone(existing));
    }

    let compositor = Arc::new(Mutex::new(WindowCompositorContext::new(target)?));
    compositors.insert(key, Arc::clone(&compositor));
    Ok(compositor)
}

impl WindowCompositorContext {
    fn new(target: QtCompositorTarget) -> Result<Self> {
        let instance = wgpu::Instance::default();
        let surface = unsafe {
            instance
                .create_surface_unsafe(target.surface_target()?)
                .map_err(|error| QtWgpuRendererError::new(error.to_string()))?
        };
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }))
        .map_err(|error| QtWgpuRendererError::new(error.to_string()))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("qt-solid-wgpu-compositor-device"),
            ..Default::default()
        }))
        .map_err(|error| QtWgpuRendererError::new(error.to_string()))?;
        let config = surface
            .get_default_config(&adapter, target.width_px, target.height_px)
            .ok_or_else(|| {
                QtWgpuRendererError::new("failed to derive wgpu surface configuration")
            })?;
        surface.configure(&device, &config);
        let pipeline = create_pipeline_state(&device, config.format);

        Ok(Self {
            target,
            instance,
            surface,
            device,
            queue,
            config,
            pipeline,
            base_texture: None,
            layer_textures: HashMap::new(),
        })
    }

    fn present(
        &mut self,
        base: &QtCompositorBaseUpload<'_>,
        layers: &[QtCompositorLayerUpload<'_>],
    ) -> Result<()> {
        self.reconfigure_if_needed(base.width_px, base.height_px)?;
        upload_image(
            &self.device,
            &self.queue,
            &self.pipeline,
            &mut self.base_texture,
            base.format,
            base.width_px,
            base.height_px,
            base.stride,
            base.upload_kind,
            base.dirty_rects,
            base.bytes,
            "qt-solid-wgpu-base-texture",
        )?;

        let mut prepared_layers = Vec::with_capacity(layers.len());
        for layer in layers {
            let cached = upload_layer_texture(self, layer)?;
            prepared_layers.push((layer, cached.bind_group.clone()));
        }

        let surface_texture = self.acquire_surface_texture()?;
        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("qt-solid-wgpu-compositor-encoder"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("qt-solid-wgpu-compositor-pass"),
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
            pass.set_pipeline(&self.pipeline.pipeline);

            if let Some(base_texture) = &self.base_texture {
                draw_quad(
                    &self.device,
                    &mut pass,
                    &base_texture.bind_group,
                    self.target.width_px,
                    self.target.height_px,
                    0,
                    0,
                    self.target.width_px as i32,
                    self.target.height_px as i32,
                    0.0,
                    0.0,
                    1.0,
                    1.0,
                );
            }

            for (layer, bind_group) in prepared_layers {
                for visible_rect in layer.visible_rects {
                    if visible_rect.width <= 0 || visible_rect.height <= 0 {
                        continue;
                    }

                    let logical_width = layer.width.max(1) as f32;
                    let logical_height = layer.height.max(1) as f32;
                    let u0 = visible_rect.x as f32 / logical_width;
                    let u1 = (visible_rect.x + visible_rect.width) as f32 / logical_width;
                    let v0 = visible_rect.y as f32 / logical_height;
                    let v1 = (visible_rect.y + visible_rect.height) as f32 / logical_height;

                    draw_quad(
                        &self.device,
                        &mut pass,
                        &bind_group,
                        self.target.width_px,
                        self.target.height_px,
                        layer.x + visible_rect.x,
                        layer.y + visible_rect.y,
                        visible_rect.width,
                        visible_rect.height,
                        u0,
                        v0,
                        u1,
                        v1,
                    );
                }
            }
        }

        self.queue.submit([encoder.finish()]);
        surface_texture.present();
        Ok(())
    }

    fn reconfigure_if_needed(&mut self, width_px: u32, height_px: u32) -> Result<()> {
        if width_px == 0 || height_px == 0 {
            return Ok(());
        }

        if self.config.width != width_px || self.config.height != height_px {
            self.config.width = width_px;
            self.config.height = height_px;
            self.surface.configure(&self.device, &self.config);
            self.target.width_px = width_px;
            self.target.height_px = height_px;
        }

        Ok(())
    }

    fn acquire_surface_texture(&mut self) -> Result<wgpu::SurfaceTexture> {
        for attempt in 0..2 {
            match self.surface.get_current_texture() {
                Ok(texture) => return Ok(texture),
                Err(wgpu::SurfaceError::Timeout) | Err(wgpu::SurfaceError::Other) => {
                    return Err(QtWgpuRendererError::new(
                        "qt wgpu compositor surface is currently unavailable",
                    ));
                }
                Err(wgpu::SurfaceError::Outdated) => {
                    self.surface.configure(&self.device, &self.config);
                }
                Err(wgpu::SurfaceError::Lost) => {
                    self.surface = unsafe {
                        self.instance
                            .create_surface_unsafe(self.target.surface_target()?)
                            .map_err(|error| QtWgpuRendererError::new(error.to_string()))?
                    };
                    self.surface.configure(&self.device, &self.config);
                }
                Err(wgpu::SurfaceError::OutOfMemory) => {
                    return Err(QtWgpuRendererError::new(
                        "qt wgpu compositor surface is out of memory",
                    ));
                }
            }

            if attempt == 1 {
                break;
            }
        }

        Err(QtWgpuRendererError::new(
            "failed to acquire current surface texture for qt wgpu compositor",
        ))
    }
}

fn upload_layer_texture<'a>(
    context: &'a mut WindowCompositorContext,
    layer: &QtCompositorLayerUpload<'_>,
) -> Result<&'a CachedImageTexture> {
    if layer.source_kind == QtCompositorLayerSourceKind::CachedTexture {
        let entry = context.layer_textures.get(&layer.node_id).ok_or_else(|| {
            QtWgpuRendererError::new(format!(
                "qt compositor cached layer {} is missing",
                layer.node_id
            ))
        })?;
        if !entry.ensure_descriptor_matches(layer.format, layer.width_px, layer.height_px) {
            return Err(QtWgpuRendererError::new(format!(
                "qt compositor cached layer {} descriptor does not match frame metadata",
                layer.node_id
            )));
        }
        return Ok(entry);
    }

    let mut upload_kind = layer.upload_kind;
    let entry = context
        .layer_textures
        .entry(layer.node_id)
        .or_insert_with(|| {
            create_cached_texture(
                &context.device,
                &context.pipeline,
                layer.format,
                layer.width_px,
                layer.height_px,
                &format!("qt-solid-wgpu-layer-{}", layer.node_id),
            )
        });
    if !entry.ensure_descriptor_matches(layer.format, layer.width_px, layer.height_px) {
        *entry = create_cached_texture(
            &context.device,
            &context.pipeline,
            layer.format,
            layer.width_px,
            layer.height_px,
            &format!("qt-solid-wgpu-layer-{}", layer.node_id),
        );
        upload_kind = QtCompositorUploadKind::Full;
    }
    write_texture_upload(
        &context.queue,
        &entry.texture,
        layer.format,
        layer.width_px,
        layer.height_px,
        layer.stride,
        upload_kind,
        layer.dirty_rects,
        layer.bytes,
    )?;
    Ok(entry)
}

fn upload_image(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    pipeline: &CompositorPipelineState,
    slot: &mut Option<CachedImageTexture>,
    format: QtCompositorImageFormat,
    width_px: u32,
    height_px: u32,
    stride: usize,
    upload_kind: QtCompositorUploadKind,
    dirty_rects: &[QtCompositorRect],
    bytes: &[u8],
    label: &str,
) -> Result<()> {
    let needs_recreate = slot
        .as_ref()
        .map(|texture| !texture.ensure_descriptor_matches(format, width_px, height_px))
        .unwrap_or(true);
    if needs_recreate {
        *slot = Some(create_cached_texture(
            device, pipeline, format, width_px, height_px, label,
        ));
    }
    let effective_upload_kind = if needs_recreate {
        QtCompositorUploadKind::Full
    } else {
        upload_kind
    };
    let texture = slot
        .as_ref()
        .expect("cached image texture inserted before upload");
    write_texture_upload(
        queue,
        &texture.texture,
        format,
        width_px,
        height_px,
        stride,
        effective_upload_kind,
        dirty_rects,
        bytes,
    )
}

fn create_cached_texture(
    device: &wgpu::Device,
    pipeline: &CompositorPipelineState,
    format: QtCompositorImageFormat,
    width_px: u32,
    height_px: u32,
    label: &str,
) -> CachedImageTexture {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: width_px.max(1),
            height: height_px.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: format.texture_format(),
        usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(label),
        layout: &pipeline.bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Sampler(&pipeline.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&view),
            },
        ],
    });

    CachedImageTexture {
        format,
        width_px,
        height_px,
        texture,
        view,
        bind_group,
    }
}

fn write_full_texture(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    format: QtCompositorImageFormat,
    width_px: u32,
    height_px: u32,
    stride: usize,
    bytes: &[u8],
) -> Result<()> {
    let bytes_per_pixel = format.bytes_per_pixel();
    if stride > u32::MAX as usize {
        return Err(QtWgpuRendererError::new(
            "qt compositor upload stride exceeds wgpu limits",
        ));
    }
    let minimum_row_bytes = width_px as usize * bytes_per_pixel;
    if stride < minimum_row_bytes {
        return Err(QtWgpuRendererError::new(
            "qt compositor upload stride is smaller than row byte size",
        ));
    }
    let expected_size = stride
        .checked_mul(height_px as usize)
        .ok_or_else(|| QtWgpuRendererError::new("qt compositor upload size overflow"))?;
    if bytes.len() < expected_size {
        return Err(QtWgpuRendererError::new(
            "qt compositor upload bytes are smaller than declared image layout",
        ));
    }

    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &bytes[..expected_size],
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(stride as u32),
            rows_per_image: Some(height_px),
        },
        wgpu::Extent3d {
            width: width_px,
            height: height_px,
            depth_or_array_layers: 1,
        },
    );
    Ok(())
}

fn write_texture_upload(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    format: QtCompositorImageFormat,
    width_px: u32,
    height_px: u32,
    stride: usize,
    upload_kind: QtCompositorUploadKind,
    dirty_rects: &[QtCompositorRect],
    bytes: &[u8],
) -> Result<()> {
    let effective_upload_kind =
        normalize_upload_kind(upload_kind, width_px, height_px, dirty_rects);
    match effective_upload_kind {
        QtCompositorUploadKind::None => Ok(()),
        QtCompositorUploadKind::Full => {
            write_full_texture(queue, texture, format, width_px, height_px, stride, bytes)
        }
        QtCompositorUploadKind::SubRects => {
            for rect in dirty_rects
                .iter()
                .copied()
                .filter_map(|rect| normalize_upload_rect(rect, width_px, height_px))
            {
                write_subrect_texture(queue, texture, format, stride, bytes, rect)?;
            }
            Ok(())
        }
    }
}

fn normalize_upload_kind(
    upload_kind: QtCompositorUploadKind,
    width_px: u32,
    height_px: u32,
    dirty_rects: &[QtCompositorRect],
) -> QtCompositorUploadKind {
    match upload_kind {
        QtCompositorUploadKind::None => QtCompositorUploadKind::None,
        QtCompositorUploadKind::Full => QtCompositorUploadKind::Full,
        QtCompositorUploadKind::SubRects => {
            let has_valid_rect = dirty_rects
                .iter()
                .copied()
                .any(|rect| normalize_upload_rect(rect, width_px, height_px).is_some());
            if has_valid_rect {
                QtCompositorUploadKind::SubRects
            } else {
                QtCompositorUploadKind::Full
            }
        }
    }
}

fn normalize_upload_rect(
    rect: QtCompositorRect,
    bounds_width: u32,
    bounds_height: u32,
) -> Option<TextureUploadRect> {
    let x0 = i64::from(rect.x).max(0) as u32;
    let y0 = i64::from(rect.y).max(0) as u32;
    let x1 = (i64::from(rect.x) + i64::from(rect.width)).max(0) as u32;
    let y1 = (i64::from(rect.y) + i64::from(rect.height)).max(0) as u32;
    let clipped_x0 = x0.min(bounds_width);
    let clipped_y0 = y0.min(bounds_height);
    let clipped_x1 = x1.min(bounds_width);
    let clipped_y1 = y1.min(bounds_height);
    if clipped_x1 <= clipped_x0 || clipped_y1 <= clipped_y0 {
        return None;
    }
    Some(TextureUploadRect {
        x: clipped_x0,
        y: clipped_y0,
        width: clipped_x1 - clipped_x0,
        height: clipped_y1 - clipped_y0,
    })
}

fn write_subrect_texture(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    format: QtCompositorImageFormat,
    stride: usize,
    bytes: &[u8],
    rect: TextureUploadRect,
) -> Result<()> {
    let bytes_per_pixel = format.bytes_per_pixel();
    if stride > u32::MAX as usize {
        return Err(QtWgpuRendererError::new(
            "qt compositor subrect upload stride exceeds wgpu limits",
        ));
    }
    let minimum_row_bytes = rect.width as usize * bytes_per_pixel;
    if stride < minimum_row_bytes {
        return Err(QtWgpuRendererError::new(
            "qt compositor subrect upload stride is smaller than row byte size",
        ));
    }

    let data_offset = rect
        .y
        .checked_mul(stride as u32)
        .and_then(|row_offset| {
            rect.x
                .checked_mul(bytes_per_pixel as u32)
                .and_then(|column_offset| row_offset.checked_add(column_offset))
        })
        .ok_or_else(|| QtWgpuRendererError::new("qt compositor subrect upload offset overflow"))?
        as usize;
    let last_row_offset = stride
        .checked_mul(rect.height.saturating_sub(1) as usize)
        .ok_or_else(|| QtWgpuRendererError::new("qt compositor subrect row size overflow"))?;
    let last_row_width = rect.width as usize * bytes_per_pixel;
    let required_size = data_offset
        .checked_add(last_row_offset)
        .and_then(|size| size.checked_add(last_row_width))
        .ok_or_else(|| QtWgpuRendererError::new("qt compositor subrect size overflow"))?;
    if bytes.len() < required_size {
        return Err(QtWgpuRendererError::new(
            "qt compositor subrect upload bytes are smaller than declared image layout",
        ));
    }

    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d {
                x: rect.x,
                y: rect.y,
                z: 0,
            },
            aspect: wgpu::TextureAspect::All,
        },
        bytes,
        wgpu::TexelCopyBufferLayout {
            offset: data_offset as u64,
            bytes_per_row: Some(stride as u32),
            rows_per_image: Some(rect.height),
        },
        wgpu::Extent3d {
            width: rect.width,
            height: rect.height,
            depth_or_array_layers: 1,
        },
    );
    Ok(())
}

fn create_pipeline_state(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
) -> CompositorPipelineState {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("qt-solid-wgpu-compositor-shader"),
        source: wgpu::ShaderSource::Wgsl(COMPOSITOR_SHADER.into()),
    });
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("qt-solid-wgpu-compositor-bind-group-layout"),
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
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("qt-solid-wgpu-compositor-pipeline-layout"),
        bind_group_layouts: &[&bind_group_layout],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("qt-solid-wgpu-compositor-pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[CompositeVertex::layout()],
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
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("qt-solid-wgpu-compositor-sampler"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });

    CompositorPipelineState {
        bind_group_layout,
        pipeline,
        sampler,
    }
}

fn draw_quad(
    device: &wgpu::Device,
    pass: &mut wgpu::RenderPass<'_>,
    bind_group: &wgpu::BindGroup,
    surface_width_px: u32,
    surface_height_px: u32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    u0: f32,
    v0: f32,
    u1: f32,
    v1: f32,
) {
    if width <= 0 || height <= 0 || surface_width_px == 0 || surface_height_px == 0 {
        return;
    }

    let left = ((x as f32 / surface_width_px as f32) * 2.0) - 1.0;
    let right = (((x + width) as f32 / surface_width_px as f32) * 2.0) - 1.0;
    let top = 1.0 - ((y as f32 / surface_height_px as f32) * 2.0);
    let bottom = 1.0 - (((y + height) as f32 / surface_height_px as f32) * 2.0);
    let vertices = [
        CompositeVertex {
            position: [left, top],
            uv: [u0, v0],
        },
        CompositeVertex {
            position: [right, top],
            uv: [u1, v0],
        },
        CompositeVertex {
            position: [left, bottom],
            uv: [u0, v1],
        },
        CompositeVertex {
            position: [left, bottom],
            uv: [u0, v1],
        },
        CompositeVertex {
            position: [right, top],
            uv: [u1, v0],
        },
        CompositeVertex {
            position: [right, bottom],
            uv: [u1, v1],
        },
    ];
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("qt-solid-wgpu-compositor-quad"),
        contents: bytemuck::cast_slice(&vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    pass.set_bind_group(0, bind_group, &[]);
    pass.set_vertex_buffer(0, vertex_buffer.slice(..));
    pass.draw(0..vertices.len() as u32, 0..1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appkit_surface_target_requires_handle() {
        let target = QtCompositorTarget {
            surface_kind: QT_COMPOSITOR_SURFACE_APPKIT_NS_VIEW,
            primary_handle: 0,
            secondary_handle: 0,
            width_px: 1,
            height_px: 1,
            scale_factor: 1.0,
        };

        let error = match unsafe { target.surface_target() } {
            Ok(_) => panic!("missing NSView must fail"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("NSView"));
    }

    #[test]
    fn win32_surface_target_uses_hwnd() {
        let target = QtCompositorTarget {
            surface_kind: QT_COMPOSITOR_SURFACE_WIN32_HWND,
            primary_handle: 1,
            secondary_handle: 0,
            width_px: 1,
            height_px: 1,
            scale_factor: 1.0,
        };

        let surface = unsafe { target.surface_target() }.expect("HWND target");
        let wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle,
            raw_window_handle,
        } = surface
        else {
            panic!("expected raw handle target");
        };
        assert!(matches!(raw_display_handle, RawDisplayHandle::Windows(_)));
        let RawWindowHandle::Win32(handle) = raw_window_handle else {
            panic!("expected Win32 raw window handle");
        };
        assert_eq!(handle.hwnd.get(), 1);
    }

    #[test]
    fn xcb_surface_target_requires_connection_and_window() {
        let target = QtCompositorTarget {
            surface_kind: QT_COMPOSITOR_SURFACE_XCB_WINDOW,
            primary_handle: 7,
            secondary_handle: 9,
            width_px: 1,
            height_px: 1,
            scale_factor: 1.0,
        };

        let surface = unsafe { target.surface_target() }.expect("XCB target");
        let wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle,
            raw_window_handle,
        } = surface
        else {
            panic!("expected raw handle target");
        };
        let RawDisplayHandle::Xcb(display) = raw_display_handle else {
            panic!("expected XCB display handle");
        };
        assert_eq!(display.screen, 0);
        assert_eq!(display.connection.expect("connection").as_ptr() as usize, 9);
        let RawWindowHandle::Xcb(window) = raw_window_handle else {
            panic!("expected XCB window handle");
        };
        assert_eq!(window.window.get(), 7);
    }

    #[test]
    fn wayland_surface_target_requires_surface_and_display() {
        let target = QtCompositorTarget {
            surface_kind: QT_COMPOSITOR_SURFACE_WAYLAND_SURFACE,
            primary_handle: 11,
            secondary_handle: 13,
            width_px: 1,
            height_px: 1,
            scale_factor: 1.0,
        };

        let surface = unsafe { target.surface_target() }.expect("Wayland target");
        let wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle,
            raw_window_handle,
        } = surface
        else {
            panic!("expected raw handle target");
        };
        let RawDisplayHandle::Wayland(display) = raw_display_handle else {
            panic!("expected Wayland display handle");
        };
        assert_eq!(display.display.as_ptr() as usize, 13);
        let RawWindowHandle::Wayland(window) = raw_window_handle else {
            panic!("expected Wayland window handle");
        };
        assert_eq!(window.surface.as_ptr() as usize, 11);
    }
}
