#![cfg(feature = "diagram-xychart")]

use merman_core::SemanticParser;
use merman_model::{
    families::xy_chart::{XyChartAxisModel, XyChartDiagramModel},
    semantic::SemanticDiagramModel,
};

fn parse_xychart(source: &str) -> XyChartDiagramModel {
    let parsed = SemanticParser::new()
        .parse_diagram_sync(source)
        .expect("valid XYChart source");
    let SemanticDiagramModel::XyChart(model) = parsed.model else {
        panic!("expected the canonical XYChart model");
    };
    model
}

#[test]
fn authored_axis_endpoints_retain_nonfinite_number_values() {
    let overflow = format!("2{}", "0".repeat(308));
    for (min, max, expected) in [
        ("0".to_string(), overflow.clone(), (0.0, f64::INFINITY)),
        (
            format!("-{overflow}"),
            "1".to_string(),
            (f64::NEG_INFINITY, 1.0),
        ),
        (overflow.clone(), overflow, (f64::INFINITY, f64::INFINITY)),
    ] {
        let source = format!("xychart\nx-axis {min} --> {max}\ny-axis {min} --> {max}\nbar [1]\n");
        let model = parse_xychart(&source);
        for axis in [&model.x_axis, &model.y_axis] {
            let XyChartAxisModel::Linear { min, max, .. } = axis else {
                panic!("authored numeric axes must remain linear");
            };
            assert_eq!((*min, *max), (Some(expected.0), Some(expected.1)));
        }

        #[cfg(feature = "compat-json")]
        {
            let projected = merman_core::MermaidEngine::new()
                .parse_diagram_sync(&source, merman_core::ParseOptions::strict())
                .expect("XYChart compatibility parse")
                .expect("XYChart diagram")
                .model;
            for axis in ["xAxis", "yAxis"] {
                for (key, value) in [("min", expected.0), ("max", expected.1)] {
                    if value.is_finite() {
                        assert_eq!(projected[axis][key].as_f64(), Some(value));
                    } else {
                        assert!(projected[axis][key].is_null());
                    }
                }
            }
        }
    }
}

#[test]
fn plot_derived_axis_endpoints_retain_overflow_across_series() {
    let overflow = format!("2{}", "0".repeat(308));
    let source = format!("xychart\nbar [0, {overflow}]\nline [-{overflow}, 1]\n");
    let model = parse_xychart(&source);
    let XyChartAxisModel::Linear { min, max, .. } = model.y_axis else {
        panic!("the plot-derived Y axis must remain linear");
    };
    assert_eq!(min, Some(f64::NEG_INFINITY));
    assert_eq!(max, Some(f64::INFINITY));
}

#[test]
fn an_unset_axis_remains_distinct_from_an_overflowing_range() {
    let model = parse_xychart("xychart\n");
    let XyChartAxisModel::Linear { min, max, .. } = model.y_axis else {
        panic!("the default Y axis must remain linear");
    };
    assert_eq!((min, max), (None, None));
}
