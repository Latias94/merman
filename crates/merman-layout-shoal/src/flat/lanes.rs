//! Place all visible lane contents on one set of flat ranks.

use super::{
    Expanded, bus, expand_component, extras, hub, order, pack, position, refine, routing, validate,
};
use crate::work::{LayoutResult, Work, filled, reserved};
use crate::{Axis, Graph, LayoutError, LayoutOutput, LayoutStats, Options, Size, WorkError};

#[derive(Clone, Copy)]
pub(crate) struct Column {
    pub(crate) min_width: f64,
    pub(crate) padding: [f64; 2],
}

#[derive(Clone, Copy)]
pub(super) struct Columns<'a> {
    pub(super) owners: &'a [usize],
    pub(super) content: &'a [[f64; 2]],
}

fn expanded_owners(g: &Expanded, items: &[usize], work: &mut Work<'_>) -> LayoutResult<Vec<usize>> {
    work.charge(g.rank.len())?;
    let mut owners = reserved(g.rank.len())?;
    owners.extend_from_slice(items);
    owners.resize(g.rank.len(), 0);
    for chain in &g.chains {
        work.visit(chain.len())?;
        let [first, inside @ .., last] = chain.as_slice() else {
            continue;
        };
        let (a, b) = (owners[*first as usize], owners[*last as usize]);
        let steps = (inside.len() + 1) as u64;
        for (i, &v) in inside.iter().enumerate() {
            // Ordinals and expanded vertex indices are bounded by u32. Interpolate by
            // logical rank so that an omitted or empty rank does not move a separator.
            let r0 = g.logical_rank[g.rank[*first as usize] as usize];
            let r1 = g.logical_rank[g.rank[*last as usize] as usize];
            let r = g.logical_rank[g.rank[v as usize] as usize];
            let portion = if r0 < r1 {
                (r - r0) as f64 / (r1 - r0) as f64
            } else {
                (i + 1) as f64 / steps as f64
            };
            owners[v as usize] = (a as f64 + (b as f64 - a as f64) * portion).round() as usize;
        }
    }
    Ok(owners)
}

struct Placement {
    shells: Vec<[f64; 2]>,
    content: Vec<[f64; 2]>,
    shifts: Vec<f64>,
    span: f64,
}

fn place(columns: &[Column], spans: &[[f64; 2]], work: &mut Work<'_>) -> LayoutResult<Placement> {
    work.charge(columns.len())?;
    let mut result = Placement {
        shells: reserved(columns.len())?,
        content: reserved(columns.len())?,
        shifts: reserved(columns.len())?,
        span: 0.0,
    };
    for (column, &[left, right]) in columns.iter().zip(spans) {
        work.visit(1)?;
        let natural = right - left;
        let [before, after] = column.padding;
        let width = (before + natural + after).max(column.min_width);
        let first = result.span;
        result.span += width;
        if !result.span.is_finite() || (width != 0.0 && result.span <= first) {
            return Err(LayoutError::InvalidInput(
                "swimlane width exceeds coordinate precision",
            ));
        }
        result.shells.push([first, result.span]);
        result.content.push([first + before, result.span - after]);
        let shift = first + before + (width - before - after - natural).max(0.0) / 2.0 - left;
        if !shift.is_finite() {
            return Err(WorkError::ArithmeticOverflow.into());
        }
        result.shifts.push(shift);
    }
    Ok(result)
}

pub(crate) fn run(
    graph: &Graph,
    options: &Options,
    g: &mut Expanded,
    items: &[usize],
    columns: &[Column],
    work: &mut Work<'_>,
) -> LayoutResult<(LayoutOutput, Vec<[f64; 2]>)> {
    debug_assert_eq!(items.len(), graph.nodes.len());
    let mut options = *options;
    options.inline_labels = false;
    work.charge(columns.len())?;
    let mut spans = filled(columns.len(), [f64::INFINITY, f64::NEG_INFINITY])?;
    let Some(c) = validate::collect(graph, &options, true, work)?.pop() else {
        spans.fill([0.0, 0.0]);
        let placement = place(columns, &spans, work)?;
        let bounds = match graph.direction.axis() {
            Axis::Vertical => Size {
                width: placement.span,
                height: 0.0,
            },
            Axis::Horizontal => Size {
                width: 0.0,
                height: placement.span,
            },
        };
        return Ok((
            LayoutOutput {
                bounds,
                direction: graph.direction,
                ..Default::default()
            },
            placement.shells,
        ));
    };
    let options = &options;
    let orientation = expand_component(&c, graph, options, work, g)?;
    g.exterior_lanes.clear();
    let owners = expanded_owners(g, items, work)?;
    let topological_crossings = order::constrained_lanes(g, &c, &owners, options, work)?;
    let mut x = position::assign(g, &c, Some(&orientation), graph, options, work)?;
    for (v, (&x, &lane)) in x.iter().zip(&owners).enumerate() {
        work.visit(1)?;
        let (before, after) = position::extents(g, &c, options, v as u32);
        let [lo, hi] = &mut spans[lane];
        *lo = lo.min(x - before);
        *hi = hi.max(x + after);
    }
    work.visit(spans.len())?;
    for span in &mut spans {
        if span[0] == f64::INFINITY {
            *span = [0.0, 0.0];
        }
    }
    let mut placement = place(columns, &spans, work)?;
    work.visit(x.len())?;
    for (x, &lane) in x.iter_mut().zip(&owners) {
        *x += placement.shifts[lane];
    }
    refine::refine_in_columns(
        g,
        &c,
        &orientation,
        graph,
        options,
        &mut x,
        Some(Columns {
            owners: &owners,
            content: &placement.content,
        }),
        work,
    )?;
    let (mut drawing, inline) =
        routing::route_with_label_gaps(g, &c, &orientation, graph, options, &x, work)?;
    debug_assert!(inline.is_empty());
    extras::finish(&mut drawing, g, &c, graph, options, work)?;
    hub::spread(&mut drawing, &c, graph, options, work)?;
    // Compound assembly measures crossings once the lane contacts and nested legs are stitched.
    bus::bundle(&mut drawing, &c, graph, options, work)?;
    if options.compact_ranks {
        routing::compact_ranks(&mut drawing, &c, g, graph, options, work)?;
    }
    let stats = LayoutStats {
        components: 1,
        scc_cyclic_count: orientation.cyclic_sccs,
        reversed_edges: orientation.reversed.iter().filter(|&&r| r).count(),
        dummy_nodes: g.rank.len() - c.nodes.len(),
        direct_fallback_edges: g.direct.iter().filter(|&&direct| direct).count(),
        topological_crossings,
        ..Default::default()
    };
    let ranks = if options.capture_node_ranks {
        work.charge(c.nodes.len())?;
        let mut ranks = reserved(c.nodes.len())?;
        ranks.extend_from_slice(&g.rank[..c.nodes.len()]);
        ranks
    } else {
        Vec::new()
    };
    let mut tile = pack::Tile::new(c.ids, ranks, drawing, graph, options.width_policy, work)?;
    let shift = tile.cover_columns(placement.span, graph.direction)?;
    work.visit(placement.shells.len())?;
    for column in &mut placement.shells {
        column[0] += shift;
        column[1] += shift;
    }
    work.charge(1)?;
    let mut tiles = reserved(1)?;
    tiles.push(tile);
    let drawing = pack::assemble(tiles, graph, options, stats, work)?;
    Ok((drawing, placement.shells))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NoopWorkControl;

    #[test]
    fn empty_columns_can_share_a_boundary_without_hiding_precision_loss() {
        let column = |min_width| Column {
            min_width,
            padding: [0.0, 0.0],
        };
        let mut control = NoopWorkControl;
        let mut work = Work::new(&mut control);
        let spans = [[0.0, 0.0]; 2];
        let placement = place(&[column(8.0), column(0.0)], &spans, &mut work).unwrap();
        assert_eq!(placement.shells, [[0.0, 8.0], [8.0, 8.0]]);
        assert_eq!(placement.content, placement.shells);
        assert!(matches!(
            place(&[column(2.0_f64.powi(54)), column(1.0)], &spans, &mut work),
            Err(LayoutError::InvalidInput(_))
        ));
    }
}
