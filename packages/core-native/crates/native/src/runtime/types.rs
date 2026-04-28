use std::{error::Error, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NodeKind {
    Root,
    Window,
}

impl NodeKind {
    pub(crate) fn is_root(self) -> bool {
        matches!(self, Self::Root)
    }

    pub(crate) fn is_window(self) -> bool {
        matches!(self, Self::Window)
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Root => "root",
            Self::Window => "window",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WidgetError {
    message: String,
}

impl WidgetError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for WidgetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for WidgetError {}

pub type WidgetResult<T> = Result<T, WidgetError>;
