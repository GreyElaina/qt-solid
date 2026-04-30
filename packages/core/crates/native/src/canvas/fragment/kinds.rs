use fragment_derive::Fragment;

use super::decl::{FragmentMutation, FragmentPropDecl};
use super::super::vello::peniko::{
    kurbo::{BezPath, RoundedRectRadii},
    Color, ImageData,
};
use super::types::{
    BorderSide, FillPaint, FragmentBoxShadow, FragmentBrush, ShapedTextCache, ShapedTextLayout,
    StrokePaint,
};

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
    #[fragment(prop(js = "borderTop"), parse = border, clear = none)]
    pub border_top: Option<BorderSide>,
    #[fragment(prop(js = "borderRight"), parse = border, clear = none)]
    pub border_right: Option<BorderSide>,
    #[fragment(prop(js = "borderBottom"), parse = border, clear = none)]
    pub border_bottom: Option<BorderSide>,
    #[fragment(prop(js = "borderLeft"), parse = border, clear = none)]
    pub border_left: Option<BorderSide>,
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
pub(crate) const SELECTION_COLOR: Color = Color::from_rgba8(51, 153, 255, 128);
/// Caret color (white).
pub(crate) const CARET_COLOR: Color = Color::from_rgba8(255, 255, 255, 255);
/// Caret width in logical pixels.
pub(crate) const CARET_WIDTH: f64 = 1.0;

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
