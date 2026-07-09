use crate::*;
use serde_json::json;

#[test]
fn parse_cynefin_domains_transitions_and_editor_facts() {
    let engine = Engine::new();
    let text = r#"cynefin-beta
  title Team Practices
  accTitle: Cynefin for team practices
  accDescr: A diagram showing the five Cynefin domains
  complex
    "Retrospectives"
    "Pair programming"
  complicated
    "Code review"
  complex --> complicated : "Pattern emerges"
  complicated --> complicated : "Self-loop"
"#;

    let parsed = engine
        .parse_diagram_with_editor_facts_sync(text, ParseOptions::strict())
        .unwrap()
        .expect("cynefin parses");

    assert_eq!(parsed.diagram.meta.diagram_type, "cynefin");
    assert_eq!(
        parsed.diagram.model,
        json!({
            "type": "cynefin",
            "title": "Team Practices",
            "accTitle": "Cynefin for team practices",
            "accDescr": "A diagram showing the five Cynefin domains",
            "domains": [
                {
                    "name": "complex",
                    "items": [
                        { "label": "Retrospectives" },
                        { "label": "Pair programming" }
                    ]
                },
                {
                    "name": "complicated",
                    "items": [
                        { "label": "Code review" }
                    ]
                }
            ],
            "transitions": [
                {
                    "from": "complex",
                    "to": "complicated",
                    "label": "Pattern emerges"
                }
            ]
        })
    );

    let ParsedEditorFacts::Available(facts) = parsed.editor_facts else {
        panic!("cynefin should expose editor facts");
    };
    assert!(
        facts
            .directive_prefixes
            .iter()
            .any(|prefix| prefix == "title")
    );
    assert!(facts.symbols.iter().any(|symbol| {
        symbol.name == "complex"
            && symbol.detail.as_deref() == Some("cynefin domain")
            && symbol.selection
                == SourceSpan::new(
                    text.find("complex").expect("complex domain"),
                    text.find("complex").expect("complex domain") + "complex".len(),
                )
    }));
    assert!(facts.symbols.iter().any(|symbol| {
        symbol.name == "Retrospectives" && symbol.detail.as_deref() == Some("cynefin domain item")
    }));
    assert!(facts.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("self-loop transition on domain \"complicated\" is skipped")
    }));
}

#[test]
fn parse_cynefin_accepts_colon_header_comments_single_quotes_and_escapes() {
    let engine = Engine::new();
    let text = "cynefin-beta:\n  %% comment\n  clear\n    'Known \\'good\\' practice %% kept'\n  clear --> chaotic %% transition comment\n";
    let parsed = engine
        .parse_diagram_sync(text, ParseOptions::strict())
        .unwrap()
        .expect("cynefin parses");

    assert_eq!(parsed.meta.diagram_type, "cynefin");
    assert_eq!(parsed.model["domains"][0]["name"], json!("clear"));
    assert_eq!(
        parsed.model["domains"][0]["items"][0]["label"],
        json!("Known 'good' practice %% kept")
    );
    assert_eq!(
        parsed.model["transitions"][0],
        json!({
            "from": "clear",
            "to": "chaotic"
        })
    );
}

#[test]
fn parse_cynefin_duplicate_domain_replaces_prior_items_like_upstream_map_set() {
    let engine = Engine::new();
    let parsed = engine
        .parse_diagram_sync(
            "cynefin-beta\n  complex\n    \"Old\"\n  complex\n    \"New\"\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .expect("cynefin parses");

    assert_eq!(
        parsed.model["domains"],
        json!([
            {
                "name": "complex",
                "items": [
                    { "label": "New" }
                ]
            }
        ])
    );
}

#[test]
fn parse_cynefin_render_model_uses_typed_model() {
    let engine = Engine::new();
    let parsed = engine
        .parse_diagram_for_render_model_sync("cynefin-beta\n  complex\n", ParseOptions::strict())
        .unwrap()
        .expect("cynefin render model parses");

    assert_eq!(parsed.meta.diagram_type, "cynefin");
    assert_eq!(parsed.model.kind(), "cynefin");
    assert!(parsed.model.supports_diagram_type("cynefin"));
    let RenderSemanticModel::Cynefin(model) = parsed.model else {
        panic!("expected typed cynefin render model");
    };
    assert_eq!(model.domains[0].name, "complex");
}
