use super::common::*;
use indexmap::IndexMap;

const BUFFER: f64 = 2.0;
const MAX_ITERATIONS: usize = 4;

#[derive(Clone)]
struct TerminalTail {
    tail_start: LayoutPoint,
    terminal: LayoutPoint,
}

fn terminal_tail_for(edges: &[WorkingEdge], edge_index: usize) -> Option<TerminalTail> {
    let points = dedupe_consecutive_points(&edges[edge_index].points, EPSILON);
    if points.len() < 4 {
        return None;
    }
    let tail_start = &points[points.len() - 2];
    let terminal = points.last()?;
    (is_horizontal_segment(tail_start, terminal, EPSILON)
        || is_vertical_segment(tail_start, terminal, EPSILON))
    .then(|| TerminalTail {
        tail_start: tail_start.clone(),
        terminal: terminal.clone(),
    })
}

fn candidate_with_destination_tail(
    edges: &[WorkingEdge],
    edge_index: usize,
    tail: &TerminalTail,
) -> Option<Vec<LayoutPoint>> {
    let points = dedupe_consecutive_points(&edges[edge_index].points, EPSILON);
    if points.len() < 3 {
        return None;
    }
    let start = &points[0];
    let first_turn = &points[1];
    let connector = if is_horizontal_segment(start, first_turn, EPSILON) {
        LayoutPoint {
            x: first_turn.x,
            y: tail.tail_start.y,
        }
    } else if is_vertical_segment(start, first_turn, EPSILON) {
        LayoutPoint {
            x: tail.tail_start.x,
            y: first_turn.y,
        }
    } else {
        return None;
    };
    let candidate = simplify_polyline(&dedupe_consecutive_points(
        &[
            start.clone(),
            first_turn.clone(),
            connector,
            tail.tail_start.clone(),
            tail.terminal.clone(),
        ],
        EPSILON,
    ));
    (segments_for(&candidate).len() == candidate.len().saturating_sub(1)).then_some(candidate)
}

fn path_has_node_hit(
    edge: &WorkingEdge,
    path: &[LayoutPoint],
    real_node_rects: &[RectEntry],
) -> bool {
    let excluded = endpoint_id_slices(edge);
    segments_for(path).iter().any(|segment| {
        segment_hits_any_rect(&segment.a, &segment.b, real_node_rects, &excluded, -BUFFER)
    })
}

fn path_has_shared_track(
    edges: &[WorkingEdge],
    edge_index: usize,
    path: &[LayoutPoint],
    replacements: &ReplacementMap,
) -> bool {
    let candidate_segments = segments_for(path);
    (0..edges.len()).any(|other_index| {
        other_index != edge_index
            && shared_track_conflicts(
                &candidate_segments,
                &segments_for(&points_for(edges, other_index, replacements)),
            )
    })
}

fn candidate_is_safe(
    edges: &[WorkingEdge],
    edge_index: usize,
    path: &[LayoutPoint],
    replacements: &ReplacementMap,
    real_node_rects: &[RectEntry],
) -> bool {
    !path_has_node_hit(&edges[edge_index], path, real_node_rects)
        && !path_has_shared_track(edges, edge_index, path, replacements)
}

fn edges_by_destination(layout: &WorkingLayout) -> IndexMap<String, Vec<usize>> {
    let mut result: IndexMap<String, Vec<usize>> = IndexMap::new();
    for (edge_index, edge) in layout.original_edges.iter().enumerate() {
        if !layout.nodes.contains_key(&edge.to)
            || dedupe_consecutive_points(&edge.points, EPSILON).len() < 4
        {
            continue;
        }
        result.entry(edge.to.clone()).or_default().push(edge_index);
    }
    result
}

pub(in crate::swimlane::direction) fn swap_destination_terminal_tails_to_reduce_crossings(
    layout: &mut WorkingLayout,
) {
    let (real_node_rects, _) = collect_node_rect_entries(layout);
    for _ in 0..MAX_ITERATIONS {
        let empty = ReplacementMap::new();
        let current_crossings = strict_crossing_count(&layout.original_edges, &empty);
        if current_crossings == 0 {
            return;
        }
        let current_bends = total_bends(&layout.original_edges, &empty);
        let mut best_replacements = None;
        let mut best_crossings = current_crossings;
        let mut best_bends = current_bends;

        for destination_edges in edges_by_destination(layout).values() {
            for first_position in 0..destination_edges.len() {
                for second_position in first_position + 1..destination_edges.len() {
                    let first_index = destination_edges[first_position];
                    let second_index = destination_edges[second_position];
                    let (Some(first_tail), Some(second_tail)) = (
                        terminal_tail_for(&layout.original_edges, first_index),
                        terminal_tail_for(&layout.original_edges, second_index),
                    ) else {
                        continue;
                    };
                    let (Some(first_candidate), Some(second_candidate)) = (
                        candidate_with_destination_tail(
                            &layout.original_edges,
                            first_index,
                            &second_tail,
                        ),
                        candidate_with_destination_tail(
                            &layout.original_edges,
                            second_index,
                            &first_tail,
                        ),
                    ) else {
                        continue;
                    };
                    let replacements = ReplacementMap::from_iter([
                        (first_index, first_candidate.clone()),
                        (second_index, second_candidate.clone()),
                    ]);
                    if !candidate_is_safe(
                        &layout.original_edges,
                        first_index,
                        &first_candidate,
                        &replacements,
                        &real_node_rects,
                    ) || !candidate_is_safe(
                        &layout.original_edges,
                        second_index,
                        &second_candidate,
                        &replacements,
                        &real_node_rects,
                    ) {
                        continue;
                    }
                    let candidate_crossings =
                        strict_crossing_count(&layout.original_edges, &replacements);
                    let candidate_bends = total_bends(&layout.original_edges, &replacements);
                    if candidate_crossings >= current_crossings
                        || candidate_crossings > best_crossings
                        || (candidate_crossings == best_crossings && candidate_bends >= best_bends)
                    {
                        continue;
                    }
                    best_replacements = Some(replacements);
                    best_crossings = candidate_crossings;
                    best_bends = candidate_bends;
                }
            }
        }
        let Some(replacements) = best_replacements else {
            return;
        };
        apply_replacements(&mut layout.original_edges, replacements);
    }
}
