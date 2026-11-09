#![cfg(all(
    feature = "diagram-pie",
    feature = "diagram-sankey",
    feature = "diagram-xychart"
))]

use merman_core::SemanticParser;
use merman_model::semantic::SemanticDiagramModel;

#[test]
fn numeric_token_grammars_remain_distinct_from_javascript_conversion() {
    // Mermaid 12.3.0: Sankey CSV + parseFloat; Pie NUMBER_PIE; XY NUMBER_WITH_DECIMAL.
    for (token, pie, xy) in [
        ("0x10", false, false),
        ("1e3", false, false),
        (".5", false, true),
        ("+1", false, true),
        ("01", false, true),
        ("01.2", true, true),
        ("1.", false, false),
        ("NaN", false, false),
        ("Infinity", false, false),
    ] {
        for (source, accepted) in [
            (format!("sankey\nA,B,{token}\n"), true),
            (format!("pie\n\"A\": {token}\n"), pie),
            (format!("xychart\nbar [{token}]\n"), xy),
        ] {
            assert_eq!(
                SemanticParser::new().parse_diagram_sync(&source).is_ok(),
                accepted,
                "conversion must not widen a family's lexical grammar: {source:?}",
            );
        }
    }
}

#[test]
fn admitted_decimal_values_round_and_overflow_before_family_validation() {
    for (token, expected) in [
        ("9007199254740993".to_string(), 9_007_199_254_740_992.0),
        ("9".repeat(309), f64::INFINITY),
    ] {
        for source in [
            format!("pie\n\"A\": {token}\n"),
            format!("xychart\nbar [{token}]\n"),
            format!("sankey\nA,B,{token}\n"),
        ] {
            let parsed = SemanticParser::new().parse_diagram_sync(&source).unwrap();
            match parsed.model {
                SemanticDiagramModel::Pie(model) => {
                    assert_eq!(model.sections[0].value, expected);
                }
                SemanticDiagramModel::XyChart(model) => {
                    assert_eq!(model.plots[0].values, [expected]);
                    assert_eq!(model.plots[0].data[0].1, Some(expected));
                }
                SemanticDiagramModel::Sankey(model) => {
                    let value = &model.graph.links[0].value;
                    if expected.is_infinite() {
                        assert!(value.is_null(), "JSON.stringify maps Infinity to null");
                    } else {
                        assert_eq!(value.as_f64(), Some(expected));
                    }
                }
                _ => panic!("expected a numeric diagram family"),
            }
        }
    }
}

#[test]
fn pie_and_xy_values_keep_negative_zero_and_decimal_underflow() {
    for token in ["-0".to_string(), format!("-0.{}1", "0".repeat(330))] {
        for source in [
            format!("pie\n\"A\": {token}\n"),
            format!("xychart\nbar [{token}]\n"),
        ] {
            let parsed = SemanticParser::new().parse_diagram_sync(&source).unwrap();
            let value = match parsed.model {
                SemanticDiagramModel::Pie(model) => model.sections[0].value,
                SemanticDiagramModel::XyChart(model) => model.plots[0].values[0],
                _ => panic!("expected Pie or XYChart"),
            };
            assert_eq!(value, 0.0);
            assert!(value.is_sign_negative(), "{source:?}");
        }
    }
}
