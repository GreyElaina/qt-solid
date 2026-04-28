use crate::{LayerFrame, LayerId, LayerWork, MotionClass, MotionLayer, MotionNode};

pub fn layer_frame_for_node(
    previous: Option<&MotionLayer>,
    next_layer_id: LayerId,
    node: &MotionNode,
) -> LayerFrame {
    let (class, work) = match previous {
        None => (MotionClass::Repaint, LayerWork::Repaint),
        Some(layer) if !layer.same_content_as(node) => (MotionClass::Repaint, LayerWork::Repaint),
        Some(layer) if !layer.same_layout_as(node) => (MotionClass::LayoutMotion, LayerWork::Reuse),
        Some(layer) if !layer.same_pose_as(node) => (MotionClass::CompositorOnly, LayerWork::Reuse),
        Some(_) => (MotionClass::Static, LayerWork::Reuse),
    };

    LayerFrame {
        layer_id: previous
            .map(|layer| layer.layer_id)
            .unwrap_or(next_layer_id),
        node_id: node.node_id,
        class,
        work,
        bounds: node.bounds,
        transform: node.transform,
        opacity: node.opacity,
        clip: node.clip.clone(),
        z_index: node.z_index,
    }
}

pub fn next_layer_for_node(
    previous: Option<&MotionLayer>,
    next_layer_id: LayerId,
    node: &MotionNode,
) -> MotionLayer {
    MotionLayer::from_node(
        previous
            .map(|layer| layer.layer_id)
            .unwrap_or(next_layer_id),
        node,
    )
}

#[cfg(test)]
mod tests {
    use kurbo::{Affine, Rect};

    use crate::{
        LayerClip, LayerWork, MotionClass, MotionLayer, MotionNode, layer_frame_for_node,
        next_layer_for_node,
    };

    fn node() -> MotionNode {
        MotionNode {
            node_id: 7,
            bounds: Rect::new(10.0, 20.0, 110.0, 220.0),
            transform: Affine::IDENTITY,
            opacity: 1.0,
            clip: Some(LayerClip::Rect(Rect::new(10.0, 20.0, 110.0, 220.0))),
            content_revision: 1,
            layout_revision: 1,
            z_index: 0,
        }
    }

    #[test]
    fn new_node_needs_repaint() {
        let current = node();
        let frame = layer_frame_for_node(None, 99, &current);
        assert_eq!(frame.class, MotionClass::Repaint);
        assert_eq!(frame.work, LayerWork::Repaint);
        assert_eq!(frame.layer_id, 99);
    }

    #[test]
    fn pose_change_reuses_layer() {
        let current = node();
        let previous = MotionLayer::from_node(4, &current);
        let mut next = current.clone();
        next.opacity = 0.4;
        next.transform = Affine::translate((25.0, 0.0));

        let frame = layer_frame_for_node(Some(&previous), 99, &next);
        assert_eq!(frame.class, MotionClass::CompositorOnly);
        assert_eq!(frame.work, LayerWork::Reuse);
        assert_eq!(frame.layer_id, 4);
    }

    #[test]
    fn layout_change_reuses_layer() {
        let current = node();
        let previous = MotionLayer::from_node(4, &current);
        let mut next = current.clone();
        next.layout_revision = 2;
        next.bounds = Rect::new(20.0, 40.0, 140.0, 260.0);

        let frame = layer_frame_for_node(Some(&previous), 99, &next);
        assert_eq!(frame.class, MotionClass::LayoutMotion);
        assert_eq!(frame.work, LayerWork::Reuse);
    }

    #[test]
    fn next_layer_keeps_existing_id() {
        let current = node();
        let previous = MotionLayer::from_node(4, &current);
        let next = next_layer_for_node(Some(&previous), 99, &current);
        assert_eq!(next.layer_id, 4);
    }
}
