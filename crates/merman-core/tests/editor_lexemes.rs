use merman_core::{
    EditorLexemeKind, EditorLexemeProducerKind, EditorSemanticCompleteness, Engine, ParseOptions,
    diagram_family_capabilities,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
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
#[ignore = "enabled after all 35 family lexical producers complete admission"]
fn every_full_profile_family_emits_rich_non_overlapping_lexemes() {
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
            .parse_metadata_sync(&source, ParseOptions::strict())
            .unwrap_or_else(|error| panic!("{family} detection failed: {error}"))
            .unwrap_or_else(|| panic!("{family} was not detected"))
            .diagram_type;
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync(
                &diagram_type,
                &source,
                ParseOptions::strict(),
            )
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
        .parse_editor_semantic_facts_with_type_sync("flowchart", source, ParseOptions::strict())
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
            .parse_editor_semantic_facts_with_type_sync(family, source, ParseOptions::strict())
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
        .parse_editor_semantic_facts_with_type_sync("flowchart", source, ParseOptions::strict())
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
fn flowchart_recovery_journals_partial_strings_and_later_tokens() {
    let source = concat!(
        "flowchart TD\n",
        "A[\"closed\"] --> B\n",
        "C[\"unterminated\n",
        "D --> E\n",
    );
    let facts = Engine::new()
        .parse_editor_semantic_facts_with_type_sync("flowchart", source, ParseOptions::strict())
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
        .parse_editor_semantic_facts_with_type_sync("zenuml", source, ParseOptions::strict())
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
        .parse_editor_semantic_facts_with_type_sync("zenuml", source, ParseOptions::strict())
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
