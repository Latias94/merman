//! Controlled browser measurements isolate the endpoint renderer from font/layout drift.
//! The companion integration test runs the provider; xtask binds this matrix to source hashes.

use super::svg_emit::{FlowchartSvgModelRequest, render_flowchart_svg_model};
use crate::environment::RenderEnvironment;
use crate::model::{FlowchartLayout, LayoutPoint};
use crate::svg::{SvgDebugOptions, SvgExecution, SvgRenderOptions};
use crate::text::{DeterministicTextMeasurer, TextMeasurer, TextMetrics, TextStyle, WrapMode};
use merman_core::{Engine, ParseOptions, RenderSemanticModel};
use serde::Deserialize;
use serde_json::Value;
use std::path::Path;

#[derive(Deserialize)]
struct BrowserSvgTextBox {
    text: String,
    tspan_bbox_height: f64,
    create_text_bbox_y: f64,
}

#[derive(Deserialize)]
struct BrowserSvgFace {
    style: TextStyle,
    texts: Vec<BrowserSvgTextBox>,
}

impl BrowserSvgFace {
    fn matches(&self, style: &TextStyle) -> bool {
        let family = |style: &TextStyle| {
            style
                .font_family
                .as_deref()
                .map(crate::config::normalize_css_font_family)
                .filter(|family| !family.is_empty())
        };
        let weight = |style: &TextStyle| {
            crate::text::apply_css_font_declarations(
                style,
                &format!(
                    "font-weight:{}",
                    style.font_weight.as_deref().unwrap_or("normal")
                ),
            )
            .font_weight
        };
        family(style).is_some()
            && family(style) == family(&self.style)
            && style.font_size == self.style.font_size
            && weight(style) == weight(&self.style)
            && style.font_style.as_deref().unwrap_or("normal")
                == self.style.font_style.as_deref().unwrap_or("normal")
    }
}

struct BrowserLabelBoxes {
    fixture: String,
    raw: std::collections::HashMap<String, TextMetrics>,
    svg_faces: Vec<BrowserSvgFace>,
    html_lines: std::collections::HashMap<String, f64>,
}

impl BrowserLabelBoxes {
    fn svg_text_box(&self, text: &str, style: &TextStyle) -> &BrowserSvgTextBox {
        self.svg_faces
            .iter()
            .filter(|face| face.matches(style))
            .flat_map(|face| &face.texts)
            .find(|probe| probe.text == text)
            .unwrap_or_else(|| {
                panic!(
                    "{}: missing browser SVG terminal primitive for {text:?} with {style:?}",
                    self.fixture
                )
            })
    }
}

impl TextMeasurer for BrowserLabelBoxes {
    fn measure_html_line_width_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.html_lines.get(text).copied().unwrap_or_else(|| {
            self.measure_wrapped(text, style, None, WrapMode::HtmlLike)
                .width
        })
    }

    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.raw
            .get(text)
            .copied()
            .unwrap_or_else(|| DeterministicTextMeasurer::default().measure(text, style))
    }

    fn measure_svg_tspan_text_bbox_height_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.svg_text_box(text, style).tspan_bbox_height
    }

    fn measure_svg_create_text_bbox_y_offset_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.svg_text_box(text, style).create_text_bbox_y
    }

    fn measure_wrapped(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        mode: WrapMode,
    ) -> TextMetrics {
        self.raw.get(text).copied().unwrap_or_else(|| {
            DeterministicTextMeasurer::default().measure_wrapped(text, style, max_width, mode)
        })
    }
}

#[test]
fn browser_svg_profile_captures_sanitized_source_placement_as_independent_literal_probes() {
    let captures: Value = serde_json::from_str(include_str!(
        "../../../../../../fixtures/_verification/flowchart-svg-terminal-measurements.json"
    ))
    .unwrap();
    let fixture =
        "upstream_cypress_newshapes_spec_newshapessets_newshapesset6_lr_md_html_false_094";
    let entry = captures["entries"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["fixture"] == fixture)
        .unwrap();
    let mut profile = BrowserLabelBoxes {
        fixture: fixture.to_owned(),
        raw: Default::default(),
        svg_faces: serde_json::from_value(entry["faces"].clone()).unwrap(),
        html_lines: Default::default(),
    };
    let style = TextStyle {
        font_family: Some("\"Recursive Variable\",arial,sans-serif".to_owned()),
        font_size: 14.0,
        font_weight: None,
        font_style: None,
    };
    for shape in ["brace-r", "braces"] {
        let text = format!("This is **bold** <br>and <strong>strong</strong> for {shape} shape");
        // The full label is four wrapped rows. Its metrics cannot supply a literal primitive.
        profile.raw.insert(
            text.clone(),
            TextMetrics {
                width: 120.0,
                height: 62.200_000_762_939_45,
                line_count: 4,
            },
        );
        assert_eq!(
            profile.measure_svg_tspan_text_bbox_height_px(&text, &style),
            16.0,
            "{shape}: independently captured single-tspan height"
        );
        assert_eq!(
            profile.measure_svg_create_text_bbox_y_offset_px(&text, &style),
            1.000_000_953_674_316_4,
            "{shape}: independently captured row-zero createText offset"
        );
        // Equivalent HTML spellings and different faces must not alias the captured request.
        let uncaptured = text.replace("<br>", "<br/>");
        assert!(std::panic::catch_unwind(|| profile.svg_text_box(&uncaptured, &style)).is_err());
        let bold = TextStyle {
            font_weight: Some("700".to_owned()),
            ..style.clone()
        };
        assert!(std::panic::catch_unwind(|| profile.svg_text_box(&text, &bold)).is_err());
    }
}

#[test]
fn flowchart_browser_measured_terminals_preserve_upstream_geometry() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let matrix: Value = serde_json::from_str(include_str!(
        "../../../../../../fixtures/_verification/flowchart-elk-browser-measurements.json"
    ))
    .unwrap();
    let terminals: Value = serde_json::from_str(include_str!(
        "../../../../../../fixtures/_verification/flowchart-svg-terminal-measurements.json"
    ))
    .unwrap();
    assert_eq!(terminals["schema_version"], 1);
    for key in ["mermaid_version", "mermaid_source_commit"] {
        assert_eq!(
            terminals[key], matrix[key],
            "{key}: SVG terminal provenance"
        );
    }
    let terminal_entries = terminals["entries"].as_array().unwrap();
    for terminal in terminal_entries {
        let fixture = matrix["entries"]
            .as_array()
            .unwrap()
            .iter()
            .find(|fixture| fixture["fixture"] == terminal["fixture"])
            .expect("SVG terminal fixture belongs to the browser matrix");
        for key in ["input_sha256", "upstream_svg_sha256"] {
            assert_eq!(
                terminal[key], fixture[key],
                "{}: SVG terminal {key}",
                terminal["fixture"]
            );
        }
    }
    let mut mismatches = Vec::new();
    let number =
        regex::Regex::new(r"[-+]?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:[eE][-+]?[0-9]+)?").unwrap();
    for fixture in matrix["entries"].as_array().unwrap() {
        let name = fixture["fixture"].as_str().unwrap();
        let source =
            std::fs::read_to_string(root.join("fixtures/flowchart").join(format!("{name}.mmd")))
                .unwrap();
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(&source, ParseOptions::default())
            .unwrap()
            .unwrap();
        let render_context = parsed.flowchart_render_context().unwrap().clone();
        let environment = RenderEnvironment::deterministic();
        let artifact = crate::family::prepare(
            parsed.clone(),
            &crate::LayoutOptions::default(),
            environment.begin_session().unwrap(),
        )
        .unwrap();
        let mut layout: FlowchartLayout = serde_json::from_value(
            artifact.layout_json().unwrap()["layout"]["FlowchartV2"].clone(),
        )
        .unwrap();
        // The public JSON projection omits this internal renderer ownership flag.
        layout.uses_elk_adapter_dom = true;
        let (metadata, semantic) = parsed.into_parts();
        let RenderSemanticModel::Flowchart(model) = semantic else {
            panic!("Flowchart")
        };
        for measured in fixture["nodes"].as_array().unwrap() {
            let id = measured["id"].as_str().unwrap();
            if let Some(center) = measured["center"].as_array() {
                for node in layout.nodes.iter_mut().filter(|n| n.id == id) {
                    node.x = center[0].as_f64().unwrap();
                    node.y = center[1].as_f64().unwrap();
                    if let Some(size) = measured["size"].as_array() {
                        node.width = size[0].as_f64().unwrap();
                        node.height = size[1].as_f64().unwrap();
                    }
                }
            }
        }
        for measured in fixture["edges"].as_array().unwrap() {
            let edge = layout
                .edges
                .iter_mut()
                .find(|e| e.id == measured["id"].as_str().unwrap())
                .unwrap();
            edge.points = measured["points"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| LayoutPoint {
                    x: p[0].as_f64().unwrap(),
                    y: p[1].as_f64().unwrap(),
                })
                .collect();
        }
        let request = SvgRenderOptions {
            diagram_id: Some(name.to_owned()),
            ..SvgRenderOptions::default()
        };
        let debug = SvgDebugOptions::default();
        let mut labels = std::collections::HashMap::new();
        for node in fixture["nodes"].as_array().unwrap() {
            let Some(text) = node["label_box"]["text"].as_str() else {
                continue;
            };
            let normalized = text
                .replace("<br>", "\n")
                .replace("<br/>", "\n")
                .replace("<br />", "\n");
            let metrics = TextMetrics {
                width: node["label_box"]["size"][0].as_f64().unwrap(),
                height: node["label_box"]["size"][1].as_f64().unwrap(),
                line_count: normalized.lines().count(),
            };
            for key in [text.to_string(), normalized] {
                if let Some(previous) = labels.insert(key.clone(), metrics) {
                    assert_eq!(
                        (previous.width, previous.height, previous.line_count),
                        (metrics.width, metrics.height, metrics.line_count),
                        "{name}: ambiguous captured label {key}"
                    );
                }
            }
        }
        let svg_faces = terminal_entries
            .iter()
            .find(|entry| entry["fixture"].as_str() == Some(name))
            .map(|entry| serde_json::from_value(entry["faces"].clone()).unwrap())
            .unwrap_or_default();
        use crate::environment::{
            MeasurementProfileId, TextMeasurementPolicy, TextMeasurementProfile,
            TextMeasurementProfileIdentity,
        };
        let profile = TextMeasurementProfile::new(
            TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("test.flowchart-browser-label-boxes").unwrap(),
                "fixture",
            )
            .unwrap(),
            std::sync::Arc::new(BrowserLabelBoxes {
                fixture: name.to_owned(),
                raw: labels,
                svg_faces,
                html_lines: Default::default(),
            }),
        );
        let session = environment
            .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile))
            .begin_session()
            .unwrap();
        let execution = SvgExecution::new(&request, &debug, &session).unwrap();
        let sidecar = crate::flowchart::FlowchartSvgLabelSidecar::default();
        let svg = render_flowchart_svg_model(
            FlowchartSvgModelRequest {
                layout: &layout,
                swimlane_layout: None,
                model: &model,
                render_context: &render_context,
                effective_config: &metadata.effective_config,
                diagram_type: metadata.diagram_type.as_str(),
                diagram_title: None,
                presentation_policy: None,
                svg_label_sidecar: &sidecar,
            },
            &execution,
            None,
        )
        .unwrap()
        .to_string();
        let reference = std::fs::read_to_string(
            root.join("fixtures/upstream-svgs/flowchart")
                .join(format!("{name}.svg")),
        )
        .unwrap();
        let actual = roxmltree::Document::parse(&svg).unwrap();
        let expected = roxmltree::Document::parse(&reference).unwrap();
        let identities = |document: &roxmltree::Document<'_>| {
            let mut ids = document
                .descendants()
                .filter_map(|n| {
                    n.attribute("id")
                        .map(|id| (n.tag_name().name().to_owned(), id.to_owned()))
                })
                .collect::<Vec<_>>();
            ids.sort();
            ids
        };
        assert_eq!(
            identities(&actual),
            identities(&expected),
            "{name}: DOM identities"
        );
        if name.ends_with("newshapesset6_lr_md_html_false_094") {
            // Joining outer lines restores only the whitespace introduced by wrapping.
            // Inner spans retain their literal text, including escaped <strong> markup.
            let labels = |document: &roxmltree::Document<'_>| {
                document
                    .descendants()
                    .filter(|n| n.has_tag_name("text"))
                    .map(|text| {
                        text.children()
                            .filter(|n| n.is_element())
                            .map(|line| {
                                line.descendants()
                                    .filter(|n| n.is_text())
                                    .filter_map(|n| n.text())
                                    .collect::<String>()
                            })
                            .collect::<Vec<_>>()
                            .join(" ")
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
                    .collect::<Vec<_>>()
            };
            let expected_labels = labels(&expected);
            assert_eq!(expected_labels.len(), 4, "{name}: SVG labels");
            assert_eq!(
                labels(&actual),
                expected_labels,
                "{name}: wrapped label content"
            );
        }
        for edge in expected
            .descendants()
            .filter(|n| n.attribute("data-edge") == Some("true"))
        {
            let id = edge.attribute("data-id").unwrap();
            let observed = actual
                .descendants()
                .find(|n| {
                    n.attribute("data-id") == Some(id) && n.attribute("data-edge") == Some("true")
                })
                .unwrap();
            for attribute in ["data-et", "data-look", "marker-start", "marker-end"] {
                assert_eq!(
                    observed.attribute(attribute),
                    edge.attribute(attribute),
                    "{name}/{id}: {attribute}"
                );
            }
            let commands = |n: roxmltree::Node<'_, '_>| {
                n.attribute("d")
                    .unwrap()
                    .chars()
                    .filter(char::is_ascii_uppercase)
                    .collect::<String>()
            };
            let coordinates = |n: roxmltree::Node<'_, '_>| {
                number
                    .find_iter(n.attribute("d").unwrap())
                    .map(|m| m.as_str().parse::<f64>().unwrap())
                    .collect::<Vec<_>>()
            };
            let observed_coordinates = coordinates(observed);
            let expected_coordinates = coordinates(edge);
            // The writer serializes coordinates to three decimal places.
            let same_coordinates = observed_coordinates.len() == expected_coordinates.len()
                && observed_coordinates
                    .iter()
                    .zip(&expected_coordinates)
                    .all(|(actual, expected)| (actual - expected).abs() <= 0.000_501);
            // RoughJS control points also depend on browser text/shape path geometry.
            // Keep hand-drawn command topology and marker identity checked, while the
            // companion provider test checks its numeric pre-paint routes separately.
            let check_coordinates = edge.attribute("data-look") != Some("handDrawn");
            if commands(observed) != commands(edge) || (check_coordinates && !same_coordinates) {
                mismatches.push(format!(
                    "{name}/{id}: expected={} actual={}\n expected_d={}\n actual_d={}",
                    commands(edge),
                    commands(observed),
                    edge.attribute("d").unwrap(),
                    observed.attribute("d").unwrap()
                ));
            }
        }
    }
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}
