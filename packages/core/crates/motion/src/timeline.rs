use std::collections::HashMap;

use crate::channel::{AnimationChannel, ChannelState};
use crate::spring::SpringParams;
use crate::transition::TransitionSpec;

/// Property keys for motion animation channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PropertyKey {
    X,
    Y,
    ScaleX,
    ScaleY,
    Rotate,
    Opacity,
    OriginX,
    OriginY,
    // Layout FLIP channels
    LayoutX,
    LayoutY,
    LayoutScaleX,
    LayoutScaleY,
    // Paint channels
    BackgroundR,
    BackgroundG,
    BackgroundB,
    BackgroundA,
    BorderRadius,
    BlurRadius,
    ShadowOffsetX,
    ShadowOffsetY,
    ShadowBlurRadius,
    ShadowR,
    ShadowG,
    ShadowB,
    ShadowA,
    // Scroll channels
    ScrollX,
    ScrollY,
}

impl PropertyKey {
    /// Default resting value for each property.
    pub fn default_value(self) -> f64 {
        match self {
            Self::X | Self::Y | Self::Rotate | Self::LayoutX | Self::LayoutY => 0.0,
            Self::ScaleX
            | Self::ScaleY
            | Self::Opacity
            | Self::LayoutScaleX
            | Self::LayoutScaleY => 1.0,
            Self::OriginX | Self::OriginY => 0.5,
            Self::BackgroundR | Self::BackgroundG | Self::BackgroundB => 0.0,
            Self::BackgroundA => 1.0,
            Self::BorderRadius | Self::BlurRadius => 0.0,
            Self::ShadowOffsetX | Self::ShadowOffsetY | Self::ShadowBlurRadius => 0.0,
            Self::ShadowR | Self::ShadowG | Self::ShadowB => 0.0,
            Self::ShadowA => 0.0,
            Self::ScrollX | Self::ScrollY => 0.0,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::X => "x", Self::Y => "y",
            Self::ScaleX => "scaleX", Self::ScaleY => "scaleY",
            Self::Rotate => "rotate", Self::Opacity => "opacity",
            Self::OriginX => "originX", Self::OriginY => "originY",
            Self::LayoutX => "layoutX", Self::LayoutY => "layoutY",
            Self::LayoutScaleX => "layoutScaleX", Self::LayoutScaleY => "layoutScaleY",
            Self::BackgroundR => "backgroundR", Self::BackgroundG => "backgroundG",
            Self::BackgroundB => "backgroundB", Self::BackgroundA => "backgroundA",
            Self::BorderRadius => "borderRadius", Self::BlurRadius => "blurRadius",
            Self::ShadowOffsetX => "shadowOffsetX", Self::ShadowOffsetY => "shadowOffsetY",
            Self::ShadowBlurRadius => "shadowBlurRadius",
            Self::ShadowR => "shadowR", Self::ShadowG => "shadowG",
            Self::ShadowB => "shadowB", Self::ShadowA => "shadowA",
            Self::ScrollX => "scrollX", Self::ScrollY => "scrollY",
        }
    }

    /// Whether animating this property requires a scene repaint (not just compositor pose).
    pub fn requires_repaint(self) -> bool {
        matches!(
            self,
            Self::BackgroundR
                | Self::BackgroundG
                | Self::BackgroundB
                | Self::BackgroundA
                | Self::BorderRadius
                | Self::BlurRadius
                | Self::ShadowOffsetX
                | Self::ShadowOffsetY
                | Self::ShadowBlurRadius
                | Self::ShadowR
                | Self::ShadowG
                | Self::ShadowB
                | Self::ShadowA
        )
    }
}

/// Assembled pose from sampled channels.
#[derive(Debug, Clone, Copy)]
pub struct SampledPose {
    pub x: f64,
    pub y: f64,
    pub scale_x: f64,
    pub scale_y: f64,
    pub rotate_deg: f64,
    pub opacity: f64,
    pub origin_x: f64,
    pub origin_y: f64,
    // Layout FLIP
    pub layout_x: f64,
    pub layout_y: f64,
    pub layout_scale_x: f64,
    pub layout_scale_y: f64,
    // Paint
    pub background_r: f64,
    pub background_g: f64,
    pub background_b: f64,
    pub background_a: f64,
    pub border_radius: f64,
    pub blur_radius: f64,
    pub shadow_offset_x: f64,
    pub shadow_offset_y: f64,
    pub shadow_blur_radius: f64,
    pub shadow_r: f64,
    pub shadow_g: f64,
    pub shadow_b: f64,
    pub shadow_a: f64,
    // Scroll
    pub scroll_x: f64,
    pub scroll_y: f64,
}

impl Default for SampledPose {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            rotate_deg: 0.0,
            opacity: 1.0,
            origin_x: 0.5,
            origin_y: 0.5,
            layout_x: 0.0,
            layout_y: 0.0,
            layout_scale_x: 1.0,
            layout_scale_y: 1.0,
            background_r: 0.0,
            background_g: 0.0,
            background_b: 0.0,
            background_a: 1.0,
            border_radius: 0.0,
            blur_radius: 0.0,
            shadow_offset_x: 0.0,
            shadow_offset_y: 0.0,
            shadow_blur_radius: 0.0,
            shadow_r: 0.0,
            shadow_g: 0.0,
            shadow_b: 0.0,
            shadow_a: 0.0,
            scroll_x: 0.0,
            scroll_y: 0.0,
        }
    }
}

/// Per-node animation timeline managing all property channels.
#[derive(Debug, Clone, Default)]
pub struct NodeTimeline {
    channels: HashMap<PropertyKey, AnimationChannel>,
    /// Snapshot of resting values (what animate prop currently declares).
    /// Used to know the "current target" for properties without active channels.
    resting: HashMap<PropertyKey, f64>,
}

impl NodeTimeline {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a single property target. Starts or interrupts the channel.
    pub fn set_target(
        &mut self,
        key: PropertyKey,
        value: f64,
        transition: TransitionSpec,
        now: f64,
        delay_secs: f64,
    ) {
        let origin = self.current_resting(key);
        self.set_target_keyframes(key, vec![origin, value], None, transition, now, delay_secs);
    }

    /// Set a keyframe animation for a single property.
    pub fn set_target_keyframes(
        &mut self,
        key: PropertyKey,
        values: Vec<f64>,
        times: Option<Vec<f64>>,
        transition: TransitionSpec,
        now: f64,
        delay_secs: f64,
    ) {
        let final_value = *values.last().unwrap();
        self.resting.insert(key, final_value);

        if let Some(existing) = self.channels.get_mut(&key) {
            if existing.state() == ChannelState::Running {
                // Retarget: collapse to simple A→B from current to new final
                let new_channel = existing.retarget(final_value, transition, now);
                self.channels.insert(key, new_channel);
                return;
            }
            if (existing.target() - final_value).abs() < 1e-10 && values.len() == 2 {
                return;
            }
        }

        // Capture prev_target from the old channel (if any) so we can infer
        // implicit velocity for a spring transition (driven mode).
        // Velocity = (current_target - prev_target) / (current_time - prev_time)
        let inferred_velocity: Option<f64> = self.channels.get(&key).and_then(|old| {
            let (prev_val, prev_time) = old.prev_target?;
            let dt = old.started_at() - prev_time;
            if dt > 0.0 && dt < 0.5 {
                Some((old.target() - prev_val) / dt)
            } else {
                None
            }
        });
        let prev_target_for_chain = self.channels.get(&key).map(|old| (old.target(), old.started_at()));

        // For simple 2-value case, use current resting as origin if values[0] matches default
        let transition = match (inferred_velocity, transition) {
            (Some(vel), TransitionSpec::Spring(params)) if params.initial_velocity == 0.0 => {
                TransitionSpec::Spring(SpringParams {
                    initial_velocity: vel,
                    ..params
                })
            }
            (_, t) => t,
        };

        let mut channel = AnimationChannel::new_keyframes(values, times, transition, now, delay_secs);
        channel.prev_target = prev_target_for_chain;
        self.channels.insert(key, channel);
    }

    /// Batch set targets from a property map.
    pub fn set_targets(
        &mut self,
        targets: &[(PropertyKey, f64)],
        default_transition: &TransitionSpec,
        per_property: &HashMap<PropertyKey, TransitionSpec>,
        now: f64,
        delay_secs: f64,
    ) {
        for &(key, value) in targets {
            let transition = per_property
                .get(&key)
                .cloned()
                .unwrap_or_else(|| default_transition.clone());
            self.set_target(key, value, transition, now, delay_secs);
        }
    }

    /// Batch set keyframe targets.
    pub fn set_targets_keyframes(
        &mut self,
        targets: Vec<(PropertyKey, Vec<f64>)>,
        times: Option<Vec<f64>>,
        default_transition: &TransitionSpec,
        per_property: &HashMap<PropertyKey, TransitionSpec>,
        now: f64,
        delay_secs: f64,
    ) {
        for (key, values) in targets {
            let transition = per_property
                .get(&key)
                .cloned()
                .unwrap_or_else(|| default_transition.clone());
            self.set_target_keyframes(key, values, times.clone(), transition, now, delay_secs);
        }
    }

    fn current_resting(&self, key: PropertyKey) -> f64 {
        self.channels
            .get(&key)
            .map(|ch| ch.target())
            .or_else(|| self.resting.get(&key).copied())
            .unwrap_or_else(|| key.default_value())
    }

    /// Sample all channels and assemble a pose. Returns `(pose, is_animating)`.
    pub fn sample_pose(&mut self, now: f64) -> (SampledPose, bool) {
        let mut pose = SampledPose::default();
        let mut animating = false;

        // Apply resting values first (for properties with no active channel)
        for (&key, &value) in &self.resting {
            apply_to_pose(&mut pose, key, value);
        }

        // Override with active channel samples
        for (&key, channel) in &mut self.channels {
            let (value, _velocity) = channel.sample(now);
            apply_to_pose(&mut pose, key, value);

            if channel.state() == ChannelState::Running {
                animating = true;
            }
        }

        (pose, animating)
    }

    /// Returns true if any channel is currently animating.
    pub fn is_animating(&self) -> bool {
        self.channels
            .values()
            .any(|ch| ch.state() == ChannelState::Running)
    }

    /// Returns true if any paint-class channel is currently animating.
    /// Used to trigger scene repaint rather than compositor-only present.
    pub fn needs_repaint(&self) -> bool {
        self.channels
            .iter()
            .any(|(key, ch)| key.requires_repaint() && ch.state() == ChannelState::Running)
    }

    /// Returns true if the compositor overlay must stay alive for this timeline.
    /// True when channels are running OR when resting pose differs from property defaults.
    pub fn needs_compositor_present(&self) -> bool {
        if self.is_animating() {
            return true;
        }
        self.resting
            .iter()
            .any(|(key, &value)| (value - key.default_value()).abs() > 1e-10)
    }

    /// Returns true if a given property has been set as a motion target (has a resting entry).
    pub fn has_property(&self, key: PropertyKey) -> bool {
        self.resting.contains_key(&key)
    }

    /// Remove completed channels to free memory.
    pub fn gc_completed(&mut self) {
        self.channels
            .retain(|_, ch| ch.state() != ChannelState::Completed);
    }

    /// Snapshot of running channels for devtools.
    /// Returns (property_name, origin, target, state) tuples.
    pub fn running_channel_snapshots(&self) -> Vec<(&'static str, f64, f64, &'static str)> {
        self.channels.iter()
            .filter(|(_, ch)| ch.state() == ChannelState::Running)
            .map(|(key, ch)| {
                (key.name(), ch.origin(), ch.target(), "running")
            })
            .collect()
    }
}

fn apply_to_pose(pose: &mut SampledPose, key: PropertyKey, value: f64) {
    match key {
        PropertyKey::X => pose.x = value,
        PropertyKey::Y => pose.y = value,
        PropertyKey::ScaleX => pose.scale_x = value,
        PropertyKey::ScaleY => pose.scale_y = value,
        PropertyKey::Rotate => pose.rotate_deg = value,
        PropertyKey::Opacity => pose.opacity = value,
        PropertyKey::OriginX => pose.origin_x = value,
        PropertyKey::OriginY => pose.origin_y = value,
        PropertyKey::LayoutX => pose.layout_x = value,
        PropertyKey::LayoutY => pose.layout_y = value,
        PropertyKey::LayoutScaleX => pose.layout_scale_x = value,
        PropertyKey::LayoutScaleY => pose.layout_scale_y = value,
        PropertyKey::BackgroundR => pose.background_r = value,
        PropertyKey::BackgroundG => pose.background_g = value,
        PropertyKey::BackgroundB => pose.background_b = value,
        PropertyKey::BackgroundA => pose.background_a = value,
        PropertyKey::BorderRadius => pose.border_radius = value,
        PropertyKey::BlurRadius => pose.blur_radius = value,
        PropertyKey::ShadowOffsetX => pose.shadow_offset_x = value,
        PropertyKey::ShadowOffsetY => pose.shadow_offset_y = value,
        PropertyKey::ShadowBlurRadius => pose.shadow_blur_radius = value,
        PropertyKey::ShadowR => pose.shadow_r = value,
        PropertyKey::ShadowG => pose.shadow_g = value,
        PropertyKey::ShadowB => pose.shadow_b = value,
        PropertyKey::ShadowA => pose.shadow_a = value,
        PropertyKey::ScrollX => pose.scroll_x = value,
        PropertyKey::ScrollY => pose.scroll_y = value,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::easing::Easing;
    use crate::spring::SpringParams;
    use crate::transition::TransitionSpec;

    #[test]
    fn empty_timeline_returns_defaults() {
        let mut tl = NodeTimeline::new();
        let (pose, animating) = tl.sample_pose(0.0);
        assert!(!animating);
        assert!((pose.opacity - 1.0).abs() < 1e-6);
        assert!((pose.x - 0.0).abs() < 1e-6);
    }

    #[test]
    fn single_property_animation() {
        let mut tl = NodeTimeline::new();
        let spec = TransitionSpec::Tween {
            duration_secs: 1.0,
            easing: Easing::LINEAR,
            repeat: None,
            times: None,
        };
        tl.set_target(PropertyKey::Opacity, 0.0, spec, 0.0, 0.0);

        let (pose, animating) = tl.sample_pose(0.5);
        assert!(animating);
        assert!((pose.opacity - 0.5).abs() < 0.1);

        let (pose, animating) = tl.sample_pose(1.0);
        assert!(!animating);
        assert!((pose.opacity - 0.0).abs() < 1e-6);
    }

    #[test]
    fn interrupt_retargets() {
        let mut tl = NodeTimeline::new();
        let spec = TransitionSpec::Tween {
            duration_secs: 1.0,
            easing: Easing::LINEAR,
            repeat: None,
            times: None,
        };
        tl.set_target(PropertyKey::X, 100.0, spec.clone(), 0.0, 0.0);

        // At 0.5s, x ≈ 50
        let (pose, _) = tl.sample_pose(0.5);
        assert!((pose.x - 50.0).abs() < 1.0);

        // Retarget to 200 mid-flight
        tl.set_target(PropertyKey::X, 200.0, spec, 0.5, 0.0);

        // At 1.0s, should be midway between ~50 and 200
        let (pose, animating) = tl.sample_pose(1.0);
        assert!(animating || !animating); // may or may not be done
        assert!(pose.x > 50.0, "should have moved past interrupt point");
    }

    #[test]
    fn completed_channel_restarts_from_previous_resting_value() {
        let mut tl = NodeTimeline::new();
        let spec = TransitionSpec::Tween {
            duration_secs: 1.0,
            easing: Easing::LINEAR,
            repeat: None,
            times: None,
        };

        tl.set_target(PropertyKey::Opacity, 0.0, spec.clone(), 0.0, 0.0);
        let (pose, animating) = tl.sample_pose(1.0);
        assert!(!animating);
        assert!((pose.opacity - 0.0).abs() < 1e-6);

        tl.gc_completed();
        tl.set_target(PropertyKey::Opacity, 1.0, spec, 1.0, 0.0);

        let (pose, animating) = tl.sample_pose(1.5);
        assert!(animating);
        assert!((pose.opacity - 0.5).abs() < 0.1);
    }

    #[test]
    fn driven_instant_to_spring_carries_velocity() {
        // Simulate drag: rapid instant writes at ~60fps, then release to spring.
        let mut tl = NodeTimeline::new();
        let instant = TransitionSpec::Instant;

        // Simulate 3 frames of drag at 60fps (16.67ms intervals)
        // Moving X from 0 → 100 → 200 → 300
        tl.set_target(PropertyKey::X, 100.0, instant.clone(), 0.000, 0.0);
        tl.set_target(PropertyKey::X, 200.0, instant.clone(), 0.016, 0.0);
        tl.set_target(PropertyKey::X, 300.0, instant.clone(), 0.032, 0.0);

        // Now release: retarget to 250 with spring (e.g. drag constraint)
        let spring = TransitionSpec::Spring(SpringParams {
            stiffness: 300.0,
            damping: 20.0,
            mass: 1.0,
            initial_velocity: 0.0, // Will be overridden by inferred velocity
            rest_delta: 0.01,
            rest_speed: 0.01,
        });
        tl.set_target(PropertyKey::X, 250.0, spring, 0.048, 0.0);

        // The spring should have picked up velocity from the instant writes.
        // Velocity ≈ (300 - 200) / 0.016 = 6250 px/s
        // So at t=0.049 (1ms after release), X should overshoot past 300
        // because the spring has high initial velocity toward positive direction
        // but target is 250 (behind current position).
        let (pose, animating) = tl.sample_pose(0.049);
        assert!(animating, "spring should be running");
        // With velocity ~6250 px/s, even 1ms later the value should have moved
        // past 300 (started at 300, velocity pushes further before spring pulls back)
        assert!(pose.x > 300.0, "expected overshoot from inferred velocity, got {}", pose.x);
    }
}
