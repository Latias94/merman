use crate::{DiagramWarningFact, SourceSpan};
use indexmap::IndexMap;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Deserializer, Serialize};

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
    #[serde(default)]
    pub tooltips: FxHashMap<String, String>,
    #[serde(
        default,
        rename = "warningFacts",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub warning_facts: Vec<DiagramWarningFact>,
}

impl FlowchartModel {
    pub(crate) fn sanitize_common_db_fields(&mut self, config: &crate::MermaidConfig) {
        crate::common_db::sanitize_optional_acc_title(&mut self.acc_title, config);
        crate::common_db::sanitize_optional_acc_descr(&mut self.acc_descr, config);
    }
}

#[doc(hidden)]
#[derive(Debug, Clone, Default)]
pub struct FlowchartRenderLabelSources {
    nodes: FxHashMap<String, String>,
    edges: FxHashMap<String, String>,
    subgraphs: FxHashMap<String, String>,
}

impl FlowchartRenderLabelSources {
    #[doc(hidden)]
    pub fn node_label_for_render<'a>(&'a self, node: &'a FlowNode) -> Option<&'a str> {
        self.nodes
            .get(&node.id)
            .map(String::as_str)
            .or(node.label.as_deref())
    }

    #[doc(hidden)]
    pub fn edge_label_for_render<'a>(&'a self, edge: &'a FlowEdge) -> Option<&'a str> {
        self.edges
            .get(&edge.id)
            .map(String::as_str)
            .or(edge.label.as_deref())
    }

    #[doc(hidden)]
    pub fn subgraph_title_for_render<'a>(&'a self, subgraph: &'a FlowSubgraph) -> &'a str {
        self.subgraphs
            .get(&subgraph.id)
            .map(String::as_str)
            .unwrap_or(subgraph.title.as_str())
    }

    pub(crate) fn insert_node(&mut self, id: String, source: String) {
        self.nodes.insert(id, source);
    }

    pub(crate) fn insert_edge(&mut self, id: String, source: String) {
        self.edges.insert(id, source);
    }

    pub(crate) fn set_subgraph(&mut self, id: String, source: Option<String>) {
        if let Some(source) = source {
            self.subgraphs.insert(id, source);
        } else {
            // Flowchart's renderer indexes subgraphs by id with last-declaration-wins semantics.
            // A later ordinary title must therefore retire an earlier provenance override.
            self.subgraphs.remove(&id);
        }
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        self.nodes
            .iter()
            .chain(&self.edges)
            .chain(&self.subgraphs)
            .fold(0usize, |total, (id, label)| {
                total.saturating_add(id.len()).saturating_add(label.len())
            })
    }
}

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
    #[serde(default, rename = "hasExplicitDir")]
    pub has_explicit_dir: bool,
    #[serde(default, rename = "labelType")]
    pub label_type: Option<String>,
    #[serde(default)]
    pub classes: Vec<String>,
    #[serde(default)]
    pub styles: Vec<String>,
    /// Captures the final CSS inputs of a FlowDB vertex that shares this subgraph's ID.
    ///
    /// Mermaid emits subgraphs before vertices. A same-ID vertex therefore replaces the
    /// subgraph's classes and styles during the later vertex projection. `None` means no such
    /// vertex was created; `Some` with empty vectors is observably different because it clears
    /// the subgraph CSS inputs.
    #[serde(
        default,
        rename = "sameIdVertexStyle",
        skip_serializing_if = "Option::is_none"
    )]
    pub same_id_vertex_style: Option<FlowSubgraphVertexStyle>,
    pub nodes: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowSubgraphVertexStyle {
    #[serde(default)]
    pub classes: Vec<String>,
    #[serde(default)]
    pub styles: Vec<String>,
}

#[derive(Debug, Clone)]
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
pub(crate) enum FlowNodeSyntax {
    BareReference,
    ExplicitDefinition,
}

#[derive(Debug, Clone)]
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
pub(crate) struct LinkToken {
    pub end: String,
    pub start_marker: FlowEdgeMarker,
    pub end_marker: FlowEdgeMarker,
    pub stroke_kind: FlowEdgeStroke,
    pub visibility: FlowEdgeVisibility,
    pub length: usize,
}

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
