use merman_core::presentation::PresentationConfig;
use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::{
    HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
    MeasurementProfileId, RenderEnvironment, TextMeasurementOperation, TextMeasurementPhase,
    TextMeasurementPolicy, TextMeasurementProfileIdentity, TextMeasurementResultKind,
};
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::json;
use std::sync::Arc;

const SOURCE: &str = r#"usecase-beta
direction LR
actor User("Customer")@{type:hollow} <<Human>>
systemBoundary Auth(Authentication)@{type:package}
Login("Sign & go")@{business:true}
Verify[Check]
end
User link@--> Login
Login ..> : include Verify
note for Login "Remember"
json Data@{"second":2,"first":{"z":0,"a":1}}
Data -- Login
"#;

fn render(source: &str, layout: &str) -> (merman_core::ManagedSemanticJson, String) {
    render_config(
        source,
        json!({
            "layout": layout, "theme": "default", "look": "classic", "htmlLabels": false,
        }),
    )
}

fn render_config(
    source: &str,
    config: serde_json::Value,
) -> (merman_core::ManagedSemanticJson, String) {
    render_config_with_environment(source, config, RenderEnvironment::deterministic())
}

fn render_config_with_environment(
    source: &str,
    config: serde_json::Value,
    environment: RenderEnvironment,
) -> (merman_core::ManagedSemanticJson, String) {
    let engine = MermaidEngine::new().with_site_config(PresentationConfig::from_value(config));
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse Usecase")
        .expect("detect Usecase");
    let session = environment.begin_session().expect("render session");
    let artifact = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .expect("prepare Usecase");
    let projection = artifact.layout_json().expect("layout JSON");
    let svg = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("usecase-test".to_owned()),
                ..Default::default()
            },
            &SvgDebugOptions::default(),
        )
        .expect("render Usecase")
        .svg()
        .to_owned();
    (projection, svg)
}

#[test]
fn usecase_html_markdown_measures_visible_inline_literals_without_added_spaces() {
    use merman_render::environment::TextMeasurementProfile;
    use merman_render::text::{TextMeasurer, TextMetrics, TextStyle};
    use std::sync::Mutex;

    #[derive(Default)]
    struct LiteralMeasurer(Mutex<Vec<String>>);
    impl TextMeasurer for LiteralMeasurer {
        fn measure(&self, text: &str, _: &TextStyle) -> TextMetrics {
            self.0.lock().unwrap().push(text.to_owned());
            TextMetrics {
                width: text.chars().count() as f64 * 2.0,
                height: 20.0,
                line_count: 1,
            }
        }
    }

    for text in [
        "x `c` [visible label](u)",
        "pre`code`post",
        "pre[visible label](u)`code`post",
        "x `c` ~~visible label~~",
        "pre~~visible~~`code`post",
        "pre`code`[]()post",
    ] {
        let measurer = Arc::new(LiteralMeasurer::default());
        let profile = TextMeasurementProfile::new(
            TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("test.usecase-inline-literals").unwrap(),
                "1",
            )
            .unwrap(),
            measurer.clone(),
        );
        let (_, svg) = render_config_with_environment(
            &format!("usecase-beta\nA(\"`{text}`\")\n"),
            json!({"layout":"dagre", "htmlLabels":true,
                "usecase":{"wrappingWidth":1000,"minNodeWidth":0}}),
            RenderEnvironment::deterministic()
                .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile)),
        );
        let document = roxmltree::Document::parse(&svg).unwrap();
        let paragraph = document
            .descendants()
            .find(|node| node.has_tag_name("p"))
            .unwrap();
        let visible: String = paragraph
            .descendants()
            .filter(|node| node.is_text())
            .filter_map(|node| node.text())
            .collect();
        assert_eq!(visible, text);
        let requests = measurer.0.lock().unwrap();
        assert!(
            requests.iter().any(|request| request == &visible),
            "{text:?}: {requests:?}"
        );
        let foreign_object = paragraph
            .ancestors()
            .find(|node| node.has_tag_name("foreignObject"))
            .unwrap();
        assert_eq!(
            foreign_object
                .attribute("width")
                .unwrap()
                .parse::<f64>()
                .unwrap(),
            visible.chars().count() as f64 * 2.0,
            "{text:?}"
        );
        assert_eq!(foreign_object.attribute("height"), Some("20"), "{text:?}");
    }
}

#[test]
fn usecase_svg_distinguishes_plain_and_positioned_labels_with_signed_host_bbox_offsets() {
    const TEXT_WIDTH: f64 = 40.0;
    const TEXT_HEIGHT: f64 = 24.0;
    const NODE_MIN_WIDTH: f64 = 64.0;
    struct FixedLabelBox {
        y_offset: f64,
    }
    impl HostTextMeasurer for FixedLabelBox {
        fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            let metrics = merman_render::text::TextMetrics {
                width: if request.text.is_empty() {
                    0.0
                } else {
                    TEXT_WIDTH
                },
                height: if request.text.is_empty() {
                    0.0
                } else {
                    TEXT_HEIGHT
                },
                line_count: 1,
            };
            Ok(Some(match request.operation.required_result_kind() {
                TextMeasurementResultKind::Metrics => HostTextMeasurement::Metrics(metrics),
                TextMeasurementResultKind::Length => {
                    HostTextMeasurement::Length(match request.operation {
                        TextMeasurementOperation::CreateTextBBoxYOffset
                        | TextMeasurementOperation::CreateTextMiddleBBoxYOffset => self.y_offset,
                        TextMeasurementOperation::RawBBoxHeight
                        | TextMeasurementOperation::SimpleBBoxHeight
                        | TextMeasurementOperation::TspanBBoxHeight => metrics.height,
                        _ => metrics.width,
                    })
                }
                TextMeasurementResultKind::HorizontalExtents => {
                    HostTextMeasurement::HorizontalExtents {
                        left: metrics.width / 2.0,
                        right: metrics.width / 2.0,
                    }
                }
                TextMeasurementResultKind::WrappedWithRawWidth => {
                    HostTextMeasurement::WrappedWithRawWidth {
                        raw_width: Some(metrics.width),
                        metrics,
                    }
                }
            }))
        }
    }
    fn translation(node: roxmltree::Node<'_, '_>) -> (f64, f64) {
        let coordinates = node
            .attribute("transform")
            .expect("label transform")
            .trim_start_matches("translate(")
            .trim_end_matches(')')
            .split(',')
            .map(|coordinate| coordinate.trim().parse::<f64>().unwrap())
            .collect::<Vec<_>>();
        (coordinates[0], coordinates[1])
    }
    fn number(node: roxmltree::Node<'_, '_>, attribute: &str) -> f64 {
        node.attribute(attribute).unwrap().parse().unwrap()
    }
    let source = r#"usecase-beta
direction LR
Plain[Plain]
actor Actor(Role) <<Human>>
Business(Business)@{business:true}
note for Plain "Help"
json Data@{"key":"value"}
Plain approved@-- "Approved" --> Business
"#;
    for backend in ["dagre", "elk"] {
        let mut actor_baseline: Option<(f64, [f64; 2])> = None;
        for y_offset in [-3.0, 5.0] {
            let identity = TextMeasurementProfileIdentity::new(
                MeasurementProfileId::new("test.usecase-fixed-label-box").unwrap(),
                "1",
            )
            .unwrap();
            let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
                TextMeasurementPolicy::host_display(
                    identity,
                    Arc::new(FixedLabelBox { y_offset }),
                    TextMeasurementPhase::ALL,
                ),
            );
            let (_, svg) = render_config_with_environment(
                source,
                json!({
                    "layout": backend, "look": "classic", "htmlLabels": false,
                    "usecase": {"minNodeWidth": NODE_MIN_WIDTH},
                }),
                environment,
            );
            let document = roxmltree::Document::parse(&svg).unwrap();
            let node = |id: &str| {
                document
                    .descendants()
                    .find(|node| node.attribute("data-usecase-id") == Some(id))
                    .unwrap()
            };
            let label = |owner, class: &str| {
                document
                    .descendants()
                    .find(|label| {
                        label.attribute("class") == Some(class)
                            && label.ancestors().any(|ancestor| ancestor == owner)
                    })
                    .unwrap()
            };
            // util.ts labelHelper centers only Y; custom positionLabel helpers
            // center the local bbox and subtract its signed SVG Y offset.
            let plain = label(node("Plain"), "label");
            assert_eq!(translation(plain), (0.0, -TEXT_HEIGHT / 2.0));
            for (id, class, width) in [
                ("Actor", "label actor-label usecase-actor-label", TEXT_WIDTH),
                ("Actor", "label usecase-stereotype", TEXT_WIDTH),
                ("Business", "label usecase-label", NODE_MIN_WIDTH),
                ("note-0", "label noteLabel", NODE_MIN_WIDTH),
            ] {
                let positioned = label(node(id), class);
                assert_eq!(
                    translation(positioned).0,
                    -width / 2.0,
                    "{backend}: {class}"
                );
                if id != "Actor" {
                    assert_eq!(translation(positioned).1, -TEXT_HEIGHT / 2.0 - y_offset);
                }
                assert!(
                    positioned
                        .descendants()
                        .filter(|child| child.has_tag_name("text"))
                        .all(|text| text.attribute("text-anchor").is_none())
                );
            }
            let actor_y = [
                "label actor-label usecase-actor-label",
                "label usecase-stereotype",
            ]
            .map(|class| translation(label(node("Actor"), class)).1);
            if let Some((previous_offset, previous_y)) = actor_baseline {
                for (y, previous) in actor_y.into_iter().zip(previous_y) {
                    assert_eq!(y + y_offset, previous + previous_offset);
                }
            }
            actor_baseline = Some((y_offset, actor_y));
            // JSON custom positioning centers each text bbox in its painted cell.
            for (class, cell_class, width) in [
                (
                    "label usecase-json-title",
                    "usecase-json-title-cell",
                    NODE_MIN_WIDTH,
                ),
                (
                    "label usecase-json-key",
                    "usecase-json-key-cell",
                    TEXT_WIDTH,
                ),
                (
                    "label usecase-json-value",
                    "usecase-json-value-cell",
                    TEXT_WIDTH,
                ),
            ] {
                let positioned = label(node("Data"), class);
                let cell = positioned
                    .parent()
                    .unwrap()
                    .children()
                    .find(|child| {
                        child.attribute("class").is_some_and(|classes| {
                            classes
                                .split_ascii_whitespace()
                                .any(|value| value == cell_class)
                        })
                    })
                    .unwrap();
                let (x, y) = translation(positioned);
                assert_eq!(
                    x + width / 2.0,
                    number(cell, "x") + number(cell, "width") / 2.0
                );
                assert_eq!(
                    y + TEXT_HEIGHT / 2.0 + y_offset,
                    number(cell, "y") + number(cell, "height") / 2.0
                );
            }
            let edge_label = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("g") && node.attribute("data-id") == Some("approved")
                })
                .unwrap();
            assert_eq!(
                translation(edge_label),
                (0.0, -TEXT_HEIGHT / 2.0 - y_offset)
            );
            let background = edge_label
                .descendants()
                .find(|node| node.attribute("class") == Some("background"))
                .unwrap();
            assert_eq!(number(background, "x"), -TEXT_WIDTH / 2.0 - 2.0);
            assert_eq!(number(background, "y"), y_offset - 2.0);
            assert_eq!(number(background, "width"), TEXT_WIDTH + 4.0);
            assert_eq!(number(background, "height"), TEXT_HEIGHT + 4.0);
        }
    }
}

#[test]
fn usecase_neo_marker_margin_is_reserved_for_static_relationships() {
    let source = r#"usecase-beta
direction LR
A static@--> B
A enabled@--> C
enabled@{animate:true}
A fast@--> D
fast@{animation:fast}
A slow@--> E
slow@{animation:slow}
A disabled@--> F
disabled@{animate:true}
disabled@{animate:false}
A styled@--> G
classDef moving animation:dash 2s linear infinite
class styled moving
A inline@--> H
style inline animation:dash 2s linear infinite
"#;
    for backend in ["dagre", "elk"] {
        let (_, svg) = render_config(
            source,
            json!({
                "layout": backend, "look": "neo", "htmlLabels": false,
            }),
        );
        let document = roxmltree::Document::parse(&svg).unwrap();
        for (id, animation, animated_marker) in [
            ("static", None, false),
            ("enabled", Some("edge-animation-fast"), true),
            ("fast", Some("edge-animation-fast"), true),
            ("slow", Some("edge-animation-slow"), true),
            ("disabled", None, false),
            ("styled", None, true),
            // edges.js considers compiled class animation, not an inline path style.
            ("inline", None, false),
        ] {
            let path = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("path") && node.attribute("data-usecase-id") == Some(id)
                })
                .unwrap();
            let classes = path.attribute("class").unwrap();
            let actual_animation = classes
                .split_ascii_whitespace()
                .find(|class| class.starts_with("edge-animation-"));
            assert_eq!(actual_animation, animation, "{backend}: {id}");
            let expected = if animated_marker {
                "_usecase-pointEnd)"
            } else {
                "_usecase-pointEnd-margin)"
            };
            assert!(
                path.attribute("marker-end").unwrap().ends_with(expected),
                "{backend}: {id}"
            );
        }
    }
}

#[test]
fn usecase_svg_labels_preserve_actual_create_text_groups_and_classic_markers() {
    const SOURCE: &str =
        include_str!("../../../fixtures/usecase/local_elk_boundary_label_guard.mmd");
    let (_, svg) = render(SOURCE, "elk");
    for svg in [
        svg.as_str(),
        include_str!("../../../fixtures/upstream-svgs/usecase/local_elk_boundary_label_guard.svg"),
    ] {
        let document = roxmltree::Document::parse(svg).expect("valid actual-reference SVG");
        for id in ["A", "B", "C"] {
            let node = document
                .descendants()
                .find(|node| {
                    node.attribute("data-usecase-id") == Some(id)
                        && node.attribute("data-usecase-kind") == Some("usecase")
                })
                .expect("Usecase node");
            let label = node
                .children()
                .find(|node| node.attribute("class") == Some("label"))
                .expect("node label");
            let children: Vec<_> = label.children().filter(|node| node.is_element()).collect();
            assert_eq!(children.len(), 2);
            assert!(children[0].has_tag_name("rect"));
            assert!(children[1].has_tag_name("g"));
            let background = children[1]
                .children()
                .find(|node| node.attribute("class") == Some("background"))
                .expect("createText background");
            assert_eq!(background.attribute("style"), Some("stroke: none"));
            let text = children[1]
                .children()
                .find(|node| node.has_tag_name("text"))
                .expect("left-anchored node text");
            assert_eq!(text.attribute("text-anchor"), None);
        }
        let labels = document
            .descendants()
            .find(|node| node.attribute("class") == Some("edgeLabels"))
            .expect("edge labels");
        let children: Vec<_> = labels.children().filter(|node| node.is_element()).collect();
        assert_eq!(children.len(), 2, "ELK omits the unlabeled relationship");
        for label in children {
            let background = label
                .descendants()
                .find(|node| node.attribute("class") == Some("background"))
                .expect("edge background");
            assert_eq!(background.attribute("style"), Some(""));
            assert!(
                background
                    .attribute("width")
                    .expect("paint width")
                    .parse::<f64>()
                    .unwrap()
                    > 4.0
            );
            assert!(
                background
                    .attribute("height")
                    .expect("paint height")
                    .parse::<f64>()
                    .unwrap()
                    > 4.0
            );
            let text = label
                .descendants()
                .find(|node| node.has_tag_name("text"))
                .expect("edge text");
            assert_eq!(text.attribute("text-anchor"), Some("middle"));
        }
        let paths: Vec<_> = document
            .descendants()
            .filter(|node| {
                node.has_tag_name("path")
                    && node.attribute("data-usecase-kind") == Some("relationship")
            })
            .collect();
        assert_eq!(paths.len(), 3);
        assert!(paths.iter().all(|path| {
            path.attribute("marker-end")
                .is_some_and(|marker| marker.ends_with("_usecase-pointEnd)"))
        }));
    }
    let neo = SOURCE.replace("look: classic", "look: neo");
    let (_, svg) = render(&neo, "elk");
    let document = roxmltree::Document::parse(&svg).expect("valid Neo SVG");
    assert!(
        document
            .descendants()
            .filter(|node| {
                node.has_tag_name("path")
                    && node.attribute("data-usecase-kind") == Some("relationship")
            })
            .all(|path| path
                .attribute("marker-end")
                .is_some_and(|marker| marker.ends_with("_usecase-pointEnd-margin)")))
    );
}

#[test]
fn usecase_dagre_preserves_shapes_source_ids_and_accessibility() {
    let (_, svg) = render(SOURCE, "dagre");
    let document = roxmltree::Document::parse(&svg).expect("valid SVG");
    for (id, kind) in [
        ("User", "actor"),
        ("Login", "usecase"),
        ("Verify", "usecase"),
        ("Auth", "boundary"),
        ("note-0", "note"),
        ("Data", "json"),
    ] {
        let node = document
            .descendants()
            .find(|node| {
                node.attribute("data-usecase-id") == Some(id)
                    && node.attribute("data-usecase-kind") == Some(kind)
            })
            .unwrap_or_else(|| panic!("missing {kind} {id}"));
        assert_eq!(
            node.attribute("id"),
            Some(format!("usecase-test-usecase-{id}").as_str())
        );
        assert_eq!(node.attribute("role"), Some("img"));
        assert!(
            node.attribute("aria-label")
                .is_some_and(|label| !label.is_empty())
        );
    }
    let relationship = document
        .descendants()
        .find(|node| node.attribute("data-usecase-id") == Some("link"))
        .expect("explicit relationship");
    assert_eq!(
        relationship.attribute("id"),
        Some("usecase-usecase-test-link")
    );
    assert_eq!(
        relationship.attribute("aria-label"),
        Some("association from Customer to Sign & go")
    );
    let note_edge = document
        .descendants()
        .find(|node| node.attribute("data-usecase-kind") == Some("note-connector"))
        .expect("note connector");
    assert_eq!(note_edge.attribute("aria-hidden"), Some("true"));
    assert!(note_edge.attribute("aria-label").is_none());
    assert!(svg.contains("usecase-actor-hollow-body"));
    assert!(svg.contains("system-boundary-package-tab"));
    assert!(svg.contains("usecase-business-marker"));
    assert!(svg.contains("usecase-json-row"));
}

#[cfg(feature = "layout-elk")]
#[test]
fn usecase_elk_preserves_raw_missing_sections_and_draws_relationships() {
    let (projection, svg) = render("usecase-beta\nactor A\nA --> B", "elk.box");
    let edges = projection["layout"]["UsecaseDiagram"]["edges"]
        .as_array()
        .expect("edges");
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0]["points"], json!([]));
    let document = roxmltree::Document::parse(&svg).expect("valid SVG");
    let edge = document
        .descendants()
        .find(|node| node.attribute("data-usecase-kind") == Some("relationship"))
        .expect("painted relationship");
    let path = edge.attribute("d").expect("path data");
    assert!(path.starts_with('M') && path.contains('L'), "{path}");
    assert!(!path.contains("NaN") && !path.contains("inf"));
}

#[cfg(feature = "layout-elk")]
#[test]
fn usecase_elk_spreads_implicit_ports_on_ellipses() {
    for font_size in [14, 40] {
        let (projection, _) = render_config(
            "usecase-beta\ndirection LR\nA --> Target\nB --> Target\nC --> Target",
            json!({"layout":"elk", "htmlLabels":false,
                "themeVariables":{"fontSize":format!("{font_size}px")},
                "usecase":{"usecaseFontSize":40}}),
        );
        let graph = &projection["layout"]["UsecaseDiagram"];
        let target = graph["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["id"] == "Target")
            .unwrap();
        let center = target["y"].as_f64().unwrap();
        let half_height = target["height"].as_f64().unwrap() / 2.0;
        let left = target["x"].as_f64().unwrap() - target["width"].as_f64().unwrap() / 2.0;
        let mut offsets: Vec<_> = graph["edges"]
            .as_array()
            .unwrap()
            .iter()
            .map(|edge| {
                assert_eq!(edge["to"], "Target");
                let port = edge["points"].as_array().unwrap().last().unwrap();
                assert!((port["x"].as_f64().unwrap() - left).abs() < 1e-9, "{port}");
                port["y"].as_f64().unwrap() - center
            })
            .collect();
        offsets.sort_by(f64::total_cmp);
        assert_eq!(offsets.len(), 3);
        assert!(offsets[0] < -1e-9 && offsets[2] > 1e-9, "{offsets:?}");
        assert!(offsets[1].abs() < 1e-9, "{offsets:?}");
        assert!((offsets[0] + offsets[2]).abs() < 1e-9, "{offsets:?}");
        assert!(
            offsets.iter().all(|offset| offset.abs() < half_height),
            "{offsets:?}"
        );
        // CENTER compresses port gaps on short sides. A tall root font restores
        // enough room for the default 20px gaps and distinguishes DISTRIBUTED.
        if font_size == 40 {
            for (actual, expected) in offsets.iter().zip([-20.0, 0.0, 20.0]) {
                assert!((actual - expected).abs() < 1e-9, "{offsets:?}");
            }
        }
    }
}

#[test]
fn usecase_sanitizes_before_stereotype_folding_and_keeps_endpoint_labels_unfolded() {
    let source = r#"usecase-beta
actor User("`**Reader**`")
Login("`<script>alert(1)</script>**Sign in**`") <<Secure>>
User edge@--> Login
json Data@{"field":"<script>alert(2)</script>clean"}
"#;
    for html in [false, true] {
        let (_, svg) = render_config(
            source,
            json!({
                "layout":"dagre", "htmlLabels":html, "securityLevel":"strict",
            }),
        );
        assert!(!svg.contains("alert("), "sanitizer left script content");
        let document = roxmltree::Document::parse(&svg).expect("valid SVG");
        let login = document
            .descendants()
            .find(|node| node.attribute("data-usecase-id") == Some("Login"))
            .unwrap();
        assert_eq!(
            login.attribute("aria-label"),
            Some("use case Sign in, stereotype Secure")
        );
        assert!(
            login
                .descendants()
                .any(|node| node.attribute("class").is_some_and(|value| value
                    .split_whitespace()
                    .any(|class| class == "usecase-stereotype")))
        );
        let edge = document
            .descendants()
            .find(|node| node.attribute("data-usecase-id") == Some("edge"))
            .unwrap();
        assert_eq!(
            edge.attribute("aria-label"),
            Some("association from Reader to Sign in")
        );
        let data = document
            .descendants()
            .find(|node| node.attribute("data-usecase-id") == Some("Data"))
            .unwrap();
        assert_eq!(data.attribute("aria-label"), Some("Data: field: clean"));
    }
}

#[test]
fn usecase_dagre_respects_shared_curve_selection() {
    let source = "usecase-beta\ndirection LR\nA --> B\nA --> C\nB --> C";
    let edge_path = |curve| {
        let (_, svg) = render_config(
            source,
            json!({"layout":"dagre", "flowchart":{"curve":curve}, "htmlLabels":false}),
        );
        let document = roxmltree::Document::parse(&svg).unwrap();
        document
            .descendants()
            .filter(|node| node.attribute("data-usecase-kind") == Some("relationship"))
            .map(|node| node.attribute("d").unwrap().to_owned())
            .collect::<Vec<_>>()
    };
    let linear = edge_path("linear");
    let basis = edge_path("basis");
    assert!(linear.iter().all(|path| !path.contains('C')));
    assert!(basis.iter().any(|path| path.contains('C')));
    assert_ne!(linear, basis);
}

#[test]
fn usecase_palette_separates_containers_and_participants_and_preserves_edge_classes() {
    let source = "usecase-beta\nactor A\nsystemBoundary S(System)\nB(Login)\nend\nA link@--> B\nlink@{animation:slow}\nclass link custom\nnote for B \"Help\"\njson Data@{\"x\":1}";
    let (_, svg) = render_config(
        source,
        json!({
            "layout":"dagre", "look":"classic", "theme":"redux-color", "htmlLabels":false,
            "usecase":{"colorScheme":"rotate"},
            "themeVariables":{"borderColorArray":["#102030","#405060"], "bkgColorArray":["#abcdef"]},
        }),
    );
    let document = roxmltree::Document::parse(&svg).unwrap();
    for (id, slot) in [
        ("A", Some("color-0")),
        ("B", Some("color-1")),
        ("S", Some("color-0")),
        ("note-0", None),
        ("Data", None),
    ] {
        let node = document
            .descendants()
            .find(|node| node.attribute("data-usecase-id") == Some(id))
            .unwrap();
        assert_eq!(node.attribute("data-color-id"), slot, "{id}");
    }
    let edge = document
        .descendants()
        .find(|node| node.attribute("data-usecase-id") == Some("link"))
        .unwrap();
    let classes: Vec<_> = edge
        .attribute("class")
        .unwrap()
        .split_whitespace()
        .collect();
    assert!(classes.contains(&"relationship-association"));
    assert!(classes.contains(&"edge-animation-slow"));
    assert!(classes.contains(&"custom"));
    let css: String = document
        .descendants()
        .filter(|node| node.has_tag_name("style"))
        .filter_map(|node| node.text())
        .collect();
    assert!(css.contains("stroke:#405060;fill:#abcdef"), "{css}");
}

#[test]
fn usecase_svg_terminal_font_uses_matching_selectors_while_html_keeps_role_font() {
    struct FontHost;
    impl HostTextMeasurer for FontHost {
        fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            if request.text != "Alpha" {
                return Ok(None);
            }
            let family_width = if request.style.font_family.as_deref() == Some("monospace") {
                7.0
            } else {
                0.0
            };
            let bold_width = if matches!(request.style.font_weight.as_deref(), Some("bold" | "700"))
            {
                40.0
            } else {
                0.0
            };
            let italic_width = if request.style.font_style.as_deref() == Some("italic") {
                5.0
            } else {
                0.0
            };
            let metrics = merman_render::text::TextMetrics {
                width: request.style.font_size * 2.0 + family_width + bold_width + italic_width,
                height: request.style.font_size * 1.5,
                line_count: 1,
            };
            // Freeze the font oracle at both the wrapping and final emitted-text operations.
            Ok(match request.operation {
                TextMeasurementOperation::Wrapped => Some(HostTextMeasurement::Metrics(metrics)),
                TextMeasurementOperation::CanvasMeasureTextWidth
                | TextMeasurementOperation::TspanBBoxWidth => {
                    Some(HostTextMeasurement::Length(metrics.width))
                }
                TextMeasurementOperation::TspanBBoxHeight => {
                    Some(HostTextMeasurement::Length(metrics.height))
                }
                TextMeasurementOperation::CreateTextBBoxYOffset => {
                    Some(HostTextMeasurement::Length(0.0))
                }
                _ => None,
            })
        }
    }
    for (html, source_override, expected_width, expected_height) in [
        (true, false, 144.0, 68.0),
        (false, false, 75.0, 41.0),
        (true, true, 164.0, 74.0),
        (false, true, 119.0, 74.0),
    ] {
        let identity = TextMeasurementProfileIdentity::new(
            MeasurementProfileId::new("test.usecase-terminal-font").unwrap(),
            "1",
        )
        .unwrap();
        let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
            TextMeasurementPolicy::host_display(
                identity,
                Arc::new(FontHost),
                TextMeasurementPhase::ALL,
            ),
        );
        let source = if source_override {
            "usecase-beta\nA[Alpha]\nstyle A font-size:36px,font-family:monospace,font-weight:bold,font-style:italic\n"
        } else {
            "usecase-beta\nA[Alpha]\n"
        };
        let (projection, svg) = render_config_with_environment(
            source,
            json!({
                "layout": "dagre", "look": "classic", "htmlLabels": html,
                "themeVariables": {"fontSize": "14px", "fontFamily": "monospace"},
                "usecase": {"usecaseFontSize": 32, "usecaseFontFamily": "sans-serif",
                    "usecaseFontWeight": "bold", "minNodeWidth": 0, "wrappingWidth": 1000},
            }),
            environment,
        );
        let document = roxmltree::Document::parse(&svg).unwrap();
        let node = document
            .descendants()
            .find(|node| node.attribute("data-usecase-id") == Some("A"))
            .unwrap();
        if html {
            let terminal = node
                .descendants()
                .find(|node| node.has_tag_name("p"))
                .unwrap();
            assert_eq!(terminal.text(), Some("Alpha"));
            assert!(
                terminal
                    .ancestors()
                    .any(|node| node.attribute("class") == Some("nodeLabel"))
            );
            let label = terminal
                .ancestors()
                .find(|node| node.attribute("class") == Some("label"))
                .unwrap();
            let style = label.attribute("style").unwrap();
            let expected_family = if source_override {
                "monospace"
            } else {
                "sans-serif"
            };
            let expected_size = if source_override { 36 } else { 32 };
            assert!(
                style.contains(&format!("font-family:{expected_family}")),
                "{style}"
            );
            assert!(
                style.contains(&format!("font-size:{expected_size}px")),
                "{style}"
            );
            assert!(
                style.contains("font-weight:bold") || style.contains("font-weight:700"),
                "{style}"
            );
        } else {
            let terminal = node
                .descendants()
                .find(|node| node.attribute("class") == Some("text-inner-tspan"))
                .unwrap();
            assert_eq!(terminal.text(), Some("Alpha"));
            assert_eq!(terminal.attribute("font-weight"), Some("normal"));
            assert_eq!(terminal.attribute("font-style"), Some("normal"));
            if source_override {
                let text = terminal
                    .ancestors()
                    .find(|node| node.has_tag_name("text"))
                    .unwrap();
                let style = text.attribute("style").unwrap();
                assert!(style.contains("font-size:36px !important"), "{style}");
                assert!(
                    style.contains("font-family:monospace !important"),
                    "{style}"
                );
                assert!(style.contains("font-weight:bold !important"), "{style}");
                assert!(style.contains("font-style:italic !important"), "{style}");
            }
            // Neither role selector matches this SVG label or any of its ancestors.
            assert!(
                !terminal
                    .ancestors()
                    .any(|node| node.attribute("class").is_some_and(|classes| {
                        classes
                            .split_ascii_whitespace()
                            .any(|class| matches!(class, "nodeLabel" | "usecase-label"))
                    }))
            );
            let css = document
                .descendants()
                .find(|node| node.has_tag_name("style"))
                .unwrap()
                .text()
                .unwrap();
            assert!(
                css.contains("#usecase-test{font-family:monospace;font-size:14px;"),
                "{css}"
            );
        }
        let geometry = projection["layout"]["UsecaseDiagram"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|node| node["id"] == "A")
            .unwrap();
        assert_eq!(
            geometry["width"].as_f64(),
            Some(expected_width),
            "html={html}"
        );
        assert_eq!(
            geometry["height"].as_f64(),
            Some(expected_height),
            "html={html}"
        );
    }
}

#[test]
fn usecase_svg_actor_percent_font_inherits_root_before_repeated_source_style() {
    assert_usecase_svg_relative_role_font(true, "150%", 164.0, 208.0);
}

#[test]
fn usecase_svg_actor_em_font_inherits_root_before_repeated_source_style() {
    assert_usecase_svg_relative_role_font(true, "2em", 276.0, 292.0);
}

#[test]
fn usecase_svg_business_percent_font_inherits_root_before_repeated_source_style() {
    assert_usecase_svg_relative_role_font(false, "150%", 184.0, 148.0);
}

#[test]
fn usecase_svg_business_em_font_inherits_root_before_repeated_source_style() {
    assert_usecase_svg_relative_role_font(false, "2em", 296.0, 232.0);
}

fn assert_usecase_svg_relative_role_font(
    actor: bool,
    relative_size: &str,
    expected_width: f64,
    expected_height: f64,
) {
    struct FontSizeHost;
    impl HostTextMeasurer for FontSizeHost {
        fn measure(&self, request: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
            if request.text != "Alpha" {
                return Ok(None);
            }
            let metrics = merman_render::text::TextMetrics {
                width: request.style.font_size * 2.0,
                height: request.style.font_size * 1.5,
                line_count: 1,
            };
            // Relative font-size assertions use host geometry, not the built-in fallback.
            Ok(match request.operation {
                TextMeasurementOperation::Wrapped => Some(HostTextMeasurement::Metrics(metrics)),
                TextMeasurementOperation::TspanBBoxWidth => {
                    Some(HostTextMeasurement::Length(metrics.width))
                }
                TextMeasurementOperation::TspanBBoxHeight => {
                    Some(HostTextMeasurement::Length(metrics.height))
                }
                TextMeasurementOperation::CreateTextBBoxYOffset => {
                    Some(HostTextMeasurement::Length(0.0))
                }
                _ => None,
            })
        }
    }
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new("test.usecase-relative-role-font").unwrap(),
        "1",
    )
    .unwrap();
    let environment = RenderEnvironment::deterministic().with_text_measurement_policy(
        TextMeasurementPolicy::host_display(
            identity,
            Arc::new(FontSizeHost),
            TextMeasurementPhase::ALL,
        ),
    );
    let declaration = if actor {
        "actor A(\"Alpha\")"
    } else {
        "A(\"Alpha\")@{business:true}"
    };
    let source = format!("usecase-beta\n{declaration}\nstyle A font-size:{relative_size}\n");
    let (projection, svg) = render_config_with_environment(
        &source,
        json!({
            "layout": "dagre", "look": "classic", "htmlLabels": false,
            "themeVariables": {"fontSize": "32px"},
            "usecase": {"actorFontSize": 14, "usecaseFontSize": 14,
                "minNodeWidth": 0, "wrappingWidth": 1000},
        }),
        environment,
    );
    let document = roxmltree::Document::parse(&svg).unwrap();
    let node = document
        .descendants()
        .find(|node| node.attribute("data-usecase-id") == Some("A"))
        .unwrap();
    let label_class = if actor {
        "label actor-label usecase-actor-label"
    } else {
        "label usecase-label"
    };
    let group = node
        .descendants()
        .find(|node| node.has_tag_name("g") && node.attribute("class") == Some(label_class))
        .unwrap();
    let expected_style = format!("font-size:{relative_size} !important;");
    assert_eq!(group.attribute("style"), Some(expected_style.as_str()));
    let text = group
        .descendants()
        .find(|node| node.has_tag_name("text"))
        .unwrap();
    assert_eq!(text.attribute("style"), Some(expected_style.as_str()));
    let wrapper = text.parent_element().unwrap();
    assert!(wrapper.has_tag_name("g"));
    assert_eq!(wrapper.parent_element(), Some(group));
    assert_eq!(wrapper.attribute("style"), None);
    let terminal = text
        .descendants()
        .find(|node| node.attribute("class") == Some("text-inner-tspan"))
        .unwrap();
    assert_eq!(terminal.text(), Some("Alpha"));
    assert_eq!(terminal.attribute("font-weight"), Some("normal"));
    assert_eq!(terminal.attribute("font-style"), Some("normal"));
    assert_eq!(terminal.attribute("style"), None);
    let css = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .unwrap()
        .text()
        .unwrap();
    assert!(css.contains("font-size:32px;"), "{css}");
    let role = if actor { "actor-" } else { "" };
    assert!(
        css.contains(&format!("font-size:var(--mermaid-usecase-{role}font-size)")),
        "{css}"
    );
    let root_style = document.root_element().attribute("style").unwrap();
    assert!(
        root_style.contains(&format!("--mermaid-usecase-{role}font-size: 14px;")),
        "{root_style}"
    );
    // The important declaration replaces the role default on g: root 32px
    // becomes 48px (150%) or 64px (2em), then text repeats it: 72px or 128px.
    let geometry = projection["layout"]["UsecaseDiagram"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == "A")
        .unwrap();
    assert_eq!(geometry["width"].as_f64(), Some(expected_width), "{source}");
    assert_eq!(
        geometry["height"].as_f64(),
        Some(expected_height),
        "{source}"
    );
}

#[test]
fn usecase_applies_compiled_text_styles_and_root_fonts_in_both_label_modes() {
    let source = r#"usecase-beta
actor A(Reader)
B(Go)
A link@-- action --> B
classDef default color:#112233,font-size:18px
classDef custom color:#445566,font-size:22px,fill:#abcdef
class B custom
style B color:#778899
class link custom
note for B "Help"
"#;
    for html in [false, true] {
        let (_, svg) = render_config(
            source,
            json!({
                "layout":"dagre", "htmlLabels":html,
                "usecase": {"useMaxWidth":html, "actorFontFamily":"Arial", "actorFontSize":20,
                    "actorFontWeight":"bold", "usecaseFontFamily":"Verdana", "usecaseFontSize":16,
                    "usecaseFontWeight":"normal", "minNodeWidth":300},
            }),
        );
        let document = roxmltree::Document::parse(&svg).unwrap();
        let root_style = document.root_element().attribute("style").unwrap();
        assert!(root_style.contains("--mermaid-usecase-actor-font-family: Arial;"));
        assert!(root_style.contains("--mermaid-usecase-actor-font-size: 20px;"));
        assert!(root_style.contains("--mermaid-usecase-font-family: Verdana;"));
        assert!(root_style.contains("--mermaid-usecase-font-size: 16px;"));
        assert_eq!(root_style.contains("max-width:"), html);
        for (id, color, size) in [
            ("A", "#112233", 18),
            ("B", "#778899", 22),
            ("note-0", "#112233", 18),
        ] {
            let node = document
                .descendants()
                .find(|node| node.attribute("data-usecase-id") == Some(id))
                .unwrap();
            let label = node
                .descendants()
                .find(|node| node.has_tag_name(if html { "span" } else { "text" }))
                .unwrap();
            let style = label.attribute("style").unwrap();
            assert!(
                style.contains(&format!(
                    "{}:{color} !important",
                    if html { "color" } else { "fill" }
                )),
                "{id}: {style}"
            );
            assert!(
                style.contains(&format!("font-size:{size}px !important")),
                "{id}: {style}"
            );
            assert!(!style.contains("#abcdef"), "shape fill leaked into text");
        }
        let edge_label = document
            .descendants()
            .find(|node| {
                node.has_tag_name(if html { "span" } else { "text" })
                    && node
                        .descendants()
                        .any(|text| text.is_text() && text.text() == Some("action"))
            })
            .unwrap();
        assert!(
            edge_label
                .attribute("style")
                .unwrap()
                .contains("#445566 !important")
        );
        let shape = document
            .descendants()
            .find(|node| {
                node.has_tag_name("ellipse")
                    && node
                        .attribute("style")
                        .is_some_and(|style| style.contains("#abcdef"))
            })
            .unwrap();
        assert!(!shape.attribute("style").unwrap().contains("font-size"));
    }
}

#[test]
fn usecase_dagre_extracts_only_boundaries_without_external_connections() {
    let source = "usecase-beta\ndirection LR\nsystemBoundary S\nA(A)\nB(B)\nend\nA --> B";
    for (direction, tail, vertical) in [
        ("LR", "", true),
        ("TB", "", false),
        ("BT", "", true),
        ("RL", "", true),
        ("LR", "\nactor C\nC --> A", false),
        ("LR", "\nnote for A \"Help\"", false),
    ] {
        let input = format!(
            "{}{tail}",
            source.replace("direction LR", &format!("direction {direction}"))
        );
        let (projection, _) = render(&input, "dagre");
        let nodes = projection["layout"]["UsecaseDiagram"]["nodes"]
            .as_array()
            .unwrap();
        let a = nodes.iter().find(|node| node["id"] == "A").unwrap();
        let b = nodes.iter().find(|node| node["id"] == "B").unwrap();
        let axis = if vertical { "y" } else { "x" };
        assert!(
            b[axis].as_f64().unwrap() > a[axis].as_f64().unwrap(),
            "{input}: {nodes:?}"
        );
        assert_eq!(nodes.iter().filter(|node| node["id"] == "S").count(), 1);
    }
}

#[test]
fn usecase_dagre_keeps_self_loop_segments_and_helper_labels() {
    for source in [
        "usecase-beta\nA(Alpha)\nA first@-- \"first loop\" --> A",
        "usecase-beta\nsystemBoundary S[System]\nA(Alpha)\nend\nA first@-- \"first loop\" --> A",
    ] {
        let (projection, svg) =
            render_config(source, json!({"layout":"dagre", "htmlLabels":false}));
        let document = roxmltree::Document::parse(&svg).unwrap();
        let paths: Vec<_> = document
            .descendants()
            .filter(|node| {
                node.has_tag_name("path")
                    && node
                        .attribute("data-id")
                        .is_some_and(|id| id.contains("cyclic-special"))
            })
            .collect();
        assert_eq!(paths.len(), 3, "{source}");
        assert!(
            paths
                .iter()
                .any(|path| path.attribute("marker-end").is_some())
        );
        assert!(paths.iter().all(|path| path.attribute("data-id").is_some()));
        let labels: Vec<_> = document
            .descendants()
            .filter(|node| node.has_tag_name("g") && node.attribute("class") == Some("edgeLabel"))
            .collect();
        assert_eq!(
            labels
                .iter()
                .filter(|label| {
                    label
                        .descendants()
                        .filter_map(|node| node.text())
                        .collect::<String>()
                        .contains("first loop")
                })
                .count(),
            1
        );
        assert_eq!(
            projection["layout"]["UsecaseDiagram"]["edges"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
    }
}

#[test]
fn usecase_plain_labels_preserve_sanitized_break_tags_and_entities_in_both_label_modes() {
    let source = r#"usecase-beta
actor A("a<br/>b")
B("c<BR />d &lt; e &amp; f")
A edge@-- "g<br/>h" --> B
"#;
    for html in [false, true] {
        let (_, svg) = render_config(
            source,
            json!({"layout":"dagre", "htmlLabels":html, "securityLevel":"loose"}),
        );
        let document = roxmltree::Document::parse(&svg).expect("valid SVG");
        let text_content = |node: roxmltree::Node<'_, '_>| {
            node.descendants()
                .filter(|child| child.is_text())
                .filter_map(|child| child.text())
                .collect::<String>()
        };
        for (id, expected) in [("A", "a<br>b"), ("B", "c<br>d &lt; e &amp; f")] {
            let node = document
                .descendants()
                .find(|node| node.attribute("data-usecase-id") == Some(id))
                .unwrap();
            let label = node
                .descendants()
                .find(|node| node.has_tag_name(if html { "span" } else { "text" }))
                .unwrap();
            assert_eq!(text_content(label), expected, "{id}, htmlLabels={html}");
            if !html {
                assert_eq!(label.children().filter(|node| node.is_element()).count(), 1);
            }
        }
        let edge_label = document
            .descendants()
            .find(|node| node.attribute("data-id") == Some("edge") && node.has_tag_name("g"))
            .unwrap();
        assert_eq!(text_content(edge_label), "g<br>h", "htmlLabels={html}");
    }
}

#[test]
fn usecase_html_labels_reset_paragraph_margins_and_match_the_measured_line_height() {
    let (_, svg) = render_config(
        "usecase-beta\nactor A(Reader)\nA --> B(Hello)",
        json!({"layout":"dagre", "htmlLabels":true}),
    );
    let document = roxmltree::Document::parse(&svg).unwrap();
    let css: String = document
        .descendants()
        .filter(|node| node.has_tag_name("style"))
        .filter_map(|node| node.text())
        .collect();
    assert!(css.contains("#usecase-test p{margin:0;}"), "{css}");
    let labels: Vec<_> = document
        .descendants()
        .filter(|node| node.has_tag_name("foreignObject"))
        .collect();
    assert!(!labels.is_empty());
    for label in labels {
        let div = label
            .children()
            .find(|node| node.has_tag_name("div"))
            .unwrap();
        assert!(div.attribute("style").unwrap().contains("line-height:1.5"));
    }
}

#[test]
fn usecase_plain_backslash_newlines_reach_measurement_and_both_label_modes() {
    for html in [false, true] {
        let config = json!({"layout":"dagre", "htmlLabels":html});
        let (single_line, _) = render_config("usecase-beta\nA(first second)", config.clone());
        let (multiline, svg) = render_config(
            r#"usecase-beta
A("first\nsecond")"#,
            config,
        );
        let height = |projection: &serde_json::Value| {
            projection["layout"]["UsecaseDiagram"]["nodes"][0]["height"]
                .as_f64()
                .unwrap()
        };
        assert!(
            height(&multiline) > height(&single_line),
            "htmlLabels={html}"
        );
        let document = roxmltree::Document::parse(&svg).unwrap();
        let node = document
            .descendants()
            .find(|node| node.attribute("data-usecase-id") == Some("A"))
            .unwrap();
        // The accessible name is computed before display-only line-break normalization upstream.
        assert_eq!(
            node.attribute("aria-label"),
            Some(r"use case first\nsecond")
        );
        let label = node
            .descendants()
            .find(|node| node.has_tag_name(if html { "p" } else { "text" }))
            .unwrap();
        let rows: Vec<_> = if html {
            assert_eq!(
                label
                    .children()
                    .filter(|node| node.has_tag_name("br"))
                    .count(),
                1
            );
            label
                .children()
                .filter_map(|node| node.text())
                .map(str::to_owned)
                .collect()
        } else {
            label
                .children()
                .filter(|node| node.has_tag_name("tspan"))
                .map(|row| {
                    row.descendants()
                        .filter(|node| node.is_text())
                        .filter_map(|node| node.text())
                        .collect::<String>()
                })
                .collect()
        };
        assert_eq!(rows, ["first", "second"], "htmlLabels={html}");
    }
}

const MATH_LABEL_SOURCES: &[&str] = &[
    "usecase-beta\nU(\"$$x^2$$\")\n",
    "usecase-beta\nactor U(\"$$x^2$$\")\n",
    "usecase-beta\nactor U <<$$x^2$$>>\n",
    "usecase-beta\nsystemBoundary S(\"$$x^2$$\")\nend\n",
    "usecase-beta\nA -- \"$$x^2$$\" --> B\n",
    "usecase-beta\nA\nnote for A \"$$x^2$$\"\n",
    "usecase-beta\njson Data@{\"formula\":\"$$x^2$$\"}\n",
    "usecase-beta\njson Data@{\"$$x^2$$\":1}\n",
    "usecase-beta\njson Data@{\"$$x\":{\"y$$\":1}}\n",
    "usecase-beta\njson Data@{\"$$x\":[{\"y$$\":1}]}\n",
    "usecase-beta\njson Data@{\"items\":[\"plain\",\"$$x^2$$\"]}\n",
];

const MATH_LITERAL_BREAK_SOURCES: &[&str] = &[
    "usecase-beta\nU(\"$$x<br/>y$$\")\n",
    "usecase-beta\njson Data@{\"formula\":\"$$x<br/>y$$\"}\n",
    "usecase-beta\njson Data@{\"$$x<br/>y$$\":1}\n",
];

#[test]
fn usecase_math_capability_admission_matches_html_label_rendering() {
    for source in MATH_LABEL_SOURCES.iter().chain(MATH_LITERAL_BREAK_SOURCES) {
        for html_labels in [true, false] {
            let parsed = MermaidEngine::new()
                .with_site_config(PresentationConfig::from_value(json!({
                    "layout": "dagre", "htmlLabels": html_labels,
                })))
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .unwrap();
            let session = RenderEnvironment::deterministic()
                .without_math_renderer()
                .begin_session()
                .unwrap();
            let plan = family::plan_render(&parsed, &session).unwrap();
            assert_eq!(plan.is_ready(), !html_labels, "{source}");
            let missing: Vec<_> = plan.missing_capability_ids().collect();
            assert_eq!(
                missing,
                if html_labels { vec!["math"] } else { vec![] },
                "{source}"
            );
            let result = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session);
            if html_labels {
                assert!(
                    matches!(result, Err(merman_render::Error::MissingCapability { .. })),
                    "{source}"
                );
            } else {
                assert!(result.is_ok(), "SVG-only labels remain literal: {source}");
            }
        }
    }
}

#[cfg(feature = "math")]
#[test]
fn usecase_math_labels_render_real_math_for_every_label_owner() {
    for source in MATH_LABEL_SOURCES {
        let (_, svg) = render_config(source, json!({"layout":"dagre", "htmlLabels":true}));
        let document = roxmltree::Document::parse(&svg).unwrap();
        assert!(
            document.descendants().any(|node| {
                node.has_tag_name("svg")
                    && node
                        .ancestors()
                        .any(|ancestor| ancestor.has_tag_name("foreignObject"))
            }),
            "expected rendered math SVG in Usecase HTML label: {source}"
        );
    }
}

#[test]
fn usecase_json_infinities_survive_the_complete_render_pipeline() {
    let source = r#"usecase-beta
json Data@{"positive":1e309,"negative":-1e309,"actualNull":null,"nested":[1e400,-1e400]}
"#;
    for layout in ["dagre", "elk"] {
        for html_labels in [false, true] {
            let (_, svg) = render_config(
                source,
                json!({
                    "layout": layout, "look": "classic", "htmlLabels": html_labels
                }),
            );
            let doc = roxmltree::Document::parse(&svg).expect("valid Usecase SVG");
            let text = doc
                .descendants()
                .filter(|node| node.is_text())
                .filter_map(|node| node.text())
                .collect::<Vec<_>>();
            for (value, count) in [("Infinity", 2), ("-Infinity", 2), ("null", 1)] {
                assert_eq!(
                    text.iter().filter(|text| **text == value).count(),
                    count,
                    "{layout}, htmlLabels={html_labels}: {value}"
                );
            }
        }
    }
}

#[test]
fn usecase_utf16_json_preserves_distinct_keys_until_utf8_svg_output() {
    let source = r#"usecase-beta
json Data@{"\ud800":"high","\udc00":"low","�":"replacement","\\ud800":"literal","nested":{"value":"a\ud800b\udc00c"},"pair":"\ud83d\ude00","overflow":1e309}
"#;
    for backend in ["dagre", "elk"] {
        for html_labels in [false, true] {
            let (_, svg) = render_config(
                source,
                json!({"layout": backend, "htmlLabels": html_labels, "look": "classic"}),
            );
            let doc = roxmltree::Document::parse(&svg).expect("valid UTF-8 SVG");
            let texts = doc
                .descendants()
                .filter(|node| node.is_text())
                .filter_map(|node| node.text())
                .collect::<Vec<_>>();
            for expected in [
                "high",
                "low",
                "replacement",
                "literal",
                "nested.value",
                "a�b�c",
                "😀",
                "Infinity",
            ] {
                assert!(
                    texts.contains(&expected),
                    "{backend}/{html_labels}: {expected}: {texts:?}"
                );
            }
            assert_eq!(texts.iter().filter(|text| **text == "�").count(), 3);
            assert!(
                texts.contains(&r"\ud800"),
                "authored escape text is not a surrogate"
            );
        }
    }
}

#[test]
fn usecase_utf16_json_math_admission_uses_decoded_strings_and_paths() {
    for source in [
        r#"usecase-beta
json Data@{"surrogate":"\ud800","formula":"$$x^2$$"}
"#,
        r#"usecase-beta
json Data@{"surrogate":"\ud800","$$x":{"y$$":1}}
"#,
        r#"usecase-beta
json Data@{"surrogate":"\ud800","values":["$$x^2$$"]}
"#,
    ] {
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let session = RenderEnvironment::deterministic()
            .without_math_renderer()
            .begin_session()
            .unwrap();
        let plan = family::plan_render(&parsed, &session).unwrap();
        assert!(
            plan.missing_capability_ids().any(|id| id == "math"),
            "{source}"
        );
        assert!(matches!(
            family::prepare(&parsed, &LayoutOptions::default(), session),
            Err(merman_render::Error::MissingCapability { .. })
        ));
    }
}

#[test]
fn usecase_neo_applies_shared_shadow_and_stroke_rules_without_overriding_roles() {
    let (_, svg) = render_config(
        SOURCE,
        json!({
            "layout":"dagre", "theme":"default", "look":"neo", "htmlLabels":true,
            "themeVariables": {
                "nodeBorder":"#102030", "dropShadow":"drop-shadow(1px 2px 2px #b9b9b9)",
                "usecaseActorBorder":"#405060", "usecaseBorder":"#708090"
            }
        }),
    );
    let document = roxmltree::Document::parse(&svg).unwrap();
    let css: String = document
        .descendants()
        .filter(|node| node.has_tag_name("style"))
        .filter_map(|node| node.text())
        .collect();
    assert!(
        css.contains(
            r#"[data-look="neo"].node .outer-path{filter:drop-shadow(1px 2px 2px #b9b9b9);}"#
        ),
        "note outlines must receive the shared neo shadow: {css}"
    );
    assert!(css.contains(r#"[data-look="neo"].node path{stroke:#102030;stroke-width:1px;}"#));
    assert!(css.contains(".node.usecase-actor .usecase-actor-glyph path"));
    assert!(css.contains(r#"[data-look="neo"].node.usecase-element rect"#));
    assert!(css.contains(r#"[data-look="neo"].node.usecase-element .usecase-business-marker"#));
}

#[test]
fn usecase_note_renders_theme_colors_in_every_builtin_theme_and_look() {
    for theme in [
        "default",
        "base",
        "dark",
        "forest",
        "neutral",
        "neo",
        "neo-dark",
        "redux",
        "redux-dark",
        "redux-color",
        "redux-dark-color",
    ] {
        for look in ["classic", "neo", "handDrawn"] {
            let (_, svg) = render_config(
                "usecase-beta\nA(Login)\nnote for A \"Remember\"",
                json!({"layout":"dagre", "theme":theme, "look":look,
                    "themeVariables":{"noteBkgColor":"var(--note-fill)","noteBorderColor":"currentColor"}}),
            );
            let document = roxmltree::Document::parse(&svg).unwrap();
            let note = document
                .descendants()
                .find(|node| node.attribute("data-usecase-kind") == Some("note"))
                .unwrap_or_else(|| panic!("missing note for {theme}/{look}"));
            assert!(
                note.descendants().any(|node| {
                    node.has_tag_name("path") && node.attribute("fill") == Some("var(--note-fill)")
                }),
                "note fill for {theme}/{look}"
            );
            assert!(
                note.descendants().any(|node| {
                    node.has_tag_name("path") && node.attribute("stroke") == Some("currentColor")
                }),
                "note stroke for {theme}/{look}"
            );
        }
    }
}

#[test]
fn usecase_raw_palette_preserves_public_appearance_normalization() {
    for (look, node_look, edge_look) in [
        (json!(7), "neo", "neo"),
        (json!("not a token"), "neo", "neo"),
        (json!("classic"), "classic", "classic"),
    ] {
        let (_, svg) = render_config(
            "usecase-beta\nactor A\nB(Login)\nA --> B",
            json!({"layout":"dagre","theme":"redux-color","look":look,
                "usecase":{"colorScheme":"rotate"},
                "themeVariables":{"borderColorArray":[17,false],"bkgColorArray":["var(--surface)"]}}),
        );
        let doc = roxmltree::Document::parse(&svg).expect("valid raw Usecase appearance SVG");
        let actor = doc
            .descendants()
            .find(|node| node.attribute("data-usecase-id") == Some("A"))
            .unwrap();
        assert_eq!(actor.attribute("data-look"), Some(node_look));
        assert_eq!(actor.attribute("data-color-id"), Some("color-0"));
        let edge = doc
            .descendants()
            .find(|node| node.has_tag_name("path") && node.attribute("data-edge") == Some("true"))
            .unwrap();
        assert_eq!(edge.attribute("data-look"), Some(edge_look));
        let css: String = doc
            .descendants()
            .filter(|node| node.has_tag_name("style"))
            .filter_map(|node| node.text())
            .collect();
        assert!(css.contains("stroke:17;fill:var(--surface);"));
        assert!(css.contains("stroke:false;fill:var(--surface);"));
    }
}

#[test]
fn usecase_neo_gradient_paint_references_have_scoped_resources() {
    for theme in ["default", "neo", "neo-dark"] {
        let (_, svg) = render_config(
            SOURCE,
            json!({
                "layout":"dagre", "look":"neo", "theme":theme,
                "themeVariables": {
                    "useGradient":true, "gradientStart":"#102030", "gradientStop":"#405060"
                }
            }),
        );
        let document = roxmltree::Document::parse(&svg).unwrap();
        let gradient = document
            .descendants()
            .find(|node| {
                node.has_tag_name("linearGradient")
                    && node.attribute("id") == Some("usecase-test-gradient")
            })
            .expect("common neo gradient resource");
        let stops: Vec<_> = gradient
            .children()
            .filter(|node| node.has_tag_name("stop"))
            .map(|node| node.attribute("stop-color").unwrap())
            .collect();
        assert_eq!(stops, ["#102030", "#405060"]);
        let css: String = document
            .descendants()
            .filter(|node| node.has_tag_name("style"))
            .filter_map(|node| node.text())
            .collect();
        assert!(
            css.contains("stroke:url(#usecase-test-gradient)"),
            "{theme}"
        );
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn usecase_elk_cyclic_entry_follows_edges_and_respects_the_toggle() {
    for nested in [false, true] {
        let declarations = "B(B)\nA(A)\nC(C)";
        let relationships = "A --> B\nB --> C\nC --> A";
        let source = if nested {
            format!(
                "usecase-beta\ndirection LR\nsystemBoundary Group(Group)\n{declarations}\nend\n{relationships}"
            )
        } else {
            format!("usecase-beta\ndirection LR\n{declarations}\n{relationships}")
        };
        let positions = |enabled| {
            let (projection, _) = render_config(
                &source,
                json!({
                    "layout": "elk", "htmlLabels": false,
                    "elk": {
                        "cycleBreakingStrategy": "GREEDY_MODEL_ORDER",
                        "keepEntryNodeOnTop": enabled,
                    },
                }),
            );
            let nodes = projection["layout"]["UsecaseDiagram"]["nodes"]
                .as_array()
                .unwrap();
            ["A", "B", "C"].map(|id| {
                nodes.iter().find(|node| node["id"] == id).unwrap()["x"]
                    .as_f64()
                    .unwrap()
            })
        };
        let disabled = positions(false);
        let enabled = positions(true);
        assert!(disabled[1] < disabled[0], "nested={nested}: {disabled:?}");
        assert!(
            enabled[0] < enabled[1] && enabled[0] < enabled[2],
            "nested={nested}: {enabled:?}"
        );
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn usecase_elk_hops_rewrite_explicit_masks_without_moving_markers() {
    use kurbo::Shape;
    use std::collections::BTreeMap;

    let mut source = "usecase-beta\ndirection LR\n".to_owned();
    for from in ["A", "B", "C"] {
        for to in ["X", "Y", "Z"] {
            source.push_str(&format!("{from} e{from}{to}@--> {to}\n"));
            source.push_str(&format!("style e{from}{to} stroke-dasharray:0 4 100 4\n"));
        }
    }
    let paths = |look: &str, hops: serde_json::Value| {
        let (_, svg) = render_config(
            &source,
            json!({
                "layout": "elk", "look": look, "htmlLabels": false,
                "elk": {"lineHops": hops},
            }),
        );
        let document = roxmltree::Document::parse(&svg).unwrap();
        document
            .descendants()
            .filter(|node| node.has_tag_name("path") && node.attribute("data-edge") == Some("true"))
            .map(|node| {
                (
                    node.attribute("data-id").unwrap().to_owned(),
                    [
                        node.attribute("d").unwrap_or_default().to_owned(),
                        node.attribute("style").unwrap_or_default().to_owned(),
                        node.attribute("marker-start")
                            .unwrap_or_default()
                            .to_owned(),
                        node.attribute("marker-end").unwrap_or_default().to_owned(),
                        node.attribute("data-points").unwrap_or_default().to_owned(),
                    ],
                )
            })
            .collect::<BTreeMap<_, _>>()
    };
    for look in ["classic", "neo"] {
        let plain = paths(look, json!(false));
        for hops in [json!(true), json!("gap")] {
            let hopped = paths(look, hops.clone());
            let mut rewritten = 0;
            for (id, after) in &hopped {
                let before = &plain[id];
                assert_eq!(&after[2..], &before[2..], "{look}/{hops}/{id}");
                if after[1] == before[1] {
                    assert_eq!(after[0], before[0], "unmodified mask at {look}/{hops}/{id}");
                    continue;
                }
                rewritten += 1;
                let mask: Vec<f64> = after[1]
                    .split(';')
                    .find_map(|part| part.trim().strip_prefix("stroke-dasharray:"))
                    .unwrap()
                    .split_whitespace()
                    .map(|number| number.parse().unwrap())
                    .collect();
                assert_eq!(mask.len(), 4);
                assert_eq!([mask[0], mask[1], mask[3]], [0.0, 4.0, 4.0]);
                // getTotalLength() returns an SVG DOM float before marker-offset arithmetic.
                let length =
                    f64::from(kurbo::BezPath::from_svg(&after[0]).unwrap().perimeter(1e-6) as f32);
                assert!((mask[2] - (length - 8.0).max(0.0)).abs() < 1e-9);
            }
            assert!(rewritten > 0, "crossing masks at {look}/{hops}");
        }
    }
}
