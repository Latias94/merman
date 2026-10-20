#![cfg(all(feature = "svg", feature = "diagram-kanban", feature = "diagram-pie"))]

use merman_bindings_core::{
    BindingEngine, BindingEngineServices, BindingOperationMetadata, BindingOperationRequest,
    BindingStatus, OperationControl,
};
use serde_json::Value;

const SOURCE: &[u8] = b"---\nconfig:\n  mindmap:\n    padding: 8\n---\nkanban\n  todo[Todo]\n    a[First]\n  doing[Doing]\n    b[Second]\n  done[Done]\n    c[Third]";
const NARROW: &[u8] = br#"{"layout":{"adaptation":"fit-width","container_width":421}}"#;

#[test]
fn fit_width_is_a_real_source_and_prepared_layout_option() {
    let engine = BindingEngine::new(b"").unwrap();
    let prepared = engine
        .prepare_source(SOURCE, OperationControl::new())
        .unwrap();
    let services = BindingEngineServices::new();
    for bytes in [
        merman_bindings_core::layout_json(SOURCE, NARROW).unwrap(),
        prepared
            .layout_json(NARROW, OperationControl::new(), &services)
            .unwrap(),
    ] {
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        let columns = value["layout"]["KanbanDiagram"]["sections"]
            .as_array()
            .unwrap();
        assert_eq!(columns[0]["rect_y"], columns[1]["rect_y"]);
        assert!(columns[2]["rect_y"].as_f64().unwrap() > columns[0]["rect_y"].as_f64().unwrap());
        assert_eq!(value["meta"]["layout_adaptation"]["actual_width"], 421.0);
        assert_eq!(value["meta"]["layout_adaptation"]["overflowed"], false);
    }
}

#[test]
fn empty_geometry_cannot_swallow_an_unsupported_adaptation() {
    let source = b"pie\n  \"One\" : 1";
    let engine = BindingEngine::new(b"").unwrap();
    let prepared = engine
        .prepare_source(source, OperationControl::new())
        .unwrap();
    for error in [
        merman_bindings_core::edge_geometry_json(source, NARROW).unwrap_err(),
        prepared
            .edge_geometry_json(
                NARROW,
                OperationControl::new(),
                &BindingEngineServices::new(),
            )
            .unwrap_err(),
    ] {
        assert_eq!(error.status(), BindingStatus::RenderError);
        assert!(error.message().contains("fit-width"), "{error:?}");
        assert!(error.message().contains("pie"), "{error:?}");
    }
}

#[test]
fn svg_results_and_capability_plans_expose_actual_adaptation() {
    let engine = BindingEngine::new(b"").unwrap();
    let prepared = engine
        .prepare_source(SOURCE, OperationControl::new())
        .unwrap();
    let services = BindingEngineServices::new();
    let source_result = engine
        .execute(BindingOperationRequest::new("svg", SOURCE).with_options_json(NARROW))
        .unwrap();
    let retained_result = prepared
        .render_svg_result(NARROW, OperationControl::new(), &services)
        .unwrap();
    assert_eq!(source_result.data(), retained_result.data());
    for result in [source_result, retained_result] {
        let observation = result.metadata().layout_adaptation().unwrap();
        assert_eq!(observation.mode(), "fit-width");
        assert_eq!(
            (
                observation.requested_width(),
                observation.actual_width(),
                observation.overflowed()
            ),
            (421.0, 421.0, false)
        );
        assert!(result.metadata().output_plan().is_none());
        let wire: Value = serde_json::from_slice(result.metadata().json_bytes()).unwrap();
        assert_eq!(wire["layout_adaptation"]["mode"], "fit-width");
        let roundtrip =
            BindingOperationMetadata::from_json_bytes(result.metadata().json_bytes()).unwrap();
        assert_eq!(
            roundtrip.layout_adaptation(),
            result.metadata().layout_adaptation()
        );
    }
    let plain = engine
        .execute(BindingOperationRequest::new("svg", SOURCE))
        .unwrap();
    assert!(plain.metadata().layout_adaptation().is_none());
    assert!(!String::from_utf8_lossy(plain.metadata().json_bytes()).contains("layout_adaptation"));
    for (source, expected) in [
        (SOURCE, serde_json::json!(["none", "fit-width"])),
        (
            b"pie\n  \"One\" : 1".as_slice(),
            serde_json::json!(["none"]),
        ),
    ] {
        let plan: Value =
            serde_json::from_slice(&merman_bindings_core::svg_plan_json(source, b"").unwrap())
                .unwrap();
        assert_eq!(plan["supported_layout_adaptation_ids"], expected);
    }
    let error = merman_bindings_core::svg_plan_json(b"pie\n  \"One\" : 1", NARROW).unwrap_err();
    assert!(error.message().contains("fit-width"));
}

#[cfg(all(feature = "png", feature = "jpeg", feature = "pdf"))]
#[test]
fn terminal_metadata_keeps_adaptation_beside_its_existing_output_plan() {
    let engine = BindingEngine::new(b"").unwrap();
    for operation in ["png", "jpeg", "pdf"] {
        let result = engine
            .execute(BindingOperationRequest::new(operation, SOURCE).with_options_json(NARROW))
            .unwrap();
        assert_eq!(
            result
                .metadata()
                .layout_adaptation()
                .unwrap()
                .actual_width(),
            421.0
        );
        assert!(result.metadata().output_plan().is_some());
        assert!(!result.data().is_empty());
    }
}
