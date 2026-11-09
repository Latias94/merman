use crate::presentation::SourcePresentationConfig;
use crate::*;
use futures::executor::block_on;
use serde_json::json;

#[test]
fn class_numeric_names_preserve_semantic_and_compatibility_identity() {
    let engine = MermaidEngine::new();
    for (declaration, id, type_param) in [
        ("1", "1", ""),
        ("`1`", "1", ""),
        ("1st", "1st", ""),
        ("`1`~T~", "1", "T"),
        ("1~T~", "1", "T"),
        ("1 \t~T~", "1", "T"),
    ] {
        let source = format!("classDiagram\nclass {declaration}\n");
        let semantic = SemanticParser::new().parse_diagram_sync(&source).unwrap();
        let SemanticDiagramModel::Class(model) = semantic.model else {
            panic!("expected canonical Class model");
        };
        let parsed = engine
            .parse_diagram_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let canonical = serde_json::to_value(&model).unwrap();
        for wire in [&canonical, &parsed.model] {
            let classes = wire["classes"].as_object().unwrap();
            assert_eq!(classes.len(), 1, "{declaration}");
            let class = &classes[id];
            assert_eq!(class["id"], id, "{declaration}");
            assert_eq!(class["label"], id, "{declaration}");
            assert_eq!(class["type"], type_param, "{declaration}");
            assert_eq!(
                class["text"],
                if type_param.is_empty() {
                    id.to_string()
                } else {
                    format!("{id}&lt;{type_param}&gt;")
                },
                "{declaration}"
            );
            assert!(!classes.contains_key(&format!("classId-{id}")));
        }
    }
}

#[test]
fn class_numeric_collisions_keep_members_namespaces_and_relations_distinct() {
    let source = concat!(
        "classDiagram\n",
        "namespace Outer {\n",
        "namespace Inner {\n",
        "class `1`~Raw~ {\n",
        "+rawField\n",
        "+rawMethod()\n",
        "}\n",
        "}\n",
        "class `classId-1`~Prefixed~ {\n",
        "+prefixedField\n",
        "+prefixedMethod()\n",
        "}\n",
        "}\n",
        "`1`~Other~ --> `classId-1`~Other~ : separate\n",
        "note for 1 \" raw note \"\n",
        "note for classId-1 \" prefixed note \"\n",
    );
    let semantic = SemanticParser::new().parse_diagram_sync(source).unwrap();
    let SemanticDiagramModel::Class(model) = semantic.model else {
        panic!("expected canonical Class model");
    };
    let engine = MermaidEngine::new();
    let parsed = engine
        .parse_diagram_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let typed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let RenderSemanticModel::Class(typed_model) = typed.model() else {
        panic!("expected typed Class model");
    };
    assert_ne!(
        typed_model.class("1").unwrap().dom_id,
        typed_model.class("classId-1").unwrap().dom_id
    );
    let canonical = serde_json::to_value(&model).unwrap();
    let full = serde_json::to_value(typed_model).unwrap();
    for wire in [&canonical, &parsed.model, &full] {
        let classes = wire["classes"].as_object().unwrap();
        assert_eq!(classes.len(), 2);
        for (id, type_param, parent, member, method) in [
            ("1", "Raw", "Outer.Inner", "rawField", "rawMethod"),
            (
                "classId-1",
                "Prefixed",
                "Outer",
                "prefixedField",
                "prefixedMethod",
            ),
        ] {
            let class = &classes[id];
            assert_eq!(class["id"], id);
            assert_eq!(class["label"], id);
            assert_eq!(class["type"], type_param);
            assert_eq!(class["text"], format!("{id}&lt;{type_param}&gt;"));
            assert_eq!(class["parent"], parent);
            assert_eq!(class["members"].as_array().unwrap().len(), 1);
            assert_eq!(class["members"][0]["id"], member);
            assert_eq!(class["methods"].as_array().unwrap().len(), 1);
            assert_eq!(class["methods"][0]["id"], method);
        }
        assert_eq!(
            wire["namespaces"]["Outer"]["classIds"],
            json!(["classId-1"])
        );
        assert_eq!(wire["namespaces"]["Outer.Inner"]["classIds"], json!(["1"]));
        assert_eq!(wire["relations"].as_array().unwrap().len(), 1);
        assert_eq!(wire["relations"][0]["id1"], "1");
        assert_eq!(wire["relations"][0]["id2"], "classId-1");
        assert_eq!(wire["notes"][0]["class"], "1");
        assert_eq!(wire["notes"][0]["text"], " raw note ");
        assert_eq!(wire["notes"][1]["class"], "classId-1");
        assert_eq!(wire["notes"][1]["text"], " prefixed note ");
    }
}

#[test]
fn class_numeric_css_targets_route_each_element_before_generic_split() {
    let declarations = concat!(
        "classDiagram\n",
        "class 1~Raw~\n",
        "class classId-1~Prefixed~\n",
        "class Peer~T~\n",
        "class ` 1`~Spaced~\n",
    );
    for (statement, styled_prefixed, styled_peer, styled_spaced) in [
        ("cssClass \"1~U~,Peer~V~\" accent", true, true, false),
        ("cssClass \"Peer~V~,1~U~\" accent", true, true, false),
        ("cssClass \"Peer~V~, 1~U~\" accent", false, true, true),
        ("cssClass \"classId-1~U~\" accent", true, false, false),
        ("class 1:::accent", true, false, false),
    ] {
        let source = format!(
            "{declarations}{statement}\nclassDef accent fill:#ff0000\nstyle 1 fill:#00ff00\n"
        );
        let parsed = MermaidEngine::new()
            .parse_diagram_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let classes = parsed.model["classes"].as_object().unwrap();
        assert_eq!(classes.len(), 4, "{statement}");
        assert_eq!(classes["1"]["cssClasses"], "default", "{statement}");
        assert_eq!(
            classes["1"]["styles"],
            json!(["fill:#00ff00"]),
            "{statement}"
        );
        for (id, styled) in [
            ("classId-1", styled_prefixed),
            ("Peer", styled_peer),
            (" 1", styled_spaced),
        ] {
            assert_eq!(
                classes[id]["cssClasses"],
                if styled { "default accent" } else { "default" },
                "{statement}: {id}"
            );
            assert_eq!(
                classes[id]["styles"],
                if styled {
                    json!(["fill:#ff0000"])
                } else {
                    json!([])
                },
                "{statement}: {id}"
            );
        }
    }
}

#[test]
fn class_numeric_links_and_tooltips_keep_separate_target_routes() {
    let declarations = concat!(
        "classDiagram\n",
        "class 1~Raw~\n",
        "class classId-1~Prefixed~\n",
        "class Peer~T~\n",
        "class 2~Unprefixed~\n",
    );
    for (command, href) in [("link", ""), ("click", " href")] {
        for targets in [
            "1~U~,Peer~V~,2~W~,Unknown,Later",
            "Peer~V~,2~W~,1~U~,Unknown,Later",
        ] {
            let source = format!(
                "{declarations}{command} `{targets}`{href} \"https://example.com/\" \"Raw tooltip\" _self\nclass Later\n"
            );
            let parsed = MermaidEngine::new()
                .parse_diagram_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap();
            let classes = parsed.model["classes"].as_object().unwrap();
            assert_eq!(classes.len(), 5, "{command}: {targets}");
            assert!(!classes.contains_key("Unknown"));
            assert!(!classes.contains_key("classId-2"));
            for id in ["classId-1", "Peer"] {
                assert_eq!(classes[id]["link"], "https://example.com/");
                assert_eq!(classes[id]["linkTarget"], "_self");
                assert_eq!(classes[id]["cssClasses"], "default clickable");
            }
            for id in ["1", "2", "Later"] {
                assert!(classes[id]["link"].is_null(), "{command}: {targets}: {id}");
                assert!(classes[id]["linkTarget"].is_null());
                assert_eq!(classes[id]["cssClasses"], "default");
            }
            for id in ["1", "2", "Peer"] {
                assert_eq!(classes[id]["tooltip"], "Raw tooltip");
            }
            assert!(classes["classId-1"]["tooltip"].is_null());
            assert!(classes["Later"]["tooltip"].is_null());
            assert_eq!(classes["1"]["type"], "Raw");
            assert_eq!(classes["classId-1"]["type"], "Prefixed");
        }
    }
}

#[test]
fn class_numeric_callbacks_keep_raw_lookup_and_prefixed_clickable_routes() {
    let declarations = concat!(
        "classDiagram\n",
        "class 1~Raw~\n",
        "class classId-1~Prefixed~\n",
        "class Peer~T~\n",
        "class 2~Unprefixed~\n",
    );
    for security_level in ["strict", "loose"] {
        let engine = MermaidEngine::new().with_site_config({
            let mut config = SourcePresentationConfig::empty_object();
            config.set_value("securityLevel", json!(security_level));
            config
        });
        for (command, action, args) in [
            ("callback", "\"open\"", None),
            ("click", "call open(value)", Some("value")),
        ] {
            for targets in [
                "1~U~,Peer~V~,2~W~,Unknown,Later",
                "Peer~V~,2~W~,1~U~,Unknown,Later",
            ] {
                let source = format!(
                    "{declarations}{command} `{targets}` {action} \"Raw tooltip\"\nclass Later\n"
                );
                let parsed = engine
                    .parse_diagram_sync(&source, ParseOptions::strict())
                    .unwrap()
                    .unwrap();
                let classes = parsed.model["classes"].as_object().unwrap();
                assert_eq!(classes.len(), 5, "{command}: {security_level}: {targets}");
                for id in ["1", "2", "Peer"] {
                    assert_eq!(classes[id]["haveCallback"], true);
                    assert_eq!(classes[id]["callback"]["function"], "open");
                    assert_eq!(classes[id]["callback"]["args"].as_str(), args);
                    assert_eq!(classes[id]["callbackEffective"], security_level == "loose");
                    assert_eq!(classes[id]["tooltip"], "Raw tooltip");
                }
                for id in ["classId-1", "Later"] {
                    assert_eq!(classes[id]["haveCallback"], false);
                    assert!(classes[id]["callback"].is_null());
                    assert!(classes[id]["tooltip"].is_null());
                }
                for id in ["classId-1", "Peer"] {
                    assert_eq!(classes[id]["cssClasses"], "default clickable");
                }
                for id in ["1", "2", "Later"] {
                    assert_eq!(classes[id]["cssClasses"], "default");
                }
            }
        }
    }
}

#[test]
fn class_note_payloads_preserve_whitespace_and_namespace_ownership_across_models() {
    for payload in [
        " padded ",
        " \t ",
        "\tTabbed\t",
        "\ninner\n",
        "\r\n  名𠀀\r\n",
        "",
    ] {
        let expected_payload = payload.replace("\r\n", "\n");
        let source = format!(
            "classDiagram\nnote \"{payload}\"\nnamespace Outer {{\nclass A\nnote for A \"{payload}\"\nnamespace Inner {{\nclass B\nnote \"{payload}\"\nnote for B \"{payload}\"\n}}\n}}\n"
        );
        let semantic = SemanticParser::new().parse_diagram_sync(&source).unwrap();
        let SemanticDiagramModel::Class(model) = semantic.model else {
            panic!("expected canonical Class model");
        };
        let engine = MermaidEngine::new();
        let parsed = engine
            .parse_diagram_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let typed = engine
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let RenderSemanticModel::Class(typed_model) = typed.model() else {
            panic!("expected typed Class model");
        };
        let canonical = serde_json::to_value(&model).unwrap();
        let full = serde_json::to_value(typed_model).unwrap();
        for wire in [&canonical, &parsed.model, &full] {
            let notes = wire["notes"].as_array().unwrap();
            assert_eq!(notes.len(), 4, "{payload:?}");
            for (index, (class_id, parent)) in [
                (None, None),
                (Some("A"), Some("Outer")),
                (None, Some("Outer.Inner")),
                (Some("B"), Some("Outer.Inner")),
            ]
            .into_iter()
            .enumerate()
            {
                assert_eq!(notes[index]["id"], format!("note{index}"));
                assert_eq!(
                    notes[index]["text"].as_str(),
                    Some(expected_payload.as_str()),
                    "{payload:?}"
                );
                assert_eq!(notes[index]["class"].as_str(), class_id);
                assert_eq!(notes[index]["parent"].as_str(), parent);
            }
            assert_eq!(wire["namespaces"]["Outer"]["classIds"], json!(["A"]));
            assert_eq!(wire["namespaces"]["Outer"]["noteIds"], json!(["note1"]));
            assert_eq!(wire["namespaces"]["Outer.Inner"]["classIds"], json!(["B"]));
            assert_eq!(
                wire["namespaces"]["Outer.Inner"]["noteIds"],
                json!(["note2", "note3"])
            );
        }
    }
}

#[test]
fn class_namespace_carriers_preserve_entry_completion_and_source_order() {
    let source = concat!(
        "classDiagram\n",
        "namespace Outer {\n",
        "class Before\n",
        "namespace Inner {\n",
        "class Nested\n",
        "note for Nested \"inside\"\n",
        "}\n",
        "class After\n",
        "note \"outer\"\n",
        "}\n",
        "class Outside\n",
    );
    let parsed = MermaidEngine::new()
        .parse_diagram_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let classes = &parsed.model["classes"];
    for id in ["Before", "After"] {
        assert_eq!(classes[id]["parent"], "Outer");
    }
    assert_eq!(classes["Nested"]["parent"], "Outer.Inner");
    assert_eq!(classes["Outside"]["parent"], json!(null));
    assert_eq!(
        parsed.model["namespaces"]["Outer"]["classIds"],
        json!(["Before", "After"])
    );
    assert_eq!(
        parsed.model["namespaces"]["Outer.Inner"]["classIds"],
        json!(["Nested"])
    );
    assert_eq!(parsed.model["notes"][0]["parent"], "Outer.Inner");
    assert_eq!(parsed.model["notes"][1]["parent"], "Outer");
    assert_eq!(parsed.model["notes"][0]["text"], "inside");
    assert_eq!(parsed.model["notes"][1]["text"], "outer");
}

#[test]
fn class_namespace_carrier_failures_preserve_eof_and_first_error_recovery() {
    let engine = MermaidEngine::new();
    let incomplete = "classDiagram\nnamespace Outer {\nnamespace Inner {\nclass Leaf\n}\n";
    let unexpected = "classDiagram\n}\nnamespace Later {\nclass Leaf\n";
    for (source, span, kind) in [
        (
            incomplete,
            SourceSpan::new(incomplete.len(), incomplete.len()),
            ParseDiagnosticSpanKind::InsertionPoint,
        ),
        (
            unexpected,
            SourceSpan::new("classDiagram\n".len(), "classDiagram\n}".len()),
            ParseDiagnosticSpanKind::Exact,
        ),
    ] {
        let error = engine
            .parse_diagram_sync(source, ParseOptions::strict())
            .expect_err("malformed Class namespace is rejected");
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("Class grammar failure must remain a parse error");
        };
        assert_eq!(diagnostic.span(), Some(span));
        assert_eq!(diagnostic.span_kind(), kind);
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("classDiagram", source)
            .unwrap()
            .unwrap();
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "Leaf"));
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == EditorSemanticDiagnosticKind::ParserRecovery
                && diagnostic.span == Some(span)
        }));
    }
}

#[test]
fn parse_diagram_class_text_label_member_annotation_and_css_classes() {
    let engine = MermaidEngine::new();
    let text = r#"classDiagram
class C1["Class 1 with text label"]
<<interface>> C1
C1: member1
cssClass "C1" styleClass
"#;

    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.meta.diagram_type, "classDiagram");

    let c1 = &res.model["classes"]["C1"];
    assert_eq!(c1["label"], json!("Class 1 with text label"));
    assert_eq!(c1["cssClasses"], json!("default styleClass"));
    assert_eq!(c1["annotations"][0], json!("interface"));
    assert_eq!(c1["members"][0]["displayText"], json!("member1"));
}

#[test]
fn parse_diagram_class_supports_inline_annotation_after_class_identifier() {
    let engine = MermaidEngine::new();
    let text = "classDiagram\nclass Shape <<interface>>\n";

    let res = block_on(engine.parse_diagram(text, ParseOptions::strict()))
        .unwrap()
        .unwrap();

    assert_eq!(
        res.model["classes"]["Shape"]["annotations"],
        json!(["interface"])
    );

    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("class", text)
        .unwrap()
        .expect("class editor facts");
    assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
    assert!(facts.symbols.iter().any(|symbol| {
        symbol.name == "interface"
            && symbol.role == EditorSemanticRole::Payload
            && symbol.detail.as_deref() == Some("class annotation")
    }));

    let with_members = "classDiagram\nclass Shape <<interface>> {\n  noOfVertices\n  draw()\n}\n";
    let res = block_on(engine.parse_diagram(with_members, ParseOptions::strict()))
        .unwrap()
        .unwrap();
    let shape = &res.model["classes"]["Shape"];
    assert_eq!(shape["annotations"], json!(["interface"]));
    assert_eq!(shape["members"].as_array().unwrap().len(), 1);
    assert_eq!(shape["methods"].as_array().unwrap().len(), 1);
}

#[test]
fn parse_class_editor_facts_do_not_expect_a_target_after_inline_annotations() {
    let engine = MermaidEngine::new();
    for newline in ["\n", "\r\n", "\r"] {
        for declaration in [
            "class Shape <<interface>>",
            "class Shape <<interface>> ",
            "class Shape <<interface>> {}",
            "class Shape <<interface>> {\n+draw()\n}",
        ] {
            let declaration = declaration.replace('\n', newline);
            let text = format!("classDiagram{newline}{declaration}{newline}");
            let facts = engine
                .parse_editor_semantic_facts_with_type_sync("classDiagram", &text)
                .unwrap()
                .unwrap();
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
            assert!(
                facts.expected_syntax.iter().all(|expected| {
                    expected.kind != EditorExpectedSyntaxKind::NodeIdentifier
                        || expected.span.start != expected.span.end
                }),
                "completed inline annotation must not request a class target: {text:?}"
            );
            assert!(facts.symbols.iter().any(|symbol| {
                symbol.name == "interface"
                    && symbol.role == EditorSemanticRole::Payload
                    && symbol.detail.as_deref() == Some("class annotation")
            }));
        }
    }
}

#[test]
fn parse_class_editor_facts_keep_the_standalone_annotation_target_slot() {
    let engine = MermaidEngine::new();
    for newline in ["\n", "\r\n", "\r"] {
        let text = format!("classDiagram{newline}class Shape{newline}<<interface>>");
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("classDiagram", &text)
            .unwrap()
            .unwrap();
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::NodeIdentifier
                && expected.span == SourceSpan::new(text.len(), text.len())
        }));

        let text = format!("{text} Shape");
        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("classDiagram", &text)
            .unwrap()
            .unwrap();
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
        let target = facts
            .symbols
            .iter()
            .find(|symbol| symbol.detail.as_deref() == Some("class annotation target"))
            .unwrap();
        assert_eq!(target.name, "Shape");
        assert_eq!(target.role, EditorSemanticRole::Reference);
        assert_eq!(&text[target.selection.start..target.selection.end], "Shape");
    }
}

#[test]
fn parse_diagram_class_multibyte_name_does_not_panic() {
    let engine = MermaidEngine::new();
    let text = r#"classDiagram
class 顧客
"#;

    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert!(res.model["classes"]["顧客"].is_object());
}

#[test]
fn parse_diagram_class_accepts_ecmascript_unicode_whitespace() {
    let engine = MermaidEngine::new();
    for whitespace in ['\u{00A0}', '\u{202F}', '\u{3000}', '\u{FEFF}'] {
        for newline in ["\n", "\r\n"] {
            let text = format!(
                "classDiagram{newline}direction{whitespace}LR{whitespace}{newline}\
                 class{whitespace}顾客{whitespace}{newline}\
                 class{whitespace}订单{newline}顾客{whitespace}-->{whitespace}订单{newline}"
            );
            let res = engine
                .parse_diagram_sync(&text, ParseOptions::default())
                .unwrap()
                .unwrap();
            assert!(res.model["classes"]["顾客"].is_object(), "{text:?}");
            assert!(res.model["classes"]["订单"].is_object(), "{text:?}");
            assert_eq!(res.model["direction"], "LR");
            assert_eq!(res.model["relations"].as_array().unwrap().len(), 1);
        }
    }
}

#[test]
fn parse_diagram_class_rejects_characters_outside_mermaid_unicode_text() {
    let engine = MermaidEngine::new();
    for name in ["𠀀", "顾客１", "顾客、", "A\u{0345}", "😀"] {
        let text = format!("classDiagram\nclass {name}\n");
        assert!(
            engine
                .parse_diagram_sync(&text, ParseOptions::default())
                .is_err(),
            "{text:?}"
        );
    }
}

#[test]
fn parse_diagram_class_preserves_unicode_in_quoted_names_and_generics() {
    let engine = MermaidEngine::new();
    let text = "classDiagram\nclass `𠀀、１`\nclass Box~𠀀、１~\n";
    let res = engine
        .parse_diagram_sync(text, ParseOptions::default())
        .unwrap()
        .unwrap();
    assert!(res.model["classes"]["𠀀、１"].is_object());
    assert_eq!(res.model["classes"]["Box"]["type"], "𠀀、１");
}

#[test]
fn parse_class_editor_facts_recovers_after_invalid_unicode_name() {
    let engine = MermaidEngine::new();
    let text = "classDiagram\nclass 𠀀\nclass User\n";
    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("classDiagram", text)
        .unwrap()
        .expect("class editor facts");
    assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
    assert!(facts.symbols.iter().any(|symbol| symbol.name == "User"));
    let start = text.find('𠀀').unwrap();
    assert!(facts.diagnostics.iter().any(|diagnostic| {
        diagnostic.span == Some(SourceSpan::new(start, start + '𠀀'.len_utf8()))
    }));
}

#[test]
fn parse_diagram_class_css_class_shorthand() {
    let engine = MermaidEngine::new();
    let text = r#"classDiagram
class C1["Class 1 with text label"]:::styleClass
"#;

    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    let c1 = &res.model["classes"]["C1"];
    assert_eq!(c1["label"], json!("Class 1 with text label"));
    assert_eq!(c1["cssClasses"], json!("default styleClass"));
}

#[test]
fn class_style_precedence_facts_follow_applied_actions() {
    let engine = MermaidEngine::new();
    let parsed = engine
        .parse_diagram_for_render_model_sync(
            concat!(
                "classDiagram\n",
                "class User\n",
                "cssClass \"Missing\" accent\n",
                "classDef accent fill:red\n",
                "cssClass \"User\" accent\n",
                "style User fill:red\n",
            ),
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("class render model");
    let RenderSemanticModel::Class(_) = parsed.model() else {
        panic!("expected ClassDiagram render model");
    };
    let facts = parsed
        .class_style_precedence_facts()
        .expect("class style precedence facts");
    assert!(!facts.assignment_before_definition_copy());
    assert!(facts.definition_before_assignment_no_backfill());
    let witness = facts
        .definition_before_assignment_no_backfill_witness()
        .expect("no-backfill witness");
    assert_eq!(witness.class_name(), "accent");
    assert_eq!(witness.target(), "User");
    assert_eq!(witness.styles(), ["fill:red"]);
    assert!(witness.earlier_style_event_ordinal() < witness.later_style_event_ordinal());

    let parsed = engine
        .parse_diagram_for_render_model_sync(
            concat!(
                "classDiagram\n",
                "class User\n",
                "cssClass \"User\" accent\n",
                "classDef accent fill:red\n",
            ),
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("class render model");
    let RenderSemanticModel::Class(_) = parsed.model() else {
        panic!("expected ClassDiagram render model");
    };
    let facts = parsed
        .class_style_precedence_facts()
        .expect("class style precedence facts");
    assert!(facts.assignment_before_definition_copy());
    assert!(!facts.definition_before_assignment_no_backfill());
    let witness = facts
        .assignment_before_definition_copy_witness()
        .expect("copy witness");
    assert_eq!(witness.class_name(), "accent");
    assert_eq!(witness.target(), "User");
    assert_eq!(witness.styles(), ["fill:red"]);
    assert!(witness.earlier_style_event_ordinal() < witness.later_style_event_ordinal());
}

#[test]
fn class_style_facts_stay_outside_the_serialized_model() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            concat!(
                "classDiagram\n",
                "class User\n",
                "cssClass \"User\" accent\n",
                "classDef accent fill:red\n",
            ),
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("class render model");
    let RenderSemanticModel::Class(model) = parsed.model() else {
        panic!("expected ClassDiagram render model");
    };
    let encoded = serde_json::to_value(model).expect("class model serializes");
    let decoded: merman_model::families::class::ClassDiagram =
        serde_json::from_value(encoded).expect("class model deserializes");
    assert_eq!(model, &decoded);
    assert!(
        parsed
            .class_style_precedence_facts()
            .and_then(|facts| facts.assignment_before_definition_copy_witness())
            .is_some()
    );
}

#[test]
fn class_style_precedence_facts_require_paint_declarations() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            concat!(
                "classDiagram\n",
                "class User\n",
                "cssClass \"User\" accent\n",
                "classDef accent font-size:24px,font-weight:bold,text-decoration:underline\n",
                "cssClass \"User\" accent\n",
            ),
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("class render model");
    let RenderSemanticModel::Class(_) = parsed.model() else {
        panic!("expected ClassDiagram render model");
    };
    let facts = parsed
        .class_style_precedence_facts()
        .expect("class style precedence facts");
    assert!(!facts.assignment_before_definition_copy());
    assert!(!facts.definition_before_assignment_no_backfill());
}

#[test]
fn class_style_precedence_facts_ignore_interaction_classes() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            concat!(
                "classDiagram\n",
                "class User\n",
                "click User href \"https://example.com\"\n",
                "classDef clickable fill:red\n",
            ),
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("class render model");
    let RenderSemanticModel::Class(_) = parsed.model() else {
        panic!("expected ClassDiagram render model");
    };
    let facts = parsed
        .class_style_precedence_facts()
        .expect("class style precedence facts");
    assert!(!facts.assignment_before_definition_copy());
    assert!(!facts.definition_before_assignment_no_backfill());
}

#[test]
fn class_style_precedence_facts_ignore_repeated_assignments() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            concat!(
                "classDiagram\n",
                "class User\n",
                "cssClass \"User\" accent\n",
                "classDef accent fill:red\n",
                "cssClass \"User\" accent\n",
            ),
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("class render model");
    let RenderSemanticModel::Class(_) = parsed.model() else {
        panic!("expected ClassDiagram render model");
    };
    let facts = parsed
        .class_style_precedence_facts()
        .expect("class style precedence facts");
    assert!(facts.assignment_before_definition_copy());
    assert!(!facts.definition_before_assignment_no_backfill());
}

#[test]
fn parse_diagram_class_namespace_and_generic_methods() {
    let engine = MermaidEngine::new();
    let text = r#"classDiagram
namespace Company.Project {
  class User {
    +login(username: String, password: String)
    +logout()
  }
}
namespace Company.Project.Module {
  class GenericClass~T~ {
    +addItem(item: T)
    +getItem() T
  }
}
"#;

    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    let user = &res.model["classes"]["User"];
    assert_eq!(user["parent"], json!("Company.Project"));
    assert_eq!(
        user["methods"][0]["displayText"],
        json!("+login(username: String, password: String)")
    );
    assert_eq!(user["methods"][1]["displayText"], json!("+logout()"));

    let generic = &res.model["classes"]["GenericClass"];
    assert_eq!(generic["type"], json!("T"));
    assert_eq!(generic["parent"], json!("Company.Project.Module"));
    assert_eq!(
        generic["methods"][0]["displayText"],
        json!("+addItem(item: T)")
    );
    assert_eq!(
        generic["methods"][1]["displayText"],
        json!("+getItem() : T")
    );
}

#[test]
fn parse_diagram_class_method_parser_matches_upstream_greedy_regex_boundary() {
    let engine = MermaidEngine::new();
    let text = r#"classDiagram
class Parser {
  +outer(inner)(value) Result$
}
"#;

    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    let method = &res.model["classes"]["Parser"]["methods"][0];
    assert_eq!(
        method["displayText"],
        json!("+outer(inner)(value) : Result")
    );
    assert_eq!(method["id"], json!("outer(inner)"));
    assert_eq!(method["parameters"], json!("value"));
    assert_eq!(method["returnType"], json!("Result"));
    assert_eq!(method["cssStyle"], json!("text-decoration:underline;"));
}

#[test]
fn parse_diagram_class_acc_descr_multiline_collapses_newline_whitespace_without_regex() {
    let engine = MermaidEngine::new();
    let text = r#"classDiagram
accTitle: My Title
accDescr {
  This is my multi
  line description
}
"#;

    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.model["accTitle"], json!("My Title"));
    assert_eq!(
        res.model["accDescr"],
        json!("This is my multi\nline description")
    );
}

#[test]
fn parse_diagram_class_hierarchical_dotted_namespace_and_notes() {
    let engine = MermaidEngine::new();
    let text = r#"classDiagram
namespace Company.Project.Module {
  class User
  note "Module scoped note"
}
"#;

    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();

    let namespaces = res.model["namespaces"].as_object().unwrap();
    assert!(namespaces.contains_key("Company"));
    assert!(namespaces.contains_key("Company.Project"));
    assert!(namespaces.contains_key("Company.Project.Module"));
    assert_eq!(namespaces["Company"]["label"], json!("Company"));
    assert_eq!(namespaces["Company"]["parent"], json!(null));
    assert_eq!(namespaces["Company"]["explicit"], json!(false));
    assert_eq!(namespaces["Company.Project"]["label"], json!("Project"));
    assert_eq!(namespaces["Company.Project"]["parent"], json!("Company"));
    assert_eq!(namespaces["Company.Project"]["explicit"], json!(false));
    assert_eq!(
        namespaces["Company.Project.Module"]["label"],
        json!("Module")
    );
    assert_eq!(
        namespaces["Company.Project.Module"]["parent"],
        json!("Company.Project")
    );
    assert_eq!(
        namespaces["Company.Project.Module"]["classIds"],
        json!(["User"])
    );
    assert_eq!(
        namespaces["Company.Project.Module"]["noteIds"],
        json!(["note0"])
    );

    assert_eq!(
        res.model["classes"]["User"]["parent"],
        json!("Company.Project.Module")
    );
    assert_eq!(
        res.model["notes"][0]["parent"],
        json!("Company.Project.Module")
    );
    assert_eq!(res.model["notes"][0]["class"], json!(null));
    assert_eq!(res.model["notes"][0]["text"], json!("Module scoped note"));
}

#[test]
fn parse_diagram_class_nested_namespace_syntax_builds_qualified_parents() {
    let engine = MermaidEngine::new();
    let text = r#"classDiagram
namespace Company {
  namespace Project {
    class User
  }
}
"#;

    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();

    assert_eq!(res.model["namespaces"]["Company"]["explicit"], json!(true));
    assert_eq!(
        res.model["namespaces"]["Company.Project"]["parent"],
        json!("Company")
    );
    assert_eq!(
        res.model["namespaces"]["Company.Project"]["explicit"],
        json!(true)
    );
    assert_eq!(
        res.model["classes"]["User"]["parent"],
        json!("Company.Project")
    );
}

#[test]
fn parse_diagram_class_quoted_namespace_whitespace_keeps_distinct_ids() {
    let engine = MermaidEngine::new();
    for namespace_id in ["M", "N ", " N", " N "] {
        let source = format!(
            "classDiagram\nnamespace `{namespace_id}` {{\nclass A\n}}\nnamespace N {{\nclass B\n}}\n"
        );
        let parsed = engine
            .parse_diagram_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let namespaces = parsed.model["namespaces"].as_object().unwrap();
        assert_eq!(namespaces.len(), 2, "{namespace_id:?}");
        for (id, class_id) in [(namespace_id, "A"), ("N", "B")] {
            assert_eq!(namespaces[id]["label"], json!(id));
            assert_eq!(namespaces[id]["parent"], json!(null));
            assert_eq!(namespaces[id]["explicit"], json!(true));
            assert_eq!(namespaces[id]["classIds"], json!([class_id]));
            assert_eq!(parsed.model["classes"][class_id]["parent"], json!(id));
        }
    }
}

#[test]
fn parse_diagram_class_nested_namespace_redeclarations_preserve_authored_ids() {
    let source = r#"classDiagram
namespace `Outer ` {
  class Before
  namespace ` Inner `["First"] {
    class A
  }
  class After
}
namespace Outer {
  namespace Inner {
    class Control
  }
}
namespace `Outer ` {
  namespace ` Inner `["Renamed"] {
    class B
  }
  class Parent
}
namespace `Outer . Inner ` {
  class C
}
class Outside
"#;
    let parsed = MermaidEngine::new()
        .parse_diagram_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let namespaces = parsed.model["namespaces"].as_object().unwrap();
    assert_eq!(namespaces.len(), 4);
    for (id, label, parent, class_ids) in [
        ("Outer ", "Outer ", None, vec!["Before", "After", "Parent"]),
        (
            "Outer . Inner ",
            "Renamed",
            Some("Outer "),
            vec!["A", "B", "C"],
        ),
        ("Outer", "Outer", None, vec![]),
        ("Outer.Inner", "Inner", Some("Outer"), vec!["Control"]),
    ] {
        assert_eq!(namespaces[id]["label"], json!(label));
        assert_eq!(namespaces[id]["parent"], json!(parent));
        assert_eq!(namespaces[id]["explicit"], json!(true));
        assert_eq!(namespaces[id]["classIds"], json!(class_ids));
        for class_id in class_ids {
            assert_eq!(parsed.model["classes"][class_id]["parent"], json!(id));
        }
    }
    assert_eq!(parsed.model["classes"]["Outside"]["parent"], json!(null));
}

#[test]
fn parse_diagram_class_hierarchical_namespaces_can_be_disabled() {
    let engine = MermaidEngine::new();
    let text = r#"---
config:
  class:
    hierarchicalNamespaces: false
---
classDiagram
namespace Company.Project.Module {
  class User
  note "Module scoped note"
}
"#;

    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();

    let namespaces = res.model["namespaces"].as_object().unwrap();
    assert_eq!(
        namespaces.keys().cloned().collect::<Vec<_>>(),
        vec!["Company.Project.Module"]
    );
    assert_eq!(
        namespaces["Company.Project.Module"]["label"],
        json!("Company.Project.Module")
    );
    assert_eq!(namespaces["Company.Project.Module"]["parent"], json!(null));
    assert_eq!(
        res.model["classes"]["User"]["parent"],
        json!("Company.Project.Module")
    );
    assert_eq!(
        res.model["notes"][0]["parent"],
        json!("Company.Project.Module")
    );
}

#[test]
fn parse_diagram_class_relation_only_generic_classes_keep_type_params() {
    let engine = MermaidEngine::new();
    let text = r#"classDiagram
Class01~T~ <|-- AveryLongClass
Class03~T~ *-- Class04~T~
Class08 <--> C2: Cool label
"#;

    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();

    let class01 = &res.model["classes"]["Class01"];
    assert_eq!(class01["type"], json!("T"));
    assert_eq!(class01["text"], json!("Class01&lt;T&gt;"));

    let class03 = &res.model["classes"]["Class03"];
    assert_eq!(class03["type"], json!("T"));
    assert_eq!(class03["text"], json!("Class03&lt;T&gt;"));

    let class04 = &res.model["classes"]["Class04"];
    assert_eq!(class04["type"], json!("T"));
    assert_eq!(class04["text"], json!("Class04&lt;T&gt;"));
}

#[test]
fn parse_diagram_class_relation_with_label_and_direction() {
    let engine = MermaidEngine::new();
    let text = r#"classDiagram
direction LR
class Admin
class Report
Admin --> Report : generates
"#;

    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    assert_eq!(res.model["direction"], json!("LR"));

    let rels = res.model["relations"].as_array().unwrap();
    assert_eq!(rels.len(), 1);
    assert_eq!(rels[0]["id1"], json!("Admin"));
    assert_eq!(rels[0]["id2"], json!("Report"));
    assert_eq!(rels[0]["title"], json!("generates"));
    assert_eq!(rels[0]["relation"]["lineType"], json!(0));
    assert_eq!(rels[0]["relation"]["type1"], json!(-1));
    assert_eq!(rels[0]["relation"]["type2"], json!(3));
}

#[test]
fn parse_diagram_class_adjacent_operators_preserve_relationship_types() {
    let engine = MermaidEngine::new();
    for (relation, source_id, type1, type2, line_type) in [
        ("A*--B", "A", 2, -1, 0),
        ("A*..B", "A", 2, -1, 1),
        ("A--*B", "A", -1, 2, 0),
        ("A<|--B", "A", 1, -1, 0),
        ("A--|>B", "A", -1, 1, 0),
        ("A<|..B", "A", 1, -1, 1),
        ("A..|>B", "A", -1, 1, 1),
        ("`A*`*--B", "A*", 2, -1, 0),
        ("A-o--B", "A-", 0, -1, 0),
        ("A-o..B", "A-", 0, -1, 1),
        ("A.o--B", "A.", 0, -1, 0),
        ("A~T~o--B", "A", 0, -1, 0),
        ("类o--B", "类", 0, -1, 0),
        ("Afoo--B", "Afoo", -1, -1, 0),
        ("`A-o`--B", "A-o", -1, -1, 0),
    ] {
        let text = format!("classDiagram\n{relation}\n");
        let parsed = engine
            .parse_diagram_sync(&text, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(
            parsed.model["classes"].as_object().unwrap().len(),
            2,
            "{relation}"
        );
        assert!(parsed.model["classes"][source_id].is_object(), "{relation}");
        assert!(parsed.model["classes"]["B"].is_object(), "{relation}");
        let edge = &parsed.model["relations"][0];
        assert_eq!(edge["id1"], source_id, "{relation}");
        assert_eq!(edge["id2"], "B", "{relation}");
        assert_eq!(
            edge["relation"],
            json!({"type1": type1, "type2": type2, "lineType": line_type}),
            "{relation}"
        );
    }
}

#[test]
fn parse_diagram_class_adjacent_lollipops_create_interfaces() {
    let engine = MermaidEngine::new();
    for (relation, id1, id2, type1, type2) in [
        ("I()--A", "interface0", "A", 4, -1),
        ("A--()I", "A", "interface0", -1, 4),
    ] {
        let text = format!("classDiagram\n{relation}\n");
        let parsed = engine
            .parse_diagram_sync(&text, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(parsed.model["classes"].as_object().unwrap().len(), 1);
        assert!(parsed.model["classes"]["A"].is_object());
        assert_eq!(parsed.model["interfaces"].as_array().unwrap().len(), 1);
        assert_eq!(parsed.model["interfaces"][0]["id"], "interface0");
        assert_eq!(parsed.model["interfaces"][0]["label"], "I");
        let edge = &parsed.model["relations"][0];
        assert_eq!(edge["id1"], id1, "{relation}");
        assert_eq!(edge["id2"], id2, "{relation}");
        assert_eq!(edge["relation"]["type1"], type1, "{relation}");
        assert_eq!(edge["relation"]["type2"], type2, "{relation}");
    }
}

#[test]
fn parse_diagram_class_compact_bodies_preserve_members_and_namespace() {
    let engine = MermaidEngine::new();
    for body in [
        "class A { +run() }",
        "class A {\n+run() }",
        "class A { +field\n+run() }",
    ] {
        let text = format!("classDiagram\nnamespace N {{\n{body}\n}}\n");
        let parsed = engine
            .parse_diagram_sync(&text, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let class = &parsed.model["classes"]["A"];
        assert_eq!(class["parent"], "N");
        assert_eq!(class["methods"].as_array().unwrap().len(), 1);
        assert_eq!(class["methods"][0]["displayText"], "+run()");
        assert_eq!(
            class["members"].as_array().unwrap().len(),
            usize::from(body.contains("+field"))
        );
        assert_eq!(parsed.model["namespaces"]["N"]["classIds"], json!(["A"]));
    }
}

#[test]
fn parse_diagram_class_rejects_unexpected_braces_after_member_text() {
    let engine = MermaidEngine::new();
    for newline in ["\n", "\r", "\r\n"] {
        for member in ["{", "+field{value", "+run() {"] {
            let text = format!("classDiagram{newline}class A {{ {member} }}{newline}");
            let opening = text.rfind('{').unwrap();
            let error = engine
                .parse_diagram_sync(&text, ParseOptions::strict())
                .expect_err("an opening brace inside a class member is invalid");
            let Error::DiagramParse { diagnostic, .. } = error else {
                panic!("unexpected brace must remain a Class parse error");
            };
            assert_eq!(
                diagnostic.span(),
                Some(SourceSpan::new(opening, opening + 1))
            );
            assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);
        }
    }
}

#[test]
fn parse_diagram_class_quoted_generic_names_keep_types_and_bodies() {
    let engine = MermaidEngine::new();
    for declaration in [
        "class `Car`~T~",
        "class `Car` \t~T~",
        "class `Car~T~`",
        "class `Car`~T~ {\n+drive()\n}",
        "class `Car~T~` {\n+drive()\n}",
    ] {
        let text = format!("classDiagram\n{declaration}\n");
        let parsed = engine
            .parse_diagram_sync(&text, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let class = &parsed.model["classes"]["Car"];
        assert_eq!(class["type"], "T");
        assert_eq!(class["text"], "Car&lt;T&gt;");
        if declaration.contains("drive") {
            assert_eq!(class["methods"][0]["displayText"], "+drive()");
        }
    }
}

#[test]
fn parse_diagram_class_spaced_generics_keep_labels_members_and_namespace_identity() {
    let engine = MermaidEngine::new();
    for whitespace in [" ", "\t", " \t", "\u{00a0}"] {
        let text = format!(
            "classDiagram\nnamespace N {{\nclass Box{whitespace}~T, U~[\"Container\"] {{\n+value\n+get()\n}}\n}}\n"
        );
        let parsed = engine
            .parse_diagram_sync(&text, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let classes = parsed.model["classes"].as_object().unwrap();
        assert_eq!(classes.len(), 1);
        let class = &classes["Box"];
        assert_eq!(class["type"], "T, U");
        assert_eq!(class["label"], "Container");
        assert_eq!(class["text"], "Container<T, U>");
        assert_eq!(class["parent"], "N");
        assert_eq!(class["members"][0]["displayText"], "+value");
        assert_eq!(class["methods"][0]["displayText"], "+get()");
        assert_eq!(parsed.model["namespaces"]["N"]["classIds"], json!(["Box"]));
    }
}

#[test]
fn parse_diagram_class_spaced_generic_relation_endpoints_keep_types_and_cardinalities() {
    let text = "classDiagram\nSource \t~T~ \"1\" o-- \"*\" Target ~U~ : uses\n";
    let parsed = MermaidEngine::new()
        .parse_diagram_sync(text, ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_eq!(parsed.model["classes"].as_object().unwrap().len(), 2);
    assert_eq!(parsed.model["classes"]["Source"]["type"], "T");
    assert_eq!(parsed.model["classes"]["Target"]["type"], "U");
    let relation = &parsed.model["relations"][0];
    assert_eq!(relation["id1"], "Source");
    assert_eq!(relation["id2"], "Target");
    assert_eq!(relation["relationTitle1"], "1");
    assert_eq!(relation["relationTitle2"], "*");
    assert_eq!(relation["title"], "uses");
}

#[test]
fn parse_diagram_class_bare_names_have_no_semantic_or_editor_side_effects() {
    let engine = MermaidEngine::new();
    let declared = "classDiagram\nclass A~T~\n";
    let text = format!("{declared}A\nA ~Changed~\nMissing~U~\n`Not A Class`\n");
    let expected = engine
        .parse_diagram_sync(declared, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let parsed = engine
        .parse_diagram_sync(&text, ParseOptions::strict())
        .unwrap()
        .unwrap();
    assert_eq!(parsed.model, expected.model);

    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("classDiagram", &text)
        .unwrap()
        .unwrap();
    assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
    let classes = facts
        .symbols
        .iter()
        .filter(|symbol| symbol.kind == EditorSemanticKind::Class)
        .collect::<Vec<_>>();
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "A");
    assert_eq!(classes[0].role, EditorSemanticRole::Entity);
}

#[test]
fn parse_diagram_class_rejects_names_without_an_identity_before_the_generic() {
    let engine = MermaidEngine::new();
    for name in ["`~A`", "`~A~`", "`~类型`", "~A~", "``"] {
        let text = format!("classDiagram\nclass {name}\nclass Valid\n");
        let start = text
            .find('~')
            .unwrap_or_else(|| text.find("``").unwrap() + 1);
        let error = engine
            .parse_diagram_sync(&text, ParseOptions::strict())
            .expect_err("a generic type cannot replace the class identity");
        let Error::DiagramParse { diagnostic, .. } = error else {
            panic!("invalid class name must remain a Class parse error");
        };
        assert_eq!(diagnostic.span(), Some(SourceSpan::new(start, start + 1)));
        assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);

        let facts = engine
            .parse_editor_semantic_facts_with_type_sync("classDiagram", &text)
            .unwrap()
            .unwrap();
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(facts.symbols.iter().any(|symbol| symbol.name == "Valid"));
        assert!(facts.symbols.iter().all(|symbol| !symbol.name.is_empty()));
        assert!(!facts.symbols.iter().any(|symbol| symbol.name == "~A"));
    }
    for statement in ["class `~A`\nclass `~B`", "Known --> `~A`", "`~A`"] {
        let text = format!("classDiagram\n{statement}\n");
        assert!(
            engine
                .parse_diagram_sync(&text, ParseOptions::strict())
                .is_err()
        );
    }
}

#[test]
fn parse_diagram_class_relabelling_uses_the_stored_generic_type() {
    let engine = MermaidEngine::new();
    for first in ["class Box~T~", "Box~T~ --> Item"] {
        for label in ["class Box[\"Container\"]", "class Box~U~[\"Container\"]"] {
            let text = format!("classDiagram\n{first}\n{label}\n");
            let parsed = engine
                .parse_diagram_sync(&text, ParseOptions::strict())
                .unwrap()
                .unwrap();
            let class = &parsed.model["classes"]["Box"];
            assert_eq!(class["type"], "T");
            assert_eq!(class["label"], "Container");
            assert_eq!(class["text"], "Container<T>");
        }
    }
}

#[test]
fn parse_diagram_class_group_generic_preserves_the_authored_type_and_text() {
    let engine = MermaidEngine::new();
    let probe = engine
        .parse_diagram_sync("classDiagram\nclass A~group~\n", ParseOptions::strict())
        .unwrap()
        .unwrap();
    let control = engine
        .parse_diagram_sync(
            "classDiagram\nclass A[\"A&lt;group&gt;\"]\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap();

    assert_eq!(probe.model["classes"].as_object().unwrap().len(), 1);
    let probe_class = &probe.model["classes"]["A"];
    let control_class = &control.model["classes"]["A"];
    assert_eq!(probe_class["type"], json!("group"));
    assert_eq!(control_class["type"], json!(""));
    assert_eq!(probe_class["text"], json!("A&lt;group&gt;"));
    assert_eq!(probe_class["text"], control_class["text"]);
    assert!(probe.model["namespaces"].as_object().unwrap().is_empty());
}

#[test]
fn parse_diagram_class_style_statement_sets_node_styles() {
    let engine = MermaidEngine::new();
    let text = r#"classDiagram
class Class01
style Class01 fill:#f9f,stroke:#333,stroke-width:4px
"#;

    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    let c = &res.model["classes"]["Class01"];
    assert_eq!(
        c["styles"],
        json!(["fill:#f9f", "stroke:#333", "stroke-width:4px"])
    );
}

#[test]
fn parse_diagram_class_classdef_applies_styles_to_css_classes() {
    let engine = MermaidEngine::new();
    let text = r#"classDiagram
class Class01
cssClass "Class01" pink
classDef pink fill:#f9f
"#;

    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    let c = &res.model["classes"]["Class01"];
    assert_eq!(c["cssClasses"], json!("default pink"));
    assert_eq!(c["styles"], json!(["fill:#f9f"]));
}

#[test]
fn parse_diagram_class_cssclass_targets_preserve_whitespace_and_generic_identity() {
    let engine = MermaidEngine::new();
    for (targets, styled_a, styled_b) in [
        ("A,B", true, true),
        ("A, B", true, false),
        (" A,B", false, true),
        ("A,B ", true, false),
        ("A~U~,B~V~", true, true),
        ("A~U~, B~V~", true, false),
    ] {
        let source = format!(
            "classDiagram\nclass A~T~\nclass B~T~\ncssClass \"{targets}\" accent\nclassDef accent fill:#ff0000\n"
        );
        let parsed = engine
            .parse_diagram_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let classes = parsed.model["classes"].as_object().unwrap();
        assert_eq!(classes.len(), 2, "{targets}");
        for (id, styled) in [("A", styled_a), ("B", styled_b)] {
            let class = &classes[id];
            assert_eq!(class["type"], json!("T"), "{targets}: {id}");
            assert_eq!(
                class["cssClasses"],
                json!(if styled { "default accent" } else { "default" }),
                "{targets}: {id}"
            );
            assert_eq!(
                class["styles"],
                if styled {
                    json!(["fill:#ff0000"])
                } else {
                    json!([])
                },
                "{targets}: {id}"
            );
        }
    }
}

#[test]
fn parse_diagram_class_cssclass_can_target_an_authored_leading_space() {
    let parsed = MermaidEngine::new()
        .parse_diagram_sync(
            concat!(
                "classDiagram\n",
                "class A\n",
                "class B\n",
                "class ` B`\n",
                "cssClass \"A, B\" accent\n",
                "classDef accent fill:#ff0000\n",
            ),
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap();

    let classes = parsed.model["classes"].as_object().unwrap();
    assert_eq!(classes.len(), 3);
    for id in ["A", " B"] {
        assert_eq!(classes[id]["cssClasses"], json!("default accent"));
        assert_eq!(classes[id]["styles"], json!(["fill:#ff0000"]));
    }
    assert_eq!(classes["B"]["cssClasses"], json!("default"));
    assert_eq!(classes["B"]["styles"], json!([]));
}

#[test]
fn parse_diagram_class_multiple_classdefs_merge_styles() {
    let engine = MermaidEngine::new();
    let text = r#"classDiagram
class Class01:::pink
cssClass "Class01" bold
classDef pink fill:#f9f
classDef bold stroke:#333,stroke-width:6px
"#;

    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    let c = &res.model["classes"]["Class01"];
    assert_eq!(c["cssClasses"], json!("default pink bold"));
    assert_eq!(
        c["styles"],
        json!(["fill:#f9f", "stroke:#333", "stroke-width:6px"])
    );
}

#[test]
fn parse_diagram_class_classdef_target_lists_preserve_order_and_precedence() {
    let engine = MermaidEngine::new();
    let assignments = "class A:::first\nclass B:::second\n";
    for names in ["first,second", "first, second"] {
        let definition = format!("classDef {names} fill:#ff0000,stroke:#333\n");
        for definition_first in [false, true] {
            let text = if definition_first {
                format!("classDiagram\n{definition}{assignments}")
            } else {
                format!("classDiagram\n{assignments}{definition}")
            };
            let parsed = engine
                .parse_diagram_sync(&text, ParseOptions::strict())
                .unwrap()
                .unwrap();
            let styles = json!(["fill:#ff0000", "stroke:#333"]);
            let definitions = parsed.model["styleClasses"].as_object().unwrap();
            assert_eq!(
                definitions.keys().map(String::as_str).collect::<Vec<_>>(),
                ["first", "second"]
            );
            for name in ["first", "second"] {
                assert_eq!(definitions[name]["styles"], styles);
            }
            let expected = if definition_first { json!([]) } else { styles };
            for id in ["A", "B"] {
                assert_eq!(parsed.model["classes"][id]["styles"], expected);
            }
        }
    }
}

#[test]
fn parse_diagram_class_classdef_target_lists_reject_missing_names() {
    let engine = MermaidEngine::new();
    for definition in [
        "classDef ,first fill:red",
        "classDef first,,second fill:red",
        "classDef first,",
    ] {
        let text = format!("classDiagram\n{definition}\n");
        assert!(
            engine
                .parse_diagram_sync(&text, ParseOptions::strict())
                .is_err(),
            "{definition}"
        );
    }
}

#[test]
fn parse_diagram_class_link_and_click_statements_set_clickable_and_metadata() {
    let engine = MermaidEngine::new();
    let text = r#"classDiagram
class Class1
link Class1 "google.com" "A tooltip" _self
click Class1 href "example.com" "B tooltip" _blank
click Class1 call functionCall(test, test1, test2) "C tooltip"
callback Class1 "otherCall" "D tooltip"
"#;

    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();

    let c = &res.model["classes"]["Class1"];
    assert!(c["cssClasses"].as_str().unwrap().contains("clickable"));
    assert_eq!(c["link"], json!("example.com"));
    assert_eq!(c["linkTarget"], json!("_blank"));
    assert_eq!(c["tooltip"], json!("D tooltip"));
    assert_eq!(c["haveCallback"], json!(true));
    assert_eq!(c["callback"]["function"], json!("otherCall"));
    assert_eq!(c["callbackEffective"], json!(false));
}

#[test]
fn parse_diagram_class_link_lists_match_individual_targets() {
    let engine = MermaidEngine::new();
    let declarations = "classDiagram\nclass A\nclass B\n";
    for (command, href) in [("link", ""), ("click", " href")] {
        for (target, expected_target) in [("", "_blank"), (" _self", "_self")] {
            let tail = format!("{href} \"https://example.com/\" \"Open class\"{target}\n");
            let source = format!("{declarations}{command} `A,Unknown,Later,B`{tail}class Later\n");
            let individual = ["A", "Unknown", "Later", "B"]
                .map(|id| format!("{command} {id}{tail}"))
                .concat();
            let control = format!("{declarations}{individual}class Later\n");
            let parsed = engine
                .parse_diagram_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap();
            let control = engine
                .parse_diagram_sync(&control, ParseOptions::strict())
                .unwrap()
                .unwrap();
            assert_eq!(
                parsed.model["classes"], control.model["classes"],
                "{command}{target}"
            );
            let classes = parsed.model["classes"].as_object().unwrap();
            assert_eq!(classes.len(), 3);
            assert!(!classes.contains_key("Unknown"));
            for id in ["A", "B"] {
                assert_eq!(classes[id]["link"], json!("https://example.com/"));
                assert_eq!(classes[id]["linkTarget"], json!(expected_target));
                assert_eq!(classes[id]["tooltip"], json!("Open class"));
                assert_eq!(classes[id]["cssClasses"], json!("default clickable"));
            }
            assert_eq!(classes["Later"]["link"], json!(null));
            assert_eq!(classes["Later"]["linkTarget"], json!(null));
            assert_eq!(classes["Later"]["tooltip"], json!(null));
            assert_eq!(classes["Later"]["cssClasses"], json!("default"));
        }
    }
}

#[test]
fn parse_diagram_class_callback_lists_match_individual_targets() {
    let declarations = "classDiagram\nclass A~T~\nclass B~U~\n";
    for security_level in ["strict", "loose"] {
        let engine = MermaidEngine::new().with_site_config({
            let mut config = SourcePresentationConfig::empty_object();
            config.set_value("securityLevel", json!(security_level));
            config
        });
        for (command, tail, expected_args) in [
            ("callback", " \"open\" \"Open class\"\n", None),
            ("click", " call open(value) \"Open class\"\n", Some("value")),
        ] {
            let source =
                format!("{declarations}{command} `A~T~,Unknown,Later,B~U~`{tail}class Later\n");
            let individual = ["A~T~", "Unknown", "Later", "B~U~"]
                .map(|id| format!("{command} {id}{tail}"))
                .concat();
            let control = format!("{declarations}{individual}class Later\n");
            let parsed = engine
                .parse_diagram_sync(&source, ParseOptions::strict())
                .unwrap()
                .unwrap();
            let control = engine
                .parse_diagram_sync(&control, ParseOptions::strict())
                .unwrap()
                .unwrap();
            assert_eq!(
                parsed.model["classes"], control.model["classes"],
                "{command} {security_level}"
            );
            let classes = parsed.model["classes"].as_object().unwrap();
            assert_eq!(classes.len(), 3);
            assert!(!classes.contains_key("Unknown"));
            for id in ["A", "B"] {
                assert_eq!(classes[id]["haveCallback"], json!(true));
                assert_eq!(classes[id]["callback"]["function"], json!("open"));
                assert_eq!(
                    classes[id]["callback"]
                        .get("args")
                        .and_then(serde_json::Value::as_str),
                    expected_args
                );
                assert_eq!(
                    classes[id]["callbackEffective"],
                    json!(security_level == "loose")
                );
                assert_eq!(classes[id]["tooltip"], json!("Open class"));
                assert_eq!(classes[id]["cssClasses"], json!("default clickable"));
            }
            assert_eq!(classes["Later"]["haveCallback"], json!(false));
            assert_eq!(classes["Later"]["callback"], json!(null));
            assert_eq!(classes["Later"]["tooltip"], json!(null));
            assert_eq!(classes["Later"]["cssClasses"], json!("default"));
        }
    }
}

#[test]
fn parse_diagram_class_href_sanitizes_javascript_urls_when_not_loose() {
    let engine = MermaidEngine::new();
    let res = block_on(engine.parse_diagram(
        r#"classDiagram
class Class1
click Class1 href "javascript:alert(1)" "A tooltip" _self"#,
        ParseOptions::default(),
    ))
    .unwrap()
    .unwrap();

    let c = &res.model["classes"]["Class1"];
    assert_eq!(c["link"], json!("about:blank"));
    assert_eq!(c["linkTarget"], json!("_self"));
    assert_eq!(c["tooltip"], json!("A tooltip"));
}

#[test]
fn parse_diagram_class_security_level_sandbox_forces_link_target_top() {
    let engine = MermaidEngine::new().with_site_config({
        let mut cfg = SourcePresentationConfig::empty_object();
        cfg.set_value("securityLevel", json!("sandbox"));
        cfg
    });

    let text = r#"classDiagram
class Class1
click Class1 href "google.com" "A tooltip" _self
"#;

    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    let c = &res.model["classes"]["Class1"];
    assert_eq!(c["link"], json!("google.com"));
    assert_eq!(c["linkTarget"], json!("_top"));
}

#[test]
fn parse_diagram_class_security_level_loose_marks_callback_effective() {
    let engine = MermaidEngine::new().with_site_config({
        let mut cfg = SourcePresentationConfig::empty_object();
        cfg.set_value("securityLevel", json!("loose"));
        cfg
    });

    let text = r#"classDiagram
class Class1
click Class1 call functionCall() "A tooltip"
"#;

    let res = block_on(engine.parse_diagram(text, ParseOptions::default()))
        .unwrap()
        .unwrap();
    let c = &res.model["classes"]["Class1"];
    assert_eq!(c["haveCallback"], json!(true));
    assert_eq!(c["callback"]["function"], json!("functionCall"));
    assert!(c["callback"].get("args").is_none());
    assert_eq!(c["callbackEffective"], json!(true));
}

#[test]
fn class_interaction_lists_keep_individual_generic_and_whitespace_reference_spans() {
    for line_ending in ["\n", "\r\n"] {
        for interaction in [
            "click `名𠀀~T~,Missing, B~U~,名𠀀~V~,B~W~` href \"https://example.com/\"",
            "click `名𠀀~T~,Missing, B~U~,名𠀀~V~,B~W~` call open(value)",
            "link `名𠀀~T~,Missing, B~U~,名𠀀~V~,B~W~` \"https://example.com/\"",
            "callback `名𠀀~T~,Missing, B~U~,名𠀀~V~,B~W~` \"open\"",
        ] {
            let source = format!("classDiagram\nclass `名𠀀`~T~\nclass B~U~\n{interaction}\n")
                .replace('\n', line_ending);
            let engine = MermaidEngine::new();
            for known_type in [false, true] {
                let facts = if known_type {
                    engine
                        .parse_editor_semantic_facts_with_type_sync("classDiagram", &source)
                        .unwrap()
                        .unwrap()
                } else {
                    let snapshot = engine
                        .parse_diagram_snapshot_sync(&source)
                        .unwrap()
                        .unwrap();
                    let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                        panic!("Class interaction facts must be available");
                    };
                    facts.clone()
                };
                let targets = facts
                    .symbols
                    .iter()
                    .filter(|symbol| symbol.detail.as_deref() == Some("class interaction target"))
                    .collect::<Vec<_>>();
                assert_eq!(
                    targets
                        .iter()
                        .map(|symbol| symbol.name.as_str())
                        .collect::<Vec<_>>(),
                    ["名𠀀", "Missing", " B", "名𠀀", "B"]
                );
                let target_start = source.find("`名𠀀~T~,Missing").unwrap() + 1;
                let body = "名𠀀~T~,Missing, B~U~,名𠀀~V~,B~W~";
                let mut offset = target_start;
                for (index, (target, authored)) in targets.iter().zip(body.split(',')).enumerate() {
                    assert_eq!(target.role, EditorSemanticRole::Reference);
                    assert_eq!(target.kind, EditorSemanticKind::Class);
                    assert_eq!(
                        target.span,
                        SourceSpan::new(
                            offset - usize::from(index == 0),
                            offset + authored.len() + usize::from(index == targets.len() - 1)
                        )
                    );
                    assert_eq!(
                        &source[target.selection.start..target.selection.end],
                        target.name
                    );
                    offset += authored.len() + 1;
                }
            }
        }
    }
}

#[test]
fn class_interaction_list_suffix_outside_quotes_keeps_each_target() {
    for separator in ["", " \t"] {
        let source = format!(
            "classDiagram\nclass A\nclass B\nclick `A`{separator}~T,B~ href \"https://example.com/\"\n"
        );
        let facts = MermaidEngine::new()
            .parse_editor_semantic_facts_with_type_sync("classDiagram", &source)
            .unwrap()
            .unwrap();
        let targets = facts
            .symbols
            .iter()
            .filter(|symbol| symbol.detail.as_deref() == Some("class interaction target"))
            .collect::<Vec<_>>();
        assert_eq!(targets.len(), 2);
        for (target, name) in targets.iter().zip(["A", "B"]) {
            assert_eq!(target.name, name);
            assert_eq!(&source[target.selection.start..target.selection.end], name);
        }
        assert_eq!(targets[1].selection.start, source.rfind("B~").unwrap());
    }
}

#[test]
fn class_interaction_newlines_restore_accessibility_fields_and_payload_spans() {
    // Mermaid 12.3.0 classDiagram.jison restores INITIAL after interaction strings/args;
    // the following accessibility directives keep their dedicated tokens and values.
    for newline in ["\n", "\r\n"] {
        for interaction in [
            "click User href \"https://example.com\" \"Open user\" _blank",
            "click User call open(userId) \"Open user\"",
            "click User call open()",
        ] {
            let text = format!(
                "classDiagram{newline}class User{newline}{interaction}{newline}accTitle: Class chart{newline}accDescr: Shows class relationships{newline}"
            );
            let engine = MermaidEngine::new();
            let canonical = engine
                .parse_diagram_sync(&text, ParseOptions::strict())
                .unwrap()
                .unwrap();
            assert_eq!(canonical.model["accTitle"], "Class chart", "{interaction}");
            assert_eq!(
                canonical.model["accDescr"], "Shows class relationships",
                "{interaction}",
            );
            let facts = engine
                .parse_editor_semantic_facts_with_type_sync("classDiagram", &text)
                .unwrap()
                .unwrap();
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
            for (value, detail) in [
                ("Class chart", "class accessibility title"),
                (
                    "Shows class relationships",
                    "class accessibility description",
                ),
            ] {
                let symbols = facts
                    .symbols
                    .iter()
                    .filter(|symbol| symbol.detail.as_deref() == Some(detail))
                    .collect::<Vec<_>>();
                assert_eq!(symbols.len(), 1, "{interaction}: {detail}");
                let symbol = symbols[0];
                assert_eq!(symbol.name, value, "{interaction}");
                assert_eq!(symbol.role, EditorSemanticRole::Payload);
                let start = text.find(value).unwrap();
                assert_eq!(symbol.selection.start, start);
                assert_eq!(symbol.selection.end, start + value.len());
                assert_eq!(&text[symbol.selection.start..symbol.selection.end], value);
            }
        }
    }
}

#[test]
fn parse_class_editor_facts_preserve_parser_symbol_spans() {
    let engine = MermaidEngine::new();
    let text = r#"classDiagram
namespace Company {
  class User {
    +login()
    -password: String
  }
}
User <|-- Admin
User: email
class Visible["Visible label"]
User <|-- Admin : manages
Class1 "1" *-- "many" Class02 : contains
<<interface>> User
note for User "Primary user"
note "Floating note"
click User href "https://example.com" "Open user" _blank
click Admin call open(userId) "Open admin"
callback User "refreshUser" "Refresh user"
accTitle: Class chart
accDescr: Shows class relationships
classDef service fill:#eee
class User:::service
cssClass "User,Admin" service
style User fill:#fff
"#;
    let canonical = engine
        .parse_diagram_sync(text, ParseOptions::strict())
        .unwrap()
        .unwrap();
    crate::diagrams::class::reset_class_syntax_construction_count();
    crate::editor::test_work::reset();
    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("classDiagram", text)
        .unwrap()
        .expect("class editor facts");
    assert_eq!(crate::diagrams::class::class_syntax_construction_count(), 1);
    let work = crate::editor::test_work::read();
    assert_eq!(work.collections, 1);
    assert_eq!(work.symbols, facts.symbols.len());
    assert!(work.directives >= facts.directive_prefixes.len());
    assert!(work.directives > 0);
    assert_eq!(work.diagnostics, facts.diagnostics.len());
    assert!(work.expected_syntax >= facts.expected_syntax.len());

    assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);

    let symbol_at = |name: &str, start: usize| {
        facts
            .symbols
            .iter()
            .find(|symbol| symbol.name == name && symbol.selection.start == start)
            .unwrap_or_else(|| panic!("missing symbol {name} at {start}"))
    };
    let symbol_with_detail = |name: &str, detail: &str| {
        facts
            .symbols
            .iter()
            .find(|symbol| symbol.name == name && symbol.detail.as_deref() == Some(detail))
            .unwrap_or_else(|| panic!("missing symbol {name} with detail {detail}"))
    };

    let company_start = text.find("Company").unwrap();
    assert_eq!(
        symbol_at("Company", company_start).selection.end,
        company_start + "Company".len()
    );

    let user_start = text.find("User {").unwrap();
    assert_eq!(
        symbol_at("User", user_start).selection.end,
        user_start + "User".len()
    );

    let login_start = text.find("+login()").unwrap();
    let login = symbol_at("+login()", login_start);
    assert_eq!(login.role, EditorSemanticRole::Outline);
    assert_eq!(login.detail.as_deref(), Some("class member"));

    let password_start = text.find("-password: String").unwrap();
    let password = symbol_at("-password: String", password_start);
    assert_eq!(password.role, EditorSemanticRole::Outline);
    assert_eq!(password.detail.as_deref(), Some("class member"));

    let admin_start = text.find("Admin").unwrap();
    assert_eq!(
        symbol_at("Admin", admin_start).selection.end,
        admin_start + "Admin".len()
    );

    let email_start = text.find("email").unwrap();
    let email = symbol_at("email", email_start);
    assert_eq!(email.role, EditorSemanticRole::Outline);
    assert_eq!(email.detail.as_deref(), Some("class member"));

    let display_label = facts
        .symbols
        .iter()
        .find(|symbol| {
            symbol.name == "Visible label"
                && symbol.detail.as_deref() == Some("class display label")
        })
        .unwrap();
    assert_eq!(display_label.role, EditorSemanticRole::Payload);

    let annotation_start = text.find("interface").unwrap();
    let annotation = symbol_at("interface", annotation_start);
    assert_eq!(annotation.role, EditorSemanticRole::Payload);
    assert_eq!(annotation.detail.as_deref(), Some("class annotation"));

    let relation_label = facts
        .symbols
        .iter()
        .find(|symbol| {
            symbol.name == "manages" && symbol.detail.as_deref() == Some("class relation label")
        })
        .unwrap();
    assert_eq!(relation_label.role, EditorSemanticRole::Payload);

    let left_multiplicity = facts
        .symbols
        .iter()
        .find(|symbol| {
            symbol.name == "1" && symbol.detail.as_deref() == Some("class relation multiplicity")
        })
        .unwrap();
    assert_eq!(left_multiplicity.role, EditorSemanticRole::Payload);

    let right_multiplicity = facts
        .symbols
        .iter()
        .find(|symbol| {
            symbol.name == "many" && symbol.detail.as_deref() == Some("class relation multiplicity")
        })
        .unwrap();
    assert_eq!(right_multiplicity.role, EditorSemanticRole::Payload);

    let note_for = facts
        .symbols
        .iter()
        .find(|symbol| {
            symbol.name == "Primary user" && symbol.detail.as_deref() == Some("class note")
        })
        .unwrap();
    assert_eq!(note_for.role, EditorSemanticRole::Payload);

    let floating_note = facts
        .symbols
        .iter()
        .find(|symbol| {
            symbol.name == "Floating note" && symbol.detail.as_deref() == Some("class note")
        })
        .unwrap();
    assert_eq!(floating_note.role, EditorSemanticRole::Payload);

    let acc_title = facts
        .symbols
        .iter()
        .find(|symbol| {
            symbol.name == "Class chart"
                && symbol.detail.as_deref() == Some("class accessibility title")
        })
        .unwrap();
    assert_eq!(acc_title.role, EditorSemanticRole::Payload);

    let acc_descr = facts
        .symbols
        .iter()
        .find(|symbol| {
            symbol.name == "Shows class relationships"
                && symbol.detail.as_deref() == Some("class accessibility description")
        })
        .unwrap();
    assert_eq!(acc_descr.role, EditorSemanticRole::Payload);

    let href_start = text.find("https://example.com").unwrap();
    let href = symbol_at("https://example.com", href_start);
    assert_eq!(href.role, EditorSemanticRole::Payload);
    assert_eq!(href.detail.as_deref(), Some("class interaction string"));

    let href_target_start = text.find("click User href").unwrap() + "click ".len();
    let href_target = symbol_at("User", href_target_start);
    assert_eq!(href_target.role, EditorSemanticRole::Reference);
    assert_eq!(href_target.kind, EditorSemanticKind::Class);
    assert_eq!(
        href_target.detail.as_deref(),
        Some("class interaction target")
    );

    let tooltip_start = text.find("Open user").unwrap();
    let tooltip = symbol_at("Open user", tooltip_start);
    assert_eq!(tooltip.role, EditorSemanticRole::Payload);
    assert_eq!(tooltip.detail.as_deref(), Some("class interaction string"));

    let target_start = text.find("_blank").unwrap();
    let target = symbol_at("_blank", target_start);
    assert_eq!(target.role, EditorSemanticRole::Payload);
    assert_eq!(target.detail.as_deref(), Some("class link target"));

    let service_start = text.find("service").unwrap();
    let service = symbol_at("service", service_start);
    assert_eq!(service.selection.end, service_start + "service".len());
    assert_eq!(service.role, EditorSemanticRole::ClassDefinition);
    assert_eq!(service.detail.as_deref(), Some("class definition"));

    let class_def_style = symbol_with_detail("fill:#eee", "class definition style");
    assert_eq!(class_def_style.role, EditorSemanticRole::Payload);

    let inline_service_start = text.find(":::service").unwrap() + ":::".len();
    let inline_service = symbol_at("service", inline_service_start);
    assert_eq!(inline_service.role, EditorSemanticRole::Payload);
    assert_eq!(inline_service.detail.as_deref(), Some("class inline class"));

    let css_admin = symbol_with_detail("Admin", "class css target");
    assert_eq!(css_admin.role, EditorSemanticRole::Reference);

    let css_service = symbol_with_detail("service", "class css reference");
    assert_eq!(css_service.role, EditorSemanticRole::Payload);

    let class_style = symbol_with_detail("fill:#fff", "class style");
    assert_eq!(class_style.role, EditorSemanticRole::Payload);

    let callback = symbol_with_detail("open", "class callback");
    assert_eq!(callback.role, EditorSemanticRole::Payload);
    assert_eq!(callback.kind, EditorSemanticKind::Function);

    let call_target_start = text.find("click Admin call").unwrap() + "click ".len();
    let call_target = symbol_at("Admin", call_target_start);
    assert_eq!(call_target.kind, EditorSemanticKind::Class);

    let callback_args = symbol_with_detail("userId", "class callback args");
    assert_eq!(callback_args.role, EditorSemanticRole::Payload);

    let callback_target_start = text.find("callback User").unwrap() + "callback ".len();
    let callback_target = symbol_at("User", callback_target_start);
    assert_eq!(callback_target.kind, EditorSemanticKind::Class);

    let callback_statement = symbol_with_detail("refreshUser", "class callback");
    assert_eq!(callback_statement.role, EditorSemanticRole::Payload);
    assert_eq!(callback_statement.kind, EditorSemanticKind::Function);

    assert!(facts.directive_prefixes.iter().any(|p| p == "click"));
    assert!(facts.directive_prefixes.iter().any(|p| p == "callback"));
    assert!(facts.directive_prefixes.iter().any(|p| p == "cssClass"));
    assert!(facts.directive_prefixes.iter().any(|p| p == "style"));
    assert!(facts.directive_prefixes.iter().any(|p| p == "classDef"));

    for known_type in [false, true] {
        crate::diagrams::class::reset_class_syntax_construction_count();
        crate::editor::test_work::reset();
        let snapshot = if known_type {
            engine.parse_diagram_snapshot_with_type_sync("classDiagram", text)
        } else {
            engine.parse_diagram_snapshot_sync(text)
        }
        .unwrap()
        .unwrap();
        assert_eq!(crate::diagrams::class::class_syntax_construction_count(), 1);
        assert_eq!(crate::editor::test_work::read().collections, 1);
        assert_eq!(snapshot.outcome().parsed_model().unwrap(), &canonical.model);
        let ParsedEditorFacts::Available(snapshot_facts) = snapshot.editor_facts() else {
            panic!("class snapshot must retain editor facts");
        };
        assert_eq!(snapshot_facts, &facts);
    }
}

#[test]
fn parse_class_editor_facts_keep_mixed_numeric_target_lists_raw_and_precise() {
    for targets in ["1~U~,Peer~V~", "Peer~V~,1~U~"] {
        let text = format!(
            "classDiagram\nclass 1~T~\nclass classId-1~T~\nclass Peer~T~\ncssClass \"{targets}\" accent\nlink `{targets}` \"https://example.com\"\ncallback `{targets}` \"open\"\n"
        );
        let facts = MermaidEngine::new()
            .parse_editor_semantic_facts_with_type_sync("classDiagram", &text)
            .unwrap()
            .unwrap();
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
        for (detail, count) in [("class css target", 2), ("class interaction target", 4)] {
            let symbols = facts
                .symbols
                .iter()
                .filter(|symbol| symbol.detail.as_deref() == Some(detail))
                .collect::<Vec<_>>();
            assert_eq!(symbols.len(), count, "{targets}: {detail}");
            for symbol in symbols {
                assert!(matches!(symbol.name.as_str(), "1" | "Peer"));
                assert_eq!(symbol.role, EditorSemanticRole::Reference);
                assert_eq!(
                    &text[symbol.selection.start..symbol.selection.end],
                    symbol.name,
                    "{targets}: {detail}"
                );
                assert!(symbol.span.start <= symbol.selection.start);
                assert!(symbol.selection.end <= symbol.span.end);
            }
        }
        assert_eq!(
            facts
                .symbols
                .iter()
                .filter(|symbol| symbol.name == "classId-1")
                .count(),
            1
        );
    }
}

#[test]
fn parse_class_editor_facts_preserve_quoted_numeric_class_selection_spans() {
    let engine = MermaidEngine::new();
    let text =
        "classDiagram\nclass `123`\n`123` --> Service\nclick `123` href \"https://example.com\"\n";
    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("classDiagram", text)
        .unwrap()
        .expect("class editor facts");

    let numeric_starts = text
        .match_indices("123")
        .map(|(start, _)| start)
        .collect::<Vec<_>>();
    assert_eq!(numeric_starts.len(), 3);

    let symbol_at = |detail: &str, start: usize| {
        facts
            .symbols
            .iter()
            .find(|symbol| {
                symbol.name == "123"
                    && symbol.detail.as_deref() == Some(detail)
                    && symbol.selection.start == start
            })
            .unwrap_or_else(|| panic!("missing quoted numeric class symbol {detail} at {start}"))
    };

    for (detail, start) in [
        ("class", numeric_starts[0]),
        ("class relation target", numeric_starts[1]),
        ("class interaction target", numeric_starts[2]),
    ] {
        let symbol = symbol_at(detail, start);
        assert_eq!(symbol.selection.end, start + "123".len());
    }

    assert_eq!(
        symbol_at("class", numeric_starts[0]).role,
        EditorSemanticRole::Entity
    );
    assert_eq!(
        symbol_at("class relation target", numeric_starts[1]).role,
        EditorSemanticRole::Reference
    );
    assert_eq!(
        symbol_at("class interaction target", numeric_starts[2]).role,
        EditorSemanticRole::Reference
    );
}

#[test]
fn parse_class_editor_facts_keep_compact_members_generics_and_classdef_target_spans() {
    let text = concat!(
        "classDiagram\n",
        "class `Car`~T~ { +drive() }\n",
        "class `123`~类型~\n",
        "A*--B\n",
        "classDef first,second fill:red\n",
    );
    let facts = MermaidEngine::new()
        .parse_editor_semantic_facts_with_type_sync("classDiagram", text)
        .unwrap()
        .unwrap();
    assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
    for (name, detail, source) in [
        ("Car", "class", "Car"),
        ("+drive()", "class member", "+drive()"),
        ("A", "class relation target", "A"),
        ("B", "class relation target", "B"),
        ("first", "class definition", "first"),
        ("second", "class definition", "second"),
    ] {
        let symbol = facts
            .symbols
            .iter()
            .find(|symbol| symbol.name == name && symbol.detail.as_deref() == Some(detail))
            .unwrap_or_else(|| panic!("missing Class symbol {name}: {detail}"));
        let start = text.find(source).unwrap();
        assert_eq!(
            symbol.selection,
            SourceSpan::new(start, start + source.len())
        );
        if detail == "class definition" {
            assert_eq!(symbol.role, EditorSemanticRole::ClassDefinition);
        }
    }
    let numeric = facts
        .symbols
        .iter()
        .find(|symbol| symbol.name == "123")
        .expect("numeric quoted generic class");
    let start = text.find("123").unwrap();
    assert_eq!(numeric.selection, SourceSpan::new(start, start + 3));
    assert_eq!(&text[numeric.span.start..numeric.span.end], "`123`~类型~");
}

#[test]
fn parse_class_editor_facts_canonicalize_generic_class_identity_and_selections() {
    let engine = MermaidEngine::new();
    for (declaration, reference, id, spelling) in [
        ("`Car`~T~", "Car", "Car", "Car"),
        ("Car~T~", "`Car`", "Car", "Car"),
        ("`Car` \t~T~", "Car~U~", "Car", "Car"),
        ("Car \t~T~", "Car ~U~", "Car", "Car"),
        ("`Car~T~`", "`Car` ~U~", "Car", "Car"),
        ("`123`~类型~", "123", "123", "123"),
        ("123~类型~", "`123`", "123", "123"),
        ("123 ~类型~", "`123` ~U~", "123", "123"),
        ("`名𠀀`~类型~", "`名𠀀`", "名𠀀", "名𠀀"),
        (
            "`classId-Car`~T~",
            "classId-Car",
            "classId-Car",
            "classId-Car",
        ),
    ] {
        let text = format!("classDiagram\nclass {declaration}\n{reference} --> Wheel\n");
        let snapshot = engine
            .parse_diagram_snapshot_with_type_sync("classDiagram", &text)
            .unwrap()
            .unwrap();
        let model = snapshot.outcome().parsed_model().unwrap();
        assert!(model["classes"][id].is_object(), "{declaration}");
        let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
            panic!("generic Class snapshot must retain editor facts");
        };
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
        let occurrences = facts
            .symbols
            .iter()
            .filter(|symbol| symbol.kind == EditorSemanticKind::Class && symbol.name == id)
            .collect::<Vec<_>>();
        assert_eq!(occurrences.len(), 2, "{declaration}: {reference}");
        assert_eq!(occurrences[0].role, EditorSemanticRole::Entity);
        assert_eq!(occurrences[1].role, EditorSemanticRole::Reference);
        assert_eq!(
            &text[occurrences[0].span.start..occurrences[0].span.end],
            declaration
        );
        assert_eq!(
            &text[occurrences[1].span.start..occurrences[1].span.end],
            reference
        );
        for symbol in occurrences {
            assert_eq!(
                &text[symbol.selection.start..symbol.selection.end],
                spelling
            );
            assert!(symbol.span.start <= symbol.selection.start);
            assert!(symbol.selection.end <= symbol.span.end);
        }
    }
}

#[test]
fn parse_class_editor_facts_canonicalize_generic_class_targets() {
    let text = concat!(
        "classDiagram\n",
        "class Car~T~\n",
        "Car~U~: +drive()\n",
        "<<interface>> Car~U~\n",
        "note for Car \"note\"\n",
        "style Car fill:red\n",
        "click Car~U~ href \"https://example.com\"\n",
        "cssClass \"Car~U~, Other\" hot\n",
    );
    let facts = MermaidEngine::new()
        .parse_editor_semantic_facts_with_type_sync("classDiagram", text)
        .unwrap()
        .unwrap();
    assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
    for (detail, authored) in [
        ("class member owner", "Car~U~"),
        ("class annotation target", "Car~U~"),
        ("class note target", "Car"),
        ("class style target", "Car"),
        ("class interaction target", "Car~U~"),
        ("class css target", "Car~U~"),
    ] {
        let symbol = facts
            .symbols
            .iter()
            .find(|symbol| symbol.name == "Car" && symbol.detail.as_deref() == Some(detail))
            .unwrap_or_else(|| panic!("missing generic Class reference: {detail}"));
        assert_eq!(symbol.role, EditorSemanticRole::Reference);
        assert_eq!(&text[symbol.span.start..symbol.span.end], authored);
        assert_eq!(&text[symbol.selection.start..symbol.selection.end], "Car");
    }
}

#[test]
fn class_relations_create_only_the_first_implicit_entity_occurrence() {
    let engine = MermaidEngine::new();
    let text = concat!(
        "classDiagram\n",
        "Future --> Known\n",
        "Known --> Future\n",
        "class Known\n",
        "style Later fill:#fff\n",
        "class Later\n",
        "<<interface>> Annotated\n",
        "class Annotated\n",
    );
    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("classDiagram", text)
        .unwrap()
        .expect("class editor facts");

    let roles = |name: &str| {
        facts
            .symbols
            .iter()
            .filter(|symbol| symbol.name == name)
            .map(|symbol| symbol.role)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        roles("Future"),
        [EditorSemanticRole::Entity, EditorSemanticRole::Reference]
    );
    assert_eq!(
        roles("Known"),
        [
            EditorSemanticRole::Entity,
            EditorSemanticRole::Reference,
            EditorSemanticRole::Entity,
        ]
    );
    assert_eq!(
        roles("Later"),
        [EditorSemanticRole::Reference, EditorSemanticRole::Entity]
    );
    assert_eq!(
        roles("Annotated"),
        [EditorSemanticRole::Entity, EditorSemanticRole::Entity]
    );
}

#[test]
fn parse_class_editor_facts_preserve_click_call_callback_name_span_after_whitespace() {
    let engine = MermaidEngine::new();
    let text = "classDiagram\nclass Admin\nclick Admin call   open(userId)\n";
    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("classDiagram", text)
        .unwrap()
        .expect("class editor facts");

    let open_start = text.find("open").unwrap();
    let callback = facts
        .symbols
        .iter()
        .find(|symbol| symbol.name == "open" && symbol.detail.as_deref() == Some("class callback"))
        .expect("callback symbol");

    assert_eq!(callback.selection.start, open_start);
    assert_eq!(callback.selection.end, open_start + "open".len());
}

#[test]
fn parse_class_editor_facts_preserve_every_crlf_unicode_occurrence_and_payload_span() {
    let engine = MermaidEngine::new();
    let text = concat!(
        "---\r\n",
        "config:\r\n",
        "  theme: dark\r\n",
        "---\r\n",
        "%%{init: {\"theme\": \"default\"}}%%\r\n",
        "classDiagram-v2\r\n",
        "namespace 公司 {\r\n",
        "  class 顧客[\"客戶\"] {\r\n",
        "    +查詢(id) 結果\r\n",
        "  }\r\n",
        "  note for 顧客 \"Primary customer\"\r\n",
        "}\r\n",
        "class 訂單\r\n",
        "顧客 \"1\" *-- \"many\" 訂單 : owns 群組\r\n",
        "note for 顧客 \"Important account\"\r\n",
        "click 顧客 call open(customerId) \"Open customer\"\r\n",
        "cssClass \"顧客,訂單\" service\r\n",
        "style 顧客 fill:#fff\r\n",
    );
    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("classDiagram", text)
        .unwrap()
        .expect("Class editor facts");

    assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
    for name in ["顧客", "訂單"] {
        let expected = text
            .match_indices(name)
            .map(|(start, value)| SourceSpan::new(start, start + value.len()))
            .collect::<Vec<_>>();
        let actual = facts
            .symbols
            .iter()
            .filter(|symbol| symbol.name == name)
            .map(|symbol| symbol.selection)
            .collect::<Vec<_>>();
        assert_eq!(
            actual, expected,
            "Class lost or collapsed a {name:?} occurrence"
        );
    }

    let payload = |name: &str, detail: &str| {
        facts
            .symbols
            .iter()
            .find(|symbol| symbol.name == name && symbol.detail.as_deref() == Some(detail))
            .unwrap_or_else(|| panic!("missing Class payload {name:?} ({detail})"))
    };
    for (name, detail) in [
        ("客戶", "class display label"),
        ("1", "class relation multiplicity"),
        ("many", "class relation multiplicity"),
        ("owns 群組", "class relation label"),
        ("Primary customer", "class note"),
        ("Important account", "class note"),
        ("open", "class callback"),
        ("customerId", "class callback args"),
        ("Open customer", "class interaction string"),
        ("fill:#fff", "class style"),
    ] {
        let start = text.find(name).unwrap();
        assert_eq!(
            payload(name, detail).selection,
            SourceSpan::new(start, start + name.len()),
            "Class payload {name:?} span drifted"
        );
    }
}

#[test]
fn parse_class_editor_facts_record_expected_class_name_spans() {
    let engine = MermaidEngine::new();
    let text = "classDiagram\nclassDef service fill:#eee\n";
    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("classDiagram", text)
        .unwrap()
        .expect("class editor facts");

    assert!(facts.expected_syntax.iter().any(|expected| {
        expected.kind == EditorExpectedSyntaxKind::ClassName
            && expected.span.start == text.find("service").unwrap()
    }));
}

#[test]
fn parse_class_editor_facts_do_not_consume_classdef_names_across_line_endings() {
    let engine = MermaidEngine::new();

    for line_ending in ["\n", "\r", "\r\n"] {
        for definition in ["classDef", "classDef first,"] {
            let text = ["classDiagram", definition, "User <|-- Admin", ""].join(line_ending);
            let facts = engine
                .parse_editor_semantic_facts_with_type_sync("classDiagram", &text)
                .unwrap()
                .expect("class editor facts");

            assert!(
                !facts.symbols.iter().any(|symbol| {
                    symbol.name == "User" && symbol.role == EditorSemanticRole::ClassDefinition
                }),
                "{definition} consumed the next physical line for {line_ending:?}"
            );
        }
    }
}

#[test]
fn parse_class_editor_facts_recovers_from_incomplete_input() {
    let engine = MermaidEngine::new();
    let text = "classDiagram\nclass User\nUser <|--";
    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("classDiagram", text)
        .unwrap()
        .expect("class editor facts");

    assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
    assert!(facts.symbols.iter().any(|symbol| symbol.name == "User"));
}

#[test]
fn parse_class_editor_facts_stop_after_non_advancing_lexer_error() {
    let engine = MermaidEngine::new();
    let text = "classDiagram\nclass User {\n  +name";
    crate::diagrams::class::reset_class_syntax_construction_count();
    crate::editor::test_work::reset();
    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("classDiagram", text)
        .unwrap()
        .expect("class editor facts");
    let work = crate::editor::test_work::read();
    assert_eq!(work.collections, 1);
    assert!(work.symbols > 0);
    assert!(work.diagnostics > 0);

    assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
    assert!(facts.symbols.iter().any(|symbol| symbol.name == "User"));
    assert_eq!(crate::diagrams::class::class_syntax_construction_count(), 1);
    assert!(facts.diagnostics.iter().any(|diagnostic| {
        diagnostic.kind == EditorSemanticDiagnosticKind::ParserRecovery
            && diagnostic.span == Some(SourceSpan::new(text.len(), text.len()))
    }));

    crate::diagrams::class::reset_class_syntax_construction_count();
    crate::editor::test_work::reset();
    let error = engine
        .parse_diagram_sync(text, ParseOptions::strict())
        .expect_err("strict Class parsing rejects an unterminated body");
    assert_eq!(crate::diagrams::class::class_syntax_construction_count(), 1);
    assert_eq!(
        crate::editor::test_work::read(),
        crate::editor::test_work::Counts::default()
    );
    let Error::DiagramParse { diagnostic, .. } = error else {
        panic!("unterminated Class body returned a non-parse error");
    };
    assert_eq!(
        diagnostic.span(),
        Some(SourceSpan::new(text.len(), text.len()))
    );
    assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);

    crate::diagrams::class::reset_class_syntax_construction_count();
    crate::editor::test_work::reset();
    let snapshot = engine
        .parse_diagram_snapshot_with_type_sync("classDiagram", text)
        .unwrap()
        .unwrap();
    assert_eq!(crate::diagrams::class::class_syntax_construction_count(), 1);
    assert_eq!(crate::editor::test_work::read().collections, 1);
    assert!(matches!(snapshot.outcome(), DiagramParseOutcome::Failed(_)));
    let ParsedEditorFacts::Available(snapshot_facts) = snapshot.editor_facts() else {
        panic!("class recovery snapshot must retain editor facts");
    };
    assert_eq!(snapshot_facts, &facts);
}

#[test]
fn parse_class_editor_facts_continue_after_advancing_lexer_error() {
    let engine = MermaidEngine::new();
    let text = "classDiagram\n\"\nclass User\n";
    let invalid_start = text.find('"').unwrap();
    crate::diagrams::class::reset_class_syntax_construction_count();
    crate::editor::test_work::reset();
    let facts = engine
        .parse_editor_semantic_facts_with_type_sync("classDiagram", text)
        .unwrap()
        .expect("class editor facts");
    let work = crate::editor::test_work::read();
    assert_eq!(work.collections, 1);
    assert!(work.symbols > 0);
    assert!(work.diagnostics > 0);

    assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
    assert!(facts.symbols.iter().any(|symbol| symbol.name == "User"));
    assert_eq!(crate::diagrams::class::class_syntax_construction_count(), 1);
    assert!(facts.diagnostics.iter().any(|diagnostic| {
        diagnostic.kind == EditorSemanticDiagnosticKind::ParserRecovery
            && diagnostic.span == Some(SourceSpan::new(invalid_start, invalid_start + 1))
    }));

    crate::diagrams::class::reset_class_syntax_construction_count();
    crate::editor::test_work::reset();
    let error = engine
        .parse_diagram_sync(text, ParseOptions::strict())
        .expect_err("strict Class parsing rejects an unterminated string");
    assert_eq!(crate::diagrams::class::class_syntax_construction_count(), 1);
    assert_eq!(
        crate::editor::test_work::read(),
        crate::editor::test_work::Counts::default()
    );
    let Error::DiagramParse { diagnostic, .. } = error else {
        panic!("unterminated Class string returned a non-parse error");
    };
    assert_eq!(
        diagnostic.span(),
        Some(SourceSpan::new(invalid_start, invalid_start + 1))
    );
    assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);

    crate::diagrams::class::reset_class_syntax_construction_count();
    crate::editor::test_work::reset();
    let snapshot = engine
        .parse_diagram_snapshot_with_type_sync("classDiagram", text)
        .unwrap()
        .unwrap();
    assert_eq!(crate::diagrams::class::class_syntax_construction_count(), 1);
    assert_eq!(crate::editor::test_work::read().collections, 1);
    assert!(matches!(snapshot.outcome(), DiagramParseOutcome::Failed(_)));
    let ParsedEditorFacts::Available(snapshot_facts) = snapshot.editor_facts() else {
        panic!("class recovery snapshot must retain editor facts");
    };
    assert_eq!(snapshot_facts, &facts);
}
