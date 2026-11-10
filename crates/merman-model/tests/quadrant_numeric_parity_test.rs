#![cfg(feature = "diagram-quadrant-chart")]

use merman_model::families::quadrant_chart::{
    QuadrantChartAxesModel, QuadrantChartDiagramModel, QuadrantChartPointModel,
    QuadrantChartQuadrantsModel, QuadrantChartStyles,
};
use serde_json::{Value, json};

fn model_with_number(value: f64) -> QuadrantChartDiagramModel {
    let styles = QuadrantChartStyles {
        radius: Some(value),
        ..QuadrantChartStyles::default()
    };
    QuadrantChartDiagramModel {
        title: None,
        acc_title: None,
        acc_descr: None,
        quadrants: QuadrantChartQuadrantsModel {
            quadrant1_text: String::new(),
            quadrant2_text: String::new(),
            quadrant3_text: String::new(),
            quadrant4_text: String::new(),
        },
        axes: QuadrantChartAxesModel {
            x_axis_left_text: String::new(),
            x_axis_right_text: String::new(),
            y_axis_bottom_text: String::new(),
            y_axis_top_text: String::new(),
        },
        points: vec![QuadrantChartPointModel {
            text: "P".to_string(),
            x: value,
            y: value,
            class_name: Some("styled".to_string()),
            styles: styles.clone(),
        }],
        classes: [("styled".to_string(), styles)].into(),
    }
}

#[test]
fn quadrant_canonical_numbers_survive_value_and_text_round_trips() {
    for value in [
        0.0,
        -0.0,
        1.0,
        18_446_744_073_709_551_616.0,
        f64::MAX,
        f64::from_bits(1),
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
    ] {
        let model = model_with_number(value);
        let wire = serde_json::to_value(&model).unwrap();
        for actual in [
            &wire["points"][0]["x"],
            &wire["points"][0]["y"],
            &wire["points"][0]["styles"]["radius"],
            &wire["classes"]["styled"]["radius"],
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
            serde_json::from_value::<QuadrantChartDiagramModel>(wire).unwrap(),
            serde_json::from_str::<QuadrantChartDiagramModel>(&text).unwrap(),
        ] {
            for actual in [
                restored.points[0].x,
                restored.points[0].y,
                restored.points[0].styles.radius.unwrap(),
                restored.classes["styled"].radius.unwrap(),
            ] {
                if value.is_nan() {
                    assert!(actual.is_nan());
                } else {
                    assert_eq!(actual.to_bits(), value.to_bits());
                }
            }
        }
    }
}

#[test]
fn quadrant_optional_radius_retains_absence() {
    for wire in [json!({}), json!({"radius": null})] {
        let text = wire.to_string();
        for styles in [
            serde_json::from_value::<QuadrantChartStyles>(wire).unwrap(),
            serde_json::from_str::<QuadrantChartStyles>(&text).unwrap(),
        ] {
            assert_eq!(styles.radius, None);
            assert!(serde_json::to_value(styles).unwrap()["radius"].is_null());
        }
    }
}

#[test]
fn quadrant_canonical_imports_reject_erased_and_unknown_required_numbers() {
    for invalid in [
        Value::Null,
        json!(true),
        json!("1"),
        json!("+Infinity"),
        json!(" Infinity"),
        json!("inf"),
    ] {
        for field in ["x", "y"] {
            let mut wire = json!({"text": "P", "x": 0, "y": 1, "styles": {}});
            wire[field] = invalid.clone();
            assert!(
                serde_json::from_str::<QuadrantChartPointModel>(&wire.to_string()).is_err(),
                "{field}"
            );
            assert!(serde_json::from_value::<QuadrantChartPointModel>(wire).is_err());
        }
        if !invalid.is_null() {
            let wire = json!({"radius": invalid});
            assert!(serde_json::from_str::<QuadrantChartStyles>(&wire.to_string()).is_err());
            assert!(serde_json::from_value::<QuadrantChartStyles>(wire).is_err());
        }
    }
}

#[cfg(feature = "compat-json")]
#[test]
fn quadrant_compatibility_json_projects_nonfinite_numbers_to_null() {
    use merman_model::MermaidConfig;
    use merman_model::families::quadrant_chart::compat;
    use merman_model::presentation::compat::CompatJsonMetadata;

    let config = MermaidConfig::empty_object();
    let metadata = CompatJsonMetadata {
        diagram_type: "quadrantChart",
        config: &config,
        authored_config: &config,
        title: None,
    };
    for value in [1.0, f64::MAX, f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        let projected = compat::project(&model_with_number(value), &metadata).unwrap();
        for actual in [
            &projected["points"][0]["x"],
            &projected["points"][0]["y"],
            &projected["points"][0]["styles"]["radius"],
            &projected["classes"]["styled"]["radius"],
        ] {
            if value.is_finite() {
                assert_eq!(actual.as_f64().unwrap().to_bits(), value.to_bits());
            } else {
                assert!(actual.is_null());
            }
        }
        assert_eq!(projected["points"][0]["text"], "P");
        assert_eq!(projected["points"][0]["className"], "styled");
    }
}
