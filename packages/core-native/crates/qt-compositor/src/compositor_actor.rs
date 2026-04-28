use std::{
    collections::{HashMap, VecDeque},
    fmt,
    sync::{Arc, LazyLock, Mutex, MutexGuard},
};

use crate::compositor_core::{Compositor, CompositorOwner, FrameReason};
use crate::types::{
    QtCompositorBaseUpload, QtCompositorError, QtCompositorLayerUpload, QtCompositorSurfaceKey,
    QtCompositorTarget, Result,
};

// ---------------------------------------------------------------------------
// Frame signal source trait (E1 integration point)
// ---------------------------------------------------------------------------

/// Trait that E1 display-link implementations will satisfy.
/// `()` serves as the "no frame signal" stub.
pub trait FrameSignalSource: Send {
    fn start(&mut self);
    fn stop(&mut self);
}

impl FrameSignalSource for () {
    fn start(&mut self) {}
    fn stop(&mut self) {}
}

// ---------------------------------------------------------------------------
// Actor state
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub(crate) struct ConfiguredTargetState {
    pub(crate) width_px: u32,
    pub(crate) height_px: u32,
    pub(crate) format: wgpu::TextureFormat,
}

pub(crate) struct ActorState {
    pub(crate) configured_target: Option<ConfiguredTargetState>,
    pub(crate) requested: bool,
    pub(crate) in_flight: bool,
    pub(crate) base_initialized: bool,
    pub(crate) epoch: u64,
}

impl Default for ActorState {
    fn default() -> Self {
        Self {
            configured_target: None,
            requested: false,
            in_flight: false,
            base_initialized: false,
            epoch: 0,
        }
    }
}

impl ActorState {
    fn should_run_frame_source(&self) -> bool {
        self.requested || self.in_flight
    }

    fn is_initialized(&self, target: QtCompositorTarget) -> bool {
        self.configured_target
            .is_some_and(|c| c.width_px == target.width_px && c.height_px == target.height_px && self.base_initialized)
    }
}

// ---------------------------------------------------------------------------
// Actor messages
// ---------------------------------------------------------------------------

enum ActorMessage {
    PresentFrame {
        target: QtCompositorTarget,
        base: OwnedBase,
        layers: Vec<OwnedLayer>,
        window_id: Option<u32>,
    },
    RequestFrame {
        target: QtCompositorTarget,
        reason: FrameReason,
    },
    BeginDrive {
        target: QtCompositorTarget,
    },
    ShouldRunFrameSource,
    IsBusy,
    IsInitialized {
        target: QtCompositorTarget,
    },
}

enum ActorReply {
    Unit,
    Bool(bool),
}

// ---------------------------------------------------------------------------
// Owned upload types (clone from borrowed lifetime)
// ---------------------------------------------------------------------------

struct OwnedBase {
    format: crate::types::QtCompositorImageFormat,
    width_px: u32,
    height_px: u32,
    stride: usize,
    upload_kind: crate::types::QtCompositorUploadKind,
    dirty_rects: Vec<crate::types::QtCompositorRect>,
    bytes: Vec<u8>,
}

impl OwnedBase {
    fn from_borrowed(u: &QtCompositorBaseUpload<'_>) -> Self {
        Self {
            format: u.format,
            width_px: u.width_px,
            height_px: u.height_px,
            stride: u.stride,
            upload_kind: u.upload_kind,
            dirty_rects: u.dirty_rects.to_vec(),
            bytes: owned_upload_bytes(u.upload_kind, u.bytes),
        }
    }

    fn borrowed(&self) -> QtCompositorBaseUpload<'_> {
        QtCompositorBaseUpload {
            format: self.format,
            width_px: self.width_px,
            height_px: self.height_px,
            stride: self.stride,
            upload_kind: self.upload_kind,
            dirty_rects: &self.dirty_rects,
            bytes: &self.bytes,
        }
    }
}

struct OwnedLayer {
    node_id: u32,
    source_kind: crate::types::QtCompositorLayerSourceKind,
    format: crate::types::QtCompositorImageFormat,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    transform: crate::types::QtCompositorAffine,
    opacity: f32,
    clip_rect: Option<crate::types::QtCompositorRect>,
    width_px: u32,
    height_px: u32,
    stride: usize,
    upload_kind: crate::types::QtCompositorUploadKind,
    dirty_rects: Vec<crate::types::QtCompositorRect>,
    visible_rects: Vec<crate::types::QtCompositorRect>,
    bytes: Vec<u8>,
}

impl OwnedLayer {
    fn from_borrowed(u: &QtCompositorLayerUpload<'_>) -> Self {
        Self {
            node_id: u.node_id,
            source_kind: u.source_kind,
            format: u.format,
            x: u.x,
            y: u.y,
            width: u.width,
            height: u.height,
            transform: u.transform,
            opacity: u.opacity,
            clip_rect: u.clip_rect,
            width_px: u.width_px,
            height_px: u.height_px,
            stride: u.stride,
            upload_kind: u.upload_kind,
            dirty_rects: u.dirty_rects.to_vec(),
            visible_rects: u.visible_rects.to_vec(),
            bytes: owned_upload_bytes(u.upload_kind, u.bytes),
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
            dirty_rects: &self.dirty_rects,
            visible_rects: &self.visible_rects,
            bytes: &self.bytes,
        }
    }
}

fn owned_upload_bytes(kind: crate::types::QtCompositorUploadKind, bytes: &[u8]) -> Vec<u8> {
    if matches!(kind, crate::types::QtCompositorUploadKind::None) || bytes.is_empty() {
        Vec::new()
    } else {
        bytes.to_vec()
    }
}

// ---------------------------------------------------------------------------
// Trace helper (mirrors macos/trace.rs)
// ---------------------------------------------------------------------------

fn trace_enabled() -> bool {
    static ENABLED: LazyLock<bool> =
        LazyLock::new(|| std::env::var_os("QT_SOLID_WGPU_TRACE").is_some());
    *ENABLED
}

fn trace(tag: &str, args: fmt::Arguments<'_>) {
    if trace_enabled() {
        println!("[qt-{tag}] {args}");
    }
}

// ---------------------------------------------------------------------------
// CompositorActor<F>
// ---------------------------------------------------------------------------

pub(crate) struct CompositorActor<F: FrameSignalSource = ()> {
    tag: &'static str,
    owner: Arc<dyn CompositorOwner>,
    state: ActorState,
    frame_signal: F,
}

impl<F: FrameSignalSource> CompositorActor<F> {
    fn handle_message(&mut self, message: ActorMessage) -> Result<ActorReply> {
        match message {
            ActorMessage::PresentFrame {
                target,
                base,
                layers,
                window_id,
            } => self.present_frame(target, base, layers, window_id).map(ActorReply::Bool),
            ActorMessage::RequestFrame { target, reason } => {
                self.request_frame(target, reason).map(ActorReply::Bool)
            }
            ActorMessage::BeginDrive { target } => {
                self.begin_drive(target);
                Ok(ActorReply::Unit)
            }
            ActorMessage::ShouldRunFrameSource => {
                Ok(ActorReply::Bool(self.state.should_run_frame_source()))
            }
            ActorMessage::IsBusy => Ok(ActorReply::Bool(self.state.in_flight)),
            ActorMessage::IsInitialized { target } => {
                Ok(ActorReply::Bool(self.state.is_initialized(target)))
            }
        }
    }

    fn request_frame(&mut self, target: QtCompositorTarget, reason: FrameReason) -> Result<bool> {
        self.state.requested = true;
        let should_run = self.state.should_run_frame_source();
        trace(
            self.tag,
            format_args!(
                "request target={}x{} reason={reason:?} run={should_run}",
                target.width_px, target.height_px,
            ),
        );
        if should_run {
            self.frame_signal.start();
        }
        self.owner.request_wake();
        Ok(should_run)
    }

    fn begin_drive(&mut self, target: QtCompositorTarget) {
        self.state.requested = false;
        trace(
            self.tag,
            format_args!("begin-drive target={}x{}", target.width_px, target.height_px),
        );
        if !self.state.should_run_frame_source() {
            self.frame_signal.stop();
        }
    }

    fn present_frame(
        &mut self,
        target: QtCompositorTarget,
        base: OwnedBase,
        layers: Vec<OwnedLayer>,
        window_id: Option<u32>,
    ) -> Result<bool> {
        trace(
            self.tag,
            format_args!("present target={}x{}", target.width_px, target.height_px),
        );
        self.state.in_flight = true;
        let base_ref = base.borrowed();
        let layers_ref: Vec<_> = layers.iter().map(OwnedLayer::borrowed).collect();
        let result = crate::surface::present_compositor_frame(target, &base_ref, &layers_ref);
        self.state.in_flight = false;
        self.state.base_initialized = true;
        result?;
        trace(
            self.tag,
            format_args!(
                "present-complete target={}x{} window={window_id:?}",
                target.width_px, target.height_px,
            ),
        );
        if let Some(wid) = window_id {
            self.owner.present_complete(wid);
        }
        Ok(true)
    }
}

// ---------------------------------------------------------------------------
// CompositorHandle<F> — Compositor trait impl via mailbox
// ---------------------------------------------------------------------------

pub(crate) struct CompositorHandle<F: FrameSignalSource = ()> {
    surface_key: QtCompositorSurfaceKey,
    inner: Mutex<Mailbox<F>>,
}

struct Mailbox<F: FrameSignalSource> {
    actor: CompositorActor<F>,
    queued: VecDeque<ActorMessage>,
}

impl<F: FrameSignalSource + Send> CompositorHandle<F> {
    fn dispatch(&self, message: ActorMessage) -> Result<ActorReply> {
        let mut mailbox = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        mailbox.queued.push_back(message);
        let mut last = ActorReply::Unit;
        while let Some(msg) = mailbox.queued.pop_front() {
            last = mailbox.actor.handle_message(msg)?;
        }
        Ok(last)
    }

    fn dispatch_bool(&self, message: ActorMessage) -> Result<bool> {
        match self.dispatch(message)? {
            ActorReply::Bool(v) => Ok(v),
            ActorReply::Unit => Err(QtCompositorError::new(
                "compositor actor returned non-bool reply",
            )),
        }
    }
}

impl<F: FrameSignalSource + Send + 'static> Compositor for CompositorHandle<F> {
    fn present_frame(
        &self,
        target: QtCompositorTarget,
        base: &QtCompositorBaseUpload<'_>,
        layers: &[QtCompositorLayerUpload<'_>],
        window_id: Option<u32>,
    ) -> Result<bool> {
        self.dispatch_bool(ActorMessage::PresentFrame {
            target,
            base: OwnedBase::from_borrowed(base),
            layers: layers.iter().map(OwnedLayer::from_borrowed).collect(),
            window_id,
        })
    }

    fn request_frame(&self, target: QtCompositorTarget, reason: FrameReason) -> Result<bool> {
        self.dispatch_bool(ActorMessage::RequestFrame { target, reason })
    }

    fn begin_drive(&self, target: QtCompositorTarget) -> Result<()> {
        self.dispatch(ActorMessage::BeginDrive { target }).map(|_| ())
    }

    fn should_run_frame_source(&self) -> bool {
        self.dispatch_bool(ActorMessage::ShouldRunFrameSource)
            .unwrap_or(false)
    }

    fn is_busy(&self) -> bool {
        crate::surface::compositor_frame_is_busy_for_key(self.surface_key)
    }

    fn is_initialized(&self, target: QtCompositorTarget) -> bool {
        self.dispatch_bool(ActorMessage::IsInitialized { target })
            .unwrap_or(false)
    }

    fn layer_handle(&self, _target: QtCompositorTarget) -> Result<u64> {
        Ok(0)
    }

    fn note_drawable(&self, _target: QtCompositorTarget, _drawable_handle: u64) -> Result<()> {
        Ok(())
    }

    fn release_drawable(&self, _drawable_handle: u64) {}
}

// ---------------------------------------------------------------------------
// Registry + construction
// ---------------------------------------------------------------------------

static WINDOW_COMPOSITORS: LazyLock<Mutex<HashMap<QtCompositorSurfaceKey, Arc<dyn Compositor>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(crate) fn lock_compositors(
) -> MutexGuard<'static, HashMap<QtCompositorSurfaceKey, Arc<dyn Compositor>>> {
    WINDOW_COMPOSITORS
        .lock()
        .unwrap_or_else(|p| p.into_inner())
}

// ---------------------------------------------------------------------------
// Surface key → node_id mapping (populated by drive path, read by frame signal)
// ---------------------------------------------------------------------------

static SURFACE_NODE_MAP: LazyLock<Mutex<HashMap<QtCompositorSurfaceKey, u32>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn register_surface_node_id(key: QtCompositorSurfaceKey, node_id: u32) {
    SURFACE_NODE_MAP
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .insert(key, node_id);
}

pub fn lookup_surface_node_id(key: &QtCompositorSurfaceKey) -> Option<u32> {
    SURFACE_NODE_MAP
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(key)
        .copied()
}

unsafe extern "C" {
    fn qt_solid_notify_window_compositor_present_complete(window_id: u32);
}

struct QtHostCompositorOwner;

impl CompositorOwner for QtHostCompositorOwner {
    fn request_wake(&self) {}

    fn present_complete(&self, window_id: u32) {
        unsafe { qt_solid_notify_window_compositor_present_complete(window_id) };
    }

    fn report_error(&self, error: &QtCompositorError) {
        if trace_enabled() {
            println!("[qt-compositor] owner-error {error}");
        }
    }
}

/// Create a `CompositorHandle<FrameSignal>` for wgpu-surface-backed platforms.
pub(crate) fn load_or_create_wgpu_compositor(
    tag: &'static str,
    expected_surface_kind: u8,
    target: QtCompositorTarget,
) -> Result<Arc<dyn Compositor>> {
    if target.surface_kind != expected_surface_kind {
        return Err(QtCompositorError::new(format!(
            "qt {tag} compositor does not support surface kind {}",
            target.surface_kind,
        )));
    }
    let key = target.surface_key();
    let mut compositors = lock_compositors();
    if let Some(existing) = compositors.get(&key) {
        return Ok(Arc::clone(existing));
    }
    let handle: Arc<dyn Compositor> = Arc::new(CompositorHandle {
        surface_key: key,
        inner: Mutex::new(Mailbox {
            actor: CompositorActor {
                tag,
                owner: Arc::new(QtHostCompositorOwner),
                state: ActorState::default(),
                frame_signal: crate::frame_signal::FrameSignal::new(key),
            },
            queued: VecDeque::new(),
        }),
    });
    compositors.insert(key, Arc::clone(&handle));
    Ok(handle)
}

pub(crate) fn remove_compositor(key: QtCompositorSurfaceKey) {
    lock_compositors().remove(&key);
    SURFACE_NODE_MAP
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .remove(&key);
}
