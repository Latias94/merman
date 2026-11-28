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
        let steps = (count - 1) as f64;
        let ink = members
            .iter()
            .map(|member| member.ink_width)
            .fold(0.0, f64::max);
        let spans = [0, 1].map(|end| {
            let usable = profiles[nodes[end]].port_span(sides[end]);
            (steps * ink).clamp(usable * 0.6, usable * 0.75)
        });
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
