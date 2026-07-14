use merman::ParseOptions;
use merman::render::{
    HeadlessRenderer, LayoutOptions, PreparedRender, RenderEnvironment, RenderResourceLimits,
    SvgRenderOptions, prepare_render_sync, prepare_semantic_sync, render_svg_sync,
};

fn assert_info_artifact(prepared: &PreparedRender) {
    assert_eq!(prepared.metadata().diagram_type, "info");
    assert_eq!(
        prepared.family_kind(),
        merman::render::RenderFamilyKind::Info
    );
}

#[test]
fn staged_render_advances_one_opaque_artifact_before_svg() {
    let engine = merman::Engine::new();

    let layout_options = LayoutOptions::headless_svg_defaults();
    let semantic = prepare_semantic_sync(&engine, "info", ParseOptions::strict(), &layout_options)
        .unwrap()
        .expect("Info should prepare typed semantics");

    assert_eq!(semantic.metadata().diagram_type, "info");
    assert_eq!(semantic.semantic_kind(), "info");

    let prepared = semantic
        .continue_layout()
        .expect("Info semantics should produce a typed layout");

    assert_info_artifact(&prepared);

    let compatibility = engine
        .parse_diagram_sync("info", ParseOptions::strict())
        .unwrap()
        .expect("Info should expose compatibility JSON");
    let layout_json = prepared.layout_json().unwrap();
    assert_eq!(layout_json["meta"]["diagram_type"], "info");
    assert_eq!(layout_json["semantic"], compatibility.model);
    assert!(layout_json["layout"]["InfoDiagram"].is_object());

    let svg = prepared
        .render_svg(&SvgRenderOptions {
            diagram_id: Some("prepared-info".to_string()),
            ..Default::default()
        })
        .unwrap();

    assert!(svg.contains(r#"id="prepared-info""#), "{svg}");
}

#[test]
fn flowchart_elk_admission_can_skip_after_parse_and_before_layout() {
    let source = r#"---
config:
  layout: elk
---
flowchart TD
A --> B
"#;
    let engine = merman::Engine::new();
    let renderer = HeadlessRenderer::new()
        .with_engine(engine)
        .with_environment(RenderEnvironment::parity())
        .with_strict_parsing()
        .with_layout_options(LayoutOptions::headless_svg_defaults())
        .with_resource_limits(RenderResourceLimits {
            max_flowchart_nodes: Some(0),
            ..RenderResourceLimits::unbounded_for_trusted_input()
        });
    let semantic = renderer
        .prepare_semantic_sync(source)
        .unwrap()
        .expect("Flowchart should prepare typed semantics before ELK layout admission");

    assert_eq!(semantic.metadata().diagram_type, "flowchart-v2");
    assert_eq!(
        semantic.metadata().effective_config.get_str("layout"),
        Some("elk")
    );
    assert_eq!(semantic.semantic_kind(), "flowchart");

    // Admission can drop the semantic stage here without starting layout.
    drop(semantic);

    let semantic = renderer
        .prepare_semantic_sync(source)
        .unwrap()
        .expect("the same source should prepare typed semantics again");
    let error = match semantic.continue_layout() {
        Ok(_) => panic!("layout should enforce the configured model limit"),
        Err(error) => error,
    };
    let message = error.to_string();
    assert!(
        message.contains("max_flowchart_nodes") || message.contains("flowchart-v2"),
        "{message}"
    );
}

#[test]
fn high_level_render_matches_the_prepared_artifact_path() {
    let engine = merman::Engine::new();
    let source = "flowchart TD\nA[Prepared] --> B[Rendered]";
    let parse_options = ParseOptions::strict();
    let layout_options = LayoutOptions::headless_svg_defaults();
    let svg_options = SvgRenderOptions {
        diagram_id: Some("prepared-flowchart".to_string()),
        ..Default::default()
    };

    let prepared = prepare_render_sync(&engine, source, parse_options, &layout_options)
        .unwrap()
        .expect("Flowchart should prepare a render artifact");
    assert_eq!(prepared.metadata().diagram_type, "flowchart-v2");
    assert_eq!(
        prepared.family_kind(),
        merman::render::RenderFamilyKind::Flowchart
    );

    let prepared_svg = prepared.render_svg(&svg_options).unwrap();
    let high_level_svg = render_svg_sync(
        &engine,
        source,
        parse_options,
        &layout_options,
        &svg_options,
    )
    .unwrap()
    .expect("Flowchart should render through the high-level helper");

    assert_eq!(prepared_svg, high_level_svg);
}

#[test]
fn prepared_gantt_exposes_owned_time_axis_diagnostics() {
    let source = r#"---
config:
  gantt:
    useWidth: 130
    leftPadding: 10
    rightPadding: 20
---
gantt
dateFormat x
section Delivery
First: first,-1,1ms
Second: second,after first,2ms
"#;
    let prepared = prepare_render_sync(
        &merman::Engine::new(),
        source,
        ParseOptions::strict(),
        &LayoutOptions::headless_svg_defaults(),
    )
    .unwrap()
    .expect("Gantt should produce a prepared render artifact");

    assert_eq!(
        prepared.family_kind(),
        merman::render::RenderFamilyKind::Gantt
    );
    let diagnostics = prepared
        .gantt_time_axis_diagnostics()
        .expect("Gantt tasks should expose time-axis diagnostics");
    prepared
        .render_svg(&SvgRenderOptions::default())
        .expect("Gantt should render through the prepared artifact");

    assert_eq!(diagnostics.unix_millis_at_rendered_x(10.0), Some(-1));
    assert_eq!(diagnostics.unix_millis_at_rendered_x(77.0), Some(1));
    assert_eq!(diagnostics.unix_millis_at_rendered_x(44.0), None);
}

#[test]
fn prepared_architecture_matches_the_canonical_high_level_operation() {
    let engine = merman::Engine::new();
    let source = r#"architecture-beta
group platform(cloud)[Platform]
group data(database)[Data] in platform
service api(server)[API] in platform
service db(database)[Database] in data
api:R --> L:db
align row api db
"#;
    let parse_options = ParseOptions::strict();
    let layout_options = LayoutOptions::headless_svg_defaults();
    let svg_options = SvgRenderOptions {
        diagram_id: Some("prepared-architecture".to_string()),
        ..Default::default()
    };

    let prepared = prepare_render_sync(&engine, source, parse_options, &layout_options)
        .unwrap()
        .expect("Architecture should produce a typed prepared artifact");
    assert_eq!(
        prepared.family_kind(),
        merman::render::RenderFamilyKind::Architecture
    );
    let prepared_svg = prepared.render_svg(&svg_options).unwrap();
    let high_level_svg = render_svg_sync(
        &engine,
        source,
        parse_options,
        &layout_options,
        &svg_options,
    )
    .unwrap()
    .expect("Architecture should render through the canonical operation");

    assert_eq!(prepared_svg, high_level_svg);
}
