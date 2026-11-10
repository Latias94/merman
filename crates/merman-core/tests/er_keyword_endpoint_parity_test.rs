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

#[test]
fn numeric_prefixes_preserve_entity_order_endpoints_and_alias_ownership() {
    for newline in ["\n", "\r\n"] {
        for number in ["1", "12", "1.5", "12.3"] {
            let source = format!(
                "\u{feff}erDiagram{newline}{number}abc ||--|| B : rel{newline}{number}abc[Shown]{newline}"
            );
            let model = parse_er(&source);
            assert!(
                model
                    .entities
                    .keys()
                    .map(String::as_str)
                    .eq([number, "abc", "B"])
            );
            assert_eq!(model.entities[number].id, format!("entity-{number}-0"));
            assert!(model.entities[number].alias.is_empty());
            assert_eq!(model.entities["abc"].id, "entity-abc-1");
            assert_eq!(model.entities["abc"].alias, "Shown");
            assert_eq!(model.entities["B"].id, "entity-B-2");
            assert_eq!(model.relationships.len(), 1);
            assert_eq!(model.relationships[0].entity_a, "entity-abc-1");
            assert_eq!(model.relationships[0].entity_b, "entity-B-2");
            assert_eq!(model.relationships[0].role_a, "rel");
        }
    }
}

#[test]
fn numeric_prefix_rules_preserve_quoted_and_non_numeric_names() {
    for (authored, name) in [
        ("\"1abc\"", "1abc"),
        ("\"12abc\"", "12abc"),
        ("\"12.3abc\"", "12.3abc"),
        ("A12", "A12"),
        ("_12abc", "_12abc"),
        ("-12abc", "-12abc"),
    ] {
        let source = format!("erDiagram\n{authored}[Shown]\n{authored} ||--|| B : rel\n");
        let model = parse_er(&source);
        assert!(model.entities.keys().map(String::as_str).eq([name, "B"]));
        assert_eq!(model.entities[name].alias, "Shown");
        assert_eq!(model.relationships[0].entity_a, format!("entity-{name}-0"));
        assert_eq!(model.relationships[0].entity_b, "entity-B-1");
    }
}

#[test]
fn numeric_prefix_priority_keeps_cardinalities_and_rejects_split_endpoint_and_alias() {
    let model = parse_er("erDiagram\n1 ||--|| 12 : rel\nA 1--1 B : owns\n");
    assert_eq!(model.relationships.len(), 2);
    assert_eq!(model.relationships[0].entity_a, "entity-1-0");
    assert_eq!(model.relationships[0].entity_b, "entity-12-1");
    assert_eq!(model.relationships[1].rel_spec.card_a, "ONLY_ONE");
    assert_eq!(model.relationships[1].rel_spec.card_b, "ONLY_ONE");
    assert_eq!(model.relationships[1].rel_spec.rel_type, "IDENTIFYING");

    for tail in ["A ||--|| 12abc : rel", "A[12abc]", "A[12.3abc]"] {
        let source = format!("erDiagram\n{tail}\n");
        assert!(
            SemanticParser::new().parse_diagram_sync(&source).is_err(),
            "{tail}"
        );
    }
}

#[test]
fn adjacent_backtick_attributes_keep_types_keys_comments_and_ecmascript_separators() {
    for newline in ["\n", "\r\n"] {
        for separator in ["", " ", "\u{00a0}", "\u{2003}", "\u{feff}"] {
            let source = format!(
                "\u{feff}erDiagram{newline}E {{int{separator}`id` pk,{separator}Fk \"comment\"}}{newline}"
            );
            let model = parse_er(&source);
            let attributes = &model.entities["E"].attributes;
            assert_eq!(attributes.len(), 1);
            let expected_type = if matches!(separator, "\u{2003}" | "\u{feff}") {
                format!("int{separator}")
            } else {
                "int".to_string()
            };
            assert_eq!(attributes[0].ty, expected_type);
            assert_eq!(attributes[0].name, "id");
            assert_eq!(attributes[0].keys, ["pk", "Fk"]);
            assert_eq!(attributes[0].comment, "comment");
        }
    }

    for body in [
        "int`id",
        "int``",
        "int`id` PK,",
        "int`id` \"comment",
        "int@id",
    ] {
        let source = format!("erDiagram\nE {{{body}}}\n");
        assert!(
            SemanticParser::new().parse_diagram_sync(&source).is_err(),
            "{body}"
        );
    }
}

#[test]
fn subgraph_ids_titles_and_member_deduplication_use_ecmascript_trimming() {
    for newline in ["\n", "\r\n"] {
        for padding in [
            " ", "\t", "\u{00a0}", "\u{2003}", "\u{2028}", "\u{2029}", "\u{feff}",
        ] {
            let member = format!("{padding}A{padding}");
            let source = format!(
                concat!(
                    "\u{feff}erDiagram{newline}",
                    "subgraph \"{padding}G{padding}\" [\"{padding}Group{padding}\"]{newline}",
                    "\"{member}\"{newline}A{newline}end{newline}",
                    "style G fill:red{newline}class G accent{newline}",
                ),
                newline = newline,
                padding = padding,
                member = member,
            );
            let model = parse_er(&source);
            assert_eq!(model.subgraphs.len(), 1);
            let subgraph = &model.subgraphs[0];
            assert_eq!(subgraph.id, "G");
            assert_eq!(subgraph.title, "Group");
            assert_eq!(subgraph.nodes, std::slice::from_ref(&member));
            assert_eq!(subgraph.css_styles, ["fill:red"]);
            assert_eq!(subgraph.classes, ["accent"]);
            assert!(
                model
                    .entities
                    .keys()
                    .map(String::as_str)
                    .eq([member.as_str(), "A"])
            );
        }
    }
}

#[test]
fn subgraph_trimming_preserves_non_ecmascript_padding_and_empty_ids() {
    let padded_id = "\u{0085}G\u{0085}";
    let padded_title = "\u{0085}Group\u{0085}";
    let padded_member = "\u{0085}A\u{0085}";
    let source = format!(
        "erDiagram\nsubgraph \"{padded_id}\" [\"{padded_title}\"]\n\"{padded_member}\"\nA\nend\nstyle G fill:red\nstyle {padded_id} stroke:blue\n"
    );
    let model = parse_er(&source);
    assert_eq!(model.subgraphs[0].id, padded_id);
    assert_eq!(model.subgraphs[0].title, padded_title);
    assert_eq!(model.subgraphs[0].nodes, [padded_member, "A"]);
    assert_eq!(model.subgraphs[0].css_styles, ["stroke:blue"]);
    assert_eq!(model.entities.len(), 2);

    for padding in [" ", "\u{feff}"] {
        let model = parse_er(&format!("erDiagram\nsubgraph \"{padding}\"\nend\n"));
        assert!(model.subgraphs[0].id.is_empty());
        assert!(model.subgraphs[0].title.is_empty());
        assert!(model.entities.is_empty());
    }
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
    fn numeric_prefix_facts_keep_canonical_entities_and_original_byte_ranges() {
        for newline in ["\n", "\r\n"] {
            for number in ["12", "12.3"] {
                let combined = format!("{number}abc");
                let source = format!(
                    "\u{feff}---{newline}title: 顧客{newline}---{newline}erDiagram{newline}{combined} ||--|| B : rel{newline}{combined}[Shown]{newline}"
                );
                let (model, facts) = capture(&source);
                assert_eq!(model.entities["abc"].alias, "Shown");
                assert_eq!(model.relationships[0].entity_a, "entity-abc-1");
                for (start, _) in source.match_indices(combined.as_str()) {
                    for (offset, name) in [(start, number), (start + number.len(), "abc")] {
                        let symbol = symbol_at(&facts, offset);
                        assert_eq!(symbol.name, name);
                        assert_eq!(symbol.role, EditorSemanticRole::Entity);
                        assert_eq!(
                            symbol.selection,
                            SourceSpan::new(offset, offset + name.len())
                        );
                        assert_eq!(&source[symbol.selection.start..symbol.selection.end], name);
                    }
                }
                let alias_start = source.find("Shown").unwrap();
                let alias = symbol_at(&facts, alias_start);
                assert_eq!(alias.name, "Shown");
                assert_eq!(alias.role, EditorSemanticRole::Payload);
                assert!(!facts.symbols.iter().any(|symbol| symbol.name == combined));
            }
        }
    }

    #[test]
    fn adjacent_backtick_facts_keep_attribute_key_and_comment_ranges() {
        for newline in ["\n", "\r\n"] {
            for separator in ["", " ", "\u{feff}"] {
                let source = format!(
                    "\u{feff}---{newline}title: 顧客{newline}---{newline}erDiagram{newline}E {{int{separator}`属性` pk,{separator}Fk \"comment\"}}{newline}"
                );
                let (model, facts) = capture(&source);
                assert_eq!(model.entities["E"].attributes[0].name, "属性");
                let expected_type = if separator == "\u{feff}" {
                    format!("int{separator}")
                } else {
                    "int".to_string()
                };
                assert_eq!(model.entities["E"].attributes[0].keys, ["pk", "Fk"]);
                for (name, start, role) in [
                    (
                        expected_type.as_str(),
                        source.find("int").unwrap(),
                        EditorSemanticRole::Payload,
                    ),
                    (
                        "属性",
                        source.find("`属性`").unwrap() + 1,
                        EditorSemanticRole::Outline,
                    ),
                    (
                        "pk",
                        source.find("pk").unwrap(),
                        EditorSemanticRole::Payload,
                    ),
                    (
                        "Fk",
                        source.find("Fk").unwrap(),
                        EditorSemanticRole::Payload,
                    ),
                    (
                        "comment",
                        source.find("\"comment\"").unwrap() + 1,
                        EditorSemanticRole::Payload,
                    ),
                ] {
                    let symbol = symbol_at(&facts, start);
                    assert_eq!(symbol.name, name);
                    assert_eq!(symbol.role, role);
                    assert_eq!(symbol.selection, SourceSpan::new(start, start + name.len()));
                    assert_eq!(&source[symbol.selection.start..symbol.selection.end], name);
                }
                let comment_start = source.find("\"comment\"").unwrap();
                assert_eq!(
                    symbol_at(&facts, comment_start + 1).span,
                    SourceSpan::new(comment_start, comment_start + "\"comment\"".len())
                );
            }
        }
    }

    #[test]
    fn subgraph_facts_select_canonical_ids_and_preserve_ordinary_quoted_whitespace() {
        for newline in ["\n", "\r\n"] {
            for padding in [
                " ", "\t", "\u{00a0}", "\u{2003}", "\u{2028}", "\u{2029}", "\u{feff}",
            ] {
                let header = format!("\"{padding}G{padding}\"");
                let entity = format!("{padding}A{padding}");
                let alias = format!("{padding}Shown{padding}");
                let source = format!(
                    concat!(
                        "\u{feff}---{newline}title: 顧客{newline}---{newline}erDiagram{newline}",
                        "subgraph {header} [\"{padding}Group{padding}\"]{newline}end{newline}",
                        "style G fill:red{newline}class G accent{newline}",
                        "\"{entity}\"[\"{alias}\"]{newline}A{newline}",
                        "\"{entity}\" ||--|| A : rel{newline}G ||--|| B : groups{newline}",
                    ),
                    newline = newline,
                    header = header,
                    padding = padding,
                    entity = entity,
                    alias = alias,
                );
                let (model, facts) = capture(&source);
                assert_eq!(model.subgraphs[0].id, "G");
                assert_eq!(model.subgraphs[0].title, "Group");
                assert_eq!(model.subgraphs[0].css_styles, ["fill:red"]);
                assert_eq!(model.subgraphs[0].classes, ["accent"]);
                assert_eq!(model.entities[entity.as_str()].alias, alias);
                assert_eq!(model.relationships[1].entity_a, "G");

                let header_start = source.find(header.as_str()).unwrap();
                let canonical_start = header_start + 1 + padding.len();
                let declaration = symbol_at(&facts, canonical_start);
                assert_eq!(declaration.name, "G");
                assert_eq!(declaration.role, EditorSemanticRole::Entity);
                assert_eq!(
                    declaration.span,
                    SourceSpan::new(header_start, header_start + header.len())
                );
                assert_eq!(
                    declaration.selection,
                    SourceSpan::new(canonical_start, canonical_start + 1)
                );
                assert_eq!(
                    &source[declaration.selection.start..declaration.selection.end],
                    "G"
                );

                for start in [
                    source.find("style G").unwrap() + "style ".len(),
                    source.find("class G").unwrap() + "class ".len(),
                    source.find("G ||").unwrap(),
                ] {
                    let reference = symbol_at(&facts, start);
                    assert_eq!(reference.name, declaration.name);
                    assert_eq!(reference.role, EditorSemanticRole::Reference);
                    assert_eq!(reference.selection, SourceSpan::new(start, start + 1));
                }

                let entity_token = format!("\"{entity}\"");
                for ((start, _), role) in source
                    .match_indices(entity_token.as_str())
                    .zip([EditorSemanticRole::Entity, EditorSemanticRole::Reference])
                {
                    let symbol = symbol_at(&facts, start + 1);
                    assert_eq!(symbol.name, entity);
                    assert_eq!(symbol.role, role);
                    assert_eq!(
                        symbol.selection,
                        SourceSpan::new(start + 1, start + 1 + entity.len())
                    );
                    assert_eq!(
                        &source[symbol.selection.start..symbol.selection.end],
                        entity
                    );
                }
                let alias_start = source.find(alias.as_str()).unwrap();
                let alias_symbol = symbol_at(&facts, alias_start);
                assert_eq!(alias_symbol.name, alias);
                assert_eq!(alias_symbol.role, EditorSemanticRole::Payload);
                assert_eq!(
                    alias_symbol.selection,
                    SourceSpan::new(alias_start, alias_start + alias.len())
                );
            }
        }
    }

    #[test]
    fn non_ecmascript_subgraph_padding_remains_in_editor_identity() {
        for newline in ["\n", "\r\n"] {
            let name = "\u{0085}G\u{0085}";
            let source = format!(
                "\u{feff}erDiagram{newline}subgraph \"{name}\"{newline}end{newline}style {name} fill:red{newline}"
            );
            let (model, facts) = capture(&source);
            assert_eq!(model.subgraphs[0].id, name);
            assert_eq!(model.subgraphs[0].title, name);
            assert_eq!(model.subgraphs[0].css_styles, ["fill:red"]);
            let definition_start = source.find(name).unwrap();
            let target_start = source.rfind(name).unwrap();
            for (start, role) in [
                (definition_start, EditorSemanticRole::Entity),
                (target_start, EditorSemanticRole::Reference),
            ] {
                let symbol = symbol_at(&facts, start);
                assert_eq!(symbol.name, name);
                assert_eq!(symbol.role, role);
                assert_eq!(symbol.selection, SourceSpan::new(start, start + name.len()));
            }
        }
    }

    #[test]
    fn numeric_and_backtick_errors_keep_original_diagnostic_ranges() {
        for newline in ["\n", "\r\n"] {
            for (tail, offending) in [
                ("A ||--|| 12abc : rel", "abc"),
                ("A[12.3abc]", "abc"),
                ("E {int`id` PK,}", "}"),
                ("E {int``}", "}"),
            ] {
                let source = format!(
                    "\u{feff}---{newline}title: 顧客{newline}---{newline}erDiagram{newline}{tail}{newline}"
                );
                let SemanticSnapshotCapture::Snapshot(snapshot) =
                    SemanticParser::new().capture_diagram_snapshot_sync(&source)
                else {
                    panic!("ER errors must retain their canonical snapshot");
                };
                let SemanticParseOutcome::Failed(merman_core::Error::DiagramParse {
                    diagnostic,
                    ..
                }) = snapshot.outcome()
                else {
                    panic!("invalid ER source must remain a parse error: {tail}");
                };
                let start = source.rfind(offending).unwrap();
                let span = SourceSpan::new(start, start + offending.len());
                assert_eq!(diagnostic.span(), Some(span), "{tail}");
                let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                    panic!("ER errors must retain editor recovery facts");
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
