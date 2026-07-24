#![cfg(feature = "svg")]

use merman::ParseOptions;
use merman::svg::{
    HeadlessRenderer, LayoutOptions, RenderFamilyKind, RenderResourcePolicy, SvgRenderOptions,
    prepare_render_sync,
};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;

const DOCS_BASIC: &str = include_str!("../../../fixtures/swimlane/upstream_docs_basic.mmd");
const CAR_SALES: &str = include_str!("../../../fixtures/swimlane/upstream_7_car_sales_constr.mmd");

#[derive(Debug, Clone, PartialEq, Eq)]
struct RenderedEdge {
    d: String,
    data_points: String,
    style: String,
}

fn try_render_swimlane(source: &str, diagram_id: &str) -> Result<String, String> {
    prepare_render_sync(
        &merman::Engine::new(),
        source,
        ParseOptions::strict(),
        &LayoutOptions::headless_svg_defaults(),
    )
    .map_err(|error| format!("prepare failed: {error}"))?
    .ok_or_else(|| "no diagram detected".to_string())?
    .render_svg(&SvgRenderOptions {
        diagram_id: Some(diagram_id.to_string()),
        ..SvgRenderOptions::default()
    })
    .map_err(|error| format!("SVG render failed: {error}"))
}

fn render_swimlane(source: &str, diagram_id: &str) -> String {
    try_render_swimlane(source, diagram_id).expect("swimlane SVG")
}

#[test]
fn line_hop_work_budget_is_reported_by_the_headless_render_operation() {
    let renderer = HeadlessRenderer::new().with_resource_policy(
        RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(merman::svg::ResourceLimitId::MaxLayoutWorkUnits, 1)
            .unwrap(),
    );

    let error = renderer
        .render_svg_sync(DOCS_BASIC)
        .expect_err("the second overlapping cross-edge segment pair must exceed the budget");
    assert!(
        error.to_string().contains("max_layout_work_units"),
        "{error}"
    );
}

fn rendered_edges(svg: &str) -> BTreeMap<String, RenderedEdge> {
    let document = roxmltree::Document::parse(svg).expect("valid SVG XML");
    document
        .descendants()
        .filter(|node| node.has_tag_name("path") && node.attribute("data-edge") == Some("true"))
        .map(|node| {
            let id = node.attribute("data-id").expect("edge data-id").to_string();
            (
                id,
                RenderedEdge {
                    d: node.attribute("d").expect("edge d").to_string(),
                    data_points: node
                        .attribute("data-points")
                        .expect("edge data-points")
                        .to_string(),
                    style: node.attribute("style").unwrap_or_default().to_string(),
                },
            )
        })
        .collect()
}

fn car_sales_with(line_hops: &str, look: Option<&str>) -> String {
    let (_, body) = CAR_SALES
        .split_once("---\nswimlane-beta")
        .expect("upstream fixture frontmatter");
    let look = look
        .map(|value| format!("  look: {value}\n"))
        .unwrap_or_default();
    format!(
        "---\nconfig:\n  layout: swimlane\n{look}  swimlane:\n    ignoreCrossLaneEdges: true\n    optimizeRanksByCrossings: true\n    lineHops: {line_hops}\n---\nswimlane-beta{body}"
    )
}

fn docs_basic_layout() -> Value {
    let prepared = prepare_render_sync(
        &merman::Engine::new(),
        DOCS_BASIC,
        ParseOptions::strict(),
        &LayoutOptions::headless_svg_defaults(),
    )
    .expect("swimlane prepare")
    .expect("swimlane diagram");
    prepared.layout_json().expect("swimlane layout JSON")["layout"]["SwimlaneDiagram"].clone()
}

fn assert_near(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 0.5,
        "expected {actual} to be within 0.5 of upstream {expected}"
    );
}

fn number(value: &Value, field: &str) -> f64 {
    value[field]
        .as_f64()
        .unwrap_or_else(|| panic!("missing numeric field {field}: {value}"))
}

fn by_id<'a>(values: &'a Value, id: &str) -> &'a Value {
    values
        .as_array()
        .expect("layout array")
        .iter()
        .find(|value| value["id"].as_str() == Some(id))
        .unwrap_or_else(|| panic!("missing layout id {id}"))
}

#[test]
fn default_swimlane_uses_the_typed_swimlane_artifact() {
    let prepared = prepare_render_sync(
        &merman::Engine::new(),
        DOCS_BASIC,
        ParseOptions::strict(),
        &LayoutOptions::headless_svg_defaults(),
    )
    .expect("swimlane prepare")
    .expect("swimlane diagram");

    assert_eq!(prepared.metadata().diagram_type, "swimlane");
    assert_eq!(prepared.family_kind(), RenderFamilyKind::Swimlane);
    assert_eq!(
        prepared.metadata().effective_config.get_str("layout"),
        Some("swimlane")
    );
    let layout_json = prepared.layout_json().expect("swimlane layout JSON");
    assert!(layout_json["layout"]["SwimlaneDiagram"].is_object());

    let svg = prepared
        .render_svg(&SvgRenderOptions {
            diagram_id: Some("typed-swimlane".to_string()),
            ..SvgRenderOptions::default()
        })
        .expect("swimlane SVG");

    assert!(svg.contains(r#"aria-roledescription="swimlane""#), "{svg}");
    assert_eq!(svg.matches(r#"class="cluster swimlane""#).count(), 3);
    assert_eq!(svg.matches(r#"class="swimlane-title""#).count(), 3);
    assert_eq!(svg.matches(r#"class="swimlane-body""#).count(), 3);
    assert!(svg.contains("rotate(-90)"), "{svg}");
    assert!(svg.contains("typed-swimlane_swimlane-pointEnd"), "{svg}");
    assert!(
        svg.contains("edge-label-triage-answer-L_triage_answer_0"),
        "{svg}"
    );
    assert!(svg.contains("Known issue"), "{svg}");
    assert!(
        svg.contains("edge-label-triage-investigate-L_triage_investigate_0"),
        "{svg}"
    );
    assert!(svg.contains("Needs code change"), "{svg}");
}

#[test]
fn docs_basic_geometry_matches_mermaid_11_16_swimlane_pipeline() {
    let layout = docs_basic_layout();
    let nodes = &layout["nodes"];

    let receive = by_id(nodes, "receive");
    let investigate = by_id(nodes, "investigate");
    assert_near(number(receive, "x"), 1158.2842718965264);
    assert_near(number(investigate, "x"), 598.6351692445817);

    let known = by_id(nodes, "edge-label-triage-answer-L_triage_answer_0");
    assert_near(number(known, "x"), 600.4116671982632);
    assert_near(number(known, "y"), 267.171875);

    let code_change = by_id(
        nodes,
        "edge-label-triage-investigate-L_triage_investigate_0",
    );
    assert_near(number(code_change, "x"), 272.70430337229084);
    assert_near(number(code_change, "y"), 537.625);

    let engineering = by_id(&layout["lanes"], "Engineering");
    assert_near(number(engineering, "width"), 1349.2452093965264);

    let bounds = &layout["bounds"];
    assert_near(number(bounds, "min_x"), -95.9453125);
    assert_near(number(bounds, "max_x"), 1253.2998968965264);
    assert_near(number(bounds, "min_y"), -63.0);
    assert_near(number(bounds, "max_y"), 600.625);
}

#[test]
fn explicit_dagre_override_uses_the_flowchart_artifact() {
    let source = format!("---\nconfig:\n  layout: dagre\n---\n{}", DOCS_BASIC);
    let prepared = prepare_render_sync(
        &merman::Engine::new(),
        &source,
        ParseOptions::strict(),
        &LayoutOptions::headless_svg_defaults(),
    )
    .expect("dagre override prepare")
    .expect("swimlane diagram with dagre override");

    assert_eq!(prepared.metadata().diagram_type, "swimlane");
    assert_eq!(prepared.family_kind(), RenderFamilyKind::Flowchart);
    assert_eq!(
        prepared.metadata().effective_config.get_str("layout"),
        Some("dagre")
    );
    let svg = prepared
        .render_svg(&SvgRenderOptions {
            diagram_id: Some("swimlane-dagre".to_string()),
            ..SvgRenderOptions::default()
        })
        .expect("dagre override SVG");
    assert!(!svg.contains("swimlane-title"), "{svg}");
    assert!(!svg.contains("swimlane-body"), "{svg}");
}

#[test]
fn loose_nodes_render_the_synthetic_default_lane() {
    let prepared = prepare_render_sync(
        &merman::Engine::new(),
        "swimlane-beta LR\nA[Start] --> B[Done]\n",
        ParseOptions::strict(),
        &LayoutOptions::headless_svg_defaults(),
    )
    .expect("default-lane prepare")
    .expect("default-lane diagram");

    let svg = prepared
        .render_svg(&SvgRenderOptions {
            diagram_id: Some("swimlane-default-lane".to_string()),
            ..SvgRenderOptions::default()
        })
        .expect("default-lane SVG");

    assert!(
        svg.contains(
            r#"<g class="cluster swimlane" id="__swimlane_default__" data-id="__swimlane_default__" data-et="cluster">"#
        ),
        "{svg}"
    );
    assert_eq!(svg.matches(r#"class="swimlane-title""#).count(), 1);
    assert_eq!(svg.matches(r#"class="swimlane-body""#).count(), 1);
    assert!(
        svg.contains(r#"<foreignObject width="0" height="0">"#),
        "{svg}"
    );
    assert!(svg.contains(r#"<g class="edges edgePath">"#), "{svg}");
}

#[test]
fn flowchart_with_swimlane_layout_uses_the_registered_layout_backend() {
    let source = r#"---
config:
  layout: swimlane
---
flowchart LR
  subgraph Team
    A[Start]
    B[Done]
  end
  A --> B
"#;
    let prepared = prepare_render_sync(
        &merman::Engine::new(),
        source,
        ParseOptions::strict(),
        &LayoutOptions::headless_svg_defaults(),
    )
    .expect("flowchart swimlane-layout prepare")
    .expect("flowchart swimlane-layout diagram");

    assert_eq!(prepared.metadata().diagram_type, "flowchart-v2");
    assert_eq!(prepared.family_kind(), RenderFamilyKind::Swimlane);
    let svg = prepared
        .render_svg(&SvgRenderOptions {
            diagram_id: Some("flowchart-swimlane-layout".to_string()),
            ..SvgRenderOptions::default()
        })
        .expect("flowchart swimlane-layout SVG");

    assert!(
        svg.contains(r#"aria-roledescription="flowchart-v2""#),
        "{svg}"
    );
    assert!(svg.contains(r#"class="cluster swimlane""#), "{svg}");
    assert!(
        svg.contains("flowchart-swimlane-layout_flowchart-v2-pointEnd"),
        "{svg}"
    );
}

#[test]
fn line_hops_use_rendered_endpoints_without_mutating_data_points() {
    let disabled = rendered_edges(&render_swimlane(
        &car_sales_with("false", None),
        "swimlane-line-hops-disabled",
    ));
    for repetition in 0..16 {
        let disabled_repeat = rendered_edges(&render_swimlane(
            &car_sales_with("false", None),
            &format!("swimlane-line-hops-disabled-repeat-{repetition}"),
        ));
        assert_eq!(
            disabled, disabled_repeat,
            "the source-ported Swimlane pipeline must be deterministic"
        );
    }
    let arc = rendered_edges(&render_swimlane(
        &car_sales_with("arc", None),
        "swimlane-line-hops-arc",
    ));

    let hopped: Vec<_> = arc
        .iter()
        .filter(|(_, edge)| edge.d.contains("A6,6 0 0"))
        .collect();
    assert!(
        !hopped.is_empty(),
        "the upstream car-sales fixture must retain at least one rendered crossing"
    );
    assert!(disabled.values().all(|edge| !edge.d.contains("A6,6 0 0")));
    assert_eq!(arc.len(), disabled.len());
    for (id, arc_edge) in &arc {
        assert_eq!(
            arc_edge.data_points, disabled[id].data_points,
            "line-hop post-processing must preserve insertEdge data-points for {id}"
        );
    }

    let gap = rendered_edges(&render_swimlane(
        &car_sales_with("gap", None),
        "swimlane-line-hops-gap",
    ));
    assert!(
        gap.values().any(|edge| edge.d.matches('M').count() > 1),
        "gap mode must split at least one crossing path"
    );
    assert!(gap.values().all(|edge| !edge.d.contains("A6,6 0 0")));
    for (id, gap_edge) in &gap {
        assert_eq!(gap_edge.data_points, disabled[id].data_points, "{id}");
    }
}

#[test]
fn line_hops_run_after_neo_and_hand_drawn_edge_construction() {
    let neo = rendered_edges(&render_swimlane(
        &car_sales_with("arc", Some("neo")),
        "swimlane-line-hops-neo",
    ));
    let neo_hopped = neo
        .values()
        .find(|edge| edge.d.contains("A6,6 0 0"))
        .expect("Neo crossing edge");
    assert!(
        neo_hopped.style.contains("stroke-dasharray: 0 "),
        "Neo marker masking must be recomputed from the hopped path: {neo_hopped:?}"
    );
    assert!(neo_hopped.style.contains("stroke-dashoffset: 0;"));

    let hand_drawn = rendered_edges(&render_swimlane(
        &car_sales_with("arc", Some("handDrawn")),
        "swimlane-line-hops-hand-drawn",
    ));
    let hand_drawn_hopped = hand_drawn
        .values()
        .find(|edge| edge.d.contains("A6,6 0 0"))
        .expect("handDrawn crossing edge");
    assert!(
        !hand_drawn_hopped.d.contains('C'),
        "upstream lineJump replaces the RoughJS path after edge construction: {hand_drawn_hopped:?}"
    );
}

#[cfg(feature = "layout-elk")]
#[test]
fn explicit_elk_override_uses_the_flowchart_artifact() {
    let source = format!("---\nconfig:\n  layout: elk\n---\n{}", DOCS_BASIC);
    let prepared = prepare_render_sync(
        &merman::Engine::new(),
        &source,
        ParseOptions::strict(),
        &LayoutOptions::headless_svg_defaults(),
    )
    .expect("ELK override prepare")
    .expect("swimlane diagram with ELK override");

    assert_eq!(prepared.metadata().diagram_type, "swimlane");
    assert_eq!(prepared.family_kind(), RenderFamilyKind::Flowchart);
    assert_eq!(
        prepared.metadata().effective_config.get_str("layout"),
        Some("elk")
    );
    let svg = prepared
        .render_svg(&SvgRenderOptions {
            diagram_id: Some("swimlane-elk".to_string()),
            ..SvgRenderOptions::default()
        })
        .expect("ELK override SVG");
    assert!(!svg.contains("swimlane-title"), "{svg}");
    assert!(!svg.contains("swimlane-body"), "{svg}");
}

#[test]
fn upstream_swimlane_ddlt_corpus_renders_valid_svg() {
    let corpus = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures/swimlane/_upstream_ddlt");
    let mut fixtures: Vec<_> = std::fs::read_dir(&corpus)
        .expect("read Swimlane DDLT corpus")
        .map(|entry| entry.expect("read corpus entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "mmd"))
        .collect();
    fixtures.sort();
    assert_eq!(fixtures.len(), 30, "Mermaid 11.16 ships 30 DDLT files");

    let mut failures = Vec::new();
    for (index, fixture) in fixtures.iter().enumerate() {
        let source = match std::fs::read_to_string(fixture) {
            Ok(source) => source,
            Err(error) => {
                failures.push(format!("{}: read failed: {error}", fixture.display()));
                continue;
            }
        };
        let svg = match std::panic::catch_unwind(|| {
            try_render_swimlane(&source, &format!("swimlane-corpus-{index}"))
        }) {
            Ok(Ok(svg)) => svg,
            Ok(Err(error)) => {
                failures.push(format!("{}: {error}", fixture.display()));
                continue;
            }
            Err(_) => {
                failures.push(format!("{}: render panicked", fixture.display()));
                continue;
            }
        };

        let document = match roxmltree::Document::parse(&svg) {
            Ok(document) => document,
            Err(error) => {
                failures.push(format!("{}: invalid SVG XML: {error}", fixture.display()));
                continue;
            }
        };
        let root = document.root_element();
        if !root.has_tag_name("svg") {
            failures.push(format!("{}: root is not svg", fixture.display()));
            continue;
        }
        let Some(view_box) = root.attribute("viewBox") else {
            failures.push(format!("{}: SVG has no viewBox", fixture.display()));
            continue;
        };
        let values = view_box
            .split_ascii_whitespace()
            .map(str::parse::<f64>)
            .collect::<Result<Vec<_>, _>>();
        match values {
            Ok(values)
                if values.len() == 4
                    && values.iter().all(|value| value.is_finite())
                    && values[2] > 0.0
                    && values[3] > 0.0 => {}
            _ => failures.push(format!(
                "{}: invalid finite positive viewBox {view_box:?}",
                fixture.display()
            )),
        }
    }

    assert!(
        failures.is_empty(),
        "Swimlane DDLT SVG failures:\n{}",
        failures.join("\n")
    );
}
