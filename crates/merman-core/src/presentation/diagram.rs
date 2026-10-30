#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
use crate::diagram::CapturedPanic;
use crate::diagram::DiagramWarningFact;
use crate::json_value::ManagedJson as ManagedSemanticJson;
#[cfg(all(feature = "compat-json", feature = "editor-facts"))]
use crate::preprocess::SourceConfigEvidence;
#[cfg(feature = "compat-json")]
use crate::{Error, OperationPhase};
use crate::{
    MermaidConfig, OperationControl, OperationControlResult, PresentationMetadata, Result,
};
use serde::{Deserialize, Serialize};
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
    meta: &PresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<Value>>;

/// Parser used by the pinned built-in semantic JSON path.
#[cfg(feature = "compat-json")]
pub(crate) type BuiltInDiagramSemanticParser =
    fn(code: &str, meta: &PresentationMetadata) -> Result<Value>;

/// Parser used by the built-in typed render-model path for one Mermaid diagram family.
pub(crate) type BuiltInRenderSemanticParser =
    fn(
        code: &str,
        meta: &PresentationMetadata,
        control: &OperationControl,
    ) -> OperationControlResult<Result<RenderSemanticParseOutput>>;

/// Parser used by a custom render-model registry overlay.
///
/// Custom adapters intentionally return only a named JSON model. Built-in typed variants are
/// reserved for the pinned family catalog and cannot be manufactured through this interface.
pub type CustomJsonRenderParser = fn(
    code: &str,
    meta: &PresentationMetadata,
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

/// Origin of a custom JSON render model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CustomJsonProvenance {
    /// Produced by a custom semantic JSON parser because no custom render parser was registered.
    #[cfg(feature = "compat-json")]
    SemanticRegistryOverlay,
    /// Produced by an explicitly registered custom render-model parser.
    RenderRegistryOverlay,
}

/// Explicit non-built-in JSON model boundary for custom parser adapters.
#[derive(Debug, Clone, PartialEq)]
pub struct CustomJsonRenderModel {
    model_name: String,
    value: ManagedSemanticJson,
    provenance: CustomJsonProvenance,
}

impl CustomJsonRenderModel {
    /// Creates the result of a custom render-model registry parser.
    pub fn new(model_name: impl Into<String>, value: impl Into<ManagedSemanticJson>) -> Self {
        Self {
            model_name: model_name.into(),
            value: value.into(),
            provenance: CustomJsonProvenance::RenderRegistryOverlay,
        }
    }

    #[cfg(feature = "compat-json")]
    pub(crate) fn from_semantic_registry(
        model_name: impl Into<String>,
        value: impl Into<ManagedSemanticJson>,
    ) -> Self {
        Self {
            model_name: model_name.into(),
            value: value.into(),
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

    /// Borrows the managed payload for iterative cloning and export.
    pub fn json(&self) -> &ManagedSemanticJson {
        &self.value
    }

    /// Consumes the adapter while retaining managed ownership.
    pub fn into_json(self) -> ManagedSemanticJson {
        self.value
    }

    /// Extracts raw JSON; subsequent deep lifecycle operations are the caller's responsibility.
    pub fn into_unmanaged_value(self) -> Value {
        self.value.into_unmanaged_value()
    }

    /// Returns which custom registry path produced the model.
    pub fn provenance(&self) -> CustomJsonProvenance {
        self.provenance
    }
}

/// Typed semantic model used by the headless renderer.
///
/// Retains family-owned typed data for headless rendering without a JSON round trip.
///
/// Family variants and their payload modules are compiled only when the owning `diagram-*`
/// feature is enabled. `Flowchart` is shared by `diagram-flowchart` and `diagram-swimlane`;
/// sharing that payload does not enable the other language. `Error` and `CustomJson` remain
/// available in infrastructure-only builds.
#[derive(Debug, Clone)]
#[allow(
    clippy::large_enum_variant,
    reason = "Family selection changes variant size ratios; keep the public typed payloads unboxed."
)]
pub enum RenderSemanticModel {
    Error(crate::diagrams::error_diagram::ErrorDiagramModel),
    CustomJson(CustomJsonRenderModel),
    #[cfg(feature = "diagram-mindmap")]
    Mindmap(crate::diagrams::mindmap::MindmapDiagramRenderModel),
    #[cfg(feature = "diagram-state")]
    State(crate::diagrams::state::StateDiagramRenderModel),
    #[cfg(feature = "diagram-sequence")]
    Sequence(crate::diagrams::sequence::SequenceDiagramModel),
    #[cfg(feature = "diagram-zenuml")]
    Zenuml(crate::diagrams::zenuml::ZenumlDiagramModel),
    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    Flowchart(crate::diagrams::flowchart::FlowchartModel),
    #[cfg(feature = "diagram-architecture")]
    Architecture(crate::diagrams::architecture::ArchitectureDiagramModel),
    #[cfg(feature = "diagram-class")]
    Class(crate::models::class_diagram::ClassDiagram),
    #[cfg(feature = "diagram-c4")]
    C4(crate::diagrams::c4::C4DiagramModel),
    #[cfg(feature = "diagram-cynefin")]
    Cynefin(crate::diagrams::cynefin::CynefinDiagramModel),
    #[cfg(feature = "diagram-railroad")]
    Railroad(crate::diagrams::railroad::RailroadDiagramModel),
    #[cfg(feature = "diagram-kanban")]
    Kanban(crate::diagrams::kanban::KanbanDiagramRenderModel),
    #[cfg(feature = "diagram-gantt")]
    Gantt(crate::diagrams::gantt::GanttDiagramRenderModel),
    #[cfg(feature = "diagram-pie")]
    Pie(merman_model::families::pie::PieDiagramModel),
    #[cfg(feature = "diagram-packet")]
    Packet(merman_model::families::packet::PacketDiagramModel),
    #[cfg(feature = "diagram-timeline")]
    Timeline(merman_model::families::timeline::TimelineDiagramModel),
    #[cfg(feature = "diagram-journey")]
    Journey(merman_model::families::journey::JourneyDiagramModel),
    #[cfg(feature = "diagram-requirement")]
    Requirement(crate::diagrams::requirement::RequirementDiagramModel),
    #[cfg(feature = "diagram-sankey")]
    Sankey(crate::diagrams::sankey::SankeyDiagramModel),
    #[cfg(feature = "diagram-radar")]
    Radar(merman_model::families::radar::RadarDiagramModel),
    #[cfg(feature = "diagram-info")]
    Info(merman_model::families::info::InfoDiagramModel),
    #[cfg(feature = "diagram-treemap")]
    Treemap(crate::diagrams::treemap::TreemapDiagramRenderModel),
    #[cfg(feature = "diagram-block")]
    Block(crate::diagrams::block::BlockDiagramRenderModel),
    #[cfg(feature = "diagram-er")]
    Er(crate::diagrams::er::ErDiagramModel),
    #[cfg(feature = "diagram-quadrant-chart")]
    QuadrantChart(crate::diagrams::quadrant_chart::QuadrantChartDiagramModel),
    #[cfg(feature = "diagram-xychart")]
    XyChart(crate::diagrams::xychart::XyChartDiagramRenderModel),
    #[cfg(feature = "diagram-git-graph")]
    GitGraph(crate::diagrams::git_graph::GitGraphDiagramModel),
    #[cfg(feature = "diagram-tree-view")]
    TreeView(crate::diagrams::tree_view::TreeViewDiagramModel),
    #[cfg(feature = "diagram-ishikawa")]
    Ishikawa(crate::diagrams::ishikawa::IshikawaDiagramModel),
    #[cfg(feature = "diagram-event-modeling")]
    EventModeling(crate::diagrams::eventmodeling::EventModelingDiagramModel),
    #[cfg(feature = "diagram-venn")]
    Venn(crate::diagrams::venn::VennDiagramModel),
    #[cfg(feature = "diagram-wardley")]
    Wardley(crate::diagrams::wardley::WardleyDiagramModel),
    #[cfg(feature = "diagram-usecase")]
    Usecase(crate::diagrams::usecase::UsecaseDiagramModel),
    #[cfg(feature = "diagram-agentflow")]
    Agentflow(crate::diagrams::agentflow::AgentflowDiagramRenderModel),
}

/// Frozen full-output facts paired with a typed semantic model.
///
/// This context is intentionally separate from [`RenderSemanticModel`] so family models retain
/// their public construction and serialization contracts. Its default declares model output;
/// full source producers explicitly supply any different output shape.
#[doc(hidden)]
#[derive(Debug, Clone, Default)]
pub struct RenderSemanticContext {
    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    flowchart: Option<crate::diagrams::flowchart::FlowchartRenderContext>,
    #[cfg(feature = "diagram-class")]
    class_style_precedence_facts: Option<crate::models::class_diagram::ClassStylePrecedenceFacts>,
    #[cfg(feature = "diagram-info")]
    info_output_data: merman_model::families::info::InfoOutputData,
    #[cfg(feature = "diagram-pie")]
    pie_output_data: merman_model::families::pie::PieOutputData,
    #[cfg(feature = "diagram-journey")]
    journey_output_data: merman_model::families::journey::JourneyOutputData,
    #[cfg(feature = "diagram-timeline")]
    timeline_output_data: merman_model::families::timeline::TimelineOutputData,
    #[cfg(feature = "diagram-packet")]
    packet_output_data: merman_model::families::packet::PacketOutputData,
    #[cfg(feature = "diagram-radar")]
    radar_output_data: merman_model::families::radar::RadarOutputData,
}

impl RenderSemanticContext {
    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    fn for_flowchart(context: crate::diagrams::flowchart::FlowchartRenderContext) -> Self {
        Self {
            flowchart: Some(context),
            ..Self::default()
        }
    }

    #[cfg(feature = "diagram-class")]
    fn for_class(
        style_precedence_facts: crate::models::class_diagram::ClassStylePrecedenceFacts,
    ) -> Self {
        Self {
            class_style_precedence_facts: Some(style_precedence_facts),
            ..Self::default()
        }
    }

    /// Borrows Flowchart's complete parser-owned render context.
    #[doc(hidden)]
    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    pub fn flowchart_render_context(
        &self,
    ) -> Option<&crate::diagrams::flowchart::FlowchartRenderContext> {
        self.flowchart.as_ref()
    }

    /// Borrows ClassDiagram encounter-order style evidence owned by this parse operation.
    #[doc(hidden)]
    #[cfg(feature = "diagram-class")]
    pub fn class_style_precedence_facts(
        &self,
    ) -> Option<&crate::models::class_diagram::ClassStylePrecedenceFacts> {
        self.class_style_precedence_facts.as_ref()
    }

    /// Returns the declared Info output shape frozen during full construction.
    #[cfg(feature = "diagram-info")]
    pub fn info_output_data(&self) -> merman_model::families::info::InfoOutputData {
        self.info_output_data
    }

    /// Returns the declared Pie output shape frozen during full construction.
    #[cfg(feature = "diagram-pie")]
    pub fn pie_output_data(&self) -> merman_model::families::pie::PieOutputData {
        self.pie_output_data
    }

    /// Returns the declared Journey output shape frozen during full construction.
    #[cfg(feature = "diagram-journey")]
    pub fn journey_output_data(&self) -> merman_model::families::journey::JourneyOutputData {
        self.journey_output_data
    }

    /// Returns the declared Timeline output shape frozen during full construction.
    #[cfg(feature = "diagram-timeline")]
    pub fn timeline_output_data(&self) -> merman_model::families::timeline::TimelineOutputData {
        self.timeline_output_data
    }

    /// Returns the declared Packet output shape frozen during full construction.
    #[cfg(feature = "diagram-packet")]
    pub fn packet_output_data(&self) -> merman_model::families::packet::PacketOutputData {
        self.packet_output_data
    }

    /// Returns the declared Radar output shape frozen during full construction.
    #[cfg(feature = "diagram-radar")]
    pub fn radar_output_data(&self) -> merman_model::families::radar::RadarOutputData {
        self.radar_output_data
    }

    pub(crate) fn retained_text_bytes(&self) -> usize {
        let bytes = 0usize;
        #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
        let bytes = bytes.saturating_add(
            self.flowchart
                .as_ref()
                .map_or(0, |context| context.retained_bytes()),
        );
        #[cfg(feature = "diagram-class")]
        let bytes = bytes.saturating_add(
            self.class_style_precedence_facts
                .as_ref()
                .map_or(0, |facts| facts.retained_bytes()),
        );
        bytes
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

    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    pub(crate) fn flowchart(
        model: crate::diagrams::flowchart::FlowchartModel,
        context: crate::diagrams::flowchart::FlowchartRenderContext,
    ) -> Self {
        Self {
            model: RenderSemanticModel::Flowchart(model),
            context: RenderSemanticContext::for_flowchart(context),
        }
    }

    #[cfg(feature = "diagram-class")]
    pub(crate) fn class(
        model: crate::models::class_diagram::ClassDiagram,
        style_precedence_facts: crate::models::class_diagram::ClassStylePrecedenceFacts,
    ) -> Self {
        Self {
            model: RenderSemanticModel::Class(model),
            context: RenderSemanticContext::for_class(style_precedence_facts),
        }
    }

    #[cfg(feature = "diagram-info")]
    pub(crate) fn info(
        model: merman_model::families::info::InfoDiagramModel,
        output_data: merman_model::families::info::InfoOutputData,
    ) -> Self {
        Self {
            model: RenderSemanticModel::Info(model),
            context: RenderSemanticContext {
                info_output_data: output_data,
                ..RenderSemanticContext::default()
            },
        }
    }

    #[cfg(feature = "diagram-pie")]
    pub(crate) fn pie(
        model: merman_model::families::pie::PieDiagramModel,
        output_data: merman_model::families::pie::PieOutputData,
    ) -> Self {
        Self {
            model: RenderSemanticModel::Pie(model),
            context: RenderSemanticContext {
                pie_output_data: output_data,
                ..RenderSemanticContext::default()
            },
        }
    }

    #[cfg(feature = "diagram-journey")]
    pub(crate) fn journey(
        model: merman_model::families::journey::JourneyDiagramModel,
        output_data: merman_model::families::journey::JourneyOutputData,
    ) -> Self {
        Self {
            model: RenderSemanticModel::Journey(model),
            context: RenderSemanticContext {
                journey_output_data: output_data,
                ..RenderSemanticContext::default()
            },
        }
    }

    #[cfg(feature = "diagram-timeline")]
    pub(crate) fn timeline(
        model: merman_model::families::timeline::TimelineDiagramModel,
        output_data: merman_model::families::timeline::TimelineOutputData,
    ) -> Self {
        Self {
            model: RenderSemanticModel::Timeline(model),
            context: RenderSemanticContext {
                timeline_output_data: output_data,
                ..RenderSemanticContext::default()
            },
        }
    }

    #[cfg(feature = "diagram-packet")]
    pub(crate) fn packet(
        model: merman_model::families::packet::PacketDiagramModel,
        output_data: merman_model::families::packet::PacketOutputData,
    ) -> Self {
        Self {
            model: RenderSemanticModel::Packet(model),
            context: RenderSemanticContext {
                packet_output_data: output_data,
                ..RenderSemanticContext::default()
            },
        }
    }

    #[cfg(feature = "diagram-radar")]
    pub(crate) fn radar(
        model: merman_model::families::radar::RadarDiagramModel,
        output_data: merman_model::families::radar::RadarOutputData,
    ) -> Self {
        Self {
            model: RenderSemanticModel::Radar(model),
            context: RenderSemanticContext {
                radar_output_data: output_data,
                ..RenderSemanticContext::default()
            },
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

#[cfg(feature = "compat-json")]
mod compatibility {
    use super::*;

    mod builtin_render_semantic_private {
        pub trait Sealed {}
    }

    /// Family-owned typed semantic data that can project the public compatibility JSON contract.
    ///
    /// This trait is sealed because built-in family membership is defined by the pinned Mermaid
    /// catalog. Consumers can use it generically but cannot manufacture a new built-in family.
    /// Ordinary projections use declared model defaults for separately paired output facts. The
    /// context-aware method retains those frozen facts when projecting a family-prepared model.
    pub trait BuiltinRenderSemantic: builtin_render_semantic_private::Sealed {
        fn compatibility_json(&self, meta: &PresentationMetadata) -> Result<ManagedSemanticJson>;

        fn compatibility_json_controlled(
            &self,
            meta: &PresentationMetadata,
            control: &OperationControl,
        ) -> OperationControlResult<Result<ManagedSemanticJson>> {
            control.checkpoint()?;
            let projected = self.compatibility_json(meta);
            control.checkpoint()?;
            Ok(projected)
        }

        /// Projects this model with explicitly paired frozen full-output facts.
        ///
        /// The context declares output data; it does not authenticate source admission. Families
        /// with no separately paired facts retain their ordinary controlled projection.
        fn compatibility_json_with_context_controlled(
            &self,
            meta: &PresentationMetadata,
            _context: &RenderSemanticContext,
            control: &OperationControl,
        ) -> OperationControlResult<Result<ManagedSemanticJson>> {
            self.compatibility_json_controlled(meta, control)
        }
    }

    #[cfg(any(
        feature = "diagram-info",
        feature = "diagram-pie",
        feature = "diagram-journey",
        feature = "diagram-timeline",
        feature = "diagram-packet",
        feature = "diagram-radar"
    ))]
    fn project_controlled(
        control: &OperationControl,
        project: impl FnOnce() -> Result<Value>,
    ) -> OperationControlResult<Result<ManagedSemanticJson>> {
        control.checkpoint()?;
        let projected = project().map(ManagedSemanticJson::from);
        control.checkpoint()?;
        Ok(projected)
    }

    macro_rules! impl_builtin_render_semantic {
        ($model:path, $project:path) => {
            impl builtin_render_semantic_private::Sealed for $model {}

            impl BuiltinRenderSemantic for $model {
                fn compatibility_json(
                    &self,
                    meta: &PresentationMetadata,
                ) -> Result<ManagedSemanticJson> {
                    $project(self, meta).map(ManagedSemanticJson::from)
                }
            }
        };
        ($model:path, $project:path, $output_data:path, $context_getter:ident) => {
            impl builtin_render_semantic_private::Sealed for $model {}

            impl BuiltinRenderSemantic for $model {
                fn compatibility_json(
                    &self,
                    meta: &PresentationMetadata,
                ) -> Result<ManagedSemanticJson> {
                    $project(self, <$output_data>::default(), meta).map(ManagedSemanticJson::from)
                }

                fn compatibility_json_with_context_controlled(
                    &self,
                    meta: &PresentationMetadata,
                    context: &RenderSemanticContext,
                    control: &OperationControl,
                ) -> OperationControlResult<Result<ManagedSemanticJson>> {
                    project_controlled(control, || $project(self, context.$context_getter(), meta))
                }
            }
        };
    }

    #[cfg(any(
        feature = "diagram-mindmap",
        feature = "diagram-architecture",
        feature = "diagram-gantt",
        feature = "diagram-block",
        feature = "diagram-state",
        feature = "diagram-treemap",
        feature = "diagram-ishikawa",
        feature = "diagram-zenuml",
        feature = "diagram-wardley"
    ))]
    macro_rules! impl_builtin_render_semantic_controlled {
        ($model:path, $project:path, $controlled_project:path) => {
            impl builtin_render_semantic_private::Sealed for $model {}

            impl BuiltinRenderSemantic for $model {
                fn compatibility_json(
                    &self,
                    meta: &PresentationMetadata,
                ) -> Result<ManagedSemanticJson> {
                    $project(self, meta).map(ManagedSemanticJson::from)
                }

                fn compatibility_json_controlled(
                    &self,
                    meta: &PresentationMetadata,
                    control: &OperationControl,
                ) -> OperationControlResult<Result<ManagedSemanticJson>> {
                    $controlled_project(self, meta, control)
                        .map(|result| result.map(ManagedSemanticJson::from))
                }
            }
        };
    }

    impl_builtin_render_semantic!(
        crate::diagrams::error_diagram::ErrorDiagramModel,
        crate::diagrams::error_diagram::render_model_to_compat_json
    );
    #[cfg(feature = "diagram-mindmap")]
    impl_builtin_render_semantic_controlled!(
        crate::diagrams::mindmap::MindmapDiagramRenderModel,
        crate::diagrams::mindmap::render_model_to_compat_json,
        crate::diagrams::mindmap::render_model_to_compat_json_controlled
    );
    #[cfg(feature = "diagram-state")]
    impl_builtin_render_semantic_controlled!(
        crate::diagrams::state::StateDiagramRenderModel,
        crate::diagrams::state::render_model_to_compat_json,
        crate::diagrams::state::render_model_to_compat_json_controlled
    );
    #[cfg(feature = "diagram-sequence")]
    impl_builtin_render_semantic!(
        crate::diagrams::sequence::SequenceDiagramModel,
        crate::diagrams::sequence::render_model_to_compat_json
    );
    #[cfg(feature = "diagram-zenuml")]
    impl_builtin_render_semantic_controlled!(
        crate::diagrams::zenuml::ZenumlDiagramModel,
        crate::diagrams::zenuml::render_model_to_compat_json,
        crate::diagrams::zenuml::render_model_to_compat_json_controlled
    );
    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    impl_builtin_render_semantic!(
        crate::diagrams::flowchart::FlowchartModel,
        crate::diagrams::flowchart::render_model_to_compat_json
    );
    #[cfg(feature = "diagram-architecture")]
    impl_builtin_render_semantic_controlled!(
        crate::diagrams::architecture::ArchitectureDiagramModel,
        crate::diagrams::architecture::render_model_to_compat_json,
        crate::diagrams::architecture::render_model_to_compat_json_controlled
    );
    #[cfg(feature = "diagram-class")]
    impl_builtin_render_semantic!(
        crate::models::class_diagram::ClassDiagram,
        crate::diagrams::class::render_model_to_compat_json
    );
    #[cfg(feature = "diagram-c4")]
    impl_builtin_render_semantic!(
        crate::diagrams::c4::C4DiagramModel,
        crate::diagrams::c4::render_model_to_compat_json
    );
    #[cfg(feature = "diagram-cynefin")]
    impl_builtin_render_semantic!(
        crate::diagrams::cynefin::CynefinDiagramModel,
        crate::diagrams::cynefin::render_model_to_compat_json
    );
    #[cfg(feature = "diagram-railroad")]
    impl_builtin_render_semantic!(
        crate::diagrams::railroad::RailroadDiagramModel,
        crate::diagrams::railroad::render_model_to_compat_json
    );
    #[cfg(feature = "diagram-kanban")]
    impl_builtin_render_semantic!(
        crate::diagrams::kanban::KanbanDiagramRenderModel,
        crate::diagrams::kanban::render_model_to_compat_json
    );
    #[cfg(feature = "diagram-gantt")]
    impl_builtin_render_semantic_controlled!(
        crate::diagrams::gantt::GanttDiagramRenderModel,
        crate::diagrams::gantt::render_model_to_compat_json,
        crate::diagrams::gantt::render_model_to_compat_json_controlled
    );
    #[cfg(feature = "diagram-pie")]
    impl_builtin_render_semantic!(
        merman_model::families::pie::PieDiagramModel,
        crate::diagrams::pie::render_model_to_compat_json,
        merman_model::families::pie::PieOutputData,
        pie_output_data
    );
    #[cfg(feature = "diagram-packet")]
    impl_builtin_render_semantic!(
        merman_model::families::packet::PacketDiagramModel,
        crate::diagrams::packet::render_model_to_compat_json,
        merman_model::families::packet::PacketOutputData,
        packet_output_data
    );
    #[cfg(feature = "diagram-timeline")]
    impl_builtin_render_semantic!(
        merman_model::families::timeline::TimelineDiagramModel,
        crate::diagrams::timeline::render_model_to_compat_json,
        merman_model::families::timeline::TimelineOutputData,
        timeline_output_data
    );
    #[cfg(feature = "diagram-journey")]
    impl_builtin_render_semantic!(
        merman_model::families::journey::JourneyDiagramModel,
        crate::diagrams::journey::render_model_to_compat_json,
        merman_model::families::journey::JourneyOutputData,
        journey_output_data
    );
    #[cfg(feature = "diagram-requirement")]
    impl_builtin_render_semantic!(
        crate::diagrams::requirement::RequirementDiagramModel,
        crate::diagrams::requirement::render_model_to_compat_json
    );
    #[cfg(feature = "diagram-sankey")]
    impl_builtin_render_semantic!(
        crate::diagrams::sankey::SankeyDiagramModel,
        crate::diagrams::sankey::render_model_to_compat_json
    );
    #[cfg(feature = "diagram-radar")]
    impl_builtin_render_semantic!(
        merman_model::families::radar::RadarDiagramModel,
        crate::diagrams::radar::render_model_to_compat_json,
        merman_model::families::radar::RadarOutputData,
        radar_output_data
    );
    #[cfg(feature = "diagram-info")]
    impl_builtin_render_semantic!(
        merman_model::families::info::InfoDiagramModel,
        crate::diagrams::info::render_model_to_compat_json,
        merman_model::families::info::InfoOutputData,
        info_output_data
    );
    #[cfg(feature = "diagram-treemap")]
    impl_builtin_render_semantic_controlled!(
        crate::diagrams::treemap::TreemapDiagramRenderModel,
        crate::diagrams::treemap::render_model_to_compat_json,
        crate::diagrams::treemap::render_model_to_compat_json_controlled
    );
    #[cfg(feature = "diagram-block")]
    impl_builtin_render_semantic_controlled!(
        crate::diagrams::block::BlockDiagramRenderModel,
        crate::diagrams::block::render_model_to_compat_json,
        crate::diagrams::block::render_model_to_compat_json_controlled
    );
    #[cfg(feature = "diagram-er")]
    impl_builtin_render_semantic!(
        crate::diagrams::er::ErDiagramModel,
        crate::diagrams::er::render_model_to_compat_json
    );
    #[cfg(feature = "diagram-quadrant-chart")]
    impl_builtin_render_semantic!(
        crate::diagrams::quadrant_chart::QuadrantChartDiagramModel,
        crate::diagrams::quadrant_chart::render_model_to_compat_json
    );
    #[cfg(feature = "diagram-xychart")]
    impl_builtin_render_semantic!(
        crate::diagrams::xychart::XyChartDiagramRenderModel,
        crate::diagrams::xychart::render_model_to_compat_json
    );
    #[cfg(feature = "diagram-git-graph")]
    impl_builtin_render_semantic!(
        crate::diagrams::git_graph::GitGraphDiagramModel,
        crate::diagrams::git_graph::render_model_to_compat_json
    );
    #[cfg(feature = "diagram-tree-view")]
    impl_builtin_render_semantic!(
        crate::diagrams::tree_view::TreeViewDiagramModel,
        crate::diagrams::tree_view::render_model_to_compat_json
    );
    #[cfg(feature = "diagram-ishikawa")]
    impl_builtin_render_semantic_controlled!(
        crate::diagrams::ishikawa::IshikawaDiagramModel,
        crate::diagrams::ishikawa::render_model_to_compat_json,
        crate::diagrams::ishikawa::render_model_to_compat_json_controlled
    );
    #[cfg(feature = "diagram-event-modeling")]
    impl_builtin_render_semantic!(
        crate::diagrams::eventmodeling::EventModelingDiagramModel,
        crate::diagrams::eventmodeling::render_model_to_compat_json
    );
    #[cfg(feature = "diagram-venn")]
    impl_builtin_render_semantic!(
        crate::diagrams::venn::VennDiagramModel,
        crate::diagrams::venn::render_model_to_compat_json
    );
    #[cfg(feature = "diagram-wardley")]
    impl_builtin_render_semantic_controlled!(
        crate::diagrams::wardley::WardleyDiagramModel,
        crate::diagrams::wardley::render_model_to_compat_json,
        crate::diagrams::wardley::render_model_to_compat_json_controlled
    );
    #[cfg(feature = "diagram-usecase")]
    impl_builtin_render_semantic!(
        crate::diagrams::usecase::UsecaseDiagramModel,
        crate::diagrams::usecase::render_model_to_compat_json
    );
    #[cfg(feature = "diagram-agentflow")]
    impl_builtin_render_semantic!(
        crate::diagrams::agentflow::AgentflowDiagramRenderModel,
        crate::diagrams::agentflow::render_model_to_compat_json
    );
}

#[cfg(feature = "compat-json")]
pub use compatibility::BuiltinRenderSemantic;

impl RenderSemanticModel {
    /// Applies Mermaid common DB sanitization to family-owned typed fields.
    pub(crate) fn sanitize_common_db_fields(&mut self, config: &MermaidConfig) {
        match self {
            Self::Error(_) => {}
            Self::CustomJson(v) => {
                crate::common_db::apply_common_db_sanitization(v.value.as_value_mut(), config);
            }
            #[cfg(feature = "diagram-mindmap")]
            Self::Mindmap(_) => {}
            #[cfg(feature = "diagram-state")]
            Self::State(v) => crate::diagrams::state::sanitize_render_common_db_fields(v, config),
            #[cfg(feature = "diagram-sequence")]
            Self::Sequence(v) => crate::diagrams::sequence::sanitize_common_db_fields(v, config),
            #[cfg(feature = "diagram-zenuml")]
            Self::Zenuml(_) => {}
            #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
            Self::Flowchart(v) => crate::diagrams::flowchart::sanitize_common_db_fields(v, config),
            #[cfg(feature = "diagram-architecture")]
            Self::Architecture(v) => {
                crate::diagrams::architecture::sanitize_common_db_fields(v, config)
            }
            #[cfg(feature = "diagram-class")]
            Self::Class(v) => {
                crate::models::class_diagram::sanitize_full_common_db_fields(v, config)
            }
            #[cfg(feature = "diagram-c4")]
            Self::C4(v) => crate::diagrams::c4::sanitize_common_db_fields(v, config),
            #[cfg(feature = "diagram-cynefin")]
            Self::Cynefin(v) => crate::diagrams::cynefin::sanitize_common_db_fields(v, config),
            #[cfg(feature = "diagram-railroad")]
            Self::Railroad(v) => crate::diagrams::railroad::sanitize_common_db_fields(v, config),
            #[cfg(feature = "diagram-kanban")]
            Self::Kanban(_) => {}
            #[cfg(feature = "diagram-gantt")]
            Self::Gantt(v) => {
                crate::diagrams::gantt::sanitize_common_db_fields(&mut v.model, config)
            }
            #[cfg(feature = "diagram-pie")]
            Self::Pie(v) => crate::diagrams::pie::sanitize_common_db_fields(v, config),
            #[cfg(feature = "diagram-packet")]
            Self::Packet(v) => crate::diagrams::packet::sanitize_common_db_fields(v, config),
            #[cfg(feature = "diagram-timeline")]
            Self::Timeline(v) => crate::diagrams::timeline::sanitize_common_db_fields(v, config),
            #[cfg(feature = "diagram-journey")]
            Self::Journey(v) => crate::diagrams::journey::sanitize_common_db_fields(v, config),
            #[cfg(feature = "diagram-requirement")]
            Self::Requirement(v) => {
                crate::diagrams::requirement::sanitize_common_db_fields(v, config)
            }
            #[cfg(feature = "diagram-sankey")]
            Self::Sankey(_) => {}
            #[cfg(feature = "diagram-radar")]
            Self::Radar(v) => crate::diagrams::radar::sanitize_common_db_fields(v, config),
            #[cfg(feature = "diagram-info")]
            Self::Info(_) => {}
            #[cfg(feature = "diagram-treemap")]
            Self::Treemap(v) => {
                crate::diagrams::treemap::sanitize_common_db_fields(&mut v.semantic, config)
            }
            #[cfg(feature = "diagram-block")]
            Self::Block(_) => {}
            #[cfg(feature = "diagram-er")]
            Self::Er(v) => crate::diagrams::er::sanitize_common_db_fields(v, config),
            #[cfg(feature = "diagram-quadrant-chart")]
            Self::QuadrantChart(v) => {
                crate::diagrams::quadrant_chart::sanitize_common_db_fields(v, config)
            }
            #[cfg(feature = "diagram-xychart")]
            Self::XyChart(v) => {
                crate::diagrams::xychart::sanitize_common_db_fields(&mut v.semantic, config)
            }
            #[cfg(feature = "diagram-git-graph")]
            Self::GitGraph(v) => crate::diagrams::git_graph::sanitize_common_db_fields(v, config),
            #[cfg(feature = "diagram-tree-view")]
            Self::TreeView(v) => crate::diagrams::tree_view::sanitize_common_db_fields(v, config),
            #[cfg(feature = "diagram-ishikawa")]
            Self::Ishikawa(v) => crate::diagrams::ishikawa::sanitize_common_db_fields(v, config),
            #[cfg(feature = "diagram-event-modeling")]
            Self::EventModeling(v) => {
                crate::diagrams::eventmodeling::sanitize_common_db_fields(v, config)
            }
            #[cfg(feature = "diagram-venn")]
            Self::Venn(v) => crate::diagrams::venn::sanitize_common_db_fields(v, config),
            #[cfg(feature = "diagram-wardley")]
            Self::Wardley(v) => crate::diagrams::wardley::sanitize_common_db_fields(v, config),
            #[cfg(feature = "diagram-usecase")]
            Self::Usecase(v) => crate::diagrams::usecase::sanitize_common_db_fields(v, config),
            #[cfg(feature = "diagram-agentflow")]
            Self::Agentflow(v) => {
                crate::diagrams::agentflow::sanitize_common_db_fields(&mut v.model, config)
            }
        }
    }

    pub(crate) fn offset_parser_diagnostic_positions(
        &mut self,
        line_offset: usize,
        column_offset: usize,
    ) {
        #[cfg(not(feature = "diagram-agentflow"))]
        let _ = (line_offset, column_offset);
        #[cfg(feature = "diagram-agentflow")]
        if let Self::Agentflow(model) = self {
            crate::diagrams::agentflow::offset_diagnostic_positions(
                &mut model.model,
                line_offset,
                column_offset,
            );
        }
    }

    pub(crate) fn remap_warning_fact_spans(
        &mut self,
        mut remap: impl FnMut(&mut DiagramWarningFact),
    ) {
        match self {
            Self::CustomJson(v) => {
                Self::remap_json_warning_fact_spans(v.value.as_value_mut(), &mut remap);
            }
            #[cfg(feature = "diagram-agentflow")]
            Self::Agentflow(v) => Self::remap_warning_fact_slice(&mut v.warning_facts, &mut remap),
            #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
            Self::Flowchart(v) => Self::remap_warning_fact_slice(&mut v.warning_facts, &mut remap),
            #[cfg(feature = "diagram-block")]
            Self::Block(v) => Self::remap_warning_fact_slice(&mut v.warning_facts, &mut remap),
            #[cfg(feature = "diagram-git-graph")]
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
        let Ok(mut warning_facts) = Vec::<DiagramWarningFact>::deserialize(&*warning_facts_value)
        else {
            return;
        };

        Self::remap_warning_fact_slice(&mut warning_facts, remap);
        crate::config::replace_value_nonrecursive(
            warning_facts_value,
            serde_json::json!(warning_facts),
        );
    }

    /// Returns a stable family label for diagnostics and timing output.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Error(_) => "error",
            Self::CustomJson(_) => "custom-json",
            #[cfg(feature = "diagram-mindmap")]
            Self::Mindmap(_) => "mindmap",
            #[cfg(feature = "diagram-state")]
            Self::State(_) => "state",
            #[cfg(feature = "diagram-sequence")]
            Self::Sequence(_) => "sequence",
            #[cfg(feature = "diagram-zenuml")]
            Self::Zenuml(_) => "zenuml",
            #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
            Self::Flowchart(_) => "flowchart",
            #[cfg(feature = "diagram-architecture")]
            Self::Architecture(_) => "architecture",
            #[cfg(feature = "diagram-class")]
            Self::Class(_) => "class",
            #[cfg(feature = "diagram-c4")]
            Self::C4(_) => "c4",
            #[cfg(feature = "diagram-cynefin")]
            Self::Cynefin(_) => "cynefin",
            #[cfg(feature = "diagram-railroad")]
            Self::Railroad(_) => "railroad",
            #[cfg(feature = "diagram-kanban")]
            Self::Kanban(_) => "kanban",
            #[cfg(feature = "diagram-gantt")]
            Self::Gantt(_) => "gantt",
            #[cfg(feature = "diagram-pie")]
            Self::Pie(_) => "pie",
            #[cfg(feature = "diagram-packet")]
            Self::Packet(_) => "packet",
            #[cfg(feature = "diagram-timeline")]
            Self::Timeline(_) => "timeline",
            #[cfg(feature = "diagram-journey")]
            Self::Journey(_) => "journey",
            #[cfg(feature = "diagram-requirement")]
            Self::Requirement(_) => "requirement",
            #[cfg(feature = "diagram-sankey")]
            Self::Sankey(_) => "sankey",
            #[cfg(feature = "diagram-radar")]
            Self::Radar(_) => "radar",
            #[cfg(feature = "diagram-info")]
            Self::Info(_) => "info",
            #[cfg(feature = "diagram-treemap")]
            Self::Treemap(_) => "treemap",
            #[cfg(feature = "diagram-block")]
            Self::Block(_) => "block",
            #[cfg(feature = "diagram-er")]
            Self::Er(_) => "er",
            #[cfg(feature = "diagram-quadrant-chart")]
            Self::QuadrantChart(_) => "quadrantChart",
            #[cfg(feature = "diagram-xychart")]
            Self::XyChart(_) => "xychart",
            #[cfg(feature = "diagram-git-graph")]
            Self::GitGraph(_) => "gitGraph",
            #[cfg(feature = "diagram-tree-view")]
            Self::TreeView(_) => "treeView",
            #[cfg(feature = "diagram-ishikawa")]
            Self::Ishikawa(_) => "ishikawa",
            #[cfg(feature = "diagram-event-modeling")]
            Self::EventModeling(_) => "eventmodeling",
            #[cfg(feature = "diagram-venn")]
            Self::Venn(_) => "venn",
            #[cfg(feature = "diagram-wardley")]
            Self::Wardley(_) => "wardley",
            #[cfg(feature = "diagram-usecase")]
            Self::Usecase(_) => "usecase",
            #[cfg(feature = "diagram-agentflow")]
            Self::Agentflow(_) => "agentflow",
        }
    }

    /// Projects this family-owned typed model into the public compatibility JSON shape.
    ///
    /// Built-in families delegate to their own lossless projector. This never reparses source;
    /// custom adapters retain their explicitly named JSON boundary. Output facts held outside this
    /// value default to their declared model shape. Use `ParsedDiagramRender::compatibility_json`
    /// to retain the output shape frozen by source construction.
    #[cfg(feature = "compat-json")]
    pub fn compatibility_json(&self, meta: &PresentationMetadata) -> Result<ManagedSemanticJson> {
        let control = OperationControl::new();
        self.compatibility_json_controlled(meta, &control)
            .expect("a private operation control cannot be cancelled")
    }

    /// Projects the public compatibility JSON while observing the caller's operation control.
    ///
    /// Output facts held in a parsed context use their manual defaults here. Parsed consumers use
    /// `ParsedDiagramRender::compatibility_json_controlled` to retain the paired output shape.
    #[cfg(feature = "compat-json")]
    pub fn compatibility_json_controlled(
        &self,
        meta: &PresentationMetadata,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ManagedSemanticJson>> {
        self.compatibility_json_with_context_controlled(
            meta,
            &RenderSemanticContext::default(),
            control,
        )
    }

    #[cfg(feature = "compat-json")]
    fn compatibility_json_with_context_controlled(
        &self,
        meta: &PresentationMetadata,
        context: &RenderSemanticContext,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ManagedSemanticJson>> {
        let _ = context;
        let control = control.for_phase(OperationPhase::Semantic);
        control.checkpoint()?;
        let projected = match self {
            Self::Error(model) => model.compatibility_json_controlled(meta, &control),
            Self::CustomJson(model) => model.json().clone_controlled(&control).map(Ok),
            #[cfg(feature = "diagram-mindmap")]
            Self::Mindmap(model) => model.compatibility_json_controlled(meta, &control),
            #[cfg(feature = "diagram-state")]
            Self::State(model) => model.compatibility_json_controlled(meta, &control),
            #[cfg(feature = "diagram-sequence")]
            Self::Sequence(model) => model.compatibility_json_controlled(meta, &control),
            #[cfg(feature = "diagram-zenuml")]
            Self::Zenuml(model) => model.compatibility_json_controlled(meta, &control),
            #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
            Self::Flowchart(model) => model.compatibility_json_controlled(meta, &control),
            #[cfg(feature = "diagram-architecture")]
            Self::Architecture(model) => model.compatibility_json_controlled(meta, &control),
            #[cfg(feature = "diagram-class")]
            Self::Class(model) => model.compatibility_json_controlled(meta, &control),
            #[cfg(feature = "diagram-c4")]
            Self::C4(model) => model.compatibility_json_controlled(meta, &control),
            #[cfg(feature = "diagram-cynefin")]
            Self::Cynefin(model) => model.compatibility_json_controlled(meta, &control),
            #[cfg(feature = "diagram-railroad")]
            Self::Railroad(model) => model.compatibility_json_controlled(meta, &control),
            #[cfg(feature = "diagram-kanban")]
            Self::Kanban(model) => model.compatibility_json_controlled(meta, &control),
            #[cfg(feature = "diagram-gantt")]
            Self::Gantt(model) => model.compatibility_json_controlled(meta, &control),
            #[cfg(feature = "diagram-pie")]
            Self::Pie(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-packet")]
            Self::Packet(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-timeline")]
            Self::Timeline(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-journey")]
            Self::Journey(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-requirement")]
            Self::Requirement(model) => model.compatibility_json_controlled(meta, &control),
            #[cfg(feature = "diagram-sankey")]
            Self::Sankey(model) => model.compatibility_json_controlled(meta, &control),
            #[cfg(feature = "diagram-radar")]
            Self::Radar(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-info")]
            Self::Info(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-treemap")]
            Self::Treemap(model) => model.compatibility_json_controlled(meta, &control),
            #[cfg(feature = "diagram-block")]
            Self::Block(model) => model.compatibility_json_controlled(meta, &control),
            #[cfg(feature = "diagram-er")]
            Self::Er(model) => model.compatibility_json_controlled(meta, &control),
            #[cfg(feature = "diagram-quadrant-chart")]
            Self::QuadrantChart(model) => model.compatibility_json_controlled(meta, &control),
            #[cfg(feature = "diagram-xychart")]
            Self::XyChart(model) => model.compatibility_json_controlled(meta, &control),
            #[cfg(feature = "diagram-git-graph")]
            Self::GitGraph(model) => model.compatibility_json_controlled(meta, &control),
            #[cfg(feature = "diagram-tree-view")]
            Self::TreeView(model) => model.compatibility_json_controlled(meta, &control),
            #[cfg(feature = "diagram-ishikawa")]
            Self::Ishikawa(model) => model.compatibility_json_controlled(meta, &control),
            #[cfg(feature = "diagram-event-modeling")]
            Self::EventModeling(model) => model.compatibility_json_controlled(meta, &control),
            #[cfg(feature = "diagram-venn")]
            Self::Venn(model) => model.compatibility_json_controlled(meta, &control),
            #[cfg(feature = "diagram-wardley")]
            Self::Wardley(model) => model.compatibility_json_controlled(meta, &control),
            #[cfg(feature = "diagram-usecase")]
            Self::Usecase(model) => model.compatibility_json_controlled(meta, &control),
            #[cfg(feature = "diagram-agentflow")]
            Self::Agentflow(model) => model.compatibility_json_controlled(meta, &control),
        }?;
        control.checkpoint()?;
        Ok(projected)
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

/// Parsed diagram metadata plus its canonically paired typed render model.
///
/// Construction is restricted to `merman-core`; its parse pipeline pairs the metadata and model.
/// External consumers may inspect the pair or consume it, but cannot assemble metadata and a
/// model produced by different parse operations:
///
/// ```compile_fail,E0451
/// use merman_core::{PresentationMetadata, ParsedDiagramRender, RenderSemanticModel};
///
/// fn forge(
///     meta: PresentationMetadata,
///     model: RenderSemanticModel,
/// ) -> ParsedDiagramRender {
///     ParsedDiagramRender { meta, model }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct ParsedDiagramRender {
    /// Diagram type and effective configuration extracted during preprocessing.
    meta: PresentationMetadata,
    /// Catalog-owned presentation family selected after effective appearance is resolved.
    family_id: Option<crate::DiagramFamilyId>,
    /// Typed model consumed by layout and SVG renderers.
    model: RenderSemanticModel,
    /// Parser-owned render-only data paired with the typed model.
    context: RenderSemanticContext,
}

impl ParsedDiagramRender {
    pub(crate) fn new(meta: PresentationMetadata, model: RenderSemanticModel) -> Self {
        let family_id = parsed_operation_family_id(&meta, &model);
        Self {
            meta,
            family_id,
            model,
            context: RenderSemanticContext::default(),
        }
    }

    pub(crate) fn from_parse_output(
        meta: PresentationMetadata,
        output: RenderSemanticParseOutput,
    ) -> Self {
        let (model, context) = output.into_parts();
        let family_id = parsed_operation_family_id(&meta, &model);
        Self {
            meta,
            family_id,
            model,
            context,
        }
    }

    /// Returns the metadata paired with this render model by the core parse pipeline.
    pub fn metadata(&self) -> &PresentationMetadata {
        &self.meta
    }

    /// Returns the typed render model paired with this metadata by the core parse pipeline.
    pub fn model(&self) -> &RenderSemanticModel {
        &self.model
    }

    /// Returns the catalog-owned presentation family selected for this render operation.
    ///
    /// Flowchart and Swimlane share consumers, so their effective layout can select a different
    /// presentation family without changing the metadata's logical diagram identity or admitting
    /// a disabled language. Custom registry models have no built-in presentation family.
    pub const fn family_id(&self) -> Option<crate::DiagramFamilyId> {
        self.family_id
    }

    /// Consumes the parsed diagram and returns its canonical metadata/model projection.
    ///
    /// This intentionally omits parser-owned render context. Renderer integrations borrow the
    /// complete canonical object and its family-specific render-context accessors instead. Use the
    /// parsed compatibility projector when the frozen output shape must be retained.
    pub fn into_parts(self) -> (PresentationMetadata, RenderSemanticModel) {
        (self.meta, self.model)
    }

    /// Borrows the frozen full-output facts paired with this canonical model.
    ///
    /// The context exposes declared data; it does not authenticate source admission.
    pub fn render_context(&self) -> &RenderSemanticContext {
        &self.context
    }

    /// Projects compatibility JSON using this pair's metadata and frozen output facts.
    #[cfg(feature = "compat-json")]
    pub fn compatibility_json(&self) -> Result<ManagedSemanticJson> {
        let control = OperationControl::new();
        self.compatibility_json_controlled(&control)
            .expect("a private operation control cannot be cancelled")
    }

    /// Projects the paired compatibility output while observing the caller's operation control.
    #[cfg(feature = "compat-json")]
    pub fn compatibility_json_controlled(
        &self,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ManagedSemanticJson>> {
        self.model
            .compatibility_json_with_context_controlled(&self.meta, &self.context, control)
    }

    #[doc(hidden)]
    pub fn retained_render_context_bytes(&self) -> usize {
        self.context.retained_text_bytes()
    }

    /// Borrows all parser-owned Flowchart render facts without exposing them in the typed model.
    #[doc(hidden)]
    #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
    pub fn flowchart_render_context(
        &self,
    ) -> Option<&crate::diagrams::flowchart::FlowchartRenderContext> {
        self.context.flowchart_render_context()
    }

    /// Borrows parser-owned ClassDiagram encounter-order style evidence.
    #[doc(hidden)]
    #[cfg(feature = "diagram-class")]
    pub fn class_style_precedence_facts(
        &self,
    ) -> Option<&crate::models::class_diagram::ClassStylePrecedenceFacts> {
        self.context.class_style_precedence_facts()
    }
}

fn parsed_operation_family_id(
    meta: &PresentationMetadata,
    model: &RenderSemanticModel,
) -> Option<crate::DiagramFamilyId> {
    if matches!(model, RenderSemanticModel::CustomJson(_)) {
        return None;
    }
    crate::family::operation_family_id(&meta.diagram_type, meta.effective_config.as_config())
}

/// Parses with a registry entry while preserving the caller-owned parse control.
#[cfg(feature = "compat-json")]
pub(crate) fn parse_or_unsupported_controlled(
    registry: &DiagramRegistry,
    diagram_type: &str,
    code: &str,
    meta: &PresentationMetadata,
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
        _meta: &PresentationMetadata,
        control: &OperationControl,
    ) -> OperationControlResult<Result<Value>> {
        control.checkpoint()?;
        Ok(Ok(Value::Null))
    }

    fn custom_render_parser(
        _code: &str,
        _meta: &PresentationMetadata,
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
        _meta: &PresentationMetadata,
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
            _meta: &PresentationMetadata,
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
                        let manual = ParsedDiagramRender::new(meta.clone(), parsed.model().clone());
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
