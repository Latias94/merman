use crate::analyzer::{AnalysisDiagnosticPolicy, AnalysisEnvironmentIdentity, Analyzer};
use crate::diagnostic_projection::{DiagnosticCandidate, append_projected_diagnostic_candidates};
use crate::payload::DiagnosticRetainedWeight;
use crate::{
    ANALYSIS_FACTS_PAYLOAD_VERSION, AnalysisCancellationToken, AnalysisCancelled,
    AnalysisDiagnostic, AnalysisPayload, DocumentDiagram, DocumentDiagramKind, FenceDelimiter,
    FenceDelimiterSpans, FenceMarker, FenceTextIndex, FenceTextIndexSource, SharedTextSlice,
    SourceDescriptor, SourceMap, Summary,
};
use merman_core::{
    EditorExpectedSyntax, EditorExpectedSyntaxKind, EditorRenamePolicy, EditorSemanticKind,
    EditorSemanticRole, EditorSemanticSymbol, SourceSpan,
};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::mem::size_of;

use crate::retained_weight::{ARC_ALLOCATION_OVERHEAD, RetainedWeight};

/// One sealed rich capture bound to an exact parser environment and snapshot policy.
#[derive(Debug)]
pub struct AnalysisGeneration {
    storage: Box<AnalysisGenerationStorage>,
}

#[derive(Debug)]
struct AnalysisGenerationStorage {
    source_map: SourceMap,
    diagrams: Vec<AnalyzedDiagram>,
    document_candidates: Vec<DiagnosticCandidate>,
    environment_identity: AnalysisEnvironmentIdentity,
    source: SourceDescriptor,
}

/// The result of rich analysis capture before diagnostic projection.
///
/// A rejected source has no generation, but retains a canonical diagnostics payload and typed
/// resource-limit reason through [`AnalysisRejection`].
#[derive(Debug)]
pub enum AnalysisCaptureOutcome {
    Ready(AnalysisGeneration),
    Rejected(AnalysisRejection),
}

/// A source rejected before an [`AnalysisGeneration`] could be created.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisRejection {
    payload: Box<AnalysisPayload>,
    resource_limit: AnalysisResourceLimit,
}

/// The exact admission budget that rejected an analysis before a generation was created.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AnalysisResourceLimit {
    SourceBytes {
        source_len: usize,
        max_source_bytes: usize,
    },
    DocumentDiagrams {
        observed_document_diagrams: usize,
        max_document_diagrams: usize,
    },
}

impl AnalysisResourceLimit {
    /// Returns the stable resource-limit identifier.
    pub const fn id(self) -> &'static str {
        match self {
            Self::SourceBytes { .. } => {
                merman_core::resources::InputResourceLimitId::MaxSourceBytes.as_str()
            }
            Self::DocumentDiagrams { .. } => crate::MAX_DOCUMENT_DIAGRAMS_RESOURCE_LIMIT_ID,
        }
    }

    /// Returns the observed resource value.
    pub const fn observed(self) -> usize {
        match self {
            Self::SourceBytes { source_len, .. } => source_len,
            Self::DocumentDiagrams {
                observed_document_diagrams,
                ..
            } => observed_document_diagrams,
        }
    }

    /// Returns the configured maximum resource value.
    pub const fn maximum(self) -> usize {
        match self {
            Self::SourceBytes {
                max_source_bytes, ..
            } => max_source_bytes,
            Self::DocumentDiagrams {
                max_document_diagrams,
                ..
            } => max_document_diagrams,
        }
    }
}

impl fmt::Display for AnalysisResourceLimit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} observed={} max={}",
            self.id(),
            self.observed(),
            self.maximum()
        )
    }
}

impl AnalysisCaptureOutcome {
    /// Borrows the ready generation, if capture succeeded.
    pub fn as_ready(&self) -> Option<&AnalysisGeneration> {
        match self {
            Self::Ready(generation) => Some(generation),
            Self::Rejected(_) => None,
        }
    }

    /// Extracts the generation or returns the typed rejection.
    pub fn into_ready(self) -> Result<AnalysisGeneration, AnalysisRejection> {
        match self {
            Self::Ready(generation) => Ok(generation),
            Self::Rejected(rejection) => Err(rejection),
        }
    }

    /// Borrows the typed rejection, if admission failed.
    pub fn rejection(&self) -> Option<&AnalysisRejection> {
        match self {
            Self::Ready(_) => None,
            Self::Rejected(rejection) => Some(rejection),
        }
    }
}

impl AnalysisRejection {
    pub(crate) fn source_limit(
        source: SourceDescriptor,
        diagnostics: Vec<AnalysisDiagnostic>,
        source_len: usize,
        max_source_bytes: usize,
    ) -> Self {
        Self {
            payload: Box::new(AnalysisPayload::new(source, diagnostics)),
            resource_limit: AnalysisResourceLimit::SourceBytes {
                source_len,
                max_source_bytes,
            },
        }
    }

    pub(crate) fn document_diagram_limit(
        source: SourceDescriptor,
        diagnostics: Vec<AnalysisDiagnostic>,
        observed_document_diagrams: usize,
        max_document_diagrams: usize,
    ) -> Self {
        Self {
            payload: Box::new(AnalysisPayload::new(source, diagnostics)),
            resource_limit: AnalysisResourceLimit::DocumentDiagrams {
                observed_document_diagrams,
                max_document_diagrams,
            },
        }
    }

    /// Borrows the canonical diagnostics payload for the rejection.
    pub fn payload(&self) -> &AnalysisPayload {
        self.payload.as_ref()
    }

    /// Extracts the canonical diagnostics payload.
    pub fn into_payload(self) -> AnalysisPayload {
        *self.payload
    }

    /// Returns the exact resource limit that rejected capture.
    pub const fn resource_limit(&self) -> AnalysisResourceLimit {
        self.resource_limit
    }
}

impl AnalysisGeneration {
    pub(crate) fn new(
        source_map: SourceMap,
        diagrams: Vec<AnalyzedDiagram>,
        analyzer: &Analyzer,
    ) -> Self {
        let environment_identity = analyzer.environment_identity().clone();
        let source = analyzer.options().source().clone();
        Self {
            storage: Box::new(AnalysisGenerationStorage {
                source_map,
                diagrams,
                document_candidates: Vec::new(),
                environment_identity,
                source,
            }),
        }
    }

    pub(crate) fn with_document_candidates(mut self, candidates: Vec<DiagnosticCandidate>) -> Self {
        self.storage.document_candidates = candidates;
        self
    }

    /// Projects diagnostics without parsing or mutating this generation.
    pub fn project(&self, policy: &AnalysisDiagnosticPolicy) -> AnalysisPayload {
        let cancellation = AnalysisCancellationToken::new();
        self.project_cancellable(policy, &cancellation)
            .expect("a private analysis cancellation token cannot be cancelled")
    }

    /// Cancellable form of [`Self::project`].
    pub fn project_cancellable(
        &self,
        policy: &AnalysisDiagnosticPolicy,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<AnalysisPayload, AnalysisCancelled> {
        cancellation.checkpoint()?;
        let mut diagnostics = Vec::with_capacity(self.storage.document_candidates.len());
        append_projected_diagnostic_candidates(
            &mut diagnostics,
            &self.storage.document_candidates,
            policy,
            cancellation,
        )?;
        for diagram in &self.storage.diagrams {
            cancellation.checkpoint()?;
            append_projected_diagnostic_candidates(
                &mut diagnostics,
                &diagram.diagnostic_candidates,
                policy,
                cancellation,
            )?;
        }
        cancellation.checkpoint()?;
        AnalysisPayload::new_cancellable(self.storage.source.clone(), diagnostics, cancellation)
    }

    pub fn environment_identity(&self) -> &AnalysisEnvironmentIdentity {
        &self.storage.environment_identity
    }

    pub fn source(&self) -> &SourceDescriptor {
        &self.storage.source
    }

    pub fn source_map(&self) -> &SourceMap {
        &self.storage.source_map
    }

    /// Estimates heap owned by this generation while excluding the shared source buffer.
    ///
    /// The generation retains only its opaque environment identity and source metadata from the
    /// capture policy. The estimate includes those allocations, the full lazy `SourceMap` metric
    /// allowance, and saturates on overflow.
    pub fn estimated_owned_heap_bytes_excluding_source(&self) -> usize {
        let mut weight = RetainedWeight::new(size_of::<AnalysisGenerationStorage>());
        let mut diagnostic_weight = DiagnosticRetainedWeight::default();
        weight.add(
            self.storage
                .source_map
                .estimated_owned_heap_bytes_excluding_source(),
        );
        weight.add_array::<AnalyzedDiagram>(self.storage.diagrams.capacity());
        for diagram in &self.storage.diagrams {
            weight.add(diagram.estimated_owned_heap_bytes_excluding_source(&mut diagnostic_weight));
        }
        weight.add_array::<DiagnosticCandidate>(self.storage.document_candidates.capacity());
        for candidate in &self.storage.document_candidates {
            candidate.add_estimated_owned_heap_bytes(&mut diagnostic_weight);
        }
        weight.add(diagnostic_weight.finish());
        weight.add(ARC_ALLOCATION_OVERHEAD);
        weight.add(self.storage.source.estimated_owned_heap_bytes());
        weight.finish()
    }

    pub fn diagrams(&self) -> &[AnalyzedDiagram] {
        &self.storage.diagrams
    }

    #[cfg(test)]
    pub(crate) fn diagnostic_candidate_rule_ids(&self) -> Vec<&'static str> {
        self.storage
            .document_candidates
            .iter()
            .chain(
                self.storage
                    .diagrams
                    .iter()
                    .flat_map(|diagram| diagram.diagnostic_candidates.iter()),
            )
            .map(DiagnosticCandidate::rule_id)
            .collect()
    }

    pub(crate) fn to_facts_payload(
        &self,
        policy: &AnalysisDiagnosticPolicy,
    ) -> AnalysisFactsPayload {
        let cancellation = AnalysisCancellationToken::new();
        self.to_facts_payload_cancellable(policy, &cancellation)
            .expect("a private analysis cancellation token cannot be cancelled")
    }

    /// Projects the serializable facts contract without reparsing this generation.
    pub fn to_facts_payload_cancellable(
        &self,
        policy: &AnalysisDiagnosticPolicy,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<AnalysisFactsPayload, AnalysisCancelled> {
        let payload = self.project_cancellable(policy, cancellation)?;
        AnalysisFactsPayload::from_generation_cancellable(self, &payload, cancellation)
    }
}

/// Parser-owned diagram outcome that is independent from diagnostic policy and severity.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagramParseDisposition {
    Parsed,
    Recovered,
    #[default]
    Unavailable,
}

#[derive(Debug, Clone)]
pub struct AnalyzedDiagram {
    pub(crate) source_id: String,
    pub(crate) index: usize,
    pub(crate) kind: DocumentDiagramKind,
    pub(crate) source: SourceDescriptor,
    pub(crate) start: usize,
    pub(crate) body_start: usize,
    pub(crate) body_end: usize,
    pub(crate) end: usize,
    pub(crate) text: SharedTextSlice,
    pub(crate) fence_delimiter: Option<FenceDelimiter>,
    pub(crate) fence_delimiter_spans: Option<FenceDelimiterSpans>,
    pub(crate) syntax: AnalysisSyntaxFacts,
    pub(crate) diagnostic_candidates: Vec<DiagnosticCandidate>,
    pub(crate) parse_disposition: DiagramParseDisposition,
}

impl AnalyzedDiagram {
    pub fn source_id(&self) -> &str {
        &self.source_id
    }

    pub const fn index(&self) -> usize {
        self.index
    }

    pub const fn kind(&self) -> DocumentDiagramKind {
        self.kind
    }

    pub fn source(&self) -> &SourceDescriptor {
        &self.source
    }

    pub fn document_range(&self) -> std::ops::Range<usize> {
        self.start..self.end
    }

    pub fn body_range(&self) -> std::ops::Range<usize> {
        self.body_start..self.body_end
    }

    pub fn text(&self) -> &SharedTextSlice {
        &self.text
    }

    pub const fn fence_delimiter(&self) -> Option<FenceDelimiter> {
        self.fence_delimiter
    }

    pub fn fence_delimiter_spans(&self) -> Option<&FenceDelimiterSpans> {
        self.fence_delimiter_spans.as_ref()
    }

    pub fn syntax(&self) -> &AnalysisSyntaxFacts {
        &self.syntax
    }

    pub(crate) fn from_document_diagram(
        diagram: &DocumentDiagram,
        syntax: AnalysisSyntaxFacts,
        diagnostic_candidates: Vec<DiagnosticCandidate>,
        parse_disposition: DiagramParseDisposition,
    ) -> Self {
        Self {
            source_id: diagram.id.clone(),
            index: diagram.index,
            kind: diagram.kind,
            source: diagram.source.clone(),
            start: diagram.start,
            body_start: diagram.body_start,
            body_end: diagram.body_end,
            end: diagram.end,
            text: diagram.text.clone(),
            fence_delimiter: diagram.fence_delimiter,
            fence_delimiter_spans: diagram.fence_delimiter_spans.clone(),
            syntax,
            diagnostic_candidates,
            parse_disposition,
        }
    }

    /// Returns the parser-owned outcome retained by this analysis generation.
    pub fn parse_disposition(&self) -> DiagramParseDisposition {
        self.parse_disposition
    }

    fn estimated_owned_heap_bytes_excluding_source(
        &self,
        diagnostic_weight: &mut DiagnosticRetainedWeight,
    ) -> usize {
        let mut weight = RetainedWeight::default();
        weight.add_string(&self.source_id);
        weight.add(self.source.estimated_owned_heap_bytes());
        weight.add(self.syntax.estimated_owned_heap_bytes());
        weight.add_array::<DiagnosticCandidate>(self.diagnostic_candidates.capacity());
        for candidate in &self.diagnostic_candidates {
            candidate.add_estimated_owned_heap_bytes(diagnostic_weight);
        }
        weight.finish()
    }
}

#[derive(Debug, Clone)]
pub struct AnalysisSyntaxFacts {
    pub diagram_type: Option<String>,
    pub effective_layout: Option<String>,
    pub text_index: FenceTextIndex,
}

impl AnalysisSyntaxFacts {
    pub fn new(diagram_type: Option<String>, text_index: FenceTextIndex) -> Self {
        Self {
            diagram_type,
            effective_layout: None,
            text_index,
        }
    }

    pub fn unavailable(diagram_type: Option<String>) -> Self {
        Self {
            text_index: FenceTextIndex::default(),
            diagram_type,
            effective_layout: None,
        }
    }

    pub fn source(&self) -> FenceTextIndexSource {
        self.text_index.source()
    }

    pub fn with_effective_layout(mut self, effective_layout: Option<String>) -> Self {
        self.effective_layout = effective_layout;
        self
    }

    fn estimated_owned_heap_bytes(&self) -> usize {
        let mut weight = RetainedWeight::default();
        weight.add_optional_string(&self.diagram_type);
        weight.add_optional_string(&self.effective_layout);
        weight.add(self.text_index.estimated_owned_heap_bytes());
        weight.finish()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AnalysisFactsPayload {
    pub version: u32,
    pub valid: bool,
    pub summary: Summary,
    pub source: SourceDescriptor,
    pub diagnostics: Vec<AnalysisDiagnostic>,
    pub diagrams: Vec<AnalysisDiagramFacts>,
}

#[derive(Deserialize)]
struct AnalysisFactsPayloadTransport {
    version: u32,
    valid: bool,
    summary: Summary,
    source: SourceDescriptor,
    diagnostics: Vec<AnalysisDiagnostic>,
    diagrams: Vec<AnalysisDiagramFacts>,
}

impl<'de> Deserialize<'de> for AnalysisFactsPayload {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Facts are a JSON binding wire contract. Read an untyped transport first so the
        // version boundary is checked before any potentially incompatible nested field.
        let raw = serde_json::Value::deserialize(deserializer)?;
        let version = raw
            .get("version")
            .ok_or_else(|| serde::de::Error::missing_field("version"))
            .and_then(|value| {
                serde_json::from_value::<u32>(value.clone()).map_err(serde::de::Error::custom)
            })?;
        if version != ANALYSIS_FACTS_PAYLOAD_VERSION {
            return Err(serde::de::Error::custom(format_args!(
                "unsupported analysis facts payload version {version}; expected {ANALYSIS_FACTS_PAYLOAD_VERSION}"
            )));
        }

        let transport: AnalysisFactsPayloadTransport =
            serde_json::from_value(raw).map_err(serde::de::Error::custom)?;
        Ok(Self {
            version: transport.version,
            valid: transport.valid,
            summary: transport.summary,
            source: transport.source,
            diagnostics: transport.diagnostics,
            diagrams: transport.diagrams,
        })
    }
}

impl AnalysisFactsPayload {
    fn from_generation_cancellable(
        generation: &AnalysisGeneration,
        payload: &AnalysisPayload,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<Self, AnalysisCancelled> {
        cancellation.checkpoint()?;
        let mut diagrams = Vec::with_capacity(generation.diagrams().len());
        for diagram in generation.diagrams() {
            cancellation.checkpoint()?;
            diagrams.push(AnalysisDiagramFacts::from_diagram_cancellable(
                diagram,
                generation.source_map(),
                cancellation,
            )?);
        }
        cancellation.checkpoint()?;
        Ok(Self {
            version: ANALYSIS_FACTS_PAYLOAD_VERSION,
            valid: payload.valid,
            summary: payload.summary,
            source: payload.source.clone(),
            diagnostics: payload.diagnostics.clone(),
            diagrams,
        })
    }

    pub fn from_rejection(rejection: &AnalysisRejection) -> Self {
        let payload = rejection.payload();
        Self {
            version: ANALYSIS_FACTS_PAYLOAD_VERSION,
            valid: payload.valid,
            summary: payload.summary,
            source: payload.source.clone(),
            diagnostics: payload.diagnostics.clone(),
            diagrams: Vec::new(),
        }
    }

    pub fn to_json_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    pub fn to_pretty_json_string(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisDiagramFacts {
    pub source_id: String,
    pub index: usize,
    pub kind: String,
    pub source: SourceDescriptor,
    pub span: Option<crate::DiagnosticSpan>,
    pub body_span: Option<crate::DiagnosticSpan>,
    pub text_len: usize,
    pub fence_delimiter: Option<AnalysisFenceDelimiterFacts>,
    #[serde(default)]
    pub parse_disposition: DiagramParseDisposition,
    pub syntax: AnalysisDiagramSyntaxFacts,
}

impl AnalysisDiagramFacts {
    fn from_diagram_cancellable(
        diagram: &AnalyzedDiagram,
        source_map: &SourceMap,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<Self, AnalysisCancelled> {
        cancellation.checkpoint()?;
        let span = source_map
            .span_cancellable(diagram.start, diagram.end, cancellation)?
            .ok();
        let body_span = source_map
            .span_cancellable(diagram.body_start, diagram.body_end, cancellation)?
            .ok();
        let syntax = AnalysisDiagramSyntaxFacts::from_syntax_cancellable(
            &diagram.syntax,
            source_map,
            diagram.body_start,
            cancellation,
        )?;
        Ok(Self {
            source_id: diagram.source_id.clone(),
            index: diagram.index,
            kind: diagram_kind_name(diagram.kind).to_string(),
            source: diagram.source.clone(),
            span,
            body_span,
            text_len: diagram.text.len(),
            fence_delimiter: diagram
                .fence_delimiter
                .map(AnalysisFenceDelimiterFacts::from),
            parse_disposition: diagram.parse_disposition(),
            syntax,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisFenceDelimiterFacts {
    pub marker: String,
    pub len: usize,
}

impl From<FenceDelimiter> for AnalysisFenceDelimiterFacts {
    fn from(value: FenceDelimiter) -> Self {
        Self {
            marker: fence_marker_name(value.marker()).to_string(),
            len: value.marker_len(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisDiagramSyntaxFacts {
    pub diagram_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_layout: Option<String>,
    pub fact_source: FenceTextIndexSource,
    pub parser_backed: bool,
    pub recovered: bool,
    pub source_mapped_spans: bool,
    pub node_ids: Vec<String>,
    pub class_names: Vec<String>,
    pub directive_prefixes: Vec<String>,
    pub references: Vec<AnalysisReferenceFacts>,
    pub outline_items: Vec<AnalysisLineItemFacts>,
    pub semantic_items: Vec<AnalysisSemanticItemFacts>,
    pub expected_syntax: Vec<AnalysisExpectedSyntaxFacts>,
}

impl AnalysisDiagramSyntaxFacts {
    fn from_syntax_cancellable(
        syntax: &AnalysisSyntaxFacts,
        source_map: &SourceMap,
        body_start: usize,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<Self, AnalysisCancelled> {
        let text_index = &syntax.text_index;
        let fact_source = text_index.source();
        cancellation.checkpoint()?;
        let mut node_ids = Vec::new();
        for node_id in text_index.node_ids() {
            cancellation.checkpoint()?;
            node_ids.push(node_id.clone());
        }
        let mut class_names = Vec::new();
        for class_name in text_index.class_names() {
            cancellation.checkpoint()?;
            class_names.push(class_name.clone());
        }
        let mut directive_prefixes = Vec::new();
        for directive_prefix in text_index.directive_prefixes() {
            cancellation.checkpoint()?;
            directive_prefixes.push(directive_prefix.clone());
        }
        let mut references = Vec::new();
        for (group, spans) in text_index.reference_groups() {
            cancellation.checkpoint()?;
            references.push(AnalysisReferenceFacts::from_reference_cancellable(
                group.name,
                group.kind,
                spans,
                source_map,
                body_start,
                cancellation,
            )?);
        }
        let mut outline_items = Vec::new();
        for item in text_index
            .semantic_items()
            .iter()
            .filter(|item| item.role.contributes_outline())
        {
            cancellation.checkpoint()?;
            outline_items.push(AnalysisLineItemFacts::from_item_cancellable(
                item,
                source_map,
                body_start,
                cancellation,
            )?);
        }
        let mut semantic_items = Vec::new();
        for item in text_index.semantic_items() {
            cancellation.checkpoint()?;
            semantic_items.push(AnalysisSemanticItemFacts::from_item_cancellable(
                item,
                source_map,
                body_start,
                cancellation,
            )?);
        }
        let mut expected_syntax = Vec::new();
        for expected in text_index.expected_syntax() {
            cancellation.checkpoint()?;
            expected_syntax.push(AnalysisExpectedSyntaxFacts::from_expected_cancellable(
                expected,
                source_map,
                body_start,
                cancellation,
            )?);
        }
        cancellation.checkpoint()?;
        Ok(Self {
            diagram_type: syntax.diagram_type.clone(),
            effective_layout: syntax.effective_layout.clone(),
            fact_source,
            parser_backed: fact_source.is_parser_backed(),
            recovered: fact_source.is_recovered(),
            source_mapped_spans: fact_source.has_source_mapped_spans(),
            node_ids,
            class_names,
            directive_prefixes,
            references,
            outline_items,
            semantic_items,
            expected_syntax,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisEditorSymbolKind {
    Class,
    Event,
    Function,
    Module,
    Namespace,
    Object,
    Package,
    Property,
    String,
    Struct,
    Variable,
}

impl From<EditorSemanticKind> for AnalysisEditorSymbolKind {
    fn from(kind: EditorSemanticKind) -> Self {
        match kind {
            EditorSemanticKind::Class => Self::Class,
            EditorSemanticKind::Event => Self::Event,
            EditorSemanticKind::Function => Self::Function,
            EditorSemanticKind::Module => Self::Module,
            EditorSemanticKind::Namespace => Self::Namespace,
            EditorSemanticKind::Object => Self::Object,
            EditorSemanticKind::Package => Self::Package,
            EditorSemanticKind::Property => Self::Property,
            EditorSemanticKind::String => Self::String,
            EditorSemanticKind::Struct => Self::Struct,
            EditorSemanticKind::Variable => Self::Variable,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisSemanticRole {
    Entity,
    ClassDefinition,
    Reference,
    Outline,
    Payload,
}

impl From<EditorSemanticRole> for AnalysisSemanticRole {
    fn from(role: EditorSemanticRole) -> Self {
        match role {
            EditorSemanticRole::Entity => Self::Entity,
            EditorSemanticRole::ClassDefinition => Self::ClassDefinition,
            EditorSemanticRole::Reference => Self::Reference,
            EditorSemanticRole::Outline => Self::Outline,
            EditorSemanticRole::Payload => Self::Payload,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisExpectedSyntaxKind {
    Directive,
    Frontmatter,
    IdList,
    NodeIdentifier,
    ClassName,
    Operator,
    Shape,
    ShapeTrigger,
    Direction,
    StyleValue,
    InteractionAction,
    Payload,
}

impl From<EditorExpectedSyntaxKind> for AnalysisExpectedSyntaxKind {
    fn from(kind: EditorExpectedSyntaxKind) -> Self {
        match kind {
            EditorExpectedSyntaxKind::Directive => Self::Directive,
            EditorExpectedSyntaxKind::Frontmatter => Self::Frontmatter,
            EditorExpectedSyntaxKind::IdList => Self::IdList,
            EditorExpectedSyntaxKind::NodeIdentifier => Self::NodeIdentifier,
            EditorExpectedSyntaxKind::ClassName => Self::ClassName,
            EditorExpectedSyntaxKind::FlowchartOperator => Self::Operator,
            EditorExpectedSyntaxKind::ShapeValue => Self::Shape,
            EditorExpectedSyntaxKind::ShapeTrigger => Self::ShapeTrigger,
            EditorExpectedSyntaxKind::FlowchartDirectionValue
            | EditorExpectedSyntaxKind::CardinalDirectionValue
            | EditorExpectedSyntaxKind::BlockDirectionValue => Self::Direction,
            EditorExpectedSyntaxKind::StyleValue => Self::StyleValue,
            EditorExpectedSyntaxKind::InteractionAction => Self::InteractionAction,
            EditorExpectedSyntaxKind::Payload => Self::Payload,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisReferenceFacts {
    pub name: String,
    pub kind: AnalysisEditorSymbolKind,
    pub spans: Vec<AnalysisFactSpan>,
}

impl AnalysisReferenceFacts {
    fn from_reference_cancellable(
        name: String,
        kind: EditorSemanticKind,
        spans: Vec<crate::ByteSpan>,
        source_map: &SourceMap,
        body_start: usize,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<Self, AnalysisCancelled> {
        let mut projected_spans = Vec::with_capacity(spans.len());
        for span in spans {
            cancellation.checkpoint()?;
            projected_spans.push(AnalysisFactSpan::from_local_cancellable(
                span,
                source_map,
                body_start,
                cancellation,
            )?);
        }
        Ok(Self {
            name,
            kind: kind.into(),
            spans: projected_spans,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisLineItemFacts {
    pub name: String,
    pub detail: Option<String>,
    pub kind: AnalysisEditorSymbolKind,
    pub span: AnalysisFactSpan,
    pub selection: AnalysisFactSpan,
}

impl AnalysisLineItemFacts {
    fn from_item_cancellable(
        item: &EditorSemanticSymbol,
        source_map: &SourceMap,
        body_start: usize,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<Self, AnalysisCancelled> {
        Ok(Self {
            name: item.name.clone(),
            detail: item.detail.clone(),
            kind: item.kind.into(),
            span: AnalysisFactSpan::from_source_cancellable(
                item.span,
                source_map,
                body_start,
                cancellation,
            )?,
            selection: AnalysisFactSpan::from_source_cancellable(
                item.selection,
                source_map,
                body_start,
                cancellation,
            )?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisSemanticItemFacts {
    pub name: String,
    pub detail: Option<String>,
    pub kind: AnalysisEditorSymbolKind,
    pub role: AnalysisSemanticRole,
    #[serde(default = "missing_rename_policy")]
    pub rename_policy: EditorRenamePolicy,
    pub span: AnalysisFactSpan,
    pub selection: AnalysisFactSpan,
}

fn missing_rename_policy() -> EditorRenamePolicy {
    EditorRenamePolicy::None
}

impl AnalysisSemanticItemFacts {
    fn from_item_cancellable(
        item: &EditorSemanticSymbol,
        source_map: &SourceMap,
        body_start: usize,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<Self, AnalysisCancelled> {
        Ok(Self {
            name: item.name.clone(),
            detail: item.detail.clone(),
            kind: item.kind.into(),
            role: item.role.into(),
            rename_policy: item.rename_policy,
            span: AnalysisFactSpan::from_source_cancellable(
                item.span,
                source_map,
                body_start,
                cancellation,
            )?,
            selection: AnalysisFactSpan::from_source_cancellable(
                item.selection,
                source_map,
                body_start,
                cancellation,
            )?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisExpectedSyntaxFacts {
    pub kind: AnalysisExpectedSyntaxKind,
    pub span: AnalysisFactSpan,
}

impl AnalysisExpectedSyntaxFacts {
    fn from_expected_cancellable(
        expected: &EditorExpectedSyntax,
        source_map: &SourceMap,
        body_start: usize,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<Self, AnalysisCancelled> {
        Ok(Self {
            kind: expected.kind.into(),
            span: AnalysisFactSpan::from_source_cancellable(
                expected.span,
                source_map,
                body_start,
                cancellation,
            )?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisFactSpan {
    pub local: crate::ByteSpan,
    pub document: Option<crate::DiagnosticSpan>,
}

impl AnalysisFactSpan {
    fn from_source_cancellable(
        local: SourceSpan,
        source_map: &SourceMap,
        body_start: usize,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<Self, AnalysisCancelled> {
        Self::from_local_cancellable(
            crate::ByteSpan {
                start: local.start,
                end: local.end,
            },
            source_map,
            body_start,
            cancellation,
        )
    }

    fn from_local_cancellable(
        local: crate::ByteSpan,
        source_map: &SourceMap,
        body_start: usize,
        cancellation: &AnalysisCancellationToken,
    ) -> Result<Self, AnalysisCancelled> {
        let document_start = body_start.saturating_add(local.start);
        let document_end = body_start.saturating_add(local.end);
        Ok(Self {
            local,
            document: source_map
                .span_cancellable(document_start, document_end, cancellation)?
                .ok(),
        })
    }
}

fn diagram_kind_name(kind: DocumentDiagramKind) -> &'static str {
    match kind {
        DocumentDiagramKind::WholeDocument => "whole_document",
        DocumentDiagramKind::MermaidFence => "mermaid_fence",
    }
}

fn fence_marker_name(marker: FenceMarker) -> &'static str {
    match marker {
        FenceMarker::Backtick => "backtick",
        FenceMarker::Tilde => "tilde",
        FenceMarker::Colon => "colon",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn diagram_parse_disposition_uses_stable_snake_case_values() {
        for (disposition, wire_value) in [
            (DiagramParseDisposition::Parsed, "parsed"),
            (DiagramParseDisposition::Recovered, "recovered"),
            (DiagramParseDisposition::Unavailable, "unavailable"),
        ] {
            assert_eq!(
                serde_json::to_value(disposition).unwrap(),
                json!(wire_value)
            );
            assert_eq!(
                serde_json::from_value::<DiagramParseDisposition>(json!(wire_value)).unwrap(),
                disposition
            );
        }
    }
}
