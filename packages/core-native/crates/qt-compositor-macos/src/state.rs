use qt_compositor_types::{QtCompositorBaseUpload, QtCompositorLayerUpload, QtCompositorTarget};

use crate::{
    QtCompositorImageFormat, QtCompositorLayerSourceKind, QtCompositorUploadKind,
    presenter::drop_retained_metal_drawable, trace::trace,
};

#[derive(Default)]
pub(crate) struct MacosCompositorState {
    pub(crate) configured_target: Option<ConfiguredTargetState>,
    pub(crate) pending_drawable: Option<PendingMetalDisplayLinkDrawable>,
    pub(crate) latest_snapshot: Option<OwnedCompositorSnapshot>,
    pub(crate) requested: bool,
    pub(crate) in_flight: bool,
    pub(crate) base_initialized: bool,
    pub(crate) epoch: u64,
    pub(crate) frame_serial: u64,
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
            bytes: upload.bytes.to_vec(),
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
    pub(crate) width_px: u32,
    pub(crate) height_px: u32,
    pub(crate) stride: usize,
    pub(crate) upload_kind: QtCompositorUploadKind,
    pub(crate) visible_rects: Vec<qt_compositor_types::QtCompositorRect>,
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
            width_px: upload.width_px,
            height_px: upload.height_px,
            stride: upload.stride,
            upload_kind: upload.upload_kind,
            visible_rects: upload.visible_rects.to_vec(),
            bytes: upload.bytes.to_vec(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct OwnedCompositorSnapshot {
    pub(crate) window_id: u32,
    pub(crate) target: QtCompositorTarget,
    pub(crate) base: OwnedQtCompositorBaseUpload,
    pub(crate) layers: Vec<OwnedQtCompositorLayerUpload>,
    pub(crate) epoch: u64,
}

impl MacosCompositorState {
    pub(crate) fn current_epoch(&self) -> u64 {
        self.epoch
    }

    pub(crate) fn should_run_frame_source(&self) -> bool {
        self.requested || self.in_flight || self.latest_snapshot.is_some()
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

    pub(crate) fn store_snapshot(&mut self, snapshot: OwnedCompositorSnapshot) {
        let mut snapshot = snapshot;
        if !self.base_initialized
            && matches!(snapshot.base.upload_kind, QtCompositorUploadKind::None)
        {
            if let Some(previous) = self.latest_snapshot.as_ref() {
                let same_base_dimensions = previous.base.width_px == snapshot.base.width_px
                    && previous.base.height_px == snapshot.base.height_px
                    && previous.base.stride == snapshot.base.stride
                    && previous.base.format == snapshot.base.format;
                if same_base_dimensions
                    && !matches!(previous.base.upload_kind, QtCompositorUploadKind::None)
                {
                    trace(format_args!(
                        "snapshot-merge-preserve-base window={} epoch={} bytes={}",
                        snapshot.window_id,
                        previous.epoch,
                        previous.base.bytes.len()
                    ));
                    snapshot.base = previous.base.clone();
                }
            }
        }
        self.latest_snapshot = Some(snapshot);
    }

    pub(crate) fn drop_stale_artifacts(&mut self, min_epoch: u64) {
        if let Some(pending) = self.pending_drawable {
            if pending.epoch < min_epoch {
                drop_retained_metal_drawable(pending.drawable_handle);
                self.pending_drawable = None;
            }
        }
        if self
            .latest_snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot.epoch < min_epoch)
        {
            self.latest_snapshot = None;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PresentEpochDisposition {
    Submit,
    KeepDeferredFrame,
    KeepPendingDrawable,
}

pub(crate) fn compare_present_state(
    enforce_size_match: bool,
    drawable_epoch: u64,
    drawable_width_px: u32,
    drawable_height_px: u32,
    frame_epoch: u64,
    frame_width_px: u32,
    frame_height_px: u32,
) -> PresentEpochDisposition {
    if enforce_size_match
        && (drawable_width_px != frame_width_px || drawable_height_px != frame_height_px)
    {
        return if drawable_epoch < frame_epoch {
            PresentEpochDisposition::KeepDeferredFrame
        } else {
            PresentEpochDisposition::KeepPendingDrawable
        };
    }

    match drawable_epoch.cmp(&frame_epoch) {
        std::cmp::Ordering::Less => PresentEpochDisposition::KeepDeferredFrame,
        std::cmp::Ordering::Equal => PresentEpochDisposition::Submit,
        std::cmp::Ordering::Greater => PresentEpochDisposition::KeepPendingDrawable,
    }
}

#[cfg(test)]
mod tests {
    use super::{PresentEpochDisposition, compare_present_state};

    #[test]
    fn older_drawable_keeps_newer_frame() {
        assert_eq!(
            compare_present_state(true, 3, 100, 100, 4, 100, 100),
            PresentEpochDisposition::KeepDeferredFrame
        );
    }

    #[test]
    fn matching_epochs_submit() {
        assert_eq!(
            compare_present_state(true, 7, 100, 100, 7, 100, 100),
            PresentEpochDisposition::Submit
        );
    }

    #[test]
    fn newer_drawable_drops_stale_frame() {
        assert_eq!(
            compare_present_state(true, 9, 100, 100, 8, 100, 100),
            PresentEpochDisposition::KeepPendingDrawable
        );
    }

    #[test]
    fn size_mismatch_keeps_newer_frame() {
        assert_eq!(
            compare_present_state(true, 4, 100, 100, 5, 120, 120),
            PresentEpochDisposition::KeepDeferredFrame
        );
    }

    #[test]
    fn size_mismatch_drops_stale_frame() {
        assert_eq!(
            compare_present_state(true, 6, 140, 140, 5, 120, 120),
            PresentEpochDisposition::KeepPendingDrawable
        );
    }

    #[test]
    fn first_present_allows_size_mismatch_before_target_is_configured() {
        assert_eq!(
            compare_present_state(false, 0, 100, 100, 0, 120, 120),
            PresentEpochDisposition::Submit
        );
    }
}
