use merman_core::{Engine, MermaidConfig, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use serde_json::json;

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

fn render(source: &str, layout: &str) -> (serde_json::Value, String) {
    render_config(
        source,
        json!({
            "layout": layout, "theme": "default", "look": "classic", "htmlLabels": false,
        }),
    )
}

fn render_config(source: &str, config: serde_json::Value) -> (serde_json::Value, String) {
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(config));
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse Usecase")
        .expect("detect Usecase");
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("render session");
    let artifact = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session)
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
    let (projection, _) = render_config(
        "usecase-beta\ndirection LR\nA --> Target\nB --> Target\nC --> Target",
        json!({"layout":"elk", "htmlLabels":false, "usecase":{"usecaseFontSize":40}}),
    );
    let graph = &projection["layout"]["UsecaseDiagram"];
    let target = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == "Target")
        .unwrap();
    let center = target["y"].as_f64().unwrap();
    let mut offsets: Vec<_> = graph["edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|edge| {
            edge["points"].as_array().unwrap().last().unwrap()["y"]
                .as_f64()
                .unwrap()
                - center
        })
        .collect();
    offsets.sort_by(f64::total_cmp);
    assert_eq!(offsets.len(), 3);
    for (actual, expected) in offsets.iter().zip([-20.0, 0.0, 20.0]) {
        assert!((actual - expected).abs() < 1e-9, "{offsets:?}");
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
fn usecase_plain_labels_preserve_literal_break_tags_and_entities_in_both_label_modes() {
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
        for (id, expected) in [("A", "a<br/>b"), ("B", "c<BR />d &lt; e &amp; f")] {
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
        assert_eq!(text_content(edge_label), "g<br/>h", "htmlLabels={html}");
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
            let parsed = Engine::new()
                .with_site_config(MermaidConfig::from_value(json!({
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
            let result = family::prepare(parsed, &LayoutOptions::headless_svg_defaults(), session);
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
        let parsed = Engine::new()
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
            family::prepare(parsed, &LayoutOptions::default(), session),
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
                json!({"layout":"dagre", "theme":theme, "look":look}),
            );
            let document = roxmltree::Document::parse(&svg).unwrap();
            let note = document
                .descendants()
                .find(|node| node.attribute("data-usecase-kind") == Some("note"))
                .unwrap_or_else(|| panic!("missing note for {theme}/{look}"));
            assert!(note.descendants().any(|node| node.has_tag_name("path")));
        }
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
