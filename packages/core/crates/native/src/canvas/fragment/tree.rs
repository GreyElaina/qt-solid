use std::collections::{HashMap, HashSet};

use taffy::prelude::*;

use super::super::vello::peniko::kurbo::{Affine, BezPath, Rect, Shape, Stroke, Vec2};
use super::super::vello::peniko::{Color, Fill};
use super::super::vello::{PaintScene, Scene};
use crate::scene_renderer::effect_pass::{BackdropBlurEffect, InnerShadowEffect};

use super::node::{apply_sampled_pose_to_fragment, FragmentData, FragmentNode, FragmentProps, LayoutResult};
use super::paint::{is_axis_aligned_affine, transform_local_bounds_to_world, PaintCollector};
use super::types::{
    push_fragment_layer, FragmentClipShape, FragmentId, FragmentLayerKey, FragmentLayoutChange,
    FragmentListeners, PaintChunk, PaintPlan, PromotedLayer, FRAGMENT_LAYER_KEY_BASE,
};

// ---------------------------------------------------------------------------
// SendTaffy — safe wrapper for TaffyTree (single-threaded access)
// ---------------------------------------------------------------------------

// TaffyTree internally holds raw pointers for its node context store,
// making it !Send. Our fragment store is only accessed from the main thread
// (libuv model); the Mutex is purely for Rust's type system. This is safe.
pub(crate) struct SendTaffy(pub(crate) TaffyTree<()>);
unsafe impl Send for SendTaffy {}
unsafe impl Sync for SendTaffy {}

impl std::ops::Deref for SendTaffy {
    type Target = TaffyTree<()>;
    fn deref(&self) -> &Self::Target { &self.0 }
}

impl std::ops::DerefMut for SendTaffy {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}

impl std::fmt::Debug for SendTaffy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SendTaffy(..)")
    }
}

// ---------------------------------------------------------------------------
// FragmentTree
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct FragmentTree {
    pub(crate) nodes: HashMap<FragmentId, FragmentNode>,
    pub(crate) root_children: Vec<FragmentId>,
    pub(crate) next_id: u32,
    pub(crate) cached_scene: Option<Scene>,
    pub(crate) any_dirty: bool,
    pub(crate) aabbs_dirty: bool,
    pub(crate) promoted_node_count: u32,
    pub(crate) next_layer_key: u32,
    pub(crate) promoted_scene_cache: HashMap<FragmentId, Scene>,
    pub(crate) previous_promoted_keys: HashSet<FragmentLayerKey>,
    /// Per-root-child subtree scene cache. Key = root child FragmentId.
    pub(crate) subtree_scene_cache: HashMap<FragmentId, Scene>,
    /// Root children whose subtree contains dirty nodes (cache invalid).
    pub(crate) dirty_root_children: HashSet<FragmentId>,
    pub(crate) taffy: SendTaffy,
    pub(crate) taffy_root: Option<taffy::tree::NodeId>,
    pub(crate) focused: Option<FragmentId>,
    /// Cached available size from last `compute_layout` call (logical pixels).
    pub(crate) last_layout_size: Option<(f64, f64)>,
    /// Per-fragment scroll offset. Only scroll containers have entries.
    pub(crate) scroll_offsets: HashMap<FragmentId, Vec2>,
    /// Fragment to highlight (devtools overlay).
    pub(crate) debug_highlight: Option<FragmentId>,
    /// When true, next frame must do a full clear+render (resize, first frame, clear_all).
    pub(crate) force_full_repaint: bool,
    /// Old bounds of invalidated nodes, keyed by FragmentId.
    /// Stores the world_aabb BEFORE the change so dirty rects cover both
    /// old and new positions. Multiple invalidations of the same node are
    /// unioned into one entry.
    pub(crate) stale_node_bounds: HashMap<FragmentId, Rect>,
    /// Nodes that have been marked dirty since last frame. Used to compute
    /// new bounds for dirty rect calculation.
    pub(crate) dirty_node_ids: HashSet<FragmentId>,
    /// Dirty clip rects in logical coordinates. Set before paint,
    /// used by paint_node_culled for subtree culling.
    pub(crate) dirty_clips: Vec<Rect>,
}

/// Maximum number of independent dirty rects before falling back to union.
const MAX_DIRTY_RECTS: usize = 8;

/// Result of dirty rect computation.
#[derive(Debug)]
pub enum DirtyRectResult {
    /// Full repaint required (first frame, resize, clear_all).
    FullRepaint,
    /// Nothing is dirty — no render needed (noop blit).
    NothingDirty,
    /// Partial update — list of dirty rects in device pixels (x, y, w, h).
    Partial(Vec<(u32, u32, u32, u32)>),
}

impl Default for FragmentTree {
    fn default() -> Self {
        let mut taffy = SendTaffy(TaffyTree::new());
        let taffy_root = taffy.new_leaf(taffy::Style {
            flex_shrink: 0.0,
            ..Default::default()
        }).unwrap();
        Self {
            nodes: HashMap::new(),
            root_children: Vec::new(),
            next_id: 0,
            cached_scene: None,
            any_dirty: false,
            aabbs_dirty: true,
            promoted_node_count: 0,
            next_layer_key: FRAGMENT_LAYER_KEY_BASE,
            promoted_scene_cache: HashMap::new(),
            previous_promoted_keys: HashSet::new(),
            subtree_scene_cache: HashMap::new(),
            dirty_root_children: HashSet::new(),
            taffy,
            taffy_root: Some(taffy_root),
            focused: None,
            last_layout_size: None,
            scroll_offsets: HashMap::new(),
            debug_highlight: None,
            force_full_repaint: true,
            stale_node_bounds: HashMap::new(),
            dirty_node_ids: HashSet::new(),
            dirty_clips: Vec::new(),
        }
    }
}

impl FragmentTree {
    pub fn new() -> Self {
        Self::default()
    }

    /// Dump the fragment tree to stderr for layout debugging.
    pub fn dump_layout(&self) {
        eprintln!("[frag-dump] root_children={:?}", self.root_children);
        let mut ids: Vec<_> = self.nodes.keys().copied().collect();
        ids.sort_by_key(|id| id.0);
        for id in ids {
            let node = &self.nodes[&id];
            let (tag, w, h) = match &node.kind {
                FragmentData::Rect(r) =>
                    (format!("rect fill={}", r.fill.is_some()), r.width, r.height),
                FragmentData::Text(t) =>
                    (format!("text \"{}\"", &t.text[..t.text.len().min(16)]), 0.0, 0.0),
                FragmentData::Group(_) =>
                    ("group".into(), node.layout.width, node.layout.height),
                _ => ("other".into(), 0.0, 0.0),
            };
            eprintln!(
                "[frag-dump] id={:<4} par={:<6} x={:<8.1} y={:<8.1} w={:<8.1} h={:<8.1} clip={} vis={} ch={:?}  {}",
                id.0,
                node.parent.map_or("-".into(), |p| p.0.to_string()),
                node.render_x(), node.render_y(), w, h,
                node.props.clip as u8, node.props.visible as u8,
                node.children.iter().map(|c| c.0).collect::<Vec<_>>(),
                tag,
            );
        }
    }

    pub fn allocate_id(&mut self) -> FragmentId {
        let id = FragmentId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Create a fragment node with the given kind and default properties.
    /// Returns the allocated ID. The node is **not** inserted into any
    /// parent's children list — call `insert_child` separately.
    pub fn create_node(&mut self, kind: FragmentData) -> FragmentId {
        let id = self.allocate_id();
        let is_span = matches!(kind, FragmentData::Span(_));
        let taffy_node = self.taffy.new_leaf(taffy::Style {
            flex_shrink: 0.0,
            ..Default::default()
        }).ok();
        self.nodes.insert(
            id,
            FragmentNode {
                id,
                kind,
                props: FragmentProps {
                    visible: !is_span,
                    pointer_events: !is_span,
                    ..Default::default()
                },
                layout: LayoutResult::default(),
                children: vec![],
                parent: None,
                dirty: true,
                pose_dirty: false,
                taffy_node,
                promoted: false,
                layer_key: None,
                timeline: None,
                world_aabb: None,
                subtree_aabb: None,
                listeners: FragmentListeners::empty(),
            },
        );
        // Node not yet attached to a parent — just mark global dirty.
        self.any_dirty = true;
        self.aabbs_dirty = true;
        self.cached_scene = None;
        id
    }

    pub fn insert(&mut self, node: FragmentNode, parent: Option<FragmentId>) {
        let id = node.id;
        self.nodes.insert(id, node);
        match parent {
            Some(parent_id) => {
                if let Some(parent_node) = self.nodes.get_mut(&parent_id) {
                    parent_node.children.push(id);
                }
                self.nodes.get_mut(&id).map(|n| n.parent = Some(parent_id));
            }
            None => self.root_children.push(id),
        }
        self.any_dirty = true;
        self.aabbs_dirty = true;
        self.cached_scene = None;
        self.invalidate_subtree_cache_for(id);
    }

    /// Insert `child` into `parent`'s children list. If `before` is `Some`,
    /// insert before that sibling; otherwise append.
    /// If `parent` is `None`, insert into root children.
    pub fn insert_child(
        &mut self,
        parent: Option<FragmentId>,
        child: FragmentId,
        before: Option<FragmentId>,
    ) {
        if let Some(child_node) = self.nodes.get_mut(&child) {
            child_node.parent = parent;
        }

        let children = match parent {
            Some(parent_id) => {
                let Some(parent_node) = self.nodes.get_mut(&parent_id) else {
                    return;
                };
                &mut parent_node.children
            }
            None => &mut self.root_children,
        };

        if let Some(anchor) = before {
            if let Some(pos) = children.iter().position(|id| *id == anchor) {
                children.insert(pos, child);
                self.sync_taffy_children(parent);
                self.any_dirty = true;
                self.aabbs_dirty = true;
                self.cached_scene = None;
                self.invalidate_subtree_cache_for(child);
                return;
            }
        }
        children.push(child);
        self.sync_taffy_children(parent);
        self.any_dirty = true;
        self.aabbs_dirty = true;
        self.cached_scene = None;
        self.invalidate_subtree_cache_for(child);
    }

    /// Sync taffy tree children for a given parent (or root).
    fn sync_taffy_children(&mut self, parent: Option<FragmentId>) {
        let (fragment_children, taffy_parent) = match parent {
            Some(parent_id) => {
                let parent_node = self.nodes.get(&parent_id);
                let children = parent_node.map(|n| n.children.clone()).unwrap_or_default();
                let taffy_parent = parent_node.and_then(|n| n.taffy_node);
                (children, taffy_parent)
            }
            None => {
                (self.root_children.clone(), self.taffy_root)
            }
        };

        if let Some(tp) = taffy_parent {
            let taffy_children: Vec<taffy::tree::NodeId> = fragment_children
                .iter()
                .filter_map(|fid| self.nodes.get(fid).and_then(|n| n.taffy_node))
                .collect();
            let _ = self.taffy.set_children(tp, &taffy_children);
        }
    }

    /// Remove `child` from `parent`'s children list without destroying it.
    pub fn detach_child(&mut self, parent: Option<FragmentId>, child: FragmentId) {
        // Invalidate BEFORE detaching — root_child_ancestor needs the parent chain.
        self.invalidate_subtree_cache_for(child);

        let children = match parent {
            Some(parent_id) => {
                let Some(parent_node) = self.nodes.get_mut(&parent_id) else {
                    return;
                };
                &mut parent_node.children
            }
            None => &mut self.root_children,
        };
        children.retain(|id| *id != child);

        if let Some(child_node) = self.nodes.get_mut(&child) {
            child_node.parent = None;
        }
        self.sync_taffy_children(parent);
        self.any_dirty = true;
        self.aabbs_dirty = true;
        self.cached_scene = None;
    }

    pub fn remove(&mut self, id: FragmentId) {
        // Invalidate BEFORE removing — root_child_ancestor needs the parent chain.
        self.invalidate_subtree_cache_for(id);

        if let Some(parent_id) = self.nodes.get(&id).and_then(|n| n.parent) {
            if let Some(parent_node) = self.nodes.get_mut(&parent_id) {
                parent_node.children.retain(|child| *child != id);
            }
        }
        self.root_children.retain(|child| *child != id);

        let mut to_remove = vec![id];
        let mut cursor = 0;
        while cursor < to_remove.len() {
            let current = to_remove[cursor];
            if let Some(node) = self.nodes.get(&current) {
                to_remove.extend_from_slice(&node.children);
            }
            cursor += 1;
        }

        for rid in &to_remove {
            self.scroll_offsets.remove(rid);
            if let Some(node) = self.nodes.remove(rid) {
                if node.promoted {
                    self.promoted_node_count = self.promoted_node_count.saturating_sub(1);
                    self.promoted_scene_cache.remove(rid);
                }
                if let Some(tn) = node.taffy_node {
                    let _ = self.taffy.remove(tn);
                }
            }
        }
        self.any_dirty = true;
        self.aabbs_dirty = true;
        self.cached_scene = None;
    }

    pub fn node(&self, id: FragmentId) -> Option<&FragmentNode> {
        self.nodes.get(&id)
    }

    pub fn node_mut(&mut self, id: FragmentId) -> Option<&mut FragmentNode> {
        self.nodes.get_mut(&id)
    }

    pub fn root_children(&self) -> &[FragmentId] {
        &self.root_children
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn has_promoted_nodes(&self) -> bool {
        self.promoted_node_count > 0
    }

    fn allocate_layer_key(&mut self) -> FragmentLayerKey {
        let key = FragmentLayerKey(self.next_layer_key);
        self.next_layer_key += 1;
        key
    }

    /// Ensure a promoted node has a stable layer key. Returns it.
    fn ensure_layer_key(&mut self, id: FragmentId) -> FragmentLayerKey {
        if let Some(key) = self.nodes.get(&id).and_then(|n| n.layer_key) {
            return key;
        }
        let key = self.allocate_layer_key();
        if let Some(node) = self.nodes.get_mut(&id) {
            node.layer_key = Some(key);
        }
        key
    }

    // -----------------------------------------------------------------------
    // Dirty tracking
    // -----------------------------------------------------------------------

    /// Walk from `id` up the parent chain to find the root child ancestor.
    fn root_child_ancestor(&self, id: FragmentId) -> Option<FragmentId> {
        let mut cur = id;
        loop {
            match self.nodes.get(&cur).and_then(|n| n.parent) {
                Some(p) => cur = p,
                None => {
                    return if self.root_children.contains(&cur) { Some(cur) } else { None };
                }
            }
        }
    }

    /// Invalidate the subtree scene cache for the root child that contains `id`.
    /// Also captures the node's old world_aabb for dirty rect calculation.
    /// Stale bounds are NOT clipped — they represent pixels that need clearing
    /// regardless of clip ancestor visibility.
    pub(crate) fn invalidate_subtree_cache_for(&mut self, id: FragmentId) {
        // Capture old bounds unclipped — these pixels exist on base_texture.
        if let Some(node) = self.nodes.get(&id) {
            if let Some(aabb) = node.world_aabb {
                self.stale_node_bounds
                    .entry(id)
                    .and_modify(|existing| *existing = existing.union(aabb))
                    .or_insert(aabb);
            }
        }
        self.dirty_node_ids.insert(id);

        if let Some(rc) = self.root_child_ancestor(id) {
            self.dirty_root_children.insert(rc);
            self.subtree_scene_cache.remove(&rc);
        }
    }

    pub fn mark_dirty(&mut self, id: FragmentId) {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.dirty = true;
        }
        self.any_dirty = true;
        self.cached_scene = None;
        self.invalidate_subtree_cache_for(id);
    }

    pub fn set_debug_highlight(&mut self, id: Option<FragmentId>) {
        if self.debug_highlight != id {
            self.debug_highlight = id;
            // Debug overlay is cosmetic — just force scene rebuild, not full repaint.
            self.any_dirty = true;
            self.cached_scene = None;
        }
    }

    // -----------------------------------------------------------------------
    // Layout (taffy)
    // -----------------------------------------------------------------------

    /// Run taffy layout and apply results to fragment nodes.
    pub fn compute_layout(&mut self, available_width: f64, available_height: f64) -> Vec<FragmentLayoutChange> {
        let Some(root) = self.taffy_root else { return Vec::new() };

        let mut root_style = self.taffy.style(root).cloned().unwrap_or_default();
        root_style.size = taffy::geometry::Size {
            width: taffy::style::Dimension::length(available_width as f32),
            height: taffy::style::Dimension::length(available_height as f32),
        };
        root_style.flex_shrink = 0.0;
        let _ = self.taffy.set_style(root, root_style);

        let available = taffy::geometry::Size {
            width: AvailableSpace::Definite(available_width as f32),
            height: AvailableSpace::Definite(available_height as f32),
        };

        // Sync fixed measure for text nodes with shaped cache.
        let text_sizes: Vec<(taffy::tree::NodeId, f32, f32)> = self.nodes.values()
            .filter_map(|node| {
                if let (Some(tn), FragmentData::Text(text)) = (node.taffy_node, &node.kind) {
                    text.shaped.as_ref().map(|s| (tn, s.width as f32, s.height as f32))
                } else {
                    None
                }
            })
            .collect();
        for (tn, w, h) in text_sizes {
            let mut style = self.taffy.style(tn).cloned().unwrap_or_default();
            style.size = taffy::geometry::Size {
                width: taffy::style::Dimension::length(w),
                height: taffy::style::Dimension::length(h),
            };
            let _ = self.taffy.set_style(tn, style);
        }

        // Sync fixed measure for text input nodes with layout cache.
        let input_sizes: Vec<(taffy::tree::NodeId, f32, f32)> = self.nodes.values()
            .filter_map(|node| {
                if let (Some(tn), FragmentData::TextInput(ti)) = (node.taffy_node, &node.kind) {
                    ti.layout.as_ref().map(|l| (tn, l.width as f32, l.height as f32))
                } else {
                    None
                }
            })
            .collect();
        for (tn, w, h) in input_sizes {
            let mut style = self.taffy.style(tn).cloned().unwrap_or_default();
            style.size = taffy::geometry::Size {
                width: taffy::style::Dimension::length(w),
                height: taffy::style::Dimension::length(h),
            };
            let _ = self.taffy.set_style(tn, style);
        }

        // Sync fixed measure for circle nodes from radius.
        let circle_sizes: Vec<(taffy::tree::NodeId, f32)> = self.nodes.values()
            .filter_map(|node| {
                if let (Some(tn), FragmentData::Circle(circle)) = (node.taffy_node, &node.kind) {
                    if circle.r > 0.0 && node.props.explicit_width.is_none() && node.props.explicit_height.is_none() {
                        Some((tn, (circle.r * 2.0) as f32))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();
        for (tn, diameter) in circle_sizes {
            let mut style = self.taffy.style(tn).cloned().unwrap_or_default();
            if style.size.width == taffy::style::Dimension::auto() {
                style.size.width = taffy::style::Dimension::length(diameter);
            }
            if style.size.height == taffy::style::Dimension::auto() {
                style.size.height = taffy::style::Dimension::length(diameter);
            }
            let _ = self.taffy.set_style(tn, style);
        }

        let _ = self.taffy.compute_layout(root, available);
        let events = self.apply_layout_results();
        self.last_layout_size = Some((available_width, available_height));
        self.aabbs_dirty = true;
        events
    }

    fn apply_layout_results(&mut self) -> Vec<FragmentLayoutChange> {
        let ids: Vec<FragmentId> = self.nodes.keys().copied().collect();
        let mut layout_dirty_ids: Vec<FragmentId> = Vec::new();
        let mut layout_events: Vec<FragmentLayoutChange> = Vec::new();
        for id in ids {
            let Some(taffy_node) = self.nodes.get(&id).and_then(|n| n.taffy_node) else {
                continue;
            };
            let Ok(layout) = self.taffy.layout(taffy_node) else {
                continue;
            };
            let lx = layout.location.x as f64;
            let ly = layout.location.y as f64;
            let lw = layout.size.width as f64;
            let lh = layout.size.height as f64;

            let node = self.nodes.get_mut(&id).unwrap();
            let has_listener = node.listeners.contains(FragmentListeners::LAYOUT);

            let pos_changed = (node.layout.x - lx).abs() > 0.01
                || (node.layout.y - ly).abs() > 0.01;
            if pos_changed {
                node.layout.x = lx;
                node.layout.y = ly;
                if node.props.explicit_x.is_none() || node.props.explicit_y.is_none() {
                    node.dirty = true;
                    self.any_dirty = true;
                    self.cached_scene = None;
                    layout_dirty_ids.push(id);
                }
            }

            let layout_size_changed = (node.layout.width - lw).abs() > 0.01
                || (node.layout.height - lh).abs() > 0.01;
            if layout_size_changed {
                node.layout.width = lw;
                node.layout.height = lh;
                node.dirty = true;
                self.any_dirty = true;
                self.cached_scene = None;
                layout_dirty_ids.push(id);
            }

            let mut paint_size_changed = false;
            match &mut node.kind {
                FragmentData::Rect(rect) => {
                    let ew = node.props.explicit_width.is_some();
                    let eh = node.props.explicit_height.is_some();
                    let new_w = if ew { rect.width } else { lw };
                    let new_h = if eh { rect.height } else { lh };
                    if (rect.width - new_w).abs() > 0.01 || (rect.height - new_h).abs() > 0.01 {
                        paint_size_changed = true;
                        rect.width = new_w;
                        rect.height = new_h;
                    }
                }
                FragmentData::Image(img) => {
                    let ew = node.props.explicit_width.is_some();
                    let eh = node.props.explicit_height.is_some();
                    let new_w = if ew { img.width } else { lw };
                    let new_h = if eh { img.height } else { lh };
                    if (img.width - new_w).abs() > 0.01 || (img.height - new_h).abs() > 0.01 {
                        paint_size_changed = true;
                        img.width = new_w;
                        img.height = new_h;
                    }
                }
                _ => {}
            }
            if paint_size_changed {
                node.dirty = true;
                self.any_dirty = true;
                self.cached_scene = None;
                layout_dirty_ids.push(id);
            }

            if has_listener && (pos_changed || layout_size_changed || paint_size_changed) {
                layout_events.push(FragmentLayoutChange {
                    fragment_id: id,
                    x: lx,
                    y: ly,
                    width: lw,
                    height: lh,
                });
            }
        }
        for id in layout_dirty_ids {
            self.invalidate_subtree_cache_for(id);
        }
        layout_events
    }

    /// Modify taffy style for a fragment node.
    pub fn with_taffy_style_mut(
        &mut self,
        id: FragmentId,
        f: impl FnOnce(&mut taffy::Style),
    ) {
        let Some(taffy_node) = self.nodes.get(&id).and_then(|n| n.taffy_node) else {
            return;
        };
        let Ok(current) = self.taffy.style(taffy_node).cloned() else {
            return;
        };
        let mut style = current;
        f(&mut style);
        let _ = self.taffy.set_style(taffy_node, style);
    }

    // -----------------------------------------------------------------------
    // Motion — per-fragment timeline tick
    // -----------------------------------------------------------------------

    pub fn set_motion_target(
        &mut self,
        id: FragmentId,
        targets: &[(motion::PropertyKey, f64)],
        default_transition: &motion::TransitionSpec,
        per_property: &std::collections::HashMap<motion::PropertyKey, motion::TransitionSpec>,
        delay_secs: f64,
        now: f64,
    ) -> bool {
        let Some(node) = self.nodes.get_mut(&id) else {
            return false;
        };
        let mut timeline = node.timeline.take().unwrap_or_else(motion::NodeTimeline::new);
        timeline.set_targets(targets, default_transition, per_property, now, delay_secs);
        let (sampled, animating) = timeline.sample_pose(now);
        apply_sampled_pose_to_fragment(node, &sampled, &timeline);
        if !animating {
            timeline.gc_completed();
        }
        node.timeline = Some(timeline);
        self.any_dirty = true;
        self.aabbs_dirty = true;
        self.cached_scene = None;
        self.invalidate_subtree_cache_for(id);
        animating
    }

    pub fn set_motion_target_keyframes(
        &mut self,
        id: FragmentId,
        targets: Vec<(motion::PropertyKey, Vec<f64>)>,
        times: Option<Vec<f64>>,
        default_transition: &motion::TransitionSpec,
        per_property: &std::collections::HashMap<motion::PropertyKey, motion::TransitionSpec>,
        delay_secs: f64,
        now: f64,
    ) -> bool {
        let Some(node) = self.nodes.get_mut(&id) else {
            return false;
        };
        let mut timeline = node.timeline.take().unwrap_or_else(motion::NodeTimeline::new);
        timeline.set_targets_keyframes(targets, times, default_transition, per_property, now, delay_secs);
        let (sampled, animating) = timeline.sample_pose(now);
        apply_sampled_pose_to_fragment(node, &sampled, &timeline);
        if !animating {
            timeline.gc_completed();
        }
        node.timeline = Some(timeline);
        self.any_dirty = true;
        self.aabbs_dirty = true;
        self.cached_scene = None;
        self.invalidate_subtree_cache_for(id);
        animating
    }

    /// Tick all fragment timelines. Returns (still_animating, completed_fragment_ids, max_visual_velocity).
    pub fn tick_motion(&mut self, now: f64) -> (bool, Vec<FragmentId>, f64) {
        let ids: Vec<FragmentId> = self.nodes.keys().copied().collect();
        let mut any_animating = false;
        let mut completed = Vec::new();
        let mut any_non_promoted_sampled = false;
        let mut scroll_updates: Vec<(FragmentId, Vec2)> = Vec::new();
        let mut max_velocity = 0.0f64;
        for id in ids {
            let Some(node) = self.nodes.get_mut(&id) else {
                continue;
            };
            let Some(mut timeline) = node.timeline.take() else {
                continue;
            };
            if !timeline.is_animating() {
                node.timeline = Some(timeline);
                continue;
            }
            let is_promoted = node.promoted;
            let (sampled, animating) = timeline.sample_pose(now);
            apply_sampled_pose_to_fragment(node, &sampled, &timeline);
            let vel = timeline.max_visual_velocity();
            if vel > max_velocity {
                max_velocity = vel;
            }
            if sampled.scroll_x.abs() > 0.01 || sampled.scroll_y.abs() > 0.01 {
                scroll_updates.push((id, Vec2::new(sampled.scroll_x, sampled.scroll_y)));
            }
            if !is_promoted {
                any_non_promoted_sampled = true;
            }
            if !animating {
                timeline.gc_completed();
                completed.push(id);
            }
            any_animating |= animating;
            node.timeline = Some(timeline);
        }
        // Apply scroll offset updates
        for (id, offset) in scroll_updates {
            if offset.x.abs() < 0.01 && offset.y.abs() < 0.01 {
                self.scroll_offsets.remove(&id);
            } else {
                self.scroll_offsets.insert(id, offset);
            }
        }
        if any_non_promoted_sampled {
            // Collect animated non-promoted IDs for targeted cache invalidation.
            let animated_ids: Vec<FragmentId> = self.nodes.values()
                .filter(|n| n.timeline.as_ref().map_or(false, |t| t.is_animating()) && !n.promoted)
                .map(|n| n.id)
                .collect();
            self.any_dirty = true;
            self.aabbs_dirty = true;
            self.cached_scene = None;
            for aid in animated_ids {
                self.invalidate_subtree_cache_for(aid);
            }
        } else if any_animating {
            self.aabbs_dirty = true;
        }
        (any_animating, completed, max_velocity)
    }

    // -----------------------------------------------------------------------
    // Layout FLIP — shared layout animation
    // -----------------------------------------------------------------------

    /// Force layout + aabb recomputation, return world-space bounds for a fragment.
    pub fn get_world_bounds(&mut self, id: FragmentId) -> Option<Rect> {
        if let Some((w, h)) = self.last_layout_size {
            let _ = self.compute_layout(w, h);
        }
        self.ensure_aabbs();
        self.nodes.get(&id)?.world_aabb
    }

    /// Set the scroll offset for a fragment node and invalidate.
    pub fn set_scroll_offset(&mut self, id: FragmentId, offset: Vec2) {
        let current = self.scroll_offsets.get(&id).copied().unwrap_or(Vec2::ZERO);
        if (current.x - offset.x).abs() > 0.01 || (current.y - offset.y).abs() > 0.01 {
            if offset.x.abs() < 0.01 && offset.y.abs() < 0.01 {
                self.scroll_offsets.remove(&id);
            } else {
                self.scroll_offsets.insert(id, offset);
            }
            if let Some(node) = self.nodes.get_mut(&id) {
                node.dirty = true;
            }
            self.any_dirty = true;
            self.aabbs_dirty = true;
            self.cached_scene = None;
            self.invalidate_subtree_cache_for(id);
        }
    }

    /// Drive scroll offset through the motion timeline (instant transition).
    /// This accumulates velocity via prev_target for later spring release.
    pub fn drive_scroll_motion(&mut self, id: FragmentId, x: f64, y: f64, now: f64) {
        let Some(node) = self.nodes.get_mut(&id) else {
            return;
        };
        let mut timeline = node.timeline.take().unwrap_or_else(motion::NodeTimeline::new);
        let instant = motion::TransitionSpec::Instant;
        let targets: Vec<(motion::PropertyKey, f64)> = vec![
            (motion::PropertyKey::ScrollX, x),
            (motion::PropertyKey::ScrollY, y),
        ];
        let empty: HashMap<motion::PropertyKey, motion::TransitionSpec> = HashMap::new();
        timeline.set_targets(&targets, &instant, &empty, now, 0.0);
        // Immediately sample to update scroll_offsets
        let (sampled, _) = timeline.sample_pose(now);
        apply_sampled_pose_to_fragment(node, &sampled, &timeline);
        node.timeline = Some(timeline);
        // Write scroll offset directly
        let offset = Vec2::new(sampled.scroll_x, sampled.scroll_y);
        if offset.x.abs() < 0.01 && offset.y.abs() < 0.01 {
            self.scroll_offsets.remove(&id);
        } else {
            self.scroll_offsets.insert(id, offset);
        }
        self.any_dirty = true;
        self.aabbs_dirty = true;
        self.cached_scene = None;
        self.invalidate_subtree_cache_for(id);
    }

    /// Release scroll: retarget ScrollX/Y to clamped values using spring transition.
    /// Velocity is auto-inferred from preceding instant drives.
    pub fn release_scroll_motion(
        &mut self,
        id: FragmentId,
        clamped_x: f64,
        clamped_y: f64,
        spring: motion::TransitionSpec,
        now: f64,
    ) -> bool {
        let Some(node) = self.nodes.get_mut(&id) else {
            return false;
        };
        let mut timeline = node.timeline.take().unwrap_or_else(motion::NodeTimeline::new);
        let targets: Vec<(motion::PropertyKey, f64)> = vec![
            (motion::PropertyKey::ScrollX, clamped_x),
            (motion::PropertyKey::ScrollY, clamped_y),
        ];
        let empty: HashMap<motion::PropertyKey, motion::TransitionSpec> = HashMap::new();
        timeline.set_targets(&targets, &spring, &empty, now, 0.0);
        let (sampled, animating) = timeline.sample_pose(now);
        apply_sampled_pose_to_fragment(node, &sampled, &timeline);
        if !animating {
            timeline.gc_completed();
        }
        node.timeline = Some(timeline);
        // Write scroll offset
        let offset = Vec2::new(sampled.scroll_x, sampled.scroll_y);
        if offset.x.abs() < 0.01 && offset.y.abs() < 0.01 {
            self.scroll_offsets.remove(&id);
        } else {
            self.scroll_offsets.insert(id, offset);
        }
        self.any_dirty = true;
        self.aabbs_dirty = true;
        self.cached_scene = None;
        self.invalidate_subtree_cache_for(id);
        animating
    }

    /// Get the content size of a fragment node from its taffy layout.
    pub fn get_content_size(&self, id: FragmentId) -> Option<(f64, f64)> {
        let node = self.nodes.get(&id)?;
        let taffy_node = node.taffy_node?;
        let layout = self.taffy.layout(taffy_node).ok()?;
        Some((layout.content_size.width as f64, layout.content_size.height as f64))
    }

    /// Start a layout FLIP animation: instantly set layout channels to the
    /// inverted delta, then animate back to identity.
    pub fn set_layout_flip(
        &mut self,
        id: FragmentId,
        dx: f64,
        dy: f64,
        sx: f64,
        sy: f64,
        transition: &motion::TransitionSpec,
        now: f64,
    ) -> bool {
        let Some(node) = self.nodes.get_mut(&id) else {
            return false;
        };
        let mut timeline = node.timeline.take().unwrap_or_else(motion::NodeTimeline::new);

        let invert_targets = [
            (motion::PropertyKey::LayoutX, dx),
            (motion::PropertyKey::LayoutY, dy),
            (motion::PropertyKey::LayoutScaleX, sx),
            (motion::PropertyKey::LayoutScaleY, sy),
        ];
        let instant = motion::TransitionSpec::Instant;
        let empty_per_prop = std::collections::HashMap::new();
        timeline.set_targets(&invert_targets, &instant, &empty_per_prop, now, 0.0);

        let identity_targets = [
            (motion::PropertyKey::LayoutX, 0.0),
            (motion::PropertyKey::LayoutY, 0.0),
            (motion::PropertyKey::LayoutScaleX, 1.0),
            (motion::PropertyKey::LayoutScaleY, 1.0),
        ];
        timeline.set_targets(&identity_targets, transition, &empty_per_prop, now, 0.0);

        let (sampled, animating) = timeline.sample_pose(now);
        apply_sampled_pose_to_fragment(node, &sampled, &timeline);
        if !animating {
            timeline.gc_completed();
        }
        node.timeline = Some(timeline);
        self.any_dirty = true;
        self.aabbs_dirty = true;
        self.cached_scene = None;
        self.invalidate_subtree_cache_for(id);
        animating
    }

    // -----------------------------------------------------------------------
    // z-index ordering helper
    // -----------------------------------------------------------------------

    pub(crate) fn sorted_children_by_z(&self, children: &[FragmentId]) -> Vec<FragmentId> {
        let needs_sort = children.iter().any(|id| {
            self.nodes.get(id).map_or(false, |n| n.props.z_index != 0)
        });
        if !needs_sort {
            return children.to_vec();
        }
        let mut sorted: Vec<FragmentId> = children.to_vec();
        sorted.sort_by_key(|id| self.nodes.get(id).map_or(0, |n| n.props.z_index));
        sorted
    }

    fn sorted_children_by_z_static(nodes: &HashMap<FragmentId, FragmentNode>, children: &[FragmentId]) -> Vec<FragmentId> {
        let needs_sort = children.iter().any(|id| {
            nodes.get(id).map_or(false, |n| n.props.z_index != 0)
        });
        if !needs_sort {
            return children.to_vec();
        }
        let mut sorted: Vec<FragmentId> = children.to_vec();
        sorted.sort_by_key(|id| nodes.get(id).map_or(0, |n| n.props.z_index));
        sorted
    }

    // -----------------------------------------------------------------------
    // Recursive paint with tree-level scene cache
    // -----------------------------------------------------------------------

    pub fn paint_into_scene(&mut self, scene: &mut Scene, base_transform: Affine) {
        // Ensure AABBs are up to date for culling.
        if !self.dirty_clips.is_empty() {
            self.ensure_aabbs();
        }

        if self.promoted_node_count == 0 {
            if !self.any_dirty {
                if let Some(cached) = &self.cached_scene {
                    scene.append_scene(cached.clone(), base_transform);
                    return;
                }
            }

            let root_children = self.sorted_children_by_z(&self.root_children.clone());
            let dirty_clips = self.dirty_clips.clone();
            let mut fresh = Scene::new();
            for &child_id in &root_children {
                if let Some(cached) = self.subtree_scene_cache.get(&child_id) {
                    fresh.append_scene(cached.clone(), Affine::IDENTITY);
                } else {
                    let mut sub = Scene::new();
                    if !dirty_clips.is_empty() {
                        self.paint_node_culled(&mut sub, child_id, Affine::IDENTITY, &dirty_clips);
                    } else {
                        self.paint_node(&mut sub, child_id, Affine::IDENTITY);
                    }
                    fresh.append_scene(sub.clone(), Affine::IDENTITY);
                    // Only cache subtree scenes from full paints.
                    if dirty_clips.is_empty() {
                        self.subtree_scene_cache.insert(child_id, sub);
                    }
                }
            }

            for node in self.nodes.values_mut() {
                node.dirty = false;
                node.pose_dirty = false;
            }
            self.any_dirty = false;
            self.dirty_root_children.clear();

            scene.append_scene(fresh.clone(), base_transform);
            // Only cache the full scene — partial (culled) paints must not be
            // reused as the complete scene for future full renders.
            if self.dirty_clips.is_empty() {
                self.cached_scene = Some(fresh);
            }
            self.paint_debug_highlight(scene, base_transform);
            return;
        }

        let plan = self.build_paint_plan();
        for chunk in plan.chunks {
            match chunk {
                PaintChunk::Inline(inline_scene) => {
                    scene.append_scene(inline_scene, base_transform);
                }
                PaintChunk::Promoted(layer) => {
                    scene.append_scene(layer.scene, base_transform);
                }
            }
        }
        self.paint_debug_highlight(scene, base_transform);
    }

    /// Paint per root-child subtrees without merging. Returns `(FragmentId, Scene, is_dirty)`.
    /// Only valid when `promoted_node_count == 0`; falls back to single merged scene otherwise.
    pub fn paint_subtrees(&mut self) -> Vec<(FragmentId, Scene, bool)> {
        if !self.dirty_clips.is_empty() {
            self.ensure_aabbs();
        }

        // Promoted path not supported — return single merged.
        if self.promoted_node_count != 0 {
            let mut merged = Scene::new();
            self.paint_into_scene(&mut merged, Affine::IDENTITY);
            // Use FragmentId(u32::MAX) as sentinel for merged scene.
            return vec![(FragmentId(u32::MAX), merged, true)];
        }

        let root_children = self.sorted_children_by_z(&self.root_children.clone());
        let dirty_clips = self.dirty_clips.clone();
        let mut result = Vec::with_capacity(root_children.len());

        for &child_id in &root_children {
            if let Some(cached) = self.subtree_scene_cache.get(&child_id) {
                result.push((child_id, cached.clone(), false));
            } else {
                let mut sub = Scene::new();
                if !dirty_clips.is_empty() {
                    self.paint_node_culled(&mut sub, child_id, Affine::IDENTITY, &dirty_clips);
                } else {
                    self.paint_node(&mut sub, child_id, Affine::IDENTITY);
                }
                if dirty_clips.is_empty() {
                    self.subtree_scene_cache.insert(child_id, sub.clone());
                }
                result.push((child_id, sub, true));
            }
        }

        for node in self.nodes.values_mut() {
            node.dirty = false;
            node.pose_dirty = false;
        }
        self.any_dirty = false;
        self.dirty_root_children.clear();

        // Cache merged scene for paint_into_scene compatibility.
        if self.dirty_clips.is_empty() {
            let mut fresh = Scene::new();
            for (_, sub, _) in &result {
                fresh.append_scene(sub.clone(), Affine::IDENTITY);
            }
            self.cached_scene = Some(fresh);
        }

        result
    }

    fn paint_node(&self, scene: &mut Scene, id: FragmentId, parent_transform: Affine) {
        let Some(node) = self.node(id) else { return };
        if !node.props.visible { return; }

        let transform = parent_transform * node.local_transform();
        let scroll = self.scroll_offsets.get(&id).copied();
        let needs_layer = node.needs_layer() || scroll.is_some();

        if needs_layer {
            let clip = node.clip_shape().or_else(|| {
                scroll.and_then(|_| node.effective_bounds().map(FragmentClipShape::Rect))
            });
            push_fragment_layer(scene, transform, clip.as_ref(), node.props.opacity, node.props.blend_mode);
        }

        node.kind.encode(scene, transform);

        let child_transform = match scroll {
            Some(s) => transform * Affine::translate((-s.x, -s.y)),
            None => transform,
        };

        let children = self.sorted_children_by_z(&node.children.clone());
        for child_id in children {
            self.paint_node(scene, child_id, child_transform);
        }

        if needs_layer {
            scene.pop_layer();
        }
    }

    /// Paint a node with layer-aware dirty rect culling.
    /// - `needs_layer()` nodes are atomic: if `subtree_aabb ∩ dirty_rects = ∅`, skip entirely;
    ///   else paint the whole subtree without further culling.
    /// - Non-layer nodes: if `world_aabb ∩ dirty_rects = ∅`, skip self encode but still recurse.
    fn paint_node_culled(
        &self,
        scene: &mut Scene,
        id: FragmentId,
        parent_transform: Affine,
        dirty_clips: &[Rect],
    ) {
        let Some(node) = self.node(id) else { return };
        if !node.props.visible { return; }

        let scroll = self.scroll_offsets.get(&id).copied();
        let needs_layer = node.needs_layer() || scroll.is_some();

        // Layer-aware culling.
        if !dirty_clips.is_empty() {
            if needs_layer {
                // Atomic unit: skip entire subtree if no intersection.
                if let Some(aabb) = node.subtree_aabb {
                    if !dirty_clips.iter().any(|c| rects_intersect(aabb, *c)) {
                        return;
                    }
                }
                // Intersects → paint full subtree without further culling.
                self.paint_node(scene, id, parent_transform);
                return;
            }
            // Non-layer node: check self, still recurse children.
        }

        let transform = parent_transform * node.local_transform();

        if needs_layer {
            let layer_clip = node.clip_shape().or_else(|| {
                scroll.and_then(|_| node.effective_bounds().map(FragmentClipShape::Rect))
            });
            push_fragment_layer(scene, transform, layer_clip.as_ref(), node.props.opacity, node.props.blend_mode);
        }

        // Cull self-encode if world_aabb doesn't intersect any dirty rect.
        let encode_self = if dirty_clips.is_empty() {
            true
        } else {
            match node.world_aabb {
                Some(aabb) => dirty_clips.iter().any(|c| rects_intersect(aabb, *c)),
                None => true,
            }
        };
        if encode_self {
            node.kind.encode(scene, transform);
        }

        let child_transform = match scroll {
            Some(s) => transform * Affine::translate((-s.x, -s.y)),
            None => transform,
        };

        let children = self.sorted_children_by_z(&node.children.clone());
        for child_id in children {
            self.paint_node_culled(scene, child_id, child_transform, dirty_clips);
        }

        if needs_layer {
            scene.pop_layer();
        }
    }

    /// Paint a single fragment node (self only, no children) into the scene.
    pub fn paint_node_self_only(&self, scene: &mut Scene, id: FragmentId, parent_transform: Affine) {
        let Some(node) = self.node(id) else { return };
        if !node.props.visible { return; }

        let transform = parent_transform * node.local_transform();
        let needs_layer = node.needs_layer();

        if needs_layer {
            let clip = node.clip_shape();
            push_fragment_layer(scene, transform, clip.as_ref(), node.props.opacity, node.props.blend_mode);
        }

        node.kind.encode(scene, transform);

        if needs_layer {
            scene.pop_layer();
        }
    }

    /// Paint a single node at an explicit transform, ignoring the node's own
    /// position/transform.
    pub fn paint_node_at_origin(&self, scene: &mut Scene, id: FragmentId, transform: Affine) {
        let Some(node) = self.node(id) else { return };
        if !node.props.visible { return; }

        let needs_layer = node.needs_layer();
        if needs_layer {
            let clip = node.clip_shape();
            push_fragment_layer(scene, transform, clip.as_ref(), node.props.opacity, node.props.blend_mode);
        }

        node.kind.encode(scene, transform);

        if needs_layer {
            scene.pop_layer();
        }
    }

    fn paint_debug_highlight(&mut self, scene: &mut Scene, base_transform: Affine) {
        let Some(hl_id) = self.debug_highlight else { return };
        self.ensure_aabbs();
        let Some(bounds) = self.nodes.get(&hl_id).and_then(|n| n.world_aabb) else { return };

        let path = BezPath::from_vec(bounds.path_elements(0.1).collect());
        scene.fill(Fill::NonZero, base_transform, Color::from_rgba8(255, 191, 0, 32), None, &path);
        scene.stroke(&Stroke::new(2.0), base_transform, Color::from_rgba8(255, 191, 0, 220), None, &path);
    }

    // -----------------------------------------------------------------------
    // Scene splitting — PaintCollector
    // -----------------------------------------------------------------------

    pub fn build_paint_plan(&mut self) -> PaintPlan {
        let reuse_cache = !self.any_dirty;
        let mut scene_cache = std::mem::take(&mut self.promoted_scene_cache);

        // Capture per-node dirty flags before clearing — needed for
        // content_dirty / pose_only_dirty on PromotedLayer.
        let dirty_flags: HashMap<FragmentId, (bool, bool)> = self.nodes.iter()
            .filter(|(_, n)| n.promoted)
            .map(|(&id, n)| (id, (n.dirty, n.pose_dirty)))
            .collect();

        let mut collector = PaintCollector {
            chunks: Vec::new(),
            current_inline: Scene::new(),
            layer_stack: Vec::new(),
        };
        let root_children = Self::sorted_children_by_z_static(&self.nodes, &self.root_children.clone());
        for &child_id in &root_children {
            Self::paint_node_collecting_cached(
                &self.nodes,
                &self.scroll_offsets,
                &mut collector,
                child_id,
                Affine::IDENTITY,
                reuse_cache,
                &mut scene_cache,
            );
        }
        collector.flush_inline();

        for chunk in &mut collector.chunks {
            if let PaintChunk::Promoted(layer) = chunk {
                layer.layer_key = self.ensure_layer_key(layer.fragment_id);
                // Propagate captured dirty flags.
                let (content_dirty, pose_dirty) = dirty_flags
                    .get(&layer.fragment_id)
                    .copied()
                    .unwrap_or((true, false));
                layer.content_dirty = content_dirty;
                layer.pose_only_dirty = !content_dirty && pose_dirty;
            }
        }

        let current_keys: HashSet<FragmentLayerKey> = collector.chunks.iter()
            .filter_map(|c| match c {
                PaintChunk::Promoted(layer) => Some(layer.layer_key),
                _ => None,
            })
            .collect();
        let stale_keys: Vec<FragmentLayerKey> = self.previous_promoted_keys
            .difference(&current_keys)
            .copied()
            .collect();
        self.previous_promoted_keys = current_keys;

        scene_cache.retain(|id, _| {
            self.nodes.get(id).map_or(false, |n| n.promoted)
        });
        self.promoted_scene_cache = scene_cache;

        for node in self.nodes.values_mut() {
            node.dirty = false;
            node.pose_dirty = false;
        }
        self.any_dirty = false;
        self.cached_scene = None;

        PaintPlan { chunks: collector.chunks, stale_keys }
    }

    /// Collect inner shadow effects from RectFragments with inset shadows.
    pub fn collect_inner_shadow_effects(&mut self, scale_factor: f64) -> Vec<InnerShadowEffect> {
        self.ensure_aabbs();
        let mut effects = Vec::new();
        for node in self.nodes.values() {
            if let FragmentData::Rect(rect) = &node.kind {
                if let Some(shadow) = &rect.shadow {
                    if !shadow.inset { continue; }
                    let Some(world_aabb) = node.world_aabb else { continue };
                    let sf = scale_factor as f32;
                    let r = rect.corner_radii.as_single_radius().unwrap_or(0.0) as f32 * sf;
                    let rgba = shadow.color.to_rgba8();
                    let a = rgba.a as f32 / 255.0;
                    effects.push(InnerShadowEffect {
                        rect_min: [world_aabb.x0 as f32 * sf, world_aabb.y0 as f32 * sf],
                        rect_size: [world_aabb.width() as f32 * sf, world_aabb.height() as f32 * sf],
                        corner_radius: r,
                        offset: [shadow.offset_x as f32 * sf, shadow.offset_y as f32 * sf],
                        blur_std_dev: shadow.blur as f32 * sf,
                        color: [
                            (rgba.r as f32 / 255.0) * a,
                            (rgba.g as f32 / 255.0) * a,
                            (rgba.b as f32 / 255.0) * a,
                            a,
                        ],
                    });
                }
            }
        }
        effects
    }

    /// Collect backdrop blur effects from fragment nodes with `backdrop_blur` set.
    pub fn collect_backdrop_blur_effects(&mut self, scale_factor: f64) -> Vec<BackdropBlurEffect> {
        self.ensure_aabbs();
        let mut effects = Vec::new();
        for node in self.nodes.values() {
            let Some(blur_radius) = node.props.backdrop_blur else { continue };
            if blur_radius <= 0.0 { continue; }
            let Some(world_aabb) = node.world_aabb else { continue };
            let sf = scale_factor as f32;
            let corner_radius = match &node.kind {
                FragmentData::Rect(rect) => rect.corner_radii.as_single_radius().unwrap_or(0.0) as f32 * sf,
                _ => 0.0,
            };
            effects.push(BackdropBlurEffect {
                rect_min: [world_aabb.x0 as f32 * sf, world_aabb.y0 as f32 * sf],
                rect_size: [world_aabb.width() as f32 * sf, world_aabb.height() as f32 * sf],
                corner_radius,
                blur_radius: blur_radius as f32 * sf,
            });
        }
        effects
    }

    fn paint_node_collecting_cached(
        nodes: &HashMap<FragmentId, FragmentNode>,
        scroll_offsets: &HashMap<FragmentId, Vec2>,
        collector: &mut PaintCollector,
        id: FragmentId,
        parent_transform: Affine,
        reuse_cache: bool,
        scene_cache: &mut HashMap<FragmentId, Scene>,
    ) {
        let Some(node) = nodes.get(&id) else { return };
        if !node.props.visible { return; }

        let transform = parent_transform * node.local_transform();

        if node.promoted && Self::is_promotion_eligible_static(nodes, id) {
            collector.flush_inline_for_split();

            let subtree_scene = if reuse_cache {
                if let Some(cached) = scene_cache.get(&id) {
                    let mut copy = Scene::new();
                    copy.append_scene(cached.clone(), Affine::IDENTITY);
                    copy
                } else {
                    let mut s = Scene::new();
                    Self::paint_promoted_subtree_local(nodes, scroll_offsets, &mut s, id);
                    s
                }
            } else {
                let mut s = Scene::new();
                Self::paint_promoted_subtree_local(nodes, scroll_offsets, &mut s, id);
                s
            };

            if !reuse_cache || !scene_cache.contains_key(&id) {
                let mut cache_copy = Scene::new();
                cache_copy.append_scene(subtree_scene.clone(), Affine::IDENTITY);
                scene_cache.insert(id, cache_copy);
            }

            let bounds = Self::compute_promoted_local_bounds(nodes, id)
                .unwrap_or(Rect::ZERO);
            let clip_rect = collector.accumulated_clip_rect();

            collector.chunks.push(PaintChunk::Promoted(PromotedLayer {
                fragment_id: id,
                layer_key: FragmentLayerKey(0),
                scene: subtree_scene,
                bounds,
                transform,
                clip: clip_rect.map(FragmentClipShape::Rect),
                opacity: node.props.opacity,
                blend_mode: node.props.blend_mode,
                content_dirty: true,   // set correctly in build_paint_plan
                pose_only_dirty: false,
            }));

            collector.resume_inline_after_split();
            return;
        }

        let scroll = scroll_offsets.get(&id).copied();
        let needs_layer = node.needs_layer() || scroll.is_some();
        if needs_layer {
            let clip = node.clip_shape().or_else(|| {
                scroll.and_then(|_| node.effective_bounds().map(FragmentClipShape::Rect))
            });
            collector.push_layer(transform, clip, node.props.opacity, node.props.blend_mode);
        }

        node.kind.encode(&mut collector.current_inline, transform);

        let child_transform = match scroll {
            Some(s) => transform * Affine::translate((-s.x, -s.y)),
            None => transform,
        };

        let children = Self::sorted_children_by_z_static(nodes, &node.children.clone());
        for child_id in children {
            Self::paint_node_collecting_cached(
                nodes, scroll_offsets, collector, child_id, child_transform,
                reuse_cache, scene_cache,
            );
        }

        if needs_layer {
            collector.pop_layer();
        }
    }

    fn paint_node_static(
        nodes: &HashMap<FragmentId, FragmentNode>,
        scroll_offsets: &HashMap<FragmentId, Vec2>,
        scene: &mut Scene,
        id: FragmentId,
        parent_transform: Affine,
    ) {
        let Some(node) = nodes.get(&id) else { return };
        if !node.props.visible { return; }

        let transform = parent_transform * node.local_transform();
        let scroll = scroll_offsets.get(&id).copied();
        let needs_layer = node.needs_layer() || scroll.is_some();

        if needs_layer {
            let clip = node.clip_shape().or_else(|| {
                scroll.and_then(|_| node.effective_bounds().map(FragmentClipShape::Rect))
            });
            push_fragment_layer(scene, transform, clip.as_ref(), node.props.opacity, node.props.blend_mode);
        }

        node.kind.encode(scene, transform);

        let child_transform = match scroll {
            Some(s) => transform * Affine::translate((-s.x, -s.y)),
            None => transform,
        };

        let children = Self::sorted_children_by_z_static(nodes, &node.children.clone());
        for child_id in children {
            Self::paint_node_static(nodes, scroll_offsets, scene, child_id, child_transform);
        }

        if needs_layer {
            scene.pop_layer();
        }
    }

    fn paint_promoted_subtree_local(
        nodes: &HashMap<FragmentId, FragmentNode>,
        scroll_offsets: &HashMap<FragmentId, Vec2>,
        scene: &mut Scene,
        id: FragmentId,
    ) {
        let Some(node) = nodes.get(&id) else { return };
        if !node.props.visible { return; }

        node.kind.encode(scene, Affine::IDENTITY);

        let children = Self::sorted_children_by_z_static(nodes, &node.children);
        for &child_id in &children {
            Self::paint_node_static(nodes, scroll_offsets, scene, child_id, Affine::IDENTITY);
        }
    }

    fn is_promotion_eligible_static(
        nodes: &HashMap<FragmentId, FragmentNode>,
        id: FragmentId,
    ) -> bool {
        let node = match nodes.get(&id) {
            Some(n) => n,
            None => return false,
        };
        if !node.promoted {
            return false;
        }
        if Self::compute_subtree_local_bounds(nodes, id, Affine::IDENTITY).is_none() {
            return false;
        }
        let mut cursor = node.parent;
        let mut composed = Affine::IDENTITY;
        while let Some(pid) = cursor {
            let Some(parent) = nodes.get(&pid) else {
                break;
            };
            if parent.promoted {
                return false;
            }
            if parent.props.opacity < 1.0 - f32::EPSILON {
                return false;
            }
            composed = parent.props.transform * composed;
            if parent.props.clip && !is_axis_aligned_affine(composed) {
                return false;
            }
            cursor = parent.parent;
        }
        true
    }

    /// Compute bounds for a promoted subtree in local space — matching the
    /// coordinate space used by `paint_promoted_subtree_local` (root at
    /// identity, children relative to identity).
    fn compute_promoted_local_bounds(
        nodes: &HashMap<FragmentId, FragmentNode>,
        id: FragmentId,
    ) -> Option<Rect> {
        let node = nodes.get(&id)?;
        // Root node encoded at identity (no local_transform applied).
        let mut result: Option<Rect> = node.effective_bounds();
        // Children also painted from identity.
        for &child_id in &node.children {
            if let Some(child_bounds) =
                Self::compute_subtree_local_bounds(nodes, child_id, Affine::IDENTITY)
            {
                result = Some(match result {
                    Some(r) => r.union(child_bounds),
                    None => child_bounds,
                });
            }
        }
        result
    }

    /// Compute the axis-aligned bounding box of an entire subtree in the
    /// coordinate space of the given `parent_transform`.
    fn compute_subtree_local_bounds(
        nodes: &HashMap<FragmentId, FragmentNode>,
        id: FragmentId,
        parent_transform: Affine,
    ) -> Option<Rect> {
        let node = nodes.get(&id)?;
        let transform = parent_transform * node.local_transform();

        let mut result: Option<Rect> = node
            .effective_bounds()
            .map(|lb| transform_local_bounds_to_world(lb, transform));

        for &child_id in &node.children {
            if let Some(child_bounds) =
                Self::compute_subtree_local_bounds(nodes, child_id, transform)
            {
                result = Some(match result {
                    Some(r) => r.union(child_bounds),
                    None => child_bounds,
                });
            }
        }
        result
    }

    /// Compute dirty rects from individual dirty node bounds.
    ///
    /// Returns a `DirtyRectResult` indicating full repaint, nothing dirty,
    /// or a list of partial device-pixel rects.
    pub fn compute_dirty_rects(&mut self, scale_factor: f64) -> DirtyRectResult {
        if self.force_full_repaint {
            return DirtyRectResult::FullRepaint;
        }
        if self.dirty_node_ids.is_empty() && self.stale_node_bounds.is_empty() {
            return DirtyRectResult::NothingDirty;
        }

        // Ensure world AABBs are up to date for new bounds estimation.
        self.ensure_aabbs();

        // Collect per-node dirty rects: union of old bounds and new bounds.
        let all_ids: HashSet<FragmentId> = self
            .stale_node_bounds
            .keys()
            .copied()
            .chain(self.dirty_node_ids.iter().copied())
            .collect();

        let mut rects: Vec<Rect> = Vec::with_capacity(all_ids.len());
        for id in all_ids {
            let mut node_rect: Option<Rect> = None;
            if let Some(&stale) = self.stale_node_bounds.get(&id) {
                node_rect = Some(stale);
            }
            if let Some(node) = self.nodes.get(&id) {
                if let Some(aabb) = node.world_aabb {
                    node_rect = Some(match node_rect {
                        Some(r) => r.union(aabb),
                        None => aabb,
                    });
                }
            }
            if let Some(r) = node_rect {
                rects.push(r);
            }
        }

        if rects.is_empty() {
            return DirtyRectResult::NothingDirty;
        }

        // Convert to device pixels first — tile math operates in pixel space.
        let mut device_rects: Vec<(u32, u32, u32, u32)> = rects
            .iter()
            .map(|r| {
                let x = (r.x0 * scale_factor).floor().max(0.0) as u32;
                let y = (r.y0 * scale_factor).floor().max(0.0) as u32;
                let x1 = (r.x1 * scale_factor).ceil() as u32;
                let y1 = (r.y1 * scale_factor).ceil() as u32;
                (x, y, x1.saturating_sub(x), y1.saturating_sub(y))
            })
            .collect();

        // Tile-aware merge: use wide tile cost (256×4) instead of pixel area.
        merge_dirty_rects_tile_aware(&mut device_rects);

        DirtyRectResult::Partial(device_rects)
    }

    /// Clear stale bounds and force_full_repaint after a frame has been presented.
    pub fn consume_dirty_state(&mut self) {
        self.stale_node_bounds.clear();
        self.dirty_node_ids.clear();
        self.force_full_repaint = false;
    }
}

// ---------------------------------------------------------------------------
// Scene bounds computation from anyrender RenderCommands
// ---------------------------------------------------------------------------

/// Compute the axis-aligned bounding box of all render commands in a scene.
pub(crate) fn scene_bounds(scene: &Scene) -> Option<Rect> {
    use anyrender::recording::RenderCommand;

    let mut result: Option<Rect> = None;

    for cmd in &scene.commands {
        let cmd_bounds = match cmd {
            RenderCommand::Fill(fill) => {
                let local = fill.shape.bounding_box();
                Some(transform_local_bounds_to_world(local, fill.transform))
            }
            RenderCommand::Stroke(stroke) => {
                let local = stroke.shape.bounding_box();
                let inflated = inflate_rect(local, stroke.style.width / 2.0);
                Some(transform_local_bounds_to_world(inflated, stroke.transform))
            }
            RenderCommand::BoxShadow(bs) => {
                let extent = bs.std_dev * 3.0 + bs.radius;
                let inflated = inflate_rect(bs.rect, extent);
                Some(transform_local_bounds_to_world(inflated, bs.transform))
            }
            RenderCommand::GlyphRun(gr) => {
                // Conservative estimate: x range from glyphs, height from font_size.
                if gr.glyphs.is_empty() {
                    None
                } else {
                    let fs = gr.font_size as f64;
                    let min_x = gr.glyphs.iter().map(|g| g.x as f64).fold(f64::INFINITY, f64::min);
                    let max_x = gr.glyphs.iter().map(|g| g.x as f64).fold(f64::NEG_INFINITY, f64::max);
                    // Last glyph advance approximated as 0.6 * font_size.
                    let local = Rect::new(min_x, -fs * 0.8, max_x + fs * 0.6, fs * 0.2);
                    Some(transform_local_bounds_to_world(local, gr.transform))
                }
            }
            RenderCommand::PushLayer(_) | RenderCommand::PushClipLayer(_) | RenderCommand::PopLayer => None,
        };

        if let Some(b) = cmd_bounds {
            result = Some(match result {
                Some(r) => r.union(b),
                None => b,
            });
        }
    }

    result
}

fn inflate_rect(r: Rect, amount: f64) -> Rect {
    Rect::new(r.x0 - amount, r.y0 - amount, r.x1 + amount, r.y1 + amount)
}

fn rects_intersect(a: Rect, b: Rect) -> bool {
    a.x0 < b.x1 && a.x1 > b.x0 && a.y0 < b.y1 && a.y1 > b.y0
}

/// Wide tile dimensions matching vello_hybrid's coarse rasterizer.
/// Content strips and clip regions are dispatched at this granularity.
const WIDE_TILE_W: u32 = 256;
const WIDE_TILE_H: u32 = 4;

/// Count how many wide tiles a device-pixel rect covers.
fn wide_tile_count(x: u32, y: u32, w: u32, h: u32) -> u32 {
    if w == 0 || h == 0 {
        return 0;
    }
    let tx0 = x / WIDE_TILE_W;
    let ty0 = y / WIDE_TILE_H;
    let tx1 = (x + w + WIDE_TILE_W - 1) / WIDE_TILE_W;
    let ty1 = (y + h + WIDE_TILE_H - 1) / WIDE_TILE_H;
    (tx1 - tx0) * (ty1 - ty0)
}

/// Union two device-pixel rects.
fn union_device_rect(
    a: (u32, u32, u32, u32),
    b: (u32, u32, u32, u32),
) -> (u32, u32, u32, u32) {
    let x0 = a.0.min(b.0);
    let y0 = a.1.min(b.1);
    let x1 = (a.0 + a.2).max(b.0 + b.2);
    let y1 = (a.1 + a.3).max(b.1 + b.3);
    (x0, y0, x1 - x0, y1 - y0)
}

/// Tile-aware dirty rect merge (O(n³) worst case, fine for n ≤ ~20).
///
/// Strategy:
/// 1. Unconditionally merge overlapping rects (tile_waste ≤ 0).
/// 2. Repeatedly merge the pair with the smallest tile_waste until
///    count ≤ MAX_DIRTY_RECTS.
/// 3. Opportunistically merge pairs where tile_waste = 0 even when
///    under the cap (same tile row, no extra tiles).
fn merge_dirty_rects_tile_aware(rects: &mut Vec<(u32, u32, u32, u32)>) {
    // Phase 1: merge all overlapping pairs (free or negative waste).
    let mut changed = true;
    while changed {
        changed = false;
        let mut i = 0;
        while i < rects.len() {
            let mut j = i + 1;
            while j < rects.len() {
                let (ax, ay, aw, ah) = rects[i];
                let (bx, by, bw, bh) = rects[j];
                // Check pixel overlap.
                let overlaps = ax < bx + bw && ax + aw > bx && ay < by + bh && ay + ah > by;
                if overlaps {
                    rects[i] = union_device_rect(rects[i], rects[j]);
                    rects.swap_remove(j);
                    changed = true;
                } else {
                    j += 1;
                }
            }
            i += 1;
        }
    }

    // Phase 2: reduce to MAX_DIRTY_RECTS by merging the cheapest pair
    // (measured in wide tiles added).
    while rects.len() > MAX_DIRTY_RECTS {
        let (mi, mj) = cheapest_merge_pair(rects);
        rects[mi] = union_device_rect(rects[mi], rects[mj]);
        rects.swap_remove(mj);
    }

    // Phase 3: opportunistic zero-cost merges (same tile rows, no extra tiles).
    let mut changed = true;
    while changed {
        changed = false;
        let mut i = 0;
        while i < rects.len() {
            let mut j = i + 1;
            while j < rects.len() {
                let merged = union_device_rect(rects[i], rects[j]);
                let cost_separate =
                    wide_tile_count(rects[i].0, rects[i].1, rects[i].2, rects[i].3)
                        + wide_tile_count(rects[j].0, rects[j].1, rects[j].2, rects[j].3);
                let cost_merged = wide_tile_count(merged.0, merged.1, merged.2, merged.3);
                if cost_merged <= cost_separate {
                    rects[i] = merged;
                    rects.swap_remove(j);
                    changed = true;
                } else {
                    j += 1;
                }
            }
            i += 1;
        }
    }
}

/// Find the pair (i, j) whose merge adds the fewest extra wide tiles.
fn cheapest_merge_pair(rects: &[(u32, u32, u32, u32)]) -> (usize, usize) {
    let mut best = (0, 1);
    let mut best_waste = u32::MAX;
    for i in 0..rects.len() {
        let ti = wide_tile_count(rects[i].0, rects[i].1, rects[i].2, rects[i].3);
        for j in (i + 1)..rects.len() {
            let tj = wide_tile_count(rects[j].0, rects[j].1, rects[j].2, rects[j].3);
            let merged = union_device_rect(rects[i], rects[j]);
            let tm = wide_tile_count(merged.0, merged.1, merged.2, merged.3);
            let waste = tm.saturating_sub(ti + tj);
            if waste < best_waste {
                best_waste = waste;
                best = (i, j);
            }
        }
    }
    best
}
