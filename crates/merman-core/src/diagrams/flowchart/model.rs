use crate::{DiagramWarningFact, SourceSpan};
use indexmap::IndexMap;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Deserializer, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowchartModel {
    #[serde(default)]
    pub keyword: String,
    #[serde(default, rename = "accDescr")]
    pub acc_descr: Option<String>,
    #[serde(default, rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(default, rename = "classDefs")]
    pub class_defs: IndexMap<String, Vec<String>>,
    #[serde(default)]
    pub direction: Option<String>,
    #[serde(default, rename = "edgeDefaults")]
    pub edge_defaults: Option<FlowEdgeDefaults>,
    #[serde(default, rename = "vertexCalls")]
    pub vertex_calls: Vec<String>,
    pub nodes: Vec<FlowNode>,
    pub edges: Vec<FlowEdge>,
    #[serde(default)]
    pub subgraphs: Vec<FlowSubgraph>,
    /// Mermaid's effective CSS source for subgraph vertices that share an ID with a group.
    ///
    /// This is a parser-owned semantic fact rather than compatibility JSON. It is kept out of
    /// serialization because Mermaid's public model exposes the group's own classes and styles,
    /// while terminal consumers still need the later vertex declaration to reproduce its paint.
    #[doc(hidden)]
    #[serde(skip)]
    pub subgraph_vertex_styles: FlowchartSubgraphStyles,
    #[serde(default)]
    pub tooltips: FxHashMap<String, String>,
    #[serde(
        default,
        rename = "warningFacts",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub warning_facts: Vec<DiagramWarningFact>,
}

/// Parser-owned CSS provenance for vertices whose ID also names a subgraph.
#[doc(hidden)]
#[derive(Debug, Clone, Default)]
pub struct FlowchartSubgraphStyles {
    sources: Option<Arc<IndexMap<String, FlowSubgraphVertexStyleSource>>>,
}

impl Serialize for FlowchartSubgraphStyles {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.sources.as_deref().serialize(serializer)
    }
}

impl FlowchartSubgraphStyles {
    pub(crate) fn effective_subgraph_css<'a>(
        &'a self,
        declaration_ordinal: usize,
        subgraph: &'a FlowSubgraph,
    ) -> (&'a [String], &'a [String]) {
        self.sources
            .as_deref()
            .and_then(|sources| sources.get(&subgraph.id))
            .filter(|source| source.declaration_ordinal == declaration_ordinal)
            .map_or_else(
                || (subgraph.classes.as_slice(), subgraph.styles.as_slice()),
                |source| {
                    (
                        source.style.classes.as_slice(),
                        source.style.styles.as_slice(),
                    )
                },
            )
    }

    pub(crate) fn contains_subgraph_vertex(&self, id: &str) -> bool {
        self.sources
            .as_deref()
            .is_some_and(|sources| sources.contains_key(id))
    }

    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    pub(crate) fn insert(
        &mut self,
        id: String,
        declaration_ordinal: usize,
        style: FlowSubgraphVertexStyle,
    ) {
        let sources = self
            .sources
            .get_or_insert_with(|| Arc::new(IndexMap::new()));
        Arc::make_mut(sources).insert(
            id,
            FlowSubgraphVertexStyleSource {
                declaration_ordinal,
                style,
            },
        );
    }

    #[cfg(any(
        feature = "diagram-flowchart",
        feature = "diagram-swimlane",
        feature = "diagram-agentflow"
    ))]
    pub(crate) fn retained_bytes(&self) -> usize {
        self.sources.iter().flat_map(|sources| sources.iter()).fold(
            0usize,
            |total, (id, source)| {
                source
                    .style
                    .classes
                    .iter()
                    .chain(&source.style.styles)
                    .fold(total.saturating_add(id.len()), |subtotal, value| {
                        subtotal.saturating_add(value.len())
                    })
            },
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub(crate) struct FlowSubgraphVertexStyle {
    pub(crate) classes: Vec<String>,
    pub(crate) styles: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct FlowSubgraphVertexStyleSource {
    #[serde(rename = "declarationOrdinal")]
    declaration_ordinal: usize,
    style: FlowSubgraphVertexStyle,
}

#[cfg(test)]
mod subgraph_style_serialization_tests {
    use super::*;

    fn styles() -> FlowchartSubgraphStyles {
        FlowchartSubgraphStyles {
            sources: Some(Arc::new(IndexMap::from([
                (
                    "z".to_owned(),
                    FlowSubgraphVertexStyleSource {
                        declaration_ordinal: 7,
                        style: FlowSubgraphVertexStyle {
                            classes: vec!["paint".to_owned(), "typography".to_owned()],
                            styles: vec!["fill:red".to_owned(), "stroke:blue".to_owned()],
                        },
                    },
                ),
                (
                    "a".to_owned(),
                    FlowSubgraphVertexStyleSource {
                        declaration_ordinal: 0,
                        style: FlowSubgraphVertexStyle::default(),
                    },
                ),
            ]))),
        }
    }

    #[test]
    fn borrowed_subgraph_styles_preserve_none_and_an_empty_owner() {
        assert_eq!(
            serde_json::to_string(&FlowchartSubgraphStyles::default()).unwrap(),
            "null"
        );
        let empty = FlowchartSubgraphStyles {
            sources: Some(Arc::new(IndexMap::new())),
        };
        assert_eq!(serde_json::to_string(&empty).unwrap(), "{}");
    }

    #[test]
    fn borrowed_subgraph_styles_preserve_ordinals_and_css_encounter_order() {
        assert_eq!(
            serde_json::to_string(&styles()).unwrap(),
            concat!(
                r#"{"z":{"declarationOrdinal":7,"style":{"classes":["paint","typography"],"styles":["fill:red","stroke:blue"]}},"#,
                r#""a":{"declarationOrdinal":0,"style":{"classes":[],"styles":[]}}}"#,
            )
        );
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

impl FlowchartModel {
    /// Returns the effective CSS source for one canonical subgraph.
    ///
    /// A vertex declaration replaces the group's CSS source; it is not merged with that source.
    /// The canonical ordinal prevents same-ID subgraphs from borrowing another owner's CSS.
    pub fn effective_subgraph_css<'a>(
        &'a self,
        declaration_ordinal: usize,
        subgraph: &'a FlowSubgraph,
    ) -> (&'a [String], &'a [String]) {
        self.subgraph_vertex_styles
            .effective_subgraph_css(declaration_ordinal, subgraph)
    }

    /// Returns whether parsing observed a FlowDB vertex with this subgraph ID.
    pub fn contains_subgraph_vertex(&self, id: &str) -> bool {
        self.subgraph_vertex_styles.contains_subgraph_vertex(id)
    }

    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    pub(crate) fn sanitize_common_db_fields(&mut self, config: &crate::MermaidConfig) {
        crate::common_db::sanitize_optional_acc_title(&mut self.acc_title, config);
        crate::common_db::sanitize_optional_acc_descr(&mut self.acc_descr, config);
    }
}

#[cfg(feature = "presentation")]
pub use super::presentation::FlowchartRenderContext;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowEdgeDefaults {
    #[serde(default)]
    pub interpolate: Option<String>,
    #[serde(default)]
    pub style: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowNode {
    pub id: String,
    /// Records whether this node was authored by the user or synthesized solely as a subgraph
    /// routing endpoint.
    ///
    /// Renderers must use this fact instead of inferring provenance from the node's visible
    /// fields. A bare authored node can otherwise be indistinguishable from an endpoint anchor.
    #[serde(default, skip_serializing_if = "FlowNodeProvenance::is_authored")]
    pub provenance: FlowNodeProvenance,
    pub label: Option<String>,
    #[serde(default, rename = "labelType")]
    pub label_type: Option<String>,
    #[serde(rename = "layoutShape")]
    pub layout_shape: Option<String>,
    #[serde(default)]
    pub shape: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub form: Option<String>,
    #[serde(default)]
    pub pos: Option<String>,
    #[serde(default)]
    pub img: Option<String>,
    #[serde(default)]
    pub constraint: Option<String>,
    #[serde(default, rename = "assetWidth")]
    pub asset_width: Option<f64>,
    #[serde(default, rename = "assetHeight")]
    pub asset_height: Option<f64>,
    #[serde(default)]
    pub classes: Vec<String>,
    #[serde(default)]
    pub styles: Vec<String>,
    #[serde(default)]
    pub link: Option<String>,
    #[serde(default, rename = "linkTarget")]
    pub link_target: Option<String>,
    #[serde(default, rename = "haveCallback")]
    pub have_callback: bool,
}

impl FlowNode {
    pub fn is_subgraph_anchor(&self) -> bool {
        matches!(self.provenance, FlowNodeProvenance::SubgraphAnchor)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum FlowNodeProvenance {
    #[default]
    Authored,
    SubgraphAnchor,
}

impl FlowNodeProvenance {
    fn is_authored(&self) -> bool {
        matches!(self, Self::Authored)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
/// Marker attached to one semantic endpoint of a Flowchart edge.
pub enum FlowEdgeMarker {
    #[default]
    None,
    Point,
    Circle,
    Cross,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
/// Visible stroke pattern, independent from whether the edge is painted.
pub enum FlowEdgeStroke {
    #[default]
    Normal,
    Dotted,
    Thick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
/// Whether the edge is painted or retained only as a layout constraint.
pub enum FlowEdgeVisibility {
    #[default]
    Visible,
    Invisible,
}

#[derive(Debug, Clone, Serialize)]
pub struct FlowEdge {
    pub id: String,
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    #[serde(default, rename = "labelType")]
    pub label_type: Option<String>,
    #[serde(default, rename = "type")]
    /// Mermaid-compatible aggregate edge type retained for legacy JSON consumers.
    pub edge_type: Option<String>,
    #[serde(default)]
    /// Authored full/end operator retained for compatibility and source inspection.
    pub arrow: String,
    #[serde(default, rename = "startMarker")]
    /// Typed marker owned by the source endpoint.
    pub start_marker: FlowEdgeMarker,
    #[serde(default, rename = "endMarker")]
    /// Typed marker owned by the target endpoint.
    pub end_marker: FlowEdgeMarker,
    #[serde(default, rename = "isUserDefinedId")]
    pub is_user_defined_id: bool,
    #[serde(default)]
    /// Mermaid-compatible stroke string retained for legacy JSON consumers.
    pub stroke: Option<String>,
    #[serde(default, rename = "strokeKind")]
    pub stroke_kind: FlowEdgeStroke,
    #[serde(default)]
    pub visibility: FlowEdgeVisibility,
    #[serde(default)]
    pub interpolate: Option<String>,
    #[serde(default)]
    pub classes: Vec<String>,
    #[serde(default)]
    pub style: Vec<String>,
    #[serde(default)]
    pub animate: Option<bool>,
    #[serde(default)]
    pub animation: Option<String>,
    pub length: usize,
}

#[derive(Deserialize)]
struct FlowEdgeWire {
    id: String,
    from: String,
    to: String,
    label: Option<String>,
    #[serde(default, rename = "labelType")]
    label_type: Option<String>,
    #[serde(default, rename = "type")]
    edge_type: Option<String>,
    #[serde(default)]
    arrow: String,
    #[serde(default, rename = "startMarker")]
    start_marker: Option<FlowEdgeMarker>,
    #[serde(default, rename = "endMarker")]
    end_marker: Option<FlowEdgeMarker>,
    #[serde(default, rename = "isUserDefinedId")]
    is_user_defined_id: bool,
    #[serde(default)]
    stroke: Option<String>,
    #[serde(default, rename = "strokeKind")]
    stroke_kind: Option<FlowEdgeStroke>,
    #[serde(default)]
    visibility: Option<FlowEdgeVisibility>,
    #[serde(default)]
    interpolate: Option<String>,
    #[serde(default)]
    classes: Vec<String>,
    #[serde(default)]
    style: Vec<String>,
    #[serde(default)]
    animate: Option<bool>,
    #[serde(default)]
    animation: Option<String>,
    length: usize,
}

impl<'de> Deserialize<'de> for FlowEdge {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = FlowEdgeWire::deserialize(deserializer)?;
        let label_is_present = wire.label.is_some();
        let (legacy_start_marker, legacy_end_marker) = markers_from_compatibility_fields(
            &wire.arrow,
            wire.edge_type.as_deref(),
            label_is_present,
        );
        let legacy_stroke_kind = stroke_from_compatibility_field(wire.stroke.as_deref());
        let legacy_visibility = visibility_from_compatibility_field(wire.stroke.as_deref());

        Ok(Self {
            id: wire.id,
            from: wire.from,
            to: wire.to,
            label: wire.label,
            label_type: wire.label_type,
            edge_type: wire.edge_type,
            arrow: wire.arrow,
            start_marker: wire.start_marker.unwrap_or(legacy_start_marker),
            end_marker: wire.end_marker.unwrap_or(legacy_end_marker),
            is_user_defined_id: wire.is_user_defined_id,
            stroke: wire.stroke,
            stroke_kind: wire.stroke_kind.unwrap_or(legacy_stroke_kind),
            visibility: wire.visibility.unwrap_or(legacy_visibility),
            interpolate: wire.interpolate,
            classes: wire.classes,
            style: wire.style,
            animate: wire.animate,
            animation: wire.animation,
            length: wire.length,
        })
    }
}

fn markers_from_compatibility_fields(
    arrow: &str,
    edge_type: Option<&str>,
    label_is_present: bool,
) -> (FlowEdgeMarker, FlowEdgeMarker) {
    let arrow = arrow.trim();
    let compatibility_marker = edge_type.and_then(marker_from_compatibility_edge_type);
    let end_marker = compatibility_marker.unwrap_or_else(|| {
        arrow
            .chars()
            .next_back()
            .and_then(marker_from_end_char)
            .unwrap_or_default()
    });
    let start_marker = if edge_type.is_some_and(|edge_type| edge_type.starts_with("double_")) {
        compatibility_marker.unwrap_or_default()
    } else if label_is_present {
        // Mermaid's split-label lexer can leave the label's final `o` or `x` at the beginning of
        // the compatibility `arrow` field (for example `--No-->` becomes `o-->`). Only an
        // unlabeled edge proves that `arrow` contains the complete authored operator.
        FlowEdgeMarker::None
    } else {
        arrow
            .chars()
            .next()
            .and_then(marker_from_start_char)
            .unwrap_or_default()
    };
    (start_marker, end_marker)
}

fn marker_from_start_char(ch: char) -> Option<FlowEdgeMarker> {
    match ch {
        '<' => Some(FlowEdgeMarker::Point),
        'o' => Some(FlowEdgeMarker::Circle),
        'x' => Some(FlowEdgeMarker::Cross),
        _ => None,
    }
}

fn marker_from_end_char(ch: char) -> Option<FlowEdgeMarker> {
    match ch {
        '>' => Some(FlowEdgeMarker::Point),
        'o' => Some(FlowEdgeMarker::Circle),
        'x' => Some(FlowEdgeMarker::Cross),
        _ => None,
    }
}

fn marker_from_compatibility_edge_type(edge_type: &str) -> Option<FlowEdgeMarker> {
    match edge_type.strip_prefix("double_").unwrap_or(edge_type) {
        "arrow" | "arrow_point" => Some(FlowEdgeMarker::Point),
        "arrow_circle" => Some(FlowEdgeMarker::Circle),
        "arrow_cross" => Some(FlowEdgeMarker::Cross),
        "arrow_open" => Some(FlowEdgeMarker::None),
        _ => None,
    }
}

fn stroke_from_compatibility_field(stroke: Option<&str>) -> FlowEdgeStroke {
    match stroke {
        Some("dotted") => FlowEdgeStroke::Dotted,
        Some("thick") => FlowEdgeStroke::Thick,
        _ => FlowEdgeStroke::Normal,
    }
}

fn visibility_from_compatibility_field(stroke: Option<&str>) -> FlowEdgeVisibility {
    if stroke == Some("invisible") {
        FlowEdgeVisibility::Invisible
    } else {
        FlowEdgeVisibility::Visible
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowSubgraph {
    pub id: String,
    pub title: String,
    pub dir: Option<String>,
    // Kept as parser-internal provenance for the ASCII adapter. Mermaid 11.17.2 no longer
    // exposes this rollback-era field in the render model or compatibility JSON.
    #[serde(skip)]
    pub has_explicit_dir: bool,
    #[serde(default, rename = "labelType")]
    pub label_type: Option<String>,
    #[serde(default)]
    pub classes: Vec<String>,
    #[serde(default)]
    pub styles: Vec<String>,
    pub nodes: Vec<String>,
    /// Opaque subgraph metadata forwarded to layout engines, including `algorithm`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
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
    pub label_span: Option<SourceSpan>,
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
