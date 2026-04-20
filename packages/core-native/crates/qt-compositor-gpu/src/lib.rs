use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::Duration,
};

use qt_compositor_types::{QtCompositorSurfaceKey, QtCompositorTarget};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompositorTimingStage {
    UploadBase,
    UploadLayers,
    AcquireSurface,
    EncodeDraw,
    SubmitPresent,
    PrepareOverlayScene,
    PaintOverlayScene,
    ConvertOverlayDirtyRects,
    RenderOverlayLayer,
}

#[derive(Default)]
struct TimingAggregate {
    count: u64,
    total: Duration,
}

impl TimingAggregate {
    fn add_sample(&mut self, duration: Duration) {
        self.count += 1;
        self.total += duration;
    }

    fn average_ms(&self) -> f64 {
        if self.count == 0 {
            return 0.0;
        }
        self.total.as_secs_f64() * 1000.0 / self.count as f64
    }
}

#[derive(Default)]
struct CompositorTimingStats {
    decisions: u64,
    presented: u64,
    upload_base: TimingAggregate,
    upload_layers: TimingAggregate,
    acquire_surface: TimingAggregate,
    encode_draw: TimingAggregate,
    submit_present: TimingAggregate,
    prepare_overlay_scene: TimingAggregate,
    paint_overlay_scene: TimingAggregate,
    convert_overlay_dirty_rects: TimingAggregate,
    render_overlay_layer: TimingAggregate,
}

impl CompositorTimingStats {
    fn stage_mut(&mut self, stage: CompositorTimingStage) -> &mut TimingAggregate {
        match stage {
            CompositorTimingStage::UploadBase => &mut self.upload_base,
            CompositorTimingStage::UploadLayers => &mut self.upload_layers,
            CompositorTimingStage::AcquireSurface => &mut self.acquire_surface,
            CompositorTimingStage::EncodeDraw => &mut self.encode_draw,
            CompositorTimingStage::SubmitPresent => &mut self.submit_present,
            CompositorTimingStage::PrepareOverlayScene => &mut self.prepare_overlay_scene,
            CompositorTimingStage::PaintOverlayScene => &mut self.paint_overlay_scene,
            CompositorTimingStage::ConvertOverlayDirtyRects => {
                &mut self.convert_overlay_dirty_rects
            }
            CompositorTimingStage::RenderOverlayLayer => &mut self.render_overlay_layer,
        }
    }
}

fn timing_enabled() -> bool {
    static TIMING_ENABLED: OnceLock<bool> = OnceLock::new();
    *TIMING_ENABLED.get_or_init(|| std::env::var_os("QT_SOLID_WGPU_TIMING").is_some())
}

fn timings() -> &'static Mutex<HashMap<QtCompositorSurfaceKey, CompositorTimingStats>> {
    static COMPOSITOR_TIMINGS: OnceLock<
        Mutex<HashMap<QtCompositorSurfaceKey, CompositorTimingStats>>,
    > = OnceLock::new();
    COMPOSITOR_TIMINGS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn maybe_log_timing(key: QtCompositorSurfaceKey, stats: &CompositorTimingStats) {
    if stats.decisions == 0 || stats.decisions % 120 != 0 {
        return;
    }

    eprintln!(
        "qt-wgpu timing kind={} primary=0x{:x} secondary=0x{:x} decisions={} presented={} upload_base_ms={:.3} upload_layers_ms={:.3} acquire_surface_ms={:.3} encode_draw_ms={:.3} submit_present_ms={:.3} prepare_overlay_scene_ms={:.3} paint_overlay_scene_ms={:.3} convert_overlay_dirty_rects_ms={:.3} render_overlay_layer_ms={:.3}",
        key.surface_kind,
        key.primary_handle,
        key.secondary_handle,
        stats.decisions,
        stats.presented,
        stats.upload_base.average_ms(),
        stats.upload_layers.average_ms(),
        stats.acquire_surface.average_ms(),
        stats.encode_draw.average_ms(),
        stats.submit_present.average_ms(),
        stats.prepare_overlay_scene.average_ms(),
        stats.paint_overlay_scene.average_ms(),
        stats.convert_overlay_dirty_rects.average_ms(),
        stats.render_overlay_layer.average_ms(),
    );
}

pub fn record_compositor_present_decision(target: QtCompositorTarget, presented: bool) {
    if !timing_enabled() {
        return;
    }

    let key = target.surface_key();
    let mut timings = timings()
        .lock()
        .expect("qt wgpu compositor timing mutex poisoned");
    let stats = timings.entry(key).or_default();
    stats.decisions += 1;
    if presented {
        stats.presented += 1;
    }
    maybe_log_timing(key, stats);
}

pub fn record_compositor_timing(
    target: QtCompositorTarget,
    stage: CompositorTimingStage,
    duration: Duration,
) {
    if !timing_enabled() {
        return;
    }

    let key = target.surface_key();
    let mut timings = timings()
        .lock()
        .expect("qt wgpu compositor timing mutex poisoned");
    let stats = timings.entry(key).or_default();
    stats.stage_mut(stage).add_sample(duration);
    maybe_log_timing(key, stats);
}
