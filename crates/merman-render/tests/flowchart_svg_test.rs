use futures::executor::block_on;
mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::diagrams::flowchart::FlowchartModel;
use merman_core::{Engine, MermaidConfig, ParseOptions, ParsedDiagramRender, RenderSemanticModel};
use merman_render::LayoutOptions;
use merman_render::environment::{
    MeasurementProfileId, RenderEnvironment, RenderSession, TextMeasurementPolicy,
    TextMeasurementProfile, TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::model::FlowchartLayout;
use merman_render::resources::RenderResourcePolicy;
use merman_render::svg::{FlowchartEdgeTraceCollector, SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{
    TextMeasurer, TextMetrics, TextStyle, VendoredFontMetricsTextMeasurer, WrapMode,
};
use std::path::PathBuf;
#[cfg(feature = "math")]
use std::sync::Arc;

fn environment_with_measurer<M>(name: &str, measurer: M) -> RenderEnvironment
where
    M: TextMeasurer + Send + Sync + 'static,
{
    let identity = TextMeasurementProfileIdentity::new(
        MeasurementProfileId::new(name).expect("valid test profile id"),
        "test",
    )
    .expect("valid test profile identity");
    RenderEnvironment::deterministic().with_text_measurement_policy(TextMeasurementPolicy::uniform(
        TextMeasurementProfile::new(identity, std::sync::Arc::new(measurer)),
    ))
}

fn flowchart_model(parsed: &ParsedDiagramRender) -> &FlowchartModel {
    let RenderSemanticModel::Flowchart(model) = parsed.model() else {
        panic!("expected Flowchart render model");
    };
    model
}

fn layout_flowchart_render_model(
    parsed: ParsedDiagramRender,
    options: &LayoutOptions,
    session: RenderSession,
) -> merman_render::Result<FlowchartLayout> {
    let artifact = family::prepare(parsed, options, session)?;
    let projection = artifact.layout_json()?;
    serde_json::from_value(projection["layout"]["FlowchartV2"].clone())
        .map_err(merman_render::Error::from)
}

fn render_flowchart_artifact(
    parsed: ParsedDiagramRender,
    layout_options: &LayoutOptions,
    session: RenderSession,
    svg_options: &SvgRenderOptions,
) -> merman_render::Result<String> {
    let artifact = family::prepare(parsed, layout_options, session)?;
    let rendered = artifact.render_svg(svg_options, &SvgDebugOptions::default())?;
    Ok(rendered.svg().to_owned())
}

fn render_flowchart_svg_from_text(text: &str) -> String {
    render_flowchart_svg_from_text_with_engine(Engine::new(), text)
}

fn render_flowchart_svg_from_text_with_engine(engine: Engine, text: &str) -> String {
    render_flowchart_svg_from_text_with_engine_and_policy(
        engine,
        text,
        RenderResourcePolicy::interactive(),
    )
}

fn render_flowchart_svg_from_text_with_engine_and_policy(
    engine: Engine,
    text: &str,
    resource_policy: RenderResourcePolicy,
) -> String {
    let session = merman_render::environment::RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .unwrap();
    let parsed = block_on(engine.parse_diagram_for_render_model(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");
    render_flowchart_artifact(
        parsed,
        &LayoutOptions::default(),
        session,
        &SvgRenderOptions::default(),
    )
    .expect("render svg")
}

#[test]
fn flowchart_edge_trace_stays_in_explicit_caller_owned_memory() {
    let source = "flowchart TD\nA --> B\n";
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("create deterministic session");
    let parsed =
        block_on(Engine::new().parse_diagram_for_render_model(source, ParseOptions::default()))
            .expect("parse succeeds")
            .expect("detects flowchart");
    let edge_id = flowchart_model(&parsed)
        .edges
        .first()
        .expect("fixture has an edge")
        .id
        .clone();
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare flowchart artifact");
    let collector = FlowchartEdgeTraceCollector::default();
    let debug =
        SvgDebugOptions::default().with_flowchart_edge_trace(edge_id.clone(), collector.clone());

    artifact
        .render_svg(&SvgRenderOptions::default(), &debug)
        .expect("render flowchart");

    let traces = collector.drain();
    assert_eq!(traces.len(), 1);
    assert_eq!(traces[0].edge_id, edge_id);
    assert!(!traces[0].base_points.is_empty());
    assert!(collector.snapshot().is_empty());
}

#[test]
fn flowchart_root_normalizes_diagram_id_before_scoping_accessibility_ids() {
    let source =
        "flowchart TD\naccTitle: Accessible title\naccDescr: Accessible description\nA-->B\n";
    let diagram_id = r#"flow&"root"#;
    let session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let parsed =
        block_on(Engine::new().parse_diagram_for_render_model(source, ParseOptions::default()))
            .expect("parse ok")
            .expect("diagram detected");
    let svg = render_flowchart_artifact(
        parsed,
        &LayoutOptions::default(),
        session,
        &SvgRenderOptions {
            diagram_id: Some(diagram_id.to_string()),
            ..SvgRenderOptions::default()
        },
    )
    .expect("render svg");

    assert!(svg.starts_with(r#"<svg id="flow-root""#));
    assert!(svg.contains(
        r#"aria-describedby="chart-desc-flow-root" aria-labelledby="chart-title-flow-root""#
    ));
    assert!(svg.contains(r#"<title id="chart-title-flow-root">Accessible title</title>"#));
    assert!(svg.contains(r#"<desc id="chart-desc-flow-root">Accessible description</desc>"#));
    assert!(!svg.contains(diagram_id));
}

#[test]
fn flowchart_html_labels_serialize_unknown_html_entities_as_well_formed_xml() {
    let svg = render_flowchart_svg_from_text("flowchart TD\nA[\"&#x41;\"]\n");

    roxmltree::Document::parse(&svg).expect("Flowchart SVG must be well-formed XML");
    assert!(svg.contains("<p>&amp;&amp;x41;</p>"), "{svg}");
    assert!(!svg.contains("&amp;&x41;"), "{svg}");
}

#[test]
fn flowchart_html_labels_trim_direct_nbsp_before_decoding_entities() {
    let nbsp = '\u{00A0}';
    let source = format!(
        r#"flowchart LR
EntityLead["&nbsp;A"] -- "A&nbsp;" --> EntityTail["A&nbsp;"]
DirectLead["{nbsp}D"] -- "{nbsp}" --> DirectOnly["{nbsp}"]
EntityOnly["&nbsp;"] -- "&nbsp;" --> EntityTarget
Internal["A{nbsp}B"] --> InternalTarget
MarkdownLead["`&nbsp;M`"] -- "`M<br>&nbsp;`" --> MarkdownTail["`A<br>&nbsp;`"]
"#
    );
    let svg = render_flowchart_svg_from_text(&source);
    let document = roxmltree::Document::parse(&svg).expect("valid Flowchart SVG");
    let text_content = |node: roxmltree::Node<'_, '_>| {
        node.descendants()
            .filter_map(|descendant| descendant.text().filter(|_| descendant.is_text()))
            .collect::<String>()
    };

    let node_labels = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("span")
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|part| part == "nodeLabel")
                })
        })
        .map(text_content)
        .collect::<Vec<_>>();
    let edge_labels = document
        .descendants()
        .filter(|node| node.has_tag_name("span") && node.attribute("class") == Some("edgeLabel"))
        .map(text_content)
        .collect::<Vec<_>>();

    for expected in [
        format!("{nbsp}A"),
        format!("A{nbsp}"),
        nbsp.to_string(),
        "D".to_string(),
        format!("A{nbsp}B"),
    ] {
        assert!(
            node_labels.contains(&expected),
            "missing {expected:?}: {svg}"
        );
    }
    for expected in [format!("A{nbsp}"), nbsp.to_string(), format!("M{nbsp}")] {
        assert!(
            edge_labels.contains(&expected),
            "missing {expected:?}: {svg}"
        );
    }
    assert!(!node_labels.contains(&format!("{nbsp}D")), "{svg}");
    assert!(
        svg.contains(&format!("<p>A<br />{nbsp}</p>")),
        "expected a visible NBSP-only trailing line: {svg}",
    );

    let pure_nbsp_node_labels = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("span")
                && text_content(*node) == nbsp.to_string()
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|part| part == "nodeLabel")
                })
        })
        .collect::<Vec<_>>();
    let pure_nbsp_edge_labels = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("span")
                && text_content(*node) == nbsp.to_string()
                && node.attribute("class") == Some("edgeLabel")
        })
        .collect::<Vec<_>>();
    assert!(
        pure_nbsp_node_labels.len() == 1,
        "only the entity-authored node label should remain: {svg}",
    );
    assert!(
        pure_nbsp_edge_labels.len() == 1,
        "only the entity-authored edge label should remain: {svg}",
    );
    for label in pure_nbsp_node_labels
        .into_iter()
        .chain(pure_nbsp_edge_labels)
    {
        let foreign_object = label
            .ancestors()
            .find(|ancestor| ancestor.has_tag_name("foreignObject"))
            .expect("NBSP label foreignObject");
        assert_ne!(foreign_object.attribute("width"), Some("0"), "{svg}");
        assert_ne!(foreign_object.attribute("height"), Some("0"), "{svg}");
    }
}

#[test]
fn flowchart_svg_labels_preserve_entity_spelling_when_html_labels_are_disabled() {
    let nbsp = '\u{00A0}';
    let nel = '\u{0085}';
    let source = format!(
        r##"%%{{init: {{"htmlLabels": false, "flowchart": {{"htmlLabels": false}}}}}}%%
flowchart LR
Direct["{nbsp}Direct{nbsp}"] --> Entity["&nbsp;Entity&nbsp;"]
Entity --> Hash["#nbsp;Hash#nbsp;"]
Numeric["#160;Numeric#160;"]
Amp["&amp;Amp&amp;"]
Less["&lt;Less&lt;"]
NestedLess["&amp;lt;Nested&amp;gt;"]
NestedAmp["&amp;amp;Nested&amp;amp;"]
NelOnly["{nel}"]
DirectOnly["{nbsp}"] --> EntityOnly["&nbsp;"]
EntityOnly -->|"&nbsp;Edge&nbsp;"| Tail
MarkdownSource -->|"`&nbsp;Edge markdown&nbsp;`"| MarkdownTarget
ShapeTextOnly@{{ label: "{nbsp}", labelType: "text", shape: "rect" }}
ShapeMarkdownOnly@{{ label: "{nbsp}", labelType: "markdown", shape: "rect" }}
subgraph SG["&nbsp;Group&nbsp;"]
  Child
end
"##
    );
    let svg = render_flowchart_svg_from_text(&source);
    let document = roxmltree::Document::parse(&svg).expect("valid Flowchart SVG");
    let text_content = |node: roxmltree::Node<'_, '_>| {
        node.descendants()
            .filter_map(|descendant| descendant.text().filter(|_| descendant.is_text()))
            .collect::<String>()
    };

    assert!(!svg.contains("<foreignObject "), "{svg}");

    let rendered_text = document
        .descendants()
        .filter(|node| node.has_tag_name("text"))
        .map(|node| {
            node.descendants()
                .filter_map(|descendant| descendant.text().filter(|_| descendant.is_text()))
                .collect::<String>()
        })
        .collect::<Vec<_>>();

    for expected in [
        "Direct",
        "&nbsp;Entity&nbsp;",
        "&nbsp;Hash&nbsp;",
        "&#160;Numeric&#160;",
        "&Amp&",
        "<Less<",
        "&lt;Nested&gt;",
        "&amp;Nested&amp;",
        "&nbsp;",
        "&nbsp;Edge&nbsp;",
        "&nbsp;Group&nbsp;",
    ] {
        assert!(
            rendered_text.iter().any(|text| text == expected),
            "missing literal SVG label {expected:?}: {rendered_text:?}\n{svg}",
        );
    }
    assert!(rendered_text.iter().any(|text| text == &nel.to_string()));

    assert!(
        rendered_text.iter().any(String::is_empty),
        "the direct-NBSP-only node must keep a zero-size empty SVG label: {svg}",
    );

    let markdown_edge = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("data-id") == Some("L_MarkdownSource_MarkdownTarget_0")
        })
        .expect("markdown edge label group");
    let markdown_rows = markdown_edge
        .descendants()
        .filter(|node| {
            node.has_tag_name("tspan")
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|part| part == "text-outer-tspan")
                })
        })
        .map(text_content)
        .collect::<Vec<_>>();
    assert_eq!(markdown_rows, ["&nbsp;Edge", "markdown&nbsp;"], "{svg}");

    let node_text = |id: &str| {
        let id_fragment = format!("-flowchart-{id}-");
        let node = document
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && node
                        .attribute("id")
                        .is_some_and(|value| value.contains(&id_fragment))
            })
            .unwrap_or_else(|| panic!("missing node {id}"));
        node.descendants()
            .filter_map(|descendant| descendant.text().filter(|_| descendant.is_text()))
            .collect::<String>()
    };
    assert_eq!(node_text("ShapeTextOnly"), "");
    assert_eq!(node_text("ShapeMarkdownOnly"), nbsp.to_string());
}

#[test]
fn flowchart_missing_icon_uses_mermaid_unknown_icon_at_requested_size() {
    let svg = render_flowchart_svg_from_text(
        "flowchart TD\nA@{ icon: \"missing:icon\", label: \"Missing\" }\n",
    );
    let unknown_icon = r#"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 80 80"><g><rect width="80" height="80" style="fill: #087ebf; stroke-width: 0px;"/><text transform="translate(21.16 64.67)" style="fill: #fff; font-family: ArialMT, Arial; font-size: 67.75px;"><tspan x="0" y="0">?</tspan></text></g></svg>"#;

    assert!(svg.contains(unknown_icon), "{svg}");
}

#[test]
fn flowchart_icon_shapes_without_icon_assets_render_source_defined_frames() {
    let svg = render_flowchart_svg_from_text(
        r#"flowchart LR
I@{ shape: icon, label: "Plain" }
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Flowchart SVG");

    let id_prefix = "merman-flowchart-I-";
    assert!(
        document.descendants().any(|node| {
            node.is_element()
                && node.tag_name().name() == "g"
                && node
                    .attribute("id")
                    .is_some_and(|id| id.starts_with(id_prefix))
                && node.attribute("class") == Some("icon-shape default")
        }),
        "missing source-defined icon frame: {svg}"
    );
    assert!(
        !svg.contains("<tspan x=\"0\" y=\"0\">?</tspan>"),
        "an absent icon asset is not an unknown registered icon: {svg}"
    );
}

#[test]
fn flowchart_icon_variants_render_their_source_defined_frames() {
    let svg = render_flowchart_svg_from_text(
        r##"flowchart LR
R@{ icon: "fa:bell", form: "rounded" }
C@{ icon: "fa:bell", form: "circle" }
style R fill:#ff99ff,stroke:#333333,stroke-width:4px
style C fill:#ff99ff,stroke:#333333,stroke-width:4px
"##,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Flowchart SVG");

    let node = |node_id: &str| {
        let id_prefix = format!("merman-flowchart-{node_id}-");
        document
            .descendants()
            .find(|node| {
                node.is_element()
                    && node.tag_name().name() == "g"
                    && node
                        .attribute("id")
                        .is_some_and(|id| id.starts_with(&id_prefix))
            })
            .unwrap_or_else(|| panic!("Flowchart node wrapper for {node_id}"))
    };

    let rounded = node("R");
    let rounded_frame = rounded
        .children()
        .find(|child| child.attribute("class") == Some("icon-shape2"))
        .expect("iconRounded must preserve Mermaid's icon-shape2 frame class");
    assert_eq!(rounded_frame.attribute("transform"), Some("translate(0,0)"));
    assert!(
        rounded_frame
            .descendants()
            .any(|child| child.attribute("fill") == Some("#ff99ff")),
        "iconRounded frame must use the node fill: {svg}"
    );

    let circle = node("C");
    let circle_frame = circle
        .children()
        .find(|child| {
            child.attribute("transform") == Some("translate(0,0)")
                && child
                    .descendants()
                    .any(|descendant| descendant.attribute("fill") == Some("#ff99ff"))
        })
        .expect("iconCircle must emit a centered RoughJS circle frame");
    assert_ne!(circle_frame.attribute("class"), Some("icon-shape2"));

    for icon in [rounded, circle] {
        assert!(
            icon.descendants().any(|child| {
                child
                    .attribute("style")
                    .is_some_and(|style| style == "color: #333333;")
            }),
            "icon color must use the node stroke: {svg}"
        );
    }
}

#[test]
fn flowchart_svg_renders_one_logical_self_loop_edge() {
    let svg = render_flowchart_svg_from_text("flowchart TB\nA -->|again| A\n");

    assert_eq!(svg.matches(r#"id="merman-L_A_A_0""#).count(), 1, "{svg}");
    assert!(svg.contains(r#"data-id="L_A_A_0""#), "{svg}");
    assert!(
        !svg.contains("cyclic-special"),
        "Dagre self-loop segments must not leak into the rendered SVG: {svg}"
    );
}

fn flowchart_svg_edge_data_points(
    svg: &str,
    edge_id: &str,
) -> Vec<merman_render::model::LayoutPoint> {
    use base64::Engine as _;

    let marker = format!(r#"data-id="{edge_id}""#);
    let marker_pos = svg
        .find(&marker)
        .unwrap_or_else(|| panic!("edge {edge_id}: {svg}"));
    let tag_start = svg[..marker_pos].rfind("<path").expect("edge path start");
    let tag_end = svg[marker_pos..]
        .find('>')
        .map(|offset| marker_pos + offset)
        .expect("edge path end");
    let tag = &svg[tag_start..=tag_end];
    let attr = r#"data-points=""#;
    let value_start = tag.find(attr).expect("data-points") + attr.len();
    let value_end = tag[value_start..]
        .find('"')
        .map(|offset| value_start + offset)
        .expect("data-points end");
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&tag[value_start..value_end])
        .expect("data-points base64");
    serde_json::from_slice(&bytes).expect("data-points JSON")
}

#[test]
fn flowchart_svg_intersects_compact_self_loop_with_rendered_shape() {
    let _session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let text = "flowchart TD\nA[box] --> A\n";
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram_for_render_model(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");
    let layout_options = LayoutOptions::default();
    let layout = layout_flowchart_render_model(
        parsed.clone(),
        &layout_options,
        RenderEnvironment::deterministic()
            .begin_session()
            .expect("begin layout session"),
    )
    .expect("layout ok");
    let node = layout
        .nodes
        .iter()
        .find(|node| node.id == "A")
        .expect("node A");
    let edge = layout.edges.first().expect("self-loop edge");
    assert_eq!(edge.points.len(), 4);

    let outer = &edge.points[1];
    let dx = outer.x - node.x;
    let dy = outer.y - node.y;
    let scale = (node.width / 2.0 / dx.abs()).min(node.height / 2.0 / dy.abs());
    let expected_x = node.x + dx * scale;
    let expected_y = node.y + dy * scale;
    assert!(
        (edge.points[0].x - expected_x).abs() > 1e-3
            || (edge.points[0].y - expected_y).abs() > 1e-3,
        "the compact layout point should still be the provisional bbox endpoint"
    );

    let svg = render_flowchart_artifact(
        parsed,
        &layout_options,
        _session,
        &SvgRenderOptions::default(),
    )
    .expect("render svg");
    let points = flowchart_svg_edge_data_points(&svg, &edge.id);
    assert_eq!(points.len(), 4);
    assert!((points[0].x - expected_x).abs() <= 1e-3, "{points:?}");
    assert!((points[0].y - expected_y).abs() <= 1e-3, "{points:?}");
}

#[test]
fn flowchart_svg_renders_regular_edges_before_compact_self_loops() {
    let svg =
        render_flowchart_svg_from_text("flowchart TD\nA loop-edge@--> A\nA normal-edge@--> B\n");

    let normal = svg.find(r#"data-id="normal-edge""#).expect("normal edge");
    let self_loop = svg.find(r#"data-id="loop-edge""#).expect("self-loop edge");
    assert!(
        normal < self_loop,
        "regular edges must render before compact self-loops"
    );
}

#[test]
fn flowchart_svg_renders_explicit_direction_cluster_as_recursive_root() {
    let svg = render_flowchart_svg_from_text(
        "flowchart TB\nsubgraph A\n  direction LR\n  a --> b\nend\na --> c\n",
    );

    assert_eq!(
        svg.matches(r#"<g class="root""#).count(),
        2,
        "the extracted cluster should render as one nested root: {svg}"
    );
    assert_eq!(
        svg.matches(r#"id="merman-A""#).count(),
        1,
        "the cluster should render exactly once inside its recursive root: {svg}"
    );
    assert_eq!(
        svg.matches(r#"id="merman-flowchart-a-"#).count(),
        1,
        "the extracted cluster's internal node should remain in the SVG DOM: {svg}"
    );
    assert_eq!(
        svg.matches(r#"id="merman-flowchart-b-"#).count(),
        1,
        "the extracted cluster's internal node should remain in the SVG DOM: {svg}"
    );
}

#[test]
fn flowchart_svg_renders_edge_to_ancestor_cluster_inside_that_root() {
    let svg = render_flowchart_svg_from_text(
        "flowchart LR\nsubgraph Outer\n  direction TB\n  subgraph Inner\n    direction LR\n    a --> b\n  end\n  b --> c\nend\nc --> Outer\n",
    );

    assert!(
        svg.contains(
            r#"<g class="root"><g class="clusters"/><g class="edgePaths"/><g class="edgeLabels"/><g class="nodes"><g class="root""#
        ),
        "the edge to an ancestor cluster must not be promoted into the top-level root: {svg}"
    );
    assert_eq!(
        svg.matches(r#"data-id="L_c_Outer_0""#).count(),
        2,
        "the ancestor edge should have one path and one label entry inside Outer: {svg}"
    );
}

#[test]
fn flowchart_svg_renders_recursive_cluster_self_loop_in_parent_root() {
    let svg = render_flowchart_svg_from_text(
        "flowchart TB\nsubgraph Outer\n  subgraph Inner\n    x\n  end\n  Inner --> Inner\nend\n",
    );

    let outer_cluster = svg.find(r#"id="merman-Outer""#).expect("Outer cluster");
    let self_loop = svg
        .find(r#"data-id="L_Inner_Inner_0""#)
        .expect("Inner self-loop");
    let inner_cluster = svg.find(r#"id="merman-Inner""#).expect("Inner cluster");
    assert!(
        outer_cluster < self_loop && self_loop < inner_cluster,
        "a recursive cluster self-loop should render in its parent root: {svg}"
    );
}

fn deep_flowchart_subgraph_chain(depth: usize) -> String {
    let mut input = String::from("flowchart TB\n");
    for level in 0..depth {
        input.push_str(&format!("subgraph S{level}\n"));
    }
    input.push_str("Leaf\n");
    for _ in 0..depth {
        input.push_str("end\n");
    }
    input
}

fn flowchart_svg_viewbox_values(svg: &str) -> [f64; 4] {
    let viewbox_start = svg.find(r#"viewBox=""#).expect("viewBox") + r#"viewBox=""#.len();
    let viewbox_end = svg[viewbox_start..].find('"').expect("viewBox end") + viewbox_start;
    let viewbox = &svg[viewbox_start..viewbox_end];
    let values = viewbox
        .split_whitespace()
        .map(|part| part.parse::<f64>().expect("viewBox number"))
        .collect::<Vec<_>>();
    assert_eq!(values.len(), 4, "expected four viewBox values: {viewbox}");
    [values[0], values[1], values[2], values[3]]
}

fn foreign_object_width_for_data_id(svg: &str, data_id: &str) -> f64 {
    let data_marker = format!(r#"<g class="label" data-id="{data_id}""#);
    let data_start = svg.find(&data_marker).expect("data-id marker");
    let width_marker = r#"<foreignObject width=""#;
    let width_start = svg[data_start..]
        .find(width_marker)
        .map(|idx| data_start + idx + width_marker.len())
        .expect("foreignObject width");
    let width_end = svg[width_start..]
        .find('"')
        .map(|idx| width_start + idx)
        .expect("foreignObject width end");
    svg[width_start..width_end]
        .parse::<f64>()
        .expect("foreignObject width number")
}

fn foreign_object_contract_for_text(svg: &str, text: &str) -> (f64, f64, String, String) {
    let document = roxmltree::Document::parse(svg).expect("valid Flowchart SVG");
    let paragraph = document
        .descendants()
        .find(|node| {
            node.is_element()
                && node.tag_name().name() == "p"
                && node.text().is_some_and(|value| value == text)
        })
        .unwrap_or_else(|| panic!("paragraph for {text:?}"));
    let foreign_object = paragraph
        .ancestors()
        .find(|node| node.is_element() && node.tag_name().name() == "foreignObject")
        .unwrap_or_else(|| panic!("foreignObject for {text:?}"));
    let div = paragraph
        .ancestors()
        .find(|node| node.is_element() && node.tag_name().name() == "div")
        .unwrap_or_else(|| panic!("div for {text:?}"));

    let width = foreign_object
        .attribute("width")
        .expect("foreignObject width")
        .parse::<f64>()
        .expect("finite foreignObject width");
    let height = foreign_object
        .attribute("height")
        .expect("foreignObject height")
        .parse::<f64>()
        .expect("finite foreignObject height");
    (
        width,
        height,
        foreign_object
            .attribute("style")
            .unwrap_or_default()
            .to_string(),
        div.attribute("style").unwrap_or_default().to_string(),
    )
}

fn flowchart_node_shape(svg: &str, node_id: &str) -> (String, Option<String>) {
    let document = roxmltree::Document::parse(svg).expect("valid Flowchart SVG");
    let id_prefix = format!("merman-flowchart-{node_id}-");
    let node = document
        .descendants()
        .find(|node| {
            node.is_element()
                && node.tag_name().name() == "g"
                && node
                    .attribute("id")
                    .is_some_and(|id| id.starts_with(&id_prefix))
        })
        .unwrap_or_else(|| panic!("Flowchart node wrapper for {node_id}"));
    let shape = node
        .children()
        .find(|child| {
            child.is_element()
                && child.attribute("class").is_some_and(|class| {
                    class
                        .split_whitespace()
                        .any(|part| part == "label-container")
                })
        })
        .unwrap_or_else(|| panic!("Flowchart shape for {node_id}"));

    (
        shape.tag_name().name().to_string(),
        shape.attribute("d").map(str::to_string),
    )
}

#[derive(Debug, Clone)]
struct WidthScaledTextMeasurer {
    inner: VendoredFontMetricsTextMeasurer,
    width_scale: f64,
}

impl WidthScaledTextMeasurer {
    fn new(width_scale: f64) -> Self {
        Self {
            inner: VendoredFontMetricsTextMeasurer::default(),
            width_scale,
        }
    }

    fn scale_width(&self, metrics: TextMetrics) -> TextMetrics {
        TextMetrics {
            width: metrics.width * self.width_scale,
            ..metrics
        }
    }
}

impl TextMeasurer for WidthScaledTextMeasurer {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.scale_width(self.inner.measure(text, style))
    }

    fn measure_wrapped(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> TextMetrics {
        self.scale_width(
            self.inner
                .measure_wrapped(text, style, max_width, wrap_mode),
        )
    }

    fn measure_wrapped_with_raw_width(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> (TextMetrics, Option<f64>) {
        let (metrics, raw_width) = self
            .inner
            .measure_wrapped_with_raw_width(text, style, max_width, wrap_mode);
        (
            self.scale_width(metrics),
            raw_width.map(|width| width * self.width_scale),
        )
    }
}

#[test]
fn flowchart_svg_security_level_controls_unsafe_click_href_rendering() {
    let strict = render_flowchart_svg_from_text(
        r#"%%{init: {"securityLevel": "strict"}}%%
flowchart TD
    A[Alpha] --> B[Beta]
    click A href "javascript:alert(1)" "tip" _blank
"#,
    );
    assert!(
        strict.contains(r#"<a transform=""#),
        "expected strict mode to keep Mermaid's anchor wrapper for a declared link: {strict}"
    );
    assert!(
        !strict.contains(r#"xlink:href="javascript:alert(1)""#),
        "expected strict mode to omit unsafe click href from SVG: {strict}"
    );
    assert!(
        !strict.contains(r#"xlink:href="about:blank""#),
        "expected Mermaid-compatible strict SVG to omit sanitized about:blank href: {strict}"
    );

    let loose = render_flowchart_svg_from_text_with_engine(
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "securityLevel": "loose"
        }))),
        r#"%%{init: {"securityLevel": "loose"}}%%
flowchart TD
    A[Alpha] --> B[Beta] --> C[Gamma]
    click A href "mailto:user@user.user" "mail" _blank
    click B href "notes://do-your-thing/id" "custom" _blank
    click C href "javascript:alert(1)" "script" _blank
"#,
    );
    assert!(
        loose.contains(r#"xlink:href="mailto:user@user.user""#),
        "expected loose mode to preserve Mermaid-renderable mailto links: {loose}"
    );
    assert!(
        loose.contains(r#"<a transform=""#),
        "expected loose mode to keep Mermaid's anchor wrappers for declared links: {loose}"
    );
    assert!(
        !loose.contains(r#"xlink:href="notes://do-your-thing/id""#)
            && !loose.contains(r#"xlink:href="javascript:alert(1)""#),
        "expected loose mode SVG sanitizer parity to omit unknown and script hrefs: {loose}"
    );
}

#[test]
fn flowchart_parse_for_render_model_handles_deep_subgraph_chain() {
    const DEPTH: usize = 1200;
    let text = deep_flowchart_subgraph_chain(DEPTH);

    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(&text, ParseOptions::strict())
        .expect("parse ok")
        .expect("diagram detected");

    assert_eq!(parsed.metadata().diagram_type, "flowchart-v2");
}

#[test]
fn flowchart_layout_handles_deep_subgraph_chain() {
    let session = merman_render::environment::RenderEnvironment::deterministic()
        .with_resource_policy(RenderResourcePolicy::unbounded_for_trusted_input())
        .begin_session()
        .unwrap();
    const DEPTH: usize = 1200;
    let text = deep_flowchart_subgraph_chain(DEPTH);
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram_for_render_model(&text, ParseOptions::strict()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout = layout_flowchart_render_model(parsed, &LayoutOptions::default(), session)
        .expect("layout ok");

    assert!(layout.nodes.iter().any(|node| node.id == "Leaf"));
    assert!(layout.clusters.iter().any(|cluster| cluster.id == "S0"));
}

#[test]
fn flowchart_svg_handles_deep_subgraph_chain() {
    const DEPTH: usize = 1200;
    let text = deep_flowchart_subgraph_chain(DEPTH);

    let svg = render_flowchart_svg_from_text_with_engine_and_policy(
        Engine::new(),
        &text,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    );

    assert!(svg.contains(r#"id="merman-flowchart-Leaf-"#));
    assert!(svg.contains(r#"id="merman-S0""#));
}

#[test]
fn flowchart_diagram_padding_zero_is_preserved() {
    let default = render_flowchart_svg_from_text(
        r#"flowchart TB
A
"#,
    );
    let zero = render_flowchart_svg_from_text(
        r#"%%{init: {"flowchart": {"diagramPadding": 0}}}%%
flowchart TB
A
"#,
    );

    let default_viewbox = flowchart_svg_viewbox_values(&default);
    let zero_viewbox = flowchart_svg_viewbox_values(&zero);

    assert!(
        (default_viewbox[2] - zero_viewbox[2] - 16.0).abs() < 1e-6,
        "default diagramPadding=8 should add 16px width over diagramPadding=0; default={default_viewbox:?}, zero={zero_viewbox:?}"
    );
    assert!(
        (default_viewbox[3] - zero_viewbox[3] - 16.0).abs() < 1e-6,
        "default diagramPadding=8 should add 16px height over diagramPadding=0; default={default_viewbox:?}, zero={zero_viewbox:?}"
    );
}

#[test]
fn flowchart_svg_uses_configured_look_for_subgraph_clusters() {
    let svg = render_flowchart_svg_from_text(
        r#"%%{init: {"look": "neo"}}%%
flowchart TB
subgraph Group
  A
end
"#,
    );

    assert!(
        svg.contains(r#"<g class="cluster" id="merman-Group" data-look="neo""#),
        "expected flowchart subgraph cluster to propagate configured look: {svg}"
    );
    assert!(
        !svg.contains(r#"data-look="classic""#),
        "configured flowchart look must not leave classic DOM attributes: {svg}"
    );
}

#[test]
fn flowchart_v2_fontawesome_edge_label_width_uses_nominal_icon_boundary() {
    // Mermaid 11.15 uses a clean 1.25em inline box for FontAwesome labels instead of
    // browser-specific per-icon advance drift.
    let mmd_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("flowchart")
        .join("upstream_flowchart_v2_icons_in_edge_labels_spec.mmd");
    let text = std::fs::read_to_string(&mmd_path).expect("read fixture .mmd");

    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram_for_render_model(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("begin render session");

    let layout = layout_flowchart_render_model(parsed, &LayoutOptions::default(), session)
        .expect("layout ok");

    let edge = layout
        .edges
        .iter()
        .find(|e| e.id == "L_C_F_0")
        .expect("edge L_C_F_0");
    let lbl = edge.label.as_ref().expect("edge label");
    assert_eq!(lbl.width, 49.03125);
    assert_eq!(lbl.height, 24.0);
}

#[test]
fn flowchart_wrapping_width_is_reflected_in_html_label_max_width_style() {
    let _session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let text = "%%{init: {\"flowchart\": {\"htmlLabels\": true, \"wrappingWidth\": 120}}}%%\nflowchart TB\nA[\"Hello\"]\n";
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram_for_render_model(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let svg = render_flowchart_artifact(
        parsed,
        &layout_options,
        _session,
        &SvgRenderOptions::default(),
    )
    .expect("render svg");
    assert!(
        svg.contains("max-width: 120px"),
        "expected flowchart.wrappingWidth=120 to affect html label max-width style"
    );
}

#[test]
fn flowchart_html_node_labels_wrap_at_mermaid_default_width() {
    let svg = render_flowchart_svg_from_text(
        r##"flowchart LR
    Security[Import / WebSurface / Data Egress Gates] --> PDF
"##,
    );

    assert!(
        svg.contains(
            r#"foreignObject width="200" height="48" style="overflow: visible;"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table; white-space: break-spaces; line-height: 1.5; max-width: 200px; text-align: center; width: 200px;""#
        ),
        "expected long flowchart HTML node label to wrap at Mermaid's default 200px width: {svg}"
    );
}

#[test]
fn flowchart_html_labels_allow_browser_font_fallback_overflow() {
    let _session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let text = r#"flowchart TD
    A[Start] --> B{Condition?}
    B -->|Yes| C[Execute]
    B -->|No| D[End]
    C --> D"#;
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram_for_render_model(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");
    let layout_options = LayoutOptions::default();
    let svg = render_flowchart_artifact(
        parsed,
        &layout_options,
        _session,
        &SvgRenderOptions::default(),
    )
    .expect("render svg");

    let contracts = ["Start", "Condition?", "Yes"].map(|text| {
        let contract = foreign_object_contract_for_text(&svg, text);
        assert!(contract.0.is_finite() && contract.0 > 0.0, "{text}");
        assert_eq!(contract.1, 24.0, "{text}");
        assert!(contract.2.contains("overflow: visible"), "{text}");
        assert!(contract.3.contains("white-space: nowrap"), "{text}");
        assert!(contract.3.contains("max-width: 200px"), "{text}");
        contract
    });
    assert!(contracts[1].0 > contracts[0].0 && contracts[0].0 > contracts[2].0);
}

#[test]
fn flowchart_layout_uses_host_text_measurer_for_font_widths() {
    let _session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let text = r#"flowchart TD
    A[Start] --> B{Condition?}
    B -->|Yes| C[Execute]"#;
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram_for_render_model(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let baseline_options = LayoutOptions::default();
    let wide_options = LayoutOptions::default();
    let wide_session = environment_with_measurer(
        "test.flowchart-width-scaled",
        WidthScaledTextMeasurer::new(1.35),
    )
    .begin_session()
    .unwrap();

    let baseline_layout =
        layout_flowchart_render_model(parsed.clone(), &baseline_options, _session)
            .expect("baseline layout ok");
    let wide_layout =
        layout_flowchart_render_model(parsed, &wide_options, wide_session).expect("wide layout ok");

    let baseline_condition = baseline_layout
        .nodes
        .iter()
        .find(|node| node.id == "B")
        .expect("baseline Condition? node");
    let wide_condition = wide_layout
        .nodes
        .iter()
        .find(|node| node.id == "B")
        .expect("wide Condition? node");
    assert!(
        wide_condition.width > baseline_condition.width * 1.15,
        "expected host-provided wider font metrics to affect flowchart label layout; baseline={}, wide={}",
        baseline_condition.width,
        wide_condition.width
    );
}

#[test]
fn flowchart_svg_honors_mermaid_11_15_numeric_stroke_width_theme() {
    let svg = render_flowchart_svg_from_text_with_engine(
        legacy_init_theme_compat_engine(),
        r##"%%{init: {"themeVariables": {"strokeWidth": 4, "lineColor": "#112233", "nodeBorder": "#445566"}}}%%
flowchart TB
    A --> B
"##,
    );

    assert!(
        svg.contains(
            r#"#merman .node rect,#merman .node circle,#merman .node ellipse,#merman .node polygon,#merman .node path{fill:#ECECFF;stroke:#445566;stroke-width:4px;}"#
        ),
        "expected numeric themeVariables.strokeWidth to drive Flowchart node stroke width CSS: {svg}"
    );
    assert!(
        svg.contains(r#"#merman .edgePath .path{stroke:#112233;stroke-width:4px;}"#),
        "expected numeric themeVariables.strokeWidth to drive Flowchart edge path stroke width CSS: {svg}"
    );
    assert!(
        svg.contains(r#"#merman .edge-thickness-normal{stroke-width:4px;}"#),
        "expected visible Flowchart edge class width to follow Mermaid 11.15 theme strokeWidth: {svg}"
    );
    assert!(
        svg.contains(
            r#"class="edge-thickness-normal edge-pattern-solid edge-thickness-normal edge-pattern-solid flowchart-link""#
        ),
        "expected the visible Flowchart edge path to carry the themed edge-thickness-normal class: {svg}"
    );
}

#[test]
fn flowchart_link_style_stroke_width_overrides_theme_default_edge_width() {
    let svg = render_flowchart_svg_from_text_with_engine(
        legacy_init_theme_compat_engine(),
        r##"%%{init: {"themeVariables": {"strokeWidth": 4, "lineColor": "#112233"}}}%%
flowchart TB
    A --> B
    linkStyle 0 stroke-width:7px,stroke:#abcdef
"##,
    );

    assert!(
        svg.contains(r#"#merman .edge-thickness-normal{stroke-width:4px;}"#),
        "expected themeVariables.strokeWidth to remain the default Flowchart edge width: {svg}"
    );

    let edge_start = svg.find(r#"id="merman-L_A_B_0""#).expect("edge path");
    let edge_end = svg[edge_start..].find("/>").expect("edge path end") + edge_start;
    let edge_chunk = &svg[edge_start..edge_end];

    assert!(
        edge_chunk.contains("stroke-width:7px"),
        "expected linkStyle stroke-width to stay on the visible Flowchart edge path: {edge_chunk}"
    );
    assert!(
        edge_chunk.contains("stroke:#abcdef"),
        "expected linkStyle stroke color to stay on the visible Flowchart edge path: {edge_chunk}"
    );
}

#[test]
fn flowchart_svg_honors_node_text_color_theme_variable() {
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "themeVariables": {
            "mainBkg": "#111827",
            "nodeTextColor": "#f8fafc",
            "textColor": "#fde68a"
        }
    })));
    let svg = render_flowchart_svg_from_text_with_engine(
        engine,
        r#"flowchart TD
    A[Dark Node] --> B[Other]
"#,
    );

    assert!(
        svg.contains(
            r##"#merman .label{font-family:"trebuchet ms",verdana,arial,sans-serif;color:#f8fafc;}"##
        ),
        "expected themeVariables.nodeTextColor to drive Flowchart label color CSS: {svg}"
    );
    assert!(
        svg.contains(r##"#merman .label text,#merman span{fill:#f8fafc;color:#f8fafc;}"##),
        "expected themeVariables.nodeTextColor to drive Flowchart label text fill CSS: {svg}"
    );
    assert!(
        svg.contains(
            r##"#merman{font-family:"trebuchet ms",verdana,arial,sans-serif;font-size:16px;fill:#fde68a;}"##
        ),
        "expected themeVariables.textColor to continue driving root SVG text fill CSS: {svg}"
    );
}

#[test]
fn flowchart_svg_dispatches_public_shape_aliases_without_rectangle_fallbacks() {
    let svg = render_flowchart_svg_from_text(
        r#"flowchart TB
R0@{ shape: rect, label: "same" }
R1@{ shape: proc, label: "same" }
R2@{ shape: process, label: "same" }
R3@{ shape: rectangle, label: "same" }
C0@{ shape: circle, label: "same" }
C1@{ shape: circ, label: "same" }
B@{ shape: bang, label: "same" }
D@{ shape: cloud, label: "same" }
"#,
    );

    for id in ["R0", "R1", "R2", "R3"] {
        assert_eq!(flowchart_node_shape(&svg, id).0, "rect", "{id}");
    }
    for id in ["C0", "C1"] {
        assert_eq!(flowchart_node_shape(&svg, id).0, "circle", "{id}");
    }

    let bang = flowchart_node_shape(&svg, "B");
    assert_eq!(bang.0, "path");
    assert_eq!(
        bang.1.as_deref().map(|path| path.matches('a').count()),
        Some(14),
        "bang must preserve Mermaid 11.16's fourteen relative arc segments"
    );

    let cloud = flowchart_node_shape(&svg, "D");
    assert_eq!(cloud.0, "path");
    assert_eq!(
        cloud.1.as_deref().map(|path| path.matches('a').count()),
        Some(10),
        "cloud must preserve Mermaid 11.16's ten relative arc segments"
    );
}

#[test]
fn flowchart_svg_uses_extended_theme_derived_secondary_color_overrides() {
    let svg = render_flowchart_svg_from_text_with_engine(
        legacy_init_theme_compat_engine(),
        r##"%%{init: {"theme": "redux", "themeVariables": {"primaryColor": "#123456"}}}%%
flowchart TD
    A[Redux Node] -- Edge Label --> B[Other]
"##,
    );

    assert!(
        svg.contains("fill:#ffffff;stroke:#28253D;stroke-width:2px;"),
        "expected Mermaid redux mainBkg default to remain the visible node fill: {svg}"
    );
    assert!(
        svg.contains(
            "#merman .edgeLabel{background-color:hsl(90, 65.3846153846%, 20.3921568627%);"
        ),
        "expected Mermaid redux primaryColor override to derive visible secondary edge-label color: {svg}"
    );
}

#[test]
fn flowchart_neo_ignores_removed_private_presentation_keys() {
    let baseline_engine =
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "theme": "redux",
            "look": "neo",
            "themeVariables": {"edgeLabelBackground": "#FFFFFF"},
            "flowchart": {"curve": "rounded"}
        })));
    let private_keys_engine =
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "theme": "redux",
            "look": "neo",
            "themeVariables": {"edgeLabelBackground": "#FFFFFF"},
            "flowchart": {
                "curve": "rounded",
                "edgeCornerRadius": 14,
                "edgeLabelPadding": 4,
                "compactEdgeCorners": true
            }
        })));
    let source = r##"flowchart TD
    A[Start] --> B{Condition?}
    B -->|Yes| C[Execute]
    B -->|No| D[End]
    C --> D
"##;
    let baseline = render_flowchart_svg_from_text_with_engine(baseline_engine, source);
    let with_private_keys = render_flowchart_svg_from_text_with_engine(private_keys_engine, source);

    assert_eq!(
        with_private_keys, baseline,
        "removed private Flowchart config keys must not alter SVG output"
    );
}

#[cfg(feature = "layout-elk")]
#[test]
fn flowchart_private_compact_edge_corners_key_does_not_change_elk_routes() {
    let text = r#"flowchart LR
    ABOVE[Above] --> TARGET([Target])
    MIDDLE[Middle] --> TARGET
    BELOW[Below] --> TARGET
"#;
    let baseline = render_flowchart_svg_from_text_with_engine(
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "layout": "elk",
            "look": "neo"
        }))),
        text,
    );
    let with_private_key = render_flowchart_svg_from_text_with_engine(
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "layout": "elk",
            "look": "neo",
            "flowchart": {"compactEdgeCorners": true}
        }))),
        text,
    );

    assert_eq!(
        with_private_key, baseline,
        "removed private compactEdgeCorners config must not alter ELK output"
    );
}

#[test]
fn flowchart_node_labels_use_root_html_labels_when_flowchart_html_labels_is_false() {
    let _session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let text =
        "%%{init: {\"flowchart\": {\"htmlLabels\": false}}}%%\nflowchart TB\nA[\"`**Node**`\"]\n";
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram_for_render_model(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let svg = render_flowchart_artifact(
        parsed,
        &layout_options,
        _session,
        &SvgRenderOptions::default(),
    )
    .expect("render svg");
    assert!(
        svg.contains("<foreignObject "),
        "expected node label to remain in the HTML label path: {svg}"
    );
    assert!(
        svg.contains(r#"class="nodeLabel markdown-node-label""#),
        "expected markdown node label class in HTML label path: {svg}"
    );
}

#[test]
fn flowchart_classic_hexagon_renders_polygon_container() {
    let _session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let text = "flowchart TB\nA{{\"`**Hex**`\"}}\n";
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram_for_render_model(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let svg = render_flowchart_artifact(
        parsed,
        &layout_options,
        _session,
        &SvgRenderOptions::default(),
    )
    .expect("render svg");
    assert!(
        svg.contains(r#"<polygon "#) && svg.contains(r#"class="label-container""#),
        "expected classic hexagon to render as a polygon label-container: {svg}"
    );
    assert!(
        !svg.contains(r#"<g class="basic label-container"><path "#),
        "expected classic hexagon not to use the hand-drawn RoughJS path branch: {svg}"
    );
}

#[test]
fn flowchart_no_label_special_shapes_render_outer_path_group() {
    let _session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let text = "flowchart TB\nA@{ shape: stop }\nB@{ shape: lightning-bolt }\nC@{ shape: crossed-circle }\n";
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram_for_render_model(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let svg = render_flowchart_artifact(
        parsed,
        &layout_options,
        _session,
        &SvgRenderOptions::default(),
    )
    .expect("render svg");
    assert!(
        svg.matches(r#"class="outer-path""#).count() >= 3,
        "expected no-label special shapes to expose Mermaid 11.15 outer-path groups: {svg}"
    );
}

#[test]
fn flowchart_hourglass_preserves_markdown_label_class_after_clearing_label() {
    let _session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let text = r#"flowchart TB
A@{ shape: hourglass, label: "Hourglass label" }
"#;
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram_for_render_model(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let svg = render_flowchart_artifact(
        parsed,
        &layout_options,
        _session,
        &SvgRenderOptions::default(),
    )
    .expect("render svg");

    assert!(
        svg.contains(r#"<span class="nodeLabel markdown-node-label"></span>"#),
        "expected Mermaid 11.15 hourglass to keep markdown label class on the empty label: {svg}"
    );
}

#[test]
fn flowchart_base_theme_renders_root_gradient() {
    let _session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let text = r##"%%{init: {"theme": "base", "themeVariables": {"primaryColor": "#BB2528", "primaryBorderColor": "#7C0000", "secondaryColor": "#006100"}}}%%
flowchart TB
A --> B
"##;
    let engine = legacy_init_theme_compat_engine();
    let parsed = block_on(engine.parse_diagram_for_render_model(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let svg = render_flowchart_artifact(
        parsed,
        &layout_options,
        _session,
        &SvgRenderOptions {
            diagram_id: Some("flowchart_theme_gradient".to_string()),
            ..SvgRenderOptions::default()
        },
    )
    .expect("render svg");

    assert!(
        svg.contains(r#"<linearGradient id="flowchart_theme_gradient-gradient" gradientUnits="objectBoundingBox" x1="0%" y1="0%" x2="100%" y2="0%">"#),
        "expected Mermaid 11.15 root gradient element: {svg}"
    );
    assert!(
        svg.contains(r##"<stop offset="0%" stop-color="#7C0000" stop-opacity="1"/>"##),
        "expected gradientStart to use primaryBorderColor: {svg}"
    );
    assert!(
        svg.contains(
            r#"<stop offset="100%" stop-color="hsl(120, 60%, 9.0196078431%)" stop-opacity="1"/>"#
        ),
        "expected gradientStop to use derived secondaryBorderColor: {svg}"
    );
}

#[test]
fn flowchart_note_shape_renders_note_label_class() {
    let _session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let text = r#"flowchart TB
A@{ shape: note, label: "Note" }
"#;
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram_for_render_model(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let svg = render_flowchart_artifact(
        parsed,
        &layout_options,
        _session,
        &SvgRenderOptions::default(),
    )
    .expect("render svg");

    assert!(
        svg.contains(r#"<g class="label noteLabel""#),
        "expected Mermaid 11.15 note labels to carry the noteLabel class: {svg}"
    );
}

#[test]
fn flowchart_svg_markdown_node_labels_wrap_when_html_labels_false() {
    let _session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let text = r#"%%{init: {"htmlLabels": false, "flowchart": {"wrappingWidth": 80}}}%%
flowchart TB
A["`**Alpha beta gamma delta epsilon zeta eta theta**`"]
"#;
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram_for_render_model(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let svg = render_flowchart_artifact(
        parsed,
        &layout_options,
        _session,
        &SvgRenderOptions::default(),
    )
    .expect("render svg");

    assert!(
        svg.matches(r#"class="row text-outer-tspan""#).count() > 1,
        "expected Mermaid 11.15 SVG markdown node labels to wrap into multiple rows: {svg}"
    );
}

#[test]
fn flowchart_svg_plain_subgraph_titles_do_not_wrap_when_html_labels_false() {
    let _session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let text = r#"%%{init: {"htmlLabels": false, "flowchart": {"htmlLabels": false}}}%%
flowchart TB
subgraph A[SupercalifragilisticexpialidociousSupercalifragilisticexpialidocious]
  x
end
"#;
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram_for_render_model(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let svg = render_flowchart_artifact(
        parsed,
        &layout_options,
        _session,
        &SvgRenderOptions::default(),
    )
    .expect("render svg");

    let cluster_start = svg.find(r#"<g class="cluster""#).expect("cluster");
    let cluster_label_start = svg[cluster_start..]
        .find(r#"<g class="cluster-label""#)
        .map(|idx| cluster_start + idx)
        .expect("cluster label");
    let cluster_label_end = svg[cluster_label_start..]
        .find(r#"</text>"#)
        .map(|idx| cluster_label_start + idx)
        .expect("cluster label text end");
    let cluster_label = &svg[cluster_label_start..cluster_label_end];

    assert_eq!(
        cluster_label.matches("text-outer-tspan").count(),
        1,
        "expected Mermaid 11.15 plain SVG subgraph titles to remain one unwrapped row: {cluster_label}"
    );
}

#[test]
fn flowchart_html_labels_treat_decoded_backslash_n_as_line_break() {
    let _session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let text = "%%{init: {\"flowchart\": {\"htmlLabels\": true}}}%%\nflowchart TB\nA[\"line1\\\\nline2\"]\n";
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram_for_render_model(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let svg = render_flowchart_artifact(
        parsed,
        &layout_options,
        _session,
        &SvgRenderOptions::default(),
    )
    .expect("render svg");
    assert!(
        svg.contains("<p>line1<br />line2</p>"),
        "expected Mermaid 11.15 nonMarkdownToHTML to treat decoded `\\\\n` as a line break: {svg}"
    );
    assert!(
        !svg.contains("line1\\nline2"),
        "expected output to not contain a literal backslash-n escape"
    );
}

#[test]
fn flowchart_html_single_image_label_uses_paragraph_wrapper() {
    let _session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let text = r#"flowchart TB
B[<img src='https://mermaid.js.org/mermaid-logo.svg'>]
"#;
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram_for_render_model(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let svg = render_flowchart_artifact(
        parsed,
        &layout_options,
        _session,
        &SvgRenderOptions::default(),
    )
    .expect("render svg");

    assert!(
        svg.contains(r#"<span class="nodeLabel"><p><img "#),
        "expected Mermaid 11.16 non-markdown image labels to keep the nonMarkdownToHTML paragraph wrapper: {svg}"
    );
}

#[test]
fn flowchart_html_single_image_edge_label_is_not_dropped_as_empty() {
    let text = r#"flowchart LR
A -->|"<img src='https://mermaid.js.org/mermaid-logo.svg'>"| B
"#;
    let svg = render_flowchart_svg_from_text(text);

    assert!(
        svg.contains(r#"<span class="edgeLabel"><p><img "#),
        "expected Mermaid 11.16 to keep image-only edge label DOM content: {svg}"
    );
}

#[test]
fn flowchart_svg_plain_labels_split_literal_backslash_n() {
    let text = r#"%%{init: {"htmlLabels": false, "flowchart": {"htmlLabels": false}}}%%
flowchart TB
A["line1\nline2"]
"#;
    let svg = render_flowchart_svg_from_text(text);

    assert_eq!(
        svg.matches("text-outer-tspan").count(),
        2,
        "expected one SVG tspan row per nonMarkdownToLines row: {svg}"
    );
    assert!(!svg.contains(r#"line1\nline2"#), "{svg}");
}

#[test]
fn flowchart_svg_plain_label_tokens_preserve_raw_tag_provenance() {
    let text = r#"%%{init: {"htmlLabels": false, "flowchart": {"htmlLabels": false, "wrappingWidth": 1000}}}%%
flowchart TB
Raw["<span class='foo bar'>X</span>"]
Encoded["&lt;span class='foo bar'&gt;X&lt;/span&gt;"]
Angle["&lt;Less&lt;"]
"#;
    let svg = render_flowchart_svg_from_text(text);
    let document = roxmltree::Document::parse(&svg).expect("valid svg");
    let words_for = |id: &str| {
        let id_fragment = format!("-flowchart-{id}-");
        let node = document
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && node
                        .attribute("id")
                        .is_some_and(|value| value.contains(&id_fragment))
            })
            .unwrap_or_else(|| panic!("missing node {id}: {svg}"));
        node.descendants()
            .filter(|node| {
                node.has_tag_name("tspan") && node.attribute("class") == Some("text-inner-tspan")
            })
            .map(|node| node.text().unwrap_or_default().to_string())
            .collect::<Vec<_>>()
    };

    assert_eq!(
        words_for("Raw"),
        ["<span class='foo bar'>", " X", " </span>"],
        "raw tag must remain three source words: {svg}"
    );
    assert_eq!(
        words_for("Encoded"),
        ["<span", " class='foo", " bar'>X</span>"],
        "entity-authored angle text must keep ordinary source spaces: {svg}"
    );
    assert_eq!(
        words_for("Angle"),
        ["<Less<"],
        "decoded angle text must remain one source word: {svg}"
    );
}

#[test]
fn flowchart_svg_break_only_edge_labels_preserve_create_text_rows() {
    let svg = render_flowchart_svg_from_text(
        r#"---
config:
  htmlLabels: false
---
flowchart LR
  A -->|<br><br>| B
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid SVG XML");
    let label = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("data-id") == Some("L_A_B_0")
                && node
                    .attribute("class")
                    .is_some_and(|class| class.split_ascii_whitespace().any(|part| part == "label"))
        })
        .expect("edge label group");
    let rows = label
        .descendants()
        .filter(|node| {
            node.has_tag_name("tspan")
                && node
                    .attribute("class")
                    .is_some_and(|class| class.split_ascii_whitespace().any(|part| part == "row"))
        })
        .count();

    assert_eq!(rows, 3, "{svg}");
}

#[test]
fn flowchart_image_shape_label_bbox_includes_mermaid_padding() {
    let _session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let text = r#"flowchart TD
A@{ img: "https://mermaid.js.org/favicon.svg", label: "My example image label", pos: "t", h: 60, constraint: "on" }
"#;
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram_for_render_model(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let layout = layout_flowchart_render_model(
        parsed.clone(),
        &layout_options,
        RenderEnvironment::deterministic()
            .begin_session()
            .expect("begin layout session"),
    )
    .expect("layout ok");

    let node = layout.nodes.iter().find(|n| n.id == "A").expect("node A");
    assert!(node.width.is_finite() && node.width > 60.0);
    assert!(node.height.is_finite() && node.height > 60.0);

    let svg = render_flowchart_artifact(
        parsed,
        &layout_options,
        _session,
        &SvgRenderOptions::default(),
    )
    .expect("render svg");

    let (label_width, label_height, foreign_object_style, _) =
        foreign_object_contract_for_text(&svg, "My example image label");
    assert_eq!(node.width, label_width);
    assert!(label_height > 16.0);
    assert!(foreign_object_style.contains("overflow: visible"));
    assert!(
        svg.contains(r#"<image href="https://mermaid.js.org/favicon.svg" width="60" height="60" preserveAspectRatio="none" transform="translate(-30,-12)"/>"#),
        "expected top image placement to use the padded label bbox: {svg}"
    );
}

#[test]
fn flowchart_shape_data_multiline_markdown_trims_trailing_block_newline() {
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("begin render session");
    let text = r#"flowchart TB
A@{
  label: |
    This is a
    multiline string
}
"#;
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram_for_render_model(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let svg = render_flowchart_artifact(
        parsed,
        &layout_options,
        session,
        &SvgRenderOptions::default(),
    )
    .expect("render svg");

    assert_eq!(
        svg.matches("<br").count(),
        1,
        "expected Mermaid 11.15 shapeData block labels to ignore the YAML trailing newline: {svg}"
    );
}

#[test]
fn flowchart_html_plain_multiline_labels_trim_source_indentation() {
    let _session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let text = "%%{init: {\"flowchart\": {\"htmlLabels\": true}}}%%\nflowchart TB\nA[\"\n  First\n      Second\n  \"]\n";
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram_for_render_model(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let svg = render_flowchart_artifact(
        parsed,
        &layout_options,
        _session,
        &SvgRenderOptions::default(),
    )
    .expect("render svg");
    assert!(
        svg.contains("<p>First<br />Second</p>"),
        "expected plain multiline HTML label to trim indentation: {svg}"
    );
    assert!(
        !svg.contains("<br />      Second"),
        "expected no source indentation after HTML line break"
    );
}

#[test]
fn flowchart_html_plain_node_labels_can_span_indented_lines() {
    let svg = render_flowchart_svg_from_text(
        "     flowchart TB
     foo[**Bold Foo**] --> bar
     bar[Multiline
     bar]",
    );

    assert!(
        svg.contains("<p>Multiline<br />bar</p>"),
        "expected indented multiline node label to render as an HTML line break: {svg}"
    );
    assert!(
        svg.contains("<p>**Bold Foo**</p>"),
        "expected plain flowchart labels to keep Markdown delimiters literal like Mermaid's nonMarkdownToHTML: {svg}"
    );
    assert!(
        !svg.contains("<strong>Bold Foo</strong>"),
        "plain flowchart text labels must not be treated as Markdown strings: {svg}"
    );
}

#[test]
fn flowchart_svg_plain_text_labels_do_not_apply_markdown_weight() {
    let _session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let text = r#"%%{init: {"htmlLabels": false}}%%
flowchart TB
foo[**Bold Foo**]
"#;
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram_for_render_model(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let svg = render_flowchart_artifact(
        parsed,
        &layout_options,
        _session,
        &SvgRenderOptions::default(),
    )
    .expect("render svg");

    assert!(
        svg.contains(">**Bold</tspan>"),
        "expected plain SVG text label to keep leading Markdown delimiter literal: {svg}"
    );
    assert!(
        svg.contains("> Foo**</tspan>"),
        "expected plain SVG text label to keep trailing Markdown delimiter literal: {svg}"
    );
    assert!(
        !svg.contains(r#"font-weight="bold""#),
        "plain SVG text labels must not apply Markdown strong styling: {svg}"
    );
}

#[test]
fn flowchart_html_plain_labels_treat_literal_backslash_n_as_line_breaks() {
    let _session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let text =
        "flowchart TB\nA[\"Remove trailing whitespace<br/>src.replace(/}\\s*\\n/g, '}\\n')\"]\n";
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram_for_render_model(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let svg = render_flowchart_artifact(
        parsed,
        &layout_options,
        _session,
        &SvgRenderOptions::default(),
    )
    .expect("render svg");

    assert!(
        svg.contains(
            "<p>Remove trailing whitespace<br />src.replace(/}\\s*<br />/g, '}<br />')</p>"
        ),
        "expected literal backslash-n sequences to match Mermaid nonMarkdownToHTML line breaks: {svg}"
    );
}

#[test]
fn flowchart_html_edge_labels_preserve_edge_order_with_empty_labels() {
    let _session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let text = "flowchart TB\nA -->|Get money| B\nB --> C\nC -->|One| D\n";
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram_for_render_model(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let svg = render_flowchart_artifact(
        parsed,
        &layout_options,
        _session,
        &SvgRenderOptions::default(),
    )
    .expect("render svg");
    let edge_labels_start = svg.find(r#"<g class="edgeLabels">"#).expect("edgeLabels");
    let nodes_start = svg[edge_labels_start..]
        .find(r#"<g class="nodes">"#)
        .map(|idx| edge_labels_start + idx)
        .expect("nodes after edgeLabels");
    let edge_labels = &svg[edge_labels_start..nodes_start];

    let ab = edge_labels.find(r#"data-id="L_A_B_0""#).expect("A-B label");
    let bc = edge_labels.find(r#"data-id="L_B_C_0""#).expect("B-C label");
    let cd = edge_labels.find(r#"data-id="L_C_D_0""#).expect("C-D label");

    assert!(
        ab < bc && bc < cd,
        "expected HTML edgeLabels to preserve graph edge order: {edge_labels}"
    );
}

#[test]
fn flowchart_html_edge_labels_use_non_markdown_paragraph_wrapper() {
    let _session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let text = "flowchart TB\nA -->|plain edge label| B\n";
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram_for_render_model(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let svg = render_flowchart_artifact(
        parsed,
        &layout_options,
        _session,
        &SvgRenderOptions::default(),
    )
    .expect("render svg");

    assert!(
        svg.contains(r#"<span class="edgeLabel"><p>plain edge label</p></span>"#),
        "expected plain HTML edge labels to use Mermaid nonMarkdownToHTML paragraph wrapper: {svg}"
    );
}

#[test]
fn flowchart_html_edge_label_svg_width_matches_layout_bbox() {
    let _session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let text = "flowchart TD\n    A[Start] --> B{Condition ?}\n    B -->|Yes| C[Execute]\n    B -->|No| D[End]\n    C --> D\n";
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram_for_render_model(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let layout = layout_flowchart_render_model(
        parsed.clone(),
        &layout_options,
        RenderEnvironment::deterministic()
            .begin_session()
            .expect("begin layout session"),
    )
    .expect("layout ok");

    let yes_label = layout
        .edges
        .iter()
        .find(|edge| edge.id == "L_B_C_0")
        .and_then(|edge| edge.label.as_ref())
        .expect("Yes edge label");
    let no_label = layout
        .edges
        .iter()
        .find(|edge| edge.id == "L_B_D_0")
        .and_then(|edge| edge.label.as_ref())
        .expect("No edge label");

    let svg = render_flowchart_artifact(
        parsed,
        &layout_options,
        _session,
        &SvgRenderOptions::default(),
    )
    .expect("render svg");

    assert!(
        svg.contains(r#"<span class="edgeLabel"><p>Yes</p></span>"#)
            && svg.contains(r#"<span class="edgeLabel"><p>No</p></span>"#),
        "expected issue #2 edge labels to render as HTML labels: {svg}"
    );
    assert_eq!(
        foreign_object_width_for_data_id(&svg, "L_B_C_0"),
        yes_label.width
    );
    assert_eq!(
        foreign_object_width_for_data_id(&svg, "L_B_D_0"),
        no_label.width
    );
}

#[test]
fn flowchart_nested_root_viewbox_includes_empty_subgraph_node() {
    let _session = RenderEnvironment::deterministic().begin_session().unwrap();
    let text = "flowchart LR\nsubgraph A\na -->b\nend\nsubgraph B\nb\nend\n";
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram_for_render_model(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let svg = render_flowchart_artifact(
        parsed,
        &layout_options,
        _session,
        &SvgRenderOptions {
            diagram_id: Some(
                "upstream_cypress_flowchart_v2_spec_57_handle_nested_subgraphs_with_outgoing_links_4_015"
                    .to_string(),
            ),
            ..Default::default()
        },
    )
    .expect("render svg");

    assert!(
        svg.contains(r#"viewBox="0 0 154.921875 364""#),
        "expected computed root viewBox to include top-level empty subgraph node: {svg}"
    );
    assert!(
        svg.contains(
            r#"<g class="node" id="upstream_cypress_flowchart_v2_spec_57_handle_nested_subgraphs_with_outgoing_links_4_015-B""#
        ),
        "expected empty subgraph node DOM id to be scoped by the diagram id: {svg}"
    );
}

#[test]
fn flowchart_empty_subgraph_node_applies_inline_style() {
    let _session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let text = "flowchart TD\nsubgraph Empty\nend\nclassDef hot fill:#0f0,color:#111\nclass Empty hot\nstyle Empty fill:#f00,stroke:#00f,color:#fff\n";
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram_for_render_model(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let svg = render_flowchart_artifact(
        parsed,
        &layout_options,
        _session,
        &SvgRenderOptions::default(),
    )
    .expect("render svg");

    assert!(
        svg.contains(r#"<g class="node hot" id="merman-Empty""#),
        "expected empty subgraph to render as a scoped node with its assigned class: {svg}"
    );
    assert!(
        svg.contains(r#"style="fill:#f00 !important;stroke:#00f !important""#),
        "expected empty subgraph inline shape style to be applied: {svg}"
    );
    assert!(
        svg.contains(r#"<span class="nodeLabel" style="color:#fff !important">"#),
        "expected empty subgraph inline label style to be applied: {svg}"
    );
}

#[test]
fn flowchart_crossed_circle_aliases_use_source_symmetric_root_bounds() {
    let _session = RenderEnvironment::deterministic().begin_session().unwrap();
    let text = r#"flowchart
 n0@{ shape: cross-circ, label: "cross-circ" }
 n1@{ shape: summary, label: "summary" }
 n2@{ shape: crossed-circle, label: "crossed-circle" }
"#;
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram_for_render_model(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let svg = render_flowchart_artifact(
        parsed,
        &layout_options,
        _session,
        &SvgRenderOptions {
            diagram_id: Some(
                "upstream_cypress_flowchart_shape_alias_spec_shape_alias_aliasset37_037"
                    .to_string(),
            ),
            ..Default::default()
        },
    )
    .expect("render svg");

    let viewbox_start = svg.find(r#"viewBox=""#).expect("viewBox") + r#"viewBox=""#.len();
    let viewbox_end = svg[viewbox_start..].find('"').expect("viewBox end") + viewbox_start;
    let viewbox = &svg[viewbox_start..viewbox_end];
    let values = viewbox
        .split_whitespace()
        .map(|part| part.parse::<f64>().expect("viewBox number"))
        .collect::<Vec<_>>();
    assert_eq!(values.len(), 4, "expected four viewBox values: {viewbox}");
    assert!(
        values[0] == 0.0 && values[1] == 0.0 && values[2] > 0.0 && values[3] == 76.0,
        "expected crossed-circle aliases to use the source-defined symmetric diameter: {svg}"
    );
}

#[test]
fn flowchart_label_styles_follow_mermaid_label_style_whitelist() {
    let _session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let text = r#"%%{init: {"flowchart": {"htmlLabels": true}}}%%
flowchart LR
A[Styled node] -->|Styled edge| B[Plain]
style A fill:#eee,stroke:#111,font-style:italic,text-decoration:underline,letter-spacing:1px,white-space:break-spaces,text-align:left,line-height:2
linkStyle 0 font-style:italic,text-decoration:underline,letter-spacing:1px,color:#123456
"#;
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram_for_render_model(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let svg = render_flowchart_artifact(
        parsed,
        &layout_options,
        _session,
        &SvgRenderOptions::default(),
    )
    .expect("render svg");

    assert!(
        svg.contains("font-style:italic !important"),
        "expected font-style to be routed to label styles: {svg}"
    );
    assert!(
        svg.contains("text-decoration:underline !important"),
        "expected text-decoration to be routed to label styles: {svg}"
    );
    assert!(
        svg.contains("letter-spacing:1px !important"),
        "expected letter-spacing to be routed to label styles: {svg}"
    );
    assert!(
        svg.contains("white-space:break-spaces !important"),
        "expected white-space to be preserved on the label span/group style: {svg}"
    );
    assert!(
        svg.contains(r#"style="fill:#eee !important;stroke:#111 !important""#),
        "expected shape styles to stay on the node shape: {svg}"
    );
    assert!(
        !svg.contains("fill:#eee !important;stroke:#111 !important;font-style"),
        "expected text-only styles not to be mixed into node shape style: {svg}"
    );
    assert!(
        svg.contains(r#"class="edgeLabel" style="font-style:italic !important;text-decoration:underline !important;letter-spacing:1px !important;color:#123456 !important""#),
        "expected edge label span to receive Mermaid label styles: {svg}"
    );
}

#[test]
fn flowchart_default_curve_renders_basis_edges_while_rounded_remains_available() {
    fn render_with_engine(engine: Engine, text: &str) -> String {
        let session = RenderEnvironment::deterministic()
            .begin_session()
            .expect("begin render session");
        let parsed = block_on(engine.parse_diagram_for_render_model(text, ParseOptions::default()))
            .expect("parse ok")
            .expect("diagram detected");

        let layout_options = LayoutOptions::default();
        render_flowchart_artifact(
            parsed,
            &layout_options,
            session,
            &SvgRenderOptions::default(),
        )
        .expect("render svg")
    }

    fn render(text: &str) -> String {
        render_with_engine(Engine::new(), text)
    }

    fn edge_path_d<'a>(svg: &'a str, edge_id: &str) -> &'a str {
        let id_attr = format!(r#"id="{edge_id}""#);
        let id_start = svg.find(&id_attr).expect("edge id");
        let path_start = svg[..id_start].rfind("<path ").expect("edge path start");
        let path_end = svg[id_start..].find("/>").expect("edge path end") + id_start;
        let path = &svg[path_start..path_end];
        let d_start = path.find(r#"d=""#).expect("edge path d") + r#"d=""#.len();
        let d_end = path[d_start..].find('"').expect("edge path d end") + d_start;
        &path[d_start..d_end]
    }

    let diagram = "flowchart LR\nA --> B\nA --> C\n";
    let basis_svg = render(diagram);
    let basis_d = edge_path_d(&basis_svg, "L_A_B_0");
    assert!(
        basis_d.contains('C'),
        "expected default flowchart curve to preserve smooth basis output in Mermaid 11.15: {basis_d}"
    );

    let rounded_svg = render(&format!(
        "%%{{init: {{\"flowchart\": {{\"curve\": \"rounded\"}}}}}}%%\n{diagram}"
    ));
    let rounded_d = edge_path_d(&rounded_svg, "L_A_B_0");
    assert!(
        rounded_d.contains('Q') && !rounded_d.contains('C'),
        "expected explicit flowchart.curve=rounded to render rounded corners: {rounded_d}"
    );
}

#[test]
fn flowchart_datastore_shape_renders_top_and_bottom_border_rect() {
    let _session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let text = r#"flowchart TB
D@{ shape: datastore, label: "Datastore" }
"#;
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram_for_render_model(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout_options = LayoutOptions::default();
    let svg = render_flowchart_artifact(
        parsed,
        &layout_options,
        _session,
        &SvgRenderOptions::default(),
    )
    .expect("render svg");

    let rect_start = svg
        .find(r#"<rect class="basic label-container""#)
        .expect("datastore rect");
    let rect_end = svg[rect_start..].find("/>").expect("rect end") + rect_start;
    let rect = &svg[rect_start..rect_end];
    let attr = |name: &str| {
        let needle = format!(r#"{name}=""#);
        let start = rect.find(&needle).expect("attribute") + needle.len();
        let end = rect[start..].find('"').expect("attribute end") + start;
        &rect[start..end]
    };
    let expected_dasharray = format!("{} {}", attr("width"), attr("height"));
    assert!(
        attr("stroke-dasharray") == expected_dasharray,
        "expected datastore rect to hide vertical borders with width/height stroke-dasharray: {svg}"
    );
    assert!(
        !rect.contains("<path"),
        "expected datastore to render as a dashed-border rect, not bow-tie path: {svg}"
    );
}

#[cfg(feature = "math")]
#[test]
fn flowchart_svg_renders_ratex_math_labels_end_to_end() {
    let text = r#"%%{init: {"flowchart": {"htmlLabels": true}}}%%
flowchart LR
A["$$x^2$$"] -->|$$x^2$$| B[Done]
"#;
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram_for_render_model(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let math_renderer = Arc::new(merman_render::math::RatexMathRenderer);
    let session = RenderEnvironment::deterministic()
        .with_math_renderer(math_renderer)
        .begin_session()
        .expect("begin render session");
    let svg = render_flowchart_artifact(
        parsed,
        &LayoutOptions::default(),
        session,
        &SvgRenderOptions::default(),
    )
    .expect("render svg");

    assert!(
        svg.contains(r#"width="0.97153em""#),
        "expected RaTeX inline SVG sizing in flowchart labels: {svg}"
    );
    assert!(
        svg.contains("<path"),
        "expected RaTeX glyph paths in flowchart SVG: {svg}"
    );
    assert!(
        !svg.contains("$$x^2$$"),
        "expected math source delimiters to be replaced by rendered SVG: {svg}"
    );
}

#[cfg(feature = "math")]
#[test]
fn flowchart_svg_renders_ratex_mixed_math_labels_end_to_end() {
    let text = r#"%%{init: {"flowchart": {"htmlLabels": true}}}%%
flowchart LR
A["value: $$x^2$$"] -->|"Solve: $$\sqrt{2+2}$$"| B[Done]
"#;
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram_for_render_model(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let math_renderer = Arc::new(merman_render::math::RatexMathRenderer);
    let session = RenderEnvironment::deterministic()
        .with_math_renderer(math_renderer)
        .begin_session()
        .expect("begin render session");
    let svg = render_flowchart_artifact(
        parsed,
        &LayoutOptions::default(),
        session,
        &SvgRenderOptions::default(),
    )
    .expect("render svg");

    assert!(
        svg.contains("value: ") && svg.contains("Solve: ") && svg.contains("<path"),
        "expected mixed prose/math labels to render as RaTeX HTML fragments: {svg}"
    );
    assert!(
        !svg.contains(r#"value: $$x^2$$"#) && !svg.contains(r#"Solve: $$\sqrt{2+2}$$"#),
        "expected mixed flowchart labels to replace source delimiters: {svg}"
    );
}

#[cfg(feature = "math")]
#[test]
fn flowchart_docs_math_fixture_renders_supported_ratex_formulas() {
    let mmd_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("flowchart")
        .join("upstream_docs_math_flowcharts_001.mmd");
    let text = std::fs::read_to_string(&mmd_path).expect("read fixture .mmd");
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram_for_render_model(&text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let math_renderer = Arc::new(merman_render::math::RatexMathRenderer);
    let session = RenderEnvironment::deterministic()
        .with_math_renderer(math_renderer)
        .begin_session()
        .expect("begin render session");
    let svg = render_flowchart_artifact(
        parsed,
        &LayoutOptions::default(),
        session,
        &SvgRenderOptions::default(),
    )
    .expect("render svg");

    let inline_formula_count = svg
        .matches(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 "#)
        .count();
    assert_eq!(
        inline_formula_count, 7,
        "expected every pure math label in the docs fixture to render through RaTeX: {svg}"
    );
    assert!(
        !svg.contains("$$"),
        "expected supported flowchart fixture formulas to replace source delimiters: {svg}"
    );
}
