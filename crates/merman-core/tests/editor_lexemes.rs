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
        .parse_editor_semantic_facts_with_type_sync("classDiagram", source, ParseOptions::strict())
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
        .parse_editor_semantic_facts_with_type_sync("sequence", source, ParseOptions::strict())
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
            .parse_editor_semantic_facts_with_type_sync(
                diagram_type,
                source,
                ParseOptions::strict(),
            )
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
        .parse_editor_semantic_facts_with_type_sync("state", source, ParseOptions::strict())
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
        .parse_editor_semantic_facts_with_type_sync("state", source, ParseOptions::strict())
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
        .parse_editor_semantic_facts_with_type_sync("er", source, ParseOptions::strict())
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
        .parse_editor_semantic_facts_with_type_sync("er", source, ParseOptions::strict())
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
