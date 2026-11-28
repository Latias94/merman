use super::super::config::ROUTE_TOUCH_TOLERANCE;
use super::super::working::{WorkingEdge, WorkingLayout, WorkingNode, WorkingNodeKind};
use super::LayoutWorkBudget;
use super::geometry::{
    EPSILON, OrthogonalSegment, RectBounds, RectEntry, RectSide, collect_node_rect_entries,
    dedupe_consecutive_points, orthogonal_bend_points, orthogonal_segments_for_points,
    orthogonal_segments_strictly_cross, rect_of_node_bounds, same_axis_segment_overlap_length,
    same_point, segment_hits_any_rect,
};
use merman_render_core::{LayoutPoint, Result};

const ALIGNED_CENTER_TOLERANCE: f64 = 1.0;
const OBSTACLE_SHRINK: f64 = 2.0;

struct Face {
    bounds: RectBounds,
    side: RectSide,
}

impl Face {
    fn meets(&self, point: &LayoutPoint) -> bool {
        let rect = self.bounds;
        let (outward, along, lo, hi) = match self.side {
            RectSide::Top => (rect.top - point.y, point.x, rect.left, rect.right),
            RectSide::Bottom => (point.y - rect.bottom, point.x, rect.left, rect.right),
            RectSide::Left => (rect.left - point.x, point.y, rect.top, rect.bottom),
            RectSide::Right => (point.x - rect.right, point.y, rect.top, rect.bottom),
        };
        outward > -EPSILON && along >= lo - EPSILON && along <= hi + EPSILON
    }
}

struct StraightLine {
    from: Face,
    to: Face,
    points: [LayoutPoint; 2],
}

fn content_bounds(node: &WorkingNode) -> Option<RectBounds> {
    if node.kind != WorkingNodeKind::Content {
        return None;
    }
    rect_of_node_bounds(node).filter(|rect| {
        [rect.left, rect.right, rect.top, rect.bottom]
            .into_iter()
            .all(f64::is_finite)
    })
}

fn straight_line_for(layout: &WorkingLayout, edge: &WorkingEdge) -> Option<StraightLine> {
    let source = layout.nodes.get(&edge.from)?;
    let target = layout.nodes.get(&edge.to)?;
    if source.parent_id == target.parent_id {
        return None;
    }
    let src = content_bounds(source)?;
    let dst = content_bounds(target)?;
    let (from, to, points) = if (source.x - target.x).abs() <= ALIGNED_CENTER_TOLERANCE {
        let (from, to, start_y, end_y) = if dst.top > src.bottom {
            (RectSide::Bottom, RectSide::Top, src.bottom, dst.top)
        } else if dst.bottom < src.top {
            (RectSide::Top, RectSide::Bottom, src.top, dst.bottom)
        } else {
            return None;
        };
        (
            from,
            to,
            [
                LayoutPoint {
                    x: source.x,
                    y: start_y,
                },
                LayoutPoint {
                    x: source.x,
                    y: end_y,
                },
            ],
        )
    } else if (source.y - target.y).abs() <= ALIGNED_CENTER_TOLERANCE {
        let (from, to, start_x, end_x) = if dst.left > src.right {
            (RectSide::Right, RectSide::Left, src.right, dst.left)
        } else if dst.right < src.left {
            (RectSide::Left, RectSide::Right, src.left, dst.right)
        } else {
            return None;
        };
        (
            from,
            to,
            [
                LayoutPoint {
                    x: start_x,
                    y: source.y,
                },
                LayoutPoint {
                    x: end_x,
                    y: source.y,
                },
            ],
        )
    } else {
        return None;
    };
    Some(StraightLine {
        from: Face {
            bounds: src,
            side: from,
        },
        to: Face {
            bounds: dst,
            side: to,
        },
        points,
    })
}

fn face_is_own(
    face: &Face,
    node_id: &str,
    owner: usize,
    edges: &[WorkingEdge],
    work: &mut LayoutWorkBudget,
) -> Result<bool> {
    for (slot, other) in edges.iter().enumerate() {
        work.charge(1)?;
        if slot == owner {
            continue;
        }
        if (other.from == node_id && other.points.first().is_some_and(|point| face.meets(point)))
            || (other.to == node_id && other.points.last().is_some_and(|point| face.meets(point)))
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn clears_rects(
    moved: &OrthogonalSegment,
    rects: &[RectEntry],
    excluded: &[&str],
    work: &mut LayoutWorkBudget,
) -> Result<bool> {
    for rect in rects {
        work.charge(1)?;
        if segment_hits_any_rect(
            &moved.a,
            &moved.b,
            std::slice::from_ref(rect),
            excluded,
            OBSTACLE_SHRINK,
        ) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn crosses(first: &OrthogonalSegment, second: &OrthogonalSegment) -> bool {
    orthogonal_segments_strictly_cross(&first.a, &first.b, &second.a, &second.b, EPSILON)
}

fn keeps_crossings(
    current: &[OrthogonalSegment],
    other: &[OrthogonalSegment],
    mut moved_crossings: usize,
    work: &mut LayoutWorkBudget,
) -> Result<bool> {
    if moved_crossings == 0 {
        return Ok(true);
    }
    for first in current {
        for second in other {
            work.charge(1)?;
            if crosses(first, second) {
                moved_crossings -= 1;
                if moved_crossings == 0 {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

fn bends_touch_path(
    bending: &[OrthogonalSegment],
    path: &[OrthogonalSegment],
    work: &mut LayoutWorkBudget,
) -> Result<bool> {
    work.charge(bending.len().saturating_sub(1))?;
    for bend in orthogonal_bend_points(bending) {
        for segment in path {
            work.charge(1)?;
            if segment.distance_to_point(bend) < ROUTE_TOUCH_TOLERANCE {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn charge_geometry(work: &mut LayoutWorkBudget, ticks: usize) -> Result<()> {
    // Admit the rounded unit containing this allocation before materializing anything.
    work.preflight(ticks)?;
    work.charge(ticks)
}

fn clear_of_other_edges(
    owner: usize,
    current: &[LayoutPoint],
    moved: &OrthogonalSegment,
    edges: &[WorkingEdge],
    work: &mut LayoutWorkBudget,
) -> Result<bool> {
    charge_geometry(work, current.len())?;
    let current = orthogonal_segments_for_points(current, EPSILON);
    for (slot, other) in edges.iter().enumerate() {
        work.charge(1)?;
        if slot == owner || other.points.len() < 2 {
            continue;
        }
        charge_geometry(work, other.points.len().saturating_mul(2))?;
        let points = dedupe_consecutive_points(&other.points, EPSILON);
        let segments = orthogonal_segments_for_points(&points, EPSILON);
        let mut moved_crossings = 0;
        for other in &segments {
            work.charge(1)?;
            if same_axis_segment_overlap_length(moved, other, EPSILON) > EPSILON {
                return Ok(false);
            }
            moved_crossings += usize::from(crosses(moved, other));
        }
        if !keeps_crossings(&current, &segments, moved_crossings, work)? {
            return Ok(false);
        }
        // The candidate itself has no bend. An existing bend touch with this edge is allowed.
        if bends_touch_path(&segments, std::slice::from_ref(moved), work)?
            && !bends_touch_path(&current, &segments, work)?
            && !bends_touch_path(&segments, &current, work)?
        {
            return Ok(false);
        }
    }
    Ok(true)
}

/// The Mermaid 12.4 late pass sees painted occurrences and their latest routes in source order.
pub(super) fn straighten_aligned_cross_group_edges(
    layout: &mut WorkingLayout,
    work: &mut LayoutWorkBudget,
) -> Result<bool> {
    if layout.original_edges.is_empty() {
        return Ok(false);
    }
    charge_geometry(work, layout.nodes.len())?;
    let (nodes, labels) = collect_node_rect_entries(layout);
    let mut changed = false;
    for owner in 0..layout.original_edges.len() {
        work.charge(1)?;
        let edges = &layout.original_edges;
        let edge = &edges[owner];
        let Some(line) = straight_line_for(layout, edge) else {
            continue;
        };
        if edge.points.len() < 2 {
            continue;
        }
        charge_geometry(work, edge.points.len())?;
        let current = dedupe_consecutive_points(&edge.points, EPSILON);
        match current.as_slice() {
            [] | [_] => continue,
            [start, end]
                if same_point(start, &line.points[0], EPSILON)
                    && same_point(end, &line.points[1], EPSILON) =>
            {
                continue;
            }
            _ => {}
        }
        if !face_is_own(&line.from, &edge.from, owner, edges, work)?
            || !face_is_own(&line.to, &edge.to, owner, edges, work)?
        {
            continue;
        }
        charge_geometry(work, 1)?;
        let segments = orthogonal_segments_for_points(&line.points, EPSILON);
        if let Some(moved) = segments.first() {
            let own_label = edge.label_node_id.as_deref().filter(|id| !id.is_empty());
            if !clears_rects(moved, &nodes, &[&edge.from, &edge.to], work)?
                || !clears_rects(moved, &labels, own_label.as_slice(), work)?
                || !clear_of_other_edges(owner, &current, moved, edges, work)?
            {
                continue;
            }
        }
        charge_geometry(work, line.points.len())?;
        layout.original_edges[owner].points = Vec::from(line.points);
        changed = true;
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::super::{PostProcessMode, post_process};
    use super::*;
    use merman_model::{OperationControl, OperationPhase};
    use merman_render_core::resources::{
        OperationWorkMeter, RenderResourcePolicy, ResourceLimitId,
    };
    use merman_render_core::{Error, SwimlaneDirection};
    use std::sync::Arc;

    fn node(id: &str, parent: Option<&str>, x: f64, y: f64) -> WorkingNode {
        WorkingNode {
            id: id.to_owned(),
            label: id.to_owned(),
            label_type: "text".to_owned(),
            shape: "rect".to_owned(),
            producer_lane_role: None,
            kind: WorkingNodeKind::Content,
            parent_id: parent.map(str::to_owned),
            top_lane_id: None,
            requested_dir: None,
            padding: 0.0,
            x,
            y,
            width: 100.0,
            height: 100.0,
            label_width: 0.0,
            label_height: 0.0,
            layer: 0,
            order: 0,
            content_top: None,
            title_rect: None,
        }
    }

    fn edge(from: &str, to: &str, coordinates: &[(f64, f64)]) -> WorkingEdge {
        WorkingEdge {
            id: "route".to_owned(),
            from: from.to_owned(),
            to: to.to_owned(),
            reference_id: "route".to_owned(),
            label_node_id: None,
            reversed_for_layout: false,
            points: coordinates
                .iter()
                .map(|&(x, y)| LayoutPoint { x, y })
                .collect(),
        }
    }

    fn stacked(target_x: f64) -> WorkingLayout {
        WorkingLayout {
            direction: SwimlaneDirection::Tb,
            nodes: [
                node("K", Some("top"), 0.0, 0.0),
                node("S", Some("low"), target_x, 200.0),
            ]
            .into_iter()
            .map(|node| (node.id.clone(), node))
            .collect(),
            graph_edges: Vec::new(),
            original_edges: vec![edge(
                "K",
                "S",
                &[
                    (50.0, 0.0),
                    (150.0, 0.0),
                    (150.0, 130.0),
                    (target_x, 130.0),
                    (target_x, 150.0),
                ],
            )],
            top_lane_order: Vec::new(),
        }
    }

    fn points(edge: &WorkingEdge) -> Vec<(f64, f64)> {
        edge.points.iter().map(|point| (point.x, point.y)).collect()
    }

    fn straighten(layout: &mut WorkingLayout) -> bool {
        straighten_aligned_cross_group_edges(layout, &mut LayoutWorkBudget::unbounded_for_tests())
            .unwrap()
    }

    #[test]
    fn aligned_straightening_respects_both_axes_and_the_source_center_at_one_pixel() {
        for (rotate, reverse, expected) in [
            (false, false, [(0.0, 50.0), (0.0, 150.0)]),
            (false, true, [(1.0, 150.0), (1.0, 50.0)]),
            (true, false, [(50.0, 0.0), (150.0, 0.0)]),
            (true, true, [(150.0, 1.0), (50.0, 1.0)]),
        ] {
            let mut layout = stacked(1.0);
            let route = &mut layout.original_edges[0];
            if rotate {
                for node in layout.nodes.values_mut() {
                    std::mem::swap(&mut node.x, &mut node.y);
                }
                for point in &mut route.points {
                    std::mem::swap(&mut point.x, &mut point.y);
                }
            }
            if reverse {
                std::mem::swap(&mut route.from, &mut route.to);
                route.points.reverse();
            }
            assert!(straighten(&mut layout));
            assert_eq!(points(&layout.original_edges[0]), expected);
            assert!(!straighten(&mut layout));
        }
        assert!(!straighten(&mut stacked(1.01)));

        let mut both_axes = stacked(1.0);
        for node in both_axes.nodes.values_mut() {
            node.width = 0.5;
        }
        both_axes.nodes["S"].y = 0.5;
        assert!(!straighten(&mut both_axes));
    }

    #[test]
    fn aligned_straightening_keeps_occupied_faces_and_foreign_obstacles() {
        let mut layout = stacked(0.0);
        layout.nodes["S"].parent_id = layout.nodes["K"].parent_id.clone();
        assert!(!straighten(&mut layout));

        layout = stacked(0.0);
        layout.nodes["S"].y = 100.0;
        assert!(!straighten(&mut layout));

        for other in [
            edge("K", "elsewhere", &[(20.0, 60.0), (200.0, 60.0)]),
            edge("elsewhere", "S", &[(200.0, 140.0), (20.0, 140.0)]),
        ] {
            let mut layout = stacked(0.0);
            let before = points(&layout.original_edges[0]);
            layout.original_edges.push(other.clone());
            assert!(!straighten(&mut layout));
            assert_eq!(points(&layout.original_edges[0]), before);
            layout.graph_edges.push(other);
            layout.original_edges.pop();
            assert!(straighten(&mut layout));
        }

        let mut obstacle = node("obstacle", None, 0.0, 100.0);
        obstacle.width = 20.0;
        obstacle.height = 20.0;
        layout = stacked(0.0);
        layout.nodes.insert(obstacle.id.clone(), obstacle);
        assert!(!straighten(&mut layout));
        layout.nodes["obstacle"].kind = WorkingNodeKind::EdgeLabel;
        assert!(!straighten(&mut layout));
        layout.original_edges[0].label_node_id = Some("obstacle".to_owned());
        assert!(straighten(&mut layout));

        layout.original_edges[0] = stacked(0.0).original_edges.remove(0);
        layout.nodes["obstacle"].x = 8.0;
        assert!(straighten(&mut layout));
    }

    #[test]
    fn aligned_straightening_keeps_each_edges_crossings_and_bend_contacts() {
        for other in [
            edge("P", "Q", &[(-40.0, 90.0), (40.0, 90.0)]),
            edge("P", "Q", &[(0.0, 60.0), (0.0, 100.0)]),
            edge("P", "Q", &[(40.0, 90.0), (5.0, 90.0), (5.0, 110.0)]),
        ] {
            let mut layout = stacked(0.0);
            let before = points(&layout.original_edges[0]);
            layout.original_edges.push(other);
            assert!(!straighten(&mut layout));
            assert_eq!(points(&layout.original_edges[0]), before);
        }

        let mut layout = stacked(0.0);
        layout
            .original_edges
            .push(edge("P", "Q", &[(-40.0, 90.0), (160.0, 90.0)]));
        assert!(straighten(&mut layout));

        let mut layout = stacked(0.0);
        layout
            .original_edges
            .push(edge("P", "Q", &[(40.0, 90.0), (6.0, 90.0), (6.0, 110.0)]));
        assert!(straighten(&mut layout));
    }

    #[test]
    fn aligned_straightening_uses_live_geometry_and_occurrence_order() {
        let mut layout = stacked(0.0);
        layout
            .nodes
            .insert("U".to_owned(), node("U", Some("top"), 0.0, 0.0));
        layout
            .nodes
            .insert("V".to_owned(), node("V", Some("low"), 0.0, 200.0));
        let second = edge(
            "U",
            "V",
            &[(-50.0, 0.0), (-150.0, 0.0), (-150.0, 200.0), (-50.0, 200.0)],
        );
        let before = points(&second);
        layout.original_edges.push(second);

        assert!(straighten(&mut layout));
        assert_eq!(
            points(&layout.original_edges[0]),
            [(0.0, 50.0), (0.0, 150.0)]
        );
        assert_eq!(points(&layout.original_edges[1]), before);
        assert_eq!(layout.original_edges[0].id, layout.original_edges[1].id);
        assert_eq!(layout.original_edges[1].from, "U");
    }

    #[test]
    fn aligned_straightening_and_refresh_share_the_transaction_work_ceiling() {
        let mut before = stacked(0.0);
        before.nodes["S"].width = 180.0;
        before.original_edges[0] = edge("K", "S", &[(50.0, 0.0), (70.0, 0.0), (70.0, 150.0)]);
        for (id, y) in [("top", 0.0), ("low", 200.0)] {
            let mut lane = node(id, None, 0.0, y);
            lane.kind = WorkingNodeKind::Group;
            lane.width = 400.0;
            lane.height = 200.0;
            before.nodes.insert(id.to_owned(), lane);
        }
        let mut label = node("label", Some("top"), 70.0, 75.0);
        label.kind = WorkingNodeKind::EdgeLabel;
        label.width = 20.0;
        label.height = 20.0;
        before.nodes.insert(label.id.clone(), label);
        before.original_edges[0].label_node_id = Some("label".to_owned());
        before.refresh_top_lane_ids();

        let mut shared = before.clone();
        post_process(
            &mut shared,
            &mut LayoutWorkBudget::unbounded_for_tests(),
            PostProcessMode::Shared,
        )
        .unwrap();
        assert!(shared.original_edges[0].points.len() > 2);

        let policy = RenderResourcePolicy::unbounded_for_trusted_input();
        let mut expected = before.clone();
        let mut budget = LayoutWorkBudget::unbounded_for_tests();
        post_process(&mut expected, &mut budget, PostProcessMode::LegacyFlow).unwrap();
        assert_eq!(
            points(&expected.original_edges[0]),
            [(0.0, 50.0), (0.0, 150.0)]
        );
        assert_eq!(expected.nodes["label"].x, 0.0);
        let required = budget.used();

        for limit in [required, required - 1] {
            let policy = policy
                .with_limit(ResourceLimitId::MaxLayoutWorkUnits, limit)
                .unwrap();
            let mut work = LayoutWorkBudget::new(policy, 0).unwrap();
            let mut layout = before.clone();
            let result = post_process(&mut layout, &mut work, PostProcessMode::LegacyFlow);
            if limit == required {
                result.unwrap();
                assert_eq!(work.used(), required);
                assert_eq!(
                    points(&layout.original_edges[0]),
                    points(&expected.original_edges[0])
                );
                assert_eq!(layout.nodes["label"].x, expected.nodes["label"].x);
            } else {
                let Error::ResourceLimitExceeded(error) = result.unwrap_err() else {
                    panic!("expected an exact layout work ceiling");
                };
                assert_eq!(
                    (error.limit, error.actual, error.max),
                    (
                        ResourceLimitId::MaxLayoutWorkUnits.as_str(),
                        required,
                        limit
                    )
                );
                assert_eq!(
                    points(&layout.original_edges[0]),
                    points(&before.original_edges[0])
                );
                assert_eq!(layout.nodes["label"].x, before.nodes["label"].x);
                assert_eq!(layout.nodes["label"].y, before.nodes["label"].y);
            }
        }
    }

    #[test]
    fn aligned_straightening_polls_inside_segment_comparisons() {
        let current = edge("K", "S", &[(50.0, 0.0), (70.0, 0.0), (70.0, 150.0)]);
        let line = edge("K", "S", &[(0.0, 50.0), (0.0, 150.0)]);
        let moved = orthogonal_segments_for_points(&line.points, EPSILON);
        let peer_points: Vec<_> = (0..120).map(|y| (300.0, f64::from(y))).collect();
        let edges = [current.clone(), edge("P", "Q", &peer_points)];
        let control = OperationControl::new();
        let meter = Arc::new(OperationWorkMeter::new_with_control(
            RenderResourcePolicy::unbounded_for_trusted_input(),
            control.clone(),
        ));
        let mut budget = LayoutWorkBudget::for_operation(Arc::clone(&meter));
        // The current allocation and the peer preflight and debit pass. The next poll falls
        // within the 119 candidate-versus-peer segment comparisons.
        control.cancel_after_checkpoints(3);
        let error =
            clear_of_other_edges(0, &current.points, &moved[0], &edges, &mut budget).unwrap_err();
        let Error::Cancelled(cancelled) = error else {
            panic!("expected cancellation during route comparison");
        };
        assert_eq!(cancelled.phase, OperationPhase::Layout);
        assert_eq!(meter.used(), 3);
        assert!(matches!(
            Error::from(meter.checkpoint(OperationPhase::Emit).unwrap_err()),
            Error::Cancelled(replayed) if replayed == cancelled
        ));
    }
}
