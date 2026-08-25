use super::*;

#[test]
fn sequence_open_line_types_render_as_headless_signals() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\nA->B: Headless\nA-->B: Dotted\nB->A: Back",
        &AsciiRenderOptions::unicode(),
    )
    .expect("headless sequence messages should render");
    let lines = rendered.lines().collect::<Vec<_>>();
    let signal_row_after = |label| {
        lines
            .get(first_line_index_containing(&rendered, label) + 1)
            .copied()
            .unwrap_or_else(|| panic!("missing signal row after {label:?}:\n{rendered}"))
    };
    let solid = signal_row_after("Headless");
    let dotted = signal_row_after("Dotted");
    let right_to_left = signal_row_after("Back");

    assert!(
        solid.contains('├') && solid.contains('─') && solid.contains('│'),
        "solid open line type should retain its stroke without an endpoint marker:\n{rendered}"
    );
    assert!(
        dotted.contains('├') && dotted.contains('┈') && dotted.contains('│'),
        "dotted open line type should retain its dotted stroke without a marker:\n{rendered}"
    );
    assert!(
        right_to_left.contains('│') && right_to_left.contains('─') && right_to_left.contains('┤'),
        "right-to-left open line type should remain headless:\n{rendered}"
    );
    assert!(
        !rendered
            .chars()
            .any(|ch| matches!(ch, '<' | '>' | '►' | '◄')),
        "headless signals must not synthesize ASCII or Unicode arrowheads:\n{rendered}"
    );
}

#[test]
fn sequence_extended_signal_markers_render_from_typed_endpoint_semantics() {
    let rendered = render_sequence(
        r#"sequenceDiagram
participant A
participant B
A-)B: Async point
A<<->>B: Bidirectional
A-|\B: Filled half
A/|-B: Reverse half
A--//B: Open half
A--)A: Self async"#,
        &AsciiRenderOptions::unicode(),
    )
    .expect("extended sequence signal markers should render");

    for label in [
        "Async point",
        "Bidirectional",
        "Filled half",
        "Reverse half",
        "Open half",
        "Self async",
    ] {
        assert!(
            rendered.contains(label),
            "extended signal label {label:?} should remain visible:\n{rendered}"
        );
    }
    assert!(
        rendered.contains(')'),
        "async point markers should remain distinct:\n{rendered}"
    );
    assert!(
        rendered.contains("├◄") && rendered.contains("►│"),
        "bidirectional messages should paint both endpoint markers:\n{rendered}"
    );
    assert!(
        rendered.contains('◢') && rendered.contains('◣'),
        "forward and reverse filled half markers should retain endpoint ownership:\n{rendered}"
    );
    assert!(
        rendered.contains('╱'),
        "open half markers should remain distinct from filled halves:\n{rendered}"
    );
    assert!(
        rendered
            .lines()
            .any(|line| line.contains('┈') && line.contains('(')),
        "dotted self messages should preserve their point marker and stroke:\n{rendered}"
    );
}

#[test]
fn sequence_ascii_half_arrows_preserve_filled_and_open_semantics() {
    let signal_row = |signal: &str| {
        let rendered = render_sequence(
            &format!("sequenceDiagram\nparticipant A\nparticipant B\nA{signal}B: Same"),
            &AsciiRenderOptions::ascii(),
        )
        .unwrap_or_else(|error| panic!("{signal:?} should render: {error}"));
        rendered
            .lines()
            .nth(first_line_index_containing(&rendered, "Same") + 1)
            .unwrap_or_else(|| panic!("missing signal row for {signal:?}:\n{rendered}"))
            .to_string()
    };

    for (filled, open, filled_glyph) in [
        ("-|\\", "-\\\\", "|\\"),
        ("-|/", "-//", "|/"),
        ("/|-", "//-", "/|"),
        ("\\|-", "\\\\-", "\\|"),
        ("--|\\", "--\\\\", "|\\"),
        ("--|/", "--//", "|/"),
        ("/|--", "//--", "/|"),
        ("\\|--", "\\\\--", "\\|"),
    ] {
        let filled_row = signal_row(filled);
        let open_row = signal_row(open);
        assert_ne!(
            filled_row, open_row,
            "filled and open half arrows must not collide for {filled:?}/{open:?}"
        );
        assert!(
            filled_row.contains(filled_glyph),
            "filled half arrow {filled:?} should render {filled_glyph:?}:\n{filled_row}"
        );
        assert!(
            !open_row.contains(filled_glyph),
            "open half arrow {open:?} must not synthesize a filled stem:\n{open_row}"
        );
    }
}

#[test]
fn sequence_ascii_self_half_arrow_expands_a_narrow_loop_without_losing_fill() {
    let mut options = AsciiRenderOptions::ascii();
    options.sequence_self_message_width = 2;
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nA-|\\A: Self filled half",
        &options,
    )
    .expect("filled self half-arrow should expand its effective loop width");

    assert!(rendered.contains("Self filled half"));
    assert!(
        rendered.contains("/|"),
        "self half-arrow should retain its filled stem:\n{rendered}"
    );
}

#[test]
fn sequence_cjk_half_arrows_preserve_fill_in_reverse_and_narrow_self_paths() {
    let mut options = AsciiRenderOptions::unicode();
    options.terminal_width_profile = TerminalWidthProfile::Cjk;

    let signal_row = |signal: &str| {
        let rendered = render_sequence(
            &format!(
                "sequenceDiagram\nparticipant 客户 服务\nparticipant 数据 库\n数据 库{signal}客户 服务: 同步"
            ),
            &options,
        )
        .unwrap_or_else(|error| panic!("CJK reverse signal {signal:?} should render: {error}"));
        assert!(rendered.contains("客户 服务") && rendered.contains("数据 库"));
        rendered
            .lines()
            .nth(first_line_index_containing(&rendered, "同步") + 1)
            .unwrap_or_else(|| panic!("missing reverse signal row for {signal:?}:\n{rendered}"))
            .to_string()
    };

    let filled = signal_row("-|\\");
    let open = signal_row("-\\\\");
    assert_ne!(filled, open);
    assert!(
        filled.contains("/|"),
        "CJK reverse filled half-arrow should retain its stem:\n{filled}"
    );
    assert!(
        !open.contains("/|"),
        "CJK reverse open half-arrow must remain unfilled:\n{open}"
    );

    options.sequence_self_message_width = 2;
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant 客户 服务\n客户 服务-|\\客户 服务: 自调用",
        &options,
    )
    .expect("a narrow CJK self-message should expand from the shared geometry");
    assert!(rendered.contains("客户 服务") && rendered.contains("自调用"));
    assert!(rendered.contains("/|"));
}

#[test]
fn sequence_spaced_actor_names_render_without_splitting_participants() {
    let input = r#"sequenceDiagram
participant cron job
participant data svc
cron job ()->>() data svc: run
Note over cron job,data svc: nightly
data svc-->>cron job: done"#;

    for options in [AsciiRenderOptions::ascii(), AsciiRenderOptions::unicode()] {
        let rendered = render_sequence(input, &options)
            .expect("Mermaid-valid spaced actor names should render");
        for expected in ["cron job", "data svc", "run", "nightly", "done"] {
            assert!(
                rendered.contains(expected),
                "spaced sequence should retain {expected:?}:\n{rendered}"
            );
        }
    }
}

#[test]
fn sequence_distinct_id_and_actor_character_sets_render_without_loss() {
    let input = r#"sequenceDiagram
participant C++
participant api(v2)
participant api-xray
activate api-xray
alice@example.com->>data@example.com: mail
deactivate api-xray"#;

    let rendered = render_sequence(input, &AsciiRenderOptions::ascii())
        .expect("pinned ID and ACTOR character sets should survive terminal projection");
    for expected in [
        "C++",
        "api(v2)",
        "api-xray",
        "alice@example.com",
        "data@example.com",
        "mail",
    ] {
        assert!(
            rendered.contains(expected),
            "sequence output should retain {expected:?}:\n{rendered}"
        );
    }
}

#[test]
fn sequence_central_marker_records_are_suppressed_without_skipping_autonumbers() {
    let rendered = render_sequence(
        r#"sequenceDiagram
participant A
participant B
autonumber
A->>()B: Target central
A()->>B: Source central
B()->>()A: Dual central"#,
        &AsciiRenderOptions::unicode(),
    )
    .expect("central connection decorations should render");

    for numbered_label in ["1. Target central", "2. Source central", "3. Dual central"] {
        assert!(
            rendered.contains(numbered_label),
            "central marker records must not consume autonumbers for {numbered_label:?}:\n{rendered}"
        );
    }
    assert!(
        rendered.matches('○').count() >= 4,
        "target, source, and dual central decorations should remain visible:\n{rendered}"
    );
}
