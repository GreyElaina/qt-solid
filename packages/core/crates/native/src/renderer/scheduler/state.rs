use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use crate::renderer::types::SurfaceTarget;
use crate::runtime::capture::WidgetCapture;

#[derive(Debug, Clone, Copy)]
pub(crate) struct FrameClockState {
    pub(crate) seq: f64,
    pub(crate) elapsed_ms: f64,
    pub(crate) delta_ms: f64,
    started_ns: Option<u64>,
    last_tick_ns: Option<u64>,
    pub(crate) next_frame_requested: bool,
}

impl Default for FrameClockState {
    fn default() -> Self {
        Self {
            seq: 0.0,
            elapsed_ms: 0.0,
            delta_ms: 0.0,
            started_ns: None,
            last_tick_ns: None,
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
    pub(crate) scene_subtrees: HashSet<u32>,
    pub(crate) frame_tick_nodes: HashSet<u32>,
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

pub(crate) struct FrameSchedulingState {
    pub(crate) requested: bool,
    #[cfg(not(target_os = "macos"))]
    pub(crate) frame_signal: Option<super::frame_signal::FrameSignal>,
}

impl Default for FrameSchedulingState {
    fn default() -> Self {
        Self {
            requested: false,
            #[cfg(not(target_os = "macos"))]
            frame_signal: None,
        }
    }
}

#[cfg(not(target_os = "macos"))]
impl FrameSchedulingState {
    pub(crate) fn ensure_frame_signal(&mut self, node_id: u32) -> &mut super::frame_signal::FrameSignal {
        self.frame_signal.get_or_insert_with(|| super::frame_signal::FrameSignal::new(node_id))
    }
}

/// Per-window state aggregated into a single struct to avoid N separate HashMaps.
pub(crate) struct WindowState {
    pub(crate) target: Option<SurfaceTarget>,
    pub(crate) frame_clock: FrameClockState,
    pub(crate) frame_scheduling: FrameSchedulingState,
    pub(crate) geometry_nodes: HashSet<u32>,
    pub(crate) scene_nodes: HashSet<u32>,
    pub(crate) scene_subtrees: HashSet<u32>,
    pub(crate) frame_tick_nodes: HashSet<u32>,
    pub(crate) dirty_nodes: HashSet<u32>,
    pub(crate) dirty_regions: Vec<WindowCompositorDirtyRegion>,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            target: None,
            frame_clock: FrameClockState::default(),
            frame_scheduling: FrameSchedulingState::default(),
            geometry_nodes: HashSet::new(),
            scene_nodes: HashSet::new(),
            scene_subtrees: HashSet::new(),
            frame_tick_nodes: HashSet::new(),
            dirty_nodes: HashSet::new(),
            dirty_regions: Vec::new(),
        }
    }
}

pub(crate) struct Scheduler {
    windows: HashMap<u32, WindowState>,
}

impl Scheduler {
    pub(crate) fn new() -> Self {
        Self {
            windows: HashMap::new(),
        }
    }

    fn window(&self, window_id: u32) -> Option<&WindowState> {
        self.windows.get(&window_id)
    }

    fn window_mut(&mut self, window_id: u32) -> &mut WindowState {
        self.windows.entry(window_id).or_default()
    }

    pub(crate) fn clear_all(&mut self) {
        self.windows.clear();
    }

    pub(crate) fn clear_window(&mut self, window_id: u32) {
        self.windows.remove(&window_id);
    }

    pub(crate) fn forget_node(&mut self, node_id: u32) {
        self.windows.remove(&node_id);
        for ws in self.windows.values_mut() {
            ws.geometry_nodes.remove(&node_id);
            ws.scene_nodes.remove(&node_id);
            ws.scene_subtrees.remove(&node_id);
            ws.frame_tick_nodes.remove(&node_id);
            ws.dirty_nodes.remove(&node_id);
            ws.dirty_regions.retain(|r| r.node_id != node_id);
        }
    }

    pub(crate) fn set_target(&mut self, window_id: u32, target: SurfaceTarget) {
        self.window_mut(window_id).target = Some(target);
    }

    pub(crate) fn target(&self, window_id: u32) -> Option<SurfaceTarget> {
        self.window(window_id).and_then(|ws| ws.target)
    }

    pub(crate) fn mark_dirty_node(&mut self, window_id: u32, node_id: u32) {
        self.window_mut(window_id).dirty_nodes.insert(node_id);
    }

    pub(crate) fn mark_geometry_node(&mut self, window_id: u32, node_id: u32) {
        self.window_mut(window_id).geometry_nodes.insert(node_id);
    }

    pub(crate) fn mark_scene_node(&mut self, window_id: u32, node_id: u32) {
        self.window_mut(window_id).scene_nodes.insert(node_id);
    }

    pub(crate) fn mark_scene_subtree(&mut self, window_id: u32, node_id: u32) {
        self.window_mut(window_id).scene_subtrees.insert(node_id);
    }

    pub(crate) fn mark_frame_tick_node(&mut self, window_id: u32, node_id: u32) {
        self.window_mut(window_id).frame_tick_nodes.insert(node_id);
    }

    pub(crate) fn mark_dirty_region(
        &mut self,
        window_id: u32,
        region: WindowCompositorDirtyRegion,
    ) {
        if region.width <= 0 || region.height <= 0 {
            return;
        }
        let ws = self.window_mut(window_id);
        ws.dirty_nodes.insert(region.node_id);
        ws.dirty_regions.push(region);
    }

    #[cfg(test)]
    pub(crate) fn take_geometry_nodes(&mut self, window_id: u32) -> HashSet<u32> {
        self.windows
            .get_mut(&window_id)
            .map(|ws| std::mem::take(&mut ws.geometry_nodes))
            .unwrap_or_default()
    }

    pub(crate) fn frame_clock(&self, window_id: u32) -> FrameClockState {
        self.window(window_id)
            .map(|ws| ws.frame_clock)
            .unwrap_or_default()
    }

    pub(crate) fn frame_clock_mut(&mut self, window_id: u32) -> &mut FrameClockState {
        &mut self.window_mut(window_id).frame_clock
    }

    pub(crate) fn tick_frame(&mut self, window_id: u32, now_ns: u64) {
        let clock = self.frame_clock_mut(window_id);
        let started_ns = *clock.started_ns.get_or_insert(now_ns);
        clock.seq += 1.0;
        clock.elapsed_ms = now_ns.saturating_sub(started_ns) as f64 / 1_000_000.0;
        clock.delta_ms = clock
            .last_tick_ns
            .map(|last_tick_ns| now_ns.saturating_sub(last_tick_ns) as f64 / 1_000_000.0)
            .unwrap_or(0.0);
        clock.last_tick_ns = Some(now_ns);
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
        if let Some(ws) = self.windows.get_mut(&window_id) {
            ws.geometry_nodes.clear();
            ws.scene_nodes.clear();
            ws.scene_subtrees.clear();
            ws.frame_tick_nodes.clear();
            ws.dirty_nodes.clear();
            ws.dirty_regions.clear();
        }
    }

    pub(crate) fn pending_state_snapshot(&self, window_id: u32) -> WindowCompositorPendingState {
        match self.window(window_id) {
            Some(ws) => WindowCompositorPendingState {
                geometry_nodes: ws.geometry_nodes.clone(),
                scene_nodes: ws.scene_nodes.clone(),
                scene_subtrees: ws.scene_subtrees.clone(),
                frame_tick_nodes: ws.frame_tick_nodes.clone(),
                dirty_nodes: ws.dirty_nodes.clone(),
                dirty_regions: ws.dirty_regions.clone(),
            },
            None => WindowCompositorPendingState::default(),
        }
    }

    #[cfg(test)]
    pub(crate) fn dirty_nodes_for_test(&self, window_id: u32) -> Option<&HashSet<u32>> {
        self.window(window_id).map(|ws| &ws.dirty_nodes)
    }

    pub(crate) fn frame_state(&self, node_id: u32) -> Option<&FrameSchedulingState> {
        self.window(node_id).map(|ws| &ws.frame_scheduling)
    }

    pub(crate) fn frame_state_mut(&mut self, node_id: u32) -> &mut FrameSchedulingState {
        &mut self.window_mut(node_id).frame_scheduling
    }

    pub(crate) fn is_configured(&self, node_id: u32) -> bool {
        self.window(node_id).is_some_and(|ws| ws.target.is_some())
    }

    pub(crate) fn remove_frame_state(&mut self, node_id: u32) {
        if let Some(ws) = self.windows.get_mut(&node_id) {
            ws.frame_scheduling = FrameSchedulingState::default();
        }
    }
}
