#![cfg(all(feature = "diagram-flowchart", feature = "layout-shoal"))]

use kurbo::{BezPath, Shape};
use merman_core::{MermaidEngine, ParseOptions};
use merman_render::environment::RenderEnvironment;
use merman_render::{LayoutOptions, family};
use roxmltree::{Document, Node};
use std::fmt::Write as _;

fn render(layout: &str, look: &str, body: &str) -> String {
    let source = format!(
        "---\nconfig:\n  layout: {layout}\n  look: {look}\n  handDrawnSeed: 7\n  htmlLabels: false\n---\nflowchart LR\n{body}"
    );
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session)
        .unwrap()
        .render_svg(&Default::default(), &Default::default())
        .unwrap()
        .svg()
        .to_owned()
}

fn node<'a>(document: &'a Document<'a>, id: &str) -> Node<'a, 'a> {
    document
        .descendants()
        .find(|node| {
            node.attribute("data-et") == Some("node") && node.attribute("data-id") == Some(id)
        })
        .unwrap_or_else(|| panic!("missing node {id}"))
}

fn is_backing(node: &Node<'_, '_>) -> bool {
    node.attribute("class") == Some("merman-rough-backing")
}

#[test]
fn shoal_hand_drawn_shapes_paint_backing_before_the_hatch() {
    let shapes = [
        "shape: bow-rect",
        "shape: browser",
        "shape: bucket",
        "shape: console",
        "shape: curv-trap",
        "shape: datastore",
        "shape: delay",
        "shape: div-rect",
        "shape: folder",
        "icon: 'missing:icon', form: circle",
        "icon: 'missing:icon', form: square",
        "shape: collate",
        "shape: lined-document",
        "shape: notch-pent",
        "shape: note",
        "shape: cloud",
        "shape: person",
        "shape: rect",
        "shape: rounded",
        "shape: shaded-process",
        "shape: stacked-document",
        "shape: stacked-rectangle",
        "shape: tag-rect",
        "shape: tag-doc",
        "shape: triangle",
        "shape: document",
        "shape: window-pane",
    ];
    let mut body = String::new();
    for (index, shape) in shapes.iter().enumerate() {
        writeln!(body, "n{index}@{{ {shape}, label: 'Shape' }}").unwrap();
        writeln!(body, "style n{index} fill:#FEDCBA,stroke:#123456").unwrap();
    }
    let svg = render("shoal", "handDrawn", &body);
    let document = Document::parse(&svg).unwrap();
    for (index, shape) in shapes.iter().enumerate() {
        let backings: Vec<_> = node(&document, &format!("n{index}"))
            .descendants()
            .filter(is_backing)
            .collect();
        let expected = usize::from(matches!(
            *shape,
            "shape: person" | "shape: stacked-document" | "shape: stacked-rectangle"
        )) + 1;
        assert_eq!(backings.len(), expected, "{shape}");
        for backing in backings {
            assert_eq!(backing.attribute("fill"), Some("#FEDCBA"), "{shape}");
            assert_eq!(backing.attribute("stroke"), Some("none"), "{shape}");
            let silhouette = BezPath::from_svg(backing.attribute("d").unwrap()).unwrap();
            assert!(silhouette.bounding_box().area() > 0.0, "{shape}");
            let hatch = backing.next_sibling_element().unwrap();
            assert_eq!(hatch.attribute("fill"), Some("none"), "{shape}");
            assert!(
                hatch
                    .attribute("style")
                    .unwrap()
                    .contains("stroke-opacity:0.15"),
                "{shape}"
            );
        }
    }

    #[cfg(feature = "layout-dagre")]
    {
        let svg = render("dagre", "handDrawn", &body);
        assert!(
            !Document::parse(&svg)
                .unwrap()
                .descendants()
                .any(|node| is_backing(&node))
        );
    }
}

fn declared_opacity(node: Node<'_, '_>, property: &str) -> Option<f64> {
    node.attribute("style")?
        .split(';')
        .filter_map(|decl| {
            let (name, value) = decl.split_once(':')?;
            (name.trim() == property)
                .then(|| value.split_ascii_whitespace().next()?.parse::<f64>().ok())
                .flatten()
        })
        .next_back()
}

#[test]
fn shoal_hand_drawn_backings_keep_colour_alpha_and_apply_group_opacity_once() {
    let svg = render(
        "shoal",
        "handDrawn",
        concat!(
            "A{Existing diamond}\nB[New process]\nC[Clear fill]\nD[Zero fill opacity]\n",
            "style A fill:#AABBCC80,fill-opacity:0.5,opacity:0.4\n",
            "classDef dim fill:#AABBCC80,opacity:0.4,fill-opacity:0.5\nclass B dim\n",
            "style C fill:transparent\nstyle D fill:#AABBCC,fill-opacity:0\n",
        ),
    );
    let document = Document::parse(&svg).unwrap();
    for id in ["A", "B"] {
        let backing = node(&document, id).descendants().find(is_backing).unwrap();
        assert_eq!(backing.attribute("fill"), Some("#AABBCC80"));
        assert_eq!(declared_opacity(backing, "fill-opacity"), Some(0.5));
        let hatch = backing.next_sibling_element().unwrap();
        let hatch_opacity = declared_opacity(hatch, "stroke-opacity").unwrap();
        assert!((hatch_opacity - 0.15 * 0.5 * (128.0 / 255.0)).abs() < 1e-8);
        assert_eq!(
            backing
                .ancestors()
                .filter_map(|element| declared_opacity(element, "opacity"))
                .collect::<Vec<_>>(),
            [0.4],
            "{id}",
        );
    }
    for (id, fill, opacity) in [("C", "transparent", 1.0), ("D", "#AABBCC", 0.0)] {
        let backing = node(&document, id).descendants().find(is_backing).unwrap();
        assert_eq!(backing.attribute("fill"), Some(fill));
        assert_eq!(declared_opacity(backing, "fill-opacity"), Some(opacity));
        let outline = backing.next_sibling_element().unwrap();
        assert_eq!(declared_opacity(outline, "stroke-opacity"), None, "{id}");
    }
}

#[test]
fn shoal_classic_double_circle_applies_opacity_to_the_ring_group_once() {
    let svg = render(
        "shoal",
        "classic",
        "A(((Double)))\nclassDef dim opacity:0.4,stroke:#123456\nclass A dim",
    );
    let document = Document::parse(&svg).unwrap();
    let rings: Vec<_> = node(&document, "A")
        .descendants()
        .filter(|node| node.has_tag_name("circle"))
        .collect();
    assert_eq!(rings.len(), 2);
    for ring in rings {
        assert_eq!(declared_opacity(ring, "opacity"), Some(1.0));
        assert_eq!(
            ring.ancestors()
                .filter_map(|el| declared_opacity(el, "opacity"))
                .product::<f64>(),
            0.4
        );
    }
}
