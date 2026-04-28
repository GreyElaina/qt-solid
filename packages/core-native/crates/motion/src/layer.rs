use kurbo::{Affine, Rect};

use crate::{LayerClip, MotionNode, NodeId};

pub type LayerId = u64;

#[derive(Debug, Clone, PartialEq)]
pub struct MotionLayer {
    pub layer_id: LayerId,
    pub node_id: NodeId,
    pub bounds: Rect,
    pub transform: Affine,
    pub opacity: f32,
    pub clip: Option<LayerClip>,
    pub content_revision: u64,
    pub layout_revision: u64,
    pub z_index: i32,
}

impl MotionLayer {
    pub fn from_node(layer_id: LayerId, node: &MotionNode) -> Self {
        Self {
            layer_id,
            node_id: node.node_id,
            bounds: node.bounds,
            transform: node.transform,
            opacity: node.opacity,
            clip: node.clip.clone(),
            content_revision: node.content_revision,
            layout_revision: node.layout_revision,
            z_index: node.z_index,
        }
    }

    pub fn same_content_as(&self, node: &MotionNode) -> bool {
        self.node_id == node.node_id && self.content_revision == node.content_revision
    }

    pub fn same_layout_as(&self, node: &MotionNode) -> bool {
        self.node_id == node.node_id
            && self.layout_revision == node.layout_revision
            && self.bounds == node.bounds
    }

    pub fn same_pose_as(&self, node: &MotionNode) -> bool {
        self.node_id == node.node_id
            && self.transform == node.transform
            && self.opacity.to_bits() == node.opacity.to_bits()
            && self.clip == node.clip
            && self.z_index == node.z_index
    }
}
