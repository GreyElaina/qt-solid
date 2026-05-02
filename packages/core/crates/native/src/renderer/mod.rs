pub(crate) mod compositor;
pub(crate) mod offscreen;
pub(crate) mod scheduler;

// qt-compositor crate usage from native:
//
// Types:
//   QtCompositorTarget          — surface handle + dimensions carrier
//   QtCompositorError / Result  — error type used by offscreen/cpu and wgpu_hybrid
//   QT_COMPOSITOR_SURFACE_*    — surface kind constants
//
// Platform / display-link:
//   load_or_create_compositor() — creates platform compositor, used for:
//     .begin_drive()            — marks frame drive start
//     .request_frame()          — wakes display-link
//     .should_run_frame_source()— queries display-link run state
//     .layer_handle()           — CAMetalLayer pointer (macOS)
//   release_metal_drawable()    — releases display-link drawable (macOS)
//   destroy_compositor()        — tears down platform compositor
//
// Surface utilities:
//   compositor_surface_target() — raw window handle conversion (non-macOS)
//   with_window_compositor_device_queue() — borrows wgpu device/queue for offscreen capture
//
// Readiness query (NOTE: queries qt-compositor's own state, not our compositor's):
//   compositor_frame_is_initialized() — used by C++ to check if first frame rendered
//
// Not used in active render path:
//   Compositor::present_frame   — upload-based present, bypassed by our compositor
//   QtCompositorBaseUpload      — CPU upload structs, not used
//   QtCompositorLayerUpload     — layer upload structs, not used
//   prepare_compositor_frame    — not called
//   present_compositor_frame    — not called
//   present_compositor_frame_async — not called

use std::collections::HashMap;
use std::sync::Mutex;

use once_cell::sync::Lazy;
use scheduler::Scheduler;

pub(crate) struct FragmentStore {
    trees: HashMap<u32, crate::canvas::fragment::FragmentTree>,
}

impl FragmentStore {
    fn new() -> Self {
        Self {
            trees: HashMap::new(),
        }
    }

    pub(crate) fn with<T>(&self, node_id: u32, run: impl FnOnce(&crate::canvas::fragment::FragmentTree) -> T) -> Option<T> {
        self.trees.get(&node_id).map(run)
    }

    pub(crate) fn with_mut<T>(&mut self, node_id: u32, run: impl FnOnce(&mut crate::canvas::fragment::FragmentTree) -> T) -> Option<T> {
        self.trees.get_mut(&node_id).map(run)
    }

    pub(crate) fn ensure(&mut self, node_id: u32) {
        self.trees
            .entry(node_id)
            .or_insert_with(crate::canvas::fragment::FragmentTree::new);
    }

    pub(crate) fn remove(&mut self, node_id: u32) {
        self.trees.remove(&node_id);
    }

    fn clear_all(&mut self) {
        self.trees.clear();
    }
}

pub(crate) struct Renderer {
    pub(crate) scheduler: Scheduler,
    pub(crate) fragments: FragmentStore,
    gpu_mode: HashMap<u32, bool>,
}

impl Renderer {
    fn new() -> Self {
        Self {
            scheduler: Scheduler::new(),
            fragments: FragmentStore::new(),
            gpu_mode: HashMap::new(),
        }
    }

    pub(crate) fn clear_all(&mut self) {
        self.scheduler.clear_all();
        self.fragments.clear_all();
    }

    pub(crate) fn set_gpu_mode(&mut self, node_id: u32, gpu: bool) {
        self.gpu_mode.insert(node_id, gpu);
    }

    pub(crate) fn gpu_enabled(&self, node_id: u32) -> bool {
        self.gpu_mode.get(&node_id).copied().unwrap_or(false)
    }

    pub(crate) fn forget_node(&mut self, node_id: u32) {
        self.fragments.remove(node_id);
        self.gpu_mode.remove(&node_id);
        self.scheduler.forget_node(node_id);
    }

    /// Clean up all renderer state for a destroyed window node.
    pub(crate) fn destroy_window(&mut self, node_id: u32) {
        self.fragments.remove(node_id);
        self.gpu_mode.remove(&node_id);
        compositor::destroy_window_renderer_state(node_id);
        crate::accessibility::destroy_window_accessibility(node_id);
        self.scheduler.clear_window(node_id);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn forget_node_preserves_other_window_state() {
        let mut renderer = Renderer::new();
        renderer.fragments.ensure(2);
        renderer.fragments.ensure(3);
        renderer.scheduler.mark_dirty_node(2, 20);
        renderer.scheduler.mark_dirty_node(3, 30);

        renderer.forget_node(3);

        assert!(renderer.fragments.with(2, |_| ()).is_some());
        assert!(renderer.fragments.with(3, |_| ()).is_none());
        assert_eq!(
            renderer.scheduler.pending_state_snapshot(2).dirty_nodes,
            HashSet::from([20_u32])
        );
        assert!(
            renderer
                .scheduler
                .pending_state_snapshot(3)
                .dirty_nodes
                .is_empty()
        );
    }
}

static RENDERER: Lazy<Mutex<Renderer>> = Lazy::new(|| Mutex::new(Renderer::new()));

pub(crate) fn with_renderer<T>(run: impl FnOnce(&Renderer) -> T) -> T {
    let renderer = RENDERER.lock().expect("renderer mutex poisoned");
    run(&renderer)
}

pub(crate) fn with_renderer_mut<T>(run: impl FnOnce(&mut Renderer) -> T) -> T {
    let mut renderer = RENDERER.lock().expect("renderer mutex poisoned");
    run(&mut renderer)
}
