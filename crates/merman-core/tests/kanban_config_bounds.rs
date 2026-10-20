#![cfg(feature = "diagram-kanban")]

use merman_core::{Engine, Error, ParseOptions};

fn source(padding: &str) -> String {
    format!("---\nconfig:\n  mindmap:\n    padding: {padding}\n---\nkanban\n  one[One]\n")
}

#[test]
fn unrepresentable_kanban_padding_returns_a_parse_error_in_both_public_paths() {
    let engine = Engine::new();
    for padding in ["1.7976931348623157e308", "-1.7976931348623157e308"] {
        let source = source(padding);
        let semantic = engine.parse_diagram_sync(&source, ParseOptions::strict());
        let typed = engine.parse_diagram_for_render_model_sync(&source, ParseOptions::strict());
        for error in [semantic.unwrap_err(), typed.unwrap_err()] {
            assert!(
                matches!(&error, Error::DiagramParse { diagram_type, .. } if diagram_type == "kanban"),
                "{error:?}"
            );
            assert!(error.to_string().contains("padding"), "{error}");
        }
        assert_eq!(
            engine
                .parse_diagram_sync(&source, ParseOptions::lenient())
                .unwrap()
                .unwrap()
                .meta
                .diagram_type,
            "error"
        );
    }
}

#[test]
fn representable_kanban_padding_keeps_its_compatibility_value() {
    let engine = Engine::new();
    for (padding, expected) in [
        ("10", 20),
        ("4611686018427387903", 9223372036854775806),
        ("-4611686018427387904", i64::MIN),
    ] {
        let parsed = engine
            .parse_diagram_sync(&source(padding), ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(parsed.model["sections"][0]["padding"], expected);
    }
}
