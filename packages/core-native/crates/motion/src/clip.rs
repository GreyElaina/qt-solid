use kurbo::Rect;

#[derive(Debug, Clone, PartialEq)]
pub enum LayerClip {
    Rect(Rect),
    RoundedRect { rect: Rect, radius: f64 },
}

impl LayerClip {
    pub fn bounds(&self) -> Rect {
        match self {
            Self::Rect(rect) => *rect,
            Self::RoundedRect { rect, .. } => *rect,
        }
    }
}
