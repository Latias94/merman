use crate::*;
use serde_json::json;

#[test]
fn parse_railroad_ir_rules_and_editor_facts() {
    let engine = Engine::new();
    let text = r#"railroad-beta
title "Simple Grammar"
rule = sequence(terminal("a"), nonterminal("other"), zeroOrMore(special("anything"))) ;
other = terminal("b") ;
"#;

    let parsed = engine
        .parse_diagram_with_editor_facts_sync(text, ParseOptions::strict())
        .unwrap()
        .expect("railroad parses");

    assert_eq!(parsed.diagram.meta.diagram_type, "railroad");
    assert_eq!(parsed.diagram.model["type"], json!("railroad"));
    assert_eq!(parsed.diagram.model["title"], json!("Simple Grammar"));
    assert_eq!(parsed.diagram.model["rules"][0]["name"], json!("rule"));
    assert_eq!(
        parsed.diagram.model["rules"][0]["definition"],
        json!({
            "type": "sequence",
            "elements": [
                { "type": "terminal", "value": "a" },
                { "type": "nonterminal", "name": "other" },
                {
                    "type": "repetition",
                    "element": { "type": "special", "text": "anything" },
                    "min": 0,
                    "max": null
                }
            ]
        })
    );

    let ParsedEditorFacts::Available(facts) = parsed.editor_facts else {
        panic!("railroad should expose editor facts");
    };
    assert!(
        facts
            .directive_prefixes
            .iter()
            .any(|prefix| prefix == "title")
    );
    assert!(facts.symbols.iter().any(|symbol| {
        symbol.name == "rule"
            && symbol.detail.as_deref() == Some("railroad rule")
            && symbol.selection
                == SourceSpan::new(
                    text.find("rule =").unwrap(),
                    text.find("rule =").unwrap() + 4,
                )
    }));
    assert!(facts.symbols.iter().any(|symbol| {
        symbol.name == "other" && symbol.detail.as_deref() == Some("railroad nonterminal reference")
    }));
    assert!(facts.symbols.iter().any(|symbol| {
        symbol.name == "a" && symbol.detail.as_deref() == Some("railroad terminal")
    }));
}

#[test]
fn parse_railroad_ebnf_choice_optional_repetition_and_special_sequence() {
    let engine = Engine::new();
    let parsed = engine
        .parse_diagram_sync(
            r#"railroad-ebnf-beta
rule ::= "a" | [ other ] , { ? special ? } ;
"#,
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("railroad ebnf parses");

    assert_eq!(parsed.meta.diagram_type, "railroadEbnf");
    assert_eq!(parsed.model["rules"][0]["name"], json!("rule"));
    assert_eq!(
        parsed.model["rules"][0]["definition"],
        json!({
            "type": "choice",
            "alternatives": [
                { "type": "terminal", "value": "a" },
                {
                    "type": "sequence",
                    "elements": [
                        { "type": "optional", "element": { "type": "nonterminal", "name": "other" } },
                        {
                            "type": "repetition",
                            "element": { "type": "special", "text": "special" },
                            "min": 0,
                            "max": null
                        }
                    ]
                }
            ]
        })
    );
}

#[test]
fn parse_railroad_abnf_repetition_optional_numval_and_comments() {
    let engine = Engine::new();
    let parsed = engine
        .parse_diagram_sync(
            r#"railroad-abnf-beta
; comment
rule = 1*2"hello" / [ other-rule ] / %x41 ;
"#,
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("railroad abnf parses");

    assert_eq!(parsed.meta.diagram_type, "railroadAbnf");
    assert_eq!(
        parsed.model["rules"][0]["definition"],
        json!({
            "type": "choice",
            "alternatives": [
                {
                    "type": "repetition",
                    "element": { "type": "terminal", "value": "hello" },
                    "min": 1,
                    "max": 2
                },
                {
                    "type": "optional",
                    "element": { "type": "nonterminal", "name": "other-rule" }
                },
                { "type": "terminal", "value": "%x41" }
            ]
        })
    );
}

#[test]
fn parse_railroad_peg_prefix_suffix_any_and_editor_facts() {
    let engine = Engine::new();
    let text = r#"railroad-peg-beta
rule <- &"a" !"b" . other? ;
"#;
    let parsed = engine
        .parse_diagram_with_editor_facts_sync(text, ParseOptions::strict())
        .unwrap()
        .expect("railroad peg parses");

    assert_eq!(parsed.diagram.meta.diagram_type, "railroadPeg");
    assert_eq!(
        parsed.diagram.model["rules"][0]["definition"],
        json!({
            "type": "sequence",
            "elements": [
                { "type": "special", "text": "&\"a\"" },
                { "type": "special", "text": "!\"b\"" },
                { "type": "special", "text": "." },
                { "type": "optional", "element": { "type": "nonterminal", "name": "other" } }
            ]
        })
    );

    let ParsedEditorFacts::Available(facts) = parsed.editor_facts else {
        panic!("railroad PEG should expose editor facts");
    };
    assert!(facts.symbols.iter().any(|symbol| {
        symbol.name == "other"
            && symbol.detail.as_deref() == Some("railroad peg nonterminal reference")
    }));
}

#[test]
fn parse_railroad_variants_expose_typed_render_models() {
    let engine = Engine::new();
    for (diagram_type, source) in [
        ("railroad", "railroad-beta\nrule = terminal(\"a\") ;\n"),
        ("railroadEbnf", "railroad-ebnf-beta\nrule = \"a\" ;\n"),
        ("railroadAbnf", "railroad-abnf-beta\nrule = \"a\" ;\n"),
        ("railroadPeg", "railroad-peg-beta\nrule <- \"a\" ;\n"),
    ] {
        let parsed = engine
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap();

        assert_eq!(parsed.meta.diagram_type, diagram_type);
        assert!(parsed.model.supports_diagram_type(diagram_type));
        let RenderSemanticModel::Railroad(model) = parsed.model else {
            panic!("expected railroad render model for {diagram_type}");
        };
        assert_eq!(model.rules.len(), 1);
        assert_eq!(model.rules[0].name, "rule");
    }
}
