use merman_ascii::{
    AsciiColorMode, AsciiColorRole, AsciiColorTheme, AsciiError, AsciiRenderOptions, AsciiRgb,
    render_model, render_sequence as render_sequence_model,
};
use merman_core::diagrams::sequence::{
    SequenceActor, SequenceBox, SequenceDiagramRenderModel, SequenceMessage,
    SequenceMessagePayload, SequenceNote,
};
use merman_core::{Engine, ParseOptions, RenderSemanticModel};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const LINETYPE_LOOP_START: i32 = 10;
const LINETYPE_LOOP_END: i32 = 11;
const LINETYPE_ALT_START: i32 = 12;
const LINETYPE_ALT_ELSE: i32 = 13;
const LINETYPE_ALT_END: i32 = 14;
const LINETYPE_OPT_START: i32 = 15;
const LINETYPE_OPT_END: i32 = 16;
const LINETYPE_PAR_START: i32 = 19;
const LINETYPE_PAR_AND: i32 = 20;
const LINETYPE_PAR_END: i32 = 21;
const LINETYPE_RECT_START: i32 = 22;
const LINETYPE_RECT_END: i32 = 23;
const LINETYPE_CRITICAL_START: i32 = 27;
const LINETYPE_CRITICAL_OPTION: i32 = 28;
const LINETYPE_CRITICAL_END: i32 = 29;
const LINETYPE_BREAK_START: i32 = 30;
const LINETYPE_BREAK_END: i32 = 31;
const LINETYPE_PAR_OVER_START: i32 = 32;

fn render_sequence(input: &str, options: &AsciiRenderOptions) -> merman_ascii::Result<String> {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("sequence diagram should parse")
        .expect("sequence diagram should be detected");

    render_model(&parsed.model, options)
}

fn parse_sequence_render_model(input: &str) -> SequenceDiagramRenderModel {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("sequence diagram should parse")
        .expect("sequence diagram should be detected");

    match parsed.model {
        RenderSemanticModel::Sequence(model) => model,
        other => panic!("expected sequence render model, got {}", other.kind()),
    }
}

fn fixture_cases(directory: &str) -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/testdata/mermaid-ascii")
        .join(directory);
    let mut cases = std::fs::read_dir(&root)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", root.display()))
        .map(|entry| entry.expect("fixture entry must be readable").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "txt"))
        .collect::<Vec<_>>();
    cases.sort();
    cases
}

fn split_fixture(path: &Path) -> (String, String) {
    let content = std::fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
        .replace("\r\n", "\n");
    let (input, expected) = content
        .split_once("\n---\n")
        .unwrap_or_else(|| panic!("fixture missing separator: {}", path.display()));
    (input.to_string(), expected.to_string())
}

fn normalize_sequence_output(text: &str) -> String {
    let mut lines = Vec::new();
    for line in text.replace("\r\n", "\n").split('\n') {
        let trimmed = line.trim_end_matches(' ');
        if !trimmed.is_empty() || !lines.is_empty() {
            lines.push(trimmed.to_string());
        }
    }

    while lines.last().is_some_and(|line| line.is_empty()) {
        lines.pop();
    }

    lines.join("\n")
}

fn strip_ansi(input: &str) -> String {
    let mut output = String::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for escaped in chars.by_ref() {
                if escaped == 'm' {
                    break;
                }
            }
            continue;
        }
        output.push(ch);
    }
    output
}

fn strip_html_spans(input: &str) -> String {
    let mut output = String::new();
    let mut index = 0;
    while index < input.len() {
        let rest = &input[index..];
        if rest.starts_with("<span ") {
            index += rest.find('>').expect("span start tag should be closed") + 1;
            continue;
        }
        if rest.starts_with("</span>") {
            index += "</span>".len();
            continue;
        }
        if rest.starts_with("&gt;") {
            output.push('>');
            index += "&gt;".len();
            continue;
        }
        if rest.starts_with("&lt;") {
            output.push('<');
            index += "&lt;".len();
            continue;
        }
        if rest.starts_with("&amp;") {
            output.push('&');
            index += "&amp;".len();
            continue;
        }
        let ch = rest
            .chars()
            .next()
            .expect("index should be on a char boundary");
        output.push(ch);
        index += ch.len_utf8();
    }
    output
}

#[test]
fn sequence_color_truecolor_emits_participant_lifeline_activation_and_message_roles() {
    let theme = AsciiColorTheme::default_light()
        .with_role(AsciiColorRole::Text, AsciiRgb::new(1, 1, 1))
        .with_role(AsciiColorRole::SequenceFrame, AsciiRgb::new(2, 2, 2))
        .with_role(AsciiColorRole::SequenceLifeline, AsciiRgb::new(3, 3, 3))
        .with_role(AsciiColorRole::SequenceActivation, AsciiRgb::new(4, 4, 4))
        .with_role(AsciiColorRole::EdgeLine, AsciiRgb::new(5, 5, 5))
        .with_role(AsciiColorRole::EdgeArrow, AsciiRgb::new(6, 6, 6))
        .with_role(AsciiColorRole::EdgeLabel, AsciiRgb::new(7, 7, 7))
        .with_role(AsciiColorRole::Junction, AsciiRgb::new(8, 8, 8));
    let options = AsciiRenderOptions::ascii()
        .with_color_mode(AsciiColorMode::TrueColor)
        .with_color_theme(theme);

    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\nA->>+B: Start\nB-->>-A: Done",
        &options,
    )
    .expect("sequence should render with color roles");

    assert_eq!(
        strip_ansi(&rendered),
        concat!(
            "+---+     +---+\n",
            "| A |     | B |\n",
            "+-+-+     +-+-+\n",
            "  |         |\n",
            "  | Start   |\n",
            "  +-------->|\n",
            "  |         #\n",
            "  | Done    #\n",
            "  |<........+\n",
            "  |         |\n",
        )
    );
    for expected_code in [
        "\u{1b}[38;2;1;1;1m",
        "\u{1b}[38;2;2;2;2m",
        "\u{1b}[38;2;3;3;3m",
        "\u{1b}[38;2;4;4;4m",
        "\u{1b}[38;2;5;5;5m",
        "\u{1b}[38;2;6;6;6m",
        "\u{1b}[38;2;7;7;7m",
        "\u{1b}[38;2;8;8;8m",
    ] {
        assert!(
            rendered.contains(expected_code),
            "missing {expected_code:?} in {rendered:?}"
        );
    }
}

#[test]
fn sequence_color_html_wraps_boxes_notes_control_frames_and_messages_without_changing_plain_text() {
    let theme = AsciiColorTheme::default_light()
        .with_role(AsciiColorRole::Text, AsciiRgb::from_hex24(0x101010))
        .with_role(
            AsciiColorRole::SequenceFrame,
            AsciiRgb::from_hex24(0x202020),
        )
        .with_role(
            AsciiColorRole::SequenceLifeline,
            AsciiRgb::from_hex24(0x303030),
        )
        .with_role(
            AsciiColorRole::SequenceActivation,
            AsciiRgb::from_hex24(0x404040),
        )
        .with_role(AsciiColorRole::EdgeLine, AsciiRgb::from_hex24(0x505050))
        .with_role(AsciiColorRole::EdgeArrow, AsciiRgb::from_hex24(0x606060))
        .with_role(AsciiColorRole::EdgeLabel, AsciiRgb::from_hex24(0x707070))
        .with_role(AsciiColorRole::Junction, AsciiRgb::from_hex24(0x808080));
    let options = AsciiRenderOptions::ascii()
        .with_color_mode(AsciiColorMode::Html)
        .with_color_theme(theme);

    let rendered = render_sequence(
        "sequenceDiagram\nbox Group\nparticipant A\nparticipant B\nend\nloop Work\nA->>+B: Start\nNote over A,B: Wait\nB-->>-A: Done\nend",
        &options,
    )
    .expect("sequence with boxes, frames, notes, and messages should render");

    assert_eq!(
        strip_html_spans(&rendered),
        concat!(
            "+- Group -------+\n",
            "|+---+     +---+|\n",
            "|| A |     | B ||\n",
            "|+-+-+     +-+-+|\n",
            "|+ loop Work ----+\n",
            "|| |         |  ||\n",
            "|| | Start   |  ||\n",
            "|| +-------->|  ||\n",
            "|| |         #  ||\n",
            "||+-----------+ ||\n",
            "|||   Wait    | ||\n",
            "||+-----------+ ||\n",
            "|| |         #  ||\n",
            "|| | Done    #  ||\n",
            "|| |<........+  ||\n",
            "|+---------------+\n",
            "|  |         |  |\n",
            "+---------------+\n",
        )
    );
    for expected_fragment in [
        "<span style=\"color:#202020\">+-</span><span style=\"color:#101010\"> Group </span>",
        "<span style=\"color:#202020\">|+</span><span style=\"color:#101010\"> loop Work </span>",
        "<span style=\"color:#202020\">||+-----------+</span>",
        "<span style=\"color:#707070\">Start</span>",
        "<span style=\"color:#505050\">--------</span><span style=\"color:#606060\">&gt;</span>",
        "<span style=\"color:#404040\">#</span>",
        "<span style=\"color:#101010\">Wait</span>",
    ] {
        assert!(
            rendered.contains(expected_fragment),
            "missing {expected_fragment:?} in {rendered:?}"
        );
    }
}

fn basic_sequence_model() -> SequenceDiagramRenderModel {
    let mut actors = BTreeMap::new();
    actors.insert(
        "A".to_string(),
        SequenceActor {
            name: "A".to_string(),
            description: "A".to_string(),
            actor_type: "participant".to_string(),
            wrap: false,
            links: Default::default(),
            properties: Default::default(),
        },
    );

    SequenceDiagramRenderModel {
        acc_title: None,
        acc_descr: None,
        title: None,
        actor_order: vec!["A".to_string()],
        actors,
        boxes: Vec::new(),
        messages: Vec::new(),
        notes: Vec::new(),
        created_actors: Default::default(),
        destroyed_actors: Default::default(),
    }
}

fn add_sequence_participant(model: &mut SequenceDiagramRenderModel, id: &str) {
    model.actor_order.push(id.to_string());
    model.actors.insert(
        id.to_string(),
        SequenceActor {
            name: id.to_string(),
            description: id.to_string(),
            actor_type: "participant".to_string(),
            wrap: false,
            links: Default::default(),
            properties: Default::default(),
        },
    );
}

fn assert_unsupported_sequence_model(model: SequenceDiagramRenderModel, feature: &'static str) {
    let err = render_sequence_model(&model, &AsciiRenderOptions::unicode()).unwrap_err();
    assert_eq!(
        err,
        AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature,
        }
    );
}

fn assert_unsupported_sequence_input(input: &str, feature: &'static str) {
    let err = render_sequence(input, &AsciiRenderOptions::unicode()).unwrap_err();
    assert_eq!(
        err,
        AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature,
        }
    );
}

fn message(from: Option<&str>, to: Option<&str>, message_type: i32) -> SequenceMessage {
    SequenceMessage {
        id: "m0".to_string(),
        from: from.map(str::to_string),
        to: to.map(str::to_string),
        message_type,
        message: SequenceMessagePayload::Text("Hi".to_string()),
        wrap: false,
        activate: false,
        placement: None,
    }
}

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
fn sequence_notes_render_from_typed_model() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\nA->>B: Start\nNote right of A: right\nNote left of B: left\nNote over A,B: over both\nB-->>A: Done",
        &AsciiRenderOptions::unicode(),
    )
    .expect("single-line sequence notes should render");

    assert!(
        rendered.contains("│ right │"),
        "right-of note should render as a note box:\n{rendered}"
    );
    assert!(
        rendered.contains("│ left │"),
        "left-of note should render as a note box:\n{rendered}"
    );
    assert!(
        rendered.contains("│ over both │"),
        "over note should render as a note box:\n{rendered}"
    );
    assert!(
        rendered.contains("├────────►│"),
        "normal messages around notes should keep rendering:\n{rendered}"
    );
}

#[test]
fn sequence_multiline_notes_are_explicitly_unsupported() {
    let mut model = basic_sequence_model();
    model.notes.push(SequenceNote {
        actor: "A".into(),
        message: "line 1\nline 2".to_string(),
        placement: 1,
        wrap: false,
    });
    model.messages.push(SequenceMessage {
        id: "n0".to_string(),
        from: Some("A".to_string()),
        to: Some("A".to_string()),
        message_type: 2,
        message: SequenceMessagePayload::Text("line 1\nline 2".to_string()),
        wrap: false,
        activate: false,
        placement: Some(1),
    });

    assert_unsupported_sequence_model(model, "multiline notes");
}

#[test]
fn sequence_wrapped_messages_render_from_typed_model() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\nA->>B:wrap: Alpha Beta Gamma",
        &AsciiRenderOptions::unicode(),
    )
    .expect("wrapped sequence messages should render");

    assert!(
        rendered.contains("Alpha") && rendered.contains("Beta") && rendered.contains("Gamma"),
        "wrapped message should keep all words:\n{rendered}"
    );
    assert!(
        !rendered.contains("Alpha Beta Gamma"),
        "wrapped message should not render as one long line:\n{rendered}"
    );
}

#[test]
fn sequence_wrapped_notes_render_from_typed_model() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\nNote over A,B:wrap: Alpha Beta Gamma Delta Epsilon Zeta",
        &AsciiRenderOptions::unicode(),
    )
    .expect("wrapped sequence notes should render");

    assert!(
        rendered.contains("Alpha") && rendered.contains("Zeta"),
        "wrapped note should keep all words:\n{rendered}"
    );
    assert!(
        !rendered.contains("Alpha Beta Gamma Delta Epsilon Zeta"),
        "wrapped note should not render as one long line:\n{rendered}"
    );
}

#[test]
fn sequence_wrapped_messages_respect_display_width_for_cjk() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\nA->>B:wrap: 数据数据数据数据",
        &AsciiRenderOptions::unicode(),
    )
    .expect("wrapped CJK sequence messages should render");

    assert!(
        !rendered.contains("数据数据数据数据"),
        "wide text without spaces should wrap by display width:\n{rendered}"
    );
}

#[test]
fn sequence_boxes_render_from_typed_model() {
    let rendered = render_sequence(
        "sequenceDiagram\nbox green Group 1\nparticipant A\nparticipant B\nend\nA->>B: Inside",
        &AsciiRenderOptions::unicode(),
    )
    .expect("sequence boxes should render");

    assert!(
        rendered
            .lines()
            .next()
            .is_some_and(|line| line.contains("Group 1")),
        "box title should render in the enclosing box border:\n{rendered}"
    );
    assert!(
        rendered.contains("│ A │") && rendered.contains("│ B │"),
        "boxed participants should keep rendering:\n{rendered}"
    );
    assert!(
        rendered.contains("├────────►│"),
        "messages inside boxes should keep rendering:\n{rendered}"
    );
}

#[test]
fn sequence_wrapped_boxes_are_explicitly_unsupported() {
    let mut model = basic_sequence_model();
    model.boxes.push(SequenceBox {
        actor_keys: vec!["A".to_string()],
        fill: "green".to_string(),
        name: Some("Group".to_string()),
        wrap: true,
    });

    assert_unsupported_sequence_model(model, "wrapped boxes");
}

#[test]
fn sequence_boxes_with_unknown_actors_are_explicitly_unsupported() {
    let mut model = basic_sequence_model();
    model.boxes.push(SequenceBox {
        actor_keys: vec!["B".to_string()],
        fill: "green".to_string(),
        name: Some("Group".to_string()),
        wrap: false,
    });

    assert_unsupported_sequence_model(model, "boxes with unknown actors");
}

#[test]
fn sequence_activations_render_from_typed_model() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\nA->>+B: Start\nB-->>A: Working\nB-->>-A: Done",
        &AsciiRenderOptions::unicode(),
    )
    .expect("sequence activations should render");

    assert!(
        rendered.contains("┃"),
        "active participant lifeline should render with an activation bar:\n{rendered}"
    );
    assert!(
        rendered.contains("│ Working"),
        "messages should still render while a participant is active:\n{rendered}"
    );
}

#[test]
fn sequence_actor_lifecycle_renders_from_typed_model() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\nA->>B: Before\ncreate participant C\nB->>C: Hello C\nC->>B: Still here\ndestroy C\nB--xC: Bye C\nB->>A: After",
        &AsciiRenderOptions::unicode(),
    )
    .expect("sequence actor create/destroy should render");

    let header = rendered.lines().take(3).collect::<Vec<_>>().join("\n");
    assert!(
        !header.contains("│ C │"),
        "created participant should not render in the initial header:\n{rendered}"
    );
    assert_eq!(
        rendered.matches("│ C │").count(),
        1,
        "created participant should render once at its creation point:\n{rendered}"
    );
    assert!(
        rendered.contains("×"),
        "destroyed participant should render a termination marker:\n{rendered}"
    );
}

#[test]
fn sequence_actor_lifecycle_validates_hand_built_indices() {
    let mut cases = Vec::new();

    let mut model = basic_sequence_model();
    model.messages.push(message(Some("A"), Some("A"), 0));
    model.created_actors.insert("B".to_string(), 0);
    cases.push((model, "actor lifecycle actors"));

    let mut model = basic_sequence_model();
    model.messages.push(message(Some("A"), Some("A"), 0));
    model.created_actors.insert("A".to_string(), 1);
    cases.push((model, "actor lifecycle message indices"));

    let mut model = basic_sequence_model();
    add_sequence_participant(&mut model, "B");
    model.messages.push(message(Some("A"), Some("A"), 0));
    model.created_actors.insert("B".to_string(), 0);
    cases.push((model, "actor creation messages"));

    let mut model = basic_sequence_model();
    add_sequence_participant(&mut model, "B");
    model.messages.push(message(Some("A"), Some("A"), 0));
    model.destroyed_actors.insert("B".to_string(), 0);
    cases.push((model, "actor destruction messages"));

    let mut model = basic_sequence_model();
    add_sequence_participant(&mut model, "B");
    model.messages.push(message(Some("A"), Some("B"), 0));
    model.messages.push(message(Some("A"), Some("B"), 0));
    model.destroyed_actors.insert("B".to_string(), 0);
    cases.push((model, "actor lifecycle visibility"));

    for (model, feature) in cases {
        assert_unsupported_sequence_model(model, feature);
    }
}

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
fn sequence_rect_par_over_blocks_are_core_control_signals() {
    struct Case {
        name: &'static str,
        input: &'static str,
        signals: &'static [(i32, &'static str)],
    }

    let cases = [
        Case {
            name: "rect",
            input: "sequenceDiagram\nparticipant A\nparticipant B\nrect rgba(0,0,0,0.1)\nA->>B: Shaded\nend",
            signals: &[
                (LINETYPE_RECT_START, "rgba(0,0,0,0.1)"),
                (LINETYPE_RECT_END, ""),
            ],
        },
        Case {
            name: "par_over",
            input: "sequenceDiagram\nparticipant A\nparticipant B\npar_over Everyone\nA->>B: Work\nend",
            signals: &[
                (LINETYPE_PAR_OVER_START, "Everyone"),
                (LINETYPE_PAR_END, ""),
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
fn sequence_rect_control_blocks_render_unicode_frames() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\nrect rgba(0,0,0,0.1)\nA->>B: Shaded\nend",
        &AsciiRenderOptions::unicode(),
    )
    .expect("rect should render with Unicode charset");

    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with("┌ rect rgba(0,0,0,0.1) ")),
        "rect should render a labeled Unicode top frame:\n{rendered}"
    );
    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with('│') && line.contains("Shaded")),
        "rect should keep contained rows inside the Unicode frame:\n{rendered}"
    );
    assert!(
        rendered.lines().any(|line| line.starts_with('└')),
        "rect should render a Unicode bottom frame:\n{rendered}"
    );
}

#[test]
fn sequence_rect_control_blocks_render_ascii_frames() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\nrect rgba(0,0,0,0.1)\nA->>B: Shaded\nend",
        &AsciiRenderOptions::ascii(),
    )
    .expect("rect should render with ASCII charset");

    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with("+ rect rgba(0,0,0,0.1) ")),
        "rect should render a labeled ASCII top frame:\n{rendered}"
    );
    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with('|') && line.contains("Shaded")),
        "rect should keep contained rows inside the ASCII frame:\n{rendered}"
    );
    assert!(
        rendered.lines().any(|line| line.starts_with('+')),
        "rect should render an ASCII bottom frame:\n{rendered}"
    );
}

#[test]
fn sequence_par_over_control_blocks_render_unicode_frames() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\npar_over Everyone\nA->>B: Work\nend",
        &AsciiRenderOptions::unicode(),
    )
    .expect("par_over should render with Unicode charset");

    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with("┌ par_over Everyone ")),
        "par_over should render a labeled Unicode top frame:\n{rendered}"
    );
    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with('│') && line.contains("Work")),
        "par_over should keep contained rows inside the Unicode frame:\n{rendered}"
    );
    assert!(
        rendered.lines().any(|line| line.starts_with('└')),
        "par_over should render a Unicode bottom frame:\n{rendered}"
    );
}

#[test]
fn sequence_par_over_control_blocks_render_ascii_frames() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\npar_over Everyone\nA->>B: Work\nend",
        &AsciiRenderOptions::ascii(),
    )
    .expect("par_over should render with ASCII charset");

    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with("+ par_over Everyone ")),
        "par_over should render a labeled ASCII top frame:\n{rendered}"
    );
    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with('|') && line.contains("Work")),
        "par_over should keep contained rows inside the ASCII frame:\n{rendered}"
    );
    assert!(
        rendered.lines().any(|line| line.starts_with('+')),
        "par_over should render an ASCII bottom frame:\n{rendered}"
    );
}

#[test]
fn sequence_rect_par_over_control_blocks_support_notes_activations_and_boxes() {
    let cases = [
        (
            "rect rgba(0,0,0,0.1)",
            "rect rgba(0,0,0,0.1)",
            "sequenceDiagram\nbox Group\nparticipant A\nparticipant B\nend\nrect rgba(0,0,0,0.1)\nNote over A,B: Wait\nA->>+B: Start\nB-->>-A: Done\nend",
        ),
        (
            "par_over Everyone",
            "par_over Everyone",
            "sequenceDiagram\nbox Group\nparticipant A\nparticipant B\nend\npar_over Everyone\nNote over A,B: Wait\nA->>+B: Start\nB-->>-A: Done\nend",
        ),
    ];

    for (label, frame_marker, input) in cases {
        let rendered = render_sequence(input, &AsciiRenderOptions::unicode())
            .unwrap_or_else(|err| panic!("{label} should render: {err}"));

        assert!(
            rendered.contains("Group"),
            "{label} should preserve participant box labels:\n{rendered}"
        );
        assert!(
            rendered.contains(frame_marker),
            "{label} should render the control frame:\n{rendered}"
        );
        assert!(
            rendered
                .lines()
                .any(|line| line.starts_with('│') && line.contains("Wait")),
            "{label} should keep notes inside the frame:\n{rendered}"
        );
        assert!(
            rendered
                .lines()
                .any(|line| line.starts_with('│') && line.contains('┃')),
            "{label} should keep active lifelines inside the frame:\n{rendered}"
        );
    }
}

#[test]
fn sequence_rect_par_over_control_blocks_support_created_and_destroyed_actors() {
    let cases = [
        (
            "rect rgba(0,0,0,0.1)",
            "sequenceDiagram\nparticipant A\nparticipant B\nrect rgba(0,0,0,0.1)\ncreate participant C\nB->>C: Hello C\nC->>B: Still here\ndestroy C\nB--xC: Bye C\nend",
        ),
        (
            "par_over Everyone",
            "sequenceDiagram\nparticipant A\nparticipant B\npar_over Everyone\ncreate participant C\nB->>C: Hello C\nC->>B: Still here\ndestroy C\nB--xC: Bye C\nend",
        ),
    ];

    for (label, input) in cases {
        let rendered = render_sequence(input, &AsciiRenderOptions::unicode())
            .unwrap_or_else(|err| panic!("{label} should render: {err}"));

        assert!(
            rendered.contains(label),
            "{label} should render the control frame:\n{rendered}"
        );
        assert!(
            rendered
                .lines()
                .any(|line| line.starts_with('│') && line.contains("Hello C")),
            "{label} should keep created actor messages inside the frame:\n{rendered}"
        );
        assert!(
            rendered
                .lines()
                .any(|line| line.starts_with('│') && line.contains("Bye C")),
            "{label} should keep destroying messages inside the frame:\n{rendered}"
        );
    }
}

#[test]
fn sequence_rect_par_over_nested_control_blocks_are_explicitly_unsupported() {
    let cases = [
        "sequenceDiagram\nparticipant A\nparticipant B\nrect rgba(0,0,0,0.1)\npar_over Everyone\nA->>B: Work\nend\nend",
        "sequenceDiagram\nparticipant A\nparticipant B\npar_over Everyone\nrect rgba(0,0,0,0.1)\nA->>B: Work\nend\nend",
    ];

    for input in cases {
        assert_unsupported_sequence_input(input, "nested control blocks");
    }
}

#[test]
fn sequence_rect_par_over_empty_sections_are_explicitly_unsupported() {
    let mut cases = Vec::new();

    let mut model = basic_sequence_model();
    add_sequence_participant(&mut model, "B");
    model
        .messages
        .push(message(None, None, LINETYPE_RECT_START));
    model.messages.push(message(None, None, LINETYPE_RECT_END));
    cases.push(model);

    let mut model = basic_sequence_model();
    add_sequence_participant(&mut model, "B");
    model
        .messages
        .push(message(None, None, LINETYPE_PAR_OVER_START));
    model.messages.push(message(None, None, LINETYPE_PAR_END));
    cases.push(model);

    for model in cases {
        assert_unsupported_sequence_model(model, "empty control block sections");
    }
}

#[test]
fn sequence_rect_par_over_malformed_ordering_is_explicitly_unsupported() {
    let mut cases = Vec::new();

    let mut model = basic_sequence_model();
    add_sequence_participant(&mut model, "B");
    model
        .messages
        .push(message(None, None, LINETYPE_RECT_START));
    model.messages.push(message(Some("A"), Some("B"), 0));
    model.messages.push(message(None, None, LINETYPE_PAR_END));
    cases.push(model);

    let mut model = basic_sequence_model();
    add_sequence_participant(&mut model, "B");
    model
        .messages
        .push(message(None, None, LINETYPE_PAR_OVER_START));
    model.messages.push(message(Some("A"), Some("B"), 0));
    model.messages.push(message(None, None, LINETYPE_RECT_END));
    cases.push(model);

    for model in cases {
        assert_unsupported_sequence_model(model, "control block ordering");
    }
}

#[test]
fn sequence_nested_control_blocks_are_explicitly_unsupported() {
    assert_unsupported_sequence_input(
        "sequenceDiagram\nparticipant A\nparticipant B\nloop Outer\nopt Inner\nA->>B: Work\nend\nend",
        "nested control blocks",
    );
}

#[test]
fn sequence_empty_control_block_sections_are_explicitly_unsupported() {
    let mut cases = Vec::new();

    let mut model = basic_sequence_model();
    add_sequence_participant(&mut model, "B");
    model
        .messages
        .push(message(None, None, LINETYPE_LOOP_START));
    model.messages.push(message(None, None, LINETYPE_LOOP_END));
    cases.push(model);

    let mut model = basic_sequence_model();
    add_sequence_participant(&mut model, "B");
    model.messages.push(message(None, None, LINETYPE_ALT_START));
    model.messages.push(message(None, None, LINETYPE_ALT_ELSE));
    model.messages.push(message(Some("A"), Some("B"), 0));
    model.messages.push(message(None, None, LINETYPE_ALT_END));
    cases.push(model);

    let mut model = basic_sequence_model();
    add_sequence_participant(&mut model, "B");
    model.messages.push(message(None, None, LINETYPE_ALT_START));
    model.messages.push(message(Some("A"), Some("B"), 0));
    model.messages.push(message(None, None, LINETYPE_ALT_ELSE));
    model.messages.push(message(None, None, LINETYPE_ALT_END));
    cases.push(model);

    for model in cases {
        assert_unsupported_sequence_model(model, "empty control block sections");
    }
}

#[test]
fn sequence_control_blocks_support_activation_lifecycle_rows() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\nloop Work\nA->>+B: Start\nB-->>-A: Done\nend",
        &AsciiRenderOptions::unicode(),
    )
    .expect("control blocks should support activation rows");

    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with("┌ loop Work ")),
        "loop should render while activation events are present:\n{rendered}"
    );
    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with('│') && line.contains('┃')),
        "active lifeline should remain visible inside the frame:\n{rendered}"
    );
}

#[test]
fn sequence_control_blocks_support_created_and_destroyed_actors() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\nloop Setup\ncreate participant C\nB->>C: Hello C\nC->>B: Still here\ndestroy C\nB--xC: Bye C\nend",
        &AsciiRenderOptions::unicode(),
    )
    .expect("control blocks should support create and destroy lifecycle rows");

    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with("┌ loop Setup ")),
        "loop should render around create/destroy lifecycle rows:\n{rendered}"
    );
    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with('│') && line.contains("Hello C")),
        "created actor message should remain inside the frame:\n{rendered}"
    );
    assert!(
        rendered
            .lines()
            .any(|line| line.starts_with('│') && line.contains("Bye C")),
        "destroying message should remain inside the frame:\n{rendered}"
    );
}

#[test]
fn sequence_control_blocks_render_inside_participant_boxes() {
    let rendered = render_sequence(
        "sequenceDiagram\nbox Group\nparticipant A\nparticipant B\nend\nloop Work\nA->>B: Hi\nend",
        &AsciiRenderOptions::unicode(),
    )
    .expect("control blocks should render with boxed participants");

    assert!(
        rendered.contains("Group"),
        "participant box label should still render:\n{rendered}"
    );
    assert!(
        rendered.contains("loop Work"),
        "control frame should still render inside participant box output:\n{rendered}"
    );
}

#[test]
fn sequence_open_arrows_render_from_typed_model() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\nA->B: Open\nA-->B: Dotted\nB->A: Back",
        &AsciiRenderOptions::unicode(),
    )
    .expect("open arrow sequence messages should render");

    assert!(
        rendered.contains("├────────>│"),
        "solid open arrow should use an open Unicode arrow head:\n{rendered}"
    );
    assert!(
        rendered.contains("├┈┈┈┈┈┈┈┈>│"),
        "dotted open arrow should use dotted line with an open Unicode arrow head:\n{rendered}"
    );
    assert!(
        rendered.contains("│<────────┤"),
        "reverse open arrow should use an open Unicode arrow head:\n{rendered}"
    );
    assert!(
        !rendered.contains("Open   │\n  ├────────►│"),
        "open arrows must stay visually distinct from filled arrows:\n{rendered}"
    );
}

#[test]
fn sequence_titles_render_above_participants() {
    let rendered = render_sequence(
        "sequenceDiagram\ntitle: Setup\nparticipant A\nparticipant B\nA->>B: Hi",
        &AsciiRenderOptions::ascii(),
    )
    .expect("sequence title should render");

    assert_eq!(
        rendered,
        concat!(
            "     Setup\n",
            "+---+     +---+\n",
            "| A |     | B |\n",
            "+-+-+     +-+-+\n",
            "  |         |\n",
            "  | Hi      |\n",
            "  +-------->|\n",
            "  |         |\n",
        )
    );

    let boxed = render_sequence(
        "sequenceDiagram\ntitle: Setup\nbox Group\nparticipant A\nparticipant B\nend\nA->>B: Hi",
        &AsciiRenderOptions::ascii(),
    )
    .expect("sequence title should render outside boxes");
    let mut boxed_lines = boxed.lines();
    assert_eq!(boxed_lines.next().unwrap().trim(), "Setup");
    assert!(
        boxed_lines.next().unwrap().starts_with("+- Group"),
        "title should stay above sequence boxes:\n{boxed}"
    );
}

#[test]
fn sequence_actor_shapes_are_explicitly_unsupported() {
    let mut model = basic_sequence_model();
    model.actors.get_mut("A").unwrap().actor_type = "actor".to_string();

    assert_unsupported_sequence_model(model, "actor participant shapes");
}

#[test]
fn sequence_wrapped_actor_labels_are_explicitly_unsupported() {
    let mut model = basic_sequence_model();
    model.actors.get_mut("A").unwrap().wrap = true;

    assert_unsupported_sequence_model(model, "wrapped actor labels");
}

#[test]
fn sequence_actor_links_are_explicitly_unsupported() {
    let mut model = basic_sequence_model();
    model
        .actors
        .get_mut("A")
        .unwrap()
        .links
        .insert("docs".to_string(), "https://example.com".into());

    assert_unsupported_sequence_model(model, "actor links/properties");
}

#[test]
fn sequence_other_model_features_are_explicitly_unsupported() {
    let mut cases = Vec::new();

    let mut model = basic_sequence_model();
    model.messages.push(SequenceMessage {
        placement: Some(0),
        ..message(Some("A"), Some("A"), 0)
    });
    cases.push((model, "message placement"));

    let mut model = basic_sequence_model();
    model.messages.push(message(None, None, 0));
    cases.push((model, "control messages"));

    let mut model = basic_sequence_model();
    model.messages.push(message(Some("A"), Some("B"), 0));
    cases.push((model, "messages with unknown actors"));

    let mut model = basic_sequence_model();
    model.messages.push(message(Some("A"), Some("A"), 42));
    cases.push((model, "message types"));

    for (model, feature) in cases {
        assert_unsupported_sequence_model(model, feature);
    }
}
