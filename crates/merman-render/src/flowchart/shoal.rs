//! Flat Flowchart projection for the experimental Shoal backend.

use super::measured::FlowchartMeasurement;
use super::{
    FlowchartConfigView, FlowchartEdgeKey, FlowchartEdgeOwners, FlowchartRenderModelRef,
    FlowchartSvgLabelOwner, FlowchartSvgLabelSidecarBuilder,
};
use crate::graph_layout::contour::ShapePortProfile;
use crate::graph_layout::ir::{
    Direction, Endpoint, MeasuredEdge, MeasuredGraph, MeasuredNode, RoutingStyle, Side, Size,
    SolvedGraph,
};
use crate::graph_layout::{RouteAxes, RouteContract, shoal_bridge};
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::{Bounds, FlowchartLayout, LayoutEdge, LayoutNode, LayoutPoint};
use crate::{Error, LayoutExecution, Result};
use merman_model::families::flowchart::{FlowchartModel, FlowchartRenderContext};
use merman_model::presentation::PresentationConfig;
use std::collections::HashMap;

pub(crate) fn layout(
    model: &FlowchartModel,
    render_context: &FlowchartRenderContext,
    config: &PresentationConfig,
    execution: &LayoutExecution<'_>,
    labels: Option<&FlowchartSvgLabelSidecarBuilder>,
    edge_styles: &crate::svg::FlowchartEdgeStylePlan,
) -> Result<FlowchartLayout> {
    let mut work = OperationLayoutWorkControl::new(execution.work_meter());
    work.charge_adapter(0)?;
    if !model.subgraphs.is_empty() {
        return Err(Error::UnsupportedLayoutConstruct {
            backend: "shoal",
            construct: "subgraphs",
        });
    }
    work.charge_adapter(model.nodes.len())?;
    super::validate_flowchart_model_shapes(model)?;
    let view = FlowchartRenderModelRef::new(model, render_context);
    let measurement = FlowchartMeasurement::new(
        view,
        config,
        execution.text_measurer(),
        execution.math_renderer(),
        labels,
        edge_styles,
    );
    let (graph, bindings) = measure(view, &measurement, config, &mut work)?;
    let solved = shoal_bridge::solve(
        &graph,
        &mut shoal_bridge::LayoutScratch::default(),
        &mut work,
    )?;
    drop(graph);
    bindings.project(model, solved, &mut work)
}

/// Node order matches the measured graph. Edge keys retain the semantic occurrence even when
/// projection hides an edge or distinct occurrences share a public ID.
struct FlowchartBindings {
    nodes: Vec<LayoutNode>,
    profiles: Vec<ShapePortProfile>,
    owners: Vec<FlowchartEdgeKey>,
}

fn measure(
    view: FlowchartRenderModelRef<'_>,
    measurement: &FlowchartMeasurement<'_>,
    config: &PresentationConfig,
    work: &mut OperationLayoutWorkControl,
) -> Result<(MeasuredGraph, FlowchartBindings)> {
    let (direction, direction_name, face) = match view
        .direction
        .as_deref()
        .unwrap_or("TB")
        .trim()
        .to_ascii_uppercase()
        .as_str()
    {
        "LR" => (Direction::Right, "LR", Side::Right),
        "RL" => (Direction::Left, "RL", Side::Left),
        "BT" => (Direction::Up, "BT", Side::Top),
        "TB" | "TD" => (Direction::Down, "TB", Side::Bottom),
        other => {
            return Err(Error::InvalidModel {
                message: format!("Unsupported Shoal Flowchart direction `{other}`"),
            });
        }
    };
    work.charge_adapter(view.nodes.len())?;
    work.charge_adapter(view.edges.len())?;
    let config_view = FlowchartConfigView::new(config);
    let configured_curve = config_view.render_curve();
    // Mermaid's `linkStyle default interpolate` takes precedence over flowchart.curve.
    // Each edge still selects its own interpolation when the final SVG path is painted.
    let curve = view
        .edge_defaults
        .as_ref()
        .and_then(|d| d.interpolate.as_deref())
        .or(configured_curve.as_deref())
        .unwrap_or("basis");
    let look_is_neo = crate::config::mermaid_config_diagram_look(config).is_neo();
    let wrapping_width = config_view.render_wrapping_width();
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
        nodes: Vec::with_capacity(view.nodes.len()),
        groups: Vec::new(),
        edges: Vec::with_capacity(view.edges.len()),
    };
    let mut node_ids = HashMap::with_capacity(view.nodes.len());
    let mut nodes = Vec::with_capacity(view.nodes.len());
    let mut profiles = Vec::with_capacity(view.nodes.len());
    for (index, node) in view.nodes.iter().enumerate() {
        work.charge_adapter(node.id.len())?;
        let seq = u32::try_from(index).map_err(|_| work.record_arithmetic_overflow())?;
        if node_ids
            .insert(node.id.as_str(), Endpoint::Node(seq))
            .is_some()
        {
            return Err(Error::InvalidModel {
                message: format!("Duplicate Shoal Flowchart node `{}`", node.id),
            });
        }
        super::validate_flowchart_node_asset_dimensions(node)?;
        let size = measurement.node(
            node,
            FlowchartSvgLabelOwner::Node(index),
            direction_name,
            |units| work.charge_adapter(units),
        )?;
        let layout_node = LayoutNode {
            id: node.id.clone(),
            x: 0.0,
            y: 0.0,
            width: size.width,
            height: size.height,
            is_cluster: false,
            label_width: Some(size.label.width),
            label_height: Some(size.label.height),
        };
        let rectangle_radii = match super::FlowchartShape::resolve(
            node.layout_shape.as_deref().unwrap_or("squareRect"),
        )? {
            super::FlowchartShape::Process => process_radius,
            super::FlowchartShape::RoundedRectangle => theme_radius,
            _ => None,
        };
        let profile = ShapePortProfile::flowchart_with_paint_metrics(
            node,
            &layout_node,
            measurement.settings.node_padding,
            look_is_neo,
            wrapping_width,
            rectangle_radii,
        )?;
        graph.nodes.push(MeasuredNode {
            size: Size {
                width: size.width,
                height: size.height,
            },
            seq,
            parent: None,
            port_span: Some(profile.port_span(face)),
            anchor_y: profile.anchor_y(),
        });
        nodes.push(layout_node);
        profiles.push(profile);
    }

    let mut owners = Vec::with_capacity(view.edges.len());
    for (key, projected) in view.projected_edges() {
        let endpoint = |id: &str| {
            node_ids
                .get(id)
                .copied()
                .ok_or_else(|| Error::InvalidModel {
                    message: format!(
                        "Missing node `{id}` for Shoal Flowchart edge `{}`",
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
        owners.push(key);
    }

    Ok((
        graph,
        FlowchartBindings {
            nodes,
            profiles,
            owners,
        },
    ))
}

impl FlowchartBindings {
    fn project(
        mut self,
        model: &FlowchartModel,
        solved: SolvedGraph,
        work: &mut OperationLayoutWorkControl,
    ) -> Result<FlowchartLayout> {
        work.charge_adapter(self.nodes.len())?;
        for (node, rect) in self.nodes.iter_mut().zip(solved.nodes) {
            let center = rect.center();
            node.x = center.x;
            node.y = center.y;
            node.width = rect.width;
            node.height = rect.height;
        }
        work.charge_adapter(model.edges.len())?;
        let mut routes = vec![None; model.edges.len()];
        work.charge_adapter(self.owners.len())?;
        let mut edges = Vec::with_capacity(self.owners.len());
        for (key, route) in self.owners.iter().zip(solved.edges) {
            let (Endpoint::Node(source), Endpoint::Node(target)) = (route.source, route.target)
            else {
                return Err(Error::UnsupportedLayoutConstruct {
                    backend: "shoal",
                    construct: "subgraphs",
                });
            };
            let (source, target) = (source as usize, target as usize);
            work.charge_adapter(route.points.len())?;
            let (points, axes) = route
                .points
                .into_iter()
                .map(|p| (LayoutPoint { x: p.x, y: p.y }, p.axis))
                .unzip();
            let index = key.semantic_index();
            routes[index] = Some(RouteAxes {
                endpoints: (source, target),
                ports: route.ports,
                axes,
            });
            edges.push(LayoutEdge {
                id: model.edges[index].id.clone(),
                from: self.nodes[source].id.clone(),
                to: self.nodes[target].id.clone(),
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
        }
        let bounds = (!self.nodes.is_empty()).then_some(Bounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: solved.bounds.width,
            max_y: solved.bounds.height,
        });
        Ok(FlowchartLayout {
            nodes: self.nodes,
            edges,
            clusters: Vec::new(),
            edge_owners: FlowchartEdgeOwners::new(self.owners),
            bounds,
            dom_node_order_by_root: HashMap::new(),
            uses_elk_adapter_dom: false,
            route_contract: RouteContract::CanonicalAxis {
                nodes: self.profiles,
                routes,
            },
        })
    }
}
