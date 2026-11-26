#![cfg(all(feature = "diagram-agentflow", feature = "compat-json"))]

#[cfg(feature = "editor-facts")]
use merman_core::DiagramParseOutcome;
use merman_core::MermaidEngine;
use serde_json::Value;

fn diagnostics(source: &str) -> Value {
    #[cfg(feature = "editor-facts")]
    {
        let snapshot = MermaidEngine::new()
            .parse_diagram_snapshot_with_type_sync("agentflow", source)
            .expect("snapshot parse")
            .expect("snapshot");
        let DiagramParseOutcome::Parsed { model, .. } = snapshot.outcome() else {
            panic!("Agentflow source should parse");
        };
        model["diagnostics"].clone()
    }
    #[cfg(not(feature = "editor-facts"))]
    {
        let parsed = MermaidEngine::new()
            .parse_diagram_with_type_sync("agentflow", source, merman_core::ParseOptions::strict())
            .expect("semantic parse")
            .expect("Agentflow diagram");
        parsed.model["diagnostics"].clone()
    }
}

#[test]
fn agentflow_positions_match_mermaid12_utf16_and_zero_indices_after_frontmatter() {
    let source = "---\r\ntitle: \"中文😀\"\r\n---\r\nagentflow-beta\r\n%% 中文😀 comment\r\na[\"中文😀\"]; b@{ shape: cloud }\r\n";
    let diagnostics = diagnostics(source);
    assert_eq!(diagnostics[0]["position"]["startLine"], 6);
    assert_eq!(diagnostics[0]["position"]["startColumn"], 11);
    assert_eq!(diagnostics[0]["position"]["endLine"], 6);
    assert_eq!(diagnostics[0]["position"]["endColumn"], 28);
    assert_eq!(diagnostics[0]["position"]["startIndex"], 0);
    assert_eq!(diagnostics[0]["position"]["endIndex"], 0);
}

#[test]
fn agentflow_trimmed_fallback_counts_bom_and_retains_original_line_origin() {
    let long_comment = format!("%% {}\r\n", "x".repeat(50_001));
    for (prefix, line, columns) in [
        ("", 1, 0),
        ("\u{feff}", 1, 1),
        ("\u{feff} \t\u{3000}", 1, 4),
        ("\u{feff}\r\n \t", 2, 2),
        ("\u{feff}%% prior\r \t", 2, 2),
        (
            "\u{feff}---\r\ntitle: Before\r\n---\r\n%%{init: {}}%%\r\n%% prior\r\n \t",
            6,
            2,
        ),
    ] {
        let source =
            format!("{prefix}agentflow-beta TD; A((Circle))\r\n  B((Circle))\r\n{long_comment}");
        let diagnostics = diagnostics(&source);
        let first = &diagnostics[0]["position"];
        assert_eq!(first["startLine"], line, "{prefix:?}");
        assert_eq!(first["endLine"], line, "{prefix:?}");
        assert_eq!(first["startColumn"], columns + 19, "{prefix:?}");
        assert_eq!(first["endColumn"], columns + 30, "{prefix:?}");
        let second = &diagnostics[1]["position"];
        assert_eq!(second["startLine"], line + 1, "{prefix:?}");
        assert_eq!(second["startColumn"], 2, "{prefix:?}");
    }
}

#[test]
fn agentflow_trimmed_fallback_keeps_original_error_byte_span() {
    use merman_core::{Error, SemanticParser};
    use merman_model::source::SourceSpan;

    let source = format!(
        "\u{feff}---\r\ntitle: Before\r\n---\r\n%%{{init: {{}}}}%%\r\n%% prior\r\n \tagentflow-beta\r\n  A ==> B\r\n%% {}\r\n",
        "x".repeat(50_001)
    );
    let error = SemanticParser::new()
        .parse_diagram_sync(&source)
        .unwrap_err();
    let Error::DiagramParse { diagnostic, .. } = error else {
        panic!("expected a structured Agentflow parse error");
    };
    let start = source.find("A ==> B").unwrap();
    assert_eq!(
        diagnostic.span(),
        Some(SourceSpan::new(start, start + "A ==> B".len()))
    );
}

#[test]
fn agentflow_positions_skip_bom_and_count_unicode_as_utf16() {
    let source = "\u{feff}agentflow-beta\r\na[\"中文😀\"]; b@{ shape: cloud }\r\n";
    let diagnostics = diagnostics(source);
    assert_eq!(diagnostics[0]["position"]["startLine"], 2);
    assert_eq!(diagnostics[0]["position"]["startColumn"], 11);
    assert_eq!(diagnostics[0]["position"]["endColumn"], 28);
    assert_eq!(diagnostics[0]["position"]["startIndex"], 0);
    assert_eq!(diagnostics[0]["position"]["endIndex"], 0);
}
