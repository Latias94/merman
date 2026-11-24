#![cfg(feature = "diagram-flowchart")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::environment::RenderEnvironment;
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};
use merman_render::theme::{NativeTheme, ThemeCompiler};
use merman_render::{LayoutOptions, family};
use roxmltree::{Document, Node};

#[test]
fn native_theme_v2_flowchart_applies_cluster_form_and_preserves_authored_paint() {
    let theme = ThemeCompiler::new()
        .compile_json(
            br##"{
                "schema_version":2,"extends":"paper",
                "roles":{"surface.container":"#ABCDEF","stroke.border":"#765432"},
                "form":{"stroke_width":2.5,"container_radius":9,"elevation":"flat"}
            }"##,
            None,
        )
        .unwrap();
    let source = "flowchart TD\nsubgraph Group[Group]\nA\nend\n\
        subgraph Authored[Authored]\nB\nend\nstyle Authored fill:#cceebb,stroke:#756341\n";
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_native_theme(NativeTheme::new(theme))
        .unwrap();
    let rendered = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap()
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    let document = Document::parse(rendered.svg()).unwrap();
    for (id, fill, stroke) in [
        ("Group", "#ABCDEF", "#765432"),
        ("Authored", "#cceebb", "#756341"),
    ] {
        let frame = document
            .descendants()
            .find(|node| node.attribute("data-id") == Some(id))
            .unwrap()
            .children()
            .find(|node| node.has_tag_name("rect"))
            .unwrap();
        assert_eq!(frame.attribute("rx"), Some("9"), "{id}");
        assert_eq!(
            property(frame, "stroke-width").map(|value| value.trim_end_matches("px")),
            Some("2.5"),
            "{id}"
        );
        for (name, expected) in [("fill", fill), ("stroke", stroke)] {
            let value = property(frame, name).unwrap_or_else(|| panic!("{id}: missing {name}"));
            assert_eq!(
                value
                    .parse::<svgtypes::Color>()
                    .unwrap_or_else(|_| panic!("{id}: invalid {name} {value:?}")),
                expected.parse::<svgtypes::Color>().unwrap(),
                "{id}: {name}"
            );
        }
    }
}

#[test]
fn native_flowchart_edge_labels_use_nested_paint_and_pill_roles() {
    const SOURCE: &str = concat!(
        "flowchart LR\nsubgraph Outer\nsubgraph Inner\nA -->|Inside| B\nend\nend\n",
        "C -->|Outside| D\nstyle Outer fill:#FF0000\nstyle Inner fill:#0000FF80\n"
    );
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(SOURCE, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    for background in ["auto", "pill", "transparent"] {
        let document = serde_json::json!({
            "schema_version": 2,
            "extends": "paper",
            "roles": {"canvas": "#FFFFFF", "surface.node": "#FFEEDD", "stroke.border": "#123456"},
            "form": {"edge_label_bg": background},
        });
        let theme = ThemeCompiler::new()
            .compile_json(document.to_string().as_bytes(), None)
            .unwrap();
        let session = RenderEnvironment::deterministic()
            .begin_session_with_native_theme(NativeTheme::new(theme))
            .unwrap();
        let svg = family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
            .unwrap()
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let svg = Document::parse(svg.svg()).unwrap();
        let plates: Vec<_> = svg
            .descendants()
            .filter(|node| node.attribute("class") == Some("merman-edge-label-background"))
            .collect();
        match background {
            "auto" => {
                assert_eq!(plates.len(), 2);
                assert!(
                    plates
                        .iter()
                        .any(|plate| plate.attribute("fill") == Some("#7F0080"))
                );
                assert!(
                    plates
                        .iter()
                        .any(|plate| plate.attribute("fill") == Some("#FFFFFF"))
                );
                assert!(
                    plates
                        .iter()
                        .all(|plate| plate.attribute("rx") == Some("4"))
                );
            }
            "pill" => {
                assert_eq!(plates.len(), 2);
                assert!(
                    plates
                        .iter()
                        .all(|plate| plate.attribute("fill") == Some("#FFEEDD")
                            && plate.attribute("stroke") == Some("#123456")
                            && plate.attribute("stroke-width") == Some("0.75"))
                );
            }
            _ => assert!(plates.is_empty()),
        }
    }
}

fn property<'a>(node: Node<'a, '_>, name: &str) -> Option<&'a str> {
    node.attribute("style")
        .and_then(|style| {
            style.rsplit(';').find_map(|part| {
                let (property, value) = part.split_once(':')?;
                (property.trim() == name)
                    .then(|| value.trim().trim_end_matches("!important").trim())
            })
        })
        .or_else(|| node.attribute(name))
}
