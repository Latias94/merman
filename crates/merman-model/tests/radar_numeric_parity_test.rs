#![cfg(feature = "diagram-radar")]

use merman_model::families::radar::{RadarCurve, RadarDiagramModel, RadarOptions};
use serde_json::{Value, json};

fn model_with_number(value: f64) -> RadarDiagramModel {
    RadarDiagramModel {
        curves: vec![RadarCurve {
            name: "c".to_string(),
            label: "Curve".to_string(),
            entries: vec![value],
        }],
        options: RadarOptions {
            ticks: value,
            min: value,
            max: Some(value),
            ..RadarOptions::default()
        },
        ..RadarDiagramModel::default()
    }
}

#[test]
fn radar_canonical_numbers_survive_value_and_text_round_trips() {
    for value in [
        0.0,
        -0.0,
        1.0,
        9_007_199_254_740_992.0,
        2_f64.powi(63),
        2_f64.powi(64),
        1e21,
        f64::MAX,
        f64::from_bits(1),
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
    ] {
        let model = model_with_number(value);
        let wire = serde_json::to_value(&model).unwrap();
        for actual in [
            &wire["options"]["ticks"],
            &wire["options"]["max"],
            &wire["options"]["min"],
            &wire["curves"][0]["entries"][0],
        ] {
            if value.is_nan() {
                assert_eq!(actual, "NaN");
            } else if value == f64::INFINITY {
                assert_eq!(actual, "Infinity");
            } else if value == f64::NEG_INFINITY {
                assert_eq!(actual, "-Infinity");
            } else {
                assert_eq!(actual.as_f64().unwrap().to_bits(), value.to_bits());
            }
        }
        let text = serde_json::to_string(&model).unwrap();
        for restored in [
            serde_json::from_value::<RadarDiagramModel>(wire).unwrap(),
            serde_json::from_str::<RadarDiagramModel>(&text).unwrap(),
        ] {
            for actual in [
                restored.options.ticks,
                restored.options.max.unwrap(),
                restored.options.min,
                restored.curves[0].entries[0],
            ] {
                if value.is_nan() {
                    assert!(actual.is_nan());
                } else {
                    assert_eq!(actual.to_bits(), value.to_bits());
                }
            }
        }
    }
    let whole = serde_json::to_value(model_with_number(1.0)).unwrap();
    assert!(whole["options"]["ticks"].is_i64());
    assert!(whole["curves"][0]["entries"][0].is_i64());
}

#[test]
fn radar_optional_max_distinguishes_absence_from_nonfinite_state() {
    let value = serde_json::to_value(RadarOptions::default()).unwrap();
    assert!(value["max"].is_null());
    let restored: RadarOptions = serde_json::from_value(value).unwrap();
    assert_eq!(restored.max, None);
    let restored: RadarOptions = serde_json::from_value(json!({
        "showLegend": true, "ticks": 5, "min": 0, "graticule": "circle",
    }))
    .unwrap();
    assert_eq!(restored.max, None);
}

#[test]
fn radar_canonical_imports_reject_erased_and_unknown_required_numbers() {
    for value in [
        Value::Null,
        json!(true),
        json!("1"),
        json!("+Infinity"),
        json!(" Infinity"),
        json!("inf"),
    ] {
        assert!(
            serde_json::from_value::<RadarCurve>(json!({
                "name": "c", "label": "Curve", "entries": [value.clone()],
            }))
            .is_err()
        );
        for field in ["ticks", "min"] {
            let mut options = serde_json::to_value(RadarOptions::default()).unwrap();
            options[field] = value.clone();
            assert!(
                serde_json::from_value::<RadarOptions>(options).is_err(),
                "{field}"
            );
        }
        if !value.is_null() {
            let mut options = serde_json::to_value(RadarOptions::default()).unwrap();
            options["max"] = value;
            assert!(serde_json::from_value::<RadarOptions>(options).is_err());
        }
    }
}

#[cfg(feature = "compat-json")]
#[test]
fn radar_compatibility_json_projects_nonfinite_numbers_to_null() {
    use merman_model::MermaidConfig;
    use merman_model::families::radar::{RadarOutputData, compat};

    for value in [1.0, f64::MAX, f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        let projected = compat::project(
            &model_with_number(value),
            RadarOutputData::Model,
            "radar",
            &MermaidConfig::empty_object(),
        );
        for actual in [
            &projected["options"]["ticks"],
            &projected["options"]["max"],
            &projected["options"]["min"],
            &projected["curves"][0]["entries"][0],
        ] {
            if value.is_finite() {
                assert_eq!(actual.as_f64().unwrap().to_bits(), value.to_bits());
            } else {
                assert!(actual.is_null());
            }
        }
    }
}
