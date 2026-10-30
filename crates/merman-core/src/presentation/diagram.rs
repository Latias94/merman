#[cfg(feature = "compat-json")]
use crate::Error;
#[cfg(test)]
use crate::OperationPhase;
#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
use crate::diagram::CapturedPanic;
use crate::json_value::ManagedJson as ManagedSemanticJson;
#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
use crate::preprocess::SourceConfigEvidence;
use crate::{
    MermaidConfig, OperationControl, OperationControlResult, PresentationMetadata, Result,
    SourcePresentationMetadata,
};
use merman_model::warnings::DiagramWarningFact;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Parser used by a custom semantic JSON registry overlay.
///
/// Implementations must call [`OperationControl::checkpoint`] inside potentially long-running loops.
/// The outer [`OperationControlResult`] reports cooperative cancellation; the inner [`Result`]
/// reports Mermaid detection or parse failures. Non-cancellable `MermaidEngine` snapshot and model APIs
/// surface an unexpected outer cancellation as [`crate::Error::OperationCancelled`].
#[cfg(feature = "compat-json")]
pub type DiagramSemanticParser = fn(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<Value>>;

/// Parser used by the pinned built-in semantic JSON path.
#[cfg(feature = "compat-json")]
pub(crate) type BuiltInDiagramSemanticParser =
    fn(code: &str, meta: &SourcePresentationMetadata) -> Result<Value>;

/// Parser used by the built-in typed render-model path for one Mermaid diagram family.
pub(crate) type BuiltInRenderSemanticParser =
    fn(
        code: &str,
        meta: &SourcePresentationMetadata,
        control: &OperationControl,
    ) -> OperationControlResult<Result<RenderSemanticParseOutput>>;

/// Parser used by a custom render-model registry overlay.
///
/// Custom adapters intentionally return only a named JSON model. Built-in typed variants are
/// reserved for the pinned family catalog and cannot be manufactured through this interface.
pub type CustomJsonRenderParser = fn(
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<CustomJsonRenderModel>>;

/// Ownership of a parser resolved from a built-in registry plus its custom overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
pub(crate) enum RegistryOwner {
    BuiltIn,
    Custom,
}

#[derive(Debug, Clone, Copy)]
#[cfg(feature = "compat-json")]
pub(crate) enum ResolvedSemanticParser {
    BuiltIn(BuiltInDiagramSemanticParser),
    Custom(DiagramSemanticParser),
}

#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
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
#[cfg(feature = "compat-json")]
pub struct DiagramRegistry {
    builtins: Arc<HashMap<&'static str, BuiltInDiagramSemanticParser>>,
    overlays: Arc<HashMap<&'static str, DiagramSemanticParser>>,
}

#[cfg(feature = "compat-json")]
impl Default for DiagramRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "compat-json")]
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

    #[cfg(all(test, feature = "all-diagrams"))]
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
#[cfg(feature = "compat-json")]
pub struct ParsedDiagram {
    /// Diagram type and effective configuration extracted during preprocessing.
    pub meta: PresentationMetadata,
    /// Semantic JSON model matching Mermaid's parser/database output shape where possible.
    pub model: ManagedSemanticJson,
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
pub use crate::editor::ParsedEditorFacts;

/// Semantic result retained by one editor-facing diagram parse operation.
#[derive(Debug)]
#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
pub enum DiagramParseOutcome {
    Parsed {
        /// Mermaid-compatible semantic JSON retained for existing projections.
        model: ManagedSemanticJson,
        /// Parser-owned warning facts after preprocessing spans have been remapped once.
        warning_facts: Vec<DiagramWarningFact>,
    },
    Failed(Error),
    /// Family construction panicked after preprocessing produced valid metadata.
    Panicked(String),
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
impl DiagramParseOutcome {
    /// Returns the semantic model when family construction succeeded.
    pub fn parsed_model(&self) -> Option<&ManagedSemanticJson> {
        match self {
            Self::Parsed { model, .. } => Some(model),
            Self::Failed(_) | Self::Panicked(_) => None,
        }
    }
}

/// One preprocessing, detection, and family-construction operation for editor consumers.
///
/// Metadata, semantic output or its original error, typed warnings, and recovery facts are
/// retained together so downstream analysis cannot reconstruct parser state from compatibility
/// JSON or by parsing the source again.
#[derive(Debug)]
#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
pub struct DiagramParseSnapshot {
    meta: PresentationMetadata,
    outcome: DiagramParseOutcome,
    editor_facts: ParsedEditorFacts,
    source_config: SourceConfigEvidence,
    recovered_incomplete_directive: bool,
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
impl DiagramParseSnapshot {
    pub(crate) fn new(
        meta: PresentationMetadata,
        outcome: DiagramParseOutcome,
        editor_facts: ParsedEditorFacts,
        source_config: SourceConfigEvidence,
        recovered_incomplete_directive: bool,
    ) -> Self {
        Self {
            meta,
            outcome,
            editor_facts,
            source_config,
            recovered_incomplete_directive,
        }
    }

    /// Consumes the closed snapshot into its three operation-owned projections.
    pub fn into_parts(self) -> (PresentationMetadata, DiagramParseOutcome, ParsedEditorFacts) {
        (self.meta, self.outcome, self.editor_facts)
    }

    /// Consumes the snapshot while retaining source-configuration evidence from preprocessing.
    pub fn into_parts_with_source_config(
        self,
    ) -> (
        PresentationMetadata,
        DiagramParseOutcome,
        ParsedEditorFacts,
        SourceConfigEvidence,
    ) {
        (
            self.meta,
            self.outcome,
            self.editor_facts,
            self.source_config,
        )
    }

    /// Returns metadata produced by this preprocessing and detection operation.
    pub fn metadata(&self) -> &PresentationMetadata {
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

    /// Source-backed frontmatter and directive evidence from the outer preprocessing pass.
    pub fn source_config(&self) -> &SourceConfigEvidence {
        &self.source_config
    }

    /// Whether editor preprocessing recovered an unterminated directive line.
    pub const fn recovered_incomplete_directive(&self) -> bool {
        self.recovered_incomplete_directive
    }
}

/// Non-cancellation result of one editor snapshot capture operation.
// Keep successful snapshots inline rather than allocating for every successful capture.

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
pub enum DiagramSnapshotCapture {
    Snapshot(Option<DiagramParseSnapshot>),
    Failed {
        error: Error,
        source_config: SourceConfigEvidence,
    },
    /// Detection or metadata finalization panicked after preprocessing evidence was captured.
    ///
    /// No diagram metadata is attached because automatic detection did not complete.
    Panicked {
        panic: CapturedPanic,
        source_config: SourceConfigEvidence,
    },
}

#[cfg(feature = "compat-json")]
#[cfg(feature = "editor-facts")]
impl DiagramSnapshotCapture {
    pub(crate) fn into_result(self) -> Result<Option<DiagramParseSnapshot>> {
        match self {
            Self::Snapshot(snapshot) => Ok(snapshot),
            Self::Failed { error, .. } => Err(error),
            // The legacy Result facade has no panic outcome. Resume the exact original payload;
            // evidence-aware callers consume this capture before reaching this adapter.
            Self::Panicked { panic, .. } => std::panic::resume_unwind(panic.into_payload()),
        }
    }
}

#[cfg(all(test, feature = "compat-json"))]
use merman_model::presentation::BuiltinRenderSemantic;
#[cfg(test)]
use merman_model::presentation::CustomJsonProvenance;
use merman_model::presentation::PresentationDiagram;
use merman_model::presentation::{CustomJsonRenderModel, PresentationContext, RenderSemanticModel};

#[derive(Debug, Clone)]
pub(crate) struct RenderSemanticParseOutput {
    model: RenderSemanticModel,
    context: PresentationContext,
}

impl RenderSemanticParseOutput {
    pub(crate) fn new(model: RenderSemanticModel) -> Self {
        Self {
            model,
            context: PresentationContext::default(),
        }
    }

    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    pub(crate) fn flowchart(
        model: crate::diagrams::flowchart::FlowchartModel,
        context: crate::diagrams::flowchart::FlowchartRenderContext,
    ) -> Self {
        Self {
            model: RenderSemanticModel::Flowchart(model),
            context: PresentationContext::Flowchart(context),
        }
    }

    #[cfg(feature = "diagram-mindmap")]
    pub(crate) fn mindmap(
        model: merman_model::families::mindmap::MindmapDiagramRenderModel,
    ) -> Self {
        let diagram_id = merman_model::families::mindmap::diagram_id_from_context(
            &crate::runtime::active_operation_context(),
        );
        Self {
            model: RenderSemanticModel::Mindmap(model),
            context: PresentationContext::Mindmap { diagram_id },
        }
    }

    #[cfg(feature = "diagram-class")]
    pub(crate) fn class(
        model: merman_model::families::class::ClassDiagram,
        style_precedence_facts: merman_model::families::class::ClassStylePrecedenceFacts,
    ) -> Self {
        Self {
            model: RenderSemanticModel::Class(model),
            context: PresentationContext::Class(style_precedence_facts),
        }
    }

    #[cfg(feature = "diagram-info")]
    pub(crate) fn info(
        model: merman_model::families::info::InfoDiagramModel,
        output_data: merman_model::families::info::InfoOutputData,
    ) -> Self {
        Self {
            model: RenderSemanticModel::Info(model),
            context: PresentationContext::Info(output_data),
        }
    }

    #[cfg(feature = "diagram-pie")]
    pub(crate) fn pie(
        model: merman_model::families::pie::PieDiagramModel,
        output_data: merman_model::families::pie::PieOutputData,
    ) -> Self {
        Self {
            model: RenderSemanticModel::Pie(model),
            context: PresentationContext::Pie(output_data),
        }
    }

    #[cfg(feature = "diagram-journey")]
    pub(crate) fn journey(
        model: merman_model::families::journey::JourneyDiagramModel,
        output_data: merman_model::families::journey::JourneyOutputData,
    ) -> Self {
        Self {
            model: RenderSemanticModel::Journey(model),
            context: PresentationContext::Journey(output_data),
        }
    }

    #[cfg(feature = "diagram-timeline")]
    pub(crate) fn timeline(
        model: merman_model::families::timeline::TimelineDiagramModel,
        output_data: merman_model::families::timeline::TimelineOutputData,
    ) -> Self {
        Self {
            model: RenderSemanticModel::Timeline(model),
            context: PresentationContext::Timeline(output_data),
        }
    }

    #[cfg(feature = "diagram-packet")]
    pub(crate) fn packet(
        model: merman_model::families::packet::PacketDiagramModel,
        output_data: merman_model::families::packet::PacketOutputData,
    ) -> Self {
        Self {
            model: RenderSemanticModel::Packet(model),
            context: PresentationContext::Packet(output_data),
        }
    }

    #[cfg(feature = "diagram-radar")]
    pub(crate) fn radar(
        model: merman_model::families::radar::RadarDiagramModel,
        output_data: merman_model::families::radar::RadarOutputData,
    ) -> Self {
        Self {
            model: RenderSemanticModel::Radar(model),
            context: PresentationContext::Radar(output_data),
        }
    }

    pub(crate) fn model(&self) -> &RenderSemanticModel {
        &self.model
    }

    pub(crate) fn model_mut(&mut self) -> &mut RenderSemanticModel {
        &mut self.model
    }

    fn into_parts(self) -> (RenderSemanticModel, PresentationContext) {
        (self.model, self.context)
    }
}

/// Applies Mermaid common DB sanitization to family-owned typed fields.
pub(crate) fn sanitize_common_db_fields(model: &mut RenderSemanticModel, config: &MermaidConfig) {
    #[allow(
        unreachable_patterns,
        reason = "Independent consumers may select model data without selecting its source parser."
    )]
    match model {
        RenderSemanticModel::Error(_) => {}
        RenderSemanticModel::CustomJson(v) => {
            crate::common_db::apply_common_db_sanitization(v.json_mut().as_value_mut(), config);
        }
        #[cfg(feature = "diagram-mindmap")]
        RenderSemanticModel::Mindmap(_) => {}
        #[cfg(feature = "diagram-state")]
        RenderSemanticModel::State(v) => {
            crate::diagrams::state::sanitize_render_common_db_fields(v, config)
        }
        #[cfg(feature = "diagram-sequence")]
        RenderSemanticModel::Sequence(v) => {
            crate::diagrams::sequence::sanitize_common_db_fields(v, config)
        }
        #[cfg(feature = "diagram-zenuml")]
        RenderSemanticModel::Zenuml(_) => {}
        #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
        RenderSemanticModel::Flowchart(v) => {
            crate::diagrams::flowchart::sanitize_common_db_fields(v, config)
        }
        #[cfg(feature = "diagram-architecture")]
        RenderSemanticModel::Architecture(v) => {
            crate::diagrams::architecture::sanitize_common_db_fields(v, config)
        }
        #[cfg(feature = "diagram-class")]
        RenderSemanticModel::Class(v) => {
            crate::diagrams::class::source_data::sanitize_full_common_db_fields(v, config)
        }
        #[cfg(feature = "diagram-c4")]
        RenderSemanticModel::C4(v) => crate::diagrams::c4::sanitize_common_db_fields(v, config),
        #[cfg(feature = "diagram-cynefin")]
        RenderSemanticModel::Cynefin(v) => {
            crate::diagrams::cynefin::sanitize_common_db_fields(v, config)
        }
        #[cfg(feature = "diagram-railroad")]
        RenderSemanticModel::Railroad(v) => {
            crate::diagrams::railroad::sanitize_common_db_fields(v, config)
        }
        #[cfg(feature = "diagram-kanban")]
        RenderSemanticModel::Kanban(_) => {}
        #[cfg(feature = "diagram-gantt")]
        RenderSemanticModel::Gantt(v) => {
            crate::diagrams::gantt::sanitize_common_db_fields(&mut v.model, config)
        }
        #[cfg(feature = "diagram-pie")]
        RenderSemanticModel::Pie(v) => crate::diagrams::pie::sanitize_common_db_fields(v, config),
        #[cfg(feature = "diagram-packet")]
        RenderSemanticModel::Packet(v) => {
            crate::diagrams::packet::sanitize_common_db_fields(v, config)
        }
        #[cfg(feature = "diagram-timeline")]
        RenderSemanticModel::Timeline(v) => {
            crate::diagrams::timeline::sanitize_common_db_fields(v, config)
        }
        #[cfg(feature = "diagram-journey")]
        RenderSemanticModel::Journey(v) => {
            crate::diagrams::journey::sanitize_common_db_fields(v, config)
        }
        #[cfg(feature = "diagram-requirement")]
        RenderSemanticModel::Requirement(v) => {
            crate::diagrams::requirement::sanitize_common_db_fields(v, config)
        }
        #[cfg(feature = "diagram-sankey")]
        RenderSemanticModel::Sankey(_) => {}
        #[cfg(feature = "diagram-radar")]
        RenderSemanticModel::Radar(v) => {
            crate::diagrams::radar::sanitize_common_db_fields(v, config)
        }
        #[cfg(feature = "diagram-info")]
        RenderSemanticModel::Info(_) => {}
        #[cfg(feature = "diagram-treemap")]
        RenderSemanticModel::Treemap(v) => {
            crate::diagrams::treemap::sanitize_common_db_fields(&mut v.semantic, config)
        }
        #[cfg(feature = "diagram-block")]
        RenderSemanticModel::Block(_) => {}
        #[cfg(feature = "diagram-er")]
        RenderSemanticModel::Er(v) => crate::diagrams::er::sanitize_common_db_fields(v, config),
        #[cfg(feature = "diagram-quadrant-chart")]
        RenderSemanticModel::QuadrantChart(v) => {
            crate::diagrams::quadrant_chart::sanitize_common_db_fields(v, config)
        }
        #[cfg(feature = "diagram-xychart")]
        RenderSemanticModel::XyChart(v) => {
            crate::diagrams::xychart::sanitize_common_db_fields(&mut v.semantic, config)
        }
        #[cfg(feature = "diagram-git-graph")]
        RenderSemanticModel::GitGraph(v) => {
            crate::diagrams::git_graph::sanitize_common_db_fields(v, config)
        }
        #[cfg(feature = "diagram-tree-view")]
        RenderSemanticModel::TreeView(v) => {
            crate::diagrams::tree_view::sanitize_common_db_fields(v, config)
        }
        #[cfg(feature = "diagram-ishikawa")]
        RenderSemanticModel::Ishikawa(v) => {
            crate::diagrams::ishikawa::sanitize_common_db_fields(v, config)
        }
        #[cfg(feature = "diagram-event-modeling")]
        RenderSemanticModel::EventModeling(v) => {
            crate::diagrams::eventmodeling::sanitize_common_db_fields(v, config)
        }
        #[cfg(feature = "diagram-venn")]
        RenderSemanticModel::Venn(v) => crate::diagrams::venn::sanitize_common_db_fields(v, config),
        #[cfg(feature = "diagram-wardley")]
        RenderSemanticModel::Wardley(v) => {
            crate::diagrams::wardley::sanitize_common_db_fields(v, config)
        }
        #[cfg(feature = "diagram-usecase")]
        RenderSemanticModel::Usecase(v) => {
            crate::diagrams::usecase::sanitize_common_db_fields(v, config)
        }
        #[cfg(feature = "diagram-agentflow")]
        RenderSemanticModel::Agentflow(v) => {
            crate::diagrams::agentflow::sanitize_common_db_fields(&mut v.model, config)
        }
        _ => {}
    }
}

pub(crate) fn offset_parser_diagnostic_positions(
    model: &mut RenderSemanticModel,
    line_offset: usize,
    column_offset: usize,
) {
    #[cfg(not(feature = "diagram-agentflow"))]
    let _ = (model, line_offset, column_offset);
    #[cfg(feature = "diagram-agentflow")]
    if let RenderSemanticModel::Agentflow(model) = model {
        crate::diagrams::agentflow::offset_diagnostic_positions(
            &mut model.model,
            line_offset,
            column_offset,
        );
    }
}

pub(crate) fn remap_warning_fact_spans(
    model: &mut RenderSemanticModel,
    mut remap: impl FnMut(&mut DiagramWarningFact),
) {
    match model {
        RenderSemanticModel::CustomJson(v) => {
            remap_json_warning_fact_spans(v.json_mut().as_value_mut(), &mut remap);
        }
        #[cfg(feature = "diagram-agentflow")]
        RenderSemanticModel::Agentflow(v) => {
            remap_warning_fact_slice(&mut v.warning_facts, &mut remap)
        }
        #[cfg(any(
            feature = "diagram-flowchart",
            feature = "diagram-swimlane",
            feature = "diagram-agentflow"
        ))]
        RenderSemanticModel::Flowchart(v) => {
            remap_warning_fact_slice(&mut v.warning_facts, &mut remap)
        }
        #[cfg(feature = "diagram-block")]
        RenderSemanticModel::Block(v) => remap_warning_fact_slice(&mut v.warning_facts, &mut remap),
        #[cfg(feature = "diagram-git-graph")]
        RenderSemanticModel::GitGraph(v) => {
            remap_warning_fact_slice(&mut v.warning_facts, &mut remap)
        }
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
    let Ok(mut warning_facts) = Vec::<DiagramWarningFact>::deserialize(&*warning_facts_value)
    else {
        return;
    };

    remap_warning_fact_slice(&mut warning_facts, remap);
    crate::config::replace_value_nonrecursive(
        warning_facts_value,
        serde_json::json!(warning_facts),
    );
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

    #[cfg(all(test, feature = "diagram-flowchart"))]
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

    #[cfg(all(test, feature = "all-diagrams"))]
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

/// A successful source operation with its checked immutable presentation data.
///
/// Construction stays private to the source pipeline. A caller-authored `PresentationDiagram`
/// describes data relationships; only this wrapper records successful source construction.
#[derive(Debug, Clone)]
pub struct ParsedDiagramRender {
    // Keep nested fallible parse results small while the family parser uses the caller's stack.
    presentation: Box<PresentationDiagram>,
}

impl ParsedDiagramRender {
    pub(crate) fn from_parse_output_controlled(
        mut meta: PresentationMetadata,
        output: RenderSemanticParseOutput,
        control: &OperationControl,
    ) -> OperationControlResult<Result<Self>> {
        let (model, context) = output.into_parts();
        // A custom overlay or a suppressed source failure does not consume family paint facts
        // captured for the originally detected language. Preserve all other frozen ownership.
        if matches!(
            model,
            RenderSemanticModel::CustomJson(_) | RenderSemanticModel::Error(_)
        ) && meta.effective_config.family_paint_inputs().is_some()
        {
            let parts = meta
                .effective_config
                .frozen_parts()
                .with_family_paint_inputs(None);
            meta.effective_config = match merman_model::presentation::PresentationConfig::try_from_frozen_parts_controlled(
                parts,
                &crate::config::presentation::source_config_construction_policy(),
                control,
            )? {
                Ok(config) => config,
                Err(error) => return Ok(Err(crate::Error::diagram_parse_fallback(&meta.diagram_type, error.to_string()))),
            };
        }
        let diagram_type = meta.diagram_type.clone();
        Ok(PresentationDiagram::try_from_frozen(
            meta,
            model,
            context,
            &crate::config::presentation::source_config_construction_policy(),
            control,
        )?
        .map(|presentation| Self {
            presentation: Box::new(presentation),
        })
        .map_err(|error| crate::Error::diagram_parse_fallback(diagram_type, error.to_string())))
    }

    /// Borrows the complete immutable input accepted by output consumers.
    pub fn presentation(&self) -> &PresentationDiagram {
        &self.presentation
    }

    /// Consumes the source guarantee and retains all presentation data and context.
    pub fn into_presentation(self) -> PresentationDiagram {
        *self.presentation
    }

    pub fn metadata(&self) -> &PresentationMetadata {
        self.presentation.metadata()
    }

    pub fn model(&self) -> &RenderSemanticModel {
        self.presentation.model()
    }

    pub const fn family_id(&self) -> Option<merman_model::family::DiagramFamilyId> {
        self.presentation.family_id()
    }

    /// Extracts metadata and model, deliberately omitting output context.
    pub fn into_parts(self) -> (PresentationMetadata, RenderSemanticModel) {
        let (metadata, model, _) = (*self.presentation).into_parts();
        (metadata, model)
    }

    pub fn render_context(&self) -> &PresentationContext {
        self.presentation.render_context()
    }

    #[cfg(feature = "compat-json")]
    pub fn compatibility_json(&self) -> Result<ManagedSemanticJson> {
        self.presentation
            .compatibility_json()
            .map_err(crate::Error::from)
    }

    #[cfg(feature = "compat-json")]
    pub fn compatibility_json_controlled(
        &self,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ManagedSemanticJson>> {
        self.presentation
            .compatibility_json_controlled(control)
            .map(|result| result.map_err(crate::Error::from))
    }

    pub fn retained_render_context_bytes(&self) -> usize {
        self.presentation.retained_render_context_bytes()
    }

    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    pub fn flowchart_render_context(
        &self,
    ) -> Option<&merman_model::families::flowchart::FlowchartRenderContext> {
        self.render_context().flowchart_render_context()
    }

    #[cfg(feature = "diagram-class")]
    pub fn class_style_precedence_facts(
        &self,
    ) -> Option<&merman_model::families::class::ClassStylePrecedenceFacts> {
        self.render_context().class_style_precedence_facts()
    }
}

/// Parses with a registry entry while preserving the caller-owned parse control.
#[cfg(feature = "compat-json")]
pub(crate) fn parse_or_unsupported_controlled(
    registry: &DiagramRegistry,
    diagram_type: &str,
    code: &str,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<ManagedSemanticJson>> {
    control.checkpoint()?;
    let Some(parser) = registry.resolve(diagram_type) else {
        return Ok(Err(Error::UnsupportedDiagram {
            diagram_type: diagram_type.to_string(),
        }));
    };
    let result = match parser {
        ResolvedSemanticParser::BuiltIn(parser) => parser(code, meta),
        ResolvedSemanticParser::Custom(parser) => parser(code, meta, control)?,
    }
    .map(ManagedSemanticJson::from);
    control.checkpoint()?;
    Ok(result)
}

#[cfg(test)]
mod registry_clone_tests {
    use super::*;
    use std::sync::Arc;

    #[cfg(feature = "compat-json")]
    fn custom_semantic_parser(
        _code: &str,
        _meta: &SourcePresentationMetadata,
        control: &OperationControl,
    ) -> OperationControlResult<Result<Value>> {
        control.checkpoint()?;
        Ok(Ok(Value::Null))
    }

    fn custom_render_parser(
        _code: &str,
        _meta: &SourcePresentationMetadata,
        control: &OperationControl,
    ) -> OperationControlResult<Result<CustomJsonRenderModel>> {
        control.checkpoint()?;
        Ok(Ok(CustomJsonRenderModel::new(
            "copy-on-write-render-test",
            Value::Null,
        )))
    }

    #[cfg(feature = "compat-json")]
    fn cancelling_semantic_parser(
        _code: &str,
        _meta: &SourcePresentationMetadata,
        control: &OperationControl,
    ) -> OperationControlResult<Result<Value>> {
        control.cancel();
        control.checkpoint()?;
        unreachable!("cancelled parser must stop at its checkpoint")
    }

    #[test]
    #[cfg(feature = "compat-json")]
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
    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
    fn custom_semantic_parsers_share_the_operation_control() {
        let mut engine = crate::MermaidEngine::new();
        engine
            .diagram_registry_mut()
            .insert("flowchart-v2", cancelling_semantic_parser);
        let control = OperationControl::new();

        assert!(matches!(
            engine.parse_diagram_snapshot_controlled_sync("flowchart TD\nA-->B\n", &control),
            Err(crate::OperationCancelled { .. })
        ));
        assert!(control.is_cancelled());
    }

    #[test]
    #[cfg(feature = "compat-json")]
    #[cfg(feature = "editor-facts")]
    fn custom_parser_cancellation_is_typed_for_non_cancellable_snapshot_apis() {
        let mut engine = crate::MermaidEngine::new();
        engine
            .diagram_registry_mut()
            .insert("flowchart-v2", cancelling_semantic_parser);
        let source = "flowchart TD\nA-->B\n";

        assert!(matches!(
            engine.parse_diagram_snapshot_sync(source),
            Err(Error::OperationCancelled(_))
        ));
        assert!(matches!(
            engine.parse_diagram_snapshot_with_type_sync("flowchart-v2", source),
            Err(Error::OperationCancelled(_))
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

    #[test]
    fn custom_render_payload_retains_managed_lifecycle_without_legacy_json_parsing() {
        const DEPTH: usize = 5_000;
        fn parser(
            _code: &str,
            _meta: &SourcePresentationMetadata,
            control: &OperationControl,
        ) -> OperationControlResult<Result<CustomJsonRenderModel>> {
            let mut value = Value::Null;
            for _ in 0..DEPTH {
                value = Value::Array(vec![value]);
            }
            let model = CustomJsonRenderModel::new("managed-custom-render", value);
            control.checkpoint()?;
            Ok(Ok(model))
        }

        let mut engine = crate::MermaidEngine::new();
        *engine.render_diagram_registry_mut() = RenderDiagramRegistry::new();
        engine
            .render_diagram_registry_mut()
            .insert("managed-custom-render", parser);
        let parsed = engine
            .parse_diagram_for_render_model_with_type_sync(
                "managed-custom-render",
                "custom input",
                crate::ParseOptions::strict(),
            )
            .expect("explicit custom render adapter")
            .expect("custom render result");
        let (_, RenderSemanticModel::CustomJson(model)) = parsed.into_parts() else {
            panic!("custom render payload must retain its named model");
        };
        assert_eq!(
            model.provenance(),
            CustomJsonProvenance::RenderRegistryOverlay
        );
        let cloned = model.clone();
        let mut output = Vec::new();
        cloned.json().write_json(&mut output).unwrap();
        assert_eq!(output.len(), DEPTH * 2 + 4);
        assert_eq!(&output[DEPTH..DEPTH + 4], b"null");
        drop(cloned);
        drop(model);
    }
}

#[cfg(all(
    test,
    any(
        feature = "diagram-info",
        feature = "diagram-pie",
        feature = "diagram-journey",
        feature = "diagram-timeline",
        feature = "diagram-packet",
        feature = "diagram-radar"
    )
))]
mod family_output_tests {
    use super::*;

    macro_rules! family_output_test {
        ($name:ident, $diagram_type:literal, $header:literal, $variant:ident,
         $output_data:path, $accessor:ident, $reset:expr, $count:expr) => {
            #[test]
            fn $name() {
                let engine = crate::MermaidEngine::new();
                let reset = $reset;
                let count = $count;
                let mut empty_canonical = None;
                for (source, expected_output) in [
                    ("", <$output_data>::Empty),
                    ($header, <$output_data>::Model),
                ] {
                    reset();
                    let parsed = engine
                        .parse_diagram_for_render_model_with_type_sync(
                            $diagram_type,
                            source,
                            crate::ParseOptions::strict(),
                        )
                        .unwrap()
                        .expect("known-type full construction must retain its output facts");
                    assert_eq!(parsed.render_context().$accessor(), expected_output);
                    assert_eq!(parsed.clone().render_context().$accessor(), expected_output);
                    let RenderSemanticModel::$variant(model) = parsed.model() else {
                        panic!("expected the canonical family payload");
                    };
                    let canonical = serde_json::to_value(model).unwrap();
                    if expected_output == <$output_data>::Empty {
                        empty_canonical = Some(canonical);
                    } else {
                        assert_eq!(empty_canonical.as_ref(), Some(&canonical));
                    }
                    assert_eq!(count(), 1);
                    #[cfg(feature = "compat-json")]
                    {
                        let meta = parsed.metadata();
                        let projected = parsed.compatibility_json().unwrap();
                        let controlled = parsed
                            .compatibility_json_controlled(&OperationControl::new())
                            .unwrap()
                            .unwrap();
                        assert_eq!(projected, controlled);
                        let contextual =
                            BuiltinRenderSemantic::compatibility_json_with_context_controlled(
                                model,
                                meta,
                                parsed.render_context(),
                                &OperationControl::new(),
                            )
                            .unwrap()
                            .unwrap();
                        assert_eq!(contextual, projected);
                        assert_eq!(parsed.clone().compatibility_json().unwrap(), projected);
                        if expected_output == <$output_data>::Empty {
                            assert_eq!(projected, serde_json::json!({}));
                        } else {
                            assert_eq!(projected["type"], $diagram_type);
                        }
                        assert_eq!(
                            BuiltinRenderSemantic::compatibility_json(model, meta).unwrap()["type"],
                            $diagram_type,
                        );
                        assert_eq!(
                            parsed.model().compatibility_json(meta).unwrap()["type"],
                            $diagram_type
                        );
                        let manual = PresentationDiagram::try_from_frozen(
                            meta.clone(),
                            parsed.model().clone(),
                            PresentationContext::None,
                            &crate::config::presentation::source_config_construction_policy(),
                            &OperationControl::new(),
                        )
                        .unwrap()
                        .unwrap();
                        assert_eq!(manual.render_context().$accessor(), <$output_data>::Model);
                        assert_eq!(manual.compatibility_json().unwrap()["type"], $diagram_type);

                        let cancelled = OperationControl::new();
                        cancelled.cancel();
                        let error = parsed
                            .compatibility_json_controlled(&cancelled)
                            .unwrap_err();
                        assert_eq!(error.phase, OperationPhase::Semantic);
                        let cancelled_after_projection = OperationControl::new();
                        cancelled_after_projection.cancel_after_checkpoints(3);
                        let error = parsed
                            .compatibility_json_controlled(&cancelled_after_projection)
                            .unwrap_err();
                        assert_eq!(error.phase, OperationPhase::Semantic);
                    }
                    // All projections and clones consume frozen data without constructing source again.
                    assert_eq!(count(), 1);
                }
            }
        };
    }

    #[cfg(feature = "diagram-info")]
    family_output_test!(
        info_registry_preserves_paired_output_facts,
        "info",
        "info",
        Info,
        merman_model::families::info::InfoOutputData,
        info_output_data,
        || crate::diagrams::langium_common::reset_family_syntax_construction_count("info"),
        || crate::diagrams::langium_common::family_syntax_construction_count("info")
    );

    #[cfg(feature = "diagram-pie")]
    family_output_test!(
        pie_registry_preserves_paired_output_facts,
        "pie",
        "pie",
        Pie,
        merman_model::families::pie::PieOutputData,
        pie_output_data,
        || crate::diagrams::langium_common::reset_family_syntax_construction_count("pie"),
        || crate::diagrams::langium_common::family_syntax_construction_count("pie")
    );

    #[cfg(feature = "diagram-journey")]
    family_output_test!(
        journey_registry_preserves_paired_output_facts,
        "journey",
        "journey",
        Journey,
        merman_model::families::journey::JourneyOutputData,
        journey_output_data,
        crate::diagrams::journey::reset_journey_syntax_construction_count,
        crate::diagrams::journey::journey_syntax_construction_count
    );

    #[cfg(feature = "diagram-timeline")]
    family_output_test!(
        timeline_registry_preserves_paired_output_facts,
        "timeline",
        "timeline",
        Timeline,
        merman_model::families::timeline::TimelineOutputData,
        timeline_output_data,
        crate::diagrams::timeline::reset_timeline_syntax_construction_count,
        crate::diagrams::timeline::timeline_syntax_construction_count
    );

    #[cfg(feature = "diagram-packet")]
    family_output_test!(
        packet_registry_preserves_paired_output_facts,
        "packet",
        "packet",
        Packet,
        merman_model::families::packet::PacketOutputData,
        packet_output_data,
        || crate::diagrams::langium_common::reset_family_syntax_construction_count("packet"),
        || crate::diagrams::langium_common::family_syntax_construction_count("packet")
    );

    #[cfg(feature = "diagram-radar")]
    family_output_test!(
        radar_registry_preserves_paired_output_facts,
        "radar",
        "radar-beta",
        Radar,
        merman_model::families::radar::RadarOutputData,
        radar_output_data,
        || crate::diagrams::langium_common::reset_family_syntax_construction_count("radar"),
        || crate::diagrams::langium_common::family_syntax_construction_count("radar")
    );
}
