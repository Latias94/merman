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

#[test]
fn sequence_message_html_breaks_preserve_rows_lifelines_and_endpoint_markers() {
    for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
        for wrap in ["", "wrap:"] {
            for html_break in ["<br>", "<br/>", "<br />"] {
                for (signal, target_actor, points_right, marker_row_offset) in [
                    ("A->>C", 2, true, 1),
                    ("C->>A", 0, false, 1),
                    ("A->>A", 0, false, 3),
                    ("C->>C", 2, false, 3),
                ] {
                    let source = format!(
                        "sequenceDiagram\nparticipant A\nparticipant B\nparticipant C\n{signal}: {wrap}\"One\"{html_break}\"Two\""
                    );
                    let rendered = render_sequence(&source, &options)
                        .expect("message HTML breaks should render");
                    let first = first_line_index_containing(&rendered, "\"One\"");
                    let second = first_line_index_containing(&rendered, "\"Two\"");
                    assert_eq!(
                        second,
                        first + 1,
                        "HTML breaks must separate complete quoted label rows:\n{rendered}"
                    );
                    assert!(!rendered.contains("<br"), "{rendered}");
                    for label in ["\"One\"", "\"Two\""] {
                        assert_label_lifelines(&rendered, label, &['A', 'B', 'C'], &options);
                    }

                    let centers = actor_columns(&rendered, &['A', 'B', 'C']);
                    let marker_column = if points_right {
                        centers[target_actor] - 1
                    } else {
                        centers[target_actor] + 1
                    };
                    let marker = match (options.charset, points_right) {
                        (merman_ascii::AsciiCharset::Ascii, true) => '>',
                        (merman_ascii::AsciiCharset::Ascii, false) => '<',
                        (merman_ascii::AsciiCharset::Unicode, true) => '►',
                        (merman_ascii::AsciiCharset::Unicode, false) => '◄',
                        _ => panic!("test needs a known character set"),
                    };
                    let marker_row = rendered
                        .lines()
                        .nth(second + marker_row_offset)
                        .expect("the message marker must follow its complete label");
                    assert_eq!(
                        glyph_at_column(marker_row, marker_column, options.terminal_width_profile,),
                        Some(marker),
                        "HTML breaks must retain message direction and self-loop ordering:\n{rendered}"
                    );
                }
            }
        }
    }
}

#[test]
fn sequence_message_html_breaks_preserve_numbering_literal_escapes_and_terminal_safety() {
    let mut model = parse_sequence_render_model(
        "sequenceDiagram\nautonumber\nparticipant A\nparticipant B\nA->>B: First\nB-->>A: Next",
    );
    let first_message = model
        .messages
        .iter_mut()
        .find(|message| message.from.is_some())
        .expect("the typed model must contain a signal");
    first_message.message =
        SequenceMessagePayload::Text("\"One\nline\"<br/>\"Two\\n\u{1b}[31m\u{202e}\"".to_string());

    for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
        let rendered = render_sequence_model(&model, &options)
            .expect("HTML breaks must retain terminal-safe message spelling");
        let first_label = "1. \"One\\u{A}line\"";
        let second_label = "\"Two\\n\\u{1B}[31m\\u{202E}\"";
        let first = first_line_index_containing(&rendered, first_label);
        let second = first_line_index_containing(&rendered, second_label);
        assert_eq!(second, first + 1, "{rendered}");
        for label in [first_label, second_label, "2. Next"] {
            assert_label_lifelines(&rendered, label, &['A', 'B'], &options);
        }
        assert!(
            first_line_index_containing(&rendered, "2. Next") > second + 1,
            "the next message must follow the first message's labels and arrow:\n{rendered}"
        );
        assert!(!rendered.contains(['\u{1b}', '\u{202e}']), "{rendered:?}");
    }
}

#[test]
fn sequence_message_html_breaks_preserve_leading_consecutive_and_trailing_empty_rows() {
    let options = AsciiRenderOptions::ascii();
    for wrap in ["", "wrap:"] {
        let source = format!(
            "sequenceDiagram\nparticipant A\nparticipant B\nA->>B: {wrap}<br/>One<br><br />Two<br/>"
        );
        let rendered =
            render_sequence(&source, &options).expect("empty authored message rows should render");
        let without_leading_break =
            render_sequence(&source.replacen("<br/>One", "One", 1), &options)
                .expect("the same message without its leading break should render");
        let rows = rendered.lines().collect::<Vec<_>>();
        let first = first_line_index_containing(&rendered, "One");
        let second = first_line_index_containing(&rendered, "Two");
        assert_eq!(
            first,
            first_line_index_containing(&without_leading_break, "One") + 1,
            "the leading authored break must add one label row:\n{rendered}"
        );
        assert_eq!(second, first + 2, "{rendered}");
        let centers = actor_columns(&rendered, &['A', 'B']);
        for row in [first - 1, first + 1, second + 1] {
            assert!(
                rows[row].chars().all(|ch| matches!(ch, ' ' | '|')),
                "authored empty label rows should contain only lifelines:\n{rendered}"
            );
            for center in &centers {
                assert_eq!(
                    glyph_at_column(rows[row], *center, options.terminal_width_profile),
                    Some('|'),
                    "{rendered}"
                );
            }
        }
        assert!(
            rows[second + 2].contains('>'),
            "the arrow must follow the trailing authored empty row:\n{rendered}"
        );
    }
}

#[test]
fn sequence_wrapping_keeps_whitespace_only_html_paragraphs() {
    for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
        for signal in ["A->>B", "B->>A", "A->>A"] {
            for separator in [" ", "   "] {
                for wrap in ["", "wrap:"] {
                    let source = format!(
                        "sequenceDiagram\nparticipant A\nparticipant B\n{signal}: {wrap}One<br/>{separator}<br/>Two"
                    );
                    let rendered = render_sequence(&source, &options).unwrap();
                    let first = first_line_index_containing(&rendered, "One");
                    let second = first_line_index_containing(&rendered, "Two");
                    assert_eq!(second, first + 2, "{rendered}");
                    let empty = rendered.lines().nth(first + 1).unwrap();
                    assert!(
                        empty.chars().all(|ch| matches!(ch, ' ' | '|' | '│')),
                        "{rendered}"
                    );
                    for center in actor_columns(&rendered, &['A', 'B']) {
                        let lifeline = if options.charset == merman_ascii::AsciiCharset::Ascii {
                            '|'
                        } else {
                            '│'
                        };
                        assert_eq!(
                            glyph_at_column(empty, center, options.terminal_width_profile),
                            Some(lifeline),
                            "{rendered}"
                        );
                    }
                }
            }
        }
    }
}
