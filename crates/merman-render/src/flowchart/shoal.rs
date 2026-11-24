//! Project visible Flowchart and Agentflow occurrences into Shoal's measured graph.

mod groups;

use super::measured::{FlowchartMeasurement, MeasuredNode as LeafSize};
use super::{
    FlowchartConfigView, FlowchartEdgeKey, FlowchartEdgeOwners, FlowchartRenderModelRef,
    FlowchartSvgLabelOwner, FlowchartSvgLabelSidecarBuilder,
};
use crate::graph_layout::contour::ShapePortProfile;
use crate::graph_layout::ir::{
    Axis, Direction, Endpoint, MeasuredEdge, MeasuredGraph, MeasuredNode, RoutingStyle, Side, Size,
    SolvedGraph,
};
use crate::graph_layout::route_paint::{Clearance, ObstacleOwner};
use crate::graph_layout::{RouteAxes, RouteContract, shoal_bridge};
use crate::layout_frame::{FrameDirection, ShoalFrameNode};
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::{
    Bounds, FlowchartLayout, FlowchartPlacement, LayoutCluster, LayoutEdge, LayoutLabel,
    LayoutNode, LayoutPoint,
};
use crate::{
    Error, LayoutAdaptation, LayoutAdaptationObservation, LayoutExecution, Result, ShoalLayoutFrame,
};
use merman_model::families::flowchart::{FlowchartModel, FlowchartRenderContext};
use merman_model::presentation::PresentationConfig;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

fn paint_clearance(
    nodes: &[LayoutNode],
    clusters: &[LayoutCluster],
    edges: &[LayoutEdge],
    owners: &[FlowchartEdgeKey],
    work: &mut OperationLayoutWorkControl,
) -> Result<Clearance> {
    if edges.is_empty() {
        return Ok(Clearance::default());
    }
    let shapes = work.checked_add(nodes.len(), clusters.len())?;
    work.charge_adapter(work.checked_add(shapes, edges.len())?)?;
    let mut boxes = Vec::with_capacity(shapes);
    let label_box = |label: &LayoutLabel| {
        kurbo::Rect::from_center_size((label.x, label.y), (label.width, label.height))
    };
    for (index, node) in nodes.iter().enumerate() {
        boxes.push((
            kurbo::Rect::from_center_size((node.x, node.y), (node.width, node.height)),
            ObstacleOwner::Node(index as u32),
        ));
    }
    for (index, group) in clusters.iter().enumerate() {
        boxes.push((
            label_box(&group.title_label),
            ObstacleOwner::GroupTitle(index as u32),
        ));
    }
    for (edge, owner) in edges.iter().zip(owners) {
        for label in [
            &edge.label,
            &edge.start_label_left,
            &edge.start_label_right,
            &edge.end_label_left,
            &edge.end_label_right,
        ]
        .into_iter()
        .flatten()
        {
            boxes.push((
                label_box(label),
                ObstacleOwner::EdgeLabel(owner.semantic_index()),
            ));
        }
    }
    let levels = boxes.len().checked_ilog2().unwrap_or(0) as usize + 1;
    work.charge_adapter(work.checked_mul(boxes.len(), levels)?)?;
    Ok(Clearance::new(boxes))
}

pub(crate) fn layout(
    model: &FlowchartModel,
    render_context: &FlowchartRenderContext,
    class_defs: &super::FlowchartClassDefinitions<'_>,
    config: &PresentationConfig,
    execution: &LayoutExecution<'_>,
    labels: Option<&FlowchartSvgLabelSidecarBuilder>,
    edge_styles: &crate::svg::FlowchartEdgeStylePlan,
) -> Result<FlowchartLayout> {
    let mut work = OperationLayoutWorkControl::new(execution.work_meter());
    work.charge_adapter(0)?;
    let view = FlowchartRenderModelRef::new(model, render_context);
    let measurement = FlowchartMeasurement::new(
        view,
        class_defs,
        config,
        execution.text_measurer(),
        execution.math_renderer(),
        labels,
        edge_styles,
    );
    let fit_width = execution.adaptation == LayoutAdaptation::FitWidth;
    let (graph, bindings) = measure(view, &measurement, config, fit_width, &mut work)?;
    let width = if fit_width {
        shoal_bridge::WidthPolicy::FitWidth {
            max_width: execution.container_width,
            allow_turn: true,
            turn_breakpoint: 640.0,
            fold_wide_ranks: execution.fold_wide_ranks,
        }
    } else {
        shoal_bridge::WidthPolicy::Natural
    };
    let hints = match execution.previous_shoal_frame.as_deref() {
        Some(previous) if previous.family == execution.family_id().as_str() => {
            bindings.frame_hints(view, &graph, previous, &mut work)?
        }
        _ => Vec::new(),
    };
    let solved = shoal_bridge::solve(
        &graph,
        shoal_bridge::Placement {
            width,
            capture_ranks: execution.capture_shoal_frame,
            hints: &hints,
            flow_relative_nodes: &bindings.flow_relative_nodes,
        },
        &mut work,
    )?;
    let placement = FlowchartPlacement {
        adaptation: fit_width.then(|| {
            LayoutAdaptationObservation::fit_width(execution.container_width, solved.bounds.width)
        }),
        frame: if execution.capture_shoal_frame {
            Some(Arc::new(bindings.capture_frame(
                view,
                &graph,
                &solved,
                execution.family_id().as_str(),
                &mut work,
            )?))
        } else {
            None
        },
    };
    drop(graph);
    bindings.project(view, &measurement, solved, placement, &mut work)
}

/// Node order matches the measured graph. Edge keys retain the semantic occurrence even when
/// projection hides an edge or distinct occurrences share a public ID.
struct FlowchartBindings {
    nodes: Vec<LayoutNode>,
    profiles: Vec<ShapePortProfile>,
    groups: Vec<usize>,
    flow_relative_nodes: Vec<(usize, Axis)>,
}

fn direction(value: &str) -> Result<Direction> {
    match value.trim().to_ascii_uppercase().as_str() {
        "LR" => Ok(Direction::Right),
        "RL" => Ok(Direction::Left),
        "BT" => Ok(Direction::Up),
        "TB" | "TD" => Ok(Direction::Down),
        other => Err(Error::InvalidModel {
            message: format!("Unsupported Shoal Flowchart direction `{other}`"),
        }),
    }
}

fn direction_name(dir: Direction) -> &'static str {
    match dir {
        Direction::Down => "TB",
        Direction::Up => "BT",
        Direction::Left => "RL",
        Direction::Right => "LR",
    }
}

fn index(value: usize, work: &OperationLayoutWorkControl) -> Result<u32> {
    u32::try_from(value).map_err(|_| work.record_arithmetic_overflow().into())
}

fn layout_node(id: &str, size: LeafSize) -> LayoutNode {
    LayoutNode {
        id: id.to_owned(),
        x: 0.0,
        y: 0.0,
        width: size.width,
        height: size.height,
        is_cluster: false,
        label_width: Some(size.label.width),
        label_height: Some(size.label.height),
    }
}

fn measure(
    view: FlowchartRenderModelRef<'_>,
    measurement: &FlowchartMeasurement<'_>,
    config: &PresentationConfig,
    fit_width: bool,
    work: &mut OperationLayoutWorkControl,
) -> Result<(MeasuredGraph, FlowchartBindings)> {
    let direction = direction(view.direction.as_deref().unwrap_or("TB"))?;
    work.charge_adapter(view.nodes.len())?;
    work.charge_adapter(view.edges.len())?;
    let config_view = FlowchartConfigView::new(config);
    let curve = config_view.effective_edge_interpolation(&view, false);
    let look_is_neo = crate::config::mermaid_config_diagram_look(config).is_neo();
    let theme_radius = crate::config::config_f64(config, &["themeVariables", "radius"])
        .map(|r| (r.max(0.), r.max(0.)));
    let process_radius = (look_is_neo
        && config.config_path_overrides_typed_default("themeVariables.radius"))
    .then_some(theme_radius)
    .flatten();
    let mut graph = MeasuredGraph {
        direction,
        routing: match curve {
            "rounded" | "step" | "stepBefore" | "stepAfter" => RoutingStyle::Orthogonal,
            "linear" => RoutingStyle::Polyline,
            _ => RoutingStyle::Curved,
        },
        node_spacing: measurement.settings.nodesep,
        rank_spacing: measurement.settings.ranksep,
        component_spacing: measurement.settings.nodesep,
        inherit_crossed_group_direction: true,
        nodes: Vec::with_capacity(view.nodes.len()),
        groups: Vec::new(),
        edges: Vec::with_capacity(view.edges.len()),
    };
    let mut endpoints =
        HashMap::with_capacity(work.checked_add(view.nodes.len(), view.subgraphs.len())?);
    let groups = groups::measure(view, measurement, config, &mut graph, &mut endpoints, work)?;
    let mut bindings = FlowchartBindings {
        nodes: Vec::with_capacity(view.nodes.len()),
        profiles: Vec::with_capacity(view.nodes.len()),
        groups: groups.declarations,
        flow_relative_nodes: Vec::new(),
    };
    for (ordinal, node) in view.nodes.iter().enumerate() {
        if matches!(endpoints.get(node.id.as_str()), Some(Endpoint::Group(_)))
            || view.collapsed_replacement(&node.id).is_some()
            || view.is_subgraph_collapsed(&node.id)
        {
            continue;
        }
        work.charge_adapter(node.id.len())?;
        let seq = index(ordinal, work)?;
        if endpoints
            .insert(
                node.id.as_str(),
                Endpoint::Node(index(graph.nodes.len(), work)?),
            )
            .is_some()
        {
            return Err(Error::InvalidModel {
                message: format!("Duplicate Shoal Flowchart node `{}`", node.id),
            });
        }
        super::validate_flowchart_node_asset_dimensions(node)?;
        let parent = groups.parents.get(node.id.as_str()).copied();
        let dir = parent.map_or(direction, |group| groups.directions[group as usize]);
        let shape =
            super::FlowchartShape::resolve(node.layout_shape.as_deref().unwrap_or("squareRect"))?;
        let size = measurement.node(
            node,
            FlowchartSvgLabelOwner::Node(ordinal),
            direction_name(dir),
            |units| work.charge_adapter(units),
        )?;
        let geometry = size.geometry;
        let layout_node = layout_node(&node.id, size);
        let rectangle_radii = match shape {
            super::FlowchartShape::Process => process_radius,
            super::FlowchartShape::RoundedRectangle => theme_radius,
            _ => None,
        };
        let profile = ShapePortProfile::flowchart_with_paint_metrics(
            node,
            &layout_node,
            measurement.settings.node_padding,
            look_is_neo,
            geometry,
            rectangle_radii,
        )?;
        if fit_width && shape == super::FlowchartShape::ForkJoin {
            let axis = match dir {
                Direction::Down | Direction::Up => Axis::Vertical,
                Direction::Left | Direction::Right => Axis::Horizontal,
            };
            bindings.flow_relative_nodes.push((graph.nodes.len(), axis));
        }
        bindings.push_leaf(&mut graph, layout_node, profile, seq, parent);
    }

    for ordinal in groups.collapsed {
        let group = &view.subgraphs[ordinal];
        let parent = groups.parents.get(group.id.as_str()).copied();
        let metrics =
            measurement.subgraph_title(group, ordinal, |units| work.charge_adapter(units))?;
        let (width, height) = super::flowchart_node_render_dimensions(
            Some("collapsedGroup"),
            metrics,
            measurement.settings.cluster_padding,
            look_is_neo,
        );
        let node = layout_node(
            &group.id,
            LeafSize {
                width,
                height,
                label: super::measured::MeasuredLabel {
                    width: metrics.width,
                    height: metrics.height,
                },
                geometry: None,
            },
        );
        let radius = if view.keyword == "agentflow-beta" {
            10.0
        } else {
            8.0
        };
        let profile = ShapePortProfile::rounded_rectangle(width, height, radius, radius);
        let seq = index(work.checked_add(view.nodes.len(), ordinal)?, work)?;
        endpoints.insert(&group.id, Endpoint::Node(index(graph.nodes.len(), work)?));
        bindings.push_leaf(&mut graph, node, profile, seq, parent);
    }

    for (key, projected) in view.projected_edges() {
        let endpoint = |id: &str| {
            endpoints
                .get(id)
                .copied()
                .ok_or_else(|| Error::InvalidModel {
                    message: format!(
                        "Missing endpoint `{id}` for Shoal Flowchart edge `{}`",
                        projected.semantic.id
                    ),
                })
        };
        work.charge_adapter(1)?;
        let edge = projected.semantic;
        let label = measurement.edge_label(edge, key)?;
        let (start_marker, end_marker) = crate::svg::flowchart_marker_metrics(edge);
        graph.edges.push(MeasuredEdge {
            source: endpoint(projected.from)?,
            target: endpoint(projected.to)?,
            min_rank_span: u32::try_from(edge.length.max(1))
                .map_err(|_| work.record_arithmetic_overflow())?,
            weight: 1.0,
            label: label.map(|label| Size {
                width: label.width,
                height: label.height,
            }),
            start_label: None,
            end_label: None,
            start_marker,
            end_marker,
        });
    }

    Ok((graph, bindings))
}

impl FlowchartBindings {
    fn frame_hints(
        &self,
        model: FlowchartRenderModelRef<'_>,
        graph: &MeasuredGraph,
        previous: &ShoalLayoutFrame,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<Vec<Option<shoal_bridge::NodeHint>>> {
        if previous.nodes.is_empty() {
            return Ok(Vec::new());
        }
        let mut hints = Vec::with_capacity(self.nodes.len());
        let comparisons = previous.nodes.len().ilog2() as usize + 1;
        work.charge_adapter(work.checked_mul(self.nodes.len(), comparisons)?)?;
        let mut parent_matches = Vec::with_capacity(self.groups.len());
        work.charge_adapter(previous.groups.len())?;
        let mut previous_groups = HashMap::with_capacity(previous.groups.len());
        for (index, id) in previous.groups.iter().enumerate() {
            work.charge_adapter(id.len())?;
            previous_groups.insert(id.as_str(), index);
        }
        for &group in &self.groups {
            let id = &model.semantic.subgraphs[group].id;
            work.charge_adapter(id.len())?;
            parent_matches.push(previous_groups.get(id.as_str()).copied());
        }
        for (layout, measured) in self.nodes.iter().zip(&graph.nodes) {
            work.charge_adapter(layout.id.len())?;
            let Some(node) = previous.nodes.get(&layout.id) else {
                hints.push(None);
                continue;
            };
            let same_parent = match (measured.parent, node.parent) {
                (None, None) => true,
                (Some(group), Some(previous_group)) => {
                    parent_matches[group as usize] == Some(previous_group as usize)
                }
                _ => false,
            };
            hints.push(same_parent.then_some((
                shoal_bridge::OrderHint {
                    rank: i32::try_from(node.rank).unwrap_or(i32::MAX),
                    cross_key: node.cross_key,
                },
                Direction::from(node.direction),
            )));
        }
        Ok(hints)
    }

    fn capture_frame(
        &self,
        model: FlowchartRenderModelRef<'_>,
        graph: &MeasuredGraph,
        solved: &SolvedGraph,
        family: &str,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<ShoalLayoutFrame> {
        if solved.node_ranks.len() != self.nodes.len() {
            return Err(Error::InvalidModel {
                message: "Shoal omitted the requested node ranks".into(),
            });
        }
        let comparisons = self.nodes.len().checked_ilog2().unwrap_or(0) as usize + 1;
        work.charge_adapter(work.checked_mul(self.nodes.len(), comparisons)?)?;
        let mut groups = Vec::with_capacity(self.groups.len());
        for &group in &self.groups {
            let id = &model.semantic.subgraphs[group].id;
            work.charge_adapter(id.len())?;
            groups.push(id.clone());
        }
        let mut nodes = BTreeMap::new();
        for ((node, measured), (&rank, rect)) in self
            .nodes
            .iter()
            .zip(&graph.nodes)
            .zip(solved.node_ranks.iter().zip(&solved.nodes))
        {
            work.charge_adapter(node.id.len())?;
            let direction = measured.parent.map_or(solved.direction, |index| {
                solved.groups[index as usize].direction
            });
            let center = rect.center();
            let cross_key = match direction {
                Direction::Down | Direction::Up => center.x,
                Direction::Right | Direction::Left => center.y,
            };
            nodes.insert(
                node.id.clone(),
                ShoalFrameNode {
                    parent: measured.parent,
                    rank,
                    cross_key,
                    direction: direction.into(),
                },
            );
        }
        Ok(ShoalLayoutFrame {
            family: family.to_owned(),
            groups,
            nodes,
        })
    }

    fn push_leaf(
        &mut self,
        graph: &mut MeasuredGraph,
        node: LayoutNode,
        profile: ShapePortProfile,
        seq: u32,
        parent: Option<u32>,
    ) {
        let (face, perpendicular) = match graph.direction {
            Direction::Down | Direction::Up => (Side::Top, Side::Left),
            Direction::Left | Direction::Right => (Side::Left, Side::Top),
        };
        graph.nodes.push(MeasuredNode {
            size: Size {
                width: node.width,
                height: node.height,
            },
            seq,
            parent,
            port_span: Some(profile.port_span(face)),
            perpendicular_port_span: Some(profile.port_span(perpendicular)),
            anchor_y: profile.anchor_y(),
        });
        self.nodes.push(node);
        self.profiles.push(profile);
    }

    fn project(
        mut self,
        model: FlowchartRenderModelRef<'_>,
        measurement: &FlowchartMeasurement<'_>,
        solved: SolvedGraph,
        placement: FlowchartPlacement,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<FlowchartLayout> {
        work.charge_adapter(self.flow_relative_nodes.len())?;
        for &(index, _) in &self.flow_relative_nodes {
            let rect = solved.nodes[index];
            self.profiles[index] = ShapePortProfile::fork_join(rect.width, rect.height);
        }
        work.charge_adapter(self.nodes.len())?;
        for (node, rect) in self.nodes.iter_mut().zip(solved.nodes) {
            let center = rect.center();
            node.x = center.x;
            node.y = center.y;
            node.width = rect.width;
            node.height = rect.height;
        }
        work.charge_adapter(self.groups.len())?;
        let group_parents = solved.groups.iter().map(|group| group.parent).collect();
        let clusters = groups::project(&self.groups, solved.groups, model, &measurement.settings);
        work.charge_adapter(model.edges.len())?;
        let mut routes = vec![None; model.edges.len()];
        work.charge_adapter(solved.edges.len())?;
        let mut owners = Vec::with_capacity(solved.edges.len());
        let mut edges = Vec::with_capacity(solved.edges.len());
        for ((key, projected), route) in model.projected_edges().zip(solved.edges) {
            work.charge_adapter(route.points.len())?;
            let (points, axes) = route
                .points
                .into_iter()
                .map(|p| (LayoutPoint { x: p.x, y: p.y }, p.axis))
                .unzip();
            let index = key.semantic_index();
            routes[index] = Some(RouteAxes {
                endpoints: (route.source, route.target),
                ports: route.ports,
                axes,
            });
            edges.push(LayoutEdge {
                id: projected.semantic.id.clone(),
                from: projected.from.to_owned(),
                to: projected.to.to_owned(),
                from_cluster: None,
                to_cluster: None,
                points,
                label: route.label,
                start_label_left: None,
                start_label_right: None,
                end_label_left: None,
                end_label_right: None,
                start_marker: None,
                end_marker: None,
                stroke_dasharray: None,
            });
            owners.push(key);
        }
        let bounds = (!self.nodes.is_empty() || !clusters.is_empty()).then_some(Bounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: solved.bounds.width,
            max_y: solved.bounds.height,
        });
        let mut border_stitches = solved.border_stitches;
        work.charge_adapter(border_stitches.len())?;
        for stitch in &mut border_stitches {
            stitch.edge = owners[stitch.edge].semantic_index();
        }
        let mut dom_node_order_by_root = HashMap::new();
        if !model.subgraphs.is_empty() {
            dom_node_order_by_root.insert(
                String::new(),
                self.nodes.iter().map(|node| node.id.clone()).collect(),
            );
        }
        let clearance = paint_clearance(&self.nodes, &clusters, &edges, &owners, work)?;
        Ok(FlowchartLayout {
            nodes: self.nodes,
            edges,
            clusters,
            edge_owners: FlowchartEdgeOwners::new(owners),
            bounds,
            dom_node_order_by_root,
            uses_elk_adapter_dom: false,
            placement,
            route_contract: RouteContract::CanonicalAxis {
                nodes: self.profiles,
                group_parents,
                border_stitches,
                clearance,
                routes,
            },
        })
    }
}

impl From<FrameDirection> for Direction {
    fn from(direction: FrameDirection) -> Self {
        match direction {
            FrameDirection::Down => Self::Down,
            FrameDirection::Up => Self::Up,
            FrameDirection::Left => Self::Left,
            FrameDirection::Right => Self::Right,
        }
    }
}

impl From<Direction> for FrameDirection {
    fn from(direction: Direction) -> Self {
        match direction {
            Direction::Down => Self::Down,
            Direction::Up => Self::Up,
            Direction::Left => Self::Left,
            Direction::Right => Self::Right,
        }
    }
}
