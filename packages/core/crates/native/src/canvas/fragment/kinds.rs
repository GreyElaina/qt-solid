use fragment_derive::Fragment;
use unicode_segmentation::UnicodeSegmentation;

use super::super::vello::peniko::{
    Color, ImageData,
    kurbo::{BezPath, RoundedRectRadii},
};
use super::decl::{FragmentMutation, FragmentPropDecl};
use super::types::{
    BorderSide, FillPaint, FragmentBoxShadow, FragmentBrush, ShapedTextCache, ShapedTextLayout,
    StrokePaint,
};

// ---------------------------------------------------------------------------
// Fragment kind structs — #[derive(Fragment)] generates FragmentDecl impls
// ---------------------------------------------------------------------------

#[derive(Fragment, Debug, Clone, Default)]
#[fragment(tag = "group", bounds = none)]
pub struct GroupFragment {}

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
/// Keep the caret away from the clipped edge while scrolling horizontal text.
pub(crate) const TEXT_INPUT_SCROLL_PADDING: f64 = 4.0;

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
    #[fragment(skip)]
    pub viewport_width: f64,
    #[fragment(skip)]
    pub viewport_height: f64,
    #[fragment(skip)]
    pub scroll_x: f64,
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
            viewport_width: 0.0,
            viewport_height: 0.0,
            scroll_x: 0.0,
        }
    }
}

impl TextInputFragment {
    pub(crate) fn visible_width(&self) -> f64 {
        if self.viewport_width > 0.0 {
            self.viewport_width
        } else {
            self.layout.as_ref().map_or(0.0, |layout| layout.width)
        }
    }

    pub(crate) fn visible_height(&self) -> f64 {
        if self.viewport_height > 0.0 {
            self.viewport_height
        } else {
            self.layout.as_ref().map_or(0.0, |layout| layout.height)
        }
    }

    pub(crate) fn text_offset_to_cursor_position(&self, text_offset: f64) -> usize {
        let target = if text_offset.is_finite() && text_offset > 0.0 {
            text_offset as usize
        } else {
            0
        };

        let mut utf16_offset = 0usize;
        for grapheme in self.text.graphemes(true) {
            if target <= utf16_offset {
                return utf16_offset;
            }
            utf16_offset += grapheme.chars().map(char::len_utf16).sum::<usize>();
            if target <= utf16_offset {
                return utf16_offset;
            }
        }
        utf16_offset
    }

    pub(crate) fn cursor_x_for_text_offset(&self, text_offset: f64) -> f64 {
        let Some(layout) = &self.layout else {
            return 0.0;
        };
        let cursor = self.text_offset_to_cursor_position(text_offset);
        let max_pos = layout.cursor_x_positions.len().saturating_sub(1);
        layout
            .cursor_x_positions
            .get(cursor.min(max_pos))
            .copied()
            .unwrap_or(0.0)
    }

    pub(crate) fn cursor_x(&self) -> f64 {
        self.cursor_x_for_text_offset(self.cursor_pos)
    }

    pub(crate) fn max_scroll_x(&self) -> f64 {
        let Some(layout) = &self.layout else {
            return 0.0;
        };
        let visible_width = self.visible_width();
        let content_width = layout.width + CARET_WIDTH;
        if visible_width <= 0.0 || content_width <= visible_width {
            0.0
        } else {
            content_width - visible_width
        }
    }

    pub(crate) fn horizontal_scroll(&self) -> f64 {
        self.scroll_x.clamp(0.0, self.max_scroll_x())
    }

    pub(crate) fn ensure_caret_visible(&mut self) -> bool {
        if self.layout.is_none() {
            return false;
        }

        let previous = self.scroll_x;
        let visible_width = self.visible_width();
        let max_scroll = self.max_scroll_x();
        if max_scroll <= 0.0 {
            self.scroll_x = 0.0;
            return (previous - self.scroll_x).abs() > 0.01;
        }

        let padding = TEXT_INPUT_SCROLL_PADDING.min((visible_width - CARET_WIDTH).max(0.0) / 2.0);
        let cursor_x = self.cursor_x();
        let left_edge = self.scroll_x + padding;
        let right_edge = self.scroll_x + visible_width - padding;

        if cursor_x < left_edge {
            self.scroll_x = (cursor_x - padding).clamp(0.0, max_scroll);
        } else if cursor_x + CARET_WIDTH > right_edge {
            self.scroll_x =
                (cursor_x + CARET_WIDTH + padding - visible_width).clamp(0.0, max_scroll);
        } else {
            self.scroll_x = self.scroll_x.clamp(0.0, max_scroll);
        }

        (previous - self.scroll_x).abs() > 0.01
    }

    pub(crate) fn visible_x_to_text_x(&self, local_x: f64) -> f64 {
        local_x + self.horizontal_scroll()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vello::peniko::kurbo::BezPath;

    fn input_with_cursor(cursor_pos: f64, viewport_width: f64, scroll_x: f64) -> TextInputFragment {
        TextInputFragment {
            text: "abcdef".to_string(),
            cursor_pos,
            viewport_width,
            viewport_height: 16.0,
            scroll_x,
            layout: Some(ShapedTextLayout {
                path: BezPath::new(),
                rasterized_glyphs: Vec::new(),
                cursor_x_positions: vec![0.0, 10.0, 20.0, 30.0, 40.0, 50.0, 60.0],
                width: 60.0,
                height: 16.0,
                ascent: 12.0,
            }),
            ..Default::default()
        }
    }

    fn input_with_text(text: &str, cursor_pos: f64) -> TextInputFragment {
        let utf16_len = text.encode_utf16().count();
        TextInputFragment {
            text: text.to_string(),
            cursor_pos,
            layout: Some(ShapedTextLayout {
                path: BezPath::new(),
                rasterized_glyphs: Vec::new(),
                cursor_x_positions: (0..=utf16_len).map(|idx| idx as f64 * 10.0).collect(),
                width: utf16_len as f64 * 10.0,
                height: 16.0,
                ascent: 12.0,
            }),
            ..Default::default()
        }
    }

    #[test]
    fn scroll_stays_when_caret_remains_inside_padded_viewport() {
        let mut input = input_with_cursor(2.0, 30.0, 10.0);

        assert!(!input.ensure_caret_visible());
        assert_eq!(input.horizontal_scroll(), 10.0);
    }

    #[test]
    fn scroll_moves_right_after_caret_crosses_right_padding() {
        let mut input = input_with_cursor(3.0, 30.0, 0.0);

        assert!(input.ensure_caret_visible());
        assert_eq!(input.horizontal_scroll(), 5.0);
    }

    #[test]
    fn scroll_moves_left_after_caret_crosses_left_padding() {
        let mut input = input_with_cursor(1.0, 30.0, 20.0);

        assert!(input.ensure_caret_visible());
        assert_eq!(input.horizontal_scroll(), 6.0);
    }

    #[test]
    fn scroll_clamps_at_right_edge() {
        let mut input = input_with_cursor(6.0, 30.0, 0.0);

        assert!(input.ensure_caret_visible());
        assert_eq!(input.horizontal_scroll(), 31.0);
    }

    #[test]
    fn scroll_clamps_at_left_edge() {
        let mut input = input_with_cursor(0.0, 30.0, 20.0);

        assert!(input.ensure_caret_visible());
        assert_eq!(input.horizontal_scroll(), 0.0);
    }

    #[test]
    fn scroll_resets_when_text_fits() {
        let mut input = input_with_cursor(6.0, 80.0, 10.0);

        assert!(input.ensure_caret_visible());
        assert_eq!(input.horizontal_scroll(), 0.0);
    }

    #[test]
    fn visible_x_to_text_x_uses_persisted_scroll() {
        let input = input_with_cursor(2.0, 30.0, 12.0);

        assert_eq!(input.visible_x_to_text_x(3.0), 15.0);
    }

    #[test]
    fn text_offset_to_cursor_position_snaps_inside_zwj_grapheme_to_trailing_boundary() {
        let input = input_with_text("a👨‍👩‍👦b", 0.0);

        assert_eq!(input.text_offset_to_cursor_position(1.0), 1);
        assert_eq!(input.text_offset_to_cursor_position(2.0), 9);
        assert_eq!(input.text_offset_to_cursor_position(8.0), 9);
        assert_eq!(input.text_offset_to_cursor_position(9.0), 9);
    }

    #[test]
    fn cursor_x_snaps_inside_zwj_grapheme_to_trailing_boundary() {
        let input = input_with_text("a👨‍👩‍👦b", 3.0);

        assert_eq!(input.cursor_x(), 90.0);
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
