#![cfg(feature = "diagram-radar")]

use merman_core::SemanticParser;
use merman_model::families::radar::RadarDiagramModel;
use merman_model::semantic::SemanticDiagramModel;
#[cfg(feature = "compat-json")]
use serde_json::Value;

// Mermaid 12.3.0 (2086904e): common.langium NUMBER = FLOAT | INT uses
// Number(input); radar.langium admits it in all three numeric options and both
// entry forms. radar/db.ts applies the last option before clamping ticks to 32.
fn parse_radar(source: &str) -> RadarDiagramModel {
    let parsed = SemanticParser::new()
        .parse_diagram_sync(source)
        .expect("valid Radar source");
    let SemanticDiagramModel::Radar(model) = parsed.model else {
        panic!("expected the canonical Radar model");
    };
    model
}

#[cfg(feature = "compat-json")]
fn assert_projected_number(actual: &Value, expected: f64, source: &str) {
    if expected.is_finite() {
        assert_eq!(
            actual.as_f64().map(f64::to_bits),
            Some(expected.to_bits()),
            "binary64 conversion for {source:?}",
        );
    } else {
        assert!(
            actual.is_null(),
            "JSON.stringify projects overflow to null: {source:?}",
        );
    }
}

#[test]
fn all_radar_number_slots_follow_binary64_decimal_conversion() {
    for (token, expected) in [
        ("0".to_string(), 0.0),
        ("000.5".to_string(), 0.5),
        ("31.5".to_string(), 31.5),
        ("32.00000000000001".to_string(), 32.00000000000001),
        ("9007199254740991".to_string(), 9_007_199_254_740_991.0),
        ("9007199254740993".to_string(), 9_007_199_254_740_992.0),
        ("9007199254740995".to_string(), 9_007_199_254_740_996.0),
        ("9007199254740993.0".to_string(), 9_007_199_254_740_992.0),
        ("9007199254740993.5".to_string(), 9_007_199_254_740_994.0),
        ("9223372036854775808".to_string(), 2_f64.powi(63)),
        ("18446744073709551616".to_string(), 2_f64.powi(64)),
        (format!("1{}", "0".repeat(308)), 1e308),
        (format!("17976931348623157{}", "0".repeat(292)), f64::MAX),
        (
            format!("17976931348623159{}", "0".repeat(292)),
            f64::INFINITY,
        ),
        ("9".repeat(309), f64::INFINITY),
        (format!("{}.5", "9".repeat(309)), f64::INFINITY),
        (format!("0.{}5", "0".repeat(323)), f64::from_bits(1)),
        (format!("0.{}1", "0".repeat(330)), 0.0),
    ] {
        let source = format!(
            "radar-beta\naxis a,b\nticks {token}, max {token}, min {token}\n\
             curve positional{{{token}, 1}}, referenced{{b: {token}, a: 1}}\n"
        );
        let model = parse_radar(&source);
        assert_eq!(model.options.ticks.to_bits(), expected.min(32.0).to_bits());
        assert_eq!(model.options.max.unwrap().to_bits(), expected.to_bits());
        assert_eq!(model.options.min.to_bits(), expected.to_bits());
        assert_eq!(model.curves.len(), 2);
        assert_eq!(model.curves[0].entries.len(), 2);
        assert_eq!(model.curves[1].entries.len(), 2);
        assert_eq!(model.curves[0].entries[0].to_bits(), expected.to_bits());
        assert_eq!(model.curves[0].entries[1], 1.0);
        assert_eq!(model.curves[1].entries[0], 1.0);
        assert_eq!(model.curves[1].entries[1].to_bits(), expected.to_bits());

        #[cfg(feature = "compat-json")]
        {
            let projected = merman_core::MermaidEngine::new()
                .parse_diagram_sync(&source, merman_core::ParseOptions::strict())
                .unwrap()
                .unwrap()
                .model;
            assert_projected_number(&projected["options"]["ticks"], expected.min(32.0), &source);
            assert_projected_number(&projected["options"]["max"], expected, &source);
            assert_projected_number(&projected["options"]["min"], expected, &source);
            assert_projected_number(&projected["curves"][0]["entries"][0], expected, &source);
            assert_projected_number(&projected["curves"][1]["entries"][1], expected, &source);
        }
    }
}

#[test]
fn radar_ticks_clamp_after_conversion_and_last_option_selection() {
    for token in [
        "9223372036854775808".to_string(),
        "9".repeat(309),
        format!("{}.5", "9".repeat(309)),
    ] {
        let model = parse_radar(&format!("radar-beta\nticks {token}\n"));
        assert_eq!(model.options.ticks, 32.0);
        for separator in ["\n", ", ", " ", ""] {
            let model = parse_radar(&format!("radar-beta\nticks {token}{separator}ticks 2.5\n"));
            assert_eq!(model.options.ticks, 2.5);
            let model = parse_radar(&format!("radar-beta\nticks 2.5{separator}ticks {token}\n"));
            assert_eq!(model.options.ticks, 32.0);
        }
    }
}

#[test]
fn radar_decimal_numbers_allow_a_following_option_keyword() {
    // The Radar entry rule repeats Option groups without requiring a separator.
    for (body, ticks, max, min) in [
        ("ticks 1.5min 2", 1.5, None, 2.0),
        ("ticks 000.5min 2", 0.5, None, 2.0),
        ("ticks 9223372036854775808min 2", 32.0, None, 2.0),
        (
            "max 9007199254740993ticks 1.5",
            1.5,
            Some(9_007_199_254_740_992.0),
            0.0,
        ),
        (
            "min 9007199254740993.0max 9223372036854775808",
            5.0,
            Some(2_f64.powi(63)),
            9_007_199_254_740_992.0,
        ),
    ] {
        let model = parse_radar(&format!("radar-beta\n{body} %% hidden\n"));
        assert_eq!(model.options.ticks, ticks, "{body}");
        assert_eq!(model.options.min, min, "{body}");
        assert_eq!(model.options.max, max, "{body}");
    }
}

#[test]
fn radar_numbers_allow_adjacent_axis_curve_and_common_statements() {
    let model = parse_radar("radar-beta\nticks 1.5axis b\n");
    assert_eq!(model.options.ticks, 1.5);
    assert_eq!(model.axes[0].name, "b");

    for body in [
        "ticks 1.5axis a,b curve c{1,2}max 3min 0",
        "ticks 1.5axis a,b curve c{\n1,2\n}max 3min 0",
    ] {
        let model = parse_radar(&format!("radar-beta\n{body}title Adjacent\n"));
        assert_eq!(model.options.ticks, 1.5);
        assert_eq!(model.options.max, Some(3.0));
        assert_eq!(model.options.min, 0.0);
        assert_eq!(model.axes.len(), 2);
        assert_eq!(model.axes[0].name, "a");
        assert_eq!(model.axes[1].name, "b");
        assert_eq!(model.curves[0].entries, [1.0, 2.0]);
        assert_eq!(model.title.as_deref(), Some("Adjacent"));
    }

    for body in [
        "ticks 1.5axisSuffix b",
        "ticks 1.5, axis b",
        "axis ticks 1.5",
        "axis a, curve c{1}",
        "curve c{1}, max 3",
        "curve c{1}maxSuffix 3",
    ] {
        let source = format!("radar-beta\n{body}\n");
        assert!(
            SemanticParser::new().parse_diagram_sync(&source).is_err(),
            "{body}"
        );
    }
}

#[test]
fn radar_number_conversion_does_not_widen_the_terminal_grammar() {
    for token in [
        "01", "00", ".5", "+1", "-1", "1.", "1e3", "1.2e3", "0x10", "0b11", "NaN", "Infinity",
        "1tail", "1.2tail", "1.2.3", "0.0.0", "1..2", "1_000",
    ] {
        for body in [
            format!("ticks {token}"),
            format!("max {token}"),
            format!("min {token}"),
            format!("curve c{{{token}}}"),
            format!("curve c{{a: {token}}}"),
        ] {
            let source = format!("radar-beta\naxis a\n{body}\n");
            assert!(
                SemanticParser::new().parse_diagram_sync(&source).is_err(),
                "only the complete common.langium NUMBER is valid: {source:?}",
            );
        }
    }
}

#[cfg(feature = "editor-facts")]
#[test]
fn radar_large_numbers_keep_complete_editor_and_recovery_snapshots() {
    use merman_core::{
        EditorSemanticCompleteness, EditorSemanticRole, ParsedEditorFacts, SemanticSnapshotCapture,
    };
    use merman_model::OperationControl;
    use merman_model::source::SourceSpan;

    for (token, expected) in [
        ("9007199254740993".to_string(), 9_007_199_254_740_992.0),
        ("9223372036854775808".to_string(), 2_f64.powi(63)),
        ("9".repeat(309), f64::INFINITY),
        (format!("{}.5", "9".repeat(309)), f64::INFINITY),
    ] {
        for newline in ["\n", "\r\n"] {
            let source = format!(
                "---\ntitle: Numeric 😀\n---\nradar-beta\naxis a\n\
                 ticks {token}min {token}, max {token} %% hidden\n\
                 curve plain{{{token}}}, detailed{{a: {token}}}\n"
            )
            .replace('\n', newline);
            let parser = SemanticParser::new();
            let SemanticSnapshotCapture::Snapshot(snapshot) =
                parser.capture_diagram_snapshot_sync(&source)
            else {
                panic!("valid Radar numbers must retain a snapshot");
            };
            assert!(snapshot.outcome().parsed_model().is_some());
            let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                panic!("Radar must expose requested editor facts");
            };
            assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
            assert!(facts.diagnostics.is_empty());
            let payloads: Vec<_> = facts
                .symbols
                .iter()
                .filter(|symbol| symbol.role == EditorSemanticRole::Payload)
                .filter(|symbol| {
                    matches!(
                        symbol.detail.as_deref(),
                        Some("radar option" | "radar curve entry")
                    )
                })
                .collect();
            assert_eq!(payloads.len(), 5);
            for (start, _) in source.match_indices(token.as_str()) {
                let span = SourceSpan::new(start, start + token.len());
                assert!(payloads.iter().any(|symbol| symbol.selection == span));
                assert!(
                    facts
                        .expected_syntax
                        .iter()
                        .any(|syntax| syntax.span == span)
                );
            }
            for payload in payloads {
                if expected.is_infinite() {
                    assert_eq!(payload.name, "Infinity");
                } else {
                    assert_eq!(payload.name.parse::<f64>().unwrap(), expected);
                }
            }

            let SemanticSnapshotCapture::Snapshot(recovery) = parser
                .capture_diagram_recovery_snapshot_controlled_sync(
                    &source,
                    &OperationControl::new(),
                )
                .unwrap()
            else {
                panic!("valid Radar numbers must retain a recovery snapshot");
            };
            assert!(recovery.outcome().parsed_model().is_some());
            let ParsedEditorFacts::Available(recovery_facts) = recovery.editor_facts() else {
                panic!("Radar must expose recovery facts");
            };
            assert_eq!(recovery_facts.completeness, facts.completeness);
            assert!(recovery_facts.diagnostics.is_empty());
            assert!(recovery_facts.symbols.is_empty());
            assert!(recovery_facts.expected_syntax.is_empty());
        }
    }
}

#[cfg(feature = "editor-facts")]
#[test]
fn radar_invalid_numeric_suffix_recovers_only_its_original_statement() {
    use merman_core::{
        EditorSemanticCompleteness, EditorSemanticDiagnosticKind, ParsedEditorFacts,
        SemanticSnapshotCapture,
    };
    use merman_model::source::SourceSpan;

    for newline in ["\n", "\r\n"] {
        let invalid = "min 9223372036854775808tail";
        let source = format!(
            "---\ntitle: Numeric 😀\n---\nradar-beta\naxis a\n\
             ticks 9223372036854775808\ncurve before{{9007199254740993}}\n\
               {invalid} %% hidden\ncurve after{{1}}\n"
        )
        .replace('\n', newline);
        let parser = SemanticParser::new();
        assert!(parser.parse_diagram_sync(&source).is_err());
        let SemanticSnapshotCapture::Snapshot(snapshot) =
            parser.capture_diagram_snapshot_sync(&source)
        else {
            panic!("invalid Radar suffix must retain recovery facts");
        };
        assert!(snapshot.outcome().parsed_model().is_none());
        let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
            panic!("Radar must expose requested editor facts");
        };
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert_eq!(facts.diagnostics.len(), 1);
        let start = source.find(invalid).unwrap();
        assert_eq!(
            facts.diagnostics[0].kind,
            EditorSemanticDiagnosticKind::ParserRecovery,
        );
        assert_eq!(
            facts.diagnostics[0].span,
            Some(SourceSpan::new(start, start + invalid.len())),
        );
        for name in ["before", "after"] {
            assert!(facts.symbols.iter().any(|symbol| symbol.name == name));
        }
    }
}

#[cfg(feature = "editor-facts")]
#[test]
fn radar_adjacent_statements_keep_original_editor_ranges() {
    use merman_core::{EditorSemanticCompleteness, ParsedEditorFacts, SemanticSnapshotCapture};
    use merman_model::source::SourceSpan;

    for newline in ["\n", "\r\n"] {
        let source = concat!(
            "---\ntitle: Numeric 😀\n---\nradar-beta\n",
            "ticks 1.5axis b curve c{9007199254740993}max 9223372036854775808\n",
        )
        .replace('\n', newline);
        let SemanticSnapshotCapture::Snapshot(snapshot) =
            SemanticParser::new().capture_diagram_snapshot_sync(&source)
        else {
            panic!("adjacent Radar statements must retain their snapshot");
        };
        assert!(snapshot.outcome().parsed_model().is_some());
        let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
            panic!("Radar must expose adjacent statement facts");
        };
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
        assert!(facts.diagnostics.is_empty());
        for (raw, name) in [
            ("1.5", "1.5"),
            ("9007199254740993", "9007199254740992"),
            ("9223372036854775808", "9223372036854776000"),
        ] {
            let start = source.find(raw).unwrap();
            let span = SourceSpan::new(start, start + raw.len());
            assert!(
                facts
                    .symbols
                    .iter()
                    .any(|symbol| symbol.name == name && symbol.selection == span)
            );
        }
        for (context, name) in [("axis b", "b"), ("curve c", "c")] {
            let start = source.find(context).unwrap() + context.len() - name.len();
            let span = SourceSpan::new(start, start + name.len());
            assert!(
                facts
                    .symbols
                    .iter()
                    .any(|symbol| symbol.name == name && symbol.selection == span)
            );
        }
    }
}
