#![cfg(feature = "svg")]

use merman::{
    OperationControl, PrepareSourceRequest, PreparedSource, RenderOutput, RenderTarget, Renderer,
    SvgRequest,
};

fn prepare(renderer: &Renderer, source: &str) -> PreparedSource {
    renderer
        .prepare_source(PrepareSourceRequest::new(source, OperationControl::new()))
        .expect("source preparation succeeds")
        .expect("source contains a diagram")
}

fn svg(renderer: &Renderer, prepared: &PreparedSource) -> merman::SvgOutput {
    match renderer
        .render_prepared(
            prepared,
            RenderTarget::Svg(SvgRequest::default()),
            OperationControl::new(),
        )
        .expect("prepared output succeeds")
    {
        RenderOutput::Svg(Some(output)) => output,
        other => panic!("unexpected output: {other:?}"),
    }
}

#[test]
fn prepared_source_survives_independent_cancellation_and_owns_completed_outputs() {
    let renderer = Renderer::new();
    let prepared = prepare(&renderer, "flowchart LR\nA --> B");
    let cancelled = OperationControl::new();
    cancelled.cancel();
    assert!(matches!(
        renderer.render_prepared(
            &prepared,
            RenderTarget::Svg(SvgRequest::default()),
            cancelled,
        ),
        Err(merman::RenderError::Cancelled(_))
    ));
    let output = svg(&renderer, &prepared);
    drop(prepared);
    assert!(output.svg().contains("<svg"));
    assert_eq!(
        output.evidence().family_id(),
        merman::DiagramFamilyId::FLOWCHART
    );
}

#[test]
fn prepared_source_freezes_parse_context_but_refreshes_output_context() {
    let parse_renderer = Renderer::new().with_runtime_policy(
        merman_core::runtime::RuntimePolicy::deterministic().with_fixed_unix_millis(0),
    );
    let prepared = prepare(
        &parse_renderer,
        "gantt\ndateFormat x\nsection Delivery\nTask: id,-1,2ms\n",
    );
    let output_renderer = Renderer::new().with_runtime_policy(
        merman_core::runtime::RuntimePolicy::deterministic().with_fixed_unix_millis(1),
    );
    let first = svg(&parse_renderer, &prepared);
    let second = svg(&output_renderer, &prepared);
    assert_eq!(prepared.parse_context().unix_millis(), 0);
    assert_eq!(first.evidence().unix_millis(), 0);
    assert_eq!(second.evidence().unix_millis(), 1);
    assert_ne!(first.svg(), second.svg());
}

fn semantic(renderer: &Renderer, prepared: &PreparedSource) -> merman_core::ManagedSemanticJson {
    let RenderOutput::Semantic(Some(artifact)) = renderer
        .render_prepared(prepared, RenderTarget::Semantic, OperationControl::new())
        .unwrap()
    else {
        panic!("semantic output");
    };
    artifact.compatibility_json().unwrap()
}

#[test]
fn implicit_gantt_dates_and_generated_git_ids_change_only_on_new_preparation() {
    use merman_core::runtime::RuntimePolicy;
    use merman_core::time::CivilDate;
    let first = Renderer::new().with_runtime_policy(
        RuntimePolicy::deterministic()
            .with_fixed_today(Some(CivilDate::new(2026, 2, 15).unwrap()))
            .with_fixed_seed(10),
    );
    let later = Renderer::new().with_runtime_policy(
        RuntimePolicy::deterministic()
            .with_fixed_today(Some(CivilDate::new(2027, 2, 15).unwrap()))
            .with_fixed_seed(20),
    );
    let source = "gantt\ndateFormat MM-DD\nsection Demo\nMissing year: id1,03-01,1d\nMissing ref: id2,after missing,1d\n";
    let prepared = prepare(&first, source);
    let before = semantic(&first, &prepared);
    let after = semantic(&later, &prepared);
    assert_eq!(before, after);
    assert_eq!(before["tasks"][0]["startTime"], 1_772_323_200_000_i64);
    let fresh = semantic(&later, &prepare(&later, source));
    assert_ne!(
        before["tasks"][0]["startTime"],
        fresh["tasks"][0]["startTime"]
    );
    let source = "gitGraph\ncommit\ncommit\n";
    let prepared = prepare(&first, source);
    assert_eq!(semantic(&first, &prepared), semantic(&later, &prepared));
    assert_ne!(
        semantic(&first, &prepared),
        semantic(&later, &prepare(&later, source))
    );
}

#[test]
fn snapshot_input_readmission_uses_current_renderer_and_explicit_override() {
    use merman::resources::{InputResourceLimitId, InputResourcePolicy};
    let renderer = Renderer::new();
    let prepared = prepare(&renderer, "flowchart LR\nA --> B");
    for (limit, maximum) in [
        (InputResourceLimitId::MaxSourceBytes, 4),
        (InputResourceLimitId::MaxModelItems, 1),
    ] {
        let strict = InputResourcePolicy::default()
            .with_limit(limit, maximum)
            .unwrap();
        let strict_renderer = Renderer::new().with_resource_policy(strict);
        assert!(matches!(
            strict_renderer.render_prepared(
                &prepared,
                RenderTarget::Svg(SvgRequest::default()),
                OperationControl::new()
            ),
            Err(merman::RenderError::ResourceLimitExceeded(_))
        ));
        assert!(
            strict_renderer
                .render_prepared_with_resource_policy(
                    &prepared,
                    RenderTarget::Svg(SvgRequest::default()),
                    OperationControl::new(),
                    InputResourcePolicy::default()
                )
                .is_ok()
        );
    }
    let strict = merman::svg::RenderResourcePolicy::default()
        .with_limit(merman::svg::ResourceLimitId::MaxSourceBytes, 4)
        .unwrap();
    let target = || {
        RenderTarget::Svg(SvgRequest {
            environment: merman::SvgEnvironment::deterministic().with_resource_policy(strict),
            ..Default::default()
        })
    };
    assert!(
        renderer
            .render_prepared(&prepared, target(), OperationControl::new())
            .is_err()
    );
    let explicit = Renderer::new().with_resource_policy(InputResourcePolicy::default());
    assert!(
        explicit
            .render_prepared(&prepared, target(), OperationControl::new())
            .is_ok()
    );
    assert!(svg(&renderer, &prepared).svg().contains("<svg"));
}

#[test]
fn deadline_and_output_budget_failure_do_not_poison_reuse() {
    let renderer = Renderer::new();
    let prepared = prepare(&renderer, "flowchart LR\nA --> B");
    let expired = OperationControl::new().with_deadline(std::time::Duration::ZERO);
    assert!(matches!(
        renderer.render_prepared(&prepared, RenderTarget::Svg(SvgRequest::default()), expired),
        Err(merman::RenderError::Cancelled(_))
    ));
    for limit in [
        merman::svg::ResourceLimitId::MaxLayoutWorkUnits,
        merman::svg::ResourceLimitId::MaxSvgBytes,
    ] {
        let policy = merman::svg::RenderResourcePolicy::default()
            .with_limit(limit, 1)
            .unwrap();
        let request = SvgRequest {
            environment: merman::SvgEnvironment::deterministic().with_resource_policy(policy),
            ..Default::default()
        };
        assert!(matches!(
            renderer.render_prepared(
                &prepared,
                RenderTarget::Svg(request),
                OperationControl::new()
            ),
            Err(merman::RenderError::ResourceLimitExceeded(_))
        ));
        assert!(svg(&renderer, &prepared).svg().contains("<svg"));
    }
}

#[test]
fn preparation_keeps_parse_options_and_existing_theme_binding() {
    let renderer = Renderer::new();
    let source = "flowchart TD\nA -->";
    assert!(
        renderer
            .prepare_source(
                PrepareSourceRequest::new(source, OperationControl::new())
                    .with_parse_options(merman::ParseOptions::strict())
            )
            .is_err()
    );
    let prepared = renderer
        .prepare_source(
            PrepareSourceRequest::new(source, OperationControl::new())
                .with_parse_options(merman::ParseOptions::lenient()),
        )
        .unwrap()
        .unwrap();
    assert_eq!(prepared.parse_options(), merman::ParseOptions::lenient());
    assert_eq!(prepared.semantic_kind(), "error");
    assert!(
        svg(
            &Renderer::new().with_parse_options(merman::ParseOptions::strict()),
            &prepared
        )
        .svg()
        .contains("<svg")
    );
    let theme = merman::svg::DiagramThemeCompiler::new()
        .compile(
            merman::svg::DiagramThemeSpec::new().with_styles(
                merman::svg::ThemeRuleSet::default().with_rule(
                    merman::svg::ThemeRule::new(
                        merman::svg::ThemeTarget::Node,
                        merman::svg::ThemeStylePatch::default()
                            .with_fill(merman::svg::CanvasPaint::solid("#1d4ed8").unwrap()),
                    )
                    .for_family(merman::DiagramFamilyId::FLOWCHART),
                ),
            ),
        )
        .unwrap();
    let source = "flowchart LR\nA --> B";
    let prepared = renderer
        .prepare_source(
            PrepareSourceRequest::new(source, OperationControl::new()).with_theme(theme.clone()),
        )
        .unwrap()
        .unwrap();
    let direct = renderer
        .render(
            merman::RenderRequest::svg(source, OperationControl::new(), SvgRequest::default())
                .with_theme(theme),
        )
        .unwrap();
    let RenderOutput::Svg(Some(direct)) = direct else {
        panic!("SVG");
    };
    assert_eq!(svg(&renderer, &prepared).svg(), direct.svg());
}

#[test]
fn all_completed_graphical_outputs_outlive_the_snapshot() {
    let renderer = Renderer::new();
    let prepared = prepare(&renderer, "flowchart LR\nA -->|edge| B");
    let RenderOutput::Document(Some(document)) = renderer
        .render_prepared(
            &prepared,
            RenderTarget::Document(SvgRequest::default()),
            OperationControl::new(),
        )
        .unwrap()
    else {
        panic!("document");
    };
    let RenderOutput::LayoutJson(Some(layout)) = renderer
        .render_prepared(
            &prepared,
            RenderTarget::LayoutJson(SvgRequest::default()),
            OperationControl::new(),
        )
        .unwrap()
    else {
        panic!("layout");
    };
    let RenderOutput::EdgeGeometryJson(Some(geometry)) = renderer
        .render_prepared(
            &prepared,
            RenderTarget::EdgeGeometryJson(SvgRequest::default()),
            OperationControl::new(),
        )
        .unwrap()
    else {
        panic!("geometry");
    };
    drop(prepared);
    assert!(document.svg().contains("<svg"));
    assert_eq!(layout.layout()["meta"]["diagram_type"], "flowchart-v2");
    assert_eq!(geometry.edges().len(), 1);
    #[cfg(feature = "png")]
    assert!(
        !document
            .export_png(&Default::default(), OperationControl::new())
            .unwrap()
            .bytes()
            .is_empty()
    );
}

#[test]
fn custom_overlay_is_parsed_once_and_keeps_its_closed_capability_boundary() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static PARSES: AtomicUsize = AtomicUsize::new(0);
    fn parse(
        _source: &str,
        _meta: &merman::ParseMetadata,
        control: &OperationControl,
    ) -> merman_core::OperationControlResult<merman_core::Result<merman_core::CustomJsonRenderModel>>
    {
        control.checkpoint()?;
        PARSES.fetch_add(1, Ordering::SeqCst);
        Ok(Ok(merman_core::CustomJsonRenderModel::new(
            "overlay",
            serde_json::json!({"meaning":42}),
        )))
    }
    let mut engine = merman::Engine::new();
    engine
        .render_diagram_registry_mut()
        .insert("flowchart-v2", parse);
    let renderer = Renderer::new().with_engine(engine);
    let prepared = prepare(&renderer, "flowchart LR\nA --> B");
    assert_eq!(PARSES.load(Ordering::SeqCst), 1);
    for _ in 0..2 {
        assert_eq!(semantic(&renderer, &prepared)["meaning"], 42);
        for target in [
            RenderTarget::Svg(SvgRequest::default()),
            RenderTarget::EdgeGeometryJson(SvgRequest::default()),
        ] {
            assert!(matches!(
                renderer.render_prepared(&prepared, target, OperationControl::new()),
                Err(merman::RenderError::Svg(
                    merman::svg::RenderError::NonRenderableCustomModel { .. }
                ))
            ));
        }
    }
    assert_eq!(PARSES.load(Ordering::SeqCst), 1);
}

#[test]
fn no_diagram_and_parse_failure_do_not_create_snapshots() {
    let renderer = Renderer::new();
    let prepared_error = renderer
        .prepare_source(PrepareSourceRequest::new("", OperationControl::new()))
        .unwrap_err();
    let source_error = renderer
        .prepare_semantic("", OperationControl::new())
        .unwrap_err();
    assert_eq!(prepared_error.to_string(), source_error.to_string());
    assert!(
        renderer
            .prepare_source(PrepareSourceRequest::new(
                "flowchart LR\nA -->",
                OperationControl::new()
            ))
            .is_err()
    );
    fn send_sync<T: Send + Sync>() {}
    send_sync::<PreparedSource>();
}

#[test]
fn prepared_outputs_refresh_width_and_measurement_without_changing_parse_facts() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    #[derive(Debug)]
    struct Host(Arc<AtomicUsize>);
    impl merman::svg::HostTextMeasurer for Host {
        fn measure(
            &self,
            _request: merman::svg::HostTextMeasurementRequest<'_>,
        ) -> merman::svg::HostMeasurementResult {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(None)
        }
    }
    let renderer = Renderer::new();
    let prepared = prepare(
        &renderer,
        "gantt\ndateFormat YYYY-MM-DD\nsection Demo\nTask: task,2026-01-01,2d",
    );
    let before = semantic(&renderer, &prepared);
    let calls = Arc::new(AtomicUsize::new(0));
    let identity = merman::svg::TextMeasurementProfileIdentity::new(
        merman::svg::MeasurementProfileId::new("test.prepared-output-fonts").unwrap(),
        "1",
    )
    .unwrap();
    for width in [480.0, 960.0] {
        let mut request = SvgRequest::default();
        request.layout.container_width = width;
        request.environment = merman::SvgEnvironment::deterministic().with_text_measurement_policy(
            merman::svg::TextMeasurementPolicy::host_display(
                identity.clone(),
                Arc::new(Host(calls.clone())),
                merman::svg::TextMeasurementPhase::ALL,
            ),
        );
        assert!(
            renderer
                .render_prepared(
                    &prepared,
                    RenderTarget::Svg(request),
                    OperationControl::new()
                )
                .is_ok()
        );
    }
    assert!(calls.load(Ordering::SeqCst) > 0);
    assert_eq!(before, semantic(&renderer, &prepared));
}

#[cfg(feature = "ascii")]
#[test]
fn completed_ascii_outlives_prepared_source() {
    let renderer = Renderer::new();
    let prepared = prepare(&renderer, "flowchart TD\nA --> B");
    let RenderOutput::Ascii(Some(output)) = renderer
        .render_prepared(
            &prepared,
            RenderTarget::Ascii(merman::AsciiRequest::default()),
            OperationControl::new(),
        )
        .unwrap()
    else {
        panic!("ASCII output");
    };
    drop(prepared);
    assert!(output.as_text().contains('A'));
    assert!(output.as_text().contains('B'));
}

#[test]
fn semantic_handoff_releases_the_original_operation_control() {
    let renderer = Renderer::new();
    let control = OperationControl::new();
    let artifact = renderer
        .prepare_semantic("flowchart LR\nA --> B", control.clone())
        .unwrap()
        .unwrap();
    control.cancel();
    let prepared = artifact.into_prepared_source();
    assert!(svg(&renderer, &prepared).svg().contains("<svg"));
}
