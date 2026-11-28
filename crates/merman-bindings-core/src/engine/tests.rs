use super::*;
use serde_json::Value;
use std::sync::Arc;

#[cfg(all(feature = "fixes", feature = "diagram-flowchart"))]
#[test]
fn legacy_full_dtos_generate_fixes_only_for_the_declared_host() {
    use crate::{
        ArtifactContractSpec, BindingTransportKey, CapabilityKey, OperationKey, TargetKey,
    };
    const SOURCE: &[u8] = b"flowchart\nA-->B\n";
    let contract = |with_fixes| {
        let spec = ArtifactContractSpec::new(TargetKey::Native, BindingTransportKey::Rust)
            .with_operations(&[OperationKey::AnalysisJson, OperationKey::ValidationJson]);
        Arc::new(if with_fixes {
            spec.with_supplemental_capabilities(&[CapabilityKey::Fixes])
                .materialize()
        } else {
            spec.materialize()
        })
    };
    for declared in [false, true] {
        let engine = BindingEngine::from_options_and_services_for_contract(
            contract(declared),
            br#"{"analysis":{"lint":{"profile":"recommended"}}}"#,
            BindingEngineServices::new(),
        )
        .unwrap();
        let payload: Value = serde_json::from_slice(&engine.analyze_json(SOURCE).unwrap()).unwrap();
        let has_fixes = payload["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| {
                diagnostic["fixes"]
                    .as_array()
                    .is_some_and(|fixes| !fixes.is_empty())
            });
        assert_eq!(has_fixes, declared);
        let options = analysis_projection_options(
            engine.analyzer.options().clone(),
            false,
            &engine.artifact_contract,
        );
        assert_eq!(
            options.diagnostic_policy().fix_generation,
            merman_analysis::FixGeneration::Omit
        );
    }
}
#[cfg(feature = "svg")]
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(all(feature = "svg", feature = "diagram-flowchart"))]
fn full_engine_materializations() -> (usize, usize) {
    (
        SEMANTIC_ENGINE_MATERIALIZATIONS.with(std::cell::Cell::get),
        crate::render::RENDER_ENGINE_MATERIALIZATIONS.with(std::cell::Cell::get),
    )
}

#[cfg(all(
    feature = "svg",
    feature = "diagram-flowchart",
    feature = "layout-dagre"
))]
#[test]
fn diagram_id_overrides_reuse_the_render_engine() {
    const BASE: &[u8] = br#"{
            "site_config": {"layout":"dagre"},
            "theme": {"preset":"studio"},
            "svg": {"diagram_id":"base","pipeline":"readable","viewbox_padding":6}
        }"#;
    const SOURCE: &[u8] = b"flowchart LR\nA[Start] --> B[End]";
    const OVERRIDES: [&[u8]; 2] = [
        br#"{"svg":{"diagram_id":"changed id"}}"#,
        br#"{"version":4,"svg":{"diagram_id":null}}"#,
    ];
    let expected = [Some("changed id"), None].map(|id| {
        let config = serde_json::json!({
            "site_config": {"layout":"dagre"},
            "theme": {"preset":"studio"},
            "svg": {"diagram_id":id,"pipeline":"readable","viewbox_padding":6}
        });
        crate::execute_once(
            crate::BindingOperationRequest::new("svg", SOURCE)
                .with_options_json(config.to_string().as_bytes()),
        )
        .unwrap()
    });
    let before = full_engine_materializations();
    let engine = BindingEngine::new(BASE).unwrap();
    for _ in 0..32 {
        for (options, expected) in OVERRIDES.into_iter().zip(&expected) {
            let request =
                crate::BindingOperationRequest::new("svg", SOURCE).with_options_json(options);
            assert_eq!(&engine.execute(request.clone()).unwrap(), expected);
            assert_eq!(engine.execute_data(request).unwrap(), expected.data());
        }
    }
    let after = full_engine_materializations();
    eprintln!(
        "128 diagram ID requests, {} render engine materializations",
        after.1 - before.1
    );
    assert_eq!(after, (before.0, before.1 + 1));

    let full_engine = BindingEngine::new(BASE).unwrap();
    let before_full = full_engine_materializations();
    for _ in 0..128 {
        let request = crate::BindingOperationRequest::new("svg", SOURCE)
            .with_options_json(br#"{"svg":{"diagram_id":"changed id","pipeline":"readable"}}"#);
        full_engine.execute_data(request).unwrap();
    }
    let after_full = full_engine_materializations();
    eprintln!(
        "128 full SVG options requests, {} render engine materializations",
        after_full.1 - before_full.1
    );
    assert_eq!(after_full, (before_full.0, before_full.1 + 128));
    assert!(
        std::str::from_utf8(&engine.render_svg(SOURCE).unwrap())
            .unwrap()
            .contains("id=\"base\"")
    );
}

#[cfg(all(
    feature = "analysis",
    feature = "svg",
    feature = "diagram-flowchart",
    feature = "layout-dagre"
))]
#[test]
fn diagram_id_overrides_keep_the_full_projection_for_wrapped_constructors() {
    use crate::{
        ArtifactContractSpec, BindingOperationRequest, BindingTransportKey, CapabilityKey,
        OperationKey, TargetKey,
    };

    let spec = ArtifactContractSpec::new(TargetKey::Native, BindingTransportKey::Rust)
        .with_operations(&[OperationKey::Svg]);
    let contract = if cfg!(feature = "layout-elk") {
        spec.with_supplemental_capabilities(&[CapabilityKey::LayoutDagre, CapabilityKey::LayoutElk])
    } else {
        spec.with_supplemental_capabilities(&[CapabilityKey::LayoutDagre])
    }
    .materialize();

    const SOURCE: &[u8] = b"flowchart TD\nA --> B";
    for base in [
            br#"{"analysis":{"merman":{"site_config":{"layout":"shoal"}}},"svg":{"pipeline":"readable"}}"#.as_slice(),
            br#"{"merman":{"analysis":{"site_config":{"layout":"shoal"}}},"svg":{"pipeline":"readable"}}"#.as_slice(),
        ] {
            let engine = contract.create_engine(base).unwrap();
            let constructor_error = engine.render_svg(SOURCE).unwrap_err();
            assert_eq!(constructor_error.capability_id(), Some("layout-shoal"));

            let request = BindingOperationRequest::new("svg", SOURCE)
                .with_options_json(br#"{"svg":{"diagram_id":"probe"}}"#);
            let full_request = request.clone().with_options_json(
                br#"{"svg":{"diagram_id":"probe","pipeline":"readable"}}"#,
            );
            let expected = engine.execute(full_request.clone()).unwrap();
            assert_eq!(engine.execute_data(full_request).unwrap(), expected.data());
            assert_eq!(engine.execute(request.clone()).unwrap(), expected);
            assert_eq!(engine.execute_data(request).unwrap(), expected.data());
        }
}

#[cfg(feature = "svg")]
#[test]
fn diagram_id_overrides_validate_options_before_rendering() {
    use crate::{BindingOperationRequest, BindingStatus};

    let engine = BindingEngine::new(b"").unwrap();
    for options in [
        br#"{"svg":{"diagram_id":42}}"#.as_slice(),
        br#"{"svg":{"diagramId":"probe"}}"#.as_slice(),
        br#"{"svg":{"diagram_id":"probe"},"version":3}"#.as_slice(),
        br#"{"svg":{"diagram_id":"probe"}"#.as_slice(),
    ] {
        let request = BindingOperationRequest::new("svg", b"").with_options_json(options);
        let expected = crate::execute_once(request.clone()).unwrap_err();
        assert_eq!(expected.status(), BindingStatus::OptionsJsonError);
        assert_eq!(engine.execute(request.clone()).unwrap_err(), expected);
        assert_eq!(engine.execute_data(request).unwrap_err(), expected);
    }

    let mut oversized = br#"{"svg":{"diagram_id":"probe"}}"#.to_vec();
    oversized.resize(crate::BINDING_OPTIONS_JSON_MAX_BYTES + 1, b' ');
    let request = BindingOperationRequest::new("svg", b"").with_options_json(&oversized);
    let expected = crate::execute_once(request.clone()).unwrap_err();
    assert_eq!(expected.status(), BindingStatus::ResourceLimitExceeded);
    assert_eq!(
        expected.resource_details().unwrap().limit_id,
        crate::resource_contract::BINDING_OPTIONS_JSON_LIMIT_ID
    );
    assert_eq!(engine.execute(request.clone()).unwrap_err(), expected);
    assert_eq!(engine.execute_data(request).unwrap_err(), expected);
    assert!(engine.render.get().is_none());
}

#[cfg(all(
    feature = "svg",
    feature = "diagram-flowchart",
    feature = "layout-dagre"
))]
#[test]
fn diagram_id_overrides_preserve_resource_ceilings() {
    use crate::{BindingOperationRequest, BindingStatus};

    const SOURCE: &[u8] = b"---\nconfig:\n  layout: dagre\n---\nflowchart TD\nA --> B";
    const ID_OVERRIDE: &[u8] = br#"{"svg":{"diagram_id":"probe"}}"#;
    const TIGHTENED: &[u8] =
        br#"{"svg":{"diagram_id":"probe"},"resources":{"limits":{"max_svg_bytes":32}}}"#;
    const LOOSENED: &[u8] =
        br#"{"svg":{"diagram_id":"probe"},"resources":{"limits":{"max_svg_bytes":65}}}"#;
    let engine = BindingEngine::new(
        br#"{"resources":{"profile":"constrained","limits":{"max_svg_bytes":64}}}"#,
    )
    .unwrap();

    for (options, maximum) in [(ID_OVERRIDE, 64), (TIGHTENED, 32), (ID_OVERRIDE, 64)] {
        let request = BindingOperationRequest::new("svg", SOURCE).with_options_json(options);
        let error = engine.execute(request.clone()).unwrap_err();
        assert_eq!(error.status(), BindingStatus::ResourceLimitExceeded);
        let details = error.resource_details().unwrap();
        assert_eq!(details.limit_id, "max_svg_bytes");
        assert_eq!(details.max, maximum);
        assert_eq!(details.profile, "constrained");
        assert_eq!(engine.execute_data(request).unwrap_err(), error);
    }

    let request = BindingOperationRequest::new("svg", SOURCE).with_options_json(LOOSENED);
    let error = engine.execute(request.clone()).unwrap_err();
    assert_eq!(error.status(), BindingStatus::OptionsJsonError);
    assert!(
        error
            .message()
            .contains("would loosen the transport ceiling")
    );
    assert_eq!(engine.execute_data(request).unwrap_err(), error);
}

#[cfg(all(
    feature = "svg",
    feature = "analysis",
    feature = "ascii",
    feature = "diagram-flowchart"
))]
#[test]
fn lean_binding_consumers_do_not_materialize_full_engines() {
    const OPTIONS: &[u8] = br#"{"site_config":{"themeVariables":{"primaryColor":"not-a-color"}}}"#;
    const SOURCE: &[u8] = concat!(
        "%%{init: {\"themeVariables\": {\"primaryColor\": \"not-a-color\"}}}%%\n",
        "flowchart TD\nA --> B",
    )
    .as_bytes();
    const DOCUMENT: &[u8] = concat!(
        "```mermaid\n",
        "%%{init: {\"themeVariables\": {\"primaryColor\": \"not-a-color\"}}}%%\n",
        "flowchart TD\nA --> B\n```",
    )
    .as_bytes();

    let before = full_engine_materializations();
    let based = BindingEngine::new(OPTIONS).expect("visual errors remain operation-local");
    let overlaid = BindingEngine::new(b"").unwrap();
    for operation in [
        "analysis-json",
        "analysis-facts-json",
        "validation-json",
        "document-analysis-json",
        "document-analysis-facts-json",
        "ascii",
    ] {
        let document = operation.starts_with("document-");
        let source = if document { DOCUMENT } else { SOURCE };
        let request = crate::BindingOperationRequest::new(operation, source)
            .with_optional_uri(document.then_some(b"file:///diagram.md".as_slice()));
        let once = crate::execute_once(request.clone().with_options_json(OPTIONS)).unwrap();
        assert_eq!(
            crate::operation::execute_once_data(request.clone().with_options_json(OPTIONS))
                .unwrap(),
            once.data(),
            "{operation}",
        );
        assert_eq!(based.execute(request.clone()).unwrap(), once, "{operation}");
        assert_eq!(
            based.execute_data(request.clone()).unwrap(),
            once.data(),
            "{operation}",
        );
        assert_eq!(
            overlaid
                .execute(request.clone().with_options_json(OPTIONS))
                .unwrap(),
            once,
            "{operation}",
        );
        assert_eq!(
            overlaid
                .execute_data(request.with_options_json(OPTIONS))
                .unwrap(),
            once.data(),
            "{operation}",
        );
        if operation != "ascii" {
            let payload: Value = serde_json::from_slice(once.data()).unwrap();
            assert_eq!(payload["valid"], true, "{operation}: {payload}");
        }
        assert_eq!(full_engine_materializations(), before, "{operation}");
    }
    assert!(based.semantic.get().is_none());
    assert!(based.render.get().is_none());
    assert!(overlaid.semantic.get().is_none());
    assert!(overlaid.render.get().is_none());
}

#[cfg(all(feature = "svg", feature = "diagram-flowchart"))]
#[test]
fn full_binding_operations_materialize_only_the_requested_engine_once() {
    const OPTIONS: &[u8] = br#"{"site_config":{"themeVariables":{"primaryColor":"not-a-color"}}}"#;
    const SOURCE: &[u8] = b"flowchart TD\nA --> B";
    let before = full_engine_materializations();
    let engine = BindingEngine::new(OPTIONS).expect("visual errors remain operation-local");
    assert_eq!(full_engine_materializations(), before);

    let cancelled = OperationControl::new();
    cancelled.cancel();
    for operation in ["semantic-json", "svg"] {
        let error = engine
            .execute(
                crate::BindingOperationRequest::new(operation, SOURCE)
                    .with_control(cancelled.clone()),
            )
            .unwrap_err();
        assert_eq!(error.status(), crate::BindingStatus::Cancelled);
    }
    assert_eq!(full_engine_materializations(), before);

    let semantic_error = engine.parse_json(SOURCE).unwrap_err();
    assert!(semantic_error.message().contains("not-a-color"));
    assert_eq!(engine.parse_json(SOURCE).unwrap_err(), semantic_error);
    assert_eq!(full_engine_materializations(), (before.0 + 1, before.1));

    let render_error = engine.render_svg(SOURCE).unwrap_err();
    assert!(render_error.message().contains("not-a-color"));
    assert_eq!(engine.render_svg(SOURCE).unwrap_err(), render_error);
    assert_eq!(full_engine_materializations(), (before.0 + 1, before.1 + 1));

    for operation in ["semantic-json", "svg"] {
        let expected = if operation == "semantic-json" {
            &semantic_error
        } else {
            &render_error
        };
        let request =
            crate::BindingOperationRequest::new(operation, SOURCE).with_options_json(OPTIONS);
        assert_eq!(&crate::execute_once(request.clone()).unwrap_err(), expected);
        assert_eq!(
            &BindingEngine::new(b"")
                .unwrap()
                .execute(request)
                .unwrap_err(),
            expected,
        );
    }
    assert_eq!(full_engine_materializations(), (before.0 + 3, before.1 + 3));
    #[cfg(feature = "analysis")]
    engine
        .analyze_json(SOURCE)
        .expect("full errors do not poison lean analysis");
    #[cfg(feature = "ascii")]
    engine
        .render_ascii(SOURCE)
        .expect("full errors do not poison ASCII");
    assert_eq!(full_engine_materializations(), (before.0 + 3, before.1 + 3));
}

#[cfg(all(feature = "svg", feature = "diagram-flowchart"))]
#[test]
fn shared_cold_render_initialization_rechecks_request_deadlines_and_cancellation() {
    use std::sync::{atomic::AtomicBool, mpsc};
    use std::time::{Duration, Instant};

    const SOURCE: &[u8] = b"flowchart TD\nA[Shared] --> B[Recovered]";
    const ID_OVERRIDE: &[u8] = br#"{"svg":{"diagram_id":"probe"}}"#;
    const WAIT: Duration = Duration::from_secs(5);

    for (options, deadline_exceeded) in [
        (b"".as_slice(), true),
        (b"".as_slice(), false),
        (ID_OVERRIDE, true),
        (ID_OVERRIDE, false),
    ] {
        // The deadline setup reads the clock first. The final pre-cache checkpoint is
        // Parse for an unchanged request and Layout after a second Admission for an override.
        let last_pre_cache_clock_read = 2 + usize::from(!options.is_empty());
        let engine = Arc::new(BindingEngine::new(b"").unwrap());
        assert!(engine.render.get().is_none());
        let (initializing_sender, initializing_receiver) = mpsc::channel();
        let (release_sender, release_receiver) = mpsc::channel();
        let (before_cache_sender, before_cache_receiver) = mpsc::channel();
        let clock_reads = Arc::new(AtomicUsize::new(0));
        let expired = Arc::new(AtomicBool::new(false));
        let observed_reads = Arc::clone(&clock_reads);
        let observed_expired = Arc::clone(&expired);
        let base = Instant::now();
        let control = OperationControl::with_test_clock(Arc::new(move || {
            let now = if observed_expired.load(Ordering::SeqCst) {
                base + Duration::from_millis(2)
            } else {
                base
            };
            if observed_reads.fetch_add(1, Ordering::SeqCst) == last_pre_cache_clock_read {
                before_cache_sender.send(()).unwrap();
            }
            now
        }))
        .with_deadline(Duration::from_millis(1));

        std::thread::scope(|scope| {
            let initializing_engine = Arc::clone(&engine);
            let initializing = scope.spawn(move || {
                let before = full_engine_materializations();
                initializing_engine.render.get_or_init(|| {
                    initializing_sender.send(()).unwrap();
                    release_receiver
                        .recv_timeout(WAIT)
                        .expect("release the occupied cold render initializer");
                    initializing_engine
                        .render_config
                        .clone()
                        .materialize(&initializing_engine.services)
                });
                assert_eq!(full_engine_materializations(), (before.0, before.1 + 1));
            });
            initializing_receiver
                .recv_timeout(WAIT)
                .expect("the shared cold render initializer must be occupied");

            let waiting_engine = Arc::clone(&engine);
            let waiting_control = control.clone();
            let waiting = scope.spawn(move || {
                let before = full_engine_materializations();
                let result = waiting_engine.execute_data(
                    crate::BindingOperationRequest::new("svg", SOURCE)
                        .with_options_json(options)
                        .with_control(waiting_control),
                );
                assert_eq!(full_engine_materializations(), before);
                result
            });
            let reached_cache = before_cache_receiver.recv_timeout(WAIT);
            let cache_is_cold = engine.render.get().is_none();
            if deadline_exceeded {
                expired.store(true, Ordering::SeqCst);
            } else {
                control.cancel();
            }
            let request_is_pending = !waiting.is_finished();
            release_sender.send(()).unwrap();
            initializing.join().expect("complete shared initialization");
            let result = waiting.join().expect("settle the waiting request");

            reached_cache.expect("request must reach the final checkpoint before the shared cache");
            assert!(cache_is_cold);
            assert!(request_is_pending);
            let error = result.expect_err("a terminal raised during initialization must survive");
            assert_eq!(error.status(), crate::BindingStatus::Cancelled);
            assert_eq!(
                error.cancellation_details(),
                Some(crate::BindingCancellationErrorDetails {
                    reason: if deadline_exceeded {
                        "deadline_exceeded"
                    } else {
                        "requested"
                    },
                    phase: "admission",
                }),
            );
        });

        let cached = engine
            .render
            .get()
            .expect("shared initialization completed");
        let before = full_engine_materializations();
        let svg = String::from_utf8(engine.render_svg(SOURCE).unwrap()).unwrap();
        assert!(svg.contains("<svg"));
        assert!(svg.contains("Recovered"));
        assert!(std::ptr::eq(cached, engine.render_engine()));
        assert_eq!(full_engine_materializations(), before);
        assert!(engine.semantic.get().is_none());
    }
}

#[cfg(all(feature = "svg", feature = "diagram-flowchart"))]
#[test]
fn delayed_render_engine_preserves_constructor_services_and_prepared_ownership() {
    let before = full_engine_materializations();
    let counter = CountingHostTextMeasurer::new();
    let services = BindingEngineServices::new().with_host_text_measurer(counter.clone());
    let engine = BindingEngine::from_options_and_services(
        br#"{"site_config":{"flowchart":{"htmlLabels":false}}}"#,
        services.clone(),
    )
    .unwrap();
    let expected = engine
        .render_svg(b"flowchart TD\nA[Retained] --> B[Output]")
        .unwrap();
    assert!(
        counter.calls() > 0,
        "lazy materialization retains the host service",
    );
    assert_eq!(full_engine_materializations(), (before.0, before.1 + 1));

    let prepared = engine
        .prepare_source(
            b"flowchart TD\nA[Retained] --> B[Output]",
            OperationControl::new(),
        )
        .unwrap();
    assert_eq!(full_engine_materializations(), (before.0, before.1 + 1));
    assert!(engine.semantic.get().is_none());
    drop(engine);
    assert_eq!(
        prepared
            .render_svg(b"", OperationControl::new(), &services)
            .unwrap(),
        expected,
    );
    assert_eq!(full_engine_materializations(), (before.0, before.1 + 2));
}

#[test]
fn theme_evaluation_limits_keep_resource_status_at_the_binding_boundary() {
    let error = common::core_error(merman::Error::ThemeEvaluationLimit(
        merman::ThemeEvaluationLimitExceeded {
            limit: "THEME_COLOR_LIMIT",
            requested: "65".to_string(),
            max: 64,
        },
    ));

    assert_eq!(error.status(), crate::BindingStatus::ResourceLimitExceeded);
    assert!(error.message().contains("THEME_COLOR_LIMIT"));
    assert_eq!(error.resource_details(), None);
}

#[cfg(feature = "svg")]
struct CountingHostTextMeasurer {
    calls: AtomicUsize,
}

#[cfg(feature = "svg")]
impl CountingHostTextMeasurer {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
        })
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

#[test]
fn semantic_parse_errors_are_terminal_safe_and_structured() {
    let span = merman::model::SourceSpan::new(5, 5);
    let diagnostic = merman::ParseDiagnostic::new("missing\u{7}value")
        .with_span(span, merman::ParseDiagnosticSpanKind::InsertionPoint)
        .with_code("merman.semantic\u{1b}");
    let error = common::core_error(merman::Error::diagram_parse_diagnostic(
        "state\u{1b}",
        diagnostic,
    ));

    assert_eq!(error.status(), crate::BindingStatus::ParseError);
    assert!(!error.message().contains('\u{1b}'));
    assert!(!error.message().contains('\u{7}'));
    let details = error
        .diagnostic_details()
        .expect("semantic parse errors preserve structured details");
    assert_eq!(details.code, "merman.semantic\\u{1B}");
    assert_eq!(details.diagram_type.as_deref(), Some("state\\u{1B}"));
    assert_eq!(
        details.span,
        Some(crate::BindingDiagnosticSpan::new(5, 5, "insertion-point"))
    );
}

#[cfg(feature = "svg")]
impl crate::HostTextMeasurer for CountingHostTextMeasurer {
    fn measure(
        &self,
        _request: crate::HostTextMeasurementRequest<'_>,
    ) -> crate::HostMeasurementResult {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(None)
    }
}

#[cfg(feature = "svg")]
struct PanicHostTextMeasurer;

#[cfg(feature = "svg")]
impl crate::HostTextMeasurer for PanicHostTextMeasurer {
    fn measure(
        &self,
        _request: crate::HostTextMeasurementRequest<'_>,
    ) -> crate::HostMeasurementResult {
        panic!("engine construction must not invoke host text measurement")
    }
}

#[test]
fn try_native_uses_the_exact_artifact_adapter_selection() {
    let contract = crate::artifact_contract::default_artifact_contract();
    let missing_adapter = contract
        .validate_native_runtime_policy()
        .err()
        .and_then(|error| error.capability_id());

    match (missing_adapter, BindingEngine::try_native(b"")) {
        (Some(expected), Err(error)) => {
            assert_eq!(error.status(), crate::BindingStatus::UnsupportedOperation);
            assert_eq!(error.kind(), crate::BindingErrorKind::MissingCapability);
            assert_eq!(error.capability_id(), Some(expected));
        }
        (None, Ok(_engine)) => {}
        (Some(expected), Ok(_)) => {
            panic!("try_native accepted an artifact missing `{expected}`")
        }
        (None, Err(error)) => panic!("complete native artifact was rejected: {error:?}"),
    }
}

#[cfg(not(feature = "svg"))]
#[test]
fn semantic_parse_is_available_without_renderer() {
    let engine = BindingEngine::new(b"").expect("semantic bindings engine");
    let model: Value = serde_json::from_slice(
        &engine
            .parse_json(b"flowchart TD\nA[Start] --> B[Done]")
            .expect("semantic JSON without SVG"),
    )
    .expect("semantic model JSON");

    assert_eq!(model["type"], "flowchart-v2");
}

#[test]
fn semantic_parse_enforces_model_item_budget() {
    assert_semantic_model_limit("max_model_items", b"flowchart TD\nA --> B");
}

#[cfg(feature = "svg")]
#[test]
fn constructor_owned_text_service_distinguishes_omitted_and_explicit_selectors() {
    for options in [b"".as_slice(), br#"{"environment":{}}"#.as_slice()] {
        BindingEngine::from_options_and_services(
            options,
            BindingEngineServices::new().with_host_text_measurer(Arc::new(PanicHostTextMeasurer)),
        )
        .expect("omitted selectors do not conflict or invoke the callback");
    }

    let options = r#"{"environment":{"text_measurement":"deterministic"}}"#;
    let error = BindingEngine::from_options_and_services(
        options.as_bytes(),
        BindingEngineServices::new().with_host_text_measurer(Arc::new(PanicHostTextMeasurer)),
    )
    .err()
    .expect("an explicit selector must conflict with the constructor service");
    assert_eq!(error.status(), crate::BindingStatus::InvalidArgument);
    assert_eq!(
        error.message(),
        "constructor service `host-text-measurement` conflicts with explicit option `environment.text_measurement`"
    );

    let null_error = BindingEngine::from_options_and_services(
        br#"{"environment":{"text_measurement":null}}"#,
        BindingEngineServices::new().with_host_text_measurer(Arc::new(PanicHostTextMeasurer)),
    )
    .err()
    .expect("an explicitly supplied null selector remains explicit provenance");
    assert_eq!(null_error.status(), crate::BindingStatus::InvalidArgument);
}

#[cfg(feature = "svg")]
#[test]
fn request_overlay_text_selector_conflicts_before_execution_work() {
    let counter = CountingHostTextMeasurer::new();
    let services = BindingEngineServices::new().with_host_text_measurer(counter.clone());
    let engine = BindingEngine::from_options_and_services(b"", services).unwrap();
    assert_eq!(counter.calls(), 0, "construction must not call the host");

    for options in [
        br#"{"environment":{"text_measurement":"deterministic"}}"#.as_slice(),
        br#"{"environment":{"text_measurement":null}}"#.as_slice(),
    ] {
        let error = engine
            .execute(
                crate::BindingOperationRequest::new("semantic-json", b"flowchart TD\nA --> B")
                    .with_options_json(options),
            )
            .expect_err("request-local selector must conflict before operation work");
        assert_eq!(error.status(), crate::BindingStatus::InvalidArgument);
        assert_eq!(counter.calls(), 0);
    }
}

#[cfg(feature = "svg")]
#[test]
fn legacy_text_measurement_selectors_are_rejected() {
    for selector in ["vendored", "parity"] {
        let options = format!(r#"{{"environment":{{"text_measurement":"{selector}"}}}}"#);
        let error = BindingEngine::from_options(options.as_bytes())
            .err()
            .expect("legacy text measurement selector must be rejected");
        assert_eq!(error.status(), crate::BindingStatus::InvalidArgument);
        assert_eq!(
            error.message(),
            format!("unsupported environment.text_measurement: {selector}")
        );
    }
}

#[cfg(feature = "svg")]
#[test]
fn cloned_services_share_one_callback_across_immutable_engines() {
    let counter = CountingHostTextMeasurer::new();
    let services = BindingEngineServices::new().with_host_text_measurer(counter.clone());
    let first = BindingEngine::from_options_and_services(b"", services.clone()).unwrap();
    let second = BindingEngine::from_options_and_services(
        br#"{"environment":{"math_renderer":"none"}}"#,
        services,
    )
    .unwrap();
    assert_eq!(counter.calls(), 0, "construction must not call the host");

    first
        .execute(
            crate::BindingOperationRequest::new("svg", b"flowchart TD\nA[First]")
                .with_options_json(br#"{"svg":{"diagram_id":"first"}}"#),
        )
        .unwrap();
    second.render_svg(b"flowchart TD\nB[Second]").unwrap();
    assert!(counter.calls() > 0);
}

#[cfg(feature = "svg")]
#[test]
fn cloned_services_share_one_immutable_icon_registry_across_engines() {
    let pack = br#"{
            "prefix":"test",
            "icons":{
                "rocket":{
                    "body":"<path data-icon=\"binding-registry\" d=\"M0 0H16V16H0z\"/>"
                }
            }
        }"#;
    let registry = crate::build_icon_registry([crate::IconPack::new(pack)])
        .expect("valid Iconify pack through the binding admission seam");
    let services = BindingEngineServices::new().with_icon_registry(registry.clone());
    let first = BindingEngine::from_options_and_services(b"", services.clone()).unwrap();
    let second = BindingEngine::from_options_and_services(b"", services).unwrap();

    for engine in [&first, &second] {
        let svg = String::from_utf8(
            engine
                .render_svg(b"flowchart TD\nA@{ icon: \"test:rocket\", label: \"A\" }")
                .unwrap(),
        )
        .unwrap();
        assert!(svg.contains(r#"data-icon="binding-registry""#), "{svg}");
    }
    assert_eq!(registry.len(), 1);
}

#[cfg(all(feature = "svg", feature = "layout-cytoscape"))]
#[test]
fn empty_icon_registry_matches_no_service_svg_output() {
    let registry = crate::build_icon_registry(std::iter::empty::<crate::IconPack<'_>>())
        .expect("an empty registry is a valid immutable value");
    let with_empty_registry = BindingEngine::from_options_and_services(
        b"",
        BindingEngineServices::new().with_icon_registry(registry),
    )
    .unwrap();
    let without_services = BindingEngine::new(b"").unwrap();
    let source = b"architecture-beta\n  service api(server)[API]";

    assert_eq!(
        with_empty_registry.render_svg(source).unwrap(),
        without_services.render_svg(source).unwrap()
    );
}

#[test]
fn semantic_parse_accepts_exact_semantic_model_item_budget() {
    let engine = BindingEngine::new(
        br#"{
                "resources": {
                    "profile": "constrained",
                    "limits": { "max_model_items": 3 }
                }
            }"#,
    )
    .expect("semantic-json engine with exact model item budget");

    engine
        .parse_json(b"flowchart TD\nA --> B")
        .expect("two nodes and one edge consume three semantic model items");
}

#[test]
fn semantic_parse_preserves_the_public_compatibility_json_projection() {
    let source = "flowchart TD\nsubgraph outer\nA[Start] --> B[Done]\nend";
    let expected = merman::MermaidEngine::new()
        .parse_diagram_sync(source, merman::ParseOptions::strict())
        .expect("compatibility JSON parse")
        .expect("flowchart diagram")
        .model;
    let actual: Value = serde_json::from_slice(
        &BindingEngine::new(b"")
            .expect("semantic bindings engine")
            .parse_json(source.as_bytes())
            .expect("binding semantic JSON"),
    )
    .expect("binding semantic JSON value");

    assert_eq!(actual, expected);
}

#[cfg(feature = "diagram-mindmap")]
#[test]
fn semantic_parse_exports_beyond_generic_serde_depth() {
    use std::fmt::Write as _;
    let mut source = String::from("mindmap\nroot\n");
    for depth in 1..=150 {
        writeln!(source, "{}node{depth}", "  ".repeat(depth)).unwrap();
    }
    let expected = merman::MermaidEngine::new()
        .parse_diagram_sync(&source, merman::ParseOptions::strict())
        .unwrap()
        .unwrap()
        .model;
    assert!(serde_json::to_vec(&expected).is_err());
    let mut expected_bytes = Vec::new();
    expected.write_json(&mut expected_bytes).unwrap();
    let actual =
        BindingEngine::new(b"{\"resources\":{\"profile\":\"unbounded-for-trusted-input\"}}")
            .unwrap()
            .parse_json(source.as_bytes())
            .unwrap();
    assert_eq!(actual, expected_bytes);
}

#[test]
fn semantic_parse_resource_boundaries_match_the_typed_model() {
    let source = "flowchart TD\nsubgraph outer\nsubgraph inner\nA[Start] --> B[Done]\nend\nend";
    let parsed = merman::MermaidEngine::new()
        .parse_diagram_for_render_model_sync(source, merman::ParseOptions::strict())
        .expect("typed model parse")
        .expect("flowchart diagram");
    let complexity = merman::model::resources::model_complexity_from_render_model(parsed.model());

    for (limit, exact) in [
        ("max_model_items", complexity.items),
        ("max_model_text_bytes", complexity.text_bytes),
        ("max_model_nesting_depth", complexity.nesting_depth),
    ] {
        assert!(exact > 0, "fixture must exercise {limit}");
        let exact_options = format!(
            r#"{{"resources":{{"profile":"constrained","limits":{{"{limit}":{exact}}}}}}}"#
        );
        BindingEngine::new(exact_options.as_bytes())
            .expect("exact semantic model limit")
            .parse_json(source.as_bytes())
            .unwrap_or_else(|error| panic!("exact {limit} boundary failed: {error:?}"));

        let lower_options = format!(
            r#"{{"resources":{{"profile":"constrained","limits":{{"{limit}":{}}}}}}}"#,
            exact - 1
        );
        let error = BindingEngine::new(lower_options.as_bytes())
            .expect("lower semantic model limit")
            .parse_json(source.as_bytes())
            .expect_err("one-below semantic model boundary must fail");
        assert_eq!(error.status(), crate::BindingStatus::ResourceLimitExceeded);
        assert!(error.message().contains(limit), "{error:?}");
        let details = error
            .resource_details()
            .expect("structured resource details");
        assert_eq!(details.limit_id, limit);
        assert_eq!(details.phase, "layout_model");
        assert_eq!(details.max, (exact - 1) as u64);
        assert_eq!(details.profile, "constrained");
    }
}

#[test]
fn semantic_parse_enforces_model_text_budget() {
    assert_semantic_model_limit("max_model_text_bytes", b"flowchart TD\nA[Long label]");
}

#[test]
fn semantic_parse_enforces_model_nesting_budget() {
    assert_semantic_model_limit(
        "max_model_nesting_depth",
        b"flowchart TD\nsubgraph outer\nsubgraph inner\nA --> B\nend\nend",
    );
}

fn assert_semantic_model_limit(limit: &str, source: &[u8]) {
    let options =
        format!(r#"{{"resources":{{"profile":"constrained","limits":{{"{limit}":1}}}}}}"#);
    let engine = BindingEngine::new(options.as_bytes())
        .expect("semantic-json must accept every semantic model resource budget");

    let error = engine.parse_json(source).unwrap_err();

    assert_eq!(error.status(), crate::BindingStatus::ResourceLimitExceeded);
    assert!(error.message().contains(limit), "{error:?}");
}

#[cfg(feature = "svg")]
#[test]
fn engine_reuses_options_for_rendering() {
    let engine = BindingEngine::new(
        br#"{
                "environment": { "text_measurement": "deterministic" },
                "svg": { "diagram_id": "cached engine", "pipeline": "readable" }
            }"#,
    )
    .unwrap();

    let svg = String::from_utf8(engine.render_svg(b"flowchart TD\nA[Hello]").unwrap()).unwrap();
    assert!(svg.contains("id=\"cached-engine\""));
    assert!(svg.contains("data-merman-foreignobject"));
}

#[cfg(all(feature = "svg", feature = "ascii"))]
#[test]
fn cached_engine_accepts_render_only_resource_limits() {
    let engine = BindingEngine::new(
        br#"{
                "resources": {
                    "profile": "constrained",
                    "limits": { "max_svg_bytes": 1048576 }
                }
            }"#,
    )
    .expect("a multi-operation engine must project each limit to its owning operation");

    let ascii = String::from_utf8(
        engine
            .render_ascii(b"flowchart TD\nA --> B")
            .expect("an unrelated render-only limit must not disable ASCII"),
    )
    .expect("ASCII output is UTF-8");
    assert!(!ascii.is_empty());
}

#[cfg(feature = "analysis")]
#[test]
fn engine_validates_with_cached_renderer() {
    let engine = BindingEngine::new(b"").unwrap();
    let validation: Value = serde_json::from_slice(&engine.validate_json(b"").unwrap()).unwrap();

    assert_eq!(validation["valid"], false);
    assert_eq!(validation["code_name"], "MERMAN_NO_DIAGRAM");
}

#[cfg(feature = "analysis")]
#[test]
fn engine_analyzes_markdown_documents() {
    let engine = BindingEngine::new(b"").unwrap();
    let payload: Value = serde_json::from_slice(
        &engine
            .analyze_document_json(
                b"before\n```mermaid\nflowchart TD\nA-->\n```\nafter\n",
                b"file:///tmp/example.md",
            )
            .unwrap(),
    )
    .unwrap();

    assert_eq!(payload["source"]["kind"], "markdown");
    assert!(
        payload["diagnostics"][0]["related"]
            .as_array()
            .unwrap()
            .iter()
            .any(|related| related["message"] == "Mermaid fence 1")
    );
}

#[cfg(feature = "analysis")]
#[test]
fn document_analysis_enforces_the_profile_diagram_limit() {
    let engine = BindingEngine::new(br#"{"resources":{"profile":"constrained"}}"#).unwrap();
    let source = "```mermaid\nflowchart TD\nA\n```\n".repeat(129);

    let payload: Value = serde_json::from_slice(
        &engine
            .analyze_document_json(source.as_bytes(), b"file:///tmp/many.md")
            .expect("analysis rejection is represented as a diagnostics payload"),
    )
    .unwrap();

    assert_eq!(payload["valid"], false);
    assert_eq!(
        payload["diagnostics"][0]["id"],
        "merman.resource.document_diagrams_exceeded"
    );
    assert!(
        payload["diagnostics"][0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("128"))
    );
}

#[cfg(not(feature = "analysis"))]
#[test]
fn engine_reports_missing_analysis_feature() {
    let engine = BindingEngine::new(b"").unwrap();

    let err = engine.validate_json(b"flowchart TD\nA").unwrap_err();
    assert_eq!(err.status(), crate::BindingStatus::UnsupportedOperation);
    assert_eq!(err.kind(), crate::BindingErrorKind::MissingCapability);
    assert_eq!(err.capability_id(), Some("analysis"));

    let err = engine
        .analyze_document_json(b"flowchart TD\nA", b"file:///tmp/example.mmd")
        .unwrap_err();
    assert_eq!(err.status(), crate::BindingStatus::UnsupportedOperation);
    assert_eq!(err.kind(), crate::BindingErrorKind::MissingCapability);
    assert_eq!(err.capability_id(), Some("analysis"));
}

#[test]
fn engine_can_render_concurrently() {
    let engine = Arc::new(BindingEngine::new(b"").unwrap());
    let mut handles = Vec::new();

    for _ in 0..8 {
        let engine = Arc::clone(&engine);
        handles.push(std::thread::spawn(move || {
            for _ in 0..8 {
                let svg = engine.render_svg(b"flowchart TD\nA[Hello] --> B[World]");
                if cfg!(feature = "svg") {
                    let svg = String::from_utf8(svg.unwrap()).unwrap();
                    assert!(svg.contains("<svg"));
                } else {
                    let err = svg.unwrap_err();
                    assert_eq!(err.status(), crate::BindingStatus::UnsupportedOperation);
                }
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }
}
