use crate::{
    EditorSemanticFacts, Error, MermaidConfig, ParseControl, ParseControlResult, ParseMetadata,
    Result, editor::SourceSpan,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

pub const BLOCK_WIDTH_WARNING_RULE_ID: &str = "merman.block.width_exceeds_columns";
pub const FLOWCHART_EXPLICIT_DIRECTION_WARNING_RULE_ID: &str =
    "merman.authoring.flowchart.explicit_direction";
pub const FLOWCHART_UNKNOWN_STYLE_TARGET_WARNING_RULE_ID: &str =
    "merman.semantic.flowchart.unknown_style_target";
pub const GIT_GRAPH_DUPLICATE_COMMIT_WARNING_RULE_ID: &str = "merman.git_graph.duplicate_commit_id";

/// Shared warning fact emitted by diagram families for analysis and lint consumers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagramWarningFact {
    pub rule_id: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<SourceSpan>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix_span: Option<SourceSpan>,
}

impl DiagramWarningFact {
    pub fn new(rule_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            rule_id: rule_id.into(),
            message: message.into(),
            span: None,
            fix_span: None,
        }
    }

    pub fn with_span(mut self, span: SourceSpan) -> Self {
        self.span = Some(span);
        self
    }

    pub fn with_fix_span(mut self, span: SourceSpan) -> Self {
        self.fix_span = Some(span);
        self
    }
}

pub(crate) fn legacy_warning_messages(facts: &[DiagramWarningFact]) -> Vec<String> {
    facts.iter().map(|fact| fact.message.clone()).collect()
}

/// Parser used by a custom semantic JSON registry overlay.
///
/// Implementations must call [`ParseControl::checkpoint`] inside potentially long-running loops.
/// The outer [`ParseControlResult`] reports cooperative cancellation; the inner [`Result`]
/// reports Mermaid detection or parse failures. Non-cancellable `Engine` snapshot and model APIs
/// surface an unexpected outer cancellation as [`crate::Error::ParseCancelled`].
pub type DiagramSemanticParser = fn(
    code: &str,
    meta: &ParseMetadata,
    control: &ParseControl,
) -> ParseControlResult<Result<Value>>;

/// Parser used by the pinned built-in semantic JSON path.
pub(crate) type BuiltInDiagramSemanticParser =
    fn(code: &str, meta: &ParseMetadata) -> Result<Value>;

/// Parser used by the built-in typed render-model path for one Mermaid diagram family.
pub(crate) type BuiltInRenderSemanticParser =
    fn(
        code: &str,
        meta: &ParseMetadata,
        control: &ParseControl,
    ) -> ParseControlResult<Result<RenderSemanticParseOutput>>;

/// Parser used by a custom render-model registry overlay.
///
/// Custom adapters intentionally return only a named JSON model. Built-in typed variants are
/// reserved for the pinned family catalog and cannot be manufactured through this interface.
pub type CustomJsonRenderParser = fn(
    code: &str,
    meta: &ParseMetadata,
    control: &ParseControl,
) -> ParseControlResult<Result<CustomJsonRenderModel>>;

/// Ownership of a parser resolved from a built-in registry plus its custom overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RegistryOwner {
    BuiltIn,
    Custom,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ResolvedSemanticParser {
    BuiltIn(BuiltInDiagramSemanticParser),
    Custom(DiagramSemanticParser),
}

impl ResolvedSemanticParser {
    pub(crate) const fn owner(self) -> RegistryOwner {
        match self {
            Self::BuiltIn(_) => RegistryOwner::BuiltIn,
            Self::Custom(_) => RegistryOwner::Custom,
        }
    }
}

/// Registry for semantic JSON parsers keyed by Mermaid diagram type id.
#[derive(Debug, Clone)]
pub struct DiagramRegistry {
    builtins: Arc<HashMap<&'static str, BuiltInDiagramSemanticParser>>,
    overlays: Arc<HashMap<&'static str, DiagramSemanticParser>>,
}

impl Default for DiagramRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl DiagramRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self {
            builtins: Arc::new(HashMap::new()),
            overlays: Arc::new(HashMap::new()),
        }
    }

    /// Registers or replaces the parser for a Mermaid diagram type id.
    ///
    /// The parser participates in the owning operation's cancellation lifecycle. It must preserve
    /// the outer-cancellation/inner-parse-error distinction documented by
    /// [`DiagramSemanticParser`].
    pub fn insert(&mut self, diagram_type: &'static str, parser: DiagramSemanticParser) {
        Arc::make_mut(&mut self.overlays).insert(diagram_type, parser);
    }

    pub(crate) fn resolve(&self, diagram_type: &str) -> Option<ResolvedSemanticParser> {
        if let Some(parser) = self.overlays.get(diagram_type).copied() {
            return Some(ResolvedSemanticParser::Custom(parser));
        }
        self.builtins
            .get(diagram_type)
            .copied()
            .map(ResolvedSemanticParser::BuiltIn)
    }

    /// Builds the semantic parser registry for the repository's pinned Mermaid baseline.
    pub fn pinned_mermaid_baseline() -> Self {
        let mut reg = Self::new();
        for fact in crate::family::semantic_parser_facts() {
            Arc::make_mut(&mut reg.builtins).insert(fact.id, fact.parser);
        }

        reg
    }

    #[cfg(test)]
    pub(crate) fn parser_ids(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.builtins
            .keys()
            .chain(
                self.overlays
                    .keys()
                    .filter(|id| !self.builtins.contains_key(**id)),
            )
            .copied()
    }
}

/// Parsed diagram metadata plus the Mermaid-compatible semantic JSON model.
#[derive(Debug, Clone)]
pub struct ParsedDiagram {
    /// Diagram type and effective configuration extracted during preprocessing.
    pub meta: ParseMetadata,
    /// Semantic JSON model matching Mermaid's parser/database output shape where possible.
    pub model: Value,
}

/// Parser-backed editor facts produced by a diagram parse operation.
#[derive(Debug)]
pub enum ParsedEditorFacts {
    Available(EditorSemanticFacts),
    Unavailable,
}

/// Semantic result retained by one editor-facing diagram parse operation.
#[derive(Debug)]
pub enum DiagramParseOutcome {
    Parsed(Value),
    Failed(Error),
    /// Family construction panicked after preprocessing produced valid metadata.
    Panicked(String),
}

impl DiagramParseOutcome {
    /// Returns the semantic model when family construction succeeded.
    pub fn parsed_model(&self) -> Option<&Value> {
        match self {
            Self::Parsed(model) => Some(model),
            Self::Failed(_) | Self::Panicked(_) => None,
        }
    }
}

/// One preprocessing, detection, and family-construction operation for editor consumers.
///
/// Metadata, semantic output or its original error, and recovery facts are retained together so
/// downstream analysis cannot reconstruct failure state by parsing the source again.
#[derive(Debug)]
pub struct DiagramParseSnapshot {
    meta: ParseMetadata,
    outcome: DiagramParseOutcome,
    editor_facts: ParsedEditorFacts,
    recovered_incomplete_directive: bool,
}

impl DiagramParseSnapshot {
    pub(crate) fn new(
        meta: ParseMetadata,
        outcome: DiagramParseOutcome,
        editor_facts: ParsedEditorFacts,
        recovered_incomplete_directive: bool,
    ) -> Self {
        Self {
            meta,
            outcome,
            editor_facts,
            recovered_incomplete_directive,
        }
    }

    /// Consumes the closed snapshot into its three operation-owned projections.
    pub fn into_parts(self) -> (ParseMetadata, DiagramParseOutcome, ParsedEditorFacts) {
        (self.meta, self.outcome, self.editor_facts)
    }

    /// Returns metadata produced by this preprocessing and detection operation.
    pub fn metadata(&self) -> &ParseMetadata {
        &self.meta
    }

    /// Returns the semantic success or original family-construction error.
    pub fn outcome(&self) -> &DiagramParseOutcome {
        &self.outcome
    }

    /// Returns parser-backed editor facts retained by this operation.
    pub fn editor_facts(&self) -> &ParsedEditorFacts {
        &self.editor_facts
    }

    /// Whether editor preprocessing recovered an unterminated directive line.
    pub const fn recovered_incomplete_directive(&self) -> bool {
        self.recovered_incomplete_directive
    }
}

/// Origin of a custom JSON render model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CustomJsonProvenance {
    /// Produced by a custom semantic JSON parser because no custom render parser was registered.
    SemanticRegistryOverlay,
    /// Produced by an explicitly registered custom render-model parser.
    RenderRegistryOverlay,
}

/// Explicit non-built-in JSON model boundary for custom parser adapters.
#[derive(Debug, Clone, PartialEq)]
pub struct CustomJsonRenderModel {
    model_name: String,
    value: Value,
    provenance: CustomJsonProvenance,
}

impl CustomJsonRenderModel {
    /// Creates the result of a custom render-model registry parser.
    pub fn new(model_name: impl Into<String>, value: Value) -> Self {
        Self {
            model_name: model_name.into(),
            value,
            provenance: CustomJsonProvenance::RenderRegistryOverlay,
        }
    }

    pub(crate) fn from_semantic_registry(model_name: impl Into<String>, value: Value) -> Self {
        Self {
            model_name: model_name.into(),
            value,
            provenance: CustomJsonProvenance::SemanticRegistryOverlay,
        }
    }

    /// Returns the adapter-defined model name.
    pub fn model_name(&self) -> &str {
        &self.model_name
    }

    /// Returns the custom JSON payload.
    pub fn value(&self) -> &Value {
        &self.value
    }

    /// Consumes the wrapper and returns the custom JSON payload.
    pub fn into_value(self) -> Value {
        self.value
    }

    /// Returns which custom registry path produced the model.
    pub fn provenance(&self) -> CustomJsonProvenance {
        self.provenance
    }
}

/// Typed semantic model used by the headless renderer.
///
/// Most public callers should use [`ParsedDiagram`] when they need JSON output. This enum is for
/// render paths that benefit from typed data and avoiding a JSON round trip.
#[derive(Debug, Clone)]
pub enum RenderSemanticModel {
    Error(crate::diagrams::error_diagram::ErrorDiagramRenderModel),
    CustomJson(CustomJsonRenderModel),
    Mindmap(crate::diagrams::mindmap::MindmapDiagramRenderModel),
    State(crate::diagrams::state::StateDiagramRenderModel),
    Sequence(crate::diagrams::sequence::SequenceDiagramRenderModel),
    Zenuml(crate::diagrams::zenuml::ZenumlDiagramRenderModel),
    Flowchart(crate::diagrams::flowchart::FlowchartModel),
    Architecture(crate::diagrams::architecture::ArchitectureDiagramRenderModel),
    Class(crate::models::class_diagram::ClassDiagram),
    C4(crate::diagrams::c4::C4DiagramRenderModel),
    Cynefin(crate::diagrams::cynefin::CynefinDiagramRenderModel),
    Railroad(crate::diagrams::railroad::RailroadDiagramRenderModel),
    Kanban(crate::diagrams::kanban::KanbanDiagramRenderModel),
    Gantt(crate::diagrams::gantt::GanttDiagramRenderModel),
    Pie(crate::diagrams::pie::PieDiagramRenderModel),
    Packet(crate::diagrams::packet::PacketDiagramRenderModel),
    Timeline(crate::diagrams::timeline::TimelineDiagramRenderModel),
    Journey(crate::diagrams::journey::JourneyDiagramRenderModel),
    Requirement(crate::diagrams::requirement::RequirementDiagramRenderModel),
    Sankey(crate::diagrams::sankey::SankeyDiagramRenderModel),
    Radar(crate::diagrams::radar::RadarDiagramRenderModel),
    Info(crate::diagrams::info::InfoDiagramRenderModel),
    Treemap(crate::diagrams::treemap::TreemapDiagramRenderModel),
    Block(crate::diagrams::block::BlockDiagramRenderModel),
    Er(crate::diagrams::er::ErDiagramRenderModel),
    QuadrantChart(crate::diagrams::quadrant_chart::QuadrantChartRenderModel),
    XyChart(crate::diagrams::xychart::XyChartDiagramRenderModel),
    GitGraph(crate::diagrams::git_graph::GitGraphRenderModel),
    TreeView(crate::diagrams::tree_view::TreeViewDiagramRenderModel),
    Ishikawa(crate::diagrams::ishikawa::IshikawaDiagramRenderModel),
    EventModeling(crate::diagrams::eventmodeling::EventModelingDiagramRenderModel),
    Venn(crate::diagrams::venn::VennDiagramRenderModel),
    Wardley(crate::diagrams::wardley::WardleyDiagramRenderModel),
}

/// Parser-owned data needed only while rendering a typed semantic model.
///
/// This context is intentionally separate from [`RenderSemanticModel`] so family models retain
/// their public construction and serialization contracts.
#[doc(hidden)]
#[derive(Debug, Clone, Default)]
pub struct RenderSemanticContext {
    flowchart_label_sources: Option<crate::diagrams::flowchart::FlowchartRenderLabelSources>,
}

impl RenderSemanticContext {
    fn for_flowchart(
        label_sources: crate::diagrams::flowchart::FlowchartRenderLabelSources,
    ) -> Self {
        Self {
            flowchart_label_sources: Some(label_sources),
        }
    }

    /// Consumes the context and returns Flowchart's parser-owned render label sources.
    #[doc(hidden)]
    pub fn into_flowchart_label_sources(
        self,
    ) -> crate::diagrams::flowchart::FlowchartRenderLabelSources {
        self.flowchart_label_sources.unwrap_or_default()
    }

    /// Borrows Flowchart's parser-owned render label sources when this context owns them.
    #[doc(hidden)]
    pub fn flowchart_label_sources(
        &self,
    ) -> Option<&crate::diagrams::flowchart::FlowchartRenderLabelSources> {
        self.flowchart_label_sources.as_ref()
    }

    pub(crate) fn retained_text_bytes(&self) -> usize {
        self.flowchart_label_sources
            .as_ref()
            .map_or(0, |sources| sources.retained_bytes())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RenderSemanticParseOutput {
    model: RenderSemanticModel,
    context: RenderSemanticContext,
}

impl RenderSemanticParseOutput {
    pub(crate) fn new(model: RenderSemanticModel) -> Self {
        Self {
            model,
            context: RenderSemanticContext::default(),
        }
    }

    pub(crate) fn flowchart(
        model: crate::diagrams::flowchart::FlowchartModel,
        label_sources: crate::diagrams::flowchart::FlowchartRenderLabelSources,
    ) -> Self {
        Self {
            model: RenderSemanticModel::Flowchart(model),
            context: RenderSemanticContext::for_flowchart(label_sources),
        }
    }

    pub(crate) fn model(&self) -> &RenderSemanticModel {
        &self.model
    }

    pub(crate) fn model_mut(&mut self) -> &mut RenderSemanticModel {
        &mut self.model
    }

    fn into_parts(self) -> (RenderSemanticModel, RenderSemanticContext) {
        (self.model, self.context)
    }
}

mod builtin_render_semantic_private {
    pub trait Sealed {}
}

/// Family-owned typed semantic data that can project the public compatibility JSON contract.
///
/// This trait is sealed because built-in family membership is defined by the pinned Mermaid
/// catalog. Consumers can use it generically but cannot manufacture a new built-in family.
pub trait BuiltinRenderSemantic: builtin_render_semantic_private::Sealed {
    fn compatibility_json(&self, meta: &ParseMetadata) -> Result<Value>;
}

macro_rules! impl_builtin_render_semantic {
    ($model:path, $project:path) => {
        impl builtin_render_semantic_private::Sealed for $model {}

        impl BuiltinRenderSemantic for $model {
            fn compatibility_json(&self, meta: &ParseMetadata) -> Result<Value> {
                $project(self, meta)
            }
        }
    };
}

impl_builtin_render_semantic!(
    crate::diagrams::error_diagram::ErrorDiagramRenderModel,
    crate::diagrams::error_diagram::render_model_to_compat_json
);
impl_builtin_render_semantic!(
    crate::diagrams::mindmap::MindmapDiagramRenderModel,
    crate::diagrams::mindmap::render_model_to_compat_json
);
impl_builtin_render_semantic!(
    crate::diagrams::state::StateDiagramRenderModel,
    crate::diagrams::state::render_model_to_compat_json
);
impl_builtin_render_semantic!(
    crate::diagrams::sequence::SequenceDiagramRenderModel,
    crate::diagrams::sequence::render_model_to_compat_json
);
impl_builtin_render_semantic!(
    crate::diagrams::zenuml::ZenumlDiagramRenderModel,
    crate::diagrams::zenuml::render_model_to_compat_json
);
impl_builtin_render_semantic!(
    crate::diagrams::flowchart::FlowchartModel,
    crate::diagrams::flowchart::render_model_to_compat_json
);
impl_builtin_render_semantic!(
    crate::diagrams::architecture::ArchitectureDiagramRenderModel,
    crate::diagrams::architecture::render_model_to_compat_json
);
impl_builtin_render_semantic!(
    crate::models::class_diagram::ClassDiagram,
    crate::diagrams::class::render_model_to_compat_json
);
impl_builtin_render_semantic!(
    crate::diagrams::c4::C4DiagramRenderModel,
    crate::diagrams::c4::render_model_to_compat_json
);
impl_builtin_render_semantic!(
    crate::diagrams::cynefin::CynefinDiagramRenderModel,
    crate::diagrams::cynefin::render_model_to_compat_json
);
impl_builtin_render_semantic!(
    crate::diagrams::railroad::RailroadDiagramRenderModel,
    crate::diagrams::railroad::render_model_to_compat_json
);
impl_builtin_render_semantic!(
    crate::diagrams::kanban::KanbanDiagramRenderModel,
    crate::diagrams::kanban::render_model_to_compat_json
);
impl_builtin_render_semantic!(
    crate::diagrams::gantt::GanttDiagramRenderModel,
    crate::diagrams::gantt::render_model_to_compat_json
);
impl_builtin_render_semantic!(
    crate::diagrams::pie::PieDiagramRenderModel,
    crate::diagrams::pie::render_model_to_compat_json
);
impl_builtin_render_semantic!(
    crate::diagrams::packet::PacketDiagramRenderModel,
    crate::diagrams::packet::render_model_to_compat_json
);
impl_builtin_render_semantic!(
    crate::diagrams::timeline::TimelineDiagramRenderModel,
    crate::diagrams::timeline::render_model_to_compat_json
);
impl_builtin_render_semantic!(
    crate::diagrams::journey::JourneyDiagramRenderModel,
    crate::diagrams::journey::render_model_to_compat_json
);
impl_builtin_render_semantic!(
    crate::diagrams::requirement::RequirementDiagramRenderModel,
    crate::diagrams::requirement::render_model_to_compat_json
);
impl_builtin_render_semantic!(
    crate::diagrams::sankey::SankeyDiagramRenderModel,
    crate::diagrams::sankey::render_model_to_compat_json
);
impl_builtin_render_semantic!(
    crate::diagrams::radar::RadarDiagramRenderModel,
    crate::diagrams::radar::render_model_to_compat_json
);
impl_builtin_render_semantic!(
    crate::diagrams::info::InfoDiagramRenderModel,
    crate::diagrams::info::render_model_to_compat_json
);
impl_builtin_render_semantic!(
    crate::diagrams::treemap::TreemapDiagramRenderModel,
    crate::diagrams::treemap::render_model_to_compat_json
);
impl_builtin_render_semantic!(
    crate::diagrams::block::BlockDiagramRenderModel,
    crate::diagrams::block::render_model_to_compat_json
);
impl_builtin_render_semantic!(
    crate::diagrams::er::ErDiagramRenderModel,
    crate::diagrams::er::render_model_to_compat_json
);
impl_builtin_render_semantic!(
    crate::diagrams::quadrant_chart::QuadrantChartRenderModel,
    crate::diagrams::quadrant_chart::render_model_to_compat_json
);
impl_builtin_render_semantic!(
    crate::diagrams::xychart::XyChartDiagramRenderModel,
    crate::diagrams::xychart::render_model_to_compat_json
);
impl_builtin_render_semantic!(
    crate::diagrams::git_graph::GitGraphRenderModel,
    crate::diagrams::git_graph::render_model_to_compat_json
);
impl_builtin_render_semantic!(
    crate::diagrams::tree_view::TreeViewDiagramRenderModel,
    crate::diagrams::tree_view::render_model_to_compat_json
);
impl_builtin_render_semantic!(
    crate::diagrams::ishikawa::IshikawaDiagramRenderModel,
    crate::diagrams::ishikawa::render_model_to_compat_json
);
impl_builtin_render_semantic!(
    crate::diagrams::eventmodeling::EventModelingDiagramRenderModel,
    crate::diagrams::eventmodeling::render_model_to_compat_json
);
impl_builtin_render_semantic!(
    crate::diagrams::venn::VennDiagramRenderModel,
    crate::diagrams::venn::render_model_to_compat_json
);
impl_builtin_render_semantic!(
    crate::diagrams::wardley::WardleyDiagramRenderModel,
    crate::diagrams::wardley::render_model_to_compat_json
);

impl RenderSemanticModel {
    /// Applies Mermaid common DB sanitization to family-owned typed fields.
    pub(crate) fn sanitize_common_db_fields(&mut self, config: &MermaidConfig) {
        match self {
            Self::Error(_) => {}
            Self::CustomJson(v) => {
                crate::common_db::apply_common_db_sanitization(&mut v.value, config);
            }
            Self::Mindmap(_) => {}
            Self::State(v) => v.sanitize_common_db_fields(config),
            Self::Sequence(v) => v.sanitize_common_db_fields(config),
            Self::Zenuml(v) => v.sanitize_common_db_fields(config),
            Self::Flowchart(v) => v.sanitize_common_db_fields(config),
            Self::Architecture(v) => v.sanitize_common_db_fields(config),
            Self::Class(v) => v.sanitize_common_db_fields(config),
            Self::C4(v) => v.sanitize_common_db_fields(config),
            Self::Cynefin(v) => v.sanitize_common_db_fields(config),
            Self::Railroad(v) => v.sanitize_common_db_fields(config),
            Self::Kanban(_) => {}
            Self::Gantt(v) => v.sanitize_common_db_fields(config),
            Self::Pie(v) => v.sanitize_common_db_fields(config),
            Self::Packet(v) => v.sanitize_common_db_fields(config),
            Self::Timeline(v) => v.sanitize_common_db_fields(config),
            Self::Journey(v) => v.sanitize_common_db_fields(config),
            Self::Requirement(v) => v.sanitize_common_db_fields(config),
            Self::Sankey(_) => {}
            Self::Radar(v) => v.sanitize_common_db_fields(config),
            Self::Info(_) => {}
            Self::Treemap(v) => v.sanitize_common_db_fields(config),
            Self::Block(_) => {}
            Self::Er(v) => v.sanitize_common_db_fields(config),
            Self::QuadrantChart(v) => v.sanitize_common_db_fields(config),
            Self::XyChart(v) => v.sanitize_common_db_fields(config),
            Self::GitGraph(v) => v.sanitize_common_db_fields(config),
            Self::TreeView(v) => v.sanitize_common_db_fields(config),
            Self::Ishikawa(v) => v.sanitize_common_db_fields(config),
            Self::EventModeling(v) => v.sanitize_common_db_fields(config),
            Self::Venn(v) => v.sanitize_common_db_fields(config),
            Self::Wardley(v) => v.sanitize_common_db_fields(config),
        }
    }

    pub(crate) fn remap_warning_fact_spans(
        &mut self,
        mut remap: impl FnMut(&mut DiagramWarningFact),
    ) {
        match self {
            Self::CustomJson(v) => {
                Self::remap_json_warning_fact_spans(&mut v.value, &mut remap);
            }
            Self::Flowchart(v) => Self::remap_warning_fact_slice(&mut v.warning_facts, &mut remap),
            Self::Block(v) => Self::remap_warning_fact_slice(&mut v.warning_facts, &mut remap),
            Self::GitGraph(v) => Self::remap_warning_fact_slice(&mut v.warning_facts, &mut remap),
            _ => {}
        }
    }

    fn remap_warning_fact_slice(
        facts: &mut [DiagramWarningFact],
        remap: &mut impl FnMut(&mut DiagramWarningFact),
    ) {
        for fact in facts {
            remap(fact);
        }
    }

    fn remap_json_warning_fact_spans(
        model: &mut Value,
        remap: &mut impl FnMut(&mut DiagramWarningFact),
    ) {
        let Some(warning_facts_value) = model.get_mut("warningFacts") else {
            return;
        };
        let Ok(mut warning_facts) =
            serde_json::from_value::<Vec<DiagramWarningFact>>(warning_facts_value.clone())
        else {
            return;
        };

        Self::remap_warning_fact_slice(&mut warning_facts, remap);
        *warning_facts_value = serde_json::json!(warning_facts);
    }

    /// Returns a stable family label for diagnostics and timing output.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Error(_) => "error",
            Self::CustomJson(_) => "custom-json",
            Self::Mindmap(_) => "mindmap",
            Self::State(_) => "state",
            Self::Sequence(_) => "sequence",
            Self::Zenuml(_) => "zenuml",
            Self::Flowchart(_) => "flowchart",
            Self::Architecture(_) => "architecture",
            Self::Class(_) => "class",
            Self::C4(_) => "c4",
            Self::Cynefin(_) => "cynefin",
            Self::Railroad(_) => "railroad",
            Self::Kanban(_) => "kanban",
            Self::Gantt(_) => "gantt",
            Self::Pie(_) => "pie",
            Self::Packet(_) => "packet",
            Self::Timeline(_) => "timeline",
            Self::Journey(_) => "journey",
            Self::Requirement(_) => "requirement",
            Self::Sankey(_) => "sankey",
            Self::Radar(_) => "radar",
            Self::Info(_) => "info",
            Self::Treemap(_) => "treemap",
            Self::Block(_) => "block",
            Self::Er(_) => "er",
            Self::QuadrantChart(_) => "quadrantChart",
            Self::XyChart(_) => "xychart",
            Self::GitGraph(_) => "gitGraph",
            Self::TreeView(_) => "treeView",
            Self::Ishikawa(_) => "ishikawa",
            Self::EventModeling(_) => "eventmodeling",
            Self::Venn(_) => "venn",
            Self::Wardley(_) => "wardley",
        }
    }

    /// Projects this family-owned typed model into the public compatibility JSON shape.
    ///
    /// Built-in families delegate to their own lossless projector. This never reparses source;
    /// custom adapters retain their explicitly named JSON boundary.
    pub fn compatibility_json(&self, meta: &ParseMetadata) -> Result<Value> {
        match self {
            Self::Error(model) => model.compatibility_json(meta),
            Self::CustomJson(model) => Ok(crate::config::clone_value_nonrecursive(model.value())),
            Self::Mindmap(model) => model.compatibility_json(meta),
            Self::State(model) => model.compatibility_json(meta),
            Self::Sequence(model) => model.compatibility_json(meta),
            Self::Zenuml(model) => model.compatibility_json(meta),
            Self::Flowchart(model) => model.compatibility_json(meta),
            Self::Architecture(model) => model.compatibility_json(meta),
            Self::Class(model) => model.compatibility_json(meta),
            Self::C4(model) => model.compatibility_json(meta),
            Self::Cynefin(model) => model.compatibility_json(meta),
            Self::Railroad(model) => model.compatibility_json(meta),
            Self::Kanban(model) => model.compatibility_json(meta),
            Self::Gantt(model) => model.compatibility_json(meta),
            Self::Pie(model) => model.compatibility_json(meta),
            Self::Packet(model) => model.compatibility_json(meta),
            Self::Timeline(model) => model.compatibility_json(meta),
            Self::Journey(model) => model.compatibility_json(meta),
            Self::Requirement(model) => model.compatibility_json(meta),
            Self::Sankey(model) => model.compatibility_json(meta),
            Self::Radar(model) => model.compatibility_json(meta),
            Self::Info(model) => model.compatibility_json(meta),
            Self::Treemap(model) => model.compatibility_json(meta),
            Self::Block(model) => model.compatibility_json(meta),
            Self::Er(model) => model.compatibility_json(meta),
            Self::QuadrantChart(model) => model.compatibility_json(meta),
            Self::XyChart(model) => model.compatibility_json(meta),
            Self::GitGraph(model) => model.compatibility_json(meta),
            Self::TreeView(model) => model.compatibility_json(meta),
            Self::Ishikawa(model) => model.compatibility_json(meta),
            Self::EventModeling(model) => model.compatibility_json(meta),
            Self::Venn(model) => model.compatibility_json(meta),
            Self::Wardley(model) => model.compatibility_json(meta),
        }
    }

    /// Returns whether this typed model can represent the given Mermaid diagram type id.
    pub fn supports_diagram_type(&self, diagram_type: &str) -> bool {
        match self {
            Self::CustomJson(model) => {
                !crate::family::is_builtin_diagram_type(diagram_type)
                    && model.model_name() == diagram_type
            }
            other => {
                crate::family::render_model_kind_supports_diagram_type(other.kind(), diagram_type)
            }
        }
    }
}

/// Registry for typed render-model parsers keyed by Mermaid diagram type id.
#[derive(Debug, Clone)]
pub struct RenderDiagramRegistry {
    builtins: Arc<HashMap<&'static str, BuiltInRenderSemanticParser>>,
    overlays: Arc<HashMap<&'static str, CustomJsonRenderParser>>,
}

#[derive(Clone, Copy)]
pub(crate) enum ResolvedRenderParser {
    BuiltIn(BuiltInRenderSemanticParser),
    Custom(CustomJsonRenderParser),
}

impl Default for RenderDiagramRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderDiagramRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self {
            builtins: Arc::new(HashMap::new()),
            overlays: Arc::new(HashMap::new()),
        }
    }

    /// Registers or replaces a custom JSON render-model parser for a diagram type id.
    pub fn insert(&mut self, diagram_type: &'static str, parser: CustomJsonRenderParser) {
        Arc::make_mut(&mut self.overlays).insert(diagram_type, parser);
    }

    /// Returns whether a built-in or custom render-model parser is registered for the id.
    pub fn contains(&self, diagram_type: &str) -> bool {
        self.resolve(diagram_type).is_some()
    }

    pub(crate) fn resolve(&self, diagram_type: &str) -> Option<ResolvedRenderParser> {
        if let Some(parser) = self.overlays.get(diagram_type).copied() {
            return Some(ResolvedRenderParser::Custom(parser));
        }
        self.builtins
            .get(diagram_type)
            .copied()
            .map(ResolvedRenderParser::BuiltIn)
    }

    #[cfg(test)]
    pub(crate) fn remove(&mut self, diagram_type: &str) -> bool {
        Arc::make_mut(&mut self.overlays)
            .remove(diagram_type)
            .is_some()
            || Arc::make_mut(&mut self.builtins)
                .remove(diagram_type)
                .is_some()
    }

    /// Builds the typed render parser registry for the repository's pinned Mermaid baseline.
    pub fn pinned_mermaid_baseline() -> Self {
        let mut reg = Self::new();
        for fact in crate::family::render_parser_facts() {
            Arc::make_mut(&mut reg.builtins).insert(fact.id, fact.parser);
        }

        reg
    }

    #[cfg(test)]
    pub(crate) fn parser_ids(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.builtins
            .keys()
            .chain(
                self.overlays
                    .keys()
                    .filter(|id| !self.builtins.contains_key(**id)),
            )
            .copied()
    }
}

/// Parsed diagram metadata plus its canonically paired typed render model.
///
/// Construction is restricted to `merman-core`; its parse pipeline pairs the metadata and model.
/// External consumers may inspect the pair or consume it, but cannot assemble metadata and a
/// model produced by different parse operations:
///
/// ```compile_fail,E0451
/// use merman_core::{ParseMetadata, ParsedDiagramRender, RenderSemanticModel};
///
/// fn forge(
///     meta: ParseMetadata,
///     model: RenderSemanticModel,
/// ) -> ParsedDiagramRender {
///     ParsedDiagramRender { meta, model }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct ParsedDiagramRender {
    /// Diagram type and effective configuration extracted during preprocessing.
    meta: ParseMetadata,
    /// Typed model consumed by layout and SVG renderers.
    model: RenderSemanticModel,
    /// Parser-owned render-only data paired with the typed model.
    context: RenderSemanticContext,
}

impl ParsedDiagramRender {
    pub(crate) fn new(meta: ParseMetadata, model: RenderSemanticModel) -> Self {
        Self {
            meta,
            model,
            context: RenderSemanticContext::default(),
        }
    }

    pub(crate) fn from_parse_output(
        meta: ParseMetadata,
        output: RenderSemanticParseOutput,
    ) -> Self {
        let (model, context) = output.into_parts();
        Self {
            meta,
            model,
            context,
        }
    }

    /// Returns the metadata paired with this render model by the core parse pipeline.
    pub fn metadata(&self) -> &ParseMetadata {
        &self.meta
    }

    /// Returns the typed render model paired with this metadata by the core parse pipeline.
    pub fn model(&self) -> &RenderSemanticModel {
        &self.model
    }

    /// Consumes the parsed diagram and returns its canonical metadata/model projection.
    ///
    /// This intentionally omits parser-owned render context. Renderer integrations that need the
    /// complete built-in render state should consume [`Self::into_render_parts`] instead.
    pub fn into_parts(self) -> (ParseMetadata, RenderSemanticModel) {
        (self.meta, self.model)
    }

    /// Consumes the parsed diagram and includes parser-owned render context for renderer crates.
    #[doc(hidden)]
    pub fn into_render_parts(self) -> (ParseMetadata, RenderSemanticModel, RenderSemanticContext) {
        (self.meta, self.model, self.context)
    }

    #[doc(hidden)]
    pub fn retained_render_context_bytes(&self) -> usize {
        self.context.retained_text_bytes()
    }

    /// Borrows parser-owned Flowchart render label sources without consuming the parsed model.
    #[doc(hidden)]
    pub fn flowchart_render_label_sources(
        &self,
    ) -> Option<&crate::diagrams::flowchart::FlowchartRenderLabelSources> {
        self.context.flowchart_label_sources()
    }
}

/// Parses with a registry entry or reports an unsupported Mermaid diagram type.
pub(crate) fn parse_or_unsupported(
    registry: &DiagramRegistry,
    diagram_type: &str,
    code: &str,
    meta: &ParseMetadata,
) -> Result<Value> {
    let Some(parser) = registry.resolve(diagram_type) else {
        return Err(Error::UnsupportedDiagram {
            diagram_type: diagram_type.to_string(),
        });
    };
    match parser {
        ResolvedSemanticParser::BuiltIn(parser) => parser(code, meta),
        ResolvedSemanticParser::Custom(parser) => {
            let control = ParseControl::new();
            parser(code, meta, &control).map_err(Error::from)?
        }
    }
}

/// Parses with a registry entry while preserving the caller-owned parse control.
pub(crate) fn parse_or_unsupported_controlled(
    registry: &DiagramRegistry,
    diagram_type: &str,
    code: &str,
    meta: &ParseMetadata,
    control: &ParseControl,
) -> ParseControlResult<Result<Value>> {
    control.checkpoint()?;
    let Some(parser) = registry.resolve(diagram_type) else {
        return Ok(Err(Error::UnsupportedDiagram {
            diagram_type: diagram_type.to_string(),
        }));
    };
    let result = match parser {
        ResolvedSemanticParser::BuiltIn(parser) => parser(code, meta),
        ResolvedSemanticParser::Custom(parser) => parser(code, meta, control)?,
    };
    control.checkpoint()?;
    Ok(result)
}

#[cfg(test)]
mod registry_clone_tests {
    use super::*;
    use std::sync::Arc;

    fn custom_semantic_parser(
        _code: &str,
        _meta: &ParseMetadata,
        control: &ParseControl,
    ) -> ParseControlResult<Result<Value>> {
        control.checkpoint()?;
        Ok(Ok(Value::Null))
    }

    fn custom_render_parser(
        _code: &str,
        _meta: &ParseMetadata,
        control: &ParseControl,
    ) -> ParseControlResult<Result<CustomJsonRenderModel>> {
        control.checkpoint()?;
        Ok(Ok(CustomJsonRenderModel::new(
            "copy-on-write-render-test",
            Value::Null,
        )))
    }

    fn cancelling_semantic_parser(
        _code: &str,
        _meta: &ParseMetadata,
        control: &ParseControl,
    ) -> ParseControlResult<Result<Value>> {
        control.cancel();
        control.checkpoint()?;
        unreachable!("cancelled parser must stop at its checkpoint")
    }

    #[test]
    fn semantic_registry_clone_uses_copy_on_write_storage() {
        let original = DiagramRegistry::pinned_mermaid_baseline();
        let mut cloned = original.clone();

        assert!(Arc::ptr_eq(&original.builtins, &cloned.builtins));
        assert!(Arc::ptr_eq(&original.overlays, &cloned.overlays));

        cloned.insert("copy-on-write-semantic-test", custom_semantic_parser);

        assert!(Arc::ptr_eq(&original.builtins, &cloned.builtins));
        assert!(!Arc::ptr_eq(&original.overlays, &cloned.overlays));
        assert!(original.resolve("copy-on-write-semantic-test").is_none());
        assert!(matches!(
            cloned.resolve("copy-on-write-semantic-test"),
            Some(ResolvedSemanticParser::Custom(_))
        ));
    }

    #[test]
    fn custom_semantic_parsers_share_the_operation_parse_control() {
        let mut engine = crate::Engine::new();
        engine
            .diagram_registry_mut()
            .insert("flowchart-v2", cancelling_semantic_parser);
        let control = ParseControl::new();

        assert!(matches!(
            engine.parse_diagram_snapshot_controlled_sync("flowchart TD\nA-->B\n", &control),
            Err(crate::ParseCancelled)
        ));
        assert!(control.is_cancelled());
    }

    #[test]
    fn custom_parser_cancellation_is_typed_for_non_cancellable_snapshot_apis() {
        let mut engine = crate::Engine::new();
        engine
            .diagram_registry_mut()
            .insert("flowchart-v2", cancelling_semantic_parser);
        let source = "flowchart TD\nA-->B\n";

        assert!(matches!(
            engine.parse_diagram_snapshot_sync(source),
            Err(Error::ParseCancelled(_))
        ));
        assert!(matches!(
            engine.parse_diagram_snapshot_with_type_sync("flowchart-v2", source),
            Err(Error::ParseCancelled(_))
        ));
    }

    #[test]
    fn render_registry_clone_uses_copy_on_write_storage() {
        let original = RenderDiagramRegistry::pinned_mermaid_baseline();
        let mut cloned = original.clone();

        assert!(Arc::ptr_eq(&original.builtins, &cloned.builtins));
        assert!(Arc::ptr_eq(&original.overlays, &cloned.overlays));

        cloned.insert("copy-on-write-render-test", custom_render_parser);

        assert!(Arc::ptr_eq(&original.builtins, &cloned.builtins));
        assert!(!Arc::ptr_eq(&original.overlays, &cloned.overlays));
        assert!(!original.contains("copy-on-write-render-test"));
        assert!(cloned.contains("copy-on-write-render-test"));
    }
}
