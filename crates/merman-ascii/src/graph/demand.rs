//! Model-owned endpoint and label demand, before coordinates or route occupancy exist.

use super::label::try_measure_edge_label;
use super::model::{AsciiGraph, GraphDirection, GraphNodeShape};
use super::shape::GraphNodeShapeSize;
use crate::error::{AsciiError, Result};
use crate::options::GraphLayoutPolicy;
use crate::resource::ResourceContext;
use rustc_hash::FxHashMap as HashMap;

#[derive(Clone, Copy, Default)]
struct IncidentDemand {
    first: Option<usize>,
    count: usize,
    independent: bool,
}

#[derive(Clone, Copy, Default)]
struct EndpointDemand {
    incoming: IncidentDemand,
    outgoing: IncidentDemand,
    loops: usize,
    label_width: usize,
    label_height: usize,
}

#[derive(Default)]
pub(super) struct SceneDemand<'a> {
    indices: HashMap<&'a str, usize>,
    endpoints: Vec<EndpointDemand>,
    horizontal: bool,
    compound: bool,
}

impl<'a> SceneDemand<'a> {
    pub(super) fn try_new(
        graph: &'a AsciiGraph,
        policy: &GraphLayoutPolicy,
        resources: &ResourceContext,
    ) -> Result<Self> {
        resources.checkpoint()?;
        let count = resources.checked_work_add(graph.nodes.len(), graph.groups.len())?;
        resources.charge_layout_work_product(count, 2)?;
        let mut indices = HashMap::default();
        indices
            .try_reserve(count)
            .map_err(|_| allocation_failed())?;
        let mut endpoints = Vec::new();
        endpoints
            .try_reserve_exact(count)
            .map_err(|_| allocation_failed())?;
        endpoints.resize(count, EndpointDemand::default());
        // Match topology ownership: an authored group wins an ID collision with a node.
        for (index, id) in graph
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .chain(graph.groups.iter().map(|group| group.id.as_str()))
            .enumerate()
        {
            resources.checkpoint()?;
            resources.charge_layout_work(id.len())?;
            indices.insert(id, index);
        }
        resources.charge_layout_work(graph.edges.len())?;
        let mut pairs = Vec::new();
        pairs
            .try_reserve_exact(graph.edges.len())
            .map_err(|_| allocation_failed())?;
        for (index, edge) in graph.edges.iter().enumerate() {
            resources.checkpoint()?;
            resources
                .charge_layout_work(resources.checked_work_add(edge.from.len(), edge.to.len())?)?;
            let (Some(&from), Some(&to)) = (
                indices.get(edge.from.as_str()),
                indices.get(edge.to.as_str()),
            ) else {
                return Err(AsciiError::UnsupportedFeature {
                    diagram_type: graph.diagram_type(),
                    feature: "edges with missing semantic endpoints",
                });
            };
            if !edge.stroke.is_visible() {
                continue;
            }
            pairs.push((from.min(to), from.max(to), from, to));
            for (endpoint, source) in [(from, true), (to, false)] {
                let demand = &mut endpoints[endpoint];
                let incident = if source {
                    &mut demand.outgoing
                } else {
                    &mut demand.incoming
                };
                if let Some(first) = incident.first {
                    let previous = &graph.edges[first];
                    incident.independent |= edge.stroke != previous.stroke
                        || edge.style != previous.style
                        || if source {
                            edge.start_marker != previous.start_marker
                        } else {
                            edge.end_marker != previous.end_marker
                        };
                } else {
                    incident.first = Some(index);
                }
                incident.count = resources.checked_work_add(incident.count, 1)?;
            }
            if from == to {
                endpoints[from].loops = resources.checked_work_add(endpoints[from].loops, 1)?;
                // BranchAllocation blocks both endpoint groups at a visible self-loop.
                endpoints[from].incoming.independent = true;
                endpoints[from].outgoing.independent = true;
            }
            if let Some(raw) = &edge.label
                && let Some(label) = try_measure_edge_label(
                    raw,
                    policy.terminal_width_profile,
                    resources,
                    edge.label_break_policy,
                )?
            {
                for endpoint in [from, to] {
                    endpoints[endpoint].label_width =
                        endpoints[endpoint].label_width.max(label.max_width);
                    endpoints[endpoint].label_height =
                        endpoints[endpoint].label_height.max(label.line_count);
                }
            }
        }
        super::layout::charge_sort_work(pairs.len(), resources)?;
        pairs.sort_unstable();
        for pair in pairs.windows(2) {
            resources.checkpoint()?;
            resources.charge_layout_work(1)?;
            if pair[0].0 == pair[1].0 && pair[0].1 == pair[1].1 {
                for &(_, _, from, to) in pair {
                    endpoints[from].outgoing.independent = true;
                    endpoints[to].incoming.independent = true;
                }
            }
        }
        Ok(Self {
            indices,
            endpoints,
            horizontal: graph.direction.canonical() == GraphDirection::LeftRight,
            compound: !graph.groups.is_empty(),
        })
    }

    pub(super) fn size_for_node(
        &self,
        id: &str,
        shape: GraphNodeShape,
        mut size: GraphNodeShapeSize,
        resources: &ResourceContext,
    ) -> Result<GraphNodeShapeSize> {
        let Some(&index) = self.indices.get(id) else {
            return Ok(size);
        };
        let demand = self.endpoints[index];
        let contacts = self.contacts_for_node(id, resources)?;
        // Fixed State pseudostates and bars retain their own contour contract. Text nodes
        // have no spare outline to enlarge. Other contours need non-corner contacts and
        // one reading cell between independent marked terminals.
        if !matches!(
            shape,
            GraphNodeShape::Text
                | GraphNodeShape::StateStart
                | GraphNodeShape::StateEnd
                | GraphNodeShape::Choice
                | GraphNodeShape::ForkJoinHorizontal
                | GraphNodeShape::ForkJoinVertical
        ) {
            let base_size = size;
            let span = resources.checked_grid_add(resources.checked_grid_mul(contacts, 2)?, 1)?;
            // Local directions and cross-container edges can use either axis. Reserve
            // both before compound layout chooses their physical endpoint sides.
            if self.horizontal || self.compound {
                size.height = size.height.max(span);
            }
            if !self.horizontal || self.compound {
                size.width = size.width.max(span);
            }
            if !matches!(shape, GraphNodeShape::Rect | GraphNodeShape::StateWithTitle) {
                // Curved/decorated outlines need shoulders above and below the label.
                size.height = size.height.max(5);
            }
            if demand.loops > 0 || demand.incoming.independent || demand.outgoing.independent {
                size.height = size.height.max(5);
            }
            if matches!(shape, GraphNodeShape::LeanLeft | GraphNodeShape::LeanRight) {
                // Match shape sizing: each extra row consumes another column of slant.
                // Preserve the original label interior without adding slant twice to
                // width already reserved for independent top/bottom contacts.
                size.width = size.width.max(
                    resources.checked_grid_add(base_size.width, size.height - base_size.height)?,
                );
            }
        }
        resources.grid_extent(size.width, size.height)?;
        Ok(size)
    }

    pub(super) fn contacts_for_node(&self, id: &str, resources: &ResourceContext) -> Result<usize> {
        let Some(&index) = self.indices.get(id) else {
            return Ok(0);
        };
        let demand = self.endpoints[index];
        let pressure = |incident: IncidentDemand| {
            // Sharing is admitted only for compatible flat branches. Parallel, reciprocal,
            // compound, and loop endpoints retain their independent semantic demand.
            if self.compound || incident.independent {
                incident.count
            } else {
                usize::from(incident.count > 0)
            }
        };
        Ok(pressure(demand.incoming)
            .max(pressure(demand.outgoing))
            .max(resources.checked_grid_mul(demand.loops, 2)?))
    }

    pub(super) fn cross_extent(
        &self,
        id: &str,
        node_extent: usize,
        resources: &ResourceContext,
    ) -> Result<usize> {
        let Some(&index) = self.indices.get(id) else {
            return Ok(node_extent);
        };
        let demand = self.endpoints[index];
        let label = if self.horizontal {
            demand.label_height
        } else {
            demand.label_width
        };
        Ok(node_extent.max(resources.checked_grid_add(label, usize::from(label > 0))?))
    }

    pub(super) fn group_clearance(&self, id: &str, resources: &ResourceContext) -> Result<isize> {
        let Some(&index) = self.indices.get(id) else {
            return Ok(1);
        };
        let demand = self.endpoints[index];
        let clearance = if demand.loops > 0 {
            // Parallel exterior returns need distinct turns, their complete label block,
            // and reading clearance before the next physical envelope.
            resources.checked_grid_add(
                resources.checked_grid_mul(demand.loops, 2)?,
                resources.checked_grid_add(demand.label_height, 2)?,
            )?
        } else if demand.incoming.count > 0 || demand.outgoing.count > 0 {
            3
        } else {
            1
        };
        isize::try_from(clearance).map_err(|_| resources.grid_overflow())
    }
}

fn allocation_failed() -> AsciiError {
    AsciiError::allocation_failed("ascii_scene_demand")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::label::{GRAPH_LABEL_LINE_GAP, GraphLabel};
    use crate::graph::layout::{GridCoord, NodeLayout};
    use crate::graph::model::{GraphEdgeMarker, GraphEdgeStroke, GraphGroupStyle};
    use crate::graph::shape::GraphNodeShapeSemantics;
    use crate::resource::AsciiResourceLimitCause;
    use crate::{AsciiRenderOptions, AsciiResourceLimitId, AsciiResourcePolicy};
    use merman_model::{CancelReason, OperationControl, OperationPhase};

    fn graph() -> AsciiGraph {
        let mut graph = AsciiGraph::new(GraphDirection::LeftRight);
        for id in ["hub", "a", "b", "c"] {
            graph.add_node(id, id);
        }
        for id in ["a", "b", "c"] {
            graph.add_edge("hub", id);
        }
        graph
    }

    #[test]
    fn compatible_branches_need_one_contact_but_mixed_markers_and_strokes_need_private_contacts() {
        let policy = AsciiRenderOptions::ascii()
            .flowchart_layout()
            .graph_policy();
        for mismatch in [
            None,
            Some(GraphEdgeStroke::Dotted),
            Some(GraphEdgeStroke::Normal),
        ] {
            let mut graph = graph();
            if let Some(stroke) = mismatch {
                graph.edges[2].stroke = stroke;
                if stroke == GraphEdgeStroke::Normal {
                    graph.edges[2].start_marker = GraphEdgeMarker::Circle;
                }
            }
            let resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
            let demand = SceneDemand::try_new(&graph, &policy, &resources).unwrap();
            let size = demand
                .size_for_node(
                    "hub",
                    GraphNodeShape::Rect,
                    GraphNodeShapeSize {
                        width: 7,
                        height: 3,
                    },
                    &resources,
                )
                .unwrap();
            assert_eq!(size.height, if mismatch.is_none() { 3 } else { 7 });
        }
    }

    #[test]
    fn lean_contours_retain_the_label_interior_when_contact_demand_increases_height() {
        let policy = AsciiRenderOptions::ascii()
            .flowchart_layout()
            .graph_policy();
        for direction in [GraphDirection::LeftRight, GraphDirection::TopDown] {
            let mut graph = graph();
            graph.direction = direction;
            graph.edges[2].start_marker = GraphEdgeMarker::Circle;
            let resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
            let demand = SceneDemand::try_new(&graph, &policy, &resources).unwrap();
            for shape in [GraphNodeShape::LeanLeft, GraphNodeShape::LeanRight] {
                let semantics = GraphNodeShapeSemantics::new(shape);
                for raw in ["ABCDE", "ABCDE\nFGHIJ"] {
                    let label = GraphLabel::new(raw);
                    let base = semantics
                        .try_size_for_label_metrics(
                            label.width(),
                            label.content_height(),
                            &policy,
                            &resources,
                        )
                        .unwrap();
                    let size = demand
                        .size_for_node("hub", shape, base, &resources)
                        .unwrap();
                    assert!(size.width - size.height >= base.width - base.height);
                    let node = NodeLayout {
                        id: "hub".into(),
                        label,
                        shape,
                        style: Default::default(),
                        grid: GridCoord { x: 0, y: 0 },
                        x: 4,
                        y: 6,
                        width: size.width,
                        height: size.height,
                    };
                    let first_row =
                        node.y + 1 + (node.height - 2 - node.label.content_height()) / 2;
                    for (index, line) in node.label.lines().iter().enumerate() {
                        let y = first_row + index * (GRAPH_LABEL_LINE_GAP + 1);
                        let (left, right) = semantics.horizontal_span(&node, y).unwrap();
                        assert!(
                            right - left > node.label.line_width(line) + policy.node_padding_x * 2,
                            "{shape:?} {direction:?}: label row must retain its padded interior: {node:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn compound_scenes_keep_private_capacity_on_both_axes_including_nested_local_directions() {
        let policy = AsciiRenderOptions::ascii()
            .flowchart_layout()
            .graph_policy();
        for direction in [
            GraphDirection::LeftRight,
            GraphDirection::RightLeft,
            GraphDirection::TopDown,
            GraphDirection::BottomTop,
        ] {
            for merging in [false, true] {
                for nested in [false, true] {
                    let mut graph = AsciiGraph::new(direction);
                    graph.add_node("hub", "H");
                    for index in 0..9 {
                        let target = format!("target-{index}");
                        graph.add_node(&target, "T");
                        if merging {
                            graph.add_edge(&target, "hub");
                            graph.edges[index].end_marker = GraphEdgeMarker::Circle;
                        } else {
                            graph.add_edge("hub", &target);
                            graph.edges[index].start_marker = GraphEdgeMarker::Circle;
                        }
                    }
                    let local_direction = if direction.canonical() == GraphDirection::LeftRight {
                        GraphDirection::TopDown
                    } else {
                        GraphDirection::LeftRight
                    };
                    // Even a group outside the hub disables flat branch allocation.
                    let members = if nested {
                        graph.nodes.iter().map(|node| node.id.clone()).collect()
                    } else {
                        vec!["target-0".into()]
                    };
                    graph.add_group_with_style(
                        "inner",
                        "Inner",
                        Some(local_direction),
                        members,
                        GraphGroupStyle::default(),
                    );
                    if nested {
                        graph.add_group_with_style(
                            "outer",
                            "Outer",
                            Some(direction),
                            vec!["inner".into()],
                            GraphGroupStyle::default(),
                        );
                    }
                    let resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
                    let demand = SceneDemand::try_new(&graph, &policy, &resources).unwrap();
                    let shape = GraphNodeShape::Rect;
                    let base = GraphNodeShapeSemantics::new(shape)
                        .try_size_for_label_metrics(1, 1, &policy, &resources)
                        .unwrap();
                    let size = demand
                        .size_for_node("hub", shape, base, &resources)
                        .unwrap();
                    for extent in [size.width, size.height] {
                        assert!(
                            (extent - 1) / 2 >= graph.edges.len(),
                            "each marked branch needs a non-corner contact and a reading cell: {direction:?}, merging={merging}, nested={nested}, {size:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn parallel_edges_reciprocal_edges_and_loops_cannot_hide_contact_pressure() {
        let policy = AsciiRenderOptions::ascii()
            .flowchart_layout()
            .graph_policy();
        for (from, to) in [("hub", "a"), ("a", "hub"), ("hub", "hub")] {
            let mut graph = graph();
            graph.add_edge(from, to);
            let resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
            let demand = SceneDemand::try_new(&graph, &policy, &resources).unwrap();
            let size = demand
                .size_for_node(
                    "hub",
                    GraphNodeShape::Rect,
                    GraphNodeShapeSize {
                        width: 7,
                        height: 3,
                    },
                    &resources,
                )
                .unwrap();
            let private_outgoing = graph.edges.iter().filter(|edge| edge.from == "hub").count();
            assert!(
                (size.height - 1) / 2 >= private_outgoing,
                "{from} -> {to}: every visible outgoing edge needs a private contact"
            );
        }
    }

    #[test]
    fn missing_endpoints_fail_before_geometry_and_invisible_constraints_still_require_identity() {
        let mut graph = graph();
        graph.edges[0].to = "missing".into();
        graph.edges[0].stroke = GraphEdgeStroke::Invisible;
        let resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
        let policy = AsciiRenderOptions::ascii()
            .flowchart_layout()
            .graph_policy();
        assert!(matches!(
            SceneDemand::try_new(&graph, &policy, &resources),
            Err(AsciiError::UnsupportedFeature {
                feature: "edges with missing semantic endpoints",
                ..
            })
        ));
    }

    #[test]
    fn demand_admits_exact_work_and_rejects_one_unit_less() {
        let graph = graph();
        let policy = AsciiRenderOptions::ascii()
            .flowchart_layout()
            .graph_policy();
        let resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
        SceneDemand::try_new(&graph, &policy, &resources).unwrap();
        let work = resources.layout_work_used();
        for (limit, admitted) in [(work, true), (work - 1, false)] {
            let resources = ResourceContext::new(
                AsciiResourcePolicy::unbounded()
                    .with_limit(AsciiResourceLimitId::MaxLayoutWorkUnits, limit)
                    .unwrap(),
            );
            assert_eq!(
                SceneDemand::try_new(&graph, &policy, &resources).is_ok(),
                admitted
            );
        }
    }

    #[test]
    fn grown_lean_contours_obey_exact_grid_limits_and_checked_width_growth() {
        let mut graph = graph();
        graph.edges[2].start_marker = GraphEdgeMarker::Circle;
        let policy = AsciiRenderOptions::ascii()
            .flowchart_layout()
            .graph_policy();
        let resources = ResourceContext::new(AsciiResourcePolicy::unbounded());
        let demand = SceneDemand::try_new(&graph, &policy, &resources).unwrap();
        let shape = GraphNodeShape::LeanRight;
        let base = GraphNodeShapeSemantics::new(shape)
            .try_size_for_label_metrics(5, 1, &policy, &resources)
            .unwrap();
        let grown = demand
            .size_for_node("hub", shape, base, &resources)
            .unwrap();
        let cells = grown.width * grown.height;
        for (limit, admitted) in [(cells, true), (cells - 1, false)] {
            let resources = ResourceContext::new(
                AsciiResourcePolicy::unbounded()
                    .with_limit(AsciiResourceLimitId::MaxGridCells, limit)
                    .unwrap(),
            );
            let result = demand.size_for_node("hub", shape, base, &resources);
            if admitted {
                assert_eq!(result.unwrap(), grown);
            } else {
                assert!(
                    matches!(result, Err(AsciiError::ResourceLimitExceeded(details))
                    if details.limit == AsciiResourceLimitId::MaxGridCells
                        && details.actual == cells && details.max == limit)
                );
            }
        }
        let result = demand.size_for_node(
            "hub",
            shape,
            GraphNodeShapeSize {
                width: usize::MAX,
                height: base.height,
            },
            &resources,
        );
        assert!(
            matches!(result, Err(AsciiError::ResourceLimitExceeded(details))
            if details.limit == AsciiResourceLimitId::MaxGridCells
                && details.cause == AsciiResourceLimitCause::ArithmeticOverflow)
        );
    }

    #[test]
    fn cancellation_precedes_demand_work_and_interrupts_endpoint_scanning() {
        let graph = graph();
        let policy = AsciiRenderOptions::ascii()
            .flowchart_layout()
            .graph_policy();
        for during_scan in [false, true] {
            let control = OperationControl::new();
            if during_scan {
                control.cancel_after_checkpoints(20);
            } else {
                control.cancel();
            }
            let resources = ResourceContext::new(AsciiResourcePolicy::unbounded())
                .controlled(control, OperationPhase::Layout);
            let result = SceneDemand::try_new(&graph, &policy, &resources);
            assert!(matches!(result, Err(AsciiError::Cancelled(cancelled))
                if cancelled.phase == OperationPhase::Layout
                    && cancelled.reason == CancelReason::Requested));
            assert_eq!(resources.layout_work_used() > 0, during_scan);
        }
    }
}
