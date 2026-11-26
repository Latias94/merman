use super::{LinkToken, TitleKind};

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(crate) use merman_model::families::flowchart::FlowSubgraphVertexStyle;
#[cfg(feature = "presentation")]
pub(crate) use merman_model::families::flowchart::FlowchartRenderContext;
pub(crate) use merman_model::families::flowchart::{
    FlowEdge, FlowEdgeDefaults, FlowEdgeMarker, FlowEdgeStroke, FlowEdgeVisibility, FlowNode,
    FlowNodeProvenance, FlowSubgraph, FlowchartModel, FlowchartSubgraphStyles,
};

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(crate) fn sanitize_common_db_fields(model: &mut FlowchartModel, config: &crate::MermaidConfig) {
    crate::common_db::sanitize_optional_acc_title(&mut model.acc_title, config);
    crate::common_db::sanitize_optional_acc_descr(&mut model.acc_descr, config);
}

#[derive(Debug, Clone)]
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(crate) struct Node {
    pub id: String,
    pub provenance: FlowNodeProvenance,
    pub label: Option<String>,
    pub label_type: TitleKind,
    pub shape: Option<String>,
    pub icon: Option<String>,
    pub form: Option<String>,
    pub pos: Option<String>,
    pub img: Option<String>,
    pub constraint: Option<String>,
    pub asset_width: Option<f64>,
    pub asset_height: Option<f64>,
    pub styles: Vec<String>,
    pub classes: Vec<String>,
    pub link: Option<String>,
    pub link_target: Option<String>,
    pub have_callback: bool,
}

#[derive(Debug, Clone)]
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(crate) struct Edge {
    pub from: String,
    pub to: String,
    pub id: Option<String>,
    pub link: LinkToken,
    pub label: Option<String>,
    pub label_type: TitleKind,
    pub style: Vec<String>,
    pub classes: Vec<String>,
    pub interpolate: Option<String>,
    pub is_user_defined_id: bool,
    pub animate: Option<bool>,
    pub animation: Option<String>,
    pub step: Option<f64>,
}

#[derive(Debug, Clone)]
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(crate) struct EdgeDefaults {
    pub style: Vec<String>,
    pub interpolate: Option<String>,
}

#[cfg(test)]
mod subgraph_style_serialization_tests {
    use super::*;
    use merman_model::families::flowchart::FlowSubgraphVertexStyle;

    fn styles() -> FlowchartSubgraphStyles {
        let mut styles = FlowchartSubgraphStyles::default();
        styles.insert(
            "z".to_owned(),
            7,
            FlowSubgraphVertexStyle {
                classes: vec!["paint".to_owned(), "typography".to_owned()],
                styles: vec!["fill:red".to_owned(), "stroke:blue".to_owned()],
            },
        );
        styles.insert("a".to_owned(), 0, FlowSubgraphVertexStyle::default());
        styles
    }

    #[test]
    fn subgraph_styles_remain_excluded_from_the_existing_typed_wire_and_complexity() {
        let mut model: FlowchartModel = serde_json::from_str(r#"{"nodes":[],"edges":[]}"#).unwrap();
        let wire = serde_json::to_string(&model).unwrap();
        let complexity = merman_model::resources::ModelComplexity::from_serializable(&model);
        model.subgraph_vertex_styles = styles();
        assert_eq!(serde_json::to_string(&model).unwrap(), wire);
        assert_eq!(
            merman_model::resources::ModelComplexity::from_serializable(&model),
            complexity
        );
    }
}
