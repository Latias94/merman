#![cfg(any(feature = "diagram-pie", feature = "diagram-xychart"))]

use merman_core::SemanticParser;
use merman_model::semantic::SemanticDiagramModel;

fn positive_number_text_cases() -> Vec<(String, &'static str)> {
    vec![
        ("0.000001".into(), "0.000001"),
        ("0.0000001".into(), "1e-7"),
        (
            "0.0000000298023223876953125".into(),
            "2.9802322387695312e-8",
        ),
        ("0.0000009999999999999997".into(), "9.999999999999997e-7"),
        ("999999999999999900000".into(), "999999999999999900000"),
        ("1000000000000000000000".into(), "1e+21"),
        ("999999999999999999999".into(), "1e+21"),
        ("9007199254740993".into(), "9007199254740992"),
        ("1000000000000000128".into(), "1000000000000000100"),
        (format!("0.{}5", "0".repeat(323)), "5e-324"),
        ("9".repeat(309), "Infinity"),
    ]
}

#[cfg(feature = "diagram-xychart")]
mod xychart {
    use super::*;
    use merman_model::families::xy_chart::XyChartDiagramModel;

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
    fn single_point_labels_use_javascript_number_text() {
        // Mermaid 12.3.0 xychartDb.ts: the single-value branch interpolates `${min}`.
        let cases = positive_number_text_cases().into_iter().chain([
            ("-0".to_string(), "0"),
            (format!("-0.{}1", "0".repeat(330)), "0"),
            (format!("-{}", "9".repeat(309)), "-Infinity"),
        ]);
        for (token, expected) in cases {
            let source = format!("xychart\nx-axis {token} --> {token}\nbar [7]\n");
            let model = parse_xychart(&source);
            assert_eq!(
                model.plots[0].data,
                [(expected.to_string(), Some(7.0))],
                "generated x label for {token:?}",
            );
        }
    }

    #[test]
    fn interpolated_labels_keep_exponents_and_nonfinite_number_text() {
        // Decimal tokens can overflow before `${min + index * step}` is evaluated.
        for (max, expected) in [
            ("0.0000001".to_string(), ["0", "1e-7"]),
            ("1000000000000000000000".to_string(), ["0", "1e+21"]),
            ("9".repeat(309), ["NaN", "Infinity"]),
            (format!("-{}", "9".repeat(309)), ["NaN", "-Infinity"]),
        ] {
            let source = format!("xychart\nx-axis 0 --> {max}\nline [1, 2]\n");
            let model = parse_xychart(&source);
            let labels: Vec<_> = model.plots[0]
                .data
                .iter()
                .map(|(label, _)| label.as_str())
                .collect();
            assert_eq!(labels, expected, "generated labels for 0 --> {max}");
        }
    }

    #[test]
    fn interpolated_labels_keep_the_shortest_unrounded_coordinates() {
        let model = parse_xychart("xychart\nx-axis 0 --> 1\nline [1, 2, 3, 4]\n");
        let labels: Vec<_> = model.plots[0]
            .data
            .iter()
            .map(|(label, _)| label.as_str())
            .collect();
        assert_eq!(
            labels,
            ["0", "0.3333333333333333", "0.6666666666666666", "1"],
        );
    }
}

#[cfg(feature = "diagram-pie")]
mod pie {
    use super::*;
    use merman_core::{Error, ParseDiagnosticSpanKind};
    use merman_model::SourceSpan;

    #[test]
    fn negative_value_errors_use_javascript_number_text_and_original_token_spans() {
        // Mermaid 12.3.0 pieDb.ts interpolates the converted value, not its source token.
        for (positive_token, positive_text) in positive_number_text_cases() {
            let token = format!("-{positive_token}");
            let source = format!("pie\n\"slice\": {token}\n");
            let Error::DiagramParse { diagnostic, .. } = SemanticParser::new()
                .parse_diagram_sync(&source)
                .expect_err("negative pie values must fail validation")
            else {
                panic!("expected a pie validation diagnostic");
            };
            assert_eq!(
                diagnostic.message(),
                format!(
                    "\"slice\" has invalid value: -{positive_text}. Negative values are not allowed in pie charts. All slice values must be >= 0.",
                ),
                "negative value diagnostic for {token:?}",
            );
            let start = source.find(&token).unwrap();
            assert_eq!(
                diagnostic.span(),
                Some(SourceSpan::new(start, start + token.len())),
            );
            assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);
        }
    }

    #[test]
    fn negative_zero_values_remain_valid_slice_values() {
        for token in ["-0".to_string(), format!("-0.{}1", "0".repeat(330))] {
            let source = format!("pie\n\"zero\": {token}\n");
            let parsed = SemanticParser::new()
                .parse_diagram_sync(&source)
                .expect("negative zero is not less than zero");
            let SemanticDiagramModel::Pie(model) = parsed.model else {
                panic!("expected the canonical Pie model");
            };
            assert_eq!(model.sections[0].value, 0.0);
            assert!(model.sections[0].value.is_sign_negative());
        }
    }

    #[cfg(feature = "editor-facts")]
    #[test]
    fn recovered_editor_diagnostics_keep_javascript_number_text() {
        use merman_core::{ParsedEditorFacts, SemanticSnapshotCapture};

        let source = "pie\n\"negative\": -0.0000001\n\"valid\": 2\n";
        let SemanticSnapshotCapture::Snapshot(snapshot) =
            SemanticParser::new().capture_diagram_snapshot_sync(source)
        else {
            panic!("invalid pie must retain a recovery snapshot");
        };
        let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
            panic!("requested pie facts must be available");
        };
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.message
                == "\"negative\" has invalid value: -1e-7. Negative values are not allowed in pie charts. All slice values must be >= 0."
        }));
    }
}
