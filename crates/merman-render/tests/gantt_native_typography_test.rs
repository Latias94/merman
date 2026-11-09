#![cfg(feature = "diagram-gantt")]

use merman_core::{MermaidEngine, ParseOptions};
use merman_model::presentation::PresentationDiagram;
use merman_model::{CancelReason, OperationControl, OperationPhase};
use merman_render::__private::unsupported_native_text_style;
use serde_json::{Value, json};

fn parsed(body: &str, config: Value) -> PresentationDiagram {
    MermaidEngine::new()
        .with_site_config(merman_core::presentation::SourcePresentationConfig::from_value(config))
        .parse_diagram_for_render_model_sync(body, ParseOptions::strict())
        .unwrap()
        .unwrap()
        .into_presentation()
}

#[test]
fn gantt_native_admission_rejects_only_consumed_unknown_section_sizes() {
    let task = "Ship: a, 2024-01-02, 1d\n";
    let marker = "Marker: vert, v, 2024-01-02, 0d\n";
    for raw in ["1rem", "calc(10px + 1em)", "1cap"] {
        for (field, heading, tasks, expected) in [
            ("fontSize", "", task, None),
            ("fontSize", "", marker, None),
            ("fontSize", "title Release\nsection Unused\n", "", None),
            (
                "sectionFontSize",
                "section Work\n",
                task,
                Some("gantt.sectionFontSize (unresolved CSS value)"),
            ),
            ("sectionFontSize", "section Unused\n", "", None),
            ("sectionFontSize", "", task, None),
            ("sectionFontSize", "section Work\n", marker, None),
            ("sectionFontSize", "section <br><BR />\n", task, None),
        ] {
            for compact in [false, true] {
                let mut config = json!({
                    "themeVariables": {"fontSize": "24px"},
                    "gantt": {"fontSize": 11, "sectionFontSize": 11}
                });
                config["gantt"][field] = json!(raw);
                if compact {
                    config["gantt"]["displayMode"] = json!("compact");
                }
                let body =
                    format!("gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\n{heading}{tasks}");
                assert_eq!(
                    unsupported_native_text_style(&parsed(&body, config), &OperationControl::new())
                        .unwrap()
                        .as_deref(),
                    expected,
                    "{field}={raw}, compact={compact}, source={body}"
                );
            }
        }
    }
}

#[test]
fn gantt_native_admission_accepts_known_used_attribute_sizes() {
    let body =
        "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nsection Work\nShip: a, 2024-01-02, 1d\n";
    for root in ["0px", "24px"] {
        for value in [
            json!(11),
            json!(0),
            json!(-1),
            json!("-1em"),
            json!("24pt"),
            json!(".5px"),
            json!("2em"),
            json!("50%"),
            json!("inherit"),
            json!("0rem"),
            json!("0cap"),
            json!("invalid"),
            json!("0px!important"),
            json!("0px;"),
        ] {
            let config = json!({
                "themeVariables": {"fontSize": root},
                "gantt": {"fontSize": value.clone(), "sectionFontSize": value.clone()}
            });
            assert_eq!(
                unsupported_native_text_style(&parsed(body, config), &OperationControl::new())
                    .unwrap(),
                None,
                "root={root}, size={value}"
            );
        }
    }
}

#[test]
fn gantt_native_admission_keeps_semantic_cancellation() {
    let diagram = parsed(
        "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\n",
        json!({"gantt": {"fontSize": "1rem", "sectionFontSize": "1rem"}}),
    );
    let control = OperationControl::new();
    control.cancel();
    let error = unsupported_native_text_style(&diagram, &control).unwrap_err();
    assert_eq!(error.reason, CancelReason::Requested);
    assert_eq!(error.phase, OperationPhase::Semantic);
}

#[test]
fn gantt_native_admission_checks_cancellation_while_scanning_unused_sections() {
    let body = format!(
        "gantt\ndateFormat YYYY-MM-DD\ntodayMarker off\nsection {}\nShip: a, 2024-01-02, 1d\n",
        "<br>".repeat(8192)
    );
    let diagram = parsed(&body, json!({"gantt": {"sectionFontSize": "1rem"}}));
    let control = OperationControl::new();
    control.cancel_after_checkpoints(8);
    let error = unsupported_native_text_style(&diagram, &control).unwrap_err();
    assert_eq!(error.reason, CancelReason::Requested);
    assert_eq!(error.phase, OperationPhase::Semantic);
}
