use kurbo::{Affine, Rect};

use crate::LayerClip;

pub type NodeId = u32;

#[derive(Debug, Clone, PartialEq)]
pub struct MotionNode {
    pub node_id: NodeId,
    pub bounds: Rect,
    pub transform: Affine,
    pub opacity: f32,
    pub clip: Option<LayerClip>,
    pub content_revision: u64,
    pub layout_revision: u64,
    pub z_index: i32,
}

impl MotionNode {
    pub fn same_content_as(&self, other: &Self) -> bool {
        self.node_id == other.node_id && self.content_revision == other.content_revision
    }

    pub fn same_layout_as(&self, other: &Self) -> bool {
        self.node_id == other.node_id
            && self.layout_revision == other.layout_revision
            && self.bounds == other.bounds
    }

    pub fn same_pose_as(&self, other: &Self) -> bool {
        self.node_id == other.node_id
            && self.transform == other.transform
            && self.opacity.to_bits() == other.opacity.to_bits()
            && self.clip == other.clip
            && self.z_index == other.z_index
    }
}
