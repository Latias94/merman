use super::*;

#[test]
fn sequence_golden_unicode_fixtures_match_upstream() {
    for path in fixture_cases("sequence") {
        let (input, expected) = split_fixture(&path);
        let rendered = render_sequence(&input, &AsciiRenderOptions::unicode())
            .unwrap_or_else(|err| panic!("{} failed: {err}", path.display()));

        assert_eq!(
            normalize_sequence_output(&rendered),
            normalize_sequence_output(&expected),
            "{}",
            path.display()
        );
    }
}

#[test]
fn sequence_golden_ascii_fixtures_match_upstream() {
    for path in fixture_cases("sequence-ascii") {
        let (input, expected) = split_fixture(&path);
        let rendered = render_sequence(&input, &AsciiRenderOptions::ascii())
            .unwrap_or_else(|err| panic!("{} failed: {err}", path.display()));

        assert_eq!(
            normalize_sequence_output(&rendered),
            normalize_sequence_output(&expected),
            "{}",
            path.display()
        );
    }
}

#[test]
fn sequence_local_semantic_fixture_covers_dense_control_rows() {
    let input = read_local_semantic_fixture("sequence/dense_control_rows.mmd");

    let rendered = render_sequence(&input, &AsciiRenderOptions::unicode())
        .expect("dense local semantic sequence fixture should render");

    for expected in [
        "Outer Work",
        "Coordinate",
        "Parallel Branches",
        "Fallback",
        "Retry",
        "Stop",
    ] {
        assert!(
            rendered.contains(expected),
            "dense semantic sequence fixture should keep {expected:?} visible:\n{rendered}"
        );
    }
    assert!(
        rendered.contains('┃'),
        "dense semantic sequence fixture should keep active lifelines visible:\n{rendered}"
    );
    assert!(
        rendered.lines().count() >= 10,
        "dense semantic sequence fixture should produce a multi-line layout:\n{rendered}"
    );
}

#[test]
fn sequence_local_semantic_fixture_covers_self_messages_with_notes_and_alt_branch() {
    let input = read_local_semantic_fixture("sequence/self_messages_with_notes.mmd");

    let rendered = render_sequence(&input, &AsciiRenderOptions::unicode())
        .expect("self-message local semantic sequence fixture should render");

    for expected in [
        "Main Process",
        "Renderer",
        "3s Fallback Timer",
        "Multiple panels",
        "Single panel",
        "closePanel(focusedId)",
        "closePanel(lastId)",
        "Panel removed",
        "Stack becomes []",
        "Panel reopens",
        "window.destroy()",
    ] {
        assert!(
            rendered.contains(expected),
            "self-message semantic sequence fixture should keep {expected:?} visible:\n{rendered}"
        );
    }
    assert!(
        first_line_index_containing(&rendered, "Multiple panels")
            < first_line_index_containing(&rendered, "Single panel"),
        "alt branch order should remain readable in the semantic fixture:\n{rendered}"
    );
    assert!(
        first_line_index_containing(&rendered, "Panel removed")
            < first_line_index_containing(&rendered, "Panel reopens"),
        "branch-local note ordering should stay visible:\n{rendered}"
    );
    assert!(
        rendered.lines().count() >= 10,
        "self-message semantic sequence fixture should produce a multi-line layout:\n{rendered}"
    );
}

#[test]
fn sequence_local_semantic_fixture_covers_multiple_reference_messages() {
    let input = read_local_semantic_fixture("sequence/multiple_messages.mmd");
    let rendered = render_sequence(&input, &AsciiRenderOptions::ascii())
        .expect("local semantic multiple-message fixture should render");

    for expected in [
        "Alice", "Bob", "Charlie", "Hello", "Forward", "Reply", "Done",
    ] {
        assert!(
            rendered.contains(expected),
            "sequence multiple-message fixture should keep {expected:?} visible:\n{rendered}"
        );
    }

    assert!(
        first_line_index_containing(&rendered, "Hello")
            < first_line_index_containing(&rendered, "Forward"),
        "sequence messages should preserve source order before the cross-participant reply:\n{rendered}"
    );
    assert!(
        first_line_index_containing(&rendered, "Reply")
            < first_line_index_containing(&rendered, "Done"),
        "sequence replies should preserve source order:\n{rendered}"
    );
}
