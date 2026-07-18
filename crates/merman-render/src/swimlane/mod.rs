mod bounds;
mod config;
mod direction;
mod geometry;
mod prepare;
mod routing;
mod sugiyama;
mod working;

use crate::Result;
use crate::math::MathRenderer;
use crate::model::{
    Bounds, SwimlaneEdgeLayout, SwimlaneLaneLayout, SwimlaneLayout, SwimlaneNodeLayout,
};
use crate::text::TextMeasurer;
use merman_core::MermaidConfig;
use merman_core::diagrams::flowchart::FlowchartModel;

fn output_bounds(layout: &working::WorkingLayout) -> Option<Bounds> {
    let mut points = Vec::new();
    for node in layout
        .nodes
        .values()
        .filter(|node| node.kind != working::WorkingNodeKind::Dummy)
    {
        let width = if node.kind == working::WorkingNodeKind::EdgeLabel {
            node.label_width
        } else {
            node.width
        };
        let height = if node.kind == working::WorkingNodeKind::EdgeLabel {
            node.label_height
        } else {
            node.height
        };
        points.push((node.x - width / 2.0, node.y - height / 2.0));
        points.push((node.x + width / 2.0, node.y + height / 2.0));
    }
    for edge in &layout.original_edges {
        points.extend(edge.points.iter().map(|point| (point.x, point.y)));
    }
    Bounds::from_points(points)
}

pub fn layout_swimlane_typed(
    model: &FlowchartModel,
    effective_config: &MermaidConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
) -> Result<SwimlaneLayout> {
    let config = config::SwimlaneConfig::from_config(effective_config);
    let mut working = prepare::prepare(model, effective_config, measurer, math_renderer);
    let reversed = sugiyama::run(&mut working, config);
    for edge in &mut working.original_edges {
        edge.reversed_for_layout = reversed.contains(&edge.id);
    }
    bounds::assign_canonical_group_bounds(&mut working);
    routing::route(&mut working);
    direction::post_process(&mut working);

    let bounds = output_bounds(&working);
    let nodes = working
        .nodes
        .values()
        .filter(|node| {
            matches!(
                node.kind,
                working::WorkingNodeKind::Content | working::WorkingNodeKind::EdgeLabel
            )
        })
        .map(|node| SwimlaneNodeLayout {
            id: node.id.clone(),
            label: node.label.clone(),
            label_type: node.label_type.clone(),
            shape: node.shape.clone(),
            parent_id: node.parent_id.clone(),
            top_lane_id: node.top_lane_id.clone(),
            x: node.x,
            y: node.y,
            width: node.width,
            height: node.height,
            label_width: node.label_width,
            label_height: node.label_height,
            layer: node.layer,
            order: node.order,
            is_edge_label: node.kind == working::WorkingNodeKind::EdgeLabel,
        })
        .collect();
    let lanes = working
        .nodes
        .values()
        .filter(|node| node.kind == working::WorkingNodeKind::Group)
        .map(|lane| SwimlaneLaneLayout {
            id: lane.id.clone(),
            title: lane.label.clone(),
            parent_id: lane.parent_id.clone(),
            x: lane.x,
            y: lane.y,
            width: lane.width,
            height: lane.height,
            padding: lane.padding,
            title_label_width: lane.label_width,
            title_label_height: lane.label_height,
            content_top: lane.content_top,
            title_rect: lane.title_rect.clone(),
            requested_dir: lane.requested_dir.clone(),
        })
        .collect();
    let edges = working
        .original_edges
        .iter()
        .map(|edge| SwimlaneEdgeLayout {
            id: edge.id.clone(),
            from: edge.from.clone(),
            to: edge.to.clone(),
            points: edge.points.clone(),
            label_node_id: edge.label_node_id.clone(),
            reversed_for_layout: edge.reversed_for_layout,
            curve: "rounded".to_string(),
        })
        .collect();

    Ok(SwimlaneLayout {
        direction: working.direction,
        nodes,
        lanes,
        edges,
        bounds,
    })
}
