use merman_core::{
    EditorLexemeKind, EditorLexemeProducerKind, EditorSemanticCompleteness, EditorSemanticFacts,
    Engine, ParseOptions,
};

struct CompleteCase {
    family: &'static str,
    source: &'static str,
    expected: &'static [(EditorLexemeKind, &'static str)],
}

#[test]
fn structured_langium_families_emit_exact_parser_lexemes() {
    let cases = [
        CompleteCase {
            family: "cynefin",
            source: concat!(
                "cynefin-beta:\n",
                "%% cynefin comment\n",
                "title Decision map\n",
                "complex \"Emergent practice\"\n",
                "complicated --> clear : \"Move\"\n",
            ),
            expected: &[
                (EditorLexemeKind::Keyword, "cynefin-beta"),
                (EditorLexemeKind::Delimiter, ":"),
                (EditorLexemeKind::Comment, "%% cynefin comment\n"),
                (EditorLexemeKind::Keyword, "title"),
                (EditorLexemeKind::String, "Decision map"),
                (EditorLexemeKind::Keyword, "complex"),
                (EditorLexemeKind::String, "\"Emergent practice\""),
                (EditorLexemeKind::Keyword, "complicated"),
                (EditorLexemeKind::Operator, "-->"),
                (EditorLexemeKind::Keyword, "clear"),
                (EditorLexemeKind::String, "\"Move\""),
            ],
        },
        CompleteCase {
            family: "treeView",
            source: concat!(
                "treeView-beta\n",
                "%% tree comment\n",
                "title Project tree\n",
                "my-project/\n",
                "    src/ :::source icon(folder) ## Source files\n",
                "        index.js\n",
                "    package.json\n",
            ),
            expected: &[
                (EditorLexemeKind::Keyword, "treeView-beta"),
                (EditorLexemeKind::Comment, "%% tree comment\n"),
                (EditorLexemeKind::Keyword, "title"),
                (EditorLexemeKind::String, "Project tree"),
                (EditorLexemeKind::Identifier, "my-project"),
                (EditorLexemeKind::Delimiter, "/"),
                (EditorLexemeKind::Identifier, "src"),
                (EditorLexemeKind::Delimiter, ":::"),
                (EditorLexemeKind::Identifier, "source"),
                (EditorLexemeKind::Keyword, "icon"),
                (EditorLexemeKind::Delimiter, "("),
                (EditorLexemeKind::Literal, "folder"),
                (EditorLexemeKind::Delimiter, ")"),
                (EditorLexemeKind::Delimiter, "##"),
                (EditorLexemeKind::String, "Source files"),
                (EditorLexemeKind::Identifier, "index.js"),
                (EditorLexemeKind::Identifier, "package.json"),
            ],
        },
        CompleteCase {
            family: "treemap",
            source: concat!(
                "treemap-beta\n",
                "%% treemap comment\n",
                "title Storage map\n",
                "classDef hot fill:#f00,stroke:#000;\n",
                "\"Root\":::hot\n",
                "  \"Leaf\": 10:::hot\n",
            ),
            expected: &[
                (EditorLexemeKind::Keyword, "treemap-beta"),
                (EditorLexemeKind::Comment, "%% treemap comment\n"),
                (EditorLexemeKind::Keyword, "title"),
                (EditorLexemeKind::String, "Storage map"),
                (EditorLexemeKind::Keyword, "classDef"),
                (EditorLexemeKind::Identifier, "hot"),
                (EditorLexemeKind::Style, "fill:#f00,stroke:#000"),
                (EditorLexemeKind::String, "\"Root\""),
                (EditorLexemeKind::Delimiter, ":::"),
                (EditorLexemeKind::String, "\"Leaf\""),
                (EditorLexemeKind::Delimiter, ":"),
                (EditorLexemeKind::Number, "10"),
            ],
        },
        CompleteCase {
            family: "venn",
            source: concat!(
                "venn-beta\n",
                "%% venn comment\n",
                "title Overlap map\n",
                "set A[\"Alpha\"]:20\n",
                "set B[Beta]:12\n",
                "union A,B[\"Both\"]:5\n",
                "style A,B fill:#ff0000, color:rgb(0, 0, 0)\n",
            ),
            expected: &[
                (EditorLexemeKind::Keyword, "venn-beta"),
                (EditorLexemeKind::Comment, "%% venn comment\n"),
                (EditorLexemeKind::Keyword, "title"),
                (EditorLexemeKind::String, "Overlap map"),
                (EditorLexemeKind::Keyword, "set"),
                (EditorLexemeKind::Identifier, "A"),
                (EditorLexemeKind::Delimiter, "["),
                (EditorLexemeKind::String, "\"Alpha\""),
                (EditorLexemeKind::Delimiter, "]"),
                (EditorLexemeKind::Delimiter, ":"),
                (EditorLexemeKind::Number, "20"),
                (EditorLexemeKind::String, "Beta"),
                (EditorLexemeKind::Keyword, "union"),
                (EditorLexemeKind::Delimiter, ","),
                (EditorLexemeKind::Keyword, "style"),
                (EditorLexemeKind::Identifier, "fill"),
                (EditorLexemeKind::Style, "#ff0000"),
                (EditorLexemeKind::Identifier, "color"),
                (EditorLexemeKind::Style, "rgb(0, 0, 0)"),
            ],
        },
    ];

    let engine = Engine::new();
    for case in cases {
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync(
                case.family,
                case.source,
                ParseOptions::strict(),
            )
            .unwrap_or_else(|error| panic!("{} editor parse failed: {error}", case.family))
            .unwrap_or_else(|| panic!("{} editor facts unavailable", case.family));

        assert_eq!(facts.lexeme_failure(), None, "{}", case.family);
        assert_eq!(
            facts.completeness,
            EditorSemanticCompleteness::Complete,
            "{}",
            case.family
        );
        assert_complete_provenance(&facts, case.family);
        for &(kind, text) in case.expected {
            assert_source_lexeme(&facts, case.source, kind, text);
        }
        assert_non_overlapping(&facts, case.source);
    }
}

#[test]
fn structured_langium_recovery_keeps_tokens_after_a_malformed_middle_line() {
    let cases = [
        (
            "cynefin",
            "cynefin-beta\ncomplex \"Before\"\n?\nclear \"After\"\n",
            (EditorLexemeKind::String, "\"Before\""),
            (EditorLexemeKind::String, "\"After\""),
        ),
        (
            "treeView",
            "treeView-beta\nBefore\n:::broken\nAfter\n",
            (EditorLexemeKind::Identifier, "Before"),
            (EditorLexemeKind::Identifier, "After"),
        ),
        (
            "treemap",
            "treemap\n\"Before\": 1\n?\n\"After\": 2\n",
            (EditorLexemeKind::String, "\"Before\""),
            (EditorLexemeKind::String, "\"After\""),
        ),
        (
            "venn",
            "venn-beta\nset Before\n?\nset After\n",
            (EditorLexemeKind::Identifier, "Before"),
            (EditorLexemeKind::Identifier, "After"),
        ),
    ];

    let engine = Engine::new();
    for (family, source, before, after) in cases {
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync(family, source, ParseOptions::strict())
            .unwrap_or_else(|error| panic!("{family} recovery failed: {error}"))
            .unwrap_or_else(|| panic!("{family} editor facts unavailable"));

        assert_eq!(facts.lexeme_failure(), None, "{family}");
        assert_eq!(
            facts.completeness,
            EditorSemanticCompleteness::Recovered,
            "{family}"
        );
        assert!(facts.lexemes().iter().all(|lexeme| {
            lexeme.producer().kind() == EditorLexemeProducerKind::FamilyRecovery
                && lexeme.producer().family().map(|id| id.as_str()) == Some(family)
        }));
        assert_source_lexeme(&facts, source, before.0, before.1);
        assert_source_lexeme(&facts, source, after.0, after.1);
        assert_non_overlapping(&facts, source);
    }
}

fn assert_complete_provenance(facts: &EditorSemanticFacts, family: &str) {
    assert!(facts.lexemes().iter().any(|lexeme| {
        lexeme.producer().kind() == EditorLexemeProducerKind::FamilyParser
            && lexeme.producer().family().map(|id| id.as_str()) == Some(family)
    }));
    assert!(facts.lexemes().iter().all(|lexeme| {
        let producer = lexeme.producer();
        match producer.kind() {
            EditorLexemeProducerKind::GlobalPreprocess => {
                lexeme.kind() == EditorLexemeKind::Comment && producer.family().is_none()
            }
            EditorLexemeProducerKind::FamilyParser => {
                producer.family().map(|id| id.as_str()) == Some(family)
            }
            EditorLexemeProducerKind::FamilyLexer | EditorLexemeProducerKind::FamilyRecovery => {
                false
            }
        }
    }));
}

fn assert_source_lexeme(
    facts: &EditorSemanticFacts,
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

fn assert_non_overlapping(facts: &EditorSemanticFacts, source: &str) {
    for lexeme in facts.lexemes() {
        let span = lexeme.span();
        assert!(span.start < span.end && span.end <= source.len());
        assert!(source.is_char_boundary(span.start));
        assert!(source.is_char_boundary(span.end));
    }
    for pair in facts.lexemes().windows(2) {
        assert!(
            pair[0].span().end <= pair[1].span().start,
            "overlapping lexemes: {pair:?}"
        );
    }
}
