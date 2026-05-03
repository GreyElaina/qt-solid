use std::collections::HashMap;

use taffy::prelude::AvailableSpace;

use crate::canvas::fragment::node::apply_sampled_pose_to_fragment;
use crate::canvas::fragment::types::FragmentId;
use crate::vello::peniko::kurbo::{Rect, Vec2};

use super::FragmentTree;

impl FragmentTree {
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
        let mut timeline = node
            .timeline
            .take()
            .unwrap_or_else(motion::NodeTimeline::new);
        timeline.set_targets(targets, default_transition, per_property, now, delay_secs);
        let (sampled, animating) = timeline.sample_pose(now);
        apply_sampled_pose_to_fragment(node, &sampled, &timeline);
        if !animating {
            timeline.gc_completed();
        }
        node.timeline = Some(timeline);
        self.any_dirty = true;
        self.aabbs_dirty = true;
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
        let mut timeline = node
            .timeline
            .take()
            .unwrap_or_else(motion::NodeTimeline::new);
        timeline.set_targets_keyframes(
            targets,
            times,
            default_transition,
            per_property,
            now,
            delay_secs,
        );
        let (sampled, animating) = timeline.sample_pose(now);
        apply_sampled_pose_to_fragment(node, &sampled, &timeline);
        if !animating {
            timeline.gc_completed();
        }
        node.timeline = Some(timeline);
        self.any_dirty = true;
        self.aabbs_dirty = true;
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
            let animated_ids: Vec<FragmentId> = self
                .nodes
                .values()
                .filter(|n| n.timeline.as_ref().map_or(false, |t| t.is_animating()) && !n.promoted)
                .map(|n| n.id)
                .collect();
            self.any_dirty = true;
            self.aabbs_dirty = true;
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
            self.invalidate_subtree_cache_for(id);
        }
    }

    /// Drive scroll offset through the motion timeline (instant transition).
    /// This accumulates velocity via prev_target for later spring release.
    pub fn drive_scroll_motion(&mut self, id: FragmentId, x: f64, y: f64, now: f64) {
        let Some(node) = self.nodes.get_mut(&id) else {
            return;
        };
        let mut timeline = node
            .timeline
            .take()
            .unwrap_or_else(motion::NodeTimeline::new);
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
        let mut timeline = node
            .timeline
            .take()
            .unwrap_or_else(motion::NodeTimeline::new);
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
        self.invalidate_subtree_cache_for(id);
        animating
    }

    /// Get the content size of a fragment node from its taffy layout.
    pub fn get_content_size(&self, id: FragmentId) -> Option<(f64, f64)> {
        let node = self.nodes.get(&id)?;
        let taffy_node = node.taffy_node?;
        let layout = self.taffy.layout(taffy_node).ok()?;
        Some((
            layout.content_size.width as f64,
            layout.content_size.height as f64,
        ))
    }

    /// Compute intrinsic (max-content) size of the fragment tree.
    /// Temporarily sets the taffy root to auto sizing, syncs leaf measures,
    /// and runs a layout pass with `MaxContent` so the root shrink-wraps.
    pub fn compute_intrinsic_size(&mut self) -> Option<(f64, f64)> {
        let root = self.taffy_root?;

        self.sync_intrinsic_leaf_measures();

        // Temporarily set root to auto so it shrink-wraps to content.
        let mut root_style = self.taffy.style(root).cloned().unwrap_or_default();
        root_style.size = taffy::geometry::Size {
            width: taffy::style::Dimension::auto(),
            height: taffy::style::Dimension::auto(),
        };
        root_style.flex_shrink = 0.0;
        let _ = self.taffy.set_style(root, root_style);

        let available = taffy::geometry::Size {
            width: AvailableSpace::MaxContent,
            height: AvailableSpace::MaxContent,
        };
        let _ = self.taffy.compute_layout(root, available);
        let layout = self.taffy.layout(root).ok()?;
        Some((
            layout.size.width.ceil() as f64,
            layout.size.height.ceil() as f64,
        ))
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
        let mut timeline = node
            .timeline
            .take()
            .unwrap_or_else(motion::NodeTimeline::new);

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
        self.invalidate_subtree_cache_for(id);
        animating
    }
}
