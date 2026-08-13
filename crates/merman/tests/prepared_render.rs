#![cfg(feature = "svg")]

use merman::svg::{LayoutOptions, RenderFamilyKind, SvgRenderOptions};
use merman::{
    OperationControl, OperationExecutionPath, RenderOutput, RenderRequest, Renderer, SvgRequest,
};
use merman_core::ParseOptions;

fn svg_request(id: &str) -> SvgRequest {
    SvgRequest {
        options: SvgRenderOptions {
            diagram_id: Some(id.to_owned()),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn render_svg(renderer: &Renderer, source: &str, request: SvgRequest) -> merman::SvgOutput {
    match renderer
        .render(RenderRequest::svg(source, OperationControl::new(), request))
        .expect("SVG render succeeds")
    {
        RenderOutput::Svg(Some(output)) => output,
        RenderOutput::Svg(None) => panic!("source did not contain a diagram"),
        other => panic!("unexpected output: {other:?}"),
    }
}

#[test]
fn completed_svg_evidence_records_the_canonical_execution_path() {
    let output = render_svg(&Renderer::new(), "info", svg_request("info-evidence"));
    assert_eq!(
        output.evidence().execution_path(),
        OperationExecutionPath::Renderer
    );
    assert_eq!(output.evidence().measurement_routes().len(), 4);
}

#[test]
fn semantic_artifact_is_format_neutral_before_target_layout() {
    let renderer = Renderer::new();
    let semantic = renderer
        .prepare_semantic("info", OperationControl::new())
        .expect("semantic preparation succeeds")
        .expect("Info should prepare");

    assert_eq!(semantic.metadata().diagram_type, "info");
    assert_eq!(semantic.semantic_kind(), "info");
    let compatibility = semantic.compatibility_json().expect("compatibility JSON");
    assert_eq!(compatibility["type"], "info");

    let layout = semantic
        .render(merman::RenderTarget::LayoutJson(SvgRequest::default()))
        .expect("layout target succeeds");
    let RenderOutput::LayoutJson(Some(layout)) = layout else {
        panic!("expected layout output");
    };
    assert_eq!(layout.layout()["meta"]["diagram_type"], "info");
    assert_eq!(layout.layout()["semantic"], compatibility);
    assert!(layout.layout()["layout"]["InfoDiagram"].is_object());
}

#[test]
fn renderer_defaults_and_request_overrides_are_used_by_typed_targets() {
    let renderer = Renderer::new().with_parse_options(ParseOptions::strict());
    let source = "flowchart TD\nA[Prepared] --> B[Rendered]";

    let default_output = render_svg(&renderer, source, svg_request("default-request"));
    let override_output = renderer
        .render(
            RenderRequest::svg(
                source,
                OperationControl::new(),
                svg_request("request-override"),
            )
            .with_parse_options(ParseOptions::strict()),
        )
        .expect("override render succeeds");
    let RenderOutput::Svg(Some(override_output)) = override_output else {
        panic!("expected SVG output");
    };

    assert!(default_output.svg().contains(r#"id="default-request""#));
    assert!(override_output.svg().contains(r#"id="request-override""#));
}

#[test]
fn flowchart_ellipse_preserves_parser_semantics_but_rejects_svg_rendering() {
    let error = Renderer::new()
        .render(RenderRequest::svg(
            "graph TD\nA(-this is an ellipse-)-->B\n",
            OperationControl::new(),
            SvgRequest::default(),
        ))
        .expect_err("the unsupported ellipse shape must fail during SVG rendering");
    assert!(
        error.to_string().contains("No such shape: ellipse"),
        "{error}"
    );
}

#[test]
fn gantt_layout_target_exposes_owned_time_axis_diagnostics() {
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
    let output = Renderer::new()
        .render(RenderRequest::layout_json(
            source,
            OperationControl::new(),
            SvgRequest {
                layout: LayoutOptions::headless_svg_defaults(),
                ..Default::default()
            },
        ))
        .expect("Gantt layout succeeds");
    let RenderOutput::LayoutJson(Some(output)) = output else {
        panic!("expected Gantt layout output");
    };
    let diagnostics = output
        .gantt_time_axis_diagnostics()
        .expect("Gantt should expose time-axis diagnostics");
    assert_eq!(diagnostics.unix_millis_at_rendered_x(10.0), Some(-1));
    assert_eq!(diagnostics.unix_millis_at_rendered_x(77.0), Some(1));
    assert_eq!(diagnostics.unix_millis_at_rendered_x(44.0), None);
}

#[test]
fn separate_requests_capture_their_own_runtime_evidence() {
    let source = r#"gantt
dateFormat x
section Delivery
First: first,-1,1ms
Second: second,after first,2ms
"#;
    let renderer = Renderer::new().with_runtime_policy(
        merman_core::runtime::RuntimePolicy::deterministic().with_fixed_unix_millis(0),
    );
    let first = render_svg(&renderer, source, svg_request("gantt-time"));
    let second_renderer = Renderer::new().with_runtime_policy(
        merman_core::runtime::RuntimePolicy::deterministic().with_fixed_unix_millis(1),
    );
    let second = render_svg(&second_renderer, source, svg_request("gantt-time"));

    fn today_line(svg: &str) -> &str {
        let start = svg.find(r#"<g class="today"><line"#).expect("today marker");
        let end = svg[start..].find("/>").expect("today marker end") + start + 2;
        &svg[start..end]
    }

    assert_ne!(today_line(first.svg()), today_line(second.svg()));
    assert_eq!(first.evidence().unix_millis(), 0);
    assert_eq!(second.evidence().unix_millis(), 1);
}

#[test]
#[cfg(feature = "layout-cytoscape")]
fn architecture_render_is_stable_through_the_canonical_operation() {
    let source = r#"architecture-beta
group platform(cloud)[Platform]
group data(database)[Data] in platform
service api(server)[API] in platform
service db(database)[Database] in data
api:R --> L:db
align row api db
"#;
    let first = render_svg(&Renderer::new(), source, svg_request("architecture"));
    let second = render_svg(&Renderer::new(), source, svg_request("architecture"));
    assert_eq!(first.svg(), second.svg());
}

#[test]
fn semantic_family_kind_remains_available_without_exposing_layout_types() {
    let artifact = Renderer::new()
        .prepare_semantic("flowchart TD\nA --> B", OperationControl::new())
        .expect("semantic preparation succeeds")
        .expect("flowchart should prepare");
    assert_eq!(artifact.semantic_kind(), "flowchart");
    assert_eq!(artifact.metadata().diagram_type, "flowchart-v2");
    let _ = RenderFamilyKind::Flowchart;
}
