use super::super::vello::peniko::{
    ColorStop as PenikoColorStop, Fill, Gradient, ImageBrushRef,
    kurbo::{Affine, BezPath, Circle, PathEl, Point, Rect, RoundedRect, Shape, Stroke},
};
use super::super::vello::{PaintScene, Scene};
use super::decl::FragmentEncode;
use super::kinds::{
    CARET_COLOR, CARET_WIDTH, CircleFragment, GroupFragment, ImageFragment, PathFragment,
    RectFragment, SELECTION_COLOR, SpanFragment, TextFragment, TextInputFragment,
};
use super::types::{FragmentBrush, GradientStop};

fn push_gradient_stops(gradient: &mut Gradient, stops: &[GradientStop]) {
    for stop in stops {
        gradient
            .stops
            .push(PenikoColorStop::from((stop.offset as f32, stop.color)));
    }
}

pub(crate) fn fill_brush_gradient(
    scene: &mut Scene,
    transform: Affine,
    brush: &FragmentBrush,
    fill_rule: Fill,
    path: &BezPath,
) {
    match brush {
        FragmentBrush::Solid(fill) => {
            // vello_hybrid reserves alpha=0 for clipping; skip invisible fills.
            if fill.color.components[3] > 0.0 {
                scene.fill(fill.rule, transform, fill.color, None, path);
            }
        }
        FragmentBrush::LinearGradient {
            start_x,
            start_y,
            end_x,
            end_y,
            stops,
        } => {
            let mut gradient =
                Gradient::new_linear(Point::new(*start_x, *start_y), Point::new(*end_x, *end_y));
            push_gradient_stops(&mut gradient, stops);
            scene.fill(
                fill_rule,
                transform,
                &gradient,
                Some(Affine::IDENTITY),
                path,
            );
        }
        FragmentBrush::RadialGradient {
            center_x,
            center_y,
            radius,
            stops,
        } => {
            let mut gradient =
                Gradient::new_radial(Point::new(*center_x, *center_y), *radius as f32);
            push_gradient_stops(&mut gradient, stops);
            scene.fill(
                fill_rule,
                transform,
                &gradient,
                Some(Affine::IDENTITY),
                path,
            );
        }
        FragmentBrush::SweepGradient {
            center_x,
            center_y,
            start_angle,
            end_angle,
            stops,
        } => {
            let mut gradient = Gradient::new_sweep(
                Point::new(*center_x, *center_y),
                start_angle.to_radians() as f32,
                end_angle.to_radians() as f32,
            );
            push_gradient_stops(&mut gradient, stops);
            scene.fill(
                fill_rule,
                transform,
                &gradient,
                Some(Affine::IDENTITY),
                path,
            );
        }
    }
}

impl FragmentEncode for GroupFragment {
    fn encode(&self, _scene: &mut Scene, _transform: Affine) {}
}

impl FragmentEncode for RectFragment {
    fn encode(&self, scene: &mut Scene, transform: Affine) {
        let rect = Rect::new(0.0, 0.0, self.width, self.height);
        let has_radius = self
            .corner_radii
            .as_single_radius()
            .map_or(true, |r| r > 0.0);

        // Shadow (behind everything). Inset shadows are handled by the GPU effect pass.
        if let Some(shadow) = &self.shadow {
            if !shadow.inset {
                let sr = if has_radius {
                    self.corner_radii.as_single_radius().unwrap_or(0.0)
                } else {
                    0.0
                };
                let shadow_rect = Rect::new(
                    shadow.offset_x,
                    shadow.offset_y,
                    self.width + shadow.offset_x,
                    self.height + shadow.offset_y,
                );
                scene.draw_box_shadow(transform, shadow_rect, shadow.color, sr, shadow.blur);
            }
        }

        // Build path once.
        let path = if has_radius {
            let rrect = RoundedRect::from_rect(rect, self.corner_radii);
            BezPath::from_vec(rrect.path_elements(0.1).collect())
        } else {
            BezPath::from_vec(rect.path_elements(0.1).collect())
        };

        // Fill.
        if let Some(brush) = &self.fill {
            fill_brush_gradient(scene, transform, brush, Fill::NonZero, &path);
        }

        // Stroke.
        if let Some(stroke) = &self.stroke {
            scene.stroke(
                &Stroke::new(self.stroke_width.max(stroke.width)),
                transform,
                stroke.color,
                None,
                &path,
            );
        }

        // Per-side borders — drawn as inset strokes along each edge.
        if self.border_top.is_some()
            || self.border_right.is_some()
            || self.border_bottom.is_some()
            || self.border_left.is_some()
        {
            let w = self.width;
            let h = self.height;
            if let Some(b) = &self.border_top {
                let half = b.width / 2.0;
                let seg = BezPath::from_vec(vec![
                    PathEl::MoveTo(Point::new(0.0, half)),
                    PathEl::LineTo(Point::new(w, half)),
                ]);
                scene.stroke(&Stroke::new(b.width), transform, b.color, None, &seg);
            }
            if let Some(b) = &self.border_right {
                let half = b.width / 2.0;
                let seg = BezPath::from_vec(vec![
                    PathEl::MoveTo(Point::new(w - half, 0.0)),
                    PathEl::LineTo(Point::new(w - half, h)),
                ]);
                scene.stroke(&Stroke::new(b.width), transform, b.color, None, &seg);
            }
            if let Some(b) = &self.border_bottom {
                let half = b.width / 2.0;
                let seg = BezPath::from_vec(vec![
                    PathEl::MoveTo(Point::new(0.0, h - half)),
                    PathEl::LineTo(Point::new(w, h - half)),
                ]);
                scene.stroke(&Stroke::new(b.width), transform, b.color, None, &seg);
            }
            if let Some(b) = &self.border_left {
                let half = b.width / 2.0;
                let seg = BezPath::from_vec(vec![
                    PathEl::MoveTo(Point::new(half, 0.0)),
                    PathEl::LineTo(Point::new(half, h)),
                ]);
                scene.stroke(&Stroke::new(b.width), transform, b.color, None, &seg);
            }
        }
    }
}

impl FragmentEncode for CircleFragment {
    fn encode(&self, scene: &mut Scene, transform: Affine) {
        let center = if self.cx != 0.0 || self.cy != 0.0 {
            (self.cx, self.cy)
        } else {
            (self.r, self.r)
        };
        let circle = Circle::new(center, self.r);
        let path = BezPath::from_vec(circle.path_elements(0.1).collect());
        if let Some(fill) = &self.fill {
            if fill.color.components[3] > 0.0 {
                scene.fill(fill.rule, transform, fill.color, None, &path);
            }
        }
        if let Some(stroke) = &self.stroke {
            scene.stroke(
                &Stroke::new(self.stroke_width.max(stroke.width)),
                transform,
                stroke.color,
                None,
                &path,
            );
        }
    }
}

impl FragmentEncode for PathFragment {
    fn encode(&self, scene: &mut Scene, transform: Affine) {
        let Some(path) = &self.parsed_path else {
            return;
        };

        // Fill.
        if let Some(brush) = &self.fill {
            fill_brush_gradient(scene, transform, brush, Fill::NonZero, path);
        }

        // Stroke.
        if let Some(stroke) = &self.stroke {
            scene.stroke(
                &Stroke::new(self.stroke_width.max(stroke.width)),
                transform,
                stroke.color,
                None,
                path,
            );
        }
    }
}

impl FragmentEncode for TextFragment {
    fn encode(&self, scene: &mut Scene, transform: Affine) {
        if let Some(cache) = &self.shaped {
            if cache.runs.is_empty() {
                // Single-style fallback: use combined path + fragment color.
                scene.fill(Fill::NonZero, transform, self.color, None, &cache.path);
            } else {
                // Rich text: paint each run with its own color.
                for run in &cache.runs {
                    scene.fill(Fill::NonZero, transform, run.color, None, &run.path);
                }
            }
            // Rasterized color glyphs (emoji) — rendered as image fills.
            for rg in &cache.rasterized_glyphs {
                if rg.image.width == 0 || rg.image.height == 0 {
                    continue;
                }
                let sf = rg.scale_factor.max(1.0);
                let logical_w = rg.image.width as f64 / sf;
                let logical_h = rg.image.height as f64 / sf;
                let dest = Rect::new(rg.x, rg.y, rg.x + logical_w, rg.y + logical_h);
                let brush: ImageBrushRef = (&rg.image).into();
                let brush_transform = Affine::translate((rg.x, rg.y))
                    * Affine::scale_non_uniform(
                        logical_w / rg.image.width as f64,
                        logical_h / rg.image.height as f64,
                    );
                scene.fill(
                    Fill::NonZero,
                    transform,
                    brush,
                    Some(brush_transform),
                    &dest,
                );
            }
        }
    }
}

impl FragmentEncode for TextInputFragment {
    fn encode(&self, scene: &mut Scene, transform: Affine) {
        let Some(layout) = &self.layout else { return };

        // Draw text path.
        scene.fill(Fill::NonZero, transform, self.color, None, &layout.path);

        let cursor = self.cursor_pos as usize;
        let anchor_raw = self.selection_anchor as i64;
        let has_selection = anchor_raw >= 0 && anchor_raw as usize != cursor;

        // Draw selection highlight.
        if has_selection {
            let anchor = anchor_raw as usize;
            let sel_start = cursor.min(anchor);
            let sel_end = cursor.max(anchor);
            let max_pos = layout.cursor_x_positions.len().saturating_sub(1);
            let x0 = layout
                .cursor_x_positions
                .get(sel_start.min(max_pos))
                .copied()
                .unwrap_or(0.0);
            let x1 = layout
                .cursor_x_positions
                .get(sel_end.min(max_pos))
                .copied()
                .unwrap_or(layout.width);
            let sel_rect = Rect::new(x0, 0.0, x1, layout.height);
            scene.fill(Fill::NonZero, transform, SELECTION_COLOR, None, &sel_rect);
        }

        // Draw caret (only when no selection and caret is visible).
        if !has_selection && self.caret_visible {
            let max_pos = layout.cursor_x_positions.len().saturating_sub(1);
            let cx = layout
                .cursor_x_positions
                .get(cursor.min(max_pos))
                .copied()
                .unwrap_or(0.0);
            let caret_rect = Rect::new(cx, 0.0, cx + CARET_WIDTH, layout.height);
            scene.fill(Fill::NonZero, transform, CARET_COLOR, None, &caret_rect);
        }
    }
}

impl FragmentEncode for SpanFragment {
    fn encode(&self, _scene: &mut Scene, _transform: Affine) {
        // Span does not paint on its own — the parent TextFragment paints all runs.
    }
}

impl FragmentEncode for ImageFragment {
    fn encode(&self, scene: &mut Scene, transform: Affine) {
        let Some(image_data) = &self.image_data else {
            return;
        };
        if self.width <= 0.0 || self.height <= 0.0 {
            return;
        }

        let src_w = image_data.width as f64;
        let src_h = image_data.height as f64;
        if src_w <= 0.0 || src_h <= 0.0 {
            return;
        }

        // Compute brush_transform and the actual paint rect (may be smaller than dest for contain/none).
        let (brush_transform, paint_rect) =
            compute_image_fit(src_w, src_h, self.width, self.height, &self.object_fit);

        let brush: ImageBrushRef = image_data.into();
        scene.fill(
            Fill::NonZero,
            transform,
            brush,
            Some(brush_transform),
            &paint_rect,
        );
    }
}

/// Compute an affine that maps image pixels into the destination rect
/// according to the given object-fit mode, plus the actual paint rect
/// (which may be smaller than `dst` for contain/none to avoid edge-pixel smearing).
fn compute_image_fit(src_w: f64, src_h: f64, dst_w: f64, dst_h: f64, mode: &str) -> (Affine, Rect) {
    let dest = Rect::new(0.0, 0.0, dst_w, dst_h);
    match mode {
        "contain" => {
            let scale = (dst_w / src_w).min(dst_h / src_h);
            let dx = (dst_w - src_w * scale) / 2.0;
            let dy = (dst_h - src_h * scale) / 2.0;
            let paint = Rect::new(dx, dy, dx + src_w * scale, dy + src_h * scale);
            (Affine::translate((dx, dy)) * Affine::scale(scale), paint)
        }
        "cover" => {
            let scale = (dst_w / src_w).max(dst_h / src_h);
            let dx = (dst_w - src_w * scale) / 2.0;
            let dy = (dst_h - src_h * scale) / 2.0;
            (Affine::translate((dx, dy)) * Affine::scale(scale), dest)
        }
        "none" => {
            let dx = (dst_w - src_w) / 2.0;
            let dy = (dst_h - src_h) / 2.0;
            let fitted = Rect::new(dx, dy, dx + src_w, dy + src_h);
            let paint = fitted.intersect(dest);
            (Affine::translate((dx, dy)), paint)
        }
        // "fill" or default — stretch to fit
        _ => (
            Affine::scale_non_uniform(dst_w / src_w, dst_h / src_h),
            dest,
        ),
    }
}
