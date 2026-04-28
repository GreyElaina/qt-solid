use crate::types::{
    QtCompositorAffine, QtCompositorBaseUpload, QtCompositorLayerUpload, QtCompositorTarget,
};

use super::presenter::drop_retained_metal_drawable;
use crate::types::{QtCompositorImageFormat, QtCompositorLayerSourceKind, QtCompositorUploadKind};

fn owned_upload_bytes(upload_kind: QtCompositorUploadKind, bytes: &[u8]) -> Vec<u8> {
    if matches!(upload_kind, QtCompositorUploadKind::None) || bytes.is_empty() {
        return Vec::new();
    }
    bytes.to_vec()
}

#[derive(Default)]
pub(crate) struct MacosCompositorState {
    pub(crate) configured_target: Option<ConfiguredTargetState>,
    pub(crate) pending_drawable: Option<PendingMetalDisplayLinkDrawable>,
    pub(crate) requested: bool,
    pub(crate) in_flight: bool,
    pub(crate) base_initialized: bool,
    pub(crate) epoch: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct PendingMetalDisplayLinkDrawable {
    pub(crate) drawable_handle: u64,
    pub(crate) width_px: u32,
    pub(crate) height_px: u32,
    pub(crate) epoch: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct ConfiguredTargetState {
    pub(crate) width_px: u32,
    pub(crate) height_px: u32,
    pub(crate) format: wgpu::TextureFormat,
}

#[derive(Debug, Clone)]
pub(crate) struct OwnedQtCompositorBaseUpload {
    pub(crate) format: QtCompositorImageFormat,
    pub(crate) width_px: u32,
    pub(crate) height_px: u32,
    pub(crate) stride: usize,
    pub(crate) upload_kind: QtCompositorUploadKind,
    pub(crate) bytes: Vec<u8>,
}

impl OwnedQtCompositorBaseUpload {
    pub(crate) fn from_borrowed(upload: &QtCompositorBaseUpload<'_>) -> Self {
        Self {
            format: upload.format,
            width_px: upload.width_px,
            height_px: upload.height_px,
            stride: upload.stride,
            upload_kind: upload.upload_kind,
            bytes: owned_upload_bytes(upload.upload_kind, upload.bytes),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct OwnedQtCompositorLayerUpload {
    pub(crate) node_id: u32,
    pub(crate) source_kind: QtCompositorLayerSourceKind,
    pub(crate) format: QtCompositorImageFormat,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) transform: QtCompositorAffine,
    pub(crate) opacity: f32,
    pub(crate) clip_rect: Option<crate::types::QtCompositorRect>,
    pub(crate) width_px: u32,
    pub(crate) height_px: u32,
    pub(crate) stride: usize,
    pub(crate) upload_kind: QtCompositorUploadKind,
    pub(crate) visible_rects: Vec<crate::types::QtCompositorRect>,
    pub(crate) bytes: Vec<u8>,
}

impl OwnedQtCompositorLayerUpload {
    pub(crate) fn from_borrowed(upload: &QtCompositorLayerUpload<'_>) -> Self {
        Self {
            node_id: upload.node_id,
            source_kind: upload.source_kind,
            format: upload.format,
            x: upload.x,
            y: upload.y,
            transform: upload.transform,
            opacity: upload.opacity,
            clip_rect: upload.clip_rect,
            width_px: upload.width_px,
            height_px: upload.height_px,
            stride: upload.stride,
            upload_kind: upload.upload_kind,
            visible_rects: upload.visible_rects.to_vec(),
            bytes: owned_upload_bytes(upload.upload_kind, upload.bytes),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct OwnedCompositorSnapshot {
    pub(crate) window_id: u32,
    pub(crate) target: QtCompositorTarget,
    pub(crate) base: OwnedQtCompositorBaseUpload,
    pub(crate) layers: Vec<OwnedQtCompositorLayerUpload>,
}

impl MacosCompositorState {
    pub(crate) fn current_epoch(&self) -> u64 {
        self.epoch
    }

    pub(crate) fn should_run_frame_source(&self) -> bool {
        self.requested || self.in_flight
    }

    pub(crate) fn is_initialized(&self, target: QtCompositorTarget) -> bool {
        self.configured_target
            .map(|configured| {
                configured.width_px == target.width_px
                    && configured.height_px == target.height_px
                    && self.base_initialized
            })
            .unwrap_or(false)
    }

    pub(crate) fn has_configured_target(&self) -> bool {
        self.configured_target.is_some()
    }

    pub(crate) fn normalized_target_for_pending_drawable(
        &self,
        mut target: QtCompositorTarget,
        pending_drawable: Option<PendingMetalDisplayLinkDrawable>,
    ) -> QtCompositorTarget {
        if self.has_configured_target() {
            return target;
        }
        let Some(pending_drawable) = pending_drawable else {
            return target;
        };
        target.width_px = pending_drawable.width_px.max(1);
        target.height_px = pending_drawable.height_px.max(1);
        target
    }

    pub(crate) fn store_pending_drawable(&mut self, pending: PendingMetalDisplayLinkDrawable) {
        if let Some(previous) = self.pending_drawable.replace(pending) {
            drop_retained_metal_drawable(previous.drawable_handle);
        }
    }

    pub(crate) fn drop_stale_artifacts(&mut self, min_epoch: u64) {
        if let Some(pending) = self.pending_drawable {
            if pending.epoch < min_epoch {
                drop_retained_metal_drawable(pending.drawable_handle);
                self.pending_drawable = None;
            }
        }
    }
}

