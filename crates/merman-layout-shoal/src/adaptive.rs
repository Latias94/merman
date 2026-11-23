//! Width trials share a work meter and reuse the same component scratch.

use crate::flat::{self, Expanded};
use crate::work::{LayoutResult, Work, reserved};
use crate::{
    Direction, Graph, GroupSpec, LayoutError, LayoutOutput, Options, PackingPolicy, TitlePosition,
    WidthPolicy,
};

pub(crate) fn run(
    graph: &Graph,
    options: &Options,
    expanded: &mut Expanded,
    work: &mut Work<'_>,
) -> LayoutResult<LayoutOutput> {
    let WidthPolicy::FitWidth {
        max_width,
        allow_turn,
        turn_breakpoint,
        fold_wide_ranks,
    } = options.width_policy
    else {
        unreachable!()
    };
    flat::validate_options(options)?;
    let mut wrapped = *options;
    wrapped.packing = PackingPolicy::FitWidth { max_width };
    let mut best = crate::solve(graph, &wrapped, expanded, work)?;
    if best.bounds.width <= max_width {
        return Ok(best);
    }

    let mut tight = wrapped;
    tight.spacing.node *= 0.5;
    tight.spacing.rank *= 0.75;
    let compact = if graph.groups.is_empty() {
        crate::solve(graph, &tight, expanded, work)
    } else {
        let mut compact = copy_graph(graph, work)?;
        tighten_groups(&mut compact.groups, work)?;
        crate::solve(&compact, &tight, expanded, work)
    };
    let compact_kept = try_narrower(&mut best, compact)?;
    if best.bounds.width <= max_width {
        return Ok(best);
    }

    if allow_turn
        && max_width < turn_breakpoint
        && matches!(graph.direction, Direction::Right | Direction::Left)
    {
        let mut turned = copy_graph(graph, work)?;
        turned.direction = Direction::Down;
        let flat = turned.groups.is_empty();
        for node in &mut turned.nodes {
            work.visit(1)?;
            let primary = node.port_span;
            node.port_span = node.perpendicular_port_span.or(primary);
            node.perpendicular_port_span = primary.or_else(|| {
                node.port_span
                    .map(|_| (node.size.height - 2.0 * wrapped.spacing.corner_clearance).max(0.0))
            });
            if flat {
                node.align_flow(turned.direction);
            }
        }
        wrapped.group_policy.inherit_crossed_direction = false;
        tight.group_policy.inherit_crossed_direction = false;
        let turned_kept = try_narrower(&mut best, crate::solve(&turned, &wrapped, expanded, work))?;
        if best.bounds.width <= max_width {
            return Ok(best);
        }
        tighten_groups(&mut turned.groups, work)?;
        let tight_turn_kept =
            try_narrower(&mut best, crate::solve(&turned, &tight, expanded, work))?;
        if fold_wide_ranks && best.bounds.width > max_width {
            let spacing = if turned_kept && !tight_turn_kept {
                &wrapped
            } else {
                &tight
            };
            try_fold(&mut best, &turned, spacing, expanded, max_width, work)?;
        }
    } else if fold_wide_ranks {
        let spacing = if compact_kept { &tight } else { &wrapped };
        try_fold(&mut best, graph, spacing, expanded, max_width, work)?;
    }
    Ok(best)
}

fn try_fold(
    best: &mut LayoutOutput,
    graph: &Graph,
    options: &Options,
    expanded: &mut Expanded,
    max_width: f64,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    match flat::fold(graph, options, expanded, max_width, work) {
        Ok(Some(candidate)) if candidate.bounds.width < best.bounds.width => *best = candidate,
        Ok(_) | Err(LayoutError::InvalidInput(_)) => {}
        Err(error) => return Err(error),
    }
    Ok(())
}

fn try_narrower(
    best: &mut LayoutOutput,
    candidate: LayoutResult<LayoutOutput>,
) -> LayoutResult<bool> {
    match candidate {
        Ok(drawing) if drawing.bounds.width < best.bounds.width => {
            *best = drawing;
            Ok(true)
        }
        Ok(_) | Err(LayoutError::InvalidInput(_)) => Ok(false),
        Err(error) => Err(error),
    }
}

pub(super) fn copy_graph(graph: &Graph, work: &mut Work<'_>) -> LayoutResult<Graph> {
    fn copy<T: Copy>(items: &[T], work: &mut Work<'_>) -> LayoutResult<Vec<T>> {
        work.charge(items.len())?;
        let mut result = reserved(items.len())?;
        for chunk in items.chunks(128) {
            work.check()?;
            result.extend_from_slice(chunk);
        }
        Ok(result)
    }
    Ok(Graph {
        direction: graph.direction,
        nodes: copy(&graph.nodes, work)?,
        groups: copy(&graph.groups, work)?,
        edges: copy(&graph.edges, work)?,
    })
}

fn tighten_groups(groups: &mut [GroupSpec], work: &mut Work<'_>) -> LayoutResult<()> {
    for group in groups {
        work.visit(1)?;
        let title_height = group.title.map_or(0.0, |title| title.height);
        let [top, bottom] = match group.title_position {
            TitlePosition::TopLeft | TitlePosition::TopCenter | TitlePosition::TopRight => {
                [title_height, 0.0]
            }
            _ => [0.0, title_height],
        };
        let tighten = |padding: f64, title: f64| {
            let band = padding.min(title);
            band + 0.6 * (padding - band)
        };
        group.padding.top = tighten(group.padding.top, top);
        group.padding.bottom = tighten(group.padding.bottom, bottom);
        group.padding.left *= 0.6;
        group.padding.right *= 0.6;
    }
    Ok(())
}
