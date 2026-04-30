use crate::canvas::vello::Scene;
use crate::runtime::capture::WidgetCapture;

use crate::runtime::qt_error;

fn renderer_trace_enabled() -> bool {
    std::env::var_os("QT_SOLID_WGPU_TRACE").is_some()
}

fn renderer_trace(args: std::fmt::Arguments<'_>) {
    if !renderer_trace_enabled() {
        return;
    }
    println!("[qt-vello-cpu] {args}");
}

fn capture_checksum(bytes: &[u8]) -> (u64, u64) {
    let mut sum = 0_u64;
    let mut xor = 0_u64;
    for (index, byte) in bytes.iter().copied().enumerate() {
        sum = sum.wrapping_add(byte as u64);
        xor ^= (byte as u64) << ((index & 7) * 8);
    }
    (sum, xor)
}

fn alpha_stats(bytes: &[u8]) -> (usize, usize, usize) {
    let mut zero = 0_usize;
    let mut opaque = 0_usize;
    let mut partial = 0_usize;
    for alpha in bytes.iter().skip(3).step_by(4).copied() {
        if alpha == 0 {
            zero += 1;
        } else if alpha == 255 {
            opaque += 1;
        } else {
            partial += 1;
        }
    }
    (zero, opaque, partial)
}

pub(crate) fn render_scene_to_capture(
    target: qt_compositor::QtCompositorTarget,
    node_id: u32,
    width_px: u32,
    height_px: u32,
    scale_factor: f64,
    scene: &Scene,
) -> napi::Result<WidgetCapture> {
    let pixmap = super::cpu::logical_scene_to_cpu_pixmap(target, node_id, width_px, height_px, scale_factor, scene)
        .map_err(|error| qt_error(error.to_string()))?;
    let bytes = pixmap.data_as_u8_slice();
    let non_zero_bytes = bytes
        .iter()
        .filter(|byte| **byte != 0)
        .count();
    let (sum, xor) = capture_checksum(bytes);
    let (alpha_zero, alpha_opaque, alpha_partial) = alpha_stats(bytes);
    renderer_trace(format_args!(
        "capture node={} size={}x{} scale={:.3} bytes={} non_zero_bytes={} checksum_sum={} checksum_xor=0x{:016x} alpha_zero={} alpha_opaque={} alpha_partial={}",
        node_id,
        width_px,
        height_px,
        scale_factor,
        bytes.len(),
        non_zero_bytes,
        sum,
        xor,
        alpha_zero,
        alpha_opaque,
        alpha_partial
    ));
    WidgetCapture::from_premul_rgba_pixels(
        width_px,
        height_px,
        width_px as usize * 4,
        scale_factor,
        pixmap.data().to_vec(),
    )
    .map_err(|error| qt_error(error.to_string()))
}
