use super::super::vello::peniko::kurbo::{Affine, Point, Rect};
use super::paint::{transform_local_bounds_to_world, union_rect};
use super::tree::FragmentTree;
use super::types::FragmentId;

// ---------------------------------------------------------------------------
// AABB recomputation
// ---------------------------------------------------------------------------

impl FragmentTree {
    pub(crate) fn ensure_aabbs(&mut self) {
        if !self.aabbs_dirty {
            return;
        }
        let root_children = self.root_children.clone();
        for &child_id in &root_children {
            self.recompute_aabb(child_id, Affine::IDENTITY);
        }
        self.aabbs_dirty = false;
    }

    fn recompute_aabb(&mut self, id: FragmentId, parent_transform: Affine) -> Option<Rect> {
        let (local_bounds, paint_outset, local_transform, children) = {
            let node = self.nodes.get(&id)?;
            (
                node.effective_bounds(),
                node.paint_outset(),
                parent_transform * node.local_transform(),
                node.children.clone(),
            )
        };

        // Inflate local bounds by paint outset (stroke, shadow, border extend
        // beyond the content rect) so dirty rects cover the full paint extent.
        let inflated = local_bounds.map(|lb| {
            if paint_outset > 0.0 {
                lb.inflate(paint_outset, paint_outset)
            } else {
                lb
            }
        });
        let world_aabb = inflated.map(|lb| transform_local_bounds_to_world(lb, local_transform));

        let child_transform = match self.scroll_offsets.get(&id) {
            Some(s) => local_transform * Affine::translate((-s.x, -s.y)),
            None => local_transform,
        };

        let mut subtree = world_aabb;
        for &child_id in &children {
            if let Some(child_subtree) = self.recompute_aabb(child_id, child_transform) {
                subtree = Some(union_rect(subtree, child_subtree));
            }
        }

        if let Some(node) = self.nodes.get_mut(&id) {
            node.world_aabb = world_aabb;
            node.subtree_aabb = subtree;
        }

        subtree
    }
}

// ---------------------------------------------------------------------------
// Hit test — reverse paint order (on FragmentTree)
// ---------------------------------------------------------------------------

impl FragmentTree {
    pub fn hit_test(&mut self, point: (f64, f64)) -> Option<FragmentId> {
        self.ensure_aabbs();
        let world_point = Point::new(point.0, point.1);
        self.hit_test_children(&self.root_children.clone(), Affine::IDENTITY, point, world_point)
    }

    fn hit_test_children(
        &self,
        children: &[FragmentId],
        parent_transform: Affine,
        point: (f64, f64),
        world_point: Point,
    ) -> Option<FragmentId> {
        let sorted = self.sorted_children_by_z(children);
        for &child_id in sorted.iter().rev() {
            if let Some(hit) = self.hit_test_node(child_id, parent_transform, point, world_point) {
                return Some(hit);
            }
        }
        None
    }

    fn hit_test_node(
        &self,
        id: FragmentId,
        parent_transform: Affine,
        point: (f64, f64),
        world_point: Point,
    ) -> Option<FragmentId> {
        let node = self.node(id)?;

        if !node.props.visible || !node.props.pointer_events {
            return None;
        }

        // Prune: if subtree AABB doesn't contain the point, skip entirely.
        if let Some(subtree_aabb) = node.subtree_aabb {
            if !subtree_aabb.contains(world_point) {
                return None;
            }
        }

        let transform = parent_transform * node.local_transform();

        if node.props.clip {
            if let Some(clip_rect) = node.effective_bounds() {
                let inverse = transform.inverse();
                let local = inverse * Point::new(point.0, point.1);
                if !clip_rect.contains(local) {
                    return None;
                }
            }
        }

        let child_transform = match self.scroll_offsets.get(&id) {
            Some(s) => transform * Affine::translate((-s.x, -s.y)),
            None => transform,
        };
        let children = node.children.clone();
        if let Some(hit) = self.hit_test_children(&children, child_transform, point, world_point) {
            return Some(hit);
        }

        // Prune: check world AABB before computing inverse transform.
        if let Some(world_aabb) = node.world_aabb {
            if !world_aabb.contains(world_point) {
                return None;
            }
        }

        if let Some(bounds) = node.effective_bounds() {
            let inverse = transform.inverse();
            let local = inverse * Point::new(point.0, point.1);
            if bounds.contains(local) {
                return Some(id);
            }
        }

        None
    }

    // -----------------------------------------------------------------------
    // Focus management
    // -----------------------------------------------------------------------

    /// Collect focusable fragment IDs in DFS (pre-order) traversal order.
    fn focusable_ids(&self) -> Vec<FragmentId> {
        let mut result = Vec::new();
        self.collect_focusable(&self.root_children, &mut result);
        result
    }

    fn collect_focusable(&self, children: &[FragmentId], out: &mut Vec<FragmentId>) {
        for &id in children {
            if let Some(node) = self.nodes.get(&id) {
                if node.props.visible && node.props.focusable {
                    out.push(id);
                }
                self.collect_focusable(&node.children, out);
            }
        }
    }

    /// Move focus to the next (forward=true) or previous focusable fragment.
    /// Returns (old_focused, new_focused). Returns (old, None) when focus
    /// escapes the fragment tree (caller should let Qt handle Tab).
    pub fn focus_next(&mut self, forward: bool) -> (Option<FragmentId>, Option<FragmentId>) {
        let ids = self.focusable_ids();
        if ids.is_empty() {
            let old = self.focused.take();
            return (old, None);
        }

        let old = self.focused;
        let current_idx = old.and_then(|f| ids.iter().position(|&id| id == f));

        let next = match current_idx {
            Some(idx) => {
                if forward {
                    if idx + 1 < ids.len() {
                        Some(ids[idx + 1])
                    } else {
                        None // escape forward
                    }
                } else if idx > 0 {
                    Some(ids[idx - 1])
                } else {
                    None // escape backward
                }
            }
            None => {
                // No current focus — enter from start or end.
                if forward {
                    Some(ids[0])
                } else {
                    Some(*ids.last().unwrap())
                }
            }
        };

        self.focused = next;
        (old, next)
    }

    /// Focus a specific fragment (or its nearest focusable ancestor).
    /// Returns the previously focused fragment if focus actually changed.
    pub fn focus_fragment(&mut self, id: FragmentId) -> Option<FragmentId> {
        let old = self.focused;
        let target = self.find_focusable_ancestor(id);
        if target == old {
            return None;
        }
        self.focused = target;
        old
    }

    fn find_focusable_ancestor(&self, id: FragmentId) -> Option<FragmentId> {
        let mut current = Some(id);
        while let Some(cid) = current {
            if let Some(node) = self.nodes.get(&cid) {
                if node.props.focusable && node.props.visible {
                    return Some(cid);
                }
                current = node.parent;
            } else {
                break;
            }
        }
        None
    }

    /// Clear focus. Returns the previously focused fragment.
    pub fn blur(&mut self) -> Option<FragmentId> {
        self.focused.take()
    }

    pub fn focused(&self) -> Option<FragmentId> {
        self.focused
    }

    /// Compute the accumulated world transform for a fragment by walking parents.
    pub fn world_transform(&self, id: FragmentId) -> Affine {
        let mut chain = Vec::new();
        let mut current = Some(id);
        while let Some(cid) = current {
            let Some(node) = self.nodes.get(&cid) else { break };
            chain.push(node.local_transform());
            current = node.parent;
        }
        chain.iter().rev().fold(Affine::IDENTITY, |acc, t| acc * *t)
    }
}
