use super::*;

#[test]
fn parse_styles_matches_quadrantdb_spec() {
    let styles = vec![
        "radius: 10".to_string(),
        "color: #ff0000".to_string(),
        "stroke-color: #ff00ff".to_string(),
        "stroke-width: 10px".to_string(),
    ];
    let obj = parse_styles(&styles).unwrap();
    assert_eq!(obj.radius, Some(10.0));
    assert_eq!(obj.color.as_deref(), Some("#ff0000"));
    assert_eq!(obj.stroke_color.as_deref(), Some("#ff00ff"));
    assert_eq!(obj.stroke_width.as_deref(), Some("10px"));

    let err = parse_styles(&["test_name: value".to_string()]).unwrap_err();
    assert!(err.contains("style named test_name is not supported."));

    let obj = parse_styles(&[]).unwrap();
    assert_eq!(obj.radius, None);
    assert!(obj.color.is_none());

    let err = parse_styles(&["radius: f".to_string()]).unwrap_err();
    assert!(err.contains("value for radius f is invalid, please use a valid number"));
}

#[test]
fn unclosed_tokens_replay_safe_lines_with_a_linear_scan_bound() {
    for open in ["\"", "\"`"] {
        for line_count in [1, 64, 512] {
            let broken = format!("quadrant-1 {open}broken");
            let source = format!(
                "{broken}\n{}",
                "x-axis Low --> High\nP: [0.5, 0.5]\n".repeat(line_count)
            );
            let mut statements = QuadrantStatements::new(&source);
            let control = OperationControl::new();
            let first = statements.next_statement(&control).unwrap().unwrap();
            assert_eq!(first.source.text, broken);
            for _ in 0..line_count {
                let axis = statements.next_statement(&control).unwrap().unwrap();
                assert_eq!(axis.source.text, "x-axis Low --> High");
                let point = statements.next_statement(&control).unwrap().unwrap();
                assert_eq!(point.source.text, "P: [0.5, 0.5]");
            }
            assert!(statements.next_statement(&control).unwrap().is_none());
            assert!(statements.scanned_bytes <= source.len() * 2);

            let mut statements = QuadrantStatements::new(&source);
            statements.next_statement(&control).unwrap().unwrap();
            control.cancel();
            assert!(statements.next_statement(&control).is_err());
        }
    }
}

#[test]
fn multiline_token_slices_retain_crlf_and_literal_backslashes() {
    for ending in ["\n", "\r\n"] {
        for (open, close) in [("\"", "\""), ("\"`", "`\"")] {
            let payload = format!("First{ending}Second\\nLiteral");
            let source = format!("{open}{payload}{close}; next");
            let mut statements = QuadrantStatements::new(&source);
            let statement = statements
                .next_statement(&OperationControl::new())
                .unwrap()
                .unwrap();
            let parsed = parse_text_slice(statement.source).unwrap();

            assert_eq!(parsed.value, payload);
            assert_eq!(parsed.content.text, payload);
            assert_eq!(parsed.content.start, open.len());
            assert_eq!(parsed.content.end(), open.len() + payload.len());
        }
    }
}

#[test]
fn long_raw_lines_tokens_and_comments_observe_scanning_cancellation() {
    for prefix in ["title ", "\"", "\"`", "%% ", "accTitle "] {
        let source = format!("{prefix}{}", " ".repeat(8_192));
        let mut statements = QuadrantStatements::new(&source);
        let control = OperationControl::new();
        control.cancel_after_checkpoints(2);
        assert!(statements.next_statement(&control).is_err(), "{prefix}");
        assert!(statements.offset < source.len(), "{prefix}");
    }
}
