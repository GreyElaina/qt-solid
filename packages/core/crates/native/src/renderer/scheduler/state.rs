use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use crate::qt::ffi::QtCompositorTarget;
use crate::runtime::capture::WidgetCapture;

#[derive(Debug, Clone, Copy)]
pub(crate) struct FrameClockState {
    pub(crate) seq: f64,
    pub(crate) elapsed_ms: f64,
    pub(crate) delta_ms: f64,
    pub(crate) next_frame_requested: bool,
}

impl Default for FrameClockState {
    fn default() -> Self {
        Self {
            seq: 0.0,
            elapsed_ms: 0.0,
            delta_ms: 0.0,
            next_frame_requested: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PartVisibleRect {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) width: i32,
    pub(crate) height: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WindowCompositorDirtyRegion {
    pub(crate) node_id: u32,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) width: i32,
    pub(crate) height: i32,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct WindowCompositorPendingState {
    pub(crate) geometry_nodes: HashSet<u32>,
    pub(crate) scene_nodes: HashSet<u32>,
    pub(crate) dirty_nodes: HashSet<u32>,
    pub(crate) dirty_regions: Vec<WindowCompositorDirtyRegion>,
}

#[derive(Debug, Clone)]
pub(crate) struct WindowCaptureComposingPart {
    pub(crate) node_id: u32,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) capture: Arc<WidgetCapture>,
}

pub(crate) struct Scheduler {
    targets: HashMap<u32, QtCompositorTarget>,
    geometry_nodes: HashMap<u32, HashSet<u32>>,
    scene_nodes: HashMap<u32, HashSet<u32>>,
    scene_subtrees: HashMap<u32, HashSet<u32>>,
    frame_tick_nodes: HashMap<u32, HashSet<u32>>,
    dirty_nodes: HashMap<u32, HashSet<u32>>,
    dirty_regions: HashMap<u32, Vec<WindowCompositorDirtyRegion>>,
    frame_clocks: HashMap<u32, FrameClockState>,
}

impl Scheduler {
    pub(crate) fn new() -> Self {
        Self {
            targets: HashMap::new(),
            geometry_nodes: HashMap::new(),
            scene_nodes: HashMap::new(),
            scene_subtrees: HashMap::new(),
            frame_tick_nodes: HashMap::new(),
            dirty_nodes: HashMap::new(),
            dirty_regions: HashMap::new(),
            frame_clocks: HashMap::new(),
        }
    }

    pub(crate) fn clear_all(&mut self) {
        self.targets.clear();
        self.geometry_nodes.clear();
        self.scene_nodes.clear();
        self.scene_subtrees.clear();
        self.frame_tick_nodes.clear();
        self.dirty_nodes.clear();
        self.dirty_regions.clear();
        self.frame_clocks.clear();
    }

    pub(crate) fn set_target(&mut self, window_id: u32, target: QtCompositorTarget) {
        self.targets.insert(window_id, target);
    }

    pub(crate) fn target(&self, window_id: u32) -> Option<QtCompositorTarget> {
        self.targets.get(&window_id).copied()
    }

    pub(crate) fn mark_dirty_node(&mut self, window_id: u32, node_id: u32) {
        self.dirty_nodes
            .entry(window_id)
            .or_default()
            .insert(node_id);
    }

    pub(crate) fn mark_geometry_node(&mut self, window_id: u32, node_id: u32) {
        self.geometry_nodes
            .entry(window_id)
            .or_default()
            .insert(node_id);
    }

    pub(crate) fn mark_scene_node(&mut self, window_id: u32, node_id: u32) {
        self.scene_nodes
            .entry(window_id)
            .or_default()
            .insert(node_id);
    }

    pub(crate) fn mark_scene_subtree(&mut self, window_id: u32, node_id: u32) {
        self.scene_subtrees
            .entry(window_id)
            .or_default()
            .insert(node_id);
    }

    pub(crate) fn mark_frame_tick_node(&mut self, window_id: u32, node_id: u32) {
        self.frame_tick_nodes
            .entry(window_id)
            .or_default()
            .insert(node_id);
    }

    pub(crate) fn mark_dirty_region(
        &mut self,
        window_id: u32,
        region: WindowCompositorDirtyRegion,
    ) {
        if region.width <= 0 || region.height <= 0 {
            return;
        }
        self.mark_dirty_node(window_id, region.node_id);
        self.dirty_regions
            .entry(window_id)
            .or_default()
            .push(region);
    }

    #[cfg(test)]
    pub(crate) fn take_geometry_nodes(&mut self, window_id: u32) -> HashSet<u32> {
        self.geometry_nodes.remove(&window_id).unwrap_or_default()
    }

    pub(crate) fn frame_clock(&self, window_id: u32) -> FrameClockState {
        self.frame_clocks
            .get(&window_id)
            .copied()
            .unwrap_or_default()
    }

    pub(crate) fn frame_clock_mut(&mut self, window_id: u32) -> &mut FrameClockState {
        self.frame_clocks.entry(window_id).or_default()
    }

    pub(crate) fn tick_frame(&mut self, window_id: u32) {
        self.frame_clock_mut(window_id).seq += 1.0;
    }

    pub(crate) fn take_next_frame_request(&mut self, window_id: u32) -> bool {
        let clock = self.frame_clock_mut(window_id);
        let requested = clock.next_frame_requested;
        if requested {
            clock.next_frame_requested = false;
        }
        requested
    }

    pub(crate) fn set_next_frame_requested(&mut self, window_id: u32, value: bool) {
        self.frame_clock_mut(window_id).next_frame_requested = value;
    }

    pub(crate) fn clear_dirty_nodes(&mut self, window_id: u32) {
        self.geometry_nodes.remove(&window_id);
        self.scene_nodes.remove(&window_id);
        self.scene_subtrees.remove(&window_id);
        self.frame_tick_nodes.remove(&window_id);
        self.dirty_nodes.remove(&window_id);
        self.dirty_regions.remove(&window_id);
    }

    pub(crate) fn pending_state_snapshot(&self, window_id: u32) -> WindowCompositorPendingState {
        WindowCompositorPendingState {
            geometry_nodes: self
                .geometry_nodes
                .get(&window_id)
                .cloned()
                .unwrap_or_default(),
            scene_nodes: self
                .scene_nodes
                .get(&window_id)
                .cloned()
                .unwrap_or_default(),
            dirty_nodes: self
                .dirty_nodes
                .get(&window_id)
                .cloned()
                .unwrap_or_default(),
            dirty_regions: self
                .dirty_regions
                .get(&window_id)
                .cloned()
                .unwrap_or_default(),
        }
    }

    #[cfg(test)]
    pub(crate) fn dirty_nodes_for_test(&self, window_id: u32) -> Option<&HashSet<u32>> {
        self.dirty_nodes.get(&window_id)
    }
}
