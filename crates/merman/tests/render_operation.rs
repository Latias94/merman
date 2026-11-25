#[cfg(feature = "svg")]
use std::borrow::Cow;
#[cfg(feature = "svg")]
use std::sync::Arc;
use std::time::Duration;

#[cfg(feature = "svg")]
use merman::theme::{Appearance, BuiltinPreset, ThemeMode, ThemeSelection};

use merman::{
    OperationControl, OperationPhase, ParseOptions, PresentationArtifact, RenderError,
    RenderOutput, RenderRequest, Renderer,
    model::resources::{InputResourceLimitId, InputResourcePolicy},
};

#[cfg(feature = "ascii")]
use merman::{
    AsciiRequest, OperationResourceDomain, OperationResourceLimitExceeded,
    OperationResourceOverride, OperationResourceProvenance, RenderTarget,
    ascii::{AsciiResourceLimitId, AsciiResourcePolicy},
};

#[cfg(feature = "svg")]
#[derive(Debug)]
struct CancellingTextMeasurer {
    control: OperationControl,
}

#[cfg(feature = "svg")]
impl merman::svg::HostTextMeasurer for CancellingTextMeasurer {
    fn measure(
        &self,
        _request: merman::svg::HostTextMeasurementRequest<'_>,
    ) -> merman::svg::HostMeasurementResult {
        self.control.cancel();
        Ok(None)
    }
}

#[cfg(feature = "svg")]
#[derive(Debug)]
struct FixedHostTextMeasurer;

#[cfg(feature = "svg")]
#[derive(Debug)]
struct MissingFragmentPostprocessor;

#[cfg(feature = "svg")]
impl merman::svg::SvgPostprocessor for MissingFragmentPostprocessor {
    fn name(&self) -> &'static str {
        "test-missing-fragment"
    }

    fn process<'a>(
        &self,
        svg: Cow<'a, str>,
        _ctx: &merman::svg::SvgPostprocessContext<'_>,
    ) -> merman::svg::RenderResult<Cow<'a, str>> {
        let mut svg = svg.into_owned();
        let root_end = svg
            .rfind("</svg>")
            .expect("renderer output should contain a closing SVG root");
        svg.insert_str(
            root_end,
            r#"<path data-test-missing-fragment="true" fill="url(#missing-resource)" d="M0 0h1v1z"/>"#,
        );
        Ok(Cow::Owned(svg))
    }
}

#[cfg(feature = "svg")]
impl merman::svg::HostTextMeasurer for FixedHostTextMeasurer {
    fn measure(
        &self,
        request: merman::svg::HostTextMeasurementRequest<'_>,
    ) -> merman::svg::HostMeasurementResult {
        let raw_width = request.text.chars().count().max(1) as f64 * 8.0;
        let max_width = request
            .max_width
            .filter(|width| width.is_finite() && *width > 0.0);
        let line_count = max_width
            .map(|width| (raw_width / width).ceil() as usize)
            .unwrap_or(1)
            .max(request.text.lines().count())
            .max(1)
            .min(request.text.len().saturating_add(1));
        let metrics = merman::svg::TextMetrics {
            width: max_width.map_or(raw_width, |width| raw_width.min(width)),
            height: line_count as f64 * 20.0,
            line_count,
        };
        let measurement = match request.operation.required_result_kind() {
            merman::svg::TextMeasurementResultKind::Metrics => {
                merman::svg::HostTextMeasurement::Metrics(metrics)
            }
            merman::svg::TextMeasurementResultKind::Length => {
                let length = match request.operation {
                    merman::svg::TextMeasurementOperation::RawBBoxHeight
                    | merman::svg::TextMeasurementOperation::SimpleBBoxHeight
                    | merman::svg::TextMeasurementOperation::TspanBBoxHeight => metrics.height,
                    merman::svg::TextMeasurementOperation::CreateTextBBoxYOffset
                    | merman::svg::TextMeasurementOperation::CreateTextMiddleBBoxYOffset => 0.0,
                    _ => raw_width,
                };
                merman::svg::HostTextMeasurement::Length(length)
            }
            merman::svg::TextMeasurementResultKind::HorizontalExtents => {
                merman::svg::HostTextMeasurement::HorizontalExtents {
                    left: raw_width / 2.0,
                    right: raw_width / 2.0,
                }
            }
            merman::svg::TextMeasurementResultKind::WrappedWithRawWidth => {
                merman::svg::HostTextMeasurement::WrappedWithRawWidth {
                    metrics,
                    raw_width: Some(raw_width),
                }
            }
        };
        Ok(Some(measurement))
    }
}

#[cfg(feature = "svg")]
fn studio_dark() -> ThemeSelection {
    ThemeSelection::preset(BuiltinPreset::Studio, Some(Appearance::Dark))
}

#[cfg(feature = "svg")]
fn render_host_measured_document(
    portability: merman::svg::ThemePortabilityRequirement,
) -> merman::RenderedDocument {
    render_host_measured_document_with_profile(portability, "merman.test-fixed-display-host")
}

#[cfg(feature = "svg")]
fn render_host_measured_document_with_profile(
    portability: merman::svg::ThemePortabilityRequirement,
    profile_id: &str,
) -> merman::RenderedDocument {
    let identity = merman::svg::TextMeasurementProfileIdentity::new(
        merman::svg::MeasurementProfileId::new(profile_id).expect("test profile id"),
        "render-operation-test@1",
    )
    .expect("test profile identity");
    let measurement = merman::svg::TextMeasurementPolicy::host_display(
        identity,
        Arc::new(FixedHostTextMeasurer),
        merman::svg::TextMeasurementPhase::ALL,
    );
    let request = merman::SvgRequest {
        environment: merman::SvgEnvironment::deterministic()
            .with_text_measurement_policy(measurement)
            .with_theme_portability_requirement(portability),
        ..merman::SvgRequest::default()
    };
    let output = Renderer::new()
        .render(
            RenderRequest::document(
                "---\nconfig:\n  htmlLabels: false\n  flowchart:\n    htmlLabels: false\n---\nflowchart TD\nA[Start] --> B[Done]",
                OperationControl::new(),
                request,
            )
            .with_native_theme(studio_dark()),
        )
        .expect("host-measured document should complete before target admission");
    let RenderOutput::Document(Some(document)) = output else {
        panic!("expected a host-measured document");
    };
    document
}

#[test]
fn semantic_request_uses_the_canonical_operation_runner() {
    let output = Renderer::new()
        .render(RenderRequest::semantic(
            "flowchart TD\nA[Start] --> B[Done]",
            OperationControl::new(),
        ))
        .expect("semantic request should succeed");

    let RenderOutput::Semantic(Some(artifact)) = output else {
        panic!("expected a semantic artifact");
    };
    assert_eq!(artifact.diagram_type(), "flowchart-v2");
    assert_eq!(artifact.semantic_kind(), "flowchart");
}

#[test]
fn cancelled_semantic_request_returns_no_partial_artifact() {
    let control = OperationControl::new();
    control.cancel();

    let error = Renderer::new()
        .render(RenderRequest::semantic("flowchart TD\nA --> B", control))
        .expect_err("cancelled operation must not return an artifact");

    assert!(matches!(
        error,
        RenderError::Cancelled(cancelled)
            if cancelled.phase == OperationPhase::Admission
                && cancelled.reason == merman::CancelReason::Requested
    ));
}

#[test]
fn expired_deadline_is_reported_as_deadline_cancellation() {
    let control = OperationControl::new().with_deadline(Duration::ZERO);

    let error = Renderer::new()
        .render(RenderRequest::semantic("flowchart TD\nA --> B", control))
        .expect_err("expired operation must stop before parsing");

    assert!(matches!(
        error,
        RenderError::Cancelled(cancelled)
            if cancelled.reason == merman::CancelReason::DeadlineExceeded
    ));
}

#[test]
fn prepare_presentation_delegates_to_the_same_runner() {
    let artifact: Option<PresentationArtifact> = Renderer::new()
        .prepare_presentation("info", OperationControl::new())
        .expect("prepare should succeed");
    assert_eq!(
        artifact.expect("info should be detected").semantic_kind(),
        "info"
    );
}

#[test]
fn renderer_defaults_apply_when_the_request_does_not_override_them() {
    let renderer = Renderer::new().with_resource_policy(
        InputResourcePolicy::default()
            .with_limit(InputResourceLimitId::MaxSourceBytes, 4)
            .expect("valid source limit"),
    );

    let error = renderer
        .render(RenderRequest::semantic(
            "flowchart TD\nA --> B",
            OperationControl::new(),
        ))
        .expect_err("the renderer's source limit must apply to one-shot requests");

    assert!(matches!(
        error,
        RenderError::ResourceLimitExceeded(limit)
            if limit.id == "max_source_bytes" && limit.maximum == 4
    ));
}

#[test]
fn source_resource_terminal_replays_before_later_cancellation() {
    let renderer = Renderer::new().with_resource_policy(
        InputResourcePolicy::default()
            .with_limit(InputResourceLimitId::MaxSourceBytes, 4)
            .expect("valid source limit"),
    );
    let control = OperationControl::new();

    let first = renderer
        .render(RenderRequest::semantic(
            "flowchart TD\nA --> B",
            control.clone(),
        ))
        .expect_err("the source limit must reject the request");
    assert!(matches!(
        &first,
        RenderError::ResourceLimitExceeded(limit)
            if limit.id == "max_source_bytes"
                && limit.phase == "source"
                && limit.actual == 20
                && limit.maximum == 4
    ));

    control.cancel();
    let replayed = renderer
        .render(RenderRequest::semantic("info", control))
        .expect_err("the first source terminal must remain sticky");
    assert!(matches!(
        replayed,
        RenderError::ResourceLimitExceeded(limit)
            if limit.id == "max_source_bytes"
                && limit.phase == "source"
                && limit.actual == 20
                && limit.maximum == 4
    ));
}

#[test]
fn request_overrides_take_precedence_over_renderer_defaults() {
    let renderer = Renderer::new()
        .with_parse_options(ParseOptions::strict())
        .with_resource_policy(
            InputResourcePolicy::default()
                .with_limit(InputResourceLimitId::MaxSourceBytes, 4)
                .expect("valid source limit"),
        );
    let request_resources = InputResourcePolicy::default()
        .with_limit(InputResourceLimitId::MaxSourceBytes, 4_096)
        .expect("valid source limit");

    let output = renderer
        .render(
            RenderRequest::semantic("flowchart TD\nA --> B", OperationControl::new())
                .with_parse_options(ParseOptions::lenient())
                .with_resource_policy(request_resources),
        )
        .expect("request overrides should replace renderer defaults");

    assert!(matches!(output, RenderOutput::Semantic(Some(_))));
}

#[cfg(feature = "svg")]
const RESOURCE_POLICY_FLOWCHART: &str = "---\nconfig:\n  layout: dagre\n---\nflowchart TD\nA --> B";

#[cfg(feature = "svg")]
fn svg_request_with_resources(resources: merman::svg::RenderResourcePolicy) -> merman::SvgRequest {
    merman::SvgRequest {
        environment: merman::SvgEnvironment::deterministic().with_resource_policy(resources),
        ..Default::default()
    }
}

#[cfg(feature = "svg")]
fn source_exceeding_interactive_limit() -> String {
    let limit = InputResourcePolicy::default()
        .value(InputResourceLimitId::MaxSourceBytes)
        .expect("interactive source limit");
    format!("{RESOURCE_POLICY_FLOWCHART}\n%%{}\n", " ".repeat(limit))
}

#[cfg(feature = "svg")]
#[test]
fn graphical_targets_use_svg_input_limits_without_duplicate_configuration() {
    let source = source_exceeding_interactive_limit();
    let request = svg_request_with_resources(merman::svg::RenderResourcePolicy::trusted_native());
    let targets = [
        ("SVG", merman::RenderTarget::Svg(request.clone())),
        (
            "layout JSON",
            merman::RenderTarget::LayoutJson(request.clone()),
        ),
        ("SVG plan", merman::RenderTarget::SvgPlan(request.clone())),
        #[cfg(feature = "png")]
        (
            "PNG",
            merman::RenderTarget::Png(merman::PngRequest {
                svg: request.clone(),
                options: Default::default(),
            }),
        ),
        #[cfg(feature = "jpeg")]
        (
            "JPEG",
            merman::RenderTarget::Jpeg(merman::JpegRequest {
                svg: request.clone(),
                options: Default::default(),
            }),
        ),
        #[cfg(feature = "pdf")]
        (
            "PDF",
            merman::RenderTarget::Pdf(merman::PdfRequest {
                svg: request.clone(),
                options: Default::default(),
            }),
        ),
    ];
    let renderer = Renderer::new();
    for (name, target) in targets {
        let output = renderer
            .render(RenderRequest::new(&source, target, OperationControl::new()))
            .unwrap_or_else(|error| panic!("{name} must use the target's input policy: {error}"));
        match output {
            RenderOutput::Svg(Some(_))
            | RenderOutput::LayoutJson(Some(_))
            | RenderOutput::SvgPlan(Some(_)) => {}
            #[cfg(feature = "png")]
            RenderOutput::Png(Some(_)) => {}
            #[cfg(feature = "jpeg")]
            RenderOutput::Jpeg(Some(_)) => {}
            #[cfg(feature = "pdf")]
            RenderOutput::Pdf(Some(_)) => {}
            other => panic!("{name} must produce an artifact: {other:?}"),
        }
    }

    let error = renderer
        .prepare_presentation(&source, OperationControl::new())
        .expect_err("format-neutral preparation must retain the renderer's input policy");
    assert!(
        matches!(error, RenderError::ResourceLimitExceeded(limit) if limit.id == "max_source_bytes")
    );
}

#[cfg(feature = "svg")]
#[test]
fn explicit_input_policies_take_precedence_over_svg_environment_defaults() {
    let source = source_exceeding_interactive_limit();
    let default_input = InputResourcePolicy::default();
    let tight_input = default_input
        .with_limit(InputResourceLimitId::MaxSourceBytes, 4)
        .expect("valid source limit");
    let native = merman::svg::RenderResourcePolicy::trusted_native();
    let default_limit = default_input
        .value(InputResourceLimitId::MaxSourceBytes)
        .unwrap();
    for (name, renderer_input, request_input, source, expected_limit) in [
        (
            "explicit renderer limit",
            Some(tight_input),
            None,
            RESOURCE_POLICY_FLOWCHART,
            Some(4),
        ),
        (
            "explicit default renderer profile",
            Some(default_input),
            None,
            source.as_str(),
            Some(default_limit),
        ),
        (
            "request narrows renderer",
            Some(*native.input_policy()),
            Some(tight_input),
            RESOURCE_POLICY_FLOWCHART,
            Some(4),
        ),
        (
            "request widens renderer",
            Some(tight_input),
            Some(*native.input_policy()),
            source.as_str(),
            None,
        ),
        (
            "request overrides target",
            None,
            Some(tight_input),
            RESOURCE_POLICY_FLOWCHART,
            Some(4),
        ),
    ] {
        let renderer = match renderer_input {
            Some(input) => Renderer::new().with_resource_policy(input),
            None => Renderer::new(),
        };
        let request = RenderRequest::svg(
            source,
            OperationControl::new(),
            svg_request_with_resources(native),
        );
        let request = match request_input {
            Some(input) => request.with_resource_policy(input),
            None => request,
        };
        let result = renderer.render(request);
        match expected_limit {
            Some(maximum) => {
                let error = result.expect_err(name);
                assert!(
                    matches!(
                        error,
                        RenderError::ResourceLimitExceeded(limit)
                            if limit.id == "max_source_bytes"
                                && limit.maximum == maximum as u64
                                && limit.provenance.as_ref().is_some_and(|provenance| {
                                    provenance.domain == merman::OperationResourceDomain::Input
                                        && provenance.profile == Some(default_input.profile())
                                })
                    ),
                    "{name}"
                );
            }
            None => assert!(
                matches!(result.expect(name), RenderOutput::Svg(Some(_))),
                "{name}"
            ),
        }
    }
}

#[cfg(feature = "svg")]
#[test]
fn svg_policy_retains_layout_and_output_limits_after_source_admission() {
    use merman::svg::{RenderResourcePolicy, ResourceLimitId};

    let source = source_exceeding_interactive_limit();
    for (limit_id, explicit_input, source) in [
        (
            ResourceLimitId::MaxLayoutWorkUnits,
            Some(InputResourcePolicy::default()),
            RESOURCE_POLICY_FLOWCHART,
        ),
        (ResourceLimitId::MaxSvgBytes, None, source.as_str()),
    ] {
        let resources = RenderResourcePolicy::trusted_native()
            .with_limit(limit_id, 1)
            .expect("valid render limit");
        let renderer = match explicit_input {
            Some(input) => Renderer::new().with_resource_policy(input),
            None => Renderer::new(),
        };
        let error = renderer
            .render(RenderRequest::svg(
                source,
                OperationControl::new(),
                svg_request_with_resources(resources),
            ))
            .expect_err("the target's rendering limit must still reject the operation");
        assert!(matches!(
            error,
            RenderError::ResourceLimitExceeded(limit)
                if limit.id == limit_id.as_str()
                    && limit.maximum == 1
                    && limit.provenance.as_ref().is_some_and(|provenance| {
                        provenance.domain == merman::OperationResourceDomain::Render
                            && provenance.profile == Some(resources.profile())
                    })
        ));
    }
}

#[cfg(feature = "svg")]
#[test]
fn svg_policy_budget_is_independent_for_each_operation() {
    use merman::svg::{RenderResourcePolicy, ResourceLimitId};

    let renderer = Renderer::new();
    let resources = RenderResourcePolicy::trusted_native();
    let RenderOutput::Svg(Some(baseline)) = renderer
        .render(RenderRequest::svg(
            RESOURCE_POLICY_FLOWCHART,
            OperationControl::new(),
            svg_request_with_resources(resources),
        ))
        .expect("baseline rendering")
    else {
        panic!("expected an SVG artifact");
    };
    let work = baseline.evidence().layout_work_units();
    assert!(work > 0);
    let request = svg_request_with_resources(
        resources
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, work)
            .expect("valid exact work limit"),
    );
    for _ in 0..2 {
        let RenderOutput::Svg(Some(output)) = renderer
            .render(RenderRequest::svg(
                RESOURCE_POLICY_FLOWCHART,
                OperationControl::new(),
                request.clone(),
            ))
            .expect("each new operation must receive the full budget")
        else {
            panic!("expected an SVG artifact");
        };
        assert_eq!(output.evidence().layout_work_units(), work);
    }
}

#[cfg(all(feature = "svg", feature = "diagram-class", feature = "layout-elk"))]
#[test]
fn default_svg_policy_admits_nested_class_diagrams_with_independent_budgets() {
    use merman::svg::{RenderResourcePolicy, ResourceLimitId};

    let source = include_str!("../benches/fixtures/class_nested_namespaces.mmd");
    let renderer = Renderer::new();
    let mut previous = None;
    for _ in 0..2 {
        let RenderOutput::Svg(Some(output)) = renderer
            .render(RenderRequest::svg(
                source,
                OperationControl::new(),
                merman::SvgRequest::default(),
            ))
            .expect("the default policy must admit the ordinary nested class diagram")
        else {
            panic!("expected an SVG artifact");
        };
        let work = output.evidence().layout_work_units();
        let maximum = RenderResourcePolicy::default()
            .value(ResourceLimitId::MaxLayoutWorkUnits)
            .expect("default work must remain finite");
        assert!(work > 0 && work <= maximum, "unexpected work: {work}");
        let current = (output.svg().to_string(), work);
        if let Some(previous) = &previous {
            assert_eq!(
                previous, &current,
                "each operation must render the same SVG with its own budget"
            );
        }
        previous = Some(current);
    }
}

#[cfg(feature = "svg")]
#[test]
fn typed_svg_targets_share_the_prepared_operation() {
    let renderer = Renderer::new();
    let source = "flowchart TD\nA[Start] --> B[Done]";

    let layout = renderer
        .render(RenderRequest::layout_json(
            source,
            OperationControl::new(),
            merman::SvgRequest::default(),
        ))
        .expect("layout target should succeed");
    let RenderOutput::LayoutJson(Some(layout)) = layout else {
        panic!("expected typed layout JSON");
    };
    assert!(layout.layout().get("layout").is_some());

    let plan = renderer
        .render(RenderRequest::svg_plan(
            source,
            OperationControl::new(),
            merman::SvgRequest::default(),
        ))
        .expect("SVG plan target should succeed");
    let RenderOutput::SvgPlan(Some(plan)) = plan else {
        panic!("expected typed SVG capability plan");
    };
    assert!(plan.is_ready());
}

#[cfg(feature = "svg")]
#[test]
fn layout_json_rejects_require_portable_without_target_admission() {
    let request = merman::SvgRequest {
        environment: merman::SvgEnvironment::deterministic().with_theme_portability_requirement(
            merman::svg::ThemePortabilityRequirement::RequirePortable,
        ),
        ..merman::SvgRequest::default()
    };
    let renderer = Renderer::new();

    let error = renderer
        .render(RenderRequest::layout_json(
            "flowchart TD\nA[Start] --> B[Done]",
            OperationControl::new(),
            request.clone(),
        ))
        .expect_err("layout JSON has no target portability admission");
    assert_eq!(
        error.to_string(),
        "render target `layout-json` does not support RequirePortable because it has no target portability admission"
    );
    assert!(matches!(
        &error,
        RenderError::PortabilityUnavailableForTarget {
            target: "layout-json"
        }
    ));

    let mut written = Vec::new();
    let error = renderer
        .write_layout_json(
            RenderRequest::layout_json(
                "flowchart TD\nA[Start] --> B[Done]",
                OperationControl::new(),
                request.clone(),
            ),
            &mut written,
            false,
        )
        .expect_err("the writer must reject unsupported portability before emission");
    assert!(matches!(
        error,
        RenderError::PortabilityUnavailableForTarget {
            target: "layout-json"
        }
    ));
    assert!(written.is_empty());

    let semantic = renderer
        .prepare_presentation(
            "flowchart TD\nA[Start] --> B[Done]",
            OperationControl::new(),
        )
        .expect("semantic preparation should succeed")
        .expect("flowchart should be detected");
    let error = semantic
        .render(merman::RenderTarget::LayoutJson(request))
        .expect_err("prepared layout JSON must use the same portability guard");
    assert!(matches!(
        error,
        RenderError::PortabilityUnavailableForTarget {
            target: "layout-json"
        }
    ));
}

#[cfg(all(feature = "svg", feature = "diagram-flowchart"))]
#[test]
fn layout_json_writer_finishes_once_emission_commits() {
    struct CancellingWriter<'a> {
        control: &'a OperationControl,
        bytes: Vec<u8>,
    }

    impl std::io::Write for CancellingWriter<'_> {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.bytes.extend_from_slice(bytes);
            self.control.cancel();
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let renderer = Renderer::new();
    let source = "flowchart TD\nA[Start] --> B[Done]";
    let request =
        |control| RenderRequest::layout_json(source, control, merman::SvgRequest::default());
    let RenderOutput::LayoutJson(Some(layout)) =
        renderer.render(request(OperationControl::new())).unwrap()
    else {
        panic!("expected managed geometry");
    };
    let mut expected = Vec::new();
    layout.layout().write_json_pretty(&mut expected).unwrap();

    let control = OperationControl::new();
    let mut writer = CancellingWriter {
        control: &control,
        bytes: Vec::new(),
    };
    assert!(
        renderer
            .write_layout_json(request(control.clone()), &mut writer, true)
            .unwrap()
    );
    assert!(control.is_cancelled());
    assert_eq!(writer.bytes, expected);
}

#[cfg(feature = "svg")]
#[test]
fn layout_json_cancellation_precedes_require_portable_rejection() {
    let control = OperationControl::new();
    let artifact = Renderer::new()
        .prepare_presentation("flowchart TD\nA --> B", control.clone())
        .expect("semantic preparation should succeed")
        .expect("flowchart should be detected");
    control.cancel();

    let request = merman::SvgRequest {
        environment: merman::SvgEnvironment::deterministic().with_theme_portability_requirement(
            merman::svg::ThemePortabilityRequirement::RequirePortable,
        ),
        ..merman::SvgRequest::default()
    };
    let error = artifact
        .render(merman::RenderTarget::LayoutJson(request))
        .expect_err("cancellation must win over target portability rejection");

    assert!(matches!(
        error,
        RenderError::Cancelled(cancelled)
            if cancelled.phase == OperationPhase::Layout
                && cancelled.reason == merman::CancelReason::Requested
    ));
}

#[cfg(feature = "svg")]
#[test]
fn layout_json_deadline_precedes_require_portable_rejection() {
    let control = OperationControl::new();
    let artifact = Renderer::new()
        .prepare_presentation("flowchart TD\nA --> B", control.clone())
        .expect("semantic preparation should succeed")
        .expect("flowchart should be detected");
    assert!(control.set_deadline(Duration::ZERO));

    let request = merman::SvgRequest {
        environment: merman::SvgEnvironment::deterministic().with_theme_portability_requirement(
            merman::svg::ThemePortabilityRequirement::RequirePortable,
        ),
        ..merman::SvgRequest::default()
    };
    let error = artifact
        .render(merman::RenderTarget::LayoutJson(request))
        .expect_err("deadline must win over target portability rejection");

    assert!(matches!(
        error,
        RenderError::Cancelled(cancelled)
            if cancelled.phase == OperationPhase::Layout
                && cancelled.reason == merman::CancelReason::DeadlineExceeded
    ));
}

#[cfg(all(feature = "svg", feature = "diagram-sequence"))]
#[test]
fn completed_svg_reports_family_and_native_theme_identity() {
    let request = merman::SvgRequest::default();

    let output = Renderer::new()
        .render(
            RenderRequest::svg(
                "sequenceDiagram\nAlice->>Bob: Hello",
                OperationControl::new(),
                request,
            )
            .with_native_theme(studio_dark()),
        )
        .expect("themed sequence SVG should render");
    let RenderOutput::Svg(Some(output)) = output else {
        panic!("expected a themed sequence SVG");
    };

    assert!(output.svg().starts_with("<svg"));
    assert_eq!(
        output.evidence().family_id(),
        merman::model::family::DiagramFamilyId::SEQUENCE
    );
    let outcome = output.theme_outcome();
    assert_eq!(outcome.mode, ThemeMode::MermanNative);
    assert_eq!(outcome.preset_id, Some(BuiltinPreset::Studio));
    assert_eq!(outcome.appearance, Some(Appearance::Dark));
}

#[cfg(feature = "svg")]
#[test]
fn unthemed_svg_reports_mermaid_parity() {
    let output = Renderer::new()
        .render(RenderRequest::svg(
            "flowchart TD\nA[Start] --> B[Done]",
            OperationControl::new(),
            merman::SvgRequest::default(),
        ))
        .expect("unthemed SVG should render");
    let RenderOutput::Svg(Some(output)) = output else {
        panic!("expected an SVG output");
    };

    assert_eq!(output.theme_outcome().mode, ThemeMode::MermaidParity);
    assert!(output.theme_outcome().notices.is_empty());
    assert!(!output.evidence().output_mutated());
}

#[cfg(all(feature = "svg", feature = "diagram-info"))]
#[test]
fn unsupported_native_treatments_are_quiet_in_best_effort_and_rejected_in_portable_mode() {
    use merman::svg::{RenderError as SvgRenderError, ThemePortabilityRequirement};

    let renderer = Renderer::new();
    for (preset, expected) in [
        (
            BuiltinPreset::Studio,
            "This diagram family does not support theme elevation.",
        ),
        (
            BuiltinPreset::Monograph,
            "This diagram family does not support categorical theme patterns.",
        ),
    ] {
        let render = |requirement| {
            let request = merman::SvgRequest {
                environment: merman::SvgEnvironment::deterministic()
                    .with_theme_portability_requirement(requirement),
                ..Default::default()
            }
            .with_native_theme(ThemeSelection::preset(preset, None));
            renderer.render(RenderRequest::svg("info", OperationControl::new(), request))
        };
        let RenderOutput::Svg(Some(output)) =
            render(ThemePortabilityRequirement::BestEffort).unwrap()
        else {
            panic!("expected a best-effort SVG");
        };
        assert_eq!(output.theme_outcome().preset_id, Some(preset));
        assert!(output.theme_outcome().notices.is_empty());
        let error = render(ThemePortabilityRequirement::RequirePortable).unwrap_err();
        assert!(
            matches!(error, RenderError::Svg(SvgRenderError::UnsupportedNativeThemeFeature {
            family_id: merman::model::family::DiagramFamilyId::INFO, message,
        }) if message == expected)
        );
    }
}

#[cfg(all(feature = "svg", feature = "diagram-packet"))]
#[test]
fn portable_packet_rejects_a_requested_corner_radius() {
    use merman::svg::{RenderError as SvgRenderError, ThemePortabilityRequirement};
    use merman::theme::ThemeCompiler;

    let theme = ThemeCompiler::new()
        .compile_json(
            br##"{"schema_version":2,"appearances":{"light":{"seed":{
                "canvas":"#FFFFFF","ink":"#111111","accent":"#324466"
            }}},"overrides":[{"families":["packet"],"form":{"radius":12}}]}"##,
            None,
        )
        .unwrap();
    let request = merman::SvgRequest {
        environment: merman::SvgEnvironment::deterministic()
            .with_theme_portability_requirement(ThemePortabilityRequirement::RequirePortable),
        ..Default::default()
    }
    .with_native_theme(ThemeSelection::compiled(theme));
    let error = Renderer::new()
        .render(RenderRequest::svg(
            "packet-beta\n0-7: \"Header\"",
            OperationControl::new(),
            request,
        ))
        .unwrap_err();
    assert!(matches!(
        error,
        RenderError::Svg(SvgRenderError::UnsupportedNativeThemeFeature { message, .. })
            if message == "This diagram family does not support theme corner radii."
    ));
}

#[cfg(all(feature = "svg", feature = "diagram-class"))]
#[test]
fn standalone_native_class_svg_preserves_font_portability() {
    let request = merman::SvgRequest {
        pipeline: Some(merman::svg::SvgPipeline::resvg_safe()),
        ..merman::SvgRequest::default()
    };
    let output = Renderer::new()
        .render(
            RenderRequest::svg(
                "classDiagram\nclass A\nclass B\nA --> B\n",
                OperationControl::new(),
                request,
            )
            .with_native_theme(studio_dark()),
        )
        .expect("best-effort standalone Class SVG should retain both evidence axes");
    let RenderOutput::Svg(Some(output)) = output else {
        panic!("expected a standalone Class SVG output");
    };

    assert_eq!(
        output.evidence().family_id(),
        merman::model::family::DiagramFamilyId::CLASS
    );
    assert_eq!(
        output.theme_outcome().preset_id,
        Some(BuiltinPreset::Studio)
    );
    let admission = output.admission();
    assert_eq!(
        admission.status(),
        merman::TargetAdmissionStatus::HostDependent
    );
    assert!(!admission.status().is_portable());
    assert!(
        admission
            .reasons()
            .contains(&merman::TargetAdmissionReason::SvgFontsNotSelfContained)
    );
}

#[cfg(all(feature = "svg", feature = "diagram-state"))]
#[test]
fn custom_postprocessing_is_visible_in_render_evidence() {
    let theme = studio_dark();
    let request = merman::SvgRequest {
        pipeline: Some(
            merman::svg::SvgPipeline::parity()
                .with_postprocessor(merman::svg::RootBackgroundPostprocessor::new("#111827")),
        ),
        ..Default::default()
    };
    let output = Renderer::new()
        .render(
            RenderRequest::svg(
                "stateDiagram-v2\n[*] --> Ready\nReady --> [*]",
                OperationControl::new(),
                request,
            )
            .with_native_theme(theme),
        )
        .expect("postprocessed themed SVG should render in best-effort mode");
    let RenderOutput::Svg(Some(output)) = output else {
        panic!("expected a postprocessed SVG");
    };

    assert_eq!(output.theme_outcome().mode, ThemeMode::MermanNative);
    assert!(output.evidence().output_mutated());
    assert!(output.svg().contains("#111827"));
}

#[cfg(all(feature = "svg", feature = "diagram-flowchart"))]
#[test]
fn rendered_document_retains_selected_theme_and_portability() {
    let output = Renderer::new()
        .render(
            RenderRequest::document(
                "flowchart TD\nA[Start] --> B[Done]",
                OperationControl::new(),
                merman::SvgRequest::default(),
            )
            .with_native_theme(studio_dark()),
        )
        .expect("rendered document should complete");
    let RenderOutput::Document(Some(document)) = output else {
        panic!("expected a rendered document");
    };

    assert!(document.svg().starts_with("<svg"));
    assert_eq!(
        document.portability().family_id(),
        document.evidence().family_id()
    );
    assert_eq!(
        document.standalone_svg_admission().artifact_kind(),
        merman::RenderArtifactKind::Svg
    );
    let outcome = document.theme_outcome();
    assert_eq!(outcome.mode, ThemeMode::MermanNative);
    assert_eq!(outcome.preset_id, Some(BuiltinPreset::Studio));
    assert_eq!(outcome.appearance, Some(Appearance::Dark));
}

#[cfg(all(feature = "svg", feature = "math"))]
#[test]
fn rendered_document_binds_prepared_math_terminal_proof() {
    let output = Renderer::new()
        .render(RenderRequest::document(
            "flowchart TD\nA[$$x^2$$]",
            OperationControl::new(),
            merman::SvgRequest::default(),
        ))
        .expect("prepared-math document should render");
    let RenderOutput::Document(Some(document)) = output else {
        panic!("expected a rendered document");
    };

    assert!(document.portability().prepared_math_evidence_valid());
    assert!(
        document
            .portability()
            .prepared_math_terminal_proof_complete()
    );
    assert!(
        !document
            .portability()
            .reasons()
            .contains(&merman::TargetAdmissionReason::PreparedMathEvidenceInvalid)
    );
    assert!(
        !document
            .portability()
            .reasons()
            .contains(&merman::TargetAdmissionReason::PreparedMathTerminalProofIncomplete)
    );
    assert!(document.svg().contains("merman-prepared-math-native"));
}

#[cfg(all(feature = "png", feature = "diagram-flowchart"))]
#[test]
fn native_export_admits_the_theme_canvas_texture() {
    let theme = ThemeSelection::preset(BuiltinPreset::Blueprint, None);
    let output = Renderer::new()
        .render(
            RenderRequest::document(
                "flowchart TD\nA[Root canvas]",
                OperationControl::new(),
                merman::SvgRequest::default(),
            )
            .with_native_theme(theme),
        )
        .expect("root canvas document should render");
    let RenderOutput::Document(Some(document)) = output else {
        panic!("expected a rendered document");
    };

    let svg = roxmltree::Document::parse(document.svg()).unwrap();
    let canvas = svg
        .descendants()
        .find(|node| node.attribute("class") == Some("merman-theme-canvas"))
        .expect("native canvas group");
    assert!(
        canvas
            .descendants()
            .any(|node| node.has_tag_name("pattern"))
    );
    let png = document
        .export_png(
            &merman::svg::export::RasterOptions::default(),
            OperationControl::new(),
        )
        .expect("qualified root canvas capabilities should export to PNG");
    assert!(png.bytes().starts_with(b"\x89PNG\r\n\x1a\n"));
    assert_eq!(png.theme_outcome(), document.theme_outcome());
}

#[cfg(feature = "svg")]
#[test]
fn host_display_measurements_make_document_and_svg_target_host_dependent() {
    let document =
        render_host_measured_document(merman::svg::ThemePortabilityRequirement::BestEffort);

    assert!(
        document
            .evidence()
            .measurement()
            .entries()
            .iter()
            .any(|entry| {
                entry.count() > 0
                    && entry.provenance().source == merman::svg::TextMeasurementSource::Host
            })
    );
    assert!(document.portability().is_host_dependent());
    assert!(
        document
            .portability()
            .reasons()
            .contains(&merman::TargetAdmissionReason::HostDependentTextLayout)
    );
    assert_eq!(
        document.standalone_svg_admission().status(),
        merman::TargetAdmissionStatus::HostDependent
    );
    assert!(
        document
            .standalone_svg_admission()
            .reasons()
            .contains(&merman::TargetAdmissionReason::HostDependentTextLayout)
    );
}

#[cfg(feature = "svg")]
#[test]
fn require_portable_svg_target_rejects_actual_host_display_measurements() {
    const SOURCE: &str = "---\nconfig:\n  htmlLabels: false\n  flowchart:\n    htmlLabels: false\n---\nflowchart TD\nA[Start] --> B[Done]";
    let identity = merman::svg::TextMeasurementProfileIdentity::new(
        merman::svg::MeasurementProfileId::new("merman.test-strict-svg-host")
            .expect("test profile id"),
        "render-operation-test@1",
    )
    .expect("test profile identity");
    let measurement = merman::svg::TextMeasurementPolicy::host_display(
        identity,
        Arc::new(FixedHostTextMeasurer),
        merman::svg::TextMeasurementPhase::ALL,
    );
    let request = merman::SvgRequest {
        environment: merman::SvgEnvironment::deterministic()
            .with_text_measurement_policy(measurement)
            .with_theme_portability_requirement(
                merman::svg::ThemePortabilityRequirement::RequirePortable,
            ),
        pipeline: Some(merman::svg::SvgPipeline::resvg_safe()),
        ..merman::SvgRequest::default()
    };

    let error = Renderer::new()
        .render(
            RenderRequest::svg(SOURCE, OperationControl::new(), request.clone())
                .with_native_theme(studio_dark()),
        )
        .expect_err("strict SVG admission must reject actual host measurement provenance");
    let data_error = Renderer::new()
        .render_svg_data(
            RenderRequest::svg(SOURCE, OperationControl::new(), request)
                .with_native_theme(studio_dark()),
        )
        .expect_err("strict SVG data must reject the same host measurement provenance");
    let RenderError::TargetAdmission(data_error) = data_error else {
        panic!("strict SVG data rejection must retain target evidence: {data_error}");
    };
    let RenderError::TargetAdmission(error) = error else {
        panic!("strict SVG rejection must retain target evidence: {error}");
    };
    assert_eq!(data_error.receipt(), error.receipt());
    assert_eq!(
        error.receipt().artifact_kind(),
        merman::RenderArtifactKind::Svg
    );
    assert_eq!(
        error.receipt().status(),
        merman::TargetAdmissionStatus::HostDependent
    );
    assert!(
        error
            .receipt()
            .reasons()
            .contains(&merman::TargetAdmissionReason::HostDependentTextLayout)
    );
}

#[cfg(feature = "svg")]
#[test]
fn portable_svg_data_matches_the_result_for_a_scene_without_font_dependencies() {
    let request = merman::SvgRequest {
        environment: merman::SvgEnvironment::deterministic().with_theme_portability_requirement(
            merman::svg::ThemePortabilityRequirement::RequirePortable,
        ),
        pipeline: Some(merman::svg::SvgPipeline::resvg_safe()),
        ..merman::SvgRequest::default()
    };
    let renderer = Renderer::new();
    let output = renderer
        .render(RenderRequest::svg(
            "classDiagram\ndirection TB\n",
            OperationControl::new(),
            request.clone(),
        ))
        .expect("an empty Class scene has no unresolved font dependency");
    let RenderOutput::Svg(Some(output)) = output else {
        panic!("expected portable standalone SVG");
    };
    assert_eq!(
        output.admission().status(),
        merman::TargetAdmissionStatus::Portable
    );
    let data = renderer
        .render_svg_data(RenderRequest::svg(
            "classDiagram\ndirection TB\n",
            OperationControl::new(),
            request,
        ))
        .expect("data path admits the same portable scene")
        .expect("Class diagram detected");
    assert_eq!(data, output.svg());
}

#[cfg(feature = "svg")]
#[test]
fn ordinary_svg_leaves_native_admission_unverified() {
    for pipeline in [None, Some(merman::svg::SvgPipeline::parity())] {
        for theme in [None, Some(studio_dark())] {
            let mut request = RenderRequest::svg(
                "flowchart TD\nA[Alpha] --> B[Beta]",
                OperationControl::new(),
                merman::SvgRequest {
                    pipeline: pipeline.clone(),
                    ..merman::SvgRequest::default()
                },
            );
            if let Some(theme) = theme {
                request = request.with_native_theme(theme);
            }
            let RenderOutput::Svg(Some(output)) = Renderer::new().render(request).unwrap() else {
                panic!("expected standalone SVG output");
            };
            assert_eq!(output.admission().status().id(), "unverified");
            assert!(!output.admission().status().is_portable());
            let reasons: Vec<_> = output
                .admission()
                .reasons()
                .iter()
                .map(|reason| reason.id())
                .collect();
            assert!(reasons.contains(&"svg_terminal_unverified"));
            assert!(!reasons.contains(&"svg_terminal_validation_failed"));
            assert!(!reasons.contains(&"svg_fonts_not_self_contained"));
        }
    }
}

#[cfg(feature = "svg")]
#[test]
fn svg_admission_preserves_the_caller_selected_pipeline() {
    const SOURCE: &str = "---\nconfig:\n  htmlLabels: true\n  flowchart:\n    htmlLabels: true\n---\nflowchart TD\nA[Alpha] --> B[Beta]";

    fn render_best_effort(pipeline: merman::svg::SvgPipeline) -> merman::SvgOutput {
        let request = merman::SvgRequest {
            pipeline: Some(pipeline),
            ..merman::SvgRequest::default()
        };
        let output = Renderer::new()
            .render(
                RenderRequest::svg(SOURCE, OperationControl::new(), request)
                    .with_native_theme(studio_dark()),
            )
            .expect("best-effort SVG should retain the selected pipeline output");
        let RenderOutput::Svg(Some(output)) = output else {
            panic!("expected standalone SVG output");
        };
        output
    }

    fn render_strict(pipeline: merman::svg::SvgPipeline) -> merman::TargetAdmissionError {
        let request = merman::SvgRequest {
            environment: merman::SvgEnvironment::deterministic()
                .with_theme_portability_requirement(
                    merman::svg::ThemePortabilityRequirement::RequirePortable,
                ),
            pipeline: Some(pipeline),
            ..merman::SvgRequest::default()
        };
        let error = Renderer::new()
            .render(
                RenderRequest::svg(SOURCE, OperationControl::new(), request)
                    .with_native_theme(studio_dark()),
            )
            .expect_err("strict SVG should reject an incompatible selected pipeline artifact");
        let RenderError::TargetAdmission(error) = error else {
            panic!("strict SVG rejection should retain target evidence: {error}");
        };
        error
    }

    let parity = render_best_effort(merman::svg::SvgPipeline::parity());
    assert!(parity.svg().contains("<foreignObject"), "{}", parity.svg());
    assert!(
        !parity
            .svg()
            .contains(r#"data-merman-foreignobject="fallback""#),
        "{}",
        parity.svg()
    );

    let parity_receipt = parity.admission();
    assert_eq!(
        parity_receipt.status(),
        merman::TargetAdmissionStatus::Unverified
    );
    assert!(
        parity_receipt
            .reasons()
            .contains(&merman::TargetAdmissionReason::SvgTerminalUnverified)
    );
    let parity_error = render_strict(merman::svg::SvgPipeline::parity());
    assert_eq!(
        parity_error.receipt().status(),
        merman::TargetAdmissionStatus::Rejected
    );

    let readable = render_best_effort(merman::svg::SvgPipeline::readable());
    assert!(
        readable.svg().contains("<foreignObject"),
        "{}",
        readable.svg()
    );
    assert!(
        readable
            .svg()
            .contains(r#"data-merman-foreignobject="fallback""#),
        "{}",
        readable.svg()
    );

    let readable_error = render_strict(merman::svg::SvgPipeline::readable());
    assert_eq!(
        readable_error.receipt().status(),
        merman::TargetAdmissionStatus::Rejected
    );

    let resvg_safe = render_best_effort(merman::svg::SvgPipeline::resvg_safe());
    assert!(
        !resvg_safe.svg().contains("<foreignObject"),
        "{}",
        resvg_safe.svg()
    );
    assert_ne!(parity.svg(), readable.svg());
    assert_ne!(readable.svg(), resvg_safe.svg());
}

#[cfg(feature = "svg")]
#[test]
fn ordinary_svg_reports_postprocessing_without_native_observation() {
    const SOURCE: &str = "---\nconfig:\n  htmlLabels: false\n  flowchart:\n    htmlLabels: false\n---\nflowchart TD\nA[Alpha] --> B[Beta]";

    let render = |pipeline| {
        let output = Renderer::new()
            .render(
                RenderRequest::svg(
                    SOURCE,
                    OperationControl::new(),
                    merman::SvgRequest {
                        pipeline: Some(pipeline),
                        ..merman::SvgRequest::default()
                    },
                )
                .with_native_theme(studio_dark()),
            )
            .expect("best-effort SVG should return the selected artifact");
        let RenderOutput::Svg(Some(output)) = output else {
            panic!("expected standalone SVG output");
        };
        output
    };

    let baseline = render(merman::svg::SvgPipeline::parity());
    assert!(baseline.theme_outcome().notices.is_empty());
    let output =
        render(merman::svg::SvgPipeline::parity().with_postprocessor(MissingFragmentPostprocessor));
    assert!(output.svg().contains("data-test-missing-fragment"));
    let admission = output.admission();
    assert_ne!(output.svg(), baseline.svg());
    assert!(output.evidence().output_mutated());
    assert_eq!(
        admission.status(),
        merman::TargetAdmissionStatus::Unverified
    );
    assert!(
        admission
            .reasons()
            .contains(&merman::TargetAdmissionReason::SvgTerminalUnverified)
    );
    assert!(
        !admission
            .reasons()
            .contains(&merman::TargetAdmissionReason::SvgTerminalValidationFailed)
    );

    let strict_error = Renderer::new()
        .render(
            RenderRequest::svg(
                SOURCE,
                OperationControl::new(),
                merman::SvgRequest {
                    environment: merman::SvgEnvironment::deterministic()
                        .with_theme_portability_requirement(
                            merman::svg::ThemePortabilityRequirement::RequirePortable,
                        ),
                    pipeline: Some(
                        merman::svg::SvgPipeline::parity()
                            .with_postprocessor(MissingFragmentPostprocessor),
                    ),
                    ..merman::SvgRequest::default()
                },
            )
            .with_native_theme(studio_dark()),
        )
        .expect_err("strict SVG should reject the same exact invalid artifact");
    let RenderError::TargetAdmission(strict_error) = strict_error else {
        panic!("strict rejection should retain target evidence: {strict_error}");
    };
    assert!(
        strict_error
            .receipt()
            .reasons()
            .contains(&merman::TargetAdmissionReason::SvgTerminalValidationFailed)
    );
}

#[cfg(feature = "png")]
#[test]
fn host_display_measurements_make_png_target_host_dependent() {
    let document =
        render_host_measured_document(merman::svg::ThemePortabilityRequirement::BestEffort);

    let output = document
        .export_png(
            &merman::svg::export::RasterOptions::default(),
            OperationControl::new(),
        )
        .expect("best-effort PNG export should retain host-dependent evidence");
    assert_eq!(
        output.admission().status(),
        merman::TargetAdmissionStatus::HostDependent
    );
    assert!(
        output
            .admission()
            .reasons()
            .contains(&merman::TargetAdmissionReason::HostDependentTextLayout)
    );
}

#[cfg(feature = "png")]
#[test]
fn require_portable_rejects_actual_host_display_measurements() {
    let document =
        render_host_measured_document(merman::svg::ThemePortabilityRequirement::RequirePortable);

    let error = document
        .export_png(
            &merman::svg::export::RasterOptions::default(),
            OperationControl::new(),
        )
        .expect_err("strict PNG admission must reject actual host measurement provenance");
    let RenderError::TargetAdmission(error) = error else {
        panic!("strict host measurement rejection must retain target evidence: {error}");
    };
    assert_eq!(
        error.receipt().status(),
        merman::TargetAdmissionStatus::HostDependent
    );
    assert!(
        error
            .receipt()
            .reasons()
            .contains(&merman::TargetAdmissionReason::HostDependentTextLayout)
    );
}

#[cfg(all(feature = "svg", feature = "diagram-class"))]
#[test]
fn require_portable_svg_accepts_valid_postprocessing() {
    const SOURCE: &str = "classDiagram\ndirection TB\n";
    let request = merman::SvgRequest {
        environment: merman::SvgEnvironment::deterministic().with_theme_portability_requirement(
            merman::svg::ThemePortabilityRequirement::RequirePortable,
        ),
        pipeline: Some(
            merman::svg::SvgPipeline::resvg_safe()
                .with_postprocessor(merman::svg::RootBackgroundPostprocessor::new("#111827")),
        ),
        ..merman::SvgRequest::default()
    };
    let renderer = Renderer::new();
    let output = renderer
        .render(RenderRequest::svg(
            SOURCE,
            OperationControl::new(),
            request.clone(),
        ))
        .expect("safe postprocessing preserves portability for a scene without text");
    let RenderOutput::Svg(Some(output)) = output else {
        panic!("expected a standalone SVG");
    };
    assert!(output.admission().status().is_portable());
    assert!(output.evidence().output_mutated());
    assert!(output.svg().contains("#111827"));

    let data = renderer
        .render_svg_data(RenderRequest::svg(SOURCE, OperationControl::new(), request))
        .expect("the data path admits the same postprocessed output")
        .expect("Class diagram detected");
    assert_eq!(data, output.svg());
}

#[cfg(all(feature = "svg", feature = "math", feature = "diagram-flowchart"))]
#[test]
fn require_portable_svg_rejects_postprocessing_that_invalidates_prepared_math() {
    let request = merman::SvgRequest {
        environment: merman::SvgEnvironment::deterministic().with_theme_portability_requirement(
            merman::svg::ThemePortabilityRequirement::RequirePortable,
        ),
        pipeline: Some(
            merman::svg::SvgPipeline::resvg_safe()
                .with_postprocessor(merman::svg::RootBackgroundPostprocessor::new("#111827")),
        ),
        ..merman::SvgRequest::default()
    };
    let error = Renderer::new()
        .render(RenderRequest::svg(
            "flowchart TD\nA[$$x^2$$]",
            OperationControl::new(),
            request,
        ))
        .expect_err("opaque postprocessing cannot preserve prepared math");
    assert!(
        matches!(
            &error,
            RenderError::Svg(merman::svg::RenderError::SvgPostprocess { pass, .. })
                if pass == "prepared-math-terminal-receipt"
        ),
        "{error}"
    );
}

#[cfg(all(feature = "svg", feature = "diagram-state"))]
#[test]
fn strict_document_completion_allows_an_explicit_standalone_svg_evaluation() {
    let theme = studio_dark();
    let request = merman::SvgRequest {
        environment: merman::SvgEnvironment::deterministic().with_theme_portability_requirement(
            merman::svg::ThemePortabilityRequirement::RequirePortable,
        ),
        ..Default::default()
    };

    let output = Renderer::new()
        .render(
            RenderRequest::document(
                "stateDiagram-v2\n[*] --> Ready\nReady --> [*]",
                OperationControl::new(),
                request,
            )
            .with_native_theme(theme),
        )
        .expect("document completion must not be preempted by one target's admission");
    let RenderOutput::Document(Some(document)) = output else {
        panic!("expected a completed document");
    };
    assert_eq!(
        document.standalone_svg_admission().artifact_kind(),
        merman::RenderArtifactKind::Svg
    );
    assert_ne!(
        document.standalone_svg_admission().status(),
        merman::TargetAdmissionStatus::Portable
    );
    assert!(!document.standalone_svg_admission().reasons().is_empty());
}

#[cfg(all(feature = "png", feature = "diagram-state"))]
#[test]
fn strict_document_png_projection_is_evaluated_by_the_png_target() {
    let theme = studio_dark();
    let svg = merman::SvgRequest {
        environment: merman::SvgEnvironment::deterministic().with_theme_portability_requirement(
            merman::svg::ThemePortabilityRequirement::RequirePortable,
        ),
        ..Default::default()
    };

    let output = Renderer::new()
        .render(
            RenderRequest::document(
                "stateDiagram-v2\n[*] --> Ready\nReady --> [*]",
                OperationControl::new(),
                svg,
            )
            .with_native_theme(theme),
        )
        .expect("one target's admission must not preempt document completion");
    let RenderOutput::Document(Some(document)) = output else {
        panic!("expected a completed document");
    };
    let error = document
        .export_png(
            &merman::svg::export::RasterOptions::default(),
            OperationControl::new(),
        )
        .expect_err("strict PNG admission should reject host-dependent text");
    let RenderError::TargetAdmission(error) = error else {
        panic!("strict PNG rejection must preserve target-owned evidence: {error}");
    };
    assert_eq!(
        error.receipt().artifact_kind(),
        merman::RenderArtifactKind::Png,
        "the standalone SVG receipt must not preempt target-local PNG admission"
    );
    assert_ne!(
        error.receipt().status(),
        merman::TargetAdmissionStatus::Portable
    );
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
#[test]
fn one_rendered_document_projects_all_native_targets_with_atomic_receipts() {
    let output = Renderer::new()
        .render(RenderRequest::document(
            "info",
            OperationControl::new(),
            merman::SvgRequest::default(),
        ))
        .expect("rendered document should complete");
    let RenderOutput::Document(Some(document)) = output else {
        panic!("expected a rendered document");
    };

    let png = document
        .export_png(
            &merman::svg::export::RasterOptions::default(),
            OperationControl::new(),
        )
        .expect("PNG projection should succeed");
    let jpeg = document
        .export_jpeg(
            &merman::svg::export::RasterOptions::default(),
            OperationControl::new(),
        )
        .expect("JPEG projection should succeed");
    let pdf = document
        .export_pdf(
            &merman::svg::export::PdfOptions::default(),
            OperationControl::new(),
        )
        .expect("PDF projection should succeed");

    assert!(png.bytes().starts_with(b"\x89PNG\r\n\x1a\n"));
    assert!(jpeg.bytes().starts_with(b"\xff\xd8\xff"));
    assert!(pdf.bytes().starts_with(b"%PDF-"));
    assert_eq!(png.plan(), png.export_report().raster());
    assert_eq!(jpeg.plan(), jpeg.export_report().raster());
    assert_eq!(pdf.plan(), pdf.export_report().filters());

    for (kind, receipt) in [
        (merman::RenderArtifactKind::Png, png.admission()),
        (merman::RenderArtifactKind::Jpeg, jpeg.admission()),
        (merman::RenderArtifactKind::Pdf, pdf.admission()),
    ] {
        assert_eq!(receipt.artifact_kind(), kind);

        assert_ne!(
            receipt.status(),
            merman::TargetAdmissionStatus::Rejected,
            "a successfully projected built-in target must not silently reject its own artifact"
        );
    }
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
#[test]
fn rendered_document_prepares_each_native_target_before_encoding() {
    let output = Renderer::new()
        .render(RenderRequest::document(
            "info",
            OperationControl::new(),
            merman::SvgRequest::default(),
        ))
        .expect("rendered document should complete");
    let RenderOutput::Document(Some(document)) = output else {
        panic!("expected a rendered document");
    };

    let png = document
        .prepare_png_export(
            &merman::svg::export::RasterOptions::default(),
            OperationControl::new(),
        )
        .expect("PNG should prepare");
    assert_eq!(png.plan(), png.export_report().raster());
    let png = png.encode().expect("prepared PNG should encode");

    let jpeg = document
        .prepare_jpeg_export(
            &merman::svg::export::RasterOptions::default(),
            OperationControl::new(),
        )
        .expect("JPEG should prepare");
    assert_eq!(jpeg.plan(), jpeg.export_report().raster());
    let jpeg = jpeg.encode().expect("prepared JPEG should encode");

    let pdf = document
        .prepare_pdf_export(
            &merman::svg::export::PdfOptions::default(),
            OperationControl::new(),
        )
        .expect("PDF should prepare");
    assert_eq!(pdf.plan(), pdf.export_report().filters());
    let pdf = pdf.encode().expect("prepared PDF should encode");

    for receipt in [png.admission(), jpeg.admission(), pdf.admission()] {
        assert_ne!(receipt.status(), merman::TargetAdmissionStatus::Rejected);
    }
}

#[cfg(all(feature = "png", feature = "diagram-info"))]
#[test]
fn png_output_retains_render_evidence_and_native_theme_outcome() {
    let svg = merman::SvgRequest::default();

    let output = Renderer::new()
        .render(
            RenderRequest::png(
                "info",
                OperationControl::new(),
                merman::PngRequest {
                    svg,
                    options: merman::svg::export::RasterOptions::default(),
                },
            )
            .with_native_theme(studio_dark()),
        )
        .expect("themed PNG should render");
    let RenderOutput::Png(Some(output)) = output else {
        panic!("expected a themed PNG");
    };

    assert!(output.bytes().starts_with(b"\x89PNG\r\n\x1a\n"));
    assert_eq!(output.export_report().raster(), output.plan());
    assert_eq!(
        output.admission().artifact_kind(),
        merman::RenderArtifactKind::Png
    );
    assert_eq!(
        output.evidence().family_id(),
        merman::model::family::DiagramFamilyId::INFO
    );
    let outcome = output.theme_outcome();
    assert_eq!(outcome.mode, ThemeMode::MermanNative);
    assert_eq!(outcome.preset_id, Some(BuiltinPreset::Studio));
    assert_eq!(outcome.appearance, Some(Appearance::Dark));
}

#[cfg(all(feature = "svg", feature = "math"))]
#[test]
fn svg_evidence_carries_preparation_owned_capability_requirements() {
    let request = merman::SvgRequest::default();
    let output = Renderer::new()
        .render(RenderRequest::svg(
            "sequenceDiagram\nA->>B: $$x$$",
            OperationControl::new(),
            request,
        ))
        .expect("math SVG render should succeed");
    let RenderOutput::Svg(Some(output)) = output else {
        panic!("expected typed SVG output");
    };

    assert_eq!(
        output.evidence().required_capabilities(),
        &[merman::svg::RenderCapability::Math]
    );
}

#[cfg(all(feature = "svg", feature = "compat-json"))]
#[test]
fn semantic_artifact_exposes_compatibility_json_without_family_types() {
    let artifact = Renderer::new()
        .prepare_presentation("flowchart TD\nA --> B", OperationControl::new())
        .expect("parse should succeed")
        .expect("diagram should be detected");
    let json = artifact
        .compatibility_json()
        .expect("compatibility JSON should be projected");
    assert_eq!(json["type"], "flowchart-v2");
}

#[test]
#[cfg(feature = "compat-json")]
fn compatibility_json_observes_the_artifact_operation_control() {
    let control = OperationControl::new();
    let artifact = Renderer::new()
        .prepare_presentation("flowchart TD\nA --> B", control.clone())
        .expect("parse should succeed")
        .expect("diagram should be detected");
    control.cancel();

    let error = artifact
        .compatibility_json()
        .expect_err("semantic projection must observe cancellation");
    assert!(matches!(
        error,
        RenderError::Cancelled(cancelled)
            if cancelled.phase == OperationPhase::Semantic
    ));
}

#[cfg(feature = "svg")]
#[test]
fn layout_json_observes_cancellation_after_layout_preparation() {
    let control = OperationControl::new();
    let identity = merman::svg::TextMeasurementProfileIdentity::new(
        merman::svg::MeasurementProfileId::new("merman.test-cancelling-host")
            .expect("static profile id"),
        "render-operation-test@1",
    )
    .expect("static profile identity");
    let policy = merman::svg::TextMeasurementPolicy::host_display(
        identity,
        Arc::new(CancellingTextMeasurer {
            control: control.clone(),
        }),
        merman::svg::TextMeasurementPhase::ALL,
    );
    let request = merman::SvgRequest {
        environment: merman::SvgEnvironment::deterministic().with_text_measurement_policy(policy),
        ..Default::default()
    };

    let error = Renderer::new()
        .render(RenderRequest::layout_json(
            "flowchart TD\nA[Start] --> B[Done]",
            control,
            request,
        ))
        .expect_err("layout projection must not return after its control terminates");
    assert!(matches!(
        error,
        RenderError::Cancelled(cancelled)
            if cancelled.phase == OperationPhase::Layout
                && cancelled.reason == merman::CancelReason::Requested
    ));
}

#[cfg(feature = "svg")]
#[test]
fn svg_request_cancellation_is_not_reported_as_a_resource_limit() {
    let control = OperationControl::new();
    control.cancel();
    let error = Renderer::new()
        .render(RenderRequest::svg(
            "flowchart TD\nA --> B",
            control,
            merman::SvgRequest::default(),
        ))
        .expect_err("cancelled SVG request must stop");
    assert!(matches!(error, RenderError::Cancelled(_)));
}

#[cfg(feature = "svg")]
#[test]
fn svg_backend_errors_use_canonical_render_error_classification() {
    let cancelled = merman::OperationCancelled {
        phase: OperationPhase::Emit,
        reason: merman::CancelReason::Requested,
    };
    let error = RenderError::from(merman::svg::RenderError::Cancelled(cancelled));
    assert!(matches!(error, RenderError::Cancelled(actual) if actual == cancelled));

    let policy = merman::svg::RenderResourcePolicy::interactive()
        .with_limit(merman::svg::ResourceLimitId::MaxSvgBytes, 1)
        .expect("max SVG bytes is overridable");
    let resource = policy
        .check_svg_bytes("<svg/>", merman::svg::ResourceLimitPhase::SvgOutput)
        .expect_err("the backend policy should reject the oversized SVG");
    let error = RenderError::from(merman::svg::RenderError::ResourceLimitExceeded(resource));
    assert!(matches!(
        error,
        RenderError::ResourceLimitExceeded(limit)
            if limit.id == "max_svg_bytes"
                && limit.phase == "svg_output"
                && limit.actual == 6
                && limit.maximum == 1
                && limit.cause == merman::ResourceLimitCause::Ceiling
    ));

    let error = RenderError::from(merman::svg::RenderError::SvgPostprocess {
        pass: "test".to_string(),
        message: "backend failure".to_string(),
    });
    assert!(matches!(
        error,
        RenderError::Svg(merman::svg::RenderError::SvgPostprocess { .. })
    ));
}

#[cfg(any(feature = "png", feature = "jpeg", feature = "pdf"))]
#[test]
fn export_backend_errors_use_canonical_render_error_classification() {
    let cancelled = merman::OperationCancelled {
        phase: OperationPhase::Export,
        reason: merman::CancelReason::Requested,
    };
    let error = RenderError::from(merman::svg::export::ExportError::Cancelled(cancelled));
    assert!(matches!(error, RenderError::Cancelled(actual) if actual == cancelled));

    let error = RenderError::from(merman::svg::export::ExportError::EmbeddedImageLimit {
        limit_name: "max_bytes_per_image",
        actual: 2,
        max: 1,
    });
    assert!(matches!(
        error,
        RenderError::ResourceLimitExceeded(limit)
            if limit.id
                == merman::svg::export::MAX_EMBEDDED_IMAGE_BYTES_RESOURCE_LIMIT_ID
                && limit.phase == "embedded_image_decode"
                && limit.actual == 2
                && limit.maximum == 1
                && limit.cause == merman::ResourceLimitCause::Ceiling
                && limit.provenance.as_ref().is_some_and(|provenance| {
                    provenance.domain == merman::OperationResourceDomain::Export
                        && provenance.profile.is_none()
                        && provenance.explicit_overrides.is_empty()
                })
    ));

    let error = RenderError::from(merman::svg::export::ExportError::SvgParse);
    assert!(matches!(
        error,
        RenderError::Export(merman::svg::export::ExportError::SvgParse)
    ));

    let error = RenderError::from(merman::svg::export::ExportError::SvgConversionLimit {
        limit_name: "svg_backend_tree_depth",
        actual: 2,
        max: 1,
    });
    assert!(matches!(
        error,
        RenderError::ResourceLimitExceeded(limit)
            if limit.id == "svg_backend_tree_depth"
                && limit.phase == "svg_postprocess"
                && limit.actual == 2
                && limit.maximum == 1
                && limit.provenance.as_ref().is_some_and(|provenance| {
                    provenance.domain == merman::OperationResourceDomain::Render
                        && provenance.profile.is_none()
                        && provenance.explicit_overrides.is_empty()
                })
    ));

    let error = RenderError::from(
        merman::svg::export::ExportError::ResourceArithmeticOverflow {
            limit_id: "max_export_bytes",
            phase: "export",
            actual: u64::MAX,
            max: 1024,
        },
    );
    assert!(matches!(
        error,
        RenderError::ResourceLimitExceeded(limit)
            if limit.id == "max_export_bytes"
                && limit.phase == "export"
                && limit.actual == u64::MAX
                && limit.maximum == 1024
                && limit.cause == merman::ResourceLimitCause::ArithmeticOverflow
                && limit.provenance.as_ref().is_some_and(|provenance| {
                    provenance.domain == merman::OperationResourceDomain::Export
                        && provenance.profile.is_none()
                        && provenance.explicit_overrides.is_empty()
                })
    ));

    let provenance = merman::OperationResourceProvenance::new(
        merman::OperationResourceDomain::Render,
        Some(merman::model::resources::ResourceProfile::Constrained),
        [merman::OperationResourceOverride {
            id: "max_svg_bytes",
            value: 17,
        }],
    );
    let terminal = merman::model::OperationLedgerError::ArithmeticOverflow {
        id: "max_svg_bytes",
        phase: OperationPhase::Postprocess,
        resource_phase: "svg_postprocess",
        actual: u64::MAX,
        maximum: 17,
        provenance: provenance.clone(),
    };
    let error = RenderError::from(merman::svg::export::ExportError::OperationResourceTerminal(
        terminal,
    ));
    assert!(matches!(
        error,
        RenderError::ResourceLimitExceeded(limit)
            if limit.id == "max_svg_bytes"
                && limit.phase == "svg_postprocess"
                && limit.actual == u64::MAX
                && limit.maximum == 17
                && limit.cause == merman::ResourceLimitCause::ArithmeticOverflow
                && limit.provenance == Some(provenance)
    ));
}

#[cfg(feature = "ascii")]
#[test]
fn ascii_backend_errors_use_canonical_render_error_classification() {
    let cancelled = merman::OperationCancelled {
        phase: OperationPhase::Layout,
        reason: merman::CancelReason::Requested,
    };
    let error = RenderError::from(merman::ascii::AsciiError::Cancelled(cancelled));
    assert!(matches!(error, RenderError::Cancelled(actual) if actual == cancelled));

    let error = RenderError::from(merman::ascii::AsciiError::UnsupportedDiagram {
        diagram_type: "test".to_string(),
    });
    assert!(matches!(
        error,
        RenderError::Ascii(merman::ascii::AsciiError::UnsupportedDiagram { .. })
    ));
}

#[cfg(feature = "ascii")]
#[test]
fn ascii_request_honors_target_local_grid_policy_at_exact_and_minus_one() {
    // The canonical layout is five columns by eight rows.
    const EXACT_GRID_CELLS: usize = 40;
    let source = "flowchart TD\nA --> B";
    let exact_resources = AsciiResourcePolicy::default()
        .with_limit(AsciiResourceLimitId::MaxGridCells, EXACT_GRID_CELLS)
        .expect("the exact grid limit must be valid");

    let output = Renderer::new()
        .render(RenderRequest::ascii(
            source,
            OperationControl::new(),
            AsciiRequest {
                resources: exact_resources,
                ..Default::default()
            },
        ))
        .expect("the exact target-local grid policy must admit the request");
    let RenderOutput::Ascii(Some(output)) = output else {
        panic!("the exact target-local grid policy must produce ASCII output");
    };
    assert_eq!(
        output.primary_extent,
        Some(merman::ascii::AsciiExtent::new(5, 8)),
        "the pinned resource boundary must match the canonical grid"
    );

    let below_resources = AsciiResourcePolicy::default()
        .with_limit(AsciiResourceLimitId::MaxGridCells, EXACT_GRID_CELLS - 1)
        .expect("the max-minus-one grid limit must be valid");
    let error = Renderer::new()
        .render(RenderRequest::ascii(
            source,
            OperationControl::new(),
            AsciiRequest {
                resources: below_resources,
                ..Default::default()
            },
        ))
        .expect_err("the max-minus-one target-local grid policy must reject the request");
    assert!(matches!(
        error,
        RenderError::ResourceLimitExceeded(limit)
            if limit.id == "max_ascii_grid_cells"
                && limit.phase == "ascii_layout"
                && limit.actual == EXACT_GRID_CELLS as u64
                && limit.maximum == (EXACT_GRID_CELLS - 1) as u64
                && limit.cause == merman::ResourceLimitCause::Ceiling
                && limit.provenance.as_ref().is_some_and(|provenance| {
                    provenance.domain == OperationResourceDomain::Ascii
                        && provenance.profile
                            == Some(merman::model::resources::ResourceProfile::Interactive)
                        && provenance.explicit_overrides.as_ref()
                            == [OperationResourceOverride {
                                id: "max_ascii_grid_cells",
                                value: (EXACT_GRID_CELLS - 1) as u64,
                            }]
                })
    ));
}

#[cfg(feature = "ascii")]
#[test]
fn ascii_request_uses_common_cancellation() {
    let control = OperationControl::new();
    control.cancel();
    let error = Renderer::new()
        .render(RenderRequest::ascii(
            "flowchart TD\nA --> B",
            control,
            AsciiRequest::default(),
        ))
        .expect_err("cancelled ASCII request must stop");
    assert!(matches!(error, RenderError::Cancelled(_)));
}

#[cfg(feature = "ascii")]
#[test]
fn ascii_target_cancellation_precedes_option_validation() {
    let control = OperationControl::new();
    let artifact = Renderer::new()
        .prepare_presentation("flowchart TD\nA --> B", control.clone())
        .expect("semantic preparation should succeed")
        .expect("flowchart should produce a semantic artifact");
    control.cancel();

    let mut request = AsciiRequest::default();
    request.options.flowchart_node_label_wrap_width = 0;
    let error = artifact
        .render(RenderTarget::Ascii(request))
        .expect_err("target admission cancellation must win over invalid options");

    assert!(matches!(
        error,
        RenderError::Cancelled(cancelled)
            if cancelled.phase == OperationPhase::Admission
                && cancelled.reason == merman::CancelReason::Requested
    ));
}

#[cfg(feature = "ascii")]
#[test]
fn ascii_target_deadline_precedes_option_validation() {
    let control = OperationControl::new();
    let artifact = Renderer::new()
        .prepare_presentation("flowchart TD\nA --> B", control.clone())
        .expect("semantic preparation should succeed")
        .expect("flowchart should produce a semantic artifact");
    assert!(control.set_deadline(Duration::ZERO));

    let mut request = AsciiRequest::default();
    request.options.flowchart_node_label_wrap_width = 0;
    let error = artifact
        .render(RenderTarget::Ascii(request))
        .expect_err("target admission deadline must win over invalid options");

    assert!(matches!(
        error,
        RenderError::Cancelled(cancelled)
            if cancelled.phase == OperationPhase::Admission
                && cancelled.reason == merman::CancelReason::DeadlineExceeded
    ));
}

#[cfg(feature = "ascii")]
#[test]
fn ascii_target_resource_terminal_precedes_option_validation_without_rewriting_provenance() {
    let control = OperationControl::new();
    let artifact = Renderer::new()
        .prepare_presentation("flowchart TD\nA --> B", control.clone())
        .expect("semantic preparation should succeed")
        .expect("flowchart should produce a semantic artifact");
    let provenance = OperationResourceProvenance::new(
        OperationResourceDomain::Render,
        Some(merman::model::resources::ResourceProfile::Constrained),
        [OperationResourceOverride {
            id: "max_svg_bytes",
            value: 17,
        }],
    );
    control.terminate_resource_limit(OperationResourceLimitExceeded {
        id: "max_svg_bytes",
        phase: OperationPhase::Emit,
        resource_phase: "svg_output",
        limit: 17,
        consumed: 17,
        requested: 1,
        provenance: provenance.clone(),
    });

    let mut request = AsciiRequest::default();
    request.options.flowchart_node_label_wrap_width = 0;
    let error = artifact
        .render(RenderTarget::Ascii(request))
        .expect_err("the sticky render resource terminal must win before ASCII option validation");

    assert!(matches!(
        error,
        RenderError::ResourceLimitExceeded(details)
            if details.id == "max_svg_bytes"
                && details.phase == "svg_output"
                && details.actual == 18
                && details.maximum == 17
                && details.provenance == Some(provenance)
    ));
}
