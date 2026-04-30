use super::super::vello::peniko::{
    color::palette,
    kurbo::{Affine, BezPath, Rect},
    BlendMode, Color, Fill, ImageData,
};
use super::super::vello::{PaintScene, Scene};

/// Fallback clip rect for opacity-only layers where no clip shape is specified.
/// anyrender's push_layer requires a clip shape; use an enormous rect when clipping
/// is not actually desired.
pub(crate) const FALLBACK_CLIP: Rect = Rect::new(-1e9, -1e9, 1e9, 1e9);

/// Clip shape resolved for a fragment layer — either a rect or an arbitrary path.
#[derive(Debug, Clone)]
pub enum FragmentClipShape {
    Rect(Rect),
    Path(BezPath),
}

pub(crate) fn push_fragment_layer(scene: &mut Scene, transform: Affine, clip: Option<&FragmentClipShape>, opacity: f32, blend_mode: BlendMode) {
    match clip {
        Some(FragmentClipShape::Rect(r)) => scene.push_layer(blend_mode, opacity, transform, r),
        Some(FragmentClipShape::Path(p)) => scene.push_layer(blend_mode, opacity, transform, p),
        None => scene.push_layer(blend_mode, opacity, transform, &FALLBACK_CLIP),
    }
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
    pub clip: Option<FragmentClipShape>,
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

pub(crate) const FRAGMENT_LAYER_KEY_BASE: u32 = 0x8000_0000;

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
pub struct BorderSide {
    pub width: f64,
    pub color: Color,
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
