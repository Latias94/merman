use super::*;

#[test]
fn requirement_direction_lookahead_reuses_same_line_suffix() {
    let source = " prefix direction LR accDescr{} direction BT";
    let mut lookahead = RequirementDirectionLookahead::default();
    let first = lookahead
        .parse(source, 0, source.len(), &OperationControl::new())
        .unwrap()
        .unwrap();
    assert_eq!(first.0.value, Some("BT"));
    let after = source.find("direction BT").unwrap() + "direction BT".len();
    assert!(
        lookahead
            .parse(source, after, source.len(), &OperationControl::new())
            .unwrap()
            .is_none()
    );
}

#[test]
fn requirement_accessibility_block_borrows_original_content_and_preserves_suffix() {
    let source = "accDescr\r\n\u{feff}{\r\n First  \r\n  Next #%%;\r\n}accTitle\n:Value\n";
    let mut cursor = LineCursor::new(source);
    cursor.next_line();
    let parsed = parse_requirement_accessibility(&mut cursor, 0, &OperationControl::new())
        .unwrap()
        .unwrap();
    let value = parsed.value.unwrap();
    assert_eq!(value.text, "First  \r\n  Next #%%;");
    assert_eq!(value.start, source.find("First").unwrap());
    assert_eq!(value.text.as_ptr(), source[value.start..].as_ptr());
    assert_eq!(parsed.statement_span.end, source.find('}').unwrap() + 1);
    assert!(parsed.complete);
    let suffix_start = parsed.statement_span.end;
    assert_eq!(cursor.next_line(), Some(("accTitle", suffix_start)));
    let suffix =
        parse_requirement_accessibility(&mut cursor, suffix_start, &OperationControl::new())
            .unwrap()
            .unwrap();
    assert_eq!(suffix.value.unwrap().text, "Value");
    assert_eq!(cursor.next_line(), Some(("", source.len() - 1)));
    assert!(cursor.next_line().is_none());
}

#[test]
fn requirement_accessibility_scans_observe_cancellation_inside_long_tokens() {
    for source in [
        format!("accTitle{}:Value", "\u{feff}\n".repeat(4096)),
        format!("accTitle:{}", "\u{0085}".repeat(8192)),
        format!("accDescr{{{}", "accDescr{\n".repeat(1024)),
        format!("accDescr{{{}}}", "\u{0085}".repeat(8192)),
    ] {
        let control = OperationControl::new();
        control.cancel_after_checkpoints(5);
        assert!(
            parse_requirement_accessibility(&mut LineCursor::new(&source), 0, &control).is_err()
        );
    }
}

#[test]
fn requirement_unclosed_accessibility_owns_recovery_tail_once() {
    let source = format!("accDescr{{\n{}", "accDescr{\n".repeat(512));
    let control = OperationControl::new();
    control.cancel_after_checkpoints(128);
    let mut cursor = LineCursor::new(&source);
    cursor.next_line();
    let parsed = parse_requirement_accessibility(&mut cursor, 0, &control)
        .unwrap()
        .unwrap();
    assert!(!parsed.complete);
    assert_eq!(parsed.statement_span, SourceSpan::new(0, source.len()));
    assert_eq!(cursor.offset(), source.len());
    assert!(cursor.next_line().is_none());
    assert!(
        parsed
            .value
            .unwrap()
            .text
            .starts_with("accDescr{\naccDescr{")
    );
}
