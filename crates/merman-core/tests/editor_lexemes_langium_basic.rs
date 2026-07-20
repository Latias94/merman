use merman_core::{
    EditorLexemeKind, EditorLexemeProducerKind, EditorSemanticCompleteness, EditorSemanticFacts,
    Engine,
};

struct CompleteCase {
    family: &'static str,
    source: &'static str,
    expected: &'static [(EditorLexemeKind, &'static str)],
}

#[test]
fn langium_basic_families_emit_parser_owned_exact_lexemes() {
    let cases = [
        CompleteCase {
            family: "info",
            source: concat!(
                "info showInfo\n",
                "%% info comment\n",
                "title Runtime facts\n",
                "accTitle: Runtime map\n",
                "accDescr {\n",
                "  Runtime details\n",
                "}\n",
            ),
            expected: &[
                (EditorLexemeKind::Keyword, "info"),
                (EditorLexemeKind::Keyword, "showInfo"),
                (EditorLexemeKind::Comment, "%% info comment\n"),
                (EditorLexemeKind::Keyword, "title"),
                (EditorLexemeKind::String, "Runtime facts"),
                (EditorLexemeKind::Keyword, "accTitle"),
                (EditorLexemeKind::Delimiter, ":"),
                (EditorLexemeKind::String, "Runtime map"),
                (EditorLexemeKind::Keyword, "accDescr"),
                (EditorLexemeKind::Delimiter, "{"),
                (EditorLexemeKind::String, "Runtime details"),
                (EditorLexemeKind::Delimiter, "}"),
            ],
        },
        CompleteCase {
            family: "pie",
            source: concat!(
                "pie showData\n",
                "%% pie comment\n",
                "title Distribution\n",
                "\"A slice\": 12.5\n",
                "'B': 7\n",
            ),
            expected: &[
                (EditorLexemeKind::Keyword, "pie"),
                (EditorLexemeKind::Keyword, "showData"),
                (EditorLexemeKind::Comment, "%% pie comment\n"),
                (EditorLexemeKind::Keyword, "title"),
                (EditorLexemeKind::String, "Distribution"),
                (EditorLexemeKind::String, "\"A slice\""),
                (EditorLexemeKind::Delimiter, ":"),
                (EditorLexemeKind::Number, "12.5"),
                (EditorLexemeKind::String, "'B'"),
                (EditorLexemeKind::Number, "7"),
            ],
        },
        CompleteCase {
            family: "packet",
            source: concat!(
                "packet-beta\n",
                "%% packet comment\n",
                "title Header packet\n",
                "0-7: \"Header\"\n",
                "+ 8: 'Payload'\n",
            ),
            expected: &[
                (EditorLexemeKind::Keyword, "packet-beta"),
                (EditorLexemeKind::Comment, "%% packet comment\n"),
                (EditorLexemeKind::Keyword, "title"),
                (EditorLexemeKind::String, "Header packet"),
                (EditorLexemeKind::Number, "0"),
                (EditorLexemeKind::Operator, "-"),
                (EditorLexemeKind::Number, "7"),
                (EditorLexemeKind::Delimiter, ":"),
                (EditorLexemeKind::String, "\"Header\""),
                (EditorLexemeKind::Operator, "+"),
                (EditorLexemeKind::Number, "8"),
                (EditorLexemeKind::String, "'Payload'"),
            ],
        },
        CompleteCase {
            family: "radar",
            source: concat!(
                "radar-beta:\n",
                "%% radar comment\n",
                "title Frameworks\n",
                "axis perf[\"Performance\"], dx[\"Dev Experience\"], eco[\"Ecosystem\"]\n",
                "curve react[\"React\"]{4, 4, 5}\n",
                "showLegend true\n",
                "graticule polygon\n",
                "max 5\n",
            ),
            expected: &[
                (EditorLexemeKind::Keyword, "radar-beta"),
                (EditorLexemeKind::Delimiter, ":"),
                (EditorLexemeKind::Comment, "%% radar comment\n"),
                (EditorLexemeKind::Keyword, "title"),
                (EditorLexemeKind::String, "Frameworks"),
                (EditorLexemeKind::Keyword, "axis"),
                (EditorLexemeKind::Identifier, "perf"),
                (EditorLexemeKind::Delimiter, "["),
                (EditorLexemeKind::String, "\"Performance\""),
                (EditorLexemeKind::Delimiter, "]"),
                (EditorLexemeKind::Delimiter, ","),
                (EditorLexemeKind::Identifier, "dx"),
                (EditorLexemeKind::Keyword, "curve"),
                (EditorLexemeKind::Identifier, "react"),
                (EditorLexemeKind::Delimiter, "{"),
                (EditorLexemeKind::Number, "4"),
                (EditorLexemeKind::Delimiter, "}"),
                (EditorLexemeKind::Keyword, "showLegend"),
                (EditorLexemeKind::Boolean, "true"),
                (EditorLexemeKind::Keyword, "graticule"),
                (EditorLexemeKind::Literal, "polygon"),
                (EditorLexemeKind::Keyword, "max"),
                (EditorLexemeKind::Number, "5"),
            ],
        },
    ];

    let engine = Engine::new();
    for case in cases {
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync(case.family, case.source)
            .unwrap_or_else(|error| panic!("{} editor parse failed: {error}", case.family))
            .unwrap_or_else(|| panic!("{} editor facts are unavailable", case.family));

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
fn langium_basic_recovery_retains_tokens_before_and_after_a_malformed_statement() {
    let cases = [
        (
            "info",
            "info showInfo\ntitle Before\n?\naccTitle: After\n",
            (EditorLexemeKind::String, "Before"),
            (EditorLexemeKind::String, "After"),
        ),
        (
            "pie",
            "pie showData\n\"Before\": 1\n?\n\"After\": 2\n",
            (EditorLexemeKind::String, "\"Before\""),
            (EditorLexemeKind::String, "\"After\""),
        ),
        (
            "packet",
            "packet\n0-7: \"Before\"\n?\n+8: \"After\"\n",
            (EditorLexemeKind::String, "\"Before\""),
            (EditorLexemeKind::String, "\"After\""),
        ),
        (
            "radar",
            "radar-beta\naxis Before\n?\naxis After\ncurve values {1, 2}\n",
            (EditorLexemeKind::Identifier, "Before"),
            (EditorLexemeKind::Identifier, "After"),
        ),
    ];

    let engine = Engine::new();
    for (family, source, before, after) in cases {
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync(family, source)
            .unwrap_or_else(|error| panic!("{family} recovery failed: {error}"))
            .unwrap_or_else(|| panic!("{family} editor facts are unavailable"));

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
