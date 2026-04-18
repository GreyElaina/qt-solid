use std::time::{Duration, Instant};

use qt::runtime::QtWidgetDefaultConstruct;
use qt_widget_derive::{Qt, qt_entity, qt_methods};

use super::shared::{WindowFrame, WindowHost};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameTime {
    pub elapsed: Duration,
    pub delta: Duration,
}

#[derive(Qt, Debug, Clone)]
#[qt_entity(widget, export = "window", children = Nodes)]
pub struct WindowWidget {
    #[qt(default)]
    frame_seq: u64,
    #[qt(default = Instant::now)]
    started_at: Instant,
    #[qt(default)]
    last_frame_at: Option<Instant>,
    #[qt(default)]
    last_frame_elapsed: Duration,
    #[qt(default)]
    last_frame_delta: Duration,
    #[qt(default)]
    next_frame_requested: bool,
}

impl WindowWidget {
    pub fn begin_frame(&mut self) -> FrameTime {
        let now = Instant::now();
        let delta = self
            .last_frame_at
            .map(|last| now.saturating_duration_since(last))
            .unwrap_or_default();
        let elapsed = now.saturating_duration_since(self.started_at);

        self.frame_seq = self.frame_seq.saturating_add(1);
        self.last_frame_at = Some(now);
        self.last_frame_elapsed = elapsed;
        self.last_frame_delta = delta;

        FrameTime { elapsed, delta }
    }

    fn tick_frame(&mut self) {
        let _ = self.begin_frame();
    }

    fn next_frame_requested(&self) -> bool {
        self.next_frame_requested
    }
}

#[qt_methods]
impl WindowWidget {
    #[qt(constructor)]
    fn create_intrinsic_instance() -> Self {
        <Self as QtWidgetDefaultConstruct>::__qt_default_construct()
    }

    #[qt(prop = seq, getter)]
    fn frame_seq(&self) -> f64 {
        self.frame_seq as f64
    }

    #[qt(prop = elapsed_ms, getter)]
    fn frame_elapsed_ms(&self) -> f64 {
        self.last_frame_elapsed.as_secs_f64() * 1000.0
    }

    #[qt(prop = delta_ms, getter)]
    fn frame_delta_ms(&self) -> f64 {
        self.last_frame_delta.as_secs_f64() * 1000.0
    }

    #[qt(prop = tick, setter)]
    fn tick_frame_intrinsic(&mut self, _value: bool) {
        self.tick_frame();
    }

    #[qt(prop = next_frame_requested, setter)]
    fn set_frame_next_frame_requested(&mut self, value: bool) {
        self.next_frame_requested = value;
    }

    #[qt(prop = next_frame_requested, getter)]
    fn frame_next_frame_requested(&self) -> bool {
        self.next_frame_requested()
    }
}

#[qt_methods]
#[qt(host)]
impl WindowHost for WindowWidget {}

#[qt_methods]
#[qt(host)]
impl WindowFrame for WindowWidget {}
