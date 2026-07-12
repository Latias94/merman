use super::FlowEdge;

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
    );
    let edge_mid = flowchart_self_loop_edge_from_base(
        base,
        format!("{node_id}-cyclic-special-mid"),
        special_id_1.clone(),
        special_id_2.clone(),
        base.label.clone(),
        base.label_type.clone(),
        Some("arrow_open".to_string()),
    );
    let edge2 = flowchart_self_loop_edge_from_base(
        base,
        format!("{node_id}-cyclic-special-2"),
        special_id_2.clone(),
        node_id.to_string(),
        endpoint_label,
        base.label_type.clone(),
        base.edge_type.clone(),
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
) -> FlowEdge {
    FlowEdge {
        id,
        from,
        to,
        label,
        label_type,
        edge_type,
        stroke: base.stroke.clone(),
        interpolate: base.interpolate.clone(),
        classes: base.classes.clone(),
        style: base.style.clone(),
        animate: base.animate,
        animation: base.animation.clone(),
        length: base.length,
    }
}
