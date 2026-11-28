#![cfg(all(feature = "diagram-architecture", feature = "layout-shoal"))]

use merman_core::{MermaidEngine, ParseOptions};
use merman_render::environment::RenderEnvironment;
use merman_render::theme::{BuiltinPreset, Color, NativeTheme, ThemeCompiler};
use merman_render::{LayoutOptions, family};

#[test]
fn shoal_architecture_explicit_transparent_canvas_omits_cyberpunk_textures() {
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(
            "---\nconfig:\n  layout: shoal\n---\narchitecture-beta\nservice app(server)[API]\n",
            ParseOptions::strict(),
        )
        .unwrap()
        .unwrap();
    let theme = ThemeCompiler::new()
        .compile_preset(BuiltinPreset::Cyberpunk, None)
        .unwrap();
    for transparent in [false, true] {
        let session = RenderEnvironment::deterministic()
            .begin_session_with_native_theme(NativeTheme::new(theme.clone()))
            .unwrap();
        let session = if transparent {
            session.with_canvas_background(Color::TRANSPARENT)
        } else {
            session
        };
        let rendered = family::prepare(parsed.presentation(), &LayoutOptions::default(), session)
            .unwrap()
            .render_svg(&Default::default(), &Default::default())
            .unwrap();
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        for texture in ["pattern", "radialGradient"] {
            assert_eq!(
                document.descendants().any(|node| {
                    node.has_tag_name(texture)
                        && node.ancestors().any(|ancestor| {
                            ancestor.attribute("class") == Some("merman-theme-canvas")
                        })
                }),
                !transparent,
                "transparent={transparent}, texture={texture}"
            );
        }
        assert!(
            document
                .descendants()
                .any(|node| node.attribute("data-id") == Some("app"))
        );
    }
}

#[test]
fn shoal_architecture_routes_around_an_occluded_group_entry() {
    use kurbo::Rect;
    use merman_render::model::ArchitectureDiagramLayout;

    let source = r#"---
config:
  layout: shoal
---
architecture-beta
  group org(cloud)[Enterprise Platform]
    group edge(server)[DMZ Edge] in org
      service cdn(internet)[Edge CDN] in edge
      service gw(server)[API Gateway] in edge
    group mesh(server)[Service Mesh] in org
      service auth(server)[AuthN] in mesh
      service pay(server)[Payments] in mesh
      junction j1 in mesh
      junction j2 in mesh
    group data(database)[Storage Tier] in org
      service pg(database)[Ledger DB] in data
      service redis(disk)[Cache] in data
  cdn:R -[HTTPS/3]-> L:gw
  gw:R -[mTLS]-> L:j1
  j1:T -[JWT]-> B:auth
  j1:R -[gRPC]-> L:pay
  j1:B -[sync]-> T:j2
  pay:B -[SQL]-> T:pg
  j2:R -[WAL]-> L:pg
  auth:R -[RESP3]-> L:redis
"#;
    let parsed = MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, ParseOptions::strict())
        .unwrap()
        .unwrap();
    let theme = ThemeCompiler::new()
        .compile_preset(BuiltinPreset::Blueprint, None)
        .unwrap();
    let session = RenderEnvironment::deterministic()
        .begin_session_with_native_theme(NativeTheme::new(theme))
        .unwrap();
    let prepared = family::prepare(
        parsed.presentation(),
        &LayoutOptions::headless_svg_defaults(),
        session,
    )
    .unwrap();
    let projection = prepared.layout_json().unwrap();
    let layout: ArchitectureDiagramLayout =
        serde_json::from_value(projection["layout"]["ArchitectureDiagram"].clone()).unwrap();
    let node = |id| layout.nodes.iter().find(|node| node.id == id).unwrap();
    let edge = |from, to| {
        layout
            .edges
            .iter()
            .find(|edge| edge.from == from && edge.to == to)
            .unwrap()
    };
    let data = node("data");
    let resp = edge("auth", "redis");
    let wal = edge("j2", "pg");
    let sql = edge("pay", "pg");
    let eps = 1e-7;
    let resp_entry = resp
        .points
        .iter()
        .position(|p| {
            (p.y - (data.y + data.height)).abs() < eps && data.x < p.x && p.x < data.x + data.width
        })
        .expect("the occluded connection enters Storage Tier from its open bottom face");
    let wal_entry = wal
        .points
        .iter()
        .position(|p| (p.x - data.x).abs() < eps && data.y < p.y && p.y < data.y + data.height)
        .expect("the direct WAL connection keeps its left-side entry");
    let segments = |points: &[merman_render::model::LayoutPoint]| {
        points
            .windows(2)
            .map(|p| Rect::from_points((p[0].x, p[0].y), (p[1].x, p[1].y)))
            .collect::<Vec<_>>()
    };
    let wal_legs = segments(&wal.points[wal_entry..]);
    for resp_leg in segments(&resp.points[resp_entry..]) {
        for &wal_leg in &wal_legs {
            assert!(
                !resp_leg.inflate(eps, eps).overlaps(wal_leg),
                "terminal paths touch in Storage Tier: {resp_leg:?} and {wal_leg:?}"
            );
        }
    }
    assert!((resp.points.last().unwrap().x - node("redis").x).abs() < eps);
    assert!(
        sql.points
            .iter()
            .all(|p| (p.x - sql.points[0].x).abs() < eps)
    );

    let svg = prepared
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
    let document = roxmltree::Document::parse(svg.svg()).unwrap();
    for id in ["auth", "pay", "redis"] {
        let service = document
            .descendants()
            .find(|element| {
                element.attribute("data-et") == Some("node")
                    && element.attribute("data-id") == Some(id)
            })
            .unwrap();
        let title = service
            .children()
            .find(|element| element.attribute("text-anchor") == Some("middle"))
            .unwrap();
        let transform: svgtypes::Transform = title.attribute("transform").unwrap().parse().unwrap();
        assert!(
            (transform.e - node(id).width / 2.0).abs() < eps,
            "{id} title is off center"
        );
        if id == "redis" {
            assert!((transform.f - node(id).height).abs() < eps);
        } else {
            assert!(
                transform.f < 0.0,
                "{id} title must leave its bottom stem clear"
            );
        }
    }
}
