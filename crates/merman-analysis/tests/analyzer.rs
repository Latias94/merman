use merman_analysis::{
    ANALYSIS_FACTS_PAYLOAD_VERSION, ANALYSIS_PAYLOAD_VERSION, AnalysisOptions,
    AnalysisResourceLimit, AnalysisRuleConfig, AnalysisRuleProfile, AnalysisStatus, Analyzer,
    DiagnosticCategory, DiagnosticSeverity, FenceExpectedSyntaxKind, FenceTextIndexSource,
    SourceDescriptor, analyze_document_facts,
    document::{analyze_document, analyze_document_generation},
    source_descriptor_for_markdown_path,
};
use merman_core::EditorLexemeProducerKind;

fn analyze(source: &str) -> merman_analysis::AnalysisPayload {
    Analyzer::new().analyze(source)
}

#[test]
fn empty_source_returns_no_diagram_error() {
    let payload = analyze("");

    assert!(!payload.valid);
    assert_eq!(payload.summary.errors, 1);
    let diagnostic = &payload.diagnostics[0];
    assert_eq!(diagnostic.id, "merman.parse.no_diagram");
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(diagnostic.category, DiagnosticCategory::Parse);
    assert_eq!(diagnostic.code, Some(AnalysisStatus::NoDiagram.code()));
    assert_eq!(
        diagnostic.code_name.as_deref(),
        Some(AnalysisStatus::NoDiagram.code_name())
    );
    assert_eq!(diagnostic.span.as_ref().unwrap().byte_start, 0);
    assert_eq!(diagnostic.span.as_ref().unwrap().byte_end, 0);
}

#[test]
fn facts_and_diagnostics_payloads_use_independent_version_constants() {
    let analyzer = Analyzer::new();

    assert_eq!(
        analyzer.analyze("flowchart TD\nA\n").version,
        ANALYSIS_PAYLOAD_VERSION
    );
    assert_eq!(
        analyzer.analyze_facts("flowchart TD\nA\n").version,
        ANALYSIS_FACTS_PAYLOAD_VERSION
    );
}

#[test]
fn unknown_source_exposes_unavailable_facts_without_inventing_body_semantics() {
    let facts = Analyzer::new().analyze_facts("unknownDiagram\nPretendNode --> OtherNode\n");
    let syntax = &facts.diagrams[0].syntax;

    assert_eq!(syntax.fact_source, FenceTextIndexSource::Unavailable);
    assert!(!syntax.parser_backed);
    assert!(!syntax.source_mapped_spans);
    assert!(syntax.node_ids.is_empty());
    assert!(syntax.references.is_empty());
    assert!(syntax.outline_items.is_empty());
    assert!(syntax.semantic_items.is_empty());
}

#[test]
fn invalid_syntax_returns_parse_error_with_diagram_type() {
    let payload = analyze("flowchart TD\nA -->\n");

    assert!(!payload.valid);
    assert_eq!(payload.summary.errors, 1);
    let diagnostic = &payload.diagnostics[0];
    assert_eq!(diagnostic.id, "merman.parse.diagram_parse");
    assert_eq!(diagnostic.category, DiagnosticCategory::Parse);
    assert_eq!(diagnostic.code, Some(AnalysisStatus::ParseError.code()));
    assert_eq!(diagnostic.diagram_type.as_deref(), Some("flowchart-v2"));
    assert!(diagnostic.span.is_some());
    assert!(!diagnostic.message.contains("UnrecognizedToken"));
    assert!(diagnostic.message.contains("unexpected"));
}

#[test]
fn common_authoring_parse_errors_are_single_precise_or_explicit_fallback_diagnostics() {
    struct Case<'a> {
        label: &'a str,
        source: &'a str,
        expected_diagram_type: &'a str,
    }

    let cases = [
        Case {
            label: "unterminated flowchart label",
            source: "flowchart TD\nA[unterminated",
            expected_diagram_type: "flowchart-v2",
        },
        Case {
            label: "dangling flowchart edge",
            source: "flowchart TD\nA -->\n",
            expected_diagram_type: "flowchart-v2",
        },
        Case {
            label: "dangling state transition",
            source: "stateDiagram-v2\nIdle --> Running\nRunning -->",
            expected_diagram_type: "stateDiagram",
        },
        Case {
            label: "dangling sequence arrow",
            source: "sequenceDiagram\nAlice->>Bob: Hi\nBob->>",
            expected_diagram_type: "sequence",
        },
        Case {
            label: "dangling class inheritance",
            source: "classDiagram\nA <|--",
            expected_diagram_type: "class",
        },
        Case {
            label: "dangling er relationship label",
            source: "erDiagram\nCUSTOMER ||--o{ ORDER :",
            expected_diagram_type: "er",
        },
    ];

    for case in cases {
        let payload = analyze(case.source);
        assert!(!payload.valid, "{}", case.label);
        let parse_diagnostics: Vec<_> = payload
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.id == "merman.parse.diagram_parse")
            .collect();
        assert_eq!(parse_diagnostics.len(), 1, "{}", case.label);

        let diagnostic = parse_diagnostics[0];
        assert_eq!(
            diagnostic.diagram_type.as_deref(),
            Some(case.expected_diagram_type),
            "{}",
            case.label
        );
        assert_eq!(
            diagnostic.category,
            DiagnosticCategory::Parse,
            "{}",
            case.label
        );
        assert_eq!(
            diagnostic.severity,
            DiagnosticSeverity::Error,
            "{}",
            case.label
        );
        assert_eq!(
            diagnostic.code,
            Some(AnalysisStatus::ParseError.code()),
            "{}",
            case.label
        );

        let span = diagnostic.span.as_ref().expect(case.label);
        assert!(
            span.byte_end <= case.source.len(),
            "{} span escaped source",
            case.label
        );
        assert!(
            span.byte_start == span.byte_end || span.byte_end - span.byte_start < case.source.len(),
            "{} should not use a whole-source parse span",
            case.label
        );
        assert!(
            span.byte_start > 0 || span.byte_end > 0,
            "{} should not default to the document start",
            case.label
        );
        assert!(
            diagnostic.related.is_empty()
                || diagnostic
                    .related
                    .iter()
                    .any(|related| related.message.contains("fallback")
                        || related.message.contains("Parser recovery produced")),
            "{} only fallback parse spans or deduped parser recovery should add related context",
            case.label
        );
    }
}

#[test]
fn source_wide_diagnostics_remain_whole_source_by_contract() {
    let no_diagram = analyze("");
    let no_diagram_span = no_diagram.diagnostics[0].span.as_ref().unwrap();
    assert_eq!(no_diagram.diagnostics[0].id, "merman.parse.no_diagram");
    assert_eq!(no_diagram_span.byte_start, 0);
    assert_eq!(no_diagram_span.byte_end, 0);

    let source = "flowchart TD\nA-->B\n";
    let options = AnalysisOptions::default().with_max_source_bytes(Some(8));
    let resource = Analyzer::with_options(options).analyze(source);
    let resource_span = resource.diagnostics[0].span.as_ref().unwrap();
    assert_eq!(
        resource.diagnostics[0].id,
        "merman.resource.source_bytes_exceeded"
    );
    assert_eq!(resource_span.byte_start, 0);
    assert_eq!(resource_span.byte_end, source.len());
}

#[test]
fn markdown_fence_parse_diagnostic_remaps_to_fence_body_not_whole_document() {
    let source = concat!(
        "# Title\n\n",
        "```mermaid\n",
        "flowchart TD\n",
        "A[unterminated\n",
        "```\n\n",
        "```mermaid\n",
        "flowchart TD\n",
        "B-->C\n",
        "```\n",
    );
    let analyzer =
        Analyzer::with_options(AnalysisOptions::default().with_source(SourceDescriptor::diagram()));
    let payload = analyze_document(
        source,
        &analyzer,
        source_descriptor_for_markdown_path(Some("doc.md")),
    );

    assert!(!payload.valid);
    let parse_diagnostics: Vec<_> = payload
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.id == "merman.parse.diagram_parse")
        .collect();
    assert_eq!(parse_diagnostics.len(), 1);

    let diagnostic = parse_diagnostics[0];
    let span = diagnostic.span.as_ref().expect("diagnostic span");
    let first_body_start = source.find("flowchart TD").unwrap();
    let unterminated_label_start = source.find("[unterminated").unwrap();
    let unterminated_label_end = unterminated_label_start + "[unterminated".len();
    let first_fence_end = source.find("\n```\n\n").unwrap();
    assert_eq!(span.byte_start, unterminated_label_start);
    assert_eq!(span.byte_end, unterminated_label_end);
    let expected_span = merman_analysis::SourceMap::new(source)
        .span(unterminated_label_start, unterminated_label_end)
        .unwrap();
    assert_eq!(span.line, expected_span.line);
    assert_eq!(span.column, expected_span.column);
    assert_eq!(span.end_line, expected_span.end_line);
    assert_eq!(span.end_column, expected_span.end_column);
    assert_eq!(span.lsp_range, expected_span.lsp_range);
    assert!(span.byte_start >= first_body_start);
    assert!(span.byte_end <= first_fence_end);
    assert!(
        span.byte_start > first_body_start || span.byte_end < first_fence_end,
        "parse diagnostic should keep token/fallback precision instead of taking the whole fence"
    );
    assert!(diagnostic.related.iter().any(|related| {
        related.message == "Mermaid fence 1"
            && related
                .span
                .as_ref()
                .is_some_and(|span| span.byte_start < first_body_start)
    }));
}

#[test]
fn recovered_gantt_editor_diagnostic_is_deduplicated_with_the_parse_error() {
    let source = "gantt\nweekday foo\n";
    let payload = analyze(source);

    assert!(!payload.valid);
    assert_eq!(payload.summary.errors, 1);
    assert_eq!(payload.summary.warnings, 0);
    assert_eq!(payload.diagnostics.len(), 1);
    let diagnostic = payload
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.id == "merman.parse.diagram_parse")
        .expect("parse diagnostic");
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(diagnostic.category, DiagnosticCategory::Parse);
    assert_eq!(diagnostic.diagram_type.as_deref(), Some("gantt"));
    assert!(diagnostic.message.contains("invalid weekday"));
    assert_eq!(
        diagnostic.span.as_ref().map(|span| span.byte_start),
        source.find("foo")
    );
    assert!(diagnostic.related.iter().any(|related| {
        related
            .message
            .contains("Parser recovery produced the same syntax problem")
    }));
    assert!(
        payload
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.id != "merman.parse.recovered_editor_facts")
    );
}

#[test]
fn recovered_mindmap_editor_diagnostic_is_merged_into_the_primary_error() {
    let source = "mindmap\nroot\n child[unterminated";
    let payload = analyze(source);

    assert!(!payload.valid);
    assert_eq!(payload.summary.errors, 1);
    assert_eq!(payload.summary.warnings, 0);
    let diagnostic = payload
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.id == "merman.parse.diagram_parse")
        .expect("primary parse diagnostic");
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(diagnostic.category, DiagnosticCategory::Parse);
    assert_eq!(diagnostic.diagram_type.as_deref(), Some("mindmap"));
    assert!(diagnostic.message.contains("unterminated node delimiter"));
    assert_eq!(
        diagnostic.span.as_ref().map(|span| span.byte_start),
        source.find("child")
    );
    assert!(diagnostic.related.iter().any(|related| {
        related
            .message
            .contains("Parser recovery produced the same syntax problem")
    }));
    assert!(
        payload
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.id != "merman.parse.recovered_editor_facts")
    );
}

#[test]
fn analysis_generation_exposes_complete_parser_syntax_facts() {
    let source = "flowchart TD\nA-->B\n";
    let analyzer = Analyzer::new();
    let result = analyzer
        .analyze_generation(source)
        .into_ready()
        .expect("source is within the analysis limit");
    let payload = result.project(analyzer.options().diagnostic_policy());

    assert!(payload.valid);
    assert!(payload.diagnostics.is_empty());
    assert_eq!(result.diagrams().len(), 1);

    let diagram = &result.diagrams()[0];
    assert_eq!(diagram.source_id(), "document");
    assert_eq!(
        diagram.syntax().diagram_type.as_deref(),
        Some("flowchart-v2")
    );
    assert_eq!(
        diagram.syntax().source(),
        FenceTextIndexSource::ParserComplete
    );
    assert!(diagram.syntax().text_index.node_ids().any(|id| id == "A"));
}

#[test]
fn analysis_index_preserves_core_lexeme_provenance() {
    let analyzer = Analyzer::new();
    let complete = analyzer
        .analyze_generation("%% global comment\nflowchart TD\nA-->B\n")
        .into_ready()
        .expect("source is within the analysis limit");
    let complete_lexemes = complete.diagrams()[0].syntax().text_index.lexemes();

    assert!(complete_lexemes.iter().any(|lexeme| {
        lexeme.producer().kind() == EditorLexemeProducerKind::GlobalPreprocess
            && lexeme.producer().family().is_none()
    }));
    assert!(complete_lexemes.iter().any(|lexeme| {
        matches!(
            lexeme.producer().kind(),
            EditorLexemeProducerKind::FamilyLexer | EditorLexemeProducerKind::FamilyParser
        ) && lexeme.producer().family().map(|family| family.as_str()) == Some("flowchart")
    }));

    let recovered = analyzer
        .analyze_generation("flowchart TD\nA-->")
        .into_ready()
        .expect("source is within the analysis limit");
    assert!(
        recovered.diagrams()[0]
            .syntax()
            .text_index
            .lexemes()
            .iter()
            .any(|lexeme| {
                lexeme.producer().kind() == EditorLexemeProducerKind::FamilyRecovery
                    && lexeme.producer().family().map(|family| family.as_str()) == Some("flowchart")
            })
    );
}

#[test]
fn analysis_generation_preserves_exact_spans_through_entity_normalization() {
    let source = concat!(
        "---\n",
        "title: quoted\n",
        "---\n",
        "sequenceDiagram\n",
        "participant Alice\n",
        "Alice->>Bob: #quot;\n",
    );
    let facts = Analyzer::new().analyze_facts(source);

    assert!(facts.valid);
    assert_eq!(facts.diagrams.len(), 1);

    let syntax = &facts.diagrams[0].syntax;
    assert_eq!(syntax.diagram_type.as_deref(), Some("sequence"));
    assert_eq!(syntax.fact_source, FenceTextIndexSource::ParserComplete);
    assert!(syntax.parser_backed);
    assert!(!syntax.recovered);
    assert!(syntax.source_mapped_spans);
    assert!(syntax.node_ids.iter().any(|id| id == "Alice"));
    assert!(syntax.node_ids.iter().any(|id| id == "Bob"));
    assert!(!syntax.references.is_empty());
    assert!(!syntax.outline_items.is_empty());
    assert!(!syntax.semantic_items.is_empty());
    let outline_projection = syntax
        .outline_items
        .iter()
        .map(|item| {
            (
                item.name.as_str(),
                item.detail.as_deref(),
                item.kind,
                &item.span,
                &item.selection,
            )
        })
        .collect::<Vec<_>>();
    let canonical_outline = syntax
        .semantic_items
        .iter()
        .filter(|item| item.role.contributes_outline())
        .map(|item| {
            (
                item.name.as_str(),
                item.detail.as_deref(),
                item.kind,
                &item.span,
                &item.selection,
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(outline_projection, canonical_outline);
    assert!(!syntax.expected_syntax.is_empty());
    assert!(syntax.expected_syntax.iter().all(|expected| {
        expected.span.document.as_ref().is_some_and(|span| {
            span.byte_end <= source.len()
                && source.is_char_boundary(span.byte_start)
                && source.is_char_boundary(span.byte_end)
        })
    }));
}

#[test]
fn analysis_generation_exposes_expected_syntax_facts_for_invalid_input() {
    let source = "flowchart TD\nA@{\n  shape: rou\n}\n";
    let analyzer = Analyzer::new();
    let result = analyzer
        .analyze_generation(source)
        .into_ready()
        .expect("source is within the analysis limit");
    let payload = result.project(analyzer.options().diagnostic_policy());

    assert!(!payload.valid);
    assert_eq!(result.diagrams().len(), 1);

    let diagram = &result.diagrams()[0];
    assert_eq!(diagram.source_id(), "document");
    assert_eq!(
        diagram.syntax().diagram_type.as_deref(),
        Some("flowchart-v2")
    );
    assert!(diagram.syntax().source().is_parser_backed());
    assert!(
        diagram
            .syntax()
            .text_index
            .expected_syntax()
            .iter()
            .any(|expected| expected.kind == FenceExpectedSyntaxKind::Shape)
    );
}

#[test]
fn document_analysis_generation_keeps_local_fence_syntax_facts() {
    let source = concat!(
        "before\n",
        "```mermaid\n",
        "flowchart TD\n",
        "A@{\n",
        "  shape: rou\n",
        "}\n",
        "```\n",
        "after\n",
    );
    let analyzer = Analyzer::new();
    let result = analyze_document_generation(
        source,
        &analyzer,
        source_descriptor_for_markdown_path(Some("doc.md")),
    )
    .into_ready()
    .expect("source is within the analysis limit");

    assert!(!result.project(analyzer.options().diagnostic_policy()).valid);
    assert_eq!(result.diagrams().len(), 1);

    let diagram = &result.diagrams()[0];
    assert_eq!(diagram.source_id(), "mermaid-fence-1");
    assert_eq!(diagram.source().diagram_index, Some(0));
    assert_eq!(
        diagram.syntax().diagram_type.as_deref(),
        Some("flowchart-v2")
    );
    assert!(diagram.syntax().source().is_parser_backed());
    assert!(
        diagram
            .syntax()
            .text_index
            .expected_syntax()
            .iter()
            .any(|expected| expected.kind == FenceExpectedSyntaxKind::Shape)
    );
}

#[test]
fn document_diagnostics_match_rich_generation_for_markdown_and_mdx() {
    let source = concat!(
        "before\n",
        "```mermaid\n",
        "cynefin-beta\n",
        "  complex\n",
        "  complicated\n",
        "  complicated --> complicated : \"Self-loop\"\n",
        "```\n",
        "after\n",
    );
    let analyzer = Analyzer::new();

    for path in ["doc.md", "doc.mdx"] {
        let descriptor = source_descriptor_for_markdown_path(Some(path));
        let diagnostics_only = analyze_document(source, &analyzer, descriptor.clone());
        let rich = analyze_document_generation(source, &analyzer, descriptor)
            .into_ready()
            .expect("document source should produce a rich analysis result");
        let projected = rich.project(analyzer.options().diagnostic_policy());
        let reprojected = rich.project(analyzer.options().diagnostic_policy());

        assert_eq!(diagnostics_only, projected, "{path}");
        assert_eq!(diagnostics_only, reprojected, "{path}");
        assert_eq!(diagnostics_only.diagnostics.len(), 1, "{path}");
        assert_eq!(
            diagnostics_only.diagnostics[0].id, "merman.parse.recovered_editor_facts",
            "{path}"
        );
        assert_eq!(
            diagnostics_only.diagnostics[0]
                .span
                .as_ref()
                .map(|span| span.line),
            Some(6),
            "{path}"
        );
        assert_eq!(
            diagnostics_only.diagnostics[0]
                .related
                .iter()
                .filter(|related| related.message == "Mermaid fence 1")
                .count(),
            1,
            "{path}"
        );
    }
}

#[test]
fn document_analysis_facts_payload_exposes_parser_backed_fence_facts() {
    let source = concat!(
        "before\n",
        "```mermaid\n",
        "flowchart TD\n",
        "A@{\n",
        "  shape: rou\n",
        "}\n",
        "```\n",
        "after\n",
    );
    let analyzer = Analyzer::new();
    let facts = merman_analysis::analyze_document_facts(
        source,
        &analyzer,
        source_descriptor_for_markdown_path(Some("doc.md")),
    );

    assert!(!facts.valid);
    assert_eq!(facts.source.kind, merman_analysis::SourceKind::Markdown);
    assert_eq!(facts.diagrams.len(), 1);

    let diagram = &facts.diagrams[0];
    assert_eq!(diagram.source_id, "mermaid-fence-1");
    assert_eq!(diagram.kind, "mermaid_fence");
    assert_eq!(diagram.source.diagram_index, Some(0));
    assert_eq!(
        diagram.body_span.as_ref().map(|span| span.byte_start),
        source.find("flowchart TD")
    );

    let syntax = &diagram.syntax;
    assert_eq!(syntax.diagram_type.as_deref(), Some("flowchart-v2"));
    assert!(syntax.parser_backed);

    let shape_expectation = syntax
        .expected_syntax
        .iter()
        .find(|expected| expected.kind == FenceExpectedSyntaxKind::Shape)
        .expect("shape expectation");
    assert_eq!(
        shape_expectation
            .span
            .document
            .as_ref()
            .map(|span| span.byte_start),
        source.find("rou")
    );
}

#[test]
fn markdown_fence_facts_compose_crlf_preprocess_edits_and_utf16_positions() {
    let source = concat!(
        "😀 before\r\n",
        "```mermaid\r\n",
        "---\r\n",
        "title: Demo\r\n",
        "---\r\n",
        "%%{wrap}%%\r\n",
        "flowchart TD\r\n",
        "classDef hot fill:#f00;\r\n",
        "A[\"😀 #quot;\"]:::hot\r\n",
        "```\r\n",
    );
    let facts = merman_analysis::analyze_document_facts(
        source,
        &Analyzer::new(),
        source_descriptor_for_markdown_path(Some("doc.md")),
    );

    assert!(facts.valid, "{:#?}", facts.diagnostics);
    let syntax = &facts.diagrams[0].syntax;
    assert_eq!(syntax.fact_source, FenceTextIndexSource::ParserComplete);
    assert!(syntax.source_mapped_spans);

    let class_definition = syntax
        .semantic_items
        .iter()
        .find(|item| item.detail.as_deref() == Some("flowchart class definition"))
        .expect("class definition fact");
    let class_selection = class_definition
        .selection
        .document
        .as_ref()
        .expect("class definition document span");
    assert_eq!(
        &source[class_selection.byte_start..class_selection.byte_end],
        "hot"
    );

    let label = syntax
        .semantic_items
        .iter()
        .find(|item| item.detail.as_deref() == Some("flowchart node label"))
        .expect("node label fact");
    let label_selection = label
        .selection
        .document
        .as_ref()
        .expect("node label document span");
    assert_eq!(
        &source[label_selection.byte_start..label_selection.byte_end],
        "😀 #quot;"
    );
    assert_eq!(label_selection.lsp_range.start.character, 3);
    assert_eq!(
        label_selection.lsp_range.end.character,
        3 + "😀 #quot;".encode_utf16().count()
    );
}

#[test]
fn valid_flowchart_returns_no_diagnostics() {
    let payload = analyze("flowchart TD\nA[Hello] --> B[World]\n");

    assert!(payload.valid);
    assert_eq!(payload.summary.errors, 0);
    assert!(payload.diagnostics.is_empty());
}

#[test]
fn flowchart_missing_direction_is_not_reported_by_core_profile() {
    let source = "flowchart\nA[Hello] --> B[World]\n";
    let payload = analyze(source);

    assert!(payload.valid);
    assert_eq!(payload.summary.errors, 0);
    assert_eq!(payload.summary.warnings, 0);
    assert!(payload.diagnostics.is_empty());
}

#[test]
fn flowchart_missing_direction_is_authoring_hint_in_recommended_profile() {
    let source = "flowchart\nA[Hello] --> B[World]\n";
    let analyzer = Analyzer::with_options(AnalysisOptions::default().with_rule_config(
        AnalysisRuleConfig::default().with_profile(AnalysisRuleProfile::Recommended),
    ));
    let payload = analyzer.analyze(source);

    assert!(payload.valid);
    assert_eq!(payload.summary.errors, 0);
    assert_eq!(payload.summary.hints, 1);
    let diagnostic = &payload.diagnostics[0];
    assert_eq!(
        diagnostic.id,
        "merman.authoring.flowchart.explicit_direction"
    );
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Hint);
    assert_eq!(diagnostic.category, DiagnosticCategory::Semantic);
    assert_eq!(diagnostic.diagram_type.as_deref(), Some("flowchart-v2"));
    assert!(diagnostic.message.contains("explicit direction"));
    let span = diagnostic.span.as_ref().expect("diagnostic span");
    assert_eq!(span.byte_start, 0);
    assert_eq!(span.byte_end, "flowchart".len());
    assert_eq!(span.line, 1);
    assert_eq!(span.column, 1);
    assert_eq!(span.end_line, 1);
    assert_eq!(span.end_column, 10);
    assert_eq!(span.lsp_range.start.line, 0);
    assert_eq!(span.lsp_range.start.character, 0);
    assert_eq!(span.lsp_range.end.line, 0);
    assert_eq!(span.lsp_range.end.character, 9);

    assert_eq!(diagnostic.fixes.len(), 1);
    let fix = &diagnostic.fixes[0];
    assert_eq!(fix.title, "Insert `TB` into the flowchart header");
    assert!(fix.is_preferred);
    assert_eq!(fix.edits.len(), 1);
    assert_eq!(fix.edits[0].replacement, " TB");
    assert_eq!(fix.edits[0].span.byte_start, "flowchart".len());
    assert_eq!(fix.edits[0].span.byte_end, "flowchart".len());
    assert_eq!(fix.edits[0].span.lsp_range.start.line, 0);
    assert_eq!(fix.edits[0].span.lsp_range.start.character, 9);
    assert_eq!(fix.edits[0].span.lsp_range.end.line, 0);
    assert_eq!(fix.edits[0].span.lsp_range.end.character, 9);
    assert_eq!(
        source[fix.edits[0].span.byte_start..].chars().next(),
        Some('\n')
    );
}

#[test]
fn flowchart_missing_direction_fix_span_survives_frontmatter_preprocess() {
    let source = "---\ntitle: Demo\n---\nflowchart\nA[Hello] --> B[World]\n";
    let analyzer = Analyzer::with_options(AnalysisOptions::default().with_rule_config(
        AnalysisRuleConfig::default().with_profile(AnalysisRuleProfile::Recommended),
    ));
    let payload = analyzer.analyze(source);

    assert!(payload.valid);
    let diagnostic = payload
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.id == "merman.authoring.flowchart.explicit_direction")
        .expect("missing direction diagnostic");
    let span = diagnostic.span.as_ref().expect("diagnostic span");
    assert_eq!(&source[span.byte_start..span.byte_end], "flowchart");

    let edit = &diagnostic.fixes[0].edits[0];
    assert_eq!(edit.replacement, " TB");
    assert_eq!(
        edit.span.byte_start,
        source.find("flowchart").unwrap() + "flowchart".len()
    );
    assert_eq!(edit.span.byte_end, edit.span.byte_start);
    assert_eq!(source[edit.span.byte_start..].chars().next(), Some('\n'));
}

#[test]
fn flowchart_missing_direction_rule_can_be_disabled() {
    let options = AnalysisOptions::default().with_rule_config(
        AnalysisRuleConfig::default()
            .with_profile(AnalysisRuleProfile::Recommended)
            .with_rule_disabled("merman.authoring.flowchart.explicit_direction")
            .unwrap(),
    );
    let payload = Analyzer::with_options(options).analyze("flowchart\nA-->B\n");

    assert!(payload.valid);
    assert!(payload.diagnostics.is_empty());
}

#[test]
fn deprecated_flowchart_html_labels_config_is_core_warning() {
    let source = "%%{init: { \"flowchart\": { \"htmlLabels\": false, \"curve\": \"linear\" } }}%%\nflowchart TD\nA-->B\n";
    let payload = analyze(source);

    assert!(payload.valid);
    assert_eq!(payload.summary.warnings, 1);
    let diagnostic = &payload.diagnostics[0];
    assert_eq!(
        diagnostic.id,
        "merman.compatibility.config.deprecated_flowchart_html_labels"
    );
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Warning);
    assert_eq!(diagnostic.category, DiagnosticCategory::Config);
    assert!(diagnostic.message.contains("deprecated"));
    assert!(diagnostic.fixes.is_empty());
    let span = diagnostic.span.as_ref().expect("htmlLabels span");
    assert_eq!(&source[span.byte_start..span.byte_end], "htmlLabels");
}

#[test]
fn prefer_frontmatter_config_for_init_directives_is_a_recommended_hint() {
    let source = "%%{ init: { \"theme\": \"dark\" } }%%\nflowchart TD\nA-->B\n";
    let analyzer = Analyzer::with_options(AnalysisOptions::default().with_rule_config(
        AnalysisRuleConfig::default().with_profile(AnalysisRuleProfile::Recommended),
    ));
    let payload = analyzer.analyze(source);

    assert!(payload.valid);
    assert_eq!(payload.summary.hints, 1);
    let diagnostic = &payload.diagnostics[0];
    assert_eq!(
        diagnostic.id,
        "merman.authoring.config.prefer_frontmatter_config"
    );
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Hint);
    assert_eq!(diagnostic.category, DiagnosticCategory::Config);
    assert_eq!(diagnostic.fixes.len(), 1);
    assert_eq!(
        diagnostic.fixes[0].title,
        "Move init directive config into frontmatter"
    );
    assert!(diagnostic.fixes[0].is_preferred);
    let span = diagnostic.span.as_ref().expect("directive span");
    assert_eq!(&source[span.byte_start..span.byte_end], "init");
}

#[test]
fn source_config_diagnostics_and_fixes_keep_original_crlf_unicode_coordinates() {
    struct Case<'a> {
        label: &'a str,
        source: &'a str,
        rule_id: &'a str,
        expected_text: &'a str,
        expected_line: usize,
        expected_character: usize,
        expected_fix_replacement: Option<&'a str>,
        recommended: bool,
    }

    let cases = [
        Case {
            label: "directive keyword fix",
            source: concat!(
                "%% 前置 🤓\r\n",
                "%%{ initialize: { \"theme\": \"dark\" } }%%\r\n",
                "flowchart TD\r\n",
                "A-->B\r\n",
            ),
            rule_id: "merman.authoring.config.prefer_init_directive",
            expected_text: "initialize",
            expected_line: 1,
            expected_character: 4,
            expected_fix_replacement: Some("init"),
            recommended: true,
        },
        Case {
            label: "frontmatter config key",
            source: concat!(
                "---\r\n",
                "title: \"中文 🤓\"\r\n",
                "config:\r\n",
                "  flowchart:\r\n",
                "    htmlLabels: false\r\n",
                "---\r\n",
                "flowchart TD\r\n",
                "A-->B\r\n",
            ),
            rule_id: "merman.compatibility.config.deprecated_flowchart_html_labels",
            expected_text: "htmlLabels",
            expected_line: 4,
            expected_character: 4,
            expected_fix_replacement: None,
            recommended: false,
        },
    ];

    for case in cases {
        let options = if case.recommended {
            AnalysisOptions::default().with_rule_config(
                AnalysisRuleConfig::default()
                    .with_profile(AnalysisRuleProfile::Recommended)
                    .with_rule_disabled("merman.authoring.config.prefer_frontmatter_config")
                    .expect("test rule id should be configurable"),
            )
        } else {
            AnalysisOptions::default()
        };
        let payload = Analyzer::with_options(options).analyze(case.source);
        let diagnostic = payload
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.id == case.rule_id)
            .unwrap_or_else(|| panic!("missing {} diagnostic", case.label));
        let span = diagnostic
            .span
            .as_ref()
            .unwrap_or_else(|| panic!("missing {} span", case.label));

        assert_eq!(
            &case.source[span.byte_start..span.byte_end],
            case.expected_text,
            "{} source slice",
            case.label
        );
        assert_eq!(
            span.byte_start,
            case.source
                .find(case.expected_text)
                .unwrap_or_else(|| panic!("missing {} source text", case.label)),
            "{} byte start",
            case.label
        );
        assert_eq!(
            span.lsp_range.start.line, case.expected_line,
            "{}",
            case.label
        );
        assert_eq!(
            span.lsp_range.start.character, case.expected_character,
            "{}",
            case.label
        );
        assert_eq!(
            span.lsp_range.end.line, case.expected_line,
            "{}",
            case.label
        );
        assert_eq!(
            span.lsp_range.end.character,
            case.expected_character + case.expected_text.encode_utf16().count(),
            "{}",
            case.label
        );

        match case.expected_fix_replacement {
            Some(replacement) => {
                let edit = diagnostic
                    .fixes
                    .iter()
                    .flat_map(|fix| fix.edits.iter())
                    .find(|edit| edit.replacement == replacement)
                    .unwrap_or_else(|| panic!("missing {} fix edit", case.label));
                assert_eq!(edit.span, *span, "{} fix span", case.label);
            }
            None => assert!(diagnostic.fixes.is_empty(), "{}", case.label),
        }
    }
}

#[test]
fn class_html_labels_config_is_not_a_core_compatibility_warning() {
    let source = "%%{init: { \"class\": { \"htmlLabels\": true } }}%%\nclassDiagram\nA <|-- B\n";
    let payload = analyze(source);

    assert!(payload.valid);
    assert_eq!(payload.summary.warnings, 0);
    assert!(payload.diagnostics.is_empty());
}

#[test]
fn class_config_wrapped_html_labels_config_is_not_a_core_compatibility_warning() {
    let source = "%%{init: { \"config\": { \"htmlLabels\": true } }}%%\nclassDiagram\nA <|-- B\n";
    let payload = analyze(source);

    assert!(payload.valid);
    assert_eq!(payload.summary.warnings, 0);
    assert!(payload.diagnostics.is_empty());
}

#[test]
fn deprecated_external_diagram_loading_config_is_core_warning() {
    let source = "%%{init: { \"lazyLoadedDiagrams\": true }}%%\nflowchart TD\nA-->B\n";
    let payload = analyze(source);

    assert!(payload.valid);
    assert_eq!(payload.summary.warnings, 1);
    let diagnostic = &payload.diagnostics[0];
    assert_eq!(
        diagnostic.id,
        "merman.compatibility.config.deprecated_external_diagram_loading"
    );
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Warning);
    assert_eq!(diagnostic.category, DiagnosticCategory::Config);
    assert!(diagnostic.message.contains("deprecated"));
    assert!(diagnostic.fixes.is_empty());
    let span = diagnostic.span.as_ref().expect("deprecated config span");
    assert_eq!(
        &source[span.byte_start..span.byte_end],
        "lazyLoadedDiagrams"
    );
}

#[test]
fn unsupported_diagram_returns_compatibility_error() {
    let mut engine = merman_core::Engine::new();
    *engine.diagram_registry_mut() = merman_core::diagram::DiagramRegistry::new();

    let payload =
        Analyzer::with_engine(engine, AnalysisOptions::default()).analyze("flowchart TD\nA-->B\n");

    assert!(!payload.valid);
    assert_eq!(payload.summary.errors, 1);
    let diagnostic = &payload.diagnostics[0];
    assert_eq!(diagnostic.id, "merman.compatibility.unsupported_diagram");
    assert_eq!(diagnostic.category, DiagnosticCategory::Compatibility);
    assert_eq!(
        diagnostic.code,
        Some(AnalysisStatus::UnsupportedFormat.code())
    );
    assert_eq!(diagnostic.diagram_type.as_deref(), Some("flowchart-v2"));
}

#[test]
fn git_graph_duplicate_commit_id_is_warning() {
    let payload = analyze("gitGraph\ncommit id:\"working on MDR\"\ncommit id:\"working on MDR\"\n");

    assert!(payload.valid);
    assert_eq!(payload.summary.warnings, 1);
    let diagnostic = &payload.diagnostics[0];
    assert_eq!(diagnostic.id, "merman.git_graph.duplicate_commit_id");
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Warning);
    assert_eq!(diagnostic.category, DiagnosticCategory::Semantic);
    assert_eq!(diagnostic.diagram_type.as_deref(), Some("gitGraph"));
    assert!(diagnostic.message.contains("already exists"));
}

#[test]
fn block_width_overflow_is_warning() {
    let payload = analyze("block-beta\n  columns 1\n  A:1\n  B:2\n  C:3\n");

    assert!(payload.valid);
    assert_eq!(payload.summary.warnings, 2);
    assert!(payload.diagnostics.iter().any(|diagnostic| {
        diagnostic.id == "merman.block.width_exceeds_columns"
            && diagnostic.diagram_type.as_deref() == Some("block")
            && diagnostic
                .message
                .contains("exceeds configured column width")
    }));
}

#[test]
fn source_byte_limit_returns_resource_error() {
    let options = AnalysisOptions::default().with_max_source_bytes(Some(8));
    let payload = Analyzer::with_options(options).analyze("flowchart TD\nA-->B\n");

    assert!(!payload.valid);
    assert_eq!(payload.summary.errors, 1);
    let diagnostic = &payload.diagnostics[0];
    assert_eq!(diagnostic.id, "merman.resource.source_bytes_exceeded");
    assert_eq!(diagnostic.category, DiagnosticCategory::Resource);
    assert_eq!(
        diagnostic.code,
        Some(AnalysisStatus::ResourceLimitExceeded.code())
    );
}

#[test]
fn source_byte_limit_rule_cannot_be_disabled() {
    assert!(
        AnalysisRuleConfig::default()
            .with_rule_disabled("merman.resource.source_bytes_exceeded")
            .is_err()
    );
    let options = AnalysisOptions::default()
        .with_max_source_bytes(Some(8))
        .with_rule_config(AnalysisRuleConfig::default());
    let payload = Analyzer::with_options(options).analyze("flowchart TD\nA-->B\n");

    assert!(!payload.valid);
    assert_eq!(payload.summary.errors, 1);
    assert_eq!(payload.diagnostics.len(), 1);
    let diagnostic = &payload.diagnostics[0];
    assert_eq!(diagnostic.id, "merman.resource.source_bytes_exceeded");
    assert_eq!(
        diagnostic.code_name.as_deref(),
        Some(AnalysisStatus::ResourceLimitExceeded.code_name())
    );
}

#[test]
fn source_byte_limit_does_not_scan_syntax_facts() {
    let options = AnalysisOptions::default().with_max_source_bytes(Some(8));
    let facts = Analyzer::with_options(options).analyze_facts("flowchart TD\nA-->B\n");

    assert!(!facts.valid);
    assert_eq!(facts.summary.errors, 1);
    assert!(facts.diagrams.is_empty());
}

#[test]
fn plain_source_byte_limit_rejects_without_constructing_a_source_map() {
    let source = format!("flowchart TD\nA-->B\n{}", "x".repeat(64));
    let analyzer =
        Analyzer::with_options(AnalysisOptions::default().with_max_source_bytes(Some(8)));

    let rejection = analyzer
        .analyze_generation(&source)
        .into_ready()
        .expect_err("source must be rejected before rich facts are constructed");

    assert_eq!(
        rejection.resource_limit(),
        AnalysisResourceLimit::SourceBytes {
            source_len: source.len(),
            max_source_bytes: 8,
        }
    );
    let diagnostic = &rejection.payload().diagnostics[0];
    assert_eq!(diagnostic.id, "merman.resource.source_bytes_exceeded");
    let span = diagnostic.span.as_ref().unwrap();
    assert_eq!(span.byte_start, 0);
    assert_eq!(span.byte_end, source.len());
}

#[test]
fn markdown_document_source_byte_limit_applies_before_fence_analysis() {
    let source = format!(
        "intro 🤓\n```mermaid\nflowchart TD\nA-->B\n```\n{}",
        "x".repeat(64)
    );
    let analyzer =
        Analyzer::with_options(AnalysisOptions::default().with_max_source_bytes(Some(8)));
    let descriptor = source_descriptor_for_markdown_path(Some("doc.md"));

    let rejection = analyze_document_generation(&source, &analyzer, descriptor.clone())
        .into_ready()
        .expect_err("document source must be rejected before fence analysis");
    assert_eq!(
        rejection.resource_limit(),
        AnalysisResourceLimit::SourceBytes {
            source_len: source.len(),
            max_source_bytes: 8,
        }
    );

    let payload = rejection.payload();

    assert!(!payload.valid);
    assert_eq!(payload.summary.errors, 1);
    let diagnostic = &payload.diagnostics[0];
    assert_eq!(diagnostic.id, "merman.resource.source_bytes_exceeded");
    let span = diagnostic.span.as_ref().unwrap();
    assert_eq!(span.byte_start, 0);
    assert_eq!(span.byte_end, source.len());
    assert_eq!(span.line, 1);
    assert_eq!(span.column, 1);
    assert_eq!(span.end_line, 6);
    assert_eq!(span.end_column, 65);
    assert_eq!(span.lsp_range.start.line, 0);
    assert_eq!(span.lsp_range.start.character, 0);
    assert_eq!(span.lsp_range.end.line, 5);
    assert_eq!(span.lsp_range.end.character, 64);

    let facts = analyze_document_facts(&source, &analyzer, descriptor);
    assert!(!facts.valid);
    assert!(facts.diagrams.is_empty());
}

#[test]
fn source_byte_limit_span_matches_source_map_for_crlf_source() {
    let source = "flowchart TD\r\nA-->B\r";
    let analyzer =
        Analyzer::with_options(AnalysisOptions::default().with_max_source_bytes(Some(8)));

    let payload = analyzer.analyze(source);

    assert!(!payload.valid);
    let span = payload.diagnostics[0].span.as_ref().unwrap();
    let expected = merman_analysis::SourceMap::new(source)
        .whole_source_span()
        .unwrap();
    assert_eq!(span, &expected);
}

#[test]
fn markdown_document_source_byte_limit_allows_exact_boundary() {
    let source = "```mermaid\nflowchart TD\nA-->B\n```\n";
    let analyzer = Analyzer::with_options(
        AnalysisOptions::default().with_max_source_bytes(Some(source.len())),
    );
    let descriptor = source_descriptor_for_markdown_path(Some("doc.md"));

    let result = analyze_document_generation(source, &analyzer, descriptor)
        .into_ready()
        .expect("source at the limit remains analyzable");

    assert!(
        result
            .project(analyzer.options().diagnostic_policy())
            .diagnostics
            .is_empty()
    );
    assert_eq!(result.diagrams().len(), 1);
}

#[test]
fn markdown_document_diagram_limit_allows_exact_boundary() {
    let source = concat!(
        "```mermaid\nflowchart TD\nA-->B\n```\n",
        "```mermaid\nsequenceDiagram\nA->>B: hi\n```\n",
    );
    let analyzer =
        Analyzer::with_options(AnalysisOptions::default().with_max_document_diagrams(Some(2)));
    let descriptor = source_descriptor_for_markdown_path(Some("doc.md"));

    let generation = analyze_document_generation(source, &analyzer, descriptor)
        .into_ready()
        .expect("a document at the diagram limit remains analyzable");

    assert_eq!(generation.diagrams().len(), 2);
}

#[test]
fn markdown_document_diagram_limit_rejects_before_rich_analysis() {
    let source = concat!(
        "```mermaid\nflowchart TD\nA-->B\n```\n",
        "```mermaid\nsequenceDiagram\nA->>B: hi\n```\n",
    );
    let analyzer =
        Analyzer::with_options(AnalysisOptions::default().with_max_document_diagrams(Some(1)));
    let descriptor = source_descriptor_for_markdown_path(Some("doc.md"));

    let rejection = analyze_document_generation(source, &analyzer, descriptor.clone())
        .into_ready()
        .expect_err("the second embedded diagram must exceed the document budget");

    assert_eq!(
        rejection.resource_limit(),
        AnalysisResourceLimit::DocumentDiagrams {
            observed_document_diagrams: 2,
            max_document_diagrams: 1,
        }
    );
    assert_eq!(rejection.payload().diagnostics.len(), 1);
    assert_eq!(
        rejection.payload().diagnostics[0].id,
        "merman.resource.document_diagrams_exceeded"
    );
    let span = rejection.payload().diagnostics[0]
        .span
        .expect("document resource rejection must retain the host span");
    assert_eq!(
        span.byte_start,
        source.match_indices("```mermaid").nth(1).unwrap().0
    );
    assert_eq!(span.byte_end, span.byte_start + "```".len());

    let facts = analyze_document_facts(source, &analyzer, descriptor);
    assert!(!facts.valid);
    assert!(facts.diagrams.is_empty());
}

#[test]
fn mdx_document_diagram_limit_counts_only_canonical_mermaid_fences() {
    let source = concat!(
        "````text\n```mermaid\nflowchart LR\nA-->B\n```\n````\n",
        "~~~ Mermaid\nflowchart TD\nA-->B\n~~~\n",
        ":::MERMAID\nsequenceDiagram\nA->>B: hi\n",
    );
    let analyzer =
        Analyzer::with_options(AnalysisOptions::default().with_max_document_diagrams(Some(1)));
    let descriptor = source_descriptor_for_markdown_path(Some("doc.mdx"));

    let rejection = analyze_document_generation(source, &analyzer, descriptor)
        .into_ready()
        .expect_err("the unclosed second Mermaid fence must exceed the limit");

    assert_eq!(
        rejection.resource_limit(),
        AnalysisResourceLimit::DocumentDiagrams {
            observed_document_diagrams: 2,
            max_document_diagrams: 1,
        }
    );
}

#[test]
fn document_diagram_limit_span_uses_host_utf16_coordinates() {
    let source = "intro 🤓\r\n  ```mermaid\nflowchart TD\nA-->B\n```\n";
    let analyzer =
        Analyzer::with_options(AnalysisOptions::default().with_max_document_diagrams(Some(0)));
    let descriptor = source_descriptor_for_markdown_path(Some("doc.md"));

    let rejection = analyze_document_generation(source, &analyzer, descriptor)
        .into_ready()
        .expect_err("the first Mermaid fence must exceed a zero diagram budget");
    let span = rejection.payload().diagnostics[0].span.unwrap();

    assert_eq!(&source[span.byte_start..span.byte_end], "```");
    assert_eq!(span.line, 2);
    assert_eq!(span.column, 3);
    assert_eq!(span.lsp_range.start.line, 1);
    assert_eq!(span.lsp_range.start.character, 2);
    assert_eq!(span.lsp_range.end.character, 5);
}

#[test]
fn standalone_diagram_ignores_host_document_diagram_limit() {
    let analyzer =
        Analyzer::with_options(AnalysisOptions::default().with_max_document_diagrams(Some(0)));

    assert!(
        analyzer
            .analyze_generation("flowchart TD\nA-->B\n")
            .into_ready()
            .is_ok()
    );
}

#[test]
fn document_diagram_limit_rule_cannot_be_disabled() {
    assert!(
        AnalysisRuleConfig::default()
            .with_rule_disabled("merman.resource.document_diagrams_exceeded")
            .is_err()
    );
}

#[test]
fn mdx_document_source_byte_limit_applies_before_fence_analysis() {
    let source = format!("```mermaid\nflowchart TD\nA-->B\n```\n{}", "x".repeat(64));
    let analyzer = Analyzer::with_options(
        AnalysisOptions::default()
            .with_max_source_bytes(Some(8))
            .with_max_document_diagrams(Some(0)),
    );
    let descriptor = source_descriptor_for_markdown_path(Some("doc.mdx"));

    let rejection = analyze_document_generation(&source, &analyzer, descriptor)
        .into_ready()
        .expect_err("document source must be rejected before fence analysis");

    assert_eq!(rejection.payload().diagnostics.len(), 1);
    assert_eq!(
        rejection.payload().diagnostics[0].id,
        "merman.resource.source_bytes_exceeded"
    );
    assert_eq!(
        rejection.resource_limit(),
        AnalysisResourceLimit::SourceBytes {
            source_len: source.len(),
            max_source_bytes: 8,
        }
    );
}

#[test]
fn panic_status_matches_binding_protocol() {
    assert_eq!(AnalysisStatus::Panic.code(), 8);
    assert_eq!(AnalysisStatus::Panic.code_name(), "MERMAN_PANIC");
}
