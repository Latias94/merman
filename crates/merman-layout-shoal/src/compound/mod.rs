//! Solve each container in its own drawing coordinates and join its routes at shared border ports.

mod assemble;
mod hierarchy;
mod pre_order;
mod refine;
mod seam_relax;
mod sides;
mod solve;

pub(super) use hierarchy::authored_directions;
pub(crate) use refine::PairSet;
pub(super) use refine::crossings;
#[cfg(test)]
mod tests;

use crate::flat::Expanded;
use crate::work::{LayoutResult, Work, filled, reserved};
use crate::{
    Endpoint, Graph, LayoutDetail, LayoutOutput, Options, RoutingStyle, Side, WidthPolicy,
    WorkError,
};
use hierarchy::Hierarchy;
use solve::Frame;

struct Pass<'a> {
    graph: &'a Graph,
    hierarchy: &'a Hierarchy,
    options: &'a Options,
    sides: &'a [Side],
    items: &'a [Option<f64>],
    ports: &'a [Option<f64>],
}

fn item_index(graph: &Graph, item: Endpoint) -> usize {
    match item {
        Endpoint::Node(id) => id.0 as usize,
        Endpoint::Group(id) => graph.nodes.len() + id.0 as usize,
    }
}

struct Frames {
    levels: Vec<Option<Frame>>,
    dummy_caps: Vec<usize>,
}

impl Frames {
    fn new(count: usize) -> LayoutResult<Self> {
        let mut levels = reserved(count)?;
        levels.resize_with(count, || None);
        Ok(Self {
            levels,
            dummy_caps: filled(count, 0)?,
        })
    }

    fn draw(
        &mut self,
        ctx: &Pass<'_>,
        previous: Option<&Pass<'_>>,
        expanded: &mut Expanded,
        crossing_limit: Option<u64>,
        work: &mut Work<'_>,
    ) -> LayoutResult<LayoutOutput> {
        let mut remaining = ctx.options.caps.max_dummies;
        let mut changed = filled(self.levels.len(), false)?;
        for &id in ctx.hierarchy.order.iter().rev() {
            let level = &ctx.hierarchy.levels[id];
            work.visit(level.items.len() + level.ports.len() + 1)?;
            let reusable = self.levels[id].as_ref().is_some_and(|frame| {
                (self.dummy_caps[id] == remaining
                    || (frame.stats.direct_fallback_edges == 0
                        && frame.stats.dummy_nodes <= remaining))
                    && previous.is_some_and(|previous| {
                        level.items.iter().all(|&item| {
                            let key = item_index(ctx.graph, item);
                            ctx.items[key] == previous.items[key]
                                && !matches!(item, Endpoint::Group(child) if changed[child.0 as usize])
                        }) && level
                            .ports
                            .iter()
                            .all(|&port| ctx.ports[port] == previous.ports[port])
                    })
            });
            if !reusable {
                let mut options = *ctx.options;
                options.caps.max_dummies = remaining;
                // The local frame is measured before sibling titles and cross-group legs exist.
                options.inline_labels = false;
                let local = Pass {
                    options: &options,
                    ..*ctx
                };
                let frame = solve::level(id, &local, &self.levels, expanded, work)?;
                changed[id] = self.levels[id]
                    .as_ref()
                    .is_none_or(|old| old.bounds != frame.bounds || old.ports != frame.ports);
                self.levels[id] = Some(frame);
                self.dummy_caps[id] = remaining;
            }
            let frame = self.levels[id].as_ref().expect("solved level");
            remaining = remaining
                .checked_sub(frame.stats.dummy_nodes)
                .ok_or(WorkError::ArithmeticOverflow)?;
        }
        let mut result = assemble::drawing(ctx, &self.levels, work)?;
        // Optional seam edits borrow their allowance from the work already spent.
        let crossing_limit = crossing_limit.or_else(|| {
            (work.editor_choice
                || (ctx.hierarchy.lanes.is_none() && seam_relax::eligible(ctx.options, &result)))
            .then_some(u64::MAX)
        });
        if let Some(limit) = crossing_limit {
            result.stats.routed_crossings =
                refine::count_crossings(ctx.graph, &result, ctx.options.routing, limit, work)?;
        }
        Ok(result)
    }
}

pub(super) struct Topology {
    hierarchy: Hierarchy,
    sides: Vec<Side>,
}

impl Topology {
    pub(super) fn new(graph: &Graph, options: &Options, work: &mut Work<'_>) -> LayoutResult<Self> {
        let hierarchy = hierarchy::build(graph, options, work)?;
        let sides = sides::assign(graph, &hierarchy, options, work)?;
        Ok(Self { hierarchy, sides })
    }
}

pub(super) fn run(
    graph: &Graph,
    options: &Options,
    expanded: &mut Expanded,
    detail: LayoutDetail,
    work: &mut Work<'_>,
) -> LayoutResult<LayoutOutput> {
    run_with_topology(
        graph,
        options,
        &Topology::new(graph, options, work)?,
        expanded,
        detail,
        work,
    )
}

pub(super) fn run_with_topology(
    graph: &Graph,
    options: &Options,
    topology: &Topology,
    expanded: &mut Expanded,
    detail: LayoutDetail,
    work: &mut Work<'_>,
) -> LayoutResult<LayoutOutput> {
    let Topology { hierarchy, sides } = topology;
    let order_ports = hierarchy.lanes.is_none()
        && (2..=options.caps.max_ordered_compound_ports).contains(&hierarchy.borders.len())
        && graph.edges.len() <= options.caps.max_ordered_compound_edges;
    let ordered = if order_ports {
        pre_order::ordered(graph, hierarchy, options, sides, expanded, work)?
    } else {
        None
    };
    let item_count = graph
        .nodes
        .len()
        .checked_add(graph.groups.len())
        .ok_or(WorkError::ArithmeticOverflow)?;
    work.charge(
        item_count
            .checked_add(hierarchy.borders.len())
            .ok_or(WorkError::ArithmeticOverflow)?,
    )?;
    let no_keys = filled(item_count, None)?;
    let no_hints = filled(hierarchy.borders.len(), None)?;
    let independent = Pass {
        graph,
        hierarchy,
        options,
        sides,
        items: &no_keys,
        ports: &no_hints,
    };
    let first = Pass {
        items: ordered.as_ref().map_or(&no_keys, |h| &h.items),
        ports: ordered.as_ref().map_or(&no_hints, |h| &h.ports),
        ..independent
    };
    let mut frames = Frames::new(hierarchy.levels.len())?;
    let measure_crossings = detail == LayoutDetail::Full
        || order_ports
        || (options.routing == RoutingStyle::Orthogonal && options.global_orthogonal);
    let mut output = frames.draw(
        &first,
        None,
        expanded,
        measure_crossings.then_some(u64::MAX),
        work,
    )?;
    let mut baseline = &first;
    let mut drawn_independently = false;
    let crossings = output.stats.routed_crossings;
    if order_ports
        && ordered.as_ref().is_some_and(|h| {
            crossings
                > h.flat_crossings
                    .saturating_add(h.flat_crossings / 2)
                    .saturating_add(4)
        })
    {
        let alternative =
            frames.draw(&independent, Some(&first), expanded, Some(crossings), work)?;
        drawn_independently = true;
        if alternative.stats.routed_crossings < crossings {
            output = alternative;
            baseline = &independent;
        }
    }
    if order_ports {
        let mut port_keys: Option<Vec<Option<f64>>> = None;
        for _ in 0..hierarchy.depth.min(4) {
            let crossings = output.stats.routed_crossings;
            if crossings == 0 {
                break;
            }
            let current = Pass {
                ports: port_keys.as_deref().unwrap_or(baseline.ports),
                ..*baseline
            };
            let Some(ports) = refine::port_hints(&current, &output, work)? else {
                break;
            };
            let adjusted = Pass {
                ports: &ports,
                ..current
            };
            // The cache follows the last trial even when its drawing was rejected.
            let previous = if drawn_independently {
                &independent
            } else {
                &current
            };
            let alternative =
                frames.draw(&adjusted, Some(previous), expanded, Some(crossings), work)?;
            if alternative.stats.routed_crossings >= crossings {
                break;
            }
            output = alternative;
            port_keys = Some(ports);
            drawn_independently = false;
        }
    }
    if options.width_policy == WidthPolicy::Natural {
        relax_seams(graph, options, &mut output, work)?;
    }
    Ok(output)
}

pub(super) fn relax_seams(
    graph: &Graph,
    options: &Options,
    output: &mut LayoutOutput,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    if graph.lane_order.is_empty() {
        seam_relax::relax(graph, options, output, work)?;
    }
    Ok(())
}
