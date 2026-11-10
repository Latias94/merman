#![cfg(feature = "diagram-block")]

use merman_core::SemanticParser;
use merman_model::families::block::BlockDiagramModel;
use merman_model::semantic::SemanticDiagramModel;
use merman_model::source::SourceSpan;

fn parse_block(source: &str) -> BlockDiagramModel {
    let parsed = SemanticParser::new().parse_diagram_sync(source).unwrap();
    let SemanticDiagramModel::Block(model) = parsed.model else {
        panic!("expected Block semantics")
    };
    model
}

#[test]
fn block_decimal_counts_follow_javascript_integer_rounding_and_overflow() {
    // Mermaid 12.3.0 block.jison: unsigned decimal tokens use parseInt(..., 10), not i64.
    for (token, expected) in [
        ("0".to_string(), 0.0),
        ("0002".to_string(), 2.0),
        ("9007199254740993".to_string(), 9_007_199_254_740_992.0),
        (
            "9223372036854775808".to_string(),
            9_223_372_036_854_775_808.0,
        ),
        (
            "18446744073709551615".to_string(),
            18_446_744_073_709_551_616.0,
        ),
        ("9".repeat(309), f64::INFINITY),
    ] {
        let source = format!("block\ncolumns {token}\nA:{token}\n");
        let model = parse_block(&source);
        assert_eq!(model.root().unwrap().columns, Some(expected), "{token}");
        assert_eq!(
            model.blocks_flat[1].width_in_columns,
            Some(expected),
            "{token}"
        );
        assert_eq!(model.blocks_flat.len(), 2, "counts do not expand nodes");
    }
}

#[test]
fn block_large_width_warnings_use_the_database_number_text() {
    for (token, text) in [
        ("9007199254740993".to_string(), "9007199254740992"),
        ("1000000000000000000000".to_string(), "1e+21"),
        ("9".repeat(309), "Infinity"),
    ] {
        let model = parse_block(&format!("block\ncolumns 2\nA:{token}\n"));
        assert_eq!(model.warning_facts.len(), 1);
        let warning = serde_json::to_string(&model.warning_facts[0]).unwrap();
        assert!(warning.contains(&format!(
            "Block A width {text} exceeds configured column width 2"
        )));
    }
}

#[test]
fn block_large_spaces_report_expansion_admission_at_the_authored_number() {
    for token in [
        "200001".to_string(),
        "9223372036854775808".to_string(),
        "9".repeat(309),
    ] {
        for source in [
            format!("block\nspace:{token}\n"),
            format!("block\nA space:{token} B\n"),
        ] {
            let error = SemanticParser::new()
                .parse_diagram_sync(&source)
                .unwrap_err();
            let merman_core::Error::DiagramParse { diagnostic, .. } = error else {
                panic!("expected Block admission error")
            };
            let start = source.find(&token).unwrap();
            assert_eq!(
                diagnostic.span(),
                Some(SourceSpan::new(start, start + token.len()))
            );
            assert!(
                diagnostic
                    .message()
                    .contains("exceeds the materialization limit")
            );
        }
    }
}

#[test]
fn block_number_conversion_keeps_decimal_token_boundaries_and_node_fallbacks() {
    for (token, value, suffix) in [("1.5", 1.0, ".5"), ("1e3", 1.0, "e3"), ("0x10", 0.0, "x10")] {
        let model = parse_block(&format!("block\ncolumns {token}\nA:{token}\n"));
        assert_eq!(model.root().unwrap().columns, Some(value));
        assert!(model.blocks_flat.iter().any(|block| block.id == suffix));
        assert_eq!(
            model
                .blocks_flat
                .iter()
                .find(|block| block.id == "A")
                .unwrap()
                .width_in_columns,
            Some(value)
        );
    }
    for token in ["+2", "Infinity"] {
        let model = parse_block(&format!("block\ncolumns {token}\n"));
        assert_eq!(model.root().unwrap().columns, Some(-1.0));
        assert_eq!(model.blocks_flat[1].id, "columns");
        assert_eq!(model.blocks_flat[2].id, token);
    }
    for source in [
        "block\ncolumns -1\n",
        "block\nA:-1\n",
        "block\nA:+2\n",
        "block\nA:Infinity\n",
        "block\nA: 2\n",
        "block\nspace: 2\n",
        "block\nspace :2\n",
    ] {
        assert!(
            SemanticParser::new().parse_diagram_sync(source).is_err(),
            "{source}"
        );
    }
    assert_eq!(
        parse_block("block\ncolumns auto\nA\n")
            .root()
            .unwrap()
            .columns,
        Some(-1.0)
    );
}

#[test]
fn block_column_tokens_and_spaces_follow_the_selected_lexer_priority() {
    let model =
        parse_block("block\ncolumns autoSuffix\ncolumns %% comment\n2\nspacer\nA space:2 B:3\n");
    assert_eq!(model.root().unwrap().columns, Some(-1.0));
    let ids: Vec<_> = model
        .blocks_flat
        .iter()
        .map(|block| block.id.as_str())
        .collect();
    for id in ["Suffix", "columns", "2", "r", "A", "B"] {
        assert!(ids.contains(&id), "{id}");
    }
    assert_eq!(
        model
            .blocks_flat
            .iter()
            .find(|block| block.id == "B")
            .unwrap()
            .width_in_columns,
        Some(3.0)
    );
    assert_eq!(model.root().unwrap().children.len(), 9);
}

#[cfg(feature = "compat-json")]
#[test]
fn block_compatibility_counts_keep_integer_shape_and_null_overflow() {
    use merman_core::{MermaidEngine, ParseOptions};
    use serde_json::json;

    for (token, expected) in [
        ("2".to_string(), json!(2)),
        (
            "9007199254740993".to_string(),
            json!(9_007_199_254_740_992_i64),
        ),
        ("9".repeat(309), serde_json::Value::Null),
    ] {
        let source = format!("block\ncolumns {token}\nA:{token}\n");
        let parsed = MermaidEngine::new()
            .parse_diagram_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(parsed.model["blocksFlat"][0]["columns"], expected);
        assert_eq!(parsed.model["blocks"][0]["widthInColumns"], expected);
    }
}

#[cfg(feature = "editor-facts")]
#[test]
fn block_large_counts_keep_original_editor_ranges_after_unicode_frontmatter() {
    use merman_core::{EditorSemanticCompleteness, ParsedEditorFacts, SemanticSnapshotCapture};

    let columns = "9223372036854775808";
    let width = "9007199254740993";
    for newline in ["\n", "\r\n"] {
        let source = format!("---\ntitle: 原稿😀\n---\nblock\ncolumns {columns}\nA:{width}\n")
            .replace('\n', newline);
        let SemanticSnapshotCapture::Snapshot(snapshot) =
            SemanticParser::new().capture_diagram_snapshot_sync(&source)
        else {
            panic!("expected Block snapshot")
        };
        assert!(snapshot.outcome().parsed_model().is_some());
        let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
            panic!("expected Block editor facts")
        };
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
        assert!(facts.diagnostics.is_empty());
        for token in [columns, width] {
            let start = source.find(token).unwrap();
            assert!(
                facts
                    .symbols
                    .iter()
                    .any(|symbol| symbol.selection == SourceSpan::new(start, start + token.len()))
            );
        }
    }
}
