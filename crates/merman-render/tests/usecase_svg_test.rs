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
