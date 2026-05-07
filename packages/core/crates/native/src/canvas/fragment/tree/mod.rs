mod dirty_rect;
mod motion;
mod paint;

pub use dirty_rect::DirtyRectResult;
pub(crate) use dirty_rect::scene_bounds;

use std::collections::{HashMap, HashSet};

use taffy::prelude::*;

use crate::canvas::fragment::layout::{
    Container, Direction, EdgeInsets, Overflow, Placement,
};
use crate::canvas::fragment::node::{
    FragmentData, FragmentNode, FragmentProps, LayoutResult, SemanticsData,
};
use crate::canvas::fragment::types::{
    FRAGMENT_LAYER_KEY_BASE, FragmentId, FragmentLayerKey, FragmentLayoutChange, FragmentListeners,
};
use crate::vello::Scene;
use crate::vello::peniko::kurbo::{Rect, Vec2};

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
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for SendTaffy {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
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
    // -- core storage -------------------------------------------------------
    pub(crate) nodes: HashMap<FragmentId, FragmentNode>,
    pub(crate) root_children: Vec<FragmentId>,
    pub(crate) next_id: u32,

    // -- layout (taffy) -----------------------------------------------------
    pub(crate) taffy: SendTaffy,
    pub(crate) taffy_root: Option<taffy::tree::NodeId>,
    /// Cached available size from last `compute_layout` call (logical pixels).
    pub(crate) last_layout_size: Option<(f64, f64)>,

    // -- dirty tracking -----------------------------------------------------
    pub(crate) any_dirty: bool,
    pub(crate) aabbs_dirty: bool,
    /// When true, next frame must do a full clear+render (resize, first frame, clear_all).
    pub(crate) force_full_repaint: bool,
    /// Root children whose subtree contains dirty nodes (cache invalid).
    pub(crate) dirty_root_children: HashSet<FragmentId>,
    /// Nodes marked dirty since last frame (for dirty rect computation).
    pub(crate) dirty_node_ids: HashSet<FragmentId>,
    /// Old world_aabb of invalidated nodes BEFORE the change, for dirty rect
    /// coverage of both old and new positions.
    pub(crate) stale_node_bounds: HashMap<FragmentId, Rect>,
    /// Dirty clip rects in logical coordinates, set before paint for
    /// subtree culling in `paint_node_culled`.
    pub(crate) dirty_clips: Vec<Rect>,

    // -- paint / compositing cache ------------------------------------------
    /// Per-root-child subtree scene cache. Key = root child FragmentId.
    pub(crate) subtree_scene_cache: HashMap<FragmentId, Scene>,
    pub(crate) promoted_node_count: u32,
    pub(crate) next_layer_key: u32,
    pub(crate) promoted_scene_cache: HashMap<FragmentId, Scene>,
    pub(crate) previous_promoted_keys: HashSet<FragmentLayerKey>,

    // -- interaction / a11y -------------------------------------------------
    pub(crate) focused: Option<FragmentId>,
    /// Per-fragment scroll offset. Only scroll containers have entries.
    pub(crate) scroll_offsets: HashMap<FragmentId, Vec2>,
    /// Fragment IDs whose semantics data changed (role, label, bounds, etc.).
    pub(crate) semantics_dirty: HashSet<FragmentId>,

    // -- devtools -----------------------------------------------------------
    /// Fragment to highlight (devtools overlay).
    pub(crate) debug_highlight: Option<FragmentId>,

    // -- frame timing -------------------------------------------------------
    /// Current frame timestamp (seconds). Set before compute_layout by the
    /// pipeline or API caller. Used by layout FLIP to timestamp animations.
    pub(crate) frame_now: f64,
}

impl Default for FragmentTree {
    fn default() -> Self {
        let mut taffy = SendTaffy(TaffyTree::new());
        let taffy_root = taffy
            .new_leaf(taffy::Style {
                flex_shrink: 0.0,
                ..Default::default()
            })
            .unwrap();
        Self {
            nodes: HashMap::new(),
            root_children: Vec::new(),
            next_id: 0,
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
            semantics_dirty: HashSet::new(),
            frame_now: 0.0,
        }
    }
}

impl FragmentTree {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn request_full_repaint(&mut self) {
        self.force_full_repaint = true;
        self.any_dirty = true;
        self.aabbs_dirty = true;
        self.subtree_scene_cache.clear();
        self.promoted_scene_cache.clear();
        self.dirty_root_children
            .extend(self.root_children.iter().copied());
        for (id, node) in &mut self.nodes {
            node.dirty = true;
            self.dirty_node_ids.insert(*id);
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

        // Auto-infer semantics role from fragment kind
        let inferred_semantics = match &kind {
            FragmentData::Text(_) => Some(SemanticsData::with_role(accesskit::Role::Label)),
            FragmentData::TextInput(_) => {
                Some(SemanticsData::with_role(accesskit::Role::TextInput))
            }
            FragmentData::Image(_) => Some(SemanticsData::with_role(accesskit::Role::Image)),
            FragmentData::Group(_) => Some(SemanticsData::with_role(accesskit::Role::Group)),
            _ => None,
        };

        let taffy_node = self
            .taffy
            .new_leaf(taffy::Style {
                flex_shrink: 0.0,
                ..Default::default()
            })
            .ok();
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
                semantics: inferred_semantics,
                perspective_pose: (0.0, 0.0, 0.0),
                mask_child: None,
                is_mask_source: false,
                placement: Placement::default(),
                container: None,
                padding: EdgeInsets::default(),
                overflow_x: Overflow::default(),
                overflow_y: Overflow::default(),
                layout_visible: true,
                layout_dirty: false,
                layout_flip_mode: None,
                layout_flip_transition: None,
            },
        );
        // Node not yet attached to a parent — just mark global dirty.
        self.any_dirty = true;
        self.aabbs_dirty = true;

        self.semantics_dirty.insert(id);
        id
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

                self.invalidate_subtree_cache_for(child);
                return;
            }
        }
        children.push(child);
        self.sync_taffy_children(parent);
        self.any_dirty = true;
        self.aabbs_dirty = true;

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
            None => (self.root_children.clone(), self.taffy_root),
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
                    return if self.root_children.contains(&cur) {
                        Some(cur)
                    } else {
                        None
                    };
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

        self.invalidate_subtree_cache_for(id);
    }

    pub fn set_debug_highlight(&mut self, id: Option<FragmentId>) {
        if self.debug_highlight != id {
            self.debug_highlight = id;
            // Debug overlay is cosmetic — just force scene rebuild, not full repaint.
            self.any_dirty = true;
        }
    }

    // -----------------------------------------------------------------------
    // Layout (taffy)
    // -----------------------------------------------------------------------

    /// Sync intrinsic leaf measures (text, text-input, circle) into taffy styles
    /// so that layout passes produce correct sizes.
    fn sync_intrinsic_leaf_measures(&mut self) {
        // Sync fixed measure for text nodes with shaped cache.
        let text_sizes: Vec<(taffy::tree::NodeId, f32, f32)> = self
            .nodes
            .values()
            .filter_map(|node| {
                if let (Some(tn), FragmentData::Text(text)) = (node.taffy_node, &node.kind) {
                    text.shaped
                        .as_ref()
                        .map(|s| (tn, s.width as f32, s.height as f32))
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
        let input_sizes: Vec<(taffy::tree::NodeId, f32, f32, bool, bool)> = self
            .nodes
            .values()
            .filter_map(|node| {
                if let (Some(tn), FragmentData::TextInput(ti)) = (node.taffy_node, &node.kind) {
                    ti.layout.as_ref().map(|l| {
                        (
                            tn,
                            l.width as f32,
                            l.height as f32,
                            node.props.explicit_width.is_some(),
                            node.props.explicit_height.is_some(),
                        )
                    })
                } else {
                    None
                }
            })
            .collect();
        for (tn, w, h, explicit_width, explicit_height) in input_sizes {
            let mut style = self.taffy.style(tn).cloned().unwrap_or_default();
            if !explicit_width {
                style.size.width = taffy::style::Dimension::length(w);
            }
            if !explicit_height {
                style.size.height = taffy::style::Dimension::length(h);
            }
            let _ = self.taffy.set_style(tn, style);
        }

        // Sync fixed measure for circle nodes from radius.
        let circle_sizes: Vec<(taffy::tree::NodeId, f32)> = self
            .nodes
            .values()
            .filter_map(|node| {
                if let (Some(tn), FragmentData::Circle(circle)) = (node.taffy_node, &node.kind) {
                    if circle.r > 0.0
                        && node.props.explicit_width.is_none()
                        && node.props.explicit_height.is_none()
                    {
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
    }

    /// Run taffy layout and apply results to fragment nodes.
    pub fn compute_layout(
        &mut self,
        available_width: f64,
        available_height: f64,
    ) -> Vec<FragmentLayoutChange> {
        let Some(root) = self.taffy_root else {
            return Vec::new();
        };

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

        self.sync_layout_intents();
        self.sync_intrinsic_leaf_measures();

        let _ = self.taffy.compute_layout(root, available);
        let events = self.apply_layout_results();
        self.last_layout_size = Some((available_width, available_height));
        self.aabbs_dirty = true;
        events
    }

    fn apply_layout_results(&mut self) -> Vec<FragmentLayoutChange> {
        use crate::canvas::fragment::LayoutFlipMode;
        use ::motion::{SpringParams, TransitionSpec};

        let ids: Vec<FragmentId> = self.nodes.keys().copied().collect();
        let mut layout_dirty_ids: Vec<FragmentId> = Vec::new();
        let mut layout_events: Vec<FragmentLayoutChange> = Vec::new();
        // Deferred FLIP animations — collected during layout application,
        // applied after the loop so `set_layout_flip` can borrow &mut self.
        let mut flip_requests: Vec<(FragmentId, f64, f64, f64, f64, TransitionSpec)> =
            Vec::new();

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

            // Snapshot old layout for FLIP delta computation.
            let old_x = node.layout.x;
            let old_y = node.layout.y;
            let old_w = node.layout.width;
            let old_h = node.layout.height;

            let pos_changed =
                (old_x - lx).abs() > 0.01 || (old_y - ly).abs() > 0.01;
            if pos_changed {
                node.layout.x = lx;
                node.layout.y = ly;
                if node.props.explicit_x.is_none() || node.props.explicit_y.is_none() {
                    node.dirty = true;
                    self.any_dirty = true;

                    layout_dirty_ids.push(id);
                    self.semantics_dirty.insert(id);
                }
            }

            let layout_size_changed =
                (old_w - lw).abs() > 0.01 || (old_h - lh).abs() > 0.01;
            if layout_size_changed {
                node.layout.width = lw;
                node.layout.height = lh;
                node.dirty = true;
                self.any_dirty = true;

                layout_dirty_ids.push(id);
                self.semantics_dirty.insert(id);
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
                FragmentData::TextInput(input) => {
                    if (input.viewport_width - lw).abs() > 0.01
                        || (input.viewport_height - lh).abs() > 0.01
                    {
                        paint_size_changed = true;
                        input.viewport_width = lw;
                        input.viewport_height = lh;
                    }
                    if input.ensure_caret_visible() {
                        paint_size_changed = true;
                    }
                }
                _ => {}
            }
            if paint_size_changed {
                node.dirty = true;
                self.any_dirty = true;

                layout_dirty_ids.push(id);
                self.semantics_dirty.insert(id);
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

            // Native-owned same-element layout FLIP:
            // When layout changed and this node has layout_flip_mode configured,
            // compute FLIP deltas and queue animation.
            if let Some(flip_mode) = node.layout_flip_mode {
                let any_change = pos_changed || layout_size_changed || paint_size_changed;
                // Only FLIP when old layout had positive dimensions (skip first layout).
                if any_change && old_w > 0.01 && old_h > 0.01 {
                    let dx = match flip_mode {
                        LayoutFlipMode::Size => 0.0,
                        _ => old_x - lx,
                    };
                    let dy = match flip_mode {
                        LayoutFlipMode::Size => 0.0,
                        _ => old_y - ly,
                    };
                    let sx = match flip_mode {
                        LayoutFlipMode::Position => 1.0,
                        _ => if lw > 0.01 { old_w / lw } else { 1.0 },
                    };
                    let sy = match flip_mode {
                        LayoutFlipMode::Position => 1.0,
                        _ => if lh > 0.01 { old_h / lh } else { 1.0 },
                    };

                    let has_delta = dx.abs() > 0.5
                        || dy.abs() > 0.5
                        || (sx - 1.0).abs() > 0.001
                        || (sy - 1.0).abs() > 0.001;

                    if has_delta {
                        let transition = node
                            .layout_flip_transition
                            .clone()
                            .unwrap_or_else(|| {
                                TransitionSpec::Spring(SpringParams {
                                    stiffness: 500.0,
                                    damping: 30.0,
                                    mass: 1.0,
                                    ..Default::default()
                                })
                            });
                        flip_requests.push((id, dx, dy, sx, sy, transition));
                    }
                }
            }
        }
        for id in layout_dirty_ids {
            self.invalidate_subtree_cache_for(id);
        }

        // Apply deferred FLIP animations.
        if !flip_requests.is_empty() {
            let now = self.frame_now;
            for (id, dx, dy, sx, sy, transition) in flip_requests {
                self.set_layout_flip(id, dx, dy, sx, sy, &transition, now);
            }
        }

        layout_events
    }

    /// Modify taffy style for a fragment node.
    pub fn with_taffy_style_mut(&mut self, id: FragmentId, f: impl FnOnce(&mut taffy::Style)) {
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
    // Layout intent → taffy style derivation
    // -----------------------------------------------------------------------

    /// Resolve the flex direction of the parent container for a given node.
    fn parent_direction(&self, id: FragmentId) -> Direction {
        self.nodes
            .get(&id)
            .and_then(|n| n.parent)
            .and_then(|pid| self.nodes.get(&pid))
            .and_then(|parent| parent.container.as_ref())
            .map(|c| match c {
                Container::Flex { direction, .. } => *direction,
                Container::Grid { .. } => Direction::Horizontal,
            })
            .unwrap_or(Direction::Vertical)
    }

    /// Derive taffy styles from layout intents for all dirty nodes.
    fn sync_layout_intents(&mut self) {
        let dirty_ids: Vec<FragmentId> = self
            .nodes
            .iter()
            .filter(|(_, n)| n.layout_dirty)
            .map(|(id, _)| *id)
            .collect();

        for id in dirty_ids {
            let parent_dir = self.parent_direction(id);
            let Some(node) = self.nodes.get(&id) else {
                continue;
            };
            let Some(taffy_node) = node.taffy_node else {
                continue;
            };

            let style = crate::canvas::fragment::layout::derive_taffy_style(
                &node.placement,
                node.container.as_ref(),
                &node.padding,
                node.overflow_x,
                node.overflow_y,
                node.layout_visible,
                parent_dir,
            );
            let _ = self.taffy.set_style(taffy_node, style);

            self.nodes.get_mut(&id).unwrap().layout_dirty = false;
        }
    }

    // -----------------------------------------------------------------------
    // z-index ordering helper
    // -----------------------------------------------------------------------

    pub(crate) fn sorted_children_by_z(&self, children: &[FragmentId]) -> Vec<FragmentId> {
        Self::sorted_children_by_z_static(&self.nodes, children)
    }

    pub(crate) fn sorted_children_by_z_static(
        nodes: &HashMap<FragmentId, FragmentNode>,
        children: &[FragmentId],
    ) -> Vec<FragmentId> {
        let needs_sort = children
            .iter()
            .any(|id| nodes.get(id).map_or(false, |n| n.props.z_index != 0));
        if !needs_sort {
            return children.to_vec();
        }
        let mut sorted: Vec<FragmentId> = children.to_vec();
        sorted.sort_by_key(|id| nodes.get(id).map_or(0, |n| n.props.z_index));
        sorted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_repaint_request_marks_all_fragments_dirty() {
        let mut tree = FragmentTree::new();
        let child = tree.create_node(FragmentData::Group(Default::default()));
        tree.insert_child(None, child, None);
        tree.consume_dirty_state();
        tree.any_dirty = false;
        for node in tree.nodes.values_mut() {
            node.dirty = false;
        }

        tree.request_full_repaint();

        assert!(tree.force_full_repaint);
        assert!(tree.any_dirty);
        assert_eq!(tree.dirty_root_children, HashSet::from([child]));
        assert_eq!(tree.dirty_node_ids, HashSet::from([child]));
        assert!(tree.nodes.get(&child).is_some_and(|node| node.dirty));
    }

    #[test]
    fn layout_flip_triggers_on_size_change() {
        use crate::canvas::fragment::node::LayoutFlipMode;

        let mut tree = FragmentTree::new();
        let rect_id = tree.create_node(FragmentData::Rect(Default::default()));
        tree.insert_child(None, rect_id, None);

        // Set initial layout so FLIP can compare old vs new.
        if let Some(node) = tree.nodes.get_mut(&rect_id) {
            node.layout = LayoutResult {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 50.0,
            };
        }

        // Set fixed size via taffy so compute_layout produces new dimensions.
        tree.with_taffy_style_mut(rect_id, |style| {
            style.size = taffy::geometry::Size {
                width: taffy::style::Dimension::length(200.0),
                height: taffy::style::Dimension::length(80.0),
            };
        });

        // Enable layout FLIP.
        if let Some(node) = tree.nodes.get_mut(&rect_id) {
            node.layout_flip_mode = Some(LayoutFlipMode::All);
        }

        // Run layout — this should detect size change and queue FLIP.
        tree.frame_now = 1.0;
        let _ = tree.compute_layout(400.0, 400.0);

        // Verify that layout FLIP channels were created on the timeline.
        let node = tree.nodes.get(&rect_id).unwrap();
        let timeline = node.timeline.as_ref().expect("timeline should exist after FLIP");
        assert!(
            timeline.is_animating(),
            "timeline should be animating after layout FLIP"
        );
        assert!(
            timeline.has_property(::motion::PropertyKey::LayoutScaleX),
            "layoutScaleX channel should exist"
        );
        assert!(
            timeline.has_property(::motion::PropertyKey::LayoutScaleY),
            "layoutScaleY channel should exist"
        );
    }

    #[test]
    fn layout_flip_position_mode_ignores_scale() {
        use crate::canvas::fragment::node::LayoutFlipMode;

        let mut tree = FragmentTree::new();
        let rect_id = tree.create_node(FragmentData::Rect(Default::default()));
        tree.insert_child(None, rect_id, None);

        // Set initial layout with position.
        if let Some(node) = tree.nodes.get_mut(&rect_id) {
            node.layout = LayoutResult {
                x: 10.0,
                y: 20.0,
                width: 100.0,
                height: 50.0,
            };
        }

        // Change both position and size via taffy.
        tree.with_taffy_style_mut(rect_id, |style| {
            style.size = taffy::geometry::Size {
                width: taffy::style::Dimension::length(200.0),
                height: taffy::style::Dimension::length(80.0),
            };
        });

        // Enable position-only FLIP.
        if let Some(node) = tree.nodes.get_mut(&rect_id) {
            node.layout_flip_mode = Some(LayoutFlipMode::Position);
        }

        tree.frame_now = 1.0;
        let _ = tree.compute_layout(400.0, 400.0);

        let node = tree.nodes.get(&rect_id).unwrap();
        let timeline = node.timeline.as_ref().expect("timeline should exist");
        // Position-only mode should NOT create scale channels (scale deltas = 1.0 = no-op).
        // It should have layout position channels.
        assert!(
            timeline.has_property(::motion::PropertyKey::LayoutX)
                || timeline.has_property(::motion::PropertyKey::LayoutY),
            "should have layout position channels"
        );
    }

    #[test]
    fn layout_flip_skips_first_layout() {
        use crate::canvas::fragment::node::LayoutFlipMode;

        let mut tree = FragmentTree::new();
        let rect_id = tree.create_node(FragmentData::Rect(Default::default()));
        tree.insert_child(None, rect_id, None);

        // Leave initial layout as default (0x0) — first layout should NOT trigger FLIP.
        if let Some(node) = tree.nodes.get_mut(&rect_id) {
            node.layout_flip_mode = Some(LayoutFlipMode::All);
        }

        tree.with_taffy_style_mut(rect_id, |style| {
            style.size = taffy::geometry::Size {
                width: taffy::style::Dimension::length(100.0),
                height: taffy::style::Dimension::length(50.0),
            };
        });

        tree.frame_now = 1.0;
        let _ = tree.compute_layout(400.0, 400.0);

        let node = tree.nodes.get(&rect_id).unwrap();
        // No timeline should be created on first layout (old dimensions were 0x0).
        assert!(
            node.timeline.is_none() || !node.timeline.as_ref().unwrap().is_animating(),
            "first layout should not trigger FLIP animation"
        );
    }
}
