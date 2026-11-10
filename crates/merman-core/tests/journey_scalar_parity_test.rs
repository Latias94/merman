#![cfg(feature = "diagram-journey")]

use merman_core::SemanticParser;
use merman_model::semantic::SemanticDiagramModel;

#[test]
fn journey_number_scores_retain_signed_infinity_and_legacy_nan_state() {
    // Mermaid 12.3.0 journeyDb.js applies Number() to unrestricted taskData text.
    for (token, expected, expected_nan) in [
        ("Infinity", f64::INFINITY, false),
        ("+Infinity", f64::INFINITY, false),
        ("-Infinity", f64::NEG_INFINITY, false),
        ("1e309", f64::INFINITY, false),
        ("-1e309", f64::NEG_INFINITY, false),
        ("1e308", 1e308, false),
        ("-0", -0.0, false),
        ("-1e-999", -0.0, false),
        ("NaN", 0.0, true),
        ("inf", 0.0, true),
        ("-inf", 0.0, true),
        ("infinity", 0.0, true),
        ("invalid", 0.0, true),
    ] {
        let source = format!("journey\nsection Checkout\nPay: {token}: Customer\n");
        let parsed = SemanticParser::new()
            .parse_diagram_sync(&source)
            .expect("Journey task data accepts scalar text");
        let SemanticDiagramModel::Journey(model) = parsed.model else {
            panic!("expected the canonical Journey model");
        };
        let task = &model.tasks[0];
        assert_eq!(task.score.to_bits(), expected.to_bits(), "{token}");
        assert_eq!(task.score_is_nan, expected_nan, "{token}");
    }
}

#[cfg(feature = "compat-json")]
#[test]
fn journey_compatibility_scores_distinguish_infinity_from_invalid_number_input() {
    use merman_core::{MermaidEngine, ParseOptions};
    use serde_json::{Value, json};

    for (token, expected, expected_nan) in [
        ("Infinity", Value::Null, false),
        ("+Infinity", Value::Null, false),
        ("-Infinity", Value::Null, false),
        ("1e309", Value::Null, false),
        ("-1e309", Value::Null, false),
        ("NaN", json!(0), true),
        ("inf", json!(0), true),
        ("invalid", json!(0), true),
        ("-0", json!(0), false),
        ("3.5", json!(3.5), false),
    ] {
        let source = format!("journey\nsection Checkout\nPay: {token}: Customer\n");
        let parsed = MermaidEngine::new()
            .parse_diagram_sync(&source, ParseOptions::strict())
            .expect("Journey scalar input must project to JSON")
            .expect("Journey source must be detected");
        let task = &parsed.model["tasks"][0];
        assert_eq!(task["score"], expected, "{token}");
        assert_eq!(
            task.get("scoreIsNaN").and_then(Value::as_bool),
            expected_nan.then_some(true),
            "{token}",
        );
    }
}
