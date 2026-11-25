use super::constants::{SEQUENCE_FRAME_GEOM_PAD_PX, SEQUENCE_FRAME_SIDE_PAD_PX};
use crate::geom::Box2;
use crate::model::LayoutNode;

#[derive(Debug, Clone)]
pub(super) struct SequenceRectOpen {
    start_id: String,
    top_y: f64,
    bounds: Option<Box2>,
}

impl SequenceRectOpen {
    pub(super) fn new(start_id: String, top_y: f64) -> Self {
        Self {
            start_id,
            top_y,
            bounds: None,
        }
    }

    pub(super) fn include_min_max(&mut self, min_x: f64, max_x: f64, max_y: f64) {
        let r = Box2::from_min_max(min_x, self.top_y, max_x, max_y);
        if let Some(ref mut cur) = self.bounds {
            cur.union(r);
        } else {
            self.bounds = Some(r);
        }
    }

    pub(super) fn close(self, actor_center_bounds: (f64, f64)) -> ClosedSequenceRect {
        let rect_left = self
            .bounds
            .map(|b| b.min_x())
            .unwrap_or(actor_center_bounds.0 - SEQUENCE_FRAME_SIDE_PAD_PX);
        let rect_right = self
            .bounds
            .map(|b| b.max_x())
            .unwrap_or(actor_center_bounds.1 + SEQUENCE_FRAME_SIDE_PAD_PX);
        let rect_bottom = self
            .bounds
            .map(|b| b.max_y() + SEQUENCE_FRAME_GEOM_PAD_PX)
            .unwrap_or(self.top_y + SEQUENCE_FRAME_GEOM_PAD_PX);
        let rect_w = (rect_right - rect_left).max(1.0);
        let rect_h = (rect_bottom - self.top_y).max(1.0);

        ClosedSequenceRect {
            node: LayoutNode {
                id: format!("rect-{}", self.start_id),
                x: rect_left + rect_w / 2.0,
                y: self.top_y + rect_h / 2.0,
                width: rect_w,
                height: rect_h,
                is_cluster: false,
                label_width: None,
                label_height: None,
            },
            left: rect_left,
            right: rect_right,
            bottom: rect_bottom,
        }
    }
}

pub(super) struct ClosedSequenceRect {
    pub(super) node: LayoutNode,
    pub(super) left: f64,
    pub(super) right: f64,
    pub(super) bottom: f64,
}

#[cfg(test)]
mod tests {
    use super::{SEQUENCE_FRAME_SIDE_PAD_PX, SequenceRectOpen};

    #[test]
    fn empty_rect_uses_precomputed_actor_extrema() {
        let closed = SequenceRectOpen::new("empty".to_string(), 20.0).close((100.0, 300.0));

        assert_eq!(closed.left, 100.0 - SEQUENCE_FRAME_SIDE_PAD_PX);
        assert_eq!(closed.right, 300.0 + SEQUENCE_FRAME_SIDE_PAD_PX);
        assert_eq!(closed.node.width, 200.0 + 2.0 * SEQUENCE_FRAME_SIDE_PAD_PX);
    }
}
