#![cfg(all(feature = "diagram-flowchart", feature = "layout-shoal"))]

use kurbo::{BezPath, Point, Shape, Vec2};
use merman_core::{MermaidEngine, ParseOptions};
use merman_render::environment::RenderEnvironment;
use merman_render::{LayoutOptions, family};
use roxmltree::{Document, Node};
use std::fmt::Write as _;

fn render(layout: &str, look: &str, body: &str) -> String {
    render_with_config(
        &format!("  layout: {layout}\n  look: {look}\n  htmlLabels: false"),
        body,
    )
}

fn render_with_config(config: &str, body: &str) -> String {
    let source = format!("---\nconfig:\n{config}\n  handDrawnSeed: 7\n---\nflowchart LR\n{body}");
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
        "shape: notch-rect",
        "shape: note",
        "shape: cloud",
        "shape: person",
        "shape: rect",
        "shape: rounded",
        "shape: shaded-process",
        "shape: sm-circ",
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

#[test]
fn shoal_hand_drawn_exotic_shapes_use_default_node_paint() {
    let shapes = [
        "card",
        "notch-rect",
        "sm-circ",
        "small-circle",
        "start",
        "cloud",
        "bang",
        "braces",
        "delay",
        "display",
        "div-rect",
        "flag",
        "lin-cyl",
        "lin-doc",
        "lin-rect",
        "odd",
        "sl-rect",
        "st-rect",
        "tag-doc",
        "tag-rect",
        "tri",
        "win-pane",
        "bolt",
        "com-link",
        "hourglass",
        "collate",
    ];
    let mut body = String::new();
    for (index, shape) in shapes.iter().enumerate() {
        writeln!(
            body,
            "n{index}@{{ shape: {shape}, label: 'Authored label' }}"
        )
        .unwrap();
    }

    for (theme, fill, stroke) in [
        ("default", "#ECECFF", "#9370DB"),
        ("dark", "#1f2020", "#ccc"),
    ] {
        let svg = render_with_config(
            &format!("  layout: shoal\n  look: handDrawn\n  htmlLabels: false\n  theme: {theme}"),
            &body,
        );
        let document = Document::parse(&svg).unwrap();
        for (index, shape) in shapes.iter().copied().enumerate() {
            let group = node(&document, &format!("n{index}"));
            assert!(
                !group.descendants().any(|part| {
                    matches!(part.tag_name().name(), "polygon" | "circle")
                        && part.attribute("fill").is_none()
                        && part.attribute("style").is_none_or(str::is_empty)
                }),
                "{theme} {shape} has an unpainted primitive"
            );
            if matches!(
                shape,
                "card" | "notch-rect" | "sm-circ" | "small-circle" | "start"
            ) {
                let backing = group.descendants().find(is_backing).unwrap();
                assert_eq!(backing.attribute("fill"), Some(fill), "{theme} {shape}");
                let hatch = backing.next_sibling_element().unwrap();
                assert_eq!(hatch.attribute("stroke"), Some(stroke), "{theme} {shape}");
                assert_eq!(declared_opacity(hatch, "stroke-opacity"), Some(0.15));
                let outline = hatch.next_sibling_element().unwrap();
                assert_eq!(outline.attribute("stroke"), Some(stroke), "{theme} {shape}");
            }
            if matches!(
                shape,
                "sm-circ" | "small-circle" | "start" | "bolt" | "com-link"
            ) {
                assert!(
                    !group
                        .descendants()
                        .any(|part| part.attribute("class") == Some("label")),
                    "{shape} must omit its label group"
                );
            }
            if matches!(shape, "hourglass" | "collate") {
                let label = label_group(group);
                assert!(
                    label
                        .descendants()
                        .all(|part| part.text().is_none_or(|text| text.trim().is_empty())),
                    "{shape} must clear its label text"
                );
            }
        }
    }
}

fn label_group<'a, 'input>(group: Node<'a, 'input>) -> Node<'a, 'input> {
    group
        .children()
        .find(|part| part.attribute("class") == Some("label"))
        .unwrap()
}

fn translation(element: Node<'_, '_>) -> Vec2 {
    let (x, y) = element
        .attribute("transform")
        .unwrap()
        .strip_prefix("translate(")
        .unwrap()
        .strip_suffix(')')
        .unwrap()
        .split_once(',')
        .unwrap();
    Vec2::new(x.trim().parse().unwrap(), y.trim().parse().unwrap())
}

#[test]
fn shoal_organic_shape_labels_are_centered_for_svg_and_html() {
    let body = concat!(
        "R@{ shape: rect, label: 'Partner Sync' }\n",
        "C@{ shape: cloud, label: 'Partner Sync' }\n",
        "B@{ shape: bang, label: 'Partner Sync' }\n",
    );
    for html_labels in [false, true] {
        let svg = render_with_config(
            &format!("  layout: shoal\n  look: handDrawn\n  htmlLabels: {html_labels}"),
            body,
        );
        let document = Document::parse(&svg).unwrap();
        let reference = translation(label_group(node(&document, "R")));
        for id in ["C", "B"] {
            let group = node(&document, id);
            let backing = group.descendants().find(is_backing).unwrap();
            let silhouette = BezPath::from_svg(backing.attribute("d").unwrap()).unwrap();
            let center =
                silhouette.bounding_box().center() + translation(backing.parent().unwrap());
            let label = label_group(group);
            let position = translation(label);
            let label_center = if html_labels {
                let html = label
                    .descendants()
                    .find(|part| part.has_tag_name("foreignObject"))
                    .unwrap();
                let width = html.attribute("width").unwrap().parse::<f64>().unwrap();
                let height = html.attribute("height").unwrap().parse::<f64>().unwrap();
                Point::new(position.x + width / 2.0, position.y + height / 2.0)
            } else {
                let row = label
                    .descendants()
                    .find(|part| part.has_tag_name("tspan"))
                    .unwrap();
                let x = row.attribute("x").unwrap().parse::<f64>().unwrap();
                // Both labels use the same text and vertical metrics. The rect's label is centered.
                Point::new(position.x + x, position.y - reference.y)
            };
            assert!(
                label_center.distance(center) <= 4.0,
                "{id} HTML={html_labels}: label {label_center:?}, silhouette {center:?}"
            );
        }
    }
}

const BRACES: &str = "B@{ shape: braces, label: 'Braces' }\n";

#[test]
fn shoal_hand_drawn_braces_emit_only_visible_rough_strokes() {
    let svg = render("shoal", "handDrawn", BRACES);
    let document = Document::parse(&svg).unwrap();
    let group = node(&document, "B");
    let body = group
        .children()
        .find(|part| part.attribute("class") == Some("text"))
        .unwrap();
    let bounds = body
        .children()
        .find(|part| part.has_tag_name("rect"))
        .unwrap();
    assert_eq!(bounds.attribute("visibility"), Some("hidden"));
    assert!(bounds.attribute("width").unwrap().parse::<f64>().unwrap() > 0.0);
    assert!(bounds.attribute("height").unwrap().parse::<f64>().unwrap() > 0.0);
    assert_eq!(
        body.descendants()
            .filter(|part| part.has_tag_name("path"))
            .count(),
        2
    );
    // Each visible stroke still uses the shared RoughJS serializer.
    assert!(group.range().len() < 15_000);
}

#[test]
#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
fn parity_layouts_keep_exotic_shape_primitives_and_brace_strokes() {
    let body = format!("C@{{ shape: card }}\nS@{{ shape: sm-circ }}\n{BRACES}");
    for layout in [
        #[cfg(feature = "layout-dagre")]
        "dagre",
        #[cfg(feature = "layout-elk")]
        "elk",
    ] {
        let svg = render(layout, "handDrawn", &body);
        let document = Document::parse(&svg).unwrap();
        assert!(
            node(&document, "C")
                .children()
                .any(|part| part.has_tag_name("polygon"))
        );
        assert!(node(&document, "S").children().any(|part| {
            part.has_tag_name("circle") && part.attribute("class") == Some("state-start")
        }));
        let hidden = node(&document, "B")
            .descendants()
            .find(|part| part.attribute("stroke-opacity") == Some("0"))
            .unwrap();
        assert!(hidden.children().any(|part| part.has_tag_name("path")));
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
