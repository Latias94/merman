use base64::{Engine as _, engine::general_purpose::STANDARD};
use kurbo::{Affine, BezPath, ParamCurve, ParamCurveNearest, PathSeg, Point, Rect};
use merman_core::{MermaidEngine, ParseOptions};
use roxmltree::{Document, Node};
use std::str::FromStr;

use crate::environment::RenderEnvironment;
use crate::model::{ArchitectureDiagramLayout, LayoutPoint};
use crate::svg::SvgRenderOptions;
use crate::{LayoutOptions, family};

fn render(source: &str) -> (ArchitectureDiagramLayout, String) {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            &format!("---\nconfig:\n  layout: shoal\n---\n{source}"),
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap()
        .into_presentation();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
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

fn shaft(node: Node<'_, '_>) -> Vec<kurbo::Line> {
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

fn near(a: Point, b: Point) -> bool {
    a.distance(b) < 1e-7
}

fn check_both_arrows(path: Node<'_, '_>, first: Point, last: Point) {
    let legs = shaft(path);
    let arrows = path
        .parent_element()
        .unwrap()
        .children()
        .filter(|node| node.has_tag_name("polygon"));
    let mut count = 0;
    for (arrow, (tip, base)) in
        arrows.zip([(first, legs[0].start()), (last, legs.last().unwrap().end())])
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
        count += 1;
    }
    assert_eq!(count, 2);
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
    assert!(api_shaft.len() >= 3);
    assert_eq!(api_shaft[0].start().x, api_shaft[0].end().x);
    let store_lead = api_shaft.last().unwrap();
    assert_eq!(store_lead.start().y, store_lead.end().y);
    assert!(api_points.len() > 3);
    check_both_arrows(api_route, api_points[0], *api_points.last().unwrap());

    let group_route = element(&doc, "edge", "edge-1");
    let group_points = contacts(group_route);
    let junction_body = hub
        .descendants()
        .find(|node| node.has_tag_name("rect"))
        .unwrap();
    let junction = transform(hub)
        * Point::new(
            number(junction_body, "width") / 2.0,
            number(junction_body, "height") / 2.0,
        );
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
    let client_body = client
        .descendants()
        .find(|node| node.has_tag_name("svg") && node.attribute("viewBox") == Some("0 0 80 80"))
        .unwrap();
    let client_left = transform(client) * Point::ZERO;
    let client_tip = *outer_points.last().unwrap();
    assert!((client_tip.x - client_left.x).abs() < 1e-7);
    assert!(
        (client_left.y..=client_left.y + number(client_body, "height")).contains(&client_tip.y)
    );
    check_both_arrows(outer_route, junction, client_tip);
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
