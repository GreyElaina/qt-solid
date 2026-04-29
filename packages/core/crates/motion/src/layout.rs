use kurbo::{Affine, Rect};

use crate::NodeId;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutChange {
    pub node_id: NodeId,
    pub previous_bounds: Rect,
    pub current_bounds: Rect,
}

impl LayoutChange {
    pub fn is_changed(&self) -> bool {
        self.previous_bounds != self.current_bounds
    }

    pub fn placement_transform(&self) -> Affine {
        let previous = self.previous_bounds;
        let current = self.current_bounds;
        let scale_x = axis_scale(previous.width(), current.width());
        let scale_y = axis_scale(previous.height(), current.height());

        Affine::translate((current.x0, current.y0))
            * Affine::scale_non_uniform(scale_x, scale_y)
            * Affine::translate((-previous.x0, -previous.y0))
    }
}

fn axis_scale(previous: f64, current: f64) -> f64 {
    if previous.abs() <= f64::EPSILON {
        1.0
    } else {
        current / previous
    }
}

#[cfg(test)]
mod tests {
    use super::LayoutChange;
    use kurbo::{Point, Rect};

    #[test]
    fn placement_transform_moves_origin() {
        let change = LayoutChange {
            node_id: 7,
            previous_bounds: Rect::new(10.0, 20.0, 110.0, 70.0),
            current_bounds: Rect::new(30.0, 50.0, 130.0, 100.0),
        };

        let mapped = change.placement_transform() * Point::new(10.0, 20.0);
        assert_eq!(mapped, Point::new(30.0, 50.0));
    }

    #[test]
    fn placement_transform_scales_far_corner() {
        let change = LayoutChange {
            node_id: 9,
            previous_bounds: Rect::new(10.0, 20.0, 110.0, 70.0),
            current_bounds: Rect::new(30.0, 50.0, 230.0, 150.0),
        };

        let mapped = change.placement_transform() * Point::new(110.0, 70.0);
        assert_eq!(mapped, Point::new(230.0, 150.0));
    }
}
