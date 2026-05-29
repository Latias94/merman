use merman_ascii::{
    AsciiError, AsciiRenderOptions, render_model, render_sequence as render_sequence_model,
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
const LINETYPE_CRITICAL_START: i32 = 27;
const LINETYPE_CRITICAL_OPTION: i32 = 28;
const LINETYPE_CRITICAL_END: i32 = 29;
const LINETYPE_BREAK_START: i32 = 30;
const LINETYPE_BREAK_END: i32 = 31;

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
fn sequence_deferred_control_blocks_are_explicitly_unsupported() {
    let cases = [
        "sequenceDiagram\nparticipant A\nparticipant B\nrect rgba(0,0,0,0.1)\nA->>B: Shaded\nend",
        "sequenceDiagram\nparticipant A\nparticipant B\npar_over Everyone\nA->>B: Work\nend",
    ];

    for input in cases {
        let model = parse_sequence_render_model(input);
        assert_unsupported_sequence_model(model, "control messages");
    }
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
fn sequence_titles_are_explicitly_unsupported() {
    let mut model = basic_sequence_model();
    model.title = Some("Setup".to_string());

    assert_unsupported_sequence_model(model, "diagram titles");
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
