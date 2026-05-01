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
    /// True when the subtree content changed (needs vello re-rasterization).
    pub content_dirty: bool,
    /// True when only pose (transform/opacity) changed (compositor-only update).
    pub pose_only_dirty: bool,
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
// RenderPlan — partitioned output for compositor
// ---------------------------------------------------------------------------

/// A composited layer that passed the z-order safety check.
/// Its texture is retained across frames and only re-rasterized when
/// `content_dirty` is true.
#[derive(Debug)]
pub struct CompositedLayer {
    pub layer_key: FragmentLayerKey,
    pub fragment_id: FragmentId,
    pub scene: Scene,
    pub bounds: Rect,
    pub transform: Affine,
    pub clip: Option<FragmentClipShape>,
    pub opacity: f32,
    pub blend_mode: BlendMode,
    pub content_dirty: bool,
    pub pose_only_dirty: bool,
}

/// Partitioned render plan produced by `PaintPlan::partition()`.
///
/// `base_scene` contains all inline content plus any demoted promoted layers
/// (those whose z-order can't be preserved if composited separately).
/// `composited_layers` are safe to render to independent textures and drawn
/// after the base in paint order.
#[derive(Debug)]
pub struct RenderPlan {
    pub base_scene: Scene,
    pub composited_layers: Vec<CompositedLayer>,
    pub stale_keys: Vec<FragmentLayerKey>,
    /// True when only composited layers changed pose (no base or content dirty).
    pub pose_only: bool,
}

impl PaintPlan {
    /// Partition chunks into a base scene (inline + demoted promoted) and
    /// compositor-safe promoted layers.
    ///
    /// A promoted chunk is safe to composite independently only if no later
    /// inline chunk's bounds overlap with it. Otherwise it is "demoted" and
    /// flattened into the base scene to preserve correct z-order.
    pub fn partition(self) -> RenderPlan {
        let chunks = self.chunks;
        let stale_keys = self.stale_keys;

        if chunks.is_empty() {
            return RenderPlan {
                base_scene: Scene::new(),
                composited_layers: Vec::new(),
                stale_keys,
                pose_only: false,
            };
        }

        // Backwards pass: determine placement for each chunk.
        // A promoted chunk is safe to composite if it does not overlap any
        // later base-painted content (inline or demoted promoted).
        let mut placements: Vec<bool> = vec![true; chunks.len()]; // true = composite
        let mut later_base_bounds: Option<Rect> = None;

        for i in (0..chunks.len()).rev() {
            match &chunks[i] {
                PaintChunk::Inline(s) => {
                    placements[i] = false; // inline always goes to base
                    if let Some(b) = super::tree::scene_bounds(s) {
                        later_base_bounds = Some(match later_base_bounds {
                            Some(u) => u.union(b),
                            None => b,
                        });
                    }
                }
                PaintChunk::Promoted(layer) => {
                    // Conservative: demote if non-default blend or path clip.
                    let unsupported_blend = layer.blend_mode != BlendMode::default();
                    let has_path_clip = matches!(layer.clip, Some(FragmentClipShape::Path(_)));

                    // Transform local bounds to world space for overlap check.
                    let world_bounds = super::paint::transform_local_bounds_to_world(
                        layer.bounds, layer.transform,
                    );
                    let overlaps = later_base_bounds
                        .map_or(false, |u| rects_intersect(world_bounds, u));

                    if unsupported_blend || has_path_clip || overlaps {
                        placements[i] = false; // demote to base
                        later_base_bounds = Some(match later_base_bounds {
                            Some(u) => u.union(world_bounds),
                            None => world_bounds,
                        });
                    }
                    // else: safe to composite
                }
            }
        }

        // Forward pass: build base_scene and composited_layers.
        let mut base_scene = Scene::new();
        let mut composited_layers = Vec::new();
        let mut any_base_dirty = false;
        let mut all_composited_pose_only = true;

        for (chunk, is_composite) in chunks.into_iter().zip(placements) {
            match (chunk, is_composite) {
                (PaintChunk::Inline(s), _) => {
                    if !s.commands.is_empty() {
                        any_base_dirty = true;
                    }
                    base_scene.append_scene(s, Affine::IDENTITY);
                }
                (PaintChunk::Promoted(layer), true) => {
                    if layer.content_dirty {
                        all_composited_pose_only = false;
                    }
                    composited_layers.push(CompositedLayer {
                        layer_key: layer.layer_key,
                        fragment_id: layer.fragment_id,
                        scene: layer.scene,
                        bounds: layer.bounds,
                        transform: layer.transform,
                        clip: layer.clip,
                        opacity: layer.opacity,
                        blend_mode: layer.blend_mode,
                        content_dirty: layer.content_dirty,
                        pose_only_dirty: layer.pose_only_dirty,
                    });
                }
                (PaintChunk::Promoted(layer), false) => {
                    // Demote: flatten into base with transform/opacity/clip.
                    any_base_dirty = true;
                    flatten_promoted_into_scene(&mut base_scene, &layer);
                }
            }
        }

        let pose_only = !any_base_dirty
            && !composited_layers.is_empty()
            && all_composited_pose_only;

        RenderPlan {
            base_scene,
            composited_layers,
            stale_keys,
            pose_only,
        }
    }
}

/// Flatten a promoted layer into a scene, applying its transform/opacity/clip.
fn flatten_promoted_into_scene(scene: &mut Scene, layer: &PromotedLayer) {
    let needs_layer = layer.opacity < 1.0 - f32::EPSILON
        || layer.clip.is_some()
        || layer.blend_mode != BlendMode::default();

    if needs_layer {
        push_fragment_layer(
            scene,
            layer.transform,
            layer.clip.as_ref(),
            layer.opacity,
            layer.blend_mode,
        );
        scene.append_scene(layer.scene.clone(), layer.transform);
        scene.pop_layer();
    } else {
        scene.append_scene(layer.scene.clone(), layer.transform);
    }
}

fn rects_intersect(a: Rect, b: Rect) -> bool {
    a.x0 < b.x1 && a.x1 > b.x0 && a.y0 < b.y1 && a.y1 > b.y0
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
