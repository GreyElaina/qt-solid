use crate::easing::tween_f64;
use crate::spring::{self, SpringParams};
use crate::transition::{RepeatConfig, RepeatCount, RepeatType, TransitionSpec};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelState {
    Idle,
    Running,
    Completed,
}

/// Keyframe track: values at normalized time points.
#[derive(Debug, Clone)]
struct TweenTrack {
    values: Vec<f64>,
    times: Vec<f64>,
}

impl TweenTrack {
    fn new(values: Vec<f64>, times: Option<Vec<f64>>) -> Self {
        assert!(values.len() >= 2, "TweenTrack needs at least 2 values");
        let times = times.unwrap_or_else(|| {
            let n = values.len();
            (0..n).map(|i| i as f64 / (n - 1) as f64).collect()
        });
        assert_eq!(values.len(), times.len(), "values and times length mismatch");
        Self { values, times }
    }

    fn origin(&self) -> f64 {
        self.values[0]
    }

    fn target(&self) -> f64 {
        *self.values.last().unwrap()
    }

    /// Whether all keyframe values are approximately equal.
    fn is_static(&self) -> bool {
        let first = self.values[0];
        self.values.iter().all(|v| (v - first).abs() < 1e-10)
    }

    /// Sample the track at normalized progress [0, 1] with given easing.
    fn sample(&self, progress: f64, easing: &crate::easing::Easing) -> f64 {
        let p = progress.clamp(0.0, 1.0);

        // Find segment
        let n = self.times.len();
        if p <= self.times[0] {
            return self.values[0];
        }
        if p >= self.times[n - 1] {
            return *self.values.last().unwrap();
        }

        let mut seg = 0;
        for i in 1..n {
            if self.times[i] >= p {
                seg = i - 1;
                break;
            }
        }

        let t0 = self.times[seg];
        let t1 = self.times[seg + 1];
        let segment_t = if (t1 - t0).abs() < 1e-15 {
            1.0
        } else {
            ((p - t0) / (t1 - t0)).clamp(0.0, 1.0)
        };

        tween_f64(self.values[seg], self.values[seg + 1], segment_t, easing)
    }
}

/// A single animating property channel.
///
/// Tracks origin, target, transition config, and current state.
/// Call `sample(now)` each frame to get the interpolated value.
#[derive(Debug, Clone)]
pub struct AnimationChannel {
    track: TweenTrack,
    transition: TransitionSpec,
    started_at: f64,
    delay_secs: f64,
    state: ChannelState,
    last_velocity: f64,
    /// Previous target value — used to infer implicit velocity when an instant
    /// (completed) channel is retargeted into a spring. This enables "driven"
    /// mode: rapid instant setTarget calls accumulate velocity that a subsequent
    /// spring transition can pick up (e.g. drag release, scroll fling).
    pub(crate) prev_target: Option<(f64, f64)>, // (value, timestamp)
}

impl AnimationChannel {
    pub fn new(
        origin: f64,
        target: f64,
        transition: TransitionSpec,
        started_at: f64,
        delay_secs: f64,
    ) -> Self {
        Self::new_keyframes(vec![origin, target], None, transition, started_at, delay_secs)
    }

    pub fn new_keyframes(
        values: Vec<f64>,
        times: Option<Vec<f64>>,
        transition: TransitionSpec,
        started_at: f64,
        delay_secs: f64,
    ) -> Self {
        let track = TweenTrack::new(values, times);

        let state = if matches!(transition, TransitionSpec::Instant) || track.is_static() {
            ChannelState::Completed
        } else {
            ChannelState::Running
        };

        Self {
            track,
            transition,
            started_at,
            delay_secs,
            state,
            last_velocity: 0.0,
            prev_target: None,
        }
    }

    pub fn state(&self) -> ChannelState {
        self.state
    }

    pub fn target(&self) -> f64 {
        self.final_value()
    }

    pub fn origin(&self) -> f64 {
        self.track.origin()
    }

    pub fn last_velocity(&self) -> f64 {
        self.last_velocity
    }

    pub fn started_at(&self) -> f64 {
        self.started_at
    }

    /// The value the channel rests at after all iterations complete.
    pub fn final_value(&self) -> f64 {
        match &self.transition {
            TransitionSpec::Tween { repeat, .. } => match repeat {
                None => self.track.target(),
                Some(RepeatConfig { count: RepeatCount::Infinite, .. }) => {
                    // Convention: last value
                    self.track.target()
                }
                Some(RepeatConfig { count: RepeatCount::Finite(n), repeat_type }) => {
                    let total = *n as u64 + 1;
                    match repeat_type {
                        RepeatType::Loop => self.track.target(),
                        RepeatType::Reverse => {
                            if total % 2 == 0 {
                                self.track.origin()
                            } else {
                                self.track.target()
                            }
                        }
                    }
                }
            },
            _ => self.track.target(),
        }
    }

    /// Sample the channel at wall-clock time `now` (seconds).
    /// Returns `(value, velocity)`.
    pub fn sample(&mut self, now: f64) -> (f64, f64) {
        match self.state {
            ChannelState::Idle | ChannelState::Completed => {
                return (self.final_value(), 0.0);
            }
            ChannelState::Running => {}
        }

        let effective_start = self.started_at + self.delay_secs;
        if now < effective_start {
            return (self.track.origin(), 0.0);
        }

        let elapsed = now - effective_start;

        let (value, velocity) = match &self.transition {
            TransitionSpec::Instant => {
                self.state = ChannelState::Completed;
                (self.final_value(), 0.0)
            }
            TransitionSpec::Tween {
                duration_secs,
                easing,
                repeat,
                ..
            } => {
                let duration = *duration_secs;
                let easing = *easing;
                let repeat = repeat.clone();
                self.sample_tween(elapsed, duration, &easing, repeat.as_ref())
            }
            TransitionSpec::Spring(params) => {
                // Spring ignores keyframes/repeat — uses origin/target only
                let origin = self.track.origin();
                let target = self.track.target();
                let sample = spring::solve_spring(params, origin, target, elapsed);
                if sample.settled {
                    self.state = ChannelState::Completed;
                    (target, 0.0)
                } else {
                    (sample.value, sample.velocity)
                }
            }
        };

        self.last_velocity = velocity;
        (value, velocity)
    }

    fn sample_tween(
        &mut self,
        elapsed: f64,
        duration: f64,
        easing: &crate::easing::Easing,
        repeat: Option<&RepeatConfig>,
    ) -> (f64, f64) {
        if duration <= 0.0 {
            self.state = ChannelState::Completed;
            return (self.final_value(), 0.0);
        }

        let raw = elapsed / duration;

        let (total_iterations, is_infinite) = match repeat {
            None => (1u64, false),
            Some(RepeatConfig { count: RepeatCount::Finite(n), .. }) => (*n as u64 + 1, false),
            Some(RepeatConfig { count: RepeatCount::Infinite, .. }) => (u64::MAX, true),
        };

        let repeat_type = repeat.map(|r| r.repeat_type).unwrap_or(RepeatType::Loop);

        let iteration = raw.floor() as u64;

        if !is_infinite && iteration >= total_iterations {
            self.state = ChannelState::Completed;
            return (self.final_value(), 0.0);
        }

        let mut progress = raw - raw.floor();
        // At exact boundaries (progress == 0 and iteration > 0), treat as end of previous
        if progress == 0.0 && iteration > 0 && (is_infinite || iteration < total_iterations) {
            // Actually at start of new iteration, progress = 0 is correct
        }

        if repeat_type == RepeatType::Reverse && iteration % 2 == 1 {
            progress = 1.0 - progress;
        }

        let value = self.track.sample(progress, easing);

        // Velocity via finite difference within same iteration
        let dt = 1.0 / 120.0;
        let next_raw = (elapsed + dt) / duration;
        let next_iteration = next_raw.floor() as u64;
        let velocity = if next_iteration == iteration {
            let mut next_progress = next_raw - next_raw.floor();
            if repeat_type == RepeatType::Reverse && next_iteration % 2 == 1 {
                next_progress = 1.0 - next_progress;
            }
            let next_value = self.track.sample(next_progress, easing);
            (next_value - value) / dt
        } else {
            0.0 // Crossing loop boundary, report zero
        };

        (value, velocity)
    }

    /// Interrupt this channel: snapshot current value/velocity, retarget.
    ///
    /// For keyframe channels, creates a simple A→B from current to new_target.
    ///
    /// When the current channel was instant (Completed), velocity is inferred
    /// from the previous target and elapsed time — enabling "driven" semantics
    /// where rapid instant writes accumulate velocity for a subsequent spring.
    pub fn retarget(&mut self, new_target: f64, new_transition: TransitionSpec, now: f64) -> Self {
        let (current_value, current_velocity) = self.sample(now);

        // Infer velocity from prev_target when channel is completed (instant snap).
        // This is the "driven mode" velocity: Δvalue / Δtime between consecutive
        // instant setTarget calls.
        let effective_velocity = if self.state == ChannelState::Completed && current_velocity == 0.0 {
            if let Some((prev_val, prev_time)) = self.prev_target {
                let dt = now - prev_time;
                if dt > 0.0 && dt < 0.5 {
                    // Only infer velocity if the previous write was recent enough
                    // to be considered part of a continuous gesture.
                    (current_value - prev_val) / dt
                } else {
                    0.0
                }
            } else {
                0.0
            }
        } else {
            current_velocity
        };

        let transition = match &new_transition {
            TransitionSpec::Spring(params) => TransitionSpec::Spring(SpringParams {
                initial_velocity: effective_velocity,
                ..*params
            }),
            other => other.clone(),
        };

        Self::new(current_value, new_target, transition, now, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::easing::Easing;

    #[test]
    fn instant_channel_completes_immediately() {
        let mut ch = AnimationChannel::new(0.0, 1.0, TransitionSpec::Instant, 0.0, 0.0);
        assert_eq!(ch.state(), ChannelState::Completed);
        let (v, _) = ch.sample(0.0);
        assert!((v - 1.0).abs() < 1e-6);
    }

    #[test]
    fn tween_channel_progression() {
        let spec = TransitionSpec::Tween {
            duration_secs: 1.0,
            easing: Easing::LINEAR,
            repeat: None,
            times: None,
        };
        let mut ch = AnimationChannel::new(0.0, 10.0, spec, 0.0, 0.0);

        let (v, _) = ch.sample(0.5);
        assert!((v - 5.0).abs() < 0.1);
        assert_eq!(ch.state(), ChannelState::Running);

        let (v, _) = ch.sample(1.0);
        assert!((v - 10.0).abs() < 1e-6);
        assert_eq!(ch.state(), ChannelState::Completed);
    }

    #[test]
    fn spring_channel_settles() {
        let spec = TransitionSpec::Spring(SpringParams::default());
        let mut ch = AnimationChannel::new(0.0, 1.0, spec, 0.0, 0.0);

        ch.sample(0.1);
        assert_eq!(ch.state(), ChannelState::Running);

        ch.sample(10.0);
        assert_eq!(ch.state(), ChannelState::Completed);
    }

    #[test]
    fn delay_holds_origin() {
        let spec = TransitionSpec::Tween {
            duration_secs: 1.0,
            easing: Easing::LINEAR,
            repeat: None,
            times: None,
        };
        let mut ch = AnimationChannel::new(0.0, 10.0, spec, 0.0, 0.5);

        let (v, _) = ch.sample(0.3);
        assert!((v - 0.0).abs() < 1e-6, "should hold at origin during delay");

        let (v, _) = ch.sample(1.0);
        assert!((v - 5.0).abs() < 0.1);
    }

    #[test]
    fn retarget_carries_velocity() {
        let spec = TransitionSpec::Spring(SpringParams::default());
        let mut ch = AnimationChannel::new(0.0, 1.0, spec, 0.0, 0.0);

        let (_, vel) = ch.sample(0.2);
        assert!(vel.abs() > 0.01, "should have nonzero velocity");

        let new_spec = TransitionSpec::Spring(SpringParams::default());
        let new_ch = ch.retarget(2.0, new_spec, 0.2);

        match &new_ch.transition {
            TransitionSpec::Spring(params) => {
                assert!(params.initial_velocity.abs() > 0.01);
            }
            _ => panic!("expected spring"),
        }
    }

    // --- New tests: repeat ---

    #[test]
    fn tween_repeat_loop_finite() {
        let spec = TransitionSpec::Tween {
            duration_secs: 1.0,
            easing: Easing::LINEAR,
            repeat: Some(RepeatConfig {
                count: RepeatCount::Finite(2), // 3 total iterations
                repeat_type: RepeatType::Loop,
            }),
            times: None,
        };
        let mut ch = AnimationChannel::new(0.0, 10.0, spec, 0.0, 0.0);

        // Midway through first iteration
        let (v, _) = ch.sample(0.5);
        assert!((v - 5.0).abs() < 0.5);
        assert_eq!(ch.state(), ChannelState::Running);

        // Start of second iteration (t=1.5 => iteration=1, progress=0.5)
        let (v, _) = ch.sample(1.5);
        assert!((v - 5.0).abs() < 0.5);
        assert_eq!(ch.state(), ChannelState::Running);

        // End of all 3 iterations (t=3.0)
        let (v, _) = ch.sample(3.0);
        assert!((v - 10.0).abs() < 1e-6);
        assert_eq!(ch.state(), ChannelState::Completed);
    }

    #[test]
    fn tween_repeat_loop_infinite() {
        let spec = TransitionSpec::Tween {
            duration_secs: 1.0,
            easing: Easing::LINEAR,
            repeat: Some(RepeatConfig {
                count: RepeatCount::Infinite,
                repeat_type: RepeatType::Loop,
            }),
            times: None,
        };
        let mut ch = AnimationChannel::new(0.0, 10.0, spec, 0.0, 0.0);

        // Should still be running at t=100
        let (v, _) = ch.sample(100.5);
        assert!((v - 5.0).abs() < 0.5);
        assert_eq!(ch.state(), ChannelState::Running);
    }

    #[test]
    fn tween_repeat_reverse() {
        let spec = TransitionSpec::Tween {
            duration_secs: 1.0,
            easing: Easing::LINEAR,
            repeat: Some(RepeatConfig {
                count: RepeatCount::Finite(1), // 2 total iterations
                repeat_type: RepeatType::Reverse,
            }),
            times: None,
        };
        let mut ch = AnimationChannel::new(0.0, 10.0, spec, 0.0, 0.0);

        // First iteration, forward: t=0.5 => 5.0
        let (v, _) = ch.sample(0.5);
        assert!((v - 5.0).abs() < 0.5);

        // Second iteration, reversed: t=1.5 => progress=0.5 reversed => 5.0
        let (v, _) = ch.sample(1.5);
        assert!((v - 5.0).abs() < 0.5);

        // t=1.75 => iteration=1 (odd, reversed), progress=0.75 => reversed to 0.25 => 2.5
        let (v, _) = ch.sample(1.75);
        assert!((v - 2.5).abs() < 0.5);

        // End: 2 iterations, reverse, even count => final_value = origin = 0.0
        let (v, _) = ch.sample(2.0);
        assert!((v - 0.0).abs() < 1e-6);
        assert_eq!(ch.state(), ChannelState::Completed);
    }

    // --- New tests: keyframes ---

    #[test]
    fn keyframe_tween_three_values() {
        let spec = TransitionSpec::Tween {
            duration_secs: 2.0,
            easing: Easing::LINEAR,
            repeat: None,
            times: None,
        };
        // 0 -> 10 -> 0 over 2 seconds, evenly spaced [0.0, 0.5, 1.0]
        let mut ch = AnimationChannel::new_keyframes(
            vec![0.0, 10.0, 0.0],
            None,
            spec,
            0.0,
            0.0,
        );

        // At t=0.5 (progress=0.25), in first segment (0->10), seg_t = 0.5 => 5.0
        let (v, _) = ch.sample(0.5);
        assert!((v - 5.0).abs() < 0.5, "got {v}");

        // At t=1.0 (progress=0.5), at keyframe 1 => 10.0
        let (v, _) = ch.sample(1.0);
        assert!((v - 10.0).abs() < 0.5, "got {v}");

        // At t=1.5 (progress=0.75), in second segment (10->0), seg_t=0.5 => 5.0
        let (v, _) = ch.sample(1.5);
        assert!((v - 5.0).abs() < 0.5, "got {v}");

        // At t=2.0 => completed, final = 0.0
        let (v, _) = ch.sample(2.0);
        assert!((v - 0.0).abs() < 1e-6);
        assert_eq!(ch.state(), ChannelState::Completed);
    }

    #[test]
    fn keyframe_with_repeat() {
        let spec = TransitionSpec::Tween {
            duration_secs: 1.0,
            easing: Easing::LINEAR,
            repeat: Some(RepeatConfig {
                count: RepeatCount::Finite(1),
                repeat_type: RepeatType::Loop,
            }),
            times: None,
        };
        let mut ch = AnimationChannel::new_keyframes(
            vec![0.0, 10.0, 5.0],
            None,
            spec,
            0.0,
            0.0,
        );

        // Halfway through second iteration
        let (v, _) = ch.sample(1.25);
        // iteration=1, progress=0.25 => first segment half => 5.0
        assert!((v - 5.0).abs() < 1.0, "got {v}");
        assert_eq!(ch.state(), ChannelState::Running);

        // Completed at t=2.0
        let (v, _) = ch.sample(2.0);
        assert!((v - 5.0).abs() < 1e-6); // final = last keyframe
        assert_eq!(ch.state(), ChannelState::Completed);
    }

    #[test]
    fn static_keyframe_track_completes_immediately() {
        let spec = TransitionSpec::Tween {
            duration_secs: 1.0,
            easing: Easing::LINEAR,
            repeat: None,
            times: None,
        };
        let ch = AnimationChannel::new_keyframes(
            vec![5.0, 5.0, 5.0],
            None,
            spec,
            0.0,
            0.0,
        );
        assert_eq!(ch.state(), ChannelState::Completed);
    }

    #[test]
    fn retarget_mid_keyframe_creates_simple_ab() {
        let spec = TransitionSpec::Tween {
            duration_secs: 2.0,
            easing: Easing::LINEAR,
            repeat: None,
            times: None,
        };
        let mut ch = AnimationChannel::new_keyframes(
            vec![0.0, 10.0, 0.0],
            None,
            spec.clone(),
            0.0,
            0.0,
        );

        // Sample at t=0.5 (should be ~5.0)
        let (v, _) = ch.sample(0.5);
        assert!((v - 5.0).abs() < 1.0);

        // Retarget to 20.0 => simple A->B from ~5.0 to 20.0
        let new_ch = ch.retarget(20.0, spec, 0.5);
        assert_eq!(new_ch.track.values.len(), 2);
        assert!((new_ch.track.values[0] - v).abs() < 1e-6);
        assert!((new_ch.track.values[1] - 20.0).abs() < 1e-6);
    }
}
