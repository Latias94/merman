use super::super::RelationGraphBox;
use super::boxes::{
    LayeredRelationEdge, LayeredRelationError, LayeredRelationPlan, PlacedRelationGraphBox,
    plan_layered_relation_boxes,
};
use super::lanes::parallel_relation_lane_offsets;
use super::route::{
    LayeredRelationRouteGeometry, LayeredRelationRouteRequest, LayeredRelationRouteStyle,
    RelationOverlay, draw_layered_relation_route,
};
use crate::Result;
use crate::canvas::Canvas;
use crate::terminal::CanvasStyle;

#[derive(Debug, Clone)]
pub(crate) struct LayeredRelationScene<'boxes> {
    plan: LayeredRelationPlan<'boxes>,
    edges: Vec<LayeredRelationEdge>,
    draw_order: Vec<(usize, isize)>,
}

#[derive(Debug, Clone)]
pub(crate) enum LayeredRelationScenePlan<'boxes> {
    Routed(LayeredRelationScene<'boxes>),
    Summary(LayeredRelationSummaryReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LayeredRelationSummaryReason {
    Crossing,
    BoxOverlap,
    GridBudget { actual: usize, limit: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LayeredRelationBoxSnapshot {
    cells: Vec<LayeredRelationBoxSnapshotCell>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LayeredRelationBoxSnapshotCell {
    x: usize,
    y: usize,
    ch: Option<char>,
    style: Option<CanvasStyle>,
}

impl LayeredRelationBoxSnapshot {
    fn matches(&self, canvas: &Canvas) -> bool {
        self.cells.iter().all(|cell| {
            canvas.get(cell.x, cell.y) == cell.ch && canvas.get_style(cell.x, cell.y) == cell.style
        })
    }
}

impl<'boxes> LayeredRelationScene<'boxes> {
    pub(crate) fn new(
        boxes: &'boxes [RelationGraphBox],
        edges: Vec<LayeredRelationEdge>,
        horizontal_gap: usize,
    ) -> std::result::Result<Self, LayeredRelationError> {
        let plan = plan_layered_relation_boxes(boxes, &edges, horizontal_gap)?;
        let lane_offsets = parallel_relation_lane_offsets(
            edges
                .iter()
                .map(|edge| (edge.source_id(), edge.target_id())),
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

    pub(crate) fn capture_box_snapshot(&self, canvas: &Canvas) -> LayeredRelationBoxSnapshot {
        let mut cells = Vec::new();
        for placed_box in self.plan.placed_boxes() {
            for y in placed_box.y()..=placed_box.bottom() {
                for x in placed_box.x()..=placed_box.right() {
                    cells.push(LayeredRelationBoxSnapshotCell {
                        x,
                        y,
                        ch: canvas.get(x, y),
                        style: canvas.get_style(x, y),
                    });
                }
            }
        }

        LayeredRelationBoxSnapshot { cells }
    }

    pub(crate) fn box_snapshot_matches(
        &self,
        canvas: &Canvas,
        snapshot: &LayeredRelationBoxSnapshot,
    ) -> bool {
        snapshot.matches(canvas)
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
        build_overlays: impl FnOnce(&LayeredRelationRouteGeometry) -> Result<Vec<RelationOverlay>>,
    ) -> Result<()> {
        let Some((top, bottom)) = self.edge_endpoints(edge_index) else {
            return Ok(());
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
        )?;
        Ok(())
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
            .find(|placed_box| placed_box.id() == edge.source_id())?;
        let bottom = self
            .plan
            .placed_boxes()
            .iter()
            .find(|placed_box| placed_box.id() == edge.target_id())?;
        Some((top, bottom))
    }
}

pub(crate) fn plan_layered_relation_scene<'boxes>(
    boxes: &'boxes [RelationGraphBox],
    edges: Vec<LayeredRelationEdge>,
    horizontal_gap: usize,
    max_grid_cells: usize,
) -> std::result::Result<LayeredRelationScenePlan<'boxes>, LayeredRelationError> {
    let scene = match LayeredRelationScene::new(boxes, edges, horizontal_gap) {
        Ok(scene) => scene,
        Err(LayeredRelationError::Crossing) => {
            return Ok(LayeredRelationScenePlan::Summary(
                LayeredRelationSummaryReason::Crossing,
            ));
        }
        Err(error) => return Err(error),
    };

    let actual = scene.cell_count();
    if actual > max_grid_cells {
        return Ok(LayeredRelationScenePlan::Summary(
            LayeredRelationSummaryReason::GridBudget {
                actual,
                limit: max_grid_cells,
            },
        ));
    }

    Ok(LayeredRelationScenePlan::Routed(scene))
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

    #[test]
    fn layered_relation_scene_plan_routes_when_readable_and_within_budget() {
        let boxes = vec![
            RelationGraphBox::new("a".to_string(), vec!["A".to_string()], 1),
            RelationGraphBox::new("b".to_string(), vec!["B".to_string()], 1),
        ];
        let edges = vec![LayeredRelationEdge::new("a", "b", 0, 0)];

        let plan = plan_layered_relation_scene(&boxes, edges, 1, 100)
            .expect("readable relation should plan");

        assert!(matches!(plan, LayeredRelationScenePlan::Routed(_)));
    }

    #[test]
    fn layered_relation_scene_plan_uses_summary_for_crossing_layouts() {
        let boxes = vec![
            RelationGraphBox::new("a".to_string(), vec!["A".to_string()], 1),
            RelationGraphBox::new("b".to_string(), vec!["B".to_string()], 1),
            RelationGraphBox::new("c".to_string(), vec!["C".to_string()], 1),
        ];
        let edges = vec![
            LayeredRelationEdge::new("a", "b", 0, 0),
            LayeredRelationEdge::new("b", "a", 0, 0),
            LayeredRelationEdge::new("a", "c", 0, 0),
            LayeredRelationEdge::new("c", "a", 0, 0),
            LayeredRelationEdge::new("b", "c", 0, 0),
            LayeredRelationEdge::new("c", "b", 0, 0),
        ];

        let plan = plan_layered_relation_scene(&boxes, edges, 1, 100)
            .expect("crossing relation should be summarized");

        assert!(matches!(
            plan,
            LayeredRelationScenePlan::Summary(LayeredRelationSummaryReason::Crossing)
        ));
    }

    #[test]
    fn layered_relation_scene_plan_uses_summary_when_grid_budget_is_tight() {
        let boxes = vec![
            RelationGraphBox::new("a".to_string(), vec!["A".to_string()], 1),
            RelationGraphBox::new("b".to_string(), vec!["B".to_string()], 1),
        ];
        let edges = vec![LayeredRelationEdge::new("a", "b", 0, 0)];

        let plan = plan_layered_relation_scene(&boxes, edges, 1, 1)
            .expect("oversized relation should be summarized");

        assert!(matches!(
            plan,
            LayeredRelationScenePlan::Summary(LayeredRelationSummaryReason::GridBudget {
                actual: 5,
                limit: 1
            })
        ));
    }
}
