use super::*;
use serde_json::json;

#[test]
fn source_lint_prefers_init_directive_and_provides_fix() {
    let source = "%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
    let source_map = SourceMap::new(source);
    let config = AnalysisRuleConfig::default().with_profile(AnalysisRuleProfile::Recommended);
    let config = config.with_rule_disabled(PREFER_FRONTMATTER_CONFIG_RULE_ID);

    let diagnostics = source_lint_diagnostics(source, &source_map, &config);

    assert_eq!(diagnostics.len(), 1);
    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic.id, PREFER_INIT_DIRECTIVE_RULE_ID);
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Hint);
    let span = diagnostic.span.as_ref().expect("keyword span");
    assert_eq!(&source[span.byte_start..span.byte_end], "initialize");
    assert_eq!(diagnostic.fixes.len(), 1);
    assert_eq!(
        diagnostic.fixes[0].title,
        "Replace `initialize` with `init`"
    );
    assert!(diagnostic.fixes[0].is_preferred);
    assert_eq!(diagnostic.fixes[0].edits.len(), 1);
    assert_eq!(diagnostic.fixes[0].edits[0].replacement, "init");
    assert_eq!(
        diagnostic.fixes[0].edits[0].span.byte_start,
        span.byte_start
    );
}

#[test]
fn source_lint_prefers_frontmatter_config_over_init_directive() {
    let source = "%%{ init: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
    let source_map = SourceMap::new(source);
    let config = AnalysisRuleConfig::default().with_profile(AnalysisRuleProfile::Recommended);

    let diagnostics = source_lint_diagnostics(source, &source_map, &config);

    assert_eq!(diagnostics.len(), 1);
    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic.id, PREFER_FRONTMATTER_CONFIG_RULE_ID);
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Hint);
    assert_eq!(diagnostic.category, DiagnosticCategory::Config);
    assert_eq!(diagnostic.fixes.len(), 1);
    assert_eq!(
        diagnostic.fixes[0].title,
        "Move init directive config into frontmatter"
    );
    assert!(diagnostic.fixes[0].is_preferred);
    assert_eq!(diagnostic.fixes[0].edits.len(), 1);
    assert!(
        diagnostic.fixes[0].edits[0]
            .replacement
            .starts_with("---\nconfig:\n")
    );
    assert!(
        diagnostic.fixes[0].edits[0]
            .replacement
            .contains("theme: dark\n")
    );
    let span = diagnostic.span.as_ref().expect("directive keyword span");
    assert_eq!(&source[span.byte_start..span.byte_end], "init");
}

#[test]
fn source_lint_prefers_frontmatter_config_over_initialize_directive() {
    let source = "%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
    let source_map = SourceMap::new(source);
    let config = AnalysisRuleConfig::default()
        .with_profile(AnalysisRuleProfile::Recommended)
        .with_rule_disabled(PREFER_INIT_DIRECTIVE_RULE_ID);

    let diagnostics = source_lint_diagnostics(source, &source_map, &config);

    assert_eq!(diagnostics.len(), 1);
    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic.id, PREFER_FRONTMATTER_CONFIG_RULE_ID);
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Hint);
    assert_eq!(diagnostic.category, DiagnosticCategory::Config);
    assert_eq!(diagnostic.fixes.len(), 1);
    assert_eq!(
        diagnostic.fixes[0].title,
        "Move init directive config into frontmatter"
    );
    assert!(diagnostic.fixes[0].is_preferred);
    assert_eq!(diagnostic.fixes[0].edits.len(), 1);
    assert!(
        diagnostic.fixes[0].edits[0]
            .replacement
            .starts_with("---\nconfig:\n")
    );
    assert!(
        diagnostic.fixes[0].edits[0]
            .replacement
            .contains("theme: dark\n")
    );
    let span = diagnostic.span.as_ref().expect("directive keyword span");
    assert_eq!(&source[span.byte_start..span.byte_end], "initialize");
}

#[test]
fn source_lint_leaves_canonical_init_directive_alone() {
    let source = "%%{ init: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
    let source_map = SourceMap::new(source);
    let config = AnalysisRuleConfig::default()
        .with_profile(AnalysisRuleProfile::Recommended)
        .with_rule_disabled(PREFER_FRONTMATTER_CONFIG_RULE_ID);

    assert!(source_lint_diagnostics(source, &source_map, &config).is_empty());
}

#[test]
fn source_authoring_lints_are_not_enabled_by_core_profile() {
    let source = "%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
    let source_map = SourceMap::new(source);

    assert!(
        source_lint_diagnostics(source, &source_map, &AnalysisRuleConfig::default()).is_empty()
    );
}

#[test]
fn rule_config_can_disable_source_lint_rules() {
    let source = "%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
    let source_map = SourceMap::new(source);
    let config = AnalysisRuleConfig::default()
        .with_profile(AnalysisRuleProfile::Recommended)
        .with_rule_disabled(PREFER_INIT_DIRECTIVE_RULE_ID)
        .with_rule_disabled(PREFER_FRONTMATTER_CONFIG_RULE_ID);

    assert!(source_lint_diagnostics(source, &source_map, &config).is_empty());
}

#[test]
fn rule_config_can_enable_authoring_rules_without_recommended_profile() {
    let source = "%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
    let source_map = SourceMap::new(source);
    let config = AnalysisRuleConfig::default().with_rule_enabled(PREFER_INIT_DIRECTIVE_RULE_ID);

    let diagnostics = source_lint_diagnostics(source, &source_map, &config);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].id, PREFER_INIT_DIRECTIVE_RULE_ID);
}

#[test]
fn rule_config_can_override_rule_severity() {
    let source = "%%{ initialize: {\"theme\":\"dark\"} }%%\nflowchart TD\nA-->B\n";
    let source_map = SourceMap::new(source);
    let config = AnalysisRuleConfig::default()
        .with_profile(AnalysisRuleProfile::Recommended)
        .with_rule_disabled(PREFER_FRONTMATTER_CONFIG_RULE_ID)
        .with_rule_severity(PREFER_INIT_DIRECTIVE_RULE_ID, DiagnosticSeverity::Warning);

    let diagnostics = source_lint_diagnostics(source, &source_map, &config);

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Warning);
}

#[test]
fn source_lint_reports_deprecated_flowchart_html_labels_directive() {
    let source = "%%{init: { \"flowchart\": { \"htmlLabels\": false, \"curve\": \"linear\" } }}%%\nflowchart TD\nA-->B\n";
    let source_map = SourceMap::new(source);

    let diagnostics = source_lint_diagnostics(source, &source_map, &AnalysisRuleConfig::default());

    assert_eq!(diagnostics.len(), 1);
    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic.id, DEPRECATED_FLOWCHART_HTML_LABELS_RULE_ID);
    assert_eq!(diagnostic.severity, DiagnosticSeverity::Warning);
    assert_eq!(diagnostic.category, DiagnosticCategory::Config);
    assert!(diagnostic.fixes.is_empty());
    let span = diagnostic.span.as_ref().expect("htmlLabels span");
    assert_eq!(&source[span.byte_start..span.byte_end], "htmlLabels");
}

#[test]
fn source_lint_defers_config_wrapped_flowchart_html_labels_until_diagram_type_is_known() {
    let source = "%%{init: { \"config\": { \"flowchart\": { \"htmlLabels\": true } } }}%%\nflowchart TD\nA-->B\n";
    let source_map = SourceMap::new(source);

    let diagnostics = source_lint_diagnostics(source, &source_map, &AnalysisRuleConfig::default());

    assert!(diagnostics.is_empty());
}

#[test]
fn parsed_source_lint_reports_flowchart_config_wrapped_flowchart_html_labels_directive() {
    let source = "%%{init: { \"config\": { \"flowchart\": { \"htmlLabels\": true } } }}%%\nflowchart TD\nA-->B\n";
    let source_map = SourceMap::new(source);

    let diagnostics = parsed_source_lint_diagnostics(
        source,
        &source_map,
        &AnalysisRuleConfig::default(),
        "flowchart-v2",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].id, DEPRECATED_FLOWCHART_HTML_LABELS_RULE_ID);
    assert!(diagnostics[0].fixes.is_empty());
    let span = diagnostics[0].span.as_ref().expect("htmlLabels span");
    assert_eq!(&source[span.byte_start..span.byte_end], "htmlLabels");
}

#[test]
fn source_lint_defers_config_wrapped_root_html_labels_until_diagram_type_is_known() {
    let source = "%%{init: { \"config\": { \"htmlLabels\": true } }}%%\nflowchart TD\nA-->B\n";
    let source_map = SourceMap::new(source);

    let diagnostics = source_lint_diagnostics(source, &source_map, &AnalysisRuleConfig::default());

    assert!(diagnostics.is_empty());
}

#[test]
fn parsed_source_lint_reports_flowchart_config_wrapped_root_html_labels_directive() {
    let source = "%%{init: { \"config\": { \"htmlLabels\": true } }}%%\nflowchart TD\nA-->B\n";
    let source_map = SourceMap::new(source);

    let diagnostics = parsed_source_lint_diagnostics(
        source,
        &source_map,
        &AnalysisRuleConfig::default(),
        "flowchart-v2",
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].id, DEPRECATED_FLOWCHART_HTML_LABELS_RULE_ID);
    assert!(diagnostics[0].fixes.is_empty());
    let span = diagnostics[0].span.as_ref().expect("htmlLabels span");
    assert_eq!(&source[span.byte_start..span.byte_end], "htmlLabels");
}

#[test]
fn parsed_source_lint_does_not_report_non_flowchart_config_wrapped_html_labels() {
    let source = "%%{init: { \"config\": { \"htmlLabels\": true } }}%%\nclassDiagram\nA <|-- B\n";
    let source_map = SourceMap::new(source);

    let diagnostics = parsed_source_lint_diagnostics(
        source,
        &source_map,
        &AnalysisRuleConfig::default(),
        "classDiagram",
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn source_lint_does_not_report_class_html_labels_without_deprecation_evidence() {
    let source = "%%{init: { \"class\": { \"htmlLabels\": true } }}%%\nclassDiagram\nA <|-- B\n";
    let source_map = SourceMap::new(source);

    let diagnostics = source_lint_diagnostics(source, &source_map, &AnalysisRuleConfig::default());

    assert!(diagnostics.is_empty());
}

#[test]
fn source_lint_leaves_root_html_labels_alone() {
    let source = "%%{init: { \"htmlLabels\": false, \"flowchart\": { \"curve\": \"linear\" } }}%%\nflowchart TD\nA-->B\n";
    let source_map = SourceMap::new(source);

    assert!(
        source_lint_diagnostics(source, &source_map, &AnalysisRuleConfig::default()).is_empty()
    );
}

#[test]
fn rule_config_can_disable_deprecated_flowchart_html_labels_rule() {
    let source = "%%{init: { \"flowchart\": { \"htmlLabels\": false } }}%%\nflowchart TD\nA-->B\n";
    let source_map = SourceMap::new(source);
    let config =
        AnalysisRuleConfig::default().with_rule_disabled(DEPRECATED_FLOWCHART_HTML_LABELS_RULE_ID);

    assert!(source_lint_diagnostics(source, &source_map, &config).is_empty());
}

#[test]
fn source_lint_reports_deprecated_flowchart_html_labels_frontmatter_config() {
    let source = "---\nconfig:\n  flowchart:\n    htmlLabels: false\n---\nflowchart TD\nA-->B\n";
    let source_map = SourceMap::new(source);

    let diagnostics = source_lint_diagnostics(source, &source_map, &AnalysisRuleConfig::default());

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].id, DEPRECATED_FLOWCHART_HTML_LABELS_RULE_ID);
    let span = diagnostics[0].span.as_ref().expect("htmlLabels span");
    assert_eq!(&source[span.byte_start..span.byte_end], "htmlLabels");
}

#[test]
fn source_lint_reports_deprecated_flowchart_html_labels_flow_style_frontmatter_config() {
    let source = "---\nconfig: { flowchart: { htmlLabels: false } }\n---\nflowchart TD\nA-->B\n";
    let source_map = SourceMap::new(source);

    let diagnostics = source_lint_diagnostics(source, &source_map, &AnalysisRuleConfig::default());

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].id, DEPRECATED_FLOWCHART_HTML_LABELS_RULE_ID);
    let span = diagnostics[0].span.as_ref().expect("htmlLabels span");
    assert_eq!(&source[span.byte_start..span.byte_end], "htmlLabels");
}

#[test]
fn source_lint_reports_deprecated_external_diagram_loading_directive_config() {
    let source = "%%{init: { \"lazyLoadedDiagrams\": true, \"loadExternalDiagramsAtStartup\": false }}%%\nflowchart TD\nA-->B\n";
    let source_map = SourceMap::new(source);

    let diagnostics = source_lint_diagnostics(source, &source_map, &AnalysisRuleConfig::default());

    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().all(|diagnostic| {
        diagnostic.id == DEPRECATED_EXTERNAL_DIAGRAM_LOADING_RULE_ID
            && diagnostic.severity == DiagnosticSeverity::Warning
            && diagnostic.category == DiagnosticCategory::Config
            && diagnostic.fixes.is_empty()
    }));
    let spans: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| {
            let span = diagnostic.span.as_ref().expect("deprecated key span");
            &source[span.byte_start..span.byte_end]
        })
        .collect();
    assert_eq!(
        spans,
        vec!["lazyLoadedDiagrams", "loadExternalDiagramsAtStartup"]
    );
}

#[test]
fn source_lint_reports_deprecated_external_diagram_loading_frontmatter_config() {
    let source = "---\nconfig:\n  lazyLoadedDiagrams: true\n  loadExternalDiagramsAtStartup: false\n---\nflowchart TD\nA-->B\n";
    let source_map = SourceMap::new(source);

    let diagnostics = source_lint_diagnostics(source, &source_map, &AnalysisRuleConfig::default());

    assert_eq!(diagnostics.len(), 2);
    let spans: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| {
            assert_eq!(diagnostic.id, DEPRECATED_EXTERNAL_DIAGRAM_LOADING_RULE_ID);
            let span = diagnostic.span.as_ref().expect("deprecated key span");
            &source[span.byte_start..span.byte_end]
        })
        .collect();
    assert_eq!(
        spans,
        vec!["lazyLoadedDiagrams", "loadExternalDiagramsAtStartup"]
    );
}

#[test]
fn source_lint_reports_deprecated_external_diagram_loading_flow_style_frontmatter_config() {
    let source = "---\nconfig: { lazyLoadedDiagrams: true, loadExternalDiagramsAtStartup: false }\n---\nflowchart TD\nA-->B\n";
    let source_map = SourceMap::new(source);

    let diagnostics = source_lint_diagnostics(source, &source_map, &AnalysisRuleConfig::default());

    assert_eq!(diagnostics.len(), 2);
    let spans: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| {
            assert_eq!(diagnostic.id, DEPRECATED_EXTERNAL_DIAGRAM_LOADING_RULE_ID);
            let span = diagnostic.span.as_ref().expect("deprecated key span");
            &source[span.byte_start..span.byte_end]
        })
        .collect();
    assert_eq!(
        spans,
        vec!["lazyLoadedDiagrams", "loadExternalDiagramsAtStartup"]
    );
}

#[test]
fn rule_config_can_disable_deprecated_external_diagram_loading_rule() {
    let source = "%%{init: { \"lazyLoadedDiagrams\": true }}%%\nflowchart TD\nA-->B\n";
    let source_map = SourceMap::new(source);
    let config = AnalysisRuleConfig::default()
        .with_rule_disabled(DEPRECATED_EXTERNAL_DIAGRAM_LOADING_RULE_ID);

    assert!(source_lint_diagnostics(source, &source_map, &config).is_empty());
}

#[test]
fn rule_config_can_disable_block_warning_rules() {
    let source = "block-beta\n  columns 1\n  A:1\n  B:2\n  C:3\n";
    let source_map = SourceMap::new(source);
    let config = AnalysisRuleConfig::default().with_rule_disabled(BLOCK_WIDTH_RULE_ID);

    let diagnostics = semantic_warning_diagnostics(
        "block",
        &json!({
            "warningFacts": [
                {
                    "ruleId": BLOCK_WIDTH_WARNING_RULE_ID,
                    "message": "Block A exceeds configured column width 1"
                }
            ]
        }),
        &source_map,
        &config,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn semantic_warning_facts_use_rule_ids_when_present() {
    let source = "block-beta\n  columns 1\n  A:1\n  B:2\n  C:3\n";
    let source_map = SourceMap::new(source);

    let diagnostics = semantic_warning_diagnostics(
        "block",
        &json!({
            "warningFacts": [
                {
                    "ruleId": BLOCK_WIDTH_WARNING_RULE_ID,
                    "message": "Block A exceeds configured column width 1"
                }
            ]
        }),
        &source_map,
        &AnalysisRuleConfig::default(),
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].id, BLOCK_WIDTH_RULE_ID);
    assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Warning);
}

#[test]
fn semantic_warning_facts_map_flowchart_missing_direction_rule_id() {
    let source = "flowchart\nA-->B\n";
    let source_map = SourceMap::new(source);
    let config = AnalysisRuleConfig::default().with_profile(AnalysisRuleProfile::Recommended);

    let diagnostics = semantic_warning_diagnostics(
        "flowchart-v2",
        &json!({
            "warningFacts": [
                {
                    "ruleId": FLOWCHART_EXPLICIT_DIRECTION_WARNING_RULE_ID,
                    "message": "flowchart headers should declare an explicit direction",
                    "span": { "start": 0, "end": 9 },
                    "fixSpan": { "start": 9, "end": 9 }
                }
            ]
        }),
        &source_map,
        &config,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].id, FLOWCHART_EXPLICIT_DIRECTION_RULE_ID);
    assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Hint);
    assert_eq!(diagnostics[0].category, DiagnosticCategory::Semantic);
    assert_eq!(diagnostics[0].diagram_type.as_deref(), Some("flowchart-v2"));
    assert_eq!(diagnostics[0].span.as_ref().unwrap().byte_start, 0);
    assert_eq!(diagnostics[0].span.as_ref().unwrap().byte_end, 9);
    assert_eq!(diagnostics[0].fixes.len(), 1);
    assert_eq!(
        diagnostics[0].fixes[0].title,
        "Insert `TB` into the flowchart header"
    );
    assert!(diagnostics[0].fixes[0].is_preferred);
    assert_eq!(diagnostics[0].fixes[0].edits[0].replacement, " TB");
    assert_eq!(diagnostics[0].fixes[0].edits[0].span.byte_start, 9);
    assert_eq!(diagnostics[0].fixes[0].edits[0].span.byte_end, 9);
}

#[test]
fn semantic_warning_facts_map_flowchart_unknown_style_target_rule_id() {
    let source = "flowchart TD\nstyle Q background:#fff\nA-->B\n";
    let source_map = SourceMap::new(source);

    let diagnostics = semantic_warning_diagnostics(
        "flowchart-v2",
        &json!({
            "warningFacts": [
                {
                    "ruleId": FLOWCHART_UNKNOWN_STYLE_TARGET_WARNING_RULE_ID,
                    "message": "Style applied to unknown node \"Q\". This may indicate a typo. The node will be created automatically.",
                    "span": { "start": 19, "end": 20 }
                }
            ]
        }),
        &source_map,
        &AnalysisRuleConfig::default(),
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].id, FLOWCHART_UNKNOWN_STYLE_TARGET_RULE_ID);
    assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Warning);
    assert_eq!(diagnostics[0].category, DiagnosticCategory::Semantic);
    assert_eq!(diagnostics[0].span.as_ref().unwrap().byte_start, 19);
    assert_eq!(diagnostics[0].span.as_ref().unwrap().byte_end, 20);
}

#[test]
fn semantic_authoring_warning_facts_are_not_enabled_by_core_profile() {
    let source = "flowchart\nA-->B\n";
    let source_map = SourceMap::new(source);

    let diagnostics = semantic_warning_diagnostics(
        "flowchart-v2",
        &json!({
            "warningFacts": [
                {
                    "ruleId": FLOWCHART_EXPLICIT_DIRECTION_WARNING_RULE_ID,
                    "message": "flowchart headers should declare an explicit direction",
                    "span": { "start": 0, "end": 9 },
                    "fixSpan": { "start": 9, "end": 9 }
                }
            ]
        }),
        &source_map,
        &AnalysisRuleConfig::default(),
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn rule_config_can_override_block_warning_severity() {
    let source = "block-beta\n  columns 1\n  A:1\n  B:2\n  C:3\n";
    let source_map = SourceMap::new(source);
    let config = AnalysisRuleConfig::default()
        .with_rule_severity(BLOCK_WIDTH_RULE_ID, DiagnosticSeverity::Hint);

    let diagnostics = semantic_warning_diagnostics(
        "block",
        &json!({
            "warningFacts": [
                {
                    "ruleId": BLOCK_WIDTH_WARNING_RULE_ID,
                    "message": "Block A exceeds configured column width 1"
                }
            ]
        }),
        &source_map,
        &config,
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Hint);
    assert_eq!(diagnostics[0].id, BLOCK_WIDTH_RULE_ID);
}

#[test]
fn rule_descriptors_expose_stable_rule_metadata() {
    let descriptors = rule_descriptors();

    assert_eq!(descriptors.len(), 19);
    assert_eq!(descriptors[0].id, PREFER_INIT_DIRECTIVE_RULE_ID);
    assert!(descriptors[0].description.contains("canonical `init`"));
    assert_eq!(descriptors[0].default_severity, DiagnosticSeverity::Hint);
    assert_eq!(descriptors[0].category, DiagnosticCategory::Config);
    assert!(!descriptors[0].default_enabled);
    assert_eq!(
        descriptors[0].default_profile,
        AnalysisRuleProfile::Recommended
    );
    assert_eq!(descriptors[0].origin, RuleOrigin::MermanAuthoring);
    assert!(descriptors[0].fixable);
    let prefer_frontmatter = descriptors
        .iter()
        .find(|descriptor| descriptor.id == PREFER_FRONTMATTER_CONFIG_RULE_ID)
        .expect("prefer frontmatter config descriptor");
    assert!(
        prefer_frontmatter
            .description
            .contains("frontmatter `config`")
    );
    assert_eq!(prefer_frontmatter.origin, RuleOrigin::MermanAuthoring);
    assert_eq!(
        prefer_frontmatter.default_profile,
        AnalysisRuleProfile::Recommended
    );
    assert_eq!(
        prefer_frontmatter.default_severity,
        DiagnosticSeverity::Hint
    );
    assert_eq!(prefer_frontmatter.category, DiagnosticCategory::Config);
    assert!(!prefer_frontmatter.default_enabled);
    assert!(prefer_frontmatter.fixable);
    assert!(
            prefer_frontmatter
                .evidence
                .contains(&"https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/docs/config/directives.md")
        );
    assert!(
            prefer_frontmatter
                .evidence
                .contains(&"https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/docs/config/configuration.md")
        );
    let deprecated_html_labels = descriptors
        .iter()
        .find(|descriptor| descriptor.id == DEPRECATED_FLOWCHART_HTML_LABELS_RULE_ID)
        .expect("deprecated htmlLabels descriptor");
    assert_eq!(
        deprecated_html_labels.origin,
        RuleOrigin::MermaidCompatibility
    );
    assert!(deprecated_html_labels.default_enabled);
    assert_eq!(
        deprecated_html_labels.default_profile,
        AnalysisRuleProfile::Core
    );
    assert_eq!(
        deprecated_html_labels.default_severity,
        DiagnosticSeverity::Warning
    );
    assert_eq!(deprecated_html_labels.category, DiagnosticCategory::Config);
    assert!(!deprecated_html_labels.fixable);
    assert!(
            deprecated_html_labels
                .evidence
                .contains(&"https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/docs/config/directives.md")
        );
    assert!(
        descriptors
            .iter()
            .any(|descriptor| descriptor.id == NO_DIAGRAM_RULE_ID)
    );
    assert!(
        descriptors
            .iter()
            .any(|descriptor| descriptor.id == DIAGRAM_PARSE_RULE_ID)
    );
    assert!(
        descriptors
            .iter()
            .any(|descriptor| descriptor.id == UNSUPPORTED_DIAGRAM_RULE_ID)
    );
    assert!(
        descriptors
            .iter()
            .any(|descriptor| descriptor.id == RECOVERED_EDITOR_FACTS_RULE_ID)
    );
    assert!(
        descriptors
            .iter()
            .any(|descriptor| descriptor.id == RESOURCE_LIMIT_RULE_ID)
    );
    assert!(
        descriptors
            .iter()
            .any(|descriptor| descriptor.id == MALFORMED_FRONT_MATTER_RULE_ID)
    );
    assert!(
        descriptors
            .iter()
            .any(|descriptor| descriptor.id == INVALID_DIRECTIVE_JSON_RULE_ID)
    );
    assert!(
        descriptors
            .iter()
            .any(|descriptor| descriptor.id == INVALID_FRONT_MATTER_YAML_RULE_ID)
    );
    assert!(
        descriptors
            .iter()
            .any(|descriptor| descriptor.id == PANIC_RULE_ID)
    );
    assert!(
        descriptors
            .iter()
            .any(|descriptor| descriptor.id == INTERNAL_RULE_REGISTRY_GAP_RULE_ID)
    );
    assert!(
        descriptors
            .iter()
            .any(|descriptor| descriptor.id == FLOWCHART_FACTS_PROJECTION_RULE_ID)
    );
    assert!(
        descriptors
            .iter()
            .any(|descriptor| descriptor.id == PREFER_FRONTMATTER_CONFIG_RULE_ID)
    );
    assert!(
        descriptors
            .iter()
            .any(|descriptor| descriptor.id == BLOCK_WIDTH_RULE_ID)
    );
    assert!(
        descriptors
            .iter()
            .any(|descriptor| descriptor.id == FLOWCHART_EXPLICIT_DIRECTION_RULE_ID)
    );
    assert!(
        descriptors
            .iter()
            .any(|descriptor| descriptor.id == DEPRECATED_FLOWCHART_HTML_LABELS_RULE_ID)
    );
    assert!(
        descriptors
            .iter()
            .any(|descriptor| descriptor.id == DEPRECATED_EXTERNAL_DIAGRAM_LOADING_RULE_ID)
    );
    let deprecated_external_loading = descriptors
        .iter()
        .find(|descriptor| descriptor.id == DEPRECATED_EXTERNAL_DIAGRAM_LOADING_RULE_ID)
        .expect("deprecated external diagram loading descriptor");
    assert_eq!(
        deprecated_external_loading.origin,
        RuleOrigin::MermaidCompatibility
    );
    assert!(deprecated_external_loading.default_enabled);
    assert_eq!(
        deprecated_external_loading.default_profile,
        AnalysisRuleProfile::Core
    );
    assert_eq!(
        deprecated_external_loading.default_severity,
        DiagnosticSeverity::Warning
    );
    assert_eq!(
        deprecated_external_loading.category,
        DiagnosticCategory::Config
    );
    assert!(!deprecated_external_loading.fixable);
    assert!(
            deprecated_external_loading
                .evidence
                .contains(&"https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/config.ts")
        );
    assert!(
        descriptors
            .iter()
            .find(|descriptor| descriptor.id == FLOWCHART_EXPLICIT_DIRECTION_RULE_ID)
            .is_some_and(|descriptor| {
                descriptor.fixable
                    && !descriptor.default_enabled
                    && descriptor.default_profile == AnalysisRuleProfile::Recommended
                    && descriptor.origin == RuleOrigin::MermanAuthoring
            })
    );
    let flowchart_unknown_style = descriptors
        .iter()
        .find(|descriptor| descriptor.id == FLOWCHART_UNKNOWN_STYLE_TARGET_RULE_ID)
        .expect("flowchart unknown style target descriptor");
    assert_eq!(
        flowchart_unknown_style.default_severity,
        DiagnosticSeverity::Warning
    );
    assert_eq!(
        flowchart_unknown_style.category,
        DiagnosticCategory::Semantic
    );
    assert!(flowchart_unknown_style.default_enabled);
    assert_eq!(
        flowchart_unknown_style.default_profile,
        AnalysisRuleProfile::Core
    );
    assert_eq!(
        flowchart_unknown_style.origin,
        RuleOrigin::MermaidCompatibility
    );
    assert!(!flowchart_unknown_style.fixable);
    assert!(
        descriptors
            .iter()
            .any(|descriptor| descriptor.id == GIT_GRAPH_DUPLICATE_COMMIT_RULE_ID)
    );
}

#[test]
fn rule_descriptors_enforce_governance_boundaries() {
    let mut ids = std::collections::BTreeSet::new();

    for descriptor in rule_descriptors() {
        assert!(
            descriptor.id.starts_with("merman."),
            "{} must stay in the Merman-owned rule namespace",
            descriptor.id
        );
        assert!(
            ids.insert(descriptor.id),
            "duplicate rule id {}",
            descriptor.id
        );
        assert!(
            !descriptor.evidence.is_empty(),
            "{} must publish evidence",
            descriptor.id
        );

        match descriptor.origin {
            RuleOrigin::MermaidSyntax | RuleOrigin::MermaidCompatibility => {
                assert!(
                    descriptor.evidence.iter().any(|evidence| evidence
                        .starts_with("https://github.com/mermaid-js/mermaid/blob/")),
                    "{} must cite public Mermaid source evidence",
                    descriptor.id
                );
                assert_eq!(
                    descriptor.default_profile,
                    AnalysisRuleProfile::Core,
                    "{} is Mermaid-backed and should remain in the conservative core profile",
                    descriptor.id
                );
            }
            RuleOrigin::MermanAuthoring => {
                assert!(
                    descriptor.id.starts_with("merman.authoring."),
                    "{} must make Merman authoring authority explicit",
                    descriptor.id
                );
                assert!(
                    !descriptor.default_enabled,
                    "{} must be opt-in outside the recommended profile",
                    descriptor.id
                );
                assert_eq!(
                    descriptor.default_profile,
                    AnalysisRuleProfile::Recommended,
                    "{} must not be enabled by the core profile",
                    descriptor.id
                );
                assert_eq!(
                    descriptor.default_severity,
                    DiagnosticSeverity::Hint,
                    "{} must surface as an authoring hint by default",
                    descriptor.id
                );
            }
            RuleOrigin::MermanResourcePolicy => {
                assert_eq!(descriptor.default_profile, AnalysisRuleProfile::Core);
                assert_eq!(descriptor.category, DiagnosticCategory::Resource);
            }
            RuleOrigin::MermanInternal => {
                assert_eq!(descriptor.category, DiagnosticCategory::Internal);
            }
        }
    }
}

#[test]
fn semantic_warning_facts_use_rule_ids_even_when_messages_differ() {
    let source = "block-beta\n  columns 1\n  A:1\n  B:2\n  C:3\n";
    let source_map = SourceMap::new(source);

    let diagnostics = semantic_warning_diagnostics(
        "block",
        &json!({
            "warningFacts": [
                {
                    "ruleId": BLOCK_WIDTH_WARNING_RULE_ID,
                    "message": "this message does not need to mention width"
                }
            ]
        }),
        &source_map,
        &AnalysisRuleConfig::default(),
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].id, BLOCK_WIDTH_RULE_ID);
}

#[test]
fn semantic_warning_facts_surface_unknown_rule_ids_as_internal_errors() {
    let source = "block-beta\n  columns 1\n  A:1\n  B:2\n  C:3\n";
    let source_map = SourceMap::new(source);

    let diagnostics = semantic_warning_diagnostics(
        "block",
        &json!({
            "warningFacts": [
                {
                    "ruleId": "merman.block.unregistered_warning",
                    "message": "Block A emitted a future warning"
                }
            ]
        }),
        &source_map,
        &AnalysisRuleConfig::default(),
    );

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].id, INTERNAL_RULE_REGISTRY_GAP_RULE_ID);
    assert_eq!(diagnostics[0].category, DiagnosticCategory::Internal);
    assert_eq!(
        diagnostics[0].code,
        Some(AnalysisStatus::InternalError.code())
    );
}

#[test]
fn configurable_rule_descriptors_exclude_internal_and_resource_rules() {
    let descriptors: Vec<_> = configurable_rule_descriptors().collect();

    assert!(descriptors.iter().all(|descriptor| !matches!(
        descriptor.category,
        DiagnosticCategory::Internal | DiagnosticCategory::Resource
    )));
    assert!(
        descriptors
            .iter()
            .any(|descriptor| descriptor.id == BLOCK_WIDTH_RULE_ID)
    );
    assert!(
        descriptors
            .iter()
            .any(|descriptor| descriptor.id == FLOWCHART_EXPLICIT_DIRECTION_RULE_ID)
    );
    assert!(
        descriptors
            .iter()
            .any(|descriptor| descriptor.id == DEPRECATED_FLOWCHART_HTML_LABELS_RULE_ID)
    );
    assert!(
        descriptors
            .iter()
            .all(|descriptor| descriptor.id != PANIC_RULE_ID)
    );
    assert!(
        descriptors
            .iter()
            .all(|descriptor| descriptor.id != RESOURCE_LIMIT_RULE_ID)
    );
    assert!(
        descriptors
            .iter()
            .all(|descriptor| descriptor.id != INTERNAL_RULE_REGISTRY_GAP_RULE_ID)
    );
    assert!(
        descriptors
            .iter()
            .all(|descriptor| descriptor.id != FLOWCHART_FACTS_PROJECTION_RULE_ID)
    );
}

#[test]
fn rule_catalog_serializes_public_rule_metadata() {
    let catalog = rule_catalog();
    let prefer_init = catalog
        .iter()
        .find(|entry| entry.id == PREFER_INIT_DIRECTIVE_RULE_ID)
        .expect("prefer init catalog entry");

    assert!(prefer_init.description.contains("canonical `init`"));
    assert_eq!(prefer_init.origin, RuleOrigin::MermanAuthoring);
    assert_eq!(
        prefer_init.default_profile,
        AnalysisRuleProfile::Recommended
    );
    assert!(prefer_init.configurable);
    assert!(prefer_init.fixable);
    assert!(
        prefer_init
            .evidence
            .contains(&"docs/adr/0072-lint-rule-governance.md")
    );
    let prefer_frontmatter = catalog
        .iter()
        .find(|entry| entry.id == PREFER_FRONTMATTER_CONFIG_RULE_ID)
        .expect("prefer frontmatter catalog entry");
    assert!(
        prefer_frontmatter
            .description
            .contains("frontmatter `config`")
    );
    assert_eq!(prefer_frontmatter.origin, RuleOrigin::MermanAuthoring);
    assert_eq!(
        prefer_frontmatter.default_profile,
        AnalysisRuleProfile::Recommended
    );
    assert_eq!(
        prefer_frontmatter.default_severity,
        DiagnosticSeverity::Hint
    );
    assert_eq!(prefer_frontmatter.category, DiagnosticCategory::Config);
    assert!(prefer_frontmatter.configurable);
    assert!(prefer_frontmatter.fixable);
    assert!(
            prefer_frontmatter
                .evidence
                .contains(&"https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/docs/config/directives.md")
        );
    assert!(
            prefer_frontmatter
                .evidence
                .contains(&"https://github.com/mermaid-js/mermaid/blob/41646dfd43ac83f001b03c70605feb036afae46d/packages/mermaid/src/docs/config/configuration.md")
        );
    assert!(catalog.iter().all(|entry| !entry.evidence.is_empty()));
    assert!(catalog.iter().all(|entry| {
        entry
            .evidence
            .iter()
            .all(|evidence| !evidence.starts_with("crates/"))
    }));
    let deprecated_html_labels = catalog
        .iter()
        .find(|entry| entry.id == DEPRECATED_FLOWCHART_HTML_LABELS_RULE_ID)
        .expect("deprecated htmlLabels catalog entry");
    assert_eq!(
        deprecated_html_labels.origin,
        RuleOrigin::MermaidCompatibility
    );
    assert!(deprecated_html_labels.default_enabled);
    assert!(!deprecated_html_labels.fixable);
    let resource_limit = catalog
        .iter()
        .find(|entry| entry.id == RESOURCE_LIMIT_RULE_ID)
        .expect("resource limit catalog entry");
    assert_eq!(resource_limit.category, DiagnosticCategory::Resource);
    assert!(!resource_limit.configurable);

    let response = rule_catalog_response();
    assert_eq!(response.version, RULE_CATALOG_RESPONSE_VERSION);
    assert_eq!(response.rules.len(), catalog.len());

    let response_json: serde_json::Value =
        serde_json::from_slice(&rule_catalog_response_json_bytes().expect("catalog response JSON"))
            .expect("catalog response should serialize as JSON");
    assert_eq!(response_json["version"], RULE_CATALOG_RESPONSE_VERSION);
    let response_rules = response_json["rules"]
        .as_array()
        .expect("catalog response rules array");
    let first = response_rules.first().unwrap();
    assert_eq!(first["id"], PREFER_INIT_DIRECTIVE_RULE_ID);
    assert_eq!(first["origin"], "merman_authoring");
    assert_eq!(first["default_profile"], "recommended");
    assert_eq!(first["default_severity"], "hint");
    assert_eq!(first["category"], "config");
    assert_eq!(first["configurable"], true);
    assert_eq!(first["fixable"], true);
    assert!(
        first["evidence"]
            .as_array()
            .expect("evidence array")
            .iter()
            .any(|value| value == "docs/adr/0072-lint-rule-governance.md")
    );
}

#[test]
fn configurable_rule_catalog_excludes_internal_and_resource_rules() {
    let catalog = configurable_rule_catalog();

    assert!(catalog.iter().all(|entry| !matches!(
        entry.category,
        DiagnosticCategory::Internal | DiagnosticCategory::Resource
    )));
    assert!(
        catalog
            .iter()
            .any(|entry| entry.id == PREFER_FRONTMATTER_CONFIG_RULE_ID)
    );
    assert!(
        catalog
            .iter()
            .any(|entry| entry.id == FLOWCHART_EXPLICIT_DIRECTION_RULE_ID)
    );
    assert!(
        catalog
            .iter()
            .any(|entry| entry.id == DEPRECATED_FLOWCHART_HTML_LABELS_RULE_ID)
    );
    assert!(
        catalog
            .iter()
            .all(|entry| entry.id != INTERNAL_RULE_REGISTRY_GAP_RULE_ID)
    );
    assert!(
        catalog
            .iter()
            .all(|entry| entry.id != RESOURCE_LIMIT_RULE_ID)
    );
    assert!(
        catalog
            .iter()
            .all(|entry| entry.id != FLOWCHART_FACTS_PROJECTION_RULE_ID)
    );
}
