//! Replay flat Shoal drawings at their intrinsic viewBox size (one SVG unit per pixel).
//! The 19 visual fixtures run by default. `MERMAN_SHOAL_GALLERY` selects the archived sources.
//! The ignored stress replay reads `MERMAN_SHOAL_STRESS_PLAN` or the archive's `stress.json`.

#![cfg(all(feature = "diagram-flowchart", feature = "layout-shoal"))]

#[cfg(feature = "diagram-agentflow")]
mod aesthetic_witnesses;

#[cfg(feature = "diagram-agentflow")]
mod compound;

use super::{
    Body, CrossAxis, Edge, End, Endpoint, FLATNESS, Label, LabelOwner, PointMarker, Report, Scene,
    Shaft, Violation, fork_join_offsets, validate,
};
use crate::{
    LayoutAdaptation, LayoutAdaptationObservation, LayoutOptions, RenderCapability,
    environment::RenderEnvironment,
    family,
    model::{FlowchartLayout, LayoutNode, LayoutPoint},
    svg::SvgRenderOptions,
};
use kurbo::{
    Affine, BezPath, Circle, Line, ParamCurve, ParamCurveNearest, PathSeg, Point, Rect,
    RoundedRect, Shape, Size, Vec2,
};
use merman_core::{MermaidEngine, ParseOptions, presentation::SourcePresentationConfig};
use merman_model::families::flowchart::{FlowEdgeMarker, FlowchartModel};
use roxmltree::{Document, Node};
use serde_json::json;
use std::{
    collections::HashMap,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
};

const RELEASE: &str = include_str!("../../../tests/fixtures/shoal-release.mmd");
const CYCLE_LOOPS: &str = include_str!("../../../tests/fixtures/shoal-cycle-loops.mmd");
const CYCLE: &str = include_str!(
    "../../../../merman-ascii/tests/testdata/local-semantic/flowchart/scene_planning/labelled-cycle-capacity.mmd"
);
const PORTS_HEAVY: &str =
    include_str!("../../../../merman/benches/fixtures/flowchart_ports_heavy.mmd");

struct Drawing {
    model: FlowchartModel,
    layout: FlowchartLayout,
    svg: String,
}

fn engine() -> MermaidEngine {
    MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(
        json!({"layout": "shoal", "htmlLabels": true}),
    ))
}

fn render(source: &str, diagram_id: &str) -> crate::Result<Drawing> {
    render_with_options(source, diagram_id, &LayoutOptions::headless_svg_defaults())
        .map(|(drawing, _)| drawing)
}

#[cfg(feature = "diagram-agentflow")]
fn render_studio(source: &str, diagram_id: &str, html_labels: bool) -> crate::Result<Drawing> {
    let engine = MermaidEngine::new().with_site_config(SourcePresentationConfig::from_value(
        json!({"layout": "shoal", "htmlLabels": html_labels}),
    ));
    let diagram = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap_or_else(|e| panic!("{diagram_id}: {e}"))
        .expect("flowchart")
        .into_presentation();
    let theme = crate::theme::ThemeCompiler::new()
        .compile_json(br#"{"schema_version":2,"extends":"studio"}"#, None)
        .expect("studio theme preset");
    let session = RenderEnvironment::deterministic()
        .begin_session_with_native_theme(crate::theme::NativeTheme::new(theme))
        .expect("deterministic studio gallery render session");
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

fn render_with_options(
    source: &str,
    diagram_id: &str,
    options: &LayoutOptions,
) -> crate::Result<(Drawing, Option<LayoutAdaptationObservation>)> {
    let diagram = engine()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap_or_else(|e| panic!("{diagram_id}: {e}"))
        .expect("flowchart")
        .into_presentation();
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("deterministic gallery render session");
    let artifact = family::prepare(&diagram, options, session)?;
    let adaptation = artifact.layout_adaptation();
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
    Ok((
        Drawing {
            model: serde_json::from_value(snapshot["semantic"].clone())?,
            layout: serde_json::from_value(snapshot["layout"]["FlowchartV2"].clone())?,
            svg: rendered.svg().to_owned(),
        },
        adaptation,
    ))
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
    let size = Size::new(number(n, "width"), number(n, "height"));
    assert!(size.is_finite() && size.width >= 0. && size.height >= 0.);
    Rect::from_origin_size((origin("x"), origin("y")), size)
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
fn port_normal(
    body: &LayoutNode,
    port: &LayoutPoint,
    shoulder: &LayoutPoint,
    body_frame: Affine,
) -> Vec2 {
    let (x, y) = (port.x - body.x, port.y - body.y);
    let (w, h) = (body.width * 0.5, body.height * 0.5);
    let on_x = (x.abs() - w).abs() < 1e-6 && y.abs() <= h + 1e-6;
    let on_y = (y.abs() - h).abs() < 1e-6 && x.abs() <= w + 1e-6;
    let outward = match (on_x, on_y) {
        (true, false) => Point::new(x.signum(), 0.),
        (false, true) => Point::new(0., y.signum()),
        (true, true)
            if (shoulder.y - port.y).abs() < 1e-6 && (shoulder.x - port.x) * x.signum() > 0. =>
        {
            Point::new(x.signum(), 0.)
        }
        (true, true)
            if (shoulder.x - port.x).abs() < 1e-6 && (shoulder.y - port.y) * y.signum() > 0. =>
        {
            Point::new(0., y.signum())
        }
        _ => panic!("{}: port {port:?} does not identify one box face", body.id),
    };
    (body_frame * outward - body_frame * Point::ORIGIN).normalize()
}

fn contour(owner: Node<'_, '_>, frame: Affine, fork_join: bool) -> BezPath {
    if fork_join {
        let fill = owner
            .descendants()
            .find(|node| {
                node.has_tag_name("path")
                    && node.attribute("fill").is_some_and(|fill| fill != "none")
            })
            .expect("filled fork/join contour");
        let mut outline = painted_outline(fill, frame);
        outline.close_path();
        return outline;
    }
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
    painted_outline(shape, frame)
}

fn painted_outline(shape: Node<'_, '_>, frame: Affine) -> BezPath {
    let path = match shape.tag_name().name() {
        "rect" => {
            let r = rect(shape);
            let radius = |key| shape.attribute(key).map(|s| s.parse::<f64>().unwrap());
            let (rx, ry) = (radius("rx"), radius("ry"));
            let (rx, ry) = (
                rx.or(ry).unwrap_or(0.).min(r.width() / 2.),
                ry.or(rx).unwrap_or(0.).min(r.height() / 2.),
            );
            if rx > 0. && ry > 0. {
                let scale = Affine::scale_non_uniform(rx, ry);
                let unit = scale.inverse().transform_rect_bbox(r);
                scale * RoundedRect::from_rect(unit, 1.).to_path(FLATNESS / rx.max(ry))
            } else {
                r.to_path(FLATNESS)
            }
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
        "circle" | "ellipse" => {
            let origin = |key| shape.attribute(key).unwrap_or("0").parse::<f64>().unwrap();
            let center = (origin("cx"), origin("cy"));
            if shape.has_tag_name("circle") {
                Circle::new(center, number(shape, "r")).to_path(FLATNESS)
            } else {
                let (rx, ry) = (number(shape, "rx"), number(shape, "ry"));
                Affine::translate(center)
                    * Affine::scale_non_uniform(rx, ry)
                    * Circle::new((0., 0.), 1.).to_path(FLATNESS / rx.max(ry))
            }
        }
        "path" => BezPath::from_svg(attr(shape, "d")).expect("node outline"),
        tag => panic!("unsupported gallery shape <{tag}>"),
    };
    let path = to_viewport(shape, frame) * path;
    let bounds = path.bounding_box();
    assert!(path.is_finite() && bounds.width() > 0. && bounds.height() > 0.);
    path
}

/// HTML labels expose their measured local box on foreignObject; native SVG labels expose
/// their painted pill box on rect with explicit dimensions. Unlabeled edges return None.
fn label_box(group: Node<'_, '_>, frame: Affine) -> Option<Rect> {
    let element = group
        .descendants()
        .find(|n| n.has_tag_name("foreignObject"))
        .or_else(|| {
            group.descendants().find(|n| {
                n.has_tag_name("rect") && n.has_attribute("width") && n.has_attribute("height")
            })
        })?;
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
    let base = ["flowchart-v2-", "agentflow-", "flowchart-"]
        .into_iter()
        .find_map(|family| id.rsplit_once(family))
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
    let bounds = view_box(marker);
    let size = Size::new(
        number(marker, "markerWidth") * scale,
        number(marker, "markerHeight") * scale,
    );
    #[cfg(feature = "diagram-agentflow")]
    {
        endpoint.marker_width = Some(
            bounds.height() * (size.width / bounds.width()).min(size.height / bounds.height()),
        );
    }
    if expected == FlowEdgeMarker::Point {
        endpoint.marker = Some(
            PointMarker::from_view_box(end, bounds, size, number(marker, "refX"))
                .expect("positive point-marker opening"),
        );
    }
}

fn check_unmarked_departure(
    endpoint: &mut Endpoint,
    shape: Option<&str>,
    path: Node<'_, '_>,
    marker: FlowEdgeMarker,
) {
    use crate::flowchart::FlowchartShape as Shape;
    if marker == FlowEdgeMarker::None
        && path.attribute("data-look") != Some("handDrawn")
        && matches!(
            shape.and_then(Shape::resolve),
            Some(
                Shape::Diamond
                    | Shape::Choice
                    | Shape::Ellipse
                    | Shape::Circle
                    | Shape::DoubleCircle
                    | Shape::CrossedCircle
                    | Shape::FilledCircle
                    | Shape::SmallCircle
                    | Shape::FramedCircle
                    | Shape::Anchor
            )
        )
    {
        endpoint.max_deviation_deg = 20.0;
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

        let bars: std::collections::HashSet<_> = self
            .model
            .nodes
            .iter()
            .filter(|node| matches!(node.layout_shape.as_deref(), Some("fork" | "join")))
            .map(|node| node.id.as_str())
            .collect();
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
            let fork_join = bars.contains(n.id.as_str());
            scene.bodies.push(Body {
                contour: Some(contour(owner, frame, fork_join)),
                ..Body::node(bounds)
            });
            if fork_join {
                continue;
            }
            let label = owner
                .descendants()
                .find(|n| n.has_tag_name("g") && has_class(*n, "label"))
                .expect("node label group");
            let label_bounds = label_box(label, frame).unwrap_or_else(|| {
                let w = n.label_width.unwrap_or(0.0);
                let h = n.label_height.unwrap_or(0.0);
                body_frame.transform_rect_bbox(Rect::from_center_size((0., 0.), (w, h)))
            });
            scene.labels.push(Label {
                owner: LabelOwner::Body(i),
                bounds: label_bounds,
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
            edge.start.outward_normal = Some(port_normal(
                &self.layout.nodes[from],
                first,
                &le.points[1],
                from_frame,
            ));
            edge.end.outward_normal = Some(port_normal(
                &self.layout.nodes[to],
                last,
                &le.points[le.points.len() - 2],
                to_frame,
            ));
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
            check_unmarked_departure(
                &mut edge.start,
                self.model.nodes[from].layout_shape.as_deref(),
                path,
                semantic.start_marker,
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

fn replay(name: &str, drawing: &Drawing) -> Result<Report, String> {
    let doc = Document::parse(&drawing.svg).expect("well-formed rendered SVG");
    let root = doc.root_element();
    let size = view_box(root).size();
    let scene = drawing.scene(&doc, size);
    let frame = viewport(root, size);
    let nonnegative = |value: f64| value.is_finite() && value >= -1e-6;
    for node in &drawing.layout.nodes {
        assert!(
            node.width > 0.
                && node.height > 0.
                && nonnegative(node.x - node.width / 2.)
                && nonnegative(node.y - node.height / 2.),
            "{name}: invalid measured box for {}",
            node.id
        );
    }
    for edge in &drawing.layout.edges {
        assert!(
            edge.points
                .iter()
                .all(|p| nonnegative(p.x) && nonnegative(p.y)),
            "{name}: invalid layout waypoint for {}",
            edge.id
        );
    }
    for label in &scene.labels {
        if let LabelOwner::Edge(i) = label.owner {
            let edge = &drawing.layout.edges[i];
            let anchor = edge.label.as_ref().unwrap();
            assert!(
                anchor.width > 0.
                    && anchor.height > 0.
                    && nonnegative(anchor.x - anchor.width / 2.)
                    && nonnegative(anchor.y - anchor.height / 2.),
                "{name}: invalid layout label for {}",
                edge.id
            );
            let painted = label.bounds.center();
            let expected = frame * Point::new(anchor.x, anchor.y);
            assert!(
                painted.distance(expected) < 1e-4,
                "{name}: {} painted label {painted:?} differs from layout anchor {expected:?}",
                edge.id
            );
        }
    }
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
    let mut failures = Vec::new();
    if name == "release" && !has_painted_bend(&scene) {
        failures.push("expected a painted cubic bend larger than the 0.15 px tolerance".into());
    }
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
        let metrics = report.edges[audit_ship].unwrap();
        let painted_return = metrics.sideways_return;
        let shaft = Shaft::read(&scene.edges[audit_ship]).unwrap();
        let across = |distance| {
            drawing
                .axis()
                .coordinate(shaft.sample(End::Start, distance))
        };
        let (start, end) = (across(0.), across(metrics.length));
        let mut previous = start;
        let mut variation = 0.;
        for index in 1..=32 {
            let position = across(metrics.length * (f64::from(index) / 32.));
            variation += (position - previous).abs();
            previous = position;
        }
        let sampled_return = (variation - (end - start).abs()).max(0.) * 0.5;
        eprintln!(
            "{name} fork_offset={fork} join_offset={join} audit_ship_return={painted_return} sampled_return={sampled_return}"
        );
        // Only floating-point roundoff is admissible at the zero-pixel target.
        if !(fork <= 1e-9 && join <= 1e-9 && painted_return <= 1e-9 && sampled_return <= 1e-9) {
            failures.push(format!("stage offset {fork}, ship offset {join}, painted audit → ship return {painted_return} (sampled {sampled_return})"));
        }
    }
    if !report.hard.is_empty() {
        for v in report.hard.iter().take(12) {
            let edge_id = |i: usize| &drawing.layout.edges[i].id;
            match *v {
                Violation::ForeignNodeHit { edge, node } => eprintln!(
                    "{v:?}: {} enters {}",
                    edge_id(edge),
                    drawing.layout.nodes[node].id
                ),
                Violation::ForeignLabelHit { edge, label } => {
                    let owner = match scene.labels[label].owner {
                        LabelOwner::Body(i) => &drawing.layout.nodes[i].id,
                        LabelOwner::Edge(i) => edge_id(i),
                    };
                    eprintln!("{v:?}: {} enters label of {owner}", edge_id(edge));
                }
                Violation::EndpointNormal { edge, .. }
                | Violation::EndpointContour { edge, .. }
                | Violation::MarkerAngle { edge, .. } => eprintln!("{v:?}: {}", edge_id(edge)),
                _ => eprintln!("{v:?}"),
            }
        }
        failures.push(format!("{} hard geometric violations", report.hard.len()));
    }
    if !report.label_overlaps.is_empty() || !report.label_node_overlaps.is_empty() {
        failures.push(format!(
            "{} overlapping label pairs, {} label/node pairs",
            report.label_overlaps.len(),
            report.label_node_overlaps.len()
        ));
    }
    // The foreign-obstacle scan exempts a loop's own rectangular body.
    for (i, edge) in scene
        .edges
        .iter()
        .enumerate()
        .filter(|(_, e)| e.start.body == e.end.body)
    {
        let id = &drawing.layout.nodes[edge.start.body].id;
        let node = drawing.model.nodes.iter().find(|n| &n.id == id).unwrap();
        if node
            .layout_shape
            .as_deref()
            .is_none_or(|shape| shape == "squareRect")
            && Shaft::read(edge)
                .unwrap()
                .hits(scene.bodies[edge.start.body].bounds)
        {
            failures.push(format!(
                "self-loop {} enters its body {id}",
                drawing.layout.edges[i].id
            ));
        }
    }
    if failures.is_empty() {
        Ok(report)
    } else {
        Err(format!("{name}: {}", failures.join(", ")))
    }
}

#[test]
fn labeled_cycle_and_self_loops_stay_clear() {
    let cycle = render(CYCLE_LOOPS, "cycle-loops").unwrap();
    assert_eq!((cycle.layout.nodes.len(), cycle.layout.edges.len()), (4, 6));
    assert_eq!(
        cycle.layout.edges.iter().filter(|e| e.from == e.to).count(),
        2
    );
    assert!(cycle.layout.edges.iter().all(|e| e.label.is_some()));
    replay("cycle-loops", &cycle).unwrap();
}

#[test]
fn fit_width_folds_fan_in_250_with_all_painted_contacts_and_markers() {
    let mut source = String::from("flowchart TD\n");
    for leaf in 1..250 {
        writeln!(&mut source, "N{leaf} --> N0").unwrap();
    }
    let options = LayoutOptions::headless_svg_defaults()
        .with_adaptation(LayoutAdaptation::FitWidth)
        .with_container_size(1200., 600.)
        .with_fold_wide_ranks(true);
    let (drawing, observation) = render_with_options(&source, "fan-in-250-fit-width", &options)
        .expect("fit-width render under the default budget");
    let observation = observation.expect("adapted layout observation");
    assert_eq!(observation.requested_width(), 1200.);
    assert!(
        !observation.overflowed(),
        "layout width {}",
        observation.actual_width()
    );
    assert_eq!(drawing.layout.nodes.len(), 250);
    assert_eq!(drawing.layout.edges.len(), 249);
    let report = replay("fan-in-250-fit-width", &drawing).unwrap();
    assert_eq!(report.markers_checked, 249);
    eprintln!(
        "fan-in-250 fit-width actual_width={:.6}px",
        observation.actual_width()
    );
}

#[test]
fn fit_width_turn_preserves_fork_and_join_contacts_on_the_painted_bars() {
    let source = "flowchart LR\nStart[Review queued change] --> F@{ shape: fork }\nF --> A[Verify upstream] & B[Check runtime]\nA --> J@{ shape: join }\nB --> J\nJ --> End[Publish verified change]";
    let options = LayoutOptions::default()
        .with_adaptation(LayoutAdaptation::FitWidth)
        .with_container_size(350.0, 600.0);
    let (drawing, observation) = render_with_options(source, "turned-bars", &options).unwrap();
    assert!(!observation.unwrap().overflowed());
    let document = Document::parse(&drawing.svg).unwrap();
    let scene = drawing.scene(&document, view_box(document.root_element()).size());
    for (id, end) in [("F", End::Start), ("J", End::End)] {
        let body = drawing.node(id);
        let bounds = scene.bodies[body].contour.as_ref().unwrap().bounding_box();
        assert!(bounds.width() > bounds.height(), "{id} {bounds:?}");
        let ports: Vec<_> = scene
            .edges
            .iter()
            .filter_map(|edge| {
                let endpoint = if end == End::Start {
                    edge.start
                } else {
                    edge.end
                };
                (endpoint.body == body).then(|| Shaft::read(edge).unwrap().sample(end, 0.0))
            })
            .collect();
        let [first, second] = ports[..] else {
            panic!("expected two contacts on {id}");
        };
        assert!((first.x - second.x).abs() >= 8.0, "{id} {ports:?}");
        assert!((first.y - second.y).abs() <= 1e-6, "{id} {ports:?}");
    }
    replay("turned-bars", &drawing).unwrap();
}

#[test]
fn duplicate_edge_ids_and_markers_at_both_ends() {
    // The authored id on X -> Y collides with the generated id on A -> B.
    let drawing = render("flowchart LR\nX((Circle)) L_A_B_0@-->|first| Y{Diamond}\nA -->|second| B\nA o--o X\nB <--> Y\nX x--x B", "marker-occurrences").unwrap();
    assert_eq!(drawing.layout.edges[0].id, drawing.layout.edges[1].id);
    let report = replay("marker-occurrences", &drawing).unwrap();
    assert_eq!(report.markers_checked, 4); // Three target points and one source point.
}

#[test]
fn basic_decision_branches_leave_separate_points_on_the_painted_diamond() {
    let source = include_str!("../../../../../fixtures/flowchart/basic.mmd");
    let pitch = merman_layout_shoal::Spacing::default().port_pitch;
    for direction in ["TD", "BT", "LR", "RL"] {
        let source = source.replacen("flowchart TD", &format!("flowchart {direction}"), 1);
        let drawing = render(&source, "basic-decision").unwrap();
        let doc = Document::parse(&drawing.svg).unwrap();
        let scene = drawing.scene(&doc, view_box(doc.root_element()).size());
        let decision = drawing.node("B");
        let axis = drawing.axis();
        let center = axis.coordinate(scene.bodies[decision].bounds.center());
        let mut exits: Vec<_> = scene
            .edges
            .iter()
            .filter(|edge| edge.start.body == decision)
            .map(|edge| {
                let shaft = Shaft::read(edge).unwrap();
                let tip = axis.coordinate(shaft.sample(End::Start, 0.));
                let target = axis.coordinate(scene.bodies[edge.end.body].bounds.center());
                (target, tip)
            })
            .collect();
        exits.sort_by(|left, right| left.0.total_cmp(&right.0));
        let [(_, left), (_, right)] = exits[..] else {
            panic!("{direction}: expected the Yes and No branches");
        };
        assert!(
            left < center && center < right && right - left >= pitch - 1e-6,
            "{direction}: branch contacts {left} and {right} around {center}"
        );
        replay("basic-decision", &drawing).unwrap();
    }
}

#[test]
fn a_clear_labelled_bypass_leaves_with_its_sibling_branch() {
    for direction in ["TD", "BT", "LR", "RL"] {
        for reversed in [false, true] {
            let branches = if reversed {
                "B -->|No| D[End]\nB -->|Yes| C[Execute]"
            } else {
                "B -->|Yes| C[Execute]\nB -->|No| D[End]"
            };
            let source =
                format!("flowchart {direction}\nA[Start] --> B{{Condition?}}\n{branches}\nC --> D");
            let drawing = render(&source, "decision-bypass").unwrap();
            let doc = Document::parse(&drawing.svg).unwrap();
            let scene = drawing.scene(&doc, view_box(doc.root_element()).size());
            let report = validate(&scene);
            assert!(report.hard.is_empty(), "{direction}: {:?}", report.hard);
            assert!(
                report.crossings.is_empty(),
                "{direction}: {:?}",
                report.crossings
            );

            let decision = drawing.node("B");
            let contour = scene.bodies[decision].contour.as_ref().unwrap();
            let axis = drawing.axis();
            let mut exits = Vec::new();
            for (index, edge) in scene
                .edges
                .iter()
                .enumerate()
                .filter(|(_, edge)| edge.start.body == decision)
            {
                let shaft = Shaft::read(edge).unwrap();
                exits.push(axis.coordinate(shaft.sample(End::Start, 0.)));
                assert!(
                    shaft
                        .lines
                        .iter()
                        .all(|line| !contour.contains(line.eval(0.5))),
                    "{direction}: branch reenters its source"
                );
                let label = scene
                    .labels
                    .iter()
                    .find(|label| label.owner == LabelOwner::Edge(index))
                    .unwrap();
                assert!(
                    shaft.hits(label.bounds),
                    "{direction}: detached branch label"
                );
                if matches!(direction, "TD" | "BT") {
                    let (first, _) = shaft.segment_from(End::Start, 0);
                    assert!(
                        matches!(first, PathSeg::Cubic(curve) if
                            (axis.coordinate(curve.p3) - axis.coordinate(curve.p0)).abs() > 1.0),
                        "{direction}, reversed declarations {reversed}: the branch should start turning at the decision"
                    );
                }
            }
            let [first_exit, second_exit] = exits[..] else {
                panic!("{direction}: expected two decision exits");
            };
            assert!((first_exit - second_exit).abs() >= 1.0, "{direction}");
        }
    }
}

#[test]
fn neo_rectangle_ports_respect_the_configured_painted_radius() {
    let source = "---\nconfig:\n  layout: shoal\n  look: neo\n  themeVariables:\n    radius: 45\n---\nflowchart TD\nA[Build] --> B & C & D & E";
    let drawing = render(source, "rounded-ports").unwrap();
    let doc = Document::parse(&drawing.svg).unwrap();
    let owner = doc
        .descendants()
        .find(|n| n.attribute("data-et") == Some("node") && n.attribute("data-id") == Some("A"))
        .unwrap();
    let shape = owner
        .descendants()
        .find(|n| n.has_tag_name("rect") && has_class(*n, "label-container"))
        .unwrap();
    let rx = number(shape, "rx");
    assert_eq!(rx, 45.);
    let node = &drawing.layout.nodes[drawing.node("A")];
    let half_flat = (node.width * 0.5 - rx).max(0.);
    assert_eq!(drawing.layout.edges.len(), 4);
    for edge in &drawing.layout.edges {
        assert_eq!(edge.from, "A");
        assert!(
            (edge.points[0].x - node.x).abs() <= half_flat + 1e-6,
            "{}: port reaches into the rounded corner",
            edge.id
        );
    }
    replay("rounded-ports", &drawing).unwrap();
}

fn has_painted_bend(scene: &Scene<'_>) -> bool {
    scene.edges.iter().any(|edge| {
        let path = edge.transform * BezPath::from_svg(edge.d).unwrap();
        path.segments().any(|segment| {
            if let PathSeg::Cubic(curve) = segment {
                let chord = Line::new(curve.start(), curve.end());
                return [0.25, 0.5, 0.75]
                    .into_iter()
                    .any(|t| chord.nearest(curve.eval(t), 1e-6).distance_sq > FLATNESS * FLATNESS);
            }
            false
        })
    })
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

fn evidence_dir() -> Option<PathBuf> {
    std::env::var_os("MERMAN_SHOAL_GALLERY").map(PathBuf::from)
}

fn read_json(path: &Path) -> serde_json::Value {
    let bytes = fs::read(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    serde_json::from_slice(&bytes).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn for_each_visual_fixture(mut check: impl FnMut(&str, &str, &str, &str, (usize, usize, usize))) {
    macro_rules! gallery_source {
        ($name:literal) => {
            include_str!(concat!(
                "../../../tests/fixtures/shoal-gallery/",
                $name,
                ".mmd"
            ))
        };
    }
    check("release", "gallery-1", RELEASE, "TB", (8, 8, 2));
    for (name, direction) in [
        ("release-lr", "LR"),
        ("release-bt", "BT"),
        ("release-rl", "RL"),
    ] {
        let source = RELEASE.replacen("flowchart TD", &format!("flowchart {direction}"), 1);
        check(
            name,
            &format!("route-study-{name}"),
            &source,
            direction,
            (8, 8, 2),
        );
    }
    for (name, source, direction, counts) in [
        ("balanced", gallery_source!("balanced"), "TB", (4, 4, 0)),
        ("uneven", gallery_source!("uneven"), "TB", (6, 7, 0)),
        ("parallel", gallery_source!("parallel"), "LR", (5, 7, 3)),
        (
            "disconnected",
            gallery_source!("disconnected"),
            "TB",
            (7, 4, 0),
        ),
        ("weave", gallery_source!("weave"), "TB", (5, 6, 0)),
        ("ports-heavy", PORTS_HEAVY, "TB", (18, 39, 7)),
        ("two-joins", gallery_source!("two-joins"), "TB", (10, 12, 0)),
    ] {
        check(
            name,
            &format!("route-study-{name}"),
            source,
            direction,
            counts,
        );
    }
    let diamonds = gallery_source!("diamonds").trim_end();
    let diamonds_lr = diamonds.replacen("flowchart TD", "flowchart LR", 1);
    for (name, source, direction, counts) in [
        ("diamonds-td", diamonds, "TB", (5, 6, 0)),
        ("diamonds-lr", diamonds_lr.as_str(), "LR", (5, 6, 0)),
        ("self-loop", gallery_source!("self-loop"), "TB", (4, 10, 6)),
        ("cycle", CYCLE, "TB", (8, 12, 6)),
        (
            "both-markers",
            include_str!(
                "../../../../../fixtures/flowchart/upstream_docs_flowchart_multi_directional_arrows_180.mmd"
            ),
            "LR",
            (4, 3, 0),
        ),
    ] {
        check(name, &format!("flat-{name}"), source, direction, counts);
    }
    for (shape, nodes) in [("fan-in", 50), ("chain", 100), ("fan-in", 250)] {
        let name = format!("{shape}-{nodes}");
        let mut source = String::from("flowchart TD\n");
        for node in 1..nodes {
            let (from, to) = if shape == "chain" {
                (node - 1, node)
            } else {
                (node, 0)
            };
            writeln!(source, "N{from} --> N{to}").unwrap();
        }
        check(
            &name,
            &format!("wb-{name}-1"),
            &source,
            "TB",
            (nodes, nodes - 1, 0),
        );
    }
}

#[cfg(feature = "diagram-agentflow")]
#[test]
fn replay_aesthetic_witnesses() {
    for_each_visual_fixture(|name, id, source, direction, _| {
        if matches!(
            name,
            "release" | "release-lr" | "release-bt" | "release-rl" | "ports-heavy" | "cycle"
        ) {
            let drawing = render(source, id).unwrap_or_else(|error| panic!("{name}: {error}"));
            let doc = Document::parse(&drawing.svg).expect("well-formed rendered SVG");
            let scene = drawing.scene(&doc, view_box(doc.root_element()).size());
            let body_names: Vec<_> = drawing.layout.nodes.iter().map(|n| n.id.as_str()).collect();
            let edge_names: Vec<_> = drawing.layout.edges.iter().map(|e| e.id.as_str()).collect();
            let metrics =
                super::aesthetic::emit(name, direction, &scene, &body_names, &edge_names, &[]);
            if name == "ports-heavy" {
                assert!(metrics.cubic_count <= 78, "{} cubics", metrics.cubic_count);
                assert!(
                    metrics.max_commands_per_edge <= 6,
                    "{} path commands",
                    metrics.max_commands_per_edge
                );
                let sink: Vec<_> = metrics
                    .marker_faces
                    .iter()
                    .filter(|face| face.body == "Sink")
                    .collect();
                assert_eq!(sink.len(), 3);
                assert_eq!(sink.iter().map(|face| face.marker_count).sum::<usize>(), 16);
                assert!(sink.iter().all(|face| face.pitch_over_width > 1.0));
                assert!(sink.iter().any(|face| face.face == "left"));
                assert!(sink.iter().any(|face| face.face == "right"));
            }
            if name == "cycle" {
                assert_eq!(metrics.micro_kink_count, 0, "{name}: micro kinks");
            }
            if name.starts_with("release") {
                aesthetic_witnesses::assert_balanced_branches(&metrics, "checks", name, direction);
                assert!(
                    metrics
                        .marker_faces
                        .iter()
                        .all(|face| face.pitch_over_width >= 1.0),
                    "{name}: incoming arrows overlap on one face"
                );
                let across = |point| drawing.axis().coordinate(point);
                for (edge, layout) in scene.edges.iter().zip(&drawing.layout.edges) {
                    if !matches!(layout.from.as_str(), "checks" | "stage") {
                        continue;
                    }
                    let shaft = Shaft::read(edge).unwrap();
                    let PathSeg::Cubic(curve) = shaft.segment_from(End::Start, 0).0 else {
                        panic!("{name}: {} must curve from its unmarked contact", layout.id);
                    };
                    if layout.from == "checks" {
                        let departure = across(curve.p1) - across(curve.p0);
                        let sweep = across(curve.p3) - across(curve.p0);
                        assert!(
                            departure * sweep > 0.,
                            "{name}: {} diamond tangent",
                            layout.id
                        );
                    }
                }
            }
        }
    });
    compound::aesthetic_witnesses();
}

#[test]
fn cycle_keeps_its_backbone_and_paints_straight_exterior_lanes_in_four_directions() {
    let ids = ["n0", "n1", "n2", "n3", "n4", "n5", "n6", "n7"];
    for direction in ["TD", "BT", "LR", "RL"] {
        let source = CYCLE.replacen("flowchart TD", &format!("flowchart {direction}"), 1);
        let drawing = render(&source, &format!("cycle-{direction}")).unwrap();
        let doc = Document::parse(&drawing.svg).unwrap();
        let viewport = view_box(doc.root_element());
        let scene = drawing.scene(&doc, viewport.size());
        let report = validate(&scene);
        assert!(report.hard.is_empty(), "{direction}: {:?}", report.hard);
        assert!(
            report.crossings.is_empty(),
            "{direction}: {:?}",
            report.crossings
        );

        let centers = ids.map(|id| scene.bodies[drawing.node(id)].bounds.center());
        let across = |p: Point| drawing.axis().coordinate(p);
        let along = |p: Point| match direction {
            "TD" => p.y,
            "BT" => -p.y,
            "LR" => p.x,
            _ => -p.x,
        };
        let edge_index = |source: usize, target: usize| {
            drawing
                .layout
                .edges
                .iter()
                .position(|e| e.from == ids[source] && e.to == ids[target])
                .unwrap()
        };
        let edge = |source, target| &scene.edges[edge_index(source, target)];
        for center in centers {
            assert!(
                (across(center) - across(centers[0])).abs() < FLATNESS,
                "{direction}: node centers must share the backbone"
            );
        }
        for i in 0..ids.len() - 1 {
            assert!(
                along(centers[i]) < along(centers[i + 1]),
                "{direction}: {} out of order",
                ids[i + 1]
            );
            let shaft = Shaft::read(edge(i, i + 1)).unwrap();
            let drift = across(Point::new(shaft.bounds.x1, shaft.bounds.y1))
                - across(Point::new(shaft.bounds.x0, shaft.bounds.y0));
            assert!(
                drift < FLATNESS,
                "{direction}: {} main shaft drifts {drift}px",
                ids[i]
            );
            assert!(
                (across(shaft.lines[0].p0) - across(centers[0])).abs() < FLATNESS,
                "{direction}: {} must leave at the center of its face",
                ids[i]
            );
            let ends = [ids[i], ids[i + 1]].map(|id| {
                let bounds = scene.bodies[drawing.node(id)].bounds;
                let a = along(Point::new(bounds.x0, bounds.y0));
                let b = along(Point::new(bounds.x1, bounds.y1));
                (a.min(b), a.max(b))
            });
            let gap = ends[1].0 - ends[0].1;
            if matches!(i, 1 | 3 | 5) {
                assert!(gap < 110.0, "{direction}: labeled gap takes {gap:.2}px");
            } else {
                assert!(
                    (gap - merman_layout_shoal::Spacing::default().rank).abs() < FLATNESS,
                    "{direction}: unlabeled gap takes {gap:.2}px"
                );
            }
        }
        for (source, target, side) in [(7usize, 0usize, 1.0), (5, 2, 1.0), (1, 7, -1.0)] {
            let edge = edge(source, target);
            for port in [edge.start, edge.end] {
                let normal = port.outward_normal.unwrap().to_point();
                assert_eq!(
                    across(normal),
                    side,
                    "{direction}: {} → {} side",
                    ids[source],
                    ids[target]
                );
            }
            let shaft = Shaft::read(edge).unwrap();
            let corners: Vec<_> = shaft
                .segments
                .iter()
                .filter_map(|(segment, _)| {
                    if let PathSeg::Cubic(curve) = segment {
                        Some(curve)
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(
                corners.len(),
                2,
                "{direction}: each side contact needs one turn"
            );
            for corner in corners {
                let leg_along = (along(corner.p3) - along(corner.p0)).abs();
                let leg_across = (across(corner.p3) - across(corner.p0)).abs();
                assert!(
                    (leg_along - leg_across).abs() < FLATNESS && leg_along <= 24.0 + FLATNESS,
                    "{direction}: unequal exterior corner arms ({leg_along:.2}, {leg_across:.2})"
                );
            }
            let mut rail = None::<f64>;
            for &id in &ids[source.min(target) + 1..source.max(target)] {
                let bounds = scene.bodies[drawing.node(id)].bounds;
                let plane = along(bounds.center());
                let rim = if side > 0.0 {
                    Point::new(bounds.x1, bounds.y1)
                } else {
                    Point::new(bounds.x0, bounds.y0)
                };
                let mut covered = false;
                for line in &shaft.lines {
                    let (a, b) = (along(line.p0), along(line.p1));
                    if a.min(b) <= plane && plane < a.max(b) {
                        let x = across(line.eval((plane - a) / (b - a)));
                        assert!(
                            side * (x - across(rim)) > FLATNESS,
                            "{direction}: {id} corridor clearance"
                        );
                        assert!(
                            (x - *rail.get_or_insert(x)).abs() < FLATNESS,
                            "{direction}: {id} corridor drift"
                        );
                        covered = true;
                    }
                }
                assert!(covered, "{direction}: {id} corridor missing");
            }
        }
        let feedback = Shaft::read(edge(5, 2)).unwrap();
        for (source, target, side) in [(3, 5, 1.0), (7, 0, -1.0)] {
            let owner = LabelOwner::Edge(edge_index(source, target));
            let label = scene
                .labels
                .iter()
                .find(|label| label.owner == owner)
                .unwrap();
            let a = Point::new(label.bounds.x0, label.bounds.y0);
            let b = Point::new(label.bounds.x1, label.bounds.y1);
            let lo = along(a).min(along(b));
            let hi = along(a).max(along(b));
            let rim = across(if side > 0.0 { b } else { a });
            let rail = feedback
                .lines
                .iter()
                .find(|line| {
                    let (start, end) = (along(line.p0), along(line.p1));
                    start.min(end) <= lo && hi <= start.max(end)
                })
                .expect("the feedback rail spans the entire label");
            let clearance = (side * (across(rail.p0) - rim)).min(side * (across(rail.p1) - rim));
            assert!(
                clearance >= 12.0 - FLATNESS,
                "{direction}: {} → {} label clearance is {clearance:.3}px",
                ids[source],
                ids[target]
            );
        }
        eprintln!(
            "cycle {direction}: {:.2} × {:.2} px, zero painted crossings and three straight exterior lanes",
            viewport.width(),
            viewport.height()
        );
    }
}

#[test]
fn a_dag_can_route_a_bypass_outside_while_keeping_an_independent_long_edge() {
    let drawing = render(
        "flowchart TD\nA --> B --> C --> D --> E\nB --> E\nA -----> F\n",
        "dag-bypass",
    )
    .unwrap();
    let doc = Document::parse(&drawing.svg).unwrap();
    let scene = drawing.scene(&doc, view_box(doc.root_element()).size());
    let report = validate(&scene);
    assert!(report.hard.is_empty(), "{:?}", report.hard);
    assert!(report.crossings.is_empty(), "{:?}", report.crossings);
    let edge = |from: &str, to: &str| {
        let index = drawing
            .layout
            .edges
            .iter()
            .position(|e| e.from == from && e.to == to)
            .unwrap();
        &scene.edges[index]
    };
    let bypass = edge("B", "E");
    assert_eq!(bypass.start.outward_normal, Some(Vec2::new(-1.0, 0.0)));
    let independent = edge("A", "F");
    assert_eq!(independent.start.outward_normal, Some(Vec2::new(0.0, 1.0)));
    assert_eq!(independent.end.outward_normal, Some(Vec2::new(0.0, -1.0)));
}

#[test]
fn replay_visual_corpus() {
    let archive = evidence_dir().map(|dir| {
        let facts = read_json(&dir.join("gallery-facts.json"));
        assert_eq!(facts["cases"].as_object().expect("gallery cases").len(), 19);
        (dir, facts)
    });
    let (mut drawn, mut nodes, mut shafts, mut edge_labels) = (0, 0, 0, 0);
    let mut failures = Vec::new();
    for_each_visual_fixture(|name, id, source, direction, expected| {
        let archived = archive.as_ref().map(|(dir, facts)| {
            let case = &facts["cases"][name];
            assert_eq!(case["diagram_id"], id, "{name}");
            assert_eq!(case["direction"], direction, "{name}");
            assert_eq!(
                case["nodes"].as_array().unwrap().len(),
                expected.0,
                "{name}"
            );
            assert_eq!(
                case["edges"].as_array().unwrap().len(),
                expected.1,
                "{name}"
            );
            let path = dir.join("sources/flat").join(format!("{name}.mmd"));
            fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
        });
        let drawing = match render(archived.as_deref().unwrap_or(source), id) {
            Ok(drawing) => drawing,
            Err(error) => {
                failures.push(format!("{name}: {error}"));
                return;
            }
        };
        assert_eq!(
            drawing.model.direction.as_deref(),
            Some(direction),
            "{name}"
        );
        let edges = &drawing.layout.edges;
        let counts = (
            drawing.layout.nodes.len(),
            edges.len(),
            edges.iter().filter(|edge| edge.label.is_some()).count(),
        );
        assert_eq!(counts, expected, "{name}: nodes, shafts, edge labels");
        drawn += 1;
        nodes += counts.0;
        shafts += counts.1;
        edge_labels += counts.2;
        failures.extend(replay(name, &drawing).err());
    });
    eprintln!(
        "flat gallery: {drawn}/19 drawn, {nodes} nodes, {shafts} shafts, {edge_labels} edge labels"
    );
    assert!(
        failures.is_empty(),
        "{drawn}/19 drawn\n{}",
        failures.join("\n")
    );
    assert_eq!((drawn, nodes, shafts, edge_labels), (19, 513, 545, 30));
}

#[test]
#[ignore = "set MERMAN_SHOAL_STRESS_PLAN to a full workload plan or MERMAN_SHOAL_GALLERY to frozen evidence"]
fn replay_stress_corpus() {
    let path = std::env::var_os("MERMAN_SHOAL_STRESS_PLAN")
        .map(PathBuf::from)
        .or_else(|| evidence_dir().map(|dir| dir.join("stress.json")))
        .expect("set MERMAN_SHOAL_STRESS_PLAN to a bench.py plan --suite full output or MERMAN_SHOAL_GALLERY to the frozen evidence directory");
    let plan = read_json(&path);
    let topologies = plan["topologies"].as_array().expect("workload topologies");
    assert_eq!(topologies.len(), 32, "expected the full topology suite");
    let mut seen = std::collections::HashSet::new();
    let mut drawn = 0;
    let mut failures = Vec::new();
    for topology in topologies {
        let name = topology["id"].as_str().unwrap();
        assert!(seen.insert(name), "{name}: duplicate topology");
        let source = topology["source"].as_str().unwrap();
        if name == "fork-join-500" {
            assert_eq!(topology["edges"], 996, "{name}");
            let result =
                engine().parse_diagram_for_render_model_sync(source, ParseOptions::strict());
            let Err(merman_core::Error::DiagramParse { diagnostic, .. }) = result else {
                panic!("{name}: expected source maxEdges rejection");
            };
            assert!(
                diagnostic.message().contains("maxEdges")
                    && diagnostic.message().contains("edge limit exceeded"),
                "{name}: {diagnostic:?}"
            );
            eprintln!("{name}: expected maxEdges rejection (996 edges)");
            continue;
        }
        let drawing = match render(source, &format!("wb-{name}-1")) {
            Ok(drawing) => drawing,
            Err(error) => {
                failures.push(format!("{name}: {error}"));
                continue;
            }
        };
        assert_eq!(
            Some(drawing.layout.nodes.len() as u64),
            topology["nodes"].as_u64(),
            "{name}"
        );
        assert_eq!(
            Some(drawing.layout.edges.len() as u64),
            topology["edges"].as_u64(),
            "{name}"
        );
        failures.extend(replay(name, &drawing).err());
        drawn += 1;
    }
    assert!(
        failures.is_empty(),
        "{drawn}/31 drawn\n{}",
        failures.join("\n")
    );
    assert_eq!(drawn, 31);
    eprintln!("stress: {drawn}/32 drawn, one expected maxEdges rejection");
}
