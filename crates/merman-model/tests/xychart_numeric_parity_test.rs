#![cfg(feature = "diagram-xychart")]

use merman_model::families::xy_chart::{
    XyChartAxisModel, XyChartDiagramModel, XyChartPlotModel, XyChartPlotType,
};
use serde_json::{Value, json};

fn model_with_number(value: f64) -> XyChartDiagramModel {
    XyChartDiagramModel {
        orientation: "vertical".to_string(),
        title: None,
        acc_title: None,
        acc_descr: None,
        x_axis: XyChartAxisModel::Linear {
            title: String::new(),
            min: Some(value),
            max: Some(value),
        },
        y_axis: XyChartAxisModel::Linear {
            title: String::new(),
            min: Some(value),
            max: Some(value),
        },
        plots: vec![XyChartPlotModel {
            plot_type: XyChartPlotType::Bar,
            title: Some("Series".to_string()),
            values: vec![value],
            data: vec![
                ("A".to_string(), Some(value)),
                ("missing".to_string(), None),
            ],
            point_labels: vec!["A".to_string()],
        }],
    }
}

#[test]
fn xychart_canonical_numbers_survive_value_and_text_round_trips() {
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
            &wire["xAxis"]["min"],
            &wire["xAxis"]["max"],
            &wire["yAxis"]["min"],
            &wire["yAxis"]["max"],
            &wire["plots"][0]["values"][0],
            &wire["plots"][0]["data"][0][1],
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
        assert_eq!(wire["plots"][0]["data"][0][0], "A");
        assert_eq!(wire["plots"][0]["data"][1], json!(["missing", null]));
        let text = serde_json::to_string(&model).unwrap();
        for restored in [
            serde_json::from_value::<XyChartDiagramModel>(wire).unwrap(),
            serde_json::from_str::<XyChartDiagramModel>(&text).unwrap(),
        ] {
            for axis in [&restored.x_axis, &restored.y_axis] {
                let XyChartAxisModel::Linear { min, max, .. } = axis else {
                    panic!("linear axes must retain their type");
                };
                for actual in [min.unwrap(), max.unwrap()] {
                    if value.is_nan() {
                        assert!(actual.is_nan());
                    } else {
                        assert_eq!(actual.to_bits(), value.to_bits());
                    }
                }
            }
            let plot = &restored.plots[0];
            for actual in [plot.values[0], plot.data[0].1.unwrap()] {
                if value.is_nan() {
                    assert!(actual.is_nan());
                } else {
                    assert_eq!(actual.to_bits(), value.to_bits());
                }
            }
            assert_eq!(plot.data[0].0, "A");
            assert_eq!(plot.data[1], ("missing".to_string(), None));
            assert_eq!(plot.point_labels, ["A"]);
        }
    }
    let whole = serde_json::to_value(model_with_number(1.0)).unwrap();
    assert!(whole["plots"][0]["values"][0].is_i64());
    assert!(whole["plots"][0]["data"][0][1].is_i64());
}

#[test]
fn xychart_optional_bounds_retain_absence() {
    for wire in [
        json!({"type": "linear"}),
        json!({"type": "linear", "min": null, "max": null}),
    ] {
        let text = wire.to_string();
        for axis in [
            serde_json::from_value::<XyChartAxisModel>(wire).unwrap(),
            serde_json::from_str::<XyChartAxisModel>(&text).unwrap(),
        ] {
            let XyChartAxisModel::Linear { min, max, .. } = &axis else {
                panic!("linear axes must retain their type");
            };
            assert_eq!((*min, *max), (None, None));
            let wire = serde_json::to_value(axis).unwrap();
            assert!(wire["min"].is_null());
            assert!(wire["max"].is_null());
        }
    }
}

#[test]
fn xychart_canonical_imports_reject_erased_values_and_unknown_number_strings() {
    for invalid in [
        Value::Null,
        json!(true),
        json!("1"),
        json!("+Infinity"),
        json!(" Infinity"),
        json!("inf"),
    ] {
        let mut wire = serde_json::to_value(&model_with_number(1.0).plots[0]).unwrap();
        wire["values"][0] = invalid.clone();
        assert!(serde_json::from_str::<XyChartPlotModel>(&wire.to_string()).is_err());
        assert!(serde_json::from_value::<XyChartPlotModel>(wire).is_err());

        if !invalid.is_null() {
            let mut wire = serde_json::to_value(&model_with_number(1.0).plots[0]).unwrap();
            wire["data"][0][1] = invalid.clone();
            assert!(serde_json::from_str::<XyChartPlotModel>(&wire.to_string()).is_err());
            assert!(serde_json::from_value::<XyChartPlotModel>(wire).is_err());
            for field in ["min", "max"] {
                let mut wire = json!({"type": "linear"});
                wire[field] = invalid.clone();
                assert!(serde_json::from_str::<XyChartAxisModel>(&wire.to_string()).is_err());
                assert!(serde_json::from_value::<XyChartAxisModel>(wire).is_err());
            }
        }
    }
}

#[cfg(feature = "compat-json")]
#[test]
fn xychart_compatibility_json_projects_nonfinite_numbers_to_null() {
    use merman_model::MermaidConfig;
    use merman_model::families::xy_chart::compat;
    use merman_model::presentation::compat::CompatJsonMetadata;

    let config = MermaidConfig::empty_object();
    let metadata = CompatJsonMetadata {
        diagram_type: "xychart",
        config: &config,
        authored_config: &config,
        title: None,
    };
    for value in [1.0, f64::MAX, f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        let model = model_with_number(value).into_render_model(&config);
        let projected = compat::project(&model, &metadata).unwrap();
        for actual in [
            &projected["xAxis"]["min"],
            &projected["xAxis"]["max"],
            &projected["yAxis"]["min"],
            &projected["yAxis"]["max"],
            &projected["plots"][0]["values"][0],
            &projected["plots"][0]["data"][0][1],
        ] {
            if value.is_finite() {
                assert_eq!(actual.as_f64().unwrap().to_bits(), value.to_bits());
            } else {
                assert!(actual.is_null());
            }
        }
        assert_eq!(projected["plots"][0]["data"][0][0], "A");
        assert_eq!(projected["plots"][0]["data"][1], json!(["missing", null]));
    }
}
