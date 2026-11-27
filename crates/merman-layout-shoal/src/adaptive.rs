//! Width trials share a work meter and reuse the same component scratch.

use crate::flat::{self, Expanded};
use crate::validate;
use crate::work::{LayoutResult, Work, reserved};
use crate::{
    Direction, Graph, GroupSpec, LayoutDetail, LayoutError, LayoutOutput, Options, PackingPolicy,
    TitlePosition, WidthPolicy,
};

#[derive(Clone, Copy)]
pub(crate) struct WidthTrial {
    options: Options,
    turned: bool,
    tight_groups: bool,
    fold_at: Option<f64>,
}

impl WidthTrial {
    pub(crate) fn replay(
        self,
        graph: &mut Graph,
        expanded: &mut Expanded,
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<LayoutOutput>> {
        if self.turned {
            turn_down(graph, &self.options, work)?;
        }
        if self.tight_groups {
            tighten_groups(&mut graph.groups, work)?;
        }
        if let Some(max_width) = self.fold_at {
            flat::fold(
                graph,
                &self.options,
                expanded,
                max_width,
                LayoutDetail::Full,
                work,
            )
        } else {
            let mut output =
                crate::solve(graph, &self.options, expanded, LayoutDetail::Full, work)?;
            crate::compound::relax_seams(graph, &self.options, &mut output, work)?;
            Ok(Some(output))
        }
    }
}

type WidthSolution = (LayoutOutput, WidthTrial);

pub(crate) fn run(
    graph: &Graph,
    options: &Options,
    expanded: &mut Expanded,
    detail: LayoutDetail,
    work: &mut Work<'_>,
) -> LayoutResult<WidthSolution> {
    let (mut drawing, trial) = candidates(graph, options, expanded, detail, work)?;
    // Seam edits stay inside the participating groups and their connecting corridor.
    // Trial-specific geometry is in the drawing. Endpoint ownership and label and marker sizes
    // come from the original graph and are shared by all trials.
    crate::compound::relax_seams(graph, &trial.options, &mut drawing, work)?;
    Ok((drawing, trial))
}

fn candidates(
    graph: &Graph,
    options: &Options,
    expanded: &mut Expanded,
    detail: LayoutDetail,
    work: &mut Work<'_>,
) -> LayoutResult<WidthSolution> {
    let WidthPolicy::FitWidth {
        max_width,
        allow_turn,
        turn_breakpoint,
        fold_wide_ranks,
    } = options.width_policy
    else {
        unreachable!()
    };
    validate::options(options)?;
    let mut wrapped = *options;
    wrapped.packing = PackingPolicy::FitWidth { max_width };
    let original = WidthTrial {
        options: wrapped,
        turned: false,
        tight_groups: false,
        fold_at: None,
    };
    let solve = |graph: &Graph, options: &Options, expanded: &mut Expanded, work: &mut Work<'_>| {
        crate::solve(graph, options, expanded, detail, work)
    };
    let mut best = (solve(graph, &wrapped, expanded, work)?, original);
    if best.0.bounds.width <= max_width {
        return Ok(best);
    }

    let mut tight = wrapped;
    tight.spacing.node *= 0.5;
    tight.spacing.rank *= 0.75;
    let compact = if graph.groups.is_empty() {
        solve(graph, &tight, expanded, work)
    } else {
        let mut compact = copy_graph(graph, work)?;
        tighten_groups(&mut compact.groups, work)?;
        solve(&compact, &tight, expanded, work)
    };
    let compact_trial = WidthTrial {
        options: tight,
        tight_groups: true,
        ..original
    };
    let compact_kept = try_narrower(&mut best, compact, compact_trial)?;
    if best.0.bounds.width <= max_width {
        return Ok(best);
    }

    if allow_turn
        && max_width < turn_breakpoint
        && matches!(graph.direction, Direction::Right | Direction::Left)
    {
        let mut turned = copy_graph(graph, work)?;
        turn_down(&mut turned, &wrapped, work)?;
        wrapped.group_policy.inherit_crossed_direction = false;
        tight.group_policy.inherit_crossed_direction = false;
        let turn_trial = WidthTrial {
            options: wrapped,
            turned: true,
            ..original
        };
        let turned_kept = try_narrower(
            &mut best,
            solve(&turned, &wrapped, expanded, work),
            turn_trial,
        )?;
        if best.0.bounds.width <= max_width {
            return Ok(best);
        }
        tighten_groups(&mut turned.groups, work)?;
        let tight_turn_trial = WidthTrial {
            options: tight,
            turned: true,
            ..compact_trial
        };
        let tight_turn_kept = try_narrower(
            &mut best,
            solve(&turned, &tight, expanded, work),
            tight_turn_trial,
        )?;
        if fold_wide_ranks && best.0.bounds.width > max_width {
            let trial = if turned_kept && !tight_turn_kept {
                turn_trial
            } else {
                tight_turn_trial
            };
            try_fold(
                &mut best,
                &turned,
                WidthTrial {
                    tight_groups: true,
                    ..trial
                },
                expanded,
                max_width,
                detail,
                work,
            )?;
        }
    } else if fold_wide_ranks {
        let trial = if compact_kept {
            WidthTrial {
                tight_groups: false,
                ..compact_trial
            }
        } else {
            original
        };
        try_fold(&mut best, graph, trial, expanded, max_width, detail, work)?;
    }
    Ok(best)
}

fn try_fold(
    best: &mut WidthSolution,
    graph: &Graph,
    trial: WidthTrial,
    expanded: &mut Expanded,
    max_width: f64,
    detail: LayoutDetail,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    match flat::fold(
        graph,
        &trial.options,
        expanded,
        max_width,
        detail.flat(&trial.options),
        work,
    ) {
        Ok(Some(candidate)) if candidate.bounds.width < best.0.bounds.width => {
            *best = (
                candidate,
                WidthTrial {
                    fold_at: Some(max_width),
                    ..trial
                },
            );
        }
        Ok(_) | Err(LayoutError::InvalidInput(_)) => {}
        Err(error) => return Err(error),
    }
    Ok(())
}

fn try_narrower(
    best: &mut WidthSolution,
    candidate: LayoutResult<LayoutOutput>,
    trial: WidthTrial,
) -> LayoutResult<bool> {
    match candidate {
        Ok(drawing) if drawing.bounds.width < best.0.bounds.width => {
            *best = (drawing, trial);
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
        lane_order: copy(&graph.lane_order, work)?,
    })
}

fn turn_down(graph: &mut Graph, options: &Options, work: &mut Work<'_>) -> LayoutResult<()> {
    graph.direction = Direction::Down;
    let flat = graph.groups.is_empty();
    for node in &mut graph.nodes {
        work.visit(1)?;
        let primary = node.port_span;
        node.port_span = node.perpendicular_port_span.or(primary);
        node.perpendicular_port_span = primary.or_else(|| {
            node.port_span
                .map(|_| (node.size.height - 2.0 * options.spacing.corner_clearance).max(0.0))
        });
        if flat {
            node.align_flow(graph.direction);
        }
    }
    Ok(())
}

fn tighten_groups(groups: &mut [GroupSpec], work: &mut Work<'_>) -> LayoutResult<()> {
    for group in groups {
        work.visit(1)?;
        let title_height = group.title.map_or(0.0, |title| title.height);
        let [top, bottom] = match group.title_position {
            TitlePosition::TopAuto
            | TitlePosition::TopLeft
            | TitlePosition::TopCenter
            | TitlePosition::TopRight => [title_height, 0.0],
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
