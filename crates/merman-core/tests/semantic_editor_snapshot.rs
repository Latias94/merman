#![cfg(feature = "editor-facts")]

#[cfg(feature = "diagram-class")]
use merman_core::{ParsedEditorFacts, SemanticDiagramModel};
use merman_core::{SemanticParser, SemanticSnapshotCapture};

#[cfg(feature = "diagram-class")]
#[test]
fn class_snapshot_captures_canonical_model_and_editor_facts_in_lean_core() {
    let capture = SemanticParser::new()
        .capture_diagram_snapshot_controlled_sync(
            "classDiagram\nclass User\n",
            &merman_core::OperationControl::new(),
        )
        .expect("snapshot operation should not be cancelled");
    let SemanticSnapshotCapture::Snapshot(snapshot) = capture else {
        panic!("valid Class input should produce a semantic snapshot");
    };
    let SemanticDiagramModel::Class(model) = snapshot.outcome().parsed_model().expect("parsed")
    else {
        panic!("snapshot should retain the canonical Class model");
    };
    assert!(model.classes.contains_key("User"));
    let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
        panic!("Class parser should provide editor facts");
    };
    assert!(facts.symbols.iter().any(|symbol| symbol.name == "User"));
    let capability = merman_core::diagram_family_capabilities()
        .iter()
        .find(|fact| fact.diagram_type == "classDiagram")
        .expect("known Class identity");
    assert!(capability.has_editor_parser);
    #[cfg(not(feature = "presentation"))]
    assert!(!capability.has_render_parser);
}

#[cfg(feature = "diagram-class")]
#[test]
fn failed_class_capture_preserves_recovery_facts_and_original_source_spans() {
    let source = "\u{feff}---\r\ntitle: \u{4e2d}\u{6587}\r\n---\r\nclassDiagram\r\nclass User {\r\n+field\r\n";
    let parser = SemanticParser::new();
    let merman_core::Error::DiagramParse {
        diagnostic: strict, ..
    } = parser.parse_diagram_sync(source).unwrap_err()
    else {
        panic!("incomplete Class input must fail strict parsing");
    };
    for known_type in [false, true] {
        let capture = if known_type {
            parser.capture_diagram_snapshot_with_type_sync("classDiagram", source)
        } else {
            parser.capture_diagram_snapshot_sync(source)
        };
        let SemanticSnapshotCapture::Snapshot(snapshot) = capture else {
            panic!("family failure must retain a snapshot");
        };
        let merman_core::SemanticParseOutcome::Failed(merman_core::Error::DiagramParse {
            diagnostic,
            ..
        }) = snapshot.outcome()
        else {
            panic!("snapshot must retain the original Class parse error");
        };
        assert_eq!(*diagnostic, strict);
        let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
            panic!("grammar failure must retain parser facts");
        };
        assert_eq!(
            facts.completeness,
            merman_core::EditorSemanticCompleteness::Recovered
        );
        let symbol = facts
            .symbols
            .iter()
            .find(|symbol| symbol.name == "User")
            .expect("partial class declaration");
        let start = source.find("User").unwrap();
        assert_eq!(
            symbol.selection,
            merman_core::SourceSpan::new(start, start + "User".len())
        );
    }
}

#[cfg(feature = "diagram-treemap")]
#[test]
fn treemap_snapshot_keeps_requested_facts_before_semantic_source_conversion() {
    let parser = SemanticParser::new();
    for (body, recovered) in [
        ("\"Root\"\r\n  \"Leaf\": 42\r\n", false),
        ("\"Root\"\r\n  \"Broken\":\r\n", true),
    ] {
        let source = format!("---\r\ntitle: Snapshot\r\n---\r\ntreemap\r\n{body}");
        let SemanticSnapshotCapture::Snapshot(snapshot) =
            parser.capture_diagram_snapshot_sync(&source)
        else {
            panic!("Treemap parsing must retain its requested snapshot");
        };
        assert_eq!(snapshot.outcome().parsed_model().is_none(), recovered);
        let merman_core::ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
            panic!("Treemap must retain requested editor facts");
        };
        let expected = if recovered {
            merman_core::EditorSemanticCompleteness::Recovered
        } else {
            merman_core::EditorSemanticCompleteness::Complete
        };
        assert_eq!(facts.completeness, expected);
        let root_start = source.find("Root").unwrap();
        assert!(facts.symbols.iter().any(|symbol| {
            symbol.name == "Root"
                && symbol.selection == merman_core::SourceSpan::new(root_start, root_start + 4)
        }));
        if recovered {
            let broken_start = source.find("\"Broken\":").unwrap();
            assert!(facts.diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains("expected number")
                    && diagnostic.span
                        == Some(merman_core::SourceSpan::new(broken_start, broken_start + 9))
            }));
        } else {
            assert!(facts.symbols.iter().any(|symbol| symbol.name == "Leaf"));
        }
        let SemanticSnapshotCapture::Snapshot(recovery) = parser
            .capture_diagram_recovery_snapshot_controlled_sync(
                &source,
                &merman_core::OperationControl::new(),
            )
            .unwrap()
        else {
            panic!("Treemap recovery must retain its snapshot");
        };
        let merman_core::ParsedEditorFacts::Available(recovery_facts) = recovery.editor_facts()
        else {
            panic!("Treemap recovery must retain its diagnostics");
        };
        assert_eq!(recovery_facts.completeness, facts.completeness);
        assert_eq!(recovery_facts.diagnostics, facts.diagnostics);
        assert!(recovery_facts.symbols.is_empty());
        assert!(recovery_facts.expected_syntax.is_empty());
        assert!(recovery_facts.directive_prefixes.is_empty());
    }
}

#[test]
fn capture_keeps_source_config_when_preprocessing_fails_without_any_family() {
    let nested = format!(
        "{}true{}",
        "{".repeat(merman_core::MAX_DIAGRAM_NESTING_DEPTH + 1),
        "}".repeat(merman_core::MAX_DIAGRAM_NESTING_DEPTH + 1)
    );
    let source = format!(
        "---\nconfig:\n  fontFamily: Serif\n---\n%%{{init: {{fontSize: 18}}}}%%\n%%{{init: {nested}}}%%\nclassDiagram\n"
    );
    let capture =
        SemanticParser::new().capture_diagram_snapshot_with_type_sync("classDiagram", &source);
    let SemanticSnapshotCapture::Failed {
        error,
        source_config,
    } = capture
    else {
        panic!("over-budget directive must fail before family construction");
    };
    assert!(matches!(
        error,
        merman_core::Error::InvalidDirectiveJson { .. }
    ));
    assert_eq!(source_config.directives().len(), 2);
    assert!(
        source_config
            .keys()
            .iter()
            .any(|key| key.matches_path(&["config", "fontFamily"]))
    );
}

#[test]
fn pre_cancelled_capture_never_becomes_a_recovery_snapshot() {
    let parser = SemanticParser::new();
    let control = merman_core::OperationControl::new();
    control.cancel();
    assert!(
        parser
            .capture_diagram_snapshot_controlled_sync("unknown", &control)
            .is_err()
    );
    assert!(
        parser
            .capture_diagram_snapshot_with_type_controlled_sync("classDiagram", "unknown", &control)
            .is_err()
    );
}
