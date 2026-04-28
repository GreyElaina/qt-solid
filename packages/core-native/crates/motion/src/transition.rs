use crate::easing::Easing;
use crate::spring::SpringParams;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatType {
    Loop,
    Reverse,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RepeatCount {
    Finite(u32),
    Infinite,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RepeatConfig {
    pub count: RepeatCount,
    pub repeat_type: RepeatType,
}

/// Describes how a single property should animate.
#[derive(Debug, Clone)]
pub enum TransitionSpec {
    /// Snap to target immediately.
    Instant,

    /// Duration-based tween with bezier easing.
    Tween {
        duration_secs: f64,
        easing: Easing,
        repeat: Option<RepeatConfig>,
        /// Normalized time points for keyframes (0.0..1.0), length matches keyframe values.
        times: Option<Vec<f64>>,
    },

    /// Physics-based spring.
    Spring(SpringParams),
}

impl Default for TransitionSpec {
    fn default() -> Self {
        Self::Spring(SpringParams::default())
    }
}
