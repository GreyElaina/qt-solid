use std::{
    collections::{HashMap, VecDeque},
    ffi::c_void,
    ptr::NonNull,
    sync::{Arc, LazyLock, Mutex, MutexGuard},
};

use foreign_types::ForeignType;
use objc2::{MainThreadMarker, runtime::ProtocolObject};
use objc2_metal::{MTLCommandQueue, MTLDevice, MTLTexture};
use objc2_quartz_core::CAMetalDrawable;
use crate::compositor_core::{Compositor, CompositorOwner, FrameReason};
use crate::types::{
    QT_COMPOSITOR_SURFACE_APPKIT_NS_VIEW, QtCompositorBaseUpload, QtCompositorError,
    QtCompositorLayerUpload, QtCompositorSurfaceKey, QtCompositorTarget, Result,
};
use raw_window_metal::Layer;

use super::{
    owner::QtHostCompositorOwner,
    presenter::{
        MetalPresenter, Presenter, RawTextureState, borrowed_metal_drawable_ref,
        create_raw_pipeline_state, drop_retained_metal_drawable, retain_protocol_object,
    },
    state::{
        ConfiguredTargetState, MacosCompositorState, OwnedCompositorSnapshot,
        OwnedQtCompositorBaseUpload, OwnedQtCompositorLayerUpload,
        PendingMetalDisplayLinkDrawable,
    },
    trace::trace,
};
use crate::surface::with_window_compositor_device_queue;

pub(crate) struct MacosCompositorActor {
    owner: Arc<dyn CompositorOwner>,
    presenter: Box<dyn Presenter>,
    state: MacosCompositorState,
}

pub(crate) struct MacosCompositorHandle {
    inner: Mutex<MacosCompositorMailbox>,
}

struct MacosCompositorMailbox {
    actor: MacosCompositorActor,
    queued: VecDeque<ActorMessage>,
}

enum ActorMessage {
    PresentFrame {
        target: QtCompositorTarget,
        base: OwnedQtCompositorBaseUpload,
        layers: Vec<OwnedQtCompositorLayerUpload>,
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
    LayerHandle,
    NoteDrawable {
        target: QtCompositorTarget,
        drawable_handle: u64,
    },
    ReleaseDrawable {
        drawable_handle: u64,
    },
}

enum ActorReply {
    Unit,
    Bool(bool),
    U64(u64),
}

static WINDOW_COMPOSITORS: LazyLock<Mutex<HashMap<QtCompositorSurfaceKey, Arc<dyn Compositor>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

impl MacosCompositorActor {
    fn handle_message(&mut self, message: ActorMessage) -> Result<ActorReply> {
        match message {
            ActorMessage::PresentFrame {
                target,
                base,
                layers,
                window_id,
            } => {
                self.present_frame(target, base, layers, window_id)
                    .map(ActorReply::Bool)
            }
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
            ActorMessage::LayerHandle => Ok(ActorReply::U64(self.presenter.layer_handle())),
            ActorMessage::NoteDrawable {
                target,
                drawable_handle,
            } => {
                self.note_drawable(target, drawable_handle)?;
                Ok(ActorReply::Unit)
            }
            ActorMessage::ReleaseDrawable { drawable_handle } => {
                drop_retained_metal_drawable(drawable_handle);
                Ok(ActorReply::Unit)
            }
        }
    }

    fn ensure_configured(
        &mut self,
        target: QtCompositorTarget,
        present_format: wgpu::TextureFormat,
    ) -> Result<u64> {
        let changed = self
            .state
            .configured_target
            .map(|configured| {
                configured.width_px != target.width_px
                    || configured.height_px != target.height_px
                    || configured.format != present_format
            })
            .unwrap_or(true);
        if changed {
            trace(format_args!(
                "configure target={}x{} format={:?}",
                target.width_px,
                target.height_px,
                present_format
            ));
            self.presenter.configure_for_target(target, present_format)?;
            self.state.epoch += 1;
            self.state.base_initialized = false;
            self.state.configured_target = Some(ConfiguredTargetState {
                width_px: target.width_px,
                height_px: target.height_px,
                format: present_format,
            });
            self.state.drop_stale_artifacts(self.state.epoch);
        }

        Ok(self.state.current_epoch())
    }

    fn request_frame(&mut self, target: QtCompositorTarget, reason: FrameReason) -> Result<bool> {
        if !self.state.has_configured_target() {
            let epoch = self.ensure_configured(target, wgpu::TextureFormat::Bgra8UnormSrgb)?;
            trace(format_args!(
                "request preconfigure target={}x{} epoch={}",
                target.width_px,
                target.height_px,
                epoch
            ));
        }
        self.state.requested = true;
        trace(format_args!(
            "request target={}x{} reason={reason:?} run={}",
            target.width_px,
            target.height_px,
            self.state.should_run_frame_source()
        ));
        self.owner.request_wake();
        Ok(self.state.should_run_frame_source())
    }

    fn begin_drive(&mut self, target: QtCompositorTarget) {
        self.state.requested = false;
        trace(format_args!(
            "begin-drive target={}x{}",
            target.width_px,
            target.height_px
        ));
    }

    fn note_drawable(&mut self, target: QtCompositorTarget, drawable_handle: u64) -> Result<()> {
        let drawable = borrowed_metal_drawable_ref(drawable_handle)
            .ok_or_else(|| QtCompositorError::new("qt macos drawable handle is null"))?;
        let drawable_texture = drawable.texture();
        let drawable_width_px = u32::try_from(drawable_texture.width())
            .map_err(|_| QtCompositorError::new("qt macos drawable width overflow"))?;
        let drawable_height_px = u32::try_from(drawable_texture.height())
            .map_err(|_| QtCompositorError::new("qt macos drawable height overflow"))?;
        trace(format_args!(
            "note-drawable target={}x{} drawable={}x{} epoch={}",
            target.width_px,
            target.height_px,
            drawable_width_px,
            drawable_height_px,
            self.state.current_epoch()
        ));
        self.state.store_pending_drawable(PendingMetalDisplayLinkDrawable {
            drawable_handle,
            width_px: drawable_width_px,
            height_px: drawable_height_px,
            epoch: self.state.current_epoch(),
        });
        Ok(())
    }


    fn present_frame(
        &mut self,
        target: QtCompositorTarget,
        base: OwnedQtCompositorBaseUpload,
        layers: Vec<OwnedQtCompositorLayerUpload>,
        window_id: Option<u32>,
    ) -> Result<bool> {
        let pending_drawable = self.state.pending_drawable.take();
        let target = self
            .state
            .normalized_target_for_pending_drawable(target, pending_drawable);
        trace(format_args!(
            "present-with-presenter target={}x{} has-drawable={}",
            target.width_px,
            target.height_px,
            pending_drawable.is_some()
        ));
        if let Some(pending_drawable) = pending_drawable {
            let frame_epoch =
                self.ensure_configured(target, wgpu::TextureFormat::Bgra8UnormSrgb)?;
            self.state.in_flight = true;
            let snapshot = OwnedCompositorSnapshot {
                window_id: window_id.unwrap_or_default(),
                target,
                base,
                layers,
            };
            let render_result =
                self.presenter
                    .render_snapshot(&mut self.state, pending_drawable, &snapshot);
            self.state.in_flight = false;
            render_result?;
            trace(format_args!(
                "present-complete target={}x{} epoch={} window={:?}",
                target.width_px,
                target.height_px,
                frame_epoch,
                window_id
            ));
            if let Some(window_id) = window_id {
                self.owner.present_complete(window_id);
            }
            Ok(true)
        } else {
            // No drawable available.  Return false so the caller can stage
            // the base bytes for the drive path instead of storing a snapshot
            // that would bypass motion overlay preparation.
            Ok(false)
        }
    }

}

impl Compositor for MacosCompositorHandle {
    fn present_frame(
        &self,
        target: QtCompositorTarget,
        base: &QtCompositorBaseUpload<'_>,
        layers: &[QtCompositorLayerUpload<'_>],
        window_id: Option<u32>,
    ) -> Result<bool> {
        self.dispatch_bool(ActorMessage::PresentFrame {
            target,
            base: OwnedQtCompositorBaseUpload::from_borrowed(base),
            layers: layers
                .iter()
                .map(OwnedQtCompositorLayerUpload::from_borrowed)
                .collect(),
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
        self.dispatch_bool(ActorMessage::IsBusy).unwrap_or(false)
    }

    fn is_initialized(&self, target: QtCompositorTarget) -> bool {
        self.dispatch_bool(ActorMessage::IsInitialized { target })
            .unwrap_or(false)
    }

    fn layer_handle(&self, _target: QtCompositorTarget) -> Result<u64> {
        self.dispatch_u64(ActorMessage::LayerHandle)
    }

    fn note_drawable(&self, target: QtCompositorTarget, drawable_handle: u64) -> Result<()> {
        self.dispatch(ActorMessage::NoteDrawable {
            target,
            drawable_handle,
        })
        .map(|_| ())
    }

    fn release_drawable(&self, drawable_handle: u64) {
        let _ = self.dispatch(ActorMessage::ReleaseDrawable { drawable_handle });
    }

}

impl MacosCompositorHandle {
    fn dispatch(&self, message: ActorMessage) -> Result<ActorReply> {
        let mut mailbox = self
            .inner
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        mailbox.queued.push_back(message);
        let mut last_reply = ActorReply::Unit;
        while let Some(message) = mailbox.queued.pop_front() {
            last_reply = mailbox.actor.handle_message(message)?;
        }
        Ok(last_reply)
    }

    fn dispatch_bool(&self, message: ActorMessage) -> Result<bool> {
        match self.dispatch(message)? {
            ActorReply::Bool(value) => Ok(value),
            ActorReply::Unit | ActorReply::U64(_) => Err(QtCompositorError::new(
                "qt macos compositor actor returned non-bool reply",
            )),
        }
    }

    fn dispatch_u64(&self, message: ActorMessage) -> Result<u64> {
        match self.dispatch(message)? {
            ActorReply::U64(value) => Ok(value),
            ActorReply::Unit | ActorReply::Bool(_) => Err(QtCompositorError::new(
                "qt macos compositor actor returned non-u64 reply",
            )),
        }
    }
}

pub fn load_or_create_compositor(target: QtCompositorTarget) -> Result<Arc<dyn Compositor>> {
    if target.surface_kind != QT_COMPOSITOR_SURFACE_APPKIT_NS_VIEW {
        return Err(QtCompositorError::new(format!(
            "qt macos compositor does not support surface kind {}",
            target.surface_kind
        )));
    }

    let key = target.surface_key();
    let mut compositors = lock_compositors();
    if let Some(compositor) = compositors.get(&key) {
        return Ok(compositor.clone());
    }

    let _main_thread = MainThreadMarker::new().ok_or_else(|| {
        QtCompositorError::new("qt macos compositor layer creation requires main thread")
    })?;
    let ns_view = NonNull::new(target.primary_handle as *mut c_void)
        .ok_or_else(|| QtCompositorError::new("qt macos compositor target is missing NSView"))?;
    let layer = unsafe { Layer::from_ns_view(ns_view) };
    let raw_device = with_window_compositor_device_queue(target, |device, _queue| {
        let raw_device = unsafe { device.as_hal::<wgpu_hal::metal::Api>() }.ok_or_else(|| {
            QtCompositorError::new("qt macos compositor device is not backed by Metal")
        })?;
        let raw_device_ptr = raw_device.raw_device().as_ptr() as *mut ProtocolObject<dyn MTLDevice>;
        retain_protocol_object(raw_device_ptr, "qt macos compositor device")
    })?;
    let present_queue = with_window_compositor_device_queue(target, |_device, queue| {
        let hal_queue = unsafe { queue.as_hal::<wgpu_hal::metal::Api>() }.ok_or_else(|| {
            QtCompositorError::new("qt macos compositor queue is not backed by Metal")
        })?;
        let raw_queue = hal_queue.as_raw().lock();
        let raw_queue_ptr = raw_queue.as_ptr() as *mut ProtocolObject<dyn MTLCommandQueue>;
        retain_protocol_object(raw_queue_ptr, "qt macos compositor present queue")
    })?;
    let raw_pipeline = create_raw_pipeline_state(raw_device.as_ref())?;
    let presenter = MetalPresenter {
        raw_device,
        layer,
        present_queue,
        raw_pipeline,
        raw_textures: RawTextureState::default(),
    };
    let compositor: Arc<dyn Compositor> = Arc::new(MacosCompositorHandle {
        inner: Mutex::new(MacosCompositorMailbox {
            actor: MacosCompositorActor {
                owner: Arc::new(QtHostCompositorOwner),
                presenter: Box::new(presenter),
                state: MacosCompositorState::default(),
            },
            queued: VecDeque::new(),
        }),
    });
    compositors.insert(key, compositor.clone());
    Ok(compositor)
}

pub(crate) fn lock_compositors(
) -> MutexGuard<'static, HashMap<QtCompositorSurfaceKey, Arc<dyn Compositor>>> {
    WINDOW_COMPOSITORS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

pub(crate) fn remove_compositor(key: QtCompositorSurfaceKey) {
    let mut compositors = lock_compositors();
    compositors.remove(&key);
}
