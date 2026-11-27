use super::*;

#[test]
fn venn_adjacent_recovery_bounds_failed_payload_lookahead() {
    for count in [256, 2048] {
        for (opener, suffix) in [("[[", ""), ("[[", "\""), ("[\"", ""), ("rgb(", "")] {
            let source = format!(
                "venn-beta\nset A {}{suffix}\nset Later\n",
                opener.repeat(count),
            );
            let mut cursor = VennCursor::new(&source);
            while cursor.peek().kind != VennTokenKind::End {
                cursor.take();
            }
            assert!(
                cursor.inspected_terminal_bytes <= source.len() * 2,
                "payload lookahead rescanned a suffix: {opener:?}, {count}, {} bytes inspected",
                cursor.inspected_terminal_bytes,
            );
            let outcome = parse_venn_source_controlled(&source, &OperationControl::new()).unwrap();
            assert!(outcome.first_issue.is_some());
            assert_eq!(outcome.model.subsets.last().unwrap().sets, ["Later"]);
        }
    }
}
