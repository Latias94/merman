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
