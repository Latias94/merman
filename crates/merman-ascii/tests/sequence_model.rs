use merman_ascii::{
    AsciiColorMode, AsciiColorRole, AsciiColorTheme, AsciiError, AsciiRenderOptions, AsciiRgb,
    TerminalWidthProfile, render_model, render_sequence as render_sequence_model,
};
use merman_core::diagrams::sequence::{
    SequenceActor, SequenceAutonumber, SequenceBox, SequenceDiagramRenderModel, SequenceMessage,
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
const LINETYPE_AUTONUMBER: i32 = 26;
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

    render_model(parsed.model(), options)
}

fn read_local_semantic_fixture(path: &str) -> String {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/testdata/local-semantic")
        .join(path);
    std::fs::read_to_string(&fixture_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", fixture_path.display()))
}

fn first_line_index_containing(rendered: &str, needle: &str) -> usize {
    rendered
        .lines()
        .position(|line| line.contains(needle))
        .unwrap_or_else(|| panic!("missing {needle:?} in rendered fixture:\n{rendered}"))
}

fn parse_sequence_render_model(input: &str) -> SequenceDiagramRenderModel {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("sequence diagram should parse")
        .expect("sequence diagram should be detected");

    match parsed.into_parts().1 {
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

fn cjk_test_width(input: &str) -> usize {
    input
        .chars()
        .map(|ch| if ch.is_ascii() { 1 } else { 2 })
        .sum()
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
        actor_lifecycles: None,
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
        central_connection: 0,
    }
}

fn empty_message(message_type: i32) -> SequenceMessage {
    let mut message = message(None, None, message_type);
    message.message = SequenceMessagePayload::Text(String::new());
    message
}

#[path = "sequence_model/actors.rs"]
mod actors;
#[path = "sequence_model/appearance.rs"]
mod appearance;
#[path = "sequence_model/control_composition.rs"]
mod control_composition;
#[path = "sequence_model/control_core.rs"]
mod control_core;
#[path = "sequence_model/control_extensions.rs"]
mod control_extensions;
#[path = "sequence_model/fixtures.rs"]
mod fixtures;
#[path = "sequence_model/layout_and_lifecycle.rs"]
mod layout_and_lifecycle;
#[path = "sequence_model/signals.rs"]
mod signals;
#[path = "sequence_model/validation.rs"]
mod validation;
