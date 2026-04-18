use std::fmt;

use crate::runtime::{QtEnumValue, QtTypeName, WidgetError, WidgetResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexDirection {
    Column,
    Row,
}

impl FlexDirection {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Column => "column",
            Self::Row => "row",
        }
    }
}

impl fmt::Display for FlexDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl QtTypeName for FlexDirection {
    fn qt_type_name() -> &'static str {
        "FlexDirection"
    }
}

impl QtEnumValue for FlexDirection {
    fn into_qt_enum(self) -> i32 {
        match self {
            Self::Column => 1,
            Self::Row => 2,
        }
    }

    fn try_from_qt_enum(value: i32) -> WidgetResult<Self> {
        match value {
            1 => Ok(Self::Column),
            2 => Ok(Self::Row),
            _ => Err(WidgetError::new(format!(
                "invalid FlexDirection tag {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignItems {
    FlexStart,
    Center,
    FlexEnd,
    Stretch,
}

impl AlignItems {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FlexStart => "flex-start",
            Self::Center => "center",
            Self::FlexEnd => "flex-end",
            Self::Stretch => "stretch",
        }
    }
}

impl fmt::Display for AlignItems {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl QtTypeName for AlignItems {
    fn qt_type_name() -> &'static str {
        "AlignItems"
    }
}

impl QtEnumValue for AlignItems {
    fn into_qt_enum(self) -> i32 {
        match self {
            Self::FlexStart => 1,
            Self::Center => 2,
            Self::FlexEnd => 3,
            Self::Stretch => 4,
        }
    }

    fn try_from_qt_enum(value: i32) -> WidgetResult<Self> {
        match value {
            1 => Ok(Self::FlexStart),
            2 => Ok(Self::Center),
            3 => Ok(Self::FlexEnd),
            4 => Ok(Self::Stretch),
            _ => Err(WidgetError::new(format!("invalid AlignItems tag {value}"))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JustifyContent {
    FlexStart,
    Center,
    FlexEnd,
}

impl JustifyContent {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FlexStart => "flex-start",
            Self::Center => "center",
            Self::FlexEnd => "flex-end",
        }
    }
}

impl fmt::Display for JustifyContent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl QtTypeName for JustifyContent {
    fn qt_type_name() -> &'static str {
        "JustifyContent"
    }
}

impl QtEnumValue for JustifyContent {
    fn into_qt_enum(self) -> i32 {
        match self {
            Self::FlexStart => 1,
            Self::Center => 2,
            Self::FlexEnd => 3,
        }
    }

    fn try_from_qt_enum(value: i32) -> WidgetResult<Self> {
        match value {
            1 => Ok(Self::FlexStart),
            2 => Ok(Self::Center),
            3 => Ok(Self::FlexEnd),
            _ => Err(WidgetError::new(format!(
                "invalid JustifyContent tag {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusPolicy {
    NoFocus,
    TabFocus,
    ClickFocus,
    StrongFocus,
}

impl FocusPolicy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoFocus => "no-focus",
            Self::TabFocus => "tab-focus",
            Self::ClickFocus => "click-focus",
            Self::StrongFocus => "strong-focus",
        }
    }
}

impl fmt::Display for FocusPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl QtTypeName for FocusPolicy {
    fn qt_type_name() -> &'static str {
        "FocusPolicy"
    }
}

impl QtEnumValue for FocusPolicy {
    fn into_qt_enum(self) -> i32 {
        match self {
            Self::NoFocus => 1,
            Self::TabFocus => 2,
            Self::ClickFocus => 3,
            Self::StrongFocus => 4,
        }
    }

    fn try_from_qt_enum(value: i32) -> WidgetResult<Self> {
        match value {
            1 => Ok(Self::NoFocus),
            2 => Ok(Self::TabFocus),
            3 => Ok(Self::ClickFocus),
            4 => Ok(Self::StrongFocus),
            _ => Err(WidgetError::new(format!("invalid FocusPolicy tag {value}"))),
        }
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SpecWidgetKey(pub &'static str);

impl SpecWidgetKey {
    pub const fn new(raw: &'static str) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> &'static str {
        self.0
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WidgetTypeId(pub u32);

impl WidgetTypeId {
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeClass {
    Root,
    Widget(WidgetTypeId),
}
