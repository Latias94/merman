use merman_core::{Engine, ParseOptions};
use merman_render::LayoutOptions;
use merman_render::environment::RenderEnvironment;
use merman_render::family;
use merman_render::model::{C4DiagramLayout, C4ShapeLayout};
use merman_render::svg::{SvgDebugOptions, SvgRenderOptions};

fn render(source: &str) -> (C4DiagramLayout, String) {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .expect("parse C4")
        .expect("C4 diagram");
    let environment = RenderEnvironment::deterministic();
    let session = environment.begin_session().expect("render session");
    let options = LayoutOptions::headless_svg_defaults().with_screen_available_width(4000.0);
    let artifact = family::prepare(parsed, &options, session).expect("prepare C4");
    let projection = artifact.layout_json().expect("layout projection");
    let layout =
        serde_json::from_value(projection["layout"]["C4Diagram"].clone()).expect("typed C4 layout");
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render C4 SVG")
        .svg()
        .to_owned();
    (layout, svg)
}

fn relation_points(node: roxmltree::Node<'_, '_>) -> [(f64, f64); 2] {
    if node.has_tag_name("line") {
        let number = |name| node.attribute(name).unwrap().parse::<f64>().unwrap();
        return [(number("x1"), number("y1")), (number("x2"), number("y2"))];
    }
    let coordinates: Vec<f64> = node
        .attribute("d")
        .expect("relation path")
        .split(|ch: char| ch.is_ascii_alphabetic() || ch.is_ascii_whitespace() || ch == ',')
        .filter(|value| !value.is_empty())
        .map(|value| value.parse().expect("path coordinate"))
        .collect();
    assert_eq!(coordinates.len(), 6, "C4 relation uses M and Q");
    [
        (coordinates[0], coordinates[1]),
        (coordinates[4], coordinates[5]),
    ]
}

fn shape<'a>(layout: &'a C4DiagramLayout, alias: &str) -> &'a C4ShapeLayout {
    layout
        .shapes
        .iter()
        .find(|node| node.alias == alias)
        .unwrap()
}

fn center(node: &C4ShapeLayout) -> (f64, f64) {
    (node.x + node.width / 2.0, node.y + node.height / 2.0)
}

fn assert_center_ray(point: (f64, f64), node: &C4ShapeLayout, target: &C4ShapeLayout) {
    let (cx, cy) = center(node);
    let (tx, ty) = center(target);
    let cross = (point.0 - cx) * (ty - cy) - (point.1 - cy) * (tx - cx);
    let distance = (tx - cx).hypot(ty - cy);
    assert!(
        cross.abs() / distance < 1e-8,
        "{} endpoint {point:?} must follow its center ray toward {}",
        node.alias,
        target.alias
    );
    assert!((point.0 - cx) * (tx - cx) + (point.1 - cy) * (ty - cy) > 0.0);
}

#[test]
fn c4_rectangular_relations_follow_center_rays_in_every_family_and_direction() {
    for family in [
        "C4Context",
        "C4Container",
        "C4Component",
        "C4Dynamic",
        "C4Deployment",
    ] {
        for shape_override in ["", ", $shape=\"component\""] {
            let mut source = format!("{family}\nUpdateLayoutConfig($c4ShapeInRow=\"3\")\n");
            for index in 0..9 {
                source.push_str(&format!(
                    "System(n{index}, \"Node\", \"Description\"{shape_override})\n"
                ));
            }
            for (index, relation) in [
                (0, "Rel"),
                (1, "Rel_Up"),
                (2, "BiRel"),
                (3, "Rel_Left"),
                (5, "Rel_Right"),
                (6, "Rel_Back"),
                (7, "Rel_Down"),
                (8, "Rel"),
            ] {
                source.push_str(&format!("{relation}(n4, n{index}, \"Calls {index}\")\n"));
            }
            let (layout, svg) = render(&source);
            let document = roxmltree::Document::parse(&svg).unwrap();
            let lines: Vec<_> = document
                .descendants()
                .filter(|node| {
                    node.attribute("marker-end").is_some()
                        || node.attribute("marker-start").is_some()
                })
                .collect();
            assert_eq!(lines.len(), 8);
            for (relation, line) in layout.rels.iter().zip(lines) {
                let from = shape(&layout, &relation.from);
                let to = shape(&layout, &relation.to);
                let [start, end] = relation_points(line);
                for (point, node, target) in [(start, from, to), (end, to, from)] {
                    assert_center_ray(point, node, target);
                    let on_vertical = (point.0 - node.x).abs() < 1e-8
                        || (point.0 - node.x - node.width).abs() < 1e-8;
                    let on_horizontal = (point.1 - node.y).abs() < 1e-8
                        || (point.1 - node.y - node.height).abs() < 1e-8;
                    assert!(
                        on_vertical || on_horizontal,
                        "endpoint must meet rectangle perimeter"
                    );
                }
                assert_eq!(
                    line.attribute("marker-start").is_some(),
                    matches!(relation.rel_type.as_str(), "birel" | "rel_b")
                );
                assert_eq!(
                    line.attribute("marker-end").is_some(),
                    relation.rel_type != "rel_b"
                );
            }
        }
    }
}

#[test]
fn c4_context_person_to_system_uses_the_target_center_ray() {
    let (layout, svg) = render(
        r#"C4Context
title System Context diagram
Person(customerA, "Customer", "A customer")
System(sys, "Banking System", "Does banking")
Rel(customerA, sys, "Uses")
"#,
    );
    let document = roxmltree::Document::parse(&svg).unwrap();
    let relation = document
        .descendants()
        .find(|node| node.attribute("marker-end").is_some())
        .unwrap();
    let [_, end] = relation_points(relation);
    assert_center_ray(end, shape(&layout, "sys"), shape(&layout, "customerA"));
    assert_eq!(end.0, shape(&layout, "sys").x);
}

#[test]
fn c4_dynamic_return_relation_keeps_the_upstream_nearly_horizontal_path() {
    let (layout, svg) = render(include_str!(
        "../../../fixtures/c4/upstream_docs_c4_c4_dynamic_diagram_c4dynamic_010.mmd"
    ));
    let document = roxmltree::Document::parse(&svg).unwrap();
    let relation = document
        .descendants()
        .filter(|node| node.attribute("marker-end").is_some())
        .nth(1)
        .unwrap();
    assert!(relation.has_tag_name("path"));
    assert!(relation.attribute("d").unwrap().contains('Q'));
    let [start, end] = relation_points(relation);
    let from = shape(&layout, "c2");
    let to = shape(&layout, "c3");
    assert!(start.0 > end.0, "return relation points left");
    assert_center_ray(start, from, to);
    assert_center_ray(end, to, from);
    assert!((end.1 - start.1).abs() <= (center(to).1 - center(from).1).abs() + 1e-8);
}

#[test]
fn c4_database_and_queue_horizontal_relations_meet_the_center_port() {
    for shape_override in ["database", "queue"] {
        let source = format!(
            "C4Context\nSystem(a, \"Node\", $shape=\"{shape_override}\")\nSystem(b, \"Node\", $shape=\"{shape_override}\")\nBiRel(a, b, \"Calls\")\n"
        );
        let (layout, svg) = render(&source);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let relation = document
            .descendants()
            .find(|node| node.attribute("marker-end").is_some())
            .unwrap();
        let [start, end] = relation_points(relation);
        let from = shape(&layout, "a");
        let to = shape(&layout, "b");
        assert_eq!(center(from).1, center(to).1);
        assert_eq!(start.1, center(from).1);
        assert_eq!(end.1, center(to).1);
        assert!(start.0 > center(from).0 && end.0 < center(to).0);
    }
}

#[test]
fn c4_person_relations_follow_the_sampled_silhouette_in_all_octants() {
    for declaration in ["Person", "Person_Ext"] {
        let mut source = String::from("C4Context\nUpdateLayoutConfig($c4ShapeInRow=\"3\")\n");
        for index in 0..9 {
            source.push_str(&format!("{declaration}(n{index}, \"Node\")\n"));
        }
        for index in 0..9 {
            source.push_str(&format!("Rel(n4, n{index}, \"Calls {index}\")\n"));
        }
        let (layout, svg) = render(&source);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let lines: Vec<_> = document
            .descendants()
            .filter(|node| node.attribute("marker-end").is_some())
            .collect();
        let from = shape(&layout, "n4");
        for (index, line) in lines.iter().enumerate() {
            let [start, end] = relation_points(*line);
            let to = shape(&layout, &format!("n{index}"));
            if index == 4 {
                assert_eq!(
                    start,
                    center(from),
                    "a zero-length polygon query returns the node center"
                );
                assert_eq!(end, center(to));
                continue;
            }
            assert_center_ray(start, from, to);
            assert_center_ray(end, to, from);
            for (point, node) in [(start, from), (end, to)] {
                assert!(point.0 >= node.x - 1e-8 && point.0 <= node.x + node.width + 1e-8);
                assert!(point.1 >= node.y - 1e-8 && point.1 <= node.y + node.height + 1e-8);
            }
        }
        let upward = relation_points(lines[1])[0];
        let downward = relation_points(lines[7])[0];
        assert!((upward.1 - from.y).abs() < 1e-8);
        // Mermaid's even head sample count misses the circle's exact top. Its polygon helper
        // aligns the sampled minimum to the box, moving the body's bottom up by the same amount.
        assert!(downward.1 < from.y + from.height);
        let leftward = relation_points(lines[3])[0];
        let rightward = relation_points(lines[5])[0];
        assert!((leftward.0 + rightward.0 - 2.0 * center(from).0).abs() < 1e-8);
    }
}

#[test]
fn c4_database_and_queue_vertical_relations_preserve_the_cap_axis() {
    for shape_override in ["database", "queue"] {
        // An integral/half-integral label height makes the axis query exact. With the default
        // 39.95px queue height, Mermaid's floating subtraction can select its cap adjustment.
        let source = format!(
            "%%{{init: {{\"c4\": {{\"systemFontSize\": 20}}}}}}%%\nC4Context\nUpdateLayoutConfig($c4ShapeInRow=\"1\")\nSystem(a, \"Node\", $shape=\"{shape_override}\")\nSystem(b, \"Node\", $shape=\"{shape_override}\")\nRel(a, b, \"Down\")\nRel(b, a, \"Up\")\n"
        );
        let (layout, svg) = render(&source);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let lines = document
            .descendants()
            .filter(|node| node.attribute("marker-end").is_some());
        for (relation, line) in layout.rels.iter().zip(lines) {
            let [start, end] = relation_points(line);
            let from = shape(&layout, &relation.from);
            let to = shape(&layout, &relation.to);
            assert_eq!(center(from).0, center(to).0);
            assert_eq!(start.0, center(from).0);
            assert_eq!(end.0, center(to).0);
            assert_center_ray(start, from, to);
            assert_center_ray(end, to, from);
        }
    }
}

#[test]
fn c4_self_relations_render_finite_degenerate_edges_for_rect_db_and_queue_shapes() {
    for declaration in ["System", "SystemDb", "SystemQueue"] {
        let source =
            format!("C4Context\n{declaration}(node, \"Node\")\nRel(node, node, \"Self\")\n");
        let (layout, svg) = render(&source);
        let document = roxmltree::Document::parse(&svg).expect("valid self relation SVG");
        let relation = document
            .descendants()
            .find(|node| node.attribute("marker-end").is_some())
            .expect("self relation marker");
        let [start, end] = relation_points(relation);
        for point in [start, end] {
            assert!(point.0.is_finite() && point.1.is_finite());
        }
        assert!(!svg.contains("NaN"));
        let node = shape(&layout, "node");
        assert_eq!(
            start, end,
            "Mermaid draws self relations as degenerate edges"
        );
        assert!((start.0 - node.x - node.width).abs() < 1e-8);
        assert!((start.1 - center(node).1).abs() < 1e-8);
    }
}

#[test]
fn c4_person_and_owning_boundary_overlap_render_the_upstream_degenerate_points() {
    let source = r#"%%{init: {"c4":{"c4ShapeMargin":0}}}%%
C4Context
Boundary(boundary, "Boundary") {
  Person(person, "Person")
}
Rel(person, boundary, "Member")
Rel(boundary, person, "Contains")
"#;
    let (layout, svg) = render(source);
    let document = roxmltree::Document::parse(&svg).expect("valid coincident C4 SVG");
    let relations: Vec<_> = document
        .descendants()
        .filter(|node| node.attribute("marker-end").is_some())
        .collect();
    assert_eq!(relations.len(), 2);
    assert!(!svg.contains("NaN"));
    let person = shape(&layout, "person");
    let boundary = layout
        .boundaries
        .iter()
        .find(|node| node.alias == "boundary")
        .expect("owning boundary");
    let person_center = center(person);
    let boundary_center = (
        boundary.x + boundary.width / 2.0,
        boundary.y + boundary.height / 2.0,
    );
    assert_eq!(person_center, boundary_center);
    let boundary_right = (boundary.x + boundary.width, boundary_center.1);
    let first = relation_points(relations[0]);
    let second = relation_points(relations[1]);
    assert_eq!(first[0], person_center);
    assert_eq!(first[1], boundary_right);
    assert_eq!(second[0], boundary_right);
    assert_eq!(second[1], person_center);
}

#[test]
fn c4_person_relation_to_close_owning_boundary_uses_the_nearest_line_intersection() {
    let source = r#"%%{init: {"c4":{"c4ShapeMargin":0.25}}}%%
C4Context
Boundary(boundary, "Boundary") {
  Person(person, "Person")
}
Rel(person, boundary, "Member")
"#;
    let (layout, svg) = render(source);
    let document = roxmltree::Document::parse(&svg).expect("valid near-coincident C4 SVG");
    let relation = document
        .descendants()
        .find(|node| node.attribute("marker-end").is_some())
        .expect("person to boundary relation");
    let person = shape(&layout, "person");
    let boundary = layout
        .boundaries
        .iter()
        .find(|node| node.alias == "boundary")
        .expect("owning boundary");
    let person_center = center(person);
    let boundary_center = (
        boundary.x + boundary.width / 2.0,
        boundary.y + boundary.height / 2.0,
    );
    assert_eq!(person_center.0, boundary_center.0);
    assert!(boundary_center.1 < person_center.1);
    let [start, end] = relation_points(relation);
    // The pinned polygon helper picks the boundary point nearest the target on the full line.
    // Its sampled person bottom is closer than the head top for this legal nested relationship,
    // despite the boundary center lying slightly above the person center.
    assert!((start.0 - person_center.0).abs() < 1e-8);
    assert!(start.1 > person_center.1);
    assert!(start.1 < person.y + person.height);
    assert!((end.1 - boundary.y - boundary.height).abs() < 1e-8);
    assert!(start.0.is_finite() && start.1.is_finite() && end.0.is_finite() && end.1.is_finite());
}

#[test]
fn c4_framed_self_relation_uses_the_polygon_center() {
    let (layout, svg) = render(
        r#"C4Context
System(node, "Node", $shape="component")
BiRel(node, node, "Self")
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid framed self relation SVG");
    let relation = document
        .descendants()
        .find(|node| node.attribute("marker-end").is_some())
        .expect("framed self relation");
    let [start, end] = relation_points(relation);
    let center = center(shape(&layout, "node"));
    assert_eq!(start, center);
    assert_eq!(end, center);
    assert!(relation.attribute("marker-start").is_some());
}
