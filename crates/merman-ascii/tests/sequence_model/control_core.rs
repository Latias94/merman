use super::*;
use unicode_width::UnicodeWidthStr;

#[test]
fn sequence_control_blocks_are_core_control_signals() {
    struct Case {
        name: &'static str,
        input: &'static str,
        signals: &'static [(i32, &'static str)],
    }

    let cases = [
        Case {
            name: "loop",
            input: "sequenceDiagram\nparticipant A\nparticipant B\nloop Every minute\nA->>B: Ping\nend",
            signals: &[
                (LINETYPE_LOOP_START, "Every minute"),
                (LINETYPE_LOOP_END, ""),
            ],
        },
        Case {
            name: "opt",
            input: "sequenceDiagram\nparticipant A\nparticipant B\nopt A is ready\nA->>B: Send\nend",
            signals: &[(LINETYPE_OPT_START, "A is ready"), (LINETYPE_OPT_END, "")],
        },
        Case {
            name: "break",
            input: "sequenceDiagram\nparticipant A\nparticipant B\nbreak Failure\nA->>B: Stop\nend",
            signals: &[(LINETYPE_BREAK_START, "Failure"), (LINETYPE_BREAK_END, "")],
        },
        Case {
            name: "alt",
            input: "sequenceDiagram\nparticipant A\nparticipant B\nalt Success\nA->>B: OK\nelse Failure\nB-->>A: Retry\nend",
            signals: &[
                (LINETYPE_ALT_START, "Success"),
                (LINETYPE_ALT_ELSE, "Failure"),
                (LINETYPE_ALT_END, ""),
            ],
        },
        Case {
            name: "par",
            input: "sequenceDiagram\nparticipant A\nparticipant B\npar First\nA->>B: One\nand Second\nB-->>A: Two\nend",
            signals: &[
                (LINETYPE_PAR_START, "First"),
                (LINETYPE_PAR_AND, "Second"),
                (LINETYPE_PAR_END, ""),
            ],
        },
        Case {
            name: "critical",
            input: "sequenceDiagram\nparticipant A\nparticipant B\ncritical Must lock\nA->>B: Lock\noption Timeout\nB-->>A: Backoff\nend",
            signals: &[
                (LINETYPE_CRITICAL_START, "Must lock"),
                (LINETYPE_CRITICAL_OPTION, "Timeout"),
                (LINETYPE_CRITICAL_END, ""),
            ],
        },
    ];

    for case in cases {
        let model = parse_sequence_render_model(case.input);
        let control_messages = model
            .messages
            .iter()
            .filter(|message| message.from.is_none() && message.to.is_none())
            .collect::<Vec<_>>();

        assert_eq!(
            control_messages.len(),
            case.signals.len(),
            "{} should have expected control marker count",
            case.name
        );

        let actual = control_messages
            .iter()
            .map(|message| (message.message_type, message.message_text()))
            .collect::<Vec<_>>();
        assert_eq!(
            actual, case.signals,
            "{} should preserve core control line types and labels",
            case.name
        );
        assert!(
            model
                .messages
                .iter()
                .any(|message| message.from.is_some() && message.to.is_some()),
            "{} should still include drawable messages inside the block",
            case.name
        );
    }
}

#[test]
fn sequence_single_section_control_blocks_render_unicode_frames() {
    let cases = [
        (
            "sequenceDiagram\nparticipant A\nparticipant B\nloop Every minute\nA->>B: Ping\nend",
            "loop",
            "Every minute",
            "Ping",
        ),
        (
            "sequenceDiagram\nparticipant A\nparticipant B\nopt A is ready\nA->>B: Send\nend",
            "opt",
            "A is ready",
            "Send",
        ),
        (
            "sequenceDiagram\nparticipant A\nparticipant B\nbreak Failure\nA->>B: Stop\nend",
            "break",
            "Failure",
            "Stop",
        ),
    ];

    for (input, keyword, label, message_label) in cases {
        let rendered = render_sequence(input, &AsciiRenderOptions::unicode())
            .unwrap_or_else(|err| panic!("{keyword} should render: {err}"));

        assert!(
            rendered
                .lines()
                .any(|line| line.starts_with(&format!("┌ {keyword} {label} "))),
            "{keyword} should render a labeled Unicode top frame:\n{rendered}"
        );
        assert!(
            rendered
                .lines()
                .any(|line| line.starts_with('│') && line.contains(message_label)),
            "{keyword} should keep contained rows inside the Unicode frame:\n{rendered}"
        );
        assert!(
            rendered.lines().any(|line| line.starts_with('└')),
            "{keyword} should render a Unicode bottom frame:\n{rendered}"
        );
    }
}

#[test]
fn sequence_single_section_control_blocks_render_ascii_frames() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\nloop Every minute\nA->>B: Ping\nend",
        &AsciiRenderOptions::ascii(),
    )
    .expect("loop should render with ASCII charset");

    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with("+ loop Every minute ")),
        "loop should render a labeled ASCII top frame:\n{rendered}"
    );
    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with('|') && line.contains("Ping")),
        "loop should keep contained rows inside the ASCII frame:\n{rendered}"
    );
    assert!(
        rendered.lines().any(|line| line.starts_with('+')),
        "loop should render an ASCII bottom frame:\n{rendered}"
    );
}

#[test]
fn sequence_single_section_control_blocks_frame_notes() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\nloop Watch\nNote over A,B: Wait\nA->>B: Continue\nend",
        &AsciiRenderOptions::unicode(),
    )
    .expect("loop should frame notes and messages");

    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with('│') && line.contains("Wait")),
        "loop should keep note rows inside the frame:\n{rendered}"
    );
    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with('│') && line.contains("Continue")),
        "loop should keep later message rows inside the same frame:\n{rendered}"
    );
}

#[test]
fn sequence_sectioned_control_blocks_render_unicode_frames() {
    let cases = [
        (
            "sequenceDiagram\nparticipant A\nparticipant B\nalt Success\nA->>B: OK\nelse Failure\nB-->>A: Retry\nend",
            "alt",
            "Success",
            "else",
            "Failure",
            "OK",
            "Retry",
        ),
        (
            "sequenceDiagram\nparticipant A\nparticipant B\npar First\nA->>B: One\nand Second\nB-->>A: Two\nend",
            "par",
            "First",
            "and",
            "Second",
            "One",
            "Two",
        ),
        (
            "sequenceDiagram\nparticipant A\nparticipant B\ncritical Must lock\nA->>B: Lock\noption Timeout\nB-->>A: Backoff\nend",
            "critical",
            "Must lock",
            "option",
            "Timeout",
            "Lock",
            "Backoff",
        ),
    ];

    for (input, keyword, label, separator, separator_label, first, second) in cases {
        let rendered = render_sequence(input, &AsciiRenderOptions::unicode())
            .unwrap_or_else(|err| panic!("{keyword} should render: {err}"));

        assert!(
            rendered
                .lines()
                .any(|line| line.starts_with(&format!("┌ {keyword} {label} "))),
            "{keyword} should render a labeled Unicode top frame:\n{rendered}"
        );
        assert!(
            rendered
                .lines()
                .any(|line| line.starts_with(&format!("├ {separator} {separator_label} "))),
            "{keyword} should render a labeled Unicode section separator:\n{rendered}"
        );
        assert!(
            rendered
                .lines()
                .any(|line| line.starts_with('│') && line.contains(first)),
            "{keyword} should keep first section rows inside the frame:\n{rendered}"
        );
        assert!(
            rendered
                .lines()
                .any(|line| line.starts_with('│') && line.contains(second)),
            "{keyword} should keep second section rows inside the frame:\n{rendered}"
        );
    }
}

#[test]
fn sequence_sectioned_control_blocks_render_ascii_frames() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\nalt Success\nA->>B: OK\nelse Failure\nB-->>A: Retry\nend",
        &AsciiRenderOptions::ascii(),
    )
    .expect("alt should render with ASCII charset");

    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with("+ alt Success ")),
        "alt should render a labeled ASCII top frame:\n{rendered}"
    );
    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with("+ else Failure ")),
        "alt should render a labeled ASCII section separator:\n{rendered}"
    );
    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with('|') && line.contains("Retry")),
        "alt should keep second section rows inside the ASCII frame:\n{rendered}"
    );
}

#[test]
fn sequence_sectioned_control_blocks_frame_multiple_sections_and_notes() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\nalt Primary path\nA->>B: First\nelse Secondary path\nNote over A,B: Wait\nelse Tertiary path\nB-->>A: Third\nend",
        &AsciiRenderOptions::unicode(),
    )
    .expect("alt should render multiple sections and notes");

    for marker in ["├ else Secondary path ", "├ else Tertiary path "] {
        assert!(
            rendered.lines().any(|line| line.starts_with(marker)),
            "alt should render every section separator:\n{rendered}"
        );
    }
    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with('│') && line.contains("Wait")),
        "alt should keep note rows inside sectioned frames:\n{rendered}"
    );
    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with('│') && line.contains("Third")),
        "alt should keep later section messages inside the frame:\n{rendered}"
    );
}

#[test]
fn sequence_nested_loop_inside_alt_keeps_frame_padding() {
    let input = "sequenceDiagram
    participant Client
    participant API
    participant Worker
    Client->>API: Submit job
    alt Valid request
      API->>Worker: Queue work
      loop Poll status
        Client->>API: GET /jobs/123
        API-->>Client: Running
      end
    else Invalid request
      API-->>Client: 400 Bad Request
    end";

    let rendered = render_sequence(input, &AsciiRenderOptions::ascii())
        .expect("nested loop inside alt should render");

    let loop_top = rendered
        .lines()
        .find(|line| line.contains("loop Poll status"))
        .unwrap_or_else(|| panic!("loop frame should render:\n{rendered}"));
    let outer_top = rendered
        .lines()
        .find(|line| line.contains("alt Valid request"))
        .unwrap_or_else(|| panic!("outer alt frame should render:\n{rendered}"));
    let outer_left = outer_top
        .find('+')
        .unwrap_or_else(|| panic!("outer alt frame should have a left border:\n{rendered}"));
    let loop_left = loop_top
        .find('+')
        .unwrap_or_else(|| panic!("nested loop should have a left border:\n{rendered}"));
    assert!(
        loop_left >= outer_left + 2,
        "nested loop frame should not touch the parent frame border:\n{rendered}"
    );
    let submit_label = text_display_column(&rendered, "Submit job");
    let nested_label = text_display_column(&rendered, "GET /jobs/123");
    assert!(
        submit_label == nested_label,
        "nested loop body should keep participant lifelines aligned with the outer frame:\n{rendered}"
    );

    let rendered = render_sequence(input, &AsciiRenderOptions::unicode())
        .expect("nested loop inside alt should render as Unicode");
    let submit_label = text_display_column(&rendered, "Submit job");
    let nested_label = text_display_column(&rendered, "GET /jobs/123");
    assert!(
        submit_label == nested_label,
        "nested Unicode loop body should keep participant lifelines aligned with the outer frame:\n{rendered}"
    );
}

fn text_display_column(rendered: &str, text: &str) -> usize {
    let line = rendered
        .lines()
        .find(|line| line.contains(text))
        .unwrap_or_else(|| panic!("{text:?} should render:\n{rendered}"));
    let byte_index = line
        .find(text)
        .unwrap_or_else(|| panic!("{text:?} should have a stable row:\n{rendered}"));
    UnicodeWidthStr::width(&line[..byte_index])
}
