use merman_analysis::{
    AnalysisOptions, AnalysisRuleConfig, Analyzer, DiagnosticSeverity, DiagramParseDisposition,
    FenceMarker, FenceTextIndexSource, SourceKind,
};
use merman_editor_core::{
    DiagramDetectionValidity, DocumentKind, DocumentUri, DocumentWorkspace, Position,
};
use std::sync::Arc;

#[test]
fn plain_mermaid_documents_create_single_snapshot_fence() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace
        .upsert(
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
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace
        .upsert(
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
    let analyzed = DocumentWorkspace::build_analysis_context_with_shared_text(
        &Analyzer::new(),
        "file:///tmp/incomplete.mmd",
        4,
        Arc::from("flowchart TD\nA[unterminated\n"),
        DocumentKind::Diagram,
    )
    .into_ready()
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
    DocumentWorkspace::build_analysis_context_with_shared_text(
        &analyzer,
        "file:///tmp/disposition.mmd",
        1,
        Arc::from(source),
        DocumentKind::Diagram,
    )
    .into_ready()
    .expect("source should remain within the analysis limit")
}

#[test]
fn cloned_snapshots_share_immutable_text_buffers() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace
        .upsert(
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
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace
        .upsert(
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
fn markdown_documents_use_shared_fence_policy_for_tilde_fences() {
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace
        .upsert(
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
    let mut workspace = DocumentWorkspace::new();
    let snapshot = workspace
        .upsert(
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
    let mut workspace = DocumentWorkspace::new();
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
        let snapshot = workspace
            .upsert(
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
fn build_snapshot_does_not_cache_document() {
    let workspace = DocumentWorkspace::new();
    let uri = DocumentUri::new("file:///tmp/example.mmd");
    let snapshot = workspace
        .build_snapshot(
            uri.clone(),
            1,
            "flowchart TD\nA-->B\n".to_string(),
            DocumentKind::Diagram,
        )
        .expect("source is within the analysis limit");

    assert_eq!(snapshot.uri(), &uri);
    assert_eq!(snapshot.fences()[0].diagram_type(), Some("flowchart-v2"));
    assert!(workspace.get(&uri).is_none());
}

#[test]
fn replacing_document_version_drops_stale_fence_state() {
    let mut workspace = DocumentWorkspace::new();
    let uri = DocumentUri::new("file:///tmp/example.mmd");

    let first = workspace
        .upsert(
            uri.clone(),
            1,
            "flowchart TD\nA-->B\n".to_string(),
            DocumentKind::Diagram,
        )
        .expect("source is within the analysis limit");
    let second = workspace
        .upsert(
            uri.clone(),
            2,
            "sequenceDiagram\nAlice->>Bob: Hi\n".to_string(),
            DocumentKind::Diagram,
        )
        .expect("source is within the analysis limit");

    assert_eq!(first.version(), 1);
    assert_eq!(second.version(), 2);

    let stored = workspace.get(&uri).unwrap();
    assert_eq!(stored.version(), 2);
    assert_eq!(stored.fences().len(), 1);
    assert_eq!(stored.fences()[0].diagram_type(), Some("sequence"));
    assert!(!stored.text().contains("flowchart TD"));
}

#[test]
fn resource_rejection_cannot_construct_or_cache_a_snapshot() {
    let limited_analyzer = Analyzer::with_options(
        AnalysisOptions::default().with_max_source_bytes(Some("flowchart TD\nA-->B\n".len() - 1)),
    );
    let mut workspace = DocumentWorkspace::with_analyzer(limited_analyzer);
    let uri = DocumentUri::new("file:///tmp/example.mmd");

    let source = "flowchart TD\nA-->B\n";
    let rejection = workspace
        .upsert(uri.clone(), 1, source.to_string(), DocumentKind::Diagram)
        .expect_err("over-limit text must not become an editable snapshot");

    assert_eq!(rejection.source_len(), source.len());
    assert_eq!(rejection.max_source_bytes(), source.len() - 1);
    assert_eq!(
        rejection.payload().diagnostics[0].id,
        "merman.resource.source_bytes_exceeded"
    );
    assert!(workspace.get(&uri).is_none());

    workspace.replace_analyzer(Analyzer::new());

    assert!(workspace.get(&uri).is_none());
    let rebuilt = workspace
        .upsert(uri, 1, source.to_string(), DocumentKind::Diagram)
        .expect("unlimited analyzer can construct a snapshot");
    assert_eq!(rebuilt.fences()[0].diagram_type(), Some("flowchart-v2"));
}
