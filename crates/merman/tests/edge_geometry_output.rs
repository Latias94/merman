#![cfg(feature = "svg")]

use merman::svg::{RenderResourcePolicy, ResourceLimitId};
use merman::{
    OperationControl, RenderOutput, RenderRequest, RenderTarget, Renderer, SvgEnvironment,
    SvgRequest,
};

#[test]
fn typed_geometry_does_not_consume_the_svg_output_budget() {
    let request = SvgRequest {
        environment: SvgEnvironment::default().with_resource_policy(
            RenderResourcePolicy::default()
                .with_limit(ResourceLimitId::MaxSvgBytes, 1)
                .unwrap()
                .with_limit(ResourceLimitId::MaxGeometryJsonBytes, 1)
                .unwrap(),
        ),
        ..SvgRequest::default()
    };
    let output = Renderer::new()
        .render(RenderRequest::new(
            "flowchart LR\nA((Circle)) -->|label| B[End]",
            RenderTarget::EdgeGeometryJson(request.clone()),
            OperationControl::default(),
        ))
        .expect("typed geometry does not emit SVG bytes");
    let RenderOutput::EdgeGeometryJson(Some(geometry)) = output else {
        panic!("geometry output")
    };
    assert_eq!(geometry.edges().len(), 1);
    assert!(
        Renderer::new()
            .render(RenderRequest::new(
                "flowchart LR\nA((Circle)) -->|label| B[End]",
                RenderTarget::Svg(request),
                OperationControl::default(),
            ))
            .is_err()
    );
}

#[test]
fn typed_geometry_still_admits_actual_layout_work() {
    let request = SvgRequest {
        environment: SvgEnvironment::default().with_resource_policy(
            RenderResourcePolicy::default()
                .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 1)
                .unwrap(),
        ),
        ..SvgRequest::default()
    };
    let error = Renderer::new()
        .render(RenderRequest::new(
            "flowchart LR\nA -->|label| B",
            RenderTarget::EdgeGeometryJson(request),
            OperationControl::new(),
        ))
        .unwrap_err();
    assert!(
        error.to_string().contains("max_layout_work_units"),
        "{error}"
    );
}

#[test]
fn geometry_rejects_portability_as_an_unsupported_target() {
    let request = SvgRequest {
        environment: SvgEnvironment::default().with_theme_portability_requirement(
            merman::svg::ThemePortabilityRequirement::RequirePortable,
        ),
        ..SvgRequest::default()
    };
    let error = Renderer::new()
        .render(RenderRequest::new(
            "flowchart LR\nA --> B",
            RenderTarget::EdgeGeometryJson(request),
            OperationControl::new(),
        ))
        .unwrap_err();
    assert!(matches!(
        error,
        merman::RenderError::PortabilityUnavailableForTarget {
            target: "edge-geometry-json"
        }
    ));
}

#[test]
fn a_non_geometry_builtin_does_not_require_unused_layout_services() {
    #[derive(Debug)]
    struct UnusedTextService;
    impl merman::svg::HostTextMeasurer for UnusedTextService {
        fn measure(
            &self,
            _: merman::svg::HostTextMeasurementRequest<'_>,
        ) -> merman::svg::HostMeasurementResult {
            panic!("empty geometry must not invoke text measurement")
        }
    }
    let request = SvgRequest {
        environment: SvgEnvironment::default()
            .with_capability_policy(merman::svg::RenderCapabilityPolicy::deny_all())
            .with_text_measurement_policy(merman::svg::TextMeasurementPolicy::host_display(
                merman::svg::TextMeasurementProfileIdentity::new(
                    merman::svg::MeasurementProfileId::new("test.unused-geometry-text").unwrap(),
                    "1",
                )
                .unwrap(),
                std::sync::Arc::new(UnusedTextService),
                merman::svg::TextMeasurementPhase::ALL,
            )),
        ..SvgRequest::default()
    };
    let output = Renderer::new()
        .render(RenderRequest::new(
            "architecture-beta\nservice api(server)[API]",
            RenderTarget::EdgeGeometryJson(request),
            OperationControl::new(),
        ))
        .unwrap();
    let RenderOutput::EdgeGeometryJson(Some(output)) = output else {
        panic!("geometry output")
    };
    assert!(output.edges().is_empty());
}

#[test]
fn encoded_geometry_has_exact_byte_admission() {
    use merman_render::model::{EdgePaintGeometry, EdgePaintGeometryOutput, LayoutPoint};
    let output = EdgePaintGeometryOutput::new(
        "flowchart-v2".into(),
        vec![EdgePaintGeometry {
            id: "escaped\"\n边".repeat(1000),
            points: Some(
                (0..2000)
                    .map(|index| LayoutPoint {
                        x: f64::from(index),
                        y: -0.5,
                    })
                    .collect(),
            ),
            label_position: None,
        }],
    );
    let expected = serde_json::to_vec(&output).unwrap();
    let exact = RenderResourcePolicy::default()
        .with_limit(ResourceLimitId::MaxGeometryJsonBytes, expected.len())
        .unwrap();
    let mut bytes = Vec::new();
    output
        .write_json_controlled(&mut bytes, exact, &OperationControl::new())
        .unwrap();
    assert_eq!(bytes, expected);
    let mut rejected = Vec::new();
    let error = output
        .write_json_controlled(
            &mut rejected,
            exact
                .with_limit(ResourceLimitId::MaxGeometryJsonBytes, expected.len() - 1)
                .unwrap(),
            &OperationControl::new(),
        )
        .unwrap_err();
    assert!(
        error.to_string().contains("max_geometry_json_bytes"),
        "{error}"
    );
    assert!(rejected.len() < expected.len());
}

#[test]
fn geometry_encoding_observes_cancellation_inside_long_ids_and_point_lists() {
    use merman_render::model::{EdgePaintGeometry, EdgePaintGeometryOutput, LayoutPoint};
    struct CancellingWriter {
        control: OperationControl,
        remaining: usize,
    }
    impl std::io::Write for CancellingWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.remaining = self.remaining.saturating_sub(bytes.len());
            if self.remaining == 0 {
                self.control.cancel();
            }
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    for (id, points, before_cancel) in [
        ("a".repeat(100_000), vec![], 4096),
        (
            "edge".to_owned(),
            (0..20_000)
                .map(|index| LayoutPoint {
                    x: f64::from(index),
                    y: 0.0,
                })
                .collect(),
            4096,
        ),
    ] {
        let output = EdgePaintGeometryOutput::new(
            "flowchart-v2".into(),
            vec![EdgePaintGeometry {
                id,
                points: Some(points),
                label_position: None,
            }],
        );
        let control = OperationControl::new();
        let mut writer = CancellingWriter {
            control: control.clone(),
            remaining: before_cancel,
        };
        let error = output
            .write_json_controlled(&mut writer, RenderResourcePolicy::default(), &control)
            .unwrap_err();
        assert!(
            matches!(error, merman::svg::RenderError::Cancelled(_)),
            "{error}"
        );
    }
}
