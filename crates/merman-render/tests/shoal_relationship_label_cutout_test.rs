#![cfg(all(
    feature = "layout-shoal",
    any(feature = "diagram-er", feature = "diagram-requirement")
))]

use kurbo::{BezPath, Point, Shape};
use merman_core::{MermaidEngine, ParseOptions};
use merman_render::environment::RenderEnvironment;
use merman_render::svg::{SvgPipeline, SvgRenderOptions};
use merman_render::theme::{Appearance, BuiltinPreset, Color, NativeTheme, ThemeCompiler};
use merman_render::{LayoutOptions, family};
use roxmltree::{Document, Node};

fn render(source: &str, layout: &str, html: bool, canvas: Color) -> family::RenderedFamilySvg {
    let source = format!(
        "---\nconfig:\n  layout: {layout}\n  look: classic\n  htmlLabels: {html}\n---\n{source}"
    );
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let theme = ThemeCompiler::new()
        .compile_preset(BuiltinPreset::Studio, Some(Appearance::Light))
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_native_theme(NativeTheme::new(theme))
        .unwrap()
        .with_canvas_background(canvas);
    family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap()
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("cutout-test".to_owned()),
                ..Default::default()
            },
            &Default::default(),
        )
        .unwrap()
}

fn reference<'a, 'input>(edge: Node<'a, 'input>, attribute: &str) -> Node<'a, 'input> {
    let id = edge
        .attribute(attribute)
        .unwrap_or_else(|| panic!("missing {attribute} on {:?}", edge.attribute("id")))
        .strip_prefix("url(#")
        .unwrap()
        .strip_suffix(')')
        .unwrap();
    let mut definitions = edge
        .document()
        .descendants()
        .filter(|node| node.attribute("id") == Some(id));
    let definition = definitions.next().expect("missing definition");
    assert!(definitions.next().is_none(), "duplicate definition {id}");
    definition
}

fn assert_caption_cutout(edge: Node<'_, '_>) {
    let clip = reference(edge, "clip-path");
    assert!(clip.has_tag_name("clipPath"));
    let path = clip.first_element_child().unwrap();
    assert_eq!(path.attribute("clip-rule"), Some("evenodd"));
    let frame = BezPath::from_svg(path.attribute("d").unwrap()).unwrap();
    let label = edge
        .document()
        .descendants()
        .find(|node| {
            node.attribute("class") == Some("label")
                && node.attribute("data-id") == edge.attribute("data-id")
        })
        .unwrap();
    let anchor = label
        .parent_element()
        .unwrap()
        .attribute("transform")
        .unwrap()
        .strip_prefix("translate(")
        .unwrap()
        .strip_suffix(')')
        .unwrap();
    let (x, y) = anchor.split_once(',').unwrap();
    let center = Point::new(x.trim().parse().unwrap(), y.trim().parse().unwrap());
    assert_eq!(frame.winding(center) % 2, 0, "caption center is clipped");
    let shaft = BezPath::from_svg(edge.attribute("d").unwrap()).unwrap();
    for contact in [
        shaft.segments().next().unwrap().start(),
        shaft.segments().last().unwrap().end(),
    ] {
        assert_ne!(frame.winding(contact) % 2, 0, "shaft end is visible");
    }
}

#[cfg(feature = "diagram-er")]
#[test]
fn er_shoal_caption_cutout_coexists_with_the_zero_marker_mask() {
    let source = "erDiagram\ndirection LR\nA |o--o{ B : owns\nC ||--|| D : \"\"\n";
    for html in [false, true] {
        let rendered = render(source, "shoal", html, Color::TRANSPARENT);
        let svg = rendered.svg().to_owned();
        let raster = rendered.finalize_resvg(&SvgPipeline::resvg_safe()).unwrap();
        for output in [svg.as_str(), raster.svg().as_str()] {
            let doc = Document::parse(output).unwrap();
            let mut edges = doc
                .descendants()
                .filter(|node| node.attribute("data-et") == Some("edge"));
            let edge = edges.next().unwrap();
            assert_caption_cutout(edge);
            let mask = reference(edge, "mask");
            assert_eq!(mask.attribute("id"), Some("cutout-test_er-marker-hole-0"));
            assert_eq!(mask.attribute("mask-type"), Some("luminance"));
            for attribute in ["marker-start", "marker-end"] {
                assert!(reference(edge, attribute).has_tag_name("marker"));
            }
            assert_eq!(edges.next().unwrap().attribute("clip-path"), None);
        }
    }
}

#[cfg(feature = "diagram-requirement")]
#[test]
fn requirement_shoal_cuts_a_transparent_caption_and_keeps_its_marker() {
    let source = "requirementDiagram\ndirection LR\n\
        requirement req1 {\n id: 1\n text: Body\n risk: high\n verifymethod: analysis\n}\n\
        element elem1 {\n type: simulation\n}\nreq1 - satisfies -> elem1\n";
    for (layout, canvas, cutout) in [
        ("shoal", Color::TRANSPARENT, true),
        ("shoal", Color::rgb(255, 255, 255), false),
        #[cfg(feature = "layout-dagre")]
        ("dagre", Color::TRANSPARENT, false),
    ] {
        for html in [false, true] {
            let rendered = render(source, layout, html, canvas);
            let svg = rendered.svg().to_owned();
            let raster = rendered.finalize_resvg(&SvgPipeline::resvg_safe()).unwrap();
            for output in [svg.as_str(), raster.svg().as_str()] {
                let doc = Document::parse(output).unwrap();
                let edge = doc
                    .descendants()
                    .find(|node| node.attribute("data-et") == Some("edge"))
                    .unwrap();
                if cutout {
                    assert_caption_cutout(edge);
                } else {
                    assert_eq!(edge.attribute("clip-path"), None);
                }
                assert!(reference(edge, "marker-end").has_tag_name("marker"));
            }
        }
    }
}
