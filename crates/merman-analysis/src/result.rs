use crate::analyzer::{
    AnalysisDiagnosticPolicy, AnalysisEnvironmentIdentity, AnalysisOptions, AnalysisSnapshotPolicy,
    Analyzer,
};
use crate::editor::FenceExpectedSyntax;
use crate::{
    ANALYSIS_FACTS_PAYLOAD_VERSION, AnalysisCancellationToken, AnalysisCancelled,
    AnalysisDiagnostic, AnalysisPayload, DocumentDiagram, DocumentDiagramKind, FenceDelimiter,
    FenceDelimiterSpans, FenceLineItem, FenceMarker, FenceReferenceGroup, FenceSemanticItem,
    FenceTextIndex, FenceTextIndexSource, SharedTextSlice, SourceDescriptor, SourceMap, Summary,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

/// One sealed rich capture bound to an exact parser environment and snapshot policy.
#[derive(Debug)]
pub struct AnalysisGeneration {
    source_map: SourceMap,
    diagrams: Vec<AnalyzedDiagram>,
    document_error: Option<Arc<merman_core::Error>>,
    environment_identity: AnalysisEnvironmentIdentity,
    snapshot_policy: AnalysisSnapshotPolicy,
}

#[derive(Debug)]
pub enum AnalysisCaptureOutcome {
    Ready(AnalysisGeneration),
    Rejected(AnalysisRejection),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisRejection {
    payload: Box<AnalysisPayload>,
    source_len: usize,
    max_source_bytes: usize,
}

impl AnalysisCaptureOutcome {
    pub fn as_ready(&self) -> Option<&AnalysisGeneration> {
        match self {
            Self::Ready(generation) => Some(generation),
            Self::Rejected(_) => None,
        }
    }

    pub fn into_ready(self) -> Result<AnalysisGeneration, AnalysisRejection> {
        match self {
            Self::Ready(generation) => Ok(generation),
            Self::Rejected(rejection) => Err(rejection),
        }
    }

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
            source_len,
            max_source_bytes,
        }
    }

    pub fn payload(&self) -> &AnalysisPayload {
        self.payload.as_ref()
    }

    pub fn into_payload(self) -> AnalysisPayload {
        *self.payload
    }

    pub const fn source_len(&self) -> usize {
        self.source_len
    }

    pub const fn max_source_bytes(&self) -> usize {
        self.max_source_bytes
    }
}

impl AnalysisGeneration {
    pub(crate) fn new(
        source_map: SourceMap,
        diagrams: Vec<AnalyzedDiagram>,
        analyzer: &Analyzer,
    ) -> Self {
        let environment_identity = analyzer.environment_identity().clone();
        let snapshot_policy = analyzer.options().snapshot_policy().clone();
        Self {
            source_map,
            diagrams,
            document_error: None,
            environment_identity,
            snapshot_policy,
        }
    }

    pub(crate) fn with_document_error(mut self, error: Arc<merman_core::Error>) -> Self {
        self.document_error = Some(error);
        self
    }

    pub(crate) fn document_error(&self) -> Option<&merman_core::Error> {
        self.document_error.as_deref()
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
        Analyzer::with_options(AnalysisOptions {
            snapshot: self.snapshot_policy.clone(),
            diagnostics: policy.clone(),
        })
        .project_generation_cancellable(self, cancellation)
    }

    pub fn environment_identity(&self) -> &AnalysisEnvironmentIdentity {
        &self.environment_identity
    }

    pub fn snapshot_policy(&self) -> &AnalysisSnapshotPolicy {
        &self.snapshot_policy
    }

    pub fn source_map(&self) -> &SourceMap {
        &self.source_map
    }

    pub fn diagrams(&self) -> &[AnalyzedDiagram] {
        &self.diagrams
    }

    pub(crate) fn to_facts_payload(
        &self,
        policy: &AnalysisDiagnosticPolicy,
    ) -> AnalysisFactsPayload {
        let payload = self.project(policy);
        AnalysisFactsPayload::from_generation(self, &payload)
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
    pub(crate) evidence: Arc<DiagramAnalysisEvidence>,
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

    pub(crate) fn from_document_diagram_with_evidence(
        diagram: &DocumentDiagram,
        syntax: AnalysisSyntaxFacts,
        evidence: Arc<DiagramAnalysisEvidence>,
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
            evidence,
        }
    }

    /// Returns the parser-owned outcome retained by this analysis generation.
    pub fn parse_disposition(&self) -> DiagramParseDisposition {
        self.evidence.parse_disposition()
    }
}

#[derive(Debug)]
pub(crate) enum DiagramAnalysisEvidence {
    SourceLimit,
    EmptySource,
    Panic {
        message: String,
    },
    NoSnapshot,
    OperationError {
        error: Arc<merman_core::Error>,
    },
    Parsed {
        metadata: merman_core::ParseMetadata,
        model: Arc<Value>,
        editor_facts: Option<Arc<merman_core::EditorSemanticFacts>>,
    },
    ParseFailed {
        metadata: merman_core::ParseMetadata,
        error: Arc<merman_core::Error>,
        editor_facts: Option<Arc<merman_core::EditorSemanticFacts>>,
    },
}

impl DiagramAnalysisEvidence {
    fn parse_disposition(&self) -> DiagramParseDisposition {
        match self {
            Self::Parsed { .. } => DiagramParseDisposition::Parsed,
            Self::ParseFailed { .. } => DiagramParseDisposition::Recovered,
            Self::SourceLimit
            | Self::EmptySource
            | Self::Panic { .. }
            | Self::NoSnapshot
            | Self::OperationError { .. } => DiagramParseDisposition::Unavailable,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AnalysisSyntaxFacts {
    pub diagram_type: Option<String>,
    pub effective_layout: Option<String>,
    pub text_index: FenceTextIndex,
    pub flowchart: Option<AnalysisFlowchartFacts>,
}

impl AnalysisSyntaxFacts {
    pub fn new(diagram_type: Option<String>, text_index: FenceTextIndex) -> Self {
        Self {
            diagram_type,
            effective_layout: None,
            text_index,
            flowchart: None,
        }
    }

    pub fn unavailable(diagram_type: Option<String>) -> Self {
        Self {
            text_index: FenceTextIndex::default(),
            diagram_type,
            effective_layout: None,
            flowchart: None,
        }
    }

    pub fn source(&self) -> FenceTextIndexSource {
        self.text_index.source()
    }

    pub fn with_flowchart(mut self, flowchart: Option<AnalysisFlowchartFacts>) -> Self {
        self.flowchart = flowchart;
        self
    }

    pub fn with_effective_layout(mut self, effective_layout: Option<String>) -> Self {
        self.effective_layout = effective_layout;
        self
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
    pub(crate) fn from_generation(
        generation: &AnalysisGeneration,
        payload: &AnalysisPayload,
    ) -> Self {
        Self {
            version: ANALYSIS_FACTS_PAYLOAD_VERSION,
            valid: payload.valid,
            summary: payload.summary,
            source: payload.source.clone(),
            diagnostics: payload.diagnostics.clone(),
            diagrams: generation
                .diagrams
                .iter()
                .map(|diagram| AnalysisDiagramFacts::from_diagram(diagram, &generation.source_map))
                .collect(),
        }
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
    fn from_diagram(diagram: &AnalyzedDiagram, source_map: &SourceMap) -> Self {
        Self {
            source_id: diagram.source_id.clone(),
            index: diagram.index,
            kind: diagram_kind_name(diagram.kind).to_string(),
            source: diagram.source.clone(),
            span: source_map.span(diagram.start, diagram.end).ok(),
            body_span: source_map.span(diagram.body_start, diagram.body_end).ok(),
            text_len: diagram.text.len(),
            fence_delimiter: diagram
                .fence_delimiter
                .map(AnalysisFenceDelimiterFacts::from),
            parse_disposition: diagram.parse_disposition(),
            syntax: AnalysisDiagramSyntaxFacts::from_syntax(
                &diagram.syntax,
                source_map,
                diagram.body_start,
            ),
        }
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flowchart: Option<AnalysisFlowchartFacts>,
    pub node_ids: Vec<String>,
    pub class_names: Vec<String>,
    pub directive_prefixes: Vec<String>,
    pub references: Vec<AnalysisReferenceFacts>,
    pub outline_items: Vec<AnalysisLineItemFacts>,
    pub semantic_items: Vec<AnalysisSemanticItemFacts>,
    pub expected_syntax: Vec<AnalysisExpectedSyntaxFacts>,
}

impl AnalysisDiagramSyntaxFacts {
    fn from_syntax(
        syntax: &AnalysisSyntaxFacts,
        source_map: &SourceMap,
        body_start: usize,
    ) -> Self {
        let text_index = &syntax.text_index;
        let fact_source = text_index.source();

        Self {
            diagram_type: syntax.diagram_type.clone(),
            effective_layout: syntax.effective_layout.clone(),
            fact_source,
            parser_backed: fact_source.is_parser_backed(),
            recovered: fact_source.is_recovered(),
            source_mapped_spans: fact_source.has_source_mapped_spans(),
            flowchart: syntax.flowchart.clone(),
            node_ids: text_index.node_ids().cloned().collect(),
            class_names: text_index.class_names().cloned().collect(),
            directive_prefixes: text_index.directive_prefixes().cloned().collect(),
            references: text_index
                .references()
                .map(|(group, spans)| {
                    AnalysisReferenceFacts::from_reference(group, spans, source_map, body_start)
                })
                .collect(),
            outline_items: text_index
                .outline_items()
                .iter()
                .map(|item| AnalysisLineItemFacts::from_item(item, source_map, body_start))
                .collect(),
            semantic_items: text_index
                .semantic_items()
                .iter()
                .map(|item| AnalysisSemanticItemFacts::from_item(item, source_map, body_start))
                .collect(),
            expected_syntax: text_index
                .expected_syntax()
                .iter()
                .map(|expected| {
                    AnalysisExpectedSyntaxFacts::from_expected(expected, source_map, body_start)
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisFlowchartFacts {
    #[serde(default)]
    pub direction: Option<String>,
    #[serde(default, rename = "classDefs")]
    pub class_defs: BTreeMap<String, Vec<String>>,
    #[serde(default, rename = "edgeDefaults")]
    pub edge_defaults: Option<AnalysisFlowchartEdgeDefaults>,
    #[serde(default, rename = "vertexCalls")]
    pub vertex_calls: Vec<String>,
    #[serde(default)]
    pub nodes: Vec<AnalysisFlowchartNodeFacts>,
    #[serde(default)]
    pub edges: Vec<AnalysisFlowchartEdgeFacts>,
    #[serde(default)]
    pub subgraphs: Vec<AnalysisFlowchartSubgraphFacts>,
    #[serde(default)]
    pub tooltips: BTreeMap<String, String>,
}

impl AnalysisFlowchartFacts {
    #[cfg(test)]
    pub(crate) fn try_from_model(
        model: &Value,
    ) -> Result<Option<Self>, AnalysisFlowchartFactsProjectionError> {
        let cancellation = crate::AnalysisCancellationToken::new();
        Self::try_from_model_cancellable(model, &cancellation)
            .expect("a private analysis cancellation token cannot be cancelled")
    }

    pub(crate) fn try_from_model_cancellable(
        model: &Value,
        cancellation: &crate::AnalysisCancellationToken,
    ) -> Result<Result<Option<Self>, AnalysisFlowchartFactsProjectionError>, crate::AnalysisCancelled>
    {
        cancellation.checkpoint()?;
        let diagram_type = model.get("type").and_then(Value::as_str);
        if !matches!(
            diagram_type,
            Some("flowchart" | "flowchart-v2" | "flowchart-elk")
        ) {
            return Ok(Ok(None));
        }

        let facts: Result<Self, CancellableFlowchartProjectionError> = (|| {
            Ok(Self {
                direction: deserialize_optional_model_field_cancellable(
                    model,
                    "direction",
                    cancellation,
                )?,
                class_defs: deserialize_model_map_cancellable(model, "classDefs", cancellation)?,
                edge_defaults: deserialize_optional_model_field_cancellable(
                    model,
                    "edgeDefaults",
                    cancellation,
                )?,
                vertex_calls: deserialize_model_array_cancellable(
                    model,
                    "vertexCalls",
                    cancellation,
                )?,
                nodes: deserialize_model_array_cancellable(model, "nodes", cancellation)?,
                edges: deserialize_model_array_cancellable(model, "edges", cancellation)?,
                subgraphs: deserialize_model_array_cancellable(model, "subgraphs", cancellation)?,
                tooltips: deserialize_model_map_cancellable(model, "tooltips", cancellation)?,
            })
        })();
        match facts {
            Ok(facts) => {
                cancellation.checkpoint()?;
                Ok(Ok(Some(facts)))
            }
            Err(CancellableFlowchartProjectionError::Cancelled) => Err(crate::AnalysisCancelled),
            Err(CancellableFlowchartProjectionError::Invalid(error)) => {
                cancellation.checkpoint()?;
                Ok(Err(error))
            }
        }
    }
}

fn deserialize_optional_model_field_cancellable<T>(
    model: &Value,
    field: &'static str,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<Option<T>, CancellableFlowchartProjectionError>
where
    T: DeserializeOwned,
{
    match model.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => {
            checkpoint_model_value(value, cancellation)?;
            T::deserialize(value)
                .map(Some)
                .map_err(AnalysisFlowchartFactsProjectionError::from)
                .map_err(CancellableFlowchartProjectionError::Invalid)
        }
    }
}

fn deserialize_model_array_cancellable<T>(
    model: &Value,
    field: &'static str,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<Vec<T>, CancellableFlowchartProjectionError>
where
    T: DeserializeOwned,
{
    let Some(value) = model.get(field) else {
        return Ok(Vec::new());
    };
    let Some(values) = value.as_array() else {
        return Err(CancellableFlowchartProjectionError::Invalid(
            AnalysisFlowchartFactsProjectionError::invalid_field(field, "an array"),
        ));
    };

    let mut projected = Vec::with_capacity(values.len());
    for value in values {
        cancellation.checkpoint()?;
        checkpoint_model_value(value, cancellation)?;
        projected.push(
            T::deserialize(value)
                .map_err(AnalysisFlowchartFactsProjectionError::from)
                .map_err(CancellableFlowchartProjectionError::Invalid)?,
        );
    }
    Ok(projected)
}

fn deserialize_model_map_cancellable<T>(
    model: &Value,
    field: &'static str,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<BTreeMap<String, T>, CancellableFlowchartProjectionError>
where
    T: DeserializeOwned,
{
    let Some(value) = model.get(field) else {
        return Ok(BTreeMap::new());
    };
    let Some(values) = value.as_object() else {
        return Err(CancellableFlowchartProjectionError::Invalid(
            AnalysisFlowchartFactsProjectionError::invalid_field(field, "an object"),
        ));
    };

    let mut projected = BTreeMap::new();
    for (key, value) in values {
        cancellation.checkpoint()?;
        checkpoint_model_value(value, cancellation)?;
        projected.insert(
            key.clone(),
            T::deserialize(value)
                .map_err(AnalysisFlowchartFactsProjectionError::from)
                .map_err(CancellableFlowchartProjectionError::Invalid)?,
        );
    }
    Ok(projected)
}

fn checkpoint_model_value(
    root: &Value,
    cancellation: &crate::AnalysisCancellationToken,
) -> Result<(), CancellableFlowchartProjectionError> {
    let mut stack = vec![root];
    let mut visited = 0usize;
    while let Some(value) = stack.pop() {
        if visited.is_multiple_of(128) {
            cancellation.checkpoint()?;
        }
        visited += 1;
        match value {
            Value::Array(values) => stack.extend(values.iter().rev()),
            Value::Object(values) => stack.extend(values.values()),
            _ => {}
        }
    }
    cancellation.checkpoint()?;
    Ok(())
}

enum CancellableFlowchartProjectionError {
    Cancelled,
    Invalid(AnalysisFlowchartFactsProjectionError),
}

impl From<crate::AnalysisCancelled> for CancellableFlowchartProjectionError {
    fn from(_: crate::AnalysisCancelled) -> Self {
        Self::Cancelled
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AnalysisFlowchartFactsProjectionError {
    message: String,
}

impl fmt::Display for AnalysisFlowchartFactsProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for AnalysisFlowchartFactsProjectionError {}

impl AnalysisFlowchartFactsProjectionError {
    fn invalid_field(field: &str, expected: &str) -> Self {
        Self {
            message: format!("flowchart model field `{field}` must be {expected}"),
        }
    }
}

impl From<serde_json::Error> for AnalysisFlowchartFactsProjectionError {
    fn from(error: serde_json::Error) -> Self {
        Self {
            message: error.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisFlowchartEdgeDefaults {
    #[serde(default)]
    pub interpolate: Option<String>,
    #[serde(default)]
    pub style: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisFlowchartNodeFacts {
    pub id: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default, rename = "labelType")]
    pub label_type: Option<String>,
    #[serde(default, rename = "layoutShape")]
    pub layout_shape: Option<String>,
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisFlowchartEdgeFacts {
    pub id: String,
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default, rename = "labelType")]
    pub label_type: Option<String>,
    #[serde(default, rename = "type")]
    pub edge_type: Option<String>,
    #[serde(default)]
    pub stroke: Option<String>,
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
    #[serde(default)]
    pub length: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisFlowchartSubgraphFacts {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub dir: Option<String>,
    #[serde(default, rename = "labelType")]
    pub label_type: Option<String>,
    #[serde(default)]
    pub classes: Vec<String>,
    #[serde(default)]
    pub styles: Vec<String>,
    #[serde(default)]
    pub nodes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisReferenceFacts {
    pub name: String,
    pub kind: crate::EditorSymbolKind,
    pub spans: Vec<AnalysisFactSpan>,
}

impl AnalysisReferenceFacts {
    fn from_reference(
        group: &FenceReferenceGroup,
        spans: &[crate::ByteSpan],
        source_map: &SourceMap,
        body_start: usize,
    ) -> Self {
        Self {
            name: group.name.clone(),
            kind: group.kind,
            spans: spans
                .iter()
                .copied()
                .map(|span| AnalysisFactSpan::from_local(span, source_map, body_start))
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisLineItemFacts {
    pub name: String,
    pub detail: Option<String>,
    pub kind: crate::EditorSymbolKind,
    pub span: AnalysisFactSpan,
    pub selection: AnalysisFactSpan,
}

impl AnalysisLineItemFacts {
    fn from_item(item: &FenceLineItem, source_map: &SourceMap, body_start: usize) -> Self {
        Self {
            name: item.name.clone(),
            detail: item.detail.clone(),
            kind: item.kind,
            span: AnalysisFactSpan::from_local(item.span, source_map, body_start),
            selection: AnalysisFactSpan::from_local(item.selection, source_map, body_start),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisSemanticItemFacts {
    pub name: String,
    pub detail: Option<String>,
    pub kind: crate::EditorSymbolKind,
    pub role: crate::FenceSemanticRole,
    #[serde(default = "missing_rename_policy")]
    pub rename_policy: crate::FenceRenamePolicy,
    pub span: AnalysisFactSpan,
    pub selection: AnalysisFactSpan,
}

fn missing_rename_policy() -> crate::FenceRenamePolicy {
    crate::FenceRenamePolicy::None
}

impl AnalysisSemanticItemFacts {
    fn from_item(item: &FenceSemanticItem, source_map: &SourceMap, body_start: usize) -> Self {
        Self {
            name: item.name.clone(),
            detail: item.detail.clone(),
            kind: item.kind,
            role: item.role,
            rename_policy: item.rename_policy,
            span: AnalysisFactSpan::from_local(item.span, source_map, body_start),
            selection: AnalysisFactSpan::from_local(item.selection, source_map, body_start),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisExpectedSyntaxFacts {
    pub kind: crate::FenceExpectedSyntaxKind,
    pub span: AnalysisFactSpan,
}

impl AnalysisExpectedSyntaxFacts {
    fn from_expected(
        expected: &FenceExpectedSyntax,
        source_map: &SourceMap,
        body_start: usize,
    ) -> Self {
        Self {
            kind: expected.kind,
            span: AnalysisFactSpan::from_local(expected.span, source_map, body_start),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisFactSpan {
    pub local: crate::ByteSpan,
    pub document: Option<crate::DiagnosticSpan>,
}

impl AnalysisFactSpan {
    fn from_local(local: crate::ByteSpan, source_map: &SourceMap, body_start: usize) -> Self {
        let document_start = body_start.saturating_add(local.start);
        let document_end = body_start.saturating_add(local.end);
        Self {
            local,
            document: source_map.span(document_start, document_end).ok(),
        }
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

    #[test]
    fn flowchart_facts_accept_legacy_flowchart_models() {
        let model = json!({
            "type": "flowchart",
            "direction": "LR",
            "nodes": [
                {
                    "id": "A",
                    "label": "Alpha"
                }
            ],
            "edges": [
                {
                    "id": "L_A_B_0",
                    "from": "A",
                    "to": "B",
                    "length": 1
                }
            ]
        });

        let facts = AnalysisFlowchartFacts::try_from_model(&model)
            .expect("legacy flowchart model should deserialize")
            .expect("legacy flowchart model should produce facts");

        assert_eq!(facts.direction.as_deref(), Some("LR"));
        assert!(
            facts
                .nodes
                .iter()
                .any(|node| node.id == "A" && node.label.as_deref() == Some("Alpha"))
        );
        assert!(
            facts
                .edges
                .iter()
                .any(|edge| edge.from == "A" && edge.to == "B")
        );
    }
}
