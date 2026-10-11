use super::*;

// These named copied oracles encode label/lifeline overlap or missing reading clearance.
// Keep their bytes immutable and compare against separately captured corrected output.
const CORRECTED_UNICODE_FIXTURES: &[(&str, &str)] = &[
    (
        "dotted_arrows_only.txt",
        include_str!("corrected-fixtures/dotted_arrows_only.txt"),
    ),
    (
        "four_participants.txt",
        include_str!("corrected-fixtures/four_participants.txt"),
    ),
    (
        "multiword_labels.txt",
        include_str!("corrected-fixtures/multiword_labels.txt"),
    ),
    (
        "self_message.txt",
        include_str!("corrected-fixtures/self_message.txt"),
    ),
    (
        "three_participants.txt",
        include_str!("corrected-fixtures/three_participants.txt"),
    ),
];

fn assert_corrected_fixture_geometry(input: &str, rendered: &str) {
    use merman_core::diagrams::sequence::SequenceMessageStroke;

    let model = parse_sequence_render_model(input);
    let rows = rendered
        .lines()
        .map(|row| row.chars().collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let centers = rows[2]
        .iter()
        .enumerate()
        .filter_map(|(column, glyph)| (*glyph == '┬').then_some(column))
        .collect::<Vec<_>>();
    assert_eq!(
        centers.len(),
        model.actor_order.len(),
        "every copied actor must remain visible:\n{rendered}"
    );
    for message in &model.messages {
        let label = message.message_text();
        assert!(
            label.is_ascii() && !label.is_empty(),
            "these named fixtures contain plain ASCII labels"
        );
        assert_eq!(
            rendered.matches(label).count(),
            1,
            "every authored label must occur once:\n{rendered}"
        );
        let label_row = rendered
            .lines()
            .position(|row| row.contains(label))
            .unwrap();
        for center in &centers {
            assert_eq!(
                rows[label_row].get(*center),
                Some(&'│'),
                "label {label:?} must preserve every actor lifeline:\n{rendered}"
            );
        }
        let actor_index = |actor: Option<&str>| {
            model
                .actor_order
                .iter()
                .position(|candidate| Some(candidate.as_str()) == actor)
                .expect("a copied signal must have a known endpoint")
        };
        let from = actor_index(message.from.as_deref());
        let to = actor_index(message.to.as_deref());
        let left_actor = from.min(to);
        let start = centers[left_actor] + 2;
        assert!(
            rows[label_row]
                .iter()
                .skip(start)
                .take(label.len())
                .copied()
                .eq(label.chars()),
            "text must occupy its selected interval without implicit wrapping:\n{rendered}"
        );
        let label_end = start + label.len();
        let next_lifeline = centers
            .get(left_actor + 1)
            .copied()
            .unwrap_or(rows[label_row].len());
        assert!(
            label_end < next_lifeline,
            "text must fit before the next actor, including nonadjacent messages:\n{rendered}"
        );
        assert_eq!(
            rows[label_row].get(label_end),
            Some(&' '),
            "a blank reading cell must separate text from the next lifeline:\n{rendered}"
        );
        let stroke = match message
            .signal_semantics()
            .expect("fixture must contain a signal")
            .stroke
        {
            SequenceMessageStroke::Solid => '─',
            SequenceMessageStroke::Dotted => '┈',
        };
        let arrow = &rows[label_row + 1];
        let source = centers[from];
        let target = centers[to];
        if source < target {
            assert_eq!(arrow[source], '├', "forward source must own its junction");
            assert_eq!(
                arrow[target], '│',
                "forward target lifeline must remain visible"
            );
            assert_eq!(
                arrow[target - 1],
                '►',
                "forward head must enter the declared target"
            );
            assert!(
                arrow[source + 1..target - 1]
                    .iter()
                    .all(|glyph| *glyph == stroke),
                "the forward shaft must remain continuous:\n{rendered}"
            );
        } else if source > target {
            assert_eq!(arrow[source], '┤', "reverse source must own its junction");
            assert_eq!(
                arrow[target], '│',
                "reverse target lifeline must remain visible"
            );
            assert_eq!(
                arrow[target + 1],
                '◄',
                "reverse head must enter the declared target"
            );
            assert!(
                arrow[target + 2..source]
                    .iter()
                    .all(|glyph| *glyph == stroke),
                "the reverse shaft must remain continuous:\n{rendered}"
            );
        } else {
            assert_eq!(arrow[source], '├', "self-loop source must own its junction");
            let right = arrow
                .iter()
                .position(|glyph| *glyph == '┐')
                .expect("self loop must turn down");
            assert!(
                right + 1 < next_lifeline,
                "self-loop geometry must clear the next actor"
            );
            assert_eq!(
                rows[label_row + 2][right],
                '│',
                "self loop must retain its vertical segment"
            );
            assert_eq!(
                rows[label_row + 3][right],
                '┘',
                "self loop must turn back to its actor"
            );
            assert_eq!(
                rows[label_row + 3][source + 1],
                '◄',
                "self loop must return to its declared actor"
            );
            for row in &rows[label_row + 1..=label_row + 3] {
                for (actor, center) in centers.iter().enumerate() {
                    if actor != from {
                        assert_eq!(
                            row[*center], '│',
                            "self-loop geometry must preserve foreign lifelines"
                        );
                    }
                }
            }
        }
    }
}

fn assert_copied_sequence_fixtures(group: &str, options: &AsciiRenderOptions) {
    let mut mismatches = Vec::new();
    let mut corrected_count = 0;
    let mut exact_count = 0;
    for path in fixture_cases(group) {
        let (input, copied) = split_fixture(&path);
        let rendered = render_sequence(&input, options)
            .unwrap_or_else(|err| panic!("{} failed: {err}", path.display()));
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("copied filename must be UTF-8");
        let corrected = (group == "sequence")
            .then(|| {
                CORRECTED_UNICODE_FIXTURES
                    .iter()
                    .find(|(candidate, _)| *candidate == name)
            })
            .flatten();
        let expected = if let Some((_, corrected)) = corrected {
            corrected_count += 1;
            assert_ne!(
                normalize_sequence_output(corrected),
                normalize_sequence_output(&copied),
                "named disposition must still represent a verified output difference"
            );
            assert_corrected_fixture_geometry(&input, &rendered);
            *corrected
        } else {
            exact_count += 1;
            copied.as_str()
        };
        if normalize_sequence_output(&rendered) != normalize_sequence_output(expected) {
            mismatches.push(format!(
                "{}\nACTUAL:\n{rendered}\nEXPECTED:\n{expected}",
                path.display()
            ));
        }
    }
    assert_eq!(
        (exact_count, corrected_count),
        if group == "sequence" { (7, 5) } else { (5, 0) },
        "the exact and corrected subsets must cover their immutable corpus"
    );
    assert!(
        mismatches.is_empty(),
        "all copied fixture differences:\n{}",
        mismatches.join("\n\n")
    );
}

#[test]
fn sequence_golden_unicode_fixtures_match_upstream() {
    assert_copied_sequence_fixtures("sequence", &AsciiRenderOptions::unicode());
}

#[test]
fn sequence_golden_ascii_fixtures_match_upstream() {
    assert_copied_sequence_fixtures("sequence-ascii", &AsciiRenderOptions::ascii());
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
