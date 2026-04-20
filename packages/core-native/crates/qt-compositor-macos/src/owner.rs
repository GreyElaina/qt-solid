use qt_compositor_core::CompositorOwner;
use qt_compositor_types::QtCompositorError;

use crate::trace::trace;

unsafe extern "C" {
    fn qt_solid_notify_window_compositor_present_complete(window_id: u32);
}

pub(crate) struct QtHostCompositorOwner;

impl CompositorOwner for QtHostCompositorOwner {
    fn request_wake(&self) {}

    fn present_complete(&self, window_id: u32) {
        unsafe { qt_solid_notify_window_compositor_present_complete(window_id) };
    }

    fn report_error(&self, error: &QtCompositorError) {
        trace(format_args!("owner-error {error}"));
    }
}
