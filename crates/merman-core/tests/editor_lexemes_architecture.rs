use merman_core::{
    EditorLexemeKind, EditorLexemeProducerKind, EditorSemanticCompleteness, Engine, ParseOptions,
};

#[test]
fn architecture_trace_emits_exact_grammar_lexemes_across_crlf_and_unicode() {
    let source = concat!(
        "architecture-beta\r\n",
        "title System map\r\n",
        "group api(cloud)[\"API 服务\"]\r\n",
        "service db(database)[Database] in api\r\n",
        "junction hub in api\r\n",
        "db:L -- R:hub\r\n",
        "align row db hub\r\n",
    );
    let facts = Engine::new()
        .parse_editor_semantic_facts_with_type_sync("architecture", source, ParseOptions::strict())
        .expect("architecture editor parse")
        .expect("architecture editor facts");

    assert_eq!(facts.lexeme_failure(), None);
    assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
    assert!(facts.lexemes().iter().all(|lexeme| {
        lexeme.producer().kind() == EditorLexemeProducerKind::FamilyParser
            && lexeme.producer().family().map(|family| family.as_str()) == Some("architecture")
    }));
    for (kind, text) in [
        (EditorLexemeKind::Keyword, "architecture-beta"),
        (EditorLexemeKind::Keyword, "title"),
        (EditorLexemeKind::Keyword, "group"),
        (EditorLexemeKind::Identifier, "api"),
        (EditorLexemeKind::Delimiter, "("),
        (EditorLexemeKind::Literal, "cloud"),
        (EditorLexemeKind::Delimiter, ")"),
        (EditorLexemeKind::Delimiter, "["),
        (EditorLexemeKind::String, "\"API 服务\""),
        (EditorLexemeKind::Keyword, "service"),
        (EditorLexemeKind::Keyword, "in"),
        (EditorLexemeKind::Delimiter, ":"),
        (EditorLexemeKind::Literal, "L"),
        (EditorLexemeKind::Operator, "--"),
        (EditorLexemeKind::Keyword, "align"),
        (EditorLexemeKind::Keyword, "row"),
    ] {
        assert_source_lexeme(&facts, source, kind, text);
    }
    assert_non_overlapping(source, &facts);
}

#[test]
fn architecture_recovery_preserves_confirmed_prefix_and_later_statement_lexemes() {
    let source = concat!(
        "architecture-beta\n",
        "service before(server)[Before]\n",
        "service broken(\n",
        "service after(server)[After]\n",
    );
    let facts = Engine::new()
        .parse_editor_semantic_facts_with_type_sync("architecture", source, ParseOptions::strict())
        .expect("architecture editor recovery")
        .expect("architecture recovery facts");

    assert_eq!(facts.lexeme_failure(), None);
    assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
    assert!(facts.lexemes().iter().all(|lexeme| {
        lexeme.producer().kind() == EditorLexemeProducerKind::FamilyRecovery
            && lexeme.producer().family().map(|family| family.as_str()) == Some("architecture")
    }));
    for (kind, text) in [
        (EditorLexemeKind::Identifier, "before"),
        (EditorLexemeKind::Keyword, "service"),
        (EditorLexemeKind::Identifier, "broken"),
        (EditorLexemeKind::Delimiter, "("),
        (EditorLexemeKind::Identifier, "after"),
        (EditorLexemeKind::String, "After"),
    ] {
        assert_source_lexeme(&facts, source, kind, text);
    }
    assert_non_overlapping(source, &facts);
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

fn assert_non_overlapping(source: &str, facts: &merman_core::EditorSemanticFacts) {
    for lexeme in facts.lexemes() {
        let span = lexeme.span();
        assert!(span.start < span.end && span.end <= source.len());
        assert!(source.is_char_boundary(span.start));
        assert!(source.is_char_boundary(span.end));
    }
    for pair in facts.lexemes().windows(2) {
        assert!(pair[0].span().end <= pair[1].span().start, "{pair:?}");
    }
}
