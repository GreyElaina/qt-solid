use super::decl::{FragmentBlendMode, FragmentValue};
use super::super::vello::peniko::{
    kurbo::RoundedRectRadii,
    BlendMode, Color, Fill,
};
use super::types::{BorderSide, FillPaint, FragmentBoxShadow, FragmentBrush, GradientStop};

// ---------------------------------------------------------------------------
// Color parsing helper — used by derive-generated code
// ---------------------------------------------------------------------------

pub fn parse_color_from_wire(value: &FragmentValue) -> Option<Color> {
    match value {
        FragmentValue::Str { value: s } => parse_css_hex_color(s),
        _ => None,
    }
}

pub(crate) fn parse_css_hex_color(s: &str) -> Option<Color> {
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

pub(crate) fn parse_gradient_stops(offsets: &[f64], colors: &[String]) -> Vec<GradientStop> {
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

pub fn parse_border_from_wire(value: &FragmentValue) -> Option<BorderSide> {
    match value {
        FragmentValue::Border { width, color } => {
            parse_css_hex_color(color).map(|c| BorderSide { width: *width, color: c })
        }
        _ => None,
    }
}

impl From<FragmentBlendMode> for BlendMode {
    fn from(mode: FragmentBlendMode) -> Self {
        use super::super::vello::peniko::{Mix, Compose};
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
