use super::super::RelationGraphBox;
use super::boxes::{
    LayeredRelationEdge, LayeredRelationError, LayeredRelationPlan, PlacedRelationGraphBox,
    plan_layered_relation_boxes,
};
use super::route::{
    LayeredRelationRouteGeometry, LayeredRelationRouteRequest, LayeredRelationRouteStyle,
    RelationOverlay, draw_layered_relation_route, parallel_relation_lane_offsets,
};
use crate::canvas::Canvas;

#[derive(Debug, Clone)]
pub(crate) struct LayeredRelationScene<'boxes, 'edges> {
    plan: LayeredRelationPlan<'boxes>,
    edges: Vec<LayeredRelationEdge<'edges>>,
    draw_order: Vec<(usize, isize)>,
}

impl<'boxes, 'edges> LayeredRelationScene<'boxes, 'edges> {
    pub(crate) fn new(
        boxes: &'boxes [RelationGraphBox],
        edges: Vec<LayeredRelationEdge<'edges>>,
        horizontal_gap: usize,
    ) -> std::result::Result<Self, LayeredRelationError> {
        let plan = plan_layered_relation_boxes(boxes, &edges, horizontal_gap)?;
        let lane_offsets = parallel_relation_lane_offsets(
            edges.iter().map(|edge| (edge.top_id(), edge.bottom_id())),
        );
        let mut draw_order = lane_offsets.into_iter().enumerate().collect::<Vec<_>>();
        draw_order.sort_by_key(|(index, lane_offset)| (lane_offset.unsigned_abs(), *index));

        Ok(Self {
            plan,
            edges,
            draw_order,
        })
    }

    pub(crate) fn width(&self) -> usize {
        self.plan.width()
    }

    pub(crate) fn height(&self) -> usize {
        self.plan.height()
    }

    pub(crate) fn cell_count(&self) -> usize {
        self.width().saturating_mul(self.height())
    }

    pub(crate) fn canvas_with_boxes(&self) -> Canvas {
        let mut canvas = Canvas::new(self.width(), self.height());
        for placed_box in self.plan.placed_boxes() {
            placed_box.draw_at(&mut canvas);
        }
        canvas
    }

    pub(crate) fn draw_order(&self) -> &[(usize, isize)] {
        &self.draw_order
    }

    pub(crate) fn draw_edge(
        &self,
        canvas: &mut Canvas,
        edge_index: usize,
        lane_offset: isize,
        style: LayeredRelationRouteStyle,
        build_overlays: impl FnOnce(&LayeredRelationRouteGeometry) -> Vec<RelationOverlay>,
    ) {
        let Some((top, bottom)) = self.edge_endpoints(edge_index) else {
            return;
        };
        draw_layered_relation_route(
            canvas,
            LayeredRelationRouteRequest::new(
                self.plan.placed_boxes(),
                top,
                bottom,
                lane_offset,
                style.profile(),
            ),
            style,
            build_overlays,
        );
    }

    fn edge_endpoints(
        &self,
        edge_index: usize,
    ) -> Option<(
        &PlacedRelationGraphBox<'boxes>,
        &PlacedRelationGraphBox<'boxes>,
    )> {
        let edge = self.edges.get(edge_index)?;
        let top = self
            .plan
            .placed_boxes()
            .iter()
            .find(|placed_box| placed_box.id() == edge.top_id())?;
        let bottom = self
            .plan
            .placed_boxes()
            .iter()
            .find(|placed_box| placed_box.id() == edge.bottom_id())?;
        Some((top, bottom))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layered_relation_scene_orders_parallel_edges_by_lane_distance() {
        let boxes = vec![
            RelationGraphBox::new("a".to_string(), vec!["A".to_string()], 1),
            RelationGraphBox::new("b".to_string(), vec!["B".to_string()], 1),
            RelationGraphBox::new("c".to_string(), vec!["C".to_string()], 1),
        ];
        let edges = vec![
            LayeredRelationEdge::new("a", "b", 0, 0),
            LayeredRelationEdge::new("a", "b", 0, 0),
            LayeredRelationEdge::new("a", "c", 0, 0),
            LayeredRelationEdge::new("a", "b", 0, 0),
        ];
        let scene = LayeredRelationScene::new(&boxes, edges, 1).expect("scene should be buildable");

        assert_eq!(scene.draw_order(), &[(1, 0), (2, 0), (0, -6), (3, 6)]);
    }
}
