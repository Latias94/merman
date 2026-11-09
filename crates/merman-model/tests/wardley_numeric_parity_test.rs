#![cfg(feature = "diagram-wardley")]

use merman_model::families::wardley::{WardleyAnnotation, WardleyDiagramModel, WardleyPoint};
use serde_json::{Value, json};

#[test]
fn label_offset_presence_and_number_states_survive_canonical_and_compatibility_projection() {
    use merman_model::families::wardley::WardleyNode;
    let ordinary = json!({"id":"n", "label":"N", "x":50, "y":50});
    for value in [
        0.0,
        -0.0,
        18_446_744_073_709_551_616.0,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
    ] {
        let mut node: WardleyNode = serde_json::from_value(ordinary.clone()).unwrap();
        node.label_offset_x = Some(value);
        node.label_offset_y = Some(-value);
        let text = serde_json::to_string(&node).unwrap();
        let restored: WardleyNode = serde_json::from_str(&text).unwrap();
        for (actual, expected) in [
            (restored.label_offset_x.unwrap(), value),
            (restored.label_offset_y.unwrap(), -value),
        ] {
            assert!(actual.to_bits() == expected.to_bits() || actual.is_nan() && expected.is_nan());
        }
        #[cfg(feature = "compat-json")]
        {
            use merman_model::families::wardley::compat;
            use merman_model::presentation::compat::CompatJsonMetadata;
            let config = merman_model::MermaidConfig::empty_object();
            let meta = CompatJsonMetadata {
                diagram_type: "wardley",
                config: &config,
                authored_config: &config,
                title: None,
            };
            let projection = compat::project(
                &WardleyDiagramModel {
                    nodes: vec![node],
                    ..Default::default()
                },
                &meta,
            )
            .unwrap();
            if value.is_finite() {
                assert_eq!(projection["nodes"][0]["labelOffsetX"].as_f64(), Some(value));
            } else {
                assert!(projection["nodes"][0]["labelOffsetX"].is_null());
                assert!(projection["nodes"][0]["labelOffsetY"].is_null());
            }
        }
    }
    for field in ["labelOffsetX", "labelOffsetY"] {
        let mut wire = ordinary.clone();
        wire[field] = Value::Null;
        let node: WardleyNode = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!((node.label_offset_x, node.label_offset_y), (None, None));
        assert!(serde_json::to_value(node).unwrap().get(field).is_none());
        wire[field] = json!("18446744073709551616");
        assert!(serde_json::from_value::<WardleyNode>(wire).is_err());
    }
}

fn model_with_annotation(number: f64) -> WardleyDiagramModel {
    WardleyDiagramModel {
        annotations: vec![WardleyAnnotation {
            number,
            coordinates: vec![WardleyPoint { x: 25.0, y: 50.0 }],
            text: Some("x".into()),
        }],
        ..Default::default()
    }
}

#[test]
fn finite_annotation_numbers_round_trip_without_integer_saturation() {
    for number in [
        0.0,
        1.0,
        9_007_199_254_740_992.0,
        1_000_000_000_000_000_128.0,
        18_446_744_073_709_551_616.0,
        18_446_744_073_709_555_712.0,
        1.0e21,
        f64::MAX,
    ] {
        let model = model_with_annotation(number);
        let value = serde_json::to_value(&model).expect("canonical Wardley JSON");
        assert!(value["annotations"][0]["number"].is_number());
        let restored: WardleyDiagramModel =
            serde_json::from_value(value).expect("canonical Wardley value round trip");
        assert_eq!(restored, model);
        assert_eq!(restored.annotations[0].number.to_bits(), number.to_bits());

        let json = serde_json::to_string(&model).expect("canonical Wardley JSON text");
        let restored: WardleyDiagramModel =
            serde_json::from_str(&json).expect("canonical Wardley text round trip");
        assert_eq!(restored, model);
        assert_eq!(restored.annotations[0].number.to_bits(), number.to_bits());
    }

    let restored: WardleyAnnotation =
        serde_json::from_str(r#"{"number":9007199254740993,"coordinates":[]}"#)
            .expect("canonical numeric imports use binary64 too");
    assert_eq!(restored.number, 9_007_199_254_740_992.0);
}

#[test]
fn canonical_nonfinite_numbers_survive_value_and_text_round_trips() {
    for (number, encoded) in [
        (f64::INFINITY, "Infinity"),
        (f64::NEG_INFINITY, "-Infinity"),
        (f64::NAN, "NaN"),
    ] {
        let model = model_with_annotation(number);
        let value = serde_json::to_value(&model).expect("canonical Wardley nonfinite JSON");
        assert_eq!(value["annotations"][0]["number"], encoded);
        let json = serde_json::to_string(&model).expect("canonical Wardley JSON text");
        for restored in [
            serde_json::from_value::<WardleyDiagramModel>(value).unwrap(),
            serde_json::from_str::<WardleyDiagramModel>(&json).unwrap(),
        ] {
            assert_eq!(restored.annotations.len(), 1);
            let annotation = &restored.annotations[0];
            if number.is_nan() {
                assert!(annotation.number.is_nan());
            } else {
                assert_eq!(annotation.number, number);
            }
            assert_eq!(annotation.coordinates, model.annotations[0].coordinates);
            assert_eq!(annotation.text.as_deref(), Some("x"));
        }
    }
}

#[test]
fn canonical_imports_reject_null_and_unknown_number_strings() {
    for number in [
        Value::Null,
        json!("1"),
        json!("+Infinity"),
        json!(" Infinity"),
        json!("inf"),
        json!(true),
    ] {
        assert!(
            serde_json::from_value::<WardleyAnnotation>(json!({"number": number})).is_err(),
            "canonical imports must not replace an invalid number with a default",
        );
    }
    assert!(serde_json::from_value::<WardleyAnnotation>(json!({})).is_err());
}

#[cfg(feature = "compat-json")]
#[test]
fn compatibility_json_projects_numbers_instead_of_canonical_nonfinite_tags() {
    use merman_model::MermaidConfig;
    use merman_model::families::wardley::compat;
    use merman_model::presentation::compat::CompatJsonMetadata;

    let config = MermaidConfig::empty_object();
    let metadata = CompatJsonMetadata {
        diagram_type: "wardley",
        config: &config,
        authored_config: &config,
        title: None,
    };
    for number in [
        1.0,
        9_007_199_254_740_992.0,
        18_446_744_073_709_551_616.0,
        18_446_744_073_709_555_712.0,
        f64::MAX,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
    ] {
        let mut model = model_with_annotation(number);
        model.annotations.push(WardleyAnnotation {
            number,
            coordinates: Vec::new(),
            text: None,
        });
        let projection = compat::project(&model, &metadata).expect("Wardley compatibility JSON");
        for annotation in projection["annotations"].as_array().unwrap() {
            let value = annotation.get("number").expect("number remains present");
            if number.is_finite() {
                assert_eq!(value.as_f64().unwrap().to_bits(), number.to_bits());
            } else {
                assert!(
                    value.is_null(),
                    "JSON.stringify maps nonfinite values to null",
                );
            }
        }
        assert_eq!(projection["annotations"][0]["text"], "x");
        assert!(projection["annotations"][1].get("text").is_none());
        assert_eq!(projection["annotations"][1]["coordinates"], json!([]));
    }
}
