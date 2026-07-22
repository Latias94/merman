#![forbid(unsafe_code)]

//! Headless layout + rendering for Mermaid diagrams.
//!
//! This crate consumes `merman-core`'s semantic models and produces:
//! - a layout JSON (geometry + routes)
//! - Mermaid-like SVG output with DOM parity checks against upstream baselines

extern crate self as web_time;

#[cfg(feature = "cytoscape-layout")]
pub mod architecture;
#[cfg(feature = "cytoscape-layout")]
pub(crate) mod architecture_metrics;
pub mod block;
pub mod c4;
mod chart_palette;
pub mod class;
mod config;
pub mod cynefin;
mod dagre;
mod entities;
pub mod environment;
pub mod er;
pub mod error;
pub mod eventmodeling;
pub mod family;
pub mod flowchart;
pub mod gantt;
mod generated;
pub mod gitgraph;
mod host_time;
pub mod info;
pub mod ishikawa;
pub mod journey;
pub mod kanban;
pub mod math;
mod mermaid_style;
#[cfg(feature = "cytoscape-layout")]
pub mod mindmap;
pub mod model;
pub mod packet;
pub mod pie;
pub mod quadrantchart;
pub mod radar;
pub mod railroad;
pub mod requirement;
pub mod resources;
pub mod sankey;
pub mod sequence;
pub mod state;
pub mod svg;
pub mod swimlane;
pub mod text;
mod theme;
pub mod timeline;
pub mod tree_view;
pub mod treemap;
mod trig_tables;
pub mod venn;
pub mod wardley;
mod xml;
pub mod xychart;
pub mod zenuml;

pub(crate) use host_time::{Duration, Instant};

use crate::environment::{RenderSession, RoutedTextMeasurer, TextMeasurementPhase};
use merman_core::diagrams::flowchart::FlowchartModel;
use merman_core::models::class_diagram::ClassDiagram;

pub use resources::{
    CLI_DEFAULT_RESOURCE_PROFILE, ClassComplexity, FlowchartComplexity,
    GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE, RESOURCE_CONTRACT_SCHEMA_VERSION,
    RenderResourcePolicy, RenderResourceProfile, RenderResourceProfileDescriptor,
    ResourceLimitDescriptor, ResourceLimitExceeded, ResourceLimitId, ResourceLimitOverride,
    ResourceLimitOverrideError, ResourceLimitPhase, ResourceProfileValues, ZenumlComplexity,
    resource_limit_descriptors, resource_profile_descriptors,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unsupported diagram type for layout: {diagram_type}")]
    UnsupportedDiagram { diagram_type: String },
    #[error("invalid semantic model: {message}")]
    InvalidModel { message: String },
    #[error(
        "custom JSON model `{model_name}` from {provenance:?} cannot render diagram type `{diagram_type}`"
    )]
    NonRenderableCustomModel {
        diagram_type: String,
        model_name: String,
        provenance: merman_core::CustomJsonProvenance,
    },
    #[error("SVG postprocessor `{pass}` failed: {message}")]
    SvgPostprocess { pass: String, message: String },
    #[error(transparent)]
    ResourceLimitExceeded(#[from] ResourceLimitExceeded),
    #[error(transparent)]
    Color(#[from] merman_core::theme_color::ColorError),
    #[error("semantic model JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn svg_postprocess(pass: impl Into<String>, message: impl Into<String>) -> Self {
        Self::SvgPostprocess {
            pass: pass.into(),
            message: message.into(),
        }
    }
}

/// Host-provided geometry available to family layout algorithms.
///
/// This models the element that owns diagram layout, not a browser page viewport and not the
/// final SVG viewport emitted after layout.
#[derive(Debug, Clone)]
pub struct LayoutOptions {
    /// Width of the host layout container in CSS pixels.
    ///
    /// Families whose Mermaid renderer reads DOM-available width use this value.
    pub container_width: f64,
    /// Height of the host layout container in CSS pixels.
    pub container_height: f64,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        Self {
            container_width: 800.0,
            container_height: 600.0,
        }
    }
}

impl LayoutOptions {
    /// Returns geometry defaults suitable for headless SVG rendering.
    pub fn headless_svg_defaults() -> Self {
        Self::default()
    }
}

pub(crate) struct LayoutExecution<'a> {
    request: &'a LayoutOptions,
    session: &'a RenderSession,
    text_measurer: RoutedTextMeasurer<'a>,
}

impl<'a> LayoutExecution<'a> {
    pub(crate) fn new(request: &'a LayoutOptions, session: &'a RenderSession) -> Self {
        Self {
            request,
            session,
            text_measurer: session.text_measurer(TextMeasurementPhase::Layout),
        }
    }

    pub(crate) fn text_measurer(&self) -> &dyn crate::text::TextMeasurer {
        &self.text_measurer
    }

    pub(crate) fn math_renderer(&self) -> Option<&(dyn crate::math::MathRenderer + Send + Sync)> {
        self.session.math_renderer()
    }

    pub(crate) const fn resource_policy(&self) -> RenderResourcePolicy {
        self.session.resource_policy()
    }

    #[cfg(feature = "cytoscape-layout")]
    pub(crate) const fn operation_seed(&self) -> u64 {
        self.session.seed().seed().get()
    }
}

impl std::ops::Deref for LayoutExecution<'_> {
    type Target = LayoutOptions;

    fn deref(&self) -> &Self::Target {
        self.request
    }
}

fn uses_elk_layout(effective_config: &merman_core::MermaidConfig) -> bool {
    effective_config.get_str("layout") == Some("elk")
}

pub(crate) fn layout_class_typed_by_engine(
    diagram_type: &str,
    model: &ClassDiagram,
    effective_config: &merman_core::MermaidConfig,
    options: &LayoutExecution<'_>,
) -> Result<model::ClassDiagramLayout> {
    if uses_elk_layout(effective_config) {
        return layout_class_elk_typed_by_feature(diagram_type, model, effective_config, options);
    }

    options.resource_policy().check_class_complexity(model)?;
    class::layout_class_diagram_typed_with_config(model, effective_config, options.text_measurer())
}

#[cfg(feature = "elk-layout")]
fn layout_class_elk_typed_by_feature(
    _diagram_type: &str,
    model: &ClassDiagram,
    effective_config: &merman_core::MermaidConfig,
    options: &LayoutExecution<'_>,
) -> Result<model::ClassDiagramLayout> {
    options.resource_policy().check_class_complexity(model)?;
    class::layout_class_diagram_elk_typed_with_config(
        model,
        effective_config,
        options.text_measurer(),
    )
}

#[cfg(not(feature = "elk-layout"))]
fn layout_class_elk_typed_by_feature(
    diagram_type: &str,
    _model: &ClassDiagram,
    _effective_config: &merman_core::MermaidConfig,
    _options: &LayoutExecution<'_>,
) -> Result<model::ClassDiagramLayout> {
    Err(Error::UnsupportedDiagram {
        diagram_type: diagram_type.to_string(),
    })
}

pub(crate) fn layout_flowchart_typed_by_engine(
    diagram_type: &str,
    model: &FlowchartModel,
    effective_config: &merman_core::MermaidConfig,
    options: &LayoutExecution<'_>,
) -> Result<model::FlowchartLayout> {
    if uses_elk_layout(effective_config) {
        return layout_flowchart_elk_typed_by_feature(
            diagram_type,
            model,
            effective_config,
            options,
        );
    }

    flowchart::layout_flowchart_typed(
        model,
        effective_config,
        options.text_measurer(),
        options.math_renderer(),
    )
}

#[cfg(feature = "elk-layout")]
fn layout_flowchart_elk_typed_by_feature(
    _diagram_type: &str,
    model: &FlowchartModel,
    effective_config: &merman_core::MermaidConfig,
    options: &LayoutExecution<'_>,
) -> Result<model::FlowchartLayout> {
    flowchart::elk::layout_flowchart_elk_typed(
        model,
        effective_config,
        options.text_measurer(),
        options.math_renderer(),
    )
}

#[cfg(not(feature = "elk-layout"))]
fn layout_flowchart_elk_typed_by_feature(
    diagram_type: &str,
    _model: &FlowchartModel,
    _effective_config: &merman_core::MermaidConfig,
    _options: &LayoutExecution<'_>,
) -> Result<model::FlowchartLayout> {
    Err(Error::UnsupportedDiagram {
        diagram_type: diagram_type.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_core::{Engine, ParseOptions};
    #[cfg(feature = "elk-layout")]
    use merman_core::{ParsedDiagramRender, RenderSemanticModel};

    #[cfg(feature = "elk-layout")]
    fn flowchart_layout(
        parsed: &ParsedDiagramRender,
        options: &LayoutOptions,
        session: &RenderSession,
    ) -> model::FlowchartLayout {
        let RenderSemanticModel::Flowchart(model) = parsed.model() else {
            panic!("expected flowchart render model");
        };
        layout_flowchart_typed_by_engine(
            &parsed.metadata().diagram_type,
            model,
            &parsed.metadata().effective_config,
            &LayoutExecution::new(options, session),
        )
        .expect("flowchart layout")
    }

    #[cfg(feature = "elk-layout")]
    fn class_layout(
        parsed: &ParsedDiagramRender,
        options: &LayoutOptions,
        session: &RenderSession,
    ) -> model::ClassDiagramLayout {
        let RenderSemanticModel::Class(model) = parsed.model() else {
            panic!("expected class render model");
        };
        layout_class_typed_by_engine(
            &parsed.metadata().diagram_type,
            model,
            &parsed.metadata().effective_config,
            &LayoutExecution::new(options, session),
        )
        .expect("class layout")
    }

    fn render_source(
        source: &str,
        layout_options: &LayoutOptions,
        svg_options: &crate::svg::SvgRenderOptions,
    ) -> String {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .expect("parse")
            .expect("diagram");
        let session = crate::environment::RenderEnvironment::parity()
            .begin_session()
            .unwrap();
        crate::family::prepare(parsed, layout_options, session)
            .expect("prepare")
            .render_svg(svg_options, &crate::svg::SvgDebugOptions::default())
            .expect("render")
            .svg()
            .to_owned()
    }

    #[cfg(feature = "elk-layout")]
    #[test]
    fn render_model_dispatch_accepts_diagram_type_aliases() {
        let session = crate::environment::RenderEnvironment::parity()
            .begin_session()
            .unwrap();
        let parsed = Engine::new()
            .parse_diagram_for_render_model_with_type_sync(
                "flowchart-elk",
                "flowchart-elk TD\nA-->B;",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();

        let artifact = crate::family::prepare(parsed, &LayoutOptions::default(), session).unwrap();
        assert_eq!(
            artifact.family_kind(),
            crate::family::RenderFamilyKind::Flowchart
        );
    }

    #[cfg(feature = "elk-layout")]
    #[test]
    fn render_model_dispatch_uses_elk_for_flowchart_default_renderer_config() {
        let session = crate::environment::RenderEnvironment::parity()
            .begin_session()
            .unwrap();
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                r#"---
config:
  flowchart:
    defaultRenderer: elk
---
flowchart TD
A-->B
"#,
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();

        assert_eq!(parsed.metadata().diagram_type, "flowchart-elk");
        let layout = flowchart_layout(&parsed, &LayoutOptions::default(), &session);
        let a = layout.nodes.iter().find(|node| node.id == "A").unwrap();
        let b = layout.nodes.iter().find(|node| node.id == "B").unwrap();
        assert!(b.y > a.y);
    }

    #[cfg(feature = "elk-layout")]
    #[test]
    fn render_model_dispatch_rejects_flowchart_over_node_resource_limit() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_with_type_sync(
                "flowchart-elk",
                "flowchart-elk TD\nA-->B;",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let options = LayoutOptions::default();
        let session = crate::environment::RenderEnvironment::parity()
            .with_resource_policy(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxFlowchartNodes, 1)
                    .unwrap(),
            )
            .begin_session()
            .unwrap();

        let err = match crate::family::prepare(parsed, &options, session) {
            Err(error) => error,
            Ok(_) => panic!("expected resource limit error"),
        };

        let Error::ResourceLimitExceeded(limit) = err else {
            panic!("expected resource limit error");
        };
        assert_eq!(limit.phase, ResourceLimitPhase::LayoutModel);
        assert_eq!(limit.limit, "max_flowchart_nodes");
    }

    #[cfg(feature = "elk-layout")]
    #[test]
    fn render_model_dispatch_uses_elk_for_class_layout_config() {
        let session = crate::environment::RenderEnvironment::parity()
            .begin_session()
            .unwrap();
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                r#"---
config:
  layout: elk
---
classDiagram
direction LR
Animal <|-- Duck
"#,
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();

        assert_eq!(parsed.metadata().diagram_type, "class");
        let layout = class_layout(&parsed, &LayoutOptions::default(), &session);
        let animal = layout
            .nodes
            .iter()
            .find(|node| node.id == "Animal")
            .unwrap();
        let duck = layout.nodes.iter().find(|node| node.id == "Duck").unwrap();
        assert!(
            duck.x > animal.x,
            "ELK LR class layout should place Duck to the right of Animal; Animal={}, Duck={}",
            animal.x,
            duck.x
        );
    }

    #[cfg(feature = "elk-layout")]
    #[test]
    fn render_model_dispatch_rejects_class_over_node_resource_limit() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "classDiagram\nAnimal <|-- Duck",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let options = LayoutOptions::default();
        let session = crate::environment::RenderEnvironment::parity()
            .with_resource_policy(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxClassNodes, 1)
                    .unwrap(),
            )
            .begin_session()
            .unwrap();

        let err = match crate::family::prepare(parsed, &options, session) {
            Err(error) => error,
            Ok(_) => panic!("expected resource limit error"),
        };

        let Error::ResourceLimitExceeded(limit) = err else {
            panic!("expected resource limit error");
        };
        assert_eq!(limit.phase, ResourceLimitPhase::LayoutModel);
        assert_eq!(limit.limit, "max_class_nodes");
    }

    #[cfg(feature = "elk-layout")]
    #[test]
    fn typed_dispatch_rejects_flowchart_over_edge_resource_limit() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "flowchart TD\nA-->B\nB-->C\nC-->D",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let options = LayoutOptions::default();
        let session = crate::environment::RenderEnvironment::parity()
            .with_resource_policy(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxFlowchartEdges, 2)
                    .unwrap(),
            )
            .begin_session()
            .unwrap();

        let err = match crate::family::prepare(parsed, &options, session) {
            Err(error) => error,
            Ok(_) => panic!("expected resource limit error"),
        };

        let Error::ResourceLimitExceeded(limit) = err else {
            panic!("expected resource limit error");
        };
        assert_eq!(limit.phase, ResourceLimitPhase::LayoutModel);
        assert_eq!(limit.limit, "max_flowchart_edges");
    }

    #[cfg(feature = "elk-layout")]
    #[test]
    fn typed_dispatch_rejects_class_over_edge_resource_limit() {
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                "classDiagram\nAnimal <|-- Duck\nDuck <|-- Mallard",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let options = LayoutOptions::default();
        let session = crate::environment::RenderEnvironment::parity()
            .with_resource_policy(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(ResourceLimitId::MaxClassEdges, 1)
                    .unwrap(),
            )
            .begin_session()
            .unwrap();

        let err = match crate::family::prepare(parsed, &options, session) {
            Err(error) => error,
            Ok(_) => panic!("expected resource limit error"),
        };

        let Error::ResourceLimitExceeded(limit) = err else {
            panic!("expected resource limit error");
        };
        assert_eq!(limit.phase, ResourceLimitPhase::LayoutModel);
        assert_eq!(limit.limit, "max_class_edges");
    }

    #[cfg(feature = "elk-layout")]
    #[test]
    fn canonical_svg_preserves_flowchart_elk_roledescription() {
        let svg = render_source(
            "flowchart-elk TD\nA-->B;",
            &LayoutOptions::default(),
            &crate::svg::SvgRenderOptions {
                diagram_id: Some("elk-smoke".to_string()),
                ..Default::default()
            },
        );

        assert!(svg.contains(r#"aria-roledescription="flowchart-elk""#));
        assert!(svg.contains("elk-smoke_flowchart-elk-pointEnd"));
        assert!(!svg.contains(r#"aria-roledescription="flowchart-v2""#));
        assert!(!svg.contains(r#"<g class="root""#));

        let marker_pos = svg
            .find(r#"<g><marker id="elk-smoke_flowchart-elk-pointEnd""#)
            .expect("ELK marker group");
        let defs_pos = svg
            .find(r#"<defs><filter id="elk-smoke-drop-shadow""#)
            .expect("ELK shadow defs");
        let subgraphs_pos = svg
            .find(r#"<g class="subgraphs"/>"#)
            .expect("ELK subgraphs group");
        let nodes_pos = svg.find(r#"<g class="nodes">"#).expect("ELK nodes group");
        let edges_pos = svg
            .find(r#"<g class="edges edgePaths">"#)
            .expect("ELK edge paths group");
        let labels_pos = svg
            .find(r#"<g class="edgeLabels">"#)
            .expect("ELK edge labels group");

        assert!(marker_pos < defs_pos);
        assert!(defs_pos < subgraphs_pos);
        assert!(subgraphs_pos < nodes_pos);
        assert!(nodes_pos < edges_pos);
        assert!(edges_pos < labels_pos);
    }

    #[cfg(feature = "elk-layout")]
    #[test]
    fn canonical_svg_uses_elk_adapter_dom_for_flowchart_layout_elk() {
        let svg = render_source(
            r#"---
config:
  layout: elk
---
flowchart LR
A{A} --> B & C
"#,
            &LayoutOptions::default(),
            &crate::svg::SvgRenderOptions {
                diagram_id: Some("elk-layout-smoke".to_string()),
                ..Default::default()
            },
        );

        assert!(svg.contains(r#"aria-roledescription="flowchart-v2""#));
        assert!(svg.contains("elk-layout-smoke_flowchart-v2-pointEnd"));
        assert!(!svg.contains(r#"<g class="root""#));

        let marker_pos = svg
            .find(r#"<g><marker id="elk-layout-smoke_flowchart-v2-pointEnd""#)
            .expect("ELK marker group");
        let defs_pos = svg
            .find(r#"<defs><filter id="elk-layout-smoke-drop-shadow""#)
            .expect("ELK shadow defs");
        let subgraphs_pos = svg
            .find(r#"<g class="subgraphs"/>"#)
            .expect("ELK subgraphs group");
        let nodes_pos = svg.find(r#"<g class="nodes">"#).expect("ELK nodes group");
        let edges_pos = svg
            .find(r#"<g class="edges edgePaths">"#)
            .expect("ELK edge paths group");
        let labels_pos = svg
            .find(r#"<g class="edgeLabels">"#)
            .expect("ELK edge labels group");

        assert!(marker_pos < defs_pos);
        assert!(defs_pos < subgraphs_pos);
        assert!(subgraphs_pos < nodes_pos);
        assert!(nodes_pos < edges_pos);
        assert!(edges_pos < labels_pos);
    }

    #[cfg(feature = "elk-layout")]
    #[test]
    fn canonical_svg_uses_right_angle_edges_for_flowchart_elk() {
        let svg = render_source(
            "flowchart-elk LR\nA --> B\nA --> C",
            &LayoutOptions::default(),
            &crate::svg::SvgRenderOptions::default(),
        );

        let path = edge_path_chunk(&svg, "L_A_B_0");
        let d = edge_path_d(path);
        assert!(
            d.contains('L') && !d.contains('C'),
            "expected ELK edges to use right-angle paths without smooth curves by default: {d}"
        );
    }

    #[cfg(feature = "elk-layout")]
    #[test]
    fn canonical_svg_keeps_source_ported_elk_rect_edge_boundary_points() {
        let svg = render_source(
            r#"---
config:
  htmlLabels: true
  flowchart:
    htmlLabels: true
  securityLevel: loose
---
flowchart-elk LR
id1(Start)-->id2(Stop)
"#,
            &LayoutOptions::default(),
            &crate::svg::SvgRenderOptions::default(),
        );

        let path = edge_path_chunk(&svg, "L_id1_id2_0");
        let d = edge_path_d(path);
        assert!(
            !d.contains('Q'),
            "straight ELK roundedRect edge should not gain a rounded corner: {d}"
        );
        let points = edge_data_points(path);
        assert_eq!(
            points.len(),
            2,
            "unexpected ELK edge data-points: {points:?}"
        );
        assert_eq!(points[0], (77.015625, 39.0));
        assert_eq!(points[1], (117.015625, 39.0));
    }

    #[cfg(feature = "elk-layout")]
    #[test]
    fn canonical_svg_keeps_source_ported_elk_self_loop_edges() {
        let svg = render_source(
            "flowchart-elk TD\nA --> A",
            &LayoutOptions::default(),
            &crate::svg::SvgRenderOptions::default(),
        );

        let path = edge_path_chunk(&svg, "L_A_A_0");
        let d = edge_path_d(path);
        assert!(
            d.contains('Q'),
            "ELK self-loop path should be rendered from the source-backed edge: {d}"
        );
        let points = edge_data_points(path);
        assert_eq!(
            points.len(),
            4,
            "unexpected ELK self-loop data-points: {points:?}"
        );
        assert!(
            !svg.contains("A---A---1") && !svg.contains("cyclic-special"),
            "ELK renderer must not reuse Dagre self-loop helper nodes: {svg}"
        );
        assert!(svg.contains(r#"data-id="L_A_A_0" transform="translate(0,0)""#));
    }

    #[cfg(not(feature = "elk-layout"))]
    #[test]
    fn render_model_dispatch_rejects_flowchart_elk_without_feature() {
        let session = crate::environment::RenderEnvironment::parity()
            .begin_session()
            .unwrap();
        let parsed = Engine::new()
            .parse_diagram_for_render_model_with_type_sync(
                "flowchart-elk",
                "flowchart-elk TD\nA-->B;",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();

        let err = match crate::family::prepare(parsed, &LayoutOptions::default(), session) {
            Err(error) => error,
            Ok(_) => panic!("expected unsupported diagram error"),
        };
        assert!(matches!(
            err,
            Error::UnsupportedDiagram { diagram_type } if diagram_type == "flowchart-elk"
        ));
    }

    #[cfg(not(feature = "elk-layout"))]
    #[test]
    fn render_model_dispatch_rejects_class_elk_without_feature() {
        let session = crate::environment::RenderEnvironment::parity()
            .begin_session()
            .unwrap();
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(
                r#"---
config:
  layout: elk
---
classDiagram
Animal <|-- Duck
"#,
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();

        let err = match crate::family::prepare(parsed, &LayoutOptions::default(), session) {
            Err(error) => error,
            Ok(_) => panic!("expected unsupported diagram error"),
        };
        assert!(matches!(
            err,
            Error::UnsupportedDiagram { diagram_type } if diagram_type == "class"
        ));
    }

    #[test]
    fn render_model_dispatch_renders_cynefin_svg() {
        let source = r#"cynefin-beta
  title Team Practices
  accTitle: Cynefin map
  accDescr: Practice movement
  complex
    "Pair programming"
  complicated
    "Architecture review"
  complex --> complicated : "Pattern emerges"
"#;
        let svg = render_source(
            source,
            &LayoutOptions::default(),
            &crate::svg::SvgRenderOptions {
                diagram_id: Some("cynefin-test".to_string()),
                ..Default::default()
            },
        );

        assert!(svg.contains(r#"aria-roledescription="cynefin""#));
        assert!(svg.contains(r#"<g class="cynefin-backgrounds">"#));
        assert!(svg.contains(r#"class="cynefinDomain""#));
        assert!(svg.contains(r#"class="cynefinBoundary""#));
        assert!(svg.contains(r#"class="cynefinCliff""#));
        assert!(svg.contains(r#"class="cynefinItem""#));
        assert!(svg.contains("Pair programming"));
        assert!(svg.contains(r#"class="cynefinArrowLine""#));
        assert!(svg.contains("Pattern emerges"));
        assert!(svg.contains(r#"<title id="chart-title-cynefin-test">Cynefin map</title>"#));
        assert!(svg.contains(r#"<desc id="chart-desc-cynefin-test">Practice movement</desc>"#));
        assert!(svg.contains("#cynefin-test .cynefinDomain{stroke:none;}"));
        assert_eq!(svg.matches("<title").count(), 2, "{svg}");
        assert_eq!(svg.matches("<desc").count(), 2, "{svg}");

        let scoped_title = svg
            .find(r#"<title id="chart-title-cynefin-test">"#)
            .expect("scoped accessibility title");
        let scoped_descr = svg
            .find(r#"<desc id="chart-desc-cynefin-test">"#)
            .expect("scoped accessibility description");
        let style = svg.find("<style>").expect("style");
        let framework_group = svg.find("<g/>").expect("Mermaid framework group");
        let renderer_title = svg
            .find("<title>Cynefin map</title>")
            .expect("renderer accessibility title");
        let renderer_descr = svg
            .find("<desc>Practice movement</desc>")
            .expect("renderer accessibility description");
        let root_group = svg
            .find(r#"<g transform="translate("#)
            .expect("cynefin root group");
        let defs = svg.find("<defs>").expect("transition marker defs");

        assert!(scoped_title < scoped_descr, "{svg}");
        assert!(scoped_descr < style, "{svg}");
        assert!(style < framework_group, "{svg}");
        assert!(framework_group < renderer_title, "{svg}");
        assert!(renderer_title < renderer_descr, "{svg}");
        assert!(renderer_descr < root_group, "{svg}");
        assert!(root_group < defs, "{svg}");
    }

    #[test]
    fn render_model_dispatch_keeps_whitespace_cynefin_transition_labels() {
        let source = r#"cynefin-beta
  complex
  complicated
  complex --> complicated : "   "
"#;
        let svg = render_source(
            source,
            &LayoutOptions::default(),
            &crate::svg::SvgRenderOptions {
                diagram_id: Some("cynefin-whitespace".to_string()),
                ..Default::default()
            },
        );

        assert!(
            svg.contains(r#"class="cynefinArrowLabel""#),
            "a whitespace-only label is truthy in JavaScript and must emit a text node: {svg}"
        );
    }

    #[test]
    fn render_model_dispatch_renders_railroad_svg() {
        let source = r#"railroad-beta
accTitle: Railroad grammar
accDescr: Expression grammar
expr = sequence(nonterminal("term"), optional(special("guard")), zeroOrMore(terminal("+"))) ;
"#;
        let svg = render_source(
            source,
            &LayoutOptions::default(),
            &crate::svg::SvgRenderOptions {
                diagram_id: Some("railroad-test".to_string()),
                ..Default::default()
            },
        );

        assert!(svg.contains(r#"aria-roledescription="railroad""#));
        assert!(svg.contains(r#"class="railroad-diagram""#));
        assert!(svg.contains(r#"class="railroad-rule""#));
        assert!(svg.contains(r#"class="railroad-rule-name""#));
        assert!(svg.contains(r#"class="railroad-nonterminal""#));
        assert!(svg.contains(r#"class="railroad-special""#));
        assert!(svg.contains(r#"class="railroad-terminal""#));
        assert!(svg.contains(r#"class="railroad-line""#));
        assert!(svg.contains("term"));
        assert!(svg.contains("? guard ?"));
        assert!(svg.contains("+"));
        assert!(svg.contains(r#"<title id="chart-title-railroad-test">Railroad grammar</title>"#));
        assert!(svg.contains(r#"<desc id="chart-desc-railroad-test">Expression grammar</desc>"#));
        assert!(
            svg.contains("</style><g/><g class=\"railroad-rule\""),
            "{svg}"
        );
    }

    #[cfg(feature = "elk-layout")]
    fn edge_path_chunk<'a>(svg: &'a str, edge_id: &str) -> &'a str {
        let id_attr = format!(r#"id="merman-{edge_id}""#);
        let id_start = svg.find(&id_attr).expect("edge id");
        let path_start = svg[..id_start].rfind("<path ").expect("edge path start");
        let path_end = svg[id_start..].find("/>").expect("edge path end") + id_start;
        &svg[path_start..path_end]
    }

    #[cfg(feature = "elk-layout")]
    fn edge_path_d(path: &str) -> &str {
        let d_start = path.find(r#"d=""#).expect("edge path d") + r#"d=""#.len();
        let d_end = path[d_start..].find('"').expect("edge path d end") + d_start;
        &path[d_start..d_end]
    }

    #[cfg(feature = "elk-layout")]
    fn edge_attr_value<'a>(path: &'a str, attr: &str) -> &'a str {
        let needle = format!(r#"{attr}=""#);
        let start = path.find(&needle).expect("edge attr") + needle.len();
        let end = path[start..].find('"').expect("edge attr end") + start;
        &path[start..end]
    }

    #[cfg(feature = "elk-layout")]
    fn edge_data_points(path: &str) -> Vec<(f64, f64)> {
        use base64::Engine as _;

        let b64 = edge_attr_value(path, "data-points");
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64.as_bytes())
            .expect("data-points base64");
        let json: serde_json::Value =
            serde_json::from_slice(&bytes).expect("data-points JSON payload");
        json.as_array()
            .expect("data-points array")
            .iter()
            .map(|point| {
                (
                    point.get("x").and_then(serde_json::Value::as_f64).unwrap(),
                    point.get("y").and_then(serde_json::Value::as_f64).unwrap(),
                )
            })
            .collect()
    }
}
