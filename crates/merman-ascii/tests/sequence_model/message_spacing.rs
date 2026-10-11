use super::*;
use merman_ascii::AsciiLayoutProfile;
use unicode_width::UnicodeWidthChar;

fn actor_columns(rendered: &str, actors: &[char]) -> Vec<usize> {
    let names = rendered
        .lines()
        .find(|row| actors.iter().all(|actor| row.contains(*actor)))
        .expect("participant labels must share their header row");
    actors
        .iter()
        .map(|actor| names.chars().position(|ch| ch == *actor).unwrap())
        .collect()
}

fn glyph_at_column(row: &str, column: usize, profile: TerminalWidthProfile) -> Option<char> {
    let mut x = 0;
    for ch in row.chars() {
        if x == column {
            return Some(ch);
        }
        x += match profile {
            TerminalWidthProfile::Unicode => ch.width().unwrap_or(0),
            TerminalWidthProfile::Cjk => ch.width_cjk().unwrap_or(0),
            _ => panic!("test needs a known terminal width profile"),
        };
    }
    None
}

fn assert_label_lifelines(
    rendered: &str,
    label: &str,
    actors: &[char],
    options: &AsciiRenderOptions,
) {
    let row = rendered
        .lines()
        .find(|row| row.contains(label))
        .unwrap_or_else(|| panic!("missing complete label {label:?}:\n{rendered}"));
    let lifeline = if options.charset == merman_ascii::AsciiCharset::Unicode {
        '│'
    } else {
        '|'
    };
    for center in actor_columns(rendered, actors) {
        assert_eq!(
            glyph_at_column(row, center, options.terminal_width_profile),
            Some(lifeline),
            "label {label:?} must preserve the lifeline at column {center}:\n{rendered}"
        );
    }
}

#[test]
fn sequence_long_message_labels_preserve_adjacent_and_intermediate_lifelines() {
    for mut options in [AsciiRenderOptions::unicode(), AsciiRenderOptions::ascii()] {
        for profile in [AsciiLayoutProfile::Canonical, AsciiLayoutProfile::Compact] {
            options.layout_profile = profile;
            for signal in ["A->>B", "B->>A", "A->>C", "C->>A"] {
                let source = format!(
                    "sequenceDiagram\nparticipant A\nparticipant B\nparticipant C\n{signal}: find user by email"
                );
                let rendered =
                    render_sequence(&source, &options).expect("long message must render");
                assert_label_lifelines(&rendered, "find user by email", &['A', 'B', 'C'], &options);
            }
        }
    }
}

#[test]
fn sequence_long_self_messages_preserve_next_actor_and_loop_clearance() {
    for options in [AsciiRenderOptions::unicode(), AsciiRenderOptions::ascii()] {
        let rendered = render_sequence(
            "sequenceDiagram\nparticipant A\nparticipant B\nA->>A: find user by email\nB->>B: last actor message",
            &options,
        ).expect("self messages must have a label host");
        for label in ["find user by email", "last actor message"] {
            assert_label_lifelines(&rendered, label, &['A', 'B'], &options);
        }
        let center = actor_columns(&rendered, &['A', 'B'])[1];
        let rows = rendered.lines().collect::<Vec<_>>();
        let label_row = first_line_index_containing(&rendered, "find user by email");
        let lifeline = if options.charset == merman_ascii::AsciiCharset::Unicode {
            '│'
        } else {
            '|'
        };
        for row in &rows[label_row + 1..label_row + 4] {
            assert_eq!(
                glyph_at_column(row, center, options.terminal_width_profile),
                Some(lifeline),
                "a neighbouring actor must remain outside the self loop:\n{rendered}"
            );
        }
    }
}

#[test]
fn sequence_numbered_and_normalized_messages_use_the_measured_spelling() {
    let options = AsciiRenderOptions::unicode();
    let mut model = parse_sequence_render_model(
        "sequenceDiagram\nautonumber\nparticipant A\nparticipant B\nA->>B: find user by email",
    );
    model
        .messages
        .iter_mut()
        .find(|message| message.from.is_some())
        .expect("the typed model must contain a signal")
        .message = SequenceMessagePayload::Text("find user\nby email".to_string());
    let rendered = render_sequence_model(&model, &options)
        .expect("numbered terminal-normalized label must render");
    assert_label_lifelines(
        &rendered,
        "1. find user\\u{A}by email",
        &['A', 'B'],
        &options,
    );
}

#[test]
fn sequence_wide_messages_keep_lifelines_in_both_width_profiles() {
    for profile in [TerminalWidthProfile::Unicode, TerminalWidthProfile::Cjk] {
        let mut options = AsciiRenderOptions::ascii();
        options.terminal_width_profile = profile;
        let rendered = render_sequence(
            "sequenceDiagram\nparticipant A\nparticipant B\nparticipant C\nC->>A: ·数据数据数据数据",
            &options,
        ).expect("wide message label must render");
        assert_label_lifelines(&rendered, "·数据数据数据数据", &['A', 'B', 'C'], &options);
    }
}

#[test]
fn sequence_explicit_wrap_uses_one_free_interval_and_retains_wrapping() {
    for signal in ["A->>C", "C->>A", "A->>A"] {
        let options = AsciiRenderOptions::unicode();
        let rendered = render_sequence(
            &format!("sequenceDiagram\nparticipant A\nparticipant B\nparticipant C\n{signal}: wrap:find user by email"),
            &options,
        ).expect("explicit wrap must remain supported");
        assert!(
            !rendered.contains("find user by email"),
            "wrap must not widen to natural label width:\n{rendered}"
        );
        let baseline = render_sequence(
            "sequenceDiagram\nparticipant A\nparticipant B\nparticipant C\nA->>C:",
            &options,
        )
        .expect("empty-label comparison must render");
        assert_eq!(
            actor_columns(&rendered, &['A', 'B', 'C']),
            actor_columns(&baseline, &['A', 'B', 'C']),
            "wrapped labels must retain the preferred participant spacing"
        );
        for word in ["find", "user", "by", "email"] {
            assert_label_lifelines(&rendered, word, &['A', 'B', 'C'], &options);
        }
    }
}
