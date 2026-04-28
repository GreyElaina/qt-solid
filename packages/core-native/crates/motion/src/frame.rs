use kurbo::{Affine, Rect};

use crate::{LayerClip, LayerId, NodeId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerWork {
    Reuse,
    Repaint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotionClass {
    Static,
    CompositorOnly,
    LayoutMotion,
    ParameterizedPaint,
    Repaint,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayerFrame {
    pub layer_id: LayerId,
    pub node_id: NodeId,
    pub class: MotionClass,
    pub work: LayerWork,
    pub bounds: Rect,
    pub transform: Affine,
    pub opacity: f32,
    pub clip: Option<LayerClip>,
    pub z_index: i32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct MotionFrame {
    pub layers: Vec<LayerFrame>,
}

impl MotionFrame {
    pub fn push(&mut self, layer: LayerFrame) {
        self.layers.push(layer);
    }

    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }
}
