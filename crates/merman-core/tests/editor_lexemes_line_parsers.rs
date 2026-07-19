use merman_core::{
    EditorLexemeKind, EditorLexemeProducerKind, EditorSemanticCompleteness, Engine, ParseOptions,
};

#[test]
fn journey_emits_lexemes_from_its_line_parser() {
    let source = concat!(
        "journey\n",
        "# journey-local comment\n",
        "title Checkout flow\n",
        "accTitle: Checkout accessibility\n",
        "accDescr {\n",
        "  A complete journey\n",
        "}\n",
        "section Checkout\n",
        "Choose item: 5: Alice, Bob\n",
    );
    let facts = Engine::new()
        .parse_editor_semantic_facts_with_type_sync("journey", source, ParseOptions::strict())
        .expect("journey editor parse")
        .expect("journey editor facts");

    assert_eq!(facts.lexeme_failure(), None);
    assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
    assert!(facts.lexemes().iter().all(|lexeme| {
        lexeme.producer().kind() == EditorLexemeProducerKind::FamilyParser
            && lexeme.producer().family().map(|family| family.as_str()) == Some("journey")
    }));
    for (kind, text) in [
        (EditorLexemeKind::Keyword, "journey"),
        (EditorLexemeKind::Comment, "# journey-local comment"),
        (EditorLexemeKind::Keyword, "title"),
        (EditorLexemeKind::String, "Checkout flow"),
        (EditorLexemeKind::Delimiter, ":"),
        (EditorLexemeKind::Keyword, "accDescr"),
        (EditorLexemeKind::Delimiter, "{"),
        (EditorLexemeKind::String, "Checkout"),
        (EditorLexemeKind::String, "Choose item"),
        (EditorLexemeKind::Number, "5"),
        (EditorLexemeKind::Identifier, "Alice"),
        (EditorLexemeKind::Delimiter, ","),
        (EditorLexemeKind::Identifier, "Bob"),
    ] {
        assert_source_lexeme(&facts, source, kind, text);
    }
    assert_valid_non_overlapping(source, &facts);
}

#[test]
fn journey_recovery_keeps_lexemes_after_a_malformed_middle_task() {
    let source = concat!(
        "journey\n",
        "Before: 5: Alice\n",
        ": malformed\n",
        "After: 4: Bob\n",
    );
    let facts = Engine::new()
        .parse_editor_semantic_facts_with_type_sync("journey", source, ParseOptions::strict())
        .expect("journey editor recovery")
        .expect("journey recovery facts");

    assert_eq!(facts.lexeme_failure(), None);
    assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
    assert!(facts.lexemes().iter().all(|lexeme| {
        lexeme.producer().kind() == EditorLexemeProducerKind::FamilyRecovery
            && lexeme.producer().family().map(|family| family.as_str()) == Some("journey")
    }));
    assert_source_lexeme(&facts, source, EditorLexemeKind::String, "Before");
    assert_source_lexeme(&facts, source, EditorLexemeKind::String, "After");
    assert_source_lexeme(&facts, source, EditorLexemeKind::Identifier, "Bob");
    assert_valid_non_overlapping(source, &facts);
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

fn assert_valid_non_overlapping(source: &str, facts: &merman_core::EditorSemanticFacts) {
    for lexeme in facts.lexemes() {
        let span = lexeme.span();
        assert!(span.start < span.end);
        assert!(span.end <= source.len());
        assert!(source.is_char_boundary(span.start));
        assert!(source.is_char_boundary(span.end));
    }
    assert!(
        facts
            .lexemes()
            .windows(2)
            .all(|pair| pair[0].span().end <= pair[1].span().start),
        "overlapping lexemes: {:?}",
        facts.lexemes()
    );
}
