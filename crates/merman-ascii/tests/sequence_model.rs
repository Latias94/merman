use merman_ascii::{
    AsciiError, AsciiRenderOptions, render_model, render_sequence as render_sequence_model,
};
use merman_core::diagrams::sequence::{
    SequenceActor, SequenceBox, SequenceDiagramRenderModel, SequenceMessage, SequenceMessagePayload,
};
use merman_core::{Engine, ParseOptions};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn render_sequence(input: &str, options: &AsciiRenderOptions) -> merman_ascii::Result<String> {
    let parsed = Engine::new()
        .parse_diagram_for_render_model_sync(input, ParseOptions::strict())
        .expect("sequence diagram should parse")
        .expect("sequence diagram should be detected");

    render_model(&parsed.model, options)
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
fn sequence_notes_are_explicitly_unsupported() {
    let err = render_sequence(
        "sequenceDiagram\nparticipant A\nNote over A: remember",
        &AsciiRenderOptions::unicode(),
    )
    .unwrap_err();

    assert_eq!(
        err,
        AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "notes",
        }
    );
}

#[test]
fn sequence_activations_are_explicitly_unsupported() {
    let err = render_sequence(
        "sequenceDiagram\nparticipant A\nparticipant B\nA->>+B: Hello",
        &AsciiRenderOptions::unicode(),
    )
    .unwrap_err();

    assert_eq!(
        err,
        AsciiError::UnsupportedFeature {
            diagram_type: "sequence",
            feature: "activations",
        }
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
    model.boxes.push(SequenceBox {
        actor_keys: vec!["A".to_string()],
        fill: "transparent".to_string(),
        name: None,
        wrap: false,
    });
    cases.push((model, "boxes"));

    let mut model = basic_sequence_model();
    model.created_actors.insert("A".to_string(), 0);
    cases.push((model, "actor create/destroy"));

    let mut model = basic_sequence_model();
    model.messages.push(SequenceMessage {
        placement: Some(0),
        ..message(Some("A"), Some("A"), 0)
    });
    cases.push((model, "message placement"));

    let mut model = basic_sequence_model();
    model.messages.push(SequenceMessage {
        wrap: true,
        ..message(Some("A"), Some("A"), 0)
    });
    cases.push((model, "wrapped messages"));

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
