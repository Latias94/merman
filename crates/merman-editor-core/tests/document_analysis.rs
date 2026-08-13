mod support;

use merman_analysis::{
    AnalysisCaptureOutcome, AnalysisOptions, AnalysisRuleConfig, Analyzer, DiagnosticSeverity,
    DiagramParseDisposition, FenceMarker, FenceTextIndexSource, SourceKind,
    analyze_document_generation_shared, source_descriptor_for_kind,
};
use merman_editor_core::{
    DiagramDetectionValidity, DocumentKind, DocumentSnapshot, DocumentSnapshotError, DocumentUri,
    Position, analyze_document_context_with_shared_text,
    analyze_document_snapshot_with_shared_text,
};
use std::sync::Arc;
use support::SnapshotHarness;

#[test]
fn plain_mermaid_documents_create_single_snapshot_fence() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.mmd",
            1,
            "flowchart TD\nclassDef highlight fill:#f00\nA-->B\n".to_string(),
            DocumentKind::Diagram,
        )
        .expect("source is within the analysis limit");

    assert_eq!(snapshot.uri().as_str(), "file:///tmp/example.mmd");
    assert_eq!(snapshot.fences().len(), 1);
    assert_eq!(snapshot.source().kind, SourceKind::Diagram);
    assert_eq!(snapshot.fences()[0].source_id(), "document");
    assert_eq!(snapshot.fences()[0].body_range().start, 0);
    assert_eq!(snapshot.fences()[0].body_range().end, snapshot.text().len());
    assert_eq!(snapshot.fences()[0].source().kind, SourceKind::Diagram);
    assert_eq!(snapshot.fences()[0].diagram_type(), Some("flowchart-v2"));
    assert!(
        snapshot.fences()[0]
            .text_index()
            .has_directive_prefix("classDef")
    );
}

#[test]
fn ready_snapshot_keeps_text_source_map_and_fence_ranges_coherent() {
    let source = "before\n```mermaid\nflowchart TD\nA-->B\n```\nafter\n";
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.md",
            3,
            source.to_string(),
            DocumentKind::Markdown,
        )
        .expect("source is within the analysis limit");

    assert_eq!(snapshot.source_map().source(), snapshot.text());
    for fence in snapshot.fences() {
        let document_range = fence.document_range();
        let body_range = fence.body_range();
        assert!(document_range.start <= body_range.start);
        assert!(body_range.start <= body_range.end);
        assert!(body_range.end <= document_range.end);
        assert_eq!(fence.text(), &snapshot.text()[body_range]);
    }
}

#[test]
fn recovered_flowchart_keeps_available_detection_in_the_shared_analysis_bundle() {
    let analyzed = analyze_document_context_with_shared_text(
        &Analyzer::new(),
        "file:///tmp/incomplete.mmd",
        4,
        Arc::from("flowchart TD\nA[unterminated\n"),
        DocumentKind::Diagram,
    )
    .expect("source is within the analysis limit");
    let detection = analyzed
        .detection()
        .expect("recovery should preserve trusted diagram identity");

    assert!(!analyzed.payload().valid);
    assert_eq!(
        detection.validity,
        DiagramDetectionValidity::RecoverableInvalid
    );
    assert_eq!(detection.diagram_type, "flowchart");
    assert_eq!(detection.syntax_id, "flowchart-v2");
    assert_eq!(detection.effective_layout_id, "dagre");
}

#[test]
fn diagram_detection_validity_is_independent_from_diagnostic_severity() {
    let parsed_source = concat!(
        "cynefin-beta\n",
        "  complex\n",
        "  complicated\n",
        "  complicated --> complicated : \"Self-loop\"\n",
    );
    let recovered_source = "flowchart TD\nA[unterminated\n";

    for severity in [
        DiagnosticSeverity::Error,
        DiagnosticSeverity::Warning,
        DiagnosticSeverity::Info,
        DiagnosticSeverity::Hint,
    ] {
        let parsed = analysis_context_with_rule_severity(
            parsed_source,
            "merman.parse.recovered_editor_facts",
            severity,
        );
        assert_eq!(
            parsed.analysis_generation().diagrams()[0].parse_disposition(),
            DiagramParseDisposition::Parsed
        );
        assert_eq!(
            parsed.detection().expect("parsed detection").validity,
            DiagramDetectionValidity::Valid,
            "parsed detection changed for {severity:?}"
        );

        let recovered = analysis_context_with_rule_severity(
            recovered_source,
            "merman.parse.diagram_parse",
            severity,
        );
        assert_eq!(
            recovered.analysis_generation().diagrams()[0].parse_disposition(),
            DiagramParseDisposition::Recovered
        );
        assert_eq!(
            recovered.detection().expect("recovered detection").validity,
            DiagramDetectionValidity::RecoverableInvalid,
            "recovered detection changed for {severity:?}"
        );
    }
}

fn analysis_context_with_rule_severity(
    source: &str,
    rule_id: &str,
    severity: DiagnosticSeverity,
) -> merman_editor_core::DocumentAnalysisContext {
    let analyzer = Analyzer::with_options(
        AnalysisOptions::default().with_rule_config(
            AnalysisRuleConfig::default()
                .with_rule_severity(rule_id, severity)
                .unwrap(),
        ),
    );
    analyze_document_context_with_shared_text(
        &analyzer,
        "file:///tmp/disposition.mmd",
        1,
        Arc::from(source),
        DocumentKind::Diagram,
    )
    .expect("source should remain within the analysis limit")
}

#[test]
fn cloned_snapshots_share_immutable_text_buffers() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.mmd",
            1,
            "flowchart TD\nA-->B\n".to_string(),
            DocumentKind::Diagram,
        )
        .expect("source is within the analysis limit");
    let cloned = snapshot.clone();

    assert!(Arc::ptr_eq(snapshot.shared_text(), cloned.shared_text()));
    let snapshot_fence_text = snapshot.fences()[0].shared_text().source_arc();
    let cloned_fence_text = cloned.fences()[0].shared_text().source_arc();
    assert!(Arc::ptr_eq(&snapshot_fence_text, &cloned_fence_text));
    assert!(Arc::ptr_eq(snapshot.shared_text(), &snapshot_fence_text));
}

#[test]
fn markdown_documents_create_multiple_fence_local_snapshots() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.markdown",
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
            DocumentKind::Markdown,
        )
        .expect("source is within the analysis limit");

    assert_eq!(snapshot.fences().len(), 2);
    assert_eq!(snapshot.source().kind, SourceKind::Markdown);
    assert_eq!(snapshot.fences()[0].source_id(), "mermaid-fence-1");
    assert_eq!(snapshot.fences()[1].source_id(), "mermaid-fence-2");
    assert_eq!(snapshot.fences()[0].source().diagram_index, Some(0));
    assert_eq!(snapshot.fences()[1].source().diagram_index, Some(1));
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
        snapshot.fences()[1]
            .text_index()
            .node_ids()
            .any(|id| id == "Alice")
    );
}

#[test]
fn markdown_fences_are_generation_views_in_stable_generation_order() {
    let source: Arc<str> = Arc::from(concat!(
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
    ));
    let analyzed = analyze_document_context_with_shared_text(
        &Analyzer::new(),
        "file:///tmp/shared.md",
        3,
        Arc::clone(&source),
        DocumentKind::Markdown,
    )
    .expect("source is within the analysis limit");
    let snapshot = analyzed.snapshot();
    let generation = analyzed.analysis_generation();

    assert!(std::ptr::eq(snapshot.source_map(), generation.source_map()));
    assert!(Arc::ptr_eq(snapshot.shared_text(), &source));
    assert_eq!(snapshot.fences().len(), generation.diagrams().len());
    for (fence, diagram) in snapshot.fences().iter().zip(generation.diagrams()) {
        assert_eq!(fence.source_id(), diagram.source_id());
        assert_eq!(fence.document_range(), diagram.document_range());
        assert_eq!(fence.body_range(), diagram.body_range());
        assert!(std::ptr::eq(
            fence.text_index(),
            &diagram.syntax().text_index
        ));
        assert!(Arc::ptr_eq(
            &fence.shared_text().source_arc(),
            snapshot.shared_text()
        ));
    }
}

#[test]
fn snapshot_debug_output_does_not_expand_the_shared_generation() {
    let analyzed = analyze_document_context_with_shared_text(
        &Analyzer::new(),
        "file:///tmp/debug.md",
        9,
        Arc::from("```mermaid\nflowchart TD\nA-->B\n```\n"),
        DocumentKind::Markdown,
    )
    .expect("source is within the analysis limit");
    let snapshot = analyzed.snapshot();

    let document_debug = format!("{snapshot:?}");
    let fence_debug = format!("{:?}", &snapshot.fences()[0]);

    assert!(document_debug.contains("fence_count: 1"));
    assert!(fence_debug.contains("diagram_ordinal: 0"));
    assert!(!document_debug.contains("AnalysisGeneration"));
    assert!(!fence_debug.contains("AnalysisGeneration"));
}

#[test]
fn markdown_documents_use_shared_fence_policy_for_tilde_fences() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.mdx",
            1,
            "before\n~~~mermaid\nsequenceDiagram\nAlice->>Bob: Hi\n~~~~\nafter\n".to_string(),
            DocumentKind::Mdx,
        )
        .expect("source is within the analysis limit");

    assert_eq!(snapshot.source().kind, SourceKind::Mdx);
    assert_eq!(snapshot.fences().len(), 1);
    assert_eq!(snapshot.fences()[0].source_id(), "mermaid-fence-1");
    assert_eq!(snapshot.fences()[0].source().kind, SourceKind::Mdx);
    assert_eq!(
        snapshot.fences()[0].fence_delimiter().unwrap().marker(),
        FenceMarker::Tilde
    );
    assert_eq!(snapshot.fences()[0].diagram_type(), Some("sequence"));
}

#[test]
fn cursor_lookup_distinguishes_prose_from_mermaid_fences() {
    let harness = SnapshotHarness::new();
    let snapshot = harness
        .analyze(
            "file:///tmp/example.md",
            1,
            "before\n```mermaid\nflowchart TD\nA-->B\n```\nafter\n".to_string(),
            DocumentKind::Markdown,
        )
        .expect("source is within the analysis limit");

    assert!(snapshot.fence_at_position(Position::new(0, 2)).is_none());
    let fence = snapshot
        .fence_at_position(Position::new(2, 4))
        .expect("cursor inside fence");
    assert_eq!(fence.diagram_type(), Some("flowchart-v2"));
}

#[test]
fn cursor_lookup_includes_unclosed_markdown_fence_at_eof() {
    let harness = SnapshotHarness::new();
    for (source, position) in [
        (
            "before\n```mermaid\nflowchart TD\nA-->",
            Position::new(3, 4),
        ),
        (
            "before\n```mermaid\nflowchart TD\nA-->\n",
            Position::new(4, 0),
        ),
    ] {
        let snapshot = harness
            .analyze(
                "file:///tmp/example.md",
                1,
                source.to_string(),
                DocumentKind::Markdown,
            )
            .expect("source is within the analysis limit");

        let fence = snapshot
            .fence_at_position(position)
            .expect("EOF should remain inside unclosed Mermaid fence");
        assert_eq!(fence.diagram_type(), Some("flowchart-v2"));
    }
}

#[test]
fn one_shot_context_uses_the_requested_document_identity() {
    let uri = DocumentUri::new("file:///tmp/example.mmd");
    let analyzed = analyze_document_context_with_shared_text(
        &Analyzer::new(),
        uri.clone(),
        1,
        Arc::from("flowchart TD\nA-->B\n"),
        DocumentKind::Diagram,
    )
    .expect("source is within the analysis limit");
    let snapshot = analyzed.snapshot();

    assert_eq!(snapshot.uri(), &uri);
    assert_eq!(snapshot.fences()[0].diagram_type(), Some("flowchart-v2"));
}

#[test]
fn snapshot_constructs_from_the_canonical_analysis_generation() {
    let source: Arc<str> = Arc::from("flowchart TD\nA-->B\n");
    let uri = DocumentUri::new("file:///tmp/canonical.mmd");
    let analyzer = Analyzer::new();
    let generation = match analyze_document_generation_shared(
        Arc::clone(&source),
        &analyzer,
        source_descriptor_for_kind(Some(uri.as_str()), SourceKind::Diagram),
    ) {
        AnalysisCaptureOutcome::Ready(generation) => Arc::new(generation),
        AnalysisCaptureOutcome::Rejected(rejection) => {
            panic!("source should be within the analysis limit: {rejection:?}")
        }
    };
    let snapshot = DocumentSnapshot::try_from_analysis_generation(7, Arc::clone(&generation))
        .expect("generation identifies its source document");

    assert_eq!(snapshot.uri(), &uri);
    assert_eq!(snapshot.kind(), DocumentKind::Diagram);
    assert!(Arc::ptr_eq(snapshot.shared_text(), &source));
    assert!(Arc::ptr_eq(
        &snapshot.shared_analysis_generation(),
        &generation
    ));
    assert_eq!(snapshot.analysis_generation().diagrams().len(), 1);
    let payload = snapshot
        .analysis_generation()
        .project(Analyzer::new().options().diagnostic_policy());
    assert!(payload.valid);
}

#[test]
fn snapshot_kind_comes_from_the_generation_source_descriptor() {
    let analyzer = Analyzer::new();

    for (source_kind, expected_kind) in [
        (SourceKind::Diagram, DocumentKind::Diagram),
        (SourceKind::Markdown, DocumentKind::Markdown),
        (SourceKind::Mdx, DocumentKind::Mdx),
    ] {
        let generation = match analyze_document_generation_shared(
            Arc::from(""),
            &analyzer,
            source_descriptor_for_kind(Some("file:///tmp/source.mmd"), source_kind),
        ) {
            AnalysisCaptureOutcome::Ready(generation) => Arc::new(generation),
            AnalysisCaptureOutcome::Rejected(rejection) => {
                panic!("empty source should be within the analysis limit: {rejection:?}")
            }
        };
        let snapshot = DocumentSnapshot::try_from_analysis_generation(1, generation)
            .expect("generation identifies its source document");

        assert_eq!(snapshot.kind(), expected_kind);
        assert_eq!(snapshot.uri().as_str(), "file:///tmp/source.mmd");
    }
}

#[test]
fn snapshot_construction_rejects_a_generation_without_a_document_path() {
    let generation = match analyze_document_generation_shared(
        Arc::from("flowchart TD\nA-->B\n"),
        &Analyzer::new(),
        source_descriptor_for_kind(None, SourceKind::Diagram),
    ) {
        AnalysisCaptureOutcome::Ready(generation) => Arc::new(generation),
        AnalysisCaptureOutcome::Rejected(rejection) => {
            panic!("source should be within the analysis limit: {rejection:?}")
        }
    };

    let error = DocumentSnapshot::try_from_analysis_generation(1, generation)
        .expect_err("editor snapshots require a document path");

    assert_eq!(
        error,
        DocumentSnapshotError::MissingSourcePath {
            kind: SourceKind::Diagram,
        }
    );
    assert_eq!(
        error.to_string(),
        "analysis generation for diagram source has no document path"
    );
}

#[test]
fn one_shot_snapshots_are_independent_across_document_versions() {
    let harness = SnapshotHarness::new();
    let uri = DocumentUri::new("file:///tmp/example.mmd");

    let first = harness
        .analyze(
            uri.clone(),
            1,
            "flowchart TD\nA-->B\n".to_string(),
            DocumentKind::Diagram,
        )
        .expect("source is within the analysis limit");
    let second = harness
        .analyze(
            uri.clone(),
            2,
            "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
            DocumentKind::Diagram,
        )
        .expect("source is within the analysis limit");

    assert_eq!(first.version(), 1);
    assert_eq!(second.version(), 2);
    assert_eq!(first.fences()[0].diagram_type(), Some("flowchart-v2"));
    assert_eq!(second.fences()[0].diagram_type(), Some("sequence"));
    assert!(first.text().contains("flowchart TD"));
    assert!(!second.text().contains("flowchart TD"));
}

#[test]
fn resource_rejection_does_not_construct_a_snapshot() {
    let limited_analyzer = Analyzer::with_options(
        AnalysisOptions::default().with_max_source_bytes(Some("flowchart TD\nA-->B\n".len() - 1)),
    );
    let uri = DocumentUri::new("file:///tmp/example.mmd");

    let source = "flowchart TD\nA-->B\n";
    let rejection = analyze_document_snapshot_with_shared_text(
        &limited_analyzer,
        uri.clone(),
        1,
        Arc::from(source),
        DocumentKind::Diagram,
    )
    .expect_err("over-limit text must not become an editable snapshot");

    assert_eq!(rejection.resource_limit().observed(), source.len());
    assert_eq!(rejection.resource_limit().maximum(), source.len() - 1);
    assert_eq!(
        rejection.payload().diagnostics[0].id,
        "merman.resource.source_bytes_exceeded"
    );

    let rebuilt = analyze_document_snapshot_with_shared_text(
        &Analyzer::new(),
        uri,
        1,
        Arc::from(source),
        DocumentKind::Diagram,
    )
    .expect("unlimited analyzer can construct a snapshot");
    assert_eq!(rebuilt.fences()[0].diagram_type(), Some("flowchart-v2"));
}
