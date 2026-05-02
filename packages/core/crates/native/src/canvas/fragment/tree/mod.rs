mod dirty_rect;
mod motion;
mod paint;

pub use dirty_rect::DirtyRectResult;
pub(crate) use dirty_rect::scene_bounds;

use std::collections::{HashMap, HashSet};

use taffy::prelude::*;

use crate::canvas::fragment::node::{FragmentData, FragmentNode, FragmentProps, LayoutResult, SemanticsData};
use crate::canvas::fragment::types::{
    FragmentId, FragmentLayerKey, FragmentLayoutChange,
    FragmentListeners, FRAGMENT_LAYER_KEY_BASE,
};
use crate::vello::peniko::kurbo::{Rect, Vec2};
use crate::vello::Scene;

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
    /// Fragment IDs whose semantics data changed (role, label, bounds, etc.).
    pub(crate) semantics_dirty: HashSet<FragmentId>,
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
            semantics_dirty: HashSet::new(),
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

        // Auto-infer semantics role from fragment kind
        let inferred_semantics = match &kind {
            FragmentData::Text(_) => Some(SemanticsData::with_role(accesskit::Role::Label)),
            FragmentData::TextInput(_) => Some(SemanticsData::with_role(accesskit::Role::TextInput)),
            FragmentData::Image(_) => Some(SemanticsData::with_role(accesskit::Role::Image)),
            FragmentData::Group(_) => Some(SemanticsData::with_role(accesskit::Role::Group)),
            _ => None,
        };

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
                semantics: inferred_semantics,
            },
        );
        // Node not yet attached to a parent — just mark global dirty.
        self.any_dirty = true;
        self.aabbs_dirty = true;
        self.cached_scene = None;
        self.semantics_dirty.insert(id);
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
                    self.semantics_dirty.insert(id);
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
                _ => {}
            }
            if paint_size_changed {
                node.dirty = true;
                self.any_dirty = true;
                self.cached_scene = None;
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

}
