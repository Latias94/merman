#![forbid(unsafe_code)]
//! Terminal-friendly ASCII and Unicode rendering for Mermaid typed models.
//!
//! `merman-ascii` is deliberately model-driven: callers parse Mermaid text with `merman-core`, then
//! pass the resulting typed render model into this crate. The renderer does not own Mermaid syntax
//! parsing.

mod canvas;
mod error;
mod graph;
mod options;
mod sequence;
mod text;

pub use error::{AsciiError, Result};
pub use options::{AsciiCharset, AsciiDirection, AsciiRenderOptions};

use merman_core::diagram::RenderSemanticModel;
use merman_core::diagrams::flowchart::FlowchartV2Model;
use merman_core::diagrams::sequence::SequenceDiagramRenderModel;

#[derive(Debug, Clone, Default)]
pub struct AsciiRenderer {
    options: AsciiRenderOptions,
}

impl AsciiRenderer {
    pub fn new(options: AsciiRenderOptions) -> Result<Self> {
        options.validate()?;
        Ok(Self { options })
    }

    pub fn options(&self) -> &AsciiRenderOptions {
        &self.options
    }

    pub fn render_model(&self, model: &RenderSemanticModel) -> Result<String> {
        render_model(model, &self.options)
    }
}

pub fn render_model(model: &RenderSemanticModel, options: &AsciiRenderOptions) -> Result<String> {
    options.validate()?;
    match model {
        RenderSemanticModel::Flowchart(model) => render_flowchart(model, options),
        RenderSemanticModel::Sequence(model) => render_sequence(model, options),
        other => Err(AsciiError::UnsupportedDiagram {
            diagram_type: other.kind().to_string(),
        }),
    }
}

pub fn render_flowchart(model: &FlowchartV2Model, options: &AsciiRenderOptions) -> Result<String> {
    options.validate()?;
    let graph = graph::from_flowchart_model(model, options)?;
    graph::render_graph(&graph, options)
}

pub fn render_sequence(
    model: &SequenceDiagramRenderModel,
    options: &AsciiRenderOptions,
) -> Result<String> {
    options.validate()?;
    let diagram = sequence::from_sequence_model(model)?;
    sequence::render_sequence_diagram(&diagram, options)
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_core::diagrams::flowchart::{FlowEdge, FlowNode, FlowSubgraph, FlowchartV2Model};

    fn empty_flowchart() -> FlowchartV2Model {
        FlowchartV2Model {
            acc_descr: None,
            acc_title: None,
            class_defs: Default::default(),
            direction: None,
            edge_defaults: None,
            vertex_calls: Vec::new(),
            nodes: Vec::new(),
            edges: Vec::new(),
            subgraphs: Vec::new(),
            tooltips: Default::default(),
        }
    }

    fn node(id: &str) -> FlowNode {
        FlowNode {
            id: id.to_string(),
            label: Some(id.to_string()),
            label_type: None,
            layout_shape: None,
            icon: None,
            form: None,
            pos: None,
            img: None,
            constraint: None,
            asset_width: None,
            asset_height: None,
            classes: Vec::new(),
            styles: Vec::new(),
            link: None,
            link_target: None,
            have_callback: false,
        }
    }

    fn edge(from: &str, to: &str) -> FlowEdge {
        FlowEdge {
            id: format!("{from}-{to}"),
            from: from.to_string(),
            to: to.to_string(),
            label: None,
            label_type: None,
            edge_type: None,
            stroke: None,
            interpolate: None,
            classes: Vec::new(),
            style: Vec::new(),
            animate: None,
            animation: None,
            length: 1,
        }
    }

    #[test]
    fn default_options_match_initial_reference_defaults() {
        let options = AsciiRenderOptions::default();
        assert_eq!(options.charset, AsciiCharset::Unicode);
        assert_eq!(options.fallback_direction, AsciiDirection::LeftRight);
        assert_eq!(options.box_border_padding, 1);
        assert_eq!(options.graph_padding_x, 5);
        assert_eq!(options.graph_padding_y, 5);
        assert_eq!(options.sequence_participant_spacing, 5);
        assert_eq!(options.sequence_message_spacing, 1);
        assert_eq!(options.sequence_self_message_width, 4);
    }

    #[test]
    fn validates_sequence_self_message_width() {
        let options = AsciiRenderOptions {
            sequence_self_message_width: 1,
            ..AsciiRenderOptions::default()
        };

        assert_eq!(
            options.validate(),
            Err(AsciiError::InvalidOption {
                field: "sequence_self_message_width",
                message: "must be at least 2",
            })
        );
    }

    #[test]
    fn render_model_routes_basic_flowchart_to_graph_renderer() {
        let model = RenderSemanticModel::Flowchart(empty_flowchart());

        let rendered = render_model(&model, &AsciiRenderOptions::default()).unwrap();

        assert_eq!(rendered, "");
    }

    #[test]
    fn render_flowchart_renders_basic_left_right_chain() {
        let mut model = empty_flowchart();
        model.direction = Some("LR".to_string());
        model.nodes = vec![node("A"), node("B")];
        model.edges = vec![edge("A", "B")];

        let rendered = render_flowchart(&model, &AsciiRenderOptions::ascii()).unwrap();

        assert_eq!(
            rendered,
            "+---+     +---+\n|   |     |   |\n| A |---->| B |\n|   |     |   |\n+---+     +---+\n"
        );
    }

    #[test]
    fn render_flowchart_respects_grid_cell_limit() {
        let mut model = empty_flowchart();
        model.nodes = vec![node("A"), node("B")];
        model.edges = vec![edge("A", "B")];
        let options = AsciiRenderOptions {
            max_grid_cells: 1,
            ..AsciiRenderOptions::ascii()
        };

        let err = render_flowchart(&model, &options).unwrap_err();

        assert_eq!(
            err,
            AsciiError::RenderLimitExceeded {
                actual: 75,
                limit: 1,
            }
        );
    }

    #[test]
    fn render_flowchart_rejects_unsupported_edge_labels() {
        let mut model = empty_flowchart();
        model.nodes = vec![node("A"), node("B")];
        model.edges = vec![FlowEdge {
            label: Some("label".to_string()),
            ..edge("A", "B")
        }];

        let err = render_flowchart(&model, &AsciiRenderOptions::ascii()).unwrap_err();

        assert_eq!(
            err,
            AsciiError::UnsupportedFeature {
                diagram_type: "flowchart",
                feature: "edge labels",
            }
        );
    }

    #[test]
    fn render_flowchart_rejects_unsupported_subgraphs() {
        let mut model = empty_flowchart();
        model.nodes = vec![node("A")];
        model.subgraphs = vec![FlowSubgraph {
            id: "cluster".to_string(),
            title: "cluster".to_string(),
            dir: None,
            label_type: None,
            classes: Vec::new(),
            styles: Vec::new(),
            nodes: vec!["A".to_string()],
        }];

        let err = render_flowchart(&model, &AsciiRenderOptions::ascii()).unwrap_err();

        assert_eq!(
            err,
            AsciiError::UnsupportedFeature {
                diagram_type: "flowchart",
                feature: "subgraphs",
            }
        );
    }

    #[test]
    fn render_flowchart_rejects_unsupported_directions() {
        let mut model = empty_flowchart();
        model.direction = Some("BT".to_string());
        model.nodes = vec![node("A")];

        let err = render_flowchart(&model, &AsciiRenderOptions::ascii()).unwrap_err();

        assert_eq!(
            err,
            AsciiError::UnsupportedFeature {
                diagram_type: "flowchart",
                feature: "non-LR/TD graph directions",
            }
        );
    }

    #[test]
    fn render_flowchart_rejects_edges_with_missing_endpoint_nodes() {
        let mut model = empty_flowchart();
        model.nodes = vec![node("A")];
        model.edges = vec![edge("A", "B")];

        let err = render_flowchart(&model, &AsciiRenderOptions::ascii()).unwrap_err();

        assert_eq!(
            err,
            AsciiError::UnsupportedFeature {
                diagram_type: "flowchart",
                feature: "edges with missing endpoint nodes",
            }
        );
    }
}
