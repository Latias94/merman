#![cfg(feature = "diagram-sequence")]

use merman_core::SemanticParser;
use merman_model::{families::sequence::SequenceDiagramModel, semantic::SemanticDiagramModel};
use serde_json::json;

fn parse_sequence(source: &str) -> SequenceDiagramModel {
    let parsed = SemanticParser::new()
        .parse_diagram_sync(source)
        .expect("valid Sequence source");
    let SemanticDiagramModel::Sequence(model) = parsed.model else {
        panic!("expected the canonical Sequence model");
    };
    model
}

#[test]
fn menu_text_modifiers_are_removed_before_reading_links_and_properties() {
    for newline in ["\n", "\r\n"] {
        for prefix in ["", "wrap:", ":wrap:", "nowrap:", ":nowrap:"] {
            let source = format!(
                concat!(
                    "sequenceDiagram\nparticipant Link as Channel\n",
                    "links Link: {0} {{\"Docs\":\"https://example.test/docs\"}}\n",
                    "link Link: {0} Help @ https://example.test/help\n",
                    "properties Link: {0} {{\"class\":\"service\",\"icon\":\"@clock\"}}\n",
                    "Link->>Other: request\n",
                ),
                prefix,
            )
            .replace('\n', newline);
            let model = parse_sequence(&source);
            assert_eq!(model.actor_order, ["Link", "Other"], "{source:?}");
            let actor = &model.actors["Link"];
            assert_eq!(actor.description, "Channel");
            assert_eq!(actor.links.len(), 2, "{source:?}");
            assert_eq!(actor.links["Docs"], "https://example.test/docs");
            assert_eq!(actor.links["Help"], "https://example.test/help");
            assert_eq!(actor.properties["class"], "service");
            assert_eq!(actor.properties["icon"], "@clock");
            assert!(!actor.wrap);
            assert_eq!(model.messages.len(), 1);
            assert!(!model.messages[0].wrap);
            assert_eq!(model.messages[0].message_text(), "request");
        }
    }
}

#[test]
fn menu_text_modifiers_are_case_sensitive_and_removed_once() {
    for (prefix, label_prefix) in [
        ("WRAP:", "WRAP:"),
        (":NOWRAP:", ":NOWRAP:"),
        ("wRaP:", "wRaP:"),
        ("wrap:nowrap:", "nowrap:"),
    ] {
        let source = format!(
            concat!(
                "sequenceDiagram\n",
                "links A: {0} {{\"Docs\":\"https://example.test/docs\"}}\n",
                "properties A: {0} {{\"class\":\"service\"}}\n",
                "link A: {0} Help @ https://example.test/help\n",
            ),
            prefix,
        );
        let model = parse_sequence(&source);
        let actor = &model.actors["A"];
        assert!(actor.properties.is_empty(), "{source:?}");
        assert_eq!(actor.links.len(), 1, "{source:?}");
        assert_eq!(
            actor.links[&format!("{label_prefix} Help")],
            json!("https://example.test/help"),
            "{source:?}",
        );
    }
}

#[test]
fn menu_json_and_message_trimming_preserves_nel_and_removes_bom() {
    for newline in ["\n", "\r\n"] {
        for (padding, valid_json) in [("\u{0085}", false), ("\u{feff}", true)] {
            for prefix in ["", "wrap:", ":wrap:", "nowrap:", ":nowrap:"] {
                let source = format!(
                    concat!(
                        "sequenceDiagram\n",
                        "links A:{prefix}{padding}{{\"Docs\":\"https://example.test/docs\"}}{padding}\n",
                        "properties A:{prefix}{padding}{{\"class\":\"service\"}}{padding}\n",
                        "A->>B:{prefix}{padding}message{padding}\n",
                    ),
                    prefix = prefix,
                    padding = padding,
                )
                .replace('\n', newline);
                let model = parse_sequence(&source);
                assert_eq!(model.actor_order, ["A", "B"], "{source:?}");
                let actor = &model.actors["A"];
                if valid_json {
                    assert_eq!(actor.links.len(), 1, "{source:?}");
                    assert_eq!(actor.links["Docs"], "https://example.test/docs");
                    assert_eq!(actor.properties.len(), 1, "{source:?}");
                    assert_eq!(actor.properties["class"], "service");
                } else {
                    assert!(actor.links.is_empty(), "{source:?}");
                    assert!(actor.properties.is_empty(), "{source:?}");
                }
                assert_eq!(model.messages.len(), 1);
                assert_eq!(
                    model.messages[0].message_text(),
                    if valid_json {
                        "message".to_string()
                    } else {
                        format!("{padding}message{padding}")
                    },
                    "{source:?}",
                );
                assert_eq!(
                    model.messages[0].wrap,
                    matches!(prefix, "wrap:" | ":wrap:"),
                    "{source:?}",
                );
            }
        }
    }
}

#[test]
fn single_link_trims_bom_without_discarding_nel_in_labels_and_urls() {
    for (padding, retained) in [("\u{0085}", "\u{0085}"), ("\u{feff}", "")] {
        for prefix in ["", "wrap:", ":wrap:", "nowrap:", ":nowrap:"] {
            let source = format!(
                "sequenceDiagram\nlink A:{prefix}{padding}Docs{padding} @ {padding}https://example.test/docs{padding}\n"
            );
            let model = parse_sequence(&source);
            let links = &model.actors["A"].links;
            assert_eq!(links.len(), 1, "{source:?}");
            let label = format!("{retained}Docs{retained}");
            assert_eq!(
                links[&label],
                json!(format!("{retained}https://example.test/docs{retained}")),
                "{source:?}",
            );
        }
    }
}
