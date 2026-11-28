//! Project visible Flowchart and Agentflow occurrences into Shoal's measured graph.

mod groups;

use super::measured::{FlowchartMeasurement, MeasuredNode as LeafSize};
use super::{
    FlowchartConfigView, FlowchartEdgeOwners, FlowchartRenderModelRef, FlowchartSvgLabelOwner,
    FlowchartSvgLabelSidecarBuilder,
};
use crate::graph_layout::axis_clearance::AxisClearance;
use crate::graph_layout::contour::{ShapePortProfile, shoal_contour};
use crate::graph_layout::ir::{
    Axis, Direction, Endpoint, MeasuredEdge, MeasuredGraph, MeasuredNode, Side, Size, SolvedGraph,
};
use crate::graph_layout::{
    AxisRoutes, FlowchartSvgEdgeGeometry, RouteContract, RouteTopology, placement,
};
use crate::layout_frame::GraphPlacement;
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::{Bounds, FlowchartLayout, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint};
use crate::{Error, LayoutAdaptation, LayoutExecution, Result};
use merman_model::families::flowchart::{FlowchartModel, FlowchartRenderContext};
use merman_model::presentation::PresentationConfig;
use merman_render_core::graph_layout::contour::Outline;
use merman_render_flow::edge_labels::HTML_EDGE_LABEL_INSET_PX;
use merman_render_flow::node_geometry::contour as flow_contour;
use merman_render_flow::projection::ProjectedFlowchartEdge;
use rustc_hash::FxHashMap;
use std::collections::HashMap;

fn admit_paint_clearance(
    nodes: usize,
    clusters: usize,
    edges: usize,
    labels: usize,
    work: &mut OperationLayoutWorkControl,
) -> Result<()> {
    if edges == 0 {
        return Ok(());
    }
    let shapes = work.checked_add(nodes, clusters)?;
    work.charge_adapter(work.checked_add(shapes, edges)?)?;
    let boxes = work.checked_add(
        work.checked_add(shapes, labels)?,
        work.checked_mul(edges, 2)?,
    )?;
    let levels = boxes.checked_ilog2().unwrap_or(0) as usize + 1;
    work.charge_adapter(work.checked_mul(boxes, levels)?)
}

#[allow(
    clippy::too_many_arguments,
    reason = "The adapter receives prepared measurement inputs and the family output purpose."
)]
pub(crate) fn layout(
    model: &FlowchartModel,
    render_context: &FlowchartRenderContext,
    class_defs: &super::FlowchartClassDefinitions<'_>,
    config: &PresentationConfig,
    execution: &LayoutExecution<'_>,
    labels: Option<&FlowchartSvgLabelSidecarBuilder>,
    edge_styles: &crate::svg::FlowchartEdgeStylePlan,
    prepare_for_svg: bool,
) -> Result<FlowchartLayout> {
    let mut work = OperationLayoutWorkControl::new(execution.work_meter());
    work.charge_adapter(0)?;
    let config_view = FlowchartConfigView::new(config);
    let view = FlowchartRenderModelRef::new(model, render_context).with_shoal_svg_text(
        config_view.node_html_labels(),
        config_view.effective_html_labels(),
    );
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
    let native = execution.native_family_theme();
    let (graph, bindings) = measure(
        view,
        &measurement,
        config,
        edge_styles,
        native,
        fit_width,
        &mut work,
    )?;
    let (solved, placement) = placement::solve(
        &graph,
        bindings.nodes.iter().map(|node| node.id.as_str()),
        bindings
            .groups
            .iter()
            .map(|&i| view.semantic.subgraphs[i].id.as_str()),
        Some(execution),
        placement::Constraints {
            flow_relative_nodes: &bindings.flow_relative_nodes,
            allow_turn: true,
            ..Default::default()
        },
        &mut work,
    )?;
    drop(graph);
    bindings.project(
        view,
        &measurement,
        solved,
        placement,
        prepare_for_svg,
        &mut work,
    )
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
    edge_styles: &crate::svg::FlowchartEdgeStylePlan,
    native: Option<&crate::theme::FamilyTheme>,
    fit_width: bool,
    work: &mut OperationLayoutWorkControl,
) -> Result<(MeasuredGraph, FlowchartBindings)> {
    let direction = direction(view.direction.as_deref().unwrap_or("TB"))?;
    work.charge_adapter(view.nodes.len())?;
    work.charge_adapter(view.edges.len())?;
    let config_view = FlowchartConfigView::new(config);
    let curve = native
        .and_then(|theme| super::native_theme::default_curve(theme.form.edge_style, &view, config))
        .unwrap_or_else(|| config_view.effective_edge_interpolation(&view, false));
    let look_is_neo = crate::config::mermaid_config_diagram_look(config).is_neo();
    let theme_radius = crate::config::config_f64(config, &["themeVariables", "radius"])
        .map(|r| (r.max(0.), r.max(0.)));
    let process_radius = (look_is_neo && config.explicit_config_owns_path("themeVariables.radius"))
        .then_some(theme_radius)
        .flatten();
    let mut graph = MeasuredGraph {
        direction,
        routing: crate::graph_layout::route_paint::RouteStyle::from_interpolate(curve, 0.0)
            .routing_style(),
        node_spacing: measurement.settings.nodesep,
        rank_spacing: measurement.settings.ranksep,
        component_spacing: measurement.settings.nodesep,
        inherit_crossed_group_direction: true,
        nodes: Vec::with_capacity(view.nodes.len()),
        groups: Vec::new(),
        edges: Vec::with_capacity(view.edges.len()),
    };
    let mut endpoints = FxHashMap::with_capacity_and_hasher(
        work.checked_add(view.nodes.len(), view.subgraphs.len())?,
        Default::default(),
    );
    let groups = groups::measure(view, measurement, config, &mut graph, &mut endpoints, work)?;
    let mut bindings = FlowchartBindings {
        nodes: Vec::with_capacity(view.nodes.len()),
        profiles: Vec::with_capacity(view.nodes.len()),
        groups: groups.declarations,
        flow_relative_nodes: Vec::new(),
    };
    let mut parallel_faces = Vec::with_capacity(view.nodes.len());
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
            super::resolve_flowchart_shape(node.layout_shape.as_deref().unwrap_or("squareRect"))?;
        let mut size = measurement.node(
            node,
            FlowchartSvgLabelOwner::Node(ordinal),
            direction_name(dir),
            |units| work.charge_adapter(units),
        )?;
        if native.is_some() && shape == super::FlowchartShape::Diamond {
            size.widen_decision(measurement.settings.node_padding);
        }
        let geometry = size.geometry;
        let layout_node = layout_node(&node.id, size);
        let rectangle_radii = match shape {
            super::FlowchartShape::Process => process_radius,
            super::FlowchartShape::RoundedRectangle => theme_radius,
            _ => None,
        };
        let profile = if shape == super::FlowchartShape::Diamond {
            flow_contour::decision(layout_node.width, layout_node.height)
        } else {
            flow_contour::from_paint_metrics(
                node,
                &layout_node,
                measurement.settings.node_padding,
                look_is_neo,
                geometry,
                rectangle_radii,
            )?
        };
        if fit_width && shape == super::FlowchartShape::ForkJoin {
            let axis = match dir {
                Direction::Down | Direction::Up => Axis::Vertical,
                Direction::Left | Direction::Right => Axis::Horizontal,
            };
            bindings.flow_relative_nodes.push((graph.nodes.len(), axis));
        }
        let face = match dir {
            Direction::Down | Direction::Up => Side::Top,
            Direction::Left | Direction::Right => Side::Left,
        };
        parallel_faces.push(
            (geometry.is_none()
                && matches!(
                    shape,
                    super::FlowchartShape::Process | super::FlowchartShape::RoundedRectangle
                ))
            .then_some((face, profile.port_span(face))),
        );
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
        parallel_faces.push(None);
        bindings.push_leaf(&mut graph, node, profile, seq, parent);
    }

    let html_label_inset = if measurement.settings.edge_html_labels && native.is_none() {
        HTML_EDGE_LABEL_INSET_PX
    } else {
        0.0
    };
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
        let markers = edge_styles.marker_metrics_for(key, edge, look_is_neo)?;
        let (start_marker, end_marker) = edge_styles.reserve_marker_stroke(key, markers)?;
        graph.edges.push(MeasuredEdge {
            source: endpoint(projected.from)?,
            target: endpoint(projected.to)?,
            min_rank_span: u32::try_from(edge.length.max(1))
                .map_err(|_| work.record_arithmetic_overflow())?,
            weight: 1.0,
            label: label.map(|label| Size {
                width: label.width + 2.0 * html_label_inset,
                height: label.height,
            }),
            start_label: None,
            end_label: None,
            start_marker,
            end_marker,
        });
    }
    bindings.reserve_parallel_faces(&mut graph, &parallel_faces, work)?;

    Ok((graph, bindings))
}

impl FlowchartBindings {
    fn reserve_parallel_faces(
        &mut self,
        graph: &mut MeasuredGraph,
        faces: &[Option<(Side, f64)>],
        work: &mut OperationLayoutWorkControl,
    ) -> Result<()> {
        const MAX_FACE_GROWTH: f64 = 64.0;
        if graph.edges.len() < 3 {
            return Ok(());
        }
        let mut pairs = FxHashMap::<[usize; 2], (usize, [f64; 2])>::default();
        work.charge_adapter(graph.edges.len())?;
        pairs
            .try_reserve(graph.edges.len())
            .map_err(|_| work.map_shoal_error(merman_layout_shoal::LayoutError::AllocationLimit))?;
        for edge in &graph.edges {
            let (Endpoint::Node(a), Endpoint::Node(b)) = (edge.source, edge.target) else {
                continue;
            };
            let mut ids = [a as usize, b as usize];
            if a == b
                || graph.nodes[ids[0]].parent != graph.nodes[ids[1]].parent
                || ids.iter().all(|&i| faces[i].is_none())
            {
                continue;
            }
            let mut widths = [edge.start_marker.width, edge.end_marker.width];
            if a > b {
                ids.swap(0, 1);
                widths.swap(0, 1);
            }
            let (count, widest) = pairs.entry(ids).or_default();
            *count += 1;
            for (widest, width) in widest.iter_mut().zip(widths) {
                *widest = widest.max(width);
            }
        }
        let minimum = merman_layout_shoal::Spacing::default().min_port_pitch;
        work.charge_adapter(pairs.len())?;
        for (ids, (count, widths)) in pairs {
            if count < 3 {
                continue;
            }
            for (i, width) in ids.into_iter().zip(widths) {
                let Some((face, original_span)) = faces[i] else {
                    continue;
                };
                let profile = &self.profiles[i];
                let Outline::Box { rx, ry, .. } = *profile.outline() else {
                    continue;
                };
                let mut spans = [profile.port_span(Side::Top), profile.port_span(Side::Left)];
                let cross = usize::from(face == Side::Left);
                let required = (count - 1) as f64 * width.max(minimum);
                let growth =
                    (required.min(original_span + MAX_FACE_GROWTH) - spans[cross]).max(0.0);
                if growth == 0.0 {
                    continue;
                }
                // Each pair requests its own width. Resize the actual node before ports are seated.
                let node = &mut self.nodes[i];
                if cross == 0 {
                    node.width += growth;
                } else {
                    node.height += growth;
                }
                spans[cross] += growth;
                let profile = ShapePortProfile::new(
                    Outline::rect(node.width, node.height, rx, ry),
                    spans[0],
                    spans[1],
                );
                let size = Size {
                    width: node.width,
                    height: node.height,
                };
                let axis = usize::from(matches!(
                    graph.direction,
                    Direction::Left | Direction::Right
                ));
                let measured = &mut graph.nodes[i];
                measured.size = size;
                measured.contour = shoal_contour(&profile, size);
                measured.port_span = Some(spans[axis]);
                measured.perpendicular_port_span = Some(spans[1 - axis]);
                self.profiles[i] = profile;
            }
        }
        Ok(())
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
        let size = Size {
            width: node.width,
            height: node.height,
        };
        graph.nodes.push(MeasuredNode {
            size,
            contour: shoal_contour(&profile, size),
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
        placement: GraphPlacement,
        prepare_for_svg: bool,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<FlowchartLayout> {
        work.charge_adapter(self.flow_relative_nodes.len())?;
        for &(index, _) in &self.flow_relative_nodes {
            let rect = solved.nodes[index];
            self.profiles[index] = flow_contour::fork_join(rect.width, rect.height);
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
        let (clusters, struck_group_titles) =
            groups::project(&self.groups, solved.groups, model, &measurement.settings);
        work.charge_adapter(model.edges.len())?;
        let mut routes = vec![None; model.edges.len()];
        let edge_count = solved.edges.len();
        work.charge_adapter(edge_count)?;
        let mut owners = Vec::with_capacity(edge_count);
        let mut svg_edges = prepare_for_svg.then(|| Vec::with_capacity(edge_count));
        let mut edges = Vec::with_capacity(if prepare_for_svg { 0 } else { edge_count });
        let mut label_count = 0;
        for ((key, projected), route) in model.projected_edges().zip(solved.edges) {
            work.charge_adapter(route.points.len())?;
            label_count += usize::from(route.label.is_some());
            let (points, knots) = route
                .points
                .into_iter()
                .map(|p| (LayoutPoint { x: p.x, y: p.y }, (p.axis, p.kind)))
                .unzip();
            let index = key.semantic_index();
            routes[index] = Some(RouteTopology {
                endpoints: (route.source, route.target),
                ports: route.ports,
                knots,
                corridor_clearance: route.corridor_clearance,
            });
            if let Some(svg_edges) = &mut svg_edges {
                svg_edges.push(FlowchartSvgEdgeGeometry {
                    points,
                    label: route.label,
                });
            } else {
                edges.push(layout_edge(projected, points, route.label));
            }
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
        admit_paint_clearance(
            self.nodes.len(),
            clusters.len(),
            owners.len(),
            label_count,
            work,
        )?;
        Ok(FlowchartLayout {
            nodes: self.nodes,
            edges,
            clusters,
            edge_owners: FlowchartEdgeOwners::new(owners),
            bounds,
            dom_node_order_by_root,
            uses_elk_adapter_dom: false,
            placement,
            route_contract: RouteContract::CanonicalAxis(
                AxisRoutes {
                    nodes: self.profiles,
                    group_parents,
                    struck_group_titles,
                    border_stitches,
                    clearance: AxisClearance::default(),
                    routes,
                    svg_edges,
                }
                .into(),
            ),
        })
    }
}

fn layout_edge(
    edge: ProjectedFlowchartEdge<'_>,
    points: Vec<LayoutPoint>,
    label: Option<LayoutLabel>,
) -> LayoutEdge {
    LayoutEdge {
        id: edge.semantic.id.clone(),
        from: edge.from.to_owned(),
        to: edge.to.to_owned(),
        from_cluster: None,
        to_cluster: None,
        points,
        label,
        start_label_left: None,
        start_label_right: None,
        end_label_left: None,
        end_label_right: None,
        start_marker: None,
        end_marker: None,
        stroke_dasharray: None,
    }
}

pub(crate) fn materialize_edges(
    model: FlowchartRenderModelRef<'_>,
    owners: &FlowchartEdgeOwners,
    geometry: &[FlowchartSvgEdgeGeometry],
) -> Result<Vec<LayoutEdge>> {
    owners.validate_projected(geometry.len(), &model.edges)?;
    owners
        .iter()
        .zip(geometry)
        .map(|(key, geometry)| {
            let edge = &model.edges[key.semantic_index()];
            let projected =
                super::project_flowchart_edge(edge, model.render_context).ok_or_else(|| {
                    Error::InvalidModel {
                        message: format!(
                            "Flowchart layout contains hidden edge occurrence {key:?}"
                        ),
                    }
                })?;
            Ok(layout_edge(
                projected,
                geometry.points.clone(),
                geometry.label.clone(),
            ))
        })
        .collect()
}
