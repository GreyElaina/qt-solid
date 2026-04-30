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
        self.invalidate();
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
        self.invalidate();
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
                self.invalidate();
                return;
            }
        }
        children.push(child);
        self.sync_taffy_children(parent);
        self.invalidate();
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
        self.invalidate();
    }

    pub fn remove(&mut self, id: FragmentId) {
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
        self.invalidate();
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
    pub(crate) fn invalidate_subtree_cache_for(&mut self, id: FragmentId) {
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

    pub(crate) fn invalidate(&mut self) {
        self.any_dirty = true;
        self.aabbs_dirty = true;
        self.cached_scene = None;
        self.subtree_scene_cache.clear();
        self.dirty_root_children.clear();
    }

    pub fn set_debug_highlight(&mut self, id: Option<FragmentId>) {
        if self.debug_highlight != id {
            self.debug_highlight = id;
            self.invalidate();
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
        apply_sampled_pose_to_fragment(node, &sampled);
        if !animating {
            timeline.gc_completed();
        }
        node.timeline = Some(timeline);
        self.invalidate();
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
        apply_sampled_pose_to_fragment(node, &sampled);
        if !animating {
            timeline.gc_completed();
        }
        node.timeline = Some(timeline);
        self.invalidate();
        animating
    }

    /// Tick all fragment timelines. Returns (still_animating, completed_fragment_ids).
    pub fn tick_motion(&mut self, now: f64) -> (bool, Vec<FragmentId>) {
        let ids: Vec<FragmentId> = self.nodes.keys().copied().collect();
        let mut any_animating = false;
        let mut completed = Vec::new();
        let mut any_non_promoted_sampled = false;
        let mut scroll_updates: Vec<(FragmentId, Vec2)> = Vec::new();
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
            apply_sampled_pose_to_fragment(node, &sampled);
            // Collect scroll offset updates from motion channels
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
            self.invalidate();
        } else if any_animating {
            self.aabbs_dirty = true;
        }
        (any_animating, completed)
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
            self.invalidate();
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
        apply_sampled_pose_to_fragment(node, &sampled);
        node.timeline = Some(timeline);
        // Write scroll offset directly
        let offset = Vec2::new(sampled.scroll_x, sampled.scroll_y);
        if offset.x.abs() < 0.01 && offset.y.abs() < 0.01 {
            self.scroll_offsets.remove(&id);
        } else {
            self.scroll_offsets.insert(id, offset);
        }
        self.invalidate();
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
        apply_sampled_pose_to_fragment(node, &sampled);
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
        self.invalidate();
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
        apply_sampled_pose_to_fragment(node, &sampled);
        if !animating {
            timeline.gc_completed();
        }
        node.timeline = Some(timeline);
        self.invalidate();
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
        if self.promoted_node_count == 0 {
            if !self.any_dirty {
                if let Some(cached) = &self.cached_scene {
                    scene.append_scene(cached.clone(), base_transform);
                    return;
                }
            }

            let root_children = self.sorted_children_by_z(&self.root_children.clone());
            let mut fresh = Scene::new();
            for &child_id in &root_children {
                if let Some(cached) = self.subtree_scene_cache.get(&child_id) {
                    fresh.append_scene(cached.clone(), Affine::IDENTITY);
                } else {
                    let mut sub = Scene::new();
                    self.paint_node(&mut sub, child_id, Affine::IDENTITY);
                    fresh.append_scene(sub.clone(), Affine::IDENTITY);
                    self.subtree_scene_cache.insert(child_id, sub);
                }
            }

            for node in self.nodes.values_mut() {
                node.dirty = false;
                node.pose_dirty = false;
            }
            self.any_dirty = false;
            self.dirty_root_children.clear();

            scene.append_scene(fresh.clone(), base_transform);
            self.cached_scene = Some(fresh);
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

            let bounds = Self::compute_subtree_local_bounds(nodes, id, Affine::IDENTITY)
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
}
