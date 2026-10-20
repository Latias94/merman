#![cfg(all(feature = "svg", feature = "diagram-kanban", feature = "diagram-pie"))]

use merman::svg::{
    HostMeasurementResult, HostTextMeasurementRequest, HostTextMeasurer, LayoutAdaptation,
    LayoutOptions,
};
use merman::{
    OperationControl, PrepareSourceRequest, RenderOutput, RenderTarget, Renderer, SvgEnvironment,
    SvgRequest,
};
use std::sync::Arc;

struct UnusedMeasurer;
impl HostTextMeasurer for UnusedMeasurer {
    fn measure(&self, _: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
        panic!("empty geometry must not activate unused text services");
    }
}

#[test]
fn empty_geometry_validates_explicit_adaptation_before_unused_text_services() {
    let renderer = Renderer::new();
    let prepared = renderer
        .prepare_source(PrepareSourceRequest::new(
            "pie\n  \"One\" : 1",
            OperationControl::new(),
        ))
        .unwrap()
        .unwrap();
    let environment = SvgEnvironment::deterministic()
        .with_capability_policy(merman::svg::RenderCapabilityPolicy::deny_all())
        .with_text_measurement_policy(merman::svg::TextMeasurementPolicy::host_display(
            merman::svg::TextMeasurementProfileIdentity::new(
                merman::svg::MeasurementProfileId::new("unused").unwrap(),
                "1",
            )
            .unwrap(),
            Arc::new(UnusedMeasurer),
            merman::svg::TextMeasurementPhase::ALL,
        ));
    let request = SvgRequest {
        environment,
        ..Default::default()
    };
    let output = renderer
        .render_prepared(
            &prepared,
            RenderTarget::EdgeGeometryJson(request.clone()),
            OperationControl::new(),
        )
        .unwrap();
    assert!(matches!(output, RenderOutput::EdgeGeometryJson(Some(_))));
    let error = renderer
        .render_prepared(
            &prepared,
            RenderTarget::EdgeGeometryJson(SvgRequest {
                layout: LayoutOptions::default().with_adaptation(LayoutAdaptation::FitWidth),
                ..request
            }),
            OperationControl::new(),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        merman::RenderError::Svg(merman::svg::RenderError::UnsupportedLayoutAdaptation { .. })
    ));
}
