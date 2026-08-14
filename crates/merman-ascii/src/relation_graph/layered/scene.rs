use super::super::RelationGraphBox;
use super::boxes::{
    LayeredRelationEdge, LayeredRelationError, LayeredRelationLayoutKind, LayeredRelationPlan,
    LayeredRelationPlanningError, PlacedRelationGraphBox, plan_layered_relation_boxes,
};
use super::lanes::parallel_relation_lane_offsets;
use super::route::{
    LayeredRelationRouteGeometry, LayeredRelationRoutePlan, LayeredRelationRouteRequest,
    plan_layered_relation_route_geometry, plan_planar_k2_2_relation_route_geometry,
};
use crate::canvas::Canvas;
use crate::options::{AsciiRenderOptions, TerminalWidthProfile};
#[cfg(test)]
use crate::resource::AsciiResourceLimitId;
use crate::resource::{AsciiResourceLimitPhase, ResourceContext};
#[cfg(test)]
use crate::terminal::{CanvasStyle, TerminalCellText};
use crate::{AsciiError, Result};

#[derive(Debug)]
pub(crate) struct LayeredRelationScene<'boxes> {
    plan: LayeredRelationPlan<'boxes>,
    edges: Vec<LayeredRelationEdge>,
    draw_order: Vec<(usize, isize)>,
}

#[derive(Debug)]
pub(crate) enum LayeredRelationScenePlan<'boxes> {
    Routed(LayeredRelationScene<'boxes>),
    Summary(LayeredRelationSummaryReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LayeredRelationSummaryReason {
    Crossing,
    RouteCollision,
    OverlayCollision,
}

#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct LayeredRelationBoxSnapshot {
    rows: Vec<LayeredRelationBoxSnapshotRow>,
}

#[cfg(test)]
#[derive(Debug, PartialEq, Eq)]
struct LayeredRelationBoxSnapshotRow {
    x: usize,
    y: usize,
    width: usize,
    chars: Vec<Option<char>>,
    needs_resolved_text: bool,
    text: Option<String>,
    styles: Vec<Option<CanvasStyle>>,
}

#[cfg(test)]
impl LayeredRelationBoxSnapshot {
    fn matches(&self, canvas: &Canvas, resources: &mut ResourceContext) -> Result<bool> {
        resources.charge_layout_work(
            self.rows
                .iter()
                .try_fold(0usize, |total, row| total.checked_add(row.width))
                .ok_or_else(|| {
                    resources
                        .policy()
                        .overflow(crate::resource::AsciiResourceLimitId::MaxLayoutWorkUnits)
                })?,
        )?;
        for row in &self.rows {
            for (offset, (expected_char, expected_style)) in
                row.chars.iter().zip(&row.styles).enumerate()
            {
                let x = row.x.checked_add(offset).ok_or_else(|| {
                    resources
                        .policy()
                        .overflow(AsciiResourceLimitId::MaxGridCells)
                })?;
                if canvas.get(x, row.y) != *expected_char
                    || canvas.get_style(x, row.y) != *expected_style
                {
                    return Ok(false);
                }
            }
        }

        if !self.rows.iter().any(|row| row.needs_resolved_text) {
            return Ok(true);
        }

        for row in self.rows.iter().filter(|row| row.needs_resolved_text) {
            if !plain_display_range_matches(canvas, row, resources)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

impl<'boxes> LayeredRelationScene<'boxes> {
    pub(crate) fn new(
        boxes: &[&'boxes RelationGraphBox],
        edges: Vec<LayeredRelationEdge>,
        horizontal_gap: usize,
        width_profile: TerminalWidthProfile,
        resources: &mut ResourceContext,
    ) -> std::result::Result<Self, LayeredRelationPlanningError> {
        debug_assert!(
            boxes
                .iter()
                .all(|relation_box| relation_box.width_profile() == width_profile),
            "layered relation boxes must match the requested terminal width profile"
        );
        let plan = plan_layered_relation_boxes(boxes, &edges, horizontal_gap, resources)?;
        let lane_offsets = parallel_relation_lane_offsets(
            edges
                .iter()
                .map(|edge| (edge.source_id(), edge.target_id())),
            resources,
        )?;
        resources.charge_layout_work(lane_offsets.len().max(1))?;
        let mut draw_order = Vec::new();
        draw_order
            .try_reserve_exact(lane_offsets.len())
            .map_err(|_| snapshot_allocation_failed())?;
        draw_order.extend(lane_offsets.into_iter().enumerate());
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

    pub(crate) fn edge_ports_fit(
        &self,
        edge_index: usize,
        geometry: &LayeredRelationRouteGeometry,
    ) -> bool {
        let Some(edge) = self.edges.get(edge_index) else {
            return false;
        };
        let Some(source) = self
            .plan
            .placed_boxes()
            .iter()
            .find(|placed| placed.id() == edge.source_id())
        else {
            return false;
        };
        let Some(target) = self
            .plan
            .placed_boxes()
            .iter()
            .find(|placed| placed.id() == edge.target_id())
        else {
            return false;
        };
        geometry.source_port().fits_box(source) && geometry.target_port().fits_box(target)
    }

    pub(crate) fn canvas_with_boxes(
        &self,
        options: &AsciiRenderOptions,
        resources: &ResourceContext,
    ) -> Result<Canvas> {
        let mut canvas = Canvas::try_with_resources(
            self.width(),
            self.height(),
            options.terminal_width_profile,
            resources,
        )?;
        for placed_box in self.plan.placed_boxes() {
            placed_box.draw_at(&mut canvas, resources)?;
        }
        Ok(canvas)
    }

    #[cfg(test)]
    pub(crate) fn capture_box_snapshot(
        &self,
        canvas: &Canvas,
        resources: &mut ResourceContext,
    ) -> Result<LayeredRelationBoxSnapshot> {
        let row_count = self
            .plan
            .placed_boxes()
            .iter()
            .try_fold(0usize, |total, placed_box| {
                total.checked_add(placed_box.height()).ok_or_else(|| {
                    resources
                        .policy()
                        .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
                })
            })?;
        let snapshot_cells =
            self.plan
                .placed_boxes()
                .iter()
                .try_fold(0usize, |total, placed_box| {
                    let cells = placed_box
                        .width()
                        .checked_mul(placed_box.height())
                        .ok_or_else(|| {
                            resources
                                .policy()
                                .overflow(AsciiResourceLimitId::MaxGridCells)
                        })?;
                    total.checked_add(cells).ok_or_else(|| {
                        resources
                            .policy()
                            .overflow(AsciiResourceLimitId::MaxGridCells)
                    })
                })?;
        let canvas_cells = resources.checked_grid_mul(self.width(), self.height())?;
        let snapshot_concurrent_cells = resources.checked_grid_add(canvas_cells, snapshot_cells)?;
        resources.grid_extent(snapshot_concurrent_cells, 1)?;
        resources.charge_document_cells(snapshot_cells)?;
        let snapshot_work = snapshot_cells.checked_add(row_count).ok_or_else(|| {
            resources
                .policy()
                .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
        })?;
        resources.charge_layout_work(snapshot_work.max(1))?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(row_count)
            .map_err(|_| snapshot_allocation_failed())?;
        for placed_box in self.plan.placed_boxes() {
            for y in placed_box.y()..=placed_box.bottom() {
                let x = placed_box.x();
                let width = placed_box
                    .right()
                    .checked_sub(x)
                    .and_then(|value| value.checked_add(1))
                    .ok_or_else(|| {
                        resources
                            .policy()
                            .overflow(crate::resource::AsciiResourceLimitId::MaxGridCells)
                    })?;
                let end = resources.checked_grid_add(x, width)?;
                let mut chars = Vec::new();
                chars
                    .try_reserve_exact(width)
                    .map_err(|_| snapshot_allocation_failed())?;
                chars.extend((x..end).map(|cell_x| canvas.get(cell_x, y)));
                let needs_resolved_text = chars.iter().any(Option::is_none);
                let mut styles = Vec::new();
                styles
                    .try_reserve_exact(width)
                    .map_err(|_| snapshot_allocation_failed())?;
                styles.extend((x..end).map(|cell_x| canvas.get_style(cell_x, y)));
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

        let resolved_cells =
            rows.iter()
                .filter(|row| row.needs_resolved_text)
                .try_fold(0usize, |total, row| {
                    total.checked_add(row.width).ok_or_else(|| {
                        resources
                            .policy()
                            .overflow(AsciiResourceLimitId::MaxGridCells)
                    })
                })?;
        if resolved_cells > 0 {
            let concurrent_cells =
                resources.checked_grid_add(snapshot_concurrent_cells, resolved_cells)?;
            resources.grid_extent(concurrent_cells, 1)?;
            resources.charge_document_cells(resolved_cells)?;
            let resolved_work = resolved_cells.checked_mul(2).ok_or_else(|| {
                resources
                    .policy()
                    .overflow(AsciiResourceLimitId::MaxLayoutWorkUnits)
            })?;
            resources.charge_layout_work(resolved_work)?;
            for row in rows.iter_mut().filter(|row| row.needs_resolved_text) {
                row.text = materialize_plain_display_range(canvas, row, resources)?;
            }
        }

        Ok(LayeredRelationBoxSnapshot { rows })
    }

    #[cfg(test)]
    pub(crate) fn box_snapshot_matches(
        &self,
        canvas: &Canvas,
        snapshot: &LayeredRelationBoxSnapshot,
        resources: &mut ResourceContext,
    ) -> Result<bool> {
        snapshot.matches(canvas, resources)
    }

    pub(crate) fn draw_order(&self) -> &[(usize, isize)] {
        &self.draw_order
    }

    pub(crate) fn is_planar_k2_2(&self) -> bool {
        self.plan.layout_kind() == LayeredRelationLayoutKind::PlanarK2x2
    }

    pub(crate) fn placed_box_count(&self) -> usize {
        self.plan.placed_boxes().len()
    }

    #[cfg(test)]
    pub(crate) fn placed_boxes(&self) -> &[PlacedRelationGraphBox<'boxes>] {
        self.plan.placed_boxes()
    }

    pub(crate) fn route_overlaps_box(&self, route: &LayeredRelationRoutePlan) -> bool {
        self.route_geometry_overlaps_box(route.geometry())
    }

    pub(crate) fn route_geometry_overlaps_box(
        &self,
        geometry: &LayeredRelationRouteGeometry,
    ) -> bool {
        self.plan.placed_boxes().iter().any(|placed| {
            geometry.overlaps_rect(placed.x(), placed.y(), placed.right(), placed.bottom())
        })
    }

    pub(crate) fn overlays_overlap_box(&self, route: &LayeredRelationRoutePlan) -> bool {
        self.plan.placed_boxes().iter().any(|placed| {
            route.overlays_overlap_rect(placed.x(), placed.y(), placed.right(), placed.bottom())
        })
    }

    pub(crate) fn plan_edge_geometry(
        &self,
        edge_index: usize,
        lane_offset: isize,
        profile: super::route::LayeredRelationRouteProfile,
        resources: &ResourceContext,
    ) -> Result<Option<LayeredRelationRouteGeometry>> {
        let Some((source, target)) = self.edge_endpoints(edge_index) else {
            return Ok(None);
        };
        if self.is_planar_k2_2() {
            return plan_planar_k2_2_relation_route_geometry(
                source,
                target,
                self.height(),
                profile,
                resources,
            );
        }
        plan_layered_relation_route_geometry(
            LayeredRelationRouteRequest::new(
                self.plan.placed_boxes(),
                source,
                target,
                lane_offset,
                profile,
            ),
            resources,
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
        let source = self
            .plan
            .placed_boxes()
            .iter()
            .find(|placed_box| placed_box.id() == edge.source_id())?;
        let target = self
            .plan
            .placed_boxes()
            .iter()
            .find(|placed_box| placed_box.id() == edge.target_id())?;
        Some((source, target))
    }
}

pub(crate) fn plan_layered_relation_scene<'boxes>(
    boxes: &[&'boxes RelationGraphBox],
    edges: Vec<LayeredRelationEdge>,
    horizontal_gap: usize,
    width_profile: TerminalWidthProfile,
    resources: &mut ResourceContext,
) -> std::result::Result<LayeredRelationScenePlan<'boxes>, LayeredRelationPlanningError> {
    let scene =
        match LayeredRelationScene::new(boxes, edges, horizontal_gap, width_profile, resources) {
            Ok(scene) => scene,
            Err(LayeredRelationPlanningError::Semantic(LayeredRelationError::Crossing)) => {
                return Ok(LayeredRelationScenePlan::Summary(
                    LayeredRelationSummaryReason::Crossing,
                ));
            }
            Err(error) => return Err(error),
        };

    Ok(LayeredRelationScenePlan::Routed(scene))
}

#[cfg(test)]
fn materialize_plain_display_range(
    canvas: &Canvas,
    row: &LayeredRelationBoxSnapshotRow,
    resources: &ResourceContext,
) -> Result<Option<String>> {
    let policy = resources.policy();
    let mut byte_len = 0usize;
    let visited = canvas.visit_plain_row_display_range(row.x, row.y, row.width, |text| {
        let additional = match text {
            TerminalCellText::Scalar(ch) => ch.len_utf8(),
            TerminalCellText::Grapheme(grapheme) => grapheme.len(),
        };
        byte_len = byte_len
            .checked_add(additional)
            .ok_or_else(|| policy.overflow(AsciiResourceLimitId::MaxOutputBytes))?;
        Ok(true)
    })?;
    if visited != Some(true) {
        return Ok(None);
    }
    policy.check(AsciiResourceLimitId::MaxOutputBytes, byte_len)?;

    let mut output = String::new();
    output
        .try_reserve_exact(byte_len)
        .map_err(|_| snapshot_allocation_failed())?;
    let visited = canvas.visit_plain_row_display_range(row.x, row.y, row.width, |text| {
        match text {
            TerminalCellText::Scalar(ch) => output.push(ch),
            TerminalCellText::Grapheme(grapheme) => output.push_str(grapheme),
        }
        Ok(true)
    })?;
    Ok((visited == Some(true)).then_some(output))
}

fn snapshot_allocation_failed() -> AsciiError {
    AsciiError::AllocationFailed {
        phase: AsciiResourceLimitPhase::LayoutWork.as_str(),
    }
}

#[cfg(test)]
fn plain_display_range_matches(
    canvas: &Canvas,
    row: &LayeredRelationBoxSnapshotRow,
    resources: &mut ResourceContext,
) -> Result<bool> {
    resources.charge_layout_work(row.width.max(1))?;
    let policy = resources.policy();
    let Some(expected) = row.text.as_deref() else {
        return Ok(canvas
            .visit_plain_row_display_range(row.x, row.y, row.width, |_| Ok(true))?
            .is_none());
    };

    let mut matched_bytes = 0usize;
    let visited = canvas.visit_plain_row_display_range(row.x, row.y, row.width, |text| {
        let matches = match text {
            TerminalCellText::Scalar(ch) => {
                let mut encoded = [0u8; 4];
                let fragment: &str = ch.encode_utf8(&mut encoded);
                expected
                    .get(matched_bytes..)
                    .is_some_and(|remaining| remaining.starts_with(fragment))
                    .then_some(fragment.len())
            }
            TerminalCellText::Grapheme(grapheme) => expected
                .get(matched_bytes..)
                .is_some_and(|remaining| remaining.starts_with(grapheme))
                .then_some(grapheme.len()),
        };
        let Some(fragment_len) = matches else {
            return Ok(false);
        };
        matched_bytes = matched_bytes
            .checked_add(fragment_len)
            .ok_or_else(|| policy.overflow(AsciiResourceLimitId::MaxOutputBytes))?;
        Ok(true)
    })?;

    Ok(visited == Some(true) && matched_bytes == expected.len())
}

#[cfg(test)]
mod tests {
    use super::super::draw::RelationLineChars;
    use super::super::route::LayeredRelationRouteProfile;
    use super::*;
    use crate::relation_graph::RelationGraphLine;
    use crate::{AsciiError, AsciiResourceLimitId, AsciiResourcePolicy};

    fn test_resources(policy: AsciiResourcePolicy) -> ResourceContext {
        ResourceContext::new(policy)
    }

    fn relation_box_refs(boxes: &[RelationGraphBox]) -> Vec<&RelationGraphBox> {
        boxes.iter().collect()
    }

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
        let mut resources = test_resources(AsciiResourcePolicy::default());
        let box_refs = relation_box_refs(&boxes);
        let scene = LayeredRelationScene::new(
            &box_refs,
            edges,
            1,
            TerminalWidthProfile::Unicode,
            &mut resources,
        )
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

        let mut resources = test_resources(AsciiResourcePolicy::default());
        let box_refs = relation_box_refs(&boxes);
        let plan = plan_layered_relation_scene(
            &box_refs,
            edges,
            1,
            TerminalWidthProfile::Unicode,
            &mut resources,
        )
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

        let mut resources = test_resources(AsciiResourcePolicy::default());
        let box_refs = relation_box_refs(&boxes);
        let plan = plan_layered_relation_scene(
            &box_refs,
            edges,
            1,
            TerminalWidthProfile::Unicode,
            &mut resources,
        )
        .expect("crossing relation should be summarized");

        assert!(matches!(
            plan,
            LayeredRelationScenePlan::Summary(LayeredRelationSummaryReason::Crossing)
        ));
    }

    #[test]
    fn layered_relation_scene_plan_propagates_grid_resource_errors() {
        let boxes = vec![
            RelationGraphBox::new("a".to_string(), vec!["A".to_string()], 1),
            RelationGraphBox::new("b".to_string(), vec!["B".to_string()], 1),
        ];
        let edges = vec![LayeredRelationEdge::new("a", "b", 0, 0)];

        let policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGridCells, 1)
            .expect("test resource limit should be valid");
        let mut resources = test_resources(policy);
        let box_refs = relation_box_refs(&boxes);
        let error = plan_layered_relation_scene(
            &box_refs,
            edges,
            1,
            TerminalWidthProfile::Unicode,
            &mut resources,
        )
        .expect_err("oversized relation must fail with a resource error");

        assert!(matches!(
            error,
            LayeredRelationPlanningError::Resource(AsciiError::ResourceLimitExceeded(details))
                if details.limit == AsciiResourceLimitId::MaxGridCells
        ));
    }

    #[test]
    fn routed_relation_snapshot_preserves_zwj_and_combining_graphemes() {
        let width_profile = TerminalWidthProfile::Unicode;
        let complex_text = "Cafe\u{301} 👩‍💻";
        let boxes = vec![
            RelationGraphBox::new_with_lines(
                "a".to_string(),
                vec![RelationGraphLine::plain(
                    complex_text.to_string(),
                    width_profile,
                )],
                7,
                width_profile,
            ),
            RelationGraphBox::new_with_lines(
                "b".to_string(),
                vec![RelationGraphLine::plain(
                    "B      ".to_string(),
                    width_profile,
                )],
                7,
                width_profile,
            ),
        ];
        let edges = vec![LayeredRelationEdge::new("a", "b", 0, 0)];
        let options = AsciiRenderOptions::ascii();
        let mut resources = test_resources(AsciiResourcePolicy::default());
        let box_refs = relation_box_refs(&boxes);
        let scene = LayeredRelationScene::new(&box_refs, edges, 1, width_profile, &mut resources)
            .expect("scene should be buildable");
        let mut canvas = scene
            .canvas_with_boxes(&options, &resources)
            .expect("scene canvas should allocate");
        let snapshot = scene
            .capture_box_snapshot(&canvas, &mut resources)
            .expect("snapshot should fit");

        let route = scene
            .plan_edge_geometry(0, 0, LayeredRelationRouteProfile::class(), &resources)
            .expect("route planning should fit")
            .expect("edge endpoints should exist");
        let route = LayeredRelationRoutePlan::new(
            route,
            '|',
            '-',
            RelationLineChars::new(['-', '|', '.', ':'], '+'),
            Vec::new(),
        );
        route
            .draw_route_at(&mut canvas)
            .expect("route glyphs should fit the test canvas");

        assert!(
            scene
                .box_snapshot_matches(&canvas, &snapshot, &mut resources)
                .expect("snapshot comparison should fit")
        );

        let first_box = &scene.plan.placed_boxes()[0];
        canvas.write_text(first_box.x(), first_box.y(), "Cafe\u{300} 👩‍💻");

        assert!(
            !scene
                .box_snapshot_matches(&canvas, &snapshot, &mut resources)
                .expect("snapshot comparison should fit")
        );
    }

    #[test]
    fn layered_relation_snapshot_checks_exact_grid_and_document_surfaces() {
        let width_profile = TerminalWidthProfile::Unicode;
        let boxes = vec![
            RelationGraphBox::new_with_lines(
                "a".to_string(),
                vec![RelationGraphLine::plain(
                    "Cafe\u{301} 👩‍💻".to_string(),
                    width_profile,
                )],
                7,
                width_profile,
            ),
            RelationGraphBox::new_with_lines(
                "b".to_string(),
                vec![RelationGraphLine::plain(
                    "B      ".to_string(),
                    width_profile,
                )],
                7,
                width_profile,
            ),
        ];
        let options = AsciiRenderOptions::ascii();
        let mut planning_resources = test_resources(AsciiResourcePolicy::default());
        let box_refs = relation_box_refs(&boxes);
        let scene = LayeredRelationScene::new(
            &box_refs,
            vec![LayeredRelationEdge::new("a", "b", 0, 0)],
            1,
            width_profile,
            &mut planning_resources,
        )
        .expect("scene should be buildable");
        let canvas = scene
            .canvas_with_boxes(&options, &planning_resources)
            .expect("scene canvas should allocate");
        let snapshot_cells = scene
            .plan
            .placed_boxes()
            .iter()
            .map(|placed_box| placed_box.width() * placed_box.height())
            .sum::<usize>();
        let resolved_cells = boxes[0].width();
        let concurrent_grid_cells =
            scene.width() * scene.height() + snapshot_cells + resolved_cells;
        let document_cells = snapshot_cells + resolved_cells;

        let exact_policy = AsciiResourcePolicy::default()
            .with_limit(AsciiResourceLimitId::MaxGridCells, concurrent_grid_cells)
            .expect("exact grid limit should be valid")
            .with_limit(AsciiResourceLimitId::MaxDocumentCells, document_cells)
            .expect("exact document limit should be valid");
        scene
            .capture_box_snapshot(&canvas, &mut test_resources(exact_policy))
            .expect("exact concurrent surfaces should fit");

        let below_grid = exact_policy
            .with_limit(
                AsciiResourceLimitId::MaxGridCells,
                concurrent_grid_cells - 1,
            )
            .expect("below-grid limit should be valid");
        let error = scene
            .capture_box_snapshot(&canvas, &mut test_resources(below_grid))
            .expect_err("N-1 concurrent grid cells must fail");
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxGridCells
                    && details.actual == concurrent_grid_cells
                    && details.max == concurrent_grid_cells - 1
        ));

        let below_document = exact_policy
            .with_limit(AsciiResourceLimitId::MaxDocumentCells, document_cells - 1)
            .expect("below-document limit should be valid");
        let error = scene
            .capture_box_snapshot(&canvas, &mut test_resources(below_document))
            .expect_err("N-1 snapshot document cells must fail");
        assert!(matches!(
            error,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxDocumentCells
                    && details.actual == document_cells
                    && details.max == document_cells - 1
        ));
    }
}
