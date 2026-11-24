//! Solve each container in its own drawing coordinates and join its routes at shared border ports.

mod assemble;
mod hierarchy;
mod pre_order;
mod refine;
mod seam_relax;
mod sides;
mod solve;

pub(super) use refine::crossings;
#[cfg(test)]
mod tests;

use crate::flat::Expanded;
use crate::work::{LayoutResult, Work, filled, reserved};
use crate::{Endpoint, Graph, LayoutOutput, Options, Side, WorkError};
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
        result.stats.routed_crossings =
            refine::crossings(ctx.graph, &result, ctx.options.routing, work)?;
        Ok(result)
    }
}

pub(super) fn run(
    graph: &Graph,
    options: &Options,
    expanded: &mut Expanded,
    work: &mut Work<'_>,
) -> LayoutResult<LayoutOutput> {
    let hierarchy = hierarchy::build(graph, options, work)?;
    let sides = sides::assign(graph, &hierarchy, options, work)?;
    let ordered = pre_order::ordered(graph, &hierarchy, options, &sides, expanded, work)?;
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
        hierarchy: &hierarchy,
        options,
        sides: &sides,
        items: &no_keys,
        ports: &no_hints,
    };
    let first = Pass {
        items: ordered.as_ref().map_or(&no_keys, |h| &h.items),
        ports: ordered.as_ref().map_or(&no_hints, |h| &h.ports),
        ..independent
    };
    let mut frames = Frames::new(hierarchy.levels.len())?;
    let mut output = 'selected: {
        let first_output = frames.draw(&first, None, expanded, work)?;
        let crossings = first_output.stats.routed_crossings;
        if crossings == 0
            || hierarchy.borders.len() < 2
            || hierarchy.borders.len() > options.caps.max_ordered_compound_ports
            || graph.edges.len() > options.caps.max_ordered_compound_edges
        {
            break 'selected first_output;
        }
        let suggestions = refine::port_hints(&first, &frames.levels, &first_output, work)?;
        let mut previous = &first;
        if ordered.as_ref().is_some_and(|h| {
            crossings
                > h.flat_crossings
                    .saturating_add(h.flat_crossings / 2)
                    .saturating_add(4)
        }) {
            let alternative = frames.draw(&independent, Some(&first), expanded, work)?;
            if alternative.stats.routed_crossings < crossings {
                break 'selected alternative;
            }
            previous = &independent;
        }
        if let Some(ports) = suggestions {
            let adjusted = Pass {
                ports: &ports,
                ..first
            };
            let alternative = frames.draw(&adjusted, Some(previous), expanded, work)?;
            if alternative.stats.routed_crossings < crossings {
                break 'selected alternative;
            }
        }
        first_output
    };
    seam_relax::relax(&first, &mut output, work)?;
    Ok(output)
}
