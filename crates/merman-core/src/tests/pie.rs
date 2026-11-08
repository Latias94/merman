use crate::*;
use futures::executor::block_on;
use serde_json::json;

#[test]
fn parse_diagram_pie_basic() {
    let engine = MermaidEngine::new();
    let text = r#"pie showData
 "Cats": 2
 'Dogs': 3
 "#;
    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.meta.diagram_type, "pie");
    assert_eq!(
        res.model,
        json!({
            "type": "pie",
            "showData": true,
            "title": null,
            "accTitle": null,
            "accDescr": null,
            "sections": [
                { "label": "Cats", "value": 2.0 },
                { "label": "Dogs", "value": 3.0 }
            ]
        })
    );
}

#[test]
fn parse_pie_multiline_strings_keep_semantics_and_editor_source_ranges() {
    let engine = MermaidEngine::new();
    for newline in ["\n", "\r\n"] {
        let text = concat!(
            "pie showData\n",
            "  \"First\nSecond\": 10 %% outside the string\n",
            "  'Other\n\\'quoted\\' %% literal': 5\n",
            "  \"First\nSecond\": 99\n",
        )
        .replace('\n', newline);
        let first_label = "First\nSecond";
        let other_label = "Other\n'quoted' %% literal";
        let canonical = engine
            .parse_diagram_sync(&text, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(
            canonical.model["sections"],
            json!([
                {"label": first_label, "value": 10.0},
                {"label": other_label, "value": 5.0},
            ])
        );
        crate::diagrams::langium_common::reset_family_syntax_construction_count("pie");
        let snapshot = engine.parse_diagram_snapshot_sync(&text).unwrap().unwrap();
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("pie"),
            1
        );
        assert_eq!(snapshot.outcome().parsed_model().unwrap(), &canonical.model);
        let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
            panic!("Pie snapshot must retain the same parser's editor facts");
        };
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
        assert!(facts.diagnostics.is_empty());
        for (label, raw_label) in [
            (first_label, format!("First{newline}Second")),
            (
                other_label,
                format!("Other{newline}\\'quoted\\' %% literal"),
            ),
        ] {
            let start = text.find(&raw_label).unwrap();
            let selection = SourceSpan::new(start, start + raw_label.len());
            let symbol = facts.symbols.iter().find(|s| s.name == label).unwrap();
            assert_eq!(symbol.selection, selection);
            assert_eq!(symbol.role, EditorSemanticRole::Outline);
            assert!(symbol.span.start < selection.start);
            assert!(symbol.span.end > selection.end);
            assert!(facts.expected_syntax.iter().any(|expected| {
                expected.kind == EditorExpectedSyntaxKind::Payload && expected.span == selection
            }));
        }
        for number in ["10", "5", "99"] {
            let start = text.find(number).unwrap();
            assert!(facts.expected_syntax.iter().any(|expected| {
                expected.kind == EditorExpectedSyntaxKind::Payload
                    && expected.span == SourceSpan::new(start, start + number.len())
            }));
        }
    }
}

#[test]
fn parse_pie_multiline_negative_value_reports_its_original_span_and_recovers() {
    let text = "pie\n\"First\nSecond\": -10\n\"Other\": 5\n";
    let engine = MermaidEngine::new();
    let error = engine
        .parse_diagram_sync(text, ParseOptions::strict())
        .unwrap_err();
    let Error::DiagramParse { diagnostic, .. } = error else {
        panic!("expected a Pie value diagnostic");
    };
    let value_start = text.find("-10").unwrap();
    let value_span = SourceSpan::new(value_start, value_start + 3);
    assert_eq!(diagnostic.span(), Some(value_span));
    assert!(
        diagnostic
            .message()
            .contains("Negative values are not allowed")
    );
    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("pie", text)
        .unwrap()
        .unwrap();
    assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
    assert!(facts.diagnostics.iter().any(|d| d.span == Some(value_span)));
    assert!(facts.symbols.iter().any(|s| s.name == "Other"));
}

#[test]
fn parse_pie_multiline_support_preserves_upstream_rejection_boundaries() {
    let engine = MermaidEngine::new();
    for text in [
        "pie\"A\": 1\n",
        "pie\n\"First\nSecond\"\n: 10\n",
        "pie\n\"First\\\nSecond\": 10\n",
        "pie\n\"First\nSecond\": 10 \"Other\": 5\n",
        "pie\n\"First\nSecond: 10\n",
    ] {
        assert!(
            engine
                .parse_diagram_sync(text, ParseOptions::strict())
                .is_err(),
            "unexpectedly accepted {text:?}"
        );
    }
}

#[test]
fn parse_diagram_pie_rejects_negative_slice_values_like_upstream() {
    let engine = MermaidEngine::new();
    let text = r#"pie title Default text position: Animal adoption
         "dogs" : -60.67
        "rats" : 40.12
        "#;
    let err = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap_err()
        .to_string();
    assert_eq!(
        err,
        "Diagram parse error (pie): \"dogs\" has invalid value: -60.67. Negative values are not allowed in pie charts. All slice values must be >= 0."
    );
}

#[test]
fn parse_pie_exposes_11_16_config_defaults_and_overrides() {
    let engine = MermaidEngine::new();
    let default = block_on(engine.parse_metadata("pie\n\"A\": 1")).unwrap();
    let pie = &default.effective_config.as_value()["pie"];
    assert_eq!(pie["textPosition"], json!(0.75));
    assert_eq!(pie["donutHole"], json!(0));
    assert_eq!(pie["legendPosition"], json!("right"));
    assert_eq!(pie["highlightSlice"], json!(""));

    let configured = block_on(engine.parse_metadata(
        r#"%%{init: {"pie": {"textPosition": 0.5, "donutHole": 0.4, "legendPosition": "bottom", "highlightSlice": "A"}}}%%
pie
"A": 1"#,
    ))
    .unwrap();
    let pie = &configured.effective_config.as_value()["pie"];
    assert_eq!(pie["textPosition"], json!(0.5));
    assert_eq!(pie["donutHole"], json!(0.4));
    assert_eq!(pie["legendPosition"], json!("bottom"));
    assert_eq!(pie["highlightSlice"], json!("A"));
}

#[test]
fn parse_pie_editor_facts_expose_parser_backed_spans() {
    let engine = MermaidEngine::new();
    let text = r#"pie showData
  title Market Share
  accTitle: Accessibility title
  accDescr: Accessibility description
  "Cats": 2
  "Dogs": 3
"#;
    let canonical = engine
        .parse_diagram_sync(text, ParseOptions::strict())
        .unwrap()
        .unwrap();
    crate::diagrams::langium_common::reset_family_syntax_construction_count("pie");
    crate::editor::test_work::reset();
    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("pie", text)
        .unwrap()
        .unwrap();
    assert_eq!(
        crate::diagrams::langium_common::family_syntax_construction_count("pie"),
        1
    );
    let work = crate::editor::test_work::read();
    assert_eq!(work.collections, 1);
    assert_eq!(work.symbols, facts.symbols.len());
    assert!(work.directives >= facts.directive_prefixes.len());
    assert!(work.directives > 0);
    assert_eq!(work.diagnostics, facts.diagnostics.len());
    assert!(work.expected_syntax >= facts.expected_syntax.len());

    assert!(facts.directive_prefixes.iter().any(|p| p == "title"));
    assert!(facts.directive_prefixes.iter().any(|p| p == "accTitle"));
    assert!(facts.directive_prefixes.iter().any(|p| p == "accDescr"));
    assert!(
        facts
            .symbols
            .iter()
            .any(|symbol| symbol.name == "Cats" && symbol.kind == EditorSemanticKind::String)
    );
    assert!(
        facts
            .symbols
            .iter()
            .any(|symbol| symbol.name == "Dogs" && symbol.kind == EditorSemanticKind::String)
    );
    assert!(facts.expected_syntax.iter().any(|expected| {
        expected.kind == EditorExpectedSyntaxKind::Payload
            && expected.span
                == SourceSpan::new(text.find("2").unwrap(), text.find("2").unwrap() + 1)
    }));
    for (index, label) in ["Cats", "Dogs"].into_iter().enumerate() {
        let start = text.find(label).unwrap();
        let symbol = facts
            .symbols
            .iter()
            .find(|symbol| symbol.name == label)
            .unwrap();
        assert_eq!(symbol.role, EditorSemanticRole::Outline);
        assert_eq!(
            symbol.selection,
            SourceSpan::new(start, start + label.len())
        );
        assert_eq!(canonical.model["sections"][index]["label"], label);
    }
    for (field, value) in [
        ("title", "Market Share"),
        ("accTitle", "Accessibility title"),
        ("accDescr", "Accessibility description"),
    ] {
        let start = text.find(value).unwrap();
        let symbol = facts
            .symbols
            .iter()
            .find(|symbol| symbol.name == value)
            .unwrap();
        assert_eq!(symbol.role, EditorSemanticRole::Payload);
        assert_eq!(
            symbol.selection,
            SourceSpan::new(start, start + value.len())
        );
        assert_eq!(canonical.model[field], value);
    }

    for known_type in [false, true] {
        crate::diagrams::langium_common::reset_family_syntax_construction_count("pie");
        crate::editor::test_work::reset();
        let snapshot = if known_type {
            engine.parse_diagram_snapshot_with_type_sync("pie", text)
        } else {
            engine.parse_diagram_snapshot_sync(text)
        }
        .unwrap()
        .unwrap();
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count("pie"),
            1
        );
        assert_eq!(crate::editor::test_work::read().collections, 1);
        assert_eq!(snapshot.outcome().parsed_model().unwrap(), &canonical.model);
        let ParsedEditorFacts::Available(snapshot_facts) = snapshot.editor_facts() else {
            panic!("pie snapshot must retain editor facts");
        };
        assert_eq!(snapshot_facts, &facts);
    }
}
