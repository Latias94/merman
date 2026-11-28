use merman_core::{MermaidEngine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::{
    HostMeasurementResult, HostTextMeasurement, HostTextMeasurementRequest, HostTextMeasurer,
    MeasurementProfileId, RenderEnvironment, RenderSession, TextMeasurementOperation,
    TextMeasurementPhase, TextMeasurementPolicy, TextMeasurementProfileIdentity,
    TextMeasurementResultKind,
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

fn render(source: &str, layout: &str) -> (merman_model::ManagedJson, String) {
    render_config(
        source,
        json!({
            "layout": layout, "theme": "default", "look": "classic", "htmlLabels": false,
        }),
    )
}

#[test]
fn usecase_relationship_labels_use_mermaid_12_3_stereotypes() {
    let (_, svg) = render(
        "usecase-beta\nA[Base]\nB[Included]\nC[Extension]\nA ..> : include B\nC ..> : extend A\n",
        "dagre",
    );
    assert!(svg.contains("«include»"), "include label is missing");
    assert!(svg.contains("«extend»"), "extend label is missing");
}

fn render_config(source: &str, config: serde_json::Value) -> (merman_model::ManagedJson, String) {
    render_config_with_environment(source, config, RenderEnvironment::deterministic())
}

fn render_config_with_environment(
    source: &str,
    config: serde_json::Value,
    environment: RenderEnvironment,
) -> (merman_model::ManagedJson, String) {
    let session = environment.begin_session().expect("render session");
    render_config_with_session(source, config, session)
}

fn render_config_with_session(
    source: &str,
    config: serde_json::Value,
    session: RenderSession,
) -> (merman_model::ManagedJson, String) {
    let engine = MermaidEngine::new()
        .with_site_config(merman_core::presentation::SourcePresentationConfig::from_value(config));
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse Usecase")
        .expect("detect Usecase")
        .into_presentation();
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
fn usecase_elk_spreads_actor_ports_for_static_neo_markers() {
    const SOURCE: &str = "usecase-beta
direction TB
actor Hub(H)
A[One]
B[Two]
C[Three]
D[Four]
A first@--> Hub
B second@--> Hub
C third@--> Hub
D fourth@--> Hub
";
    const ANIMATION: &str = "first@{animate:true}
second@{animate:true}
third@{animate:true}
fourth@{animate:true}
";

    let port_span = |source: &str| {
        let (projection, _) = render_config(
            source,
            json!({"layout": "elk", "look": "neo", "htmlLabels": false,
                "elk": {"orientFeedbackEdges": false}}),
        );
        let graph = &projection["layout"]["UsecaseDiagram"];
        let mut ports: Vec<_> = graph["edges"]
            .as_array()
            .unwrap()
            .iter()
            .map(|edge| {
                assert_eq!(edge["to"], "Hub");
                edge["points"].as_array().unwrap().last().unwrap()["x"]
                    .as_f64()
                    .unwrap()
            })
            .collect();
        assert_eq!(ports.len(), 4);
        ports.sort_by(f64::total_cmp);
        ports[3] - ports[0]
    };
    // Four incoming points are crowded on this actor only with Neo's wider margins.
    let static_span = port_span(SOURCE);
    let animated_span = port_span(&format!("{SOURCE}{ANIMATION}"));
    assert!(
        static_span > animated_span + 1.0,
        "{static_span} vs {animated_span}"
    );
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
fn usecase_html_inline_rem_uses_the_document_root_and_em_uses_the_role_font() {
    fn label_boxes(svg: &str) -> Vec<(f64, f64)> {
        roxmltree::Document::parse(svg)
            .unwrap()
            .descendants()
            .filter(|node| {
                node.has_tag_name("foreignObject")
                    && node
                        .descendants()
                        .filter(|child| child.is_text())
                        .filter_map(|child| child.text())
                        .collect::<String>()
                        == "ABCDEFGHIJKLMN"
            })
            .map(|node| {
                (
                    node.attribute("width").unwrap().parse().unwrap(),
                    node.attribute("height").unwrap().parse().unwrap(),
                )
            })
            .collect()
    }

    for role_size in [12, 24] {
        let config = json!({
            "layout": "dagre", "htmlLabels": true,
            "themeVariables": {"fontSize": format!("{role_size}px")},
            "usecase": {
                "usecaseFontSize": role_size, "actorFontSize": role_size,
                "wrappingWidth": 1000, "minNodeWidth": 0
            }
        });
        for actor in [false, true] {
            for source_style in [
                "style A font-size:SIZE",
                "classDef large font-size:SIZE\nclass A large",
            ] {
                let declaration = if actor {
                    "actor A(ABCDEFGHIJKLMN)"
                } else {
                    "A[ABCDEFGHIJKLMN]"
                };
                let source = format!("usecase-beta\n{declaration}\n{source_style}\n");
                let (_, actual) = render_config(&source.replace("SIZE", "2rem"), config.clone());
                let (_, expected) = render_config(&source.replace("SIZE", "32px"), config.clone());
                let boxes = label_boxes(&actual);
                assert_eq!(boxes.len(), 1, "actor={actor}, {source_style}: {actual}");
                assert_eq!(boxes, label_boxes(&expected), "{source}");
                assert_eq!(boxes[0].1, 48.0, "{source}");
            }
            for (relative, pixels) in [("2rem", 32), ("2em", role_size * 2)] {
                let source = |size: &str| {
                    let label =
                        format!("\"`<span style='font-size:{size}'>ABCDEFGHIJKLMN</span>`\"");
                    if actor {
                        format!("usecase-beta\nactor A({label})\n")
                    } else {
                        format!("usecase-beta\nA[{label}]\n")
                    }
                };
                let (_, actual) = render_config(&source(relative), config.clone());
                let (_, expected) = render_config(&source(&format!("{pixels}px")), config.clone());
                let boxes = label_boxes(&actual);
                assert_eq!(boxes.len(), 1, "actor={actor}, {relative}: {actual}");
                assert_eq!(
                    boxes,
                    label_boxes(&expected),
                    "actor={actor}, role_size={role_size}, size={relative}"
                );
                assert_eq!(boxes[0].1, f64::from(pixels) * 1.5);
                assert!(actual.contains(&format!("font-size:{relative}")));
            }
        }
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
                .with_site_config(
                    merman_core::presentation::SourcePresentationConfig::from_value(json!({
                        "layout": "dagre", "htmlLabels": html_labels,
                    })),
                )
                .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
                .unwrap()
                .unwrap()
                .into_presentation();
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
            .unwrap()
            .into_presentation();
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

mod theme {
    use super::*;
    use merman_render::environment::TextMeasurementProfile;
    use merman_render::text::{TextMeasurer, TextMetrics, TextStyle};
    use merman_render::theme::{BuiltinPreset, NativeTheme, ThemeCompiler};
    use std::sync::Mutex;

    #[derive(Default)]
    struct ObservedFonts(Mutex<Vec<(String, TextStyle)>>);

    impl TextMeasurer for ObservedFonts {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            self.0
                .lock()
                .unwrap()
                .push((text.to_owned(), style.clone()));
            TextMetrics {
                width: text.chars().count() as f64 * style.font_size,
                height: style.font_size,
                line_count: 1,
            }
        }
    }

    fn stylesheet_property(
        document: &roxmltree::Document<'_>,
        selector: &str,
        key: &str,
    ) -> Option<String> {
        document
            .descendants()
            .filter(|node| node.has_tag_name("style"))
            .filter_map(|node| node.text())
            .flat_map(|css| css.split('}'))
            .filter_map(|rule| rule.split_once('{'))
            .filter(|(selectors, _)| selectors.split(',').any(|value| value.trim() == selector))
            .flat_map(|(_, declarations)| declarations.split(';'))
            .filter_map(|declaration| declaration.split_once(':'))
            .filter(|(name, _)| name.trim() == key)
            .map(|(_, value)| value.trim().to_owned())
            .next_back()
    }

    #[test]
    fn usecase_absolute_generic_size_drives_geometry_and_host_measurement() {
        let mut geometries = Vec::new();
        for size in ["24pt", "32px"] {
            let source = format!(
                "---\nconfig:\n  theme: base\n  layout: dagre\n  look: classic\n  htmlLabels: false\n  themeVariables:\n    fontSize: {size}\n  usecase:\n    minNodeWidth: 0\n    wrappingWidth: 1000\n---\nusecase-beta\nA(Alpha)\n"
            );
            let fonts = Arc::new(ObservedFonts::default());
            let profile = TextMeasurementProfile::new(
                TextMeasurementProfileIdentity::new(
                    MeasurementProfileId::new("test.usecase-absolute-size").unwrap(),
                    "1",
                )
                .unwrap(),
                fonts.clone(),
            );
            let (layout, svg) = render_config_with_environment(
                &source,
                json!({}),
                RenderEnvironment::deterministic()
                    .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile)),
            );
            let measurements = fonts.0.lock().unwrap();
            let labels = measurements
                .iter()
                .filter(|(text, _)| text == "Alpha")
                .collect::<Vec<_>>();
            assert!(
                !labels.is_empty(),
                "{size}: production label was not measured"
            );
            assert!(
                labels.iter().all(|(_, style)| style.font_size == 32.0),
                "{size}: {labels:?}"
            );
            let geometry = &layout["layout"]["UsecaseDiagram"];
            assert_eq!(
                geometry["nodes"][0]["width"].as_f64(),
                Some(200.0),
                "{size}"
            );
            let document = roxmltree::Document::parse(&svg).unwrap();
            assert_eq!(
                stylesheet_property(&document, "#usecase-test", "font-size").as_deref(),
                Some("32px"),
                "{size}"
            );
            assert!(
                document
                    .descendants()
                    .any(|node| node.text() == Some("Alpha"))
            );
            geometries.push(geometry.clone());
        }
        assert_eq!(geometries[0], geometries[1]);
    }

    #[test]
    fn usecase_source_property_spelling_survives_svg_and_html() {
        // A hyphenated mixed-case name is a CSS_IDENTIFIER in the pinned grammar.
        let source = "usecase-beta\nA(Alpha)\nstyle A stroke:#ccddee,stroke-WIDTH:3px\n";
        for html in [false, true] {
            let (_, svg) = render_config(
                source,
                json!({"layout":"dagre", "look":"classic", "htmlLabels":html}),
            );
            let document = roxmltree::Document::parse(&svg).unwrap();
            let ellipse = document
                .descendants()
                .find(|node| node.has_tag_name("ellipse"))
                .unwrap();
            assert_eq!(
                ellipse.attribute("style"),
                Some("stroke:#ccddee !important;stroke-WIDTH:3px !important;")
            );
        }
    }

    #[test]
    fn usecase_native_theme_keeps_boundary_strokes_with_site_neo() {
        let source = r#"usecase-beta
systemBoundary Plain(Plain)@{type:package}
A(Alpha)
end
systemBoundary Branded(Branded)
B(Beta)
end
style Branded stroke:#123456
"#;
        let theme = ThemeCompiler::new()
            .compile_preset(BuiltinPreset::Studio, None)
            .unwrap();
        let border = theme.roles().stroke_border.first_color().to_string();
        let session = RenderEnvironment::deterministic()
            .begin_session_with_native_theme(NativeTheme::new(theme))
            .unwrap();
        let (_, svg) = render_config_with_session(
            source,
            json!({"layout":"dagre", "theme":"neo", "look":"neo", "htmlLabels":false}),
            session,
        );
        let document = roxmltree::Document::parse(&svg).unwrap();
        assert_eq!(
            stylesheet_property(
                &document,
                "#usecase-test .system-boundary rect.boundary-body",
                "stroke"
            )
            .as_deref(),
            Some(border.as_str())
        );
        assert_eq!(
            stylesheet_property(
                &document,
                r#"#usecase-test [data-look="neo"].cluster rect"#,
                "stroke"
            ),
            None
        );
        for (id, style) in [("Plain", ""), ("Branded", "stroke:#123456 !important;")] {
            let boundary = document
                .descendants()
                .find(|node| node.attribute("data-usecase-id") == Some(id))
                .unwrap();
            assert_eq!(boundary.attribute("data-look"), Some("neo"));
            let body = boundary
                .children()
                .find(|node| {
                    node.has_tag_name("rect")
                        && node.attribute("class").is_some_and(|classes| {
                            classes
                                .split_ascii_whitespace()
                                .any(|class| class == "boundary-body")
                        })
                })
                .unwrap();
            assert_eq!(body.attribute("style"), Some(style));
        }
    }

    #[test]
    fn usecase_native_theme_styles_roles_and_keeps_source_paint() {
        const SOURCE: &str = r#"---
title: Usecase title
---
usecase-beta
direction LR
actor User(Customer)
systemBoundary Auth(Authentication)@{type:package}
Login(Signin)
Verify[Check]
end
User access@-- "Access" --> Login
Login ..> : include Verify
note for Login "Remember"
style Login fill:#abcdef
"#;
        let theme = ThemeCompiler::new()
            .compile_json(
                br##"{
                    "schema_version": 2, "extends": "studio",
                    "roles": {
                        "surface.node": "#DCE8F4", "surface.container": "#E7DFF0",
                        "surface.callout": "#F9E4B2", "stroke.connector": "#234567"
                    },
                    "typography": {"sans": ["Usecase Text", "sans-serif"], "base_size_px": 19},
                    "form": {"elevation": {"kind": "layered_stack", "layers": [
                        {"dx": 3, "dy": 5, "blur": 0, "color": "#1A2B3C"}
                    ]}}
                }"##,
                None,
            )
            .unwrap();
        for html in [false, true] {
            let fonts = Arc::new(ObservedFonts::default());
            let profile = TextMeasurementProfile::new(
                TextMeasurementProfileIdentity::new(
                    MeasurementProfileId::new("test.usecase-native-theme").unwrap(),
                    "1",
                )
                .unwrap(),
                fonts.clone(),
            );
            let session = RenderEnvironment::deterministic()
                .with_text_measurement_policy(TextMeasurementPolicy::uniform(profile))
                .begin_session_with_native_theme(NativeTheme::new(theme.clone()))
                .unwrap();
            let (_, svg) = render_config_with_session(
                SOURCE,
                json!({"layout":"dagre", "look":"classic", "htmlLabels":html}),
                session,
            );
            let document = roxmltree::Document::parse(&svg).unwrap();
            for (selector, property, value) in [
                (".usecase-element ellipse", "fill", "#DCE8F4"),
                (".system-boundary rect.boundary-body", "fill", "#E7DFF0"),
                (".relationship", "stroke", "#234567"),
                (".marker", "stroke", "#234567"),
                (
                    ".usecaseDiagramTitleText",
                    "font-family",
                    "\"Usecase Text\", sans-serif",
                ),
            ] {
                assert_eq!(
                    stylesheet_property(&document, &format!("#usecase-test {selector}"), property)
                        .as_deref(),
                    Some(value),
                    "{selector} in HTML mode {html}"
                );
            }
            let login = document
                .descendants()
                .find(|node| node.attribute("data-usecase-id") == Some("Login"))
                .unwrap();
            let body = login
                .descendants()
                .find(|node| node.has_tag_name("ellipse"))
                .unwrap();
            assert_eq!(body.attribute("style"), Some("fill:#abcdef !important;"));
            assert_eq!(
                login
                    .descendants()
                    .any(|node| node.has_tag_name("foreignObject")),
                html
            );
            assert!(
                document
                    .descendants()
                    .any(|node| node.has_tag_name("path")
                        && node.attribute("fill") == Some("#F9E4B2"))
            );
            let depth: Vec<_> = document
                .descendants()
                .filter(|node| node.attribute("data-merman-theme-depth") == Some("node"))
                .collect();
            assert_eq!(depth.len(), 2);
            assert!(depth.iter().all(|node| {
                node.children().any(|child| {
                    child.has_tag_name("path") && child.attribute("fill") == Some("#1A2B3C")
                })
            }));
            assert!(
                fonts
                    .0
                    .lock()
                    .unwrap()
                    .iter()
                    .any(|(text, style)| text == "Signin"
                        && style.font_size == 19.0
                        && style.font_family.as_deref() == Some("\"Usecase Text\", sans-serif"))
            );
        }
    }
}

#[cfg(feature = "layout-shoal")]
#[test]
fn usecase_shoal_centers_non_html_ellipse_note_and_json_titles() {
    let source = r#"usecase-beta
A(Run Real-Time AML Screening)
note for A "PCI DSS scope"
json Config@{"rotationDays":30,"algorithm":"AES-256-GCM"}
"#;
    let (_, svg) = render_config(
        source,
        json!({"layout": "shoal", "look": "classic", "htmlLabels": false}),
    );
    let document = roxmltree::Document::parse(&svg).unwrap();
    for (id, class) in [
        ("A", "label"),
        ("note-0", "label noteLabel"),
        ("Config", "label usecase-json-title"),
    ] {
        let node = document
            .descendants()
            .find(|node| node.attribute("data-usecase-id") == Some(id))
            .unwrap();
        let label = node
            .descendants()
            .find(|label| label.attribute("class") == Some(class))
            .unwrap();
        assert!(
            label
                .attribute("transform")
                .unwrap()
                .starts_with("translate(0,")
        );
        let text = label
            .descendants()
            .find(|node| node.has_tag_name("text"))
            .unwrap();
        assert_eq!(text.attribute("text-anchor"), Some("middle"));
    }
}

#[cfg(feature = "layout-shoal")]
#[test]
fn usecase_shoal_business_nodes_share_typography_and_json_text_has_no_stroke() {
    let (_, svg) = render_config(
        "usecase-beta\nA(Review the access request)\nB(Review the access request)@{business:true}\njson Claims@{\"role\":\"editor\"}\n",
        json!({"layout": "shoal", "theme": "forest", "htmlLabels": false}),
    );
    let doc = roxmltree::Document::parse(&svg).unwrap();
    let node = |id| {
        doc.descendants()
            .find(|node| node.attribute("data-usecase-id") == Some(id))
            .unwrap()
    };
    let label_style = |id| {
        node(id)
            .descendants()
            .find(|child| child.has_tag_name("text"))
            .unwrap()
            .attribute("style")
            .unwrap()
    };
    assert_eq!(label_style("A"), label_style("B"));
    assert!(label_style("B").contains("font-size:16px !important"));
    assert!(
        node("B")
            .children()
            .any(|child| child.attribute("class") == Some("usecase-business-marker"))
    );
    assert!(
        node("Claims")
            .descendants()
            .any(|child| child.has_tag_name("rect")
                && child.attribute("class") == Some("usecase-json-cell usecase-json-value-cell"))
    );
    let css = doc
        .descendants()
        .find(|child| child.has_tag_name("style"))
        .unwrap()
        .text()
        .unwrap();
    assert!(css.contains(".usecase-json-table text{stroke:none}"));
    assert!(css.contains(".usecase-json-cell{fill:"));
}

#[cfg(feature = "layout-shoal")]
#[test]
fn usecase_shoal_actor_paints_the_resolved_font_with_a_centered_caption() {
    let source = "usecase-beta\nactor Agent(\"Support Agent\")\n";
    let (_, svg) = render_config(
        source,
        json!({"layout": "shoal", "htmlLabels": false, "usecase": {"actorFontSize": 23}}),
    );
    let document = roxmltree::Document::parse(&svg).unwrap();
    let text = document
        .descendants()
        .find(|node| {
            node.has_tag_name("text")
                && node.ancestors().any(|parent| {
                    parent.attribute("class") == Some("label actor-label usecase-actor-label")
                })
        })
        .unwrap();
    assert_eq!(text.attribute("text-anchor"), Some("middle"));
    assert!(
        text.attribute("style")
            .unwrap()
            .contains("font-size:23px !important")
    );
}

#[cfg(feature = "layout-shoal")]
#[test]
fn usecase_shoal_preserves_authored_actor_outlines_and_adapts_the_forest_default() {
    let source = "usecase-beta\nactor A(Developer)\n";
    for (variables, expected) in [
        (json!({}), "#13540c"),
        (json!({"actorBorder": "#ff0000"}), "#ff0000"),
    ] {
        let (_, svg) = render_config(
            source,
            json!({"layout": "shoal", "theme": "forest", "themeVariables": variables}),
        );
        let css = format!(".usecase-actor-glyph{{fill:#cde498;stroke:{expected};");
        assert!(svg.contains(&css), "{svg}");
    }
}

#[cfg(feature = "layout-shoal")]
#[test]
fn usecase_shoal_actor_contacts_clear_the_stick_figure_and_caption() {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use merman_render::model::LayoutPoint;

    let translation = |node: roxmltree::Node<'_, '_>| {
        let pair = node
            .attribute("transform")
            .unwrap()
            .strip_prefix("translate(")
            .unwrap()
            .strip_suffix(')')
            .unwrap();
        let (x, y) = pair.split_once(',').unwrap();
        (
            x.trim().parse::<f64>().unwrap(),
            y.trim().parse::<f64>().unwrap(),
        )
    };
    for (variant, side_contact) in [("normal", 21.0), ("hollow", 26.0)] {
        for (direction, relation) in [
            ("LR", "A --> B"),
            ("RL", "A --> B"),
            ("TB", "A --> B"),
            ("BT", "B --> A"),
        ] {
            let (_, svg) = render_config(
                &format!(
                    "usecase-beta\ndirection {direction}\nactor A(Buyer)@{{type:{variant}}}\nB(Cart)\n{relation}\n"
                ),
                json!({"layout": "shoal", "look": "classic", "htmlLabels": true}),
            );
            let doc = roxmltree::Document::parse(&svg).unwrap();
            let actor = doc
                .descendants()
                .find(|node| node.attribute("data-usecase-id") == Some("A"))
                .unwrap();
            let (x, y) = translation(actor);
            let edge = doc
                .descendants()
                .find(|node| node.attribute("data-et") == Some("edge"))
                .unwrap();
            let points: Vec<LayoutPoint> = serde_json::from_slice(
                &STANDARD
                    .decode(edge.attribute("data-points").unwrap())
                    .unwrap(),
            )
            .unwrap();
            let contact = if direction == "BT" {
                points.last().unwrap()
            } else {
                &points[0]
            };
            if matches!(direction, "LR" | "RL") {
                let offset = if direction == "LR" {
                    side_contact
                } else {
                    -side_contact
                };
                assert!(
                    (contact.x - x - offset).abs() < 1e-6,
                    "{variant} {direction}: {contact:?}"
                );
            } else {
                let label = actor
                    .descendants()
                    .find(|node| node.has_tag_name("foreignObject"))
                    .unwrap();
                let bottom = y
                    + translation(label.parent().unwrap()).1
                    + label.attribute("height").unwrap().parse::<f64>().unwrap();
                assert!(
                    (contact.y - bottom - 5.0).abs() < 1e-6,
                    "{variant} {direction}: {contact:?}"
                );
            }
        }
    }
}

#[cfg(feature = "layout-shoal")]
#[test]
fn usecase_shoal_hand_drawn_paints_shells_glyphs_and_relationships() {
    use kurbo::{BezPath, Rect, Shape};

    let (_, svg) = render_config(
        &format!("{SOURCE}\nstyle User opacity:0.5,stroke-width:4px\n"),
        json!({"layout": "shoal", "look": "handDrawn", "handDrawnSeed": 17,
            "htmlLabels": false, "usecase": {"diagramPadding": 0}}),
    );
    let doc = roxmltree::Document::parse(&svg).unwrap();
    let viewbox: Vec<f64> = doc
        .root_element()
        .attribute("viewBox")
        .unwrap()
        .split_whitespace()
        .map(|n| n.parse().unwrap())
        .collect();
    let viewport = Rect::from_origin_size((viewbox[0], viewbox[1]), (viewbox[2], viewbox[3]));
    for class in [
        "boundary-body",
        "boundary-tab",
        "usecase-actor-hollow-head",
        "usecase-actor-hollow-body",
        "usecase-business-ellipse",
        "outer-path",
        "usecase-json-border",
    ] {
        let shape = doc
            .descendants()
            .find(|node| {
                node.attribute("class").is_some_and(|classes| {
                    classes
                        .split_whitespace()
                        .any(|part| part == "usecase-sketch-shape")
                        && classes.split_whitespace().any(|part| part == class)
                })
            })
            .unwrap_or_else(|| panic!("missing hand-drawn {class}"));
        let stroke = shape.last_element_child().unwrap();
        assert_eq!(stroke.attribute("fill"), Some("none"));
        let d = stroke.attribute("d").unwrap();
        assert!(
            d.contains('C') && d.matches('M').count() > 1,
            "{class}: {d}"
        );
        if class.starts_with("boundary-") {
            let paint = BezPath::from_svg(d)
                .unwrap()
                .bounding_box()
                .inflate(0.5, 0.5);
            assert!(viewport.inflate(1e-6, 1e-6).contains_rect(paint));
        }
        if class.starts_with("usecase-actor-") {
            let glyph = shape.parent_element().unwrap();
            let style = glyph.attribute("style").unwrap();
            assert!(style.contains("opacity:0.5"), "{style}");
            assert!(style.contains("stroke-width:4px"), "{style}");
            assert!(shape.attribute("style").is_none());
        }
    }
    assert!(
        doc.descendants()
            .filter(|node| node.attribute("data-et") == Some("edge"))
            .all(|node| {
                node.attribute("data-look") == Some("handDrawn")
                    && node.attribute("d").unwrap().matches('M').count() > 1
            })
    );
}

#[cfg(any(feature = "layout-dagre", feature = "layout-shoal"))]
#[test]
fn usecase_svg_accepts_exact_output_byte_limit() {
    use merman_render::{
        Error,
        resources::{RenderResourcePolicy, ResourceLimitId, ResourceLimitPhase},
    };

    let source = "usecase-beta\nA(\"Login\") <<Human>>\nA ..> : include B\n";
    for backend in [
        #[cfg(feature = "layout-dagre")]
        "dagre",
        #[cfg(feature = "layout-shoal")]
        "shoal",
    ] {
        let engine = MermaidEngine::new().with_site_config(
            merman_core::presentation::SourcePresentationConfig::from_value(
                json!({"layout":backend}),
            ),
        );
        let parsed = engine
            .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let render = |policy| {
            let session = RenderEnvironment::deterministic()
                .with_resource_policy(policy)
                .begin_session()
                .unwrap();
            family::prepare(
                parsed.presentation(),
                &LayoutOptions::headless_svg_defaults(),
                session,
            )?
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .map(|svg| svg.svg().to_owned())
        };
        let policy = RenderResourcePolicy::unbounded_for_trusted_input();
        let baseline = render(policy).unwrap();
        let exact = policy
            .with_limit(ResourceLimitId::MaxSvgBytes, baseline.len())
            .unwrap();
        assert_eq!(render(exact).unwrap(), baseline, "{backend}");
        let below = policy
            .with_limit(ResourceLimitId::MaxSvgBytes, baseline.len() - 1)
            .unwrap();
        let Error::ResourceLimitExceeded(limit) = render(below).unwrap_err() else {
            panic!("{backend}: expected an incremental MaxSvgBytes limit");
        };
        assert_eq!(limit.limit, ResourceLimitId::MaxSvgBytes.as_str());
        assert_eq!(limit.phase, ResourceLimitPhase::SvgOutput);
    }
}

#[cfg(feature = "layout-shoal")]
#[test]
fn usecase_shoal_decodes_plain_entities_once_for_labels_and_accessibility() {
    let source = r#"usecase-beta
direction TB
actor Dev("Developer") <<Human>>
systemBoundary Auth("Auth &amp; SSO")@{type:package}
  Login("Sign in &amp; verify &amp;lt;MFA&amp;gt;")@{business:true}
end
json Claims@{"literal":"&amp;"}
Dev -- status &amp; security --> Login
Claims -- Login
"#;
    for html_labels in [false, true] {
        let (_, svg) = render_config(
            source,
            json!({"layout": "shoal", "htmlLabels": html_labels}),
        );
        let doc = roxmltree::Document::parse(&svg).unwrap();
        let find = |id| {
            doc.descendants()
                .find(|node| node.attribute("data-usecase-id") == Some(id))
                .unwrap()
        };
        let visible = |node: roxmltree::Node<'_, '_>| -> String {
            node.descendants()
                .filter(roxmltree::Node::is_text)
                .filter_map(|node| node.text())
                .collect()
        };
        let login = find("Login");
        let expected = "Sign in & verify &lt;MFA&gt;";
        assert!(visible(login).contains(expected), "{}", visible(login));
        assert_eq!(
            login.attribute("aria-label").unwrap(),
            format!("business use case {expected}")
        );
        assert!(visible(find("Auth")).contains("Auth & SSO"));
        assert!(visible(find("Claims")).contains("&amp;"));
        assert!(visible(doc.root_element()).contains("status & security"));
        if !html_labels {
            let stereotype = find("Dev")
                .descendants()
                .find(|n| n.attribute("class") == Some("label usecase-stereotype"))
                .unwrap();
            assert!(stereotype.descendants().any(|n| n.has_tag_name("text") && n.attribute("text-anchor") == Some("middle")));
        }
    }
}

#[cfg(feature = "layout-shoal")]
#[test]
fn usecase_shoal_keeps_associations_clear_of_the_package_tab() {
    use kurbo::{BezPath, ParamCurve, Rect};

    let source = r#"usecase-beta
direction TB
actor Customer("Retail Customer")@{type:hollow}
actor Support("Support Agent")@{business:true}
systemBoundary Checkout("Checkout Subsystem")@{type:package}
  PlaceOrder("Place Order")@{business:true}
  Pay("Process Payment")
  Fraud("Run Fraud Check")
end
systemBoundary Fulfillment("Fulfillment Center")
  Ship("Dispatch Shipment")
  Refund("Issue Refund")@{business:true}
end
Customer --> PlaceOrder
PlaceOrder ..> : include Pay
Fraud ..> : extend Pay
PlaceOrder ..> : include Ship
Support --> Refund
Refund ..> : include Pay
note for Pay "PCI DSS scope"
"#;
    let native = merman_render::theme::ThemeCompiler::new()
        .compile_preset(merman_render::theme::BuiltinPreset::Studio, None)
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_native_theme(merman_render::theme::NativeTheme::new(native))
        .unwrap();
    let (_, svg) = render_config_with_session(
        source,
        json!({"layout":"shoal", "htmlLabels":false}),
        session,
    );
    let document = roxmltree::Document::parse(&svg).unwrap();
    let tab = document
        .descendants()
        .find(|node| {
            node.has_tag_name("rect")
                && node
                    .attribute("class")
                    .unwrap_or_default()
                    .starts_with("boundary-tab ")
        })
        .unwrap();
    let number = |key| tab.attribute(key).unwrap().parse::<f64>().unwrap();
    let body = tab
        .parent()
        .unwrap()
        .children()
        .find(|node| node.attribute("class") == Some("boundary-body label-container"))
        .unwrap();
    assert_eq!(tab.attribute("x"), body.attribute("x"));
    assert_eq!(tab.attribute("y"), body.attribute("y"));
    let tab = Rect::from_origin_size(
        (number("x"), number("y")),
        (number("width"), number("height")),
    );
    let edge = document
        .descendants()
        .find(|node| node.attribute("data-id") == Some("edge-0") && node.has_tag_name("path"))
        .unwrap();
    let path = BezPath::from_svg(edge.attribute("d").unwrap()).unwrap();
    assert!(
        path.segments()
            .all(|segment| (0..=100).all(|i| !tab.contains(segment.eval(f64::from(i) / 100.0)))),
        "{path:?} crosses {tab:?}"
    );
    let css = document
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .and_then(|node| node.text())
        .unwrap();
    assert!(css.contains(".actor-label text{color:"));
    assert!(css.contains(".usecase-note .noteLabel text{color:"));
    assert!(!css.contains(".usecase-actor-hollow{"));
}

#[cfg(feature = "layout-shoal")]
#[test]
fn usecase_shoal_reads_root_spacing_before_family_defaults() {
    let source = "usecase-beta\ndirection TB\nA --> B\nA --> C\n";
    let gap = |spacing: serde_json::Value| {
        let mut config = json!({"layout":"shoal"});
        config
            .as_object_mut()
            .unwrap()
            .extend(spacing.as_object().unwrap().clone());
        let (layout, _) = render_config(source, config);
        let nodes = layout["layout"]["UsecaseDiagram"]["nodes"]
            .as_array()
            .unwrap();
        let x = |id| {
            nodes.iter().find(|node| node["id"] == id).unwrap()["x"]
                .as_f64()
                .unwrap()
        };
        (x("B") - x("C")).abs()
    };
    let nested = gap(json!({"usecase":{"nodeSpacing":120}}));
    assert_eq!(
        gap(json!({"nodeSpacing":120, "usecase":{"nodeSpacing":20}})),
        nested
    );
    assert!(nested > gap(json!({"usecase":{"nodeSpacing":20}})) + 90.0);
}

#[cfg(feature = "layout-shoal")]
#[test]
fn usecase_native_theme_paints_sketch_with_a_cutout_on_the_labeled_edge() {
    use kurbo::{BezPath, ParamCurve, Point, Shape};
    use merman_render::theme::{NativeTheme, ThemeCompiler};

    let source = "usecase-beta\ndirection LR\nA -- status --> B\nC --> D\n";
    for (style, has_curves) in [("orthogonal", false), ("sketch", true)] {
        let theme = ThemeCompiler::new().compile_json(
            format!(r#"{{"schema_version":2,"extends":"paper","form":{{"edge_style":"{style}","marker_style":"open","edge_label_bg":"transparent"}}}}"#).as_bytes(),
            None,
        ).unwrap();
        let session = RenderEnvironment::deterministic()
            .begin_session_with_native_theme(NativeTheme::new(theme))
            .unwrap();
        let (layout, svg) = render_config_with_session(
            source,
            json!({"layout":"shoal", "htmlLabels":false, "look":"neo", "themeVariables":{"useGradient":true}}),
            session,
        );
        let document = roxmltree::Document::parse(&svg).unwrap();
        assert!(!svg.contains("url(#usecase-test-gradient)"));
        let edges: Vec<_> = document
            .descendants()
            .filter(|node| node.attribute("data-edge") == Some("true"))
            .collect();
        let [edge, unlabeled] = edges[..] else {
            panic!("expected two relationships");
        };
        assert_eq!(
            edge.attribute("d").unwrap().contains('C'),
            has_curves,
            "{style}"
        );
        let marker = edge
            .attribute("marker-end")
            .unwrap()
            .strip_prefix("url(#")
            .unwrap()
            .strip_suffix(')')
            .unwrap();
        let marker = document
            .descendants()
            .find(|node| node.attribute("id") == Some(marker))
            .unwrap();
        assert!(
            marker
                .first_element_child()
                .unwrap()
                .attribute("style")
                .unwrap()
                .contains("fill:none")
        );
        assert_eq!(edge.parent_element().unwrap().attribute("mask"), None);
        assert_eq!(unlabeled.attribute("clip-path"), None);
        let clip_id = "usecase-test_usecase-edge-label-0";
        let clip_url = format!("url(#{clip_id})");
        assert_eq!(edge.attribute("clip-path"), Some(clip_url.as_str()));
        let clip = document
            .descendants()
            .find(|node| node.has_tag_name("clipPath") && node.attribute("id") == Some(clip_id))
            .unwrap()
            .first_element_child()
            .unwrap();
        assert_eq!(clip.attribute("clip-rule"), Some("evenodd"));
        let geometry = BezPath::from_svg(clip.attribute("d").unwrap()).unwrap();
        let label = &layout["layout"]["UsecaseDiagram"]["edges"][0]["label"];
        let anchor = Point::new(label["x"].as_f64().unwrap(), label["y"].as_f64().unwrap());
        assert_eq!(geometry.winding(anchor) % 2, 0);
        let shaft = BezPath::from_svg(edge.attribute("d").unwrap()).unwrap();
        assert_ne!(
            geometry.winding(shaft.segments().next().unwrap().start()) % 2,
            0
        );
        assert_ne!(
            geometry.winding(shaft.segments().last().unwrap().end()) % 2,
            0
        );
    }
}

#[cfg(feature = "layout-shoal")]
#[test]
fn shoal_usecase_cuts_panel_captions_on_an_opaque_canvas() {
    use merman_render::theme::{Appearance, BuiltinPreset, Color, NativeTheme, ThemeCompiler};

    let source = "usecase-beta\ndirection TB\nsystemBoundary Panel(\"Payments\")\n  A(\"Authorize\")\n  B(\"Capture\")\nend\nA -- internal --> B\nC -- external --> D\n";
    let theme = ThemeCompiler::new()
        .compile_preset(BuiltinPreset::Studio, Some(Appearance::Dark))
        .unwrap();
    for html in [false, true] {
        let session = RenderEnvironment::deterministic()
            .begin_session_with_native_theme(NativeTheme::new(theme.clone()))
            .unwrap();
        let (_, svg) = render_config_with_session(
            source,
            json!({"layout":"shoal", "htmlLabels":html}),
            session,
        );
        let document = roxmltree::Document::parse(&svg).unwrap();
        if html {
            let css = document
                .descendants()
                .find(|node| node.has_tag_name("style"))
                .unwrap()
                .text()
                .unwrap();
            assert!(
                css.contains(".edgeLabel.usecase-panel-caption p{background-color:transparent}")
            );
        }
        for (id, on_panel) in [("edge-0", true), ("edge-1", false)] {
            let path = document
                .descendants()
                .find(|node| {
                    node.attribute("data-et") == Some("edge")
                        && node.attribute("data-id") == Some(id)
                })
                .unwrap();
            let caption = document
                .descendants()
                .find(|node| {
                    node.attribute("class") == Some("label")
                        && node.attribute("data-id") == Some(id)
                })
                .unwrap();
            assert_eq!(path.attribute("clip-path").is_some(), on_panel, "{id}");
            if !html {
                let rect = caption
                    .descendants()
                    .find(|node| node.attribute("class") == Some("background"))
                    .unwrap();
                assert_eq!(
                    rect.attribute("style"),
                    Some(if on_panel { "fill:none" } else { "" })
                );
            }
            assert_eq!(
                caption.parent_element().unwrap().attribute("class"),
                Some(if on_panel {
                    "edgeLabel usecase-panel-caption"
                } else {
                    "edgeLabel"
                }),
            );
        }

        let session = RenderEnvironment::deterministic()
            .begin_session_with_native_theme(
                NativeTheme::new(theme.clone()).with_background(Color::TRANSPARENT),
            )
            .unwrap();
        let (_, svg) = render_config_with_session(
            source,
            json!({"layout":"shoal", "htmlLabels":html}),
            session,
        );
        let document = roxmltree::Document::parse(&svg).unwrap();
        for (id, exposed) in [("edge-0", false), ("edge-1", true)] {
            let caption = document
                .descendants()
                .find(|node| {
                    node.attribute("class") == Some("label")
                        && node.attribute("data-id") == Some(id)
                })
                .unwrap();
            let plate = caption
                .parent_element()
                .unwrap()
                .children()
                .find(|node| node.attribute("class") == Some("merman-edge-label-background"));
            assert_eq!(plate.is_some(), exposed, "{id}");
            if let Some(plate) = plate {
                let parse = |paint| {
                    let [r, g, b, a] = merman_model::css_color::parse(paint)
                        .unwrap()
                        .rgba
                        .to_rgba8();
                    Color::rgba(r, g, b, a)
                };
                let fill = parse(plate.attribute("fill").unwrap());
                assert_eq!(fill.channels()[3], 255);
                let css = document
                    .descendants()
                    .find(|node| node.has_tag_name("style"))
                    .unwrap()
                    .text()
                    .unwrap();
                let (_, ink) = css.split_once(".edgeLabel{color:").unwrap();
                let ink = parse(ink.split('}').next().unwrap());
                assert!(ink.contrast(fill) >= 4.5);
            }
            assert!(document.descendants().any(|path| {
                path.attribute("data-et") == Some("edge")
                    && path.attribute("data-id") == Some(id)
                    && path.attribute("clip-path").is_some()
            }));
        }
    }
}
