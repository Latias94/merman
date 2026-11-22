#![cfg(all(feature = "svg", feature = "layout-elk", feature = "diagram-flowchart"))]

use std::fmt::Write as _;

use merman::svg::{RenderResourcePolicy, ResourceLimitId, SvgRenderOptions};
use merman::{
    CancelReason, OperationControl, OperationPhase, OperationResourceDomain,
    OperationResourceOverride, OperationResourceProvenance, ParseOptions, RenderError,
    RenderOutput, RenderRequest, Renderer, ResourceLimitCause, SvgEnvironment, SvgOutput,
    SvgRequest,
};

const ELK_FLOWCHART: &str = concat!(
    "---\nconfig:\n  layout: elk\n---\n",
    include_str!("../benches/fixtures/flowchart_large.mmd")
);

fn svg_request(resources: RenderResourcePolicy) -> SvgRequest {
    SvgRequest {
        environment: SvgEnvironment::deterministic().with_resource_policy(resources),
        options: SvgRenderOptions {
            diagram_id: Some("elk-layout-model-resource".to_owned()),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn expect_svg(output: RenderOutput) -> SvgOutput {
    let RenderOutput::Svg(Some(output)) = output else {
        panic!("expected a completed SVG artifact, got {output:?}");
    };
    output
}

#[test]
fn wide_star_uses_the_default_layout_work_budget() {
    let mut source = String::from("flowchart TD\n");
    for leaf in 1..500 {
        writeln!(source, "N0 --> N{leaf}").unwrap();
    }
    let mut output = Vec::new();
    Renderer::new()
        .write_layout_json(
            RenderRequest::layout_json(&source, OperationControl::new(), SvgRequest::default()),
            &mut output,
            false,
        )
        .expect("the default layout budget must admit a 500-node star");
    let geometry: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let layout = &geometry["layout"]["FlowchartV2"];
    assert_eq!(layout["nodes"].as_array().unwrap().len(), 500);
    assert_eq!(layout["edges"].as_array().unwrap().len(), 499);
}

#[test]
fn elk_work_boundary_stays_sticky_after_cancellation_and_allows_a_fresh_retry() {
    let renderer = Renderer::new().with_parse_options(ParseOptions::strict());
    let resources = RenderResourcePolicy::trusted_native();
    let baseline = expect_svg(
        renderer
            .render(RenderRequest::svg(
                ELK_FLOWCHART,
                OperationControl::new(),
                svg_request(resources),
            ))
            .expect("the finite baseline policy must produce the complete fixture SVG"),
    );
    let work = baseline.evidence().layout_work_units();
    assert!(
        work > 1,
        "the fixture must have a positive max-minus-one limit"
    );

    let exact_resources = resources
        .with_limit(ResourceLimitId::MaxLayoutWorkUnits, work)
        .expect("the observed work must be a valid exact limit");
    let exact = expect_svg(
        renderer
            .render(RenderRequest::svg(
                ELK_FLOWCHART,
                OperationControl::new(),
                svg_request(exact_resources),
            ))
            .expect("the exact work limit must admit the complete SVG"),
    );
    assert_eq!(exact.evidence().layout_work_units(), work);
    assert_eq!(exact.svg(), baseline.svg());

    let below_resources = resources
        .with_limit(ResourceLimitId::MaxLayoutWorkUnits, work - 1)
        .expect("the max-minus-one work limit must be valid");
    let control = OperationControl::new();
    let error = renderer
        .render(RenderRequest::svg(
            ELK_FLOWCHART,
            control.clone(),
            svg_request(below_resources),
        ))
        .expect_err("the max-minus-one limit must reject without returning a partial artifact");
    let RenderError::ResourceLimitExceeded(first) = error else {
        panic!("expected a structured layout-work rejection, got {error:?}");
    };
    assert_eq!(first.id, "max_layout_work_units");
    assert_eq!(first.phase, "layout_model");
    assert_eq!(first.actual, work as u64);
    assert_eq!(first.maximum, (work - 1) as u64);
    assert_eq!(first.cause, ResourceLimitCause::Ceiling);
    assert_eq!(
        first.provenance,
        Some(OperationResourceProvenance::new(
            OperationResourceDomain::Render,
            Some(resources.profile()),
            [OperationResourceOverride {
                id: "max_layout_work_units",
                value: (work - 1) as u64,
            }],
        ))
    );

    control.cancel();
    let replayed = renderer
        .render(RenderRequest::svg(
            ELK_FLOWCHART,
            control.clone(),
            svg_request(exact_resources),
        ))
        .expect_err("a larger limit must replay the first terminal without returning SVG");
    let RenderError::ResourceLimitExceeded(replayed) = replayed else {
        panic!("expected the original resource terminal after cancellation, got {replayed:?}");
    };
    assert_eq!(replayed, first);

    let replayed = renderer
        .prepare_presentation(ELK_FLOWCHART, control)
        .expect_err("preparation must replay the first terminal without returning an artifact");
    let RenderError::ResourceLimitExceeded(replayed) = replayed else {
        panic!("expected the original resource terminal during preparation, got {replayed:?}");
    };
    assert_eq!(replayed, first);

    let fresh = expect_svg(
        renderer
            .render(RenderRequest::svg(
                ELK_FLOWCHART,
                OperationControl::new(),
                svg_request(exact_resources),
            ))
            .expect("a fresh control must receive an independent budget and complete normally"),
    );
    assert_eq!(fresh.evidence().layout_work_units(), work);
    assert_eq!(fresh.svg(), baseline.svg());
}

#[test]
fn pre_cancelled_elk_request_takes_precedence_over_a_low_layout_work_limit() {
    let renderer = Renderer::new().with_parse_options(ParseOptions::strict());
    let resources = RenderResourcePolicy::trusted_native()
        .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 1)
        .expect("a positive low work limit must be valid");
    let control = OperationControl::new();
    control.cancel();

    let error = renderer
        .render(RenderRequest::svg(
            ELK_FLOWCHART,
            control.clone(),
            svg_request(resources),
        ))
        .expect_err("pre-cancellation must stop admission without returning a partial SVG");
    let RenderError::Cancelled(first) = error else {
        panic!("expected cancellation ahead of the low work limit, got {error:?}");
    };
    assert_eq!(first.phase, OperationPhase::Admission);
    assert_eq!(first.reason, CancelReason::Requested);

    let replayed = renderer
        .prepare_presentation(ELK_FLOWCHART, control)
        .expect_err("preparation must preserve the admission cancellation");
    assert!(matches!(
        replayed,
        RenderError::Cancelled(cancelled) if cancelled == first
    ));

    let error = renderer
        .render(RenderRequest::svg(
            ELK_FLOWCHART,
            OperationControl::new(),
            svg_request(resources),
        ))
        .expect_err("without cancellation, the same low limit must reject layout work");
    assert!(matches!(
        error,
        RenderError::ResourceLimitExceeded(limit)
            if limit.id == "max_layout_work_units"
                && limit.phase == "layout_model"
                && limit.actual > 1
                && limit.maximum == 1
                && limit.cause == ResourceLimitCause::Ceiling
    ));
}
