#![cfg(feature = "svg")]

use merman::svg::{LayoutOptions, RenderResourcePolicy, SvgRenderOptions};
use merman::{OperationControl, ParseOptions, RenderOutput, RenderRequest, Renderer, SvgRequest};
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

fn svg_request(diagram_id: &str) -> SvgRequest {
    SvgRequest {
        layout: LayoutOptions::headless_svg_defaults(),
        options: SvgRenderOptions {
            diagram_id: Some(diagram_id.to_string()),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn try_render_swimlane(source: &str, diagram_id: &str) -> Result<String, String> {
    let output = Renderer::new()
        .render(
            RenderRequest::svg(source, OperationControl::new(), svg_request(diagram_id))
                .with_parse_options(ParseOptions::strict()),
        )
        .map_err(|error| format!("SVG render failed: {error}"))?;
    let RenderOutput::Svg(Some(svg)) = output else {
        return Err("no diagram detected".to_string());
    };
    Ok(svg.into_parts().0)
}

fn render_swimlane(source: &str, diagram_id: &str) -> String {
    try_render_swimlane(source, diagram_id).expect("swimlane SVG")
}

fn swimlane_title_uses_html(svg: &str) -> bool {
    let document = roxmltree::Document::parse(svg).expect("valid SVG XML");
    let title = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|part| part == "swimlane-label")
                })
        })
        .expect("swimlane title group");
    title
        .descendants()
        .any(|node| node.has_tag_name("foreignObject"))
}

fn swimlane_title_text(svg: &str) -> String {
    let document = roxmltree::Document::parse(svg).expect("valid SVG XML");
    let title = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|part| part == "swimlane-label")
                })
        })
        .expect("swimlane title group");
    title
        .descendants()
        .filter_map(|node| node.text().filter(|_| node.is_text()))
        .collect()
}

fn swimlane_edge_label_uses_html(svg: &str) -> bool {
    let document = roxmltree::Document::parse(svg).expect("valid SVG XML");
    let label = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .all(|part| matches!(part, "label" | "edgeLabel"))
                        && class.split_ascii_whitespace().count() == 2
                })
                && node
                    .attribute("id")
                    .is_some_and(|id| id.starts_with("edge-label-"))
        })
        .expect("swimlane edge label node");
    label
        .descendants()
        .any(|node| node.has_tag_name("foreignObject"))
}

#[test]
fn line_hop_work_budget_is_reported_by_the_typed_render_operation() {
    // The fixture's stable preflight estimate is 90 units. Probe the precise layout boundary so
    // this contract remains about operation-wide accounting rather than internal routing passes.
    let layout_boundary = (90..=256)
        .find(|&max_layout_work_units| {
            let resources = RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(
                    merman::svg::ResourceLimitId::MaxLayoutWorkUnits,
                    max_layout_work_units,
                )
                .unwrap();
            let request = SvgRequest {
                environment: merman::SvgEnvironment::deterministic()
                    .with_resource_policy(resources),
                ..svg_request("swimlane-budget-probe")
            };
            match Renderer::new().render(RenderRequest::layout_json(
                DOCS_BASIC,
                OperationControl::new(),
                request,
            )) {
                Ok(RenderOutput::LayoutJson(Some(_))) => true,
                Ok(RenderOutput::LayoutJson(None)) => panic!("expected swimlane diagram"),
                Ok(_) => panic!("unexpected target output"),
                Err(error) if error.to_string().contains("max_layout_work_units") => false,
                Err(error) => panic!("unexpected layout error: {error}"),
            }
        })
        .expect("layout must fit within the bounded probe range");
    let request = SvgRequest {
        environment: merman::SvgEnvironment::deterministic().with_resource_policy(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(
                    merman::svg::ResourceLimitId::MaxLayoutWorkUnits,
                    layout_boundary,
                )
                .unwrap(),
        ),
        ..Default::default()
    };
    let error = Renderer::new()
        .render(RenderRequest::svg(
            DOCS_BASIC,
            OperationControl::new(),
            request,
        ))
        .expect_err("line-hop segment inspection must consume the remaining operation budget");
    assert!(
        error.to_string().contains("max_layout_work_units"),
        "{error}"
    );

    let without_line_hops =
        format!("---\nconfig:\n  swimlane:\n    lineHops: false\n---\n{DOCS_BASIC}");
    let request = SvgRequest {
        environment: merman::SvgEnvironment::deterministic().with_resource_policy(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(
                    merman::svg::ResourceLimitId::MaxLayoutWorkUnits,
                    layout_boundary,
                )
                .unwrap(),
        ),
        ..Default::default()
    };
    Renderer::new()
        .render(RenderRequest::svg(
            &without_line_hops,
            OperationControl::new(),
            request,
        ))
        .expect("the same budget must admit rendering when line-hop inspection is disabled");
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
    let output = Renderer::new()
        .render(
            RenderRequest::layout_json(
                DOCS_BASIC,
                OperationControl::new(),
                svg_request("swimlane-layout"),
            )
            .with_parse_options(ParseOptions::strict()),
        )
        .expect("swimlane layout");
    let RenderOutput::LayoutJson(Some(layout)) = output else {
        panic!("swimlane diagram");
    };
    layout.layout()["layout"]["SwimlaneDiagram"].clone()
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
    let renderer = Renderer::new();
    let semantic = renderer
        .prepare_semantic(DOCS_BASIC, OperationControl::new())
        .expect("swimlane prepare")
        .expect("swimlane diagram");
    assert_eq!(semantic.metadata().diagram_type, "swimlane");
    assert_eq!(
        semantic.metadata().effective_config.get_str("layout"),
        Some("swimlane")
    );

    let layout_output = renderer
        .render(RenderRequest::layout_json(
            DOCS_BASIC,
            OperationControl::new(),
            svg_request("typed-swimlane"),
        ))
        .expect("swimlane layout JSON");
    let RenderOutput::LayoutJson(Some(layout_output)) = layout_output else {
        panic!("expected swimlane layout JSON");
    };
    assert!(layout_output.layout()["layout"]["SwimlaneDiagram"].is_object());

    let svg = render_swimlane(DOCS_BASIC, "typed-swimlane");

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
fn swimlane_titles_follow_the_flowchart_local_html_labels_setting() {
    let root_false_flowchart_true = render_swimlane(
        r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: true
---
swimlane-beta LR
subgraph Lane["&nbsp;Lane&nbsp;"]
  A[Node]
end
"#,
        "swimlane-local-html",
    );
    assert!(
        swimlane_title_uses_html(&root_false_flowchart_true),
        "flowchart.htmlLabels=true must select the upstream Swimlane HTML title path: {root_false_flowchart_true}"
    );
    assert_eq!(
        swimlane_title_text(&root_false_flowchart_true),
        "\u{00a0}Lane\u{00a0}"
    );

    let root_true_flowchart_false = render_swimlane(
        r#"---
config:
  htmlLabels: true
  flowchart:
    htmlLabels: false
---
swimlane-beta LR
subgraph Lane["&nbsp;Lane&nbsp;"]
  A[Node]
end
"#,
        "swimlane-local-svg",
    );
    assert!(
        !swimlane_title_uses_html(&root_true_flowchart_false),
        "flowchart.htmlLabels=false must select the upstream Swimlane SVG title path: {root_true_flowchart_false}"
    );
    assert_eq!(
        swimlane_title_text(&root_true_flowchart_false),
        "&nbsp;Lane&nbsp;"
    );
}

#[test]
fn swimlane_edge_label_nodes_follow_root_html_labels() {
    let svg = render_swimlane(
        r#"---
config:
  flowchart:
    htmlLabels: false
---
swimlane-beta LR
subgraph Lane
  A[Node]
end
A -->|"&nbsp;Edge&nbsp;"| B
"#,
        "swimlane-edge-label-mode",
    );

    assert!(
        swimlane_edge_label_uses_html(&svg),
        "labelRect must follow root htmlLabels (default true), not the Flowchart fallback: {svg}"
    );
}

#[test]
fn swimlane_markdown_edge_labels_render_as_plain_label_rect_text() {
    let svg = render_swimlane(
        r#"swimlane-beta LR
A -->|`This is **bold**`| B
"#,
        "swimlane-edge-label-markdown-conversion",
    );
    let document = roxmltree::Document::parse(&svg).expect("valid SVG XML");
    let label = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node
                    .attribute("id")
                    .is_some_and(|id| id.starts_with("edge-label-"))
        })
        .expect("swimlane edge label node");
    let text = label
        .descendants()
        .filter_map(|node| node.text().filter(|_| node.is_text()))
        .collect::<String>();

    assert!(text.contains("**bold**"), "{svg}");
    assert!(
        !label
            .descendants()
            .any(|node| node.has_tag_name("strong") || node.has_tag_name("em")),
        "the converted labelRect must not inherit the original edge Markdown type: {svg}"
    );
}

#[test]
fn docs_basic_geometry_preserves_swimlane_topology() {
    let layout = docs_basic_layout();
    let nodes = &layout["nodes"];

    let request = by_id(nodes, "request");
    let receive = by_id(nodes, "receive");
    let triage = by_id(nodes, "triage");
    let investigate = by_id(nodes, "investigate");
    let answer = by_id(nodes, "answer");
    let fix = by_id(nodes, "fix");

    assert_eq!(number(request, "x"), number(triage, "x"));
    assert_eq!(number(receive, "x"), number(answer, "x"));
    assert_eq!(number(receive, "x"), number(fix, "x"));
    assert!(number(request, "x") < number(investigate, "x"));
    assert!(number(investigate, "x") < number(receive, "x"));
    assert!(number(request, "y") < number(triage, "y"));
    assert!(number(triage, "y") < number(investigate, "y"));
    assert_eq!(number(investigate, "y"), number(fix, "y"));

    let known = by_id(nodes, "edge-label-triage-answer-L_triage_answer_0");
    assert_eq!(number(known, "y"), number(triage, "y"));
    assert!(number(triage, "x") < number(known, "x"));
    assert!(number(known, "x") < number(answer, "x"));

    let code_change = by_id(
        nodes,
        "edge-label-triage-investigate-L_triage_investigate_0",
    );
    assert_eq!(number(code_change, "y"), number(investigate, "y"));
    assert!(number(triage, "x") < number(code_change, "x"));
    assert!(number(code_change, "x") < number(investigate, "x"));

    let customer = by_id(&layout["lanes"], "Customer");
    let support = by_id(&layout["lanes"], "Support");
    let engineering = by_id(&layout["lanes"], "Engineering");
    assert!(number(customer, "y") < number(support, "y"));
    assert!(number(support, "y") < number(engineering, "y"));
    assert_eq!(number(customer, "width"), number(support, "width"));
    assert_eq!(number(support, "width"), number(engineering, "width"));
    assert!(number(engineering, "width").is_finite());
    assert!(number(engineering, "width") > 0.0);

    let bounds = &layout["bounds"];
    let min_x = number(bounds, "min_x");
    let max_x = number(bounds, "max_x");
    let min_y = number(bounds, "min_y");
    let max_y = number(bounds, "max_y");
    assert!(min_x.is_finite() && max_x.is_finite() && min_x < max_x);
    assert!(min_y.is_finite() && max_y.is_finite() && min_y < max_y);
    for node in nodes.as_array().expect("layout nodes") {
        let half_width = number(node, "width") / 2.0;
        let half_height = number(node, "height") / 2.0;
        assert!(number(node, "x") - half_width >= min_x);
        assert!(number(node, "x") + half_width <= max_x);
        assert!(number(node, "y") - half_height >= min_y);
        assert!(number(node, "y") + half_height <= max_y);
    }
}

#[test]
fn explicit_dagre_override_uses_the_flowchart_artifact() {
    let source = format!("---\nconfig:\n  layout: dagre\n---\n{}", DOCS_BASIC);
    let semantic = Renderer::new()
        .prepare_semantic(&source, OperationControl::new())
        .expect("dagre override prepare")
        .expect("swimlane diagram with dagre override");
    assert_eq!(semantic.metadata().diagram_type, "swimlane");
    assert_eq!(
        semantic.metadata().effective_config.get_str("layout"),
        Some("dagre")
    );
    let svg = render_swimlane(&source, "swimlane-dagre");
    assert!(!svg.contains("swimlane-title"), "{svg}");
    assert!(!svg.contains("swimlane-body"), "{svg}");
}

#[test]
fn loose_nodes_render_the_synthetic_default_lane() {
    let svg = render_swimlane(
        "swimlane-beta LR\nA[Start] --> B[Done]\n",
        "swimlane-default-lane",
    );

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
    let semantic = Renderer::new()
        .prepare_semantic(source, OperationControl::new())
        .expect("flowchart swimlane-layout prepare")
        .expect("flowchart swimlane-layout diagram");
    assert_eq!(semantic.metadata().diagram_type, "flowchart-v2");
    assert_eq!(
        semantic.metadata().effective_config.get_str("layout"),
        Some("swimlane")
    );
    let svg = render_swimlane(source, "flowchart-swimlane-layout");

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
    let semantic = Renderer::new()
        .prepare_semantic(&source, OperationControl::new())
        .expect("ELK override prepare")
        .expect("swimlane diagram with ELK override");
    assert_eq!(semantic.metadata().diagram_type, "swimlane");
    assert_eq!(
        semantic.metadata().effective_config.get_str("layout"),
        Some("elk")
    );
    let svg = render_swimlane(&source, "swimlane-elk");
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
