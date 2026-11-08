#![cfg(feature = "diagram-er")]

use merman_core::SemanticParser;
use merman_model::{families::er::ErDiagramModel, semantic::SemanticDiagramModel};

fn parse_er(source: &str) -> ErDiagramModel {
    let parsed = SemanticParser::new()
        .parse_diagram_sync(source)
        .expect("valid ER source");
    assert_eq!(parsed.metadata.diagram_type, "er");
    let SemanticDiagramModel::Er(model) = parsed.model else {
        panic!("ER source must retain its canonical model");
    };
    model
}

#[test]
fn completed_subgraph_endpoint_reserves_its_entity_id_before_the_other_endpoint() {
    for declaration in ["", "G\n"] {
        let source = format!("erDiagram\nsubgraph G\nA\nend\n{declaration}G ||--|| B : rel\n");
        let model = parse_er(&source);
        assert!(
            model
                .entities
                .keys()
                .map(String::as_str)
                .eq(["A", "G", "B"])
        );
        assert_eq!(model.entities["A"].id, "entity-A-0");
        assert_eq!(model.entities["G"].id, "entity-G-1");
        assert_eq!(model.entities["B"].id, "entity-B-2");
        assert_eq!(model.subgraphs.len(), 1);
        assert_eq!(model.subgraphs[0].id, "G");
        assert_eq!(model.subgraphs[0].nodes, ["A"]);
        assert_eq!(model.relationships.len(), 1);
        assert_eq!(model.relationships[0].entity_a, "G");
        assert_eq!(model.relationships[0].entity_b, "entity-B-2");
    }
}

#[test]
fn two_completed_subgraph_endpoints_keep_shadow_ids_separate_from_group_routing() {
    let model = parse_er(concat!(
        "erDiagram\n",
        "subgraph G\nA\nend\n",
        "subgraph H\nB\nend\n",
        "G:::left ||--|| H:::right : rel\n",
        "C\n",
    ));
    assert!(
        model
            .entities
            .keys()
            .map(String::as_str)
            .eq(["A", "B", "G", "H", "C"])
    );
    assert_eq!(model.entities["G"].id, "entity-G-2");
    assert_eq!(model.entities["H"].id, "entity-H-3");
    assert_eq!(model.entities["C"].id, "entity-C-4");
    assert_eq!(model.entities["G"].css_classes, "default left");
    assert_eq!(model.entities["H"].css_classes, "default right");
    assert_eq!(model.subgraphs[0].classes, ["left"]);
    assert_eq!(model.subgraphs[1].classes, ["right"]);
    assert_eq!(model.relationships[0].entity_a, "G");
    assert_eq!(model.relationships[0].entity_b, "H");
    assert_ne!(model.relationships[0].entity_a, model.entities["G"].id);
    assert_ne!(model.relationships[0].entity_b, model.entities["H"].id);
}

#[test]
fn endpoint_entity_keeps_inline_class_and_style_when_an_alias_is_declared_later() {
    for declaration in ["", "G\n"] {
        let source = format!(
            concat!(
                "erDiagram\nsubgraph G\nA\nend\n{}",
                "G:::accent ||--|| B : rel\n",
                "style G fill:red\n",
                "G[Later]\n",
            ),
            declaration,
        );
        let model = parse_er(&source);
        let entity = &model.entities["G"];
        assert_eq!(entity.id, "entity-G-1");
        assert_eq!(entity.alias, "Later");
        assert_eq!(entity.css_classes, "default accent");
        assert_eq!(entity.css_styles, ["fill:red"]);
        assert_eq!(model.entities["B"].id, "entity-B-2");
        assert_eq!(model.subgraphs[0].classes, ["accent"]);
        assert_eq!(model.subgraphs[0].css_styles, ["fill:red"]);
    }
}

#[test]
fn style_targets_and_classdef_names_accept_keywords_only_in_the_exclusive_context() {
    for name in [
        "style",
        "class",
        "classDef",
        "subgraph",
        "end",
        "erDiagram",
        "StYlE",
        "end-user",
        "Item",
    ] {
        let source = format!(
            concat!(
                "erDiagram\n\"{0}\"\n",
                "style {0} fill:red\n",
                "classDef {0} stroke:blue,color:green\n",
                "After\n",
            ),
            name,
        );
        let model = parse_er(&source);
        assert_eq!(model.entities.len(), 2, "{name}");
        assert_eq!(model.entities[name].css_styles, ["fill:red"], "{name}");
        assert_eq!(model.entities["After"].id, "entity-After-1", "{name}");
        assert_eq!(model.classes.len(), 1, "{name}");
        assert_eq!(model.classes[name].id, name);
        assert_eq!(model.classes[name].styles, ["stroke:blue", "color:green"]);
        assert_eq!(model.classes[name].text_styles, ["color:green"]);
    }
}

#[test]
fn initial_class_and_inline_lists_keep_keywords_reserved_and_require_style_payloads() {
    let parser = SemanticParser::new();
    for name in ["style", "class", "classDef", "subgraph", "end", "erDiagram"] {
        for tail in [
            format!("class {name} accent\n"),
            format!("class A {name}\n"),
            format!("A:::{name}\n"),
        ] {
            let source = format!("erDiagram\nA\n\"{name}\"\n{tail}");
            assert!(parser.parse_diagram_sync(&source).is_err(), "{tail}");
        }
    }
    for source in [
        "erDiagram\nstyle\n",
        "erDiagram\nclassDef class\n",
        "erDiagram\n\"style\"\nstyle style\n",
    ] {
        assert!(parser.parse_diagram_sync(source).is_err(), "{source}");
    }

    let model = parse_er("erDiagram\nA:::accent\nclass A peer\nclassDef accent fill:red\n");
    assert_eq!(model.entities["A"].css_classes, "default accent peer");
    assert_eq!(model.classes["accent"].styles, ["fill:red"]);
}

#[cfg(feature = "editor-facts")]
mod editor {
    use super::*;
    use merman_core::{
        EditorExpectedSyntaxKind, EditorSemanticCompleteness, EditorSemanticFacts,
        EditorSemanticRole, EditorSemanticSymbol, ParsedEditorFacts, SemanticParseOutcome,
        SemanticSnapshotCapture,
    };
    use merman_model::source::SourceSpan;

    fn capture(source: &str) -> (ErDiagramModel, EditorSemanticFacts) {
        let SemanticSnapshotCapture::Snapshot(snapshot) =
            SemanticParser::new().capture_diagram_snapshot_sync(source)
        else {
            panic!("valid ER source must produce a semantic snapshot");
        };
        let (_, outcome, editor_facts) = snapshot.into_parts();
        let SemanticParseOutcome::Parsed(SemanticDiagramModel::Er(model)) = outcome else {
            panic!("valid ER source must retain its canonical model");
        };
        let ParsedEditorFacts::Available(facts) = editor_facts else {
            panic!("ER parser must retain editor facts");
        };
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
        assert!(facts.diagnostics.is_empty(), "{:?}", facts.diagnostics);
        (model, facts)
    }

    fn symbol_at(facts: &EditorSemanticFacts, start: usize) -> &EditorSemanticSymbol {
        facts
            .symbols
            .iter()
            .find(|symbol| symbol.selection.start == start)
            .expect("symbol at the original source offset")
    }

    #[test]
    fn keyword_style_facts_keep_target_and_class_roles_at_original_crlf_offsets() {
        let source = concat!(
            "\u{feff}---\r\ntitle: 顧客\r\n---\r\n",
            "erDiagram\r\n\"style\"\r\n",
            "style style fill:red\r\n",
            "A\r\nclassDef class fill:blue\r\n",
        );
        let (model, facts) = capture(source);
        assert_eq!(model.entities["style"].css_styles, ["fill:red"]);
        assert_eq!(model.classes["class"].styles, ["fill:blue"]);

        let declaration = source.find("\"style\"").unwrap() + 1;
        let target = source.find("style style").unwrap() + "style ".len();
        let class = source.find("classDef class").unwrap() + "classDef ".len();
        for (start, name, role) in [
            (declaration, "style", EditorSemanticRole::Entity),
            (target, "style", EditorSemanticRole::Reference),
            (class, "class", EditorSemanticRole::ClassDefinition),
        ] {
            let symbol = symbol_at(&facts, start);
            assert_eq!(symbol.name, name);
            assert_eq!(symbol.role, role);
            assert_eq!(symbol.selection, SourceSpan::new(start, start + name.len()));
            assert_eq!(&source[symbol.selection.start..symbol.selection.end], name);
        }
        assert!(facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::ClassName
                && expected.span == SourceSpan::new(class, class + "class".len())
        }));
        assert!(!facts.symbols.iter().any(|symbol| symbol.name == "fill"));
    }

    #[test]
    fn subgraph_endpoint_facts_preserve_references_inline_class_and_late_alias_ranges() {
        let source = concat!(
            "\u{feff}---\r\ntitle: 顧客\r\n---\r\n",
            "erDiagram\r\nsubgraph G\r\nA\r\nend\r\n",
            "G:::accent ||--|| B : rel\r\n",
            "style G fill:red\r\nG[\"Later\"]\r\n",
        );
        let (model, facts) = capture(source);
        assert_eq!(model.entities["G"].id, "entity-G-1");
        assert_eq!(model.entities["G"].alias, "Later");
        assert_eq!(model.entities["B"].id, "entity-B-2");
        assert_eq!(model.relationships[0].entity_a, "G");

        for (start, name, role) in [
            (
                source.find("G:::").unwrap(),
                "G",
                EditorSemanticRole::Reference,
            ),
            (
                source.find("accent").unwrap(),
                "accent",
                EditorSemanticRole::Payload,
            ),
            (source.find("B :").unwrap(), "B", EditorSemanticRole::Entity),
            (
                source.find("rel\r\n").unwrap(),
                "rel",
                EditorSemanticRole::Payload,
            ),
            (
                source.find("style G").unwrap() + "style ".len(),
                "G",
                EditorSemanticRole::Reference,
            ),
            (
                source.find("Later").unwrap(),
                "Later",
                EditorSemanticRole::Payload,
            ),
        ] {
            let symbol = symbol_at(&facts, start);
            assert_eq!(symbol.name, name);
            assert_eq!(symbol.role, role);
            assert_eq!(symbol.selection, SourceSpan::new(start, start + name.len()));
            assert_eq!(&source[symbol.selection.start..symbol.selection.end], name);
        }
        let alias_start = source.find("\"Later\"").unwrap();
        assert_eq!(
            symbol_at(&facts, alias_start + 1).span,
            SourceSpan::new(alias_start, alias_start + "\"Later\"".len())
        );
    }

    #[test]
    fn initial_keyword_errors_keep_recovered_facts_and_original_diagnostic_ranges() {
        for (tail, keyword) in [("class A style", "style"), ("A:::class", "class")] {
            let source =
                format!("\u{feff}---\r\ntitle: 顧客\r\n---\r\nerDiagram\r\nA\r\n{tail}\r\n");
            let SemanticSnapshotCapture::Snapshot(snapshot) =
                SemanticParser::new().capture_diagram_snapshot_sync(&source)
            else {
                panic!("ER grammar errors must retain a semantic snapshot");
            };
            let SemanticParseOutcome::Failed(merman_core::Error::DiagramParse {
                diagnostic, ..
            }) = snapshot.outcome()
            else {
                panic!("INITIAL keywords must remain invalid in class lists");
            };
            let start = source.rfind(keyword).unwrap();
            let span = SourceSpan::new(start, start + keyword.len());
            assert_eq!(diagnostic.span(), Some(span));
            let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                panic!("ER grammar errors must retain editor recovery facts");
            };
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
            assert!(
                facts
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.span == Some(span))
            );
        }
    }
}
