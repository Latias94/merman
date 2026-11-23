#![cfg(all(feature = "svg", feature = "diagram-flowchart"))]

use merman_bindings_core::{BindingEngine, BindingOperationRequest, BindingStatus};

#[cfg(feature = "layout-shoal")]
const SOURCE: &[u8] = b"---\nconfig:\n  layout: shoal\n---\nflowchart TD\nRoot --> A\nRoot --> B";
const CAPTURE: &[u8] = br#"{"layout":{"capture_shoal_frame":true}}"#;

#[cfg(feature = "layout-shoal")]
#[test]
fn svg_results_round_trip_opaque_shoal_frames_without_embedding_them() {
    use merman_bindings_core::{BindingEngineServices, BindingOperationMetadata, OperationControl};
    use serde_json::{Value, json};

    let engine = BindingEngine::new(b"").unwrap();
    let plain = engine
        .execute(BindingOperationRequest::new("svg", SOURCE))
        .unwrap();
    let captured = engine
        .execute(BindingOperationRequest::new("svg", SOURCE).with_options_json(CAPTURE))
        .unwrap();
    let frame = captured.metadata().shoal_layout_frame().unwrap();
    assert!(frame.is_object());
    assert_eq!(plain.data(), captured.data());
    assert!(plain.metadata().shoal_layout_frame().is_none());
    let original_wire: Value = serde_json::from_slice(plain.metadata_json()).unwrap();
    assert!(original_wire.get("shoal_layout_frame").is_none());
    let round_trip = BindingOperationMetadata::from_json_bytes(captured.metadata_json()).unwrap();
    assert_eq!(round_trip.shoal_layout_frame(), Some(frame));
    assert!(
        !std::str::from_utf8(captured.data())
            .unwrap()
            .contains("shoal_layout_frame")
    );
    let snapshot = merman_bindings_core::layout_json(SOURCE, CAPTURE).unwrap();
    assert!(
        !std::str::from_utf8(&snapshot)
            .unwrap()
            .contains("shoal_layout_frame")
    );

    let next = [SOURCE, b"\nB --> C[New]"].concat();
    let prepared = engine
        .prepare_source(&next, OperationControl::new())
        .unwrap();
    let replay = serde_json::to_vec(&json!({"layout":{"previous_shoal_frame":frame}})).unwrap();
    let redrawn = prepared
        .render_svg_result(
            &replay,
            OperationControl::new(),
            &BindingEngineServices::new(),
        )
        .unwrap();
    assert!(redrawn.metadata().shoal_layout_frame().unwrap().is_object());
    assert_ne!(redrawn.metadata().shoal_layout_frame(), Some(frame));
}

#[cfg(feature = "layout-shoal")]
#[test]
fn shoal_preview_options_use_typed_validation_and_existing_adaptation_metadata() {
    let engine = BindingEngine::new(b"").unwrap();
    for (options, status, message) in [
        (
            br#"{"layout":{"previous_shoal_frame":[]}}"#.as_slice(),
            BindingStatus::InvalidArgument,
            "layout.previous_shoal_frame must be an object",
        ),
        (
            br#"{"layout":{"fold_wide_ranks":"yes"}}"#.as_slice(),
            BindingStatus::OptionsJsonError,
            "invalid options_json",
        ),
    ] {
        let error = engine
            .execute(BindingOperationRequest::new("svg", SOURCE).with_options_json(options))
            .unwrap_err();
        assert_eq!(error.status(), status, "{error:?}");
        assert!(error.message().contains(message), "{error:?}");
    }
    let options =
        br#"{"layout":{"adaptation":"fit-width","container_width":640,"fold_wide_ranks":true}}"#;
    let output = engine
        .execute(BindingOperationRequest::new("svg", SOURCE).with_options_json(options))
        .unwrap();
    let observation = output.metadata().layout_adaptation().unwrap();
    assert_eq!(observation.mode(), "fit-width");
    assert_eq!(observation.requested_width(), 640.0);
    assert!(output.metadata().shoal_layout_frame().is_none());
}

#[cfg(feature = "layout-shoal")]
#[test]
fn empty_shoal_fit_width_metadata_round_trips_a_zero_actual_width() {
    use merman_bindings_core::BindingOperationMetadata;
    use serde_json::{Value, json};

    let engine = BindingEngine::new(b"").unwrap();
    let source = b"---\nconfig:\n  layout: shoal\n---\nflowchart LR";
    let options = br#"{"layout":{"adaptation":"fit-width","container_width":640}}"#;
    let result = engine
        .execute(BindingOperationRequest::new("svg", source).with_options_json(options))
        .unwrap();
    let decoded = BindingOperationMetadata::from_json_bytes(result.metadata_json()).unwrap();
    let observation = decoded.layout_adaptation().unwrap();
    assert_eq!(observation.requested_width(), 640.0);
    assert_eq!(observation.actual_width(), 0.0);
    assert!(!observation.overflowed());

    let mut wire: Value = serde_json::from_slice(result.metadata_json()).unwrap();
    wire["layout_adaptation"]["actual_width"] = json!(-1);
    assert!(
        BindingOperationMetadata::from_json_bytes(&serde_json::to_vec(&wire).unwrap()).is_err()
    );
    wire["layout_adaptation"]["actual_width"] = json!(0);
    wire["layout_adaptation"]["requested_width"] = json!(0);
    assert!(
        BindingOperationMetadata::from_json_bytes(&serde_json::to_vec(&wire).unwrap()).is_err()
    );
}

#[cfg(feature = "layout-shoal")]
#[test]
fn a_long_shared_group_id_can_replay_successive_previews() {
    use serde_json::json;
    use std::fmt::Write as _;

    let engine = BindingEngine::new(b"").unwrap();
    let group = "g".repeat(24_000);
    let mut source =
        format!("---\nconfig:\n  layout: shoal\n---\nflowchart TB\nsubgraph {group}[Group]\n");
    for index in 0..180 {
        writeln!(source, "  node{index}").unwrap();
    }
    source.push_str("end");
    let mut options = CAPTURE.to_vec();
    for label in ["First", "Second", "Third"] {
        let edit = source.replacen("  node0\n", &format!("  node0[{label}]\n"), 1);
        let result = engine
            .execute(
                BindingOperationRequest::new("svg", edit.as_bytes()).with_options_json(&options),
            )
            .unwrap();
        let frame = result.metadata().shoal_layout_frame().unwrap();
        let receipt = serde_json::to_string(frame).unwrap();
        assert_eq!(receipt.matches(&group).count(), 1);
        options = serde_json::to_vec(&json!({"layout":{"previous_shoal_frame":frame}})).unwrap();
    }
}

#[cfg(not(feature = "layout-shoal"))]
#[test]
fn explicit_shoal_preview_options_report_the_missing_capability() {
    use merman_bindings_core::BindingErrorKind;

    let engine = BindingEngine::new(b"").unwrap();
    let source = b"flowchart TD\nA --> B";
    for options in [
        CAPTURE,
        br#"{"layout":{"fold_wide_ranks":true}}"#.as_slice(),
    ] {
        let error = engine
            .execute(BindingOperationRequest::new("svg", source).with_options_json(options))
            .unwrap_err();
        assert_eq!(error.status(), BindingStatus::UnsupportedOperation);
        assert_eq!(error.kind(), BindingErrorKind::MissingCapability);
        assert_eq!(error.capability_id(), Some("layout-shoal"));
    }
    let off = br#"{"layout":{"fold_wide_ranks":false,"capture_shoal_frame":false}}"#;
    let output = engine
        .execute(BindingOperationRequest::new("svg", source).with_options_json(off))
        .unwrap();
    assert!(output.metadata().shoal_layout_frame().is_none());
}
