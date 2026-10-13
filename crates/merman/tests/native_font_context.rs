#![cfg(all(feature = "native-fonts", feature = "diagram-flowchart"))]

use merman::svg::export::{NativeFontLimits, NativeFontOptions};
use merman::{OperationControl, RenderRequest, RenderTarget, Renderer, SvgEnvironment, SvgRequest};

fn context(
    limits: NativeFontLimits,
    control: &OperationControl,
) -> merman::svg::export::NativeFontContext {
    NativeFontOptions::default()
        .with_font_file(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../merman-export/tests/fonts/Cousine-Regular.ttf"),
        )
        .with_limits(limits)
        .load(control)
        .unwrap()
}

#[test]
fn facade_replays_native_failure_even_with_different_request_control() {
    let context = context(
        NativeFontLimits {
            max_label_bytes: 1,
            ..Default::default()
        },
        &OperationControl::new(),
    );
    let environment = SvgEnvironment::deterministic()
        .with_native_font_context(context.clone())
        .unwrap();
    let request = SvgRequest {
        environment,
        ..Default::default()
    };
    let output = Renderer::new().render(RenderRequest::new(
        "---\ntitle: Much too long\n---\nflowchart LR\nA-->B\n",
        RenderTarget::Svg(request),
        OperationControl::new(),
    ));
    assert!(
        output.is_err(),
        "host failure must never become successful fallback output"
    );
    assert!(context.check().is_err());
}

#[test]
fn facade_checks_context_cancellation_before_rendering() {
    let control = OperationControl::new();
    let context = context(Default::default(), &control);
    let environment = SvgEnvironment::deterministic()
        .with_native_font_context(context)
        .unwrap();
    control.cancel();
    let output = Renderer::new().render(RenderRequest::new(
        "flowchart LR\nA-->B\n",
        RenderTarget::Svg(SvgRequest {
            environment,
            ..Default::default()
        }),
        OperationControl::new(),
    ));
    assert!(output.is_err());
}

fn native_request() -> SvgRequest {
    SvgRequest {
        environment: SvgEnvironment::deterministic()
            .with_native_font_context(context(Default::default(), &OperationControl::new()))
            .unwrap(),
        ..Default::default()
    }
}

#[test]
fn native_fonts_reject_unmeasured_node_and_class_text_geometry() {
    for directive in [
        "style A letter-spacing:4px",
        "style A word-spacing:4px",
        "style A text-transform:uppercase",
        "style A white-space:nowrap",
        "classDef spaced letter-spacing:4px\nclass A spaced",
    ] {
        let source = format!("flowchart LR\nA[Measured label]-->B\n{directive}\n");
        let result = Renderer::new().render(RenderRequest::new(
            &source,
            RenderTarget::Svg(native_request()),
            OperationControl::new(),
        ));
        let error = result.expect_err("unsupported typography must fail admission");
        assert!(error.to_string().contains("text geometry style"), "{error}");
    }
}

#[cfg(feature = "diagram-class")]
#[test]
fn native_class_rejects_relative_and_unsupported_theme_sizes() {
    for header in ["classDiagram", "classDiagram-v2"] {
        for size in [
            r#""2em""#,
            r#""150%""#,
            r#""large""#,
            r#""24""#,
            "24",
            r#""0px""#,
        ] {
            let source = format!(
                "%%{{init: {{\"themeVariables\":{{\"fontSize\":{size}}}}}}}%%\n{header}\nclass User\nUser --> Account : owns\n"
            );
            let error = Renderer::new()
                .render(RenderRequest::new(
                    &source,
                    RenderTarget::Svg(native_request()),
                    OperationControl::new(),
                ))
                .expect_err("Class layout/CSS size mismatch must fail native admission");
            assert!(
                error.to_string().contains("native Class"),
                "{header}, {size}: {error}"
            );
            assert!(
                error.to_string().contains("themeVariables.fontSize"),
                "{header}, {size}: {error}"
            );
            assert!(error.to_string().contains("deterministic"), "{error}");
            let compatibility = Renderer::new().render(RenderRequest::new(
                &source,
                RenderTarget::Svg(SvgRequest::default()),
                OperationControl::new(),
            ));
            assert!(
                compatibility.is_ok(),
                "{header}, {size}: {:?}",
                compatibility.err()
            );
        }
    }
}

#[cfg(feature = "diagram-class")]
#[test]
fn native_class_admits_px_theme_size() {
    for header in ["classDiagram", "classDiagram-v2"] {
        let source = format!(
            "%%{{init: {{\"themeVariables\":{{\"fontSize\":\"24px\"}}}}}}%%\n{header}\nclass User\nUser --> Account : owns\n"
        );
        let result = Renderer::new().render(RenderRequest::new(
            &source,
            RenderTarget::Svg(native_request()),
            OperationControl::new(),
        ));
        assert!(result.is_ok(), "{header}: {:?}", result.err());
    }
}

#[test]
fn native_fonts_allow_paint_styles_and_property_names_inside_labels() {
    let source = "flowchart LR\nA[\"letter-spacing:4px and white-space:nowrap\"]-->B\nstyle A fill:#fff,stroke:#123456,stroke-width:2px\nclassDef painted color:red\nclass A painted\n";
    let result = Renderer::new().render(RenderRequest::new(
        source,
        RenderTarget::Svg(native_request()),
        OperationControl::new(),
    ));
    assert!(result.is_ok(), "{:?}", result.err());
}

#[test]
fn native_fonts_reject_css_injected_by_output_policy() {
    let mut request = native_request();
    request.pipeline = Some(
        merman::svg::SvgOutputPolicy {
            scoped_css: Some("text { letter-spacing: 4px; }".to_owned()),
            ..Default::default()
        }
        .pipeline(),
    );
    let result = Renderer::new().render(RenderRequest::new(
        "flowchart LR\nA[Measured label]-->B\n",
        RenderTarget::Svg(request),
        OperationControl::new(),
    ));
    let error = result.expect_err("post-layout CSS must fail native admission");
    assert!(error.to_string().contains("scoped CSS"), "{error}");
}

#[test]
fn deterministic_fonts_preserve_existing_custom_typography() {
    let result = Renderer::new().render(RenderRequest::new(
        "flowchart LR\nA[Measured label]-->B\nstyle A letter-spacing:4px\n",
        RenderTarget::Svg(SvgRequest::default()),
        OperationControl::new(),
    ));
    assert!(result.is_ok(), "{:?}", result.err());
}

#[cfg(any(
    feature = "diagram-class",
    feature = "diagram-er",
    feature = "diagram-state"
))]
#[test]
fn native_fonts_check_other_families_structured_style_slots() {
    let sources = [
        #[cfg(feature = "diagram-class")]
        "classDiagram\nclass User\nstyle User word-spacing:2px\n",
        #[cfg(feature = "diagram-er")]
        "erDiagram\nCUSTOMER ||--o{ ORDER : places\nstyle CUSTOMER letter-spacing:2px\n",
        #[cfg(feature = "diagram-state")]
        "stateDiagram-v2\nA-->B\nclassDef spaced word-spacing:2px\nclass A spaced\n",
    ];
    for source in sources {
        let result = Renderer::new().render(RenderRequest::new(
            source,
            RenderTarget::Svg(native_request()),
            OperationControl::new(),
        ));
        let error = result.expect_err("unmeasured text geometry must fail admission");
        assert!(
            error.to_string().contains("text geometry style"),
            "{source}: {error}"
        );
    }
}

#[cfg(feature = "png")]
#[test]
fn native_binary_fonts_apply_the_same_typography_admission() {
    for (source, accepted) in [
        (
            "flowchart LR\nA[Measured label]-->B\nstyle A word-spacing:4px\n",
            false,
        ),
        (
            "flowchart LR\nA[Measured label]-->B\nstyle A fill:#fff,stroke:red\n",
            true,
        ),
    ] {
        let result = Renderer::new().render(RenderRequest::new(
            source,
            RenderTarget::Png(merman::PngRequest {
                svg: native_request(),
                options: Default::default(),
            }),
            OperationControl::new(),
        ));
        assert_eq!(result.is_ok(), accepted, "{:?}", result.err());
    }
    let mut request = native_request();
    request.pipeline = Some(
        merman::svg::SvgOutputPolicy {
            scoped_css: Some("text { text-transform: uppercase; }".to_owned()),
            ..Default::default()
        }
        .pipeline(),
    );
    let result = Renderer::new().render(RenderRequest::new(
        "flowchart LR\nA-->B\n",
        RenderTarget::Png(merman::PngRequest {
            svg: request,
            options: Default::default(),
        }),
        OperationControl::new(),
    ));
    assert!(result.err().unwrap().to_string().contains("scoped CSS"));
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
mod binary_context_contract {
    use super::*;
    use merman::svg::export::NativeFontContext;

    fn targets(svg: SvgRequest, painting: Option<NativeFontContext>) -> Vec<RenderTarget> {
        vec![
            #[cfg(feature = "png")]
            RenderTarget::Png(merman::PngRequest {
                svg: svg.clone(),
                options: merman::svg::export::RasterOptions {
                    font_context: painting.clone(),
                    ..Default::default()
                },
            }),
            #[cfg(feature = "jpeg")]
            RenderTarget::Jpeg(merman::JpegRequest {
                svg: svg.clone(),
                options: merman::svg::export::RasterOptions {
                    font_context: painting.clone(),
                    ..Default::default()
                },
            }),
            #[cfg(feature = "pdf")]
            RenderTarget::Pdf(merman::PdfRequest {
                svg: svg.clone(),
                options: merman::svg::export::PdfOptions {
                    font_context: painting.clone(),
                    ..Default::default()
                },
            }),
        ]
    }

    fn render(
        source: &str,
        target: RenderTarget,
    ) -> Result<merman::RenderOutput, merman::RenderError> {
        Renderer::new().render(RenderRequest::new(source, target, OperationControl::new()))
    }

    #[test]
    fn different_contexts_are_rejected_even_for_the_same_font_bytes() {
        let measuring = context(Default::default(), &OperationControl::new());
        let painting = context(Default::default(), &OperationControl::new());
        assert_ne!(measuring, painting);
        let svg = SvgRequest {
            environment: SvgEnvironment::deterministic()
                .with_native_font_context(measuring)
                .unwrap(),
            ..Default::default()
        };
        for target in targets(svg, Some(painting)) {
            let error = render("flowchart LR\nA[Hello]", target).unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("measurement and painting conflict"),
                "{error}"
            );
        }
    }

    #[test]
    fn painting_only_context_participates_in_native_measurement() {
        for mut target in targets(
            SvgRequest::default(),
            Some(context(
                NativeFontLimits {
                    max_label_bytes: 1,
                    ..Default::default()
                },
                &OperationControl::new(),
            )),
        ) {
            let painting = match &mut target {
                #[cfg(feature = "png")]
                RenderTarget::Png(request) => &mut request.options.font_context,
                #[cfg(feature = "jpeg")]
                RenderTarget::Jpeg(request) => &mut request.options.font_context,
                #[cfg(feature = "pdf")]
                RenderTarget::Pdf(request) => &mut request.options.font_context,
                _ => unreachable!("binary context test only creates binary targets"),
            };
            *painting = Some(
                painting
                    .as_ref()
                    .unwrap()
                    .for_operation(&OperationControl::new()),
            );
            let error = render("flowchart LR\nA[Long label]", target).unwrap_err();
            let merman::RenderError::ResourceLimitExceeded(limit) = error else {
                panic!("expected active native measurement resource limit, got {error}");
            };
            assert_eq!(limit.id, "native_font_label_bytes");
            assert_eq!(limit.phase, "native-font-measurement");
            assert_eq!(limit.actual, 47);
            assert_eq!(limit.maximum, 1);
            assert_eq!(limit.cause, merman::ResourceLimitCause::Ceiling);
        }
    }

    #[test]
    fn painting_only_context_runs_typography_admission_before_layout() {
        for target in targets(
            SvgRequest::default(),
            Some(context(Default::default(), &OperationControl::new())),
        ) {
            let error =
                render("flowchart LR\nA[Hello]\nstyle A word-spacing:4px", target).unwrap_err();
            assert!(error.to_string().contains("text geometry style"), "{error}");
        }
    }

    struct DecliningHost;

    impl merman::svg::HostTextMeasurer for DecliningHost {
        fn measure(
            &self,
            _: merman::svg::HostTextMeasurementRequest<'_>,
        ) -> merman::svg::HostMeasurementResult {
            panic!("conflicting host must not be invoked");
        }
    }

    #[test]
    fn painting_context_does_not_override_explicit_measurement_policies() {
        let identity = merman::svg::TextMeasurementProfileIdentity::new(
            merman::svg::MeasurementProfileId::new("merman.test-host").unwrap(),
            "test@1",
        )
        .unwrap();
        for policy in [
            merman::svg::TextMeasurementPolicy::host_display(
                identity,
                std::sync::Arc::new(DecliningHost),
                merman::svg::TextMeasurementPhase::ALL,
            ),
            merman::svg::TextMeasurementPolicy::deterministic(),
        ] {
            let painting = context(Default::default(), &OperationControl::new());
            for environment in [
                SvgEnvironment::deterministic().with_text_measurement_policy(policy.clone()),
                SvgEnvironment::deterministic()
                    .with_native_font_context(painting.clone())
                    .unwrap()
                    .with_text_measurement_policy(policy),
            ] {
                let svg = SvgRequest {
                    environment,
                    ..Default::default()
                };
                for target in targets(svg, Some(painting.clone())) {
                    let error = render("flowchart LR\nA[Hello]", target).unwrap_err();
                    assert!(
                        error
                            .to_string()
                            .contains("explicit text measurement policy"),
                        "{error}"
                    );
                }
            }
        }
    }

    #[test]
    fn shared_environment_and_painting_context_succeeds() {
        let context = context(Default::default(), &OperationControl::new());
        let svg = SvgRequest {
            environment: SvgEnvironment::deterministic()
                .with_native_font_context(context.clone())
                .unwrap(),
            ..Default::default()
        };
        for target in targets(svg, Some(context)) {
            let output = render("flowchart LR\nA[Hello]", target);
            assert!(output.is_ok(), "{:?}", output.err());
        }
    }

    #[test]
    fn either_context_source_is_shared_with_the_other_stage() {
        let context = context(Default::default(), &OperationControl::new());
        let svg = SvgRequest {
            environment: SvgEnvironment::deterministic()
                .with_native_font_context(context.clone())
                .unwrap(),
            ..Default::default()
        };
        for target in targets(svg, None)
            .into_iter()
            .chain(targets(SvgRequest::default(), Some(context)))
        {
            let output = render("flowchart LR\nA[Hello]", target);
            assert!(output.is_ok(), "{:?}", output.err());
        }
    }

    #[test]
    fn absent_context_keeps_compatibility_typography_admission() {
        for target in targets(SvgRequest::default(), None) {
            let output = render("flowchart LR\nA[Hello]\nstyle A word-spacing:4px", target);
            assert!(output.is_ok(), "{:?}", output.err());
        }
    }
}
