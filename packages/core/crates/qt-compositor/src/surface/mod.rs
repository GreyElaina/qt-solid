use std::{
    collections::{HashMap, HashSet},
    env,
    ffi::c_void,
    num::{NonZeroIsize, NonZeroU32},
    ptr::NonNull,
    sync::{Arc, Condvar, Mutex},
    thread,
};

use bytemuck::{Pod, Zeroable};
use once_cell::sync::Lazy;
use crate::compositor_core::Compositor;
use crate::types::{
    QtCompositorAffine, QtCompositorBaseUpload, QtCompositorError as QtWgpuRendererError,
    QtCompositorImageFormat, QtCompositorLayerSourceKind, QtCompositorLayerUpload,
    QtCompositorRect, QtCompositorSurfaceKey, QtCompositorTarget, QtCompositorUploadKind, Result,
    QT_COMPOSITOR_SURFACE_APPKIT_NS_VIEW, QT_COMPOSITOR_SURFACE_WAYLAND_SURFACE,
    QT_COMPOSITOR_SURFACE_WIN32_HWND, QT_COMPOSITOR_SURFACE_XCB_WINDOW,
};
use raw_window_handle::{
    AppKitDisplayHandle, AppKitWindowHandle, RawDisplayHandle, RawWindowHandle,
    WaylandDisplayHandle, WaylandWindowHandle, Win32WindowHandle, WindowsDisplayHandle,
    XcbDisplayHandle, XcbWindowHandle,
};
use wgpu::util::DeviceExt;

unsafe extern "C" {
    fn qt_solid_notify_window_compositor_present_complete(window_id: u32);
}

pub unsafe fn compositor_surface_target(
    target: QtCompositorTarget,
) -> Result<wgpu::SurfaceTargetUnsafe> {
    match target.surface_kind {
        QT_COMPOSITOR_SURFACE_APPKIT_NS_VIEW => {
            let Some(ns_view) = NonNull::new(target.primary_handle as *mut c_void) else {
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
            let Some(hwnd) = NonZeroIsize::new(target.primary_handle as isize) else {
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
            let Some(window) = NonZeroU32::new(target.primary_handle as u32) else {
                return Err(QtWgpuRendererError::new(
                    "qt compositor target is missing XCB window handle",
                ));
            };
            let Some(connection) = NonNull::new(target.secondary_handle as *mut c_void) else {
                return Err(QtWgpuRendererError::new(
                    "qt compositor target is missing XCB connection handle",
                ));
            };
            Ok(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: RawDisplayHandle::Xcb(XcbDisplayHandle::new(
                    Some(connection),
                    0,
                )),
                raw_window_handle: RawWindowHandle::Xcb(XcbWindowHandle::new(window)),
            })
        }
        QT_COMPOSITOR_SURFACE_WAYLAND_SURFACE => {
            let Some(surface) = NonNull::new(target.primary_handle as *mut c_void) else {
                return Err(QtWgpuRendererError::new(
                    "qt compositor target is missing Wayland surface handle",
                ));
            };
            let Some(display) = NonNull::new(target.secondary_handle as *mut c_void) else {
                return Err(QtWgpuRendererError::new(
                    "qt compositor target is missing Wayland display handle",
                ));
            };
            Ok(wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: RawDisplayHandle::Wayland(WaylandDisplayHandle::new(display)),
                raw_window_handle: RawWindowHandle::Wayland(WaylandWindowHandle::new(surface)),
            })
        }
        other => Err(QtWgpuRendererError::new(format!(
            "unsupported qt compositor surface kind {other}",
        ))),
    }
}

fn texture_format(format: QtCompositorImageFormat) -> wgpu::TextureFormat {
    match format {
        QtCompositorImageFormat::Bgra8UnormPremultiplied => wgpu::TextureFormat::Bgra8UnormSrgb,
        QtCompositorImageFormat::Rgba8UnormPremultiplied => wgpu::TextureFormat::Rgba8Unorm,
    }
}

fn bytes_per_pixel(format: QtCompositorImageFormat) -> usize {
    match format {
        QtCompositorImageFormat::Bgra8UnormPremultiplied
        | QtCompositorImageFormat::Rgba8UnormPremultiplied => 4,
    }
}

#[derive(Debug, Clone)]
struct OwnedQtCompositorBaseUpload {
    format: QtCompositorImageFormat,
    width_px: u32,
    height_px: u32,
    stride: usize,
    upload_kind: QtCompositorUploadKind,
    dirty_rects: Vec<QtCompositorRect>,
    bytes: Vec<u8>,
}

impl OwnedQtCompositorBaseUpload {
    fn from_borrowed(upload: &QtCompositorBaseUpload<'_>) -> Self {
        Self {
            format: upload.format,
            width_px: upload.width_px,
            height_px: upload.height_px,
            stride: upload.stride,
            upload_kind: upload.upload_kind,
            dirty_rects: upload.dirty_rects.to_vec(),
            bytes: upload.bytes.to_vec(),
        }
    }

    fn borrowed(&self) -> QtCompositorBaseUpload<'_> {
        QtCompositorBaseUpload {
            format: self.format,
            width_px: self.width_px,
            height_px: self.height_px,
            stride: self.stride,
            upload_kind: self.upload_kind,
            dirty_rects: self.dirty_rects.as_slice(),
            bytes: self.bytes.as_slice(),
        }
    }
}

#[derive(Debug, Clone)]
struct OwnedQtCompositorLayerUpload {
    node_id: u32,
    source_kind: QtCompositorLayerSourceKind,
    format: QtCompositorImageFormat,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    transform: QtCompositorAffine,
    opacity: f32,
    clip_rect: Option<QtCompositorRect>,
    width_px: u32,
    height_px: u32,
    stride: usize,
    upload_kind: QtCompositorUploadKind,
    dirty_rects: Vec<QtCompositorRect>,
    visible_rects: Vec<QtCompositorRect>,
    bytes: Vec<u8>,
}

impl OwnedQtCompositorLayerUpload {
    fn from_borrowed(upload: &QtCompositorLayerUpload<'_>) -> Self {
        Self {
            node_id: upload.node_id,
            source_kind: upload.source_kind,
            format: upload.format,
            x: upload.x,
            y: upload.y,
            width: upload.width,
            height: upload.height,
            transform: upload.transform,
            opacity: upload.opacity,
            clip_rect: upload.clip_rect,
            width_px: upload.width_px,
            height_px: upload.height_px,
            stride: upload.stride,
            upload_kind: upload.upload_kind,
            dirty_rects: upload.dirty_rects.to_vec(),
            visible_rects: upload.visible_rects.to_vec(),
            bytes: upload.bytes.to_vec(),
        }
    }

    fn borrowed(&self) -> QtCompositorLayerUpload<'_> {
        QtCompositorLayerUpload {
            node_id: self.node_id,
            source_kind: self.source_kind,
            format: self.format,
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
            transform: self.transform,
            opacity: self.opacity,
            clip_rect: self.clip_rect,
            width_px: self.width_px,
            height_px: self.height_px,
            stride: self.stride,
            upload_kind: self.upload_kind,
            dirty_rects: self.dirty_rects.as_slice(),
            visible_rects: self.visible_rects.as_slice(),
            bytes: self.bytes.as_slice(),
        }
    }
}

#[derive(Clone)]
struct AsyncPresentRequest {
    window_id: u32,
    context: WindowCompositorContextHandle,
    target: QtCompositorTarget,
    base: OwnedQtCompositorBaseUpload,
    layers: Vec<OwnedQtCompositorLayerUpload>,
}

impl AsyncPresentRequest {
    fn from_borrowed(
        window_id: u32,
        context: WindowCompositorContextHandle,
        target: QtCompositorTarget,
        base: &QtCompositorBaseUpload<'_>,
        layers: &[QtCompositorLayerUpload<'_>],
    ) -> Self {
        Self {
            window_id,
            context,
            target,
            base: OwnedQtCompositorBaseUpload::from_borrowed(base),
            layers: layers
                .iter()
                .map(OwnedQtCompositorLayerUpload::from_borrowed)
                .collect(),
        }
    }

    fn present(&self) -> Result<()> {
        let base = self.base.borrowed();
        let layers = self
            .layers
            .iter()
            .map(OwnedQtCompositorLayerUpload::borrowed)
            .collect::<Vec<_>>();
        let frame = prepare_compositor_frame_inner(&self.context, &base, &layers)?;
        frame.finish_to_surface()
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CompositeVertex {
    position: [f32; 2],
    uv: [f32; 2],
    opacity: f32,
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
                wgpu::VertexAttribute {
                    offset: (size_of::<[f32; 2]>() * 2) as u64,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32,
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

struct CachedPresentTexture {
    format: wgpu::TextureFormat,
    width_px: u32,
    height_px: u32,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
}

impl CachedPresentTexture {
    fn ensure_descriptor_matches(
        &self,
        format: wgpu::TextureFormat,
        width_px: u32,
        height_px: u32,
    ) -> bool {
        self.format == format && self.width_px == width_px && self.height_px == height_px
    }
}

const BASE_TEXTURE_USAGE: wgpu::TextureUsages =
    wgpu::TextureUsages::COPY_DST.union(wgpu::TextureUsages::TEXTURE_BINDING);

const LAYER_TEXTURE_USAGE: wgpu::TextureUsages = wgpu::TextureUsages::COPY_DST
    .union(wgpu::TextureUsages::TEXTURE_BINDING)
    .union(wgpu::TextureUsages::RENDER_ATTACHMENT);

const PRESENT_TEXTURE_USAGE: wgpu::TextureUsages = wgpu::TextureUsages::RENDER_ATTACHMENT
    .union(wgpu::TextureUsages::TEXTURE_BINDING)
    .union(wgpu::TextureUsages::COPY_SRC);

struct CompositorPipelineState {
    bind_group_layout: wgpu::BindGroupLayout,
    sampled_pipeline: wgpu::RenderPipeline,
    cached_texture_pipeline: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
}

#[derive(Debug, Clone, Copy)]
struct TextureUploadRect {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone)]
struct PreparedCompositorLayer {
    bind_group: wgpu::BindGroup,
    source_kind: QtCompositorLayerSourceKind,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    transform: QtCompositorAffine,
    opacity: f32,
    clip_rect: Option<QtCompositorRect>,
    visible_rects: Vec<QtCompositorRect>,
}

struct WindowCompositorSurfaceState {
    target: QtCompositorTarget,
    instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
}

#[derive(Default)]
struct WindowCompositorTextureState {
    base_texture: Option<CachedImageTexture>,
    layer_textures: HashMap<u32, CachedImageTexture>,
    present_texture: Option<CachedPresentTexture>,
}

struct WindowCompositorContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: CompositorPipelineState,
    surface_state: Mutex<WindowCompositorSurfaceState>,
    texture_state: Mutex<WindowCompositorTextureState>,
    adapter_pci_bus_id: Option<String>,
}

type WindowCompositorContextHandle = Arc<WindowCompositorContext>;

static WINDOW_COMPOSITORS: Lazy<
    Mutex<HashMap<QtCompositorSurfaceKey, WindowCompositorContextHandle>>,
> = Lazy::new(|| Mutex::new(HashMap::new()));
static WINDOW_COMPOSITOR_HANDLES: Lazy<
    Mutex<HashMap<QtCompositorSurfaceKey, Arc<dyn Compositor>>>,
> = Lazy::new(|| Mutex::new(HashMap::new()));
static ASYNC_PRESENT_QUEUE: Lazy<Arc<AsyncPresentQueue>> =
    Lazy::new(|| Arc::new(AsyncPresentQueue::new()));

struct SurfaceCompositorHandle {
    surface_key: QtCompositorSurfaceKey,
}

struct AsyncPresentQueue {
    state: Mutex<AsyncPresentQueueState>,
    ready: Condvar,
}

pub struct PreparedCompositorFrame {
    context: WindowCompositorContextHandle,
    target: QtCompositorTarget,
    queue: wgpu::Queue,
    encoder: Option<wgpu::CommandEncoder>,
    present_texture: wgpu::Texture,
    present_bind_group: wgpu::BindGroup,
}

impl PreparedCompositorFrame {
    pub fn target(&self) -> QtCompositorTarget {
        self.target
    }

    pub fn present_texture(&self) -> &wgpu::Texture {
        &self.present_texture
    }

    pub fn encoder_mut(&mut self) -> &mut wgpu::CommandEncoder {
        self.encoder
            .as_mut()
            .expect("prepared compositor frame encoder consumed once")
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.context.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn into_submission(mut self) -> (wgpu::Queue, wgpu::CommandBuffer) {
        let encoder = self
            .encoder
            .take()
            .expect("prepared compositor frame encoder consumed once");
        (self.queue, encoder.finish())
    }
}

#[derive(Default)]
struct AsyncPresentQueueState {
    worker_started: bool,
    pending: HashMap<QtCompositorSurfaceKey, AsyncPresentRequest>,
    in_flight: HashSet<QtCompositorSurfaceKey>,
}

impl AsyncPresentQueue {
    fn new() -> Self {
        Self {
            state: Mutex::new(AsyncPresentQueueState::default()),
            ready: Condvar::new(),
        }
    }

    fn enqueue(self: &Arc<Self>, request: AsyncPresentRequest) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .expect("qt wgpu async present queue mutex poisoned");
        if !state.worker_started {
            let queue = Arc::clone(self);
            thread::Builder::new()
                .name("qt-solid-wgpu-present".to_owned())
                .spawn(move || queue.run())
                .map_err(|error| QtWgpuRendererError::new(error.to_string()))?;
            state.worker_started = true;
        }
        state.pending.insert(request.target.surface_key(), request);
        self.ready.notify_one();
        Ok(())
    }

    fn is_busy(&self, key: QtCompositorSurfaceKey) -> bool {
        let state = self
            .state
            .lock()
            .expect("qt wgpu async present queue mutex poisoned");
        state.pending.contains_key(&key) || state.in_flight.contains(&key)
    }

    fn run(self: Arc<Self>) {
        loop {
            let requests = {
                let mut state = self
                    .state
                    .lock()
                    .expect("qt wgpu async present queue mutex poisoned");
                while state.pending.is_empty() {
                    state = self
                        .ready
                        .wait(state)
                        .expect("qt wgpu async present queue mutex poisoned");
                }
                let requests = state.pending.drain().collect::<Vec<_>>();
                for (key, _) in &requests {
                    state.in_flight.insert(*key);
                }
                requests
            };

            for (key, request) in requests {
                if let Err(error) = request.present() {
                    eprintln!("qt-wgpu async present failed: {error}");
                }
                {
                    let mut state = self
                        .state
                        .lock()
                        .expect("qt wgpu async present queue mutex poisoned");
                    state.in_flight.remove(&key);
                    if !state.pending.is_empty() {
                        self.ready.notify_one();
                    }
                }
                unsafe { qt_solid_notify_window_compositor_present_complete(request.window_id) };
            }
        }
    }
}

impl Compositor for SurfaceCompositorHandle {
    fn present_frame(
        &self,
        target: QtCompositorTarget,
        base: &QtCompositorBaseUpload<'_>,
        layers: &[QtCompositorLayerUpload<'_>],
        _window_id: Option<u32>,
    ) -> Result<bool> {
        crate::present_compositor_frame(target, base, layers).map(|()| true)
    }


    fn request_frame(
        &self,
        _target: QtCompositorTarget,
        _reason: crate::compositor_core::FrameReason,
    ) -> Result<bool> {
        Ok(false)
    }

    fn begin_drive(&self, _target: QtCompositorTarget) -> Result<()> {
        Ok(())
    }

    fn should_run_frame_source(&self) -> bool {
        false
    }

    fn is_busy(&self) -> bool {
        ASYNC_PRESENT_QUEUE.is_busy(self.surface_key)
    }

    fn is_initialized(&self, target: QtCompositorTarget) -> bool {
        crate::compositor_frame_is_initialized(target)
    }

    fn layer_handle(&self, _target: QtCompositorTarget) -> Result<u64> {
        Ok(0)
    }

    fn note_drawable(&self, _target: QtCompositorTarget, _drawable_handle: u64) -> Result<()> {
        Ok(())
    }

    fn release_drawable(&self, _drawable_handle: u64) {}
}

pub fn load_or_create_compositor(target: QtCompositorTarget) -> Result<Arc<dyn Compositor>> {
    let key = target.surface_key();
    let mut handles = WINDOW_COMPOSITOR_HANDLES
        .lock()
        .expect("qt wgpu compositor handle registry mutex poisoned");
    if let Some(existing) = handles.get(&key) {
        return Ok(Arc::clone(existing));
    }
    let compositor: Arc<dyn Compositor> = Arc::new(SurfaceCompositorHandle { surface_key: key });
    handles.insert(key, Arc::clone(&compositor));
    Ok(compositor)
}

const COMPOSITOR_SHADER: &str = include_str!("../../shader/compositor.wgsl");
const COMPOSITOR_CACHED_TEXTURE_SHADER: &str = include_str!("../../shader/cached_texture.wgsl");

pub fn present_compositor_frame(
    target: QtCompositorTarget,
    base: &QtCompositorBaseUpload<'_>,
    layers: &[QtCompositorLayerUpload<'_>],
) -> Result<()> {
    if target.width_px == 0 || target.height_px == 0 {
        return Ok(());
    }

    let frame = prepare_compositor_frame(target, base, layers)?;
    frame.finish_to_surface()
}


pub fn prepare_compositor_frame(
    target: QtCompositorTarget,
    base: &QtCompositorBaseUpload<'_>,
    layers: &[QtCompositorLayerUpload<'_>],
) -> Result<PreparedCompositorFrame> {
    if target.width_px == 0 || target.height_px == 0 {
        return Err(QtWgpuRendererError::new(
            "qt compositor cannot prepare zero-sized frame",
        ));
    }

    let context_handle = load_or_create_window_compositor(target)?;
    prepare_compositor_frame_inner(&context_handle, base, layers)
}

pub fn present_compositor_frame_async(
    window_id: u32,
    target: QtCompositorTarget,
    base: &QtCompositorBaseUpload<'_>,
    layers: &[QtCompositorLayerUpload<'_>],
) -> Result<()> {
    if target.width_px == 0 || target.height_px == 0 {
        return Ok(());
    }

    let context_handle = load_or_create_window_compositor(target)?;
    let request =
        AsyncPresentRequest::from_borrowed(window_id, context_handle, target, base, layers);
    ASYNC_PRESENT_QUEUE.enqueue(request)
}

pub fn compositor_frame_is_busy(target: QtCompositorTarget) -> bool {
    ASYNC_PRESENT_QUEUE.is_busy(target.surface_key())
}

pub fn compositor_frame_is_busy_for_key(key: QtCompositorSurfaceKey) -> bool {
    ASYNC_PRESENT_QUEUE.is_busy(key)
}

pub fn compositor_frame_is_initialized(target: QtCompositorTarget) -> bool {
    let compositors = WINDOW_COMPOSITORS
        .lock()
        .expect("qt wgpu compositor registry mutex poisoned");
    let Some(context) = compositors.get(&target.surface_key()) else {
        return false;
    };
    let texture_state = context
        .texture_state
        .lock()
        .expect("qt wgpu compositor texture mutex poisoned");
    texture_state.base_texture.is_some()
}

pub fn with_window_compositor_device_queue<T, F>(target: QtCompositorTarget, run: F) -> Result<T>
where
    F: FnOnce(&wgpu::Device, &wgpu::Queue) -> Result<T>,
{
    let context_handle = load_or_create_window_compositor(target)?;
    run(&context_handle.device, &context_handle.queue)
}

pub fn window_compositor_adapter_pci_bus_id(
    target: QtCompositorTarget,
) -> Option<String> {
    let context_handle = load_or_create_window_compositor(target).ok()?;
    context_handle.adapter_pci_bus_id.clone()
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
    F: FnOnce(&wgpu::Device, &wgpu::Queue, &wgpu::TextureView) -> Result<T>,
{
    let context_handle = load_or_create_window_compositor(target)?;
    let mut texture_state = context_handle
        .texture_state
        .lock()
        .expect("qt wgpu compositor texture mutex poisoned");
    let needs_recreate = texture_state
        .layer_textures
        .get(&node_id)
        .map(|entry| !entry.ensure_descriptor_matches(format, width_px, height_px))
        .unwrap_or(true);
    if needs_recreate {
        let next_entry = CachedImageTexture::new(
            &context_handle.device,
            &context_handle.pipeline,
            format,
            width_px,
            height_px,
            LAYER_TEXTURE_USAGE,
            &format!("qt-solid-wgpu-layer-{node_id}"),
        );
        texture_state.layer_textures.insert(node_id, next_entry);
    }
    let entry = texture_state.layer_textures.get(&node_id).ok_or_else(|| {
        QtWgpuRendererError::new(format!(
            "qt compositor cached layer {} could not be allocated",
            node_id
        ))
    })?;
    run(&context_handle.device, &context_handle.queue, &entry.view)
}

pub fn with_window_compositor_layer_texture_handle<T, F>(
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
    let mut texture_state = context_handle
        .texture_state
        .lock()
        .expect("qt wgpu compositor texture mutex poisoned");
    let needs_recreate = texture_state
        .layer_textures
        .get(&node_id)
        .map(|entry| !entry.ensure_descriptor_matches(format, width_px, height_px))
        .unwrap_or(true);
    if needs_recreate {
        let next_entry = CachedImageTexture::new(
            &context_handle.device,
            &context_handle.pipeline,
            format,
            width_px,
            height_px,
            LAYER_TEXTURE_USAGE,
            &format!("qt-solid-wgpu-layer-{node_id}"),
        );
        texture_state.layer_textures.insert(node_id, next_entry);
    }
    let entry = texture_state.layer_textures.get(&node_id).ok_or_else(|| {
        QtWgpuRendererError::new(format!(
            "qt compositor cached layer {} could not be allocated",
            node_id
        ))
    })?;
    run(
        &context_handle.device,
        &context_handle.queue,
        &entry.texture,
        &entry.view,
    )
}

pub fn evict_layer_textures(target: QtCompositorTarget, keys: &[u32]) {
    let Ok(context_handle) = load_or_create_window_compositor(target) else {
        return;
    };
    let mut texture_state = context_handle
        .texture_state
        .lock()
        .expect("qt wgpu compositor texture mutex poisoned");
    for &key in keys {
        texture_state.layer_textures.remove(&key);
    }
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

    let compositor = Arc::new(WindowCompositorContext::new(target)?);
    compositors.insert(key, Arc::clone(&compositor));
    Ok(compositor)
}

pub fn destroy_window_compositor(target: QtCompositorTarget) {
    let key = target.surface_key();
    {
        let mut compositors = WINDOW_COMPOSITORS
            .lock()
            .expect("qt wgpu compositor registry mutex poisoned");
        compositors.remove(&key);
    }
    {
        let mut handles = WINDOW_COMPOSITOR_HANDLES
            .lock()
            .expect("qt wgpu compositor handles mutex poisoned");
        handles.remove(&key);
    }
}

fn preferred_surface_format(
    formats: &[wgpu::TextureFormat],
    default_format: wgpu::TextureFormat,
) -> wgpu::TextureFormat {
    if default_format.is_srgb() {
        return default_format;
    }

    let srgb_variant = default_format.add_srgb_suffix();
    if formats.contains(&srgb_variant) {
        srgb_variant
    } else {
        default_format
    }
}

fn preferred_present_mode(
    present_modes: &[wgpu::PresentMode],
    default_present_mode: wgpu::PresentMode,
) -> wgpu::PresentMode {
    preferred_present_mode_with_experiment(
        present_modes,
        default_present_mode,
        should_prefer_immediate_present_mode(),
    )
}

fn preferred_present_mode_with_experiment(
    present_modes: &[wgpu::PresentMode],
    default_present_mode: wgpu::PresentMode,
    prefer_immediate: bool,
) -> wgpu::PresentMode {
    if prefer_immediate && present_modes.contains(&wgpu::PresentMode::Immediate) {
        wgpu::PresentMode::Immediate
    } else if present_modes.contains(&wgpu::PresentMode::Mailbox) {
        wgpu::PresentMode::Mailbox
    } else if present_modes.contains(&wgpu::PresentMode::Fifo) {
        wgpu::PresentMode::Fifo
    } else {
        default_present_mode
    }
}

fn should_prefer_immediate_present_mode() -> bool {
    cfg!(target_os = "macos")
        && env::var_os("QT_SOLID_WGPU_PRESENT_IMMEDIATE").is_some_and(|value| value == "1")
}

impl WindowCompositorContext {
    fn new(target: QtCompositorTarget) -> Result<Self> {
        let instance = wgpu::Instance::default();
        let surface = unsafe {
            instance
                .create_surface_unsafe(compositor_surface_target(target)?)
                .map_err(|error| QtWgpuRendererError::new(error.to_string()))?
        };
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
        }))
        .map_err(|error| QtWgpuRendererError::new(error.to_string()))?;
        let adapter_pci_bus_id = {
            let id = adapter.get_info().device_pci_bus_id;
            if id.is_empty() { None } else { Some(id) }
        };
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("qt-solid-wgpu-compositor-device"),
            ..Default::default()
        }))
        .map_err(|error| QtWgpuRendererError::new(error.to_string()))?;
        let capabilities = surface.get_capabilities(&adapter);
        let config = surface
            .get_default_config(&adapter, target.width_px, target.height_px)
            .ok_or_else(|| QtWgpuRendererError::new("failed to derive wgpu surface configuration"))
            .map(|mut config| {
                config.format = preferred_surface_format(&capabilities.formats, config.format);
                config.present_mode =
                    preferred_present_mode(&capabilities.present_modes, config.present_mode);
                config.desired_maximum_frame_latency = 2;
                config.alpha_mode = wgpu::CompositeAlphaMode::Auto;
                config
            })?;
        surface.configure(&device, &config);
        let pipeline = CompositorPipelineState::new(&device, config.format);

        Ok(Self {
            device,
            queue,
            pipeline,
            surface_state: Mutex::new(WindowCompositorSurfaceState {
                target,
                instance,
                surface,
                config,
            }),
            texture_state: Mutex::new(WindowCompositorTextureState::default()),
            adapter_pci_bus_id,
        })
    }

    fn prepare_frame(
        &self,
        context_handle: &WindowCompositorContextHandle,
        base: &QtCompositorBaseUpload<'_>,
        layers: &[QtCompositorLayerUpload<'_>],
    ) -> Result<PreparedCompositorFrame> {
        let base_bind_group = {
            let mut texture_state = self
                .texture_state
                .lock()
                .expect("qt wgpu compositor texture mutex poisoned");
            if base.upload_kind != QtCompositorUploadKind::None {
                upload_image(
                    &self.device,
                    &self.queue,
                    &self.pipeline,
                    &mut texture_state.base_texture,
                    base.format,
                    base.width_px,
                    base.height_px,
                    base.stride,
                    base.upload_kind,
                    base.dirty_rects,
                    base.bytes,
                    "qt-solid-wgpu-base-texture",
                )?;
            } else if texture_state.base_texture.is_none() {
                return Err(QtWgpuRendererError::new(
                    "qt compositor cannot reuse base texture before first upload",
                ));
            }
            texture_state
                .base_texture
                .as_ref()
                .map(|texture| texture.bind_group.clone())
        };

        let prepared_layers = self.prepare_layer_draws(layers)?;

        let (target, surface_format) = {
            let mut surface_state = self
                .surface_state
                .lock()
                .expect("qt wgpu compositor surface mutex poisoned");
            if base.width_px != 0
                && base.height_px != 0
                && (surface_state.config.width != base.width_px
                    || surface_state.config.height != base.height_px)
            {
                surface_state.config.width = base.width_px;
                surface_state.config.height = base.height_px;
                surface_state
                    .surface
                    .configure(&self.device, &surface_state.config);
                surface_state.target.width_px = base.width_px;
                surface_state.target.height_px = base.height_px;
            }
            (surface_state.target, surface_state.config.format)
        };
        let (present_texture, present_bind_group, present_view) = {
            let mut texture_state = self
                .texture_state
                .lock()
                .expect("qt wgpu compositor texture mutex poisoned");
            let needs_recreate = texture_state
                .present_texture
                .as_ref()
                .map(|texture| {
                    !texture.ensure_descriptor_matches(
                        surface_format,
                        target.width_px.max(1),
                        target.height_px.max(1),
                    )
                })
                .unwrap_or(true);
            if needs_recreate {
                texture_state.present_texture = Some(CachedPresentTexture::new(
                    &self.device,
                    &self.pipeline,
                    surface_format,
                    target.width_px.max(1),
                    target.height_px.max(1),
                    "qt-solid-wgpu-present-texture",
                ));
            }
            let present_texture = texture_state
                .present_texture
                .as_ref()
                .expect("present texture inserted before use");
            (
                present_texture.texture.clone(),
                present_texture.bind_group.clone(),
                present_texture.view.clone(),
            )
        };
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("qt-solid-wgpu-compositor-encoder"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("qt-solid-wgpu-compositor-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &present_view,
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
            pass.set_pipeline(&self.pipeline.sampled_pipeline);

            if let Some(base_bind_group) = base_bind_group.as_ref() {
                draw_quad(
                    &self.device,
                    &mut pass,
                    base_bind_group,
                    target.width_px,
                    target.height_px,
                    QtCompositorAffine::IDENTITY,
                    1.0,
                    0,
                    0,
                    target.width_px as i32,
                    target.height_px as i32,
                    0.0,
                    0.0,
                    1.0,
                    1.0,
                );
            }

            for layer in &prepared_layers {
                if layer.source_kind == QtCompositorLayerSourceKind::CachedTexture {
                    pass.set_pipeline(&self.pipeline.cached_texture_pipeline);
                } else {
                    pass.set_pipeline(&self.pipeline.sampled_pipeline);
                }
                for visible_rect in &layer.visible_rects {
                    if visible_rect.width <= 0 || visible_rect.height <= 0 {
                        continue;
                    }
                    if let Some(clip_rect) = layer.clip_rect {
                        pass.set_scissor_rect(
                            clip_rect.x.max(0) as u32,
                            clip_rect.y.max(0) as u32,
                            clip_rect.width.max(0) as u32,
                            clip_rect.height.max(0) as u32,
                        );
                    } else {
                        pass.set_scissor_rect(0, 0, target.width_px.max(1), target.height_px.max(1));
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
                        &layer.bind_group,
                        target.width_px,
                        target.height_px,
                        layer.transform,
                        layer.opacity,
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

        Ok(PreparedCompositorFrame {
            context: Arc::clone(context_handle),
            target,
            queue: self.queue.clone(),
            encoder: Some(encoder),
            present_texture,
            present_bind_group,
        })
    }

    fn prepare_layer_draws(
        &self,
        layers: &[QtCompositorLayerUpload<'_>],
    ) -> Result<Vec<PreparedCompositorLayer>> {
        let mut texture_state = self
            .texture_state
            .lock()
            .expect("qt wgpu compositor texture mutex poisoned");
        let mut prepared_layers = Vec::with_capacity(layers.len());
        for layer in layers {
            let cached = upload_layer_texture(self, &mut texture_state, layer)?;
            prepared_layers.push(PreparedCompositorLayer {
                bind_group: cached.bind_group.clone(),
                source_kind: layer.source_kind,
                x: layer.x,
                y: layer.y,
                width: layer.width,
                height: layer.height,
                transform: layer.transform,
                opacity: layer.opacity,
                clip_rect: layer.clip_rect,
                visible_rects: layer.visible_rects.to_vec(),
            });
        }
        Ok(prepared_layers)
    }
}

fn prepare_compositor_frame_inner(
    context_handle: &WindowCompositorContextHandle,
    base: &QtCompositorBaseUpload<'_>,
    layers: &[QtCompositorLayerUpload<'_>],
) -> Result<PreparedCompositorFrame> {
    context_handle.prepare_frame(context_handle, base, layers)
}

impl PreparedCompositorFrame {
    pub fn finish_to_surface(mut self) -> Result<()> {
        let target = self.target;
        let context = Arc::clone(&self.context);
        let mut encoder = self
            .encoder
            .take()
            .expect("prepared compositor frame encoder consumed once");

    let mut surface_state = context
        .surface_state
        .lock()
        .expect("qt wgpu compositor surface mutex poisoned");
    let surface_texture = {
        let mut acquired = None;
        for attempt in 0..2 {
            match surface_state.surface.get_current_texture() {
                Ok(texture) => {
                    acquired = Some(texture);
                    break;
                }
                Err(wgpu::SurfaceError::Timeout) | Err(wgpu::SurfaceError::Other) => {
                    return Err(QtWgpuRendererError::new(
                        "qt wgpu compositor surface is currently unavailable",
                    ));
                }
                Err(wgpu::SurfaceError::Outdated) => {
                    surface_state
                        .surface
                        .configure(&context.device, &surface_state.config);
                }
                Err(wgpu::SurfaceError::Lost) => {
                    surface_state.surface = unsafe {
                        surface_state
                            .instance
                            .create_surface_unsafe(compositor_surface_target(surface_state.target)?)
                            .map_err(|error| QtWgpuRendererError::new(error.to_string()))?
                    };
                    surface_state
                        .surface
                        .configure(&context.device, &surface_state.config);
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
        acquired.ok_or_else(|| {
            QtWgpuRendererError::new(
                "failed to acquire current surface texture for qt wgpu compositor",
            )
        })?
    };
    let surface_view = surface_texture
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("qt-solid-wgpu-present-pass"),
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
        pass.set_pipeline(&context.pipeline.sampled_pipeline);
        draw_quad(
            &context.device,
            &mut pass,
            &self.present_bind_group,
            target.width_px,
            target.height_px,
            QtCompositorAffine::IDENTITY,
            1.0,
            0,
            0,
            target.width_px as i32,
            target.height_px as i32,
            0.0,
            0.0,
            1.0,
            1.0,
        );
    }

    self.queue.submit([encoder.finish()]);
    surface_texture.present();
    Ok(())
    }
}

fn upload_layer_texture<'a>(
    context: &'a WindowCompositorContext,
    texture_state: &'a mut WindowCompositorTextureState,
    layer: &QtCompositorLayerUpload<'_>,
) -> Result<&'a CachedImageTexture> {
    if layer.source_kind == QtCompositorLayerSourceKind::CachedTexture {
        let entry = texture_state
            .layer_textures
            .get(&layer.node_id)
            .ok_or_else(|| {
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
    let entry = texture_state
        .layer_textures
        .entry(layer.node_id)
        .or_insert_with(|| {
            CachedImageTexture::new(
                &context.device,
                &context.pipeline,
                layer.format,
                layer.width_px,
                layer.height_px,
                LAYER_TEXTURE_USAGE,
                &format!("qt-solid-wgpu-layer-{}", layer.node_id),
            )
        });
    if !entry.ensure_descriptor_matches(layer.format, layer.width_px, layer.height_px) {
        *entry = CachedImageTexture::new(
            &context.device,
            &context.pipeline,
            layer.format,
            layer.width_px,
            layer.height_px,
            LAYER_TEXTURE_USAGE,
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
        *slot = Some(CachedImageTexture::new(
            device,
            pipeline,
            format,
            width_px,
            height_px,
            BASE_TEXTURE_USAGE,
            label,
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

impl CachedImageTexture {
    fn new(
        device: &wgpu::Device,
        pipeline: &CompositorPipelineState,
        format: QtCompositorImageFormat,
        width_px: u32,
        height_px: u32,
        usage: wgpu::TextureUsages,
        label: &str,
    ) -> Self {
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
        format: texture_format(format),
        usage,
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

    Self {
        format,
        width_px,
        height_px,
        texture,
        view,
        bind_group,
    }
    }
}

impl CachedPresentTexture {
    fn new(
        device: &wgpu::Device,
        pipeline: &CompositorPipelineState,
        format: wgpu::TextureFormat,
        width_px: u32,
        height_px: u32,
        label: &str,
    ) -> Self {
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
        format,
        usage: PRESENT_TEXTURE_USAGE,
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

    Self {
        format,
        width_px,
        height_px,
        texture,
        view,
        bind_group,
    }
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
    let bytes_per_pixel = bytes_per_pixel(format);
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
    let bytes_per_pixel = bytes_per_pixel(format);
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

impl CompositorPipelineState {
    fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
    let sampled_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("qt-solid-wgpu-compositor-shader"),
        source: wgpu::ShaderSource::Wgsl(COMPOSITOR_SHADER.into()),
    });
    let cached_texture_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("qt-solid-wgpu-compositor-cached-texture-shader"),
        source: wgpu::ShaderSource::Wgsl(COMPOSITOR_CACHED_TEXTURE_SHADER.into()),
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
    let create_pipeline = |label: &str, shader: &wgpu::ShaderModule| -> wgpu::RenderPipeline {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: Some("vs_main"),
                buffers: &[CompositeVertex::layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
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
        })
    };
    let sampled_pipeline = create_pipeline("qt-solid-wgpu-compositor-pipeline", &sampled_shader);
    let cached_texture_pipeline = create_pipeline(
        "qt-solid-wgpu-compositor-cached-texture-pipeline",
        &cached_texture_shader,
    );
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("qt-solid-wgpu-compositor-sampler"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    });

    Self {
        bind_group_layout,
        sampled_pipeline,
        cached_texture_pipeline,
        sampler,
    }
    }
}

fn draw_quad(
    device: &wgpu::Device,
    pass: &mut wgpu::RenderPass<'_>,
    bind_group: &wgpu::BindGroup,
    surface_width_px: u32,
    surface_height_px: u32,
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
) {
    if width <= 0
        || height <= 0
        || surface_width_px == 0
        || surface_height_px == 0
        || opacity <= 0.0
    {
        return;
    }

    let map_position = |local_x: f64, local_y: f64| {
        let (dx, dy) = transform.map_point(local_x, local_y);
        let x = x as f64 + dx;
        let y = y as f64 + dy;
        [
            ((x as f32 / surface_width_px as f32) * 2.0) - 1.0,
            1.0 - ((y as f32 / surface_height_px as f32) * 2.0),
        ]
    };
    let top_left = map_position(0.0, 0.0);
    let top_right = map_position(width as f64, 0.0);
    let bottom_left = map_position(0.0, height as f64);
    let bottom_right = map_position(width as f64, height as f64);
    let vertices = [
        CompositeVertex {
            position: top_left,
            uv: [u0, v0],
            opacity,
        },
        CompositeVertex {
            position: top_right,
            uv: [u1, v0],
            opacity,
        },
        CompositeVertex {
            position: bottom_left,
            uv: [u0, v1],
            opacity,
        },
        CompositeVertex {
            position: bottom_left,
            uv: [u0, v1],
            opacity,
        },
        CompositeVertex {
            position: top_right,
            uv: [u1, v0],
            opacity,
        },
        CompositeVertex {
            position: bottom_right,
            uv: [u1, v1],
            opacity,
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

        let error = match unsafe { compositor_surface_target(target) } {
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

        let surface = unsafe { compositor_surface_target(target) }.expect("HWND target");
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

        let surface = unsafe { compositor_surface_target(target) }.expect("XCB target");
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

        let surface = unsafe { compositor_surface_target(target) }.expect("Wayland target");
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

    #[test]
    fn layer_texture_usage_excludes_storage_binding() {
        assert!(!LAYER_TEXTURE_USAGE.contains(wgpu::TextureUsages::STORAGE_BINDING));
        assert!(!BASE_TEXTURE_USAGE.contains(wgpu::TextureUsages::STORAGE_BINDING));
    }

    #[test]
    fn layer_texture_usage_includes_render_attachment() {
        assert!(LAYER_TEXTURE_USAGE.contains(wgpu::TextureUsages::RENDER_ATTACHMENT));
    }

    #[test]
    fn present_texture_usage_includes_copy_src() {
        assert!(PRESENT_TEXTURE_USAGE.contains(wgpu::TextureUsages::COPY_SRC));
    }

    #[test]
    fn bgra_backingstore_sampling_uses_srgb() {
        assert_eq!(
            texture_format(QtCompositorImageFormat::Bgra8UnormPremultiplied),
            wgpu::TextureFormat::Bgra8UnormSrgb
        );
        assert_eq!(
            texture_format(QtCompositorImageFormat::Rgba8UnormPremultiplied),
            wgpu::TextureFormat::Rgba8Unorm
        );
    }

    #[test]
    fn preferred_surface_format_upgrades_to_srgb_when_available() {
        let formats = [
            wgpu::TextureFormat::Bgra8Unorm,
            wgpu::TextureFormat::Bgra8UnormSrgb,
        ];
        assert_eq!(
            preferred_surface_format(&formats, wgpu::TextureFormat::Bgra8Unorm),
            wgpu::TextureFormat::Bgra8UnormSrgb
        );
        assert_eq!(
            preferred_surface_format(&formats, wgpu::TextureFormat::Bgra8UnormSrgb),
            wgpu::TextureFormat::Bgra8UnormSrgb
        );
    }

    #[test]
    fn preferred_present_mode_prefers_mailbox() {
        let present_modes = [wgpu::PresentMode::Fifo, wgpu::PresentMode::Mailbox];
        assert_eq!(
            preferred_present_mode(&present_modes, wgpu::PresentMode::Fifo),
            wgpu::PresentMode::Mailbox
        );
    }

    #[test]
    fn preferred_present_mode_falls_back_to_fifo() {
        let present_modes = [wgpu::PresentMode::Fifo, wgpu::PresentMode::Immediate];
        assert_eq!(
            preferred_present_mode(&present_modes, wgpu::PresentMode::Immediate),
            wgpu::PresentMode::Fifo
        );
    }

    #[test]
    fn preferred_present_mode_can_prefer_immediate_for_experiment() {
        let present_modes = [wgpu::PresentMode::Fifo, wgpu::PresentMode::Immediate];
        assert_eq!(
            preferred_present_mode_with_experiment(&present_modes, wgpu::PresentMode::Fifo, true,),
            wgpu::PresentMode::Immediate
        );
    }
}
