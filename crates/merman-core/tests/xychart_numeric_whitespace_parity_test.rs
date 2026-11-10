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
fn axis_and_plot_numbers_accept_ecmascript_whitespace() {
    for padding in ["\u{feff}", "\u{00a0}", "\u{2003}"] {
        for newline in ["\n", "\r\n"] {
            let source = format!(
                concat!(
                    "xychart\nx-axis {p}0{p}-->{p}1{p}\n",
                    "y-axis {p}-1{p}-->{p}+2{p}\nbar [{p}.5{p}, {p}+1{p}]\n",
                ),
                p = padding,
            )
            .replace('\n', newline);
            let model = parse_xychart(&source);
            for (axis, expected) in [(&model.x_axis, (0.0, 1.0)), (&model.y_axis, (-1.0, 2.0))] {
                let XyChartAxisModel::Linear { min, max, .. } = axis else {
                    panic!("authored numeric axes must remain linear");
                };
                assert_eq!((*min, *max), (Some(expected.0), Some(expected.1)));
            }
            assert_eq!(model.plots[0].values, [0.5, 1.0]);
            assert_eq!(model.plots[0].data[0], ("0".to_string(), Some(0.5)));
            assert_eq!(model.plots[0].data[1], ("1".to_string(), Some(1.0)));
        }
    }
}

#[test]
fn nel_is_not_discarded_from_numeric_tokens_or_axis_boundaries() {
    for source in [
        "xychart\nbar [\u{0085}1]\n",
        "xychart\nbar [1\u{0085}]\n",
        "xychart\nx-axis \u{0085}0 --> 1\nbar [1]\n",
        "xychart\ny-axis 0 --> 1\u{0085}\nbar [1]\n",
        "xychart\nbar [1]\u{0085}\n",
    ] {
        assert!(
            SemanticParser::new().parse_diagram_sync(source).is_err(),
            "U+0085 is not the Jison lexer's whitespace: {source:?}",
        );
    }
}

#[cfg(feature = "editor-facts")]
#[test]
fn numeric_whitespace_keeps_original_payload_and_error_ranges() {
    use merman_core::{
        EditorSemanticCompleteness, EditorSemanticDiagnosticKind, Error, ParseDiagnosticSpanKind,
        ParsedEditorFacts, SemanticSnapshotCapture,
    };
    use merman_model::source::SourceSpan;

    for newline in ["\n", "\r\n"] {
        let prefix = "---\ntitle: 原稿😀\n---\nxychart\n";
        let valid = format!("{prefix}bar [\u{feff}1\u{feff}]\n").replace('\n', newline);
        let parser = SemanticParser::new();
        let SemanticSnapshotCapture::Snapshot(snapshot) =
            parser.capture_diagram_snapshot_sync(&valid)
        else {
            panic!("valid XYChart must produce a snapshot");
        };
        assert!(snapshot.outcome().parsed_model().is_some());
        let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
            panic!("requested XYChart facts must be available");
        };
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
        assert!(facts.diagnostics.is_empty());
        let start = valid.find('1').unwrap();
        assert!(
            facts
                .expected_syntax
                .iter()
                .any(|expected| { expected.span == SourceSpan::new(start, start + 1) })
        );

        let invalid = format!("{prefix}bar [1]\nbar [\u{feff}1\u{0085}\u{feff}]\nbar [2]\n",)
            .replace('\n', newline);
        let start = invalid.find("1\u{0085}").unwrap();
        let expected = SourceSpan::new(start, start + "1\u{0085}".len());
        let Error::DiagramParse { diagnostic, .. } = parser
            .parse_diagram_sync(&invalid)
            .expect_err("NEL must not be stripped before number conversion")
        else {
            panic!("invalid numeric whitespace must remain a parse error");
        };
        assert_eq!(diagnostic.span(), Some(expected));
        assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);
        let SemanticSnapshotCapture::Snapshot(snapshot) =
            parser.capture_diagram_snapshot_sync(&invalid)
        else {
            panic!("invalid XYChart must retain a recovery snapshot");
        };
        let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
            panic!("recovered XYChart facts must be available");
        };
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == EditorSemanticDiagnosticKind::ParserRecovery
                && diagnostic.span == Some(expected)
        }));
        for payload in ["[1]", "[2]"] {
            let start = invalid.find(payload).unwrap();
            assert!(facts.expected_syntax.iter().any(|syntax| {
                syntax.span == SourceSpan::new(start + 1, start + payload.len() - 1)
            }));
        }
    }
}
