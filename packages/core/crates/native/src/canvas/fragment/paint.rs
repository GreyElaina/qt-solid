use super::super::vello::peniko::kurbo::{Affine, Point, Rect, Shape};
use super::super::vello::peniko::BlendMode;
use super::super::vello::{PaintScene, Scene};
use super::types::{FragmentClipShape, PaintChunk, push_fragment_layer};

// ---------------------------------------------------------------------------
// PaintCollector — maintains clip/layer stack for scene splitting
// ---------------------------------------------------------------------------

pub(crate) struct LayerEntry {
    pub(crate) transform: Affine,
    pub(crate) clip: Option<FragmentClipShape>,
    pub(crate) opacity: f32,
    pub(crate) blend_mode: BlendMode,
}

pub(crate) struct PaintCollector {
    pub(crate) chunks: Vec<PaintChunk>,
    pub(crate) current_inline: Scene,
    pub(crate) layer_stack: Vec<LayerEntry>,
}

impl PaintCollector {
    pub(crate) fn push_layer(&mut self, transform: Affine, clip: Option<FragmentClipShape>, opacity: f32, blend_mode: BlendMode) {
        push_fragment_layer(&mut self.current_inline, transform, clip.as_ref(), opacity, blend_mode);
        self.layer_stack.push(LayerEntry { transform, clip, opacity, blend_mode });
    }

    pub(crate) fn pop_layer(&mut self) {
        self.current_inline.pop_layer();
        self.layer_stack.pop();
    }

    /// Flush current inline scene as a chunk if non-empty.
    pub(crate) fn flush_inline(&mut self) {
        let scene = std::mem::replace(&mut self.current_inline, Scene::new());
        if !scene.commands.is_empty() {
            self.chunks.push(PaintChunk::Inline(scene));
        }
    }

    /// Close all active layers on the current inline scene, then flush it.
    pub(crate) fn flush_inline_for_split(&mut self) {
        // Balance: pop all active layers so the inline scene is self-contained.
        for _ in 0..self.layer_stack.len() {
            self.current_inline.pop_layer();
        }
        self.flush_inline();
    }

    /// Start a fresh inline scene and replay the active layer stack into it.
    pub(crate) fn resume_inline_after_split(&mut self) {
        self.current_inline = Scene::new();
        for entry in &self.layer_stack {
            push_fragment_layer(&mut self.current_inline, entry.transform, entry.clip.as_ref(), entry.opacity, entry.blend_mode);
        }
    }

    /// Compute the accumulated clip rect from the layer stack (in window coords).
    pub(crate) fn accumulated_clip_rect(&self) -> Option<Rect> {
        let mut result: Option<Rect> = None;
        for entry in &self.layer_stack {
            let bounds = match &entry.clip {
                Some(FragmentClipShape::Rect(r)) => Some(*r),
                Some(FragmentClipShape::Path(p)) => Some(p.bounding_box()),
                None => None,
            };
            if let Some(b) = bounds {
                let world_clip = transform_local_bounds_to_world(b, entry.transform);
                result = Some(match result {
                    Some(r) => r.intersect(world_clip),
                    None => world_clip,
                });
            }
        }
        result
    }
}

// ---------------------------------------------------------------------------
// AABB cache — lazy recompute for hit_test pruning
// ---------------------------------------------------------------------------

pub(crate) fn is_axis_aligned_affine(t: Affine) -> bool {
    let coeffs = t.as_coeffs();
    // Affine coeffs: [a, b, c, d, e, f] → matrix [[a,c],[b,d]] + [e,f]
    // Axis-aligned if off-diagonal (b, c) are ~zero.
    coeffs[1].abs() < 1e-9 && coeffs[2].abs() < 1e-9
}

pub(crate) fn transform_local_bounds_to_world(local_bounds: Rect, transform: Affine) -> Rect {
    let corners = [
        transform * Point::new(local_bounds.x0, local_bounds.y0),
        transform * Point::new(local_bounds.x1, local_bounds.y0),
        transform * Point::new(local_bounds.x0, local_bounds.y1),
        transform * Point::new(local_bounds.x1, local_bounds.y1),
    ];
    let min_x = corners.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
    let min_y = corners.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
    let max_x = corners.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max);
    let max_y = corners.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max);
    Rect::new(min_x, min_y, max_x, max_y)
}

pub(crate) fn union_rect(a: Option<Rect>, b: Rect) -> Rect {
    match a {
        Some(a) => a.union(b),
        None => b,
    }
}
