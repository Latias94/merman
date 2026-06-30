use merman_analysis::{
    AnalysisOptions, AnalysisRuleConfig, AnalysisRuleProfile, AnalysisStatus, Analyzer,
    DiagnosticCategory, DiagnosticSeverity, SourceDescriptor, document::analyze_document,
    markdown::markdown_source_descriptor,
};

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
            expected_diagram_type: "classDiagram",
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
                    .any(|related| related.message.contains("fallback")),
            "{} only fallback parse spans should add related fallback context",
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
        markdown_source_descriptor(Some("doc.md")),
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
fn recovered_gantt_editor_diagnostic_is_projected() {
    let source = "gantt\nweekday foo\n";
    let payload = analyze(source);

    assert!(!payload.valid);
    assert_eq!(payload.summary.errors, 1);
    assert_eq!(payload.summary.warnings, 1);
    let diagnostic = payload
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.id == "merman.parse.recovered_editor_facts")
        .expect("recovered editor diagnostic");
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Warning);
    assert_eq!(diagnostic.category, DiagnosticCategory::Parse);
    assert_eq!(diagnostic.diagram_type.as_deref(), Some("gantt"));
    assert!(diagnostic.message.contains("invalid weekday"));
    assert_eq!(
        diagnostic.span.as_ref().map(|span| span.byte_start),
        source.find("foo")
    );
}

#[test]
fn recovered_mindmap_editor_diagnostic_is_projected() {
    let source = "mindmap\nroot\n child[unterminated";
    let payload = analyze(source);

    assert!(!payload.valid);
    assert_eq!(payload.summary.errors, 1);
    assert_eq!(payload.summary.warnings, 1);
    let diagnostic = payload
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.id == "merman.parse.recovered_editor_facts")
        .expect("recovered editor diagnostic");
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Warning);
    assert_eq!(diagnostic.category, DiagnosticCategory::Parse);
    assert_eq!(diagnostic.diagram_type.as_deref(), Some("mindmap"));
    assert!(diagnostic.message.contains("unterminated node delimiter"));
    assert_eq!(
        diagnostic.span.as_ref().map(|span| span.byte_start),
        source.find("child")
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
fn flowchart_missing_direction_rule_can_be_disabled() {
    let options = AnalysisOptions::default().with_rule_config(
        AnalysisRuleConfig::default()
            .with_profile(AnalysisRuleProfile::Recommended)
            .with_rule_disabled("merman.authoring.flowchart.explicit_direction"),
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
    assert_eq!(diagnostic.fixes.len(), 1);
    assert_eq!(
        diagnostic.fixes[0].title,
        "Move deprecated `flowchart.htmlLabels` to root `htmlLabels`"
    );
    assert!(diagnostic.fixes[0].is_preferred);
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
fn class_html_labels_config_is_not_a_core_compatibility_warning() {
    let source = "%%{init: { \"class\": { \"htmlLabels\": true } }}%%\nclassDiagram\nA <|-- B\n";
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

    let payload = Analyzer::with_engine_and_options(engine, AnalysisOptions::default())
        .analyze("flowchart TD\nA-->B\n");

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
fn panic_status_matches_binding_protocol() {
    assert_eq!(AnalysisStatus::Panic.code(), 8);
    assert_eq!(AnalysisStatus::Panic.code_name(), "MERMAN_PANIC");
}
