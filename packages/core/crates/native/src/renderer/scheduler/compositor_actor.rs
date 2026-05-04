use std::fmt;
use std::sync::LazyLock;

use crate::renderer::types::SurfaceTarget;

fn trace_enabled() -> bool {
    static ENABLED: LazyLock<bool> =
        LazyLock::new(|| std::env::var_os("QT_SOLID_WGPU_TRACE").is_some());
    *ENABLED
}

fn trace(args: fmt::Arguments<'_>) {
    if trace_enabled() {
        println!("[qt-compositor] {args}");
    }
}

pub(crate) fn begin_drive(node_id: u32, target: SurfaceTarget) {
    crate::renderer::with_renderer_mut(|r| {
        let frame = r.scheduler.frame_state_mut(node_id);
        frame.requested = false;
        frame.configured = Some((target.width_px, target.height_px));
        #[cfg(not(target_os = "macos"))]
        frame.ensure_frame_signal(node_id).stop();
    });
    trace(format_args!(
        "begin-drive node={node_id} target={}x{}",
        target.width_px, target.height_px,
    ));
}

pub(crate) fn request_frame(node_id: u32) -> bool {
    crate::renderer::with_renderer_mut(|r| {
        let frame = r.scheduler.frame_state_mut(node_id);
        frame.requested = true;
        #[cfg(not(target_os = "macos"))]
        frame.ensure_frame_signal(node_id).start();
    });
    trace(format_args!("request node={node_id} run=true"));
    true
}

pub(crate) fn should_run_frame_source(node_id: u32) -> bool {
    crate::renderer::with_renderer(|r| {
        r.scheduler.frame_state(node_id).is_some_and(|s| s.requested)
    })
}

pub(crate) fn is_initialized(node_id: u32) -> bool {
    crate::renderer::with_renderer(|r| {
        r.scheduler.is_configured(node_id)
    })
}

pub(crate) fn destroy(node_id: u32) {
    crate::renderer::with_renderer_mut(|r| {
        r.scheduler.remove_frame_state(node_id);
    });
    #[cfg(not(target_os = "macos"))]
    crate::renderer::compositor::surface::destroy_window_compositor_by_node(node_id);
    trace(format_args!("destroy node={node_id}"));
}
