use merman_core::{EditorLexemeKind, EditorLexemeProducerKind, EditorSemanticCompleteness, Engine};

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
        .parse_editor_semantic_facts_with_type_sync("journey", source)
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
        .parse_editor_semantic_facts_with_type_sync("journey", source)
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

#[test]
fn gantt_emits_dates_durations_dependencies_and_click_lexemes_from_its_parser() {
    let source = concat!(
        "gantt\n",
        "title A Gantt Diagram\n",
        "dateFormat YYYY-MM-DD\n",
        "tickInterval 1d\n",
        "todayMarker stroke-width:2px\n",
        "section Build\n",
        "Task one: active, task1, 2024-01-01, 2d\n",
        "Task two: done, task2, after task1, 1d\n",
        "click task1 HREF \"https://example.com/task\"\n",
    );
    let facts = Engine::new()
        .parse_editor_semantic_facts_with_type_sync("gantt", source)
        .expect("gantt editor parse")
        .expect("gantt editor facts");

    assert_eq!(facts.lexeme_failure(), None);
    assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
    assert!(facts.lexemes().iter().all(|lexeme| {
        lexeme.producer().kind() == EditorLexemeProducerKind::FamilyParser
            && lexeme.producer().family().map(|family| family.as_str()) == Some("gantt")
    }));
    for (kind, text) in [
        (EditorLexemeKind::Keyword, "gantt"),
        (EditorLexemeKind::Keyword, "dateFormat"),
        (EditorLexemeKind::Literal, "YYYY-MM-DD"),
        (EditorLexemeKind::Duration, "1d"),
        (EditorLexemeKind::Style, "stroke-width:2px"),
        (EditorLexemeKind::String, "Build"),
        (EditorLexemeKind::String, "Task one"),
        (EditorLexemeKind::Keyword, "active"),
        (EditorLexemeKind::Identifier, "task1"),
        (EditorLexemeKind::Date, "2024-01-01"),
        (EditorLexemeKind::Keyword, "after"),
        (EditorLexemeKind::Keyword, "HREF"),
        (EditorLexemeKind::String, "https://example.com/task"),
        (EditorLexemeKind::Delimiter, "\""),
    ] {
        assert_source_lexeme(&facts, source, kind, text);
    }
    assert_valid_non_overlapping(source, &facts);
}

#[test]
fn gantt_recovery_keeps_lexemes_after_a_malformed_middle_statement() {
    let source = concat!(
        "gantt\n",
        "Before: before, 2024-01-01, 1d\n",
        "malformed statement\n",
        "After: after, 2024-01-02, 2d\n",
    );
    let facts = Engine::new()
        .parse_editor_semantic_facts_with_type_sync("gantt", source)
        .expect("gantt editor recovery")
        .expect("gantt recovery facts");

    assert_eq!(facts.lexeme_failure(), None);
    assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
    assert!(facts.lexemes().iter().all(|lexeme| {
        lexeme.producer().kind() == EditorLexemeProducerKind::FamilyRecovery
            && lexeme.producer().family().map(|family| family.as_str()) == Some("gantt")
    }));
    assert_source_lexeme(&facts, source, EditorLexemeKind::String, "Before");
    assert_source_lexeme(
        &facts,
        source,
        EditorLexemeKind::Literal,
        "malformed statement",
    );
    assert_source_lexeme(&facts, source, EditorLexemeKind::String, "After");
    assert_valid_non_overlapping(source, &facts);
}

#[test]
fn gantt_unterminated_accessibility_block_upgrades_late_attached_lexemes_to_recovery() {
    let source = "gantt\naccDescr {\n  partial description\n";
    let facts = Engine::new()
        .parse_editor_semantic_facts_with_type_sync("gantt", source)
        .expect("gantt editor recovery")
        .expect("gantt recovery facts");

    assert_eq!(facts.lexeme_failure(), None);
    assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
    assert!(facts.lexemes().iter().all(|lexeme| {
        lexeme.producer().kind() == EditorLexemeProducerKind::FamilyRecovery
            && lexeme.producer().family().map(|family| family.as_str()) == Some("gantt")
    }));
    assert_source_lexeme(&facts, source, EditorLexemeKind::Keyword, "accDescr");
    assert_source_lexeme(
        &facts,
        source,
        EditorLexemeKind::String,
        "partial description",
    );
    assert_valid_non_overlapping(source, &facts);
}

#[test]
fn sankey_emits_csv_fields_quotes_delimiters_and_values_from_its_parser() {
    let source = concat!(
        "  sankey-beta\r\n",
        "\"Source, 源\",Target,12.5\r\n",
        "Target,End,NaN\r\n",
    );
    let facts = Engine::new()
        .parse_editor_semantic_facts_with_type_sync("sankey", source)
        .expect("sankey editor parse")
        .expect("sankey editor facts");

    assert_eq!(facts.lexeme_failure(), None);
    assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
    assert!(facts.lexemes().iter().all(|lexeme| {
        lexeme.producer().kind() == EditorLexemeProducerKind::FamilyParser
            && lexeme.producer().family().map(|family| family.as_str()) == Some("sankey")
    }));
    for (kind, text) in [
        (EditorLexemeKind::Keyword, "sankey-beta"),
        (EditorLexemeKind::Delimiter, "\""),
        (EditorLexemeKind::String, "Source, 源"),
        (EditorLexemeKind::Delimiter, ","),
        (EditorLexemeKind::Identifier, "Target"),
        (EditorLexemeKind::Number, "12.5"),
        (EditorLexemeKind::Identifier, "End"),
        (EditorLexemeKind::Literal, "NaN"),
    ] {
        assert_source_lexeme(&facts, source, kind, text);
    }
    assert_valid_non_overlapping(source, &facts);
}

#[test]
fn sankey_recovery_keeps_csv_lexemes_after_a_malformed_middle_record() {
    let source = concat!("sankey\n", "Before,Middle,1\n", "broken\n", "After,End,2\n",);
    let facts = Engine::new()
        .parse_editor_semantic_facts_with_type_sync("sankey", source)
        .expect("sankey editor recovery")
        .expect("sankey recovery facts");

    assert_eq!(facts.lexeme_failure(), None);
    assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
    assert!(facts.lexemes().iter().all(|lexeme| {
        lexeme.producer().kind() == EditorLexemeProducerKind::FamilyRecovery
            && lexeme.producer().family().map(|family| family.as_str()) == Some("sankey")
    }));
    for text in ["Before", "broken", "After", "End"] {
        assert_source_lexeme(&facts, source, EditorLexemeKind::Identifier, text);
    }
    assert_valid_non_overlapping(source, &facts);
}

#[test]
fn quadrant_emits_structural_point_class_and_style_lexemes_from_its_parser() {
    let source = concat!(
        "quadrantChart\n",
        "%% family comment\n",
        "title \"Portfolio\"\n",
        "x-axis \"Low Reach\" --> High Reach\n",
        "quadrant-1 Expand\n",
        "classDef priority color: #109060, radius: 10\n",
        "Campaign A:::priority: [0.2, 0.8] radius: 8, color: #ff0000\n",
    );
    let facts = Engine::new()
        .parse_editor_semantic_facts_with_type_sync("quadrantChart", source)
        .expect("quadrant editor parse")
        .expect("quadrant editor facts");

    assert_eq!(facts.lexeme_failure(), None);
    assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
    assert!(facts.lexemes().iter().all(|lexeme| {
        let producer = lexeme.producer();
        producer.kind() == EditorLexemeProducerKind::GlobalPreprocess
            || (producer.kind() == EditorLexemeProducerKind::FamilyParser
                && producer.family().map(|family| family.as_str()) == Some("quadrantChart"))
    }));
    for (kind, text) in [
        (EditorLexemeKind::Keyword, "quadrantChart"),
        (EditorLexemeKind::Comment, "%% family comment\n"),
        (EditorLexemeKind::Keyword, "x-axis"),
        (EditorLexemeKind::Operator, "-->"),
        (EditorLexemeKind::Keyword, "classDef"),
        (EditorLexemeKind::Identifier, "priority"),
        (EditorLexemeKind::Operator, ":::"),
        (EditorLexemeKind::Delimiter, "["),
        (EditorLexemeKind::Number, "0.2"),
        (EditorLexemeKind::Style, "radius"),
        (EditorLexemeKind::Color, "#ff0000"),
    ] {
        assert_source_lexeme(&facts, source, kind, text);
    }
    assert_valid_non_overlapping(source, &facts);
}

#[test]
fn quadrant_recovery_keeps_point_lexemes_after_a_malformed_middle_statement() {
    let source = concat!(
        "quadrantChart\n",
        "Before: [0.2, 0.8]\n",
        "Broken: [1.2, 0.4]\n",
        "After: [0.3, 0.7]\n",
    );
    let facts = Engine::new()
        .parse_editor_semantic_facts_with_type_sync("quadrantChart", source)
        .expect("quadrant editor recovery")
        .expect("quadrant recovery facts");

    assert_eq!(facts.lexeme_failure(), None);
    assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
    assert!(facts.lexemes().iter().all(|lexeme| {
        lexeme.producer().kind() == EditorLexemeProducerKind::FamilyRecovery
            && lexeme.producer().family().map(|family| family.as_str()) == Some("quadrantChart")
    }));
    assert_source_lexeme(&facts, source, EditorLexemeKind::String, "Before");
    assert_source_lexeme(
        &facts,
        source,
        EditorLexemeKind::Literal,
        "Broken: [1.2, 0.4]",
    );
    assert_source_lexeme(&facts, source, EditorLexemeKind::String, "After");
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
