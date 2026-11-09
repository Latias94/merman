#![cfg(feature = "diagram-class")]

use merman_core::SemanticParser;
use merman_model::{
    families::class::{ClassDiagramModel, LINE_DOTTED, LINE_SOLID, REL_AGGREGATION, REL_NONE},
    semantic::SemanticDiagramModel,
};

fn class_model(source: &str) -> ClassDiagramModel {
    let parsed = SemanticParser::new()
        .parse_diagram_sync(source)
        .expect("valid Class source");
    let SemanticDiagramModel::Class(model) = parsed.model else {
        panic!("expected the canonical Class model");
    };
    model
}

#[test]
fn aggregation_markers_before_quoted_and_unicode_targets_keep_their_endpoint() {
    for newline in ["\n", "\r\n"] {
        for (authored, id, type_param) in [
            ("`Target`", "Target", ""),
            ("目标", "目标", ""),
            ("`Target name`~T~", "Target name", "T"),
            ("`目标😀`~T~", "目标😀", "T"),
        ] {
            for (line, line_type) in [("--", LINE_SOLID), ("..", LINE_DOTTED)] {
                let source = format!("classDiagram{newline}A{line}o{authored}{newline}");
                let model = class_model(&source);
                assert_eq!(model.classes.len(), 2, "{source:?}");
                assert!(model.classes.contains_key("A"));
                assert_eq!(model.classes[id].id, id);
                assert_eq!(model.classes[id].type_param, type_param);
                assert_eq!(model.relations.len(), 1);
                let relation = &model.relations[0];
                assert_eq!(relation.id1, "A");
                assert_eq!(relation.id2, id);
                assert_eq!(relation.relation.type1, REL_NONE);
                assert_eq!(relation.relation.type2, REL_AGGREGATION);
                assert_eq!(relation.relation.line_type, line_type);
            }
        }
    }
}

#[test]
fn longer_ascii_words_and_quoted_o_prefixes_remain_endpoint_names() {
    for (authored, id) in [
        ("other", "other"),
        ("oTarget", "oTarget"),
        ("o_target", "o_target"),
        ("o1", "o1"),
        ("`oTarget`", "oTarget"),
        ("`o目标`", "o目标"),
    ] {
        let source = format!("classDiagram\nA --{authored}\n");
        let model = class_model(&source);
        assert_eq!(model.classes.len(), 2, "{source:?}");
        assert!(model.classes.contains_key(id), "{source:?}");
        assert_eq!(model.relations[0].id2, id);
        assert_eq!(model.relations[0].relation.type1, REL_NONE);
        assert_eq!(model.relations[0].relation.type2, REL_NONE);
    }
}

#[cfg(feature = "editor-facts")]
#[test]
fn adjacent_aggregation_targets_keep_exact_authored_editor_ranges() {
    use merman_core::{
        EditorExpectedSyntaxKind, EditorRenamePolicy, EditorSemanticCompleteness,
        EditorSemanticKind, EditorSemanticRole, ParsedEditorFacts, SemanticSnapshotCapture,
    };
    use merman_model::source::SourceSpan;

    for newline in ["\n", "\r\n"] {
        let source = format!(
            "\u{feff}---{newline}title: 関係😀{newline}---{newline}classDiagram{newline}class Root{newline}class `目标😀`~T~{newline}Root --o`目标😀`~T~{newline}Root ..o目标{newline}"
        );
        let SemanticSnapshotCapture::Snapshot(snapshot) =
            SemanticParser::new().capture_diagram_snapshot_sync(&source)
        else {
            panic!("Class source must retain its semantic snapshot");
        };
        let SemanticDiagramModel::Class(model) = snapshot
            .outcome()
            .parsed_model()
            .expect("valid Class relation must produce its semantic model")
        else {
            panic!("expected the canonical Class model");
        };
        assert_eq!(model.relations[0].id2, "目标😀");
        assert_eq!(model.relations[1].id2, "目标");
        let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
            panic!("Class source must retain requested editor facts");
        };
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
        assert!(facts.diagnostics.is_empty(), "{:?}", facts.diagnostics);

        for (index, (start, _)) in source.match_indices("`目标😀`~T~").enumerate() {
            let span = SourceSpan::new(start, start + "`目标😀`~T~".len());
            let selection = SourceSpan::new(start + 1, start + 1 + "目标😀".len());
            let symbol = facts
                .symbols
                .iter()
                .find(|symbol| symbol.name == "目标😀" && symbol.span == span)
                .expect("declared and referenced generic names keep their token spans");
            assert_eq!(symbol.selection, selection);
            assert_eq!(symbol.kind, EditorSemanticKind::Class);
            assert_eq!(
                symbol.role,
                if index == 0 {
                    EditorSemanticRole::Entity
                } else {
                    EditorSemanticRole::Reference
                },
            );
            assert_eq!(symbol.rename_policy, EditorRenamePolicy::Identifier);
        }
        for (marker, authored) in [("--o", "`目标😀`~T~"), ("..o", "目标")] {
            let start = source.find(marker).unwrap() + marker.len();
            let span = SourceSpan::new(start, start + authored.len());
            assert!(facts.expected_syntax.iter().any(|expected| {
                expected.kind == EditorExpectedSyntaxKind::NodeIdentifier && expected.span == span
            }));
            assert!(!facts.symbols.iter().any(|symbol| symbol.name == "o目标"));
        }
    }
}
