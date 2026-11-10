#![cfg(feature = "diagram-wardley")]

use merman_core::{Error, ParseDiagnosticSpanKind, SemanticParser};
use merman_model::families::wardley::{WardleyDiagramModel, WardleyPoint};
use merman_model::semantic::SemanticDiagramModel;
use merman_model::source::SourceSpan;

fn parse_wardley(source: &str) -> WardleyDiagramModel {
    let parsed = SemanticParser::new()
        .parse_diagram_sync(source)
        .expect("valid Wardley source");
    let SemanticDiagramModel::Wardley(model) = parsed.model else {
        panic!("expected the canonical Wardley model");
    };
    model
}

#[test]
fn annotation_ints_keep_javascript_rounding_and_decimal_overflow() {
    // Mermaid 12.3.0: Annotation.number is INT, converted with Number(), without a bit limit.
    for (token, expected_bits) in [
        ("0".to_string(), 0x0000_0000_0000_0000),
        ("9007199254740991".into(), 0x433f_ffff_ffff_ffff),
        ("9007199254740993".into(), 0x4340_0000_0000_0000),
        ("9007199254740995".into(), 0x4340_0000_0000_0002),
        ("18446744073709551615".into(), 0x43f0_0000_0000_0000),
        ("18446744073709551616".into(), 0x43f0_0000_0000_0000),
        ("18446744073709555712".into(), 0x43f0_0000_0000_0001),
        ("1000000000000000128".into(), 0x43ab_c16d_674e_c801),
        ("1000000000000000000000".into(), 0x444b_1ae4_d6e2_ef50),
        (
            format!("17976931348623157{}", "0".repeat(292)),
            0x7fef_ffff_ffff_ffff,
        ),
        ("9".repeat(308), 0x7fe1_ccf3_85eb_c8a0),
        ("9".repeat(309), 0x7ff0_0000_0000_0000),
        (format!("1{}", "0".repeat(309)), 0x7ff0_0000_0000_0000),
    ] {
        let source = format!("wardley-beta\nannotation {token}, [0.5, 0.25] \"x\"\n");
        let model = parse_wardley(&source);
        assert_eq!(model.annotations.len(), 1);
        let annotation = &model.annotations[0];
        assert_eq!(annotation.number.to_bits(), expected_bits, "{token}");
        assert_eq!(annotation.coordinates, [WardleyPoint { x: 25.0, y: 50.0 }]);
        assert_eq!(annotation.text.as_deref(), Some("x"));

        #[cfg(feature = "compat-json")]
        {
            let parsed = merman_core::MermaidEngine::new()
                .parse_diagram_sync(&source, merman_core::ParseOptions::strict())
                .expect("valid Wardley source projects to compatibility JSON")
                .expect("Wardley is detected");
            let number = &parsed.model["annotations"][0]["number"];
            if annotation.number.is_finite() {
                assert_eq!(number.as_f64().unwrap().to_bits(), expected_bits, "{token}");
            } else {
                assert!(number.is_null(), "JSON.stringify maps Infinity to null");
            }
        }
    }
}

#[test]
fn number_conversion_keeps_annotation_int_grammar() {
    for token in [
        "01", "1.0", "1e3", "-1", "+1", "0x10", "1tail", "Infinity", "NaN",
    ] {
        let source = format!("wardley-beta\nannotation {token}, [0.5, 0.5] \"x\"\n");
        assert!(
            SemanticParser::new().parse_diagram_sync(&source).is_err(),
            "Number conversion must not widen the INT grammar: {token}",
        );
    }
}

#[test]
fn signed_label_offsets_preserve_number_values_for_components_and_pipeline_children() {
    for (token, expected) in [
        ("0".to_string(), 0.0),
        ("9007199254740993".into(), 9_007_199_254_740_992.0),
        ("18446744073709551616".into(), 18_446_744_073_709_551_616.0),
        ("1000000000000000000000".into(), 1e21),
        ("9".repeat(309), f64::INFINITY),
    ] {
        let source = format!(
            "wardley-beta\ncomponent Regular [0.5, 0.5] label [{token}, - {token}]\n\
             component Parent [0.5, 0.5]\npipeline Parent {{\n\
             component Child [0.25] label [- {token}, {token}]\n}}\n"
        );
        let model = parse_wardley(&source);
        for (id, sign) in [("Regular", 1.0), ("Parent_Child", -1.0)] {
            let node = model.nodes.iter().find(|node| node.id == id).unwrap();
            assert_eq!(
                node.label_offset_x.unwrap().to_bits(),
                (sign * expected).to_bits()
            );
            assert_eq!(
                node.label_offset_y.unwrap().to_bits(),
                (-sign * expected).to_bits()
            );
        }
        #[cfg(feature = "compat-json")]
        {
            let projected = merman_core::MermaidEngine::new()
                .parse_diagram_sync(&source, merman_core::ParseOptions::strict())
                .unwrap()
                .unwrap()
                .model;
            for node in projected["nodes"].as_array().unwrap() {
                if node["id"] == "Parent" {
                    assert!(node.get("labelOffsetX").is_none());
                    continue;
                }
                if expected.is_infinite() {
                    assert!(node["labelOffsetX"].is_null());
                    assert!(node["labelOffsetY"].is_null());
                } else {
                    let sign = if node["id"] == "Regular" { 1.0 } else { -1.0 };
                    assert_eq!(node["labelOffsetX"].as_f64().unwrap(), sign * expected);
                    assert_eq!(node["labelOffsetY"].as_f64().unwrap(), -sign * expected);
                }
            }
        }
    }
    for token in ["01", "1.0", "+1", "1e3", "Infinity", "- -1"] {
        assert!(
            merman_core::SemanticParser::new()
                .parse_diagram_sync(&format!(
                    "wardley-beta\ncomponent X [0.5, 0.5] label [{token}, 0]\n"
                ))
                .is_err(),
            "{token}"
        );
    }
}

#[cfg(feature = "editor-facts")]
#[test]
fn signed_label_offsets_keep_sign_whitespace_in_editor_and_recovery_ranges() {
    use merman_core::{EditorSemanticCompleteness, ParsedEditorFacts, SemanticSnapshotCapture};
    for newline in ["\n", "\r\n"] {
        for invalid_suffix in [false, true] {
            let mut source = concat!(
                "\u{feff}---\ntitle: Numeric 😀\n---\nwardley-beta\n",
                "component Regular [0.5, 0.5] label [1000000000000000000000, - \t1000000000000000000000]\n",
                "component Parent [0.5, 0.5]\npipeline Parent {\n",
                "component Child [0.25] label [- \t1000000000000000000000, 1000000000000000000000]\n}\n",
            ).to_string();
            if invalid_suffix {
                source.push_str("component Bad [0.5, 0.5] label [1.5, 0]\n");
            }
            let source = source.replace('\n', newline);
            let SemanticSnapshotCapture::Snapshot(snapshot) =
                SemanticParser::new().capture_diagram_snapshot_sync(&source)
            else {
                panic!("Wardley retains a snapshot")
            };
            let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                panic!("Wardley retains editor facts")
            };
            assert_eq!(
                facts.completeness,
                if invalid_suffix {
                    EditorSemanticCompleteness::Recovered
                } else {
                    EditorSemanticCompleteness::Complete
                }
            );
            let offsets: Vec<_> = facts
                .symbols
                .iter()
                .filter(|symbol| {
                    matches!(
                        symbol.detail.as_deref(),
                        Some("wardley label X offset" | "wardley label Y offset")
                    )
                })
                .collect();
            assert_eq!(offsets.len(), 4);
            for (start, _) in source.match_indices("1000000000000000000000") {
                let negative = source[..start].ends_with("- \t");
                let span = SourceSpan::new(start - if negative { 3 } else { 0 }, start + 22);
                assert!(offsets.iter().any(|symbol| symbol.selection == span
                    && symbol.name == if negative { "-1e+21" } else { "1e+21" }));
            }
        }
    }
}

#[test]
fn annotation_coordinate_errors_keep_number_text_and_exact_source_spans() {
    for (token, label, coordinate, received) in [
        (
            "18446744073709551616".to_string(),
            "18446744073709552000",
            "101.0".to_string(),
            "101",
        ),
        ("9".repeat(309), "Infinity", "101.0".into(), "101"),
        (
            "1".into(),
            "1",
            format!("{}.0", "9".repeat(309)),
            "Infinity",
        ),
    ] {
        for newline in ["\n", "\r\n"] {
            let source = format!(
                concat!(
                    "\u{feff}---\ntitle: 原稿😀\n---\nwardley-beta\n",
                    "annotation {token}, [0.5, {coordinate}] \"x\"\n",
                ),
                token = token,
                coordinate = coordinate,
            )
            .replace('\n', newline);
            let start = source.find(&coordinate).expect("coordinate is authored");
            let expected = SourceSpan::new(start, start + coordinate.len());
            let Error::DiagramParse { diagnostic, .. } = SemanticParser::new()
                .parse_diagram_sync(&source)
                .expect_err("out-of-range coordinates remain invalid")
            else {
                panic!("expected a structured Wardley parse diagnostic");
            };
            assert_eq!(diagnostic.span(), Some(expected));
            assert_eq!(diagnostic.span_kind(), ParseDiagnosticSpanKind::Exact);
            assert_eq!(
                diagnostic.message(),
                format!(
                    "Annotation {label} evolution must be between 0-1 (decimal) or 0-100 (percentage). Received: {received}",
                ),
            );
            assert_eq!(&source[expected.start..expected.end], coordinate);
        }
    }
}

#[cfg(feature = "editor-facts")]
#[test]
fn annotation_editor_numbers_keep_authored_ranges_after_rounding_and_overflow() {
    use merman_core::{
        EditorExpectedSyntaxKind, EditorSemanticCompleteness, EditorSemanticKind,
        EditorSemanticRole, ParsedEditorFacts, SemanticSnapshotCapture,
    };

    for (token, display) in [
        ("9007199254740993".to_string(), "9007199254740992"),
        ("18446744073709551616".into(), "18446744073709552000"),
        ("1000000000000000000000".into(), "1e+21"),
        ("9".repeat(309), "Infinity"),
    ] {
        for newline in ["\n", "\r\n"] {
            for invalid_suffix in [false, true] {
                let mut source = format!(
                    concat!(
                        "\u{feff}---\ntitle: 原稿😀\n---\nwardley-beta\n",
                        "note \"前置😀\" [0.4, 0.6]\n",
                        "annotation {token}, [0.5, 0.25] \"注釈😀\"\n",
                        "component \"後続😀\" [0.6, 0.7]\n",
                    ),
                    token = token,
                );
                if invalid_suffix {
                    source.push_str("annotation 1.5, [0.5, 0.5] \"invalid\"\n");
                }
                let source = source.replace('\n', newline);
                let start = source.find(&token).expect("annotation number is authored");
                let expected = SourceSpan::new(start, start + token.len());
                let SemanticSnapshotCapture::Snapshot(snapshot) =
                    SemanticParser::new().capture_diagram_snapshot_sync(&source)
                else {
                    panic!("Wardley source must retain a semantic snapshot");
                };
                let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
                    panic!("Wardley numbers must retain requested editor facts");
                };
                assert_eq!(snapshot.outcome().parsed_model().is_none(), invalid_suffix);
                assert_eq!(
                    facts.completeness,
                    if invalid_suffix {
                        EditorSemanticCompleteness::Recovered
                    } else {
                        EditorSemanticCompleteness::Complete
                    },
                );
                let numbers: Vec<_> = facts
                    .symbols
                    .iter()
                    .filter(|symbol| symbol.detail.as_deref() == Some("wardley annotation number"))
                    .collect();
                assert_eq!(numbers.len(), 1);
                let number = numbers[0];
                assert_eq!(number.name, display);
                assert_eq!(number.kind, EditorSemanticKind::Property);
                assert_eq!(number.role, EditorSemanticRole::Payload);
                assert_eq!(number.span, expected);
                assert_eq!(number.selection, expected);
                assert_eq!(&source[number.selection.start..number.selection.end], token);
                assert!(facts.expected_syntax.iter().any(|syntax| {
                    syntax.kind == EditorExpectedSyntaxKind::Payload && syntax.span == expected
                }));
                let name = "後続😀";
                let start = source.find(name).expect("following component is authored");
                let following = facts
                    .symbols
                    .iter()
                    .find(|symbol| symbol.name == name)
                    .expect("following component remains available for navigation");
                assert_eq!(
                    following.selection,
                    SourceSpan::new(start, start + name.len()),
                );
                if invalid_suffix {
                    let start = source.find("1.5").expect("invalid INT is authored");
                    assert!(facts.diagnostics.iter().any(|diagnostic| {
                        diagnostic.span == Some(SourceSpan::new(start, start + 3))
                    }));
                } else {
                    assert!(facts.diagnostics.is_empty(), "{:?}", facts.diagnostics);
                }
            }
        }
    }
}
