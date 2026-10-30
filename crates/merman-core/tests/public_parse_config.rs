#![cfg(feature = "diagram-flowchart")]
#![cfg(feature = "compat-json")]

use merman_core::presentation::SourcePresentationConfig;
use merman_core::{MermaidEngine, ParseOptions};
use serde_json::json;

#[test]
fn public_parse_config_contains_only_admitted_source_overrides() {
    let engine =
        MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(json!({
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

#[cfg(feature = "all-diagrams")]
#[test]
fn legacy_init_uses_upstream_detector_keys_before_source_admission() {
    let engine = MermaidEngine::new();
    for (diagram_type, source, namespace, admits_legacy) in [
        ("flowchart-v2", "flowchart TD\nA-->B", "flowchart", true),
        (
            "flowchart-elk",
            "flowchart-elk TD\nA-->B",
            "flowchart",
            false,
        ),
        ("sequence", "sequenceDiagram\nA->>B: hi", "sequence", true),
        ("classDiagram", "classDiagram\nclass A", "class", false),
        ("class", "classDiagram\nclass A", "class", false),
        ("stateDiagram", "stateDiagram-v2\n[*]-->A", "state", false),
        ("state", "stateDiagram-v2\n[*]-->A", "state", false),
        ("er", "erDiagram\nA ||--o{ B : link", "er", true),
        ("erDiagram", "erDiagram\nA ||--o{ B : link", "er", true),
        (
            "xychart",
            "xychart\nx-axis [a,b]\ny-axis 0 --> 10\nbar [2,3]",
            "xyChart",
            false,
        ),
        ("info", "info", "info", false),
    ] {
        for legacy in [false, true] {
            let args = if legacy {
                json!({"config": {"useMaxWidth": false}})
            } else {
                json!({(namespace): {"useMaxWidth": false}})
            };
            let source = format!("%%{{init: {args}}}%%\n{source}");
            let expected = if (legacy && admits_legacy) || (!legacy && namespace != "info") {
                json!({(namespace): {"useMaxWidth": false}})
            } else {
                json!({})
            };
            for metadata in [
                engine.parse_metadata_sync(&source).unwrap(),
                engine
                    .parse_metadata_with_type_sync(diagram_type, &source)
                    .unwrap(),
                engine
                    .parse_diagram_sync(&source, ParseOptions::strict())
                    .unwrap()
                    .unwrap()
                    .meta,
                engine
                    .parse_diagram_for_render_model_with_type_controlled_sync(
                        diagram_type,
                        &source,
                        ParseOptions::strict(),
                        &merman_model::operation::OperationControl::new(),
                    )
                    .unwrap()
                    .unwrap()
                    .unwrap()
                    .metadata()
                    .clone(),
            ] {
                assert_eq!(
                    metadata.config.as_value(),
                    &expected,
                    "{diagram_type}, legacy={legacy}: {source}"
                );
            }
        }
    }
}

#[cfg(feature = "diagram-mindmap")]
#[test]
fn mindmap_compatibility_projection_retains_the_final_layout_selection() {
    for (site, prefix, expected) in [
        (json!({}), "", json!("cose-bilkent")),
        (json!({"layout": "dagre"}), "", json!("dagre")),
        (json!({"mindmap": {"layout": "elk"}}), "", json!("elk")),
        (
            json!({"layout": "elk"}),
            "---\nconfig:\n  layout: dagre\n---\n",
            json!("dagre"),
        ),
        (
            json!({"layout": "elk"}),
            "---\nconfig:\n  layout: null\n---\n",
            json!("elk"),
        ),
    ] {
        let engine =
            MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(site));
        let source = format!("{prefix}mindmap\nroot\n  child");
        let parsed = engine
            .parse_diagram_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(parsed.meta.effective_config.as_value()["layout"], expected);
        assert_eq!(parsed.model["config"]["layout"], expected);
    }
}
