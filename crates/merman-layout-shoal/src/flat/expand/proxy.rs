use super::{Link, PortSideHint};
use crate::flat::to_canonical_port;
use crate::work::{LayoutResult, Work, push};
use crate::{Direction, EdgeSpec, Graph, LayoutError, NodeSpec, Side};

#[cfg(test)]
mod tests;

pub(super) struct ProxyPorts<'a> {
    pub nodes: &'a [NodeSpec],
    pub reversed: &'a [bool],
    pub clearance: f64,
}

#[derive(Clone, Copy)]
struct Contact {
    offset: f64,
    half_marker: f64,
}

fn contacts(edge: &EdgeSpec, reversed: bool, direction: Direction) -> Option<[Contact; 2]> {
    if edge.label.is_some() || edge.start_label.is_some() || edge.end_label.is_some() {
        return None;
    }
    let mut ends = [
        (edge.source_port, edge.start_marker.width),
        (edge.target_port, edge.end_marker.width),
    ];
    if reversed {
        ends.swap(0, 1);
    }
    let fixed = |(port, width), face| match to_canonical_port(direction, port) {
        PortSideHint::FixedOffset { side, offset } if side == face => Some(Contact {
            offset,
            half_marker: width * 0.5,
        }),
        _ => None,
    };
    Some([fixed(ends[0], Side::Bottom)?, fixed(ends[1], Side::Top)?])
}

impl ProxyPorts<'_> {
    pub(super) fn separated(
        self,
        arcs: &[Link],
        ranks: &[u64],
        graph: &Graph,
        work: &mut Work<'_>,
    ) -> LayoutResult<Vec<(u32, u32)>> {
        let mut lanes = Vec::new();
        for (i, arc) in arcs.iter().enumerate() {
            work.visit(1)?;
            let (from, to) = (arc.from as usize, arc.to as usize);
            if (!self.nodes[from].group_proxy && !self.nodes[to].group_proxy)
                || ranks[to].checked_sub(ranks[from]) != Some(1)
            {
                continue;
            }
            let spec = graph
                .edges
                .get(arc.edge as usize)
                .ok_or(LayoutError::InvalidInput("invalid global edge occurrence"))?;
            let ports = contacts(spec, self.reversed[i], graph.direction);
            push(&mut lanes, ((arc.from, arc.to), ports))?;
        }
        work.charge_sort(lanes.len())?;
        lanes.sort_unstable_by(|(pair_a, a), (pair_b, b)| {
            let source =
                |ports: &Option<[Contact; 2]>| ports.map_or(f64::NEG_INFINITY, |p| p[0].offset);
            pair_a
                .cmp(pair_b)
                .then_with(|| source(a).total_cmp(&source(b)))
        });
        let mut separated = Vec::new();
        for bundle in lanes.chunk_by(|a, b| a.0 == b.0) {
            work.visit(bundle.len())?;
            if bundle.len() >= 2
                && bundle.windows(2).all(|pair| {
                    let (Some(left), Some(right)) = (pair[0].1, pair[1].1) else {
                        return false;
                    };
                    left.into_iter().zip(right).all(|(a, b)| {
                        b.offset > a.offset
                            && b.offset - a.offset >= a.half_marker + self.clearance + b.half_marker
                    })
                })
            {
                push(&mut separated, bundle[0].0)?;
            }
        }
        Ok(separated)
    }
}
