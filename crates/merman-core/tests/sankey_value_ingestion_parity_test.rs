#![cfg(feature = "diagram-sankey")]

use merman_core::SemanticParser;
use merman_model::{families::sankey::SankeyDiagramModel, semantic::SemanticDiagramModel};

fn parse_sankey(source: &str) -> SankeyDiagramModel {
    let parsed = SemanticParser::new()
        .parse_diagram_sync(source)
        .expect("valid Sankey source");
    let SemanticDiagramModel::Sankey(model) = parsed.model else {
        panic!("expected the canonical Sankey model");
    };
    model
}

#[test]
fn csv_values_keep_parse_float_prefix_rounding_and_nonfinite_semantics() {
    // sankey.jison converts CSV text with parseFloat(), not Number().
    for (token, expected) in [
        ("12.5units", Some(12.5)),
        ("1e+tail", Some(1.0)),
        ("1e-2tail", Some(0.01)),
        ("0x10", Some(0.0)),
        (".5", Some(0.5)),
        ("1.", Some(1.0)),
        ("9007199254740993", Some(9_007_199_254_740_992.0)),
        ("1e309", None),
        ("-Infinity", None),
        ("NaN", None),
        ("invalid", None),
    ] {
        for newline in ["\n", "\r\n"] {
            let source = format!("sankey{newline}A,B,{token}{newline}");
            let model = parse_sankey(&source);
            let value = &model.graph.links[0].value;
            assert_eq!(value.as_f64(), expected, "{source:?}");
            assert_eq!(value.is_null(), expected.is_none(), "{source:?}");
        }
    }
}

#[test]
fn csv_empty_fields_are_syntax_errors_but_space_content_remains_a_field() {
    for field in 0..3 {
        for empty in ["", "\"\""] {
            let mut fields = ["A", "B", "1"];
            fields[field] = empty;
            let source = format!("sankey\n{}\nC,D,2\n", fields.join(","));
            assert!(
                SemanticParser::new().parse_diagram_sync(&source).is_err(),
                "empty fields must not become zero or NaN: {source:?}",
            );
        }
    }

    for field in ["   ", "\" \""] {
        let model = parse_sankey(&format!("sankey\nA,B,{field}\nC,D,2\n"));
        assert!(model.graph.links[0].value.is_null());
        assert_eq!(model.graph.links[1].value.as_f64(), Some(2.0));
    }
    let model = parse_sankey("sankey\n ,B,1\n");
    assert_eq!(model.graph.links[0].source, "");
    for padding in ["\t", "\u{feff}", "\u{00a0}"] {
        let model = parse_sankey(&format!("sankey\nA,B,1{padding}\n"));
        assert_eq!(model.graph.links[0].value.as_f64(), Some(1.0));
    }
}

#[test]
fn csv_textdata_rejects_non_ascii_and_control_characters_before_conversion() {
    // RFC2234 TEXTDATA in the pinned Sankey lexer excludes tabs and Unicode.
    for token in ["\t1", "1\t", "\u{0085}1", "\u{feff}1", "1中"] {
        for field in 0..3 {
            for quoted in [false, true] {
                let token = if quoted {
                    format!("\"{token}\"")
                } else {
                    token.to_string()
                };
                let mut fields = ["A", "B", "1"];
                fields[field] = &token;
                let source = format!("sankey\n{}\nC,D,2\n", fields.join(","));
                assert!(
                    SemanticParser::new().parse_diagram_sync(&source).is_err(),
                    "invalid CSV characters must not reach parseFloat: {source:?}",
                );
            }
        }
    }
    assert!(
        SemanticParser::new()
            .parse_diagram_sync("sankey\nA,B,1\u{0085}\n")
            .is_err()
    );
}

#[cfg(feature = "editor-facts")]
#[test]
fn invalid_csv_value_keeps_original_diagnostic_and_surrounding_record_ranges() {
    use merman_core::{
        EditorSemanticCompleteness, EditorSemanticDiagnosticKind, Error, ParseDiagnosticSpanKind,
        ParsedEditorFacts, SemanticParseOutcome, SemanticSnapshotCapture,
    };
    use merman_model::source::SourceSpan;

    for newline in ["\n", "\r\n"] {
        let source = concat!(
            "---\ntitle: 原稿😀\n---\nsankey\n",
            "Before,Node,1\nBad,Value,\"1\t\"\nAfter,Node,2\n",
        )
        .replace('\n', newline);
        let start = source.find('\t').expect("the invalid value contains a tab");
        let expected = SourceSpan::new(start, start + 1);
        let parser = SemanticParser::new();
        let Error::DiagramParse { diagnostic, .. } = parser
            .parse_diagram_sync(&source)
            .expect_err("tabs are not CSV TEXTDATA")
        else {
            panic!("invalid CSV content must remain a parse error");
        };
        assert_eq!(diagnostic.span(), Some(expected));
        assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);

        let SemanticSnapshotCapture::Snapshot(snapshot) =
            parser.capture_diagram_snapshot_sync(&source)
        else {
            panic!("invalid CSV must retain a recovery snapshot");
        };
        assert!(matches!(
            snapshot.outcome(),
            SemanticParseOutcome::Failed(_)
        ));
        let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
            panic!("requested Sankey editor facts must be available");
        };
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(facts.diagnostics.iter().any(|diagnostic| {
            diagnostic.kind == EditorSemanticDiagnosticKind::ParserRecovery
                && diagnostic.span == Some(expected)
        }));
        for name in ["Before", "After"] {
            let symbol = facts
                .symbols
                .iter()
                .find(|symbol| symbol.name == name)
                .unwrap();
            let start = source.find(name).unwrap();
            assert_eq!(symbol.selection, SourceSpan::new(start, start + name.len()));
            assert_eq!(&source[symbol.selection.start..symbol.selection.end], name);
        }
    }
}
