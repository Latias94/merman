#![cfg(feature = "diagram-state")]

use merman_core::SemanticParser;
#[cfg(feature = "editor-facts")]
use merman_core::{
    EditorExpectedSyntaxKind, EditorSemanticCompleteness, EditorSemanticFacts, EditorSemanticRole,
    ParsedEditorFacts, SemanticSnapshotCapture,
};
use merman_model::families::state::{StateDiagramModel, StateLinks};
use merman_model::semantic::SemanticDiagramModel;
#[cfg(feature = "editor-facts")]
use merman_model::source::SourceSpan;

const ROOT_DIRECTIVES: &str = concat!(
    "stateDiagram-v2\n",
    "scale 100 width\n",
    "hide empty description\n",
    "state Outer {\n",
    "  click: Action\n",
    "  href: Target\n",
    "}\n",
    "scale 200 width\n",
    "hide empty description\n",
    "A --> B\n",
    "click A href \"https://example.test/a\"\n",
    "click B \"https://example.test/b\" \"B details\"\n",
);

fn state_model(source: &str) -> StateDiagramModel {
    let parsed = SemanticParser::new()
        .parse_diagram_sync(source)
        .expect("valid State source");
    let SemanticDiagramModel::State(model) = parsed.model else {
        panic!("expected the canonical State model");
    };
    model
}

#[test]
fn root_keywords_are_ordinary_states_inside_composites() {
    for header in ["stateDiagram", "stateDiagram-v2"] {
        for newline in ["\n", "\r\n"] {
            for (statement, ids, description) in [
                ("click: Action", &["click"][..], Some("Action")),
                ("href: Action", &["href"][..], Some("Action")),
                ("scale: Action", &["scale"][..], Some("Action")),
                ("scale 100 width", &["scale", "100", "width"][..], None),
                (
                    "hide empty description",
                    &["hide", "empty", "description"][..],
                    None,
                ),
            ] {
                let source =
                    format!("{header}\nstate Outer {{\n  {statement}\n}}\n").replace('\n', newline);
                let model = state_model(&source);
                assert_eq!(model.nodes.len(), ids.len() + 1, "{source}");
                assert!(model.links.is_empty(), "{source}");
                for id in ids {
                    let node = model.nodes.iter().find(|node| node.id == *id).unwrap();
                    assert_eq!(node.parent_id.as_deref(), Some("Outer"), "{source}");
                    assert_eq!(
                        node.label.as_ref().and_then(|label| label.as_str()),
                        Some(description.unwrap_or(id)),
                        "{source}",
                    );
                }
            }
        }
    }
}

#[test]
fn root_directives_work_before_and_after_composites() {
    let model = state_model(ROOT_DIRECTIVES);
    assert_eq!(model.nodes.len(), 5);
    assert_eq!(model.links.len(), 2);
    assert_eq!(model.edges.len(), 1);
    assert_eq!(
        (model.edges[0].start.as_str(), model.edges[0].end.as_str()),
        ("A", "B"),
    );
    for (id, url, tooltip) in [
        ("A", "https://example.test/a", ""),
        ("B", "https://example.test/b", "B details"),
    ] {
        let StateLinks::One(link) = &model.links[id] else {
            panic!("expected one root click declaration for {id}");
        };
        assert_eq!(link.url, url);
        assert_eq!(link.tooltip, tooltip);
    }
}

#[cfg(feature = "editor-facts")]
fn assert_original_fact(
    facts: &EditorSemanticFacts,
    source: &str,
    name: &str,
    role: EditorSemanticRole,
    marker: &str,
) {
    let start = source.find(marker).expect("authored marker");
    let selection = SourceSpan::new(start, start + name.len());
    let symbol = facts
        .symbols
        .iter()
        .find(|symbol| symbol.name == name && symbol.role == role && symbol.selection == selection)
        .unwrap_or_else(|| panic!("missing {role:?} fact for {name} at {selection:?}"));
    assert_eq!(
        source.get(symbol.selection.start..symbol.selection.end),
        Some(name),
    );
}

#[cfg(feature = "editor-facts")]
#[test]
fn nested_keyword_facts_keep_original_entity_reference_and_payload_ranges() {
    for newline in ["\n", "\r\n"] {
        let source = concat!(
            "\u{feff}---\ntitle: \u{4e2d}\u{6587}\n---\n",
            "%% removed before parsing\n",
            "stateDiagram-v2\n",
            "state Outer {\n",
            "  state Inner {\n",
            "    click: Click action\n",
            "    href: Href action\n",
            "    scale: Scale action\n",
            "    scale 100 width\n",
            "    hide empty description\n",
            "    click --> href: Continue\n",
            "  }\n",
            "}\n",
        )
        .replace('\n', newline);
        let SemanticSnapshotCapture::Snapshot(snapshot) =
            SemanticParser::new().capture_diagram_snapshot_sync(&source)
        else {
            panic!("valid State source must retain a snapshot");
        };
        let SemanticDiagramModel::State(model) = snapshot.outcome().parsed_model().unwrap() else {
            panic!("expected the canonical State model");
        };
        assert_eq!(
            (model.edges[0].start.as_str(), model.edges[0].end.as_str()),
            ("click", "href"),
        );
        let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
            panic!("State capture must retain editor facts");
        };
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
        assert!(facts.diagnostics.is_empty());
        for (name, marker) in [
            ("click", "click: Click action"),
            ("href", "href: Href action"),
            ("scale", "scale: Scale action"),
            ("scale", "scale 100 width"),
            ("100", "100 width"),
            ("width", "width"),
            ("hide", "hide empty description"),
            ("empty", "empty description"),
            ("description", "description"),
        ] {
            assert_original_fact(facts, &source, name, EditorSemanticRole::Entity, marker);
        }
        for (name, marker) in [("click", "click --> href"), ("href", "href: Continue")] {
            assert_original_fact(facts, &source, name, EditorSemanticRole::Reference, marker);
        }
        for name in ["Click action", "Href action", "Scale action", "Continue"] {
            assert_original_fact(facts, &source, name, EditorSemanticRole::Payload, name);
        }
        assert!(
            !facts
                .directive_prefixes
                .iter()
                .any(|prefix| prefix == "click")
        );
        let start = source.find("click: Click action").unwrap();
        assert!(facts.expected_syntax.iter().any(|expected| {
            expected.kind == EditorExpectedSyntaxKind::NodeIdentifier
                && expected.span == SourceSpan::new(start, start + "click".len())
        }));
    }
}

#[cfg(feature = "editor-facts")]
#[test]
fn root_interactions_keep_reference_action_and_payload_facts() {
    let source = format!("\u{feff}---\r\ntitle: Root controls\r\n---\r\n{ROOT_DIRECTIVES}");
    let SemanticSnapshotCapture::Snapshot(snapshot) =
        SemanticParser::new().capture_diagram_snapshot_sync(&source)
    else {
        panic!("root State directives must retain a snapshot");
    };
    assert!(snapshot.outcome().parsed_model().is_some());
    let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
        panic!("State capture must retain editor facts");
    };
    assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
    assert!(facts.diagnostics.is_empty());
    assert!(
        facts
            .directive_prefixes
            .iter()
            .any(|prefix| prefix == "click")
    );
    for (name, marker) in [("A", "A href"), ("B", "B \"https")] {
        assert_original_fact(facts, &source, name, EditorSemanticRole::Reference, marker);
    }
    for name in [
        "https://example.test/a",
        "https://example.test/b",
        "B details",
    ] {
        assert_original_fact(facts, &source, name, EditorSemanticRole::Payload, name);
    }
    let start = source.find("href \"https").unwrap();
    assert!(facts.expected_syntax.iter().any(|expected| {
        expected.kind == EditorExpectedSyntaxKind::InteractionAction
            && expected.span == SourceSpan::new(start, start + "href".len())
    }));
}
