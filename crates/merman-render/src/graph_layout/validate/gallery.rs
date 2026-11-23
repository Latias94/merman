//! Replay flat Shoal drawings at their intrinsic viewBox size (one SVG unit per pixel).
//! `MERMAN_SHOAL_GALLERY` optionally names the evidence directory containing gallery-facts.json
//! and sources/flat. Each replay paints the current renderer, including when the corpus is frozen.

#![cfg(all(feature = "diagram-flowchart", feature = "layout-shoal"))]

use super::{
    Body, CrossAxis, Edge, End, Endpoint, FLATNESS, Label, LabelOwner, PointMarker, Report, Scene,
    Violation, fork_join_offsets, validate,
};
use crate::{
    Error, LayoutOptions, RenderCapability,
    environment::RenderEnvironment,
    family,
    model::{FlowchartLayout, LayoutNode, LayoutPoint},
    svg::SvgRenderOptions,
};
use kurbo::{Affine, BezPath, Circle, Point, Rect, RoundedRect, Shape, Size, Vec2};
use merman_core::{MermaidEngine, ParseOptions, presentation::SourcePresentationConfig};
use merman_model::families::flowchart::{FlowEdgeMarker, FlowchartModel};
use roxmltree::{Document, Node};
use serde_json::json;
use std::{collections::HashMap, fs, path::PathBuf};

const RELEASE: &str = include_str!("../../../tests/fixtures/shoal-release.mmd");
const PORTS_HEAVY: &str =
    include_str!("../../../../merman/benches/fixtures/flowchart_ports_heavy.mmd");

struct Drawing {
    model: FlowchartModel,
    layout: FlowchartLayout,
    svg: String,
}

fn render(source: &str, diagram_id: &str) -> crate::Result<Drawing> {
    let engine = MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(
        json!({"layout": "shoal", "htmlLabels": true}),
    ));
    let diagram = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap_or_else(|e| panic!("{diagram_id}: {e}"))
        .expect("flowchart")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("deterministic gallery render session");
    let artifact = family::prepare(&diagram, &LayoutOptions::headless_svg_defaults(), session)?;
    let snapshot = artifact.layout_json()?;
    let rendered = artifact.render_svg(
        &SvgRenderOptions {
            diagram_id: Some(diagram_id.into()),
            ..Default::default()
        },
        &Default::default(),
    )?;
    assert!(
        rendered
            .required_capabilities()
            .contains(&RenderCapability::LayoutShoal),
        "{diagram_id}: expected Shoal"
    );
    Ok(Drawing {
        model: serde_json::from_value(snapshot["semantic"].clone())?,
        layout: serde_json::from_value(snapshot["layout"]["FlowchartV2"].clone())?,
        svg: rendered.svg().to_owned(),
    })
}

fn attr<'a>(n: Node<'a, '_>, key: &str) -> &'a str {
    n.attribute(key).unwrap_or_else(|| {
        panic!(
            "missing {key} on <{}> {:?}",
            n.tag_name().name(),
            n.attribute("data-id")
        )
    })
}

fn has_class(n: Node<'_, '_>, class: &str) -> bool {
    n.attribute("class")
        .is_some_and(|s| s.split_ascii_whitespace().any(|c| c == class))
}

fn number(n: Node<'_, '_>, key: &str) -> f64 {
    attr(n, key).parse().unwrap()
}

fn rect(n: Node<'_, '_>) -> Rect {
    let origin = |key| n.attribute(key).map_or(0., |s| s.parse::<f64>().unwrap());
    Rect::from_origin_size(
        (origin("x"), origin("y")),
        (number(n, "width"), number(n, "height")),
    )
}

fn view_box(n: Node<'_, '_>) -> Rect {
    let v: svgtypes::ViewBox = attr(n, "viewBox").parse().unwrap();
    Rect::from_origin_size((v.x, v.y), (v.w, v.h))
}

/// Flowchart emits the SVG default xMidYMid meet. Resolve fractional origins and letterboxing
/// before the oracle flattens shafts, so its tolerance is in the selected viewport's pixels.
fn viewport(root: Node<'_, '_>, size: Size) -> Affine {
    assert!(matches!(
        root.attribute("preserveAspectRatio"),
        None | Some("xMidYMid" | "xMidYMid meet")
    ));
    let v = view_box(root);
    let scale = (size.width / v.width()).min(size.height / v.height());
    Affine::translate((size.width / 2., size.height / 2.))
        * Affine::scale(scale)
        * Affine::translate(-v.center().to_vec2())
}

fn to_viewport(n: Node<'_, '_>, frame: Affine) -> Affine {
    let mut local = Affine::IDENTITY;
    for raw in n.ancestors().filter_map(|a| a.attribute("transform")) {
        let t: svgtypes::Transform = raw.parse().unwrap();
        local = Affine::new([t.a, t.b, t.c, t.d, t.e, t.f]) * local;
    }
    frame * local
}

/// The first and last layout JSON waypoints are the declared ports on the measured boxes,
/// before the painter insets a contact to the actual shape and trims the marker. Read the
/// face independently at each end, so reverse edges and same-side loops retain their sign.
fn port_normal(body: &LayoutNode, port: &LayoutPoint, body_frame: Affine) -> Vec2 {
    let (x, y) = (port.x - body.x, port.y - body.y);
    let (w, h) = (body.width * 0.5, body.height * 0.5);
    let on_x = (x.abs() - w).abs() < 1e-6 && y.abs() <= h + 1e-6;
    let on_y = (y.abs() - h).abs() < 1e-6 && x.abs() <= w + 1e-6;
    let outward = match (on_x, on_y) {
        (true, false) => Point::new(x.signum(), 0.),
        (false, true) => Point::new(0., y.signum()),
        _ => panic!("{}: port {port:?} does not identify one box face", body.id),
    };
    (body_frame * outward - body_frame * Point::ORIGIN).normalize()
}

fn contour(owner: Node<'_, '_>, frame: Affine) -> BezPath {
    let container = owner
        .descendants()
        .find(|n| has_class(*n, "label-container"))
        .unwrap_or_else(|| panic!("{}: missing shape", attr(owner, "data-id")));
    // Neo stadiums paint the closed fill and outline separately. The fill describes the body.
    let shape = if container.has_tag_name("g") {
        container
            .descendants()
            .find(|n| {
                n.has_tag_name("path") && n.attribute("fill").is_some_and(|fill| fill != "none")
            })
            .expect("filled outline in the gallery shape group")
    } else {
        container
    };
    let path = match shape.tag_name().name() {
        "rect" => {
            let r = rect(shape);
            let radius = |key| shape.attribute(key).map(|s| s.parse::<f64>().unwrap());
            let (rx, ry) = (radius("rx"), radius("ry"));
            assert_eq!(
                rx.or(ry),
                ry.or(rx),
                "gallery rectangle has elliptical corners"
            );
            RoundedRect::from_rect(r, rx.or(ry).unwrap_or(0.)).to_path(FLATNESS)
        }
        "polygon" => {
            let mut vertices = svgtypes::PointsParser::from(attr(shape, "points"));
            let mut path = BezPath::new();
            path.move_to(vertices.next().expect("polygon start"));
            for p in vertices {
                path.line_to(p);
            }
            path.close_path();
            path
        }
        "circle" => Circle::new(
            (number(shape, "cx"), number(shape, "cy")),
            number(shape, "r"),
        )
        .to_path(FLATNESS),
        "path" => BezPath::from_svg(attr(shape, "d")).expect("node outline"),
        tag => panic!(
            "{}: unsupported gallery shape <{tag}>",
            attr(owner, "data-id")
        ),
    };
    let path = to_viewport(shape, frame) * path;
    let bounds = path.bounding_box();
    assert!(path.is_finite() && bounds.width() > 0. && bounds.height() > 0.);
    path
}

/// HTML labels expose their measured local box on foreignObject. An unlabeled edge has a
/// zero-sized placeholder in the same DOM position as a visible label.
fn label_box(group: Node<'_, '_>, frame: Affine) -> Option<Rect> {
    let element = group
        .descendants()
        .find(|n| n.has_tag_name("foreignObject"))
        .expect("gallery uses HTML labels with explicit measured extents");
    let local = rect(element);
    assert!(local.is_finite() && local.width() >= 0. && local.height() >= 0.);
    (local.width() > 0. && local.height() > 0.)
        .then(|| to_viewport(element, frame).transform_rect_bbox(local))
}

fn read_marker(
    path: Node<'_, '_>,
    markers: &HashMap<&str, Node<'_, '_>>,
    end: End,
    expected: FlowEdgeMarker,
    trim: f64,
    transform: Affine,
    endpoint: &mut Endpoint,
) {
    let (key, side) = match end {
        End::Start => ("marker-start", "Start"),
        End::End => ("marker-end", "End"),
    };
    let kind = match expected {
        FlowEdgeMarker::None => {
            assert!(path.attribute(key).is_none(), "unexpected {key}");
            return;
        }
        FlowEdgeMarker::Point => "point",
        FlowEdgeMarker::Circle => "circle",
        FlowEdgeMarker::Cross => "cross",
    };
    let id = attr(path, key)
        .strip_prefix("url(#")
        .and_then(|s| s.strip_suffix(')'))
        .expect("local marker IRI");
    let marker = markers[id];
    let base = id
        .rsplit_once("flowchart-v2-")
        .unwrap()
        .1
        .split(['-', '_'])
        .next()
        .unwrap();
    assert_eq!(
        base,
        format!("{kind}{side}"),
        "{} {key}",
        attr(path, "data-id")
    );
    assert_eq!(attr(marker, "markerUnits"), "userSpaceOnUse");
    let [a, b, c, d, ..] = transform.as_coeffs();
    let scale = a.hypot(b);
    // A scalar marker setback/opening is valid for the similarity transforms used on shafts.
    assert!(scale > 0. && (c.hypot(d) - scale).abs() < 1e-8 && (a * c + b * d).abs() < 1e-8);
    endpoint.setback = trim * scale;
    if expected == FlowEdgeMarker::Point {
        endpoint.marker = Some(
            PointMarker::from_view_box(
                end,
                view_box(marker),
                Size::new(
                    number(marker, "markerWidth") * scale,
                    number(marker, "markerHeight") * scale,
                ),
                number(marker, "refX"),
            )
            .expect("positive point-marker opening"),
        );
    }
}

impl Drawing {
    fn axis(&self) -> CrossAxis {
        match self.model.direction.as_deref() {
            Some("TD" | "TB" | "BT") => CrossAxis::X,
            Some("LR" | "RL") => CrossAxis::Y,
            other => panic!("unknown gallery direction {other:?}"),
        }
    }

    fn node(&self, id: &str) -> usize {
        self.layout.nodes.iter().position(|n| n.id == id).unwrap()
    }

    fn scene<'a>(&self, doc: &'a Document<'_>, size: Size) -> Scene<'a> {
        let frame = viewport(doc.root_element(), size);
        let mut nodes = HashMap::new();
        let mut markers = HashMap::new();
        let mut paths = Vec::new();
        let mut labels = Vec::new();
        for n in doc.descendants().filter(Node::is_element) {
            match (n.tag_name().name(), n.attribute("data-et")) {
                ("g", Some("node")) => assert!(nodes.insert(attr(n, "data-id"), n).is_none()),
                ("marker", _) => assert!(markers.insert(attr(n, "id"), n).is_none()),
                ("path", Some("edge")) => paths.push(n),
                ("g", _)
                    if has_class(n, "label")
                        && n.parent_element()
                            .is_some_and(|p| has_class(p, "edgeLabel")) =>
                {
                    labels.push(n)
                }
                _ => {}
            }
        }
        assert!(self.layout.clusters.is_empty(), "flat gallery");
        assert_eq!(self.model.nodes.len(), self.layout.nodes.len());
        assert_eq!(nodes.len(), self.layout.nodes.len());
        assert_eq!(self.model.edges.len(), self.layout.edges.len());
        assert_eq!(paths.len(), self.layout.edges.len());
        assert_eq!(paths.len(), labels.len(), "one edge-label slot per shaft");

        let mut scene = Scene::default();
        let mut body_ids = HashMap::new();
        for (i, n) in self.layout.nodes.iter().enumerate() {
            let owner = nodes.remove(n.id.as_str()).expect("layout node in SVG");
            let body_frame = to_viewport(owner, frame);
            assert!(body_ids.insert(n.id.as_str(), (i, body_frame)).is_none());
            // The oracle counts box entries against the measured dimensions at the painted
            // center. Endpoint contact uses the separate SVG contour, including shape transforms.
            let bounds = body_frame
                .transform_rect_bbox(Rect::from_center_size((0., 0.), (n.width, n.height)));
            assert!(bounds.is_finite() && bounds.width() > 0. && bounds.height() > 0.);
            scene.bodies.push(Body {
                contour: Some(contour(owner, frame)),
                ..Body::node(bounds)
            });
            let label = owner
                .descendants()
                .find(|n| n.has_tag_name("g") && has_class(*n, "label"))
                .expect("node label group");
            scene.labels.push(Label {
                owner: LabelOwner::Body(i),
                bounds: label_box(label, frame).expect("visible gallery node label"),
            });
        }
        // Both SVG passes iterate the same flat render order. Pair their slots before matching
        // layout occurrences; parallel edges and colliding public data-ids must stay distinct.
        let mut painted = HashMap::<_, Vec<_>>::new();
        for (path, label) in paths.into_iter().zip(labels) {
            let id = attr(path, "data-id");
            assert_eq!(id, attr(label, "data-id"));
            painted.entry(id).or_default().push((path, label));
        }
        for (i, (le, semantic)) in self.layout.edges.iter().zip(&self.model.edges).enumerate() {
            assert_eq!(
                (&le.id, &le.from, &le.to),
                (&semantic.id, &semantic.from, &semantic.to)
            );
            let slots = painted.get_mut(le.id.as_str()).expect("layout edge in SVG");
            let slot = if slots.len() > 1 {
                let suffix = format!("-edge-{i}");
                slots
                    .iter()
                    .position(|(path, _)| attr(*path, "id").ends_with(&suffix))
                    .expect("scoped DOM id for duplicate data-id")
            } else {
                0
            };
            let (path, label) = slots.swap_remove(slot);
            let (from, from_frame) = body_ids[le.from.as_str()];
            let (to, to_frame) = body_ids[le.to.as_str()];
            let mut edge = Edge::new(
                from,
                to,
                path.attribute("d").unwrap_or_default(),
                self.axis(),
            );
            let [first, .., last] = le.points.as_slice() else {
                panic!("{}: missing layout ports", le.id)
            };
            edge.start.outward_normal =
                Some(port_normal(&self.layout.nodes[from], first, from_frame));
            edge.end.outward_normal = Some(port_normal(&self.layout.nodes[to], last, to_frame));
            edge.transform = to_viewport(path, frame);
            let (start, end) = crate::svg::flowchart_marker_metrics(semantic);
            read_marker(
                path,
                &markers,
                End::Start,
                semantic.start_marker,
                start.trim,
                edge.transform,
                &mut edge.start,
            );
            read_marker(
                path,
                &markers,
                End::End,
                semantic.end_marker,
                end.trim,
                edge.transform,
                &mut edge.end,
            );
            scene.edges.push(edge);
            let bounds = label_box(label, frame);
            let labeled = semantic
                .label
                .as_deref()
                .is_some_and(|text| !text.trim().is_empty());
            assert_eq!(le.label.is_some(), labeled, "{} layout label", le.id);
            assert_eq!(bounds.is_some(), labeled, "{} SVG label", le.id);
            if let Some(bounds) = bounds {
                scene.labels.push(Label {
                    owner: LabelOwner::Edge(i),
                    bounds,
                });
            }
        }
        scene
    }
}

fn replay(name: &str, drawing: &Drawing) -> Report {
    let doc = Document::parse(&drawing.svg).expect("well-formed rendered SVG");
    let scene = drawing.scene(&doc, view_box(doc.root_element()).size());
    let report = validate(&scene);
    assert_eq!(report.edges.len(), scene.edges.len());
    for (i, measured) in report.edges.iter().enumerate() {
        assert!(
            measured.is_some_and(|m| m.length.is_finite()
                && m.length > 0.
                && m.sideways_return.is_finite()),
            "{name} shaft {i} ({}): {measured:?}",
            drawing.layout.edges[i].id
        );
    }
    assert!(
        !report.hard.iter().any(|v| matches!(
            v,
            Violation::InvalidOwner { .. }
                | Violation::InvalidPath { .. }
                | Violation::InvalidNormal { .. }
        )),
        "{name}: {:?}",
        report.hard
    );

    let edge_labels = scene
        .labels
        .iter()
        .filter(|l| matches!(l.owner, LabelOwner::Edge(_)))
        .count();
    let starts = scene
        .edges
        .iter()
        .filter(|e| e.start.marker.is_some())
        .count();
    let ends = scene
        .edges
        .iter()
        .filter(|e| e.end.marker.is_some())
        .count();
    assert_eq!(report.contacts_checked, 2 * scene.edges.len(), "{name}");
    assert_eq!(report.normals_checked, 2 * scene.edges.len(), "{name}");
    assert_eq!(report.markers_checked, starts + ends, "{name}");
    let mut node_hits = 0;
    let mut node_label_hits = 0;
    let mut edge_label_hits = 0;
    let mut label_overlaps = 0;
    let mut label_node_overlaps = 0;
    let mut contour_errors = 0;
    let mut normal_errors = 0;
    let mut marker_angle_errors = 0;
    for v in &report.hard {
        match *v {
            Violation::ForeignNodeHit { .. } => node_hits += 1,
            Violation::ForeignLabelHit { label, .. } => match scene.labels[label].owner {
                LabelOwner::Body(_) => node_label_hits += 1,
                LabelOwner::Edge(_) => edge_label_hits += 1,
            },
            Violation::LabelOverlap { .. } => label_overlaps += 1,
            Violation::LabelNodeOverlap { .. } => label_node_overlaps += 1,
            Violation::EndpointContour { .. } => contour_errors += 1,
            Violation::EndpointNormal { .. } => normal_errors += 1,
            Violation::MarkerAngle { .. } => marker_angle_errors += 1,
            _ => {}
        }
    }
    eprintln!(
        "{name} nodes={} shafts={} node_labels={} edge_labels={edge_labels} point_starts={starts} point_ends={ends} contacts={} normals={} contour_errors={contour_errors} normal_errors={normal_errors} marker_angle_errors={marker_angle_errors} foreign_nodes={node_hits} foreign_node_labels={node_label_hits} foreign_edge_labels={edge_label_hits} label_overlaps={label_overlaps} label_node_overlaps={label_node_overlaps} crossings={} hard={}",
        scene.bodies.len(),
        scene.edges.len(),
        scene.labels.len() - edge_labels,
        report.contacts_checked,
        report.normals_checked,
        report.crossings.len(),
        report.hard.len()
    );
    if name == "release" || name.starts_with("release-") {
        let (fork, join) = fork_join_offsets(
            &scene,
            drawing.node("stage"),
            drawing.node("ship"),
            &[drawing.node("smoke"), drawing.node("audit")],
            drawing.axis(),
        )
        .expect("release fork/join topology");
        let audit_ship = drawing
            .layout
            .edges
            .iter()
            .position(|e| e.from == "audit" && e.to == "ship")
            .unwrap();
        assert!(fork.is_finite() && join.is_finite());
        eprintln!(
            "{name} fork_offset={fork} join_offset={join} audit_ship_return={}",
            report.edges[audit_ship].unwrap().sideways_return
        );
    }
    report
}

fn replay_ci_fixtures() {
    for (dir, canonical) in [("TD", "TB"), ("BT", "BT"), ("LR", "LR"), ("RL", "RL")] {
        let name = format!("release-{}", dir.to_ascii_lowercase());
        let source = RELEASE.replacen("flowchart TD", &format!("flowchart {dir}"), 1);
        let drawing = render(&source, &name).unwrap();
        assert_eq!(drawing.model.direction.as_deref(), Some(canonical));
        assert_eq!(
            (drawing.layout.nodes.len(), drawing.layout.edges.len()),
            (8, 8)
        );
        assert_eq!(
            drawing
                .layout
                .edges
                .iter()
                .filter(|e| e.label.is_some())
                .count(),
            2
        );
        replay(&name, &drawing);
    }
    let dense = render(PORTS_HEAVY, "ports-heavy").unwrap();
    assert_eq!(
        (dense.layout.nodes.len(), dense.layout.edges.len()),
        (18, 39)
    );
    assert_eq!(
        dense
            .layout
            .edges
            .iter()
            .filter(|e| e.label.is_some())
            .count(),
        7
    );
    replay("ports-heavy", &dense);
}

#[test]
fn duplicate_edge_ids_and_markers_at_both_ends() {
    // The authored id on X -> Y collides with the generated id on A -> B.
    let drawing = render("flowchart LR\nX((Circle)) L_A_B_0@-->|first| Y{Diamond}\nA -->|second| B\nA o--o X\nB <--> Y\nX x--x B", "marker-occurrences").unwrap();
    assert_eq!(drawing.layout.edges[0].id, drawing.layout.edges[1].id);
    let report = replay("marker-occurrences", &drawing);
    assert_eq!(report.markers_checked, 4); // Three target points and one source point.
}

#[test]
fn viewport_and_nested_svg_transforms_preserve_capture() {
    let drawing = render(RELEASE, "viewport").unwrap();
    let doc = Document::parse(&drawing.svg).unwrap();
    let root = doc.root_element();
    let v = view_box(root);
    // Exercise xMidYMid meet with horizontal letterboxing as well as a nonzero viewBox origin.
    let size = Size::new(v.width() + 60., v.height());
    let before = drawing.scene(&doc, size);
    let stage = drawing.node("stage");
    let layout = &drawing.layout.nodes[stage];
    let expected = Point::new(layout.x - v.x0 + 30., layout.y - v.y0);
    assert!(before.bodies[stage].bounds.center().distance(expected) < 1e-5);

    // Re-express exactly the same drawing in a translated, doubled SVG user space.
    // Apply edits from the end so that the original XML byte offsets remain valid.
    let g = root
        .children()
        .find(|n| n.has_tag_name("g"))
        .expect("graph group");
    let mut svg = wrap_in_transform(&drawing.svg, g, "translate(31.25 -17.75) scale(2)");
    svg.replace_range(
        root.attribute_node("viewBox").unwrap().range_value(),
        &format!(
            "{} {} {} {}",
            2. * v.x0 + 31.25,
            2. * v.y0 - 17.75,
            2. * v.width(),
            2. * v.height()
        ),
    );
    let transformed = Document::parse(&svg).unwrap();
    let after = drawing.scene(&transformed, size);
    for (a, b) in before.bodies.iter().zip(&after.bodies) {
        close_rect(a.bounds, b.bounds);
        close_rect(
            a.contour.as_ref().unwrap().bounding_box(),
            b.contour.as_ref().unwrap().bounding_box(),
        );
    }
    for (a, b) in before.labels.iter().zip(&after.labels) {
        assert_eq!(a.owner, b.owner);
        close_rect(a.bounds, b.bounds);
    }
    for (a, b) in before.edges.iter().zip(&after.edges) {
        for (x, y) in a
            .transform
            .as_coeffs()
            .into_iter()
            .zip(b.transform.as_coeffs())
        {
            assert!((x - y).abs() < 1e-8);
        }
        assert_eq!(a.end.setback, b.end.setback);
        assert_eq!(a.end.marker.unwrap().depth, b.end.marker.unwrap().depth);
        assert_eq!(a.start.outward_normal, b.start.outward_normal);
        assert_eq!(a.end.outward_normal, b.end.outward_normal);
    }
}

fn close_rect(a: Rect, b: Rect) {
    assert!(
        a.origin().distance(b.origin()) < 1e-8
            && (a.width() - b.width()).abs() < 1e-8
            && (a.height() - b.height()).abs() < 1e-8,
        "{a:?} != {b:?}"
    );
}

fn wrap_in_transform(svg: &str, node: Node<'_, '_>, transform: &str) -> String {
    let mut wrapped = svg.to_owned();
    wrapped.insert_str(node.range().end, "</g>");
    wrapped.insert_str(
        node.range().start,
        &format!(r#"<g transform="{transform}">"#),
    );
    wrapped
}

#[test]
fn moving_only_the_svg_edge_label_reaches_the_hard_overlap_report() {
    let drawing = render("flowchart LR\nA[Build] -->|ready| B[Ship]", "label-control").unwrap();
    let doc = Document::parse(&drawing.svg).unwrap();
    let size = view_box(doc.root_element()).size();
    let scene = drawing.scene(&doc, size);
    let clean = validate(&scene);
    assert!(
        clean.hard.is_empty(),
        "straight two-box control: {:?}",
        clean.hard
    );

    let node = drawing.node("A");
    let label_for = |owner| scene.labels.iter().position(|l| l.owner == owner).unwrap();
    let first = label_for(LabelOwner::Body(node));
    let second = label_for(LabelOwner::Edge(0));
    let outer = doc
        .descendants()
        .find(|n| n.has_tag_name("g") && has_class(*n, "edgeLabel"))
        .unwrap();
    let local = to_viewport(
        outer.parent_element().unwrap(),
        viewport(doc.root_element(), size),
    )
    .inverse();
    let delta =
        local * scene.labels[first].bounds.center() - local * scene.labels[second].bounds.center();
    let svg = wrap_in_transform(
        &drawing.svg,
        outer,
        &format!("translate({} {})", delta.x, delta.y),
    );
    let moved = Document::parse(&svg).unwrap();
    let report = validate(&drawing.scene(&moved, size));
    assert!(
        report
            .hard
            .contains(&Violation::LabelOverlap { first, second })
    );
    assert!(report.hard.contains(&Violation::LabelNodeOverlap {
        label: second,
        node
    }));
}

#[test]
fn foreign_hits_follow_the_svg_curve_with_the_same_layout_json() {
    let drawing = render(RELEASE, "curve-control").unwrap();
    let doc = Document::parse(&drawing.svg).unwrap();
    let size = view_box(doc.root_element()).size();
    let scene = drawing.scene(&doc, size);
    let edge = drawing
        .layout
        .edges
        .iter()
        .position(|e| e.from == "change" && e.to == "checks")
        .unwrap();
    let audit = drawing.node("audit");
    let yes = drawing
        .layout
        .edges
        .iter()
        .position(|e| e.from == "checks" && e.to == "stage")
        .unwrap();
    let path = doc
        .descendants()
        .find(|n| {
            n.attribute("data-et") == Some("edge")
                && n.attribute("data-id") == Some(&drawing.layout.edges[edge].id)
        })
        .unwrap();
    let range = path.attribute_node("d").unwrap().range_value();
    let local = scene.edges[edge].transform.inverse();
    assert_eq!(FLATNESS, 0.15);
    for owner in [LabelOwner::Body(audit), LabelOwner::Edge(yes)] {
        let label = scene.labels.iter().position(|l| l.owner == owner).unwrap();
        let bounds = scene.labels[label].bounds;
        let p = bounds.center();
        // The chord stays above the obstacle; the quadratic with the same ends goes through
        // its center. Both variants preserve every other DOM attribute and the JSON waypoints.
        let top = if owner == LabelOwner::Body(audit) {
            bounds.y0.min(scene.bodies[audit].bounds.y0)
        } else {
            bounds.y0
        } - 4.;
        let start = local * Point::new(bounds.x0 - 4., top);
        let end = local * Point::new(bounds.x1 + 4., top);
        let control = local * Point::new(p.x, 2. * p.y - top);
        for curved in [false, true] {
            let mid = if curved {
                format!("Q{} {} ", control.x, control.y)
            } else {
                "L".into()
            };
            let mut svg = drawing.svg.clone();
            svg.replace_range(
                range.clone(),
                &format!("M{} {} {mid}{} {}", start.x, start.y, end.x, end.y),
            );
            let variant = Document::parse(&svg).unwrap();
            let report = validate(&drawing.scene(&variant, size));
            if !curved {
                // Both declared ports are vertical. Replacing only d with a horizontal chord
                // must report both tangents even though their replacement remains axis-aligned.
                for end in [End::Start, End::End] {
                    assert!(report.hard.iter().any(|v| matches!(v, Violation::EndpointNormal { edge: e, end: side, .. } if *e == edge && *side == end)));
                }
            }
            assert_eq!(
                report
                    .hard
                    .contains(&Violation::ForeignLabelHit { edge, label }),
                curved,
                "{owner:?}"
            );
            if owner == LabelOwner::Body(audit) {
                assert_eq!(
                    report
                        .hard
                        .contains(&Violation::ForeignNodeHit { edge, node: audit }),
                    curved
                );
            }
        }
    }
}

#[test]
fn replay_visual_corpus() {
    let Some(dir) = std::env::var_os("MERMAN_SHOAL_GALLERY") else {
        return replay_ci_fixtures();
    };
    let dir = PathBuf::from(dir);
    let path = dir.join("gallery-facts.json");
    let facts: serde_json::Value = serde_json::from_slice(
        &fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())),
    )
    .unwrap();
    let cases = facts["cases"].as_object().expect("gallery cases");
    assert!(!cases.is_empty());
    for (name, case) in cases {
        let path = dir.join("sources/flat").join(format!("{name}.mmd"));
        let source =
            fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let drawing = match render(&source, case["diagram_id"].as_str().unwrap()) {
            Ok(drawing) => drawing,
            // These two frozen inputs predate preview support. Start measuring each as soon as
            // the backend accepts it. Other rejections are collection failures.
            Err(Error::UnsupportedLayoutConstruct {
                backend: "shoal",
                construct,
            }) if matches!(
                (name.as_str(), construct),
                ("self-loop", "self-loops") | ("cycle", "directed cycles")
            ) =>
            {
                eprintln!("{name} unsupported={construct}");
                continue;
            }
            Err(e) => panic!("{name}: {e}"),
        };
        assert_eq!(
            drawing.model.direction.as_deref(),
            case["direction"].as_str(),
            "{name}"
        );
        assert_eq!(
            drawing.layout.nodes.len(),
            case["nodes"].as_array().unwrap().len(),
            "{name}"
        );
        assert_eq!(
            drawing.layout.edges.len(),
            case["edges"].as_array().unwrap().len(),
            "{name}"
        );
        replay(name, &drawing);
    }
}
