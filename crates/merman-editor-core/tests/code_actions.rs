use merman_analysis::{DiagnosticFix, DiagnosticFixEdit, SourceMap};
use merman_editor_core::{Position, code_actions_from_fixes};

#[test]
fn code_action_planner_sorts_non_overlapping_edits() {
    let map = SourceMap::new("0123456789");
    let later = map.span(5, 6).unwrap();
    let earlier = map.span(1, 2).unwrap();
    let fix = DiagnosticFix::new(
        "Sort edits",
        vec![
            DiagnosticFixEdit::new(later, "late"),
            DiagnosticFixEdit::new(earlier, "early"),
        ],
    )
    .preferred();

    let actions = code_actions_from_fixes([&fix]);

    assert_eq!(actions.len(), 1);
    assert!(actions[0].is_preferred);
    assert_eq!(actions[0].title, "Sort edits");
    assert_eq!(actions[0].edits[0].range.start, Position::new(0, 1));
    assert_eq!(actions[0].edits[0].new_text, "early");
    assert_eq!(actions[0].edits[1].range.start, Position::new(0, 5));
    assert_eq!(actions[0].edits[1].new_text, "late");
}

#[test]
fn code_action_planner_rejects_overlapping_edits() {
    let map = SourceMap::new("0123456789");
    let left = map.span(0, 4).unwrap();
    let right = map.span(2, 5).unwrap();
    let fix = DiagnosticFix::new(
        "Reject overlapping edits",
        vec![
            DiagnosticFixEdit::new(right, "right"),
            DiagnosticFixEdit::new(left, "left"),
        ],
    );

    let actions = code_actions_from_fixes([&fix]);

    assert!(actions.is_empty());
}
