use super::super::RelationGraphBox;
use super::boxes::{
    LayeredRelationEdge, LayeredRelationError, LayeredRelationPlan, PlacedRelationGraphBox,
    plan_layered_relation_boxes,
};
use super::lanes::parallel_relation_lane_offsets;
use super::route::{
    LayeredRelationRouteGeometry, LayeredRelationRoutePlan, LayeredRelationRouteRequest,
    LayeredRelationRouteStyle, RelationOverlay, plan_layered_relation_route_draw,
};
use crate::Result;
use crate::canvas::Canvas;
use crate::options::{AsciiRenderOptions, TerminalWidthProfile};
use crate::safe_text::SafeLine;
use crate::terminal::CanvasStyle;

#[derive(Debug, Clone)]
pub(crate) struct LayeredRelationScene<'boxes> {
    plan: LayeredRelationPlan<'boxes>,
    edges: Vec<LayeredRelationEdge>,
    draw_order: Vec<(usize, isize)>,
    width_profile: TerminalWidthProfile,
}

#[derive(Debug, Clone)]
pub(crate) enum LayeredRelationScenePlan<'boxes> {
    Routed(LayeredRelationScene<'boxes>),
    Summary(LayeredRelationSummaryReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LayeredRelationSummaryReason {
    Crossing,
    RouteCollision,
    OverlayCollision,
    GridBudget { actual: usize, limit: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LayeredRelationBoxSnapshot {
    rows: Vec<LayeredRelationBoxSnapshotRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LayeredRelationBoxSnapshotRow {
    x: usize,
    y: usize,
    width: usize,
    chars: Vec<Option<char>>,
    needs_resolved_text: bool,
    text: Option<String>,
    styles: Vec<Option<CanvasStyle>>,
}

impl LayeredRelationBoxSnapshot {
    fn matches(&self, canvas: &Canvas, width_profile: TerminalWidthProfile) -> bool {
        if !self.rows.iter().all(|row| {
            row.chars.iter().zip(&row.styles).enumerate().all(
                |(offset, (expected_char, expected_style))| {
                    let x = row.x.saturating_add(offset);
                    canvas.get(x, row.y) == *expected_char
                        && canvas.get_style(x, row.y) == *expected_style
                },
            )
        }) {
            return false;
        }

        if !self.rows.iter().any(|row| row.needs_resolved_text) {
            return true;
        }

        let rendered_rows = rendered_plain_rows(canvas, width_profile);
        self.rows
            .iter()
            .filter(|row| row.needs_resolved_text)
            .all(|row| {
                rendered_rows
                    .get(row.y)
                    .and_then(|line| display_column_range(line, row.x, row.width, width_profile))
                    == row.text
            })
    }
}

impl<'boxes> LayeredRelationScene<'boxes> {
    pub(crate) fn new(
        boxes: &'boxes [RelationGraphBox],
        edges: Vec<LayeredRelationEdge>,
        horizontal_gap: usize,
        width_profile: TerminalWidthProfile,
    ) -> std::result::Result<Self, LayeredRelationError> {
        debug_assert!(
            boxes
                .iter()
                .all(|relation_box| relation_box.width_profile() == width_profile),
            "layered relation boxes must match the requested terminal width profile"
        );
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
            width_profile,
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
        let mut canvas =
            Canvas::with_width_profile(self.width(), self.height(), self.width_profile);
        for placed_box in self.plan.placed_boxes() {
            placed_box.draw_at(&mut canvas);
        }
        canvas
    }

    pub(crate) fn capture_box_snapshot(&self, canvas: &Canvas) -> LayeredRelationBoxSnapshot {
        let mut rows = Vec::new();
        for placed_box in self.plan.placed_boxes() {
            for y in placed_box.y()..=placed_box.bottom() {
                let x = placed_box.x();
                let width = placed_box.right().saturating_sub(x).saturating_add(1);
                let chars = (x..x.saturating_add(width))
                    .map(|cell_x| canvas.get(cell_x, y))
                    .collect::<Vec<_>>();
                let needs_resolved_text = chars.iter().any(Option::is_none);
                let styles = (x..x.saturating_add(width))
                    .map(|cell_x| canvas.get_style(cell_x, y))
                    .collect();
                rows.push(LayeredRelationBoxSnapshotRow {
                    x,
                    y,
                    width,
                    chars,
                    needs_resolved_text,
                    text: None,
                    styles,
                });
            }
        }

        if rows.iter().any(|row| row.needs_resolved_text) {
            let rendered_rows = rendered_plain_rows(canvas, self.width_profile);
            for row in rows.iter_mut().filter(|row| row.needs_resolved_text) {
                row.text = rendered_rows.get(row.y).and_then(|line| {
                    display_column_range(line, row.x, row.width, self.width_profile)
                });
            }
        }

        LayeredRelationBoxSnapshot { rows }
    }

    pub(crate) fn box_snapshot_matches(
        &self,
        canvas: &Canvas,
        snapshot: &LayeredRelationBoxSnapshot,
    ) -> bool {
        snapshot.matches(canvas, self.width_profile)
    }

    pub(crate) fn draw_order(&self) -> &[(usize, isize)] {
        &self.draw_order
    }

    pub(crate) fn plan_edge_draw(
        &self,
        edge_index: usize,
        lane_offset: isize,
        style: LayeredRelationRouteStyle,
        build_overlays: impl FnOnce(&LayeredRelationRouteGeometry) -> Result<Vec<RelationOverlay>>,
    ) -> Result<Option<LayeredRelationRoutePlan>> {
        let Some((top, bottom)) = self.edge_endpoints(edge_index) else {
            return Ok(None);
        };
        plan_layered_relation_route_draw(
            LayeredRelationRouteRequest::new(
                self.plan.placed_boxes(),
                top,
                bottom,
                lane_offset,
                style.profile(),
            ),
            style,
            build_overlays,
        )
        .map(Some)
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
    width_profile: TerminalWidthProfile,
) -> std::result::Result<LayeredRelationScenePlan<'boxes>, LayeredRelationError> {
    let scene = match LayeredRelationScene::new(boxes, edges, horizontal_gap, width_profile) {
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

fn rendered_plain_rows(canvas: &Canvas, width_profile: TerminalWidthProfile) -> Vec<String> {
    let options = AsciiRenderOptions::ascii().with_terminal_width_profile(width_profile);
    canvas
        .clone()
        .finish_with_options(&options)
        .lines()
        .map(ToOwned::to_owned)
        .collect()
}

fn display_column_range(
    line: &str,
    start: usize,
    width: usize,
    width_profile: TerminalWidthProfile,
) -> Option<String> {
    let end = start.checked_add(width)?;
    let mut column = 0usize;
    let mut output = String::new();

    for grapheme in SafeLine::new(line).graphemes(width_profile) {
        let grapheme_end = column.checked_add(grapheme.width())?;
        if grapheme_end <= start {
            column = grapheme_end;
            continue;
        }
        if column >= end {
            break;
        }
        if column < start || grapheme_end > end {
            return None;
        }
        output.push_str(grapheme.text());
        column = grapheme_end;
    }

    (column >= end).then_some(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relation_graph::RelationGraphLine;

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
        let scene = LayeredRelationScene::new(&boxes, edges, 1, TerminalWidthProfile::Unicode)
            .expect("scene should be buildable");

        assert_eq!(scene.draw_order(), &[(1, 0), (2, 0), (0, -6), (3, 6)]);
    }

    #[test]
    fn layered_relation_scene_plan_routes_when_readable_and_within_budget() {
        let boxes = vec![
            RelationGraphBox::new("a".to_string(), vec!["A".to_string()], 1),
            RelationGraphBox::new("b".to_string(), vec!["B".to_string()], 1),
        ];
        let edges = vec![LayeredRelationEdge::new("a", "b", 0, 0)];

        let plan =
            plan_layered_relation_scene(&boxes, edges, 1, 100, TerminalWidthProfile::Unicode)
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

        let plan =
            plan_layered_relation_scene(&boxes, edges, 1, 100, TerminalWidthProfile::Unicode)
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

        let plan = plan_layered_relation_scene(&boxes, edges, 1, 1, TerminalWidthProfile::Unicode)
            .expect("oversized relation should be summarized");

        assert!(matches!(
            plan,
            LayeredRelationScenePlan::Summary(LayeredRelationSummaryReason::GridBudget {
                actual: 5,
                limit: 1
            })
        ));
    }

    #[test]
    fn layered_relation_snapshot_compares_complete_arena_graphemes() {
        let width_profile = TerminalWidthProfile::Unicode;
        let boxes = vec![
            RelationGraphBox::new_with_lines(
                "a".to_string(),
                vec![RelationGraphLine::plain("👩‍💻".to_string(), width_profile)],
                2,
                width_profile,
            ),
            RelationGraphBox::new_with_lines(
                "b".to_string(),
                vec![RelationGraphLine::plain("B ".to_string(), width_profile)],
                2,
                width_profile,
            ),
        ];
        let edges = vec![LayeredRelationEdge::new("a", "b", 0, 0)];
        let scene = LayeredRelationScene::new(&boxes, edges, 1, width_profile)
            .expect("scene should be buildable");
        let mut canvas = scene.canvas_with_boxes();
        let snapshot = scene.capture_box_snapshot(&canvas);

        assert!(scene.box_snapshot_matches(&canvas, &snapshot));

        let first_box = &scene.plan.placed_boxes()[0];
        canvas.write_text(first_box.x(), first_box.y(), "👩‍🔬");

        assert!(!scene.box_snapshot_matches(&canvas, &snapshot));
    }
}
