mod derive;

pub use derive::derive_taffy_style;

// ─── Sizing ───

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sizing {
    Hug,
    Fill,
    Fixed(f32),
    Percent(f32),
    Flex(f32),
}

impl Default for Sizing {
    fn default() -> Self {
        Self::Hug
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SizeIntent {
    pub w: Sizing,
    pub h: Sizing,
    pub min_w: Option<f32>,
    pub max_w: Option<f32>,
    pub min_h: Option<f32>,
    pub max_h: Option<f32>,
}

// ─── Direction ───

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Direction {
    #[default]
    Vertical,
    Horizontal,
}

// ─── Alignment ───

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum PrimaryAlign {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum CrossAlign {
    Start,
    Center,
    End,
    #[default]
    Stretch,
    Baseline,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum WrapDistribute {
    #[default]
    Packed,
    SpaceBetween,
}

// ─── Grid ───

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrackSize {
    Fixed(f32),
    Flex(f32),
    Hug,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum CellAlign {
    #[default]
    Auto,
    Start,
    Center,
    End,
}

// ─── Edge ───

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct EdgeInsets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

// ─── Overflow ───

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Overflow {
    #[default]
    Visible,
    Clip,
    Hidden,
    Scroll,
}

// ─── Placement ───

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AbsoluteInset {
    pub top: Option<f32>,
    pub right: Option<f32>,
    pub bottom: Option<f32>,
    pub left: Option<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Placement {
    Flow {
        sizing: SizeIntent,
        align_self: Option<CrossAlign>,
        margin: Option<EdgeInsets>,
    },
    Absolute {
        inset: AbsoluteInset,
        sizing: SizeIntent,
    },
    GridCell {
        row: u16,
        column: u16,
        row_span: u16,
        col_span: u16,
        h_align: CellAlign,
        v_align: CellAlign,
        sizing: SizeIntent,
    },
}

impl Default for Placement {
    fn default() -> Self {
        Self::Flow {
            sizing: SizeIntent::default(),
            align_self: None,
            margin: None,
        }
    }
}

// ─── Container ───

#[derive(Debug, Clone, PartialEq)]
pub enum Container {
    Flex {
        direction: Direction,
        primary_align: PrimaryAlign,
        cross_align: CrossAlign,
        gap: f32,
        cross_gap: Option<f32>,
        wrap: bool,
        wrap_distribute: WrapDistribute,
    },
    Grid {
        columns: Vec<TrackSize>,
        rows: Vec<TrackSize>,
        column_gap: f32,
        row_gap: f32,
    },
}
