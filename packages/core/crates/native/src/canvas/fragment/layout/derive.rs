use taffy::prelude::*;
use taffy::style::{
    AlignContent, AlignItems, AlignSelf, Dimension, Display, FlexDirection, FlexWrap,
    JustifyContent, LengthPercentage, LengthPercentageAuto, Position,
};

use super::{
    AbsoluteInset, CellAlign, Container, CrossAlign, Direction, EdgeInsets, Overflow, Placement,
    PrimaryAlign, SizeIntent, Sizing, TrackSize, WrapDistribute,
};

pub fn derive_taffy_style(
    placement: &Placement,
    container: Option<&Container>,
    padding: &EdgeInsets,
    overflow_x: Overflow,
    overflow_y: Overflow,
    layout_visible: bool,
    parent_direction: Direction,
) -> taffy::Style {
    let mut style = taffy::Style {
        flex_shrink: 0.0,
        ..Default::default()
    };

    if !layout_visible {
        style.display = Display::None;
        return style;
    }

    // ─── Container ───
    match container {
        Some(Container::Flex {
            direction,
            primary_align,
            cross_align,
            gap,
            cross_gap,
            wrap,
            wrap_distribute,
        }) => {
            style.display = Display::Flex;
            style.flex_direction = match direction {
                Direction::Horizontal => FlexDirection::Row,
                Direction::Vertical => FlexDirection::Column,
            };
            style.justify_content = Some(map_primary_align(*primary_align));
            style.align_items = Some(map_cross_align_to_items(*cross_align));
            style.gap = taffy::geometry::Size {
                width: LengthPercentage::length(*gap),
                height: LengthPercentage::length(cross_gap.unwrap_or(*gap)),
            };
            style.flex_wrap = if *wrap { FlexWrap::Wrap } else { FlexWrap::NoWrap };
            if *wrap {
                style.align_content = Some(map_wrap_distribute(*wrap_distribute));
            }
        }
        Some(Container::Grid {
            columns,
            rows,
            column_gap,
            row_gap,
        }) => {
            style.display = Display::Grid;
            style.grid_template_columns = columns.iter().map(|t| map_track_size(*t)).collect();
            style.grid_template_rows = rows.iter().map(|t| map_track_size(*t)).collect();
            style.gap = taffy::geometry::Size {
                width: LengthPercentage::length(*column_gap),
                height: LengthPercentage::length(*row_gap),
            };
        }
        None => {
            style.display = Display::Flex;
            style.flex_direction = FlexDirection::Column;
            style.align_items = Some(AlignItems::Stretch);
        }
    }

    // ─── Padding ───
    style.padding = map_edge_insets_lp(padding);

    // ─── Overflow ───
    style.overflow = taffy::geometry::Point {
        x: map_overflow(overflow_x),
        y: map_overflow(overflow_y),
    };

    // ─── Placement ───
    match placement {
        Placement::Flow {
            sizing,
            align_self,
            margin,
        } => {
            style.position = Position::Relative;
            apply_sizing(&mut style, sizing, parent_direction);
            if let Some(a) = align_self {
                style.align_self = Some(map_cross_align_to_self(*a));
            }
            if let Some(m) = margin {
                style.margin = map_edge_insets_auto(m);
            }
        }
        Placement::Absolute { inset, sizing } => {
            style.position = Position::Absolute;
            style.inset = map_inset(inset);
            apply_sizing_absolute(&mut style, sizing);
        }
        Placement::GridCell {
            row,
            column,
            row_span,
            col_span,
            h_align,
            v_align,
            sizing,
        } => {
            style.grid_row = taffy::geometry::Line {
                start: GridPlacement::from_line_index(*row as i16 + 1),
                end: GridPlacement::Span((*row_span).max(1)),
            };
            style.grid_column = taffy::geometry::Line {
                start: GridPlacement::from_line_index(*column as i16 + 1),
                end: GridPlacement::Span((*col_span).max(1)),
            };
            style.justify_self = Some(map_cell_align_to_self(*h_align));
            style.align_self = Some(map_cell_align_to_self(*v_align));
            apply_sizing_absolute(&mut style, sizing);
        }
    }

    style
}

// ─── Sizing application ───

fn apply_sizing(style: &mut taffy::Style, sizing: &SizeIntent, parent_dir: Direction) {
    let w_is_main = parent_dir == Direction::Horizontal;
    apply_axis(style, sizing.w, true, w_is_main);
    apply_axis(style, sizing.h, false, !w_is_main);

    if let Some(v) = sizing.min_w {
        style.min_size.width = Dimension::length(v);
    }
    if let Some(v) = sizing.max_w {
        style.max_size.width = Dimension::length(v);
    }
    if let Some(v) = sizing.min_h {
        style.min_size.height = Dimension::length(v);
    }
    if let Some(v) = sizing.max_h {
        style.max_size.height = Dimension::length(v);
    }
}

fn apply_sizing_absolute(style: &mut taffy::Style, sizing: &SizeIntent) {
    // Absolute/grid-cell: no main-axis concept, map Fixed/Percent directly, ignore Fill
    set_dim_axis(style, sizing.w, true);
    set_dim_axis(style, sizing.h, false);

    if let Some(v) = sizing.min_w {
        style.min_size.width = Dimension::length(v);
    }
    if let Some(v) = sizing.max_w {
        style.max_size.width = Dimension::length(v);
    }
    if let Some(v) = sizing.min_h {
        style.min_size.height = Dimension::length(v);
    }
    if let Some(v) = sizing.max_h {
        style.max_size.height = Dimension::length(v);
    }
}

fn apply_axis(style: &mut taffy::Style, sizing: Sizing, is_width: bool, is_main_axis: bool) {
    match sizing {
        Sizing::Hug => {
            set_size(style, is_width, Dimension::auto());
            if is_main_axis {
                style.flex_grow = 0.0;
                style.flex_shrink = 0.0;
            }
        }
        Sizing::Fill => {
            if is_main_axis {
                set_size(style, is_width, Dimension::length(0.0));
                style.flex_grow = 1.0;
                style.flex_shrink = 1.0;
            } else {
                set_size(style, is_width, Dimension::auto());
                style.align_self = Some(AlignSelf::Stretch);
            }
        }
        Sizing::Fixed(v) => {
            set_size(style, is_width, Dimension::length(v));
            if is_main_axis {
                style.flex_grow = 0.0;
                style.flex_shrink = 0.0;
            }
        }
        Sizing::Percent(v) => {
            set_size(style, is_width, Dimension::percent(v));
            if is_main_axis {
                style.flex_grow = 0.0;
                style.flex_shrink = 0.0;
            }
        }
        Sizing::Flex(weight) => {
            if is_main_axis {
                set_size(style, is_width, Dimension::length(0.0));
                style.flex_grow = weight;
                style.flex_shrink = 1.0;
            } else {
                set_size(style, is_width, Dimension::auto());
                style.align_self = Some(AlignSelf::Stretch);
            }
        }
    }
}

fn set_dim_axis(style: &mut taffy::Style, sizing: Sizing, is_width: bool) {
    match sizing {
        Sizing::Hug | Sizing::Fill | Sizing::Flex(_) => set_size(style, is_width, Dimension::auto()),
        Sizing::Fixed(v) => set_size(style, is_width, Dimension::length(v)),
        Sizing::Percent(v) => set_size(style, is_width, Dimension::percent(v)),
    }
}

fn set_size(style: &mut taffy::Style, is_width: bool, dim: Dimension) {
    if is_width {
        style.size.width = dim;
    } else {
        style.size.height = dim;
    }
}

// ─── Mappers ───

fn map_primary_align(a: PrimaryAlign) -> JustifyContent {
    match a {
        PrimaryAlign::Start => JustifyContent::FlexStart,
        PrimaryAlign::Center => JustifyContent::Center,
        PrimaryAlign::End => JustifyContent::FlexEnd,
        PrimaryAlign::SpaceBetween => JustifyContent::SpaceBetween,
        PrimaryAlign::SpaceAround => JustifyContent::SpaceAround,
        PrimaryAlign::SpaceEvenly => JustifyContent::SpaceEvenly,
    }
}

fn map_cross_align_to_items(a: CrossAlign) -> AlignItems {
    match a {
        CrossAlign::Start => AlignItems::FlexStart,
        CrossAlign::Center => AlignItems::Center,
        CrossAlign::End => AlignItems::FlexEnd,
        CrossAlign::Stretch => AlignItems::Stretch,
        CrossAlign::Baseline => AlignItems::Baseline,
    }
}

fn map_cross_align_to_self(a: CrossAlign) -> AlignSelf {
    match a {
        CrossAlign::Start => AlignSelf::FlexStart,
        CrossAlign::Center => AlignSelf::Center,
        CrossAlign::End => AlignSelf::FlexEnd,
        CrossAlign::Stretch => AlignSelf::Stretch,
        CrossAlign::Baseline => AlignSelf::Baseline,
    }
}

fn map_cell_align_to_self(a: CellAlign) -> AlignSelf {
    match a {
        CellAlign::Auto => AlignSelf::Start,
        CellAlign::Start => AlignSelf::Start,
        CellAlign::Center => AlignSelf::Center,
        CellAlign::End => AlignSelf::End,
    }
}

fn map_wrap_distribute(d: WrapDistribute) -> AlignContent {
    match d {
        WrapDistribute::Packed => AlignContent::FlexStart,
        WrapDistribute::SpaceBetween => AlignContent::SpaceBetween,
    }
}

fn map_track_size(t: TrackSize) -> GridTemplateComponent<String> {
    match t {
        TrackSize::Fixed(v) => GridTemplateComponent::from_length(v),
        TrackSize::Flex(v) => GridTemplateComponent::from_fr(v),
        TrackSize::Hug => GridTemplateComponent::AUTO,
    }
}

fn map_overflow(o: Overflow) -> taffy::style::Overflow {
    match o {
        Overflow::Visible => taffy::style::Overflow::Visible,
        Overflow::Clip => taffy::style::Overflow::Clip,
        Overflow::Hidden => taffy::style::Overflow::Hidden,
        Overflow::Scroll => taffy::style::Overflow::Scroll,
    }
}

fn map_edge_insets_lp(e: &EdgeInsets) -> taffy::geometry::Rect<LengthPercentage> {
    taffy::geometry::Rect {
        top: LengthPercentage::length(e.top),
        right: LengthPercentage::length(e.right),
        bottom: LengthPercentage::length(e.bottom),
        left: LengthPercentage::length(e.left),
    }
}

fn map_edge_insets_auto(e: &EdgeInsets) -> taffy::geometry::Rect<LengthPercentageAuto> {
    taffy::geometry::Rect {
        top: LengthPercentageAuto::length(e.top),
        right: LengthPercentageAuto::length(e.right),
        bottom: LengthPercentageAuto::length(e.bottom),
        left: LengthPercentageAuto::length(e.left),
    }
}

fn map_inset(inset: &AbsoluteInset) -> taffy::geometry::Rect<LengthPercentageAuto> {
    taffy::geometry::Rect {
        top: inset
            .top
            .map(LengthPercentageAuto::length)
            .unwrap_or(LengthPercentageAuto::auto()),
        right: inset
            .right
            .map(LengthPercentageAuto::length)
            .unwrap_or(LengthPercentageAuto::auto()),
        bottom: inset
            .bottom
            .map(LengthPercentageAuto::length)
            .unwrap_or(LengthPercentageAuto::auto()),
        left: inset
            .left
            .map(LengthPercentageAuto::length)
            .unwrap_or(LengthPercentageAuto::auto()),
    }
}
