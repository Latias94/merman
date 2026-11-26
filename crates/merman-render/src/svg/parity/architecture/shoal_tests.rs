use base64::{Engine as _, engine::general_purpose::STANDARD};
use kurbo::{Affine, BezPath, Line, ParamCurve, ParamCurveNearest, PathSeg, Point, Rect};
use merman_core::{MermaidEngine, ParseOptions};
use roxmltree::{Document, Node};
use std::str::FromStr;

use crate::environment::RenderEnvironment;
use crate::model::{ArchitectureDiagramLayout, LayoutPoint};
use crate::svg::SvgRenderOptions;
use crate::theme::{NativeTheme, ThemeCompiler};
use crate::{LayoutOptions, family};

fn render(source: &str) -> (ArchitectureDiagramLayout, String) {
    render_with_theme(source, None)
}

fn render_with_theme(
    source: &str,
    theme: Option<NativeTheme>,
) -> (ArchitectureDiagramLayout, String) {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            &format!("---\nconfig:\n  layout: shoal\n---\n{source}"),
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap()
        .into_presentation();
    let environment = RenderEnvironment::deterministic();
    let session = match theme {
        Some(theme) => environment.begin_session_with_native_theme(theme),
        None => environment.begin_session(),
    }
    .unwrap();
    let artifact =
        family::prepare(&parsed, &LayoutOptions::headless_svg_defaults(), session).unwrap();
    let json = artifact.layout_json().unwrap();
    let layout = serde_json::from_value(json["layout"]["ArchitectureDiagram"].clone()).unwrap();
    let svg = artifact
        .render_svg(&SvgRenderOptions::default(), &Default::default())
        .unwrap();
    (layout, svg.svg().to_owned())
}

fn element<'a, 'i>(doc: &'a Document<'i>, kind: &str, id: &str) -> Node<'a, 'i> {
    doc.descendants()
        .find(|node| {
            node.attribute("data-et") == Some(kind) && node.attribute("data-id") == Some(id)
        })
        .unwrap_or_else(|| panic!("missing Architecture {kind} {id}"))
}

fn transform(node: Node<'_, '_>) -> Affine {
    let Some(transform) = node.attribute("transform") else {
        return Affine::IDENTITY;
    };
    let transform = svgtypes::Transform::from_str(transform).unwrap();
    Affine::new([
        transform.a,
        transform.b,
        transform.c,
        transform.d,
        transform.e,
        transform.f,
    ])
}

fn number(node: Node<'_, '_>, key: &str) -> f64 {
    node.attribute(key).unwrap().parse().unwrap()
}

fn shaft(node: Node<'_, '_>) -> Vec<Line> {
    BezPath::from_svg(node.attribute("d").unwrap())
        .unwrap()
        .segments()
        .map(|seg| {
            let PathSeg::Line(leg) = seg else {
                panic!("Architecture shaft must be orthogonal")
            };
            assert!(leg.p0.x == leg.p1.x || leg.p0.y == leg.p1.y, "{leg:?}");
            leg
        })
        .collect()
}

fn contacts(node: Node<'_, '_>) -> Vec<Point> {
    let json = STANDARD
        .decode(node.attribute("data-points").unwrap())
        .unwrap();
    serde_json::from_slice::<Vec<LayoutPoint>>(&json)
        .unwrap()
        .into_iter()
        .map(|p| Point::new(p.x, p.y))
        .collect()
}

const GEOMETRY_EPSILON: f64 = 1e-7;

fn near(a: Point, b: Point) -> bool {
    a.distance(b) < GEOMETRY_EPSILON
}

fn junction_contact(junction: Node<'_, '_>) -> Point {
    let body = junction
        .descendants()
        .find(|node| node.has_tag_name("rect"))
        .unwrap();
    transform(junction) * Point::new(number(body, "width") / 2.0, number(body, "height") / 2.0)
}

fn check_left_icon_contact(service: Node<'_, '_>, point: Point) {
    let body = service
        .descendants()
        .find(|node| node.has_tag_name("svg"))
        .unwrap();
    let top = transform(service) * Point::ZERO;
    let bottom = transform(service) * Point::new(0.0, number(body, "height"));
    assert!(
        (point.x - top.x).abs() < GEOMETRY_EPSILON && (top.y..=bottom.y).contains(&point.y),
        "expected {point:?} on the left face of {:?}",
        service.attribute("data-id")
    );
}

fn vertical(leg: Line) -> bool {
    leg.p0.x == leg.p1.x
}

fn check_comb_corner_guards(legs: &[Line], points: &[Point]) {
    for (index, bend) in legs.windows(2).enumerate() {
        let (incoming, outgoing) = (bend[0], bend[1]);
        if vertical(incoming) == vertical(outgoing) {
            continue;
        }
        // Guard points sit strictly inside the straight legs on both sides of the bend.
        assert!(
            index > 0
                && vertical(legs[index - 1]) == vertical(incoming)
                && legs
                    .get(index + 2)
                    .is_some_and(|&next| vertical(next) == vertical(outgoing)),
            "expected two corner guards in the SVG at {:?}",
            incoming.end()
        );
        for guard in [incoming.start(), outgoing.end()] {
            assert!(points.iter().any(|&point| near(point, guard)), "{guard:?}");
        }
    }
}

fn check_both_arrows(path: Node<'_, '_>, first: Point, last: Point) {
    let legs = shaft(path);
    let arrows: Vec<_> = path
        .parent_element()
        .unwrap()
        .children()
        .filter(|node| node.has_tag_name("polygon"))
        .collect();
    assert_eq!(arrows.len(), 2);
    for (arrow, (tip, base)) in arrows
        .into_iter()
        .zip([(first, legs[0].start()), (last, legs.last().unwrap().end())])
    {
        let vertices: Vec<_> = svgtypes::PointsParser::from(arrow.attribute("points").unwrap())
            .map(|(x, y)| transform(arrow) * Point::new(x, y))
            .collect();
        assert_eq!(vertices.len(), 3);
        assert_eq!(
            vertices.iter().filter(|&&vertex| near(vertex, tip)).count(),
            1
        );
        let mut base_points = vertices.into_iter().filter(|&point| !near(point, tip));
        assert!(near(
            base_points
                .next()
                .unwrap()
                .midpoint(base_points.next().unwrap()),
            base
        ));
    }
}

#[test]
fn architecture_shoal_paints_a_shared_crossbar_with_a_marked_leaf() {
    let (_, svg) = render(
        "architecture-beta\n\
         junction hub\n\
         service leaf0(server)\n\
         service leaf1(server)\n\
         service leaf2(server)\n\
         service leaf3(server)\n\
         service leaf4(server)\n\
         service leaf5(server)\n\
         align column leaf0 leaf1 leaf2 leaf3 leaf4 leaf5\n\
         hub:R --> L:leaf0\n\
         hub:R -- L:leaf1\n\
         hub:R -- L:leaf2\n\
         hub:R -- L:leaf3\n\
         hub:R -- L:leaf4\n\
         hub:R -- L:leaf5\n",
    );
    let doc = Document::parse(&svg).unwrap();
    assert_eq!(
        doc.descendants()
            .filter(|node| node.attribute("data-et") == Some("edge"))
            .count(),
        6
    );
    let diagram_id = doc.root_element().attribute("id").unwrap();
    let hub = junction_contact(element(&doc, "node", "hub"));
    let mut leaf_rows: Vec<f64> = Vec::new();
    let mut bars = Vec::new();
    for index in 0..6 {
        let leaf_id = format!("leaf{index}");
        let path = element(&doc, "edge", &format!("edge-{index}"));
        let id = format!("{diagram_id}-L_hub_{leaf_id}_{index}");
        assert!(path.has_tag_name("path"));
        assert_eq!(path.attribute("id"), Some(id.as_str()));
        assert!(
            ["marker-start", "marker-end"]
                .iter()
                .all(|&name| path.attribute(name).is_none())
        );
        let arrows: Vec<_> = path
            .parent_element()
            .unwrap()
            .children()
            .filter(|node| node.has_tag_name("polygon"))
            .collect();
        assert_eq!(arrows.len(), usize::from(index == 0));
        assert!(!path.parent_element().unwrap().children().any(|node| {
            node.has_tag_name("polyline") || node.attribute("data-et") == Some("edge-label")
        }));

        let legs = shaft(path);
        let first = legs.first().unwrap();
        let last = legs.last().unwrap();
        let points = contacts(path);
        let tip = *points.last().unwrap();
        assert!(
            near(first.start(), hub),
            "{id}: {:?} vs {hub:?}",
            first.start()
        );
        assert!(near(points[0], hub), "{id}: {:?} vs {hub:?}", points[0]);
        if let [arrow] = arrows.as_slice() {
            let vertices: Vec<_> = svgtypes::PointsParser::from(arrow.attribute("points").unwrap())
                .map(|(x, y)| transform(*arrow) * Point::new(x, y))
                .collect();
            assert!(vertices.iter().any(|&p| near(p, tip)), "{id}");
            let mut base = vertices.into_iter().filter(|&p| !near(p, tip));
            assert!(
                near(
                    base.next().unwrap().midpoint(base.next().unwrap()),
                    last.end()
                ),
                "{id}"
            );
        } else {
            assert!(near(last.end(), tip), "{id}");
        }
        check_left_icon_contact(element(&doc, "node", &leaf_id), tip);
        assert!(first.p0.x < first.p1.x && last.p0.x < last.p1.x, "{id}");
        assert!(
            legs.windows(2)
                .all(|leg| near(leg[0].end(), leg[1].start()))
        );
        check_comb_corner_guards(&legs, &points);

        assert!(
            leaf_rows
                .iter()
                .all(|&y| (y - tip.y).abs() > GEOMETRY_EPSILON)
        );
        leaf_rows.push(tip.y);
        // Only the vertical bar in the open corridor counts, even if the hub lead overlaps.
        bars.extend(
            legs.into_iter()
                .filter(|&leg| vertical(leg) && hub.x < leg.p0.x && leg.p0.x < tip.x)
                .map(|leg| (index, Rect::from_points(leg.p0, leg.p1))),
        );
    }
    assert!(
        bars.iter().enumerate().any(|(index, &(edge, bar))| {
            bars[..index].iter().any(|&(peer, other)| {
                edge != peer
                    && (bar.x0 - other.x0).abs() < GEOMETRY_EPSILON
                    && bar.y1.min(other.y1) - bar.y0.max(other.y0) > GEOMETRY_EPSILON
            })
        }),
        "expected two edge paths to share a positive length of the crossbar"
    );
}

#[test]
fn architecture_shoal_paints_full_routes_and_true_double_arrow_contacts() {
    let (layout, svg) = render(
        "architecture-beta\n\
         group outer[Network]\n\
         group inner[Application] in outer\n\
         service api[Application Gateway] in inner\n\
         service client(server)[Public client]\n\
         service store(database)[Cold storage] in inner\n\
         junction hub in outer\n\
         api:B <-[synchronize]-> L:store\n\
         api{group}:R <--> L:hub\n\
         hub:R <--> L:client\n",
    );
    let doc = Document::parse(&svg).unwrap();
    let inner = element(&doc, "cluster", "inner");
    let api = element(&doc, "node", "api");
    let hub = element(&doc, "node", "hub");
    let client = element(&doc, "node", "client");
    let store = element(&doc, "node", "store");
    let api_outline = api
        .descendants()
        .find(|node| node.has_tag_name("path"))
        .unwrap();
    let painted_outline =
        transform(api) * BezPath::from_svg(api_outline.attribute("d").unwrap()).unwrap();
    let api_route = element(&doc, "edge", "edge-0");
    let api_points = contacts(api_route);
    assert!(
        painted_outline
            .segments()
            .any(|segment| near(segment.start(), api_points[0])
                || segment.nearest(api_points[0], 1e-6).distance_sq < 1e-10)
    );
    let api_shaft = shaft(api_route);
    assert!(api_shaft.len() >= 2);
    assert_eq!(api_shaft[0].start().x, api_shaft[0].end().x);
    let store_lead = api_shaft.last().unwrap();
    assert_eq!(store_lead.start().y, store_lead.end().y);
    assert!(api_points.len() > 3);
    let store_tip = *api_points.last().unwrap();
    check_left_icon_contact(store, store_tip);
    check_both_arrows(api_route, api_points[0], store_tip);
    let label = element(&doc, "edge-label", "edge-0");
    assert_eq!(label.parent_element(), api_route.parent_element());
    assert!(
        label
            .descendants()
            .any(|node| node.is_text() && node.text() == Some("synchronize"))
    );

    let group_route = element(&doc, "edge", "edge-1");
    let group_points = contacts(group_route);
    let junction = junction_contact(hub);
    let group_rect = Rect::new(
        number(inner, "x"),
        number(inner, "y"),
        number(inner, "x") + number(inner, "width"),
        number(inner, "y") + number(inner, "height"),
    );
    assert!((group_points[0].x - group_rect.x1).abs() < 1e-7);
    assert!((group_rect.y0..=group_rect.y1).contains(&group_points[0].y));
    assert!(near(*group_points.last().unwrap(), junction));
    check_both_arrows(group_route, group_points[0], junction);
    assert_eq!(layout.edges[1].from, "api");
    assert_eq!(layout.edges[1].from_cluster.as_deref(), Some("inner"));

    let outer_route = element(&doc, "edge", "edge-2");
    let outer_points = contacts(outer_route);
    assert!(near(outer_points[0], junction));
    let client_tip = *outer_points.last().unwrap();
    check_left_icon_contact(client, client_tip);
    check_both_arrows(outer_route, junction, client_tip);
}

#[test]
fn architecture_shoal_open_markers_keep_canonical_contacts_under_theme_v2() {
    let source = "architecture-beta\n\
        group root[Network]\n\
        service api[Application Gateway] in root\n\
        service store(database)[Cold storage] in root\n\
        api:B <-[synchronize]-> L:store\n";
    let theme = ThemeCompiler::new()
        .compile_json(
            br##"{
                "schema_version":2,"extends":"studio",
                "roles":{"surface.node":"#DDEEFF","surface.container":"#E0E8F0"},
                "typography":{"sans":["Architecture Text","sans-serif"],"base_size_px":24},
                "form":{"radius":7,"container_radius":11,"connector_width":4,
                        "edge_style":"orthogonal","marker_style":"open","elevation":"flat"}
            }"##,
            None,
        )
        .unwrap();
    let (classic, _) = render(source);
    let (layout, svg) = render_with_theme(source, Some(NativeTheme::new(theme)));
    let doc = Document::parse(&svg).unwrap();

    let group = element(&doc, "cluster", "root");
    assert_eq!(group.attribute("rx"), Some("11"));
    assert!(group.attribute("style").unwrap().contains("fill:#E0E8F0"));
    let api = element(&doc, "node", "api");
    let card = api
        .children()
        .find(|node| node.attribute("class") == Some("architecture-native-service-card"))
        .unwrap();
    assert_eq!(card.attribute("rx"), Some("7"));
    assert_eq!(card.attribute("fill"), Some("#DDEEFF"));
    let css = doc
        .descendants()
        .find(|node| node.has_tag_name("style"))
        .unwrap()
        .text()
        .unwrap();
    assert!(css.contains("font-family:\"Architecture Text\", sans-serif;font-size:24px;"));
    assert!(css.contains(".edge{stroke-width:4px;"));
    assert!(
        layout.edges[0].label.as_ref().unwrap().width
            > classic.edges[0].label.as_ref().unwrap().width
    );
    element(&doc, "edge-label", "edge-0");

    let path = element(&doc, "edge", "edge-0");
    let points = contacts(path);
    let legs = shaft(path);
    let start = points[0];
    let end = *points.last().unwrap();
    assert!(near(legs[0].start(), start));
    assert!(near(legs.last().unwrap().end(), end));
    let markers: Vec<_> = path
        .parent_element()
        .unwrap()
        .children()
        .filter(|node| node.has_tag_name("polyline"))
        .collect();
    assert_eq!(markers.len(), 2);
    for (marker, tip) in markers.into_iter().zip([start, end]) {
        assert_eq!(marker.attribute("class"), Some("arrow arrow-open"));
        let (x, y) = svgtypes::PointsParser::from(marker.attribute("points").unwrap())
            .nth(1)
            .unwrap();
        assert!(near(transform(marker) * Point::new(x, y), tip));
    }
}

#[test]
fn architecture_shoal_honors_row_column_and_declared_order_in_a_wired_group() {
    let (layout, svg) = render(
        "architecture-beta\n\
         group root(cloud)[Architecture grid]\n\
         service a[A] in root\n\
         service b[B] in root\n\
         service c[C] in root\n\
         service d[D] in root\n\
         align row a b\n\
         align row c d\n\
         align column a c\n\
         align column b d\n\
         a:R <--> L:b\n\
         a:B <--> T:c\n\
         b:B <--> T:d\n\
         c:R <--> L:d\n",
    );
    let centers: Vec<_> = layout
        .nodes
        .iter()
        .take(4)
        .map(|node| Point::new(node.x + node.width / 2.0, node.y + node.height / 2.0))
        .collect();
    let [a, b, c, d] = centers.as_slice() else {
        panic!("missing services")
    };
    assert_eq!(a.y, b.y);
    assert_eq!(c.y, d.y);
    assert_eq!(a.x, c.x);
    assert_eq!(b.x, d.x);
    assert!(a.x < b.x && a.y < c.y);
    let doc = Document::parse(&svg).unwrap();
    for index in 0..4 {
        let route = element(&doc, "edge", &format!("edge-{index}"));
        let points = contacts(route);
        check_both_arrows(route, points[0], *points.last().unwrap());
    }
}

#[test]
fn architecture_shoal_studio_keeps_bounded_rounded_corners() {
    let source = "architecture-beta\n\
        group edge(cloud)[Edge]\n\
        service cdn(internet)[CDN] in edge\n\
        service gw(server)[API Gateway] in edge\n\
        group core(cloud)[Core Services]\n\
        service auth(server)[Auth] in core\n\
        service orders(server)[Orders] in core\n\
        group data(database)[Storage]\n\
        service cache(disk)[Redis] in data\n\
        service db(database)[Postgres] in data\n\
        cdn:R --> L:gw\n\
        gw:B --> T:auth\n\
        gw:R --> L:orders\n\
        auth:R --> L:cache\n\
        orders:B --> T:db\n";
    let theme = ThemeCompiler::new()
        .compile_json(br#"{"schema_version":2,"extends":"studio"}"#, None)
        .unwrap();
    let (_, svg) = render_with_theme(source, Some(NativeTheme::new(theme)));
    let doc = Document::parse(&svg).unwrap();
    let mut corner_count = 0usize;
    let mut max_arm = 0.0_f64;
    for node in doc
        .descendants()
        .filter(|node| node.has_tag_name("path") && node.attribute("class") == Some("edge"))
    {
        let path = kurbo::BezPath::from_svg(node.attribute("d").unwrap()).unwrap();
        for segment in path.segments() {
            if let kurbo::PathSeg::Cubic(curve) = segment {
                let chord = curve.p3 - curve.p0;
                let arm = chord.x.abs().max(chord.y.abs());
                corner_count += 1;
                max_arm = max_arm.max(arm);
            }
        }
    }
    assert!(corner_count > 0, "expected rounded corners on studio architecture routes");
    assert!(
        max_arm <= 16.0,
        "studio architecture corners must stay bounded, got max arm {max_arm:.2}px"
    );
}

#[test]
fn architecture_shoal_studio_keeps_aligned_trunks_straight_without_overshoot() {
    let source = "architecture-beta\n\
        group edge(cloud)[Edge Layer]\n\
        group core(server)[Core Services]\n\
        service cdn(internet)[CDN] in edge\n\
        service gw(server)[API Gateway] in edge\n\
        service auth(server)[Auth] in core\n\
        service orders(server)[Orders] in core\n\
        service cache(disk)[Redis Cache] in core\n\
        service db(database)[Postgres] in core\n\
        cdn:R --> L:gw\n\
        gw:B --> T:auth\n\
        gw:R --> L:orders\n\
        auth:R --> L:cache\n\
        orders:B --> T:db\n\
        cache:B --> T:db\n";
    let theme = ThemeCompiler::new()
        .compile_json(br#"{"schema_version":2,"extends":"studio"}"#, None)
        .unwrap();
    let (_, svg) = render_with_theme(source, Some(NativeTheme::new(theme)));
    let doc = Document::parse(&svg).unwrap();

    // Aligned vertical edges stay straight without title U-detours or 8 px port jogs.
    for edge_id in ["edge-1", "edge-5"] {
        let d = element(&doc, "edge", edge_id).attribute("d").unwrap();
        assert!(
            !d.contains('A') && !d.contains('C'),
            "{edge_id} should stay dead straight, got {d}"
        );
        let path = kurbo::BezPath::from_svg(d).unwrap();
        let start_x = path.segments().next().unwrap().start().x;
        for seg in path.segments() {
            assert!(
                (seg.start().x - start_x).abs() < 1e-6 && (seg.end().x - start_x).abs() < 1e-6,
                "{edge_id} drifted off x={start_x}: {d}"
            );
            assert!(
                (seg.end().y - seg.start().y).abs() > 1e-3,
                "{edge_id} emitted a degenerate horizontal micro-segment: {d}"
            );
        }
    }

    // gw:R --> L:orders enters core on orders's left corridor without overshooting past orders:L.
    let orders_d = element(&doc, "edge", "edge-2").attribute("d").unwrap();
    let orders_path = kurbo::BezPath::from_svg(orders_d).unwrap();
    for seg in orders_path.segments() {
        assert!(
            seg.end().x >= seg.start().x - 1e-6,
            "gw:R --> L:orders must not backtrack horizontally: {orders_d}"
        );
    }
}
