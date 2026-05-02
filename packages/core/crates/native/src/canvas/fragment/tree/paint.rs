use std::collections::{HashMap, HashSet};

use crate::canvas::fragment::node::{FragmentData, FragmentNode};
use crate::canvas::fragment::paint::{is_axis_aligned_affine, transform_local_bounds_to_world, PaintCollector};
use crate::canvas::fragment::types::{
    push_fragment_layer, FragmentClipShape, FragmentId, FragmentLayerKey,
    PaintChunk, PaintPlan, PromotedLayer,
};
use crate::renderer::compositor::effects::{BackdropBlurEffect, InnerShadowEffect};
use crate::vello::peniko::kurbo::{Affine, BezPath, Rect, Shape, Stroke, Vec2};
use crate::vello::peniko::{Color, Fill};
use crate::vello::{PaintScene, Scene};

use super::dirty_rect::rects_intersect;
use super::FragmentTree;

impl FragmentTree {
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
}
