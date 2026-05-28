use super::model::{AsciiGraph, GraphDirection, GraphEdgeArrow, GraphEdgeStroke, GraphNodeShape};
use crate::AsciiDirection;
use crate::error::{AsciiError, Result};
use crate::options::AsciiRenderOptions;
use merman_core::diagrams::flowchart::FlowchartV2Model;
use std::collections::HashSet;

pub(crate) fn from_flowchart_model(
    model: &FlowchartV2Model,
    options: &AsciiRenderOptions,
) -> Result<AsciiGraph> {
    validate_supported_flowchart_model(model)?;

    let direction = if let Some(direction) = model.direction.as_deref() {
        parse_direction(direction)?
    } else {
        match options.fallback_direction {
            AsciiDirection::LeftRight => GraphDirection::LeftRight,
            AsciiDirection::TopDown => GraphDirection::TopDown,
        }
    };
    let mut graph = AsciiGraph::new(direction);

    for node in &model.nodes {
        graph.add_node_with_shape(
            &node.id,
            node.label.as_deref().unwrap_or(&node.id),
            parse_node_shape(node.layout_shape.as_deref())?,
        );
    }

    for edge in &model.edges {
        graph.add_edge_with_attrs(
            &edge.from,
            &edge.to,
            edge.label
                .as_deref()
                .map(str::trim)
                .filter(|label| !label.is_empty())
                .map(ToOwned::to_owned),
            parse_edge_stroke(edge.stroke.as_deref().unwrap_or("normal"))?,
            parse_edge_arrow(edge.edge_type.as_deref().unwrap_or("arrow_point"))?,
            edge.length,
        );
    }

    for subgraph in &model.subgraphs {
        graph.add_group(&subgraph.title, subgraph.nodes.clone());
    }

    Ok(graph)
}

fn parse_direction(direction: &str) -> Result<GraphDirection> {
    match direction {
        "LR" => Ok(GraphDirection::LeftRight),
        "TB" | "TD" => Ok(GraphDirection::TopDown),
        _ => Err(AsciiError::UnsupportedFeature {
            diagram_type: "flowchart",
            feature: "non-LR/TD graph directions",
        }),
    }
}

fn parse_edge_stroke(stroke: &str) -> Result<GraphEdgeStroke> {
    match stroke {
        "normal" => Ok(GraphEdgeStroke::Normal),
        "dotted" => Ok(GraphEdgeStroke::Dotted),
        _ => Err(AsciiError::UnsupportedFeature {
            diagram_type: "flowchart",
            feature: "non-normal edge strokes",
        }),
    }
}

fn parse_edge_arrow(edge_type: &str) -> Result<GraphEdgeArrow> {
    match edge_type {
        "arrow_open" => Ok(GraphEdgeArrow::Open),
        "arrow_point" => Ok(GraphEdgeArrow::Point),
        _ => Err(AsciiError::UnsupportedFeature {
            diagram_type: "flowchart",
            feature: "non-point edge arrows",
        }),
    }
}

fn parse_node_shape(shape: Option<&str>) -> Result<GraphNodeShape> {
    match shape.unwrap_or("squareRect") {
        "rect" | "rectangle" | "square" | "squareRect" => Ok(GraphNodeShape::Rect),
        "roundedRect" | "rounded" | "event" | "stadium" | "terminal" | "pill" | "circle"
        | "circ" | "doublecircle" | "dbl-circ" | "double-circle" => Ok(GraphNodeShape::Rounded),
        "diamond" | "question" | "diam" | "decision" => Ok(GraphNodeShape::Diamond),
        "subroutine" | "fr-rect" | "subproc" | "subprocess" | "framed-rectangle" => {
            Ok(GraphNodeShape::Subroutine)
        }
        "cylinder" | "cyl" | "db" | "database" => Ok(GraphNodeShape::Cylinder),
        _ => Err(AsciiError::UnsupportedFeature {
            diagram_type: "flowchart",
            feature: "non-rectangular node shapes",
        }),
    }
}

fn validate_supported_flowchart_model(model: &FlowchartV2Model) -> Result<()> {
    if model.subgraphs.iter().any(|subgraph| {
        subgraph.title.contains('\n') || subgraph.nodes.iter().any(|node| node.contains('\n'))
    }) {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "flowchart",
            feature: "multiline subgraph labels",
        });
    }

    if model.nodes.iter().any(|node| {
        node.label
            .as_deref()
            .is_some_and(|label| label.contains('\n'))
    }) {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "flowchart",
            feature: "multiline node labels",
        });
    }

    if model.edges.iter().any(|edge| {
        edge.label
            .as_deref()
            .is_some_and(|label| label.contains('\n'))
    }) {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "flowchart",
            feature: "multiline edge labels",
        });
    }

    let node_ids = model
        .nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect::<HashSet<_>>();
    if model
        .edges
        .iter()
        .any(|edge| !node_ids.contains(edge.from.as_str()) || !node_ids.contains(edge.to.as_str()))
    {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "flowchart",
            feature: "edges with missing endpoint nodes",
        });
    }

    if model
        .subgraphs
        .iter()
        .flat_map(|subgraph| subgraph.nodes.iter())
        .any(|node| !node_ids.contains(node.as_str()))
    {
        return Err(AsciiError::UnsupportedFeature {
            diagram_type: "flowchart",
            feature: "subgraphs with missing member nodes",
        });
    }

    Ok(())
}
