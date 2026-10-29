#![cfg(all(
    feature = "native-fonts",
    feature = "diagram-mindmap",
    feature = "layout-cytoscape"
))]

use merman::svg::export::{NativeFontContext, NativeFontLimits, NativeFontOptions};
use merman::{RenderOutput, RenderRequest, RenderTarget, Renderer, SvgEnvironment, SvgRequest};
use merman_bindings_core::{
    ArtifactContractSpec, BindingEngine, BindingEngineServices, BindingOperationRequest,
    BindingStatus, BindingTransportKey, ConstructorServiceKey, HostMeasurementResult,
    HostTextMeasurementRequest, HostTextMeasurer, OperationControl, OperationKey, TargetKey,
};
use std::sync::Arc;

const SOURCE: &str = "---\nconfig:\n  fontFamily: Cousine\n---\nmindmap\nroot\n  child[Square]\n    plain\n    with icon\n    ::icon(mdi mdi-fire)\n";
const OPTIONS: &[u8] = br#"{"svg":{"diagram_id":"native-binding","pipeline":"resvg-safe"}}"#;

fn context(limits: NativeFontLimits, control: &OperationControl) -> NativeFontContext {
    let fonts =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../merman-export/tests/fonts");
    NativeFontOptions::default()
        .with_font_file(fonts.join("Cousine-Regular.ttf"))
        .with_font_file(fonts.join("Cousine-Bold.ttf"))
        .with_font_file(fonts.join("Cousine-Italic.ttf"))
        .with_limits(limits)
        .load(control)
        .expect("the checked-in closed font set loads")
}

fn services(context: NativeFontContext) -> BindingEngineServices {
    BindingEngineServices::new().with_native_font_context(context)
}

fn facade_svg(context: &NativeFontContext) -> SvgRequest {
    SvgRequest {
        environment: SvgEnvironment::deterministic()
            .with_native_font_context(context.clone())
            .expect("reusable native font environment"),
        options: merman::svg::SvgRenderOptions {
            diagram_id: Some("native-binding".to_owned()),
            ..Default::default()
        },
        pipeline: Some(merman::svg::SvgPipeline::resvg_safe()),
        ..Default::default()
    }
}

#[test]
fn native_binding_mindmap_svg_matches_the_facade_and_preserves_all_labels() {
    let context = context(Default::default(), &OperationControl::new());
    let engine = BindingEngine::from_options_and_services(OPTIONS, services(context.clone()))
        .expect("native font service is accepted by the Rust binding facade");
    let control = OperationControl::new();
    let expected = Renderer::new()
        .render(RenderRequest::new(
            SOURCE,
            RenderTarget::Svg(facade_svg(&context)),
            control,
        ))
        .expect("the same facade native SVG request succeeds");
    let RenderOutput::Svg(Some(expected)) = expected else {
        panic!("expected native SVG");
    };
    let actual = engine.render_svg(SOURCE.as_bytes()).expect("binding SVG");
    assert_eq!(actual, expected.svg().as_bytes());
    let svg = std::str::from_utf8(&actual).expect("SVG UTF-8");
    let document = roxmltree::Document::parse(svg).expect("SVG XML");
    for label in ["Square", "plain", "with icon"] {
        assert!(
            document
                .descendants()
                .filter(|node| node.has_tag_name("text"))
                .any(|node| node
                    .descendants()
                    .filter(|child| child.is_text())
                    .filter_map(|child| child.text())
                    .collect::<String>()
                    == label),
            "the exported fallback must contain the complete label {label:?}"
        );
    }
    assert!(
        context.selected_faces().is_empty(),
        "engine operations must not mutate the reusable context's measurement state"
    );
}

#[cfg(feature = "png")]
#[test]
fn native_binding_mindmap_png_matches_the_facade_using_the_same_fonts() {
    let context = context(Default::default(), &OperationControl::new());
    let engine = BindingEngine::from_options_and_services(OPTIONS, services(context.clone()))
        .expect("native binding engine");
    let control = OperationControl::new();
    let expected = Renderer::new()
        .render(RenderRequest::new(
            SOURCE,
            RenderTarget::Png(merman::PngRequest {
                svg: facade_svg(&context),
                options: Default::default(),
            }),
            control,
        ))
        .expect("the facade native PNG request succeeds");
    let RenderOutput::Png(Some(expected)) = expected else {
        panic!("expected native PNG");
    };
    let actual = engine
        .execute(BindingOperationRequest::new("png", SOURCE.as_bytes()))
        .expect("binding native PNG");
    assert_eq!(actual.data(), expected.bytes());
    assert!(context.selected_faces().is_empty());
}

#[cfg(feature = "pdf")]
#[test]
fn native_binding_mindmap_pdf_matches_the_facade_using_the_same_fonts() {
    let context = context(Default::default(), &OperationControl::new());
    let engine = BindingEngine::from_options_and_services(OPTIONS, services(context.clone()))
        .expect("native binding engine");
    let control = OperationControl::new();
    let expected = Renderer::new()
        .render(RenderRequest::new(
            SOURCE,
            RenderTarget::Pdf(merman::PdfRequest {
                svg: facade_svg(&context),
                options: Default::default(),
            }),
            control,
        ))
        .expect("the facade native PDF request succeeds");
    let RenderOutput::Pdf(Some(expected)) = expected else {
        panic!("expected native PDF");
    };
    let actual = engine
        .execute(BindingOperationRequest::new("pdf", SOURCE.as_bytes()))
        .expect("binding native PDF");
    assert_eq!(actual.data(), expected.bytes());
    assert!(context.selected_faces().is_empty());
}

struct PanicMeasurer;

impl HostTextMeasurer for PanicMeasurer {
    fn measure(&self, _: HostTextMeasurementRequest<'_>) -> HostMeasurementResult {
        panic!("conflicting services must be rejected before measurement");
    }
}

#[test]
fn native_font_service_rejects_host_measurement_in_either_configuration_order() {
    let context = context(Default::default(), &OperationControl::new());
    for services in [
        services(context.clone()).with_host_text_measurer(Arc::new(PanicMeasurer)),
        BindingEngineServices::new()
            .with_host_text_measurer(Arc::new(PanicMeasurer))
            .with_native_font_context(context),
    ] {
        let error = BindingEngine::from_options_and_services(b"", services)
            .err()
            .expect("neither setter may silently replace the other service");
        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        assert!(error.message().contains("native-font-context"));
        assert!(error.message().contains("host-text-measurement"));
    }
}

#[test]
fn native_font_service_rejects_explicit_base_and_overlay_measurement_selectors() {
    let context = context(Default::default(), &OperationControl::new());
    let engine = BindingEngine::from_options_and_services(OPTIONS, services(context.clone()))
        .expect("an omitted measurement selector admits the native service");
    for options in [
        br#"{"environment":{"text_measurement":"deterministic"}}"#.as_slice(),
        br#"{"environment":{"text_measurement":null}}"#.as_slice(),
    ] {
        let constructor =
            BindingEngine::from_options_and_services(options, services(context.clone()))
                .err()
                .expect("explicit constructor selector conflicts");
        let overlay = engine
            .execute(
                BindingOperationRequest::new("svg", SOURCE.as_bytes()).with_options_json(options),
            )
            .expect_err("explicit overlay selector conflicts");
        for error in [constructor, overlay] {
            assert_eq!(error.status(), BindingStatus::InvalidArgument);
            assert!(error.message().contains("native-font-context"));
            assert!(error.message().contains("environment.text_measurement"));
        }
    }
}

#[test]
fn native_font_measurement_failures_do_not_poison_later_cached_or_overlay_requests() {
    let context = context(
        NativeFontLimits {
            max_label_bytes: 96,
            ..Default::default()
        },
        &OperationControl::new(),
    );
    let engine = BindingEngine::from_options_and_services(OPTIONS, services(context.clone()))
        .expect("font limits belong to each operation");
    let expected = engine
        .render_svg(SOURCE.as_bytes())
        .expect("short labels fit the native limit");
    let cloned_engine = engine.clone();
    let long = format!(
        "---\nconfig:\n  fontFamily: Cousine\n---\nmindmap\n{}\n",
        "W".repeat(512)
    );
    let plan = engine
        .execute(BindingOperationRequest::new(
            "svg-plan-json",
            long.as_bytes(),
        ))
        .expect("capability planning does not measure text or consume font measurement limits");
    let plan: serde_json::Value =
        serde_json::from_slice(plan.data()).expect("capability plan JSON");
    assert_eq!(plan["diagram_type"], "mindmap");
    assert_eq!(plan["ready"], true);
    for operation in [
        "svg",
        "layout-json",
        #[cfg(feature = "png")]
        "png",
        #[cfg(feature = "pdf")]
        "pdf",
    ] {
        let error = engine
            .execute(BindingOperationRequest::new(operation, long.as_bytes()))
            .expect_err("the long label exceeds the native font boundary");
        assert_eq!(
            error.status(),
            BindingStatus::ResourceLimitExceeded,
            "{operation}: {error:?}"
        );
        let details = error
            .resource_details()
            .expect("structured native resource error");
        assert_eq!(details.limit_id, "native_font_label_bytes");
        assert_eq!(details.max, 96);
        assert!(details.actual > details.max);
        assert_eq!(
            engine
                .render_svg(SOURCE.as_bytes())
                .expect("a failed operation cannot poison the cached plan"),
            expected
        );
        assert_eq!(
            cloned_engine
                .render_svg(SOURCE.as_bytes())
                .expect("cloned engines also start fresh font operations"),
            expected
        );
        let overlay = engine
            .execute(
                BindingOperationRequest::new("svg", SOURCE.as_bytes())
                    .with_options_json(br#"{"svg":{"viewbox_padding":1}}"#),
            )
            .expect("an overlay also starts a fresh font operation");
        assert!(!overlay.data().is_empty());
    }
    assert!(context.selected_faces().is_empty());
    context
        .check()
        .expect("the constructor's reusable context remains healthy");
}

#[test]
fn native_font_context_does_not_reuse_its_loading_control_or_cancelled_requests() {
    let loading = OperationControl::new();
    let context = context(Default::default(), &loading);
    let engine = BindingEngine::from_options_and_services(OPTIONS, services(context))
        .expect("native engine stores font data without binding the loading operation");
    loading.cancel();
    let expected = engine
        .render_svg(SOURCE.as_bytes())
        .expect("loading cancellation is not a render cancellation");
    let cancelled = OperationControl::new();
    cancelled.cancel();
    let expired = OperationControl::new().with_deadline(std::time::Duration::ZERO);
    for control in [cancelled, expired] {
        let error = engine
            .execute(BindingOperationRequest::new("svg", SOURCE.as_bytes()).with_control(control))
            .expect_err("active request cancellation remains structured");
        assert_eq!(error.status(), BindingStatus::Cancelled);
        assert!(error.cancellation_details().is_some());
        assert!(error.resource_details().is_none());
        assert_eq!(
            engine
                .render_svg(SOURCE.as_bytes())
                .expect("a fresh request stays usable"),
            expected
        );
    }
}

#[test]
fn prepared_native_outputs_isolate_service_controls_and_measurement_failures() {
    let loading = OperationControl::new();
    let limited_fonts = context(
        NativeFontLimits {
            max_label_bytes: 1,
            ..Default::default()
        },
        &loading,
    );
    let limited = services(limited_fonts.clone());
    let healthy = services(context(Default::default(), &OperationControl::new()));
    let prepared = BindingEngine::new(OPTIONS)
        .unwrap()
        .prepare_source(SOURCE.as_bytes(), OperationControl::new())
        .unwrap();
    let control = OperationControl::new();
    let expected = prepared.render_svg(b"", control.clone(), &healthy).unwrap();
    loading.cancel();

    for error in [
        prepared
            .render_svg(b"", control.clone(), &limited)
            .expect_err("SVG bytes enforce the active font limit"),
        prepared
            .render_svg_result(b"", control.clone(), &limited)
            .expect_err("SVG results enforce the same active font limit"),
        prepared
            .layout_json(b"", control.clone(), &limited)
            .expect_err("prepared layout also measures through an independent font operation"),
    ] {
        assert_eq!(error.status(), BindingStatus::ResourceLimitExceeded);
        let details = error.resource_details().unwrap();
        assert_eq!(details.limit_id, "native_font_label_bytes");
        assert_eq!(details.max, 1);
    }
    let geometry = prepared
        .edge_geometry_json(b"", control.clone(), &limited)
        .expect("empty Mindmap geometry does not measure text");
    let geometry: serde_json::Value = serde_json::from_slice(&geometry).unwrap();
    assert_eq!(geometry["edges"], serde_json::json!([]));
    assert_eq!(
        prepared
            .render_svg_result(b"", control, &healthy)
            .expect("a later output can select healthy services with the same caller control")
            .data(),
        expected.as_slice(),
    );
    assert!(limited_fonts.selected_faces().is_empty());
}

#[test]
fn native_font_measurement_counts_reset_for_each_reusable_engine_operation() {
    let context = context(
        NativeFontLimits {
            max_measurements: 64,
            max_cache_entries: 0,
            ..Default::default()
        },
        &OperationControl::new(),
    );
    let engine = BindingEngine::from_options_and_services(OPTIONS, services(context))
        .expect("reusable native font engine");
    let source = b"---\nconfig:\n  fontFamily: Cousine\n---\nmindmap\nroot\n";
    let expected = engine
        .render_svg(source)
        .expect("one label fits the operation ceiling");
    for _ in 0..64 {
        assert_eq!(
            engine
                .render_svg(source)
                .expect("measurement counts are request-local"),
            expected
        );
    }
}

#[test]
fn native_font_sources_and_discovery_are_not_binding_json_options() {
    for options in [
        br#"{"environment":{"font_files":["font.ttf"]}}"#.as_slice(),
        br#"{"environment":{"system_fonts":true}}"#.as_slice(),
        br#"{"environment":{"native_font_context":{}}}"#.as_slice(),
    ] {
        let error = BindingEngine::new(options)
            .err()
            .expect("font acquisition remains outside the binding request boundary");
        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
    }
}

#[test]
fn native_font_service_and_provider_metadata_follow_installable_transports() {
    let contract = ArtifactContractSpec::new(TargetKey::Native, BindingTransportKey::Rust)
        .with_operations(&[OperationKey::Svg])
        .with_constructor_services(&[ConstructorServiceKey::NativeFontContext])
        .materialize();
    let catalog = contract.runtime_catalog(1);
    assert_eq!(catalog.constructor_service_ids, ["native-font-context"]);
    assert_eq!(
        catalog.capabilities.text_measurement.unwrap().provider_ids,
        ["deterministic", "native-fonts"]
    );
    assert_eq!(
        catalog.constructor_service_contracts[0].provided_text_measurement_provider_ids,
        ["native-fonts"]
    );
    for transport in BindingTransportKey::ALL {
        assert_eq!(
            transport
                .spec()
                .constructor_service_candidates()
                .contains(&ConstructorServiceKey::NativeFontContext),
            matches!(
                transport,
                BindingTransportKey::Rust
                    | BindingTransportKey::NativeC
                    | BindingTransportKey::Wasi
            ),
            "{transport:?} must report only installable services"
        );
    }
    for transport in [BindingTransportKey::NativeC, BindingTransportKey::Wasi] {
        let selected = ArtifactContractSpec::new(TargetKey::Native, transport)
            .with_operations(&[OperationKey::Svg])
            .with_supplemental_capabilities(&[merman_bindings_core::CapabilityKey::LayoutCytoscape])
            .with_constructor_services(&[ConstructorServiceKey::NativeFontContext])
            .materialize();
        assert!(
            selected
                .runtime_capabilities()
                .has_capability("native-fonts")
        );
        let engine = selected
            .create_engine_with_services(
                OPTIONS,
                services(context(Default::default(), &OperationControl::new())),
            )
            .expect("selected native transports can install the already admitted context");
        assert!(!engine.render_svg(SOURCE.as_bytes()).unwrap().is_empty());
    }
    let context = context(Default::default(), &OperationControl::new());
    for transport in [
        BindingTransportKey::NativeC,
        BindingTransportKey::UniFfi,
        BindingTransportKey::AndroidJni,
    ] {
        let foreign = ArtifactContractSpec::new(TargetKey::Native, transport)
            .with_operations(&[OperationKey::Svg])
            .materialize();
        let error = foreign
            .create_engine_with_services(b"", services(context.clone()))
            .err()
            .expect("unadvertised artifacts cannot install the native font context");
        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        assert!(error.message().contains("not exposed"));
    }
}
