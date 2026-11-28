use crate::Result;
use crate::graph_layout::contour::ShapePortProfile;
use crate::graph_layout::ir::{Axis, Endpoint, MeasuredGraph, RoutePoint, Side, SolvedGraph};
use crate::layout_work::OperationLayoutWorkControl;
use merman_layout_shoal::PortSideHint;
use rustc_hash::FxHashMap;

type PortHints = (PortSideHint, PortSideHint);

struct ParallelMember {
    edge: usize,
    reversed: bool,
    ports: [f64; 2],
    ink_width: f64,
}

pub(super) fn parallel_ports(
    graph: &MeasuredGraph,
    profiles: &[ShapePortProfile],
    solved: &SolvedGraph,
    work: &mut OperationLayoutWorkControl,
) -> Result<Vec<PortHints>> {
    work.charge_adapter(graph.edges.len())?;
    if graph
        .edges
        .iter()
        .filter(|edge| edge.label.is_some())
        .take(3)
        .count()
        < 3
    {
        return Ok(Vec::new());
    }
    work.charge_adapter(graph.nodes.len())?;
    let mut face_counts = vec![[0usize; 4]; graph.nodes.len()];
    let mut bundles: FxHashMap<_, Vec<ParallelMember>> = FxHashMap::default();
    for (index, (edge, spec)) in solved.edges.iter().zip(&graph.edges).enumerate() {
        work.charge_adapter(0)?;
        let mut sides = [edge.ports.0, edge.ports.1];
        for (endpoint, side) in [(edge.source, sides[0]), (edge.target, sides[1])] {
            if let Endpoint::Node(node) = endpoint {
                face_counts[node as usize][side as usize] += 1;
            }
        }
        let (Endpoint::Node(a), Endpoint::Node(b), Some(label)) =
            (edge.source, edge.target, spec.label)
        else {
            continue;
        };
        if a == b || sides[0] == sides[1] || sides[0].axis() != sides[1].axis() {
            continue;
        }
        let [first, .., last] = edge.points.as_slice() else {
            continue;
        };
        let reversed = a > b;
        let mut nodes = [a as usize, b as usize];
        let mut ports = [across(sides[0], *first), across(sides[1], *last)];
        if reversed {
            nodes.swap(0, 1);
            sides.swap(0, 1);
            ports.swap(0, 1);
        }
        let label_width = match sides[0].axis() {
            Axis::Horizontal => label.height,
            Axis::Vertical => label.width,
        };
        bundles
            .entry((nodes, sides.map(|side| side as usize)))
            .or_default()
            .push(ParallelMember {
                edge: index,
                reversed,
                ports,
                ink_width: label_width.max(spec.start_marker.width.max(spec.end_marker.width)),
            });
    }

    let mut hints = Vec::new();
    for ((nodes, face_indices), mut members) in bundles {
        let count = members.len();
        // Preserve any face order that the solver shares with connections outside this bundle.
        if count < 3 || (0..2).any(|end| face_counts[nodes[end]][face_indices[end]] != count) {
            continue;
        }
        work.charge_adapter(work.checked_mul(count, count.ilog2() as usize + 1)?)?;
        members
            .sort_unstable_by(|a, b| a.ports[0].total_cmp(&b.ports[0]).then(a.edge.cmp(&b.edge)));
        if members
            .windows(2)
            .any(|pair| pair[0].ports[1] > pair[1].ports[1])
        {
            continue;
        }
        let first = &members[0];
        let last = &members[count - 1];
        let (a, b) = solved.edges[first.edge].ports;
        let sides = if first.reversed { [b, a] } else { [a, b] };
        let usable = [0, 1].map(|end| profiles[nodes[end]].port_span(sides[end]));
        if usable.iter().any(|&span| !span.is_finite() || span <= 0.0) {
            continue;
        }
        let steps = (count - 1) as f64;
        let ink = members
            .iter()
            .map(|member| member.ink_width)
            .fold(0.0, f64::max);
        let spans = usable.map(|span| (steps * ink).clamp(span * 0.6, span * 0.75));
        if (0..2).any(|end| spans[end] <= last.ports[end] - first.ports[end]) {
            continue;
        }
        if hints.is_empty() {
            work.charge_adapter(solved.edges.len())?;
            hints.resize(solved.edges.len(), (PortSideHint::Auto, PortSideHint::Auto));
        }
        for (slot, member) in members.iter().enumerate() {
            let fraction = slot as f64 / steps - 0.5;
            let [a, b] = [0, 1].map(|end| PortSideHint::FixedOffset {
                side: sides[end],
                offset: fraction * spans[end],
            });
            hints[member.edge] = if member.reversed { (b, a) } else { (a, b) };
        }
    }
    Ok(hints)
}

fn across(side: Side, point: RoutePoint) -> f64 {
    match side.axis() {
        Axis::Horizontal => point.y,
        Axis::Vertical => point.x,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph_layout::ir::{
        Direction, MeasuredEdge, MeasuredNode, RoutingStyle, Size, SolvedEdge,
    };
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy};
    use std::sync::Arc;

    #[test]
    fn parallel_ports_require_a_finite_positive_span_at_both_ends() {
        let source = Endpoint::Node(0);
        let target = Endpoint::Node(1);
        let size = Size {
            width: 80.0,
            height: 80.0,
        };
        let graph = MeasuredGraph {
            direction: Direction::Right,
            routing: RoutingStyle::Orthogonal,
            node_spacing: 0.0,
            rank_spacing: 0.0,
            component_spacing: 0.0,
            inherit_crossed_group_direction: false,
            nodes: vec![
                MeasuredNode::new(size, 0, None),
                MeasuredNode::new(size, 1, None),
            ],
            groups: Vec::new(),
            edges: (0..3)
                .map(|_| MeasuredEdge {
                    source,
                    target,
                    min_rank_span: 1,
                    weight: 1.0,
                    label: Some(Size {
                        width: 24.0,
                        height: 10.0,
                    }),
                    start_label: None,
                    end_label: None,
                    start_marker: Default::default(),
                    end_marker: Default::default(),
                })
                .collect(),
        };
        let solved = SolvedGraph {
            direction: graph.direction,
            bounds: Default::default(),
            nodes: Vec::new(),
            node_ranks: Vec::new(),
            groups: Vec::new(),
            edges: (0..3)
                .map(|i| SolvedEdge {
                    source,
                    target,
                    points: vec![
                        RoutePoint::new(80.0, f64::from(i), Axis::Horizontal),
                        RoutePoint::new(160.0, f64::from(i), Axis::Horizontal),
                    ],
                    ports: (Side::Right, Side::Left),
                    label: None,
                    start_label: None,
                    end_label: None,
                    corridor_clearance: 0.0,
                    reversed_for_ranking: false,
                })
                .collect(),
            border_stitches: Vec::new(),
        };
        let profile = ShapePortProfile::rectangle(size.width, size.height);
        let mut work = OperationLayoutWorkControl::new(Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::constrained(),
        )));
        let hints = parallel_ports(
            &graph,
            &[profile.clone(), profile.clone()],
            &solved,
            &mut work,
        )
        .unwrap();
        assert_eq!(hints.len(), 3);
        assert!(hints.iter().all(|(start, end)| {
            matches!(start, PortSideHint::FixedOffset { offset, .. } if offset.is_finite())
                && matches!(end, PortSideHint::FixedOffset { offset, .. } if offset.is_finite())
        }));
        for span in [0.0, -5.0, f64::NAN, f64::INFINITY] {
            for (end, side) in [(0, Side::Right), (1, Side::Left)] {
                let mut profiles = [profile.clone(), profile.clone()];
                profiles[end] = profile.clone().with_port_span(side, span);
                assert!(
                    parallel_ports(&graph, &profiles, &solved, &mut work)
                        .unwrap()
                        .is_empty()
                );
            }
        }
    }
}
