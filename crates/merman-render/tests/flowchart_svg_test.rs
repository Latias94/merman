use futures::executor::block_on;
mod common;

use common::legacy_init_theme_compat_engine;
use merman_core::diagrams::flowchart::FlowchartModel;
use merman_core::{
    DiagramFamilyId, Engine, MermaidConfig, ParseOptions, ParsedDiagramRender, RenderSemanticModel,
};
use merman_render::LayoutOptions;
use merman_render::diagram_theme::{
    CanvasPaint, DiagramTheme, DiagramThemeCompiler, DiagramThemeSpec, InsetsPx, Specified,
    ThemePortabilityRequirement, ThemeRule, ThemeRuleSet, ThemeStrokePatch, ThemeStylePatch,
    ThemeTarget, ThemeVariant,
};
use merman_render::environment::{
    MeasurementProfileId, RenderEnvironment, RenderSession, TextMeasurementPolicy,
    TextMeasurementProfile, TextMeasurementProfileIdentity,
};
use merman_render::family;
use merman_render::model::{FlowchartLayout, SwimlaneLayout};
use merman_render::resources::{
    RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
};
use merman_render::svg::{FlowchartEdgeTraceCollector, SvgDebugOptions, SvgRenderOptions};
use merman_render::text::{
    DeterministicTextMeasurer, TextMeasurer, TextMetrics, TextStyle, WrapMode,
};
use std::path::PathBuf;

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

fn assert_svg_ids_are_unique(svg: &str) {
    let document = roxmltree::Document::parse(svg).expect("valid Flowchart-family SVG");
    let mut ids = std::collections::BTreeSet::new();
    for id in document
        .descendants()
        .filter_map(|node| node.attribute("id"))
    {
        assert!(ids.insert(id), "duplicate SVG id `{id}`: {svg}");
    }
}

fn edge_label_padding_theme(padding: InsetsPx) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::EdgeLabel,
                ThemeStylePatch::default().with_padding(padding),
            ))),
        )
        .expect("compile edge-label padding theme")
}

fn edge_stroke_width_theme(width: f32) -> DiagramTheme {
    edge_stroke_width_theme_rule(width, false)
}

fn explicit_default_edge_stroke_width_theme(width: f32) -> DiagramTheme {
    edge_stroke_width_theme_rule(width, true)
}

fn edge_stroke_width_theme_rule(width: f32, explicit_default: bool) -> DiagramTheme {
    let rule = ThemeRule::new(
        ThemeTarget::Edge,
        ThemeStylePatch::default()
            .with_stroke_width(width)
            .expect("valid edge stroke width"),
    );
    let rule = if explicit_default {
        rule.with_variant(ThemeVariant::Default)
    } else {
        rule
    };
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule)))
        .expect("compile edge stroke-width theme")
}

fn node_label_fill_theme(family: DiagramFamilyId, color: &str) -> DiagramTheme {
    DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::NodeLabel,
                        ThemeStylePatch::default().with_fill(
                            CanvasPaint::solid(color).expect("valid Flowchart NodeLabel fill"),
                        ),
                    )
                    .for_family(family),
                ),
            ),
        )
        .expect("compile Flowchart NodeLabel fill theme")
}

fn prepare_flowchart_family_with_theme(
    source: &str,
    theme: &DiagramTheme,
) -> family::FamilyRenderArtifact {
    prepare_flowchart_family_with_theme_and_portability(
        source,
        theme,
        ThemePortabilityRequirement::RequirePortable,
    )
}

fn prepare_flowchart_family_with_theme_and_portability(
    source: &str,
    theme: &DiagramTheme,
    portability: ThemePortabilityRequirement,
) -> family::FamilyRenderArtifact {
    prepare_flowchart_family_with_theme_engine_and_portability(
        source,
        theme,
        Engine::new(),
        portability,
    )
}

fn prepare_flowchart_family_with_theme_engine_and_portability(
    source: &str,
    theme: &DiagramTheme,
    engine: Engine,
    portability: ThemePortabilityRequirement,
) -> family::FamilyRenderArtifact {
    let parsed = block_on(
        merman_render::__private::install_parse_compatibility(theme, engine)
            .parse_diagram_for_render_model(source, ParseOptions::default()),
    )
    .expect("parse themed Flowchart family")
    .expect("detect themed Flowchart family");
    let session = RenderEnvironment::deterministic()
        .with_theme_portability_requirement(portability)
        .begin_session_with_theme(theme)
        .expect("begin themed Flowchart session");
    family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare themed Flowchart family")
}

fn render_dagre_flowchart_svg_from_text(text: &str) -> String {
    render_flowchart_svg_from_text_with_engine(
        Engine::new().with_site_config(MermaidConfig::from_value(
            serde_json::json!({"layout": "dagre"}),
        )),
        text,
    )
}

#[test]
fn flowchart_elk_svg_keeps_upstream_flat_node_order_across_subgraphs() {
    for (source, expected) in [
        (
            include_str!(
                "../../../fixtures/flowchart/stress_flowchart_deeply_nested_clusters_019.mmd"
            ),
            [
                "l1a", "l1b", "l2a", "l2b", "l3a", "l3b", "l4a", "l4b", "outside", "outside2",
            ]
            .as_slice(),
        ),
        (
            include_str!(
                "../../../fixtures/flowchart/stress_flowchart_subgraph_dir_inherit_vs_local_016.mmd"
            ),
            [
                "t1", "b1", "t2", "b2", "outside1", "outside2", "outside3", "outside4",
            ]
            .as_slice(),
        ),
        (
            include_str!(
                "../../../fixtures/flowchart/upstream_cypress_flowchart_spec_30_possibility_to_style_text_color_of_nodes_and_subgraphs_as_wel_030.mmd"
            ),
            ["A", "B", "E", "C", "D"].as_slice(),
        ),
    ] {
        let engine = Engine::new().with_site_config(MermaidConfig::from_value(
            serde_json::json!({"layout": "elk"}),
        ));
        let svg = render_flowchart_svg_from_text_with_engine(engine, source);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let actual = document
            .descendants()
            .filter(|node| node.attribute("data-et") == Some("node"))
            .map(|node| node.attribute("data-id").expect("semantic node identity"))
            .collect::<Vec<_>>();
        assert_eq!(actual, expected, "ELK paint order for {source}");
    }
}

#[test]
fn flowchart_elk_ancestor_descendant_edges_keep_upstream_visible_routes() {
    let fixtures = [
        (
            include_str!(
                "../../../fixtures/flowchart/stress_flowchart_subgraph_title_margins_extreme_nested_030.mmd"
            ),
            include_str!(
                "../../../fixtures/upstream-svgs/flowchart/stress_flowchart_subgraph_title_margins_extreme_nested_030.svg"
            ),
            "L_c_Outer_0",
        ),
        (
            include_str!(
                "../../../fixtures/flowchart/upstream_cypress_flowchart_v2_spec_5064_should_render_when_subgraph_child_has_links_to_outside_node_044.mmd"
            ),
            include_str!(
                "../../../fixtures/upstream-svgs/flowchart/upstream_cypress_flowchart_v2_spec_5064_should_render_when_subgraph_child_has_links_to_outside_node_044.svg"
            ),
            "L_Sub_In_0",
        ),
        (
            include_str!(
                "../../../fixtures/flowchart/upstream_flowchart_v2_subgraph_child_links_outside_spec.mmd"
            ),
            include_str!(
                "../../../fixtures/upstream-svgs/flowchart/upstream_flowchart_v2_subgraph_child_links_outside_spec.svg"
            ),
            "L_Sub_In_0",
        ),
    ];
    for (source, upstream, edge_id) in fixtures {
        let svg = render_flowchart_svg_from_text_with_engine(
            Engine::new().with_site_config(MermaidConfig::from_value(
                serde_json::json!({"layout": "elk"}),
            )),
            source,
        );
        let endpoints = |svg: &str| {
            let document = roxmltree::Document::parse(svg).unwrap();
            let edge = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("path") && node.attribute("data-id") == Some(edge_id)
                })
                .expect("ancestor/descendant edge");
            svgtypes::PathParser::from(edge.attribute("d").unwrap())
                .map(|segment| match segment.unwrap() {
                    svgtypes::PathSegment::MoveTo { abs: true, x, y }
                    | svgtypes::PathSegment::LineTo { abs: true, x, y } => (x, y),
                    other => panic!("expected a visible straight route, got {other:?}"),
                })
                .collect::<Vec<_>>()
        };
        let actual = endpoints(&svg);
        let expected = endpoints(upstream);
        assert_eq!(actual.len(), 2, "edge {edge_id} must remain visible");
        assert_ne!(actual[0], actual[1]);
        assert_eq!(actual.len(), expected.len());
        for ((actual_x, actual_y), (expected_x, expected_y)) in actual.into_iter().zip(expected) {
            assert!((actual_x - expected_x).abs() < 0.001);
            assert!((actual_y - expected_y).abs() < 0.001);
        }
    }
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

fn try_render_flowchart_svg_with_resource_policy(
    text: &str,
    resource_policy: RenderResourcePolicy,
) -> merman_render::Result<String> {
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(resource_policy)
        .begin_session()
        .expect("begin Flowchart resource-bound session");
    let parsed =
        block_on(Engine::new().parse_diagram_for_render_model(text, ParseOptions::default()))
            .expect("parse Flowchart resource-bound fixture")
            .expect("detect Flowchart resource-bound fixture");
    render_flowchart_artifact(
        parsed,
        &LayoutOptions::default(),
        session,
        &SvgRenderOptions::default(),
    )
}

#[test]
fn flowchart_families_accept_exact_max_svg_bytes_and_reject_one_byte_less() {
    for (family, source) in [
        (
            "Flowchart",
            "flowchart TD\nA[Alpha] -->|advance| B[Beta]\nB --> C[Gamma]\n",
        ),
        (
            "Swimlane",
            "swimlane-beta LR\nA[Alpha] -->|advance| B[Beta]\nB --> C[Gamma]\n",
        ),
    ] {
        let baseline = try_render_flowchart_svg_with_resource_policy(
            source,
            RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .unwrap_or_else(|error| panic!("render the unbounded {family} baseline: {error}"));
        let exact_bytes = baseline.len();
        assert!(
            exact_bytes > 1,
            "{family} fixture must emit a non-empty SVG"
        );

        let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
            .unwrap_or_else(|error| panic!("valid exact {family} SVG byte ceiling: {error}"));
        let exact = try_render_flowchart_svg_with_resource_policy(source, exact_policy)
            .unwrap_or_else(|error| {
                panic!("exact {family} SVG byte ceiling must succeed: {error}")
            });
        assert_eq!(exact.as_bytes(), baseline.as_bytes(), "{family}");

        let below_exact = exact_bytes - 1;
        let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
            .unwrap_or_else(|error| panic!("valid below-exact {family} SVG ceiling: {error}"));
        let error = match try_render_flowchart_svg_with_resource_policy(source, below_policy) {
            Ok(_) => panic!("one byte below the {family} SVG size must fail"),
            Err(error) => error,
        };
        let merman_render::Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected {family} MaxSvgBytes rejection, got {error}");
        };
        assert_eq!(limit.cause, ResourceLimitCause::Ceiling, "{family}");
        assert_eq!(limit.phase, ResourceLimitPhase::SvgOutput, "{family}");
        assert_eq!(
            limit.limit,
            ResourceLimitId::MaxSvgBytes.as_str(),
            "{family}"
        );
        assert_eq!(limit.max, below_exact, "{family}");
        assert!(limit.actual > limit.max, "{family}");
        assert!(
            limit.explicit_overrides.iter().any(|resource_override| {
                resource_override.id == ResourceLimitId::MaxSvgBytes
                    && resource_override.value == below_exact
            }),
            "{family}"
        );
    }
}

#[test]
fn duplicate_subgraph_ids_render_one_cluster_with_the_first_title() {
    for source in [
        "flowchart TD\n  subgraph X[First title]\n    A\n  end\n  subgraph X[Second title]\n    B\n  end\n",
        "flowchart TD\n  subgraph X[First title]\n  end\n  subgraph X[Second title]\n    A\n  end\n",
        "flowchart TD\n  subgraph X[First title]\n    A\n  end\n  subgraph X[Second title]\n  end\n",
        concat!(
            "flowchart TD\n",
            "  subgraph X[First title]\n    A\n  end\n",
            "  subgraph X[\"&nbsp;Second title&nbsp;\"]\n    B\n  end\n",
        ),
        "---\nconfig:\n  htmlLabels: false\n  flowchart:\n    htmlLabels: false\n---\nflowchart TD\n  subgraph X[First title]\n    A\n  end\n  subgraph X[\"&nbsp;Second title\"]\n    B\n  end\n",
        "---\nconfig:\n  htmlLabels: false\n  flowchart:\n    htmlLabels: false\n---\nflowchart TD\n  subgraph X[\"&nbsp;First title\"]\n    A\n  end\n  subgraph X[Second title]\n    B\n  end\n",
    ] {
        let svg = render_flowchart_svg_from_text(source);
        let document = roxmltree::Document::parse(&svg).expect("valid Flowchart SVG");
        let clusters = document
            .descendants()
            .filter(|node| {
                node.has_tag_name("g")
                    && node.attribute("data-id") == Some("X")
                    && node.attribute("data-et") == Some("cluster")
                    && node.attribute("class").is_some_and(|class| {
                        class.split_ascii_whitespace().any(|part| part == "cluster")
                    })
            })
            .collect::<Vec<_>>();

        assert_eq!(clusters.len(), 1, "{svg}");
        let visible_text = clusters[0]
            .descendants()
            .filter_map(|node| node.text().filter(|_| node.is_text()))
            .collect::<String>();
        assert!(visible_text.contains("First title"), "{svg}");
        assert!(!visible_text.contains("Second title"), "{svg}");
    }
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
fn failed_flowchart_render_does_not_publish_staged_edge_trace() {
    let source = "flowchart TD\nA --> B\n";
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram_for_render_model(source, ParseOptions::default()))
        .expect("parse succeeds")
        .expect("detects flowchart");
    let edge_id = flowchart_model(&parsed)
        .edges
        .first()
        .expect("fixture has an edge")
        .id
        .clone();
    let collector = FlowchartEdgeTraceCollector::default();
    let debug =
        SvgDebugOptions::default().with_flowchart_edge_trace(edge_id.clone(), collector.clone());

    let successful_session = RenderEnvironment::deterministic()
        .with_resource_policy(RenderResourcePolicy::unbounded_for_trusted_input())
        .begin_session()
        .expect("create successful session");
    let successful_artifact =
        family::prepare(parsed, &LayoutOptions::default(), successful_session)
            .expect("prepare successful artifact");
    let successful_svg = successful_artifact
        .render_svg(&SvgRenderOptions::default(), &debug)
        .expect("render succeeds");
    let retained_trace = collector.snapshot();
    assert_eq!(retained_trace.len(), 1);

    let failing_policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(
            ResourceLimitId::MaxSvgBytes,
            successful_svg.svg().len().saturating_sub(1),
        )
        .expect("configure the N-1 SVG ceiling");
    let failing_session = RenderEnvironment::deterministic()
        .with_resource_policy(failing_policy)
        .begin_session()
        .expect("create failing session");
    let parsed =
        block_on(Engine::new().parse_diagram_for_render_model(source, ParseOptions::default()))
            .expect("parse succeeds")
            .expect("detects flowchart");
    let failing_artifact = family::prepare(parsed, &LayoutOptions::default(), failing_session)
        .expect("prepare failing artifact");
    assert!(
        failing_artifact
            .render_svg(&SvgRenderOptions::default(), &debug)
            .is_err(),
        "the N-1 SVG ceiling must reject the second render"
    );

    assert_eq!(
        collector.snapshot(),
        retained_trace,
        "a failed render must not publish staged trace records or erase prior successes"
    );
}

#[test]
fn public_flowchart_artifact_replays_diagram_id_terminal_during_emit() {
    let mut source = String::from(
        "flowchart TD\nclassDef hot fill:#123456,stroke:#654321,color:#fff\nN0:::hot --> N1\n",
    );
    for index in 0..256 {
        if index == 0 {
            continue;
        }
        source.push_str(&format!("N{index} --> N{}\n", index + 1));
    }
    let parsed =
        block_on(Engine::new().parse_diagram_for_render_model(&source, ParseOptions::default()))
            .expect("parse succeeds")
            .expect("detects Flowchart");
    let policy = RenderResourcePolicy::unbounded_for_trusted_input()
        .with_limit(ResourceLimitId::MaxSvgBytes, 1)
        .expect("valid SVG byte limit");
    let session = RenderEnvironment::deterministic()
        .with_resource_policy(policy)
        .begin_session()
        .expect("begin render session");
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session)
        .expect("prepare public Flowchart artifact");

    let error = match artifact.render_svg(
        &SvgRenderOptions {
            diagram_id: Some("terminal".to_string()),
            ..SvgRenderOptions::default()
        },
        &SvgDebugOptions::default(),
    ) {
        Ok(_) => panic!("diagram-ID projection must reject the public render path"),
        Err(error) => error,
    };

    let merman_render::Error::ResourceLimitExceeded(details) = error else {
        panic!("expected SVG byte rejection, got {error}");
    };
    assert_eq!(details.limit, ResourceLimitId::MaxSvgBytes.as_str());
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
    assert!(svg.contains(r#"aria-describedby="flow-root-merman-flowchart-document-a11y-description" aria-labelledby="flow-root-merman-flowchart-document-a11y-title""#));
    assert!(svg.contains(
        r#"<title id="flow-root-merman-flowchart-document-a11y-title">Accessible title</title>"#
    ));
    assert!(svg.contains(r#"<desc id="flow-root-merman-flowchart-document-a11y-description">Accessible description</desc>"#));
    assert!(!svg.contains(diagram_id));
}

#[test]
fn flowchart_accessibility_ids_close_for_title_only_and_description_only() {
    let render = |source: &str| {
        let session = RenderEnvironment::deterministic()
            .begin_session()
            .expect("begin Flowchart accessibility session");
        let parsed =
            block_on(Engine::new().parse_diagram_for_render_model(source, ParseOptions::default()))
                .expect("parse Flowchart accessibility fixture")
                .expect("detect Flowchart accessibility fixture");
        render_flowchart_artifact(
            parsed,
            &LayoutOptions::default(),
            session,
            &SvgRenderOptions {
                diagram_id: Some("aria.scope:v1".to_string()),
                ..SvgRenderOptions::default()
            },
        )
        .expect("render Flowchart accessibility fixture")
    };
    let scope = "aria-scope-v1-merman-flowchart-document";
    let title_id = format!("{scope}-a11y-title");
    let description_id = format!("{scope}-a11y-description");

    let title_svg = render("flowchart TD\naccTitle: Title only\nA --> B\n");
    let title_document = roxmltree::Document::parse(&title_svg).expect("valid title-only SVG");
    assert_eq!(
        title_document.root_element().attribute("aria-labelledby"),
        Some(title_id.as_str())
    );
    assert_eq!(
        title_document.root_element().attribute("aria-describedby"),
        None
    );
    assert_eq!(
        title_document
            .descendants()
            .find(|node| node.has_tag_name("title"))
            .and_then(|node| node.attribute("id")),
        Some(title_id.as_str())
    );

    let description_svg = render("flowchart TD\naccDescr: Description only\nA --> B\n");
    let description_document =
        roxmltree::Document::parse(&description_svg).expect("valid description-only SVG");
    assert_eq!(
        description_document
            .root_element()
            .attribute("aria-describedby"),
        Some(description_id.as_str())
    );
    assert_eq!(
        description_document
            .root_element()
            .attribute("aria-labelledby"),
        None
    );
    assert_eq!(
        description_document
            .descendants()
            .find(|node| node.has_tag_name("desc"))
            .and_then(|node| node.attribute("id")),
        Some(description_id.as_str())
    );
}

#[test]
fn flowchart_shadow_css_references_the_emitted_document_filters() {
    let source = "flowchart TD\nA --> B\n";
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "look": "neo",
        "themeVariables": {
            "dropShadow": "url(#drop-shadow) url(#drop-shadow-small)"
        }
    })));
    let session = RenderEnvironment::deterministic()
        .begin_session()
        .expect("begin Flowchart shadow-closure session");
    let parsed = block_on(engine.parse_diagram_for_render_model(source, ParseOptions::default()))
        .expect("parse Flowchart shadow-closure fixture")
        .expect("detect Flowchart shadow-closure fixture");
    let svg = render_flowchart_artifact(
        parsed,
        &LayoutOptions::default(),
        session,
        &SvgRenderOptions {
            diagram_id: Some("shadow.scope:v1".to_string()),
            ..SvgRenderOptions::default()
        },
    )
    .expect("render Flowchart shadow-closure fixture");

    let scope = "shadow-scope-v1-merman-flowchart-document";
    let drop_shadow = format!("{scope}-filter-drop-shadow");
    let drop_shadow_small = format!("{scope}-filter-drop-shadow-small");
    assert!(svg.contains(&format!(r#"id="{drop_shadow}""#)), "{svg}");
    assert!(
        svg.contains(&format!(r#"id="{drop_shadow_small}""#)),
        "{svg}"
    );
    assert!(
        svg.contains(&format!(
            "filter:url(#{drop_shadow}) url(#{drop_shadow_small});"
        )),
        "{svg}"
    );
    assert!(!svg.contains("url(#drop-shadow)"), "{svg}");
    assert!(!svg.contains("url(#drop-shadow-small)"), "{svg}");
}

#[test]
fn flowchart_edge_and_cluster_with_the_same_raw_id_use_distinct_document_ids() {
    let svg = render_flowchart_svg_from_text("flowchart TD\nsubgraph L_A_B_0\n  A --> B\nend\n");

    assert_svg_ids_are_unique(&svg);
    assert!(
        svg.contains(r#"data-id="L_A_B_0" data-et="cluster""#),
        "{svg}"
    );
    assert!(svg.contains(r#"data-et="edge" data-id="L_A_B_0""#), "{svg}");
}

#[cfg(feature = "layout-elk")]
#[test]
fn flowchart_elk_multiple_clusters_have_distinct_document_ids() {
    let svg = render_flowchart_svg_from_text(
        "---\nconfig:\n  layout: elk\n---\nflowchart TD\nsubgraph First\n  A\nend\nsubgraph Second\n  B\nend\n",
    );

    assert_svg_ids_are_unique(&svg);
    assert_eq!(svg.matches(r#"data-et="cluster""#).count(), 2, "{svg}");
    assert!(!svg.contains(r#"id="[object Object]""#), "{svg}");
}

#[test]
fn swimlane_a11y_and_lane_raw_ids_remain_in_disjoint_namespaces() {
    let source =
        "swimlane-beta LR\naccTitle: Accessible title\nsubgraph chart-title-flow-root\n  A\nend\n";
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let parsed =
        block_on(Engine::new().parse_diagram_for_render_model(source, ParseOptions::default()))
            .expect("parse ok")
            .expect("diagram detected");
    let svg = render_flowchart_artifact(
        parsed,
        &LayoutOptions::default(),
        session,
        &SvgRenderOptions {
            diagram_id: Some("flow-root".to_string()),
            ..SvgRenderOptions::default()
        },
    )
    .expect("render Swimlane SVG");

    assert_svg_ids_are_unique(&svg);
    assert!(
        svg.contains(r#"data-id="chart-title-flow-root" data-et="cluster""#),
        "{svg}"
    );
}

#[test]
fn swimlane_generated_edge_label_nodes_receive_document_owned_ids() {
    let svg = render_flowchart_svg_from_text(
        "swimlane-beta LR\nA labeled@-->|generated label owner| B\n",
    );

    assert_svg_ids_are_unique(&svg);
    assert!(
        svg.contains(r#"data-id="labeled" data-et="edge-label""#),
        "{svg}"
    );
}

#[test]
fn swimlane_title_foreign_object_reuses_layout_text_metrics() {
    let source = r#"swimlane-beta TB
subgraph Lane[Large Lane Title]
  A[Node]
end
classDef huge font-size:40px,font-family:Arial
class Lane huge
"#;
    let parsed =
        block_on(Engine::new().parse_diagram_for_render_model(source, ParseOptions::default()))
            .expect("parse Swimlane source")
            .expect("detect Swimlane diagram");
    let artifact = family::prepare(
        parsed,
        &LayoutOptions::default(),
        RenderEnvironment::deterministic()
            .begin_session()
            .expect("begin Swimlane session"),
    )
    .expect("prepare Swimlane artifact");
    let projection = artifact.layout_json().expect("project Swimlane layout");
    let layout: SwimlaneLayout =
        serde_json::from_value(projection["layout"]["SwimlaneDiagram"].clone())
            .expect("decode Swimlane layout");
    let lane = layout.lanes.first().expect("prepared Swimlane lane");
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("render Swimlane SVG");
    let document = roxmltree::Document::parse(rendered.svg()).expect("valid Swimlane SVG");
    let foreign_object = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("class").is_some_and(|classes| {
                    classes
                        .split_ascii_whitespace()
                        .any(|class| class == "swimlane-label")
                })
        })
        .and_then(|group| {
            group
                .children()
                .find(|node| node.has_tag_name("foreignObject"))
        })
        .expect("Swimlane title foreignObject");
    let width = foreign_object
        .attribute("width")
        .expect("title foreignObject width")
        .parse::<f64>()
        .expect("numeric title foreignObject width");
    let height = foreign_object
        .attribute("height")
        .expect("title foreignObject height")
        .parse::<f64>()
        .expect("numeric title foreignObject height");
    assert!((width - lane.title_label_width).abs() <= 1.0e-6);
    assert!((height - lane.title_label_height).abs() <= 1.0e-6);

    let title_rect = document
        .descendants()
        .find(|node| {
            node.has_tag_name("rect")
                && node.attribute("class").is_some_and(|classes| {
                    classes
                        .split_ascii_whitespace()
                        .any(|class| class == "swimlane-title")
                })
        })
        .expect("Swimlane title rect");
    let rendered_lane_width = title_rect
        .attribute("width")
        .expect("title rect width")
        .parse::<f64>()
        .expect("numeric title rect width");
    assert!(
        rendered_lane_width >= lane.title_label_width + lane.padding,
        "rendered lane must contain the prepared title: {}",
        rendered.svg()
    );

    let [viewbox_x, _, viewbox_width, _] = flowchart_svg_viewbox_values(rendered.svg());
    let title_x = title_rect
        .attribute("x")
        .expect("title rect x")
        .parse::<f64>()
        .expect("numeric title rect x");
    assert!(
        viewbox_x <= title_x && viewbox_x + viewbox_width >= title_x + rendered_lane_width,
        "viewBox must contain the rendered Swimlane title band: {}",
        rendered.svg()
    );
}

#[test]
fn flowchart_root_gradient_and_same_named_cluster_use_distinct_document_ids() {
    let svg = render_flowchart_svg_from_text_with_engine(
        legacy_init_theme_compat_engine(),
        r##"%%{init: {"theme": "base"}}%%
flowchart TD
subgraph gradient
  A
end
"##,
    );

    assert_svg_ids_are_unique(&svg);
    assert!(
        svg.contains(r#"data-id="gradient" data-et="cluster""#),
        "{svg}"
    );
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
    assert_eq!(
        markdown_rows.join(" "),
        "&nbsp;Edge markdown&nbsp;",
        "{svg}"
    );

    let node_text = |id: &str| {
        let node = document
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && node.attribute("data-id") == Some(id)
                    && node.attribute("data-et") == Some("node")
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

    assert!(
        document.descendants().any(|node| {
            node.is_element()
                && node.tag_name().name() == "g"
                && node.attribute("data-id") == Some("I")
                && node.attribute("data-et") == Some("node")
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
        document
            .descendants()
            .find(|node| {
                node.is_element()
                    && node.tag_name().name() == "g"
                    && node.attribute("data-id") == Some(node_id)
                    && node.attribute("data-et") == Some("node")
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
fn flowchart_hand_drawn_icon_frame_uses_hachure_geometry() {
    let svg = render_flowchart_svg_from_text(
        r##"%%{init: {"look": "handDrawn", "handDrawnSeed": 7}}%%
flowchart LR
I@{ icon: "fa:bell", form: "rounded", label: "Icon" }
"##,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid handDrawn icon SVG");
    let node = document
        .descendants()
        .find(|element| {
            element.has_tag_name("g")
                && element.attribute("data-id") == Some("I")
                && element.attribute("data-et") == Some("node")
        })
        .expect("handDrawn icon node");
    assert!(
        node.descendants().any(|element| {
            element.has_tag_name("path")
                && element.attribute("stroke-width") == Some("4")
                && element.attribute("stroke-dasharray") == Some("0 0")
                && !element.attribute("d").unwrap_or_default().is_empty()
        }),
        "handDrawn icon frame must emit a non-empty hachure path: {svg}"
    );
}

#[test]
fn flowchart_icon_and_image_labels_emit_the_typography_used_for_measurement() {
    let svg = render_flowchart_svg_from_text(
        r##"flowchart LR
I@{ icon: "missing:icon", label: "Icon" }
C@{ icon: "missing:icon", form: "circle", label: "Circle" }
S@{ icon: "missing:icon", form: "square", label: "Square" }
M@{ img: "https://mermaid.js.org/favicon.svg", label: "Image", h: 60, constraint: "on" }
classDef themed font-family:Arial,font-size:28px,font-weight:700,font-style:italic,letter-spacing:2px,color:#123456
class I,C,S,M themed
"##,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Flowchart SVG");

    for node_id in ["I", "C", "S", "M"] {
        let node = document
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && node.attribute("data-id") == Some(node_id)
                    && node.attribute("data-et") == Some("node")
            })
            .unwrap_or_else(|| panic!("missing Flowchart node {node_id}: {svg}"));
        let label = node
            .descendants()
            .find(|child| child.has_tag_name("g") && child.attribute("class") == Some("label"))
            .unwrap_or_else(|| panic!("missing label for Flowchart node {node_id}: {svg}"));
        let group_style = label.attribute("style").unwrap_or_default();
        for declaration in [
            "font-family:Arial !important",
            "font-size:28px !important",
            "font-weight:700 !important",
            "font-style:italic !important",
            "letter-spacing:2px !important",
            "color:#123456 !important",
        ] {
            assert!(
                group_style.contains(declaration),
                "node {node_id} label must emit {declaration}: {svg}"
            );
        }

        let span_style = label
            .descendants()
            .find(|child| child.has_tag_name("span"))
            .and_then(|span| span.attribute("style"))
            .unwrap_or_default();
        assert_eq!(span_style, group_style, "node {node_id}: {svg}");

        let label_height = label
            .descendants()
            .find(|child| child.has_tag_name("foreignObject"))
            .and_then(|foreign_object| foreign_object.attribute("height"))
            .and_then(|height| height.parse::<f64>().ok())
            .unwrap_or_else(|| panic!("missing label metrics for node {node_id}: {svg}"));
        assert!(
            label_height > 28.0,
            "node {node_id} must be measured with the emitted 28px font: {svg}"
        );
    }
}

#[test]
fn flowchart_svg_renders_one_logical_self_loop_edge() {
    let svg = render_flowchart_svg_from_text("flowchart TB\nA -->|again| A\n");

    assert_eq!(
        svg.matches(r#"id="merman-merman-flowchart-document-edge-0""#)
            .count(),
        1,
        "{svg}"
    );
    assert!(svg.contains(r#"data-id="L_A_A_0""#), "{svg}");
    assert!(
        !svg.contains("cyclic-special"),
        "Dagre self-loop segments must not leak into the rendered SVG: {svg}"
    );
}

#[test]
fn duplicate_flowchart_edge_ids_keep_occurrence_bound_geometry_styles_labels_and_markers() {
    let svg = render_flowchart_svg_from_text(
        r##"flowchart LR
X L_A_B_0@-->|first &amp; owner| Y
A -->|second &lt; owner| B
linkStyle 0 stroke:#ef4444
linkStyle 1 stroke:#2563eb
"##,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Flowchart SVG");
    let paths = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("path")
                && node.attribute("data-edge") == Some("true")
                && node.attribute("data-id") == Some("L_A_B_0")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        paths.len(),
        2,
        "both semantic occurrences must render: {svg}"
    );
    assert_duplicate_raw_edge_path_dom_identity(&paths, "L_A_B_0", &svg);
    assert_ne!(
        paths[0].attribute("d"),
        paths[1].attribute("d"),
        "duplicate raw ids must not alias routed geometry"
    );
    let styles = paths
        .iter()
        .filter_map(|path| path.attribute("style"))
        .collect::<Vec<_>>();
    assert!(styles.iter().any(|style| style.contains("stroke:#ef4444")));
    assert!(styles.iter().any(|style| style.contains("stroke:#2563eb")));
    let marker_ends = paths
        .iter()
        .filter_map(|path| path.attribute("marker-end"))
        .collect::<Vec<_>>();
    assert!(marker_ends.iter().any(|marker| marker.contains("__ef4444")));
    assert!(marker_ends.iter().any(|marker| marker.contains("__2563eb")));

    let labels = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("g")
                && node.attribute("class") == Some("label")
                && node.attribute("data-id") == Some("L_A_B_0")
        })
        .map(|label| {
            label
                .descendants()
                .filter_map(|node| node.text().filter(|_| node.is_text()))
                .collect::<String>()
        })
        .collect::<Vec<_>>();
    assert!(
        labels.iter().any(|label| label == "first & owner"),
        "{labels:?}"
    );
    assert!(
        labels.iter().any(|label| label == "second < owner"),
        "{labels:?}"
    );
}

#[test]
fn duplicate_flowchart_edge_ids_on_the_same_graphlib_key_keep_the_last_semantic_owner() {
    let svg = render_flowchart_svg_from_text(
        r##"---
config:
  layout: dagre
---
flowchart LR
A L_A_B_2@-->|first owner| B
A -->|second owner| B
A -->|third owner| B
linkStyle 0 stroke:#ef4444
linkStyle 1 stroke:#2563eb
linkStyle 2 stroke:#16a34a
"##,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Flowchart SVG");
    let paths = document
        .descendants()
        .filter(|node| node.has_tag_name("path") && node.attribute("data-edge") == Some("true"))
        .collect::<Vec<_>>();
    assert_eq!(
        paths.len(),
        2,
        "Graphlib must retain one L_A_B_2 owner: {svg}"
    );
    let collided = paths
        .iter()
        .find(|path| path.attribute("data-id") == Some("L_A_B_2"))
        .expect("surviving collided edge");
    let style = collided.attribute("style").unwrap_or_default();
    assert!(style.contains("stroke:#2563eb"), "{style}");
    assert!(!style.contains("stroke:#ef4444"), "{style}");
    assert!(svg.contains("second owner"), "{svg}");
    assert!(!svg.contains("first owner"), "{svg}");
}

#[test]
fn duplicate_flowchart_self_loop_ids_merge_each_semantic_occurrence_independently() {
    let svg = render_flowchart_svg_from_text(
        "flowchart LR\nX L_A_A_0@-->|first loop| X\nA -->|second loop| A\n",
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Flowchart SVG");
    let paths = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("path")
                && node.attribute("data-edge") == Some("true")
                && node.attribute("data-id") == Some("L_A_A_0")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        paths.len(),
        2,
        "both compact self-loops must survive: {svg}"
    );
    assert_ne!(paths[0].attribute("d"), paths[1].attribute("d"));
    assert!(
        !svg.contains("cyclic-special"),
        "helper segments leaked: {svg}"
    );
    assert!(svg.contains("first loop"), "{svg}");
    assert!(svg.contains("second loop"), "{svg}");
}

fn assert_duplicate_raw_edge_path_dom_identity(
    paths: &[roxmltree::Node<'_, '_>],
    raw_id: &str,
    svg: &str,
) {
    assert_eq!(paths.len(), 2, "expected two duplicate-id paths: {svg}");
    assert!(
        paths
            .iter()
            .all(|path| path.attribute("data-id") == Some(raw_id)),
        "public data-id must preserve the raw Mermaid id: {svg}"
    );
    let first_dom_id = paths[0]
        .attribute("id")
        .unwrap_or_else(|| panic!("first duplicate-id path is missing a DOM id: {svg}"));
    let second_dom_id = paths[1]
        .attribute("id")
        .unwrap_or_else(|| panic!("second duplicate-id path is missing a DOM id: {svg}"));
    assert_ne!(
        first_dom_id, second_dom_id,
        "duplicate raw ids must use occurrence-safe DOM ids: {svg}"
    );
}

fn assert_duplicate_raw_edge_occurrences_are_isolated(svg: &str) {
    let document = roxmltree::Document::parse(svg).expect("valid Flowchart-family SVG");
    let paths = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("path")
                && node.attribute("data-edge") == Some("true")
                && node.attribute("data-id") == Some("L_A_B_0")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        paths.len(),
        2,
        "both semantic occurrences must render: {svg}"
    );
    assert_duplicate_raw_edge_path_dom_identity(&paths, "L_A_B_0", svg);
    assert_ne!(
        paths[0].attribute("d"),
        paths[1].attribute("d"),
        "duplicate raw ids must not alias routed geometry"
    );

    let occurrence_bindings = paths
        .iter()
        .map(|path| {
            (
                path.attribute("style").unwrap_or_default(),
                path.attribute("marker-end").unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(occurrence_bindings.len(), 2);
    assert!(occurrence_bindings[0].0.contains("stroke:#ef4444"));
    assert!(occurrence_bindings[0].1.contains("crossEnd"));
    assert!(occurrence_bindings[1].0.contains("stroke:#2563eb"));
    assert!(occurrence_bindings[1].1.contains("pointEnd"));

    let labels = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("g")
                && node.attribute("class").is_some_and(|classes| {
                    classes
                        .split_ascii_whitespace()
                        .any(|class| class == "edgeLabel")
                })
        })
        .map(|label| {
            label
                .descendants()
                .filter_map(|node| node.text().filter(|_| node.is_text()))
                .collect::<String>()
        })
        .collect::<Vec<_>>();
    assert_eq!(labels, ["first owner", "second owner"], "{svg}");
}

#[test]
fn swimlane_duplicate_raw_edge_ids_keep_occurrence_bound_geometry_styles_labels_and_markers() {
    let svg = render_flowchart_svg_from_text(
        r##"swimlane-beta LR
X L_A_B_0@--x|first owner| Y
A -->|second owner| B
linkStyle 0 stroke:#ef4444
linkStyle 1 stroke:#2563eb
"##,
    );

    assert_duplicate_raw_edge_occurrences_are_isolated(&svg);
}

#[cfg(feature = "layout-elk")]
#[test]
fn flowchart_elk_duplicate_raw_edge_ids_keep_occurrence_bound_geometry_styles_labels_and_markers() {
    let svg = render_flowchart_svg_from_text(
        r##"---
config:
  layout: elk
---
flowchart LR
X L_A_B_0@--x|first owner| Y
A -->|second owner| B
linkStyle 0 stroke:#ef4444
linkStyle 1 stroke:#2563eb
"##,
    );

    assert_duplicate_raw_edge_occurrences_are_isolated(&svg);
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
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(
        serde_json::json!({"layout": "dagre"}),
    ));
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
    let svg = render_dagre_flowchart_svg_from_text(
        "flowchart TD\nA loop-edge@--> A\nA normal-edge@--> B\n",
    );

    let normal = svg.find(r#"data-id="normal-edge""#).expect("normal edge");
    let self_loop = svg.find(r#"data-id="loop-edge""#).expect("self-loop edge");
    assert!(
        normal < self_loop,
        "regular edges must render before compact self-loops"
    );
}

#[test]
fn flowchart_svg_keeps_external_direction_cluster_in_parent_root() {
    let svg = render_flowchart_svg_from_text(
        "flowchart TB\nsubgraph A\n  direction LR\n  a --> b\nend\na --> c\n",
    );

    assert_eq!(
        svg.matches(r#"<g class="root""#).count(),
        1,
        "an external edge must keep the directioned cluster in the parent root: {svg}"
    );
    assert_eq!(
        svg.matches(r#"data-id="A" data-et="cluster""#).count(),
        1,
        "the cluster should render exactly once in the parent root: {svg}"
    );
    assert_eq!(
        svg.matches(r#"data-id="a" data-et="node""#).count(),
        1,
        "the cluster's internal node should remain in the SVG DOM: {svg}"
    );
    assert_eq!(
        svg.matches(r#"data-id="b" data-et="node""#).count(),
        1,
        "the cluster's internal node should remain in the SVG DOM: {svg}"
    );
}

#[test]
fn flowchart_svg_keeps_edge_to_ancestor_cluster_in_parent_root() {
    let svg = render_dagre_flowchart_svg_from_text(
        "flowchart LR\nsubgraph Outer\n  direction TB\n  subgraph Inner\n    direction LR\n    a --> b\n  end\n  b --> c\nend\nc --> Outer\n",
    );

    assert_eq!(
        svg.matches(r#"<g class="root""#).count(),
        1,
        "external connections must keep directioned clusters in the parent root: {svg}"
    );
    assert_eq!(
        svg.matches(r#"data-id="L_c_Outer_0""#).count(),
        2,
        "the ancestor edge should have one path and one label entry inside Outer: {svg}"
    );
}

#[test]
fn flowchart_svg_renders_recursive_cluster_self_loop_in_parent_root() {
    let svg = render_dagre_flowchart_svg_from_text(
        "flowchart TB\nsubgraph Outer\n  subgraph Inner\n    x\n  end\n  Inner --> Inner\nend\n",
    );

    let outer_cluster = svg
        .find(r#"data-id="Outer" data-et="cluster""#)
        .expect("Outer cluster");
    let self_loop = svg
        .find(r#"data-id="L_Inner_Inner_0""#)
        .expect("Inner self-loop");
    let inner_cluster = svg
        .find(r#"data-id="Inner" data-et="cluster""#)
        .expect("Inner cluster");
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

fn svg_translate_values(transform: &str) -> [f64; 2] {
    let values = transform
        .strip_prefix("translate(")
        .and_then(|value| value.strip_suffix(')'))
        .unwrap_or_else(|| panic!("expected translate(...), got {transform:?}"))
        .split([',', ' '])
        .filter(|part| !part.is_empty())
        .map(|part| part.parse::<f64>().expect("translate number"))
        .collect::<Vec<_>>();
    assert_eq!(
        values.len(),
        2,
        "expected two translate values: {transform}"
    );
    [values[0], values[1]]
}

fn flowchart_svg_edge_label_geometry(svg: &str, data_id: &str) -> ([f64; 2], [f64; 2], [f64; 4]) {
    let document = roxmltree::Document::parse(svg).expect("valid Flowchart SVG");
    let label = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("data-id") == Some(data_id)
                && node
                    .attribute("class")
                    .is_some_and(|class| class.split_ascii_whitespace().any(|part| part == "label"))
        })
        .unwrap_or_else(|| panic!("edge label {data_id}: {svg}"));
    let outer = label.parent().expect("edgeLabel parent");
    assert!(
        outer.attribute("class").is_some_and(|class| {
            class
                .split_ascii_whitespace()
                .any(|part| part == "edgeLabel")
        }),
        "edge label parent: {svg}"
    );
    let background = label
        .descendants()
        .find(|node| {
            node.has_tag_name("rect")
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|part| part == "background")
                })
        })
        .expect("edge label background");
    let number = |name: &str| {
        background
            .attribute(name)
            .unwrap_or_else(|| panic!("edge label background {name}"))
            .parse::<f64>()
            .unwrap_or_else(|_| panic!("edge label background {name} number"))
    };
    (
        svg_translate_values(outer.attribute("transform").expect("edgeLabel transform")),
        svg_translate_values(label.attribute("transform").expect("label transform")),
        [number("x"), number("y"), number("width"), number("height")],
    )
}

fn assert_flowchart_svg_edge_label_is_centered_and_contained(svg: &str, data_id: &str) {
    let (anchor, translate, rect) = flowchart_svg_edge_label_geometry(svg, data_id);
    let [rect_x, rect_y, rect_width, rect_height] = rect;
    assert!(
        (translate[0] + rect_x + rect_width / 2.0).abs() < 1e-9,
        "edge label background must be horizontally centered on its anchor: {svg}"
    );
    assert!(
        (translate[1] + rect_y + rect_height / 2.0).abs() < 1e-9,
        "edge label background must be vertically centered on its anchor: {svg}"
    );

    let [viewbox_x, viewbox_y, viewbox_width, viewbox_height] = flowchart_svg_viewbox_values(svg);
    let viewbox = [viewbox_x, viewbox_y, viewbox_width, viewbox_height];
    let left = anchor[0] + translate[0] + rect_x;
    let top = anchor[1] + translate[1] + rect_y;
    let right = left + rect_width;
    let bottom = top + rect_height;
    assert!(
        left >= viewbox_x - 1e-9
            && top >= viewbox_y - 1e-9
            && right <= viewbox_x + viewbox_width + 1e-9
            && bottom <= viewbox_y + viewbox_height + 1e-9,
        "edge label background must remain inside the root viewBox: rect={rect:?}, anchor={anchor:?}, translate={translate:?}, viewBox={viewbox:?}, svg={svg}"
    );
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
    let node = document
        .descendants()
        .find(|node| {
            node.is_element()
                && node.tag_name().name() == "g"
                && node.attribute("data-id") == Some(node_id)
                && node.attribute("data-et") == Some("node")
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
    inner: DeterministicTextMeasurer,
    width_scale: f64,
}

impl WidthScaledTextMeasurer {
    fn new(width_scale: f64) -> Self {
        Self {
            inner: DeterministicTextMeasurer::default(),
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
        loose.contains(r#"target="_blank""#),
        "expected loose flowchart parity to preserve the Mermaid link target: {loose}"
    );
    assert!(
        loose.contains(r#"xlink:href="notes://do-your-thing/id""#)
            && loose.contains(r#"xlink:href="javascript:alert(1)""#),
        "expected loose mode to skip Mermaid's final SVG sanitizer for trusted links: {loose}"
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
    // Keep the SVG fixture below the backend's f32 coordinate safety ceiling while retaining
    // enough nesting to exercise the deep-render path.
    const DEPTH: usize = 1100;
    let text = deep_flowchart_subgraph_chain(DEPTH);

    let svg = render_flowchart_svg_from_text_with_engine_and_policy(
        Engine::new(),
        &text,
        RenderResourcePolicy::unbounded_for_trusted_input(),
    );

    assert!(svg.contains(r#"data-id="Leaf" data-et="node""#));
    assert!(svg.contains(r#"data-id="S0" data-et="cluster""#));
}

#[test]
fn flowchart_diagram_padding_zero_keeps_zero_user_padding() {
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

    // Mermaid applies the configured padding directly to the measured bounds.
    for axis in [2, 3] {
        assert!(
            (default_viewbox[axis] - zero_viewbox[axis] - 16.0).abs() < 1e-6,
            "default diagramPadding=8 should add 16px over zero padding; default={default_viewbox:?}, zero={zero_viewbox:?}"
        );
    }
    for axis in [0, 1] {
        assert!(
            (zero_viewbox[axis] - default_viewbox[axis] - 8.0).abs() < 1e-6,
            "diagramPadding=0 should remove the full default 8px inset; default={default_viewbox:?}, zero={zero_viewbox:?}"
        );
    }
}

#[test]
fn swimlane_small_diagram_padding_tracks_configured_inset() {
    let render = |padding: &str| {
        render_flowchart_svg_from_text(&format!(
            r#"%%{{init: {{"flowchart": {{"diagramPadding": {padding}}}}}}}%%
swimlane-beta LR
A --> B
"#,
        ))
    };
    let zero = flowchart_svg_viewbox_values(&render("0"));
    let fractional = flowchart_svg_viewbox_values(&render("0.5"));
    let one = flowchart_svg_viewbox_values(&render("1"));

    assert!((fractional[0] - zero[0] + 0.5).abs() < 1e-9);
    assert!((fractional[1] - zero[1] + 0.5).abs() < 1e-9);
    assert!((fractional[2] - zero[2] - 1.0).abs() < 1e-9);
    assert!((fractional[3] - zero[3] - 1.0).abs() < 1e-9);
    assert!((one[0] - zero[0] + 1.0).abs() < 1e-9);
    assert!((one[1] - zero[1] + 1.0).abs() < 1e-9);
    assert!((one[2] - zero[2] - 2.0).abs() < 1e-9);
    assert!((one[3] - zero[3] - 2.0).abs() < 1e-9);
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
        svg.contains(r#"data-id="Group" data-et="cluster" data-look="neo""#),
        "expected flowchart subgraph cluster to propagate configured look: {svg}"
    );
    assert!(
        !svg.contains(r#"data-look="classic""#),
        "configured flowchart look must not leave classic DOM attributes: {svg}"
    );
}

#[test]
fn flowchart_v2_fontawesome_edge_label_width_uses_nominal_icon_boundary() {
    // Mermaid uses a clean 1.25em inline box for FontAwesome labels instead of
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
    let text = DeterministicTextMeasurer::default().measure_wrapped(
        " Car",
        &TextStyle {
            font_size: 14.0,
            ..TextStyle::default()
        },
        None,
        WrapMode::HtmlLike,
    );
    let expected_width = merman_render::text::ceil_to_1_64_px(text.width + 14.0 * 1.25);
    assert_eq!(lbl.width, expected_width);
    assert_eq!(lbl.height, 21.0);
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

    let document = roxmltree::Document::parse(&svg).expect("valid Flowchart SVG");
    let node = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("data-et") == Some("node")
                && node.attribute("data-id") == Some("Security")
        })
        .expect("Security node");
    let foreign_object = node
        .descendants()
        .find(|node| node.has_tag_name("foreignObject"))
        .expect("Security foreignObject");
    let measured_width = foreign_object
        .attribute("width")
        .expect("foreignObject width")
        .parse::<f64>()
        .expect("numeric foreignObject width");
    let div_style = foreign_object
        .descendants()
        .find(|node| node.has_tag_name("div"))
        .and_then(|node| node.attribute("style"))
        .expect("HTML label div style");

    assert!(measured_width.is_finite() && (0.0..=120.0).contains(&measured_width));
    assert_eq!(foreign_object.attribute("height"), Some("63"));
    assert!(div_style.contains("max-width: 120px") && div_style.contains("width: 120px"));
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
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "flowchart": {"minNodeWidth": 0}
    })));
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
        assert_eq!(contract.1, 21.0, "{text}");
        assert!(contract.2.contains("overflow: visible"), "{text}");
        assert!(contract.3.contains("white-space: nowrap"), "{text}");
        // Edge labels retain createText's 200px default; nodes use flowchart.wrappingWidth.
        let max_width = if text == "Yes" { "200px" } else { "120px" };
        assert!(
            contract.3.contains(&format!("max-width: {max_width}")),
            "{text}"
        );
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
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "flowchart": {"minNodeWidth": 0}
    })));
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
        r##"%%{init: {"theme": "default", "look": "classic", "themeVariables": {"strokeWidth": 4, "lineColor": "#112233", "nodeBorder": "#445566"}}}%%
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
        svg.contains(r#"#merman .edgePaths .path{stroke:#112233;stroke-width:4px;}"#),
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
fn flowchart_and_swimlane_typed_edge_stroke_width_reaches_classic_and_neo_paths() {
    let unqualified_theme = edge_stroke_width_theme(2.5);
    let default_theme = explicit_default_edge_stroke_width_theme(3.25);
    for (case, source, theme, expected_width) in [
        (
            "Flowchart classic unqualified",
            "flowchart LR\nA --> B\n",
            &unqualified_theme,
            2.5,
        ),
        (
            "Flowchart Neo explicit Default",
            "%%{init: {\"look\": \"neo\"}}%%\nflowchart LR\nA --> B\n",
            &default_theme,
            3.25,
        ),
        (
            "Swimlane classic explicit Default",
            "swimlane-beta LR\nA --> B\n",
            &default_theme,
            3.25,
        ),
    ] {
        let rendered = prepare_flowchart_family_with_theme(source, theme)
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap_or_else(|error| panic!("render {case} typed edge stroke width: {error}"));
        let document = roxmltree::Document::parse(rendered.svg())
            .unwrap_or_else(|error| panic!("valid {case} SVG: {error}"));
        let edge = document
            .descendants()
            .find(|node| node.has_tag_name("path") && node.attribute("data-edge") == Some("true"))
            .unwrap_or_else(|| panic!("{case} terminal edge path: {}", rendered.svg()));
        let style = edge
            .attribute("style")
            .unwrap_or_else(|| panic!("{case} terminal edge style"));
        assert!(
            style.contains(&format!("stroke-width:{expected_width}px !important")),
            "{case}: {style}"
        );

        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.applied_count(), 1, "{case}");
        assert_eq!(evidence.not_applicable_count(), 0, "{case}");
        assert_eq!(evidence.theme_residual_count(), 0, "{case}");
    }
}

#[test]
fn flowchart_default_edge_stroke_keeps_upstream_geometric_viewbox_height() {
    let svg = render_flowchart_svg_from_text(include_str!(
        "../../../fixtures/flowchart/upstream_cypress_flowchart_spec_6_should_render_a_flowchart_full_of_circles_006.mmd"
    ));
    let expected = flowchart_svg_viewbox_values(include_str!(
        "../../../fixtures/upstream-svgs/flowchart/upstream_cypress_flowchart_spec_6_should_render_a_flowchart_full_of_circles_006.svg"
    ));
    let actual = flowchart_svg_viewbox_values(&svg);
    // Width retains browser text-measurement residuals. The default edge stroke must not add
    // a new vertical extent to the otherwise shared geometric bounds.
    assert_eq!(actual[1], expected[1]);
    assert_eq!(actual[3], expected[3]);
}

#[test]
fn flowchart_source_stroke_width_does_not_inherit_suppressed_typed_paint_bounds() {
    let theme = edge_stroke_width_theme(160.0);
    for backend in ["elk", "dagre"] {
        let source = format!(
            "---\nconfig:\n  layout: {backend}\n  flowchart:\n    diagramPadding: 0\n---\nflowchart LR\nA --- B\nlinkStyle 0 stroke-width:7px\n"
        );
        let control = render_flowchart_svg_from_text(&source);
        let themed = prepare_flowchart_family_with_theme(&source, &theme)
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("source stroke width supersedes typed width");
        assert_eq!(
            flowchart_svg_viewbox_values(themed.svg()),
            flowchart_svg_viewbox_values(&control),
            "{backend}: a suppressed typed stroke must not extend the viewport"
        );
        assert!(themed.svg().contains("stroke-width:7px"));
        assert!(!themed.svg().contains("stroke-width:160px"));
    }
}

#[test]
fn flowchart_and_swimlane_typed_edge_stroke_width_expands_paint_bounds_without_relayout() {
    const STROKE_WIDTH: f32 = 160.0;

    fn prepare_control(source: &str) -> family::FamilyRenderArtifact {
        let parsed =
            block_on(Engine::new().parse_diagram_for_render_model(source, ParseOptions::default()))
                .expect("parse control Flowchart family")
                .expect("detect control Flowchart family");
        family::prepare(
            parsed,
            &LayoutOptions::default(),
            RenderEnvironment::deterministic()
                .begin_session()
                .expect("begin control Flowchart session"),
        )
        .expect("prepare control Flowchart family")
    }

    let theme = edge_stroke_width_theme(STROKE_WIDTH);
    for (family_name, source, layout_key) in [
        (
            "Flowchart",
            "%%{init: {\"flowchart\": {\"diagramPadding\": 0}}}%%\nflowchart LR\nA --- B\n",
            "FlowchartV2",
        ),
        (
            "Swimlane",
            "%%{init: {\"flowchart\": {\"diagramPadding\": 0}}}%%\nswimlane-beta LR\nA --- B\n",
            "SwimlaneDiagram",
        ),
    ] {
        let control = prepare_control(source);
        let control_layout = control
            .layout_json()
            .unwrap_or_else(|error| panic!("project control {family_name} layout: {error}"));
        let control_svg = control
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap_or_else(|error| panic!("render control {family_name} SVG: {error}"));

        let themed = prepare_flowchart_family_with_theme(source, &theme);
        let themed_layout = themed
            .layout_json()
            .unwrap_or_else(|error| panic!("project themed {family_name} layout: {error}"));
        assert_eq!(
            control_layout["layout"][layout_key], themed_layout["layout"][layout_key],
            "{family_name} stroke paint must not alter layout geometry"
        );
        let themed_svg = themed
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap_or_else(|error| panic!("render themed {family_name} SVG: {error}"));

        let control_viewbox = flowchart_svg_viewbox_values(control_svg.svg());
        let themed_viewbox = flowchart_svg_viewbox_values(themed_svg.svg());
        assert!(
            themed_viewbox[3] + 1.0e-6 >= f64::from(STROKE_WIDTH),
            "{family_name} viewBox must contain the full {STROKE_WIDTH}px edge stroke; control={control_viewbox:?}, themed={themed_viewbox:?}, svg={}",
            themed_svg.svg()
        );
        assert!(
            themed_viewbox[3] > control_viewbox[3],
            "{family_name} paint bounds must expand beyond the unchanged layout bounds; control={control_viewbox:?}, themed={themed_viewbox:?}"
        );
    }
}

#[test]
fn flowchart_source_and_explicit_config_stroke_width_override_the_typed_edge_value() {
    let theme = edge_stroke_width_theme(2.5);
    for (case, engine, source, expected_path_style, expected_svg) in [
        (
            "linkStyle inline",
            Engine::new(),
            "flowchart LR\nA edge@--> B\nlinkStyle 0 stroke-width:7px\n",
            Some("stroke-width:7px"),
            None,
        ),
        (
            "assigned classDef",
            Engine::new(),
            "flowchart LR\nA edge@--> B\nclassDef wide stroke-width:8px\nclass edge wide\n",
            Some("stroke-width:8px"),
            None,
        ),
        (
            "explicit Mermaid config",
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "theme": "base",
                "themeVariables": {"strokeWidth": 9}
            }))),
            "flowchart LR\nA --> B\n",
            None,
            Some(".edge-thickness-normal{stroke-width:9px;}"),
        ),
    ] {
        let rendered = prepare_flowchart_family_with_theme_engine_and_portability(
            source,
            &theme,
            engine,
            ThemePortabilityRequirement::RequirePortable,
        )
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap_or_else(|error| panic!("render {case} edge stroke-width precedence: {error}"));
        let document = roxmltree::Document::parse(rendered.svg())
            .unwrap_or_else(|error| panic!("valid {case} SVG: {error}"));
        let edge = document
            .descendants()
            .find(|node| node.has_tag_name("path") && node.attribute("data-edge") == Some("true"))
            .unwrap_or_else(|| panic!("{case} terminal edge path: {}", rendered.svg()));
        let style = edge.attribute("style").unwrap_or_default();
        if let Some(expected_path_style) = expected_path_style {
            assert!(style.contains(expected_path_style), "{case}: {style}");
        }
        if let Some(expected_svg) = expected_svg {
            assert!(
                rendered.svg().contains(expected_svg),
                "{case}: {}",
                rendered.svg()
            );
        }
        assert!(
            !style.contains("stroke-width:2.5px !important"),
            "{case}: {style}"
        );

        let completion = rendered.into_completion();
        let evidence = merman_render::__private::family_evidence(completion.report());
        assert_eq!(evidence.applied_count(), 0, "{case}");
        assert_eq!(evidence.not_applicable_count(), 1, "{case}");
        assert_eq!(evidence.theme_residual_count(), 0, "{case}");
    }
}

#[test]
fn flowchart_and_swimlane_source_label_color_supersedes_typed_fill_without_residual() {
    for (family, source) in [
        (
            DiagramFamilyId::FLOWCHART,
            "---\nconfig:\n  htmlLabels: false\n---\nflowchart LR\nclassDef local color:#22c55e\nA[Alpha]:::local\n",
        ),
        (
            DiagramFamilyId::SWIMLANE,
            "---\nconfig:\n  layout: swimlane\n  htmlLabels: false\n---\nflowchart TD\nclassDef local color:#22c55e\nA[Alpha]:::local\n",
        ),
    ] {
        let theme = node_label_fill_theme(family, "#ef4444");
        let rendered = prepare_flowchart_family_with_theme(source, &theme)
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap_or_else(|error| {
                panic!("source-owned {family} NodeLabel fill must remain portable: {error}")
            });
        let document = roxmltree::Document::parse(rendered.svg()).expect("valid themed SVG");
        let node = document
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && node.attribute("data-id") == Some("A")
                    && node.attribute("data-et") == Some("node")
            })
            .unwrap_or_else(|| panic!("{family} node A: {}", rendered.svg()));
        let label_style = node
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && node.attribute("class").is_some_and(|class| {
                        class.split_ascii_whitespace().any(|part| part == "label")
                    })
                    && node
                        .descendants()
                        .any(|descendant| descendant.text() == Some("Alpha"))
            })
            .and_then(|node| node.attribute("style"))
            .unwrap_or_else(|| panic!("{family} node label style: {}", rendered.svg()));
        assert!(
            label_style.contains("color:#22c55e !important"),
            "{family}: {label_style}"
        );
        assert!(!label_style.contains("#ef4444"), "{family}: {label_style}");

        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.required_count(), 1, "{family}");
        assert_eq!(evidence.accounted_count(), 1, "{family}");
        assert_eq!(evidence.applied_count(), 0, "{family}");
        assert_eq!(evidence.not_applicable_count(), 1, "{family}");
        assert_eq!(evidence.theme_residual_count(), 0, "{family}");
    }
}

#[test]
fn flowchart_and_swimlane_node_label_fill_uses_the_effective_mermaid_color_owner() {
    struct Case {
        name: &'static str,
        config: serde_json::Value,
        typed_applies: bool,
    }

    let cases = [
        Case {
            name: "default textColor is shadowed by nodeTextColor",
            config: serde_json::json!({
                "theme": "default",
                "themeVariables": {"textColor": "#16a34a"}
            }),
            typed_applies: true,
        },
        Case {
            name: "base textColor is shadowed by nodeTextColor",
            config: serde_json::json!({
                "theme": "base",
                "themeVariables": {"textColor": "#16a34a"}
            }),
            typed_applies: true,
        },
        Case {
            name: "explicit nodeTextColor owns the winner",
            config: serde_json::json!({
                "theme": "default",
                "themeVariables": {
                    "nodeTextColor": "#16a34a",
                    "textColor": "#2563eb"
                }
            }),
            typed_applies: false,
        },
        Case {
            name: "default primaryTextColor owns derived nodeTextColor",
            config: serde_json::json!({
                "theme": "default",
                "themeVariables": {"primaryTextColor": "#16a34a"}
            }),
            typed_applies: false,
        },
        Case {
            name: "base primaryTextColor owns derived nodeTextColor",
            config: serde_json::json!({
                "theme": "base",
                "themeVariables": {"primaryTextColor": "#16a34a"}
            }),
            typed_applies: false,
        },
        Case {
            name: "dark primaryTextColor does not own textColor fallback",
            config: serde_json::json!({
                "theme": "dark",
                "themeVariables": {"primaryTextColor": "#16a34a"}
            }),
            typed_applies: true,
        },
        Case {
            name: "dark nodeTextColor owns the winner",
            config: serde_json::json!({
                "theme": "dark",
                "themeVariables": {"nodeTextColor": "#16a34a"}
            }),
            typed_applies: false,
        },
        Case {
            name: "dark textColor owns the fallback winner",
            config: serde_json::json!({
                "theme": "dark",
                "themeVariables": {"textColor": "#16a34a"}
            }),
            typed_applies: false,
        },
    ];

    for (family, source) in [
        (DiagramFamilyId::FLOWCHART, "flowchart LR\nA[Alpha]\n"),
        (
            DiagramFamilyId::SWIMLANE,
            "---\nconfig:\n  layout: swimlane\n---\nflowchart TD\nA[Alpha]\n",
        ),
    ] {
        for case in &cases {
            let theme = node_label_fill_theme(family, "#ef4444");
            let engine =
                Engine::new().with_site_config(MermaidConfig::from_value(case.config.clone()));
            let rendered = prepare_flowchart_family_with_theme_engine_and_portability(
                source,
                &theme,
                engine,
                ThemePortabilityRequirement::RequirePortable,
            )
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap_or_else(|error| {
                panic!(
                    "{} {family} NodeLabel fill must remain portable: {error}",
                    case.name
                )
            });
            let document = roxmltree::Document::parse(rendered.svg()).expect("valid themed SVG");
            let node = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("g")
                        && node.attribute("data-id") == Some("A")
                        && node.attribute("data-et") == Some("node")
                })
                .unwrap_or_else(|| panic!("{} {family} node A: {}", case.name, rendered.svg()));
            let label = node
                .descendants()
                .find(|node| {
                    node.has_tag_name("g")
                        && node.attribute("class").is_some_and(|class| {
                            class.split_ascii_whitespace().any(|part| part == "label")
                        })
                        && node
                            .descendants()
                            .any(|descendant| descendant.text() == Some("Alpha"))
                })
                .unwrap_or_else(|| panic!("{} {family} node label: {}", case.name, rendered.svg()));
            let label_style = label.attribute("style").unwrap_or_default();
            assert_eq!(
                label_style.contains("fill:#ef4444 !important;color:#ef4444 !important"),
                case.typed_applies,
                "{} {family}: {label_style}",
                case.name
            );

            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.required_count(), 1, "{} {family}", case.name);
            assert_eq!(evidence.accounted_count(), 1, "{} {family}", case.name);
            assert_eq!(
                evidence.applied_count(),
                usize::from(case.typed_applies),
                "{} {family}",
                case.name
            );
            assert_eq!(
                evidence.not_applicable_count(),
                usize::from(!case.typed_applies),
                "{} {family}",
                case.name
            );
            assert_eq!(evidence.theme_residual_count(), 0, "{} {family}", case.name);
        }
    }
}

#[test]
fn flowchart_cleared_edge_stroke_width_remains_an_explicit_geometry_residual() {
    let theme = DiagramThemeCompiler::new()
        .compile(
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::Edge,
                ThemeStylePatch {
                    stroke: ThemeStrokePatch {
                        width: Specified::Clear,
                        ..ThemeStrokePatch::default()
                    },
                    ..ThemeStylePatch::default()
                },
            ))),
        )
        .expect("compile cleared edge stroke-width theme");
    let rendered = prepare_flowchart_family_with_theme_and_portability(
        "flowchart LR\nA --> B\n",
        &theme,
        ThemePortabilityRequirement::BestEffort,
    )
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("best-effort cleared edge stroke width");
    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 1);
}

#[test]
fn flowchart_hand_drawn_edge_stroke_width_is_a_fail_closed_residual() {
    let theme = edge_stroke_width_theme(2.5);
    let rendered = prepare_flowchart_family_with_theme_and_portability(
        "%%{init: {\"look\": \"handDrawn\", \"handDrawnSeed\": 7}}%%\nflowchart LR\nA --> B\n",
        &theme,
        ThemePortabilityRequirement::BestEffort,
    )
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .expect("best-effort hand-drawn edge stroke width");
    let document =
        roxmltree::Document::parse(rendered.svg()).expect("valid hand-drawn Flowchart SVG");
    let style = document
        .descendants()
        .find(|node| node.has_tag_name("path") && node.attribute("data-edge") == Some("true"))
        .and_then(|node| node.attribute("style"))
        .expect("hand-drawn terminal edge style");
    assert!(!style.contains("stroke-width:2.5px !important"), "{style}");

    let completion = rendered.into_completion();
    let evidence = merman_render::__private::family_evidence(completion.report());
    assert_eq!(evidence.applied_count(), 0);
    assert_eq!(evidence.not_applicable_count(), 0);
    assert_eq!(evidence.theme_residual_count(), 1);
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

    let document = roxmltree::Document::parse(&svg).expect("valid Flowchart SVG");
    let edge = document
        .descendants()
        .find(|node| {
            node.has_tag_name("path")
                && node.attribute("data-et") == Some("edge")
                && node.attribute("data-id") == Some("L_A_B_0")
        })
        .expect("edge path");
    let edge_style = edge.attribute("style").expect("edge path style");

    assert!(
        edge_style.contains("stroke-width:7px"),
        "expected linkStyle stroke-width to stay on the visible Flowchart edge path: {edge_style}"
    );
    assert!(
        edge_style.contains("stroke:#abcdef"),
        "expected linkStyle stroke color to stay on the visible Flowchart edge path: {edge_style}"
    );
}

#[test]
fn flowchart_colored_marker_whitespace_follows_security_level() {
    fn marker_path_colors(svg: &str) -> (String, String) {
        let document = roxmltree::Document::parse(svg).expect("valid Flowchart SVG");
        let marker_ids = document
            .descendants()
            .filter(|node| node.is_element() && node.tag_name().name() == "marker")
            .filter_map(|node| node.attribute("id"))
            .collect::<Vec<_>>();
        let marker = document
            .descendants()
            .find(|node| {
                node.is_element()
                    && node.tag_name().name() == "marker"
                    && node
                        .attribute("id")
                        .is_some_and(|id| id.ends_with("-pointEnd-margin__orange"))
            })
            .unwrap_or_else(|| panic!("orange point-end marker; emitted ids: {marker_ids:?}"));
        let path = marker
            .children()
            .find(|node| node.is_element() && node.tag_name().name() == "path")
            .expect("marker path");
        (
            path.attribute("stroke").expect("marker stroke").to_string(),
            path.attribute("fill").expect("marker fill").to_string(),
        )
    }

    let source = r#"flowchart TB
    A --> B
    linkStyle 0 color:orange, stroke: orange;
"#;

    for security_level in ["strict", "sandbox"] {
        let svg = render_flowchart_svg_from_text_with_engine(
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "securityLevel": security_level
            }))),
            source,
        );
        assert_eq!(
            marker_path_colors(&svg),
            ("orange".to_string(), "orange".to_string()),
            "{security_level}"
        );
    }

    let loose = render_flowchart_svg_from_text_with_engine(
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "securityLevel": "loose"
        }))),
        source,
    );
    assert_eq!(
        marker_path_colors(&loose),
        (" orange".to_string(), " orange".to_string())
    );
}

#[test]
fn flowchart_colored_bidirectional_markers_define_every_referenced_variant() {
    let svg = render_flowchart_svg_from_text(
        r##"---
config:
  theme: default
  look: classic
  layout: dagre
---
flowchart LR
    A o--o B
    B <--> C
    C x--x D
    linkStyle 0 stroke:#ef4444
    linkStyle 1 stroke:#22c55e
    linkStyle 2 stroke:#2563eb
"##,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Flowchart SVG");
    let marker_ids = document
        .descendants()
        .filter(|node| node.has_tag_name("marker"))
        .filter_map(|node| node.attribute("id"))
        .collect::<std::collections::BTreeSet<_>>();
    let references = document
        .descendants()
        .filter(|node| node.has_tag_name("path") && node.attribute("data-edge") == Some("true"))
        .flat_map(|node| [node.attribute("marker-start"), node.attribute("marker-end")])
        .flatten()
        .map(|reference| {
            reference
                .strip_prefix("url(#")
                .and_then(|value| value.strip_suffix(')'))
                .expect("local Flowchart marker reference")
        })
        .collect::<Vec<_>>();

    for reference in &references {
        assert!(
            marker_ids.contains(reference),
            "missing marker definition for {reference}: {marker_ids:?}"
        );
    }
    for suffix in [
        "circleStart__ef4444",
        "circleEnd__ef4444",
        "pointStart__22c55e",
        "pointEnd__22c55e",
        "crossStart__2563eb",
        "crossEnd__2563eb",
    ] {
        assert!(
            references
                .iter()
                .any(|reference| reference.ends_with(suffix)),
            "missing colored marker reference {suffix}: {references:?}"
        );
    }
}

#[test]
fn flowchart_neo_markers_use_margin_geometry_only_for_non_animated_edges() {
    let svg = render_flowchart_svg_from_text(
        r##"%%{init: {"look": "neo"}}%%
flowchart LR
    A o--o B
    B animated@==> C
    C explicit-speed@==> D
    animated@{ animate: true }
    explicit-speed@{ animate: false, animation: fast }
    linkStyle 0 stroke:#ef4444
    linkStyle 1 stroke:#2563eb
    linkStyle 2 stroke:#16a34a
"##,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Flowchart SVG");
    let edge_paths = document
        .descendants()
        .filter(|node| node.has_tag_name("path") && node.attribute("data-edge") == Some("true"))
        .collect::<Vec<_>>();
    assert_eq!(edge_paths.len(), 3, "expected three rendered edges: {svg}");

    let static_edge = edge_paths
        .iter()
        .find(|node| node.attribute("data-id") == Some("L_A_B_0"))
        .expect("static circle edge");
    assert!(
        static_edge
            .attribute("marker-start")
            .is_some_and(|value| value.ends_with("-circleStart-margin__ef4444)")),
        "non-animated Neo start marker must use margin geometry: {static_edge:?}"
    );
    assert!(
        static_edge
            .attribute("marker-end")
            .is_some_and(|value| value.ends_with("-circleEnd-margin__ef4444)")),
        "non-animated Neo end marker must use margin geometry: {static_edge:?}"
    );
    assert!(
        static_edge
            .attribute("style")
            .is_some_and(|value| value.starts_with("stroke-dasharray: 0 ")),
        "non-animated Neo edge must retain the marker mask: {static_edge:?}"
    );

    let animated_edge = edge_paths
        .iter()
        .find(|node| node.attribute("data-id") == Some("animated"))
        .expect("animated point edge");
    assert!(
        animated_edge
            .attribute("marker-end")
            .is_some_and(|value| value.ends_with("-pointEnd__2563eb)")),
        "animated Neo marker must retain non-margin geometry: {animated_edge:?}"
    );
    assert!(
        !animated_edge
            .attribute("marker-end")
            .is_some_and(|value| value.contains("-margin")),
        "animated Neo marker must not reference a margin variant: {animated_edge:?}"
    );
    assert!(
        !animated_edge
            .attribute("style")
            .is_some_and(|value| value.starts_with("stroke-dasharray: 0 ")),
        "animated Neo edge must not receive the non-animated marker mask: {animated_edge:?}"
    );

    let explicit_speed = edge_paths
        .iter()
        .find(|node| node.attribute("data-id") == Some("explicit-speed"))
        .expect("explicit-speed edge");
    assert!(
        explicit_speed.attribute("class").is_some_and(|value| value
            .split_ascii_whitespace()
            .any(|class| { class == "edge-animation-fast" })),
        "explicit animation speed must override animate:false: {explicit_speed:?}"
    );
    assert!(
        explicit_speed
            .attribute("marker-end")
            .is_some_and(|value| value.ends_with("-pointEnd__16a34a)")),
        "explicit animation speed must retain non-margin marker geometry: {explicit_speed:?}"
    );
    assert!(
        !explicit_speed
            .attribute("style")
            .is_some_and(|value| value.starts_with("stroke-dasharray: 0 ")),
        "explicit animation speed must suppress the non-animated Neo mask: {explicit_speed:?}"
    );

    for reference in edge_paths
        .iter()
        .flat_map(|node| [node.attribute("marker-start"), node.attribute("marker-end")])
        .flatten()
    {
        let id = reference
            .strip_prefix("url(#")
            .and_then(|value| value.strip_suffix(')'))
            .expect("local marker reference");
        assert!(
            document
                .descendants()
                .any(|node| { node.has_tag_name("marker") && node.attribute("id") == Some(id) }),
            "missing Neo marker definition for {id}: {svg}"
        );
    }
}

#[test]
fn flowchart_colored_marker_id_collisions_follow_actual_root_emission_order() {
    let svg = render_flowchart_svg_from_text(
        r#"---
config:
  theme: default
  look: classic
  layout: dagre
---
flowchart LR
    subgraph Nested
        A --> B
    end
    C --> D
    linkStyle 0 stroke:hsl(-30 100% 50%)
    linkStyle 1 stroke:hsl(+30 100% 50%)
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Flowchart SVG");
    let colored_markers = document
        .descendants()
        .filter(|node| {
            node.has_tag_name("marker")
                && node
                    .attribute("id")
                    .is_some_and(|id| id.ends_with("-pointEnd_hsl__30_100__50__"))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        colored_markers.len(),
        1,
        "sanitized marker IDs must be deduplicated: {svg}"
    );
    let path = colored_markers[0]
        .children()
        .find(|node| node.has_tag_name("path"))
        .expect("colored point marker path");
    assert_eq!(
        path.attribute("stroke"),
        Some("hsl(+30 100% 50%)"),
        "the root edge is emitted before the nested root and must own the colliding marker ID"
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
            r##"#merman .label{font-family:"Recursive Variable",arial,sans-serif;color:#f8fafc;}"##
        ),
        "expected themeVariables.nodeTextColor to drive Flowchart label color CSS: {svg}"
    );
    assert!(
        svg.contains(r##"#merman .label text,#merman span{fill:#f8fafc;color:#f8fafc;}"##),
        "expected themeVariables.nodeTextColor to drive Flowchart label text fill CSS: {svg}"
    );
    assert!(
        svg.contains(
            r##"#merman{font-family:"Recursive Variable",arial,sans-serif;font-size:14px;fill:#fde68a;}"##
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
fn flowchart_cylinder_label_offset_y_follows_security_level() {
    fn cylinder_label_offset(svg: &str, node_id: &str) -> Option<(String, String)> {
        let document = roxmltree::Document::parse(svg).expect("valid Flowchart SVG");
        let node = document
            .descendants()
            .find(|node| {
                node.is_element()
                    && node.tag_name().name() == "g"
                    && node.attribute("data-id") == Some(node_id)
                    && node.attribute("data-et") == Some("node")
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
        let offset = shape.attribute("label-offset-y")?.to_string();
        let path_offset = shape
            .attribute("d")
            .expect("cylinder path data")
            .strip_prefix("M0,")
            .expect("cylinder path starts at x=0")
            .split_once(' ')
            .expect("cylinder path first segment")
            .0
            .to_string();
        Some((offset, path_offset))
    }

    let source = r#"flowchart TB
C@{ shape: cylinder, label: "Cylinder" }
L@{ shape: lined-cylinder, label: "Lined cylinder" }
"#;

    for security_level in ["strict", "sandbox"] {
        let svg = render_flowchart_svg_from_text_with_engine(
            Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "securityLevel": security_level
            }))),
            source,
        );
        for node_id in ["C", "L"] {
            assert_eq!(
                cylinder_label_offset(&svg, node_id),
                None,
                "{security_level}"
            );
        }
    }

    let loose = render_flowchart_svg_from_text_with_engine(
        Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "securityLevel": "loose"
        }))),
        source,
    );
    for node_id in ["C", "L"] {
        let (offset, path_offset) = cylinder_label_offset(&loose, node_id)
            .unwrap_or_else(|| panic!("label-offset-y for {node_id}"));
        assert_eq!(offset, path_offset, "{node_id}");
    }
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
fn flowchart_neo_source_animation_uses_the_parsed_final_css_winner() {
    let svg = render_flowchart_svg_from_text(
        r##"%%{init: {"look": "neo"}}%%
flowchart LR
    A inline-active@==> B
    B class-disabled@==> C
    C custom-token@==> D
    classDef disabled animation:none
    classDef token --animation-token:dash
    class class-disabled disabled
    class custom-token token
    linkStyle 0 animation:dash 2s linear
"##,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Flowchart SVG");
    let edge = |id| {
        document
            .descendants()
            .find(|node| {
                node.has_tag_name("path")
                    && node.attribute("data-edge") == Some("true")
                    && node.attribute("data-id") == Some(id)
            })
            .unwrap_or_else(|| panic!("missing edge {id}: {svg}"))
    };

    let inline_active = edge("inline-active");
    assert!(
        inline_active
            .attribute("marker-end")
            .is_some_and(|value| !value.contains("-margin")),
        "inline animation must use animated marker geometry: {inline_active:?}"
    );
    assert!(
        !inline_active
            .attribute("style")
            .is_some_and(|value| value.starts_with("stroke-dasharray: 0 ")),
        "inline animation must suppress the non-animated Neo mask: {inline_active:?}"
    );

    for inactive_id in ["class-disabled", "custom-token"] {
        let inactive = edge(inactive_id);
        assert!(
            inactive
                .attribute("marker-end")
                .is_some_and(|value| value.contains("-pointEnd-margin")),
            "inactive edge must use margin marker geometry: {inactive:?}"
        );
        assert!(
            inactive
                .attribute("style")
                .is_some_and(|value| value.starts_with("stroke-dasharray: 0 ")),
            "inactive edge must retain the Neo marker mask: {inactive:?}"
        );
    }
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
fn flowchart_stadium_path_uses_html_label_font_size_for_fractional_theme_size() {
    // Mermaid `stadium.ts` sizes its path from the `labelHelper(...)` bbox, i.e. the HTML label
    // measured with the CSS `font-size: 12.5px`, not the integer `parseFontSize` number.
    for look in ["neo", "classic"] {
        let source = format!(
            "%%{{init: {{\"look\": \"{look}\", \"themeVariables\": {{\"fontSize\": \"12.5px\"}}}}}}%%\nflowchart LR\nA([Client]) --> B[Rect]\n"
        );
        let parse = || {
            block_on(Engine::new().parse_diagram_for_render_model(&source, ParseOptions::default()))
                .expect("parse ok")
                .expect("diagram detected")
        };
        let layout = layout_flowchart_render_model(
            parse(),
            &LayoutOptions::default(),
            RenderEnvironment::deterministic().begin_session().unwrap(),
        )
        .expect("layout ok");
        let svg = render_flowchart_artifact(
            parse(),
            &LayoutOptions::default(),
            RenderEnvironment::deterministic().begin_session().unwrap(),
            &SvgRenderOptions::default(),
        )
        .expect("render svg");

        let node = layout.nodes.iter().find(|node| node.id == "A").unwrap();
        let document = roxmltree::Document::parse(&svg).expect("valid SVG");
        let path = document
            .descendants()
            .find(|n| {
                n.has_tag_name("g")
                    && n.attribute("data-id") == Some("A")
                    && n.attribute("data-et") == Some("node")
            })
            .and_then(|group| group.descendants().find(|n| n.has_tag_name("path")))
            .and_then(|path| path.attribute("d"))
            .expect("stadium path");
        let numbers: Vec<f64> = path
            .split(|c: char| c.is_ascii_alphabetic() || c == ',' || c.is_whitespace())
            .filter(|token| !token.is_empty())
            .map(|token| token.parse().expect("path number"))
            .collect();
        let ys = numbers.chunks_exact(2).map(|pair| pair[1]);
        let painted_height = ys.clone().fold(f64::MIN, f64::max) - ys.fold(f64::MAX, f64::min);

        assert!(
            (painted_height - node.height).abs() < 1e-6,
            "{look}: stadium path height {painted_height} must match layout height {}",
            node.height
        );
    }
}

#[test]
fn flowchart_folder_shape_renders_aliases_and_clips_edges_to_the_tab_polygon() {
    let _session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let text = r#"flowchart TB
A@{ shape: folder, label: "src" } --> B@{ shape: directory, label: "include" }
"#;
    let engine = Engine::new();
    let parsed = block_on(engine.parse_diagram_for_render_model(text, ParseOptions::default()))
        .expect("parse ok")
        .expect("diagram detected");

    let layout = layout_flowchart_render_model(
        parsed.clone(),
        &LayoutOptions::default(),
        RenderEnvironment::deterministic()
            .begin_session()
            .expect("begin layout session"),
    )
    .expect("layout ok");
    let nodes = |id: &str| {
        layout
            .nodes
            .iter()
            .find(|node| node.id == id)
            .unwrap_or_else(|| panic!("node {id}"))
    };

    let svg = render_flowchart_artifact(
        parsed,
        &LayoutOptions::default(),
        _session,
        &SvgRenderOptions::default(),
    )
    .expect("render svg");
    let document = roxmltree::Document::parse(&svg).expect("valid Flowchart SVG");
    for id in ["A", "B"] {
        let node = document
            .descendants()
            .find(|element| {
                element.has_tag_name("g")
                    && element.attribute("data-id") == Some(id)
                    && element.attribute("data-et") == Some("node")
            })
            .unwrap_or_else(|| panic!("rendered node {id}: {svg}"));
        assert!(
            node.children().any(|child| {
                child.has_tag_name("path")
                    && child.attribute("class") == Some("basic label-container")
            }),
            "folder alias {id} must emit its path container: {svg}"
        );
    }

    let edge = layout.edges.first().expect("folder edge");
    let points = flowchart_svg_edge_data_points(&svg, &edge.id);
    assert!(
        points.len() >= 2,
        "folder edge must have endpoints: {points:?}"
    );
    let source = nodes("A");
    let target = nodes("B");
    assert!(
        (points[0].y - (source.y + source.height / 2.0)).abs() <= 1.0,
        "source endpoint should meet the folder body bottom: {points:?}"
    );
    assert!(
        points[points.len() - 1].y > target.y - target.height / 2.0 + 5.0,
        "target endpoint should use the folder body top below the tab: {points:?}"
    );

    let hand_drawn = render_flowchart_svg_from_text(
        r#"%%{init: {"look": "handDrawn"}}%%
flowchart TB
A@{ shape: folder, label: "src" }
"#,
    );
    assert!(
        hand_drawn.contains(r#"class="basic label-container""#),
        "handDrawn folder must retain the basic label-container contract: {hand_drawn}"
    );
}

#[test]
fn flowchart_mermaid_1172_object_shapes_render_their_native_chrome() {
    let text = r#"flowchart TB
A@{ shape: person, label: "person" } --> B@{ shape: bucket, label: "bucket" }
B --> C@{ shape: console, label: "console" }
C --> D@{ shape: browser, label: "browser" }
"#;
    let svg = render_flowchart_svg_from_text(text);
    let document = roxmltree::Document::parse(&svg).expect("valid Flowchart SVG");

    let node = |id: &str| {
        document
            .descendants()
            .find(|element| {
                element.has_tag_name("g")
                    && element.attribute("data-id") == Some(id)
                    && element.attribute("data-et") == Some("node")
            })
            .unwrap_or_else(|| panic!("rendered node {id}: {svg}"))
    };

    let person = node("A");
    assert!(person.children().any(|child| child.has_tag_name("g")
        && child.attribute("class") == Some("basic label-container")));
    assert!(
        person
            .descendants()
            .any(|child| child.has_tag_name("circle"))
    );
    assert!(person.descendants().any(|child| child.has_tag_name("rect")));

    let bucket = node("B");
    assert!(
        bucket
            .descendants()
            .any(|child| child.has_tag_name("ellipse"))
    );
    assert!(bucket.descendants().any(|child| child.has_tag_name("path")));

    let console = node("C");
    assert!(console.descendants().any(|child| {
        child.has_tag_name("text") && child.attribute("class") == Some("console-glyph")
    }));

    let browser = node("D");
    assert!(
        browser
            .descendants()
            .any(|child| child.has_tag_name("line"))
    );
    assert!(browser.descendants().any(|child| {
        child.has_tag_name("rect") && child.attribute("class") == Some("browser-address-bar")
    }));
    assert_eq!(
        browser
            .descendants()
            .filter(|child| child.has_tag_name("circle"))
            .count(),
        3
    );

    let edges = document
        .descendants()
        .filter(|element| {
            element.has_tag_name("path") && element.attribute("data-edge") == Some("true")
        })
        .count();
    assert_eq!(edges, 3, "all object-shape edges should render: {svg}");

    for shape in ["person", "bucket", "console", "browser"] {
        let source = format!(
            "%%{{init: {{\"look\": \"handDrawn\"}}}}%%\nflowchart TB\nA@{{ shape: {shape}, label: \"{shape}\" }}"
        );
        let hand_drawn = render_flowchart_svg_from_text(&source);
        let hand_document = roxmltree::Document::parse(&hand_drawn).expect("valid handDrawn SVG");
        let node = hand_document
            .descendants()
            .find(|element| {
                element.has_tag_name("g")
                    && element.attribute("data-id") == Some("A")
                    && element.attribute("data-et") == Some("node")
            })
            .unwrap_or_else(|| panic!("handDrawn node for {shape}: {hand_drawn}"));
        assert!(
            node.children().any(|child| {
                child.has_tag_name("g") && child.attribute("class") == Some("basic label-container")
            }),
            "handDrawn {shape} must retain its shape group: {hand_drawn}"
        );
        assert!(
            node.descendants().any(|child| {
                child.has_tag_name("path")
                    && child.attribute("fill") == Some("none")
                    && child.attribute("stroke-width") == Some("4")
                    && child.attribute("stroke-dasharray") == Some("0 0")
            }),
            "handDrawn {shape} must emit a RoughJS hachure fill sketch: {hand_drawn}"
        );
    }

    let folder_source = "%%{init: {\"look\": \"handDrawn\"}}%%\nflowchart TB\nA@{ shape: folder, label: \"folder\" }";
    let folder_svg = render_flowchart_svg_from_text(folder_source);
    let folder_document =
        roxmltree::Document::parse(&folder_svg).expect("valid handDrawn folder SVG");
    let folder_node = folder_document
        .descendants()
        .find(|element| {
            element.has_tag_name("g")
                && element.attribute("data-id") == Some("A")
                && element.attribute("data-et") == Some("node")
        })
        .expect("handDrawn folder node");
    assert!(folder_node.descendants().any(|child| {
        child.has_tag_name("path")
            && child.attribute("stroke-width") == Some("4")
            && child.attribute("stroke-dasharray") == Some("0 0")
    }));
}

#[test]
fn flowchart_object_shape_decorations_keep_theme_border_when_node_stroke_is_overridden() {
    let svg = render_flowchart_svg_from_text_with_engine(
        legacy_init_theme_compat_engine(),
        r##"%%{init: {"themeVariables": {"nodeBorder": "#00ff00"}}}%%
flowchart TB
A@{ shape: bucket, label: "bucket" } --> B@{ shape: console, label: "console" }
B --> C@{ shape: browser, label: "browser" }
style A stroke:#ff0000
style B stroke:#ff0000
style C stroke:#ff0000
"##,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid themed object-shape SVG");
    let node = |id: &str| {
        document
            .descendants()
            .find(|element| {
                element.has_tag_name("g")
                    && element.attribute("data-id") == Some(id)
                    && element.attribute("data-et") == Some("node")
            })
            .unwrap_or_else(|| panic!("rendered node {id}: {svg}"))
    };

    let bucket = node("A");
    let bucket_rim = bucket
        .descendants()
        .find(|element| element.has_tag_name("ellipse"))
        .expect("bucket rim");
    assert_eq!(
        bucket_rim.attribute("style"),
        Some("fill:none;stroke:#00ff00;stroke-width:1px")
    );

    let console = node("B");
    let console_glyph = console
        .descendants()
        .find(|element| element.attribute("class") == Some("console-glyph"))
        .expect("console glyph");
    assert_eq!(
        console_glyph.attribute("style"),
        Some("font-family:monospace;font-weight:bold;font-size:14px;fill:#00ff00")
    );

    let browser = node("C");
    let browser_accent = browser
        .descendants()
        .find(|element| element.has_tag_name("line"))
        .expect("browser toolbar line");
    assert_eq!(
        browser_accent.attribute("style"),
        Some("stroke:#00ff00;stroke-width:1px")
    );
    let address_bar = browser
        .descendants()
        .find(|element| element.attribute("class") == Some("browser-address-bar"))
        .expect("browser address bar");
    assert!(
        address_bar
            .attribute("style")
            .is_some_and(|style| style.contains("stroke:#00ff00")),
        "browser address bar must keep the theme border: {svg}"
    );
}

#[test]
fn flowchart_hand_drawn_triangle_uses_rough_fill_and_is_deterministic() {
    let source = r#"%%{init: {"look": "handDrawn", "handDrawnSeed": 1}}%%
flowchart TB
A@{ shape: triangle, label: "Extract" }
style A fill:#123456,stroke:#654321
"#;
    let svg = render_flowchart_svg_from_text(source);
    assert_eq!(
        svg,
        render_flowchart_svg_from_text(source),
        "handDrawn triangle output must be deterministic"
    );

    let document = roxmltree::Document::parse(&svg).expect("valid handDrawn triangle SVG");
    let node = document
        .descendants()
        .find(|element| {
            element.has_tag_name("g")
                && element.attribute("data-id") == Some("A")
                && element.attribute("data-et") == Some("node")
        })
        .expect("handDrawn triangle node");
    let shape_group = node
        .children()
        .find(|element| {
            element.has_tag_name("g") && element.attribute("class") == Some("outer-path")
        })
        .expect("handDrawn triangle shape group");
    let paths: Vec<_> = shape_group
        .descendants()
        .filter(|element| element.has_tag_name("path"))
        .collect();
    assert!(
        paths.iter().any(|path| {
            path.attribute("stroke") == Some("#123456")
                && path.attribute("stroke-width") == Some("4")
                && path.attribute("stroke-dasharray") == Some("0 0")
                && !path.attribute("d").unwrap_or_default().is_empty()
        }),
        "triangle must emit a non-empty RoughJS hachure fill path: {svg}"
    );
    assert!(
        paths.iter().any(|path| {
            path.attribute("stroke") == Some("#654321")
                && path
                    .attribute("stroke-width")
                    .and_then(|value| value.parse::<f64>().ok())
                    .is_some_and(|value| (value - 1.3).abs() <= 1e-6)
                && path.attribute("stroke-dasharray") == Some("0 0")
                && !path.attribute("d").unwrap_or_default().is_empty()
        }),
        "triangle must emit a non-empty RoughJS outline path: {svg}"
    );

    let classic_svg = render_flowchart_svg_from_text(
        r#"flowchart TB
A@{ shape: triangle, label: "Extract" }
style A fill:#123456,stroke:#654321
"#,
    );
    let classic_document = roxmltree::Document::parse(&classic_svg).expect("valid classic SVG");
    let classic_node = classic_document
        .descendants()
        .find(|element| {
            element.has_tag_name("g")
                && element.attribute("data-id") == Some("A")
                && element.attribute("data-et") == Some("node")
        })
        .expect("classic triangle node");
    assert!(
        classic_node.descendants().any(|path| {
            path.has_tag_name("path")
                && path.attribute("fill") == Some("#123456")
                && path.attribute("stroke") == Some("none")
                && !path.attribute("d").unwrap_or_default().is_empty()
        }),
        "classic triangle must retain a solid fill path: {classic_svg}"
    );
    assert!(
        !classic_node.descendants().any(|path| {
            path.has_tag_name("path") && path.attribute("stroke-width") == Some("4")
        }),
        "classic triangle must not use the handDrawn fill sketch: {classic_svg}"
    );
}

#[test]
fn flowchart_hand_drawn_triangle_preserves_geometry_for_non_hex_colors() {
    let source = r#"%%{init: {"theme": "default", "look": "handDrawn", "handDrawnSeed": 1}}%%
flowchart TB
A@{ shape: triangle, label: "Extract" }
style A fill:red,stroke:blue
"#;
    let svg = render_flowchart_svg_from_text(source);
    let hex_svg = render_flowchart_svg_from_text(
        &source.replace("fill:red,stroke:blue", "fill:#ff0000,stroke:#0000ff"),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid handDrawn triangle SVG");
    let hex_document = roxmltree::Document::parse(&hex_svg).expect("valid hex triangle SVG");
    let node = document
        .descendants()
        .find(|element| {
            element.has_tag_name("g")
                && element.attribute("data-id") == Some("A")
                && element.attribute("data-et") == Some("node")
        })
        .expect("handDrawn triangle node");
    let hex_node = hex_document
        .descendants()
        .find(|element| {
            element.has_tag_name("g")
                && element.attribute("data-id") == Some("A")
                && element.attribute("data-et") == Some("node")
        })
        .expect("hex handDrawn triangle node");
    let geometry = |node: roxmltree::Node<'_, '_>| {
        node.descendants()
            .filter(|element| element.has_tag_name("path"))
            .map(|path| {
                path.attribute("d")
                    .expect("triangle path geometry")
                    .to_owned()
            })
            .collect::<Vec<_>>()
    };
    let paths = geometry(node);
    assert_eq!(
        paths,
        geometry(hex_node),
        "triangle geometry must be independent of paint tokens"
    );
    assert_eq!(
        paths.len(),
        2,
        "triangle must retain fill and outline paths: {svg}"
    );
    assert!(paths.iter().all(|path| !path.is_empty()));
    assert!(
        node.descendants().any(|path| {
            path.has_tag_name("path")
                && path.attribute("stroke") == Some("red")
                && path.attribute("stroke-width") == Some("4")
                && path.attribute("stroke-dasharray") == Some("0 0")
        }),
        "triangle must preserve the named fill on its hachure path: {svg}"
    );
    assert!(
        node.descendants().any(|path| {
            path.has_tag_name("path")
                && path.attribute("stroke") == Some("blue")
                && path
                    .attribute("stroke-width")
                    .and_then(|width| width.parse::<f64>().ok())
                    .is_some_and(|width| (width - 1.3).abs() < 1e-6)
                && path.attribute("stroke-dasharray") == Some("0 0")
        }),
        "triangle must preserve the named stroke on its outline path: {svg}"
    );
}

#[test]
fn flowchart_hand_drawn_object_shapes_preserve_inline_style_and_geometry_for_non_hex_colors() {
    let styled = render_flowchart_svg_from_text(
        r#"%%{init: {"look": "handDrawn"}}%%
flowchart TB
A@{ shape: browser, label: "browser" }
style A opacity:0.4,stroke-linecap:round
"#,
    );
    let styled_document = roxmltree::Document::parse(&styled).expect("valid styled handDrawn SVG");
    let styled_group = styled_document
        .descendants()
        .find(|element| {
            element.has_tag_name("g")
                && element.attribute("data-id") == Some("A")
                && element.attribute("data-et") == Some("node")
        })
        .expect("styled handDrawn browser node");
    let shape_group = styled_group
        .children()
        .find(|element| {
            element.has_tag_name("g") && element.attribute("class") == Some("basic label-container")
        })
        .expect("styled browser shape group");
    let group_style = shape_group.attribute("style").unwrap_or_default();
    assert!(
        group_style.contains("opacity:0.4"),
        "style was not preserved: {styled}"
    );
    assert!(
        group_style.contains("stroke-linecap:round"),
        "style was not preserved: {styled}"
    );

    let source = r#"%%{init: {"theme": "default", "look": "handDrawn", "handDrawnSeed": 1}}%%
flowchart TB
A@{ shape: person, label: "person" }
style A fill:red,stroke:blue
"#;
    let svg = render_flowchart_svg_from_text(source);
    let hex_svg = render_flowchart_svg_from_text(
        &source.replace("fill:red,stroke:blue", "fill:#ff0000,stroke:#0000ff"),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid handDrawn person SVG");
    let hex_document = roxmltree::Document::parse(&hex_svg).expect("valid hex person SVG");
    let node = document
        .descendants()
        .find(|element| {
            element.has_tag_name("g")
                && element.attribute("data-id") == Some("A")
                && element.attribute("data-et") == Some("node")
        })
        .expect("handDrawn person node");
    let hex_node = hex_document
        .descendants()
        .find(|element| {
            element.has_tag_name("g")
                && element.attribute("data-id") == Some("A")
                && element.attribute("data-et") == Some("node")
        })
        .expect("hex handDrawn person node");
    let geometry = |node: roxmltree::Node<'_, '_>| {
        node.descendants()
            .filter(|element| element.has_tag_name("path"))
            .map(|path| {
                path.attribute("d")
                    .expect("person path geometry")
                    .to_owned()
            })
            .collect::<Vec<_>>()
    };
    let paths = geometry(node);
    assert_eq!(
        paths,
        geometry(hex_node),
        "person geometry must be independent of paint tokens"
    );
    assert_eq!(
        paths.len(),
        4,
        "person must retain body and head fill and outline paths: {svg}"
    );
    assert!(paths.iter().all(|path| !path.is_empty()));
    assert_eq!(
        node.descendants()
            .filter(|path| {
                path.has_tag_name("path")
                    && path.attribute("stroke") == Some("red")
                    && path.attribute("stroke-width") == Some("4")
                    && path.attribute("stroke-dasharray") == Some("0 0")
            })
            .count(),
        2,
        "person must preserve the named fill on both body and head hachure paths: {svg}"
    );
    assert_eq!(
        node.descendants()
            .filter(|path| {
                path.has_tag_name("path")
                    && path.attribute("stroke") == Some("blue")
                    && path
                        .attribute("stroke-width")
                        .and_then(|width| width.parse::<f64>().ok())
                        .is_some_and(|width| (width - 1.3).abs() < 1e-6)
                    && path.attribute("stroke-dasharray") == Some("0 0")
            })
            .count(),
        2,
        "person must preserve the named stroke on both body and head outline paths: {svg}"
    );
}

#[test]
fn flowchart_hand_drawn_datastore_uses_rough_fill_and_double_border_lines() {
    let source = r#"%%{init: {"look": "handDrawn", "handDrawnSeed": 1}}%%
flowchart TB
A@{ shape: datastore, label: "Store" }
style A fill:#123456,stroke:#654321
"#;
    let svg = render_flowchart_svg_from_text(source);
    let repeat = render_flowchart_svg_from_text(source);
    assert_eq!(
        svg, repeat,
        "handDrawn datastore output must be deterministic"
    );
    let document = roxmltree::Document::parse(&svg).expect("valid handDrawn datastore SVG");
    let node = document
        .descendants()
        .find(|element| {
            element.has_tag_name("g")
                && element.attribute("data-id") == Some("A")
                && element.attribute("data-et") == Some("node")
        })
        .expect("handDrawn datastore node");
    let shape_group = node
        .children()
        .find(|element| {
            element.has_tag_name("g") && element.attribute("class") == Some("basic label-container")
        })
        .expect("handDrawn datastore shape group");
    let paths: Vec<_> = shape_group
        .descendants()
        .filter(|element| element.has_tag_name("path"))
        .collect();
    assert!(
        paths.iter().any(|path| {
            path.attribute("stroke") == Some("#123456")
                && path.attribute("stroke-width") == Some("4")
                && path.attribute("stroke-dasharray") == Some("0 0")
                && !path.attribute("d").unwrap_or_default().is_empty()
        }),
        "datastore must emit a non-empty RoughJS hachure fill path: {svg}"
    );
    assert_eq!(
        paths
            .iter()
            .filter(|path| {
                path.attribute("stroke") == Some("#654321")
                    && path.attribute("stroke-width") == Some("1.3")
                    && path.attribute("stroke-dasharray") == Some("0 0")
                    && !path.attribute("d").unwrap_or_default().is_empty()
            })
            .count(),
        2,
        "datastore must emit rough top and bottom border lines: {svg}"
    );
}

#[test]
fn flowchart_hand_drawn_datastore_preserves_geometry_for_non_hex_colors() {
    let source = r#"%%{init: {"look": "handDrawn", "handDrawnSeed": 1}}%%
flowchart TB
A@{ shape: datastore, label: "Store" }
style A fill:red,stroke:blue
"#;
    let svg = render_flowchart_svg_from_text(source);
    let hex_svg = render_flowchart_svg_from_text(
        &source.replace("fill:red,stroke:blue", "fill:#ff0000,stroke:#0000ff"),
    );
    let document = roxmltree::Document::parse(&svg).expect("valid handDrawn datastore SVG");
    let hex_document = roxmltree::Document::parse(&hex_svg).expect("valid hex datastore SVG");
    let node = document
        .descendants()
        .find(|element| {
            element.has_tag_name("g")
                && element.attribute("data-id") == Some("A")
                && element.attribute("data-et") == Some("node")
        })
        .expect("handDrawn datastore node");
    let hex_node = hex_document
        .descendants()
        .find(|element| {
            element.has_tag_name("g")
                && element.attribute("data-id") == Some("A")
                && element.attribute("data-et") == Some("node")
        })
        .expect("hex handDrawn datastore node");
    let geometry = |node: roxmltree::Node<'_, '_>| {
        node.descendants()
            .filter(|element| element.has_tag_name("path"))
            .map(|path| {
                path.attribute("d")
                    .expect("datastore path geometry")
                    .to_owned()
            })
            .collect::<Vec<_>>()
    };
    let paths = geometry(node);
    assert_eq!(
        paths,
        geometry(hex_node),
        "datastore geometry must be independent of paint tokens"
    );
    assert_eq!(
        paths.len(),
        3,
        "datastore must retain fill and both border paths: {svg}"
    );
    assert!(paths.iter().all(|path| !path.is_empty()));
    assert!(
        node.descendants().any(|path| {
            path.has_tag_name("path")
                && path.attribute("stroke") == Some("red")
                && path.attribute("stroke-width") == Some("4")
                && path.attribute("stroke-dasharray") == Some("0 0")
        }),
        "datastore must preserve the named fill on its hachure path: {svg}"
    );
    assert_eq!(
        node.descendants()
            .filter(|path| {
                path.has_tag_name("path")
                    && path.attribute("stroke") == Some("blue")
                    && path.attribute("stroke-width") == Some("1.3")
                    && path.attribute("stroke-dasharray") == Some("0 0")
            })
            .count(),
        2,
        "datastore must preserve the named stroke on its top and bottom border paths: {svg}"
    );
}

#[test]
fn flowchart_hand_drawn_extended_shapes_emit_deterministic_hachure_paths() {
    let source = r#"%%{init: {"look": "handDrawn", "handDrawnSeed": 7}}%%
flowchart TB
A@{ shape: delay, label: "delay" }
B@{ shape: bow-rect, label: "bow" }
C@{ shape: curv-trap, label: "curved" }
D@{ shape: div-rect, label: "divided" }
E@{ shape: document, label: "document" }
F@{ shape: lined-document, label: "lined" }
G@{ shape: manual-file, label: "manual-file" }
H@{ shape: manual-input, label: "manual-input" }
I@{ shape: notch-pent, label: "notched" }
J@{ shape: odd, label: "odd" }
K@{ shape: paper-tape, label: "paper" }
L@{ shape: shaded-process, label: "shaded" }
M@{ shape: docs, label: "stacked-document" }
N@{ shape: st-rect, label: "stacked-rectangle" }
O@{ shape: tag-doc, label: "tagged-document" }
P@{ shape: tag-rect, label: "tagged-rectangle" }
Q@{ shape: win-pane, label: "window" }
R@{ shape: collate, label: "collate" }
style A fill:#123456,stroke:#654321
style B fill:#123456,stroke:#654321
style C fill:#123456,stroke:#654321
style D fill:#123456,stroke:#654321
style E fill:#123456,stroke:#654321
style F fill:#123456,stroke:#654321
style G fill:#123456,stroke:#654321
style H fill:#123456,stroke:#654321
style I fill:#123456,stroke:#654321
style J fill:#123456,stroke:#654321
style K fill:#123456,stroke:#654321
style L fill:#123456,stroke:#654321
style M fill:#123456,stroke:#654321
style N fill:#123456,stroke:#654321
style O fill:#123456,stroke:#654321
style P fill:#123456,stroke:#654321
style Q fill:#123456,stroke:#654321
style R fill:#123456,stroke:#654321
"#;
    let svg = render_flowchart_svg_from_text(source);
    assert_eq!(
        svg,
        render_flowchart_svg_from_text(source),
        "extended handDrawn shape output must be deterministic"
    );

    let document = roxmltree::Document::parse(&svg).expect("valid extended handDrawn SVG");
    for id in 'A'..='R' {
        let id = id.to_string();
        let node = document
            .descendants()
            .find(|element| {
                element.has_tag_name("g")
                    && element.attribute("data-id") == Some(id.as_str())
                    && element.attribute("data-et") == Some("node")
            })
            .unwrap_or_else(|| panic!("missing handDrawn node {id}: {svg}"));
        assert!(
            node.descendants().any(|path| {
                path.has_tag_name("path")
                    && path.attribute("stroke") == Some("#123456")
                    && path.attribute("stroke-width") == Some("4")
                    && path.attribute("stroke-dasharray") == Some("0 0")
                    && !path.attribute("d").unwrap_or_default().is_empty()
            }),
            "handDrawn node {id} must emit a non-empty hachure fill path: {svg}"
        );
        assert!(
            node.descendants().any(|path| {
                path.has_tag_name("path")
                    && path.attribute("stroke") == Some("#654321")
                    && !path.attribute("d").unwrap_or_default().is_empty()
            }),
            "handDrawn node {id} must emit a non-empty outline path: {svg}"
        );
    }
}

#[test]
fn flowchart_collapsed_subgraph_renders_as_one_leaf_and_redirects_boundary_edges() {
    let svg = render_flowchart_svg_from_text(
        r#"flowchart TD
subgraph one[My Group]
  A --> B
end
C --> A
one@{ view: collapsed }
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid collapsed Flowchart SVG");

    let collapsed = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("data-id") == Some("one")
                && node.attribute("data-et") == Some("node")
        })
        .unwrap_or_else(|| panic!("collapsed subgraph node: {svg}"));
    assert_eq!(collapsed.attribute("class"), Some("node"));
    assert!(collapsed.children().any(|child| {
        child.has_tag_name("rect")
            && child.attribute("class") == Some("basic label-container collapsed-group")
    }));
    assert_eq!(
        collapsed
            .children()
            .filter(|child| child.has_tag_name("circle"))
            .count(),
        3,
        "collapsed node must expose the ellipsis indicators"
    );
    assert!(collapsed.children().any(|child| {
        child.has_tag_name("line") && child.attribute("class") == Some("collapsed-separator")
    }));

    for hidden_id in ["A", "B"] {
        assert!(
            !document.descendants().any(|node| {
                node.has_tag_name("g")
                    && node.attribute("data-id") == Some(hidden_id)
                    && node.attribute("data-et") == Some("node")
            }),
            "hidden member {hidden_id} must not be emitted: {svg}"
        );
    }

    let edge = document
        .descendants()
        .find(|node| node.has_tag_name("path") && node.attribute("data-edge") == Some("true"))
        .expect("redirected boundary edge");
    assert_eq!(edge.attribute("data-id"), Some("L_C_A_0"));
    assert!(
        edge.attribute("d").is_some_and(|path| !path.is_empty()),
        "redirected boundary edge must retain a route: {svg}"
    );

    let hand_drawn_svg = render_flowchart_svg_from_text(
        r##"%%{init: {"look": "handDrawn", "handDrawnSeed": 7}}%%
flowchart TD
subgraph one[My Group]
  A --> B
end
one@{ view: collapsed }
"##,
    );
    let hand_drawn_document =
        roxmltree::Document::parse(&hand_drawn_svg).expect("valid handDrawn collapsed SVG");
    let hand_drawn_collapsed = hand_drawn_document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("data-id") == Some("one")
                && node.attribute("data-et") == Some("node")
        })
        .expect("handDrawn collapsed node");
    assert!(
        hand_drawn_collapsed.descendants().any(|element| {
            element.has_tag_name("path")
                && element.attribute("stroke") == Some("none")
                && element.attribute("fill").is_some_and(|fill| fill != "none")
                && !element.attribute("d").unwrap_or_default().is_empty()
        }),
        "handDrawn collapsed group must emit a non-empty solid fill: {hand_drawn_svg}"
    );
    assert!(
        hand_drawn_collapsed.descendants().any(|element| {
            element.has_tag_name("path")
                && element.attribute("fill") == Some("none")
                && element
                    .attribute("stroke")
                    .is_some_and(|stroke| stroke != "none")
                && element.attribute("stroke-dasharray") == Some("0 0")
                && !element.attribute("d").unwrap_or_default().is_empty()
        }),
        "handDrawn collapsed group must emit a non-empty outline: {hand_drawn_svg}"
    );
}

#[test]
fn flowchart_collapsed_nested_subgraphs_keep_only_the_outermost_node() {
    let svg = render_flowchart_svg_from_text(
        r#"flowchart TD
subgraph inner[Inner]
  A --> B
end
subgraph outer[Outer]
  inner
  C
end
D --> A
inner@{ view: collapsed }
outer@{ view: collapsed }
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid nested collapsed SVG");
    let node_ids = document
        .descendants()
        .filter(|node| node.has_tag_name("g") && node.attribute("data-et") == Some("node"))
        .filter_map(|node| node.attribute("data-id"))
        .collect::<Vec<_>>();
    assert!(node_ids.contains(&"outer"), "{svg}");
    assert!(!node_ids.contains(&"inner"), "{svg}");
    for hidden_id in ["A", "B", "C"] {
        assert!(
            !node_ids.iter().any(|id| *id == hidden_id),
            "hidden member {hidden_id} must not be emitted: {svg}"
        );
    }
    let edge = document
        .descendants()
        .find(|node| node.has_tag_name("path") && node.attribute("data-edge") == Some("true"))
        .expect("outer boundary edge");
    assert_eq!(edge.attribute("data-id"), Some("L_D_A_0"));
}

#[cfg(feature = "layout-elk")]
#[test]
fn flowchart_elk_collapsed_subgraph_hides_members_before_layout() {
    let svg = render_flowchart_svg_from_text(
        r#"---
config:
  layout: elk
---
flowchart TD
subgraph one[My Group]
  A --> B
end
C --> A
one@{ view: collapsed }
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid ELK collapsed SVG");
    assert!(document.descendants().any(|node| {
        node.has_tag_name("g")
            && node.attribute("data-id") == Some("one")
            && node.attribute("data-et") == Some("node")
    }));
    for hidden_id in ["A", "B"] {
        assert!(
            !document.descendants().any(|node| {
                node.has_tag_name("g")
                    && node.attribute("data-id") == Some(hidden_id)
                    && node.attribute("data-et") == Some("node")
            }),
            "ELK hidden member {hidden_id} must not be emitted: {svg}"
        );
    }
    assert!(document.descendants().any(|node| {
        node.has_tag_name("path")
            && node.attribute("data-edge") == Some("true")
            && node.attribute("data-id") == Some("L_C_A_0")
    }));
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
    for html_labels in [true, false] {
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let text = "flowchart TB\nA@{ shape: hourglass, label: \"Hourglass label\" }\n";
        let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "htmlLabels": html_labels,
            "flowchart": { "htmlLabels": html_labels },
        })));
        let parsed = block_on(engine.parse_diagram_for_render_model(text, ParseOptions::default()))
            .expect("parse ok")
            .expect("diagram detected");
        let svg = render_flowchart_artifact(
            parsed,
            &LayoutOptions::default(),
            session,
            &SvgRenderOptions::default(),
        )
        .expect("render svg");
        if html_labels {
            assert!(
                svg.contains(r#"<span class="nodeLabel markdown-node-label"></span>"#),
                "hourglass must keep the markdown class on its cleared label: {svg}"
            );
        }
        let document = roxmltree::Document::parse(&svg).expect("valid SVG");
        let label = document
            .descendants()
            .find(|node| node.has_tag_name("g") && node.attribute("class") == Some("label"))
            .expect("hourglass label group");
        assert_eq!(
            label.attribute("transform"),
            Some("translate(0,0)"),
            "clearing the label must also clear its measured offset (HTML={html_labels})"
        );
        if html_labels {
            let bbox = label
                .descendants()
                .find(|node| node.has_tag_name("foreignObject"))
                .unwrap();
            assert_eq!(bbox.attribute("width"), Some("0"));
            assert_eq!(bbox.attribute("height"), Some("0"));
        }
    }
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
        svg.contains(r#"<linearGradient id="flowchart_theme_gradient-merman-flowchart-document-gradient-root" gradientUnits="objectBoundingBox" x1="0%" y1="0%" x2="100%" y2="0%">"#),
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

#[cfg(feature = "layout-elk")]
#[test]
fn flowchart_elk_markdown_group_titles_paint_at_the_final_frame_width() {
    let title = "**alpha** beta *gamma* delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma tau upsilon phi chi psi omega";
    let expected_text = "alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma tau upsilon phi chi psi omega";
    for html_labels in [false, true] {
        for wrapping_width in [60, 240] {
            let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                "layout": "elk", "look": "classic", "htmlLabels": html_labels,
                "flowchart": { "htmlLabels": html_labels, "wrappingWidth": wrapping_width, "minNodeWidth": 0 }
            })));
            let source =
                format!("flowchart TB\nsubgraph G[\"`{title}`\"]\nA[x]\nend\nG --> Outside[out]\n");
            let parsed = engine
                .parse_diagram_for_render_model_sync(&source, ParseOptions::default())
                .unwrap()
                .unwrap();
            let session = RenderEnvironment::deterministic().begin_session().unwrap();
            let artifact = family::prepare(parsed, &LayoutOptions::default(), session).unwrap();
            let layout: FlowchartLayout = serde_json::from_value(
                artifact.layout_json().unwrap()["layout"]["FlowchartV2"].clone(),
            )
            .unwrap();
            let frame = layout
                .clusters
                .iter()
                .find(|cluster| cluster.id == "G")
                .unwrap();
            let rendered = artifact
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .unwrap();
            let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
            let cluster = doc
                .descendants()
                .find(|node| node.has_tag_name("g") && node.attribute("class") == Some("cluster"))
                .unwrap();
            let rect = cluster
                .children()
                .find(|node| node.has_tag_name("rect"))
                .unwrap();
            let rect_left = rect.attribute("x").unwrap().parse::<f64>().unwrap();
            let rect_width = rect.attribute("width").unwrap().parse::<f64>().unwrap();
            let label = cluster
                .descendants()
                .find(|node| node.attribute("class") == Some("cluster-label"))
                .unwrap();
            let label_left = svg_translate_values(label.attribute("transform").unwrap())[0];
            let context = format!("htmlLabels={html_labels}, wrappingWidth={wrapping_width}");
            assert!((rect_width - frame.width).abs() < 1e-3, "{context}");
            assert!(
                (rect_width - 200.0).abs() > 1.0,
                "{context}: exercise a non-default paint width"
            );
            if html_labels {
                let foreign_object = label
                    .descendants()
                    .find(|node| node.has_tag_name("foreignObject"))
                    .unwrap();
                let width = foreign_object
                    .attribute("width")
                    .unwrap()
                    .parse::<f64>()
                    .unwrap();
                let height = foreign_object
                    .attribute("height")
                    .unwrap()
                    .parse::<f64>()
                    .unwrap();
                let div = foreign_object
                    .descendants()
                    .find(|node| node.has_tag_name("div"))
                    .unwrap();
                let style = div.attribute("style").unwrap();
                let max_width = style
                    .split(';')
                    .find_map(|declaration| {
                        declaration.trim().strip_prefix("max-width:").map(|value| {
                            value.trim().trim_end_matches("px").parse::<f64>().unwrap()
                        })
                    })
                    .unwrap();
                assert!((max_width - rect_width).abs() < 1e-3, "{context}: {style}");
                assert!(
                    (width - rect_width).abs() < 1e-3,
                    "{context}: wrapped HTML width"
                );
                assert!(height > 24.0, "{context}: long title must remain wrapped");
                assert!(
                    (label_left + width / 2.0 - rect_left - rect_width / 2.0).abs() < 1e-3,
                    "{context}: painted HTML label must remain centered"
                );
                let text = div
                    .descendants()
                    .filter(|node| node.is_text())
                    .filter_map(|node| node.text())
                    .collect::<String>();
                assert_eq!(text.trim(), expected_text, "{context}: preserve title text");
                assert!(
                    div.descendants().any(|node| node.has_tag_name("strong")),
                    "{context}"
                );
                assert!(
                    div.descendants().any(|node| node.has_tag_name("em")),
                    "{context}"
                );
            } else {
                assert!(
                    !label
                        .descendants()
                        .any(|node| node.has_tag_name("foreignObject")),
                    "{context}"
                );
                let rows = label
                    .descendants()
                    .filter(|node| {
                        node.has_tag_name("tspan")
                            && node.attribute("class").is_some_and(|value| {
                                value
                                    .split_whitespace()
                                    .any(|class| class == "text-outer-tspan")
                            })
                    })
                    .map(|row| {
                        row.descendants()
                            .filter(|node| node.is_text())
                            .filter_map(|node| node.text())
                            .collect::<String>()
                    })
                    .collect::<Vec<_>>();
                assert!(
                    rows.len() > 1,
                    "{context}: final frame must wrap SVG Markdown"
                );
                assert_eq!(
                    rows.join(" "),
                    expected_text,
                    "{context}: preserve wrapped title words"
                );
                assert!(
                    label_left >= rect_left - 1e-3 && label_left < rect_left + rect_width / 2.0,
                    "{context}: wrapped SVG label must fit inside the frame"
                );
                assert!(
                    label
                        .descendants()
                        .any(|node| node.attribute("font-weight") == Some("bold")),
                    "{context}"
                );
                assert!(
                    label
                        .descendants()
                        .any(|node| node.attribute("font-style") == Some("italic")),
                    "{context}"
                );
            }
        }
    }
}

#[test]
fn duplicate_subgraph_vertex_css_updates_the_canonical_cluster() {
    let svg = render_flowchart_svg_from_text(concat!(
        "flowchart TD\n",
        "classDef hot stroke:#123456\n",
        "subgraph X[First title]\n  A\nend\n",
        "subgraph X[Second title]\n  B\nend\n",
        "style X fill:#010203\n",
        "class X hot\n",
    ));
    let document = roxmltree::Document::parse(&svg).expect("valid Flowchart SVG");
    let cluster = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("data-id") == Some("X")
                && node.attribute("data-et") == Some("cluster")
                && node.attribute("class").is_some_and(|class| {
                    class.split_ascii_whitespace().any(|part| part == "cluster")
                })
        })
        .expect("canonical X cluster");
    let classes = cluster
        .attribute("class")
        .expect("cluster class attribute")
        .split_ascii_whitespace()
        .collect::<Vec<_>>();
    let shape_style = cluster
        .children()
        .find(|node| node.has_tag_name("rect") || node.has_tag_name("path"))
        .and_then(|node| node.attribute("style"))
        .unwrap_or_default();

    assert!(classes.contains(&"hot"), "{svg}");
    assert!(shape_style.contains("#010203"), "{svg}");
    assert!(shape_style.contains("#123456"), "{svg}");
}

#[test]
fn duplicate_subgraphs_share_collapsed_metadata_before_and_after_redeclaration() {
    for backend in ["dagre", "elk"] {
        if backend == "elk" && !cfg!(feature = "layout-elk") {
            continue;
        }
        for between in [false, true] {
            let collapsed = "X@{ view: collapsed }\n";
            let source = format!(
                "---\nconfig:\n  layout: {backend}\n---\nflowchart TB\nsubgraph X[First]\nA\nend\n{}subgraph X[Second]\nB\nend\n{}C --> A\nC --> B\n",
                if between { collapsed } else { "" },
                if between { "" } else { collapsed },
            );
            let svg = render_flowchart_svg_from_text(&source);
            let document = roxmltree::Document::parse(&svg).unwrap();
            let groups: Vec<_> = document
                .descendants()
                .filter(|node| node.has_tag_name("g") && node.attribute("data-id") == Some("X"))
                .collect();
            assert_eq!(groups.len(), 1, "{backend}, between={between}: {svg}");
            assert!(
                groups[0]
                    .children()
                    .any(|node| node.attribute("class")
                        == Some("basic label-container collapsed-group")),
                "{svg}"
            );
            for member in ["A", "B"] {
                assert!(
                    !document.descendants().any(|node| {
                        node.has_tag_name("g")
                            && node.attribute("data-id") == Some(member)
                            && node.attribute("data-et") == Some("node")
                    }),
                    "{backend}, between={between}: hidden member {member}: {svg}"
                );
            }
            assert_eq!(
                document
                    .descendants()
                    .filter(|node| node.has_tag_name("path")
                        && node.attribute("data-edge") == Some("true"))
                    .count(),
                2,
                "{svg}"
            );
        }
    }
}

#[test]
fn flowchart_svg_uses_parser_owned_same_id_group_css_in_statement_order() {
    let cases = [
        (
            concat!(
                "flowchart TD\n",
                "classDef base stroke:#00f\n",
                "subgraph G\n  A\nend\n",
                "class G base\n",
                "style G fill:#f00\n",
            ),
            false,
        ),
        (
            concat!(
                "flowchart TD\n",
                "classDef base stroke:#00f\n",
                "subgraph G\n  A\nend\n",
                "style G fill:#f00\n",
                "class G base\n",
            ),
            true,
        ),
    ];

    for (source, expects_base_class) in cases {
        let svg = render_flowchart_svg_from_text(source);
        let document = roxmltree::Document::parse(&svg).expect("valid Flowchart SVG");
        let cluster = document
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && node.attribute("data-id") == Some("G")
                    && node.attribute("data-et") == Some("cluster")
                    && node.attribute("class").is_some_and(|class| {
                        class.split_ascii_whitespace().any(|part| part == "cluster")
                    })
            })
            .expect("subgraph G cluster");
        let classes = cluster
            .attribute("class")
            .expect("cluster class attribute")
            .split_ascii_whitespace()
            .collect::<Vec<_>>();
        let shape_style = cluster
            .children()
            .find(|node| node.has_tag_name("rect") || node.has_tag_name("path"))
            .and_then(|node| node.attribute("style"))
            .expect("cluster shape style");

        assert_eq!(classes.contains(&"base"), expects_base_class, "{svg}");
        assert!(shape_style.contains("fill:#f00 !important"), "{svg}");
        assert_eq!(
            shape_style.contains("stroke:#00f !important"),
            expects_base_class,
            "{svg}",
        );
    }
}

#[test]
fn flowchart_svg_edge_label_wraps_with_inherited_style_before_link_style_bbox() {
    fn source(extra: &str) -> String {
        format!(
            r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
A -->|alpha beta gamma delta epsilon zeta eta theta iota kappa lambda mu| B
{extra}
"#
        )
    }

    fn render_and_layout(source: &str) -> (FlowchartLayout, String) {
        let session = RenderEnvironment::deterministic()
            .begin_session()
            .expect("begin render session");
        let parsed =
            block_on(Engine::new().parse_diagram_for_render_model(source, ParseOptions::default()))
                .expect("parse ok")
                .expect("diagram detected");
        let layout =
            layout_flowchart_render_model(parsed.clone(), &LayoutOptions::default(), session)
                .expect("layout ok");
        let session = RenderEnvironment::deterministic()
            .begin_session()
            .expect("begin svg session");
        let svg = render_flowchart_artifact(
            parsed,
            &LayoutOptions::default(),
            session,
            &SvgRenderOptions::default(),
        )
        .expect("render svg");
        (layout, svg)
    }

    let (base_layout, base_svg) = render_and_layout(&source(""));
    let (styled_layout, styled_svg) = render_and_layout(&source(
        "linkStyle default font-size:32px\nlinkStyle 0 font-size:24px",
    ));

    let base_label = base_layout
        .edges
        .iter()
        .find_map(|edge| edge.label.as_ref())
        .expect("base edge label");
    let styled_label = styled_layout
        .edges
        .iter()
        .find_map(|edge| edge.label.as_ref())
        .expect("styled edge label");
    assert!(styled_label.height > base_label.height + 1e-6);

    fn edge_label_rows(svg: &str) -> usize {
        let document = roxmltree::Document::parse(svg).expect("valid SVG");
        let group = document
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && node.attribute("class") == Some("label")
                    && node.attribute("data-id").is_some()
            })
            .expect("edge label group");
        group
            .descendants()
            .filter(|node| {
                node.has_tag_name("tspan")
                    && node
                        .attribute("class")
                        .is_some_and(|class| class.split_ascii_whitespace().any(|c| c == "row"))
            })
            .count()
    }

    let base_rows = edge_label_rows(&base_svg);
    let styled_rows = edge_label_rows(&styled_svg);
    assert!(base_rows > 1, "fixture must wrap: {base_svg}");
    assert_eq!(
        styled_rows, base_rows,
        "linkStyle must not change wrapping: {styled_svg}"
    );
    let styled_document = roxmltree::Document::parse(&styled_svg).expect("valid styled SVG");
    let edge_text_style = styled_document
        .descendants()
        .find(|node| node.has_tag_name("text") && node.attribute("style").is_some())
        .and_then(|node| node.attribute("style"))
        .expect("styled edge text");
    assert!(
        edge_text_style.contains("font-size:24px !important"),
        "expected per-edge style to override default style: {styled_svg}"
    );
    assert!(
        !edge_text_style.contains("font-size:32px !important"),
        "default style must not override the per-edge style: {styled_svg}"
    );
}

#[test]
fn flowchart_and_swimlane_edge_label_padding_changes_layout_and_terminal_content_box() {
    const PADDING: InsetsPx = InsetsPx {
        top: 3.0,
        right: 7.0,
        bottom: 5.0,
        left: 11.0,
    };

    fn prepare_control(source: &str) -> family::FamilyRenderArtifact {
        let parsed =
            block_on(Engine::new().parse_diagram_for_render_model(source, ParseOptions::default()))
                .expect("parse control Flowchart family")
                .expect("detect control Flowchart family");
        family::prepare(
            parsed,
            &LayoutOptions::default(),
            RenderEnvironment::deterministic()
                .begin_session()
                .expect("begin control Flowchart session"),
        )
        .expect("prepare control Flowchart family")
    }

    fn parse_number(node: roxmltree::Node<'_, '_>, attribute: &str) -> f64 {
        node.attribute(attribute)
            .unwrap_or_else(|| panic!("missing {attribute} on {}", node.tag_name().name()))
            .parse::<f64>()
            .unwrap_or_else(|_| panic!("numeric {attribute} on {}", node.tag_name().name()))
    }

    fn parse_translate(node: roxmltree::Node<'_, '_>) -> (f64, f64) {
        let transform = node.attribute("transform").expect("label transform");
        let values = transform
            .strip_prefix("translate(")
            .and_then(|value| value.strip_suffix(')'))
            .expect("translate transform")
            .split([',', ' '])
            .filter(|value| !value.is_empty())
            .map(|value| value.parse::<f64>().expect("numeric translate component"))
            .collect::<Vec<_>>();
        assert_eq!(values.len(), 2, "{transform}");
        (values[0], values[1])
    }

    fn assert_close(actual: f64, expected: f64, context: &str) {
        assert!(
            (actual - expected).abs() <= 1.0e-6,
            "{context}: expected {expected}, got {actual}"
        );
    }

    fn assert_padded_html_label(
        svg: &str,
        label_group: roxmltree::Node<'_, '_>,
        total_width: f64,
        total_height: f64,
        content_width: f64,
        content_height: f64,
    ) {
        let outer = label_group.parent().expect("outer edge-label group");
        let background = outer
            .children()
            .find(|node| node.has_tag_name("rect") && node.attribute("class") == Some("background"))
            .unwrap_or_else(|| panic!("theme-owned edge-label background: {svg}"));
        assert_close(
            parse_number(background, "x"),
            -total_width / 2.0,
            "background x",
        );
        assert_close(
            parse_number(background, "y"),
            -total_height / 2.0,
            "background y",
        );
        assert_close(
            parse_number(background, "width"),
            total_width,
            "background width",
        );
        assert_close(
            parse_number(background, "height"),
            total_height,
            "background height",
        );

        let (content_x, content_y) = parse_translate(label_group);
        assert_close(
            content_x,
            -total_width / 2.0 + f64::from(PADDING.left),
            "content x",
        );
        assert_close(
            content_y,
            -total_height / 2.0 + f64::from(PADDING.top),
            "content y",
        );
        let foreign_object = label_group
            .descendants()
            .find(|node| node.has_tag_name("foreignObject"))
            .expect("edge-label foreignObject");
        assert_close(
            parse_number(foreign_object, "width"),
            content_width,
            "content width",
        );
        assert_close(
            parse_number(foreign_object, "height"),
            content_height,
            "content height",
        );
    }

    let theme = edge_label_padding_theme(PADDING);
    for (family_name, source, layout_key) in [
        (
            "Flowchart",
            "flowchart LR\nA padded@-->|edge label padding| B\n",
            "FlowchartV2",
        ),
        (
            "Swimlane",
            "swimlane-beta LR\nA padded@-->|edge label padding| B\n",
            "SwimlaneDiagram",
        ),
    ] {
        let control = prepare_control(source);
        let control_projection = control.layout_json().expect("control layout projection");
        let control_svg = control
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("render control SVG");
        let themed = prepare_flowchart_family_with_theme(source, &theme);
        let themed_projection = themed.layout_json().expect("themed layout projection");
        let themed_svg = themed
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("render themed SVG");

        let (control_width, control_height, themed_width, themed_height) =
            if layout_key == "FlowchartV2" {
                let control_layout: FlowchartLayout =
                    serde_json::from_value(control_projection["layout"][layout_key].clone())
                        .expect("control Flowchart layout");
                let themed_layout: FlowchartLayout =
                    serde_json::from_value(themed_projection["layout"][layout_key].clone())
                        .expect("themed Flowchart layout");
                let control_label = control_layout.edges[0]
                    .label
                    .as_ref()
                    .expect("control Flowchart edge label");
                let themed_label = themed_layout.edges[0]
                    .label
                    .as_ref()
                    .expect("themed Flowchart edge label");
                (
                    control_label.width,
                    control_label.height,
                    themed_label.width,
                    themed_label.height,
                )
            } else {
                let control_layout: SwimlaneLayout =
                    serde_json::from_value(control_projection["layout"][layout_key].clone())
                        .expect("control Swimlane layout");
                let themed_layout: SwimlaneLayout =
                    serde_json::from_value(themed_projection["layout"][layout_key].clone())
                        .expect("themed Swimlane layout");
                let control_label = control_layout
                    .nodes
                    .iter()
                    .find(|node| node.is_edge_label)
                    .expect("control Swimlane edge label");
                let themed_label = themed_layout
                    .nodes
                    .iter()
                    .find(|node| node.is_edge_label)
                    .expect("themed Swimlane edge label");
                (
                    control_label.label_width,
                    control_label.label_height,
                    themed_label.label_width,
                    themed_label.label_height,
                )
            };

        assert_close(
            themed_width - control_width,
            f64::from(PADDING.left + PADDING.right),
            &format!("{family_name} padded layout width"),
        );
        assert_close(
            themed_height - control_height,
            f64::from(PADDING.top + PADDING.bottom),
            &format!("{family_name} padded layout height"),
        );

        let document = roxmltree::Document::parse(themed_svg.svg())
            .unwrap_or_else(|error| panic!("valid themed {family_name} SVG: {error}"));
        let label_group = if family_name == "Flowchart" {
            document
                .descendants()
                .find(|node| {
                    node.has_tag_name("g")
                        && node.attribute("class") == Some("label")
                        && node.attribute("data-id") == Some("padded")
                })
                .expect("themed Flowchart label group")
        } else {
            let outer = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("g")
                        && node.attribute("data-id") == Some("padded")
                        && node.attribute("data-et") == Some("edge-label")
                })
                .expect("themed Swimlane edge-label group");
            outer
                .children()
                .find(|node| {
                    node.has_tag_name("g")
                        && node
                            .attribute("class")
                            .is_some_and(|classes| classes.split_whitespace().any(|c| c == "label"))
                })
                .expect("themed Swimlane content label group")
        };
        assert_padded_html_label(
            themed_svg.svg(),
            label_group,
            themed_width,
            themed_height,
            control_width,
            control_height,
        );
        assert!(!control_svg.svg().contains(r#"class="background" x="#));
    }
}

#[test]
fn flowchart_and_swimlane_svg_edge_label_padding_preserves_the_inner_content_box() {
    const PADDING: InsetsPx = InsetsPx {
        top: 3.0,
        right: 7.0,
        bottom: 5.0,
        left: 11.0,
    };

    fn parse_number(node: roxmltree::Node<'_, '_>, attribute: &str) -> f64 {
        node.attribute(attribute)
            .unwrap_or_else(|| panic!("missing {attribute} on {}", node.tag_name().name()))
            .parse::<f64>()
            .unwrap_or_else(|_| panic!("numeric {attribute} on {}", node.tag_name().name()))
    }

    fn parse_translate(node: roxmltree::Node<'_, '_>) -> (f64, f64) {
        let transform = node.attribute("transform").expect("label transform");
        let values = transform
            .strip_prefix("translate(")
            .and_then(|value| value.strip_suffix(')'))
            .expect("translate transform")
            .split([',', ' '])
            .filter(|value| !value.is_empty())
            .map(|value| value.parse::<f64>().expect("numeric translate component"))
            .collect::<Vec<_>>();
        assert_eq!(values.len(), 2, "{transform}");
        (values[0], values[1])
    }

    fn assert_close(actual: f64, expected: f64, context: &str) {
        assert!(
            (actual - expected).abs() <= 1.0e-6,
            "{context}: expected {expected}, got {actual}"
        );
    }

    let theme = edge_label_padding_theme(PADDING);
    for (family_name, source, layout_key) in [
        (
            "Flowchart",
            r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
A padded@-->|edge label padding| B
"#,
            "FlowchartV2",
        ),
        (
            "Swimlane",
            r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
swimlane-beta LR
A padded@-->|edge label padding| B
"#,
            "SwimlaneDiagram",
        ),
    ] {
        let themed = prepare_flowchart_family_with_theme(source, &theme);
        let projection = themed.layout_json().expect("themed layout projection");
        let (total_width, total_height) = if layout_key == "FlowchartV2" {
            let layout: FlowchartLayout =
                serde_json::from_value(projection["layout"][layout_key].clone())
                    .expect("themed Flowchart layout");
            let label = layout.edges[0]
                .label
                .as_ref()
                .expect("themed Flowchart edge label");
            (label.width, label.height)
        } else {
            let layout: SwimlaneLayout =
                serde_json::from_value(projection["layout"][layout_key].clone())
                    .expect("themed Swimlane layout");
            let label = layout
                .nodes
                .iter()
                .find(|node| node.is_edge_label)
                .expect("themed Swimlane edge label");
            (label.label_width, label.label_height)
        };
        let svg = themed
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("render themed SVG");
        assert!(
            !svg.svg().contains("foreignObject"),
            "{family_name} must keep the SVG-label route: {}",
            svg.svg()
        );

        let document = roxmltree::Document::parse(svg.svg())
            .unwrap_or_else(|error| panic!("valid themed {family_name} SVG: {error}"));
        let (label_group, background) = if family_name == "Flowchart" {
            let label_group = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("g")
                        && node.attribute("class") == Some("label")
                        && node.attribute("data-id") == Some("padded")
                })
                .expect("themed Flowchart SVG label group");
            let background = label_group
                .descendants()
                .find(|node| {
                    node.has_tag_name("rect") && node.attribute("class") == Some("background")
                })
                .expect("themed Flowchart SVG label background");
            (label_group, background)
        } else {
            let outer = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("g")
                        && node.attribute("data-id") == Some("padded")
                        && node.attribute("data-et") == Some("edge-label")
                })
                .expect("themed Swimlane SVG edge-label group");
            let label_group = outer
                .children()
                .find(|node| {
                    node.has_tag_name("g")
                        && node.attribute("class").is_some_and(|classes| {
                            classes.split_whitespace().any(|class| class == "label")
                        })
                })
                .expect("themed Swimlane SVG content label group");
            let background = outer
                .children()
                .find(|node| {
                    node.has_tag_name("rect") && node.attribute("class") == Some("background")
                })
                .expect("themed Swimlane SVG label background");
            (label_group, background)
        };

        let (content_x, content_y) = parse_translate(label_group);
        assert_close(
            content_x,
            -total_width / 2.0 + f64::from(PADDING.left),
            &format!("{family_name} SVG content x"),
        );
        assert_close(
            content_y,
            -total_height / 2.0 + f64::from(PADDING.top),
            &format!("{family_name} SVG content y"),
        );
        assert_close(
            parse_number(background, "width"),
            total_width,
            &format!("{family_name} SVG background width"),
        );
        assert_close(
            parse_number(background, "height"),
            total_height,
            &format!("{family_name} SVG background height"),
        );
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn flowchart_elk_edge_label_padding_reaches_the_layout_graph_before_routing() {
    let source = r#"---
config:
  layout: elk
---
flowchart LR
A padded@-->|edge label padding| B
"#;
    let control_parsed =
        block_on(Engine::new().parse_diagram_for_render_model(source, ParseOptions::default()))
            .expect("parse control ELK Flowchart")
            .expect("detect control ELK Flowchart");
    let control = family::prepare(
        control_parsed,
        &LayoutOptions::default(),
        RenderEnvironment::deterministic()
            .begin_session()
            .expect("begin control ELK session"),
    )
    .expect("prepare control ELK Flowchart");
    let control_projection = control
        .layout_json()
        .expect("control ELK layout projection");
    let control_layout: FlowchartLayout =
        serde_json::from_value(control_projection["layout"]["FlowchartV2"].clone())
            .expect("control ELK Flowchart layout");

    let padding = InsetsPx {
        top: 3.0,
        right: 7.0,
        bottom: 5.0,
        left: 11.0,
    };
    let theme = edge_label_padding_theme(padding);
    let themed = prepare_flowchart_family_with_theme(source, &theme);
    let themed_projection = themed.layout_json().expect("themed ELK layout projection");
    let themed_layout: FlowchartLayout =
        serde_json::from_value(themed_projection["layout"]["FlowchartV2"].clone())
            .expect("themed ELK Flowchart layout");

    let control_label = control_layout.edges[0]
        .label
        .as_ref()
        .expect("control ELK edge label");
    let themed_label = themed_layout.edges[0]
        .label
        .as_ref()
        .expect("themed ELK edge label");
    assert!(
        ((themed_label.width - control_label.width) - f64::from(padding.left + padding.right))
            .abs()
            <= 1.0e-6,
        "ELK width: control={control_label:?}, themed={themed_label:?}"
    );
    assert!(
        ((themed_label.height - control_label.height) - f64::from(padding.top + padding.bottom))
            .abs()
            <= 1.0e-6,
        "ELK height: control={control_label:?}, themed={themed_label:?}"
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
fn flowchart_html_image_after_many_tags_matches_case_variants() {
    let spans = "<span>α</span>".repeat(128);
    let source = |image_tag: &str| format!("flowchart TB\nA[\"{spans}{image_tag}\"]\n");
    let lower = render_flowchart_svg_from_text(&source("<img src='https://example.com/x.svg'>"));
    let mixed = render_flowchart_svg_from_text(&source("<iMg src='https://example.com/x.svg'>"));
    assert_eq!(mixed, lower);
    let document = roxmltree::Document::parse(&mixed).expect("valid SVG with XHTML labels");
    let images = document
        .descendants()
        .filter(|node| node.has_tag_name("img"))
        .collect::<Vec<_>>();
    assert_eq!(images.len(), 1);
    assert_eq!(
        images[0].attribute("src"),
        Some("https://example.com/x.svg")
    );
    assert!(
        images[0]
            .attribute("style")
            .unwrap()
            .contains("width: 100%;")
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
        let node = document
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && node.attribute("data-id") == Some(id)
                    && node.attribute("data-et") == Some("node")
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
    assert_eq!(
        label.attribute("transform"),
        Some("translate(0,0)"),
        "{svg}"
    );
    let (_, _, background) = flowchart_svg_edge_label_geometry(&svg, "L_A_B_0");
    assert_eq!(background, [-2.0, -2.0, 4.0, 4.0], "{svg}");
    assert_flowchart_svg_edge_label_is_centered_and_contained(&svg, "L_A_B_0");
}

#[test]
fn flowchart_svg_edge_label_background_is_centered_and_contained() {
    let svg = render_flowchart_svg_from_text(
        r#"---
config:
  htmlLabels: false
---
flowchart LR
  A -->|ordinary edge label| B
"#,
    );

    let (_, translate, _) = flowchart_svg_edge_label_geometry(&svg, "L_A_B_0");
    assert_eq!(translate[0], 0.0, "{svg}");
    assert_flowchart_svg_edge_label_is_centered_and_contained(&svg, "L_A_B_0");
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
    // imageSquare.ts positions a top-labelled image at outerHeight / 2 - imageHeight.
    let document = roxmltree::Document::parse(&svg).expect("valid image SVG");
    let image = document
        .descendants()
        .find(|node| node.has_tag_name("image"))
        .expect("image");
    let expected_y = node.height / 2.0 - 60.0;
    assert_eq!(image.attribute("width"), Some("60"));
    assert_eq!(image.attribute("height"), Some("60"));
    assert_eq!(
        image.attribute("transform"),
        Some(format!("translate(-30,{expected_y})").as_str())
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
fn flowchart_html_plain_multiline_labels_preserve_source_indentation() {
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
        svg.contains("<p>First<br />      Second</p>"),
        "expected Mermaid nonMarkdownToHTML to preserve source whitespace for browser collapse: {svg}"
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
        svg.contains("<p>Multiline<br />     bar</p>"),
        "expected indented multiline node label to preserve source whitespace after its HTML line break: {svg}"
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
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(
        serde_json::json!({"layout": "dagre"}),
    ));
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
fn dagre_flowchart_nested_root_viewbox_includes_empty_subgraph_node() {
    let render = |text: &str| {
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let parsed = block_on(
            Engine::new()
                .with_site_config(MermaidConfig::from_value(
                    serde_json::json!({"layout": "dagre"}),
                ))
                .parse_diagram_for_render_model(text, ParseOptions::default()),
        )
        .expect("parse ok")
        .expect("diagram detected");
        render_flowchart_artifact(
            parsed,
            &LayoutOptions::default(),
            session,
            &SvgRenderOptions {
                diagram_id: Some(
                    "upstream_cypress_flowchart_v2_spec_57_handle_nested_subgraphs_with_outgoing_links_4_015"
                        .to_string(),
                ),
                ..Default::default()
            },
        )
        .expect("render svg")
    };
    let svg = render("flowchart LR\nsubgraph A\na -->b\nend\nsubgraph B\nb\nend\n");
    let without_empty_subgraph = render("flowchart LR\nsubgraph A\na -->b\nend\n");
    let viewbox = flowchart_svg_viewbox_values(&svg);
    let control_viewbox = flowchart_svg_viewbox_values(&without_empty_subgraph);

    assert!(
        viewbox[2] > control_viewbox[2] || viewbox[3] > control_viewbox[3],
        "the top-level empty subgraph node must expand the root bounds: with={viewbox:?}, without={control_viewbox:?}"
    );
    assert!(
        svg.contains(r#"data-id="B" data-et="node""#),
        "expected empty subgraph node to retain its semantic owner: {svg}"
    );
}

#[test]
fn dagre_flowchart_empty_subgraph_node_applies_inline_style() {
    let _session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .unwrap();
    let text = "flowchart TD\nsubgraph Empty\nend\nstyle Empty fill:#f00,stroke:#00f,color:#fff\n";
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(
        serde_json::json!({"layout": "dagre"}),
    ));
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
        svg.contains(r#"data-id="Empty" data-et="node""#),
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
fn flowchart_node_style_preserves_source_css_while_using_canonical_winners() {
    let svg = render_flowchart_svg_from_text(
        r"flowchart LR
A[Alpha]
style A FILL:#ef4444,stroke:#111827,f\69ll:#22c55e,stroke:#334155
",
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Flowchart SVG");
    let node = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("data-id") == Some("A")
                && node.attribute("data-et") == Some("node")
        })
        .unwrap_or_else(|| panic!("missing Flowchart node A: {svg}"));
    let shape = node
        .descendants()
        .find(|node| {
            node.has_tag_name("rect")
                && node.attribute("class").is_some_and(|class| {
                    let mut tokens = class.split_ascii_whitespace();
                    tokens.any(|token| token == "basic")
                        && class
                            .split_ascii_whitespace()
                            .any(|token| token == "label-container")
                })
        })
        .unwrap_or_else(|| panic!("missing Flowchart node A shape: {svg}"));

    assert_eq!(
        shape.attribute("style"),
        Some(r"FILL:#ef4444 !important;stroke:#334155 !important;f\69ll:#22c55e !important"),
        "source CSS spelling and order must survive the no-theme render path: {svg}"
    );
}

#[test]
fn flowchart_node_style_routes_label_keys_by_source_identity() {
    let svg = render_flowchart_svg_from_text(
        r"flowchart LR
A[Alpha]
style A COLOR:#ef4444,color:#22c55e,c\6flor:#2563eb
",
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Flowchart SVG");
    let node = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("data-id") == Some("A")
                && node.attribute("data-et") == Some("node")
        })
        .unwrap_or_else(|| panic!("missing Flowchart node A: {svg}"));
    let shape = node
        .descendants()
        .find(|node| {
            node.has_tag_name("rect")
                && node.attribute("class").is_some_and(|class| {
                    class.split_ascii_whitespace().any(|token| token == "basic")
                        && class
                            .split_ascii_whitespace()
                            .any(|token| token == "label-container")
                })
        })
        .unwrap_or_else(|| panic!("missing Flowchart node A shape: {svg}"));
    let label = node
        .descendants()
        .find(|node| {
            node.has_tag_name("span")
                && node.attribute("class").is_some_and(|class| {
                    class
                        .split_ascii_whitespace()
                        .any(|token| token == "nodeLabel")
                })
        })
        .unwrap_or_else(|| panic!("missing Flowchart node A label: {svg}"));

    assert_eq!(
        shape.attribute("style"),
        Some(r"COLOR:#ef4444 !important;c\6flor:#2563eb !important"),
        "only the exact pinned-Mermaid label keys may leave the node style channel: {svg}"
    );
    assert_eq!(
        label.attribute("style"),
        Some("color:#22c55e !important"),
        "the exact lowercase label key must retain its source value and position: {svg}"
    );
}

#[test]
fn flowchart_classdef_css_uses_cssom_property_cascade_with_safe_values() {
    let svg = render_flowchart_svg_from_text(
        r"flowchart LR
A[Alpha]
classDef hot FILL:#ef4444,stroke:#111827,f\69ll:#22c55e,filter:url(https://example.com/filter.svg#x)
class A hot
",
    );

    assert!(
        svg.contains(
            "#merman .hot&gt;*{stroke:rgb(17, 24, 39)!important;fill:rgb(34, 197, 94)!important;}"
        ),
        "generated classDef CSS must use the CSSOM winner identity and order: {svg}"
    );
    assert!(
        !svg.contains("example.com/filter.svg"),
        "the headless safe-value boundary intentionally remains narrower than browser CSSOM: {svg}"
    );
}

#[test]
fn flowchart_classdef_text_styles_preserve_pre_cssom_source_classification() {
    let svg = render_flowchart_svg_from_text(
        r"flowchart LR
A[Alpha]
classDef hot color:#ef4444,COLOR:#22c55e,--Br\61nd:#f59e0b,--fill-color:#2563eb
class A hot
",
    );

    assert!(
        svg.contains(
            "#merman .hot&gt;*{color:rgb(34, 197, 94)!important;--Brand:#f59e0b!important;--fill-color:#2563eb!important;}"
        ),
        "generated classDef shape CSS must use CSSOM property identity and spelling: {svg}"
    );
    assert!(
        svg.contains(
            "#merman .hot tspan{fill:rgb(239, 68, 68)!important;--bgFill-fill:#2563eb!important;}"
        ),
        "the tspan rule must come from Mermaid's source-classified lowercase color declaration: {svg}"
    );
}

#[test]
fn dagre_flowchart_empty_subgraph_svg_uses_configured_wrapping_width() {
    fn render(wrapping_width: usize) -> String {
        render_dagre_flowchart_svg_from_text(&format!(
            r#"%%{{init: {{"htmlLabels": false, "flowchart": {{"htmlLabels": false, "wrappingWidth": {wrapping_width}}}}}}}%%
flowchart TB
subgraph Empty["alpha beta gamma delta epsilon zeta eta theta"]
end
"#
        ))
    }

    fn rows_and_text(svg: &str) -> (usize, String) {
        let document = roxmltree::Document::parse(svg).expect("valid Flowchart SVG");
        let node = document
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && node.attribute("data-id") == Some("Empty")
                    && node.attribute("data-et") == Some("node")
                    && node.attribute("class").is_some_and(|class| {
                        class.split_ascii_whitespace().any(|part| part == "node")
                    })
            })
            .expect("empty subgraph node");
        let rows = node
            .descendants()
            .filter(|node| {
                node.has_tag_name("tspan")
                    && node.attribute("class").is_some_and(|class| {
                        class
                            .split_ascii_whitespace()
                            .any(|part| part == "text-outer-tspan")
                    })
            })
            .count();
        let text = node
            .descendants()
            .filter(|node| {
                node.has_tag_name("tspan")
                    && node.attribute("class").is_some_and(|class| {
                        class
                            .split_ascii_whitespace()
                            .any(|part| part == "text-inner-tspan")
                    })
            })
            .filter_map(|node| node.text())
            .map(str::trim)
            .collect::<Vec<_>>()
            .join(" ");
        (rows, text)
    }

    let narrow_svg = render(60);
    let wide_svg = render(240);
    let (narrow_rows, narrow_text) = rows_and_text(&narrow_svg);
    let (wide_rows, wide_text) = rows_and_text(&wide_svg);
    assert!(
        narrow_rows > wide_rows,
        "configured width must reach final empty-subgraph SVG wrapping: narrow={narrow_rows}, wide={wide_rows}"
    );
    assert_eq!(narrow_text, "alpha beta gamma delta epsilon zeta eta theta");
    assert_eq!(wide_text, narrow_text);
}

#[cfg(feature = "layout-elk")]
#[test]
fn flowchart_elk_final_subgraph_title_remains_unbounded_after_temporary_layout_wrapping() {
    fn render(wrapping_width: usize) -> String {
        render_flowchart_svg_from_text(&format!(
            r#"---
config:
  layout: elk
  htmlLabels: false
  flowchart:
    htmlLabels: false
    wrappingWidth: {wrapping_width}
---
flowchart TB
subgraph Group["alpha beta gamma delta epsilon zeta eta theta"]
  A[child]
end
"#
        ))
    }

    fn rows_and_text(svg: &str) -> (usize, String) {
        let document = roxmltree::Document::parse(svg).expect("valid ELK Flowchart SVG");
        let label = document
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && node.attribute("class").is_some_and(|class| {
                        class
                            .split_ascii_whitespace()
                            .any(|part| part == "cluster-label")
                    })
            })
            .expect("ELK cluster label");
        let rows = label
            .descendants()
            .filter(|node| {
                node.has_tag_name("tspan")
                    && node.attribute("class").is_some_and(|class| {
                        class
                            .split_ascii_whitespace()
                            .any(|part| part == "text-outer-tspan")
                    })
            })
            .count();
        let text = label
            .descendants()
            .filter_map(|node| node.text().filter(|_| node.is_text()))
            .collect::<String>();
        (rows, text)
    }

    for wrapping_width in [60, 240] {
        let svg = render(wrapping_width);
        let (rows, text) = rows_and_text(&svg);
        assert_eq!(rows, 1, "wrappingWidth={wrapping_width}: {svg}");
        assert_eq!(text, "alpha beta gamma delta epsilon zeta eta theta");
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn flowchart_elk_root_algorithms_execute_and_paint_edges() {
    // Rectpacking intentionally delegates to Box for unstackable equal rectangles.
    // One tall node makes this corpus distinguish their actual packing strategies.
    let mut geometries = Vec::new();
    for algorithm in [
        "elk",
        "elk.stress",
        "elk.force",
        "elk.mrtree",
        "elk.sporeOverlap",
        "elk.box",
        "elk.rectpacking",
    ] {
        let source = format!(
            "---\nconfig:\n  layout: {algorithm}\n  htmlLabels: false\n---\nflowchart TB\nA[Alpha<br/>one<br/>two<br/>three<br/>four<br/>five<br/>six] first@-->|first label| B[Beta]\nA second@-->|second label| C[Gamma]\n"
        );
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        let session = RenderEnvironment::deterministic().begin_session().unwrap();
        let plan = family::plan_render(&parsed, &session).unwrap();
        assert_eq!(
            plan.required_capabilities(),
            &[merman_render::RenderCapability::LayoutElk]
        );
        let artifact = family::prepare(parsed, &LayoutOptions::default(), session).unwrap();
        let layout: FlowchartLayout = serde_json::from_value(
            artifact.layout_json().unwrap()["layout"]["FlowchartV2"].clone(),
        )
        .unwrap();
        let geometry: Vec<_> = layout
            .nodes
            .iter()
            .map(|node| (node.x, node.y, node.width, node.height))
            .collect();
        assert!(
            !geometries.contains(&geometry),
            "{algorithm} unexpectedly reuses a preceding provider's geometry"
        );
        geometries.push(geometry);
        let rendered = artifact
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        let svg = rendered.svg();
        for id in ["first", "second"] {
            let points = flowchart_svg_edge_data_points(svg, id);
            assert!(points.len() >= 2, "{algorithm}: {id}");
            assert!(
                points
                    .iter()
                    .all(|point| point.x.is_finite() && point.y.is_finite())
            );
            let (anchor, _, _) = flowchart_svg_edge_label_geometry(svg, id);
            assert!(
                anchor.iter().all(|value| value.is_finite()),
                "{algorithm}: {id}"
            );
        }
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn flowchart_elk_unrouted_container_edges_are_clipped_and_labels_are_centered() {
    for algorithm in ["elk.box", "elk.rectpacking"] {
        let source = format!(
            "---\nconfig:\n  layout: elk\n  htmlLabels: false\n---\nflowchart TB\nsubgraph G[Container]\nA[Small] edge@-->|wide label sentinel| B[Wider target]\nend\nG@{{algorithm: {algorithm}}}\n"
        );
        let parsed = Engine::new()
            .parse_diagram_for_render_model_sync(&source, ParseOptions::default())
            .expect("parse")
            .expect("diagram");
        let layout = layout_flowchart_render_model(
            parsed.clone(),
            &LayoutOptions::default(),
            RenderEnvironment::deterministic().begin_session().unwrap(),
        )
        .unwrap();
        let raw_edge = layout.edges.iter().find(|edge| edge.id == "edge").unwrap();
        assert!(
            raw_edge.points.is_empty(),
            "the provider must remain unrouted"
        );
        let svg = render_flowchart_artifact(
            parsed,
            &LayoutOptions::default(),
            RenderEnvironment::deterministic().begin_session().unwrap(),
            &SvgRenderOptions::default(),
        )
        .unwrap();
        let points = flowchart_svg_edge_data_points(&svg, "edge");
        assert_eq!(points.len(), 2, "{algorithm}: {points:?}");
        for (point, id) in points.iter().zip(["A", "B"]) {
            let node = layout.nodes.iter().find(|node| node.id == id).unwrap();
            let dx = (point.x - node.x).abs();
            let dy = (point.y - node.y).abs();
            assert!(
                ((dx - node.width / 2.0).abs() < 1e-6 && dy <= node.height / 2.0 + 1e-6)
                    || ((dy - node.height / 2.0).abs() < 1e-6 && dx <= node.width / 2.0 + 1e-6),
                "{algorithm}: {id}, {point:?}, {node:?}"
            );
        }
        let (label, _, _) = flowchart_svg_edge_label_geometry(&svg, "edge");
        assert!((label[0] - (points[0].x + points[1].x) / 2.0).abs() < 1e-6);
        assert!((label[1] - (points[0].y + points[1].y) / 2.0).abs() < 1e-6);
        let document = roxmltree::Document::parse(&svg).unwrap();
        let path = document
            .descendants()
            .find(|node| node.has_tag_name("path") && node.attribute("data-id") == Some("edge"))
            .unwrap();
        let d = path.attribute("d").unwrap();
        assert!(
            d.contains('L') && !d.contains('C') && !d.contains('Q'),
            "{d}"
        );
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn flowchart_elk_parallel_edge_labels_remain_bound_to_explicit_ids() {
    let svg = render_flowchart_svg_from_text(
        r#"---
config:
  layout: elk
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
flowchart LR
A e1@-->|first owner sentinel| B
A e2@-->|second owner sentinel| B
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid ELK Flowchart SVG");

    for (edge_id, expected) in [
        ("e1", "first owner sentinel"),
        ("e2", "second owner sentinel"),
    ] {
        let labels = document
            .descendants()
            .filter(|node| {
                node.has_tag_name("g")
                    && node.attribute("data-id") == Some(edge_id)
                    && node.attribute("class").is_some_and(|class| {
                        class.split_ascii_whitespace().any(|part| part == "label")
                    })
            })
            .collect::<Vec<_>>();
        assert_eq!(labels.len(), 1, "edge={edge_id}: {svg}");
        let text = labels[0]
            .descendants()
            .filter_map(|node| node.text().filter(|_| node.is_text()))
            .collect::<String>();
        assert_eq!(text, expected, "edge={edge_id}: {svg}");
    }
}

#[test]
fn swimlane_svg_edge_label_uses_only_the_first_concatenated_mermaid_style() {
    let svg = render_flowchart_svg_from_text(
        r#"---
config:
  htmlLabels: false
  flowchart:
    htmlLabels: false
---
swimlane-beta LR
A styled@-->|styled edge label| B
linkStyle default font-size:24px,font-weight:bold
linkStyle 0 font-size:12px,font-style:italic
"#,
    );
    let document = roxmltree::Document::parse(&svg).expect("valid Swimlane SVG");
    let label = document
        .descendants()
        .find(|node| {
            node.has_tag_name("g")
                && node.attribute("class").is_some_and(|class| {
                    let classes = class.split_ascii_whitespace().collect::<Vec<_>>();
                    classes.contains(&"label") && classes.contains(&"edgeLabel")
                })
        })
        .unwrap_or_else(|| panic!("Swimlane edge label group: {svg}"));
    let styles = std::iter::once(label)
        .chain(label.descendants())
        .filter_map(|node| node.attribute("style"))
        .collect::<Vec<_>>();

    assert!(
        styles.iter().any(|style| style.contains("font-size:24px")),
        "the first default style must reach the generated labelRect DOM: styles={styles:?}, svg={svg}"
    );
    assert!(
        styles.iter().all(|style| {
            !style.contains("font-weight:bold")
                && !style.contains("font-size:12px")
                && !style.contains("font-style:italic")
                && !style.contains("!important")
        }),
        "later edge styles must not override the generated labelRect label style: styles={styles:?}, svg={svg}"
    );
}

#[test]
fn dagre_flowchart_crossed_circle_aliases_use_source_symmetric_root_bounds() {
    let _session = RenderEnvironment::deterministic().begin_session().unwrap();
    let text = r#"flowchart
 n0@{ shape: cross-circ, label: "cross-circ" }
 n1@{ shape: summary, label: "summary" }
 n2@{ shape: crossed-circle, label: "crossed-circle" }
"#;
    let engine = Engine::new().with_site_config(MermaidConfig::from_value(
        serde_json::json!({"layout": "dagre"}),
    ));
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
        render_with_engine(
            Engine::new().with_site_config(MermaidConfig::from_value(
                serde_json::json!({"layout": "dagre"}),
            )),
            text,
        )
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

    let session = RenderEnvironment::deterministic()
        .with_compiled_math_renderer()
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

    let session = RenderEnvironment::deterministic()
        .with_compiled_math_renderer()
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

    let session = RenderEnvironment::deterministic()
        .with_compiled_math_renderer()
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

fn flowchart_title_theme(rules: impl IntoIterator<Item = ThemeRule>) -> DiagramTheme {
    let mut styles = ThemeRuleSet::default();
    for rule in rules {
        styles = styles.with_rule(rule);
    }
    DiagramThemeCompiler::new()
        .compile(DiagramThemeSpec::new().with_styles(styles))
        .unwrap()
}

fn flowchart_title_rule(color: &str) -> ThemeRule {
    ThemeRule::new(
        ThemeTarget::Title,
        ThemeStylePatch::default().with_fill(CanvasPaint::solid(color).unwrap()),
    )
}

#[test]
fn flowchart_and_swimlane_title_fill_preserve_cluster_css_consumers() {
    let baseline = flowchart_title_theme([]);
    for swimlane in [false, true] {
        for html in [false, true] {
            for look in ["classic", "neo", "handDrawn"] {
                for explicit_default in [false, true] {
                    let source = format!(
                        "---\ntitle: Diagram title\nconfig:\n  theme: default\n  look: {look}\n  htmlLabels: {html}\n  flowchart:\n    htmlLabels: {html}\n{}---\nflowchart TD\nsubgraph Group[Group title]\nA[Alpha]\nend\n",
                        if swimlane { "  layout: swimlane\n" } else { "" }
                    );
                    let mut rule = flowchart_title_rule("#2468ac");
                    if explicit_default {
                        rule = rule.with_variant(ThemeVariant::Default);
                    }
                    let theme = flowchart_title_theme([rule]);
                    let render = |theme: &DiagramTheme, engine| {
                        prepare_flowchart_family_with_theme_engine_and_portability(
                            &source,
                            theme,
                            engine,
                            ThemePortabilityRequirement::RequirePortable,
                        )
                        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                        .expect("portable cluster title")
                    };
                    let typed = render(&theme, Engine::new());
                    let legacy = render(&baseline, Engine::new().with_site_config(MermaidConfig::from_value(
                        serde_json::json!({ "themeVariables": { "titleColor": "#2468ac", "nodeTextColor": "#333" } })
                    )));
                    assert_eq!(
                        typed.svg(),
                        legacy.svg(),
                        "swimlane={swimlane}, html={html}, look={look}, default={explicit_default}"
                    );
                    let evidence =
                        merman_render::__private::family_evidence(typed.into_completion().report());
                    assert_eq!(evidence.applied_count(), 1);
                    assert_eq!(evidence.theme_residual_count(), 0);
                }
            }
        }
    }
}

#[test]
fn flowchart_title_evidence_tracks_cluster_occurrences_and_property_winners() {
    use merman_render::diagram_theme::OrdinalSelector;
    let source =
        "flowchart TD\nsubgraph One[First]\nA[Alpha]\nend\nsubgraph Two[Second]\nB[Beta]\nend\n";
    for (source, rules, not_applicable, residual) in [
        (
            "---\ntitle: Diagram title\n---\nflowchart TD\nA[Alpha]\n",
            vec![flowchart_title_rule("#2468ac")],
            1,
            0,
        ),
        (
            source,
            vec![
                flowchart_title_rule("#2468ac").with_variant(ThemeVariant::Default),
                flowchart_title_rule("#123456"),
            ],
            1,
            0,
        ),
        (
            source,
            vec![flowchart_title_rule("#2468ac").with_ordinal(OrdinalSelector::Exact(2))],
            0,
            1,
        ),
        (
            source,
            vec![flowchart_title_rule("#2468ac").with_ordinal(OrdinalSelector::Exact(3))],
            1,
            0,
        ),
        (
            source,
            vec![
                flowchart_title_rule("#2468ac").with_ordinal(OrdinalSelector::Exact(1)),
                flowchart_title_rule("#123456"),
            ],
            1,
            0,
        ),
    ] {
        let theme = flowchart_title_theme(rules);
        let rendered = prepare_flowchart_family_with_theme_and_portability(
            source,
            &theme,
            ThemePortabilityRequirement::BestEffort,
        )
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.not_applicable_count(), not_applicable);
        assert_eq!(evidence.theme_residual_count(), residual);
        let strict = prepare_flowchart_family_with_theme(source, &theme)
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default());
        assert_eq!(strict.is_err(), residual != 0);
    }
}

#[test]
fn flowchart_title_fill_preserves_configuration_and_html_source_ownership() {
    for html in [false, true] {
        let source = format!(
            "---\nconfig:\n  htmlLabels: {html}\n  flowchart:\n    htmlLabels: {html}\n---\nflowchart TD\nsubgraph Group[Group title]\nA[Alpha]\nend\nstyle Group color:#123456\n"
        );
        let theme = flowchart_title_theme([flowchart_title_rule("#2468ac")]);
        for config_owned in [false, true] {
            let engine = if config_owned {
                Engine::new().with_site_config(MermaidConfig::from_value(
                    serde_json::json!({ "themeVariables": { "titleColor": "#abcdef" } }),
                ))
            } else {
                Engine::new()
            };
            let rendered = prepare_flowchart_family_with_theme_engine_and_portability(
                &source,
                &theme,
                engine,
                ThemePortabilityRequirement::RequirePortable,
            )
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            // SVG cluster labels do not emit the local label_style color; HTML spans do.
            assert_eq!(
                evidence.applied_count(),
                usize::from(!html && !config_owned)
            );
            assert_eq!(
                evidence.not_applicable_count(),
                usize::from(html || config_owned)
            );
        }
    }
}

#[test]
fn flowchart_title_transparency_and_clear_have_distinct_evidence() {
    let source = "flowchart TD\nsubgraph Group[Group title]\nA[Alpha]\nend\n";
    for fill in [Specified::Value(CanvasPaint::Transparent), Specified::Clear] {
        let clear = matches!(fill, Specified::Clear);
        let mut patch = ThemeStylePatch::default();
        patch.paint.fill = fill;
        let theme = flowchart_title_theme([ThemeRule::new(ThemeTarget::Title, patch)]);
        let rendered = prepare_flowchart_family_with_theme_and_portability(
            source,
            &theme,
            ThemePortabilityRequirement::BestEffort,
        )
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
        if !clear {
            assert!(
                rendered
                    .svg()
                    .contains(".cluster-label text{fill:transparent;}")
            );
        }
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), usize::from(!clear));
        assert_eq!(evidence.theme_residual_count(), usize::from(clear));
    }
}

#[test]
fn flowchart_title_fill_yields_to_assigned_class_tspan_color() {
    let theme = flowchart_title_theme([flowchart_title_rule("#2468ac")]);
    for swimlane in [false, true] {
        let source = format!(
            "---\nconfig:\n  htmlLabels: false\n  flowchart:\n    htmlLabels: false\n{}---\nflowchart TD\nsubgraph Group[Group title]\nA[Alpha]\nend\nclassDef ink color:#123456\nstyle Group color:#abcdef\nclass Group ink\n",
            if swimlane { "  layout: swimlane\n" } else { "" }
        );
        let rendered = prepare_flowchart_family_with_theme(&source, &theme)
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        assert!(
            rendered
                .svg()
                .contains(".ink tspan{fill:rgb(18, 52, 86)!important;}")
        );
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        assert!(document.descendants().any(|node| {
            node.attribute("class")
                .is_some_and(|classes| classes.split_whitespace().any(|class| class == "ink"))
                && node.descendants().any(|child| child.has_tag_name("tspan"))
        }));
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 0);
        assert_eq!(evidence.not_applicable_count(), 1);
    }
}

#[test]
fn flowchart_and_swimlane_generic_text_fill_has_typed_terminal_evidence() {
    for swimlane in [false, true] {
        for html in [false, true] {
            for look in ["classic", "neo", "handDrawn"] {
                for explicit_default in [false, true] {
                    let source = format!(
                        "---\ntitle: Diagram title\nconfig:\n  look: {look}\n  htmlLabels: {html}\n  flowchart:\n    htmlLabels: {html}\n{}---\nflowchart TD\nsubgraph Group[Group title]\nA[Alpha] -->|Advance| B[Beta]\nend\n",
                        if swimlane { "  layout: swimlane\n" } else { "" }
                    );
                    let mut rule = ThemeRule::new(
                        ThemeTarget::Text,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#2468ac").unwrap()),
                    );
                    if explicit_default {
                        rule = rule.with_variant(ThemeVariant::Default);
                    }
                    let theme = flowchart_title_theme([rule]);
                    let rendered = prepare_flowchart_family_with_theme(&source, &theme)
                        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                        .expect("generic text paint must have direct terminal evidence");
                    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
                    for label in ["Alpha", "Beta", "Advance", "Group title", "Diagram title"] {
                        assert!(
                            document
                                .descendants()
                                .filter(|node| node.has_tag_name("text")
                                    || node.has_tag_name("p")
                                    || node.has_tag_name("span"))
                                .any(|node| node
                                    .descendants()
                                    .filter(|child| child.is_text())
                                    .filter_map(|child| child.text())
                                    .collect::<String>()
                                    .contains(label)),
                            "missing {label}: {}",
                            rendered.svg()
                        );
                    }
                    assert!(rendered.svg().contains("#2468ac"));
                    let evidence = merman_render::__private::family_evidence(
                        rendered.into_completion().report(),
                    );
                    assert_eq!(
                        evidence.applied_count(),
                        1,
                        "swimlane={swimlane}, html={html}, look={look}, default={explicit_default}"
                    );
                    assert_eq!(evidence.theme_residual_count(), 0);
                }
            }
        }
    }
}

#[test]
fn flowchart_generic_text_fill_shadowed_by_role_winners_is_not_applicable() {
    let source = "flowchart TD\nA[Alpha]\n";
    let theme = flowchart_title_theme([
        ThemeRule::new(
            ThemeTarget::Text,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#2468ac").unwrap()),
        ),
        ThemeRule::new(
            ThemeTarget::NodeLabel,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ac6824").unwrap()),
        ),
    ]);
    let rendered = prepare_flowchart_family_with_theme(source, &theme)
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .expect("unused generic fallback must not block strict rendering");
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn flowchart_generic_text_fill_keeps_unconsumed_sibling_and_ordinal_residuals() {
    use merman_render::diagram_theme::OrdinalSelector;
    let source = "flowchart TD\nA[Alpha] -->|Advance| B[Beta]\n";
    let fill = || CanvasPaint::solid("#2468ac").unwrap();
    for rules in [
        vec![ThemeRule::new(
            ThemeTarget::Text,
            ThemeStylePatch::default()
                .with_fill(fill())
                .with_stroke(fill()),
        )],
        vec![
            ThemeRule::new(
                ThemeTarget::Text,
                ThemeStylePatch::default().with_fill(fill()),
            ),
            ThemeRule::new(
                ThemeTarget::Text,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ac6824").unwrap()),
            )
            .with_ordinal(OrdinalSelector::Exact(2)),
        ],
    ] {
        let theme = flowchart_title_theme(rules);
        let rendered = prepare_flowchart_family_with_theme_and_portability(
            source,
            &theme,
            ThemePortabilityRequirement::BestEffort,
        )
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert!(
            evidence.theme_residual_count() > 0,
            "unconsumed text facets must remain residual"
        );
        let strict = prepare_flowchart_family_with_theme(source, &theme)
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default());
        assert!(strict.is_err());
    }
}

#[test]
fn flowchart_generic_text_fill_overwritten_ordinal_is_not_applicable() {
    use merman_render::diagram_theme::OrdinalSelector;
    let theme = flowchart_title_theme([
        ThemeRule::new(
            ThemeTarget::Text,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ac6824").unwrap()),
        )
        .with_ordinal(OrdinalSelector::Exact(2)),
        ThemeRule::new(
            ThemeTarget::Text,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#2468ac").unwrap()),
        ),
    ]);
    let rendered =
        prepare_flowchart_family_with_theme("flowchart TD\nA[Alpha] --> B[Beta]\n", &theme)
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("fully overwritten ordinal must not block generic text paint");
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(evidence.applied_count(), 1);
    assert_eq!(evidence.not_applicable_count(), 1);
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn flowchart_generic_text_uses_the_emitted_markdown_edge_color_owner() {
    let source = "---\nconfig:\n  htmlLabels: false\n  flowchart:\n    htmlLabels: false\n---\nflowchart TD\nA[Alpha] -->|`**Advance**`| B[Beta]\nlinkStyle 0 color:#22c55e\n";
    let theme = flowchart_title_theme([
        ThemeRule::new(
            ThemeTarget::Text,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#2468ac").unwrap()),
        ),
        ThemeRule::new(
            ThemeTarget::NodeLabel,
            ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ac6824").unwrap()),
        ),
    ]);
    let rendered = prepare_flowchart_family_with_theme_and_portability(
        source,
        &theme,
        ThemePortabilityRequirement::BestEffort,
    )
    .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
    .unwrap();
    let document = roxmltree::Document::parse(rendered.svg()).unwrap();
    let edge_label = document
        .descendants()
        .find(|node| node.attribute("class") == Some("edgeLabel"))
        .expect("edge label");
    let emitted_source_color = edge_label.descendants().any(|node| {
        node.attribute("style")
            .is_some_and(|style| style.contains("#22c55e"))
    });
    let evidence = merman_render::__private::family_evidence(rendered.into_completion().report());
    assert_eq!(
        evidence.applied_count(),
        if emitted_source_color { 1 } else { 2 }
    );
    assert_eq!(
        evidence.not_applicable_count(),
        usize::from(emitted_source_color)
    );
    assert_eq!(evidence.theme_residual_count(), 0);
}

#[test]
fn flowchart_and_swimlane_edge_label_background_fill_has_typed_terminal_evidence() {
    for swimlane in [false, true] {
        for html in [false, true] {
            for look in ["classic", "neo", "handDrawn"] {
                for explicit_default in [false, true] {
                    let source = format!(
                        "---\ntitle: Diagram title\nconfig:\n  look: {look}\n  htmlLabels: {html}\n  flowchart:\n    htmlLabels: {html}\n{}---\nflowchart TD\nA[Alpha] -->|Advance| B[Beta]\n",
                        if swimlane { "  layout: swimlane\n" } else { "" }
                    );
                    let mut rule = ThemeRule::new(
                        ThemeTarget::EdgeLabelBackground,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#2468ac").unwrap()),
                    );
                    if explicit_default {
                        rule = rule.with_variant(ThemeVariant::Default);
                    }
                    let theme = flowchart_title_theme([rule]);
                    let rendered = prepare_flowchart_family_with_theme(&source, &theme)
                        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                        .expect("edge label background must have direct terminal evidence");
                    assert!(rendered.svg().contains("#2468ac"));
                    if swimlane {
                        let doc = roxmltree::Document::parse(rendered.svg()).unwrap();
                        assert!(doc.descendants().any(|node| node.has_tag_name("rect")
                            && node.attribute("class") == Some("background")
                            && ["width", "height"].iter().all(|name| {
                                node.attribute(*name)
                                    .and_then(|value| value.parse::<f64>().ok())
                                    .is_some_and(|value| value > 1.0)
                            })));
                    }
                    let evidence = merman_render::__private::family_evidence(
                        rendered.into_completion().report(),
                    );
                    assert_eq!(
                        evidence.applied_count(),
                        1,
                        "swimlane={swimlane}, html={html}, look={look}, default={explicit_default}"
                    );
                    assert_eq!(evidence.theme_residual_count(), 0);
                }
            }
        }
    }
}

#[test]
fn flowchart_and_swimlane_background_unconsumed_sibling_and_ordinal_remain_residual() {
    use merman_render::diagram_theme::OrdinalSelector;
    for source in [
        "flowchart TD\nA[Alpha] -->|Advance| B[Beta]\nB -->|Finish| C[Gamma]\n",
        "---\nconfig:\n  layout: swimlane\n---\nflowchart TD\nA[Alpha] -->|Advance| B[Beta]\nB -->|Finish| C[Gamma]\n",
    ] {
        let fill = || CanvasPaint::solid("#2468ac").unwrap();
        for rules in [
            vec![
                ThemeRule::new(
                    ThemeTarget::EdgeLabelBackground,
                    ThemeStylePatch::default().with_fill(fill()),
                )
                .with_ordinal(OrdinalSelector::Exact(1)),
            ],
            vec![ThemeRule::new(
                ThemeTarget::EdgeLabelBackground,
                ThemeStylePatch::default().with_stroke(fill()),
            )],
            vec![ThemeRule::new(
                ThemeTarget::EdgeLabelBackground,
                ThemeStylePatch::default()
                    .with_fill(fill())
                    .with_stroke(fill()),
            )],
            vec![
                ThemeRule::new(
                    ThemeTarget::EdgeLabelBackground,
                    ThemeStylePatch::default().with_fill(fill()),
                ),
                ThemeRule::new(
                    ThemeTarget::EdgeLabelBackground,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ac6824").unwrap()),
                )
                .with_ordinal(OrdinalSelector::Exact(2)),
            ],
        ] {
            let theme = flowchart_title_theme(rules);
            let rendered = prepare_flowchart_family_with_theme_and_portability(
                source,
                &theme,
                ThemePortabilityRequirement::BestEffort,
            )
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert!(evidence.theme_residual_count() > 0, "source={source}");
            let strict = prepare_flowchart_family_with_theme(source, &theme)
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default());
            assert!(strict.is_err());
        }
    }
}

#[test]
fn flowchart_background_requires_a_real_terminal_and_respects_config_owner() {
    let theme = flowchart_title_theme([ThemeRule::new(
        ThemeTarget::EdgeLabelBackground,
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#2468ac").unwrap()),
    )]);
    for prefix in ["", "---\nconfig:\n  layout: swimlane\n---\n"] {
        for (source, engine) in [
            ("flowchart TD\nA[Alpha] --> B[Beta]\n", Engine::new()),
            (
                "flowchart TD\nA[Alpha] -->|Advance| B[Beta]\n",
                Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
                    "theme": "base", "themeVariables": {"edgeLabelBackground": "#ac6824"}
                }))),
            ),
        ] {
            let source = format!("{prefix}{source}");
            let rendered = prepare_flowchart_family_with_theme_engine_and_portability(
                &source,
                &theme,
                engine,
                ThemePortabilityRequirement::RequirePortable,
            )
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
            let evidence =
                merman_render::__private::family_evidence(rendered.into_completion().report());
            assert_eq!(evidence.applied_count(), 0, "source={source}");
            assert_eq!(evidence.not_applicable_count(), 1);
            assert_eq!(evidence.theme_residual_count(), 0);
        }
    }
}

#[test]
fn flowchart_background_covers_special_node_labels_without_an_edge() {
    let theme = flowchart_title_theme([ThemeRule::new(
        ThemeTarget::EdgeLabelBackground,
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#2468ac").unwrap()),
    )]);
    for source in [
        "flowchart TD\nA@{ icon: 'missing:icon', label: 'Special label', form: 'square' }\n",
        "flowchart TD\nA@{ icon: 'missing:icon', label: 'Special label' }\n",
    ] {
        let rendered = prepare_flowchart_family_with_theme(source, &theme)
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .unwrap();
        assert!(rendered.svg().contains("labelBkg"));
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn flowchart_background_overwritten_ordinal_is_not_applicable() {
    use merman_render::diagram_theme::OrdinalSelector;
    let ordinal = ThemeRule::new(
        ThemeTarget::EdgeLabelBackground,
        ThemeStylePatch::default().with_fill(CanvasPaint::solid("#ac6824").unwrap()),
    )
    .with_ordinal(OrdinalSelector::Exact(1));
    let winner = ThemeRule::new(
        ThemeTarget::EdgeLabelBackground,
        ThemeStylePatch::default().with_fill(CanvasPaint::Transparent),
    );
    let theme = flowchart_title_theme([ordinal, winner]);
    for prefix in ["", "---\nconfig:\n  layout: swimlane\n---\n"] {
        let source = format!("{prefix}flowchart TD\nA[Alpha] -->|Advance| B[Beta]\n");
        let rendered = prepare_flowchart_family_with_theme(&source, &theme)
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("overwritten ordinal must not block the static transparent winner");
        let evidence =
            merman_render::__private::family_evidence(rendered.into_completion().report());
        assert_eq!(evidence.applied_count(), 1);
        assert_eq!(evidence.not_applicable_count(), 1);
        assert_eq!(evidence.theme_residual_count(), 0);
    }
}

#[test]
fn brutalist_preset_emits_ordinal_accents_and_preserves_source_fill() {
    use merman_render::diagram_theme::ThemePreset;

    let theme = DiagramThemeCompiler::new()
        .compile_preset(ThemePreset::Brutalist)
        .expect("compile Brutalist preset");
    for source_override in [false, true] {
        let mut source = String::from("---\nconfig:\n  look: classic\n---\nflowchart LR\n");
        for ordinal in 1..=30 {
            source.push_str(&format!("N{ordinal}[Node {ordinal}]\n"));
        }
        if source_override {
            source.push_str("style N2 fill:#123456\n");
        }
        let rendered = prepare_flowchart_family_with_theme(&source, &theme)
            .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            .expect("Brutalist ordinal node fills must be portable");
        let document = roxmltree::Document::parse(rendered.svg()).unwrap();
        for ordinal in 1..=30 {
            let id = format!("N{ordinal}");
            let node = document
                .descendants()
                .find(|node| node.attribute("data-id") == Some(id.as_str()))
                .expect("emitted semantic node");
            let shape = node
                .descendants()
                .find(|node| {
                    node.attribute("class").is_some_and(|classes| {
                        classes
                            .split_whitespace()
                            .any(|class| class == "label-container")
                    })
                })
                .expect("node shape");
            let expected = if source_override && ordinal == 2 {
                "#123456"
            } else if ordinal % 5 == 0 {
                "#FF6B35"
            } else if ordinal % 3 == 0 {
                "#4ECDC4"
            } else if ordinal % 2 == 0 {
                "#FFE66D"
            } else {
                "#ffffff"
            };
            assert!(
                shape
                    .attribute("style")
                    .unwrap_or_default()
                    .contains(&format!("fill:{expected}")),
                "{id} must emit {expected}; source_override={source_override}: {:?}",
                shape.attribute("style")
            );
        }
    }
}

#[test]
fn flowchart_family_ordinal_transparent_fill_preserves_other_node_fills() {
    use merman_render::diagram_theme::OrdinalSelector;

    for family in ["flowchart", "swimlane-beta"] {
        for ordinal in [
            OrdinalSelector::Exact(2),
            OrdinalSelector::Cycle {
                period: 2,
                offset: 1,
            },
        ] {
            let theme = flowchart_title_theme([
                ThemeRule::new(
                    ThemeTarget::Node,
                    ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
                ),
                ThemeRule::new(
                    ThemeTarget::Node,
                    ThemeStylePatch::default().with_fill(CanvasPaint::Transparent),
                )
                .with_variant(ThemeVariant::Default)
                .with_ordinal(ordinal),
            ]);
            let source = format!("{family} LR\nA[Alpha] --> B[Beta] --> C[Gamma]\n");
            let rendered = prepare_flowchart_family_with_theme(&source, &theme)
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .expect("ordinal transparent node fills must be portable");
            let document = roxmltree::Document::parse(rendered.svg()).unwrap();
            for (id, expected) in [("A", "#123456"), ("B", "none"), ("C", "#123456")] {
                let node = document
                    .descendants()
                    .find(|node| node.attribute("data-id") == Some(id))
                    .unwrap();
                let shape = node
                    .descendants()
                    .find(|node| {
                        node.attribute("class").is_some_and(|classes| {
                            classes
                                .split_whitespace()
                                .any(|class| class == "label-container")
                        })
                    })
                    .unwrap();
                assert!(
                    shape
                        .attribute("style")
                        .unwrap_or_default()
                        .contains(&format!("fill:{expected}")),
                    "{family} {ordinal:?} {id}: {:?}",
                    shape.attribute("style")
                );
            }
        }
    }
}

#[test]
fn flowchart_rounded_rect_radius_uses_effective_theme_and_source_truthiness() {
    for (look, theme, radius, expected) in [
        ("neo", "neo", None, Some(3.0)),
        ("neo", "redux", None, Some(12.0)),
        ("classic", "redux", None, Some(12.0)),
        ("neo", "default", None, Some(5.0)),
        ("classic", "default", None, Some(5.0)),
        (
            "classic",
            "default",
            Some(serde_json::json!(7.5)),
            Some(7.5),
        ),
        ("neo", "redux", Some(serde_json::json!(40)), Some(40.0)),
        ("neo", "redux", Some(serde_json::json!(0)), None),
        ("classic", "default", Some(serde_json::json!(0)), None),
        ("classic", "default", Some(serde_json::json!(false)), None),
        ("neo", "redux", Some(serde_json::json!("")), None),
        // initialize ignores a null override before the shape reads its effective theme.
        ("neo", "redux", Some(serde_json::json!(null)), Some(12.0)),
        ("neo", "redux", Some(serde_json::json!("0")), Some(0.0)),
        (
            "classic",
            "default",
            Some(serde_json::json!("7.5")),
            Some(7.5),
        ),
    ] {
        for html_labels in [true, false] {
            let mut config = serde_json::json!({
                "look": look, "theme": theme, "htmlLabels": html_labels
            });
            if let Some(radius) = radius.clone() {
                config["themeVariables"] = serde_json::json!({"radius": radius});
            }
            let engine = Engine::new().with_site_config(MermaidConfig::from_value(config));
            let svg =
                render_flowchart_svg_from_text_with_engine(engine, "flowchart TD\nA(Label)\n");
            let document = roxmltree::Document::parse(&svg).expect("Flowchart SVG");
            let rect = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("rect")
                        && node.attribute("class") == Some("basic label-container")
                })
                .expect("rounded rectangle");
            for attr in ["rx", "ry"] {
                let actual = rect
                    .attribute(attr)
                    .map(|value| value.parse::<f64>().unwrap());
                assert_eq!(
                    actual, expected,
                    "look={look}, theme={theme}, radius={radius:?}, htmlLabels={html_labels}, {attr}"
                );
            }
        }
    }
}

#[test]
fn flowchart_handdrawn_rounded_rect_honors_radius_and_square_fallback() {
    let render_paths = |radius: serde_json::Value, source: &str| {
        let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "look": "handDrawn", "theme": "default", "handDrawnSeed": 42,
            // Equal padding isolates the drawRect primitive from each shape's sizing policy.
            "flowchart": {"padding": 0},
            "themeVariables": {"radius": radius}
        })));
        let svg = render_flowchart_svg_from_text_with_engine(engine, source);
        let document = roxmltree::Document::parse(&svg).expect("Flowchart SVG");
        let shape = document
            .descendants()
            .find(|node| {
                node.has_tag_name("g") && node.attribute("class") == Some("basic label-container")
            })
            .expect("hand-drawn shape");
        let paths: Vec<String> = shape
            .descendants()
            .filter(|node| node.has_tag_name("path"))
            .map(|node| node.attribute("d").unwrap().to_owned())
            .collect();
        assert!(!paths.is_empty());
        paths
    };
    let source = "flowchart TD\nA(Label)\n";
    assert_ne!(
        render_paths(serde_json::json!(5), source),
        render_paths(serde_json::json!(12), source)
    );
    let square = render_paths(serde_json::json!(0), "flowchart TD\nA[Label]\n");
    assert_eq!(render_paths(serde_json::json!(0), source), square);
    assert_eq!(render_paths(serde_json::json!(false), source), square);
    assert_eq!(render_paths(serde_json::json!(""), source), square);
    // A nonempty string remains truthy in drawRect and uses its path branch.
    assert_ne!(render_paths(serde_json::json!("0"), source), square);
}

fn assert_flowchart_fork_join_geometry(backend: &str, source: &str, direction: &str) {
    use kurbo::Shape as _;

    let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
        "layout": backend,
        "look": "classic",
        "state": { "padding": 8 }
    })));
    let parsed = engine
        .parse_diagram_for_render_model_sync(source, ParseOptions::default())
        .unwrap()
        .unwrap();
    let session = RenderEnvironment::deterministic().begin_session().unwrap();
    let artifact = family::prepare(parsed, &LayoutOptions::default(), session).unwrap();
    let layout: FlowchartLayout =
        serde_json::from_value(artifact.layout_json().unwrap()["layout"]["FlowchartV2"].clone())
            .unwrap();
    let rendered = artifact
        .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
        .unwrap();
    let doc = roxmltree::Document::parse(rendered.svg()).unwrap();

    // forkJoin.ts adds state.padding / 2 to layout dimensions only. Check the actual
    // filled SVG path independently so measurement and painting cannot diverge.
    let (width, height) = match direction {
        "LR" | "RL" => (10.0, 70.0),
        _ => (70.0, 10.0),
    };
    for id in ["F", "J"] {
        let context = format!("{backend}/{direction}/{id}: {source}");
        let node = layout.nodes.iter().find(|node| node.id == id).unwrap();
        assert_eq!(node.width, width + 4.0, "{context}: layout width");
        assert_eq!(node.height, height + 4.0, "{context}: layout height");

        let group = doc
            .descendants()
            .find(|node| node.has_tag_name("g") && node.attribute("data-id") == Some(id))
            .unwrap();
        let path = group
            .descendants()
            .find(|node| node.has_tag_name("path") && node.attribute("stroke") == Some("none"))
            .unwrap();
        let bounds = kurbo::BezPath::from_svg(path.attribute("d").unwrap())
            .unwrap()
            .bounding_box();
        assert!(
            (bounds.width() - width).abs() < 1e-6,
            "{context}: painted width {} instead of {width}",
            bounds.width()
        );
        assert!(
            (bounds.height() - height).abs() < 1e-6,
            "{context}: painted height {} instead of {height}",
            bounds.height()
        );
    }
}

#[test]
fn flowchart_fork_join_bars_are_perpendicular_to_flow_across_backends() {
    for backend in [
        "dagre",
        #[cfg(feature = "layout-elk")]
        "elk",
    ] {
        for direction in ["TB", "BT", "LR", "RL"] {
            let source = format!(
                "flowchart {direction}\nA --> F@{{shape: fork}} --> J@{{shape: join}} --> B\n"
            );
            assert_flowchart_fork_join_geometry(backend, &source, direction);
        }
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn flowchart_elk_fork_join_use_the_nearest_enclosing_direction() {
    for (root, outer, inner, expected) in [
        ("TB", "LR", None, "LR"),
        ("BT", "RL", None, "RL"),
        ("LR", "TB", None, "TB"),
        ("RL", "BT", None, "BT"),
        ("TB", "LR", Some("TB"), "TB"),
        ("LR", "TB", Some("RL"), "RL"),
    ] {
        let inner_direction = inner
            .map(|dir| format!("direction {dir}"))
            .unwrap_or_default();
        let source = format!(
            "flowchart {root}\nsubgraph Outer\ndirection {outer}\nsubgraph Middle\nsubgraph Inner\n{inner_direction}\nA --> F@{{shape: fork}} --> J@{{shape: join}} --> B\nend\nend\nend\n"
        );
        assert_flowchart_fork_join_geometry("elk", &source, expected);
    }
}

#[test]
fn flowchart_image_labels_wrap_only_at_the_configured_width() {
    for backend in ["dagre", "elk"] {
        for (label, wraps) in [("Hi", false), ("My example image label", true)] {
            let source = format!(
                r#"---
config:
  layout: {backend}
  htmlLabels: true
  flowchart:
    wrappingWidth: 120
---
flowchart TD
A@{{ img: "https://mermaid.js.org/favicon.svg", label: "{label}", pos: "t", h: 60, constraint: "on" }}
"#
            );
            let svg = render_flowchart_svg_from_text(&source);
            let (_, _, _, div_style) = foreign_object_contract_for_text(&svg, label);
            if wraps {
                assert!(
                    div_style.contains("display: table;"),
                    "{backend}: {div_style}"
                );
                assert!(
                    div_style.contains("white-space: break-spaces;"),
                    "{backend}: {div_style}"
                );
                assert!(
                    div_style.contains("; width: 120px;"),
                    "{backend}: {div_style}"
                );
            } else {
                assert!(
                    div_style.contains("display: table-cell;"),
                    "{backend}: {div_style}"
                );
                assert!(
                    div_style.contains("white-space: nowrap;"),
                    "{backend}: {div_style}"
                );
                assert!(!div_style.contains("; width:"), "{backend}: {div_style}");
            }
        }
    }
}

#[test]
fn flowchart_stadium_preserves_measured_nontext_html_label_height() {
    for look in ["neo", "classic"] {
        for label in ["<br/><br/>", "<i class='fa fa-car'></i>", ""] {
            let source = format!(
                "%%{{init: {{\"look\": \"{look}\"}}}}%%\nflowchart LR\nA([\"{label}\"]) --> B[Rect]\n"
            );
            let parsed = block_on(
                Engine::new().parse_diagram_for_render_model(&source, ParseOptions::default()),
            )
            .expect("parse ok")
            .expect("diagram detected");
            let artifact = family::prepare(
                parsed,
                &LayoutOptions::default(),
                RenderEnvironment::deterministic().begin_session().unwrap(),
            )
            .expect("prepare diagram");
            let layout: FlowchartLayout = serde_json::from_value(
                artifact.layout_json().unwrap()["layout"]["FlowchartV2"].clone(),
            )
            .expect("flowchart layout");
            let rendered = artifact
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
                .expect("render svg");
            let node = layout.nodes.iter().find(|node| node.id == "A").unwrap();
            let document = roxmltree::Document::parse(rendered.svg()).expect("valid SVG");
            let path = document
                .descendants()
                .find(|node| {
                    node.has_tag_name("g")
                        && node.attribute("data-id") == Some("A")
                        && node.attribute("data-et") == Some("node")
                })
                .and_then(|group| group.descendants().find(|node| node.has_tag_name("path")))
                .and_then(|path| path.attribute("d"))
                .expect("stadium path");
            let numbers: Vec<f64> = path
                .split(|ch: char| ch.is_ascii_alphabetic() || ch == ',' || ch.is_whitespace())
                .filter(|token| !token.is_empty())
                .map(|token| token.parse().expect("path number"))
                .collect();
            let ys = numbers.chunks_exact(2).map(|pair| pair[1]);
            let height = ys.clone().fold(f64::MIN, f64::max) - ys.fold(f64::MAX, f64::min);
            assert!(
                (height - node.height).abs() < 1e-6,
                "{look}, {label:?}: painted height {height}, layout height {}",
                node.height
            );
        }
    }
}

#[cfg(feature = "layout-elk")]
#[test]
fn flowchart_elk_line_hops_rewrite_original_dashed_and_authored_masks() {
    use kurbo::Shape as _;

    struct PaintedEdge {
        id: String,
        d: String,
        style: String,
        points: String,
        marker: String,
    }
    let render = |look: &str, line_hops: serde_json::Value| {
        let engine = Engine::new().with_site_config(MermaidConfig::from_value(serde_json::json!({
            "layout": "elk", "look": look, "elk": { "lineHops": line_hops }
        })));
        let mut source = "flowchart TD\nA & B -.-> C & D\n".to_owned();
        if look == "classic" {
            source.push_str("linkStyle default stroke-dasharray:0 7 50 9,stroke:#123456\n");
        }
        let svg = render_flowchart_svg_from_text_with_engine(engine, &source);
        let doc = roxmltree::Document::parse(&svg).unwrap();
        doc.descendants()
            .filter(|node| node.attribute("data-edge") == Some("true"))
            .map(|node| PaintedEdge {
                id: node.attribute("data-id").unwrap().to_owned(),
                d: node.attribute("d").unwrap().to_owned(),
                style: node.attribute("style").unwrap().to_owned(),
                points: node.attribute("data-points").unwrap().to_owned(),
                marker: node.attribute("marker-end").unwrap().to_owned(),
            })
            .collect::<Vec<_>>()
    };
    let masks = |style: &str| {
        style
            .split(';')
            .filter_map(|declaration| declaration.trim().strip_prefix("stroke-dasharray:"))
            .map(|value| {
                value
                    .split_whitespace()
                    .map(|value| value.parse::<f64>().unwrap())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    };
    for look in ["neo", "classic"] {
        let original = render(look, serde_json::json!(false));
        assert_eq!(original.len(), 4);
        for mode in [serde_json::json!(true), serde_json::json!("gap")] {
            let hopped = render(look, mode.clone());
            let mut crossing_count = 0;
            let mut unchanged_count = 0;
            assert_eq!(hopped.len(), original.len());
            for (before, after) in original.iter().zip(&hopped) {
                let context = format!("look={look}, lineHops={mode}, edge={}", before.id);
                assert_eq!(after.id, before.id, "{context}");
                assert_eq!(after.points, before.points, "{context}: provider route");
                assert_eq!(after.marker, before.marker, "{context}: marker attachment");
                if look == "classic" {
                    assert!(
                        before.style.contains("stroke:#123456"),
                        "{context}: original color"
                    );
                    assert!(
                        after.style.contains("stroke:#123456"),
                        "{context}: after-paint color"
                    );
                }
                if before.d == after.d {
                    unchanged_count += 1;
                    assert_eq!(
                        after.style, before.style,
                        "{context}: noncrossing edge style"
                    );
                    continue;
                }
                crossing_count += 1;
                let original_masks = masks(&before.style);
                let rewritten = masks(&after.style);
                assert!(!original_masks.is_empty(), "{context}");
                assert_eq!(
                    rewritten.len(),
                    original_masks.len(),
                    "{context}: every declaration survives"
                );
                if look == "neo" {
                    assert!(
                        original_masks[0].len() > 4,
                        "{context}: start from the repeated 2/2 pattern"
                    );
                    assert_eq!(
                        original_masks[0][3], 2.0,
                        "{context}: upstream takes the fourth number, not marker clearance"
                    );
                }
                // getTotalLength() returns an SVG DOM float before marker-offset arithmetic.
                let length = f64::from(
                    kurbo::BezPath::from_svg(&after.d)
                        .unwrap()
                        .perimeter(1.0e-6) as f32,
                );
                for mask in rewritten {
                    assert_eq!(
                        mask.len(),
                        4,
                        "{context}: after-paint rewrite collapses the original dash pattern"
                    );
                    assert_eq!(mask[0], 0.0, "{context}");
                    assert_eq!(
                        mask[1], original_masks[0][1],
                        "{context}: retain original start offset"
                    );
                    assert_eq!(
                        mask[3], original_masks[0][3],
                        "{context}: retain original fourth number"
                    );
                    assert!(
                        (mask[2] - (length - mask[1] - mask[3]).max(0.0)).abs() < 1e-6,
                        "{context}: mask follows the actual hopped path length"
                    );
                }
            }
            assert!(
                crossing_count > 0,
                "{look}/{mode}: exercise a real crossing"
            );
            assert!(
                unchanged_count > 0,
                "{look}/{mode}: preserve noncrossing edges"
            );
        }
    }
}
