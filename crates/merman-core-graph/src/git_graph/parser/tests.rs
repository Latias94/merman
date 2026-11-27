use super::*;

#[test]
fn gitgraph_statement_scan_can_cancel_inside_a_multiline_string() {
    let source = format!("commit tag:\"first\n{}\"\n", "x".repeat(32_768));
    let control = OperationControl::new();
    control.cancel_after_checkpoints(3);
    assert!(matches!(
        gitgraph_statement(&source, 0, &control),
        Err(OperationCancelled { .. })
    ));
}

#[test]
fn gitgraph_quoted_converter_decodes_langium_escapes_and_preserves_source_spans() {
    let control = OperationControl::new();
    for quote in ['\'', '"'] {
        let raw = r#"第一\b\f\n\r\t\v\0\q\\n\"\'"#;
        let source = format!("{quote}{raw}{quote}");
        let mut parser = LineParser::new(&source).with_base(11);
        let parsed = parser.parse_quoted_spanned(&control).unwrap().unwrap();
        assert_eq!(parsed.text, "第一\u{0008}\u{000c}\n\r\t\u{000b}\0q\\n\"'");
        assert_eq!(parsed.span, SourceSpan::new(12, 12 + raw.len()));
        assert!(parser.is_eof());
    }
}

#[test]
fn gitgraph_parser_keeps_raw_cr_and_crlf_boundaries() {
    // The engine normalizes CR before parsing; the family still owns exact raw EOL boundaries.
    let control = OperationControl::new();
    assert!(matches!(
        parse_git_graph_command(
            "commit id:\"C\"\r",
            0,
            #[cfg(feature = "editor-facts")]
            false,
            &control,
        ),
        Err(GitGraphCommandParseAbort::Invalid(_))
    ));
    assert_eq!(
        gitgraph_statement("commit\r\n", 0, &control).unwrap(),
        ("commit", 8)
    );
}
