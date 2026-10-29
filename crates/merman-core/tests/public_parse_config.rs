#![cfg(feature = "diagram-flowchart")]
#![cfg(feature = "compat-json")]

use merman_core::presentation::PresentationConfig;
use merman_core::{MermaidEngine, ParseOptions};
use serde_json::json;

#[test]
fn public_parse_config_contains_only_admitted_source_overrides() {
    let engine = MermaidEngine::new().with_site_config(PresentationConfig::from_value(json!({
        "securityLevel": "strict", "secure": ["securityLevel", "maxTextSize"]
    })));
    for source in [
        "%%{init: {\"securityLevel\":\"loose\",\"maxTextSize\":1,\"flowchart\":{\"htmlLabels\":false}}}%%\nflowchart TD\n A-->B",
        "---\nconfig:\n  securityLevel: loose\n  maxTextSize: 1\n  flowchart:\n    htmlLabels: false\n---\nflowchart TD\n A-->B",
    ] {
        let parsed = engine
            .parse_diagram_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(
            parsed.meta.config.as_value(),
            &json!({"flowchart":{"htmlLabels":false}})
        );
        assert_eq!(
            parsed.meta.effective_config.get_str("securityLevel"),
            Some("strict")
        );
        let typed = engine
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(
            typed.metadata().config.as_value(),
            parsed.meta.config.as_value()
        );
        assert_eq!(
            typed.metadata().effective_config.get_str("securityLevel"),
            Some("strict")
        );
    }
}

#[test]
fn init_namespaces_match_selected_mermaid_single_and_multiple_directives() {
    let engine = MermaidEngine::new();
    for (directives, expected) in [
        (
            vec![json!({"config": {"curve": "step"}})],
            json!({"flowchart": {"curve": "step"}}),
        ),
        (
            vec![json!({
                "config": {"curve": "step"},
                "flowchart": {"htmlLabels": false}
            })],
            json!({"flowchart": {"curve": "step"}}),
        ),
        (
            vec![
                json!({"config": {"curve": "step"}}),
                json!({"config": {"htmlLabels": false}}),
            ],
            json!({}),
        ),
        (
            vec![
                json!({"config": {"curve": "step"}}),
                json!({"flowchart": {"htmlLabels": false}}),
            ],
            json!({"flowchart": {"htmlLabels": false}}),
        ),
        (
            vec![
                json!({"flowchart": {"htmlLabels": false}}),
                json!({"config": {"curve": "step"}}),
            ],
            json!({"flowchart": {"htmlLabels": false}}),
        ),
        (
            vec![
                json!({"flowchart": {"curve": "step"}}),
                json!({"flowchart": {"htmlLabels": false}}),
            ],
            json!({"flowchart": {"curve": "step", "htmlLabels": false}}),
        ),
    ] {
        let source = directives
            .iter()
            .enumerate()
            .map(|(index, directive)| {
                let keyword = if index == 0 { "init" } else { "initialize" };
                format!("%%{{{keyword}: {directive}}}%%\n")
            })
            .collect::<String>()
            + "flowchart TD\nA-->B";
        for known_type in [false, true] {
            let (json, typed) = if known_type {
                (
                    engine.parse_diagram_with_type_sync(
                        "flowchart-v2",
                        &source,
                        ParseOptions::strict(),
                    ),
                    engine.parse_diagram_for_render_model_with_type_sync(
                        "flowchart-v2",
                        &source,
                        ParseOptions::strict(),
                    ),
                )
            } else {
                (
                    engine.parse_diagram_sync(&source, ParseOptions::strict()),
                    engine.parse_diagram_for_render_model_sync(&source, ParseOptions::strict()),
                )
            };
            assert_eq!(json.unwrap().unwrap().meta.config.as_value(), &expected);
            assert_eq!(
                typed.unwrap().unwrap().metadata().config.as_value(),
                &expected
            );
        }
    }
}
