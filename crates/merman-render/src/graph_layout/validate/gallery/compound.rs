use super::{
    Body, CrossAxis, Edge, End, FLATNESS, Label, LabelOwner, Scene, Shaft, Violation, attr,
    has_class, painted_outline, read_marker, rect, to_viewport, view_box, viewport,
};
use crate::{
    LayoutOptions, RenderCapability, environment::RenderEnvironment, family,
    flowchart::FlowchartRenderModelRef, model::FlowchartLayout, svg::SvgRenderOptions,
};
use kurbo::{Affine, BezPath, ParamCurve, Point, Rect, Shape, Vec2};
use merman_core::{MermaidEngine, ParseOptions};
use merman_model::{
    families::flowchart::{FlowEdgeVisibility, FlowchartModel, FlowchartRenderContext},
    presentation::{PresentationConfig, PresentationDiagram, RenderSemanticModel},
};
use roxmltree::{Document, Node};
use std::collections::{BTreeMap, BTreeSet, HashMap};

mod corpus;
mod ink;
mod stitches;

const EMPTY_OWN: &str = include_str!("../../../../tests/fixtures/shoal-compound/empty-own.mmd");
const AGENT_REFERENCE: &str =
    include_str!("../../../../tests/fixtures/shoal-compound/collapsed-reference.mmd");
const SIBLING_CROSS_EDGES: &str = include_str!(
    "../../../../../../fixtures/flowchart/stress_flowchart_subgraph_cross_edges_between_siblings_048.mmd"
);

struct Drawing {
    model: FlowchartModel,
    context: FlowchartRenderContext,
    config: PresentationConfig,
    layout: FlowchartLayout,
    svg: String,
}

fn shoal_source(source: &str) -> String {
    format!("{source}\n%%{{init: {{\"layout\":\"shoal\"}}}}%%\n")
}

fn prepare(diagram: &PresentationDiagram) -> Result<Drawing, String> {
    let error = |e: crate::Error| e.to_string();
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .map_err(|e| e.to_string())?;
    let plan = family::plan_render(diagram, &session).map_err(error)?;
    if !plan
        .required_capabilities()
        .contains(&RenderCapability::LayoutShoal)
        || !plan.is_ready()
    {
        return Err(format!(
            "{} capability plan {plan:?}",
            diagram.metadata().diagram_type
        ));
    }
    let (model, context, tag) = match diagram.model() {
        RenderSemanticModel::Flowchart(model) => (
            model.clone(),
            diagram
                .render_context()
                .flowchart_render_context()
                .cloned()
                .unwrap_or_default(),
            "FlowchartV2",
        ),
        RenderSemanticModel::Agentflow(model) => {
            let (flow, context) = model.to_flowchart_model();
            (flow, context, "AgentflowDiagram")
        }
        other => return Err(format!("unexpected compound model {}", other.kind())),
    };
    let artifact = family::prepare(diagram, &LayoutOptions::headless_svg_defaults(), session)
        .map_err(error)?;
    let snapshot = artifact.layout_geometry_json().map_err(error)?;
    let layout = serde_json::from_value(snapshot["layout"][tag].clone())
        .map_err(|e| format!("{tag} layout: {e}"))?;
    let painted = artifact
        .render_svg(
            &SvgRenderOptions {
                diagram_id: Some("compound-replay".into()),
                ..Default::default()
            },
            &Default::default(),
        )
        .map_err(error)?;
    assert!(
        painted
            .required_capabilities()
            .contains(&RenderCapability::LayoutShoal)
    );
    Ok(Drawing {
        model,
        context,
        layout,
        svg: painted.svg().to_owned(),
        config: diagram.metadata().effective_config.clone(),
    })
}

fn draw(source: &str) -> Drawing {
    draw_with(&MermaidEngine::new(), source)
}

fn draw_with(engine: &MermaidEngine, source: &str) -> Drawing {
    let parsed = engine
        .parse_diagram_for_render_model_sync(&shoal_source(source), ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation();
    prepare(&parsed).unwrap()
}

#[derive(Default, Debug)]
struct Counts {
    nodes: usize,
    groups: usize,
    titles: usize,
    node_labels: usize,
    edge_labels: usize,
    shafts: usize,
    stroke_passes: usize,
    rough_subpaths: usize,
    contacts: usize,
    normals: usize,
    markers: usize,
    stitches: usize,
}

struct Capture<'a> {
    scene: Scene<'a>,
    body_names: Vec<String>,
    edge_names: Vec<String>,
    model_boxes: Vec<Rect>,
    edge_knots: Vec<Vec<Point>>,
    painted_knots: Vec<Vec<Point>>,
    to_layout: Affine,
    rough_contacts: BTreeMap<usize, [Point; 2]>,
    rough_group_frames: BTreeMap<usize, BezPath>,
    ink: ink::Deviation,
    counts: Counts,
    failures: Vec<String>,
}

impl Capture<'_> {
    fn label_name(&self, index: usize) -> String {
        match self.scene.labels[index].owner {
            LabelOwner::Body(id) => format!("{} label", self.body_names[id]),
            LabelOwner::Edge(id) => format!("{} label", self.edge_names[id]),
        }
    }

    fn describe(&self, violation: &Violation) -> String {
        use super::super::Item;
        let body = |i: usize| &self.body_names[i];
        let edge = |i: usize| &self.edge_names[i];
        let item_name = |item| match item {
            Item::Body(i) => body(i).clone(),
            Item::Edge(i) => edge(i).clone(),
            Item::Label(i) => self.label_name(i),
        };
        let owners = match *violation {
            Violation::SiblingOverlap { first, second } => {
                format!("{} / {}", body(first), body(second))
            }
            Violation::LabelOverlap { first, second } => {
                format!("{} / {}", self.label_name(first), self.label_name(second))
            }
            Violation::LabelNodeOverlap { label, node }
            | Violation::LabelGroupOverlap { label, group: node } => {
                format!("{} overlaps {}", self.label_name(label), body(node))
            }
            Violation::ForeignNodeHit { edge: i, node } => {
                format!("{} enters {}", edge(i), body(node))
            }
            Violation::ForeignGroupHit { edge: i, group } => {
                format!("{} enters {}", edge(i), body(group))
            }
            Violation::ForeignLabelHit { edge: i, label } => {
                format!("{} enters {}", edge(i), self.label_name(label))
            }
            Violation::OutsideGroup { item, group } => {
                format!("{} outside {}", item_name(item), body(group))
            }
            Violation::InvalidOwner { item } | Violation::InvalidGeometry { item } => {
                item_name(item)
            }
            Violation::InvalidPath { edge: i, .. }
            | Violation::InvalidNormal { edge: i, .. }
            | Violation::EndpointContour { edge: i, .. }
            | Violation::EndpointNormal { edge: i, .. }
            | Violation::MarkerAngle { edge: i, .. } => edge(i).clone(),
        };
        format!("{owners}: {violation:?}")
    }

    fn audit(&mut self) {
        use super::super::{Report, check_bodies, check_labels, check_shaft};
        let mut report = Report::default();
        let ancestry = check_bodies(&self.scene, &mut report);
        check_labels(&self.scene, &ancestry, &mut report);
        for violation in &report.hard {
            self.failures.push(self.describe(violation));
        }
        for (a, b) in report.label_overlaps {
            if matches!(
                (self.scene.labels[a].owner, self.scene.labels[b].owner),
                (LabelOwner::Body(_), LabelOwner::Body(_))
            ) && [a, b].into_iter().all(|label| {
                self.scene.labels[label]
                    .group_owner(&self.scene.bodies)
                    .is_none()
            }) {
                self.failures.push(format!(
                    "label overlap: {} / {}",
                    self.label_name(a),
                    self.label_name(b)
                ));
            }
        }
        for (label, node) in report.label_node_overlaps {
            let node_label = self.scene.labels[label];
            if matches!(node_label.owner, LabelOwner::Body(_))
                && node_label.group_owner(&self.scene.bodies).is_none()
            {
                self.failures.push(format!(
                    "{} overlaps {}",
                    self.label_name(label),
                    self.body_names[node]
                ));
            }
        }
        for index in 0..self.scene.edges.len() {
            let contacts = self.rough_contacts.get(&index).copied();
            let edge = &self.scene.edges[index];
            let terminals = [(End::Start, edge.start), (End::End, edge.end)];
            let passes = if contacts.is_some() {
                ink::stroke_passes(edge).map(|(subpaths, passes)| {
                    self.counts.rough_subpaths += subpaths;
                    passes
                })
            } else {
                Shaft::read(edge).map(|shaft| vec![shaft])
            };
            let passes = match passes {
                Ok(passes) => passes,
                Err(error) => {
                    self.failures
                        .push(format!("{}: {error}", self.edge_names[index]));
                    continue;
                }
            };
            for (pass, shaft) in passes.iter().enumerate() {
                let name = if contacts.is_some() {
                    format!("{} stroke {}", self.edge_names[index], pass + 1)
                } else {
                    self.edge_names[index].clone()
                };
                let mut report = Report::default();
                check_shaft(&self.scene, &ancestry, index, shaft, contacts, &mut report);
                self.counts.contacts += report.contacts_checked;
                self.counts.normals += report.normals_checked;
                self.counts.markers += report.markers_checked;
                self.counts.stroke_passes += 1;
                for violation in report.hard {
                    let mut description = self.describe(&violation);
                    if let Violation::ForeignLabelHit { label, .. } = violation {
                        let bounds = self.scene.labels[label].bounds;
                        if let Some(line) = shaft
                            .lines
                            .iter()
                            .find(|line| super::super::enters_rect(**line, bounds))
                        {
                            use std::fmt::Write as _;
                            write!(
                                description,
                                " at {:?} → {:?} in {bounds:?}",
                                line.p0, line.p1
                            )
                            .unwrap();
                        }
                    }
                    self.failures.push(if contacts.is_some() {
                        format!("{description} (stroke {})", pass + 1)
                    } else {
                        description
                    });
                }
                if contacts.is_some() {
                    for (end, terminal) in terminals {
                        let (segment, _) = shaft.segment_from(end, 0);
                        let restored = segment.start()
                            - super::super::start_tangent(segment) * terminal.setback;
                        let distance =
                            super::super::contour_gap(&self.scene.bodies[terminal.body], restored);
                        self.ink.endpoint_to_outline = self.ink.endpoint_to_outline.max(distance);
                    }
                }
                stitches::check(self, index, shaft, &name);
            }
        }
    }
}

fn shape_outline(owner: Node<'_, '_>, frame: Affine, group: bool) -> BezPath {
    let container = if group {
        owner
    } else {
        owner
            .descendants()
            .find(|n| has_class(*n, "label-container"))
            .unwrap_or(owner)
    };
    let mut candidates = container.descendants().filter(|n| {
        matches!(
            n.tag_name().name(),
            "rect" | "path" | "polygon" | "circle" | "ellipse"
        ) && !n
            .ancestors()
            .take_while(|a| *a != container)
            .any(|a| has_class(a, "label") || has_class(a, "cluster-label"))
            && (!n.has_tag_name("rect") || n.has_attribute("width") && n.has_attribute("height"))
    });
    if !container.has_tag_name("g") {
        return painted_outline(container, frame);
    }
    if owner.attribute("data-look") == Some("handDrawn") {
        let stroke = candidates
            .rfind(|shape| shape.has_tag_name("path") && shape.has_attribute("stroke"))
            .unwrap_or_else(|| {
                panic!("{}: missing RoughJS outline stroke", attr(owner, "data-id"))
            });
        return painted_outline(stroke, frame);
    }
    let first = candidates
        .next()
        .unwrap_or_else(|| panic!("{}: missing painted outline", attr(owner, "data-id")));
    let mut outline = painted_outline(first, frame);
    for candidate in candidates {
        let path = painted_outline(candidate, frame);
        let area = |p: &BezPath| p.bounding_box().area();
        if area(&path) > area(&outline) {
            outline = path;
        }
    }
    outline
}

fn explicit_label_bounds(group: Node<'_, '_>, frame: Affine) -> Option<Rect> {
    let mut boxes = group
        .descendants()
        .filter(|n| {
            n.has_tag_name("foreignObject")
                || n.has_tag_name("rect")
                    && has_class(*n, "background")
                    && n.has_attribute("width")
                    && n.has_attribute("height")
        })
        .map(|n| to_viewport(n, frame).transform_rect_bbox(rect(n)))
        .filter(|r| r.width() > 0. && r.height() > 0.);
    let first = boxes.next()?;
    Some(boxes.fold(first, |bounds, other| bounds.union(other)))
}

fn text_element<'a, 'input>(group: Node<'a, 'input>) -> Option<Node<'a, 'input>> {
    group.descendants().find(|n| {
        n.has_tag_name("text")
            && n.descendants()
                .any(|child| child.is_text() && child.text().is_some_and(|s| !s.is_empty()))
    })
}

#[test]
fn collapsed_agentflow_preserves_semantic_endpoints_and_reference_marker() {
    let drawing = draw(AGENT_REFERENCE);
    let doc = Document::parse(&drawing.svg).unwrap();
    let capture = drawing.capture(&doc);
    let actual: Vec<_> = drawing
        .layout
        .edges
        .iter()
        .map(|e| (e.id.as_str(), e.from.as_str(), e.to.as_str()))
        .collect();
    assert_eq!(
        actual,
        [
            ("loop", "inner", "inner"),
            ("enter", "start", "inner"),
            ("exit", "inner", "api"),
            ("reference", "inner", "api")
        ]
    );
    assert_eq!(capture.body_names, ["api", "inner", "outer", "start"]);
    assert_eq!(
        (
            capture.counts.groups,
            capture.counts.nodes,
            capture.counts.titles
        ),
        (1, 3, 1)
    );
    let inner = capture
        .body_names
        .iter()
        .position(|id| id == "inner")
        .unwrap();
    let outer = capture
        .body_names
        .iter()
        .position(|id| id == "outer")
        .unwrap();
    assert!(!capture.scene.bodies[inner].is_group);
    assert_eq!(capture.scene.bodies[inner].parent, Some(outer));
    let reference = doc
        .descendants()
        .find(|n| {
            n.attribute("data-et") == Some("edge") && n.attribute("data-id") == Some("reference")
        })
        .unwrap();
    assert!(has_class(reference, "edge-pattern-dotted"));
    assert!(
        reference.attribute("marker-start").is_none()
            && reference.attribute("marker-end").is_none()
    );
    assert!(
        capture.failures.is_empty(),
        "{}",
        capture.failures.join("\n")
    );
}

#[test]
fn own_member_empty_group_and_duplicate_public_edge_ids_keep_their_occurrences() {
    let drawing = draw(EMPTY_OWN);
    let doc = Document::parse(&drawing.svg).unwrap();
    let capture = drawing.capture(&doc);
    assert_eq!(
        (
            capture.counts.nodes,
            capture.counts.groups,
            capture.counts.titles,
            capture.counts.shafts,
            capture.counts.edge_labels
        ),
        (2, 2, 2, 3, 1)
    );
    assert_eq!(drawing.layout.edges[1].id, drawing.layout.edges[2].id);
    assert_ne!(
        capture.scene.edges[1].start.body,
        capture.scene.edges[2].start.body
    );
    assert!(
        capture.failures.is_empty(),
        "{}",
        capture.failures.join("\n")
    );
}

#[test]
fn sibling_crossings_stay_in_their_separate_aligned_lanes_between_faces() {
    use kurbo::{Line, ParamCurveExtrema};
    let drawing = draw(SIBLING_CROSS_EDGES);
    let doc = Document::parse(&drawing.svg).unwrap();
    let capture = drawing.capture(&doc);
    let body = |name| {
        capture.scene.bodies[capture.body_names.iter().position(|id| id == name).unwrap()].bounds
    };
    let (left, right) = (body("left"), body("right"));
    assert!(left.x1 < right.x0);
    let mut lanes = Vec::new();
    for (from, to, faces) in [
        ("l2", "r1", [(left.x1, left), (right.x0, right)]),
        ("r2", "l1", [(right.x0, right), (left.x1, left)]),
    ] {
        let index = drawing
            .layout
            .edges
            .iter()
            .position(|e| e.from == from && e.to == to)
            .unwrap();
        let shaft = Shaft::read(&capture.scene.edges[index]).unwrap();
        let mut cuts = Vec::new();
        let mut ports = Vec::new();
        for (x, rect) in faces {
            let face = Line::new((x, rect.y0), (x, rect.y1));
            let port = capture.edge_knots[index]
                .iter()
                .find(|p| (p.x - x).abs() < 1e-6 && (rect.y0..=rect.y1).contains(&p.y))
                .unwrap();
            ports.push(port.y);
            let mut crossings: Vec<_> = shaft
                .segments
                .iter()
                .enumerate()
                .flat_map(|(i, (segment, _))| {
                    segment
                        .intersect_line(face)
                        .into_iter()
                        .map(move |h| i as f64 + h.segment_t.clamp(0., 1.))
                })
                .collect();
            crossings.sort_unstable_by(f64::total_cmp);
            crossings.dedup_by(|a, b| (*a - *b).abs() < 1e-7);
            assert_eq!(
                crossings.len(),
                1,
                "{from} → {to} at face {x}: {crossings:?}"
            );
            cuts.push(crossings[0]);
        }
        assert!(
            (ports[0] - ports[1]).abs() < 1e-6 && cuts[0] < cuts[1],
            "{from} → {to}: {ports:?} {cuts:?}"
        );
        lanes.push(ports[0]);
        let sign = (faces[1].0 - faces[0].0).signum();
        let mut backtrack = 0.;
        let mut excursion = 0_f64;
        for (i, (segment, _)) in shaft.segments.iter().enumerate() {
            let lo = (cuts[0] - i as f64).max(0.);
            let hi = (cuts[1] - i as f64).min(1.);
            if lo >= hi {
                continue;
            }
            let segment = segment.subsegment(lo..hi);
            let bounds = ParamCurveExtrema::bounding_box(&segment);
            excursion = excursion
                .max((bounds.y0 - ports[0]).abs())
                .max((bounds.y1 - ports[0]).abs());
            let mut previous = segment.start().x;
            for t in segment.extrema().into_iter().chain([1.]) {
                let next = segment.eval(t).x;
                backtrack += (sign * (previous - next)).max(0.);
                previous = next;
            }
        }
        assert!(
            excursion <= FLATNESS && backtrack <= 1e-7,
            "{from} → {to}: aligned lane y={}, excursion {excursion}px, backtrack {backtrack}px",
            ports[0]
        );
    }
    assert!(
        (lanes[0] - lanes[1]).abs() > FLATNESS,
        "shared lane: {lanes:?}"
    );
}

pub(super) fn aesthetic_witnesses() {
    use super::super::aesthetic::{Landmark, Seam, emit};

    let engine = super::engine();
    for (name, source) in [
        (
            "flowchart-stress_flowchart_subgraph_cross_edges_between_siblings_048",
            SIBLING_CROSS_EDGES,
        ),
        (
            "flowchart-stress_flowchart_nested_subgraphs_titles_001",
            include_str!(
                "../../../../../../fixtures/flowchart/stress_flowchart_nested_subgraphs_titles_001.mmd"
            ),
        ),
        (
            "agentflow-local_nested_collapse_routes",
            include_str!("../../../../../../fixtures/agentflow/local_nested_collapse_routes.mmd"),
        ),
    ] {
        let drawing = draw_with(&engine, source);
        let doc = Document::parse(&drawing.svg).expect("well-formed rendered SVG");
        let capture = drawing.capture(&doc);
        let body = |name: &str| {
            capture
                .body_names
                .iter()
                .position(|id| id == name)
                .unwrap_or_else(|| panic!("missing aesthetic witness body {name}"))
        };
        let bounds = |name| capture.scene.bodies[body(name)].bounds;
        let edge = |from, to| {
            let (start, end) = (body(from), body(to));
            capture
                .scene
                .edges
                .iter()
                .position(|edge| edge.start.body == start && edge.end.body == end)
                .unwrap_or_else(|| panic!("missing aesthetic witness edge {from} → {to}"))
        };
        let seams = if name.ends_with("_048") {
            vec![Seam {
                edge: edge("r2", "l1"),
                landmarks: vec![
                    Landmark::Plane("r1.right", bounds("r1").x1),
                    Landmark::Plane("right.left", bounds("right").x0),
                    Landmark::Plane("left.right", bounds("left").x1),
                    Landmark::Plane("l2.left", bounds("l2").x0),
                ],
            }]
        } else if name.ends_with("_001") {
            ["b1", "c1"]
                .into_iter()
                .map(|from| Seam {
                    edge: edge(from, "d1"),
                    landmarks: vec![
                        Landmark::Plane("A1.bottom", bounds("A1").y1),
                        Landmark::Plane("A2.top", bounds("A2").y0),
                        Landmark::ShaftEndpoint("d1.marker_end", End::End),
                    ],
                })
                .collect()
        } else {
            vec![Seam {
                edge: edge("inner", "api"),
                landmarks: vec![
                    Landmark::ShaftEndpoint("inner.shaft_start", End::Start),
                    Landmark::Plane("outer.right", bounds("outer").x1),
                    Landmark::ShaftEndpoint("api.marker_end", End::End),
                ],
            }]
        };
        let metrics = emit(
            name,
            drawing
                .model
                .direction
                .as_deref()
                .expect("witness direction"),
            &capture.scene,
            &capture.body_names,
            &capture.edge_names,
            &seams,
        );
        if name.ends_with("_048") {
            assert!(
                metrics.border_seam_bump.is_some(),
                "missing seam measurement"
            );
        }
    }
}

#[test]
fn moving_only_painted_geometry_exposes_title_collision_and_disconnected_border() {
    let drawing = draw(
        "%%{init: {\"htmlLabels\": false}}%%\nflowchart LR\nsubgraph one[One]\n a{A}\nend\nsubgraph two[Two]\n b((B))\nend\nsubgraph three[Three]\n c[C]\nend\na --> b",
    );
    let doc = Document::parse(&drawing.svg).unwrap();
    let clean = drawing.capture(&doc);
    let body = |name| clean.body_names.iter().position(|id| id == name).unwrap();
    let (one, two, c) = (body("one"), body("two"), body("c"));
    assert_eq!(clean.counts.stitches, 2);
    assert!(clean.failures.is_empty(), "{}", clean.failures.join("\n"));
    let frame = viewport(doc.root_element(), view_box(doc.root_element()).size());
    let title = doc
        .descendants()
        .find(|n| {
            has_class(*n, "cluster-label")
                && n.parent_element().and_then(|p| p.attribute("data-id")) == Some("one")
        })
        .unwrap();
    let title_box = clean
        .scene
        .labels
        .iter()
        .find(|l| l.owner == LabelOwner::Body(one))
        .unwrap()
        .bounds;
    let length = text_element(title)
        .unwrap()
        .attribute_node("textLength")
        .unwrap();
    let mut wider = drawing.svg.clone();
    let width = 2. * title_box.width();
    wider.replace_range(length.range_value(), &width.to_string());
    let wider_doc = Document::parse(&wider).unwrap();
    let widened = drawing.capture(&wider_doc);
    let actual = widened
        .scene
        .labels
        .iter()
        .find(|l| l.owner == LabelOwner::Body(one))
        .unwrap()
        .bounds;
    assert!((actual.x0 - title_box.x0).abs() < 1e-7 && (actual.width() - width).abs() < 1e-7);
    assert!(
        widened
            .failures
            .iter()
            .any(|failure| failure.contains("one title: painted bounds"))
    );
    let local = to_viewport(title.parent_element().unwrap(), frame).inverse();
    let delta = local * clean.scene.bodies[c].bounds.center() - local * title_box.center();
    let svg = super::wrap_in_transform(
        &drawing.svg,
        title,
        &format!("translate({} {})", delta.x, delta.y),
    );
    let moved_doc = Document::parse(&svg).unwrap();
    let moved = drawing.capture(&moved_doc);
    assert!(
        moved
            .failures
            .iter()
            .any(|e| e.contains("one label overlaps c: LabelNodeOverlap"))
    );
    assert!(
        moved
            .failures
            .iter()
            .any(|e| e.contains("one label outside one"))
    );
    let edge = &clean.scene.edges[0];
    let shaft = Shaft::read(edge).unwrap();
    let start = shaft.sample(End::Start, 0.);
    let end = shaft.sample(End::End, 0.);
    let line = kurbo::Line::new(start, end);
    let outline = clean.scene.bodies[two].contour.as_ref().unwrap();
    let crossing = outline
        .segments()
        .find_map(|s| s.intersect_line(line).first().map(|h| line.eval(h.line_t)))
        .unwrap();
    let axis = (end - start).normalize();
    let local = edge.transform.inverse();
    let points = [start, crossing - axis * 2., crossing + axis * 2., end].map(|p| local * p);
    let path = doc
        .descendants()
        .find(|n| n.attribute("data-et") == Some("edge"))
        .unwrap();
    let replace = |d: String| {
        let mut svg = drawing.svg.clone();
        svg.replace_range(path.attribute_node("d").unwrap().range_value(), &d);
        let doc = Document::parse(&svg).unwrap();
        drawing.capture(&doc).failures
    };
    let [a, b, cross, d] = points;
    let failures = replace(format!(
        "M{} {}L{} {}M{} {}L{} {}",
        a.x, a.y, b.x, b.y, cross.x, cross.y, d.x, d.y
    ));
    assert!(
        failures
            .iter()
            .any(|e| e.contains("disconnected painted subpaths"))
    );
    assert!(
        failures
            .iter()
            .any(|e| e.contains("group two expected one border crossing"))
    );
    let sibling = local * clean.scene.bodies[c].bounds.center();
    let failures = replace(format!(
        "M{} {}L{} {}L{} {}",
        a.x, a.y, sibling.x, sibling.y, d.x, d.y
    ));
    assert!(
        failures
            .iter()
            .any(|e| e.contains("enters three: ForeignGroupHit"))
    );
    assert!(
        failures
            .iter()
            .any(|e| e.contains("enters c: ForeignNodeHit"))
    );
}

fn centered(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect::from_center_size((x, y), (w, h))
}

fn compare_bounds(actual: Rect, expected: Rect, owner: &str, failures: &mut Vec<String>) {
    if [
        actual.x0 - expected.x0,
        actual.y0 - expected.y0,
        actual.x1 - expected.x1,
        actual.y1 - expected.y1,
    ]
    .iter()
    .any(|delta| !delta.is_finite() || delta.abs() > FLATNESS)
    {
        failures.push(format!(
            "{owner}: painted bounds {actual:?}, layout {expected:?}"
        ));
    }
}

fn normal(bounds: Rect, port: Point, shoulder: Point, own_member: bool, frame: Affine) -> Vec2 {
    let delta = shoulder - port;
    let faces = [
        ((port.x - bounds.x0).abs(), Vec2::new(-1., 0.)),
        ((port.x - bounds.x1).abs(), Vec2::new(1., 0.)),
        ((port.y - bounds.y0).abs(), Vec2::new(0., -1.)),
        ((port.y - bounds.y1).abs(), Vec2::new(0., 1.)),
    ];
    let (_, mut face) = faces
        .into_iter()
        .filter(|(d, _)| *d < 1e-5)
        .max_by(|(_, a), (_, b)| a.dot(delta).abs().total_cmp(&b.dot(delta).abs()))
        .unwrap_or_else(|| panic!("layout port {port:?} outside faces of {bounds:?}"));
    if own_member && face.dot(delta) < 0. {
        face = -face;
    }
    (frame * face.to_point() - frame * Point::ORIGIN).normalize()
}

fn ancestors(scene: &Scene<'_>, body: usize) -> Vec<usize> {
    let mut chain = Vec::new();
    let mut cursor = scene.bodies[body].parent;
    while let Some(parent) = cursor {
        assert!(
            !chain.contains(&parent),
            "cyclic parent chain at body {parent}"
        );
        chain.push(parent);
        cursor = scene.bodies[parent].parent;
    }
    chain
}

impl Drawing {
    fn capture<'a>(&self, doc: &'a Document<'_>) -> Capture<'a> {
        let root = doc.root_element();
        let frame = viewport(root, view_box(root).size());
        let graph_frame = doc
            .descendants()
            .find(|n| has_class(*n, "root"))
            .map_or(frame, |n| to_viewport(n, frame));
        let view = FlowchartRenderModelRef::new(&self.model, &self.context);
        let mut bodies = BTreeMap::new();
        let mut paths = Vec::new();
        let mut labels = Vec::new();
        let mut markers = HashMap::new();
        for n in doc.descendants().filter(Node::is_element) {
            match (n.tag_name().name(), n.attribute("data-et")) {
                ("g", Some(kind @ ("node" | "cluster"))) => {
                    assert!(
                        bodies.insert(attr(n, "data-id"), (kind, n)).is_none(),
                        "duplicate body {}",
                        attr(n, "data-id")
                    );
                }
                ("marker", _) => {
                    assert!(markers.insert(attr(n, "id"), n).is_none());
                }
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
        let groups: BTreeMap<_, _> = view.subgraphs.iter().map(|g| (g.id.as_str(), g)).collect();
        let nodes: BTreeMap<_, _> = view.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
        let visible: BTreeSet<_> = groups
            .keys()
            .chain(nodes.keys())
            .copied()
            .filter(|id| view.collapsed_replacement(id).is_none())
            .collect();
        assert_eq!(
            visible,
            bodies.keys().copied().collect(),
            "semantic / painted bodies"
        );
        let parents: HashMap<_, _> = view.visible_memberships().collect();
        let ids: BTreeMap<_, _> = visible
            .iter()
            .enumerate()
            .map(|(i, &name)| (name, i))
            .collect();
        let layout_nodes: HashMap<_, _> = self
            .layout
            .nodes
            .iter()
            .filter(|n| !n.is_cluster)
            .map(|n| (n.id.as_str(), n))
            .collect();
        let layout_groups: HashMap<_, _> = self
            .layout
            .clusters
            .iter()
            .map(|g| (g.id.as_str(), g))
            .collect();
        let mut capture = Capture {
            scene: Scene::default(),
            body_names: Vec::new(),
            edge_names: Vec::new(),
            model_boxes: Vec::new(),
            edge_knots: Vec::new(),
            painted_knots: Vec::new(),
            to_layout: graph_frame.inverse(),
            rough_contacts: BTreeMap::new(),
            rough_group_frames: BTreeMap::new(),
            ink: ink::Deviation::default(),
            counts: Counts::default(),
            failures: Vec::new(),
        };
        let settings = crate::flowchart::FlowchartConfigView::new(&self.config).layout_settings();
        for (name, (kind, owner)) in bodies {
            let id = ids[name];
            let is_group = groups.contains_key(name) && !view.is_subgraph_collapsed(name);
            assert_eq!(kind, if is_group { "cluster" } else { "node" }, "{name}");
            let outline = shape_outline(owner, frame, is_group);
            let measured;
            let bounds;
            let label_bounds;
            if is_group {
                let cluster = layout_groups[name];
                measured = centered(cluster.x, cluster.y, cluster.width, cluster.height);
                bounds = outline.bounding_box();
                let expected = graph_frame.transform_rect_bbox(measured);
                if owner.attribute("data-look") == Some("handDrawn") {
                    let delta = [
                        bounds.x0 - expected.x0,
                        bounds.y0 - expected.y0,
                        bounds.x1 - expected.x1,
                        bounds.y1 - expected.y1,
                    ];
                    for distance in delta {
                        capture.ink.group_box = capture.ink.group_box.max(distance.abs());
                    }
                    let radius = if view.keyword == "agentflow-beta" {
                        10.
                    } else {
                        0.
                    };
                    let nominal = kurbo::RoundedRect::from_rect(measured, radius);
                    capture
                        .rough_group_frames
                        .insert(id, graph_frame * nominal.to_path(FLATNESS));
                } else {
                    compare_bounds(bounds, expected, name, &mut capture.failures);
                }
                let title = owner
                    .descendants()
                    .find(|n| has_class(*n, "cluster-label"))
                    .unwrap_or_else(|| panic!("group {name}: missing title slot"));
                let anchor = &cluster.title_label;
                label_bounds = explicit_label_bounds(title, frame).or_else(|| {
                    text_element(title).map(|text| {
                        let width = text
                            .attribute("textLength")
                            .map_or(anchor.width, |length| length.parse::<f64>().unwrap());
                        to_viewport(text, frame).transform_rect_bbox(Rect::from_origin_size(
                            (0., 0.),
                            (width, anchor.height),
                        ))
                    })
                });
                if let Some(bounds) = label_bounds {
                    compare_bounds(
                        bounds,
                        graph_frame.transform_rect_bbox(centered(
                            anchor.x,
                            anchor.y,
                            anchor.width,
                            anchor.height,
                        )),
                        &format!("{name} title"),
                        &mut capture.failures,
                    );
                    capture.counts.titles += 1;
                } else {
                    assert!(
                        cluster.title.trim().is_empty(),
                        "{name}: missing painted title {:?}",
                        cluster.title
                    );
                }
                capture.counts.groups += 1;
            } else {
                let node = layout_nodes[name];
                measured = centered(node.x, node.y, node.width, node.height);
                bounds = to_viewport(owner, frame).transform_rect_bbox(centered(
                    0.,
                    0.,
                    node.width,
                    node.height,
                ));
                compare_bounds(
                    bounds,
                    graph_frame.transform_rect_bbox(measured),
                    name,
                    &mut capture.failures,
                );
                let label = owner
                    .descendants()
                    .find(|n| n.has_tag_name("g") && has_class(*n, "label"));
                label_bounds = label.and_then(|label| {
                    explicit_label_bounds(label, frame).or_else(|| {
                        let text = text_element(label)?;
                        let (raw, label_type, classes, styles) =
                            if let Some(group) = groups.get(name) {
                                (
                                    group.title.as_str(),
                                    group.label_type.as_deref(),
                                    &group.classes,
                                    &group.styles,
                                )
                            } else {
                                let semantic = nodes[name];
                                (
                                    view.node_label_for_render(semantic).unwrap_or(name),
                                    semantic.label_type.as_deref(),
                                    &semantic.classes,
                                    &semantic.styles,
                                )
                            };
                        let style =
                            crate::flowchart::flowchart_effective_text_style_for_node_classes(
                                &settings.text_style,
                                &view.class_defs,
                                classes,
                                styles,
                            );
                        let metrics = crate::flowchart::flowchart_label_metrics_for_layout(
                            crate::flowchart::FlowchartLabelMetricsRequest {
                                measurer: &crate::text::DeterministicTextMeasurer::default(),
                                raw_label: raw,
                                label_type: label_type.unwrap_or("text"),
                                style: &style,
                                max_width_px: Some(settings.wrapping_width),
                                wrap_mode: crate::text::WrapMode::SvgLike,
                                config: &self.config,
                                math_renderer: None,
                            },
                        );
                        Some(
                            to_viewport(text, frame).transform_rect_bbox(Rect::from_origin_size(
                                (-metrics.width / 2., 0.),
                                (metrics.width, metrics.height),
                            )),
                        )
                    })
                });
                if label_bounds.is_none() {
                    let expected = groups
                        .get(name)
                        .map(|group| group.title.as_str())
                        .or_else(|| {
                            nodes
                                .get(name)
                                .and_then(|node| view.node_label_for_render(node))
                        })
                        .unwrap_or(name);
                    let empty_svg_row = expected.trim().is_empty()
                        && label
                            .is_some_and(|slot| slot.descendants().any(|n| n.has_tag_name("text")));
                    if !crate::flowchart::flowchart_label_is_empty_for_render(expected)
                        && !empty_svg_row
                    {
                        capture
                            .failures
                            .push(format!("{name}: absent painted label {expected:?}"));
                    }
                }
                capture.counts.node_labels += usize::from(label_bounds.is_some());
                capture.counts.nodes += 1;
            }
            capture.scene.bodies.push(Body {
                bounds,
                parent: parents.get(name).map(|parent| ids[parent]),
                contour: Some(outline),
                is_group,
            });
            capture.body_names.push(name.to_owned());
            capture.model_boxes.push(measured);
            if let Some(bounds) = label_bounds {
                capture.scene.labels.push(Label {
                    owner: LabelOwner::Body(id),
                    bounds,
                });
            }
        }
        assert_eq!(
            capture.counts.nodes,
            layout_nodes.len(),
            "layout leaf count"
        );
        assert_eq!(
            capture.counts.groups,
            layout_groups.len(),
            "layout group count"
        );
        let mut painted = HashMap::<_, Vec<_>>::new();
        assert_eq!(
            paths.len(),
            labels.len(),
            "every painted edge needs a label slot"
        );
        for (path, label) in paths.into_iter().zip(labels) {
            let name = attr(path, "data-id");
            assert_eq!(
                name,
                attr(label, "data-id"),
                "pair edge and label by painted occurrence"
            );
            painted.entry(name).or_default().push((path, label));
        }
        let projected: Vec<_> = view.projected_edges().collect();
        let duplicated: BTreeSet<_> = painted
            .iter()
            .filter(|(_, slots)| slots.len() > 1)
            .map(|(&name, _)| name)
            .collect();
        assert_eq!(
            projected.len(),
            self.layout.edges.len(),
            "visible semantic / layout occurrences"
        );
        let axis = if matches!(view.direction.as_deref(), Some("LR" | "RL")) {
            CrossAxis::Y
        } else {
            CrossAxis::X
        };
        for ((key, projected), layout) in projected.into_iter().zip(&self.layout.edges) {
            let semantic = projected.semantic;
            let occurrence = key.semantic_index();
            let name = format!(
                "{}#{occurrence} ({} → {})",
                semantic.id, projected.from, projected.to
            );
            assert_eq!(
                (&*layout.id, &*layout.from, &*layout.to),
                (&*semantic.id, projected.from, projected.to),
                "{name}"
            );
            let slots = painted
                .get_mut(layout.id.as_str())
                .unwrap_or_else(|| panic!("{name}: missing shaft"));
            let suffix = format!("-edge-{occurrence}");
            let index = if duplicated.contains(layout.id.as_str()) {
                slots
                    .iter()
                    .position(|(path, _)| attr(*path, "id").ends_with(&suffix))
                    .unwrap_or_else(|| panic!("{name}: missing scoped SVG occurrence"))
            } else {
                0
            };
            let (path, label) = slots.remove(index);
            if semantic.visibility == FlowEdgeVisibility::Invisible {
                assert!(
                    attr(path, "class").contains("invisible"),
                    "{name}: layout-only constraint is painted"
                );
                continue;
            }
            let (from, to) = (ids[projected.from], ids[projected.to]);
            let mut edge = Edge::new(from, to, attr(path, "d"), axis);
            edge.transform = to_viewport(path, frame);
            let knots: Vec<_> = layout.points.iter().map(|p| Point::new(p.x, p.y)).collect();
            assert!(
                knots.len() >= 2 && knots.iter().all(|p| p.is_finite()),
                "{name}: invalid layout route"
            );
            let (start, end) = crate::svg::flowchart_marker_metrics(semantic);
            for (side, body, opposite, end, port, shoulder, marker, trim) in [
                (
                    &mut edge.start,
                    from,
                    to,
                    End::Start,
                    0,
                    1,
                    semantic.start_marker,
                    start.trim,
                ),
                (
                    &mut edge.end,
                    to,
                    from,
                    End::End,
                    knots.len() - 1,
                    knots.len() - 2,
                    semantic.end_marker,
                    end.trim,
                ),
            ] {
                let inward = ancestors(&capture.scene, opposite).contains(&body);
                side.outward_normal = Some(normal(
                    capture.model_boxes[body],
                    knots[port],
                    knots[shoulder],
                    inward,
                    graph_frame,
                ));
                read_marker(path, &markers, end, marker, trim, edge.transform, side);
            }
            super::check_unmarked_departure(
                &mut edge.start,
                nodes
                    .get(projected.from)
                    .and_then(|node| node.layout_shape.as_deref()),
                path,
                semantic.start_marker,
            );
            let edge_id = capture.scene.edges.len();
            use base64::Engine as _;
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(attr(path, "data-points"))
                .unwrap();
            let projected: Vec<crate::model::LayoutPoint> = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(
                projected.len(),
                knots.len(),
                "{name}: projected route occurrences"
            );
            let projected: Vec<_> = projected
                .iter()
                .map(|p| edge.transform * Point::new(p.x, p.y))
                .collect();
            if path.attribute("data-look") == Some("handDrawn") {
                let contacts = [projected[0], projected[knots.len() - 1]];
                for (endpoint, contact, raw) in [
                    (from, contacts[0], knots[0]),
                    (to, contacts[1], knots[knots.len() - 1]),
                ] {
                    if nodes
                        .get(capture.body_names[endpoint].as_str())
                        .is_some_and(|node| {
                            matches!(
                                node.layout_shape.as_deref(),
                                None | Some("squareRect" | "rect")
                            )
                        })
                    {
                        assert!(
                            contact.distance(graph_frame * raw) <= FLATNESS,
                            "{name}: projected rectangle port {contact:?} differs from declared layout port {raw:?}"
                        );
                    }
                }
                capture.rough_contacts.insert(edge_id, contacts);
            }
            capture.painted_knots.push(projected);
            capture.scene.edges.push(edge);
            capture
                .edge_knots
                .push(knots.iter().map(|&p| graph_frame * p).collect());
            let label_bounds = explicit_label_bounds(label, frame);
            assert_eq!(
                label_bounds.is_some(),
                layout.label.is_some(),
                "{name}: painted / measured edge label"
            );
            if let (Some(bounds), Some(anchor)) = (label_bounds, &layout.label) {
                if bounds
                    .center()
                    .distance(graph_frame * Point::new(anchor.x, anchor.y))
                    > FLATNESS
                {
                    capture.failures.push(format!(
                        "{name}: painted label center {:?}, layout {anchor:?}",
                        bounds.center()
                    ));
                }
                capture.scene.labels.push(Label {
                    owner: LabelOwner::Edge(edge_id),
                    bounds,
                });
                capture.counts.edge_labels += 1;
            }
            capture.edge_names.push(name);
            capture.counts.shafts += 1;
        }
        assert!(
            painted.values().all(Vec::is_empty),
            "extra painted edge occurrence"
        );
        capture.audit();
        capture
    }
}
