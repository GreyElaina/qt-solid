pub(crate) mod types;
pub(crate) mod compositor;
pub(crate) mod offscreen;
pub(crate) mod scheduler;

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

    pub(crate) fn with<T>(
        &self,
        node_id: u32,
        run: impl FnOnce(&crate::canvas::fragment::FragmentTree) -> T,
    ) -> Option<T> {
        self.trees.get(&node_id).map(run)
    }

    pub(crate) fn with_mut<T>(
        &mut self,
        node_id: u32,
        run: impl FnOnce(&mut crate::canvas::fragment::FragmentTree) -> T,
    ) -> Option<T> {
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

    pub(crate) fn motion_root_ids(&self, candidates: &[u32]) -> Vec<u32> {
        candidates
            .iter()
            .copied()
            .filter(|id| {
                self.trees
                    .get(id)
                    .map_or(false, |tree| tree.has_active_motion())
            })
            .collect()
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

    #[test]
    fn pending_state_includes_non_pixel_frame_work() {
        let mut renderer = Renderer::new();

        renderer.scheduler.mark_scene_subtree(2, 20);
        renderer.scheduler.mark_frame_tick_node(2, 21);

        let pending = renderer.scheduler.pending_state_snapshot(2);
        assert_eq!(pending.scene_subtrees, HashSet::from([20_u32]));
        assert_eq!(pending.frame_tick_nodes, HashSet::from([21_u32]));
    }

    #[test]
    fn frame_clock_tracks_elapsed_and_delta_time() {
        let mut renderer = Renderer::new();

        renderer.scheduler.tick_frame(2, 1_000_000_000);
        renderer.scheduler.tick_frame(2, 1_016_500_000);

        let clock = renderer.scheduler.frame_clock(2);
        assert_eq!(clock.seq, 2.0);
        assert_eq!(clock.elapsed_ms, 16.5);
        assert_eq!(clock.delta_ms, 16.5);
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
