use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::{
    AnalyzerConfigurationChange, AnalyzerOptionsPreparation, CachedAnalysisGeneration,
    ConfigurationUpdateOutcome, DEFAULT_LSP_ANALYSIS_CACHE_BUDGET_BYTES,
    DEFAULT_LSP_MAX_DOCUMENT_DIAGRAMS, DEFAULT_LSP_MAX_SOURCE_BYTES,
    DiagnosticProjectionPreparation, DocumentDiagnosticState, DocumentDiscardedSource,
    DocumentResourceLimit, DocumentSyncError, PreparedDocumentText, SemanticTokensState,
    SessionState, SnapshotConfigurationPlan, StoredDocument, TextChangePreparation,
    TextDocumentUpdate, cached_analysis_weight, default_lsp_analysis_options,
};
use crate::session::analysis::request::{
    AnalysisBuildError, AnalysisBuildRequest, DiagnosticProjectionOrigin,
    DiagnosticReprojectionRequest,
};
use crate::session::cache::WeightedReplacement;
use crate::snapshot::{DocumentAnalysisContext, DocumentSnapshot, SnapshotContext};
use merman_analysis::{
    AnalysisCancellationToken, AnalysisOptions, AnalysisPayload, AnalysisResourceLimit,
    AnalysisRuleConfig, AnalysisRuleProfile, Analyzer, DiagnosticSeverity, FenceSemanticRole,
    FenceTextIndexSource, source_limit_diagnostic_span,
};
use merman_editor_core::DocumentKind;
use tower_lsp_server::ls_types::{
    Position, Range, SemanticToken, TextDocumentContentChangeEvent, Uri,
};

static CUSTOM_SESSION_PARSE_CALLS: AtomicUsize = AtomicUsize::new(0);
static CUSTOM_ASYNC_SESSION_PARSE_CALLS: AtomicUsize = AtomicUsize::new(0);
static REPEATED_DIAGNOSTIC_PARSE_CALLS: AtomicUsize = AtomicUsize::new(0);

fn reprojection_for(store: &SessionState, uri: &Uri) -> DiagnosticReprojectionRequest {
    store
        .diagnostic_reprojection_request(uri)
        .expect("expected a lazy diagnostic reprojection")
}

fn custom_session_flowchart_parser(
    _source: &str,
    _metadata: &merman_core::ParseMetadata,
    control: &merman_core::ParseControl,
) -> merman_core::ParseControlResult<merman_core::Result<serde_json::Value>> {
    control.checkpoint()?;
    CUSTOM_SESSION_PARSE_CALLS.fetch_add(1, Ordering::SeqCst);
    Ok(Ok(serde_json::json!({ "warningFacts": [] })))
}

fn custom_async_session_flowchart_parser(
    _source: &str,
    _metadata: &merman_core::ParseMetadata,
    control: &merman_core::ParseControl,
) -> merman_core::ParseControlResult<merman_core::Result<serde_json::Value>> {
    control.checkpoint()?;
    CUSTOM_ASYNC_SESSION_PARSE_CALLS.fetch_add(1, Ordering::SeqCst);
    Ok(Ok(serde_json::json!({ "warningFacts": [] })))
}

fn repeated_diagnostic_flowchart_parser(
    _source: &str,
    _metadata: &merman_core::ParseMetadata,
    control: &merman_core::ParseControl,
) -> merman_core::ParseControlResult<merman_core::Result<serde_json::Value>> {
    control.checkpoint()?;
    REPEATED_DIAGNOSTIC_PARSE_CALLS.fetch_add(1, Ordering::SeqCst);
    Ok(Ok(serde_json::json!({ "warningFacts": [] })))
}

trait PreparedDocumentTextTestExt {
    fn new(text: String) -> Self;
}

impl PreparedDocumentTextTestExt for PreparedDocumentText {
    fn new(text: String) -> Self {
        Self {
            text,
            rejection: None,
        }
    }
}

trait SessionStateTestExt: Sized {
    fn new() -> Self;

    fn with_analyzer_for_tests(analyzer: Analyzer) -> Self;

    fn with_analysis_cache_budget(analysis_cache_budget: usize) -> Self;

    fn begin_analyzer_options(&mut self, options: AnalysisOptions) -> AnalyzerConfigurationChange;

    fn prepare_snapshot_configuration(
        &self,
        next_options: AnalysisOptions,
    ) -> SnapshotConfigurationPlan;

    fn apply_analyzer_options(&mut self, options: AnalysisOptions) -> AnalyzerConfigurationChange;

    fn upsert_text(
        &mut self,
        uri: Uri,
        version: i32,
        text: String,
        kind: DocumentKind,
    ) -> StoredDocument;

    fn open_text(
        &mut self,
        uri: Uri,
        version: i32,
        text: String,
        kind: DocumentKind,
    ) -> StoredDocument;

    fn apply_text_changes(
        &mut self,
        uri: Uri,
        version: i32,
        changes: impl IntoIterator<Item = TextDocumentContentChangeEvent>,
    ) -> TextDocumentUpdate;

    fn upsert(&mut self, uri: Uri, version: i32, text: String) -> Arc<DocumentSnapshot>;

    fn snapshot(&mut self, uri: &Uri) -> Option<Arc<DocumentSnapshot>>;

    fn snapshot_context(&mut self, uri: &Uri) -> Option<SnapshotContext>;
}

impl SessionStateTestExt for SessionState {
    fn new() -> Self {
        Self::with_session_cancellation(AnalysisCancellationToken::new())
    }

    fn with_analyzer_for_tests(analyzer: Analyzer) -> Self {
        Self::with_analyzer_and_cache_budget(
            analyzer,
            AnalysisCancellationToken::new(),
            DEFAULT_LSP_ANALYSIS_CACHE_BUDGET_BYTES,
        )
    }

    fn with_analysis_cache_budget(analysis_cache_budget: usize) -> Self {
        Self::with_session_cancellation_and_cache_budget(
            AnalysisCancellationToken::new(),
            analysis_cache_budget,
        )
    }

    fn begin_analyzer_options(&mut self, options: AnalysisOptions) -> AnalyzerConfigurationChange {
        let request = self.begin_analyzer_configuration_request();
        match self
            .prepare_analyzer_options(request, options)
            .expect("a synchronous analyzer update cannot be superseded")
        {
            AnalyzerOptionsPreparation::Applied(change) => change,
            AnalyzerOptionsPreparation::RequiresSnapshotPreparation(plan) => {
                let batch = plan
                    .prepare()
                    .expect("a synchronous analyzer update cannot be cancelled");
                self.commit_snapshot_configuration(batch)
                    .expect("a synchronous analyzer update cannot become stale")
            }
        }
    }

    fn prepare_snapshot_configuration(
        &self,
        next_options: AnalysisOptions,
    ) -> SnapshotConfigurationPlan {
        self.prepare_snapshot_configuration_for(self.latest_configuration_request, next_options)
    }

    fn apply_analyzer_options(&mut self, options: AnalysisOptions) -> AnalyzerConfigurationChange {
        self.begin_analyzer_options(options)
    }

    fn upsert_text(
        &mut self,
        uri: Uri,
        version: i32,
        text: String,
        kind: DocumentKind,
    ) -> StoredDocument {
        let text = Arc::<str>::from(text);
        let source = super::document_source_descriptor(&uri, kind);
        let document = match self
            .analyzer
            .options()
            .resource_limits()
            .preflight_document(text.as_ref(), &source)
        {
            Some(rejection) => StoredDocument {
                uri: uri.clone(),
                version,
                kind,
                source: super::document_source_from_rejection(Arc::clone(&text), rejection),
            },
            None => StoredDocument::available(uri.clone(), version, kind, text),
        };
        self.upsert_document(uri, document)
    }

    fn open_text(
        &mut self,
        uri: Uri,
        version: i32,
        text: String,
        kind: DocumentKind,
    ) -> StoredDocument {
        self.upsert_text(uri, version, text, kind)
    }

    fn apply_text_changes(
        &mut self,
        uri: Uri,
        version: i32,
        changes: impl IntoIterator<Item = TextDocumentContentChangeEvent>,
    ) -> TextDocumentUpdate {
        match self.capture_text_changes(uri, version, changes) {
            TextChangePreparation::Immediate(update) => update,
            TextChangePreparation::Prepare(plan) => self.commit_prepared_text_changes(
                plan.prepare()
                    .expect("a private text-change token cannot be cancelled"),
            ),
        }
    }

    fn upsert(&mut self, uri: Uri, version: i32, text: String) -> Arc<DocumentSnapshot> {
        let kind = DocumentKind::from_path(uri.path().as_str());
        self.upsert_text(uri.clone(), version, text, kind);
        self.snapshot(&uri)
            .expect("snapshot should exist after inserting document text")
    }

    fn snapshot(&mut self, uri: &Uri) -> Option<Arc<DocumentSnapshot>> {
        self.snapshot_context(uri).map(|context| context.snapshot)
    }

    fn snapshot_context(&mut self, uri: &Uri) -> Option<SnapshotContext> {
        if let Some(cached) = self.cached_snapshot_context_for_uri(uri) {
            return Some(cached);
        }

        let request = self.snapshot_build_request(uri)?;
        let cancellation = AnalysisCancellationToken::new();
        let analysis = match request.build_cancellable(&cancellation) {
            Ok(analysis) => analysis,
            Err(AnalysisBuildError::Rejected(rejection)) => {
                let document = self.documents.get(uri)?.document.clone();
                let text = Arc::clone(
                    document
                        .retained_text()
                        .expect("a rejected build must still have its captured source"),
                );
                let source = super::document_source_from_rejection(text, rejection);
                self.upsert_document(
                    document.uri.clone(),
                    StoredDocument {
                        uri: document.uri,
                        version: document.version,
                        kind: document.kind,
                        source,
                    },
                );
                return None;
            }
            Err(AnalysisBuildError::Cancelled(_)) => {
                unreachable!("a fresh test snapshot cancellation cannot be cancelled")
            }
        };
        commit_built_snapshot_for_test(self, &request, analysis).map(|(context, _)| context)
    }
}

fn project_snapshot_for_test(
    store: &SessionState,
    snapshot: Arc<DocumentSnapshot>,
) -> Arc<DocumentAnalysisContext> {
    Arc::new(
        DocumentAnalysisContext::project_cancellable(
            snapshot,
            store.analyzer.options().diagnostic_policy(),
            store.diagnostic_generation,
            &AnalysisCancellationToken::new(),
        )
        .expect("test diagnostic projection should complete"),
    )
}

fn commit_built_snapshot_for_test(
    store: &mut SessionState,
    request: &AnalysisBuildRequest,
    snapshot: Arc<DocumentSnapshot>,
) -> Option<(SnapshotContext, Arc<DocumentAnalysisContext>)> {
    let snapshot = store.commit_built_snapshot(request, snapshot)?;
    match store.prepare_diagnostic_projection_for_snapshot(
        &snapshot,
        DiagnosticProjectionOrigin::FreshBuild,
    )? {
        DiagnosticProjectionPreparation::Ready(context) => {
            let cached = Arc::clone(store.cached_analysis_generation(request.uri())?);
            Some((context, cached))
        }
        DiagnosticProjectionPreparation::Project(projection) => {
            let projected = project_snapshot_for_test(store, Arc::clone(projection.snapshot()));
            let context = SnapshotContext::with_analysis(
                Arc::clone(&projected.snapshot),
                Arc::clone(&projected.payload),
                projection.snapshot_generation(),
                store.diagnostic_generation,
                projection.document_epoch(),
            );
            let replacement = WeightedReplacement {
                key: projection.uri().clone(),
                value: CachedAnalysisGeneration {
                    context: Arc::clone(&projected),
                    document_epoch: projection.document_epoch(),
                    snapshot_generation: projection.snapshot_generation(),
                },
                weight: cached_analysis_weight(projection.uri(), &projected),
            };
            let cached_matches_generation = store
                .analysis_generations
                .peek(projection.uri())
                .is_some_and(|cached| {
                    cached.document_epoch == projection.document_epoch()
                        && cached.snapshot_generation == projection.snapshot_generation()
                        && cached.context.generation_identity() == projected.generation_identity()
                });
            if cached_matches_generation {
                store
                    .analysis_generations
                    .replace_batch_preserving_recency(vec![replacement]);
            } else if store.analysis_generations.peek(projection.uri()).is_some() {
                return None;
            } else if matches!(projection.origin(), DiagnosticProjectionOrigin::FreshBuild) {
                store.analysis_generations.insert(
                    replacement.key,
                    replacement.value,
                    replacement.weight,
                );
            }
            Some((context, projected))
        }
    }
}

#[test]
fn plain_mermaid_documents_create_single_snapshot_fence() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "flowchart TD\nclassDef highlight fill:#f00\nA-->B\n".to_string(),
    );

    assert_eq!(snapshot.fences().len(), 1);
    assert_eq!(snapshot.fences()[0].body_range().start, 0);
    assert_eq!(
        snapshot.fences()[0].text(),
        "flowchart TD\nclassDef highlight fill:#f00\nA-->B\n"
    );
    assert_eq!(snapshot.fences()[0].diagram_type(), Some("flowchart-v2"));
    assert!(
        snapshot.fences()[0]
            .text_index()
            .has_directive_prefix("classDef")
    );
}

#[test]
fn new_store_uses_lsp_default_source_limit() {
    let store = SessionState::new();

    assert_eq!(
        store.analyzer_options().max_source_bytes(),
        Some(DEFAULT_LSP_MAX_SOURCE_BYTES)
    );
    assert_eq!(
        store.analyzer_options().max_document_diagrams(),
        Some(DEFAULT_LSP_MAX_DOCUMENT_DIAGRAMS)
    );
}

#[test]
fn markdown_diagram_limit_retains_text_without_admitting_a_snapshot() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/limited.md").unwrap();
    let source = concat!(
        "```mermaid\nflowchart TD\nA-->B\n```\n",
        "```mermaid\nsequenceDiagram\nA->>B: hi\n```\n",
    );
    store
        .apply_analyzer_options(default_lsp_analysis_options().with_max_document_diagrams(Some(1)));

    let document = store.open_text(uri.clone(), 1, source.to_string(), DocumentKind::Markdown);

    assert_eq!(document.retained_text().unwrap().as_ref(), source);
    assert!(document.is_analysis_unavailable());
    assert_eq!(
        document.analysis_rejection().unwrap().resource_limit(),
        AnalysisResourceLimit::DocumentDiagrams {
            observed_document_diagrams: 2,
            max_document_diagrams: 1,
        }
    );
    assert_eq!(document.resource_limit(), None);
    assert!(store.snapshot_build_request(&uri).is_none());
    assert!(store.snapshot(&uri).is_none());
}

#[test]
fn ranged_changes_cross_markdown_diagram_limit_in_both_directions() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/ranged-limit.md").unwrap();
    let source = concat!(
        "```mermaid\nflowchart TD\nA-->B\n```\n",
        "```mermaid\nsequenceDiagram\nA->>B: hi\n```\n",
    );
    store
        .apply_analyzer_options(default_lsp_analysis_options().with_max_document_diagrams(Some(1)));
    store.open_text(uri.clone(), 1, source.to_string(), DocumentKind::Markdown);

    let recovered = store.apply_text_changes(
        uri.clone(),
        2,
        [TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(4, 3), Position::new(4, 10))),
            range_length: None,
            text: "text".to_string(),
        }],
    );
    assert_eq!(recovered, TextDocumentUpdate::Applied);
    let document = store.get(&uri).unwrap();
    assert!(!document.is_analysis_unavailable());
    assert!(document.analysis_rejection().is_none());
    assert!(store.snapshot(&uri).is_some());

    let rejected = store.apply_text_changes(
        uri.clone(),
        3,
        [TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(4, 3), Position::new(4, 7))),
            range_length: None,
            text: "mermaid".to_string(),
        }],
    );
    assert_eq!(rejected, TextDocumentUpdate::Applied);
    let document = store.get(&uri).unwrap();
    assert!(document.is_analysis_unavailable());
    assert!(document.analysis_rejection().is_some());
    assert!(!store.has_snapshot(&uri));
}

#[test]
fn configuration_reclassifies_retained_markdown_without_reopening() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/config-limit.md").unwrap();
    let source = concat!(
        "```mermaid\nflowchart TD\nA-->B\n```\n",
        "```mermaid\nsequenceDiagram\nA->>B: hi\n```\n",
    );
    store
        .apply_analyzer_options(default_lsp_analysis_options().with_max_document_diagrams(Some(2)));
    store.open_text(uri.clone(), 1, source.to_string(), DocumentKind::Markdown);
    let original = Arc::clone(store.get(&uri).unwrap().retained_text().unwrap());

    store
        .apply_analyzer_options(default_lsp_analysis_options().with_max_document_diagrams(Some(1)));
    let rejected = store.get(&uri).unwrap();
    assert!(rejected.analysis_rejection().is_some());
    assert!(Arc::ptr_eq(&original, rejected.retained_text().unwrap()));

    store
        .apply_analyzer_options(default_lsp_analysis_options().with_max_document_diagrams(Some(2)));
    let recovered = store.get(&uri).unwrap();
    assert!(!recovered.is_analysis_unavailable());
    assert!(recovered.analysis_rejection().is_none());
    assert!(Arc::ptr_eq(&original, recovered.retained_text().unwrap()));
}

#[test]
fn source_byte_limit_precedes_document_diagram_limit_and_discards_text() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/dual-limit.md").unwrap();
    let source = "```mermaid\nflowchart TD\nA-->B\n```\n";
    store.apply_analyzer_options(
        AnalysisOptions::default()
            .with_max_source_bytes(Some(1))
            .with_max_document_diagrams(Some(0)),
    );

    let document = store.open_text(uri, 1, source.to_string(), DocumentKind::Markdown);

    assert!(document.retained_text().is_none());
    assert!(document.analysis_rejection().is_none());
    assert!(document.resource_limit().is_some());
}

#[test]
fn markdown_documents_create_fences_for_markdown_extensions() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.markdown").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "before\n```mermaid\n%%{init: {\"theme\": \"dark\"}}%%\nflowchart TD\nA-->B\n```\nafter\n"
            .to_string(),
    );

    assert_eq!(snapshot.fences().len(), 1);
    assert!(snapshot.fences()[0].text().contains("flowchart TD"));
    assert!(
        snapshot.fences()[0]
            .text_index()
            .node_ids()
            .any(|id| id == "A")
    );
    assert_eq!(snapshot.fences()[0].diagram_type(), Some("flowchart-v2"));
    assert!(
        snapshot.fences()[0]
            .text_index()
            .has_directive_prefix("init")
    );
}

#[test]
fn markdown_documents_create_multiple_mermaid_fences() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.markdown").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        concat!(
            "before\n",
            "```mermaid\n",
            "flowchart TD\n",
            "A-->B\n",
            "```\n",
            "middle\n",
            "```mermaid\n",
            "sequenceDiagram\n",
            "Alice->>Bob: Hi\n",
            "```\n",
            "after\n",
        )
        .to_string(),
    );

    assert_eq!(snapshot.fences().len(), 2);
    assert_eq!(snapshot.fences()[0].diagram_type(), Some("flowchart-v2"));
    assert_eq!(snapshot.fences()[1].diagram_type(), Some("sequence"));
    assert_eq!(
        snapshot.fences()[0].text_index().source(),
        FenceTextIndexSource::ParserComplete
    );
    assert_eq!(
        snapshot.fences()[1].text_index().source(),
        FenceTextIndexSource::ParserComplete
    );
    assert!(
        snapshot.fences()[0]
            .text_index()
            .node_ids()
            .any(|id| id == "A")
    );
    assert!(
        snapshot.fences()[0]
            .text_index()
            .node_ids()
            .any(|id| id == "B")
    );
    assert!(
        snapshot.fences()[1]
            .text_index()
            .node_ids()
            .any(|id| id == "Alice")
    );
    assert!(
        snapshot.fences()[1]
            .text_index()
            .node_ids()
            .any(|id| id == "Bob")
    );
}

#[test]
fn newer_versions_replace_the_stored_snapshot() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    let first = store.upsert(uri.clone(), 1, "flowchart TD\nA-->B\n".to_string());
    let second = store.upsert(
        uri.clone(),
        2,
        "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
    );

    assert_eq!(first.version(), 1);
    assert_eq!(second.version(), 2);

    let stored = store.get(&uri).unwrap();
    assert_eq!(stored.version, 2);
    assert!(stored.text().unwrap().contains("sequenceDiagram"));
    assert!(!stored.text().unwrap().contains("flowchart TD"));
    let stored = store
        .snapshot(&uri)
        .expect("expected stored snapshot after replacement");
    assert_eq!(stored.fences().len(), 1);
    assert_eq!(stored.fences()[0].diagram_type(), Some("sequence"));
}

#[test]
fn upsert_text_defers_snapshot_until_requested() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    let document = store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    assert_eq!(document.version, 1);
    assert_eq!(document.text().unwrap().as_ref(), "flowchart TD\nA-->B\n");
    assert!(!store.has_snapshot(&uri));

    let snapshot = store
        .snapshot(&uri)
        .expect("expected lazy snapshot for stored document");
    assert!(store.has_snapshot(&uri));
    assert_eq!(snapshot.version(), 1);
    assert_eq!(snapshot.fences()[0].diagram_type(), Some("flowchart-v2"));
}

#[test]
fn upsert_text_limits_oversized_documents_without_retaining_source() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/large.mmd").unwrap();
    let source = "flowchart TD\nA-->B\n".to_string();

    store.apply_analyzer_options(AnalysisOptions::default().with_max_source_bytes(Some(8)));
    let document = store.open_text(uri.clone(), 1, source.clone(), DocumentKind::Diagram);

    assert_eq!(document.version, 1);
    assert!(document.text().is_none());
    assert_eq!(
        document.resource_limit(),
        Some(DocumentResourceLimit {
            source_len: source.len(),
            max_source_bytes: 8,
            span: source_limit_diagnostic_span(&source),
        })
    );
    assert_eq!(document.discarded_source(), None);
    assert!(store.snapshot(&uri).is_none());
}

#[test]
fn prepared_text_limits_oversized_documents_without_scanning_under_the_store_lock() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/large-prepared.mmd").unwrap();
    let source = "flowchart TD\nA-->B\n".to_string();

    store.apply_analyzer_options(AnalysisOptions::default().with_max_source_bytes(Some(8)));
    let cancellation = AnalysisCancellationToken::new();
    let prepared = PreparedDocumentText::new_cancellable(
        source.clone(),
        store.analyzer_options().resource_limits(),
        &super::document_source_descriptor(&uri, DocumentKind::Diagram),
        &cancellation,
    )
    .unwrap();
    let document = store.open_prepared_text(uri, 1, prepared, DocumentKind::Diagram);

    assert!(document.text().is_none());
    assert_eq!(
        document.resource_limit(),
        Some(DocumentResourceLimit {
            source_len: source.len(),
            max_source_bytes: 8,
            span: source_limit_diagnostic_span(&source),
        })
    );
}

#[test]
fn prepared_text_only_projects_a_span_when_the_source_is_oversized() {
    let source = "flowchart TD\nA-->B\n".to_string();
    let cancellation = AnalysisCancellationToken::new();

    let descriptor = super::document_source_descriptor(
        &Uri::from_str("file:///tmp/prepared.mmd").unwrap(),
        DocumentKind::Diagram,
    );
    let admitted = PreparedDocumentText::new_cancellable(
        source.clone(),
        AnalysisOptions::default()
            .with_max_source_bytes(Some(source.len()))
            .resource_limits(),
        &descriptor,
        &cancellation,
    )
    .expect("the admitted source should be prepared");
    assert_eq!(admitted.rejection, None);

    let unlimited = PreparedDocumentText::new_cancellable(
        source.clone(),
        AnalysisOptions::default().resource_limits(),
        &descriptor,
        &cancellation,
    )
    .expect("the unlimited source should be prepared");
    assert_eq!(unlimited.rejection, None);

    let oversized = PreparedDocumentText::new_cancellable(
        source.clone(),
        AnalysisOptions::default()
            .with_max_source_bytes(Some(source.len() - 1))
            .resource_limits(),
        &descriptor,
        &cancellation,
    )
    .expect("the oversized source should be prepared");
    assert_eq!(
        oversized
            .rejection
            .as_ref()
            .and_then(|rejection| rejection.payload().diagnostics[0].span),
        Some(source_limit_diagnostic_span(&source)),
    );
}

#[test]
fn source_limit_reclassification_rejects_a_stale_document_epoch() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/reclassified.mmd").unwrap();
    let source = "flowchart TD\nA-->B\n".to_string();
    store.open_text(uri.clone(), 1, source.clone(), DocumentKind::Diagram);

    let batch = store
        .prepare_snapshot_configuration(
            default_lsp_analysis_options().with_max_source_bytes(Some(8)),
        )
        .prepare()
        .expect("test projection should not be cancelled");
    store.open_text(
        uri.clone(),
        2,
        "flowchart TD\nC-->D\n".to_string(),
        DocumentKind::Diagram,
    );

    assert!(store.commit_snapshot_configuration(batch).is_none());
    let document = store.get(&uri).expect("replacement document should remain");
    assert_eq!(document.version, 2);
    assert_eq!(document.resource_limit(), None);
    assert_eq!(
        store.analyzer_options().max_source_bytes(),
        Some(DEFAULT_LSP_MAX_SOURCE_BYTES)
    );
}

#[test]
fn session_cancellation_aborts_pending_source_limit_projection() {
    let cancellation = AnalysisCancellationToken::new();
    let mut store = SessionState::with_session_cancellation(cancellation.clone());
    let uri = Uri::from_str("file:///tmp/cancelled-reclassification.mmd").unwrap();
    store.open_text(
        uri,
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let plan = store.prepare_snapshot_configuration(
        default_lsp_analysis_options().with_max_source_bytes(Some(8)),
    );

    cancellation.cancel();

    assert!(matches!(
        plan.prepare(),
        Err(merman_analysis::AnalysisCancelled)
    ));
}

#[test]
fn source_limit_reclassification_rejects_a_superseded_configuration_request() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/configuration-order.mmd").unwrap();
    let source = "flowchart TD\nA-->B\n".to_string();
    store.open_text(uri.clone(), 1, source.clone(), DocumentKind::Diagram);

    let older_request = store.begin_analyzer_configuration_request();
    let older_plan = match store
        .prepare_analyzer_options(
            older_request,
            default_lsp_analysis_options().with_max_source_bytes(Some(8)),
        )
        .expect("the first request is current")
    {
        AnalyzerOptionsPreparation::RequiresSnapshotPreparation(plan) => plan,
        AnalyzerOptionsPreparation::Applied(_) => {
            panic!("the lower source limit must require source projection")
        }
    };
    let older_batch = older_plan
        .prepare()
        .expect("test projection should not be cancelled");

    let latest_request = store.begin_analyzer_configuration_request();
    let latest = store
        .prepare_analyzer_options(latest_request, default_lsp_analysis_options())
        .expect("the latest request is current");
    assert!(matches!(
        latest,
        AnalyzerOptionsPreparation::Applied(AnalyzerConfigurationChange::Unchanged)
    ));

    assert!(!store.is_analyzer_configuration_request_current(older_request));
    assert!(store.commit_snapshot_configuration(older_batch).is_none());
    assert_eq!(
        store.analyzer_options().max_source_bytes(),
        Some(DEFAULT_LSP_MAX_SOURCE_BYTES)
    );
    let document = store.get(&uri).expect("latest configuration keeps source");
    assert_eq!(document.text().unwrap().as_ref(), source);
    assert_eq!(document.resource_limit(), None);
}

#[test]
fn full_replacement_recovers_from_resource_limited_document() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/large.mmd").unwrap();

    store.apply_analyzer_options(AnalysisOptions::default().with_max_source_bytes(Some(8)));
    store.open_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    let update = store.apply_text_changes(
        uri.clone(),
        2,
        [TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "A-->B\n".to_string(),
        }],
    );

    assert_eq!(update, TextDocumentUpdate::Applied);
    let stored = store.get(&uri).expect("expected recovered document");
    assert_eq!(stored.version, 2);
    assert_eq!(stored.text().unwrap().as_ref(), "A-->B\n");
    assert_eq!(stored.resource_limit(), None);
    assert_eq!(stored.discarded_source(), None);
}

#[test]
fn ranged_changes_on_resource_limited_documents_keep_lightweight_state() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/large.mmd").unwrap();
    let source = "flowchart TD\nA-->B\n".to_string();

    store.apply_analyzer_options(AnalysisOptions::default().with_max_source_bytes(Some(8)));
    store.open_text(uri.clone(), 1, source.clone(), DocumentKind::Diagram);

    let update = store.apply_text_changes(
        uri.clone(),
        2,
        [TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(1, 0), Position::new(1, 1))),
            range_length: None,
            text: "C".to_string(),
        }],
    );

    assert_eq!(update, TextDocumentUpdate::NeedsFullSync);
    let stored = store.get(&uri).expect("expected limited document");
    assert_eq!(stored.version, 2);
    assert!(stored.text().is_none());
    assert_eq!(stored.resource_limit(), None);
    assert_eq!(stored.discarded_source(), None);
    assert_eq!(
        stored.sync_error_state(),
        Some(DocumentSyncError::FullReplacementRequired {
            source_len: source.len(),
            last_max_source_bytes: 8,
        })
    );
    assert!(store.snapshot(&uri).is_none());
}

#[test]
fn resource_limited_documents_update_limit_when_configuration_still_excludes_them() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/large.mmd").unwrap();
    let source = "flowchart TD\nA-->B\n".to_string();

    store.apply_analyzer_options(AnalysisOptions::default().with_max_source_bytes(Some(8)));
    store.open_text(uri.clone(), 1, source.clone(), DocumentKind::Diagram);
    store.apply_analyzer_options(AnalysisOptions::default().with_max_source_bytes(Some(16)));

    let stored = store.get(&uri).expect("expected limited document");
    assert_eq!(
        stored.resource_limit(),
        Some(DocumentResourceLimit {
            source_len: source.len(),
            max_source_bytes: 16,
            span: source_limit_diagnostic_span(&source),
        })
    );
    assert_eq!(stored.discarded_source(), None);
    assert!(store.snapshot(&uri).is_none());
}

#[test]
fn resource_limited_documents_become_discarded_when_configuration_would_allow_them() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/large.mmd").unwrap();
    let source = "flowchart TD\nA-->B\n".to_string();

    store.apply_analyzer_options(AnalysisOptions::default().with_max_source_bytes(Some(8)));
    store.open_text(uri.clone(), 1, source.clone(), DocumentKind::Diagram);
    store.apply_analyzer_options(AnalysisOptions::default().with_max_source_bytes(Some(64)));

    let stored = store.get(&uri).expect("expected discarded document");
    assert_eq!(stored.resource_limit(), None);
    assert_eq!(
        stored.discarded_source(),
        Some(DocumentDiscardedSource {
            source_len: source.len(),
            previous_max_source_bytes: 8,
            span: source_limit_diagnostic_span(&source),
        })
    );
    assert!(store.snapshot(&uri).is_none());

    let update = store.apply_text_changes(
        uri.clone(),
        2,
        [TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: source.clone(),
        }],
    );

    assert_eq!(update, TextDocumentUpdate::Applied);
    let stored = store.get(&uri).expect("expected recovered document");
    assert_eq!(stored.resource_limit(), None);
    assert_eq!(stored.discarded_source(), None);
    assert_eq!(stored.text().unwrap().as_ref(), source);
}

#[test]
fn upsert_text_invalidates_cached_snapshot() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let first = store
        .snapshot(&uri)
        .expect("expected initial lazy snapshot");
    assert_eq!(first.version(), 1);
    assert!(store.has_snapshot(&uri));

    store.upsert_text(
        uri.clone(),
        2,
        "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
        DocumentKind::Diagram,
    );

    assert!(!store.has_snapshot(&uri));
    let second = store
        .snapshot(&uri)
        .expect("expected refreshed lazy snapshot");
    assert_eq!(second.version(), 2);
    assert_eq!(second.fences()[0].diagram_type(), Some("sequence"));
}

#[test]
fn apply_text_change_rejects_missing_documents() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/missing.mmd").unwrap();

    let update = store.apply_text_changes(
        uri.clone(),
        2,
        [TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
        }],
    );

    assert_eq!(update, TextDocumentUpdate::MissingDocument);
    assert!(store.get(&uri).is_none());
    assert!(!store.has_snapshot(&uri));
}

#[test]
fn apply_text_change_rejects_stale_versions_without_invalidating_current_state() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.open_text(
        uri.clone(),
        3,
        "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
        DocumentKind::Diagram,
    );
    let snapshot = store
        .snapshot(&uri)
        .expect("expected current snapshot before stale edit");
    assert_eq!(snapshot.version(), 3);
    assert!(store.has_snapshot(&uri));

    let update = store.apply_text_changes(
        uri.clone(),
        2,
        [TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "flowchart TD\nA-->B\n".to_string(),
        }],
    );

    assert_eq!(
        update,
        TextDocumentUpdate::StaleVersion {
            current_version: 3,
            attempted_version: 2,
        }
    );
    let stored = store.get(&uri).expect("expected current document");
    assert_eq!(stored.version, 3);
    assert!(stored.text().unwrap().contains("sequenceDiagram"));
    assert!(store.has_snapshot(&uri));
}

#[test]
fn apply_text_changes_applies_lsp_utf16_ranges_in_order() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.open_text(
        uri.clone(),
        1,
        "flowchart TD\nA[🤓]-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    let update = store.apply_text_changes(
        uri.clone(),
        2,
        [
            TextDocumentContentChangeEvent {
                range: Some(Range::new(Position::new(1, 2), Position::new(1, 4))),
                range_length: None,
                text: "C".to_string(),
            },
            TextDocumentContentChangeEvent {
                range: Some(Range::new(Position::new(1, 8), Position::new(1, 8))),
                range_length: None,
                text: "\nC-->D".to_string(),
            },
        ],
    );

    assert_eq!(update, TextDocumentUpdate::Applied);
    let stored = store.get(&uri).expect("expected updated document");
    assert_eq!(stored.version, 2);
    assert_eq!(
        stored.text().unwrap().as_ref(),
        "flowchart TD\nA[C]-->B\nC-->D\n"
    );
}

#[test]
fn prepared_text_changes_cannot_overwrite_a_newer_document_epoch() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/prepared-change-cas.mmd").unwrap();
    store.open_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let TextChangePreparation::Prepare(plan) = store.capture_text_changes(
        uri.clone(),
        2,
        [TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "flowchart TD\nA-->C\n".to_string(),
        }],
    ) else {
        panic!("valid change should require lock-free preparation");
    };

    store.open_text(
        uri.clone(),
        3,
        "sequenceDiagram\nAlice->>Bob: newer\n".to_string(),
        DocumentKind::Diagram,
    );
    let update = store.commit_prepared_text_changes(
        plan.prepare()
            .expect("test text preparation should not be cancelled"),
    );

    assert_eq!(update, TextDocumentUpdate::Superseded);
    let stored = store
        .get(&uri)
        .expect("newer document should remain stored");
    assert_eq!(stored.version, 3);
    assert!(stored.text().unwrap().starts_with("sequenceDiagram"));
}

#[test]
fn session_cancellation_aborts_pending_text_change_preparation() {
    let cancellation = AnalysisCancellationToken::new();
    let mut store = SessionState::with_session_cancellation(cancellation.clone());
    let uri = Uri::from_str("file:///tmp/cancelled-change.mmd").unwrap();
    store.open_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let TextChangePreparation::Prepare(plan) = store.capture_text_changes(
        uri,
        2,
        [TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "flowchart TD\nA-->C\n".to_string(),
        }],
    ) else {
        panic!("valid change should require lock-free preparation");
    };

    cancellation.cancel();

    assert!(matches!(
        plan.prepare(),
        Err(merman_analysis::AnalysisCancelled)
    ));
}

#[test]
fn apply_text_changes_updates_line_index_between_batched_edits() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.open_text(
        uri.clone(),
        1,
        "flowchart TD\r\nA[🤓]\rB\n".to_string(),
        DocumentKind::Diagram,
    );

    let update = store.apply_text_changes(
        uri.clone(),
        2,
        [
            TextDocumentContentChangeEvent {
                range: Some(Range::new(Position::new(1, 2), Position::new(1, 4))),
                range_length: None,
                text: "C\nD".to_string(),
            },
            TextDocumentContentChangeEvent {
                range: Some(Range::new(Position::new(2, 0), Position::new(2, 1))),
                range_length: None,
                text: "E".to_string(),
            },
            TextDocumentContentChangeEvent {
                range: Some(Range::new(
                    Position::new(3, 10_000),
                    Position::new(3, 10_000),
                )),
                range_length: None,
                text: "-->C".to_string(),
            },
        ],
    );

    assert_eq!(update, TextDocumentUpdate::Applied);
    let stored = store.get(&uri).expect("expected updated document");
    assert_eq!(
        stored.text().unwrap().as_ref(),
        "flowchart TD\r\nA[C\nE]\rB-->C\n"
    );
}

#[test]
fn apply_text_changes_allows_nonconsecutive_versions_for_incremental_ranges() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.open_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    let update = store.apply_text_changes(
        uri.clone(),
        3,
        [TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(1, 0), Position::new(1, 1))),
            range_length: None,
            text: "C".to_string(),
        }],
    );

    assert_eq!(update, TextDocumentUpdate::Applied);
    let stored = store.get(&uri).expect("expected updated document");
    assert_eq!(stored.version, 3);
    assert_eq!(stored.text().unwrap().as_ref(), "flowchart TD\nC-->B\n");
}

#[test]
fn apply_text_changes_rejects_empty_change_sets_without_advancing_version() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.open_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    let update = store.apply_text_changes(uri.clone(), 3, []);

    assert_eq!(update, TextDocumentUpdate::EmptyChangeSet);
    let stored = store.get(&uri).expect("expected current document");
    assert_eq!(stored.version, 1);
    assert_eq!(stored.text().unwrap().as_ref(), "flowchart TD\nA-->B\n");
}

#[test]
fn apply_text_changes_allows_skipped_versions_for_full_replacements() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.open_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    let update = store.apply_text_changes(
        uri.clone(),
        4,
        [TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
        }],
    );

    assert_eq!(update, TextDocumentUpdate::Applied);
    let stored = store.get(&uri).expect("expected updated document");
    assert_eq!(stored.version, 4);
    assert_eq!(
        stored.text().unwrap().as_ref(),
        "sequenceDiagram\nAlice->>Bob: Hi\n"
    );
}

#[test]
fn apply_text_changes_marks_document_unsynced_after_invalid_range() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.open_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let snapshot = store
        .snapshot(&uri)
        .expect("expected current snapshot before invalid edit");
    assert_eq!(snapshot.version(), 1);
    assert!(store.has_snapshot(&uri));

    let update = store.apply_text_changes(
        uri.clone(),
        2,
        [TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(20, 0), Position::new(20, 1))),
            range_length: None,
            text: "bad".to_string(),
        }],
    );

    assert_eq!(update, TextDocumentUpdate::InvalidRange);
    let stored = store.get(&uri).expect("expected current document");
    assert_eq!(stored.version, 2);
    assert!(stored.text().is_none());
    assert_eq!(stored.resource_limit(), None);
    assert_eq!(stored.discarded_source(), None);
    assert_eq!(
        stored.sync_error_state(),
        Some(DocumentSyncError::InvalidIncrementalRange)
    );
    assert!(!store.has_snapshot(&uri));
}

#[test]
fn apply_text_changes_marks_document_unsynced_after_reversed_range() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.open_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    let update = store.apply_text_changes(
        uri.clone(),
        2,
        [TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(1, 4), Position::new(1, 2))),
            range_length: None,
            text: "bad".to_string(),
        }],
    );

    assert_eq!(update, TextDocumentUpdate::InvalidRange);
    let stored = store.get(&uri).expect("expected unsynced document");
    assert!(stored.text().is_none());
    assert_eq!(
        stored.sync_error_state(),
        Some(DocumentSyncError::InvalidIncrementalRange)
    );
}

#[test]
fn apply_text_changes_clamps_utf16_positions_past_line_end() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.open_text(
        uri.clone(),
        1,
        "flowchart TD\nA[🤓]-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    let update = store.apply_text_changes(
        uri.clone(),
        2,
        [TextDocumentContentChangeEvent {
            range: Some(Range::new(
                Position::new(1, 10_000),
                Position::new(1, 10_000),
            )),
            range_length: None,
            text: "bad".to_string(),
        }],
    );

    assert_eq!(update, TextDocumentUpdate::Applied);
    let stored = store.get(&uri).expect("expected updated document");
    assert_eq!(stored.version, 2);
    assert_eq!(
        stored.text().unwrap().as_ref(),
        "flowchart TD\nA[🤓]-->Bbad\n"
    );
    assert_eq!(stored.sync_error_state(), None);
}

#[test]
fn full_replacement_recovers_from_unsynced_document_after_invalid_range() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.open_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    assert_eq!(
        store.apply_text_changes(
            uri.clone(),
            2,
            [TextDocumentContentChangeEvent {
                range: Some(Range::new(Position::new(20, 0), Position::new(20, 1))),
                range_length: None,
                text: "bad".to_string(),
            }],
        ),
        TextDocumentUpdate::InvalidRange
    );

    let update = store.apply_text_changes(
        uri.clone(),
        3,
        [TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
        }],
    );

    assert_eq!(update, TextDocumentUpdate::Applied);
    let stored = store.get(&uri).expect("expected recovered document");
    assert_eq!(stored.version, 3);
    assert_eq!(
        stored.text().unwrap().as_ref(),
        "sequenceDiagram\nAlice->>Bob: Hi\n"
    );
    assert_eq!(stored.sync_error_state(), None);
}

#[test]
fn ranged_changes_on_unsynced_documents_keep_lightweight_state() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.open_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    assert_eq!(
        store.apply_text_changes(
            uri.clone(),
            2,
            [TextDocumentContentChangeEvent {
                range: Some(Range::new(Position::new(20, 0), Position::new(20, 1))),
                range_length: None,
                text: "bad".to_string(),
            }],
        ),
        TextDocumentUpdate::InvalidRange
    );

    let update = store.apply_text_changes(
        uri.clone(),
        3,
        [TextDocumentContentChangeEvent {
            range: Some(Range::new(Position::new(1, 0), Position::new(1, 1))),
            range_length: None,
            text: "C".to_string(),
        }],
    );

    assert_eq!(update, TextDocumentUpdate::NeedsFullSync);
    let stored = store.get(&uri).expect("expected unsynced document");
    assert_eq!(stored.version, 3);
    assert!(stored.text().is_none());
    assert_eq!(
        stored.sync_error_state(),
        Some(DocumentSyncError::InvalidIncrementalRange)
    );
}

#[test]
fn full_replacement_later_in_unsynced_batch_recovers_document() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.open_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    assert_eq!(
        store.apply_text_changes(
            uri.clone(),
            2,
            [TextDocumentContentChangeEvent {
                range: Some(Range::new(Position::new(20, 0), Position::new(20, 1))),
                range_length: None,
                text: "bad".to_string(),
            }],
        ),
        TextDocumentUpdate::InvalidRange
    );

    let update = store.apply_text_changes(
        uri.clone(),
        3,
        [
            TextDocumentContentChangeEvent {
                range: Some(Range::new(Position::new(0, 0), Position::new(0, 1))),
                range_length: None,
                text: "ignored".to_string(),
            },
            TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
            },
        ],
    );

    assert_eq!(update, TextDocumentUpdate::Applied);
    let stored = store.get(&uri).expect("expected recovered document");
    assert_eq!(stored.version, 3);
    assert_eq!(
        stored.text().unwrap().as_ref(),
        "sequenceDiagram\nAlice->>Bob: Hi\n"
    );
    assert_eq!(stored.sync_error_state(), None);
}

#[test]
fn full_replacement_later_in_available_batch_ignores_prior_invalid_ranges() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.open_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    let update = store.apply_text_changes(
        uri.clone(),
        2,
        [
            TextDocumentContentChangeEvent {
                range: Some(Range::new(Position::new(20, 0), Position::new(20, 1))),
                range_length: None,
                text: "bad".to_string(),
            },
            TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
            },
        ],
    );

    assert_eq!(update, TextDocumentUpdate::Applied);
    let stored = store.get(&uri).expect("expected replaced document");
    assert_eq!(stored.version, 2);
    assert_eq!(
        stored.text().unwrap().as_ref(),
        "sequenceDiagram\nAlice->>Bob: Hi\n"
    );
    assert_eq!(stored.sync_error_state(), None);
}

#[test]
fn ranged_changes_after_a_full_replacement_apply_to_the_replacement() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.open_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    let update = store.apply_text_changes(
        uri.clone(),
        2,
        [
            TextDocumentContentChangeEvent {
                range: None,
                range_length: None,
                text: "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
            },
            TextDocumentContentChangeEvent {
                range: Some(Range::new(Position::new(1, 13), Position::new(1, 15))),
                range_length: None,
                text: "Hello".to_string(),
            },
        ],
    );

    assert_eq!(update, TextDocumentUpdate::Applied);
    let stored = store.get(&uri).expect("expected updated document");
    assert_eq!(
        stored.text().unwrap().as_ref(),
        "sequenceDiagram\nAlice->>Bob: Hello\n"
    );
}

#[test]
fn stale_snapshot_build_request_is_not_committed_after_text_replacement() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let stale_request = store
        .snapshot_build_request(&uri)
        .expect("uncached document should require analysis");
    let stale_snapshot = stale_request
        .build_cancellable(&AnalysisCancellationToken::new())
        .expect("test source should be accepted");

    store.upsert_text(
        uri.clone(),
        2,
        "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
        DocumentKind::Diagram,
    );

    assert!(commit_built_snapshot_for_test(&mut store, &stale_request, stale_snapshot).is_none());
    assert!(!store.has_snapshot(&uri));

    let current = store
        .snapshot(&uri)
        .expect("current snapshot should build after rejecting stale request");
    assert_eq!(current.version(), 2);
    assert_eq!(current.fences()[0].diagram_type(), Some("sequence"));
}

#[test]
fn initial_lsp_payload_matches_the_sealed_generation_source() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/shared-payload.mmd").unwrap();

    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    store.snapshot(&uri).expect("expected cached snapshot");
    let context = store
        .cached_analysis_generation(&uri)
        .expect("expected cached analysis generation");
    assert_eq!(
        context.payload.source,
        *context.snapshot.analysis_generation().source()
    );
}

#[test]
fn cached_snapshot_build_context_stales_after_text_replacement() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/cached.mmd").unwrap();

    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let cached_snapshot = store.snapshot(&uri).expect("expected cached snapshot");

    let context = store
        .cached_snapshot_context_for_uri(&uri)
        .expect("cached document should reuse its current context");
    assert!(std::sync::Arc::ptr_eq(&context.snapshot, &cached_snapshot));

    store.upsert_text(
        uri,
        2,
        "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
        DocumentKind::Diagram,
    );

    assert!(!store.is_snapshot_context_current(&context));
}

#[test]
fn unchanged_analyzer_update_preserves_context_generations_snapshots_and_tokens() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let snapshot_context = store
        .snapshot_context(&uri)
        .expect("expected initial snapshot context");
    let diagnostic_context = store
        .diagnostic_context(&uri)
        .expect("expected initial diagnostic context");
    assert!(store.set_semantic_tokens_state_if_current(
        &snapshot_context,
        SemanticTokensState::new(Some("tokens-1".to_string()), Vec::new()),
    ));
    assert!(store.set_diagnostic_state_if_current(
        &diagnostic_context,
        DocumentDiagnosticState {
            result_id: "diagnostics-1".to_string(),
            diagnostics: Vec::new(),
        },
    ));

    assert_eq!(
        store.apply_analyzer_options(default_lsp_analysis_options()),
        AnalyzerConfigurationChange::Unchanged
    );

    assert!(store.is_snapshot_context_current(&snapshot_context));
    assert!(store.is_diagnostic_context_current(&diagnostic_context));
    assert!(store.has_snapshot(&uri));
    assert_eq!(
        store.diagnostic_state(&uri).map(|state| state.result_id),
        Some("diagnostics-1".to_string())
    );
    assert_eq!(
        store
            .semantic_tokens_state(&uri)
            .and_then(|state| state.result_id.as_deref()),
        Some("tokens-1")
    );
}

#[test]
fn analyzer_configuration_change_classifies_each_policy_scope() {
    let current = AnalysisOptions::default();
    let diagnostic_only = AnalysisOptions::default().with_rule_config(
        AnalysisRuleConfig::default()
            .with_rule_severity("merman.parse.no_diagram", DiagnosticSeverity::Hint)
            .unwrap(),
    );
    let source_limit = AnalysisOptions::default().with_max_source_bytes(Some(1));
    let runtime_policy =
        AnalysisOptions::default().with_fixed_today(Some("2026-07-02".parse().unwrap()));

    assert_eq!(
        super::analyzer_configuration_change(&current, &current),
        super::AnalyzerConfigurationChange::Unchanged
    );
    assert_eq!(
        super::analyzer_configuration_change(&current, &diagnostic_only),
        super::AnalyzerConfigurationChange::DiagnosticsOnly
    );
    for next in [&source_limit, &runtime_policy] {
        assert_eq!(
            super::analyzer_configuration_change(&current, next),
            super::AnalyzerConfigurationChange::SnapshotAffecting
        );
    }
}

#[test]
fn diagnostic_state_is_bound_to_the_document_epoch() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/diagnostic-state.mmd").unwrap();
    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let context = store
        .diagnostic_context(&uri)
        .expect("expected diagnostic context");
    let state = DocumentDiagnosticState {
        result_id: "result-1".to_string(),
        diagnostics: Vec::new(),
    };

    assert!(store.set_diagnostic_state_if_current(&context, state.clone()));
    assert_eq!(
        store
            .diagnostic_state(&uri)
            .expect("expected cached diagnostics")
            .result_id,
        "result-1"
    );

    store.upsert_text(
        uri.clone(),
        2,
        "flowchart TD\nA-->C\n".to_string(),
        DocumentKind::Diagram,
    );

    assert!(store.diagnostic_state(&uri).is_none());
    assert!(!store.set_diagnostic_state_if_current(&context, state));
}

#[test]
fn no_op_configuration_does_not_supersede_prepared_text_changes() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/no-op-config-text-ticket.mmd").unwrap();
    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let TextChangePreparation::Prepare(plan) = store.capture_text_changes(
        uri.clone(),
        2,
        [TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "flowchart TD\nA-->C\n".to_string(),
        }],
    ) else {
        panic!("expected a prepared text transaction");
    };
    let prepared = plan.prepare().expect("text preparation should succeed");

    let request = store.begin_analyzer_configuration_request();
    let preparation = store
        .prepare_analyzer_options(request, store.analyzer_options().clone())
        .expect("current no-op configuration should be classified");
    assert!(matches!(
        preparation,
        AnalyzerOptionsPreparation::Applied(AnalyzerConfigurationChange::Unchanged)
    ));

    assert_eq!(
        store.commit_prepared_text_changes(prepared),
        TextDocumentUpdate::Applied
    );
    assert_eq!(store.get(&uri).unwrap().version, 2);
}

#[test]
fn applied_configuration_supersedes_prepared_text_changes() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/applied-config-text-ticket.mmd").unwrap();
    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let TextChangePreparation::Prepare(plan) = store.capture_text_changes(
        uri.clone(),
        2,
        [TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "flowchart TD\nA-->C\n".to_string(),
        }],
    ) else {
        panic!("expected a prepared text transaction");
    };
    let prepared = plan.prepare().expect("text preparation should succeed");

    store.apply_analyzer_options(
        default_lsp_analysis_options().with_rule_config(
            AnalysisRuleConfig::default()
                .with_rule_severity("merman.parse.no_diagram", DiagnosticSeverity::Hint)
                .unwrap(),
        ),
    );

    assert_eq!(
        store.commit_prepared_text_changes(prepared),
        TextDocumentUpdate::Superseded
    );
    assert_eq!(store.get(&uri).unwrap().version, 1);
}

#[test]
fn snapshot_update_without_a_source_limit_change_skips_source_projection() {
    let mut store = SessionState::new();
    let request = store.begin_analyzer_configuration_request();
    let preparation = store
        .prepare_analyzer_options(
            request,
            default_lsp_analysis_options().with_fixed_today(Some("2026-07-30".parse().unwrap())),
        )
        .expect("current configuration should be classified");

    let AnalyzerOptionsPreparation::RequiresSnapshotPreparation(plan) = preparation else {
        panic!("snapshot-affecting options must be prepared outside the session lock");
    };
    let batch = plan.prepare().expect("snapshot preparation should succeed");
    assert!(batch.resource_rejections.is_none());
    assert!(store.commit_snapshot_configuration(batch).is_some());
}

#[test]
fn snapshot_update_without_a_source_limit_change_ignores_unrelated_document_writes() {
    let mut store = SessionState::new();
    let request = store.begin_analyzer_configuration_request();
    let preparation = store
        .prepare_analyzer_options(
            request,
            default_lsp_analysis_options().with_fixed_today(Some("2026-07-30".parse().unwrap())),
        )
        .expect("current configuration should be classified");
    let AnalyzerOptionsPreparation::RequiresSnapshotPreparation(plan) = preparation else {
        panic!("snapshot-affecting options must be prepared outside the session lock");
    };
    let batch = plan.prepare().expect("snapshot preparation should succeed");

    store.upsert_text(
        Uri::from_str("file:///tmp/unrelated-during-config.mmd").unwrap(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    assert!(store.commit_snapshot_configuration(batch).is_some());
}

#[test]
fn snapshot_update_rejects_an_environment_replacement_with_the_same_request_id() {
    let mut store = SessionState::new();
    let request = store.begin_analyzer_configuration_request();
    let preparation = store
        .prepare_analyzer_options(
            request,
            default_lsp_analysis_options().with_fixed_today(Some("2026-07-30".parse().unwrap())),
        )
        .expect("current configuration should be classified");
    let AnalyzerOptionsPreparation::RequiresSnapshotPreparation(plan) = preparation else {
        panic!("snapshot-affecting options must be prepared outside the session lock");
    };
    let batch = plan.prepare().expect("snapshot preparation should succeed");
    let replacement =
        Analyzer::with_engine(merman_core::Engine::new(), store.analyzer.options().clone());

    store.replace_analyzer(replacement, None);

    assert!(store.is_analyzer_configuration_request_current(request));
    assert!(
        store.commit_snapshot_configuration(batch).is_none(),
        "environment replacement must invalidate a prepared configuration even when its request id remains current"
    );
}

#[test]
fn snapshot_policy_update_preserves_a_custom_parser_registry() {
    CUSTOM_SESSION_PARSE_CALLS.store(0, Ordering::SeqCst);
    let mut engine = merman_core::Engine::new();
    engine
        .diagram_registry_mut()
        .insert("flowchart-v2", custom_session_flowchart_parser);
    let options = default_lsp_analysis_options();
    let analyzer = Analyzer::with_engine(engine, options.clone());
    let initial_identity = analyzer.environment_identity().clone();
    let mut store = SessionState::with_analyzer_for_tests(analyzer);
    let uri = Uri::from_str("file:///tmp/custom-registry-config.mmd").unwrap();

    store.upsert(uri.clone(), 1, "flowchart TD\nA-->B\n".to_string());
    assert_eq!(CUSTOM_SESSION_PARSE_CALLS.load(Ordering::SeqCst), 1);

    let change = store.apply_analyzer_options(
        options.with_max_source_bytes(Some(DEFAULT_LSP_MAX_SOURCE_BYTES.saturating_add(1))),
    );
    assert!(change.affects_snapshots());
    assert_ne!(store.analyzer_environment_identity(), &initial_identity);

    store.upsert(uri, 2, "flowchart TD\nA-->C\n".to_string());
    assert_eq!(
        CUSTOM_SESSION_PARSE_CALLS.load(Ordering::SeqCst),
        2,
        "snapshot policy derivation must retain the custom parser registry"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn language_session_configuration_preserves_a_custom_parser_registry() {
    CUSTOM_ASYNC_SESSION_PARSE_CALLS.store(0, Ordering::SeqCst);
    let mut engine = merman_core::Engine::new();
    engine
        .diagram_registry_mut()
        .insert("flowchart-v2", custom_async_session_flowchart_parser);
    let options = default_lsp_analysis_options();
    let session = super::LanguageSession::with_analyzer_for_tests(Analyzer::with_engine(
        engine,
        options.clone(),
    ));
    let uri = Uri::from_str("file:///tmp/custom-registry-session.mmd").unwrap();

    assert!(
        session
            .open_document(
                uri.clone(),
                1,
                "flowchart TD\nA-->B\n".to_string(),
                DocumentKind::Diagram,
            )
            .await
    );
    session.structure_snapshot(&uri).await.unwrap();
    assert_eq!(CUSTOM_ASYNC_SESSION_PARSE_CALLS.load(Ordering::SeqCst), 1);

    let change = session
        .update_configuration(
            options.with_max_source_bytes(Some(DEFAULT_LSP_MAX_SOURCE_BYTES.saturating_add(1))),
        )
        .await;
    assert!(change.affects_snapshots());
    session.structure_snapshot(&uri).await.unwrap();
    assert_eq!(
        CUSTOM_ASYNC_SESSION_PARSE_CALLS.load(Ordering::SeqCst),
        2,
        "the typed session configuration path must retain the custom parser registry"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn termination_fences_a_configuration_update_waiting_for_session_state() {
    let options = default_lsp_analysis_options();
    let session = super::LanguageSession::with_cancellation(AnalysisCancellationToken::new());
    let state = session.inner.state.lock().await;
    let before_snapshot_generation = state.snapshot_generation;
    let before_diagnostic_generation = state.diagnostic_generation;
    let before_configuration_revision = state.configuration_revision;
    let before_configuration_request = state.latest_configuration_request;
    let before_documents_revision = state.documents_revision;
    let before_cache_len = state.analysis_cache_len();
    let before_cache_weight = state.analysis_cache_total_weight();
    let before_environment = state.analyzer_environment_identity().clone();

    let update = session.update_configuration(
        options.with_rule_config(
            AnalysisRuleConfig::default()
                .with_rule_severity("merman.parse.no_diagram", DiagnosticSeverity::Hint)
                .unwrap(),
        ),
    );
    tokio::pin!(update);
    assert!(futures::poll!(&mut update).is_pending());

    assert!(session.terminate());
    drop(state);

    assert_eq!(update.await, ConfigurationUpdateOutcome::Cancelled);
    let state = session.inner.state.lock().await;
    assert_eq!(state.snapshot_generation, before_snapshot_generation);
    assert_eq!(state.diagnostic_generation, before_diagnostic_generation);
    assert_eq!(state.configuration_revision, before_configuration_revision);
    assert_eq!(
        state.latest_configuration_request,
        before_configuration_request
    );
    assert_eq!(state.documents_revision, before_documents_revision);
    assert_eq!(state.analysis_cache_len(), before_cache_len);
    assert_eq!(state.analysis_cache_total_weight(), before_cache_weight);
    assert_eq!(state.analyzer_environment_identity(), &before_environment);
}

#[tokio::test]
async fn repeated_diagnostic_updates_preserve_environment_and_parse_once() {
    REPEATED_DIAGNOSTIC_PARSE_CALLS.store(0, Ordering::SeqCst);
    let mut engine = merman_core::Engine::new();
    engine
        .diagram_registry_mut()
        .insert("flowchart-v2", repeated_diagnostic_flowchart_parser);
    let options = default_lsp_analysis_options();
    let analyzer = Analyzer::with_engine(engine, options.clone());
    let identity = analyzer.environment_identity().clone();
    let mut store = SessionState::with_analyzer_for_tests(analyzer);
    let uri = Uri::from_str("file:///tmp/repeated-diagnostic-config.mmd").unwrap();
    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    store
        .snapshot_context(&uri)
        .expect("initial analysis should succeed");
    let generation = store
        .cached_analysis_generation(&uri)
        .expect("initial generation should be cached")
        .shared_analysis_generation();
    assert_eq!(REPEATED_DIAGNOSTIC_PARSE_CALLS.load(Ordering::SeqCst), 1);

    for severity in [DiagnosticSeverity::Hint, DiagnosticSeverity::Warning] {
        let change = store.begin_analyzer_options(
            options.clone().with_rule_config(
                AnalysisRuleConfig::default()
                    .with_rule_severity("merman.parse.no_diagram", severity)
                    .unwrap(),
            ),
        );
        assert_eq!(change, AnalyzerConfigurationChange::DiagnosticsOnly);
        assert_eq!(store.analyzer_environment_identity(), &identity);

        let request = reprojection_for(&store, &uri);
        let projection = store
            .analysis_executor()
            .execute_diagnostic_reprojection(&request)
            .await
            .expect("diagnostic reprojection should succeed");
        store
            .commit_diagnostic_reprojection_context(&projection)
            .expect("current diagnostic reprojection should commit");

        assert!(Arc::ptr_eq(
            &generation,
            &store
                .cached_analysis_generation(&uri)
                .expect("reprojected generation should remain cached")
                .shared_analysis_generation()
        ));
        assert_eq!(REPEATED_DIAGNOSTIC_PARSE_CALLS.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn replacing_an_engine_changes_identity_even_when_options_match() {
    let options = default_lsp_analysis_options();
    let mut first_engine = merman_core::Engine::new();
    first_engine
        .diagram_registry_mut()
        .insert("flowchart-v2", custom_session_flowchart_parser);
    let mut store =
        SessionState::with_analyzer_for_tests(Analyzer::with_engine(first_engine, options.clone()));
    let initial_identity = store.analyzer_environment_identity().clone();

    let mut replacement_engine = merman_core::Engine::new();
    replacement_engine
        .diagram_registry_mut()
        .insert("flowchart-v2", custom_session_flowchart_parser);
    store.replace_analyzer(
        Analyzer::with_engine(replacement_engine, options.clone()),
        None,
    );

    assert_eq!(store.analyzer_options(), &options);
    assert_ne!(store.analyzer_environment_identity(), &initial_identity);
}

#[tokio::test]
async fn diagnostic_only_analyzer_update_reprojects_the_cached_generation() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let snapshot_context = store
        .snapshot_context(&uri)
        .expect("expected initial snapshot context");
    let initial_payload = snapshot_context.analysis_payload() as *const AnalysisPayload;
    let initial_text_index = snapshot_context.snapshot.fences()[0].text_index() as *const _;
    let canonical = store
        .cached_analysis_generation(&uri)
        .expect("expected cached analysis generation")
        .shared_analysis_generation();
    let analyzer_environment_identity = store.analyzer_environment_identity().clone();
    let diagnostic_context = store
        .diagnostic_context(&uri)
        .expect("expected initial diagnostic context");
    assert!(store.set_semantic_tokens_state_if_current(
        &snapshot_context,
        SemanticTokensState::new(Some("tokens-1".to_string()), Vec::new()),
    ));

    let change = store.begin_analyzer_options(
        default_lsp_analysis_options().with_rule_config(
            AnalysisRuleConfig::default()
                .with_rule_severity("merman.parse.no_diagram", DiagnosticSeverity::Hint)
                .unwrap(),
        ),
    );

    assert_eq!(change, AnalyzerConfigurationChange::DiagnosticsOnly);
    assert_eq!(
        store.analyzer_environment_identity(),
        &analyzer_environment_identity
    );
    assert!(store.is_snapshot_context_current(&snapshot_context));
    assert!(!store.is_analysis_context_current(&snapshot_context));
    assert!(!store.is_diagnostic_context_current(&diagnostic_context));
    assert!(store.has_snapshot(&uri));
    assert!(!store.has_analysis_payload(&uri));
    assert!(
        store.snapshot_build_request(&uri).is_none(),
        "a stale diagnostic projection must not trigger another parse"
    );

    let request = reprojection_for(&store, &uri);
    let projection = store
        .analysis_executor()
        .execute_diagnostic_reprojection(&request)
        .await
        .expect("current reprojection should complete");
    let committed = store
        .commit_diagnostic_reprojection_context(&projection)
        .is_some();

    assert!(committed);
    assert!(store.has_analysis_payload(&uri));
    let current_context = store
        .snapshot_context(&uri)
        .expect("expected reprojected analysis context");
    assert!(store.is_analysis_context_current(&current_context));
    assert!(Arc::ptr_eq(
        &snapshot_context.snapshot,
        &current_context.snapshot
    ));
    assert_ne!(
        current_context.analysis_payload() as *const AnalysisPayload,
        initial_payload
    );
    assert_eq!(
        current_context.snapshot.fences()[0].text_index() as *const _,
        initial_text_index
    );
    assert!(Arc::ptr_eq(
        &canonical,
        &store
            .cached_analysis_generation(&uri)
            .expect("expected reprojected analysis generation")
            .shared_analysis_generation()
    ));
    assert_eq!(
        store
            .semantic_tokens_state(&uri)
            .and_then(|state| state.result_id.as_deref()),
        Some("tokens-1")
    );
}

#[tokio::test]
async fn equivalent_reprojection_waiters_share_work_and_commit_idempotently() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/reprojection-waiters.mmd").unwrap();
    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    store
        .snapshot_context(&uri)
        .expect("initial analysis should be cached");
    let _ = store.begin_analyzer_options(
        default_lsp_analysis_options().with_rule_config(
            AnalysisRuleConfig::default()
                .with_rule_severity("merman.parse.no_diagram", DiagnosticSeverity::Hint)
                .unwrap(),
        ),
    );
    let request = reprojection_for(&store, &uri);
    let executor = store.analysis_executor();

    let first = executor
        .execute_diagnostic_reprojection(&request)
        .await
        .unwrap();
    let second = executor
        .execute_diagnostic_reprojection(&request)
        .await
        .unwrap();

    assert!(first.shares_result_with(&second));
    assert_eq!(executor.reprojection_count(), 1);
    let first_context = store
        .commit_diagnostic_reprojection_context(&first)
        .expect("first waiter should commit");
    let second_context = store
        .commit_diagnostic_reprojection_context(&second)
        .expect("second waiter should observe the equivalent commit");
    assert!(Arc::ptr_eq(
        &first_context.snapshot,
        &second_context.snapshot
    ));
    assert_eq!(
        first_context.diagnostic_generation(),
        second_context.diagnostic_generation()
    );
}

#[tokio::test]
async fn diagnostic_reprojection_does_not_overwrite_a_newer_document_epoch() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    store
        .snapshot_context(&uri)
        .expect("expected initial snapshot context");
    let _ = store.begin_analyzer_options(
        default_lsp_analysis_options().with_rule_config(
            AnalysisRuleConfig::default()
                .with_rule_severity("merman.parse.no_diagram", DiagnosticSeverity::Hint)
                .unwrap(),
        ),
    );
    let request = reprojection_for(&store, &uri);
    let projection = store
        .analysis_executor()
        .execute_diagnostic_reprojection(&request)
        .await
        .expect("current reprojection should complete");

    store.upsert_text(
        uri.clone(),
        2,
        "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
        DocumentKind::Diagram,
    );
    let current = store
        .snapshot_context(&uri)
        .expect("replacement analysis should be current");

    assert!(
        store
            .commit_diagnostic_reprojection_context(&projection)
            .is_none()
    );
    assert!(store.has_snapshot(&uri));
    assert!(store.has_analysis_payload(&uri));
    assert!(store.is_analysis_context_current(&current));
    assert_eq!(
        store
            .get(&uri)
            .expect("expected replacement document")
            .version,
        2
    );
}

#[tokio::test]
async fn diagnostic_reprojection_does_not_commit_a_superseded_policy_epoch() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    store
        .snapshot_context(&uri)
        .expect("expected initial snapshot context");
    let _ = store.begin_analyzer_options(
        default_lsp_analysis_options().with_rule_config(
            AnalysisRuleConfig::default()
                .with_rule_severity("merman.parse.no_diagram", DiagnosticSeverity::Hint)
                .unwrap(),
        ),
    );
    let first_request = reprojection_for(&store, &uri);
    let _ = store.begin_analyzer_options(
        default_lsp_analysis_options().with_rule_config(
            AnalysisRuleConfig::default()
                .with_rule_severity("merman.parse.no_diagram", DiagnosticSeverity::Warning)
                .unwrap(),
        ),
    );
    let second_request = reprojection_for(&store, &uri);
    let first_projection = store
        .analysis_executor()
        .execute_diagnostic_reprojection(&first_request)
        .await;
    let second_projection = store
        .analysis_executor()
        .execute_diagnostic_reprojection(&second_request)
        .await
        .expect("current reprojection should complete");

    assert!(matches!(
        first_projection,
        Err(error) if error.is_stale()
    ));
    assert!(!store.has_analysis_payload(&uri));
    assert!(
        store
            .commit_diagnostic_reprojection_context(&second_projection)
            .is_some()
    );
    assert!(store.has_analysis_payload(&uri));
}

#[test]
fn text_replacement_stales_contexts_but_keeps_committed_token_baseline() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let snapshot_context = store
        .snapshot_context(&uri)
        .expect("expected initial snapshot context");
    let diagnostic_context = store
        .diagnostic_context(&uri)
        .expect("expected initial diagnostic context");
    assert!(store.set_semantic_tokens_state_if_current(
        &snapshot_context,
        SemanticTokensState::new(Some("tokens-1".to_string()), Vec::new()),
    ));

    store.upsert_text(
        uri.clone(),
        1,
        "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
        DocumentKind::Diagram,
    );

    assert!(!store.is_snapshot_context_current(&snapshot_context));
    assert!(!store.is_diagnostic_context_current(&diagnostic_context));
    assert!(!store.has_snapshot(&uri));
    assert_eq!(
        store
            .semantic_tokens_state(&uri)
            .and_then(|state| state.result_id.as_deref()),
        Some("tokens-1")
    );
}

#[test]
fn snapshot_affecting_analyzer_update_stales_all_contexts_and_clears_snapshot_state() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let snapshot_context = store
        .snapshot_context(&uri)
        .expect("expected initial snapshot context");
    let diagnostic_context = store
        .diagnostic_context(&uri)
        .expect("expected initial diagnostic context");
    assert!(store.set_semantic_tokens_state_if_current(
        &snapshot_context,
        SemanticTokensState::new(Some("tokens-1".to_string()), Vec::new()),
    ));

    store.apply_analyzer_options(
        AnalysisOptions::default().with_max_source_bytes(Some("flowchart TD\nA-->B\n".len() - 1)),
    );

    assert!(!store.is_snapshot_context_current(&snapshot_context));
    assert!(!store.is_diagnostic_context_current(&diagnostic_context));
    assert!(!store.has_snapshot(&uri));
    assert!(store.semantic_tokens_state(&uri).is_none());
}

#[test]
fn remove_stales_existing_contexts_and_clears_document_state() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let snapshot_context = store
        .snapshot_context(&uri)
        .expect("expected initial snapshot context");
    let diagnostic_context = store
        .diagnostic_context(&uri)
        .expect("expected initial diagnostic context");
    assert!(store.set_semantic_tokens_state_if_current(
        &snapshot_context,
        SemanticTokensState::new(Some("tokens-1".to_string()), Vec::new()),
    ));

    store.remove(&uri);

    assert!(store.get(&uri).is_none());
    assert!(!store.has_snapshot(&uri));
    assert!(store.semantic_tokens_state(&uri).is_none());
    assert!(!store.is_snapshot_context_current(&snapshot_context));
    assert!(!store.is_diagnostic_context_current(&diagnostic_context));
}

#[test]
fn stale_snapshot_context_cannot_record_semantic_token_state_after_text_replacement() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let snapshot_context = store
        .snapshot_context(&uri)
        .expect("expected initial snapshot context");

    store.upsert_text(
        uri.clone(),
        1,
        "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
        DocumentKind::Diagram,
    );

    assert!(!store.set_semantic_tokens_state_if_current(
        &snapshot_context,
        SemanticTokensState::new(Some("tokens-stale".to_string()), Vec::new()),
    ));
    assert!(store.semantic_tokens_state(&uri).is_none());
}

#[test]
fn semantic_token_delta_baseline_survives_text_replacement_but_not_snapshot_config_change() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let snapshot_context = store
        .snapshot_context(&uri)
        .expect("expected initial snapshot context");
    assert!(store.set_semantic_tokens_state_if_current(
        &snapshot_context,
        SemanticTokensState::new(
            Some("tokens-1".to_string()),
            vec![SemanticToken {
                delta_line: 0,
                delta_start: 0,
                length: 4,
                token_type: 0,
                token_modifiers_bitset: 0,
            }],
        ),
    ));

    store.upsert_text(
        uri.clone(),
        2,
        "flowchart TD\nAlpha-->B\n".to_string(),
        DocumentKind::Diagram,
    );

    assert!(
        store
            .semantic_tokens_state_for_delta(&uri, "tokens-1")
            .is_some(),
        "text edits should preserve the last committed token state as a delta baseline"
    );

    store.apply_analyzer_options(
        AnalysisOptions::default()
            .with_max_source_bytes(Some("flowchart TD\nAlpha-->B\n".len() - 1)),
    );

    assert!(
        store
            .semantic_tokens_state_for_delta(&uri, "tokens-1")
            .is_none()
    );
}

#[test]
fn snapshot_affecting_source_limit_update_rejects_the_cached_generation() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();

    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let snapshot = store
        .snapshot(&uri)
        .expect("expected initial lazy snapshot");
    assert_eq!(snapshot.fences()[0].diagram_type(), Some("flowchart-v2"));
    let token_context = store
        .snapshot_context(&uri)
        .expect("expected initial snapshot context");
    assert!(store.set_semantic_tokens_state_if_current(
        &token_context,
        SemanticTokensState::new(Some("tokens-1".to_string()), Vec::new()),
    ));

    store.apply_analyzer_options(
        AnalysisOptions::default().with_max_source_bytes(Some("flowchart TD\nA-->B\n".len() - 1)),
    );

    assert!(!store.has_snapshot(&uri));
    assert!(store.semantic_tokens_state(&uri).is_none());
    let stored = store.get(&uri).expect("expected resource-limited document");
    assert!(stored.text().is_none());
    assert_eq!(
        stored.resource_limit(),
        Some(DocumentResourceLimit {
            source_len: "flowchart TD\nA-->B\n".len(),
            max_source_bytes: "flowchart TD\nA-->B\n".len() - 1,
            span: source_limit_diagnostic_span("flowchart TD\nA-->B\n"),
        })
    );
    assert!(store.snapshot_build_request(&uri).is_none());
    assert!(store.snapshot(&uri).is_none());
}

#[test]
fn incomplete_flowchart_documents_use_recovered_parser_facts() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "flowchart TD\nsubgraph group\nA-->B\nC-->".to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(index.source(), FenceTextIndexSource::ParserRecovered);
    assert!(index.node_ids().any(|id| id == "A"));
    assert!(index.node_ids().any(|id| id == "C"));
    assert!(
        index
            .outline_items()
            .iter()
            .any(|item| item.name == "group")
    );
}

#[test]
fn sequence_documents_use_parser_facts() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "sequenceDiagram\nparticipant Alice\nactor Bob\nAlice->>Bob: Hi\n".to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.node_ids().any(|id| id == "Alice"));
    assert!(index.node_ids().any(|id| id == "Bob"));
}

#[test]
fn sequence_payload_facts_do_not_pollute_completion_ids() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        concat!(
            "sequenceDiagram\n",
            "title: Diagram Title\n",
            "accTitle: Accessible Title\n",
            "accDescr: Accessible Description\n",
            "participant Alice\n",
            "actor Bob\n",
            "Alice->>Bob: Hello\n",
            "Note over Alice,Bob: Review\n",
            "details Alice: {\"owner\": \"platform\"}\n",
            "links Alice: { \"Repo\": \"https://example.com/\" }\n",
            "link Alice: Endpoint @ https://alice.example.com\n",
            "properties Alice: {\"class\": \"internal-service-actor\"}\n",
        )
        .to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.node_ids().any(|id| id == "Alice"));
    assert!(index.node_ids().any(|id| id == "Bob"));
    assert!(
        index
            .semantic_items()
            .iter()
            .any(|item| item.name == "Alice" && item.role == FenceSemanticRole::Entity)
    );
    assert!(
        index
            .semantic_items()
            .iter()
            .any(|item| item.name == "Bob" && item.role == FenceSemanticRole::Entity)
    );
    for payload in [
        "Diagram Title",
        "Accessible Title",
        "Accessible Description",
        "Hello",
        "Review",
        r#"{"owner": "platform"}"#,
        r#"{ "Repo": "https://example.com/" }"#,
        "Endpoint @ https://alice.example.com",
        r#"{"class": "internal-service-actor"}"#,
    ] {
        assert!(
            index
                .semantic_items()
                .iter()
                .any(|item| item.name == payload && item.role == FenceSemanticRole::Payload),
            "sequence payload {payload:?} was not retained as a semantic item"
        );
    }

    for leaked in [
        "Diagram Title",
        "Accessible Title",
        "Accessible Description",
        "Hello",
        "Review",
        r#"{"owner": "platform"}"#,
        "Repo",
        "Endpoint",
        "https://example.com/",
        "https://alice.example.com",
        "internal-service-actor",
    ] {
        assert!(
            !index.node_ids().any(|id| id == leaked),
            "sequence payload leaked {leaked:?} into completion ids"
        );
        assert!(
            !index.outline_items().iter().any(|item| item.name == leaked),
            "sequence payload leaked {leaked:?} into outline items"
        );
    }

    for prefix in [
        "title",
        "accTitle",
        "accDescr",
        "details",
        "links",
        "link",
        "properties",
    ] {
        assert!(index.has_directive_prefix(prefix));
    }
}

#[test]
fn architecture_documents_use_parser_facts() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        concat!(
            "architecture-beta\n",
            "group platform(cloud)[Platform]\n",
            "service api(server)[API] in platform\n",
            "junction hub in platform\n",
            "api:R -- L:hub\n",
        )
        .to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(snapshot.fences()[0].diagram_type(), Some("architecture"));
    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    for id in ["platform", "api", "hub"] {
        assert!(index.node_ids().any(|candidate| candidate == id));
    }
    assert!(index.outline_items().iter().any(
        |item| item.name == "platform" && item.detail.as_deref() == Some("architecture group")
    ));
    assert!(
        index
            .semantic_items()
            .iter()
            .any(|item| item.name == "Platform" && item.role == FenceSemanticRole::Payload)
    );
    assert!(
        index
            .semantic_items()
            .iter()
            .any(|item| item.name == "API" && item.role == FenceSemanticRole::Payload)
    );
}

#[test]
fn radar_documents_use_parser_facts() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        concat!(
            "radar-beta\n",
            "title Radar diagram\n",
            "accTitle: Radar accTitle\n",
            "accDescr: Radar accDescription\n",
            "axis A[\"Axis A\"], B[\"Axis B\"], C[\"Axis C\"]\n",
            "curve mycurve[\"My Curve\"]{1,2,3}\n",
        )
        .to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(snapshot.fences()[0].diagram_type(), Some("radar"));
    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    for id in ["A", "B", "C", "mycurve"] {
        assert!(index.node_ids().any(|candidate| candidate == id));
    }
    assert!(
        index
            .semantic_items()
            .iter()
            .any(|item| item.name == "Axis A" && item.role == FenceSemanticRole::Payload)
    );
}

#[test]
fn treemap_documents_use_parser_facts() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        concat!(
            "treemap\n",
            "title Treemap Title\n",
            "accTitle: Treemap accTitle\n",
            "accDescr: Treemap accDescr\n",
            "\"Root\"\n",
            "  \"Leaf\": 42 :::highlight\n",
            "classDef highlight fill:#f00\n",
        )
        .to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(snapshot.fences()[0].diagram_type(), Some("treemap"));
    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.node_ids().any(|candidate| candidate == "Root"));
    assert!(index.node_ids().any(|candidate| candidate == "Leaf"));
    assert!(
        index
            .semantic_items()
            .iter()
            .any(|item| item.name == "highlight" && item.role == FenceSemanticRole::Outline)
    );
    assert!(
        index
            .semantic_items()
            .iter()
            .any(|item| item.name == "42" && item.role == FenceSemanticRole::Payload)
    );
}

#[test]
fn block_documents_use_parser_facts() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        concat!(
            "block\n",
            "  columns 2\n",
            "  block:group[\"Group label\"]\n",
            "    A[\"Start\"] -- \"edge label\" --> B[\"End\"]\n",
            "  end\n",
            "  arrow<[\"go\"]>(right, down)\n",
            "  classDef hot fill:#f00\n",
            "  class A,B hot\n",
            "  style B stroke:#333\n",
        )
        .to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(snapshot.fences()[0].diagram_type(), Some("block"));
    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    for id in ["group", "A", "B", "arrow"] {
        assert!(index.node_ids().any(|candidate| candidate == id));
    }
    assert!(
        index
            .semantic_items()
            .iter()
            .any(|item| item.name == "hot" && item.role == FenceSemanticRole::Outline)
    );
    for payload in ["Group label", "Start", "edge label", "End", "go", "right"] {
        assert!(
            index
                .semantic_items()
                .iter()
                .any(|item| item.name == payload && item.role == FenceSemanticRole::Payload),
            "missing block payload semantic item {payload:?}"
        );
    }
}

#[test]
fn c4_documents_use_parser_facts() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        concat!(
            "C4Context\n",
            "title Banking Context\n",
            "accTitle: Banking accessibility title\n",
            "accDescr: Banking accessibility description\n",
            "Boundary(bank, \"Bank\") {\n",
            "Person(customer, \"Customer\", \"Uses the system\")\n",
            "System(system, \"Internet Banking\", \"Core system\")\n",
            "}\n",
            "Rel(customer, system, \"Uses\", \"HTTPS\")\n",
            "UpdateElementStyle(system, $bgColor=\"red\")\n",
            "UpdateRelStyle(customer, system, $lineColor=\"blue\")\n",
        )
        .to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(snapshot.fences()[0].diagram_type(), Some("c4"));
    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    for id in ["bank", "customer", "system"] {
        assert!(index.node_ids().any(|candidate| candidate == id));
    }
    for prefix in ["title", "accTitle", "accDescr"] {
        assert!(index.has_directive_prefix(prefix));
    }
    for payload in [
        "Banking Context",
        "Banking accessibility title",
        "Banking accessibility description",
        "Bank",
        "Customer",
        "Uses the system",
        "Internet Banking",
        "Core system",
        "Uses",
        "HTTPS",
        "red",
        "blue",
    ] {
        assert!(
            index
                .semantic_items()
                .iter()
                .any(|item| item.name == payload && item.role == FenceSemanticRole::Payload),
            "missing C4 payload semantic item {payload:?}"
        );
        assert!(!index.node_ids().any(|candidate| candidate == payload));
    }
}

#[test]
fn zenuml_documents_use_parser_facts() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        concat!(
            "zenuml\n",
            "title Login Flow\n",
            "Alice\n",
            "Bob\n",
            "A as API\n",
            "accTitle: Login accessibility title\n",
            "accDescr: Login accessibility description\n",
            "Alice->Bob: Login\n",
            "SomeType result = A.SyncMessage()\n",
            "new Session(with, params)\n",
        )
        .to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(snapshot.fences()[0].diagram_type(), Some("zenuml"));
    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    for id in ["Alice", "Bob", "A", "accTitle", "accDescr", "Session"] {
        assert!(index.node_ids().any(|candidate| candidate == id));
    }
    assert!(index.has_directive_prefix("title"));
    assert!(!index.has_directive_prefix("accTitle"));
    assert!(!index.has_directive_prefix("accDescr"));
    for payload in [
        "Login Flow",
        "Login accessibility title",
        "Login accessibility description",
        "API",
        "Login",
        "SyncMessage",
        "result",
    ] {
        assert!(
            index
                .semantic_items()
                .iter()
                .any(|item| item.name == payload && item.role == FenceSemanticRole::Payload),
            "missing ZenUML payload semantic item {payload:?}"
        );
        assert!(!index.node_ids().any(|candidate| candidate == payload));
    }
}

#[test]
fn newer_family_documents_keep_parser_facts_when_recovered() {
    for case in [
        (
            "gitGraph",
            concat!("gitGraph\n", "commit id:\"C1\"\n", "commit id:\"broken\n",),
            "C1",
            FenceSemanticRole::Entity,
        ),
        (
            "radar",
            concat!(
                "radar-beta\n",
                "axis A[\"Axis A\"], B[\"Axis B\"]\n",
                "curve mycurve{1,2}\n",
                "curve broken\n",
            ),
            "A",
            FenceSemanticRole::Entity,
        ),
        (
            "kanban",
            concat!(
                "kanban\n",
                "    root\n",
                "      child1\n",
                "      broken[unfinished\n",
            ),
            "child1",
            FenceSemanticRole::Entity,
        ),
        (
            "treemap",
            concat!(
                "treemap\n",
                "\"Root\"\n",
                "  \"Leaf\": 42\n",
                "\"Broken\":\n",
            ),
            "Leaf",
            FenceSemanticRole::Entity,
        ),
        (
            "block",
            concat!(
                "block\n",
                "  block:group[\"Group label\"]\n",
                "    A[\"Start\"]\n",
            ),
            "A",
            FenceSemanticRole::Entity,
        ),
        (
            "c4",
            concat!(
                "C4Context\n",
                "Person(customer, \"Customer\")\n",
                "NotAMacro customer\n",
            ),
            "customer",
            FenceSemanticRole::Entity,
        ),
        (
            "wardley",
            concat!(
                "wardley-beta\n",
                "component API [0.6, 0.7]\n",
                "component Broken [\n",
            ),
            "API",
            FenceSemanticRole::Entity,
        ),
        (
            "zenuml",
            concat!(
                "zenuml\n",
                "Alice\n",
                "Unsupported ? statement\n",
                "Alice->Bob: Hi\n",
            ),
            "Alice",
            FenceSemanticRole::Entity,
        ),
    ] {
        let mut store = SessionState::new();
        let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
        let snapshot = store.upsert(uri, 1, case.1.to_string());
        let index = snapshot.fences()[0].text_index();

        assert_eq!(
            index.source(),
            FenceTextIndexSource::ParserRecovered,
            "unexpected recovered provenance for {}",
            case.0
        );
        assert!(
            index
                .semantic_items()
                .iter()
                .any(|item| item.name == case.2 && item.role == case.3),
            "missing recovered semantic item {:?} for {}",
            case.2,
            case.0
        );
    }
}

#[test]
fn incomplete_sequence_documents_use_recovered_parser_facts() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "sequenceDiagram\nAlice->>Bob: Hi\nBob->>".to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(index.source(), FenceTextIndexSource::ParserRecovered);
    assert!(index.node_ids().any(|id| id == "Alice"));
    assert!(index.node_ids().any(|id| id == "Bob"));
}

#[test]
fn state_documents_use_parser_facts() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        concat!(
            "stateDiagram-v2\n",
            "[*] --> Idle\n",
            "Idle --> Running\n",
            "Idle: Waiting state\n",
            "Idle --> Running: starts\n",
            "state \"Paused State\" as Paused\n",
            "note right of Running : Running details\n",
            "note \"Floating note\" as note1\n",
            "classDef activeStyle fill:#0f0,border:#333\n",
            "class Idle, Running activeStyle\n",
            "style Running fill:#f00\n",
            "accTitle: Lifecycle chart\n",
            "accDescr: Shows state transitions\n",
            "click Running \"https://example.com/run\" \"Run details\"\n",
        )
        .to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.node_ids().any(|id| id == "Idle"));
    assert!(index.node_ids().any(|id| id == "Running"));
    assert!(!index.node_ids().any(|id| id == "activeStyle"));
    assert!(!index.node_ids().any(|id| id == "Waiting state"));
    assert!(!index.node_ids().any(|id| id == "starts"));
    assert!(!index.node_ids().any(|id| id == "Paused State"));
    assert!(!index.node_ids().any(|id| id == "Running details"));
    assert!(!index.node_ids().any(|id| id == "Floating note"));
    assert!(!index.node_ids().any(|id| id == "note1"));
    assert!(!index.node_ids().any(|id| id == "fill:#0f0,border:#333"));
    assert!(!index.node_ids().any(|id| id == "fill:#f00"));
    assert!(!index.node_ids().any(|id| id == "Lifecycle chart"));
    assert!(!index.node_ids().any(|id| id == "Shows state transitions"));
    assert!(!index.node_ids().any(|id| id == "https://example.com/run"));
    assert!(!index.node_ids().any(|id| id == "Run details"));
    assert!(
        index
            .outline_items()
            .iter()
            .any(|item| item.name == "activeStyle"
                && item.detail.as_deref() == Some("state class definition"))
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "Waiting state")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "starts")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "Paused State")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "Running details")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "Lifecycle chart")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "https://example.com/run")
    );
}

#[test]
fn incomplete_state_documents_use_recovered_parser_facts() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "stateDiagram-v2\nIdle --> Running\nRunning -->".to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(index.source(), FenceTextIndexSource::ParserRecovered);
    assert!(index.node_ids().any(|id| id == "Idle"));
    assert!(index.node_ids().any(|id| id == "Running"));
}

#[test]
fn class_documents_use_parser_facts() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "classDiagram\nclass User\nUser <|-- Admin\n".to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.node_ids().any(|id| id == "User"));
    assert!(index.node_ids().any(|id| id == "Admin"));
}

#[test]
fn incomplete_class_documents_use_recovered_parser_facts() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(uri, 1, "classDiagram\nclass User\nUser <|--".to_string());
    let index = snapshot.fences()[0].text_index();

    assert_eq!(index.source(), FenceTextIndexSource::ParserRecovered);
    assert!(index.node_ids().any(|id| id == "User"));
}

#[test]
fn class_member_outline_facts_do_not_pollute_completion_ids() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        concat!(
            "classDiagram\n",
            "class User {\n",
            "  +login()\n",
            "  -password: String\n",
            "}\n",
            "class Visible[\"Visible label\"]\n",
            "<<interface>> User\n",
            "User: email\n",
            "Class1 \"1\" *-- \"many\" Class02 : contains\n",
            "User <|-- Admin : manages\n",
            "note for User \"Primary user\"\n",
            "note \"Floating note\"\n",
            "click User href \"https://example.com\" \"Open user\" _blank\n",
            "click User call open(userId) \"Open user\"\n",
            "accTitle: Class chart\n",
            "accDescr: Shows class relationships\n",
            "classDef service fill:#eee\n",
            "class User:::service\n",
            "cssClass \"User,Admin\" service\n",
            "style User fill:#fff\n",
        )
        .to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.has_directive_prefix("classDef"));
    assert!(index.has_directive_prefix("class"));
    assert!(index.has_directive_prefix("cssClass"));
    assert!(index.has_directive_prefix("style"));
    assert!(index.has_directive_prefix("click"));
    assert!(index.node_ids().any(|id| id == "User"));
    assert!(!index.node_ids().any(|id| id == "+login()"));
    assert!(!index.node_ids().any(|id| id == "-password: String"));
    assert!(!index.node_ids().any(|id| id == "email"));
    assert!(!index.node_ids().any(|id| id == "interface"));
    assert!(!index.node_ids().any(|id| id == "https://example.com"));
    assert!(!index.node_ids().any(|id| id == "Open user"));
    assert!(!index.node_ids().any(|id| id == "_blank"));
    assert!(!index.node_ids().any(|id| id == "service"));
    assert!(!index.node_ids().any(|id| id == "fill:#eee"));
    assert!(!index.node_ids().any(|id| id == "fill:#fff"));
    assert!(!index.node_ids().any(|id| id == "open"));
    assert!(!index.node_ids().any(|id| id == "userId"));
    assert!(!index.node_ids().any(|id| id == "Visible label"));
    assert!(!index.node_ids().any(|id| id == "1"));
    assert!(!index.node_ids().any(|id| id == "many"));
    assert!(!index.node_ids().any(|id| id == "manages"));
    assert!(!index.node_ids().any(|id| id == "Primary user"));
    assert!(!index.node_ids().any(|id| id == "Floating note"));
    assert!(!index.node_ids().any(|id| id == "Class chart"));
    assert!(!index.node_ids().any(|id| id == "Shows class relationships"));

    assert!(
        index
            .outline_items()
            .iter()
            .any(|item| item.name == "+login()" && item.detail.as_deref() == Some("class member"))
    );
    assert!(
        index
            .outline_items()
            .iter()
            .any(|item| item.name == "-password: String"
                && item.detail.as_deref() == Some("class member"))
    );
    assert!(
        index
            .outline_items()
            .iter()
            .any(|item| item.name == "email" && item.detail.as_deref() == Some("class member"))
    );
    assert!(
        index.outline_items().iter().any(
            |item| item.name == "service" && item.detail.as_deref() == Some("class definition")
        )
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "interface")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "https://example.com")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "Open user")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "_blank")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "fill:#eee")
    );
    assert!(!index.outline_items().iter().any(|item| item.name == "open"));
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "userId")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "manages")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "Visible label")
    );
    assert!(!index.outline_items().iter().any(|item| item.name == "1"));
    assert!(!index.outline_items().iter().any(|item| item.name == "many"));
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "Primary user")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "Class chart")
    );
}

#[test]
fn er_documents_use_parser_facts() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "erDiagram\nCUSTOMER ||--o{ ORDER : places\n".to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.node_ids().any(|id| id == "CUSTOMER"));
    assert!(index.node_ids().any(|id| id == "ORDER"));
}

#[test]
fn incomplete_er_documents_use_recovered_parser_facts() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(uri, 1, "erDiagram\nCUSTOMER ||--o{ ORDER :".to_string());
    let index = snapshot.fences()[0].text_index();

    assert_eq!(index.source(), FenceTextIndexSource::ParserRecovered);
    assert!(index.node_ids().any(|id| id == "CUSTOMER"));
    assert!(index.node_ids().any(|id| id == "ORDER"));
}

#[test]
fn er_attribute_payload_facts_do_not_pollute_completion_ids() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        concat!(
            "erDiagram\n",
            "BOOK {\n",
            "  string title PK, FK \"primary title\"\n",
            "}\n",
        )
        .to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.node_ids().any(|id| id == "BOOK"));
    assert!(!index.node_ids().any(|id| id == "title"));
    assert!(!index.node_ids().any(|id| id == "string"));
    assert!(!index.node_ids().any(|id| id == "PK"));
    assert!(!index.node_ids().any(|id| id == "FK"));
    assert!(!index.node_ids().any(|id| id == "primary title"));
    assert!(
        index
            .outline_items()
            .iter()
            .any(|item| item.name == "title" && item.detail.as_deref() == Some("er attribute"))
    );
}

#[test]
fn gantt_documents_use_parser_facts() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        concat!(
            "gantt\n",
            "title Roadmap\n",
            "accTitle: Roadmap chart\n",
            "accDescr: Shows release tasks\n",
            "accDescr {\n",
            "  Shows release tasks\n",
            "  across releases\n",
            "}\n",
            "dateFormat YYYY-MM-DD\n",
            "section Demo\n",
            "Task 1: id1,2014-01-01,1d\n",
            "Task 2: id2,after id1,2d\n",
            "click id2 call open(userId) href \"https://example.com/\"\n",
        )
        .to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(snapshot.fences()[0].diagram_type(), Some("gantt"));
    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.node_ids().any(|id| id == "id1"));
    assert!(index.node_ids().any(|id| id == "id2"));
    assert!(!index.node_ids().any(|id| id == "Demo"));
    assert!(!index.node_ids().any(|id| id == "Roadmap"));
    assert!(!index.node_ids().any(|id| id == "Roadmap chart"));
    assert!(!index.node_ids().any(|id| id == "Shows release tasks"));
    assert!(
        !index
            .node_ids()
            .any(|id| id == "Shows release tasks\n  across releases")
    );
    assert!(!index.node_ids().any(|id| id == "YYYY-MM-DD"));
    assert!(!index.node_ids().any(|id| id == "open"));
    assert!(!index.node_ids().any(|id| id == "userId"));
    assert!(!index.node_ids().any(|id| id == "https://example.com/"));
    assert!(
        index
            .outline_items()
            .iter()
            .any(|item| item.name == "Demo" && item.detail.as_deref() == Some("gantt section"))
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "Roadmap")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "Roadmap chart")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "Shows release tasks")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "Shows release tasks\n  across releases")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "YYYY-MM-DD")
    );
    assert!(!index.outline_items().iter().any(|item| item.name == "open"));
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "userId")
    );
    assert!(
        !index
            .outline_items()
            .iter()
            .any(|item| item.name == "https://example.com/")
    );
    assert!(index.has_directive_prefix("title"));
    assert!(index.has_directive_prefix("accTitle"));
    assert!(index.has_directive_prefix("accDescr"));
    assert!(index.has_directive_prefix("dateFormat"));
    assert!(index.has_directive_prefix("section"));
    assert!(index.has_directive_prefix("click"));
}

#[test]
fn incomplete_gantt_documents_use_recovered_parser_facts() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        "gantt\ndateFormat YYYY-MM-DD\nTask 1: id1,2014-01-01,1d\nTask 2".to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(index.source(), FenceTextIndexSource::ParserRecovered);
    assert!(index.node_ids().any(|id| id == "id1"));
    assert!(!index.node_ids().any(|id| id == "Task"));
}

#[test]
fn mindmap_documents_use_parser_facts() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(
        uri,
        1,
        concat!(
            "mindmap\n",
            "root(Root Node)\n",
            " child1(Child 1)\n",
            " :::hot\n",
            " ::icon(bomb)\n",
            " child2\n",
        )
        .to_string(),
    );
    let index = snapshot.fences()[0].text_index();

    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.node_ids().any(|id| id == "root"));
    assert!(index.node_ids().any(|id| id == "child1"));
    assert!(index.node_ids().any(|id| id == "child2"));
    assert!(!index.node_ids().any(|id| id == "hot"));
    assert!(!index.node_ids().any(|id| id == "bomb"));
    assert!(!index.outline_items().iter().any(|item| item.name == "hot"));
    assert!(!index.outline_items().iter().any(|item| item.name == "bomb"));
}

#[test]
fn incomplete_mindmap_documents_use_recovered_parser_facts() {
    let mut store = SessionState::new();
    let uri = Uri::from_str("file:///tmp/example.mmd").unwrap();
    let snapshot = store.upsert(uri, 1, "mindmap\nroot\n child[unterminated".to_string());
    let index = snapshot.fences()[0].text_index();

    assert_eq!(index.source(), FenceTextIndexSource::ParserRecovered);
    assert!(index.node_ids().any(|id| id == "root"));
    assert!(!index.node_ids().any(|id| id == "child"));
}

fn estimated_entry_weight(uri: &Uri, text: &str, kind: DocumentKind) -> usize {
    let mut sizing = SessionState::new();
    sizing.upsert_text(uri.clone(), 1, text.to_string(), kind);
    let request = sizing
        .snapshot_build_request(uri)
        .expect("sizing document should require analysis");
    let analysis = request
        .build_cancellable(&AnalysisCancellationToken::new())
        .expect("sizing analysis should be accepted");
    let context = project_snapshot_for_test(&sizing, analysis);
    SessionState::estimated_analysis_cache_entry_weight(uri, &context)
}

fn assert_default_cache_admits(uri: Uri, text: String, kind: DocumentKind) {
    let mut store = SessionState::new();
    store.upsert_text(uri.clone(), 1, text, kind);
    let request = store.snapshot_build_request(&uri).unwrap();
    let analysis = request
        .build_cancellable(&AnalysisCancellationToken::new())
        .unwrap();
    let context = project_snapshot_for_test(&store, Arc::clone(&analysis));
    let weight = SessionState::estimated_analysis_cache_entry_weight(&uri, &context);
    assert!(weight <= DEFAULT_LSP_ANALYSIS_CACHE_BUDGET_BYTES);
    assert!(commit_built_snapshot_for_test(&mut store, &request, analysis).is_some());
    assert_eq!(store.analysis_cache_len(), 1);
    assert_eq!(store.analysis_cache_total_weight(), weight);
    let statistics = store.analysis_cache_statistics();
    assert_eq!(statistics.current_weight, weight);
    assert_eq!(statistics.high_water_weight, weight);
    assert_eq!(statistics.oversized_entries, 0);
}

#[test]
fn weighted_analysis_cache_touches_and_evicts_complete_generations() {
    let a = Uri::from_str("file:///tmp/a.mmd").unwrap();
    let b = Uri::from_str("file:///tmp/b.mmd").unwrap();
    let c = Uri::from_str("file:///tmp/c.mmd").unwrap();
    let text = "flowchart TD\nA-->B\n";
    let entry_weight = estimated_entry_weight(&a, text, DocumentKind::Diagram);
    let budget = entry_weight.checked_mul(2).expect("test cache budget fits");
    let mut store = SessionState::with_analysis_cache_budget(budget);

    for uri in [&a, &b] {
        store.upsert_text(uri.clone(), 1, text.to_string(), DocumentKind::Diagram);
        store
            .snapshot(uri)
            .expect("entry should fit the test cache");
    }
    assert_eq!(store.analysis_cache_len(), 2);
    assert_eq!(store.analysis_cache_total_weight(), budget);
    store
        .snapshot(&a)
        .expect("touch should return the cached entry");

    store.upsert_text(c.clone(), 1, text.to_string(), DocumentKind::Diagram);
    store
        .snapshot(&c)
        .expect("third analysis remains request-local");

    assert!(store.has_snapshot(&a));
    assert!(!store.has_snapshot(&b));
    assert!(store.has_snapshot(&c));
    assert_eq!(store.analysis_cache_len(), 2);
    assert_eq!(store.analysis_cache_total_weight(), budget);
    assert_eq!(store.analysis_cache_statistics().evictions, 1);
}

#[test]
fn oversized_analysis_is_returned_current_without_disturbing_cache_invariants() {
    let uri = Uri::from_str("file:///tmp/a.mmd").unwrap();
    let text = "flowchart TD\nA-->B\n";
    let entry_weight = estimated_entry_weight(&uri, text, DocumentKind::Diagram);
    let mut store = SessionState::with_analysis_cache_budget(entry_weight - 1);
    store.upsert_text(uri.clone(), 1, text.to_string(), DocumentKind::Diagram);
    let request = store
        .snapshot_build_request(&uri)
        .expect("uncached document should require analysis");
    let analysis = request
        .build_cancellable(&AnalysisCancellationToken::new())
        .expect("analysis should succeed");
    let weak = Arc::downgrade(&analysis);

    let (context, projected) =
        commit_built_snapshot_for_test(&mut store, &request, Arc::clone(&analysis))
            .expect("oversized current analysis remains request-local");
    drop(projected);
    drop(analysis);

    assert!(store.is_analysis_context_current(&context));
    assert_eq!(store.analysis_cache_len(), 0);
    assert_eq!(store.analysis_cache_total_weight(), 0);
    let statistics = store.analysis_cache_statistics();
    assert_eq!(statistics.oversized_entries, 1);
    assert_eq!(statistics.current_weight, 0);
    assert_eq!(statistics.high_water_weight, 0);
    assert!(store.get(&uri).is_some());
    assert!(weak.upgrade().is_some());
    drop(context);
    assert!(weak.upgrade().is_none());
}

#[test]
fn cancelled_and_stale_builds_do_not_change_cache_weight_or_eviction_order() {
    let uri = Uri::from_str("file:///tmp/a.mmd").unwrap();
    let mut store = SessionState::new();
    store.upsert_text(
        uri.clone(),
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    let request = store.snapshot_build_request(&uri).unwrap();
    let before = (
        store.analysis_cache_total_weight(),
        store.analysis_cache_len(),
        store.analysis_cache_statistics().evictions,
    );
    let cancellation = AnalysisCancellationToken::new();
    cancellation.cancel();
    assert!(request.build_cancellable(&cancellation).is_err());
    assert_eq!(
        (
            store.analysis_cache_total_weight(),
            store.analysis_cache_len(),
            store.analysis_cache_statistics().evictions,
        ),
        before
    );

    let stale_analysis = request
        .build_cancellable(&AnalysisCancellationToken::new())
        .unwrap();
    store.upsert_text(
        uri.clone(),
        2,
        "flowchart TD\nA-->C\n".to_string(),
        DocumentKind::Diagram,
    );
    assert!(commit_built_snapshot_for_test(&mut store, &request, stale_analysis).is_none());
    assert_eq!(
        (
            store.analysis_cache_total_weight(),
            store.analysis_cache_len(),
            store.analysis_cache_statistics().evictions,
        ),
        before
    );
}

#[test]
fn eviction_releases_cache_ownership_but_preserves_request_local_generation() {
    let a = Uri::from_str("file:///tmp/a.mmd").unwrap();
    let b = Uri::from_str("file:///tmp/b.mmd").unwrap();
    let text = "flowchart TD\nA-->B\n";
    let entry_weight = estimated_entry_weight(&a, text, DocumentKind::Diagram);
    let mut store = SessionState::with_analysis_cache_budget(entry_weight);
    store.upsert_text(a.clone(), 1, text.to_string(), DocumentKind::Diagram);
    let request = store.snapshot_build_request(&a).unwrap();
    let analysis = request
        .build_cancellable(&AnalysisCancellationToken::new())
        .unwrap();
    let weak_snapshot = Arc::downgrade(&analysis);
    let generation = analysis.shared_analysis_generation();
    let weak_generation = Arc::downgrade(&generation);
    let (request_local, projected) =
        commit_built_snapshot_for_test(&mut store, &request, Arc::clone(&analysis)).unwrap();
    let weak_payload = Arc::downgrade(&projected.payload);
    drop(projected);
    drop(generation);
    drop(analysis);

    store.upsert_text(b.clone(), 1, text.to_string(), DocumentKind::Diagram);
    store.snapshot(&b).unwrap();

    assert!(weak_snapshot.upgrade().is_some());
    assert!(weak_generation.upgrade().is_some());
    assert!(weak_payload.upgrade().is_some());
    assert!(store.is_analysis_context_current(&request_local));
    drop(request_local);
    assert!(weak_snapshot.upgrade().is_none());
    assert!(weak_generation.upgrade().is_none());
    assert!(weak_payload.upgrade().is_none());
}

#[test]
fn eviction_keeps_open_document_diagnostic_and_token_identity() {
    let a = Uri::from_str("file:///tmp/a.mmd").unwrap();
    let b = Uri::from_str("file:///tmp/b.mmd").unwrap();
    let text = "flowchart TD\nA-->B\n";
    let entry_weight = estimated_entry_weight(&a, text, DocumentKind::Diagram);
    let mut store = SessionState::with_analysis_cache_budget(entry_weight);
    store.upsert_text(a.clone(), 1, text.to_string(), DocumentKind::Diagram);
    let snapshot = store.snapshot_context(&a).unwrap();
    assert!(store.set_semantic_tokens_state_if_current(
        &snapshot,
        SemanticTokensState::new(Some("tokens-a".to_string()), Vec::new()),
    ));
    let diagnostic = store
        .diagnostic_contexts()
        .into_iter()
        .find(|context| context.document.uri == a)
        .unwrap();
    assert!(store.set_diagnostic_state_if_current(
        &diagnostic,
        DocumentDiagnosticState {
            result_id: "diagnostics-a".to_string(),
            diagnostics: Vec::new(),
        },
    ));

    store.upsert_text(b.clone(), 1, text.to_string(), DocumentKind::Diagram);
    store.snapshot(&b).unwrap();

    assert!(!store.has_snapshot(&a));
    assert!(store.get(&a).is_some());
    assert_eq!(
        store.diagnostic_state(&a).unwrap().result_id,
        "diagnostics-a"
    );
    assert_eq!(
        store
            .semantic_tokens_state(&a)
            .and_then(|state| state.result_id.as_deref()),
        Some("tokens-a")
    );
}

#[tokio::test]
async fn larger_diagnostic_reprojection_can_be_current_without_being_cached() {
    let uri = Uri::from_str("file:///tmp/a.mmd").unwrap();
    let text = "%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
    let entry_weight = estimated_entry_weight(&uri, text, DocumentKind::Diagram);
    let mut store = SessionState::with_analysis_cache_budget(entry_weight);
    store.upsert_text(uri.clone(), 1, text.to_string(), DocumentKind::Diagram);
    store.snapshot_context(&uri).unwrap();
    let _ = store.begin_analyzer_options(default_lsp_analysis_options().with_rule_config(
        AnalysisRuleConfig::default().with_profile(AnalysisRuleProfile::Recommended),
    ));
    let request = reprojection_for(&store, &uri);
    let executor = store.analysis_executor();
    let first = executor
        .execute_diagnostic_reprojection(&request)
        .await
        .unwrap();
    let second = executor
        .execute_diagnostic_reprojection(&request)
        .await
        .unwrap();
    assert!(first.shares_result_with(&second));
    assert_eq!(executor.reprojection_count(), 1);

    let first_context = store
        .commit_diagnostic_reprojection_context(&first)
        .expect("valid projected context must be returned even when oversized");
    let second_context = store
        .commit_diagnostic_reprojection_context(&second)
        .expect("an equivalent waiter must receive the current uncached result");

    assert!(store.is_analysis_context_current(&first_context));
    assert!(store.is_analysis_context_current(&second_context));
    assert!(!first_context.analysis_payload().diagnostics.is_empty());
    assert_eq!(store.analysis_cache_len(), 0);
    assert_eq!(store.analysis_cache_total_weight(), 0);
    assert!(store.get(&uri).is_some());
}

#[test]
fn default_cache_admits_representative_source_limit_boundary_documents() {
    let flow_uri = Uri::from_str("file:///tmp/boundary.mmd").unwrap();
    let flow_prefix = "flowchart TD\nA-->B\n%% ";
    let flow = format!(
        "{flow_prefix}{}",
        "x".repeat(DEFAULT_LSP_MAX_SOURCE_BYTES - flow_prefix.len())
    );
    assert_default_cache_admits(flow_uri, flow, DocumentKind::Diagram);

    let markdown_uri = Uri::from_str("file:///tmp/boundary.md").unwrap();
    let markdown_suffix = "\n```mermaid\nflowchart TD\nA-->B\n```\n";
    let markdown = format!(
        "{}{}",
        "x".repeat(DEFAULT_LSP_MAX_SOURCE_BYTES - markdown_suffix.len()),
        markdown_suffix
    );
    assert_default_cache_admits(markdown_uri, markdown, DocumentKind::Markdown);

    let high_fact_uri = Uri::from_str("file:///tmp/high-fact.mmd").unwrap();
    let mut high_fact = (0..4096)
        .map(|index| format!("N{index}[Node {index}] --> N{}\n", index + 1))
        .fold("flowchart TD\n".to_string(), |mut source, line| {
            source.push_str(&line);
            source
        });
    high_fact.push_str("%% ");
    high_fact.extend(std::iter::repeat_n(
        'x',
        DEFAULT_LSP_MAX_SOURCE_BYTES - high_fact.len(),
    ));
    assert_eq!(high_fact.len(), DEFAULT_LSP_MAX_SOURCE_BYTES);
    assert_default_cache_admits(high_fact_uri, high_fact, DocumentKind::Diagram);
}

#[test]
fn open_commit_is_uri_local_and_rejects_changed_target_or_configuration_state() {
    let mut state = SessionState::new();
    let uri = Uri::from_str("file:///tmp/open-ticket.mmd").unwrap();
    let other = Uri::from_str("file:///tmp/other.mmd").unwrap();
    let ticket = state.capture_open_document(uri.clone());
    state.upsert_text(
        other,
        1,
        "flowchart TD\nA-->B\n".to_string(),
        DocumentKind::Diagram,
    );
    assert!(state.commit_open_document(
        ticket,
        1,
        PreparedDocumentText::new("flowchart TD\nA-->B\n".to_string()),
        DocumentKind::Diagram,
    ));
    assert_eq!(state.get(&uri).unwrap().version, 1);

    let ticket = state.capture_open_document(uri.clone());
    state.upsert_text(
        uri.clone(),
        2,
        "flowchart TD\nA-->C\n".to_string(),
        DocumentKind::Diagram,
    );
    assert!(!state.commit_open_document(
        ticket,
        3,
        PreparedDocumentText::new("flowchart TD\nA-->D\n".to_string()),
        DocumentKind::Diagram,
    ));
    assert_eq!(state.get(&uri).unwrap().version, 2);

    let ticket = state.capture_open_document(uri.clone());
    state.apply_analyzer_options(
        default_lsp_analysis_options().with_rule_config(
            AnalysisRuleConfig::default()
                .with_rule_severity("merman.parse.no_diagram", DiagnosticSeverity::Hint)
                .unwrap(),
        ),
    );
    assert!(!state.commit_open_document(
        ticket,
        3,
        PreparedDocumentText::new("flowchart TD\nA-->D\n".to_string()),
        DocumentKind::Diagram,
    ));
    assert_eq!(state.get(&uri).unwrap().version, 2);
}

#[test]
fn open_commit_rejects_an_absent_present_absent_aba() {
    let mut state = SessionState::new();
    let uri = Uri::from_str("file:///tmp/open-ticket-aba.mmd").unwrap();
    let ticket = state.capture_open_document(uri.clone());

    state.open_prepared_text(
        uri.clone(),
        1,
        PreparedDocumentText::new("flowchart TD\nA-->B\n".to_string()),
        DocumentKind::Diagram,
    );
    state.remove(&uri);
    assert!(state.get(&uri).is_none());

    assert!(!state.commit_open_document(
        ticket,
        2,
        PreparedDocumentText::new("flowchart TD\nA-->C\n".to_string()),
        DocumentKind::Diagram,
    ));
    assert!(state.get(&uri).is_none());
    assert!(
        crate::sync::lock_recovering_poison(&state.open_document_tracker.entries).is_empty(),
        "completed open tickets must not leave URI tombstones"
    );
}

#[test]
fn open_tickets_share_a_uri_clock_until_the_last_ticket_finishes() {
    let mut state = SessionState::new();
    let uri = Uri::from_str("file:///tmp/open-ticket-overlap.mmd").unwrap();
    let first = state.capture_open_document(uri.clone());
    let second = state.capture_open_document(uri.clone());

    assert_eq!(
        crate::sync::lock_recovering_poison(&state.open_document_tracker.entries)
            .get(&uri)
            .map(|clock| clock.active_tickets),
        Some(2)
    );
    drop(first);
    assert_eq!(
        crate::sync::lock_recovering_poison(&state.open_document_tracker.entries)
            .get(&uri)
            .map(|clock| clock.active_tickets),
        Some(1),
        "dropping one ticket must not erase another ticket's URI clock"
    );

    assert!(state.commit_open_document(
        second,
        1,
        PreparedDocumentText::new("flowchart TD\nA-->B\n".to_string()),
        DocumentKind::Diagram,
    ));
    assert!(
        crate::sync::lock_recovering_poison(&state.open_document_tracker.entries).is_empty(),
        "the last completed ticket must release its URI clock"
    );
}

#[test]
fn dropping_an_uncommitted_open_ticket_releases_its_uri_clock() {
    let state = SessionState::new();
    let uri = Uri::from_str("file:///tmp/open-ticket-dropped.mmd").unwrap();
    let ticket = state.capture_open_document(uri);

    drop(ticket);

    assert!(crate::sync::lock_recovering_poison(&state.open_document_tracker.entries).is_empty());
}
