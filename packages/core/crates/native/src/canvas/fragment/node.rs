use fragment_derive::Fragment;

use super::super::vello::peniko::{
    kurbo::{Affine, BezPath, Rect, RoundedRectRadii},
    BlendMode, Color, Fill,
};
use super::kinds::{
    CircleFragment, GroupFragment, ImageFragment, PathFragment, RectFragment, SpanFragment,
    TextFragment, TextInputFragment,
};
use super::types::{
    FillPaint, FragmentBoxShadow, FragmentBrush, FragmentClipShape, FragmentId, FragmentLayerKey,
    FragmentListeners,
};

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
    pub clip_path: Option<BezPath>,
    pub visible: bool,
    pub pointer_events: bool,
    pub cursor: u8,
    pub focusable: bool,
    pub transform: Affine,
    pub backdrop_blur: Option<f64>,
    pub z_index: i32,
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
            clip_path: None,
            visible: true,
            pointer_events: true,
            cursor: 0,
            focusable: false,
            transform: Affine::IDENTITY,
            backdrop_blur: None,
            z_index: 0,
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
    /// Return bounds as a Rect if width and height are positive.
    pub fn bounds(&self) -> Option<Rect> {
        if self.width > 0.0 && self.height > 0.0 {
            Some(Rect::new(0.0, 0.0, self.width, self.height))
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Fragment node — one entry in the tree
// ---------------------------------------------------------------------------

#[derive(Debug)]
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
    pub(crate) world_aabb: Option<Rect>,
    /// Cached AABB enclosing this node and all descendants.
    pub(crate) subtree_aabb: Option<Rect>,
    pub listeners: FragmentListeners,
}

impl FragmentNode {
    pub(crate) fn render_x(&self) -> f64 {
        self.props.explicit_x.unwrap_or(self.layout.x)
    }

    pub(crate) fn render_y(&self) -> f64 {
        self.props.explicit_y.unwrap_or(self.layout.y)
    }

    pub(crate) fn local_transform(&self) -> Affine {
        Affine::translate((self.render_x(), self.render_y())) * self.props.transform
    }

    pub(crate) fn needs_layer(&self) -> bool {
        self.props.opacity < 1.0 - f32::EPSILON
            || self.props.clip
            || self.props.clip_path.is_some()
            || self.props.blend_mode != BlendMode::default()
    }

    /// Kind-level paint bounds, falling back to layout-computed bounds.
    pub(crate) fn effective_bounds(&self) -> Option<Rect> {
        self.kind.local_bounds().or_else(|| self.layout.bounds())
    }

    pub(crate) fn clip_shape(&self) -> Option<FragmentClipShape> {
        if let Some(path) = &self.props.clip_path {
            return Some(FragmentClipShape::Path(path.clone()));
        }
        if self.props.clip {
            return self.effective_bounds().map(FragmentClipShape::Rect);
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Motion pose → fragment property mapping
// ---------------------------------------------------------------------------

pub(crate) fn apply_sampled_pose_to_fragment(
    node: &mut FragmentNode,
    pose: &motion::SampledPose,
    timeline: &motion::NodeTimeline,
) {
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

    // Paint channels — only apply when the timeline actually targets them
    if let FragmentData::Rect(ref mut rect) = node.kind {
        use motion::PropertyKey;

        // Background color
        if timeline.has_property(PropertyKey::BackgroundR)
            || timeline.has_property(PropertyKey::BackgroundG)
            || timeline.has_property(PropertyKey::BackgroundB)
            || timeline.has_property(PropertyKey::BackgroundA)
        {
            let color = Color::new([
                pose.background_r as f32,
                pose.background_g as f32,
                pose.background_b as f32,
                pose.background_a as f32,
            ]);
            rect.fill = Some(FragmentBrush::Solid(FillPaint {
                color,
                rule: Fill::NonZero,
            }));
        }

        // Border radius
        if timeline.has_property(PropertyKey::BorderRadius) {
            rect.corner_radii = RoundedRectRadii::from_single_radius(pose.border_radius);
        }

        // Box shadow
        if timeline.has_property(PropertyKey::ShadowBlurRadius)
            || timeline.has_property(PropertyKey::ShadowOffsetX)
            || timeline.has_property(PropertyKey::ShadowOffsetY)
            || timeline.has_property(PropertyKey::ShadowR)
        {
            let shadow_color = Color::new([
                pose.shadow_r as f32,
                pose.shadow_g as f32,
                pose.shadow_b as f32,
                pose.shadow_a as f32,
            ]);
            rect.shadow = Some(FragmentBoxShadow {
                offset_x: pose.shadow_offset_x,
                offset_y: pose.shadow_offset_y,
                blur: pose.shadow_blur_radius,
                color: shadow_color,
                inset: false,
            });
        }

        // Blur radius (backdrop blur on the node)
        if timeline.has_property(PropertyKey::BlurRadius) {
            node.props.backdrop_blur = if pose.blur_radius > 0.01 {
                Some(pose.blur_radius)
            } else {
                None
            };
        }
    }

    if node.promoted {
        // Promoted nodes: pose-only dirty — compositor handles transform/opacity.
        node.pose_dirty = true;
    } else {
        node.dirty = true;
    }
}
