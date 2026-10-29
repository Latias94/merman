use merman_core::{Engine, SourceSpan, preprocess_diagram_with_known_type};
use serde_json::json;

#[test]
fn single_and_multiple_inits_admit_config_in_the_upstream_order() {
    let engine = Engine::new();
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
                json!({"flowchart": {"curve": "step", "htmlLabels": false}}),
                json!({"flowchart": {"curve": "linear", "htmlLabels": null}}),
            ],
            json!({"flowchart": {"curve": "linear", "htmlLabels": false}}),
        ),
    ] {
        let mut source = String::new();
        for (index, args) in directives.iter().enumerate() {
            let keyword = if index == 0 { "init" } else { "initialize" };
            source.push_str(&format!("%%{{{keyword}: {args}}}%%\n"));
        }
        source.push_str("flowchart TD\nA-->B");

        for metadata in [
            engine.parse_metadata_sync(&source).unwrap(),
            engine
                .parse_metadata_with_type_sync("flowchart-v2", &source)
                .unwrap(),
        ] {
            assert_eq!(metadata.config.as_value(), &expected, "{source}");
        }

        #[cfg(feature = "diagram-flowchart")]
        for known_type in [false, true] {
            use merman_core::ParseOptions;

            let (parsed, rendered) = if known_type {
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
            assert_eq!(parsed.unwrap().unwrap().meta.config.as_value(), &expected);
            assert_eq!(
                rendered.unwrap().unwrap().metadata().config.as_value(),
                &expected
            );
        }
    }
}

#[test]
fn legacy_and_explicit_init_namespaces_follow_source_ids_across_families() {
    let engine = Engine::new();
    // Expected configs come from Mermaid 12.1.0 detectInit and sanitizeDirective.
    for (known_types, diagram, namespace, legacy_expected, explicit_expected) in [
        (
            &["flowchart-v2", "flowchart", "flowchart-elk"] as &[&str],
            "flowchart TD\nA-->B",
            "flowchart",
            json!({"flowchart": {"useMaxWidth": false}}),
            json!({"flowchart": {"useMaxWidth": false}}),
        ),
        (
            &["flowchart-elk", "flowchart-v2", "flowchart"],
            "flowchart-elk TD\nA-->B",
            "flowchart",
            json!({}),
            json!({"flowchart": {"useMaxWidth": false}}),
        ),
        (
            &["sequence"],
            "sequenceDiagram\nA->>B: hi",
            "sequence",
            json!({"sequence": {"useMaxWidth": false}}),
            json!({"sequence": {"useMaxWidth": false}}),
        ),
        (
            &["classDiagram", "class"],
            "classDiagram\nclass A",
            "class",
            json!({}),
            json!({"class": {"useMaxWidth": false}}),
        ),
        (
            &["stateDiagram", "state"],
            "stateDiagram-v2\n[*]-->A",
            "state",
            json!({}),
            json!({"state": {"useMaxWidth": false}}),
        ),
        (
            &["er", "erDiagram"],
            "erDiagram\nA ||--o{ B : link",
            "er",
            json!({"er": {"useMaxWidth": false}}),
            json!({"er": {"useMaxWidth": false}}),
        ),
        (
            &["xychart"],
            "xychart\nx-axis [a]\ny-axis 0 --> 10\nbar [2]",
            "xyChart",
            json!({}),
            json!({"xyChart": {"useMaxWidth": false}}),
        ),
        (&["info"], "info", "info", json!({}), json!({})),
    ] {
        for (key, single_expected, multi_expected) in [
            ("config", legacy_expected, json!({})),
            (namespace, explicit_expected.clone(), explicit_expected),
        ] {
            for (suffix, expected) in [
                ("", single_expected),
                ("\n%%{initialize: {}}%%", multi_expected),
            ] {
                let args = json!({(key): {"useMaxWidth": false}});
                let source = format!("%%{{init: {args}}}%%{suffix}\n{diagram}");
                let detected = engine.parse_metadata_sync(&source).unwrap();
                assert_eq!(detected.config.as_value(), &expected, "detect: {source}");
                for known_type in known_types {
                    let metadata = engine
                        .parse_metadata_with_type_sync(known_type, &source)
                        .unwrap();
                    assert_eq!(
                        metadata.config.as_value(),
                        &expected,
                        "{known_type}: {source}"
                    );
                }
            }
        }
    }
}

#[test]
fn init_remapping_preserves_frontmatter_and_original_source_spans() {
    let engine = Engine::new();
    for (suffix, expected) in [
        (
            "",
            json!({"flowchart": {"rankSpacing": 70, "curve": "step"}}),
        ),
        (
            "%%{initialize: {flowchart: {curve: 'linear'}}}%%\r\n",
            json!({"flowchart": {"rankSpacing": 70, "htmlLabels": false, "curve": "linear"}}),
        ),
    ] {
        let source = format!(
            concat!(
                "\u{feff}---\r\ntitle: 图\r\nconfig:\r\n",
                "  flowchart:\r\n    rankSpacing: 70\r\n---\r\n",
                "%%{{init: {{config: {{curve: 'step'}}, flowchart: {{htmlLabels: false}}}}}}%%\r\n",
                "{}flowchart TD\r\nA[节点]-->B\r\n",
            ),
            suffix,
        );
        for known_type in [None, Some("flowchart-v2")] {
            let preprocessed =
                preprocess_diagram_with_known_type(&source, engine.registry(), known_type).unwrap();
            assert_eq!(preprocessed.config.as_value(), &expected);
            assert_eq!(preprocessed.title.as_deref(), Some("图"));
            assert_eq!(preprocessed.code(), "flowchart TD\nA[节点]-->B\n");
            let start = preprocessed.code().find("节点").unwrap();
            let span = preprocessed
                .source
                .try_map_span(SourceSpan::new(start, start + "节点".len()))
                .unwrap();
            assert_eq!(&source[span.start..span.end], "节点");
        }
    }
}
