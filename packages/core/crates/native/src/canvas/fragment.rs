use std::collections::{HashMap, HashSet};

use fragment_derive::Fragment;
use taffy::prelude::*;

use super::fragment_decl::{
    FragmentBlendMode, FragmentEncode, FragmentMutation, FragmentPropDecl, FragmentValue,
};
use crate::scene_renderer::effect_pass::{BackdropBlurEffect, InnerShadowEffect};
use super::vello::{
    peniko::{
        color::palette,
        kurbo::{Affine, BezPath, Circle, Point, Rect, RoundedRect, RoundedRectRadii, Shape, Stroke, Vec2},
        BlendMode, Color, Fill, ImageBrushRef, ImageData,
    },
    PaintScene, Scene,
};

/// Fallback clip rect for opacity-only layers where no clip shape is specified.
/// anyrender's push_layer requires a clip shape; use an enormous rect when clipping
/// is not actually desired.
const FALLBACK_CLIP: Rect = Rect::new(-1e9, -1e9, 1e9, 1e9);

fn push_fragment_layer(scene: &mut Scene, transform: Affine, clip_rect: Option<Rect>, opacity: f32, blend_mode: BlendMode) {
    let clip = clip_rect.unwrap_or(FALLBACK_CLIP);
    scene.push_layer(blend_mode, opacity, transform, &clip);
}

// ---------------------------------------------------------------------------
// Layout listener flags
// ---------------------------------------------------------------------------

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub struct FragmentListeners: u32 {
        const LAYOUT = 1 << 0;
    }
}

pub struct FragmentLayoutChange {
    pub fragment_id: FragmentId,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

// ---------------------------------------------------------------------------
// Layer promotion — paint plan types
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct PromotedLayer {
    pub fragment_id: FragmentId,
    pub layer_key: FragmentLayerKey,
    pub scene: Scene,
    pub bounds: Rect,
    pub transform: Affine,
    pub clip_rect: Option<Rect>,
    pub opacity: f32,
    pub blend_mode: BlendMode,
}

#[derive(Debug)]
pub enum PaintChunk {
    Inline(Scene),
    Promoted(PromotedLayer),
}

#[derive(Debug)]
pub struct PaintPlan {
    pub chunks: Vec<PaintChunk>,
    pub stale_keys: Vec<FragmentLayerKey>,
}

impl PaintPlan {
    pub fn has_promoted(&self) -> bool {
        self.chunks.iter().any(|c| matches!(c, PaintChunk::Promoted(_)))
    }

    /// Merge all chunks into a single scene (P1 fallback / non-compositor path).
    pub fn into_single_scene(self, base_transform: Affine) -> Scene {
        let mut scene = Scene::new();
        for chunk in self.chunks {
            match chunk {
                PaintChunk::Inline(inline) => {
                    scene.append_scene(inline, base_transform);
                }
                PaintChunk::Promoted(layer) => {
                    scene.append_scene(layer.scene, base_transform);
                }
            }
        }
        scene
    }

    /// Split into inline scene (all inline chunks merged) + promoted layers.
    pub fn split(self) -> (Scene, Vec<PromotedLayer>) {
        let mut inline_scene = Scene::new();
        let mut promoted = Vec::new();
        for chunk in self.chunks {
            match chunk {
                PaintChunk::Inline(s) => {
                    inline_scene.append_scene(s, Affine::IDENTITY);
                }
                PaintChunk::Promoted(layer) => {
                    promoted.push(layer);
                }
            }
        }
        (inline_scene, promoted)
    }
}

// ---------------------------------------------------------------------------
// Fragment identity
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FragmentId(pub u32);

/// Compositor texture cache key for promoted fragment layers.
/// Allocated from 0x8000_0000+ to avoid collision with Qt widget node IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FragmentLayerKey(pub u32);

const FRAGMENT_LAYER_KEY_BASE: u32 = 0x8000_0000;

// ---------------------------------------------------------------------------
// Paint properties
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FillPaint {
    pub color: Color,
    pub rule: Fill,
}

impl Default for FillPaint {
    fn default() -> Self {
        Self {
            color: palette::css::TRANSPARENT,
            rule: Fill::NonZero,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StrokePaint {
    pub color: Color,
    pub width: f64,
}

#[derive(Debug, Clone)]
pub struct GradientStop {
    pub offset: f64,
    pub color: Color,
}

#[derive(Debug, Clone)]
pub enum FragmentBrush {
    Solid(FillPaint),
    LinearGradient {
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
        stops: Vec<GradientStop>,
    },
    RadialGradient {
        center_x: f64,
        center_y: f64,
        radius: f64,
        stops: Vec<GradientStop>,
    },
    SweepGradient {
        center_x: f64,
        center_y: f64,
        start_angle: f64,
        end_angle: f64,
        stops: Vec<GradientStop>,
    },
}

#[derive(Debug, Clone)]
pub struct FragmentBoxShadow {
    pub offset_x: f64,
    pub offset_y: f64,
    pub blur: f64,
    pub color: Color,
    pub inset: bool,
}

// ---------------------------------------------------------------------------
// Shaped text layout — rich layout snapshot with cursor positions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ShapedTextLayout {
    pub path: BezPath,
    pub cursor_x_positions: Vec<f64>,
    pub width: f64,
    pub height: f64,
    pub ascent: f64,
}

// ---------------------------------------------------------------------------
// Shaped text cache — populated by the native crate via C++ Qt shaping
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ShapedTextLine {
    pub y_offset: f64,
    pub width: f64,
    pub height: f64,
    pub ascent: f64,
    pub descent: f64,
}

#[derive(Debug, Clone)]
pub struct ShapedRun {
    pub path: BezPath,
    pub color: Color,
}

/// A rasterized color glyph (emoji) rendered as a bitmap by Qt.
#[derive(Debug, Clone)]
pub struct RasterizedGlyph {
    pub image: ImageData,
    pub x: f64,
    pub y: f64,
    pub scale_factor: f64,
}

#[derive(Debug, Clone)]
pub struct ShapedTextCache {
    pub path: BezPath,
    pub width: f64,
    pub height: f64,
    pub ascent: f64,
    pub lines: Vec<ShapedTextLine>,
    pub runs: Vec<ShapedRun>,
    pub rasterized_glyphs: Vec<RasterizedGlyph>,
}

// ---------------------------------------------------------------------------
// Color parsing helper — used by derive-generated code
// ---------------------------------------------------------------------------

pub fn parse_color_from_wire(value: &FragmentValue) -> Option<Color> {
    match value {
        FragmentValue::Str { value: s } => parse_css_hex_color(s),
        _ => None,
    }
}

fn parse_css_hex_color(s: &str) -> Option<Color> {
    if s == "transparent" {
        return Some(Color::from_rgba8(0, 0, 0, 0));
    }
    let s = s.strip_prefix('#')?;
    match s.len() {
        6 => {
            let r = u8::from_str_radix(&s[0..2], 16).ok()?;
            let g = u8::from_str_radix(&s[2..4], 16).ok()?;
            let b = u8::from_str_radix(&s[4..6], 16).ok()?;
            Some(Color::from_rgba8(r, g, b, 255))
        }
        8 => {
            let r = u8::from_str_radix(&s[0..2], 16).ok()?;
            let g = u8::from_str_radix(&s[2..4], 16).ok()?;
            let b = u8::from_str_radix(&s[4..6], 16).ok()?;
            let a = u8::from_str_radix(&s[6..8], 16).ok()?;
            Some(Color::from_rgba8(r, g, b, a))
        }
        _ => None,
    }
}

fn parse_gradient_stops(offsets: &[f64], colors: &[String]) -> Vec<GradientStop> {
    offsets.iter().zip(colors.iter())
        .filter_map(|(&offset, color_str)| {
            parse_css_hex_color(color_str).map(|color| GradientStop { offset, color })
        })
        .collect()
}

pub fn parse_brush_from_wire(value: &FragmentValue) -> Option<FragmentBrush> {
    match value {
        FragmentValue::Str { value: s } => {
            parse_css_hex_color(s).map(|color| FragmentBrush::Solid(FillPaint {
                color,
                rule: Fill::NonZero,
            }))
        }
        FragmentValue::LinearGradient { start_x, start_y, end_x, end_y, stop_offsets, stop_colors } => {
            let stops = parse_gradient_stops(stop_offsets, stop_colors);
            if stops.is_empty() { None }
            else { Some(FragmentBrush::LinearGradient { start_x: *start_x, start_y: *start_y, end_x: *end_x, end_y: *end_y, stops }) }
        }
        FragmentValue::RadialGradient { center_x, center_y, radius, stop_offsets, stop_colors } => {
            let stops = parse_gradient_stops(stop_offsets, stop_colors);
            if stops.is_empty() { None }
            else { Some(FragmentBrush::RadialGradient { center_x: *center_x, center_y: *center_y, radius: *radius, stops }) }
        }
        FragmentValue::SweepGradient { center_x, center_y, start_angle, end_angle, stop_offsets, stop_colors } => {
            let stops = parse_gradient_stops(stop_offsets, stop_colors);
            if stops.is_empty() { None }
            else { Some(FragmentBrush::SweepGradient { center_x: *center_x, center_y: *center_y, start_angle: *start_angle, end_angle: *end_angle, stops }) }
        }
        _ => None,
    }
}

pub fn parse_shadow_from_wire(value: &FragmentValue) -> Option<FragmentBoxShadow> {
    match value {
        FragmentValue::BoxShadow { offset_x, offset_y, blur, color, inset } => {
            parse_css_hex_color(color).map(|c| FragmentBoxShadow {
                offset_x: *offset_x,
                offset_y: *offset_y,
                blur: *blur,
                color: c,
                inset: *inset,
            })
        }
        _ => None,
    }
}

impl From<FragmentBlendMode> for BlendMode {
    fn from(mode: FragmentBlendMode) -> Self {
        use super::vello::peniko::{Mix, Compose};
        let mix = match mode {
            FragmentBlendMode::Normal => return BlendMode::default(),
            FragmentBlendMode::Multiply => Mix::Multiply,
            FragmentBlendMode::Screen => Mix::Screen,
            FragmentBlendMode::Overlay => Mix::Overlay,
            FragmentBlendMode::Darken => Mix::Darken,
            FragmentBlendMode::Lighten => Mix::Lighten,
            FragmentBlendMode::ColorDodge => Mix::ColorDodge,
            FragmentBlendMode::ColorBurn => Mix::ColorBurn,
            FragmentBlendMode::HardLight => Mix::HardLight,
            FragmentBlendMode::SoftLight => Mix::SoftLight,
            FragmentBlendMode::Difference => Mix::Difference,
            FragmentBlendMode::Exclusion => Mix::Exclusion,
            FragmentBlendMode::Hue => Mix::Hue,
            FragmentBlendMode::Saturation => Mix::Saturation,
            FragmentBlendMode::Color => Mix::Color,
            FragmentBlendMode::Luminosity => Mix::Luminosity,
        };
        BlendMode { mix, compose: Compose::SrcOver }
    }
}

pub fn parse_radii_from_wire(value: &FragmentValue) -> Option<RoundedRectRadii> {
    match value {
        FragmentValue::F64 { value } => Some(RoundedRectRadii::from_single_radius(*value)),
        FragmentValue::Radii { top_left, top_right, bottom_right, bottom_left } => {
            Some(RoundedRectRadii::new(*top_left, *top_right, *bottom_right, *bottom_left))
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Fragment kind structs — #[derive(Fragment)] generates FragmentDecl impls
// ---------------------------------------------------------------------------

#[derive(Fragment, Debug, Clone, Default)]
#[fragment(tag = "group", bounds = none)]
pub struct GroupFragment {
}

#[derive(Fragment, Debug, Clone, Default)]
#[fragment(tag = "rect", bounds = rect)]
pub struct RectFragment {
    #[fragment(prop)]
    pub width: f64,
    #[fragment(prop)]
    pub height: f64,
    #[fragment(prop(js = "cornerRadius"), parse = radii)]
    pub corner_radii: RoundedRectRadii,
    #[fragment(prop, parse = brush, clear = none)]
    pub fill: Option<FragmentBrush>,
    #[fragment(prop, parse = shadow, clear = none)]
    pub shadow: Option<FragmentBoxShadow>,
    #[fragment(prop, parse = stroke_color, clear = none)]
    pub stroke: Option<StrokePaint>,
    #[fragment(prop(js = "strokeWidth"))]
    pub stroke_width: f64,
}

#[derive(Fragment, Debug, Clone, Default)]
#[fragment(tag = "circle", bounds = circle)]
pub struct CircleFragment {
    #[fragment(prop)]
    pub cx: f64,
    #[fragment(prop)]
    pub cy: f64,
    #[fragment(prop)]
    pub r: f64,
    #[fragment(prop, parse = color, clear = none)]
    pub fill: Option<FillPaint>,
    #[fragment(prop, parse = stroke_color, clear = none)]
    pub stroke: Option<StrokePaint>,
    #[fragment(prop(js = "strokeWidth"))]
    pub stroke_width: f64,
}

#[derive(Debug, Clone)]
pub struct TextStyleRun {
    pub text: String,
    pub font_size: f64,
    pub font_family: String,
    pub font_weight: i32,
    pub font_italic: bool,
    pub color: Color,
}

#[derive(Fragment, Debug, Clone)]
#[fragment(tag = "text", bounds = text)]
pub struct TextFragment {
    #[fragment(prop)]
    pub text: String,
    #[fragment(prop(js = "fontSize"))]
    pub font_size: f64,
    #[fragment(prop(js = "fontFamily"))]
    pub font_family: String,
    #[fragment(prop(js = "fontWeight"))]
    pub font_weight: f64,
    #[fragment(prop(js = "fontStyle"))]
    pub font_style: String,
    #[fragment(prop(js = "textMaxWidth"))]
    pub text_max_width: f64,
    #[fragment(prop(js = "textOverflow"))]
    pub text_overflow: String,
    #[fragment(prop, parse = plain_color, default = Color::from_rgba8(255, 255, 255, 255))]
    pub color: Color,
    #[fragment(skip)]
    pub shaped: Option<ShapedTextCache>,
}

impl Default for TextFragment {
    fn default() -> Self {
        Self {
            text: String::new(),
            font_size: 14.0,
            font_family: String::new(),
            font_weight: 0.0,
            font_style: String::new(),
            text_max_width: 0.0,
            text_overflow: String::new(),
            color: Color::from_rgba8(255, 255, 255, 255),
            shaped: None,
        }
    }
}

// ---------------------------------------------------------------------------
// TextInput fragment — editable text with cursor/selection state
// ---------------------------------------------------------------------------

/// Selection highlight color (semi-transparent blue).
const SELECTION_COLOR: Color = Color::from_rgba8(51, 153, 255, 128);
/// Caret color (white).
const CARET_COLOR: Color = Color::from_rgba8(255, 255, 255, 255);
/// Caret width in logical pixels.
const CARET_WIDTH: f64 = 1.0;

#[derive(Fragment, Debug, Clone)]
#[fragment(tag = "textinput", bounds = text_input)]
pub struct TextInputFragment {
    #[fragment(prop)]
    pub text: String,
    #[fragment(prop(js = "fontSize"))]
    pub font_size: f64,
    #[fragment(prop(js = "fontFamily"))]
    pub font_family: String,
    #[fragment(prop(js = "fontWeight"))]
    pub font_weight: f64,
    #[fragment(prop(js = "fontStyle"))]
    pub font_style: String,
    #[fragment(prop, parse = plain_color, default = Color::from_rgba8(255, 255, 255, 255))]
    pub color: Color,
    /// Cursor position in UTF-16 code units.
    #[fragment(prop(js = "cursorPos"))]
    pub cursor_pos: f64,
    /// Selection anchor in UTF-16 code units (-1 = no selection).
    #[fragment(prop(js = "selectionAnchor"))]
    pub selection_anchor: f64,
    #[fragment(skip)]
    pub layout: Option<ShapedTextLayout>,
    #[fragment(skip)]
    pub caret_visible: bool,
}

impl Default for TextInputFragment {
    fn default() -> Self {
        Self {
            text: String::new(),
            font_size: 14.0,
            font_family: String::new(),
            font_weight: 0.0,
            font_style: String::new(),
            color: Color::from_rgba8(255, 255, 255, 255),
            cursor_pos: 0.0,
            selection_anchor: -1.0,
            layout: None,
            caret_visible: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Span fragment — styled text run, must be a child of a TextFragment
// ---------------------------------------------------------------------------

#[derive(Fragment, Debug, Clone)]
#[fragment(tag = "span", bounds = none)]
pub struct SpanFragment {
    #[fragment(prop)]
    pub text: String,
    #[fragment(prop(js = "fontSize"))]
    pub font_size: f64,
    #[fragment(prop(js = "fontFamily"))]
    pub font_family: String,
    #[fragment(prop(js = "fontWeight"))]
    pub font_weight: f64,
    #[fragment(prop(js = "fontStyle"))]
    pub font_style: String,
    #[fragment(prop, parse = plain_color, default = Color::from_rgba8(255, 255, 255, 255))]
    pub color: Color,
}

impl Default for SpanFragment {
    fn default() -> Self {
        Self {
            text: String::new(),
            font_size: 0.0,
            font_family: String::new(),
            font_weight: 0.0,
            font_style: String::new(),
            color: Color::from_rgba8(255, 255, 255, 255),
        }
    }
}

// ---------------------------------------------------------------------------
// Path fragment — SVG path data rendering
// ---------------------------------------------------------------------------

#[derive(Fragment, Debug, Clone)]
#[fragment(tag = "path", bounds = rect)]
pub struct PathFragment {
    #[fragment(prop)]
    pub d: String,
    #[fragment(prop)]
    pub width: f64,
    #[fragment(prop)]
    pub height: f64,
    #[fragment(prop, parse = brush, clear = none)]
    pub fill: Option<FragmentBrush>,
    #[fragment(prop, parse = stroke_color, clear = none)]
    pub stroke: Option<StrokePaint>,
    #[fragment(prop(js = "strokeWidth"))]
    pub stroke_width: f64,
    #[fragment(skip)]
    pub parsed_path: Option<BezPath>,
}

impl Default for PathFragment {
    fn default() -> Self {
        Self {
            d: String::new(),
            width: 0.0,
            height: 0.0,
            fill: None,
            stroke: None,
            stroke_width: 0.0,
            parsed_path: None,
        }
    }
}

impl PathFragment {
    pub fn reparse_path(&mut self) {
        self.parsed_path = if self.d.is_empty() {
            None
        } else {
            BezPath::from_svg(&self.d).ok()
        };
    }
}

// ---------------------------------------------------------------------------
// Image fragment — decoded image rendering via anyrender image brush
// ---------------------------------------------------------------------------

#[derive(Fragment, Debug, Clone, Default)]
#[fragment(tag = "image", bounds = rect)]
pub struct ImageFragment {
    #[fragment(prop)]
    pub width: f64,
    #[fragment(prop)]
    pub height: f64,
    #[fragment(prop(js = "objectFit"))]
    pub object_fit: String,
    #[fragment(skip)]
    pub image_data: Option<ImageData>,
}

// ---------------------------------------------------------------------------
// FragmentEncode — hand-written paint encoding per kind
// ---------------------------------------------------------------------------

use super::vello::peniko::{Gradient, ColorStop as PenikoColorStop};

fn push_gradient_stops(gradient: &mut Gradient, stops: &[GradientStop]) {
    for stop in stops {
        gradient.stops.push(PenikoColorStop::from((stop.offset as f32, stop.color)));
    }
}

fn fill_brush_gradient(scene: &mut Scene, transform: Affine, brush: &FragmentBrush, fill_rule: Fill, path: &BezPath) {
    match brush {
        FragmentBrush::Solid(fill) => {
            scene.fill(fill.rule, transform, fill.color, None, path);
        }
        FragmentBrush::LinearGradient { start_x, start_y, end_x, end_y, stops } => {
            let mut gradient = Gradient::new_linear(
                Point::new(*start_x, *start_y),
                Point::new(*end_x, *end_y),
            );
            push_gradient_stops(&mut gradient, stops);
            scene.fill(fill_rule, transform, &gradient, Some(Affine::IDENTITY), path);
        }
        FragmentBrush::RadialGradient { center_x, center_y, radius, stops } => {
            let mut gradient = Gradient::new_radial(
                Point::new(*center_x, *center_y),
                *radius as f32,
            );
            push_gradient_stops(&mut gradient, stops);
            scene.fill(fill_rule, transform, &gradient, Some(Affine::IDENTITY), path);
        }
        FragmentBrush::SweepGradient { center_x, center_y, start_angle, end_angle, stops } => {
            let mut gradient = Gradient::new_sweep(
                Point::new(*center_x, *center_y),
                start_angle.to_radians() as f32,
                end_angle.to_radians() as f32,
            );
            push_gradient_stops(&mut gradient, stops);
            scene.fill(fill_rule, transform, &gradient, Some(Affine::IDENTITY), path);
        }
    }
}

impl FragmentEncode for GroupFragment {
    fn encode(&self, _scene: &mut Scene, _transform: Affine) {}
}

impl FragmentEncode for RectFragment {
    fn encode(&self, scene: &mut Scene, transform: Affine) {
        let rect = Rect::new(0.0, 0.0, self.width, self.height);
        let has_radius = self.corner_radii.as_single_radius().map_or(true, |r| r > 0.0);

        // Shadow (behind everything). Inset shadows are handled by the GPU effect pass.
        if let Some(shadow) = &self.shadow {
            if !shadow.inset {
                let sr = if has_radius { self.corner_radii.as_single_radius().unwrap_or(0.0) } else { 0.0 };
                let shadow_rect = Rect::new(
                    shadow.offset_x, shadow.offset_y,
                    self.width + shadow.offset_x, self.height + shadow.offset_y,
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
            scene.stroke(&Stroke::new(self.stroke_width.max(stroke.width)), transform, stroke.color, None, &path);
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
            scene.fill(fill.rule, transform, fill.color, None, &path);
        }
        if let Some(stroke) = &self.stroke {
            scene.stroke(&Stroke::new(self.stroke_width.max(stroke.width)), transform, stroke.color, None, &path);
        }
    }
}

impl FragmentEncode for PathFragment {
    fn encode(&self, scene: &mut Scene, transform: Affine) {
        let Some(path) = &self.parsed_path else { return };

        // Fill.
        if let Some(brush) = &self.fill {
            fill_brush_gradient(scene, transform, brush, Fill::NonZero, path);
        }

        // Stroke.
        if let Some(stroke) = &self.stroke {
            scene.stroke(&Stroke::new(self.stroke_width.max(stroke.width)), transform, stroke.color, None, path);
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
                if rg.image.width == 0 || rg.image.height == 0 { continue; }
                let sf = rg.scale_factor.max(1.0);
                let logical_w = rg.image.width as f64 / sf;
                let logical_h = rg.image.height as f64 / sf;
                let dest = Rect::new(rg.x, rg.y, rg.x + logical_w, rg.y + logical_h);
                let brush: ImageBrushRef = (&rg.image).into();
                let brush_transform = Affine::translate((rg.x, rg.y))
                    * Affine::scale_non_uniform(logical_w / rg.image.width as f64, logical_h / rg.image.height as f64);
                scene.fill(Fill::NonZero, transform, brush, Some(brush_transform), &dest);
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
            let x0 = layout.cursor_x_positions.get(sel_start.min(max_pos)).copied().unwrap_or(0.0);
            let x1 = layout.cursor_x_positions.get(sel_end.min(max_pos)).copied().unwrap_or(layout.width);
            let sel_rect = Rect::new(x0, 0.0, x1, layout.height);
            scene.fill(Fill::NonZero, transform, SELECTION_COLOR, None, &sel_rect);
        }

        // Draw caret (only when no selection and caret is visible).
        if !has_selection && self.caret_visible {
            let max_pos = layout.cursor_x_positions.len().saturating_sub(1);
            let cx = layout.cursor_x_positions.get(cursor.min(max_pos)).copied().unwrap_or(0.0);
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
        let Some(image_data) = &self.image_data else { return };
        if self.width <= 0.0 || self.height <= 0.0 { return; }

        let src_w = image_data.width as f64;
        let src_h = image_data.height as f64;
        if src_w <= 0.0 || src_h <= 0.0 { return; }

        // Compute brush_transform and the actual paint rect (may be smaller than dest for contain/none).
        let (brush_transform, paint_rect) = compute_image_fit(src_w, src_h, self.width, self.height, &self.object_fit);

        let brush: ImageBrushRef = image_data.into();
        scene.fill(Fill::NonZero, transform, brush, Some(brush_transform), &paint_rect);
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
        _ => (Affine::scale_non_uniform(dst_w / src_w, dst_h / src_h), dest),
    }
}

// ---------------------------------------------------------------------------
// FragmentData enum — derive generates from_tag, apply_prop, etc.
// ---------------------------------------------------------------------------

#[derive(Fragment, Debug, Clone)]
pub enum FragmentData {
    Group(GroupFragment),
    Rect(RectFragment),
    Circle(CircleFragment),
    Path(PathFragment),
    Text(TextFragment),
    TextInput(TextInputFragment),
    Span(SpanFragment),
    Image(ImageFragment),
}

// Also accept capitalized tags for backwards compat.
impl FragmentData {
    pub fn from_tag_loose(tag: &str) -> Option<Self> {
        Self::from_tag(tag).or_else(|| {
            let lower = tag.to_ascii_lowercase();
            Self::from_tag(&lower)
        })
    }
}

// ---------------------------------------------------------------------------
// Fragment visual props — user-specified via JSX / prop writes
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FragmentProps {
    pub explicit_x: Option<f64>,
    pub explicit_y: Option<f64>,
    pub explicit_width: Option<f64>,
    pub explicit_height: Option<f64>,
    pub opacity: f32,
    pub blend_mode: BlendMode,
    pub clip: bool,
    pub visible: bool,
    pub pointer_events: bool,
    pub cursor: u8,
    pub focusable: bool,
    pub transform: Affine,
    pub backdrop_blur: Option<f64>,
}

impl Default for FragmentProps {
    fn default() -> Self {
        Self {
            explicit_x: None,
            explicit_y: None,
            explicit_width: None,
            explicit_height: None,
            opacity: 1.0,
            blend_mode: BlendMode::default(),
            clip: false,
            visible: true,
            pointer_events: true,
            cursor: 0,
            focusable: false,
            transform: Affine::IDENTITY,
            backdrop_blur: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Layout result — taffy output after compute_layout
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default)]
pub struct LayoutResult {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl LayoutResult {
    /// Bounding rect at local origin (0,0) with layout-computed size.
    pub fn bounds(&self) -> Option<Rect> {
        if self.width > 0.0 || self.height > 0.0 {
            Some(Rect::new(0.0, 0.0, self.width, self.height))
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Fragment node
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FragmentNode {
    pub id: FragmentId,
    pub kind: FragmentData,
    pub props: FragmentProps,
    pub layout: LayoutResult,
    pub children: Vec<FragmentId>,
    pub parent: Option<FragmentId>,
    pub dirty: bool,
    pub pose_dirty: bool,
    pub taffy_node: Option<taffy::tree::NodeId>,
    pub promoted: bool,
    pub layer_key: Option<FragmentLayerKey>,
    pub timeline: Option<motion::NodeTimeline>,
    /// Cached world-space AABB (set by `recompute_aabbs`).
    world_aabb: Option<Rect>,
    /// Cached AABB enclosing this node and all descendants.
    subtree_aabb: Option<Rect>,
    pub listeners: FragmentListeners,
}

impl FragmentNode {
    fn render_x(&self) -> f64 {
        self.props.explicit_x.unwrap_or(self.layout.x)
    }

    fn render_y(&self) -> f64 {
        self.props.explicit_y.unwrap_or(self.layout.y)
    }

    fn local_transform(&self) -> Affine {
        Affine::translate((self.render_x(), self.render_y())) * self.props.transform
    }

    fn needs_layer(&self) -> bool {
        self.props.opacity < 1.0 - f32::EPSILON || self.props.clip || self.props.blend_mode != BlendMode::default()
    }

    /// Kind-level paint bounds, falling back to layout-computed bounds.
    fn effective_bounds(&self) -> Option<Rect> {
        self.kind.local_bounds().or_else(|| self.layout.bounds())
    }

    fn clip_rect(&self) -> Option<Rect> {
        if !self.props.clip {
            return None;
        }
        self.effective_bounds()
    }
}

// ---------------------------------------------------------------------------
// Motion pose → fragment property mapping
// ---------------------------------------------------------------------------

fn apply_sampled_pose_to_fragment(node: &mut FragmentNode, pose: &motion::SampledPose) {
    // Motion offset and layout FLIP are applied purely through `transform`,
    // keeping `explicit_x/y` untouched so that taffy layout (flex flow)
    // is not disrupted.  `explicit_x/y` remains under the control of the
    // user-set JSX `x`/`y` prop.
    node.props.opacity = pose.opacity as f32;

    // Origin values are [0,1] proportions (0.5 = center). Resolve to pixels.
    let bounds = node.effective_bounds().unwrap_or(Rect::ZERO);
    let origin_x = pose.origin_x * bounds.width();
    let origin_y = pose.origin_y * bounds.height();
    let rotate_rad = pose.rotate_deg.to_radians();

    // User transform: scale + rotate around user-specified origin
    let to_origin = Affine::translate((-origin_x, -origin_y));
    let from_origin = Affine::translate((origin_x, origin_y));
    let user_scale = Affine::scale_non_uniform(pose.scale_x, pose.scale_y);
    let rotate = Affine::rotate(rotate_rad);
    let user_scale_rotate = from_origin * user_scale * rotate * to_origin;

    // Layout FLIP scale: applied around top-left (0,0)
    let layout_scale = Affine::scale_non_uniform(pose.layout_scale_x, pose.layout_scale_y);

    // Motion offset (pose.x/y) + layout FLIP translation
    let motion_translate = Affine::translate((pose.x + pose.layout_x, pose.y + pose.layout_y));

    node.props.transform = motion_translate * layout_scale * user_scale_rotate;

    if node.promoted {
        // Promoted nodes: pose-only dirty — compositor handles transform/opacity.
        node.pose_dirty = true;
    } else {
        node.dirty = true;
    }
}

// ---------------------------------------------------------------------------
// Fragment tree
// ---------------------------------------------------------------------------

// TaffyTree internally holds raw pointers for its node context store,
// making it !Send. Our fragment store is only accessed from the main thread
// (libuv model); the Mutex is purely for Rust's type system. This is safe.
struct SendTaffy(TaffyTree<()>);
unsafe impl Send for SendTaffy {}
unsafe impl Sync for SendTaffy {}

impl std::ops::Deref for SendTaffy {
    type Target = TaffyTree<()>;
    fn deref(&self) -> &Self::Target { &self.0 }
}

impl std::ops::DerefMut for SendTaffy {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.0 }
}

impl std::fmt::Debug for SendTaffy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SendTaffy(..)")
    }
}

#[derive(Debug)]
pub struct FragmentTree {
    nodes: HashMap<FragmentId, FragmentNode>,
    root_children: Vec<FragmentId>,
    next_id: u32,
    cached_scene: Option<Scene>,
    any_dirty: bool,
    aabbs_dirty: bool,
    promoted_node_count: u32,
    next_layer_key: u32,
    promoted_scene_cache: HashMap<FragmentId, Scene>,
    previous_promoted_keys: HashSet<FragmentLayerKey>,
    /// Per-root-child subtree scene cache. Key = root child FragmentId.
    subtree_scene_cache: HashMap<FragmentId, Scene>,
    /// Root children whose subtree contains dirty nodes (cache invalid).
    dirty_root_children: HashSet<FragmentId>,
    taffy: SendTaffy,
    taffy_root: Option<taffy::tree::NodeId>,
    focused: Option<FragmentId>,
    /// Cached available size from last `compute_layout` call (logical pixels).
    last_layout_size: Option<(f64, f64)>,
    /// Per-fragment scroll offset. Only scroll containers have entries.
    scroll_offsets: HashMap<FragmentId, Vec2>,
    /// Fragment to highlight (devtools overlay).
    debug_highlight: Option<FragmentId>,
}

impl Default for FragmentTree {
    fn default() -> Self {
        let mut taffy = SendTaffy(TaffyTree::new());
        let taffy_root = taffy.new_leaf(taffy::Style {
            flex_shrink: 0.0,
            ..Default::default()
        }).unwrap();
        Self {
            nodes: HashMap::new(),
            root_children: Vec::new(),
            next_id: 0,
            cached_scene: None,
            any_dirty: false,
            aabbs_dirty: true,
            promoted_node_count: 0,
            next_layer_key: FRAGMENT_LAYER_KEY_BASE,
            promoted_scene_cache: HashMap::new(),
            previous_promoted_keys: HashSet::new(),
            subtree_scene_cache: HashMap::new(),
            dirty_root_children: HashSet::new(),
            taffy,
            taffy_root: Some(taffy_root),
            focused: None,
            last_layout_size: None,
            scroll_offsets: HashMap::new(),
            debug_highlight: None,
        }
    }
}

impl FragmentTree {
    pub fn new() -> Self {
        Self::default()
    }

    /// Dump the fragment tree to stderr for layout debugging.
    pub fn dump_layout(&self) {
        eprintln!("[frag-dump] root_children={:?}", self.root_children);
        let mut ids: Vec<_> = self.nodes.keys().copied().collect();
        ids.sort_by_key(|id| id.0);
        for id in ids {
            let node = &self.nodes[&id];
            let (tag, w, h) = match &node.kind {
                FragmentData::Rect(r) =>
                    (format!("rect fill={}", r.fill.is_some()), r.width, r.height),
                FragmentData::Text(t) =>
                    (format!("text \"{}\"", &t.text[..t.text.len().min(16)]), 0.0, 0.0),
                FragmentData::Group(_) =>
                    ("group".into(), node.layout.width, node.layout.height),
                _ => ("other".into(), 0.0, 0.0),
            };
            eprintln!(
                "[frag-dump] id={:<4} par={:<6} x={:<8.1} y={:<8.1} w={:<8.1} h={:<8.1} clip={} vis={} ch={:?}  {}",
                id.0,
                node.parent.map_or("-".into(), |p| p.0.to_string()),
                node.render_x(), node.render_y(), w, h,
                node.props.clip as u8, node.props.visible as u8,
                node.children.iter().map(|c| c.0).collect::<Vec<_>>(),
                tag,
            );
        }
    }

    pub fn allocate_id(&mut self) -> FragmentId {
        let id = FragmentId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Create a fragment node with the given kind and default properties.
    /// Returns the allocated ID. The node is **not** inserted into any
    /// parent's children list — call `insert_child` separately.
    pub fn create_node(&mut self, kind: FragmentData) -> FragmentId {
        let id = self.allocate_id();
        let is_span = matches!(kind, FragmentData::Span(_));
        let taffy_node = self.taffy.new_leaf(taffy::Style {
            flex_shrink: 0.0,
            ..Default::default()
        }).ok();
        self.nodes.insert(
            id,
            FragmentNode {
                id,
                kind,
                props: FragmentProps {
                    visible: !is_span,
                    pointer_events: !is_span,
                    ..Default::default()
                },
                layout: LayoutResult::default(),
                children: vec![],
                parent: None,
                dirty: true,
                pose_dirty: false,
                taffy_node,
                promoted: false,
                layer_key: None,
                timeline: None,
                world_aabb: None,
                subtree_aabb: None,
                listeners: FragmentListeners::empty(),
            },
        );
        self.invalidate();
        id
    }

    pub fn insert(&mut self, node: FragmentNode, parent: Option<FragmentId>) {
        let id = node.id;
        self.nodes.insert(id, node);
        match parent {
            Some(parent_id) => {
                if let Some(parent_node) = self.nodes.get_mut(&parent_id) {
                    parent_node.children.push(id);
                }
                self.nodes.get_mut(&id).map(|n| n.parent = Some(parent_id));
            }
            None => self.root_children.push(id),
        }
        self.invalidate();
    }

    /// Insert `child` into `parent`'s children list. If `before` is `Some`,
    /// insert before that sibling; otherwise append.
    /// If `parent` is `None`, insert into root children.
    pub fn insert_child(
        &mut self,
        parent: Option<FragmentId>,
        child: FragmentId,
        before: Option<FragmentId>,
    ) {
        if let Some(child_node) = self.nodes.get_mut(&child) {
            child_node.parent = parent;
        }

        let children = match parent {
            Some(parent_id) => {
                let Some(parent_node) = self.nodes.get_mut(&parent_id) else {
                    return;
                };
                &mut parent_node.children
            }
            None => &mut self.root_children,
        };

        if let Some(anchor) = before {
            if let Some(pos) = children.iter().position(|id| *id == anchor) {
                children.insert(pos, child);
                self.sync_taffy_children(parent);
                self.invalidate();
                return;
            }
        }
        children.push(child);
        self.sync_taffy_children(parent);
        self.invalidate();
    }

    /// Sync taffy tree children for a given parent (or root).
    fn sync_taffy_children(&mut self, parent: Option<FragmentId>) {
        let (fragment_children, taffy_parent) = match parent {
            Some(parent_id) => {
                let parent_node = self.nodes.get(&parent_id);
                let children = parent_node.map(|n| n.children.clone()).unwrap_or_default();
                let taffy_parent = parent_node.and_then(|n| n.taffy_node);
                (children, taffy_parent)
            }
            None => {
                (self.root_children.clone(), self.taffy_root)
            }
        };

        if let Some(tp) = taffy_parent {
            let taffy_children: Vec<taffy::tree::NodeId> = fragment_children
                .iter()
                .filter_map(|fid| self.nodes.get(fid).and_then(|n| n.taffy_node))
                .collect();
            let _ = self.taffy.set_children(tp, &taffy_children);
        }
    }

    /// Remove `child` from `parent`'s children list without destroying it.
    pub fn detach_child(&mut self, parent: Option<FragmentId>, child: FragmentId) {
        let children = match parent {
            Some(parent_id) => {
                let Some(parent_node) = self.nodes.get_mut(&parent_id) else {
                    return;
                };
                &mut parent_node.children
            }
            None => &mut self.root_children,
        };
        children.retain(|id| *id != child);

        if let Some(child_node) = self.nodes.get_mut(&child) {
            child_node.parent = None;
        }
        self.sync_taffy_children(parent);
        self.invalidate();
    }

    pub fn remove(&mut self, id: FragmentId) {
        if let Some(parent_id) = self.nodes.get(&id).and_then(|n| n.parent) {
            if let Some(parent_node) = self.nodes.get_mut(&parent_id) {
                parent_node.children.retain(|child| *child != id);
            }
        }
        self.root_children.retain(|child| *child != id);

        let mut to_remove = vec![id];
        let mut cursor = 0;
        while cursor < to_remove.len() {
            let current = to_remove[cursor];
            if let Some(node) = self.nodes.get(&current) {
                to_remove.extend_from_slice(&node.children);
            }
            cursor += 1;
        }

        for rid in &to_remove {
            self.scroll_offsets.remove(rid);
            if let Some(node) = self.nodes.remove(rid) {
                if node.promoted {
                    self.promoted_node_count = self.promoted_node_count.saturating_sub(1);
                    self.promoted_scene_cache.remove(rid);
                }
                if let Some(tn) = node.taffy_node {
                    let _ = self.taffy.remove(tn);
                }
            }
        }
        self.invalidate();
    }

    pub fn node(&self, id: FragmentId) -> Option<&FragmentNode> {
        self.nodes.get(&id)
    }

    pub fn node_mut(&mut self, id: FragmentId) -> Option<&mut FragmentNode> {
        self.nodes.get_mut(&id)
    }

    pub fn root_children(&self) -> &[FragmentId] {
        &self.root_children
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn has_promoted_nodes(&self) -> bool {
        self.promoted_node_count > 0
    }

    fn allocate_layer_key(&mut self) -> FragmentLayerKey {
        let key = FragmentLayerKey(self.next_layer_key);
        self.next_layer_key += 1;
        key
    }

    /// Ensure a promoted node has a stable layer key. Returns it.
    fn ensure_layer_key(&mut self, id: FragmentId) -> FragmentLayerKey {
        if let Some(key) = self.nodes.get(&id).and_then(|n| n.layer_key) {
            return key;
        }
        let key = self.allocate_layer_key();
        if let Some(node) = self.nodes.get_mut(&id) {
            node.layer_key = Some(key);
        }
        key
    }

    // -----------------------------------------------------------------------
    // Dirty tracking
    // -----------------------------------------------------------------------

    /// Walk from `id` up the parent chain to find the root child ancestor.
    fn root_child_ancestor(&self, id: FragmentId) -> Option<FragmentId> {
        let mut cur = id;
        loop {
            match self.nodes.get(&cur).and_then(|n| n.parent) {
                Some(p) => cur = p,
                None => {
                    // `cur` has no parent → it's a root child (if it exists in root_children).
                    return if self.root_children.contains(&cur) { Some(cur) } else { None };
                }
            }
        }
    }

    /// Invalidate the subtree scene cache for the root child that contains `id`.
    fn invalidate_subtree_cache_for(&mut self, id: FragmentId) {
        if let Some(rc) = self.root_child_ancestor(id) {
            self.dirty_root_children.insert(rc);
            self.subtree_scene_cache.remove(&rc);
        }
    }

    pub fn mark_dirty(&mut self, id: FragmentId) {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.dirty = true;
        }
        self.any_dirty = true;
        self.cached_scene = None;
        self.invalidate_subtree_cache_for(id);
    }

    fn invalidate(&mut self) {
        self.any_dirty = true;
        self.aabbs_dirty = true;
        self.cached_scene = None;
        self.subtree_scene_cache.clear();
        self.dirty_root_children.clear();
    }

    pub fn set_debug_highlight(&mut self, id: Option<FragmentId>) {
        if self.debug_highlight != id {
            self.debug_highlight = id;
            self.invalidate();
        }
    }

    // -----------------------------------------------------------------------
    // Layout (taffy)
    // -----------------------------------------------------------------------

    /// Run taffy layout and apply results to fragment nodes.
    pub fn compute_layout(&mut self, available_width: f64, available_height: f64) -> Vec<FragmentLayoutChange> {
        let Some(root) = self.taffy_root else { return Vec::new() };

        // Set root size to available space, preserving user-set style props.
        let mut root_style = self.taffy.style(root).cloned().unwrap_or_default();
        root_style.size = taffy::geometry::Size {
            width: taffy::style::Dimension::length(available_width as f32),
            height: taffy::style::Dimension::length(available_height as f32),
        };
        root_style.flex_shrink = 0.0;
        let _ = self.taffy.set_style(root, root_style);

        let available = taffy::geometry::Size {
            width: AvailableSpace::Definite(available_width as f32),
            height: AvailableSpace::Definite(available_height as f32),
        };

        // Sync fixed measure for text nodes with shaped cache.
        let text_sizes: Vec<(taffy::tree::NodeId, f32, f32)> = self.nodes.values()
            .filter_map(|node| {
                if let (Some(tn), FragmentData::Text(text)) = (node.taffy_node, &node.kind) {
                    text.shaped.as_ref().map(|s| (tn, s.width as f32, s.height as f32))
                } else {
                    None
                }
            })
            .collect();
        for (tn, w, h) in text_sizes {
            let mut style = self.taffy.style(tn).cloned().unwrap_or_default();
            style.size = taffy::geometry::Size {
                width: taffy::style::Dimension::length(w),
                height: taffy::style::Dimension::length(h),
            };
            let _ = self.taffy.set_style(tn, style);
        }

        // Sync fixed measure for text input nodes with layout cache.
        let input_sizes: Vec<(taffy::tree::NodeId, f32, f32)> = self.nodes.values()
            .filter_map(|node| {
                if let (Some(tn), FragmentData::TextInput(ti)) = (node.taffy_node, &node.kind) {
                    ti.layout.as_ref().map(|l| (tn, l.width as f32, l.height as f32))
                } else {
                    None
                }
            })
            .collect();
        for (tn, w, h) in input_sizes {
            let mut style = self.taffy.style(tn).cloned().unwrap_or_default();
            style.size = taffy::geometry::Size {
                width: taffy::style::Dimension::length(w),
                height: taffy::style::Dimension::length(h),
            };
            let _ = self.taffy.set_style(tn, style);
        }

        // Sync fixed measure for circle nodes from radius.
        let circle_sizes: Vec<(taffy::tree::NodeId, f32)> = self.nodes.values()
            .filter_map(|node| {
                if let (Some(tn), FragmentData::Circle(circle)) = (node.taffy_node, &node.kind) {
                    if circle.r > 0.0 && node.props.explicit_width.is_none() && node.props.explicit_height.is_none() {
                        Some((tn, (circle.r * 2.0) as f32))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();
        for (tn, diameter) in circle_sizes {
            let mut style = self.taffy.style(tn).cloned().unwrap_or_default();
            if style.size.width == taffy::style::Dimension::auto() {
                style.size.width = taffy::style::Dimension::length(diameter);
            }
            if style.size.height == taffy::style::Dimension::auto() {
                style.size.height = taffy::style::Dimension::length(diameter);
            }
            let _ = self.taffy.set_style(tn, style);
        }

        let _ = self.taffy.compute_layout(root, available);
        let events = self.apply_layout_results();
        self.last_layout_size = Some((available_width, available_height));
        self.aabbs_dirty = true;
        events
    }

    fn apply_layout_results(&mut self) -> Vec<FragmentLayoutChange> {
        let ids: Vec<FragmentId> = self.nodes.keys().copied().collect();
        let mut layout_dirty_ids: Vec<FragmentId> = Vec::new();
        let mut layout_events: Vec<FragmentLayoutChange> = Vec::new();
        for id in ids {
            let Some(taffy_node) = self.nodes.get(&id).and_then(|n| n.taffy_node) else {
                continue;
            };
            let Ok(layout) = self.taffy.layout(taffy_node) else {
                continue;
            };
            let lx = layout.location.x as f64;
            let ly = layout.location.y as f64;
            let lw = layout.size.width as f64;
            let lh = layout.size.height as f64;

            let node = self.nodes.get_mut(&id).unwrap();
            let has_listener = node.listeners.contains(FragmentListeners::LAYOUT);

            let pos_changed = (node.layout.x - lx).abs() > 0.01
                || (node.layout.y - ly).abs() > 0.01;
            if pos_changed {
                node.layout.x = lx;
                node.layout.y = ly;
                if node.props.explicit_x.is_none() || node.props.explicit_y.is_none() {
                    node.dirty = true;
                    self.any_dirty = true;
                    self.cached_scene = None;
                    layout_dirty_ids.push(id);
                }
            }

            // Sync layout size for all nodes (used for clip, hit-test, AABB).
            let layout_size_changed = (node.layout.width - lw).abs() > 0.01
                || (node.layout.height - lh).abs() > 0.01;
            if layout_size_changed {
                node.layout.width = lw;
                node.layout.height = lh;
                node.dirty = true;
                self.any_dirty = true;
                self.cached_scene = None;
                layout_dirty_ids.push(id);
            }

            // Sync kind-level paint geometry for Rect/Image (their encode reads width/height).
            let mut paint_size_changed = false;
            match &mut node.kind {
                FragmentData::Rect(rect) => {
                    let ew = node.props.explicit_width.is_some();
                    let eh = node.props.explicit_height.is_some();
                    let new_w = if ew { rect.width } else { lw };
                    let new_h = if eh { rect.height } else { lh };
                    if (rect.width - new_w).abs() > 0.01 || (rect.height - new_h).abs() > 0.01 {
                        paint_size_changed = true;
                        rect.width = new_w;
                        rect.height = new_h;
                    }
                }
                FragmentData::Image(img) => {
                    let ew = node.props.explicit_width.is_some();
                    let eh = node.props.explicit_height.is_some();
                    let new_w = if ew { img.width } else { lw };
                    let new_h = if eh { img.height } else { lh };
                    if (img.width - new_w).abs() > 0.01 || (img.height - new_h).abs() > 0.01 {
                        paint_size_changed = true;
                        img.width = new_w;
                        img.height = new_h;
                    }
                }
                _ => {}
            }
            if paint_size_changed {
                node.dirty = true;
                self.any_dirty = true;
                self.cached_scene = None;
                layout_dirty_ids.push(id);
            }

            if has_listener && (pos_changed || layout_size_changed || paint_size_changed) {
                layout_events.push(FragmentLayoutChange {
                    fragment_id: id,
                    x: lx,
                    y: ly,
                    width: lw,
                    height: lh,
                });
            }
        }
        for id in layout_dirty_ids {
            self.invalidate_subtree_cache_for(id);
        }
        layout_events
    }

    /// Modify taffy style for a fragment node.
    pub fn with_taffy_style_mut(
        &mut self,
        id: FragmentId,
        f: impl FnOnce(&mut taffy::Style),
    ) {
        let Some(taffy_node) = self.nodes.get(&id).and_then(|n| n.taffy_node) else {
            return;
        };
        let Ok(current) = self.taffy.style(taffy_node).cloned() else {
            return;
        };
        let mut style = current;
        f(&mut style);
        let _ = self.taffy.set_style(taffy_node, style);
    }

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
        let mut timeline = node.timeline.take().unwrap_or_else(motion::NodeTimeline::new);
        timeline.set_targets(targets, default_transition, per_property, now, delay_secs);
        let (sampled, animating) = timeline.sample_pose(now);
        apply_sampled_pose_to_fragment(node, &sampled);
        if !animating {
            timeline.gc_completed();
        }
        node.timeline = Some(timeline);
        self.invalidate();
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
        let mut timeline = node.timeline.take().unwrap_or_else(motion::NodeTimeline::new);
        timeline.set_targets_keyframes(targets, times, default_transition, per_property, now, delay_secs);
        let (sampled, animating) = timeline.sample_pose(now);
        apply_sampled_pose_to_fragment(node, &sampled);
        if !animating {
            timeline.gc_completed();
        }
        node.timeline = Some(timeline);
        self.invalidate();
        animating
    }

    /// Tick all fragment timelines. Returns (still_animating, completed_fragment_ids).
    pub fn tick_motion(&mut self, now: f64) -> (bool, Vec<FragmentId>) {
        let ids: Vec<FragmentId> = self.nodes.keys().copied().collect();
        let mut any_animating = false;
        let mut completed = Vec::new();
        let mut any_non_promoted_sampled = false;
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
            apply_sampled_pose_to_fragment(node, &sampled);
            // Any sampled non-promoted node needs scene rebuild — including
            // the final frame where animating becomes false.
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
        // Invalidate scene cache if any non-promoted node was sampled (even final frame).
        if any_non_promoted_sampled {
            self.invalidate();
        } else if any_animating {
            // Promoted-only: still need aabb refresh but skip scene cache clear.
            self.aabbs_dirty = true;
        }
        (any_animating, completed)
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
            self.invalidate();
        }
    }

    /// Get the content size of a fragment node from its taffy layout.
    /// Returns (content_width, content_height) or None if the node has no taffy node.
    pub fn get_content_size(&self, id: FragmentId) -> Option<(f64, f64)> {
        let node = self.nodes.get(&id)?;
        let taffy_node = node.taffy_node?;
        let layout = self.taffy.layout(taffy_node).ok()?;
        Some((layout.content_size.width as f64, layout.content_size.height as f64))
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
        let mut timeline = node.timeline.take().unwrap_or_else(motion::NodeTimeline::new);

        // Step 1: instantly snap to inverted delta
        let invert_targets = [
            (motion::PropertyKey::LayoutX, dx),
            (motion::PropertyKey::LayoutY, dy),
            (motion::PropertyKey::LayoutScaleX, sx),
            (motion::PropertyKey::LayoutScaleY, sy),
        ];
        let instant = motion::TransitionSpec::Instant;
        let empty_per_prop = std::collections::HashMap::new();
        timeline.set_targets(&invert_targets, &instant, &empty_per_prop, now, 0.0);

        // Step 2: animate to identity
        let identity_targets = [
            (motion::PropertyKey::LayoutX, 0.0),
            (motion::PropertyKey::LayoutY, 0.0),
            (motion::PropertyKey::LayoutScaleX, 1.0),
            (motion::PropertyKey::LayoutScaleY, 1.0),
        ];
        timeline.set_targets(&identity_targets, transition, &empty_per_prop, now, 0.0);

        let (sampled, animating) = timeline.sample_pose(now);
        apply_sampled_pose_to_fragment(node, &sampled);
        if !animating {
            timeline.gc_completed();
        }
        node.timeline = Some(timeline);
        self.invalidate();
        animating
    }

    // -----------------------------------------------------------------------
    // Recursive paint with tree-level scene cache
    // -----------------------------------------------------------------------

    pub fn paint_into_scene(&mut self, scene: &mut Scene, base_transform: Affine) {
        // Fast path: no promoted nodes → per-subtree cached scene path.
        if self.promoted_node_count == 0 {
            // Level-0 cache: nothing dirty at all → reuse full scene.
            if !self.any_dirty {
                if let Some(cached) = &self.cached_scene {
                    scene.append_scene(cached.clone(), base_transform);
                    return;
                }
            }

            // Level-1 cache: per-root-child subtree caching.
            let root_children = self.root_children.clone();
            let mut fresh = Scene::new();
            for &child_id in &root_children {
                if let Some(cached) = self.subtree_scene_cache.get(&child_id) {
                    fresh.append_scene(cached.clone(), Affine::IDENTITY);
                } else {
                    let mut sub = Scene::new();
                    self.paint_node(&mut sub, child_id, Affine::IDENTITY);
                    fresh.append_scene(sub.clone(), Affine::IDENTITY);
                    self.subtree_scene_cache.insert(child_id, sub);
                }
            }

            for node in self.nodes.values_mut() {
                node.dirty = false;
                node.pose_dirty = false;
            }
            self.any_dirty = false;
            self.dirty_root_children.clear();

            scene.append_scene(fresh.clone(), base_transform);
            self.cached_scene = Some(fresh);
            self.paint_debug_highlight(scene, base_transform);
            return;
        }

        // Split path: build paint plan, append inline chunks to scene.
        // build_paint_plan resets dirty state internally.
        let plan = self.build_paint_plan();
        for chunk in plan.chunks {
            match chunk {
                PaintChunk::Inline(inline_scene) => {
                    scene.append_scene(inline_scene, base_transform);
                }
                PaintChunk::Promoted(layer) => {
                    // P1: fall back to painting promoted content inline too,
                    // so visual output stays correct before compositor integration.
                    scene.append_scene(layer.scene, base_transform);
                }
            }
        }
        self.paint_debug_highlight(scene, base_transform);
    }

    fn paint_node(&self, scene: &mut Scene, id: FragmentId, parent_transform: Affine) {
        let Some(node) = self.node(id) else { return };
        if !node.props.visible { return; }

        let transform = parent_transform * node.local_transform();
        let scroll = self.scroll_offsets.get(&id).copied();
        let needs_layer = node.needs_layer() || scroll.is_some();

        if needs_layer {
            // Scroll containers need clip even without opacity/clip props.
            let clip_rect = node.clip_rect().or_else(|| {
                scroll.and_then(|_| node.effective_bounds())
            });
            push_fragment_layer(scene, transform, clip_rect, node.props.opacity, node.props.blend_mode);
        }

        node.kind.encode(scene, transform);

        let child_transform = match scroll {
            Some(s) => transform * Affine::translate((-s.x, -s.y)),
            None => transform,
        };

        let children = node.children.clone();
        for child_id in children {
            self.paint_node(scene, child_id, child_transform);
        }

        if needs_layer {
            scene.pop_layer();
        }
    }

    /// Paint a single fragment node (self only, no children) into the scene.
    pub fn paint_node_self_only(&self, scene: &mut Scene, id: FragmentId, parent_transform: Affine) {
        let Some(node) = self.node(id) else { return };
        if !node.props.visible { return; }

        let transform = parent_transform * node.local_transform();
        let needs_layer = node.needs_layer();

        if needs_layer {
            let clip_rect = node.clip_rect();
            push_fragment_layer(scene, transform, clip_rect, node.props.opacity, node.props.blend_mode);
        }

        node.kind.encode(scene, transform);

        if needs_layer {
            scene.pop_layer();
        }
    }

    /// Paint a single node at an explicit transform, ignoring the node's own
    /// position/transform (caller supplies the final placement).  Used by
    /// isolated-fragment capture where we want the content at the viewport
    /// origin regardless of where the node sits in the tree.
    pub fn paint_node_at_origin(&self, scene: &mut Scene, id: FragmentId, transform: Affine) {
        let Some(node) = self.node(id) else { return };
        if !node.props.visible { return; }

        let needs_layer = node.needs_layer();
        if needs_layer {
            let clip_rect = node.clip_rect();
            push_fragment_layer(scene, transform, clip_rect, node.props.opacity, node.props.blend_mode);
        }

        node.kind.encode(scene, transform);

        if needs_layer {
            scene.pop_layer();
        }
    }

    fn paint_debug_highlight(&mut self, scene: &mut Scene, base_transform: Affine) {
        let Some(hl_id) = self.debug_highlight else { return };
        self.ensure_aabbs();
        let Some(bounds) = self.nodes.get(&hl_id).and_then(|n| n.world_aabb) else { return };

        let path = BezPath::from_vec(bounds.path_elements(0.1).collect());

        // Semi-transparent amber fill
        scene.fill(Fill::NonZero, base_transform, Color::from_rgba8(255, 191, 0, 32), None, &path);

        // Amber border
        scene.stroke(&Stroke::new(2.0), base_transform, Color::from_rgba8(255, 191, 0, 220), None, &path);
    }

    // -----------------------------------------------------------------------
    // Scene splitting — PaintCollector
    // -----------------------------------------------------------------------

    pub fn build_paint_plan(&mut self) -> PaintPlan {
        // Take scene cache out so paint_node_collecting (which borrows &self)
        // can reference it without conflicting with &mut self.
        let reuse_cache = !self.any_dirty;
        let mut scene_cache = std::mem::take(&mut self.promoted_scene_cache);

        let mut collector = PaintCollector {
            chunks: Vec::new(),
            current_inline: Scene::new(),
            layer_stack: Vec::new(),
        };
        let root_children = self.root_children.clone();
        for &child_id in &root_children {
            Self::paint_node_collecting_cached(
                &self.nodes,
                &self.scroll_offsets,
                &mut collector,
                child_id,
                Affine::IDENTITY,
                reuse_cache,
                &mut scene_cache,
            );
        }
        collector.flush_inline();

        // Assign stable layer keys to promoted layers.
        for chunk in &mut collector.chunks {
            if let PaintChunk::Promoted(layer) = chunk {
                layer.layer_key = self.ensure_layer_key(layer.fragment_id);
            }
        }

        // Compute stale keys: previous − current.
        let current_keys: HashSet<FragmentLayerKey> = collector.chunks.iter()
            .filter_map(|c| match c {
                PaintChunk::Promoted(layer) => Some(layer.layer_key),
                _ => None,
            })
            .collect();
        let stale_keys: Vec<FragmentLayerKey> = self.previous_promoted_keys
            .difference(&current_keys)
            .copied()
            .collect();
        self.previous_promoted_keys = current_keys;

        // Write back scene cache, pruning entries for non-promoted nodes.
        scene_cache.retain(|id, _| {
            self.nodes.get(id).map_or(false, |n| n.promoted)
        });
        self.promoted_scene_cache = scene_cache;

        // Reset dirty state (same as paint_into_scene).
        for node in self.nodes.values_mut() {
            node.dirty = false;
            node.pose_dirty = false;
        }
        self.any_dirty = false;
        self.cached_scene = None;

        PaintPlan { chunks: collector.chunks, stale_keys }
    }

    /// Collect inner shadow effects from RectFragments with inset shadows.
    /// Returns effects in device pixel coordinates.
    pub fn collect_inner_shadow_effects(&mut self, scale_factor: f64) -> Vec<InnerShadowEffect> {
        self.ensure_aabbs();
        let mut effects = Vec::new();
        for node in self.nodes.values() {
            if let FragmentData::Rect(rect) = &node.kind {
                if let Some(shadow) = &rect.shadow {
                    if !shadow.inset { continue; }
                    let Some(world_aabb) = node.world_aabb else { continue };
                    let sf = scale_factor as f32;
                    let r = rect.corner_radii.as_single_radius().unwrap_or(0.0) as f32 * sf;
                    let rgba = shadow.color.to_rgba8();
                    let a = rgba.a as f32 / 255.0;
                    effects.push(InnerShadowEffect {
                        rect_min: [world_aabb.x0 as f32 * sf, world_aabb.y0 as f32 * sf],
                        rect_size: [world_aabb.width() as f32 * sf, world_aabb.height() as f32 * sf],
                        corner_radius: r,
                        offset: [shadow.offset_x as f32 * sf, shadow.offset_y as f32 * sf],
                        blur_std_dev: shadow.blur as f32 * sf,
                        color: [
                            (rgba.r as f32 / 255.0) * a,
                            (rgba.g as f32 / 255.0) * a,
                            (rgba.b as f32 / 255.0) * a,
                            a,
                        ],
                    });
                }
            }
        }
        effects
    }

    /// Collect backdrop blur effects from fragment nodes with `backdrop_blur` set.
    /// Returns effects in device pixel coordinates.
    pub fn collect_backdrop_blur_effects(&mut self, scale_factor: f64) -> Vec<BackdropBlurEffect> {
        self.ensure_aabbs();
        let mut effects = Vec::new();
        for node in self.nodes.values() {
            let Some(blur_radius) = node.props.backdrop_blur else { continue };
            if blur_radius <= 0.0 { continue; }
            let Some(world_aabb) = node.world_aabb else { continue };
            let sf = scale_factor as f32;
            let corner_radius = match &node.kind {
                FragmentData::Rect(rect) => rect.corner_radii.as_single_radius().unwrap_or(0.0) as f32 * sf,
                _ => 0.0,
            };
            effects.push(BackdropBlurEffect {
                rect_min: [world_aabb.x0 as f32 * sf, world_aabb.y0 as f32 * sf],
                rect_size: [world_aabb.width() as f32 * sf, world_aabb.height() as f32 * sf],
                corner_radius,
                blur_radius: blur_radius as f32 * sf,
            });
        }
        effects
    }

    fn paint_node_collecting_cached(
        nodes: &HashMap<FragmentId, FragmentNode>,
        scroll_offsets: &HashMap<FragmentId, Vec2>,
        collector: &mut PaintCollector,
        id: FragmentId,
        parent_transform: Affine,
        reuse_cache: bool,
        scene_cache: &mut HashMap<FragmentId, Scene>,
    ) {
        let Some(node) = nodes.get(&id) else { return };
        if !node.props.visible { return; }

        let transform = parent_transform * node.local_transform();

        // If this node is promotion-eligible, split.
        if node.promoted && Self::is_promotion_eligible_static(nodes, id) {
            collector.flush_inline_for_split();

            // Render subtree in local space: content relative to promoted root origin.
            // The root's local_transform() is NOT applied to content — compositor
            // will apply the world transform to position the quad.
            let subtree_scene = if reuse_cache {
                if let Some(cached) = scene_cache.get(&id) {
                    let mut copy = Scene::new();
                    copy.append_scene(cached.clone(), Affine::IDENTITY);
                    copy
                } else {
                    let mut s = Scene::new();
                    Self::paint_promoted_subtree_local(nodes, scroll_offsets, &mut s, id);
                    s
                }
            } else {
                let mut s = Scene::new();
                Self::paint_promoted_subtree_local(nodes, scroll_offsets, &mut s, id);
                s
            };

            // Update cache with current scene content.
            if !reuse_cache || !scene_cache.contains_key(&id) {
                let mut cache_copy = Scene::new();
                cache_copy.append_scene(subtree_scene.clone(), Affine::IDENTITY);
                scene_cache.insert(id, cache_copy);
            }

            // Subtree bounds in promoted root's local space for texture sizing.
            let bounds = Self::compute_subtree_local_bounds(nodes, id, Affine::IDENTITY)
                .unwrap_or(Rect::ZERO);
            let clip_rect = collector.accumulated_clip_rect();

            collector.chunks.push(PaintChunk::Promoted(PromotedLayer {
                fragment_id: id,
                layer_key: FragmentLayerKey(0),
                scene: subtree_scene,
                bounds,
                transform,
                clip_rect,
                opacity: node.props.opacity,
                blend_mode: node.props.blend_mode,
            }));

            collector.resume_inline_after_split();
            return;
        }

        // Normal inline paint.
        let scroll = scroll_offsets.get(&id).copied();
        let needs_layer = node.needs_layer() || scroll.is_some();
        if needs_layer {
            let clip_rect = node.clip_rect().or_else(|| {
                scroll.and_then(|_| node.effective_bounds())
            });
            collector.push_layer(transform, clip_rect, node.props.opacity, node.props.blend_mode);
        }

        node.kind.encode(&mut collector.current_inline, transform);

        let child_transform = match scroll {
            Some(s) => transform * Affine::translate((-s.x, -s.y)),
            None => transform,
        };

        let children = node.children.clone();
        for child_id in children {
            Self::paint_node_collecting_cached(
                nodes, scroll_offsets, collector, child_id, child_transform,
                reuse_cache, scene_cache,
            );
        }

        if needs_layer {
            collector.pop_layer();
        }
    }

    fn paint_node_static(
        nodes: &HashMap<FragmentId, FragmentNode>,
        scroll_offsets: &HashMap<FragmentId, Vec2>,
        scene: &mut Scene,
        id: FragmentId,
        parent_transform: Affine,
    ) {
        let Some(node) = nodes.get(&id) else { return };
        if !node.props.visible { return; }

        let transform = parent_transform * node.local_transform();
        let scroll = scroll_offsets.get(&id).copied();
        let needs_layer = node.needs_layer() || scroll.is_some();

        if needs_layer {
            let clip_rect = node.clip_rect().or_else(|| {
                scroll.and_then(|_| node.effective_bounds())
            });
            push_fragment_layer(scene, transform, clip_rect, node.props.opacity, node.props.blend_mode);
        }

        node.kind.encode(scene, transform);

        let child_transform = match scroll {
            Some(s) => transform * Affine::translate((-s.x, -s.y)),
            None => transform,
        };

        let children = node.children.clone();
        for child_id in children {
            Self::paint_node_static(nodes, scroll_offsets, scene, child_id, child_transform);
        }

        if needs_layer {
            scene.pop_layer();
        }
    }

    /// Render a promoted subtree in local space: content at origin, no root
    /// local_transform applied. The root's kind is encoded at IDENTITY, children
    /// are relative to the root. Compositor applies the world transform externally.
    fn paint_promoted_subtree_local(
        nodes: &HashMap<FragmentId, FragmentNode>,
        scroll_offsets: &HashMap<FragmentId, Vec2>,
        scene: &mut Scene,
        id: FragmentId,
    ) {
        let Some(node) = nodes.get(&id) else { return };
        if !node.props.visible { return; }

        // Encode root kind at identity (no root x/y/transform applied).
        node.kind.encode(scene, Affine::IDENTITY);

        // Children are painted relative to the root's local space.
        for &child_id in &node.children {
            Self::paint_node_static(nodes, scroll_offsets, scene, child_id, Affine::IDENTITY);
        }
    }

    fn is_promotion_eligible_static(
        nodes: &HashMap<FragmentId, FragmentNode>,
        id: FragmentId,
    ) -> bool {
        let node = match nodes.get(&id) {
            Some(n) => n,
            None => return false,
        };
        if !node.promoted {
            return false;
        }
        // Reject if subtree bounds unknown (empty subtree).
        if Self::compute_subtree_local_bounds(nodes, id, Affine::IDENTITY).is_none() {
            return false;
        }
        let mut cursor = node.parent;
        let mut composed = Affine::IDENTITY;
        while let Some(pid) = cursor {
            let Some(parent) = nodes.get(&pid) else {
                break;
            };
            if parent.promoted {
                return false;
            }
            if parent.props.opacity < 1.0 - f32::EPSILON {
                return false;
            }
            composed = parent.props.transform * composed;
            if parent.props.clip && !is_axis_aligned_affine(composed) {
                return false;
            }
            cursor = parent.parent;
        }
        true
    }

    /// Compute the axis-aligned bounding box of an entire subtree in the
    /// coordinate space of the given `parent_transform`.
    fn compute_subtree_local_bounds(
        nodes: &HashMap<FragmentId, FragmentNode>,
        id: FragmentId,
        parent_transform: Affine,
    ) -> Option<Rect> {
        let node = nodes.get(&id)?;
        let transform = parent_transform * node.local_transform();

        let mut result: Option<Rect> = node
            .effective_bounds()
            .map(|lb| transform_local_bounds_to_world(lb, transform));

        for &child_id in &node.children {
            if let Some(child_bounds) =
                Self::compute_subtree_local_bounds(nodes, child_id, transform)
            {
                result = Some(match result {
                    Some(r) => r.union(child_bounds),
                    None => child_bounds,
                });
            }
        }
        result
    }
}

// ---------------------------------------------------------------------------
// PaintCollector — maintains clip/layer stack for scene splitting
// ---------------------------------------------------------------------------

struct LayerEntry {
    transform: Affine,
    clip_rect: Option<Rect>,
    opacity: f32,
    blend_mode: BlendMode,
}

struct PaintCollector {
    chunks: Vec<PaintChunk>,
    current_inline: Scene,
    layer_stack: Vec<LayerEntry>,
}

impl PaintCollector {
    fn push_layer(&mut self, transform: Affine, clip_rect: Option<Rect>, opacity: f32, blend_mode: BlendMode) {
        push_fragment_layer(&mut self.current_inline, transform, clip_rect, opacity, blend_mode);
        self.layer_stack.push(LayerEntry { transform, clip_rect, opacity, blend_mode });
    }

    fn pop_layer(&mut self) {
        self.current_inline.pop_layer();
        self.layer_stack.pop();
    }

    /// Flush current inline scene as a chunk if non-empty.
    fn flush_inline(&mut self) {
        let scene = std::mem::replace(&mut self.current_inline, Scene::new());
        if !scene.commands.is_empty() {
            self.chunks.push(PaintChunk::Inline(scene));
        }
    }

    /// Close all active layers on the current inline scene, then flush it.
    fn flush_inline_for_split(&mut self) {
        // Balance: pop all active layers so the inline scene is self-contained.
        for _ in 0..self.layer_stack.len() {
            self.current_inline.pop_layer();
        }
        self.flush_inline();
    }

    /// Start a fresh inline scene and replay the active layer stack into it.
    fn resume_inline_after_split(&mut self) {
        self.current_inline = Scene::new();
        for entry in &self.layer_stack {
            push_fragment_layer(&mut self.current_inline, entry.transform, entry.clip_rect, entry.opacity, entry.blend_mode);
        }
    }

    /// Compute the accumulated clip rect from the layer stack (in window coords).
    fn accumulated_clip_rect(&self) -> Option<Rect> {
        let mut result: Option<Rect> = None;
        for entry in &self.layer_stack {
            if let Some(clip) = &entry.clip_rect {
                let world_clip = transform_local_bounds_to_world(*clip, entry.transform);
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

fn is_axis_aligned_affine(t: Affine) -> bool {
    let coeffs = t.as_coeffs();
    // Affine coeffs: [a, b, c, d, e, f] → matrix [[a,c],[b,d]] + [e,f]
    // Axis-aligned if off-diagonal (b, c) are ~zero.
    coeffs[1].abs() < 1e-9 && coeffs[2].abs() < 1e-9
}

fn transform_local_bounds_to_world(local_bounds: Rect, transform: Affine) -> Rect {
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

fn union_rect(a: Option<Rect>, b: Rect) -> Rect {
    match a {
        Some(a) => a.union(b),
        None => b,
    }
}

impl FragmentTree {
    fn ensure_aabbs(&mut self) {
        if !self.aabbs_dirty {
            return;
        }
        let root_children = self.root_children.clone();
        for &child_id in &root_children {
            self.recompute_aabb(child_id, Affine::IDENTITY);
        }
        self.aabbs_dirty = false;
    }

    fn recompute_aabb(&mut self, id: FragmentId, parent_transform: Affine) -> Option<Rect> {
        let (local_bounds, local_transform, children) = {
            let node = self.nodes.get(&id)?;
            (
                node.effective_bounds(),
                parent_transform * node.local_transform(),
                node.children.clone(),
            )
        };

        let world_aabb = local_bounds.map(|lb| transform_local_bounds_to_world(lb, local_transform));

        let child_transform = match self.scroll_offsets.get(&id) {
            Some(s) => local_transform * Affine::translate((-s.x, -s.y)),
            None => local_transform,
        };

        let mut subtree = world_aabb;
        for &child_id in &children {
            if let Some(child_subtree) = self.recompute_aabb(child_id, child_transform) {
                subtree = Some(union_rect(subtree, child_subtree));
            }
        }

        if let Some(node) = self.nodes.get_mut(&id) {
            node.world_aabb = world_aabb;
            node.subtree_aabb = subtree;
        }

        subtree
    }
}

// ---------------------------------------------------------------------------
// Hit test — reverse paint order (on FragmentTree)
// ---------------------------------------------------------------------------

impl FragmentTree {
    pub fn hit_test(&mut self, point: (f64, f64)) -> Option<FragmentId> {
        self.ensure_aabbs();
        let world_point = Point::new(point.0, point.1);
        self.hit_test_children(&self.root_children.clone(), Affine::IDENTITY, point, world_point)
    }

    fn hit_test_children(
        &self,
        children: &[FragmentId],
        parent_transform: Affine,
        point: (f64, f64),
        world_point: Point,
    ) -> Option<FragmentId> {
        for &child_id in children.iter().rev() {
            if let Some(hit) = self.hit_test_node(child_id, parent_transform, point, world_point) {
                return Some(hit);
            }
        }
        None
    }

    fn hit_test_node(
        &self,
        id: FragmentId,
        parent_transform: Affine,
        point: (f64, f64),
        world_point: Point,
    ) -> Option<FragmentId> {
        let node = self.node(id)?;

        if !node.props.visible || !node.props.pointer_events {
            return None;
        }

        // Prune: if subtree AABB doesn't contain the point, skip entirely.
        if let Some(subtree_aabb) = node.subtree_aabb {
            if !subtree_aabb.contains(world_point) {
                return None;
            }
        }

        let transform = parent_transform * node.local_transform();

        if node.props.clip {
            if let Some(clip_rect) = node.effective_bounds() {
                let inverse = transform.inverse();
                let local = inverse * Point::new(point.0, point.1);
                if !clip_rect.contains(local) {
                    return None;
                }
            }
        }

        let child_transform = match self.scroll_offsets.get(&id) {
            Some(s) => transform * Affine::translate((-s.x, -s.y)),
            None => transform,
        };
        let children = node.children.clone();
        if let Some(hit) = self.hit_test_children(&children, child_transform, point, world_point) {
            return Some(hit);
        }

        // Prune: check world AABB before computing inverse transform.
        if let Some(world_aabb) = node.world_aabb {
            if !world_aabb.contains(world_point) {
                return None;
            }
        }

        if let Some(bounds) = node.effective_bounds() {
            let inverse = transform.inverse();
            let local = inverse * Point::new(point.0, point.1);
            if bounds.contains(local) {
                return Some(id);
            }
        }

        None
    }

    // -----------------------------------------------------------------------
    // Focus management
    // -----------------------------------------------------------------------

    /// Collect focusable fragment IDs in DFS (pre-order) traversal order.
    fn focusable_ids(&self) -> Vec<FragmentId> {
        let mut result = Vec::new();
        self.collect_focusable(&self.root_children, &mut result);
        result
    }

    fn collect_focusable(&self, children: &[FragmentId], out: &mut Vec<FragmentId>) {
        for &id in children {
            if let Some(node) = self.nodes.get(&id) {
                if node.props.visible && node.props.focusable {
                    out.push(id);
                }
                self.collect_focusable(&node.children, out);
            }
        }
    }

    /// Move focus to the next (forward=true) or previous focusable fragment.
    /// Returns (old_focused, new_focused). Returns (old, None) when focus
    /// escapes the fragment tree (caller should let Qt handle Tab).
    pub fn focus_next(&mut self, forward: bool) -> (Option<FragmentId>, Option<FragmentId>) {
        let ids = self.focusable_ids();
        if ids.is_empty() {
            let old = self.focused.take();
            return (old, None);
        }

        let old = self.focused;
        let current_idx = old.and_then(|f| ids.iter().position(|&id| id == f));

        let next = match current_idx {
            Some(idx) => {
                if forward {
                    if idx + 1 < ids.len() {
                        Some(ids[idx + 1])
                    } else {
                        None // escape forward
                    }
                } else if idx > 0 {
                    Some(ids[idx - 1])
                } else {
                    None // escape backward
                }
            }
            None => {
                // No current focus — enter from start or end.
                if forward {
                    Some(ids[0])
                } else {
                    Some(*ids.last().unwrap())
                }
            }
        };

        self.focused = next;
        (old, next)
    }

    /// Focus a specific fragment (or its nearest focusable ancestor).
    /// Returns the previously focused fragment if focus actually changed.
    pub fn focus_fragment(&mut self, id: FragmentId) -> Option<FragmentId> {
        let old = self.focused;
        let target = self.find_focusable_ancestor(id);
        if target == old {
            return None;
        }
        self.focused = target;
        old
    }

    fn find_focusable_ancestor(&self, id: FragmentId) -> Option<FragmentId> {
        let mut current = Some(id);
        while let Some(cid) = current {
            if let Some(node) = self.nodes.get(&cid) {
                if node.props.focusable && node.props.visible {
                    return Some(cid);
                }
                current = node.parent;
            } else {
                break;
            }
        }
        None
    }

    /// Clear focus. Returns the previously focused fragment.
    pub fn blur(&mut self) -> Option<FragmentId> {
        self.focused.take()
    }

    pub fn focused(&self) -> Option<FragmentId> {
        self.focused
    }

    /// Compute the accumulated world transform for a fragment by walking parents.
    pub fn world_transform(&self, id: FragmentId) -> Affine {
        let mut chain = Vec::new();
        let mut current = Some(id);
        while let Some(cid) = current {
            let Some(node) = self.nodes.get(&cid) else { break };
            chain.push(node.local_transform());
            current = node.parent;
        }
        chain.iter().rev().fold(Affine::IDENTITY, |acc, t| acc * *t)
    }
}

// ---------------------------------------------------------------------------
// Fragment store — delegated to runtime state (per-window FragmentTree)
// ---------------------------------------------------------------------------

use crate::runtime;

pub fn fragment_store_ensure(canvas_node_id: u32) {
    runtime::ensure_fragment_tree(canvas_node_id);
}

pub fn fragment_store_remove(canvas_node_id: u32) {
    runtime::remove_fragment_tree(canvas_node_id);
}

pub fn fragment_store_create_node(canvas_node_id: u32, tag: &str) -> Option<FragmentId> {
    let kind = FragmentData::from_tag_loose(tag)?;
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| tree.create_node(kind))
}

pub fn fragment_store_insert_child(
    canvas_node_id: u32,
    parent: Option<FragmentId>,
    child: FragmentId,
    before: Option<FragmentId>,
) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.insert_child(parent, child, before);
        // Invalidate parent text shaped cache when a span is inserted.
        if let Some(parent_id) = parent {
            if let Some(child_node) = tree.nodes.get(&child) {
                if matches!(child_node.kind, FragmentData::Span(_)) {
                    if let Some(parent_node) = tree.nodes.get_mut(&parent_id) {
                        if let FragmentData::Text(ref mut t) = parent_node.kind {
                            t.shaped = None;
                            parent_node.dirty = true;
                        }
                    }
                }
            }
        }
    });
}

pub fn fragment_store_detach_child(
    canvas_node_id: u32,
    parent: Option<FragmentId>,
    child: FragmentId,
) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        // Check before detach whether child is a span.
        let is_span = tree.nodes.get(&child).map_or(false, |n| matches!(n.kind, FragmentData::Span(_)));
        tree.detach_child(parent, child);
        if is_span {
            if let Some(parent_id) = parent {
                if let Some(parent_node) = tree.nodes.get_mut(&parent_id) {
                    if let FragmentData::Text(ref mut t) = parent_node.kind {
                        t.shaped = None;
                        parent_node.dirty = true;
                    }
                }
            }
        }
    });
}

pub fn fragment_store_destroy(canvas_node_id: u32, fragment_id: FragmentId) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.remove(fragment_id);
    });
}

pub fn fragment_store_set_image_data(
    canvas_node_id: u32,
    fragment_id: FragmentId,
    image_data: ImageData,
) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        if let Some(node) = tree.nodes.get_mut(&fragment_id) {
            if let FragmentData::Image(ref mut img) = node.kind {
                img.image_data = Some(image_data);
                node.dirty = true;
                tree.any_dirty = true;
                tree.cached_scene = None;
            }
        }
        tree.invalidate_subtree_cache_for(fragment_id);
    });
}

pub fn fragment_store_clear_image_data(
    canvas_node_id: u32,
    fragment_id: FragmentId,
) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        if let Some(node) = tree.nodes.get_mut(&fragment_id) {
            if let FragmentData::Image(ref mut img) = node.kind {
                img.image_data = None;
                node.dirty = true;
                tree.any_dirty = true;
                tree.cached_scene = None;
            }
        }
        tree.invalidate_subtree_cache_for(fragment_id);
    });
}

pub fn fragment_store_set_prop(
    canvas_node_id: u32,
    fragment_id: FragmentId,
    key: &str,
    value: FragmentValue,
) -> bool {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        if is_layout_prop(key) {
            if let FragmentValue::F64 { value } = &value {
                let v = *value as f32;
                tree.with_taffy_style_mut(fragment_id, |style| {
                    apply_layout_prop_to_style(style, key, v);
                });
            } else if let FragmentValue::Str { ref value } = value {
                tree.with_taffy_style_mut(fragment_id, |style| {
                    apply_layout_string_prop_to_style(style, key, value);
                });
            }
            tree.invalidate();
            return;
        }

        // Track promoted_node_count for "layer" prop changes.
        if key == "layer" {
            if let FragmentValue::Bool { value } = &value {
                let was_promoted = tree.nodes.get(&fragment_id).map_or(false, |n| n.promoted);
                if *value && !was_promoted {
                    tree.promoted_node_count += 1;
                } else if !*value && was_promoted {
                    tree.promoted_node_count = tree.promoted_node_count.saturating_sub(1);
                    tree.promoted_scene_cache.remove(&fragment_id);
                }
            }
        }

        // Track explicit width/height and sync taffy size (before value is moved).
        if key == "width" || key == "height" {
            if let FragmentValue::F64 { value: v } = &value {
                let fv = *v;
                if let Some(node) = tree.nodes.get_mut(&fragment_id) {
                    if key == "width" {
                        node.props.explicit_width = if fv > 0.0 { Some(fv) } else { None };
                    } else {
                        node.props.explicit_height = if fv > 0.0 { Some(fv) } else { None };
                    }
                }
                let v32 = fv as f32;
                tree.with_taffy_style_mut(fragment_id, |style| {
                    if key == "width" {
                        style.size.width = if v32 > 0.0 { taffy::style::Dimension::length(v32) } else { taffy::style::Dimension::auto() };
                    } else {
                        style.size.height = if v32 > 0.0 { taffy::style::Dimension::length(v32) } else { taffy::style::Dimension::auto() };
                    }
                });
            } else if let FragmentValue::Str { ref value } = value {
                if let Some(dim) = parse_dimension_string(value) {
                    // Percentage/auto dimensions are layout-driven, clear explicit paint geometry.
                    if let Some(node) = tree.nodes.get_mut(&fragment_id) {
                        if key == "width" { node.props.explicit_width = None; } else { node.props.explicit_height = None; }
                    }
                    tree.with_taffy_style_mut(fragment_id, |style| {
                        if key == "width" { style.size.width = dim; } else { style.size.height = dim; }
                    });
                    tree.any_dirty = true;
                    tree.cached_scene = None;
                }
            }
        }

        if let Some(node) = tree.nodes.get_mut(&fragment_id) {
            apply_fragment_prop(node, key, value);
            node.dirty = true;
            tree.any_dirty = true;
            tree.cached_scene = None;
        }
        // When a span child changes, invalidate parent text shaped cache.
        if let Some(node) = tree.nodes.get(&fragment_id) {
            if matches!(node.kind, FragmentData::Span(_)) {
                if let Some(parent_id) = node.parent {
                    if let Some(parent) = tree.nodes.get_mut(&parent_id) {
                        if let FragmentData::Text(ref mut t) = parent.kind {
                            t.shaped = None;
                            parent.dirty = true;
                        }
                    }
                }
            }
        }
        tree.invalidate_subtree_cache_for(fragment_id);

        // Sync taffy position mode when x/y explicit state changes.
        if key == "x" || key == "y" {
            let (ex, ey) = tree.nodes.get(&fragment_id)
                .map(|n| (n.props.explicit_x, n.props.explicit_y))
                .unwrap_or((None, None));
            tree.with_taffy_style_mut(fragment_id, |style| {
                if ex.is_some() || ey.is_some() {
                    style.position = taffy::style::Position::Absolute;
                    style.inset.left = ex
                        .map(|v| taffy::style::LengthPercentageAuto::length(v as f32))
                        .unwrap_or(taffy::style::LengthPercentageAuto::auto());
                    style.inset.top = ey
                        .map(|v| taffy::style::LengthPercentageAuto::length(v as f32))
                        .unwrap_or(taffy::style::LengthPercentageAuto::auto());
                } else {
                    style.position = taffy::style::Position::Relative;
                    style.inset.left = taffy::style::LengthPercentageAuto::auto();
                    style.inset.top = taffy::style::LengthPercentageAuto::auto();
                }
            });
        }
    })
    .is_some()
}

pub fn fragment_store_paint(canvas_node_id: u32, scene: &mut Scene, transform: Affine) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.paint_into_scene(scene, transform);
    });
}

pub fn fragment_store_paint_single(canvas_node_id: u32, fragment_id: FragmentId, scene: &mut Scene, transform: Affine) {
    runtime::with_fragment_tree(canvas_node_id, |tree| {
        tree.paint_node_self_only(scene, fragment_id, transform);
    });
}

pub fn fragment_store_paint_at_origin(canvas_node_id: u32, fragment_id: FragmentId, scene: &mut Scene, transform: Affine) {
    runtime::with_fragment_tree(canvas_node_id, |tree| {
        tree.paint_node_at_origin(scene, fragment_id, transform);
    });
}

pub fn fragment_store_world_bounds(canvas_node_id: u32, fragment_id: FragmentId) -> Option<Rect> {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.ensure_aabbs();
        tree.node(fragment_id)?.world_aabb
    }).flatten()
}

pub fn fragment_store_build_paint_plan(canvas_node_id: u32) -> Option<PaintPlan> {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| tree.build_paint_plan())
}

pub fn fragment_store_has_promoted(canvas_node_id: u32) -> bool {
    runtime::with_fragment_tree(canvas_node_id, |tree| tree.has_promoted_nodes())
        .unwrap_or(false)
}

pub fn fragment_store_collect_inner_shadows(canvas_node_id: u32, scale_factor: f64) -> Vec<InnerShadowEffect> {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.collect_inner_shadow_effects(scale_factor)
    })
    .unwrap_or_default()
}

pub fn fragment_store_collect_backdrop_blurs(canvas_node_id: u32, scale_factor: f64) -> Vec<BackdropBlurEffect> {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.collect_backdrop_blur_effects(scale_factor)
    })
    .unwrap_or_default()
}

pub fn fragment_store_has_animating(canvas_node_id: u32) -> bool {
    runtime::with_fragment_tree(canvas_node_id, |tree| {
        tree.nodes.values().any(|n| {
            n.timeline.as_ref().map_or(false, |t| t.is_animating())
        })
    })
    .unwrap_or(false)
}

pub fn fragment_store_hit_test(canvas_node_id: u32, x: f64, y: f64) -> Option<FragmentId> {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| tree.hit_test((x, y)))?
}

pub fn fragment_store_set_debug_highlight(canvas_node_id: u32, fragment_id: Option<FragmentId>) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.set_debug_highlight(fragment_id);
    });
}

pub fn fragment_store_get_cursor(canvas_node_id: u32, fragment_id: FragmentId) -> u8 {
    runtime::with_fragment_tree(canvas_node_id, |tree| {
        tree.nodes.get(&fragment_id).map_or(0, |n| n.props.cursor)
    })
    .unwrap_or(0)
}

/// Move focus to the next/previous focusable fragment.
/// Returns (old_fragment_id, new_fragment_id) as i32 (-1 = none).
pub fn fragment_store_focus_next(canvas_node_id: u32, forward: bool) -> (i32, i32) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        let (old, new) = tree.focus_next(forward);
        (
            old.map(|id| id.0 as i32).unwrap_or(-1),
            new.map(|id| id.0 as i32).unwrap_or(-1),
        )
    })
    .unwrap_or((-1, -1))
}

/// Focus a specific fragment by click. Returns (old_id, new_id) as i32.
pub fn fragment_store_focus_fragment(canvas_node_id: u32, fragment_id: FragmentId) -> (i32, i32) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        let old = tree.focus_fragment(fragment_id);
        let new_focused = tree.focused().map(|id| id.0 as i32).unwrap_or(-1);
        (
            old.map(|id| id.0 as i32).unwrap_or(-1),
            new_focused,
        )
    })
    .unwrap_or((-1, -1))
}

pub fn fragment_store_focused(canvas_node_id: u32) -> i32 {
    runtime::with_fragment_tree(canvas_node_id, |tree| {
        tree.focused().map(|id| id.0 as i32).unwrap_or(-1)
    })
    .unwrap_or(-1)
}

pub fn fragment_store_set_text_shape_cache(
    canvas_node_id: u32,
    fragment_id: FragmentId,
    cache: ShapedTextCache,
) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        if let Some(node) = tree.nodes.get_mut(&fragment_id) {
            if let FragmentData::Text(ref mut text) = node.kind {
                text.shaped = Some(cache);
            }
            node.dirty = true;
        }
        tree.any_dirty = true;
        tree.cached_scene = None;
        tree.invalidate_subtree_cache_for(fragment_id);
    });
}

pub fn fragment_store_set_text_input_layout_cache(
    canvas_node_id: u32,
    fragment_id: FragmentId,
    layout: ShapedTextLayout,
) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        if let Some(node) = tree.nodes.get_mut(&fragment_id) {
            if let FragmentData::TextInput(ref mut ti) = node.kind {
                ti.layout = Some(layout);
            }
            node.dirty = true;
        }
        tree.any_dirty = true;
        tree.cached_scene = None;
        tree.invalidate_subtree_cache_for(fragment_id);
    });
}

pub fn fragment_store_set_text_input_state(
    canvas_node_id: u32,
    fragment_id: FragmentId,
    text: String,
    cursor_pos: f64,
    selection_anchor: f64,
    layout: ShapedTextLayout,
) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        if let Some(node) = tree.nodes.get_mut(&fragment_id) {
            if let FragmentData::TextInput(ref mut ti) = node.kind {
                ti.text = text;
                ti.cursor_pos = cursor_pos;
                ti.selection_anchor = selection_anchor;
                ti.layout = Some(layout);
            }
            node.dirty = true;
        }
        tree.any_dirty = true;
        tree.cached_scene = None;
        tree.invalidate_subtree_cache_for(fragment_id);
    });
}

pub fn fragment_store_set_caret_visible(
    canvas_node_id: u32,
    fragment_id: FragmentId,
    visible: bool,
) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        if let Some(node) = tree.nodes.get_mut(&fragment_id) {
            if let FragmentData::TextInput(ref mut ti) = node.kind {
                ti.caret_visible = visible;
            }
            node.dirty = true;
        }
        tree.any_dirty = true;
        tree.cached_scene = None;
        tree.invalidate_subtree_cache_for(fragment_id);
    });
}

pub fn fragment_store_read_text_props(
    canvas_node_id: u32,
    fragment_id: FragmentId,
) -> Option<(String, f64, String, i32, bool, f64, String)> {
    runtime::with_fragment_tree(canvas_node_id, |tree| {
        let node = tree.node(fragment_id)?;
        if let FragmentData::Text(ref text) = node.kind {
            if text.shaped.is_some() {
                return None;
            }
            let weight = text.font_weight as i32;
            let italic = text.font_style == "italic";
            Some((text.text.clone(), text.font_size, text.font_family.clone(), weight, italic, text.text_max_width, text.text_overflow.clone()))
        } else {
            None
        }
    })?
}

/// Collect styled text runs from span children of a text fragment.
/// Returns None if no spans present, already cached, or not a text fragment.
pub fn fragment_store_read_text_style_runs(
    canvas_node_id: u32,
    fragment_id: FragmentId,
) -> Option<(Vec<TextStyleRun>, f64, String, f64, String)> {
    runtime::with_fragment_tree(canvas_node_id, |tree| {
        let node = tree.node(fragment_id)?;
        let text_frag = match &node.kind {
            FragmentData::Text(t) => t,
            _ => return None,
        };
        if text_frag.shaped.is_some() {
            return None;
        }
        let children = &node.children;
        if children.is_empty() {
            return None;
        }
        let mut runs = Vec::new();
        for &child_id in children {
            let child = tree.node(child_id)?;
            if let FragmentData::Span(ref span) = child.kind {
                runs.push(TextStyleRun {
                    text: span.text.clone(),
                    font_size: if span.font_size > 0.0 { span.font_size } else { text_frag.font_size },
                    font_family: if span.font_family.is_empty() { text_frag.font_family.clone() } else { span.font_family.clone() },
                    font_weight: if span.font_weight > 0.0 { span.font_weight as i32 } else { text_frag.font_weight as i32 },
                    font_italic: if span.font_style.is_empty() { text_frag.font_style == "italic" } else { span.font_style == "italic" },
                    color: span.color,
                });
            }
        }
        if runs.is_empty() {
            None
        } else {
            Some((runs, text_frag.font_size, text_frag.font_family.clone(), text_frag.text_max_width, text_frag.text_overflow.clone()))
        }
    })?
}

/// If the given fragment is a Span child of a Text parent, return the parent Text id.
pub fn fragment_store_parent_text_for_span(
    canvas_node_id: u32,
    fragment_id: FragmentId,
) -> Option<FragmentId> {
    runtime::with_fragment_tree(canvas_node_id, |tree| {
        let node = tree.node(fragment_id)?;
        if !matches!(node.kind, FragmentData::Span(_)) {
            return None;
        }
        let parent_id = node.parent?;
        let parent = tree.node(parent_id)?;
        if matches!(parent.kind, FragmentData::Text(_)) {
            Some(parent_id)
        } else {
            None
        }
    }).flatten()
}

/// Read text input props for reshaping. Returns None if already cached.
pub fn fragment_store_read_text_input_props(
    canvas_node_id: u32,
    fragment_id: FragmentId,
) -> Option<(String, f64, String, i32, bool)> {
    runtime::with_fragment_tree(canvas_node_id, |tree| {
        let node = tree.node(fragment_id)?;
        if let FragmentData::TextInput(ref ti) = node.kind {
            if ti.layout.is_some() {
                return None;
            }
            let weight = ti.font_weight as i32;
            let italic = ti.font_style == "italic";
            Some((ti.text.clone(), ti.font_size, ti.font_family.clone(), weight, italic))
        } else {
            None
        }
    })?
}

pub fn fragment_store_click_to_cursor(
    canvas_node_id: u32,
    fragment_id: FragmentId,
    window_x: f64,
    _window_y: f64,
) {
    // Compute local x by inverting the fragment's world transform.
    let local_x = runtime::with_fragment_tree(canvas_node_id, |tree| {
        let node = tree.node(fragment_id)?;
        if !matches!(node.kind, FragmentData::TextInput(_)) {
            return None;
        }
        let world = tree.world_transform(fragment_id);
        let inv = world.inverse();
        let local = inv * Point::new(window_x, 0.0);
        Some(local.x)
    }).flatten();

    if let Some(lx) = local_x {
        let _ = crate::qt::ffi::qt_text_edit_click_to_cursor(canvas_node_id, lx);
    }
}

pub fn fragment_store_drag_to_cursor(
    canvas_node_id: u32,
    window_x: f64,
    _window_y: f64,
) {
    // Find the focused TextInput and compute local x.
    let local_x = runtime::with_fragment_tree(canvas_node_id, |tree| {
        let focused_id = tree.focused()?;
        let node = tree.node(focused_id)?;
        if !matches!(node.kind, FragmentData::TextInput(_)) {
            return None;
        }
        let world = tree.world_transform(focused_id);
        let inv = world.inverse();
        let local = inv * Point::new(window_x, 0.0);
        Some(local.x)
    }).flatten();

    if let Some(lx) = local_x {
        let _ = crate::qt::ffi::qt_text_edit_drag_to_cursor(canvas_node_id, lx);
    }
}

pub fn fragment_store_mark_dirty(canvas_node_id: u32, fragment_id: FragmentId) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.mark_dirty(fragment_id);
    });
}

pub fn fragment_store_compute_layout(
    canvas_node_id: u32,
    available_width: f64,
    available_height: f64,
) {
    let events = runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.compute_layout(available_width, available_height)
    });
    if let Some(events) = events {
        for event in events {
            runtime::emit_js_event(crate::api::QtHostEvent::FragmentLayout {
                canvas_node_id,
                fragment_id: event.fragment_id.0,
                x: event.x,
                y: event.y,
                width: event.width,
                height: event.height,
            });
        }
    }
}

pub fn fragment_store_set_listener(
    canvas_node_id: u32,
    fragment_id: u32,
    listener_bit: u32,
    enabled: bool,
) {
    let id = FragmentId(fragment_id);
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        if let Some(node) = tree.nodes.get_mut(&id) {
            let flags = FragmentListeners::from_bits_truncate(listener_bit);
            if enabled {
                node.listeners.insert(flags);
            } else {
                node.listeners.remove(flags);
            }
        }
    });
}

pub fn fragment_store_set_motion_target(
    canvas_node_id: u32,
    fragment_id: FragmentId,
    targets: &[(motion::PropertyKey, f64)],
    default_transition: &motion::TransitionSpec,
    per_property: &std::collections::HashMap<motion::PropertyKey, motion::TransitionSpec>,
    delay_secs: f64,
    now: f64,
) -> bool {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.set_motion_target(fragment_id, targets, default_transition, per_property, delay_secs, now)
    })
    .unwrap_or(false)
}

pub fn fragment_store_set_motion_target_keyframes(
    canvas_node_id: u32,
    fragment_id: FragmentId,
    targets: Vec<(motion::PropertyKey, Vec<f64>)>,
    times: Option<Vec<f64>>,
    default_transition: &motion::TransitionSpec,
    per_property: &std::collections::HashMap<motion::PropertyKey, motion::TransitionSpec>,
    delay_secs: f64,
    now: f64,
) -> bool {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.set_motion_target_keyframes(fragment_id, targets, times, default_transition, per_property, delay_secs, now)
    })
    .unwrap_or(false)
}

pub fn fragment_store_tick_motion(canvas_node_id: u32, now: f64) -> (bool, Vec<FragmentId>) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| tree.tick_motion(now))
        .unwrap_or((false, Vec::new()))
}

pub fn fragment_store_get_world_bounds(canvas_node_id: u32, fragment_id: FragmentId) -> Option<Rect> {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| tree.get_world_bounds(fragment_id))
        .flatten()
}

pub fn fragment_store_set_scroll_offset(
    canvas_node_id: u32,
    fragment_id: FragmentId,
    x: f64,
    y: f64,
) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.set_scroll_offset(fragment_id, Vec2::new(x, y));
    });
}

pub fn fragment_store_get_content_size(
    canvas_node_id: u32,
    fragment_id: FragmentId,
) -> Option<(f64, f64)> {
    runtime::with_fragment_tree(canvas_node_id, |tree| {
        tree.get_content_size(fragment_id)
    }).flatten()
}

pub fn fragment_store_set_layout_flip(
    canvas_node_id: u32,
    fragment_id: FragmentId,
    dx: f64,
    dy: f64,
    sx: f64,
    sy: f64,
    transition: &motion::TransitionSpec,
    now: f64,
) -> bool {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.set_layout_flip(fragment_id, dx, dy, sx, sy, transition, now)
    })
    .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Layout prop keys
// ---------------------------------------------------------------------------

const LAYOUT_PROPS: &[&str] = &[
    "flexDirection", "flexGrow", "flexShrink", "flexBasis",
    "flexWrap", "alignItems", "alignSelf", "justifyContent",
    "gap", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft",
    "margin", "marginTop", "marginRight", "marginBottom", "marginLeft",
    "minWidth", "minHeight", "maxWidth", "maxHeight",
    "position", "overflow", "overflowX", "overflowY",
];

fn is_layout_prop(key: &str) -> bool {
    LAYOUT_PROPS.contains(&key)
}

/// Parse dimension strings like "100%", "50%", "auto".
fn parse_dimension_string(s: &str) -> Option<taffy::style::Dimension> {
    let s = s.trim();
    if s == "auto" {
        return Some(taffy::style::Dimension::auto());
    }
    if let Some(pct) = s.strip_suffix('%') {
        if let Ok(v) = pct.trim().parse::<f32>() {
            return Some(taffy::style::Dimension::percent(v / 100.0));
        }
    }
    None
}

fn apply_layout_prop_to_style(style: &mut taffy::Style, key: &str, v: f32) {
    match key {
        "flexGrow" => style.flex_grow = v,
        "flexShrink" => style.flex_shrink = v,
        "flexBasis" => style.flex_basis = taffy::style::Dimension::length(v),
        "gap" => {
            style.gap = taffy::geometry::Size {
                width: taffy::style::LengthPercentage::length(v),
                height: taffy::style::LengthPercentage::length(v),
            };
        }
        "padding" => {
            let lp = taffy::style::LengthPercentage::length(v);
            style.padding = taffy::geometry::Rect { top: lp, right: lp, bottom: lp, left: lp };
        }
        "margin" => {
            let lpa = taffy::style::LengthPercentageAuto::length(v);
            style.margin = taffy::geometry::Rect { top: lpa, right: lpa, bottom: lpa, left: lpa };
        }
        "paddingTop" => style.padding.top = taffy::style::LengthPercentage::length(v),
        "paddingRight" => style.padding.right = taffy::style::LengthPercentage::length(v),
        "paddingBottom" => style.padding.bottom = taffy::style::LengthPercentage::length(v),
        "paddingLeft" => style.padding.left = taffy::style::LengthPercentage::length(v),
        "marginTop" => style.margin.top = taffy::style::LengthPercentageAuto::length(v),
        "marginRight" => style.margin.right = taffy::style::LengthPercentageAuto::length(v),
        "marginBottom" => style.margin.bottom = taffy::style::LengthPercentageAuto::length(v),
        "marginLeft" => style.margin.left = taffy::style::LengthPercentageAuto::length(v),
        "minWidth" => style.min_size.width = taffy::style::Dimension::length(v),
        "minHeight" => style.min_size.height = taffy::style::Dimension::length(v),
        "maxWidth" => style.max_size.width = taffy::style::Dimension::length(v),
        "maxHeight" => style.max_size.height = taffy::style::Dimension::length(v),
        _ => {}
    }
}

fn apply_layout_string_prop_to_style(style: &mut taffy::Style, key: &str, v: &str) {
    match key {
        "flexDirection" => {
            style.flex_direction = match v {
                "row" => taffy::style::FlexDirection::Row,
                "column" => taffy::style::FlexDirection::Column,
                "row-reverse" => taffy::style::FlexDirection::RowReverse,
                "column-reverse" => taffy::style::FlexDirection::ColumnReverse,
                _ => taffy::style::FlexDirection::Column,
            };
        }
        "flexWrap" => {
            style.flex_wrap = match v {
                "nowrap" => taffy::style::FlexWrap::NoWrap,
                "wrap" => taffy::style::FlexWrap::Wrap,
                "wrap-reverse" => taffy::style::FlexWrap::WrapReverse,
                _ => taffy::style::FlexWrap::NoWrap,
            };
        }
        "alignItems" => {
            style.align_items = match v {
                "flex-start" | "start" => Some(taffy::style::AlignItems::FlexStart),
                "flex-end" | "end" => Some(taffy::style::AlignItems::FlexEnd),
                "center" => Some(taffy::style::AlignItems::Center),
                "stretch" => Some(taffy::style::AlignItems::Stretch),
                "baseline" => Some(taffy::style::AlignItems::Baseline),
                _ => None,
            };
        }
        "alignSelf" => {
            style.align_self = match v {
                "flex-start" | "start" => Some(taffy::style::AlignSelf::FlexStart),
                "flex-end" | "end" => Some(taffy::style::AlignSelf::FlexEnd),
                "center" => Some(taffy::style::AlignSelf::Center),
                "stretch" => Some(taffy::style::AlignSelf::Stretch),
                _ => None,
            };
        }
        "justifyContent" => {
            style.justify_content = match v {
                "flex-start" | "start" => Some(taffy::style::JustifyContent::FlexStart),
                "flex-end" | "end" => Some(taffy::style::JustifyContent::FlexEnd),
                "center" => Some(taffy::style::JustifyContent::Center),
                "space-between" => Some(taffy::style::JustifyContent::SpaceBetween),
                "space-around" => Some(taffy::style::JustifyContent::SpaceAround),
                "space-evenly" => Some(taffy::style::JustifyContent::SpaceEvenly),
                _ => None,
            };
        }
        "position" => {
            style.position = match v {
                "relative" => taffy::style::Position::Relative,
                "absolute" => taffy::style::Position::Absolute,
                _ => taffy::style::Position::Relative,
            };
        }
        "overflow" => {
            let ov = match v {
                "visible" => taffy::style::Overflow::Visible,
                "clip" => taffy::style::Overflow::Clip,
                "hidden" => taffy::style::Overflow::Hidden,
                "scroll" => taffy::style::Overflow::Scroll,
                _ => taffy::style::Overflow::Visible,
            };
            style.overflow = taffy::geometry::Point { x: ov, y: ov };
        }
        "overflowX" => {
            style.overflow.x = match v {
                "visible" => taffy::style::Overflow::Visible,
                "clip" => taffy::style::Overflow::Clip,
                "hidden" => taffy::style::Overflow::Hidden,
                "scroll" => taffy::style::Overflow::Scroll,
                _ => taffy::style::Overflow::Visible,
            };
        }
        "overflowY" => {
            style.overflow.y = match v {
                "visible" => taffy::style::Overflow::Visible,
                "clip" => taffy::style::Overflow::Clip,
                "hidden" => taffy::style::Overflow::Hidden,
                "scroll" => taffy::style::Overflow::Scroll,
                _ => taffy::style::Overflow::Visible,
            };
        }
        _ => {}
    }
}

fn apply_fragment_prop(node: &mut FragmentNode, key: &str, value: FragmentValue) {
    match key {
        "x" => {
            match value {
                FragmentValue::F64 { value } => { node.props.explicit_x = Some(value); }
                FragmentValue::Unset => { node.props.explicit_x = None; }
                _ => {}
            }
            return;
        }
        "y" => {
            match value {
                FragmentValue::F64 { value } => { node.props.explicit_y = Some(value); }
                FragmentValue::Unset => { node.props.explicit_y = None; }
                _ => {}
            }
            return;
        }
        "opacity" => {
            if let FragmentValue::F64 { value } = value { node.props.opacity = value as f32; }
            return;
        }
        "clip" => {
            if let FragmentValue::Bool { value } = value { node.props.clip = value; }
            return;
        }
        "visible" => {
            if let FragmentValue::Bool { value } = value { node.props.visible = value; }
            return;
        }
        "pointerEvents" => {
            if let FragmentValue::Bool { value } = value { node.props.pointer_events = value; }
            return;
        }
        "cursor" => {
            if let FragmentValue::Str { ref value } = value {
                node.props.cursor = match value.as_str() {
                    "pointer" | "hand" => 1,
                    "text" | "ibeam" => 2,
                    "crosshair" => 3,
                    "move" => 4,
                    "wait" => 5,
                    "not-allowed" | "forbidden" => 6,
                    "grab" => 7,
                    "grabbing" => 8,
                    _ => 0,
                };
            }
            return;
        }
        "focusable" => {
            if let FragmentValue::Bool { value } = value { node.props.focusable = value; }
            return;
        }
        "layer" => {
            if let FragmentValue::Bool { value } = value { node.promoted = value; }
            return;
        }
        "blendMode" => {
            if let FragmentValue::BlendMode { value } = value {
                node.props.blend_mode = value.into();
            }
            return;
        }
        "backdropBlur" => {
            match value {
                FragmentValue::F64 { value } => {
                    node.props.backdrop_blur = if value > 0.0 { Some(value) } else { None };
                }
                FragmentValue::Unset => { node.props.backdrop_blur = None; }
                _ => {}
            }
            return;
        }
        _ => {}
    }
    // Handle Unset → reset the kind-level prop to default.
    if matches!(value, FragmentValue::Unset) {
        node.kind.reset_prop(key);
        return;
    }
    node.kind.apply_prop(key, value);
    // Invalidate shaped-text cache when text content or font size changes,
    // so the next reshape pass picks up the new value.
    if matches!(key, "text" | "fontSize" | "fontFamily" | "fontWeight" | "fontStyle" | "textMaxWidth" | "textOverflow") {
        match &mut node.kind {
            FragmentData::Text(t) => { t.shaped = None; }
            FragmentData::TextInput(ti) => { ti.layout = None; }
            _ => {}
        }
    }
    // Reparse SVG path data when `d` prop changes on a PathFragment.
    if key == "d" {
        if let FragmentData::Path(ref mut path) = node.kind {
            path.reparse_path();
        }
    }
}

// ---------------------------------------------------------------------------
// Devtools snapshot
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FragmentNodeSnapshot {
    pub id: u32,
    pub tag: String,
    pub parent_id: Option<u32>,
    pub child_ids: Vec<u32>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub clip: bool,
    pub visible: bool,
    pub opacity: f32,
    pub props: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct LayerSnapshot {
    pub fragment_id: u32,
    pub layer_key: u32,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub opacity: f32,
    pub reasons: String,
}

#[derive(Debug, Clone)]
pub struct AnimationChannelSnapshot {
    pub property: String,
    pub origin: f64,
    pub target: f64,
    pub state: String,
}

#[derive(Debug, Clone)]
pub struct AnimationSnapshot {
    pub fragment_id: u32,
    pub tag: String,
    pub channels: Vec<AnimationChannelSnapshot>,
}

fn format_color(c: &Color) -> String {
    let rgba = c.to_rgba8();
    if rgba.a == 255 {
        format!("#{:02x}{:02x}{:02x}", rgba.r, rgba.g, rgba.b)
    } else {
        format!("#{:02x}{:02x}{:02x}{:02x}", rgba.r, rgba.g, rgba.b, rgba.a)
    }
}

fn serialize_taffy_style(props: &mut HashMap<String, String>, style: &taffy::Style) {
    match style.flex_direction {
        taffy::FlexDirection::Row => {}
        taffy::FlexDirection::Column => { props.insert("flexDirection".into(), "column".into()); }
        taffy::FlexDirection::RowReverse => { props.insert("flexDirection".into(), "row-reverse".into()); }
        taffy::FlexDirection::ColumnReverse => { props.insert("flexDirection".into(), "column-reverse".into()); }
    }
    if style.flex_grow != 0.0 {
        props.insert("flexGrow".into(), format!("{}", style.flex_grow));
    }
    if style.flex_shrink != 0.0 {
        props.insert("flexShrink".into(), format!("{}", style.flex_shrink));
    }
    if let Some(v) = lp_length_value(style.gap.width) {
        if v != 0.0 { props.insert("columnGap".into(), format!("{}", v)); }
    }
    if let Some(v) = lp_length_value(style.gap.height) {
        if v != 0.0 { props.insert("rowGap".into(), format!("{}", v)); }
    }
    serialize_taffy_rect_lp("padding", &style.padding, props);
    if let Some(ai) = style.align_items {
        props.insert("alignItems".into(), format!("{:?}", ai).to_ascii_lowercase());
    }
    if let Some(jc) = style.justify_content {
        props.insert("justifyContent".into(), format!("{:?}", jc).to_ascii_lowercase());
    }
    if style.overflow.x != taffy::Overflow::Visible || style.overflow.y != taffy::Overflow::Visible {
        props.insert("overflow".into(), format!("{:?}/{:?}", style.overflow.x, style.overflow.y).to_ascii_lowercase());
    }
}

fn lp_length_value(lp: taffy::LengthPercentage) -> Option<f32> {
    use taffy::style::CompactLength;
    let raw = lp.into_raw();
    if raw.tag() == CompactLength::LENGTH_TAG {
        Some(raw.value())
    } else {
        None
    }
}

fn serialize_taffy_rect_lp(
    prefix: &str,
    rect: &taffy::geometry::Rect<taffy::LengthPercentage>,
    props: &mut HashMap<String, String>,
) {
    let sides = [("Top", rect.top), ("Right", rect.right), ("Bottom", rect.bottom), ("Left", rect.left)];
    for (suffix, val) in sides {
        if let Some(v) = lp_length_value(val) {
            if v != 0.0 {
                props.insert(format!("{}{}", prefix, suffix), format!("{}", v));
            }
        }
    }
}

fn snapshot_tag(kind: &FragmentData) -> &'static str {
    match kind {
        FragmentData::Group(_) => "group",
        FragmentData::Rect(_) => "rect",
        FragmentData::Circle(_) => "circle",
        FragmentData::Path(_) => "path",
        FragmentData::Text(_) => "text",
        FragmentData::TextInput(_) => "textinput",
        FragmentData::Image(_) => "image",
        FragmentData::Span(_) => "span",
    }
}

impl FragmentTree {
    /// Devtools snapshot — returns a flat list of all nodes with layout data.
    pub fn snapshot(&self) -> Vec<FragmentNodeSnapshot> {
        self.nodes.values().map(|node| {
            let tag = snapshot_tag(&node.kind).to_string();

            // Use layout-computed size (available for all node types).
            let width = node.layout.width;
            let height = node.layout.height;

            let taffy_style = node.taffy_node
                .and_then(|tn| self.taffy.style(tn).ok())
                .cloned();

            let mut props = HashMap::new();
            match &node.kind {
                FragmentData::Rect(r) => {
                    if let Some(FragmentBrush::Solid(fp)) = &r.fill {
                        props.insert("fill".into(), format_color(&fp.color));
                    }
                    let radii = &r.corner_radii;
                    if radii.as_single_radius().map_or(true, |r| r > 0.0) {
                        props.insert("cornerRadius".into(), format!("{:.1}", radii.as_single_radius().unwrap_or(0.0)));
                    }
                    if r.stroke_width > 0.0 {
                        props.insert("strokeWidth".into(), format!("{:.1}", r.stroke_width));
                    }
                    if let Some(sp) = &r.stroke {
                        props.insert("stroke".into(), format_color(&sp.color));
                    }
                }
                FragmentData::Text(t) => {
                    props.insert("text".into(), t.text.clone());
                    props.insert("fontSize".into(), format!("{}", t.font_size));
                    if !t.font_family.is_empty() {
                        props.insert("fontFamily".into(), t.font_family.clone());
                    }
                    props.insert("color".into(), format_color(&t.color));
                }
                FragmentData::TextInput(t) => {
                    props.insert("text".into(), t.text.clone());
                    props.insert("fontSize".into(), format!("{}", t.font_size));
                    props.insert("color".into(), format_color(&t.color));
                }
                FragmentData::Circle(c) => {
                    props.insert("cx".into(), format!("{}", c.cx));
                    props.insert("cy".into(), format!("{}", c.cy));
                    props.insert("r".into(), format!("{}", c.r));
                    if let Some(fp) = &c.fill {
                        props.insert("fill".into(), format_color(&fp.color));
                    }
                }
                FragmentData::Path(p) => {
                    if !p.d.is_empty() {
                        props.insert("d".into(), p.d.clone());
                    }
                    if let Some(FragmentBrush::Solid(fp)) = &p.fill {
                        props.insert("fill".into(), format_color(&fp.color));
                    }
                }
                FragmentData::Group(_) => {}
                FragmentData::Image(img) => {
                    if !img.object_fit.is_empty() {
                        props.insert("objectFit".into(), img.object_fit.clone());
                    }
                    props.insert("hasImage".into(), img.image_data.is_some().to_string());
                }
                FragmentData::Span(s) => {
                    props.insert("text".into(), s.text.clone());
                    props.insert("fontSize".into(), format!("{}", s.font_size));
                    if !s.font_family.is_empty() {
                        props.insert("fontFamily".into(), s.font_family.clone());
                    }
                    props.insert("color".into(), format_color(&s.color));
                }
            }

            if let Some(style) = &taffy_style {
                serialize_taffy_style(&mut props, style);
            }

            FragmentNodeSnapshot {
                id: node.id.0,
                tag,
                parent_id: node.parent.map(|p| p.0),
                child_ids: node.children.iter().map(|c| c.0).collect(),
                x: node.render_x(),
                y: node.render_y(),
                width,
                height,
                clip: node.props.clip,
                visible: node.props.visible,
                opacity: node.props.opacity,
                props,
            }
        }).collect()
    }

    /// Snapshot of promoted layers for devtools LayerTree domain.
    pub fn snapshot_layers(&mut self) -> Vec<LayerSnapshot> {
        self.ensure_aabbs();
        self.nodes.values()
            .filter(|n| n.promoted && n.layer_key.is_some())
            .map(|n| {
                let bounds = n.world_aabb.unwrap_or(Rect::ZERO);
                let reasons = if n.props.opacity < 1.0 - f32::EPSILON && n.props.clip {
                    "opacity,clip"
                } else if n.props.opacity < 1.0 - f32::EPSILON {
                    "opacity"
                } else if n.props.clip {
                    "clip"
                } else {
                    "explicitly promoted"
                };
                LayerSnapshot {
                    fragment_id: n.id.0,
                    layer_key: n.layer_key.unwrap().0,
                    x: bounds.x0,
                    y: bounds.y0,
                    width: bounds.width(),
                    height: bounds.height(),
                    opacity: n.props.opacity,
                    reasons: reasons.to_string(),
                }
            })
            .collect()
    }

    /// Snapshot of active animations for devtools Animation domain.
    pub fn snapshot_animations(&self) -> Vec<AnimationSnapshot> {
        self.nodes.values()
            .filter_map(|n| {
                let timeline = n.timeline.as_ref()?;
                if !timeline.is_animating() { return None; }
                let channels = timeline.running_channel_snapshots()
                    .into_iter()
                    .map(|(prop, origin, target, state)| AnimationChannelSnapshot {
                        property: prop.to_string(),
                        origin,
                        target,
                        state: state.to_string(),
                    })
                    .collect();
                Some(AnimationSnapshot {
                    fragment_id: n.id.0,
                    tag: snapshot_tag(&n.kind).to_string(),
                    channels,
                })
            })
            .collect()
    }
}

pub fn fragment_store_snapshot(canvas_node_id: u32) -> Vec<FragmentNodeSnapshot> {
    runtime::with_fragment_tree(canvas_node_id, |tree| tree.snapshot())
        .unwrap_or_default()
}

pub fn fragment_store_snapshot_layers(canvas_node_id: u32) -> Vec<LayerSnapshot> {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| tree.snapshot_layers())
        .unwrap_or_default()
}

pub fn fragment_store_snapshot_animations(canvas_node_id: u32) -> Vec<AnimationSnapshot> {
    runtime::with_fragment_tree(canvas_node_id, |tree| tree.snapshot_animations())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

