#![cfg(feature = "diagram-c4")]

use merman_core::SemanticParser;
use merman_model::families::c4::C4RelationOffset;
use merman_model::semantic::SemanticDiagramModel;

#[test]
fn c4_named_wrap_conversion_follows_the_argument_slot() {
    for (token, numeric_truth) in [
        ("0".to_string(), false),
        ("-0".to_string(), false),
        ("invalid".to_string(), false),
        ("1".to_string(), true),
        ("9".repeat(309), true),
    ] {
        for (arguments, expected) in [
            (format!("$wrap=\"{token}\""), true),
            (
                format!("\"red\", \"blue\", $wrap=\"{token}\""),
                numeric_truth,
            ),
        ] {
            let source =
                format!("C4Context\nRel(a, b, \"Calls\")\nUpdateRelStyle(a, b, {arguments})\n");
            let parsed = SemanticParser::new().parse_diagram_sync(&source).unwrap();
            let SemanticDiagramModel::C4(model) = parsed.model else {
                panic!("expected C4")
            };
            assert_eq!(model.rels[0].wrap, expected, "{arguments}");
        }
    }
}

#[test]
fn c4_numeric_style_assignments_validate_string_fields_after_repair() {
    for key in ["from", "to", "lineColor", "textColor"] {
        let source = format!(
            "C4Context\nRel(a, b, \"Calls\")\nUpdateRelStyle(a, b, \"red\", \"blue\", ${key}=\"0\")\n"
        );
        assert!(
            SemanticParser::new().parse_diagram_sync(&source).is_err(),
            "{key}"
        );
        let repaired = format!("{source}UpdateRelStyle(a, b, ${key}=\"restored\")\n");
        assert!(
            SemanticParser::new().parse_diagram_sync(&repaired).is_ok()
                == matches!(key, "lineColor" | "textColor"),
            "{key}"
        );
    }
}

#[test]
fn c4_numeric_arguments_preserve_the_database_coercion_stage() {
    // Mermaid 12.3.0 (2086904e), c4Db.ts updateRelStyle/updateLayoutConfig:
    // offset slots parseInt strings, color slots retain named strings, and row counts
    // compare the binary64 parseInt result against 1 without narrowing to a Rust integer.
    let cases = [
        ("1000000000000000000000".to_string(), 1e21),
        ("999999999999999999999".to_string(), 1e21),
        ("999999999999999900000".to_string(), 9.999999999999999e20),
        ("9007199254740993".to_string(), 9_007_199_254_740_992.0),
        ("-9007199254740993".to_string(), -9_007_199_254_740_992.0),
        (
            "1000000000000000128".to_string(),
            1_000_000_000_000_000_128.0,
        ),
        (
            "18446744073709551616".to_string(),
            18_446_744_073_709_551_616.0,
        ),
        ("-1000000000000000000000".to_string(), -1e21),
        ("0x20000000000001tail".to_string(), 9_007_199_254_740_992.0),
        (
            "-0x100000000000000000000000000000001".to_string(),
            -2_f64.powi(128),
        ),
        ("0x100000000000008000".to_string(), 2_f64.powi(68)),
        (
            "0x100000000000008001".to_string(),
            2_f64.powi(68) + 65_536.0,
        ),
        (
            "0x100000000000018000".to_string(),
            2_f64.powi(68) + 131_072.0,
        ),
        (
            "0x100000000000017fff".to_string(),
            2_f64.powi(68) + 65_536.0,
        ),
        ("9".repeat(80), 1e80),
        ("9".repeat(309), f64::INFINITY),
        (format!("-{}", "9".repeat(309)), f64::NEG_INFINITY),
        ("\u{feff}-7px".to_string(), -7.0),
        ("\u{0085}-7".to_string(), f64::NAN),
        ("+60.75".to_string(), 60.0),
        ("0b11".to_string(), 0.0),
        ("-0".to_string(), -0.0),
        ("not-a-number".to_string(), f64::NAN),
    ];
    let parser = SemanticParser::new();
    for (token, expected) in cases {
        let source = format!(
            "C4Context\n\
             Rel(a, b, \"Number\")\n\
             Rel(c, d, \"String\")\n\
             UpdateRelStyle(a, b, \"red\", \"blue\", $offsetX=\"{token}\", $offsetY=\"{token}\")\n\
             UpdateRelStyle(c, d, $offsetX=\"{token}\", $offsetY=\"{token}\")\n\
             UpdateLayoutConfig($c4ShapeInRow=\"{token}\", $c4BoundaryInRow=\"{token}\")\n"
        );
        let parsed = parser.parse_diagram_sync(&source).expect("valid C4 source");
        let SemanticDiagramModel::C4(model) = parsed.model else {
            panic!("expected a C4 model");
        };
        for offset in [&model.rels[0].offset_x, &model.rels[0].offset_y] {
            let Some(C4RelationOffset::Number(actual)) = offset else {
                panic!("an offset argument must retain the parsed Number: {token:?}");
            };
            if expected.is_nan() {
                assert!(actual.is_nan(), "parseInt({token:?})");
            } else {
                assert_eq!(actual.to_bits(), expected.to_bits(), "parseInt({token:?})");
            }
        }
        let trimmed = token.trim_matches(merman_model::text::is_ecmascript_whitespace);
        for offset in [&model.rels[1].offset_x, &model.rels[1].offset_y] {
            assert_eq!(
                offset,
                &Some(C4RelationOffset::String(trimmed.to_string())),
                "a named offset in a color slot must remain a string: {token:?}",
            );
        }
        let shape_limit = if expected >= 1.0 { expected } else { 4.0 };
        let boundary_limit = if expected >= 1.0 { expected } else { 2.0 };
        assert_eq!(model.layout.c4_shape_in_row, shape_limit, "{token:?}");
        assert_eq!(model.layout.c4_boundary_in_row, boundary_limit, "{token:?}");

        #[cfg(feature = "compat-json")]
        {
            let projected = merman_core::MermaidEngine::new()
                .parse_diagram_sync(&source, merman_core::ParseOptions::strict())
                .expect("C4 compatibility parse")
                .expect("C4 diagram")
                .model;
            for key in ["offsetX", "offsetY"] {
                assert_eq!(
                    projected["rels"][0][key],
                    merman_model::json::number_value(expected),
                    "numeric {key} projection for {token:?}",
                );
                assert_eq!(projected["rels"][1][key].as_str(), Some(trimmed));
            }
            assert_eq!(
                projected["layout"]["c4ShapeInRow"],
                merman_model::json::number_value(shape_limit),
            );
            assert_eq!(
                projected["layout"]["c4BoundaryInRow"],
                merman_model::json::number_value(boundary_limit),
            );
        }
    }
}
