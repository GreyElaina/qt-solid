use super::super::vello::{
    peniko::kurbo::{Affine, Rect},
    Scene,
};

// ---------------------------------------------------------------------------
// Blend mode — napi string enum for the JS → Rust boundary
// ---------------------------------------------------------------------------

#[napi_derive::napi(string_enum)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FragmentBlendMode {
    #[napi(value = "normal")]
    Normal,
    #[napi(value = "multiply")]
    Multiply,
    #[napi(value = "screen")]
    Screen,
    #[napi(value = "overlay")]
    Overlay,
    #[napi(value = "darken")]
    Darken,
    #[napi(value = "lighten")]
    Lighten,
    #[napi(value = "color-dodge")]
    ColorDodge,
    #[napi(value = "color-burn")]
    ColorBurn,
    #[napi(value = "hard-light")]
    HardLight,
    #[napi(value = "soft-light")]
    SoftLight,
    #[napi(value = "difference")]
    Difference,
    #[napi(value = "exclusion")]
    Exclusion,
    #[napi(value = "hue")]
    Hue,
    #[napi(value = "saturation")]
    Saturation,
    #[napi(value = "color")]
    Color,
    #[napi(value = "luminosity")]
    Luminosity,
}

// ---------------------------------------------------------------------------
// Prop value — the typed value crossing the JS → Rust boundary
// ---------------------------------------------------------------------------

#[napi_derive::napi(object)]
#[derive(Debug, Clone)]
pub struct FragmentTextRunWire {
    pub text: String,
    pub font_size: f64,
    pub font_family: String,
    pub font_weight: i32,
    pub font_italic: bool,
    pub color: String,
}

#[napi_derive::napi(discriminant_case = "lowercase")]
#[derive(Debug, Clone)]
pub enum FragmentValue {
    F64 { value: f64 },
    Str { value: String },
    Bool { value: bool },
    LinearGradient {
        start_x: f64,
        start_y: f64,
        end_x: f64,
        end_y: f64,
        stop_offsets: Vec<f64>,
        stop_colors: Vec<String>,
    },
    RadialGradient {
        center_x: f64,
        center_y: f64,
        radius: f64,
        stop_offsets: Vec<f64>,
        stop_colors: Vec<String>,
    },
    SweepGradient {
        center_x: f64,
        center_y: f64,
        start_angle: f64,
        end_angle: f64,
        stop_offsets: Vec<f64>,
        stop_colors: Vec<String>,
    },
    BoxShadow {
        offset_x: f64,
        offset_y: f64,
        blur: f64,
        color: String,
        inset: bool,
    },
    TextRuns {
        runs: Vec<FragmentTextRunWire>,
    },
    Radii {
        top_left: f64,
        top_right: f64,
        bottom_right: f64,
        bottom_left: f64,
    },
    GridTracks {
        tracks: Vec<String>,
    },
    Border {
        width: f64,
        color: String,
    },
    BlendMode {
        value: FragmentBlendMode,
    },
    Unset,
}

// ---------------------------------------------------------------------------
// Mutation flags — what changed as a result of a prop update
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FragmentMutation(u8);

impl FragmentMutation {
    pub const NONE: Self = Self(0);
    pub const PAINT: Self = Self(1);
    pub const LAYOUT: Self = Self(2);
    pub const HIT_TEST: Self = Self(4);
    pub const RESHAPE_TEXT: Self = Self(8);

    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl std::ops::BitOr for FragmentMutation {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for FragmentMutation {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

// ---------------------------------------------------------------------------
// Prop declaration — schema metadata for a single prop
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FragmentPropDecl {
    pub rust_name: &'static str,
    pub js_name: &'static str,
    pub mutation: FragmentMutation,
}

// ---------------------------------------------------------------------------
// FragmentDecl — trait implemented per-kind struct via #[derive(Fragment)]
// ---------------------------------------------------------------------------

pub trait FragmentDecl: Default + Clone + std::fmt::Debug {
    const TAG: &'static str;
    const PROPS: &'static [FragmentPropDecl];

    fn apply_prop(&mut self, key: &str, value: FragmentValue) -> FragmentMutation;
    fn reset_prop(&mut self, key: &str) -> FragmentMutation;
    fn local_bounds(&self) -> Option<Rect>;
}

// ---------------------------------------------------------------------------
// FragmentEncode — hand-written paint encoding per-kind
// ---------------------------------------------------------------------------

pub trait FragmentEncode {
    fn encode(&self, scene: &mut Scene, transform: Affine);
}

// ---------------------------------------------------------------------------
// Schema entry — returned by enum-level all_schemas()
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FragmentSchemaEntry {
    pub tag: &'static str,
    pub props: &'static [FragmentPropDecl],
}
