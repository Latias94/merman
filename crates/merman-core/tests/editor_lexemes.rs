use merman_core::time::CivilDate;
use merman_core::{
    EditorExpectedSyntaxKind, EditorLexemeKind, EditorLexemeProducerKind,
    EditorSemanticCompleteness, EditorSemanticFacts, Engine, Error, MermaidConfig, SourceSpan,
    diagram_family_capabilities,
};
use merman_fixture_render_context::RenderContextCatalog;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

fn fixtures_root() -> PathBuf {
    workspace_root().join("fixtures")
}

fn fixture_render_contexts() -> &'static RenderContextCatalog {
    static CATALOG: OnceLock<RenderContextCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        RenderContextCatalog::load(fixtures_root()).expect("valid fixture render context catalog")
    })
}

fn fixture_site_config_for_path(path: &Path) -> Option<MermaidConfig> {
    fixture_render_contexts()
        .context_for_fixture(path)
        .unwrap_or_else(|error| {
            panic!(
                "invalid fixture render context lookup for {}: {error}",
                path.display()
            )
        })
        .map(|context| MermaidConfig::from_value(context.site_config_value()))
}

fn engine_for_fixture(base: &Engine, path: &Path) -> Engine {
    match fixture_site_config_for_path(path) {
        Some(site_config) => base.clone().with_site_config(site_config),
        None => base.clone(),
    }
}

fn list_formal_fixture_mmd_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if dir
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with('_'))
        {
            continue;
        }
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with('_'))
                {
                    continue;
                }
                stack.push(path);
                continue;
            }
            if path.extension().is_some_and(|extension| extension == "mmd") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

#[derive(Default)]
struct FormalFixtureEditorFactsAudit {
    total: usize,
    malformed_metadata: usize,
    unsupported: usize,
    supported: usize,
    available: usize,
    malformed_editor: usize,
    localized_drop_documents: usize,
}

impl FormalFixtureEditorFactsAudit {
    fn accounted(&self) -> usize {
        self.malformed_metadata + self.unsupported + self.available + self.malformed_editor
    }
}

#[test]
fn formal_fixture_corpus_keeps_supported_editor_facts_available() {
    let root = fixtures_root();
    let fixtures = list_formal_fixture_mmd_files(&root);
    assert!(
        !fixtures.is_empty(),
        "formal fixture corpus must not be empty"
    );

    let editor_capabilities = diagram_family_capabilities()
        .iter()
        .map(|capability| (capability.diagram_type, capability.has_editor_parser))
        .collect::<BTreeMap<_, _>>();
    let base = Engine::new()
        .with_fixed_today(Some(
            CivilDate::new(2026, 2, 15).expect("valid fixed fixture date"),
        ))
        .try_with_fixed_local_offset_minutes(0)
        .expect("UTC is a valid fixed offset");
    let mut audit = FormalFixtureEditorFactsAudit {
        total: fixtures.len(),
        ..Default::default()
    };
    let mut unavailable = Vec::new();
    let mut malformed_samples = Vec::new();
    let mut localized_drop_fixtures = Vec::new();
    let mut unsupported_types = BTreeMap::<String, usize>::new();

    for path in fixtures {
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        let relative = path.strip_prefix(&root).unwrap_or(&path);
        let engine = engine_for_fixture(&base, &path);
        let metadata = match engine.parse_metadata_sync(&source) {
            Ok(metadata) => metadata,
            Err(error) => {
                assert!(
                    is_malformed_fixture_error(&error),
                    "{} failed metadata extraction for a non-syntax reason: {error}",
                    relative.display()
                );
                audit.malformed_metadata += 1;
                record_sample(
                    &mut malformed_samples,
                    format!("{} (metadata): {error}", relative.display()),
                );
                continue;
            }
        };

        let has_editor_parser = editor_capabilities
            .get(metadata.diagram_type.as_str())
            .unwrap_or_else(|| {
                panic!(
                    "{} detected uncataloged diagram type {}",
                    relative.display(),
                    metadata.diagram_type
                )
            });
        if !*has_editor_parser {
            audit.unsupported += 1;
            *unsupported_types
                .entry(metadata.diagram_type.clone())
                .or_default() += 1;
            continue;
        }
        audit.supported += 1;

        match engine.parse_editor_semantic_facts_with_type_sync(&metadata.diagram_type, &source) {
            Ok(Some(facts)) => {
                audit.available += 1;
                let localized_drop_diagnostics = facts
                    .diagnostics
                    .iter()
                    .filter(|diagnostic| {
                        diagnostic
                            .message
                            .contains("editor fact span(s) that crossed a preprocessing edit")
                    })
                    .map(|diagnostic| diagnostic.message.as_str())
                    .collect::<Vec<_>>();
                if !localized_drop_diagnostics.is_empty() {
                    audit.localized_drop_documents += 1;
                    localized_drop_fixtures.push(format!(
                        "{} ({}): {}",
                        relative.display(),
                        metadata.diagram_type,
                        localized_drop_diagnostics.join("; ")
                    ));
                }
                assert_editor_fact_spans_belong_to_source(relative, &source, &facts);
            }
            Ok(None) => unavailable.push(format!(
                "{} ({})",
                relative.display(),
                metadata.diagram_type
            )),
            Err(error) => {
                assert!(
                    is_malformed_fixture_error(&error),
                    "{} ({}) failed editor parsing for a non-syntax reason: {error}",
                    relative.display(),
                    metadata.diagram_type
                );
                audit.malformed_editor += 1;
                record_sample(
                    &mut malformed_samples,
                    format!(
                        "{} ({} editor): {error}",
                        relative.display(),
                        metadata.diagram_type
                    ),
                );
            }
        }
    }

    assert_eq!(
        audit.accounted() + unavailable.len(),
        audit.total,
        "every formal fixture must receive exactly one honest outcome"
    );
    assert_eq!(
        audit.supported,
        audit.available + audit.malformed_editor + unavailable.len(),
        "every supported fixture must receive exactly one editor-parser outcome"
    );
    assert!(audit.available > 0, "the corpus must exercise editor facts");
    assert!(
        unavailable.is_empty(),
        "supported fixtures must not lose the whole editor-fact document because one span is unmappable:\n{}",
        unavailable.join("\n")
    );

    eprintln!(
        "formal fixture editor-fact audit: total={}, supported={}, available={}, localized_drops={}, malformed_editor={}, unsupported={} {:?}, malformed_metadata={}{}{}",
        audit.total,
        audit.supported,
        audit.available,
        audit.localized_drop_documents,
        audit.malformed_editor,
        audit.unsupported,
        unsupported_types,
        audit.malformed_metadata,
        if localized_drop_fixtures.is_empty() {
            String::new()
        } else {
            format!(
                "\nlocalized preprocessing drops retained document facts:\n{}",
                localized_drop_fixtures.join("\n")
            )
        },
        if malformed_samples.is_empty() {
            String::new()
        } else {
            format!("\nmalformed samples:\n{}", malformed_samples.join("\n"))
        }
    );
}

fn is_malformed_fixture_error(error: &Error) -> bool {
    matches!(
        error,
        Error::DiagramParse { .. }
            | Error::MalformedFrontMatter
            | Error::InvalidDirectiveJson { .. }
            | Error::InvalidFrontMatterYaml { .. }
    )
}

fn record_sample(samples: &mut Vec<String>, sample: String) {
    const SAMPLE_LIMIT: usize = 8;
    if samples.len() < SAMPLE_LIMIT {
        samples.push(sample);
    }
}

fn assert_editor_fact_spans_belong_to_source(
    fixture: &Path,
    source: &str,
    facts: &EditorSemanticFacts,
) {
    let fixture = fixture.display().to_string();
    for symbol in &facts.symbols {
        assert_source_span(&fixture, source, "symbol", symbol.span);
        assert_source_span(&fixture, source, "symbol selection", symbol.selection);
    }
    for lexeme in facts.lexemes() {
        assert_source_span(&fixture, source, "lexeme", lexeme.span());
    }
    for diagnostic in &facts.diagnostics {
        if let Some(span) = diagnostic.span {
            assert_source_span(&fixture, source, "diagnostic", span);
        }
    }
    for expected in &facts.expected_syntax {
        assert_source_span(&fixture, source, "expected syntax", expected.span);
    }
}

fn assert_source_span(fixture: &str, source: &str, owner: &str, span: SourceSpan) {
    assert!(
        span.start <= span.end,
        "{fixture}: {owner} span is reversed: {span:?}"
    );
    assert!(
        span.end <= source.len(),
        "{fixture}: {owner} span is out of bounds: {span:?}"
    );
    assert!(
        source.is_char_boundary(span.start) && source.is_char_boundary(span.end),
        "{fixture}: {owner} span splits a UTF-8 code point: {span:?}"
    );
}

fn family_baselines() -> BTreeMap<String, PathBuf> {
    let root = workspace_root();
    let manifest: Value = serde_json::from_str(
        &fs::read_to_string(root.join("playground/examples/manifest.json"))
            .expect("Playground example manifest"),
    )
    .expect("valid Playground example manifest");

    manifest["examples"]
        .as_array()
        .expect("examples array")
        .iter()
        .filter(|example| example["evidence"]["role"] == "family-baseline")
        .map(|example| {
            let family = example["diagramType"]
                .as_str()
                .expect("baseline diagram type")
                .to_string();
            let fixture = example["fixture"].as_str().expect("baseline fixture");
            (family, root.join(fixture))
        })
        .collect()
}

#[test]
fn every_catalog_family_emits_rich_non_overlapping_lexemes() {
    let baselines = family_baselines();
    let supported = diagram_family_capabilities()
        .iter()
        .filter_map(|family| family.metadata_id)
        .collect::<BTreeSet<_>>();
    assert_eq!(supported.len(), 35);
    assert_eq!(baselines.len(), 35);

    let engine = Engine::new();
    for family in supported {
        let fixture = baselines
            .get(family)
            .unwrap_or_else(|| panic!("missing family baseline for {family}"));
        let source = fs::read_to_string(fixture)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", fixture.display()));
        let diagram_type = engine
            .parse_metadata_sync(&source)
            .unwrap_or_else(|error| panic!("{family} detection failed: {error}"))
            .diagram_type;
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync(&diagram_type, &source)
            .unwrap_or_else(|error| panic!("{family} editor parse failed: {error}"))
            .unwrap_or_else(|| panic!("{family} has no editor facts"));
        assert_eq!(facts.lexeme_failure(), None, "{family} lexical validity");

        assert!(
            facts
                .lexemes()
                .iter()
                .any(|lexeme| lexeme.kind() == EditorLexemeKind::Keyword),
            "{family} must expose a grammar keyword: {:?}",
            facts.lexemes()
        );
        let kinds = facts
            .lexemes()
            .iter()
            .map(|lexeme| lexeme.kind())
            .collect::<BTreeSet<_>>();
        assert!(
            kinds.len() >= 2,
            "{family} baseline must expose more than its header: {kinds:?}"
        );
        assert_lexemes_are_valid_and_non_overlapping(family, &source, facts.lexemes());
    }
}

#[test]
fn global_preprocessing_owns_frontmatter_directives_and_comments() {
    let source = concat!(
        "---\n",
        "config:\n",
        "  flowchart:\n",
        "    curve: basis\n",
        "---\n",
        "%%{init: { 'theme': 'dark' }}%%\n",
        "%% an editor-visible comment\n",
        "flowchart TD\n",
        "A --> B\n",
    );
    let facts = Engine::new()
        .parse_editor_semantic_facts_with_type_sync("flowchart", source)
        .expect("editor parse")
        .expect("editor facts");
    assert_eq!(facts.lexeme_failure(), None);

    for kind in [
        EditorLexemeKind::Frontmatter,
        EditorLexemeKind::Directive,
        EditorLexemeKind::Comment,
    ] {
        assert!(
            facts.lexemes().iter().any(|lexeme| lexeme.kind() == kind),
            "missing {kind:?}: {:?}",
            facts.lexemes()
        );
    }
    assert_lexemes_are_valid_and_non_overlapping("flowchart", source, facts.lexemes());
}

#[test]
fn flowchart_and_swimlane_use_the_real_flowchart_lexer_journal() {
    let cases = [
        ("flowchart", "flowchart LR\nalpha[\"closed\"] --x beta\n"),
        (
            "swimlane",
            "swimlane-beta LR\nsubgraph Customer\nrequest[Request]\nend\n",
        ),
    ];
    let engine = Engine::new();

    for (family, source) in cases {
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync(family, source)
            .expect("editor parse")
            .expect("editor facts");
        assert_eq!(facts.lexeme_failure(), None);
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
        assert!(!facts.lexemes().is_empty());
        assert!(facts.lexemes().iter().all(|lexeme| {
            lexeme.producer().kind() == EditorLexemeProducerKind::FamilyLexer
                && lexeme.producer().family().map(|family| family.as_str()) == Some(family)
        }));
        assert!(facts.lexemes().iter().any(|lexeme| {
            let span = lexeme.span();
            lexeme.kind() == EditorLexemeKind::Keyword
                && matches!(
                    &source[span.start..span.end],
                    "flowchart" | "swimlane-beta" | "subgraph" | "end"
                )
        }));
        assert_lexemes_are_valid_and_non_overlapping(family, source, facts.lexemes());
    }
}

#[test]
fn flowchart_compound_tokens_emit_parser_owned_components() {
    let source = concat!(
        "flowchart LR\n",
        "subgraph Customer[\"Customer title\"]\n",
        "direction TB\n",
        "A[\"closed\"]\n",
        "style A fill:#fff,stroke:#000\n",
        "classDef hot fill:red,stroke:black\n",
        "class A hot\n",
        "click A href \"https://example.com\" \"Tip\" _blank\n",
        "linkStyle 0 interpolate basis stroke:red\n",
        "end\n",
    );
    let facts = Engine::new()
        .parse_editor_semantic_facts_with_type_sync("flowchart", source)
        .expect("editor parse")
        .expect("editor facts");
    assert_eq!(facts.lexeme_failure(), None);

    assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
    for (kind, text) in [
        (EditorLexemeKind::Keyword, "subgraph"),
        (EditorLexemeKind::Identifier, "Customer"),
        (EditorLexemeKind::Delimiter, "["),
        (EditorLexemeKind::String, "\"Customer title\""),
        (EditorLexemeKind::Keyword, "direction"),
        (EditorLexemeKind::Literal, "TB"),
        (EditorLexemeKind::Keyword, "style"),
        (EditorLexemeKind::Style, "fill:#fff,stroke:#000"),
        (EditorLexemeKind::Keyword, "classDef"),
        (EditorLexemeKind::Keyword, "class"),
        (EditorLexemeKind::Keyword, "click"),
        (EditorLexemeKind::Keyword, "href"),
        (EditorLexemeKind::String, "\"https://example.com\""),
        (EditorLexemeKind::Keyword, "linkStyle"),
        (EditorLexemeKind::Number, "0"),
        (EditorLexemeKind::Keyword, "interpolate"),
        (EditorLexemeKind::Literal, "basis"),
    ] {
        assert!(
            facts.lexemes().iter().any(|lexeme| {
                let span = lexeme.span();
                lexeme.kind() == kind && &source[span.start..span.end] == text
            }),
            "missing {kind:?} component {text:?}: {:?}",
            facts.lexemes()
        );
    }
    assert!(!facts.lexemes().iter().any(|lexeme| {
        let span = lexeme.span();
        source[span.start..span.end].starts_with("style A")
    }));
    assert_lexemes_are_valid_and_non_overlapping("flowchart", source, facts.lexemes());
}

#[test]
fn flowchart_labeled_edge_lexemes_keep_operator_spans_disjoint() {
    let source = "flowchart TD\nA-- \"go\" -->\u{FEFF}";
    let facts = Engine::new()
        .parse_editor_semantic_facts_with_type_sync("flowchart", source)
        .expect("flowchart editor recovery")
        .expect("flowchart editor facts");

    assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
    assert_eq!(facts.lexeme_failure(), None);

    let operator_text = facts
        .lexemes()
        .iter()
        .filter(|lexeme| lexeme.kind() == EditorLexemeKind::Operator)
        .map(|lexeme| {
            let span = lexeme.span();
            &source[span.start..span.end]
        })
        .collect::<Vec<_>>();
    assert_eq!(operator_text, ["--", "-->"]);
    assert!(facts.lexemes().iter().any(|lexeme| {
        let span = lexeme.span();
        lexeme.kind() == EditorLexemeKind::String && &source[span.start..span.end] == "\"go\""
    }));
    assert!(facts.expected_syntax.iter().any(|expected| {
        expected.kind == EditorExpectedSyntaxKind::NodeIdentifier
            && expected.span == SourceSpan::new(source.len(), source.len())
    }));
    assert_lexemes_are_valid_and_non_overlapping("flowchart", source, facts.lexemes());
}

#[test]
fn flowchart_recovery_journals_partial_strings_and_later_tokens() {
    let source = concat!(
        "flowchart TD\n",
        "A[\"closed\"] --> B\n",
        "C[\"unterminated\n",
        "D --> E\n",
    );
    let facts = Engine::new()
        .parse_editor_semantic_facts_with_type_sync("flowchart", source)
        .expect("editor recovery")
        .expect("editor facts");
    assert_eq!(facts.lexeme_failure(), None);

    assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
    assert!(facts.lexemes().iter().all(|lexeme| {
        lexeme.producer().kind() == EditorLexemeProducerKind::FamilyRecovery
            && lexeme.producer().family().map(|family| family.as_str()) == Some("flowchart")
    }));
    assert!(facts.lexemes().iter().any(|lexeme| {
        let span = lexeme.span();
        lexeme.kind() == EditorLexemeKind::String
            && source[span.start..span.end].contains("unterminated")
    }));
    assert!(
        facts.lexemes().iter().any(|lexeme| {
            let span = lexeme.span();
            lexeme.kind() == EditorLexemeKind::Identifier && &source[span.start..span.end] == "D"
        }),
        "{:?}",
        facts.lexemes()
    );
    assert_lexemes_are_valid_and_non_overlapping("flowchart", source, facts.lexemes());
}

#[test]
fn zenuml_uses_its_grammar_lexer_for_valid_and_recovered_facts() {
    let source = concat!(
        "zenuml\n",
        "Client.call(\"closed\")\n",
        "Client.call(\"unterminated\n",
        "Service.next(12ms)\n",
        "Client-->Service: reply\n",
        "date = 2026-01-15\n",
    );
    let facts = Engine::new()
        .parse_editor_semantic_facts_with_type_sync("zenuml", source)
        .expect("editor recovery")
        .expect("editor facts");
    assert_eq!(facts.lexeme_failure(), None);

    assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
    assert!(facts.lexemes().iter().all(|lexeme| {
        lexeme.producer().kind() == EditorLexemeProducerKind::FamilyRecovery
            && lexeme.producer().family().map(|family| family.as_str()) == Some("zenuml")
    }));
    assert!(facts.lexemes().iter().any(|lexeme| {
        let span = lexeme.span();
        lexeme.kind() == EditorLexemeKind::String
            && &source[span.start..span.end] == "\"unterminated"
    }));
    assert!(facts.lexemes().iter().any(|lexeme| {
        let span = lexeme.span();
        lexeme.kind() == EditorLexemeKind::Number && &source[span.start..span.end] == "12ms"
    }));
    assert!(!facts.lexemes().iter().any(|lexeme| {
        matches!(
            lexeme.kind(),
            EditorLexemeKind::Date | EditorLexemeKind::Duration
        )
    }));
    assert!(facts.lexemes().iter().any(|lexeme| {
        let span = lexeme.span();
        lexeme.kind() == EditorLexemeKind::Operator && &source[span.start..span.end] == "-->"
    }));
    assert!(facts.lexemes().iter().any(|lexeme| {
        let span = lexeme.span();
        lexeme.kind() == EditorLexemeKind::Identifier && &source[span.start..span.end] == "Service"
    }));
    assert_lexemes_are_valid_and_non_overlapping("zenuml", source, facts.lexemes());
}

#[test]
fn zenuml_companion_matrix_tokens_keep_emoji_units_and_digit_names_exact() {
    let source = concat!(
        "zenuml\n",
        "[rocket] 2FAService\n",
        "[rocket]2FAService->[lock]3DSecure.call(10ms)\n",
        "if(5xx_error)\n",
        "const result = await 3DSecure.next()\n",
    );
    let facts = Engine::new()
        .parse_editor_semantic_facts_with_type_sync("zenuml", source)
        .expect("editor parse")
        .expect("editor facts");
    assert_eq!(facts.lexeme_failure(), None);

    assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
    assert!(facts.lexemes().iter().all(|lexeme| {
        lexeme.producer().kind() == EditorLexemeProducerKind::FamilyLexer
            && lexeme.producer().family().map(|family| family.as_str()) == Some("zenuml")
    }));
    for (kind, text) in [
        (EditorLexemeKind::Delimiter, "["),
        (EditorLexemeKind::Identifier, "rocket"),
        (EditorLexemeKind::Identifier, "2FAService"),
        (EditorLexemeKind::Identifier, "3DSecure"),
        (EditorLexemeKind::Identifier, "5xx_error"),
        (EditorLexemeKind::Number, "10ms"),
        (EditorLexemeKind::Operator, "->"),
        (EditorLexemeKind::Keyword, "const"),
        (EditorLexemeKind::Keyword, "await"),
    ] {
        assert!(
            facts.lexemes().iter().any(|lexeme| {
                let span = lexeme.span();
                lexeme.kind() == kind && &source[span.start..span.end] == text
            }),
            "missing {kind:?} token {text:?}: {:?}",
            facts.lexemes()
        );
    }
    assert!(!facts.lexemes().iter().any(|lexeme| matches!(
        lexeme.kind(),
        EditorLexemeKind::Date | EditorLexemeKind::Duration
    )));
    assert_lexemes_are_valid_and_non_overlapping("zenuml", source, facts.lexemes());
}

#[test]
fn class_lexemes_come_from_the_single_lalrpop_lexer_pass() {
    let source = concat!(
        "classDiagram\n",
        "%% class comment\n",
        "direction LR\n",
        "class Order[\"Order label\"] {\n",
        "  +id: int\n",
        "}\n",
        "class Base\n",
        "Order --|> Base : inherits\n",
        "classDef hot fill:#f00,stroke:#000\n",
        "style Order fill:#fff\n",
        "click Order href \"https://example.com\" \"Open\" _blank\n",
    );
    let facts = Engine::new()
        .parse_editor_semantic_facts_with_type_sync("classDiagram", source)
        .expect("class editor parse")
        .expect("class editor facts");
    assert_eq!(facts.lexeme_failure(), None);
    assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
    assert_complete_family_lexeme_provenance(&facts, "class");

    for (kind, text) in [
        (EditorLexemeKind::Keyword, "classDiagram"),
        (EditorLexemeKind::Comment, "%% class comment\n"),
        (EditorLexemeKind::Keyword, "direction"),
        (EditorLexemeKind::Literal, "LR"),
        (EditorLexemeKind::Identifier, "Order"),
        (EditorLexemeKind::Delimiter, "["),
        (EditorLexemeKind::String, "\"Order label\""),
        (EditorLexemeKind::Delimiter, "{"),
        (EditorLexemeKind::Literal, "+id: int"),
        (EditorLexemeKind::Operator, "--"),
        (EditorLexemeKind::Operator, "|>"),
        (EditorLexemeKind::String, "inherits"),
        (EditorLexemeKind::Keyword, "classDef"),
        (EditorLexemeKind::Style, "fill:#f00,stroke:#000"),
        (EditorLexemeKind::Keyword, "href"),
        (EditorLexemeKind::String, "\"https://example.com\""),
    ] {
        assert_source_lexeme(&facts, source, kind, text);
    }
    assert_lexemes_are_valid_and_non_overlapping("class", source, facts.lexemes());
}

#[test]
fn sequence_lexemes_come_from_the_single_lalrpop_lexer_pass() {
    let source = concat!(
        "sequenceDiagram\n",
        "%% sequence comment\n",
        "title: Checkout\n",
        "autonumber 10 5\n",
        "participant Client as \"Web Client\"\n",
        "participant Service@{ \"type\": \"control\" }\n",
        "rect rgb(245,245,245)\n",
        "Client->>+Service: create order\n",
        "Service-->>-Client: accepted\n",
        "end\n",
    );
    let facts = Engine::new()
        .parse_editor_semantic_facts_with_type_sync("sequence", source)
        .expect("sequence editor parse")
        .expect("sequence editor facts");
    assert_eq!(facts.lexeme_failure(), None);
    assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
    assert_complete_family_lexeme_provenance(&facts, "sequence");

    for (kind, text) in [
        (EditorLexemeKind::Keyword, "sequenceDiagram"),
        (EditorLexemeKind::Comment, "%% sequence comment\n"),
        (EditorLexemeKind::Keyword, "title"),
        (EditorLexemeKind::Delimiter, ":"),
        (EditorLexemeKind::String, "Checkout"),
        (EditorLexemeKind::Keyword, "autonumber"),
        (EditorLexemeKind::Number, "10"),
        (EditorLexemeKind::Identifier, "Client"),
        (EditorLexemeKind::Keyword, "as"),
        (EditorLexemeKind::String, "\"Web Client\""),
        (EditorLexemeKind::Style, "@{ \"type\": \"control\" }"),
        (EditorLexemeKind::Keyword, "rect"),
        (EditorLexemeKind::Color, "rgb(245,245,245)"),
        (EditorLexemeKind::Operator, "->>"),
        (EditorLexemeKind::Operator, "+"),
        (EditorLexemeKind::String, "create order"),
        (EditorLexemeKind::Operator, "-->>"),
        (EditorLexemeKind::Operator, "-"),
        (EditorLexemeKind::Keyword, "end"),
    ] {
        assert_source_lexeme(&facts, source, kind, text);
    }
    assert_lexemes_are_valid_and_non_overlapping("sequence", source, facts.lexemes());
}

#[test]
fn class_and_sequence_recovery_keep_tokens_on_both_sides_of_the_error() {
    let cases = [
        (
            "classDiagram",
            "class",
            "classDiagram\nclass Before\n?\nclass After\n",
        ),
        (
            "sequence",
            "sequence",
            concat!(
                "sequenceDiagram\n",
                "participant Before\n",
                "?\n",
                "participant After\n",
                "Before->>After: ok\n",
            ),
        ),
    ];
    let engine = Engine::new();

    for (diagram_type, family, source) in cases {
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync(diagram_type, source)
            .expect("recoverable editor parse")
            .expect("recoverable editor facts");
        assert_eq!(facts.lexeme_failure(), None, "{family}");
        assert_eq!(
            facts.completeness,
            EditorSemanticCompleteness::Recovered,
            "{family}"
        );
        assert_recovered_family_lexeme_provenance(&facts, family);
        for identifier in ["Before", "After"] {
            assert_source_lexeme(&facts, source, EditorLexemeKind::Identifier, identifier);
        }
        assert_lexemes_are_valid_and_non_overlapping(family, source, facts.lexemes());
    }
}

fn assert_source_lexeme(
    facts: &merman_core::EditorSemanticFacts,
    source: &str,
    kind: EditorLexemeKind,
    text: &str,
) {
    assert!(
        facts.lexemes().iter().any(|lexeme| {
            let span = lexeme.span();
            lexeme.kind() == kind && &source[span.start..span.end] == text
        }),
        "missing {kind:?} token {text:?}: {:?}",
        facts.lexemes()
    );
}

fn assert_complete_family_lexeme_provenance(
    facts: &merman_core::EditorSemanticFacts,
    family: &str,
) {
    assert!(facts.lexemes().iter().any(|lexeme| {
        lexeme.producer().kind() == EditorLexemeProducerKind::FamilyLexer
            && lexeme.producer().family().map(|id| id.as_str()) == Some(family)
    }));
    assert!(facts.lexemes().iter().all(|lexeme| {
        let producer = lexeme.producer();
        match producer.kind() {
            EditorLexemeProducerKind::GlobalPreprocess => producer.family().is_none(),
            EditorLexemeProducerKind::FamilyLexer => {
                producer.family().map(|id| id.as_str()) == Some(family)
            }
            EditorLexemeProducerKind::FamilyParser | EditorLexemeProducerKind::FamilyRecovery => {
                false
            }
        }
    }));
}

fn assert_recovered_family_lexeme_provenance(
    facts: &merman_core::EditorSemanticFacts,
    family: &str,
) {
    assert!(!facts.lexemes().is_empty(), "{family}");
    assert!(facts.lexemes().iter().all(|lexeme| {
        lexeme.producer().kind() == EditorLexemeProducerKind::FamilyRecovery
            && lexeme.producer().family().map(|id| id.as_str()) == Some(family)
    }));
}

#[test]
fn state_uses_its_grammar_lexer_for_compound_tokens() {
    let source = concat!(
        "stateDiagram-v2\n",
        "direction LR trailing\n",
        "state \"Ready state\" as Ready\n",
        "Ready:::hot --> Done: complete\n",
        "classDef hot fill:#f00,stroke:#000\n",
        "class Ready,Done hot\n",
        "click Ready href \"https://example.com\"\n",
        "note right of Ready : waiting\n",
        "# state-local comment\n",
    );
    let facts = Engine::new()
        .parse_editor_semantic_facts_with_type_sync("state", source)
        .expect("state editor parse")
        .expect("state editor facts");
    assert_eq!(facts.lexeme_failure(), None);
    assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
    assert_complete_family_lexeme_provenance(&facts, "state");

    for (kind, text) in [
        (EditorLexemeKind::Keyword, "stateDiagram-v2"),
        (EditorLexemeKind::Keyword, "direction"),
        (EditorLexemeKind::Literal, "LR"),
        (EditorLexemeKind::Keyword, "state"),
        (EditorLexemeKind::String, "Ready state"),
        (EditorLexemeKind::Delimiter, ":::"),
        (EditorLexemeKind::Operator, "-->"),
        (EditorLexemeKind::Style, "fill:#f00,stroke:#000"),
        (EditorLexemeKind::Delimiter, ","),
        (EditorLexemeKind::Keyword, "href"),
        (EditorLexemeKind::String, "https://example.com"),
        (EditorLexemeKind::Keyword, "right"),
        (EditorLexemeKind::Comment, "# state-local comment"),
    ] {
        assert!(
            facts.lexemes().iter().any(|lexeme| {
                let span = lexeme.span();
                lexeme.kind() == kind && &source[span.start..span.end] == text
            }),
            "missing state {kind:?} token {text:?}: {:?}",
            facts.lexemes()
        );
    }
    assert!(!facts.lexemes().iter().any(|lexeme| {
        let span = lexeme.span();
        source[span.start..span.end].contains("trailing")
    }));
    assert_lexemes_are_valid_and_non_overlapping("state", source, facts.lexemes());
}

#[test]
fn state_lexer_recovery_keeps_tokens_after_a_malformed_middle_statement() {
    let source = concat!(
        "stateDiagram-v2\n",
        "Before --> Middle\n",
        "- malformed\n",
        "After --> End\n",
    );
    let facts = Engine::new()
        .parse_editor_semantic_facts_with_type_sync("state", source)
        .expect("state editor recovery")
        .expect("state editor facts");
    assert_eq!(facts.lexeme_failure(), None);
    assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
    assert_recovered_family_lexeme_provenance(&facts, "state");
    assert!(facts.lexemes().iter().any(|lexeme| {
        let span = lexeme.span();
        lexeme.kind() == EditorLexemeKind::Identifier && &source[span.start..span.end] == "After"
    }));
    assert_lexemes_are_valid_and_non_overlapping("state", source, facts.lexemes());
}

#[test]
fn er_uses_its_grammar_lexer_for_relationships_attributes_and_styles() {
    let source = concat!(
        "erDiagram\n",
        "direction LR trailing\n",
        "CUSTOMER[\"Customer label\"]:::hot\n",
        "CUSTOMER ||--o{ ORDER : \"places many\"\n",
        "CUSTOMER {\n",
        "  string name PK \"display name\"\n",
        "  `custom-type` value\n",
        "}\n",
        "classDef hot fill:#f00,stroke:#000\n",
        "class CUSTOMER,ORDER hot\n",
        "style ORDER fill:#fff\n",
    );
    let facts = Engine::new()
        .parse_editor_semantic_facts_with_type_sync("er", source)
        .expect("er editor parse")
        .expect("er editor facts");
    assert_eq!(facts.lexeme_failure(), None);
    assert_eq!(
        facts.completeness,
        EditorSemanticCompleteness::Complete,
        "{:?}",
        facts.diagnostics
    );
    assert_complete_family_lexeme_provenance(&facts, "er");

    for (kind, text) in [
        (EditorLexemeKind::Keyword, "erDiagram"),
        (EditorLexemeKind::Keyword, "direction"),
        (EditorLexemeKind::Literal, "LR"),
        (EditorLexemeKind::Identifier, "CUSTOMER"),
        (EditorLexemeKind::Delimiter, "["),
        (EditorLexemeKind::String, "Customer label"),
        (EditorLexemeKind::Delimiter, ":::"),
        (EditorLexemeKind::Operator, "||"),
        (EditorLexemeKind::Operator, "--"),
        (EditorLexemeKind::Operator, "o{"),
        (EditorLexemeKind::String, "places many"),
        (EditorLexemeKind::Keyword, "PK"),
        (EditorLexemeKind::Delimiter, "`"),
        (EditorLexemeKind::Identifier, "custom-type"),
        (EditorLexemeKind::Keyword, "classDef"),
        (EditorLexemeKind::Style, "fill:#f00,stroke:#000"),
        (EditorLexemeKind::Keyword, "class"),
        (EditorLexemeKind::Keyword, "style"),
    ] {
        assert!(
            facts.lexemes().iter().any(|lexeme| {
                let span = lexeme.span();
                lexeme.kind() == kind && &source[span.start..span.end] == text
            }),
            "missing er {kind:?} token {text:?}: {:?}",
            facts.lexemes()
        );
    }
    assert!(!facts.lexemes().iter().any(|lexeme| {
        let span = lexeme.span();
        source[span.start..span.end].contains("trailing")
    }));
    assert_lexemes_are_valid_and_non_overlapping("er", source, facts.lexemes());
}

#[test]
fn er_lexer_recovery_keeps_tokens_after_a_malformed_middle_statement() {
    let source = concat!(
        "erDiagram\n",
        "BEFORE ||--o{ MIDDLE : valid\n",
        "@ malformed\n",
        "AFTER ||--|| END : retained\n",
    );
    let facts = Engine::new()
        .parse_editor_semantic_facts_with_type_sync("er", source)
        .expect("er editor recovery")
        .expect("er editor facts");
    assert_eq!(facts.lexeme_failure(), None);
    assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
    assert_recovered_family_lexeme_provenance(&facts, "er");
    assert!(facts.lexemes().iter().any(|lexeme| {
        let span = lexeme.span();
        lexeme.kind() == EditorLexemeKind::Identifier && &source[span.start..span.end] == "AFTER"
    }));
    assert_lexemes_are_valid_and_non_overlapping("er", source, facts.lexemes());
}

fn assert_lexemes_are_valid_and_non_overlapping(
    family: &str,
    source: &str,
    lexemes: &[merman_core::EditorLexeme],
) {
    let mut ordered = lexemes.to_vec();
    ordered.sort_by_key(|lexeme| {
        let span = lexeme.span();
        (span.start, span.end)
    });
    for lexeme in &ordered {
        let span = lexeme.span();
        assert!(span.start < span.end, "{family}: {lexeme:?}");
        assert!(span.end <= source.len(), "{family}: {lexeme:?}");
        assert!(source.is_char_boundary(span.start), "{family}: {lexeme:?}");
        assert!(source.is_char_boundary(span.end), "{family}: {lexeme:?}");
    }
    for pair in ordered.windows(2) {
        assert!(
            pair[0].span().end <= pair[1].span().start,
            "{family} emitted overlapping lexemes: {:?}",
            pair
        );
    }
}
