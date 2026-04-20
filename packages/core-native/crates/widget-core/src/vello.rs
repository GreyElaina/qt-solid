use std::time::Duration;

pub use vello_api::{PaintScene, Scene, peniko};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameTime {
    pub elapsed: Duration,
    pub delta: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VelloDirtyRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub struct PaintSceneFrame<'a> {
    width: f64,
    height: f64,
    scale_factor: f64,
    time: FrameTime,
    scene: &'a mut Scene,
    next_frame_requested: &'a mut bool,
    dirty_rects: &'a mut Vec<VelloDirtyRect>,
}

impl std::fmt::Debug for PaintSceneFrame<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PaintSceneFrame")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("scale_factor", &self.scale_factor)
            .field("time", &self.time)
            .field("next_frame_requested", self.next_frame_requested)
            .field("dirty_rects", self.dirty_rects)
            .finish_non_exhaustive()
    }
}

impl<'a> PaintSceneFrame<'a> {
    pub fn new(
        width: f64,
        height: f64,
        scale_factor: f64,
        time: FrameTime,
        scene: &'a mut Scene,
        next_frame_requested: &'a mut bool,
        dirty_rects: &'a mut Vec<VelloDirtyRect>,
    ) -> Self {
        Self {
            width,
            height,
            scale_factor,
            time,
            scene,
            next_frame_requested,
            dirty_rects,
        }
    }

    pub fn width(&self) -> f64 {
        self.width
    }

    pub fn height(&self) -> f64 {
        self.height
    }

    pub fn scale_factor(&self) -> f64 {
        self.scale_factor
    }

    pub fn time(&self) -> FrameTime {
        self.time
    }

    pub fn elapsed(&self) -> Duration {
        self.time.elapsed
    }

    pub fn delta(&self) -> Duration {
        self.time.delta
    }

    pub fn scene(&mut self) -> &mut Scene {
        self.scene
    }

    pub fn request_next_frame(&mut self) {
        *self.next_frame_requested = true;
    }

    pub fn next_frame_requested(&self) -> bool {
        *self.next_frame_requested
    }

    pub fn request_dirty_rect(&mut self, x: f64, y: f64, width: f64, height: f64) {
        if !x.is_finite()
            || !y.is_finite()
            || !width.is_finite()
            || !height.is_finite()
            || width <= 0.0
            || height <= 0.0
        {
            return;
        }

        self.dirty_rects.push(VelloDirtyRect {
            x,
            y,
            width,
            height,
        });
    }

    pub fn dirty_rects(&self) -> &[VelloDirtyRect] {
        self.dirty_rects
    }
}

pub type VelloFrame<'a> = PaintSceneFrame<'a>;

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{FrameTime, PaintSceneFrame, Scene};

    #[test]
    fn vello_frame_requests_next_frame() {
        let mut scene = Scene::new(false);
        let mut next_frame_requested = false;
        let mut dirty_rects = Vec::new();
        let mut frame = PaintSceneFrame::new(
            320.0,
            180.0,
            2.0,
            FrameTime {
                elapsed: Duration::from_millis(20),
                delta: Duration::from_millis(16),
            },
            &mut scene,
            &mut next_frame_requested,
            &mut dirty_rects,
        );

        assert_eq!(frame.width(), 320.0);
        assert_eq!(frame.height(), 180.0);
        assert_eq!(frame.scale_factor(), 2.0);
        assert_eq!(frame.elapsed(), Duration::from_millis(20));
        assert_eq!(frame.delta(), Duration::from_millis(16));
        assert!(!frame.next_frame_requested());
        let _ = frame.scene();
        frame.request_next_frame();
        frame.request_dirty_rect(10.0, 12.0, 20.0, 24.0);
        assert!(frame.next_frame_requested());
        assert_eq!(frame.dirty_rects().len(), 1);
    }
}
