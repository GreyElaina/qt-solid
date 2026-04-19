use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use napi::Result;
use qt_solid_widget_core::runtime::{WidgetCapture, WidgetCaptureFormat};

use crate::{
    qt::{QtRect, QtWindowCompositorPartMeta, ffi::QtCompositorTarget},
    runtime::qt_error,
};

pub(crate) const WINDOW_COMPOSITOR_RGBA8_PREMULTIPLIED_TAG: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WindowCompositorDirtyFlags(pub(crate) u8);

impl WindowCompositorDirtyFlags {
    pub(crate) const GEOMETRY: Self = Self(1 << 0);
    pub(crate) const SCENE: Self = Self(1 << 1);
    pub(crate) const PIXELS: Self = Self(1 << 2);
    pub(crate) const ALL_BITS: u8 = Self::GEOMETRY.0 | Self::SCENE.0 | Self::PIXELS.0;

    pub(crate) const fn from_bits(bits: u8) -> Self {
        Self(bits & Self::ALL_BITS)
    }

    pub(crate) const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub(crate) const fn bits(self) -> u8 {
        self.0
    }
}

impl std::ops::BitOr for WindowCompositorDirtyFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
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
    pub(crate) visible_rects: Vec<PartVisibleRect>,
    pub(crate) capture: Arc<WidgetCapture>,
}

#[derive(Debug, Clone)]
pub(crate) enum WindowCompositorLayerPayload {
    CpuCapture(Arc<WidgetCapture>),
    CachedTexture {
        fallback_capture: Option<Arc<WidgetCapture>>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WindowCompositorLayerSourceKind {
    CpuCapture,
    CachedTexture,
}

#[derive(Debug, Clone)]
pub(crate) struct WindowCompositorLayerEntry {
    pub(crate) node_id: u32,
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) visible_rects: Vec<PartVisibleRect>,
    pub(crate) format_tag: u8,
    pub(crate) width_px: u32,
    pub(crate) height_px: u32,
    pub(crate) stride: usize,
    pub(crate) scale_factor: f64,
    pub(crate) payload: WindowCompositorLayerPayload,
}

#[derive(Debug, Clone)]
pub(crate) struct WindowCompositorCache {
    pub(crate) generation: u64,
    pub(crate) width_px: u32,
    pub(crate) height_px: u32,
    pub(crate) stride: usize,
    pub(crate) scale_factor: f64,
    pub(crate) parts: Vec<WindowCompositorLayerEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WindowCompositorPartUploadKind {
    None,
    Full,
    SubRects,
}

#[derive(Debug, Clone)]
pub(crate) struct QtPreparedWindowCompositorPart {
    pub(crate) meta: QtWindowCompositorPartMeta,
    pub(crate) visible_rects: Vec<QtRect>,
    pub(crate) upload_kind: WindowCompositorPartUploadKind,
    pub(crate) dirty_rects: Vec<QtRect>,
    pub(crate) source_kind: WindowCompositorLayerSourceKind,
    pub(crate) needs_layer_redraw: bool,
    pub(crate) capture: Option<Arc<WidgetCapture>>,
}

#[derive(Debug, Clone)]
pub(crate) struct QtPreparedWindowCompositorFrame {
    pub(crate) base_upload_kind: WindowCompositorPartUploadKind,
    pub(crate) overlay_layout_changed: bool,
    pub(crate) parts: Vec<QtPreparedWindowCompositorPart>,
}

impl WindowCompositorLayerEntry {
    pub(crate) fn from_capture_part(
        part: WindowCaptureComposingPart,
        source_kind: WindowCompositorLayerSourceKind,
    ) -> Self {
        let WindowCaptureComposingPart {
            node_id,
            x,
            y,
            width,
            height,
            visible_rects,
            capture,
        } = part;
        let format_tag = widget_capture_format_tag(capture.format());
        let width_px = capture.width_px();
        let height_px = capture.height_px();
        let stride = capture.stride();
        let scale_factor = capture.scale_factor();
        let payload = match source_kind {
            WindowCompositorLayerSourceKind::CpuCapture => {
                WindowCompositorLayerPayload::CpuCapture(capture)
            }
            WindowCompositorLayerSourceKind::CachedTexture => {
                WindowCompositorLayerPayload::CachedTexture {
                    fallback_capture: Some(capture),
                }
            }
        };

        Self {
            node_id,
            x,
            y,
            width,
            height,
            visible_rects,
            format_tag,
            width_px,
            height_px,
            stride,
            scale_factor,
            payload,
        }
    }

    pub(crate) fn source_kind(&self) -> WindowCompositorLayerSourceKind {
        match self.payload {
            WindowCompositorLayerPayload::CpuCapture(_) => {
                WindowCompositorLayerSourceKind::CpuCapture
            }
            WindowCompositorLayerPayload::CachedTexture { .. } => {
                WindowCompositorLayerSourceKind::CachedTexture
            }
        }
    }

    pub(crate) fn capture(&self) -> Option<&Arc<WidgetCapture>> {
        match &self.payload {
            WindowCompositorLayerPayload::CpuCapture(capture) => Some(capture),
            WindowCompositorLayerPayload::CachedTexture { fallback_capture } => {
                fallback_capture.as_ref()
            }
        }
    }

    pub(crate) fn to_capture_part(&self) -> Option<WindowCaptureComposingPart> {
        Some(WindowCaptureComposingPart {
            node_id: self.node_id,
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
            visible_rects: self.visible_rects.clone(),
            capture: Arc::clone(self.capture()?),
        })
    }

    pub(crate) fn into_compositor_meta(&self) -> QtWindowCompositorPartMeta {
        QtWindowCompositorPartMeta {
            node_id: self.node_id,
            format_tag: self.format_tag,
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
            width_px: self.width_px,
            height_px: self.height_px,
            stride: self.stride,
            scale_factor: self.scale_factor,
        }
    }

    pub(crate) fn cached_texture(
        node_id: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        visible_rects: Vec<PartVisibleRect>,
        width_px: u32,
        height_px: u32,
        scale_factor: f64,
    ) -> Self {
        Self {
            node_id,
            x,
            y,
            width,
            height,
            visible_rects,
            format_tag: WINDOW_COMPOSITOR_RGBA8_PREMULTIPLIED_TAG,
            width_px,
            height_px,
            stride: 0,
            scale_factor,
            payload: WindowCompositorLayerPayload::CachedTexture {
                fallback_capture: None,
            },
        }
    }
}

impl QtPreparedWindowCompositorFrame {
    pub(crate) fn part(&self, index: usize) -> Result<&QtPreparedWindowCompositorPart> {
        self.parts
            .get(index)
            .ok_or_else(|| qt_error("window compositor frame part index out of range"))
    }

    pub(crate) fn part_count(&self) -> usize {
        self.parts.len()
    }

    pub(crate) fn base_upload_kind(&self) -> WindowCompositorPartUploadKind {
        self.base_upload_kind
    }
}

#[derive(Debug)]
pub(crate) struct CompositorState {
    targets: HashMap<u32, QtCompositorTarget>,
    caches: HashMap<u32, WindowCompositorCache>,
    geometry_nodes: HashMap<u32, HashSet<u32>>,
    scene_nodes: HashMap<u32, HashSet<u32>>,
    scene_subtrees: HashMap<u32, HashSet<u32>>,
    frame_tick_nodes: HashMap<u32, HashSet<u32>>,
    dirty_nodes: HashMap<u32, HashSet<u32>>,
    dirty_regions: HashMap<u32, Vec<WindowCompositorDirtyRegion>>,
}

impl CompositorState {
    pub(crate) fn new() -> Self {
        Self {
            targets: HashMap::new(),
            caches: HashMap::new(),
            geometry_nodes: HashMap::new(),
            scene_nodes: HashMap::new(),
            scene_subtrees: HashMap::new(),
            frame_tick_nodes: HashMap::new(),
            dirty_nodes: HashMap::new(),
            dirty_regions: HashMap::new(),
        }
    }

    pub(crate) fn clear_all(&mut self) {
        self.targets.clear();
        self.caches.clear();
        self.geometry_nodes.clear();
        self.scene_nodes.clear();
        self.scene_subtrees.clear();
        self.frame_tick_nodes.clear();
        self.dirty_nodes.clear();
        self.dirty_regions.clear();
    }

    pub(crate) fn cache(&self, window_id: u32) -> Option<&WindowCompositorCache> {
        self.caches.get(&window_id)
    }

    pub(crate) fn set_target(&mut self, window_id: u32, target: QtCompositorTarget) {
        self.targets.insert(window_id, target);
    }

    pub(crate) fn target(&self, window_id: u32) -> Option<QtCompositorTarget> {
        self.targets.get(&window_id).copied()
    }

    pub(crate) fn set_cache(&mut self, window_id: u32, cache: WindowCompositorCache) {
        self.caches.insert(window_id, cache);
    }

    pub(crate) fn clear_cache(&mut self, window_id: u32) {
        self.caches.remove(&window_id);
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
        self.mark_dirty_node(window_id, region.node_id);
        self.dirty_regions
            .entry(window_id)
            .or_default()
            .push(region);
    }

    pub(crate) fn take_dirty_nodes(&mut self, window_id: u32) -> HashSet<u32> {
        self.dirty_nodes.remove(&window_id).unwrap_or_default()
    }

    pub(crate) fn take_geometry_nodes(&mut self, window_id: u32) -> HashSet<u32> {
        self.geometry_nodes.remove(&window_id).unwrap_or_default()
    }

    pub(crate) fn take_scene_nodes(&mut self, window_id: u32) -> HashSet<u32> {
        self.scene_nodes.remove(&window_id).unwrap_or_default()
    }

    pub(crate) fn take_scene_subtrees(&mut self, window_id: u32) -> HashSet<u32> {
        self.scene_subtrees.remove(&window_id).unwrap_or_default()
    }

    pub(crate) fn take_frame_tick_nodes(&mut self, window_id: u32) -> HashSet<u32> {
        self.frame_tick_nodes.remove(&window_id).unwrap_or_default()
    }

    pub(crate) fn take_dirty_regions(
        &mut self,
        window_id: u32,
    ) -> Vec<WindowCompositorDirtyRegion> {
        self.dirty_regions.remove(&window_id).unwrap_or_default()
    }

    pub(crate) fn clear_dirty_nodes(&mut self, window_id: u32) {
        self.geometry_nodes.remove(&window_id);
        self.scene_nodes.remove(&window_id);
        self.scene_subtrees.remove(&window_id);
        self.frame_tick_nodes.remove(&window_id);
        self.dirty_nodes.remove(&window_id);
        self.dirty_regions.remove(&window_id);
    }

    pub(crate) fn pending_dirty_flags(&self, window_id: u32) -> WindowCompositorDirtyFlags {
        let mut flags = WindowCompositorDirtyFlags::from_bits(0);
        if self
            .geometry_nodes
            .get(&window_id)
            .is_some_and(|nodes| !nodes.is_empty())
        {
            flags = flags | WindowCompositorDirtyFlags::GEOMETRY;
        }
        if self
            .scene_nodes
            .get(&window_id)
            .is_some_and(|nodes| !nodes.is_empty())
            || self
                .scene_subtrees
                .get(&window_id)
                .is_some_and(|nodes| !nodes.is_empty())
        {
            flags = flags | WindowCompositorDirtyFlags::SCENE;
        }
        if self
            .dirty_nodes
            .get(&window_id)
            .is_some_and(|nodes| !nodes.is_empty())
            || self
                .frame_tick_nodes
                .get(&window_id)
                .is_some_and(|nodes| !nodes.is_empty())
            || self
                .dirty_regions
                .get(&window_id)
                .is_some_and(|regions| !regions.is_empty())
        {
            flags = flags | WindowCompositorDirtyFlags::PIXELS;
        }
        flags
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
            scene_subtrees: self
                .scene_subtrees
                .get(&window_id)
                .cloned()
                .unwrap_or_default(),
            frame_tick_nodes: self
                .frame_tick_nodes
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

fn widget_capture_format_tag(format: WidgetCaptureFormat) -> u8 {
    match format {
        WidgetCaptureFormat::Argb32Premultiplied => 1,
        WidgetCaptureFormat::Rgba8Premultiplied => 2,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{CompositorState, WindowCompositorDirtyFlags};

    #[test]
    fn window_compositor_dirty_flags_preserve_combined_bits() {
        let combined = WindowCompositorDirtyFlags::GEOMETRY | WindowCompositorDirtyFlags::PIXELS;

        assert_eq!(
            WindowCompositorDirtyFlags::from_bits(0),
            WindowCompositorDirtyFlags(0)
        );
        assert!(combined.contains(WindowCompositorDirtyFlags::GEOMETRY));
        assert!(combined.contains(WindowCompositorDirtyFlags::PIXELS));
        assert!(!combined.contains(WindowCompositorDirtyFlags::SCENE));
        assert_eq!(
            WindowCompositorDirtyFlags::from_bits(u8::MAX),
            WindowCompositorDirtyFlags(
                WindowCompositorDirtyFlags::GEOMETRY.0
                    | WindowCompositorDirtyFlags::SCENE.0
                    | WindowCompositorDirtyFlags::PIXELS.0
            )
        );
    }

    #[test]
    fn dirty_node_marks_compositor_node() {
        let mut state = CompositorState::new();

        state.mark_dirty_node(7, 9);

        assert_eq!(state.dirty_nodes_for_test(7), Some(&HashSet::from([9_u32])));
    }

    #[test]
    fn geometry_node_tracking_keeps_dirty_state_separate() {
        let mut state = CompositorState::new();

        state.mark_geometry_node(7, 9);

        assert_eq!(state.take_geometry_nodes(7), HashSet::from([9_u32]));
    }

    #[test]
    fn frame_tick_nodes_are_tracked_separately() {
        let mut state = CompositorState::new();

        state.mark_frame_tick_node(7, 11);
        state.mark_frame_tick_node(7, 13);

        assert!(
            state
                .pending_dirty_flags(7)
                .contains(WindowCompositorDirtyFlags::PIXELS)
        );
        assert_eq!(
            state.take_frame_tick_nodes(7),
            HashSet::from([11_u32, 13_u32])
        );
        assert_eq!(state.take_frame_tick_nodes(7), HashSet::new());
    }
}
