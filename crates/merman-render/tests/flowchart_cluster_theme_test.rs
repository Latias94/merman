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
