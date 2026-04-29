use crate::types::{
    QtCompositorBaseUpload, QtCompositorError, QtCompositorLayerUpload, QtCompositorTarget,
    Result,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompositorBackendKind {
    Surface,
    Macos,
    Windows,
    X11,
    Wayland,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FrameReason {
    SnapshotIngested,
    OverlayInvalidated,
    PresentCompleted,
    VisibilityChanged,
    Resize,
    ExternalWake,
}

pub trait CompositorOwner: Send + Sync {
    fn request_wake(&self);

    fn present_complete(&self, window_id: u32);

    fn report_error(&self, error: &QtCompositorError);
}

pub trait Compositor: Send + Sync {
    /// Present a compositor frame.  Returns `true` if the frame was rendered
    /// to a drawable, `false` if it was deferred (no drawable available).
    fn present_frame(
        &self,
        target: QtCompositorTarget,
        base: &QtCompositorBaseUpload<'_>,
        layers: &[QtCompositorLayerUpload<'_>],
        window_id: Option<u32>,
    ) -> Result<bool>;

    fn request_frame(&self, target: QtCompositorTarget, reason: FrameReason) -> Result<bool>;

    fn begin_drive(&self, target: QtCompositorTarget) -> Result<()>;

    fn should_run_frame_source(&self) -> bool;

    fn is_busy(&self) -> bool;

    fn is_initialized(&self, target: QtCompositorTarget) -> bool;

    fn layer_handle(&self, target: QtCompositorTarget) -> Result<u64>;

    fn note_drawable(&self, target: QtCompositorTarget, drawable_handle: u64) -> Result<()>;

    fn release_drawable(&self, drawable_handle: u64);
}
