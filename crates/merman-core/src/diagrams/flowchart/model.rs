use crate::SourceSpan;

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(crate) use merman_model::families::flowchart::FlowSubgraphVertexStyle;
#[cfg(feature = "presentation")]
pub use merman_model::families::flowchart::FlowchartRenderContext;
pub use merman_model::families::flowchart::{
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
    pub syntax: FlowNodeSyntax,
    pub id_span: Option<SourceSpan>,
    pub label: Option<String>,
    pub label_type: TitleKind,
    pub label_span: Option<SourceSpan>,
    pub label_selection: Option<SourceSpan>,
    pub shape: Option<String>,
    pub shape_data: Option<String>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(crate) enum FlowNodeSyntax {
    BareReference,
    ExplicitDefinition,
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
    #[cfg_attr(
        not(feature = "editor-facts"),
        allow(
            dead_code,
            reason = "Shared grammar actions retain label provenance for the optional editor observer."
        )
    )]
    pub label_span: Option<SourceSpan>,
    #[cfg_attr(
        not(feature = "editor-facts"),
        allow(
            dead_code,
            reason = "Shared grammar actions retain label provenance for the optional editor observer."
        )
    )]
    pub label_selection: Option<SourceSpan>,
    pub style: Vec<String>,
    pub classes: Vec<String>,
    pub interpolate: Option<String>,
    pub is_user_defined_id: bool,
    pub animate: Option<bool>,
    pub animation: Option<String>,
}

#[derive(Debug, Clone)]
#[cfg_attr(
    not(any(feature = "diagram-flowchart", feature = "diagram-swimlane")),
    allow(
        dead_code,
        reason = "Agentflow shares presentation lexing but does not consume Flowchart grammar and recovery payloads."
    )
)]
pub(crate) struct LinkToken {
    pub end: String,
    pub start_marker: FlowEdgeMarker,
    pub end_marker: FlowEdgeMarker,
    pub stroke_kind: FlowEdgeStroke,
    pub visibility: FlowEdgeVisibility,
    pub length: usize,
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
impl LinkToken {
    pub(crate) const fn compatibility_edge_type(&self) -> &'static str {
        match (self.start_marker, self.end_marker) {
            (FlowEdgeMarker::Point, FlowEdgeMarker::Point) => "double_arrow_point",
            (FlowEdgeMarker::Circle, FlowEdgeMarker::Circle) => "double_arrow_circle",
            (FlowEdgeMarker::Cross, FlowEdgeMarker::Cross) => "double_arrow_cross",
            (_, FlowEdgeMarker::Point) => "arrow_point",
            (_, FlowEdgeMarker::Circle) => "arrow_circle",
            (_, FlowEdgeMarker::Cross) => "arrow_cross",
            (_, FlowEdgeMarker::None) => "arrow_open",
        }
    }

    pub(crate) const fn compatibility_stroke(&self) -> &'static str {
        if matches!(self.visibility, FlowEdgeVisibility::Invisible) {
            return "invisible";
        }
        match self.stroke_kind {
            FlowEdgeStroke::Normal => "normal",
            FlowEdgeStroke::Dotted => "dotted",
            FlowEdgeStroke::Thick => "thick",
        }
    }
}

#[derive(Debug, Clone)]
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(crate) struct EdgeDefaults {
    pub style: Vec<String>,
    pub interpolate: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TitleKind {
    Text,
    String,
    Markdown,
}

#[derive(Debug, Clone)]
pub(crate) struct LabeledText {
    pub text: String,
    pub kind: TitleKind,
    pub span: Option<SourceSpan>,
    pub selection: Option<SourceSpan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SubgraphHeader {
    pub raw_id: String,
    pub header_span: Option<SourceSpan>,
    pub raw_id_span: Option<SourceSpan>,
    pub raw_title: String,
    pub title_kind: TitleKind,
    pub id_equals_title: bool,
}

impl Default for SubgraphHeader {
    fn default() -> Self {
        Self {
            raw_id: String::new(),
            header_span: None,
            raw_id_span: None,
            raw_title: String::new(),
            title_kind: TitleKind::Text,
            id_equals_title: true,
        }
    }
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
        let complexity = crate::resources::ModelComplexity::from_serializable(&model);
        model.subgraph_vertex_styles = styles();
        assert_eq!(serde_json::to_string(&model).unwrap(), wire);
        assert_eq!(
            crate::resources::ModelComplexity::from_serializable(&model),
            complexity
        );
    }
}
