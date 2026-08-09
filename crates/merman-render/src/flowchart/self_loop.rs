use super::FlowEdge;
use merman_core::diagrams::flowchart::FlowEdgeMarker;

pub(crate) struct FlowchartSelfLoopHelperEdges {
    pub(crate) special_id_1: String,
    pub(crate) special_id_2: String,
    pub(crate) edge1: FlowEdge,
    pub(crate) edge_mid: FlowEdge,
    pub(crate) edge2: FlowEdge,
}

pub(crate) fn flowchart_self_loop_helper_edges(base: &FlowEdge) -> FlowchartSelfLoopHelperEdges {
    let node_id = base.from.as_str();
    let special_id_1 = format!("{node_id}---{node_id}---1");
    let special_id_2 = format!("{node_id}---{node_id}---2");
    let endpoint_label = Some(String::new());

    let edge1 = flowchart_self_loop_edge_from_base(
        base,
        format!("{node_id}-cyclic-special-1"),
        node_id.to_string(),
        special_id_1.clone(),
        endpoint_label.clone(),
        None,
        Some("arrow_open".to_string()),
        base.start_marker,
        FlowEdgeMarker::None,
    );
    let edge_mid = flowchart_self_loop_edge_from_base(
        base,
        format!("{node_id}-cyclic-special-mid"),
        special_id_1.clone(),
        special_id_2.clone(),
        base.label.clone(),
        base.label_type.clone(),
        Some("arrow_open".to_string()),
        FlowEdgeMarker::None,
        FlowEdgeMarker::None,
    );
    let edge2 = flowchart_self_loop_edge_from_base(
        base,
        format!("{node_id}-cyclic-special-2"),
        special_id_2.clone(),
        node_id.to_string(),
        endpoint_label,
        base.label_type.clone(),
        base.edge_type.clone(),
        FlowEdgeMarker::None,
        base.end_marker,
    );

    FlowchartSelfLoopHelperEdges {
        special_id_1,
        special_id_2,
        edge1,
        edge_mid,
        edge2,
    }
}

fn flowchart_self_loop_edge_from_base(
    base: &FlowEdge,
    id: String,
    from: String,
    to: String,
    label: Option<String>,
    label_type: Option<String>,
    edge_type: Option<String>,
    start_marker: FlowEdgeMarker,
    end_marker: FlowEdgeMarker,
) -> FlowEdge {
    FlowEdge {
        id,
        from,
        to,
        label,
        label_type,
        edge_type,
        arrow: base.arrow.clone(),
        start_marker,
        end_marker,
        is_user_defined_id: false,
        stroke: base.stroke.clone(),
        stroke_kind: base.stroke_kind,
        visibility: base.visibility,
        interpolate: base.interpolate.clone(),
        classes: base.classes.clone(),
        style: base.style.clone(),
        animate: base.animate,
        animation: base.animation.clone(),
        length: base.length,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_core::diagrams::flowchart::{FlowEdgeStroke, FlowEdgeVisibility};

    fn self_loop(start_marker: FlowEdgeMarker, end_marker: FlowEdgeMarker) -> FlowEdge {
        FlowEdge {
            id: "L_A_A_0".to_string(),
            from: "A".to_string(),
            to: "A".to_string(),
            label: Some("loop".to_string()),
            label_type: Some("text".to_string()),
            edge_type: Some("arrow_cross".to_string()),
            arrow: "o--x".to_string(),
            start_marker,
            end_marker,
            is_user_defined_id: false,
            stroke: Some("normal".to_string()),
            stroke_kind: FlowEdgeStroke::Normal,
            visibility: FlowEdgeVisibility::Visible,
            interpolate: None,
            classes: Vec::new(),
            style: Vec::new(),
            animate: None,
            animation: None,
            length: 1,
        }
    }

    #[test]
    fn self_loop_helpers_keep_endpoint_markers_on_the_outer_segments_only() {
        for (start_marker, end_marker) in [
            (FlowEdgeMarker::Circle, FlowEdgeMarker::Cross),
            (FlowEdgeMarker::Point, FlowEdgeMarker::Point),
        ] {
            let helpers = flowchart_self_loop_helper_edges(&self_loop(start_marker, end_marker));

            assert_eq!(helpers.edge1.start_marker, start_marker);
            assert_eq!(helpers.edge1.end_marker, FlowEdgeMarker::None);
            assert_eq!(helpers.edge_mid.start_marker, FlowEdgeMarker::None);
            assert_eq!(helpers.edge_mid.end_marker, FlowEdgeMarker::None);
            assert_eq!(helpers.edge2.start_marker, FlowEdgeMarker::None);
            assert_eq!(helpers.edge2.end_marker, end_marker);
        }
    }
}
