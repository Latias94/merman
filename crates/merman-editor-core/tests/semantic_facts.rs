mod support;

use merman_analysis::{FenceTextIndex, FenceTextIndexSource};
#[cfg(feature = "diagram-class")]
use merman_core::{EditorSemanticKind, SemanticParser};
use merman_core::{EditorSemanticRole, EditorSemanticSymbol};
use merman_editor_core::DocumentKind;
#[cfg(feature = "diagram-class")]
use merman_editor_core::{Position, Range, goto_definition, prepare_rename, references, rename};
#[cfg(feature = "diagram-class")]
use merman_model::semantic::SemanticDiagramModel;
use support::SnapshotHarness;

fn outline_items(index: &FenceTextIndex) -> impl Iterator<Item = &EditorSemanticSymbol> {
    index
        .semantic_items()
        .iter()
        .filter(|item| item.role.contributes_outline())
}

#[test]
#[cfg(feature = "diagram-gantt")]
fn gantt_empty_end_fields_keep_dependency_navigation_and_rename() {
    use merman_editor_core::{Position, Range, goto_definition, references, rename};

    let source = "gantt\r\ndateFormat YYYY-MM-DD\r\nA:a,2026-01-01,  \r\nB:b,after a,1d\r\n";
    let snapshot = SnapshotHarness::new()
        .analyze(
            "file:///tmp/gantt-empty-end.mmd",
            1,
            source,
            DocumentKind::Diagram,
        )
        .unwrap();
    assert_eq!(
        snapshot.fences()[0].text_index().source(),
        FenceTextIndexSource::ParserComplete
    );
    let ranges = [source.find("a,").unwrap(), source.rfind("a,").unwrap()].map(|offset| {
        let position = snapshot.source_map().utf16_position(offset).unwrap();
        Range::new(
            Position::new(position.line, position.character),
            Position::new(position.line, position.character + 1),
        )
    });
    assert_eq!(
        goto_definition(&snapshot, ranges[1].start).unwrap().range,
        ranges[0]
    );
    assert_eq!(
        references(&snapshot, ranges[1].start, true)
            .unwrap()
            .iter()
            .map(|location| location.range)
            .collect::<Vec<_>>(),
        ranges
    );
    let edit = rename(&snapshot, ranges[1].start, "renamed")
        .unwrap()
        .unwrap();
    assert_eq!(
        edit.changes[snapshot.uri()]
            .iter()
            .map(|change| change.range)
            .collect::<Vec<_>>(),
        ranges
    );
}

#[test]
#[cfg(feature = "diagram-gantt")]
fn gantt_tasks_after_directives_keep_navigation_and_applied_rename() {
    use merman_editor_core::{Position, goto_definition, rename};

    let diagram = concat!(
        "gantt\r\n",
        "dateFormat YYYY-MM-DD;\u{1f680} Build:build,2026-01-01,1d\r\n",
        "axisFormat %d#Ship:ship,after build,1d\r\n",
    );
    for markdown in [false, true] {
        let source = if markdown {
            format!("# Delivery\r\n\r\n```mermaid\r\n{diagram}```\r\n")
        } else {
            diagram.to_string()
        };
        let kind = if markdown {
            DocumentKind::Markdown
        } else {
            DocumentKind::Diagram
        };
        let snapshot = SnapshotHarness::new()
            .analyze("file:///tmp/gantt-directives.mmd", 1, source.as_str(), kind)
            .unwrap();
        assert_eq!(
            snapshot.fences()[0].text_index().source(),
            FenceTextIndexSource::ParserComplete
        );
        let definition_offset = source.find("build,").unwrap();
        let reference_offset = source.rfind("build,").unwrap();
        let position = snapshot
            .source_map()
            .utf16_position(reference_offset)
            .unwrap();
        let position = Position::new(position.line, position.character);
        let definition = goto_definition(&snapshot, position).unwrap();
        assert_eq!(
            snapshot.byte_offset_for_position(definition.range.start),
            Some(definition_offset)
        );
        let edits = rename(&snapshot, position, "release").unwrap().unwrap();
        assert_eq!(edits.changes[snapshot.uri()].len(), 2);
        let mut changed = source.clone();
        for edit in edits.changes[snapshot.uri()].iter().rev() {
            let start = snapshot.byte_offset_for_position(edit.range.start).unwrap();
            let end = snapshot.byte_offset_for_position(edit.range.end).unwrap();
            changed.replace_range(start..end, &edit.new_text);
        }
        assert_eq!(changed, source.replace("build", "release"));
        let reparsed = SnapshotHarness::new()
            .analyze(
                "file:///tmp/gantt-directives.mmd",
                2,
                changed.as_str(),
                kind,
            )
            .unwrap();
        assert_eq!(
            reparsed.fences()[0].text_index().source(),
            FenceTextIndexSource::ParserComplete
        );
        let position = reparsed
            .source_map()
            .utf16_position(changed.rfind("release,").unwrap())
            .unwrap();
        let definition =
            goto_definition(&reparsed, Position::new(position.line, position.character)).unwrap();
        assert_eq!(
            reparsed.byte_offset_for_position(definition.range.start),
            changed.find("release,")
        );
    }
}

#[test]
#[cfg(feature = "diagram-git-graph")]
fn gitgraph_overwritten_ids_do_not_participate_in_navigation_or_rename() {
    use merman_editor_core::{
        Position, Range, goto_definition, prepare_rename, references, rename,
    };

    let source = concat!(
        "gitGraph\r\ncommit id:\"stale\" id:\"live\"\r\n",
        "branch topic\r\ncommit id:\"child\"\r\ncheckout main\r\n",
        "cherry-pick id:\"stale\" id:\"child\"\r\n",
    );
    let snapshot = SnapshotHarness::new()
        .analyze(
            "file:///tmp/gitgraph-winning-ids.mmd",
            1,
            source,
            DocumentKind::Diagram,
        )
        .unwrap();
    let index = snapshot.fences()[0].text_index();
    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(!index.node_ids().any(|name| name == "stale"));
    for (offset, _) in source.match_indices("stale") {
        let position = snapshot.source_map().utf16_position(offset).unwrap();
        let position = Position::new(position.line, position.character);
        assert!(goto_definition(&snapshot, position).is_none());
        assert!(prepare_rename(&snapshot, position).is_none());
    }
    let ranges = source
        .match_indices("child")
        .map(|(offset, _)| {
            let position = snapshot.source_map().utf16_position(offset).unwrap();
            Range::new(
                Position::new(position.line, position.character),
                Position::new(position.line, position.character + 5),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(ranges.len(), 2);
    assert_eq!(
        goto_definition(&snapshot, ranges[1].start).unwrap().range,
        ranges[0]
    );
    assert_eq!(
        references(&snapshot, ranges[1].start, true)
            .unwrap()
            .iter()
            .map(|location| location.range)
            .collect::<Vec<_>>(),
        ranges
    );
    let edit = rename(&snapshot, ranges[1].start, "renamed")
        .unwrap()
        .unwrap();
    assert_eq!(
        edit.changes[snapshot.uri()]
            .iter()
            .map(|change| change.range)
            .collect::<Vec<_>>(),
        ranges
    );
}

#[test]
#[cfg(feature = "diagram-er")]
fn er_trimmed_subgraph_identity_resolves_style_navigation() {
    use merman_editor_core::{Position, Range, goto_definition, references};

    let source = "erDiagram\r\nsubgraph \" \u{feff}G\u{feff} \"\r\nend\r\nstyle G fill:red\r\n";
    let snapshot = SnapshotHarness::new()
        .analyze(
            "file:///tmp/er-trimmed-subgraph.mmd",
            1,
            source,
            DocumentKind::Diagram,
        )
        .unwrap();
    assert_eq!(
        snapshot.fences()[0].text_index().source(),
        FenceTextIndexSource::ParserComplete
    );
    let ranges = source
        .match_indices('G')
        .map(|(offset, _)| {
            let position = snapshot.source_map().utf16_position(offset).unwrap();
            Range::new(
                Position::new(position.line, position.character),
                Position::new(position.line, position.character + 1),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(ranges.len(), 2);
    assert_eq!(
        goto_definition(&snapshot, ranges[1].start).unwrap().range,
        ranges[0]
    );
    assert_eq!(
        references(&snapshot, ranges[1].start, true)
            .unwrap()
            .iter()
            .map(|location| location.range)
            .collect::<Vec<_>>(),
        ranges
    );
}

#[test]
#[cfg(feature = "diagram-er")]
fn er_rename_preserves_adjacent_numeric_entity_boundaries() {
    use merman_core::SemanticParser;
    use merman_editor_core::{Position, rename};
    use merman_model::semantic::SemanticDiagramModel;

    for (pair, old_name, new_name, first, endpoint) in [
        ("12abc", "12", "X", "X", "abc"),
        ("12abc", "12", "1", "1", "abc"),
        ("12abc", "abc", "3", "12", "3"),
        ("1abc", "abc", "3", "1", "3"),
        ("12.3abc", "12.3", "X", "X", "abc"),
        ("12\"abc\"", "12", "X", "X", "abc"),
        ("\"12abc\"", "12abc", "X", "X", "X"),
        ("A", "A", "12abc", "12abc", "12abc"),
        ("\"A\"", "A", "12abc", "12abc", "12abc"),
        ("\"12\"abc", "abc", "3", "12", "3"),
    ] {
        for markdown in [false, true] {
            let diagram = format!("erDiagram\r\n{pair} ||--|| B : rel\r\n");
            let source = if markdown {
                format!("# Example\r\n\r\n```mermaid\r\n{diagram}```\r\n")
            } else {
                diagram.clone()
            };
            let snapshot = SnapshotHarness::new()
                .analyze(
                    "file:///tmp/er-rename-boundaries.mmd",
                    1,
                    source.as_str(),
                    if markdown {
                        DocumentKind::Markdown
                    } else {
                        DocumentKind::Diagram
                    },
                )
                .unwrap();
            let offset = source.find(old_name).unwrap();
            let position = snapshot.source_map().utf16_position(offset).unwrap();
            let edits = rename(
                &snapshot,
                Position::new(position.line, position.character),
                new_name,
            )
            .unwrap()
            .unwrap();
            let mut changed = source.clone();
            for edit in edits.changes[snapshot.uri()].iter().rev() {
                let start = snapshot.byte_offset_for_position(edit.range.start).unwrap();
                let end = snapshot.byte_offset_for_position(edit.range.end).unwrap();
                changed.replace_range(start..end, &edit.new_text);
            }
            let changed_diagram = if markdown {
                changed
                    .split("```mermaid\r\n")
                    .nth(1)
                    .unwrap()
                    .split("```")
                    .next()
                    .unwrap()
            } else {
                &changed
            };
            let parsed = SemanticParser::new()
                .parse_diagram_sync(changed_diagram)
                .unwrap();
            let SemanticDiagramModel::Er(model) = parsed.model else {
                panic!("expected ER model");
            };
            let expected = if first == endpoint {
                vec![first, "B"]
            } else {
                vec![first, endpoint, "B"]
            };
            assert_eq!(
                model
                    .entities
                    .keys()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
                expected,
                "{changed_diagram}"
            );
            assert_eq!(model.relationships[0].entity_a, model.entities[endpoint].id);
            assert_eq!(model.relationships[0].entity_b, model.entities["B"].id);
        }
    }
}

#[test]
#[cfg(feature = "diagram-er")]
fn er_numeric_rename_keeps_quoted_names_and_style_lists_in_their_parser_contexts() {
    use merman_core::SemanticParser;
    use merman_editor_core::{Position, rename};
    use merman_model::semantic::SemanticDiagramModel;

    for header in [
        "A",
        "\"A\"",
        "subgraph \" \u{feff}A\u{feff} \"\r\nInner\r\nend\r\nA",
    ] {
        let source = format!(
            "erDiagram\r\n{header} ||--|| B : rel\r\nstyle A fill:red\r\nclass A accent\r\nclassDef accent stroke:blue\r\n"
        );
        let snapshot = SnapshotHarness::new()
            .analyze(
                "file:///tmp/er-numeric-style-rename.mmd",
                1,
                source.as_str(),
                DocumentKind::Diagram,
            )
            .unwrap();
        let offset = source.find('A').unwrap();
        let position = snapshot.source_map().utf16_position(offset).unwrap();
        let edits = rename(
            &snapshot,
            Position::new(position.line, position.character),
            "12abc",
        )
        .unwrap()
        .unwrap();
        let mut changed = source.clone();
        for edit in edits.changes[snapshot.uri()].iter().rev() {
            let start = snapshot.byte_offset_for_position(edit.range.start).unwrap();
            let end = snapshot.byte_offset_for_position(edit.range.end).unwrap();
            changed.replace_range(start..end, &edit.new_text);
        }
        assert!(changed.contains("style 12abc fill:red"));
        assert!(changed.contains("class 12abc accent"));
        let parsed = SemanticParser::new().parse_diagram_sync(&changed).unwrap();
        let SemanticDiagramModel::Er(model) = parsed.model else {
            panic!("expected ER model");
        };
        if header.starts_with("subgraph") {
            assert_eq!(model.subgraphs[0].id, "12abc");
            assert_eq!(model.subgraphs[0].css_styles, ["fill:red"]);
            assert_eq!(model.subgraphs[0].classes, ["accent"]);
            assert_eq!(model.relationships[0].entity_a, "12abc");
        } else {
            assert_eq!(
                model
                    .entities
                    .keys()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
                ["12abc", "B"]
            );
            assert_eq!(model.entities["12abc"].css_styles, ["fill:red"]);
            assert_eq!(model.entities["12abc"].css_classes, "default accent");
            assert_eq!(model.relationships[0].entity_a, model.entities["12abc"].id);
        }
    }
}

#[test]
#[cfg(feature = "diagram-class")]
fn class_numeric_collisions_keep_parser_identity_and_rename_groups_distinct() {
    let source = concat!(
        "classDiagram\r\n",
        "namespace Raw {\r\n",
        "class `1`~T~\r\n",
        "}\r\n",
        "namespace Prefixed {\r\n",
        "class `classId-1`~U~\r\n",
        "}\r\n",
        "`1`~Other~ --> `classId-1`~Other~\r\n",
        "`1`: +rawField\r\n",
        "`classId-1`: +prefixedField\r\n",
        "callback `1` \"open\"\r\n",
        "callback `classId-1` \"open\"\r\n",
    );
    let snapshot = SnapshotHarness::new()
        .analyze(
            "file:///tmp/class-numeric-collisions.mmd",
            1,
            source,
            DocumentKind::Diagram,
        )
        .expect("numeric Class collisions remain valid editor input");
    let index = snapshot.fences()[0].text_index();
    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    for (id, token) in [("1", "`1`"), ("classId-1", "`classId-1`")] {
        assert!(index.node_ids().any(|candidate| candidate == id));
        let expected = source
            .match_indices(token)
            .map(|(start, _)| {
                let start = snapshot.source_map().utf16_position(start + 1).unwrap();
                Range::new(
                    Position::new(start.line, start.character),
                    Position::new(start.line, start.character + id.len()),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(expected.len(), 4);
        let symbols = index
            .semantic_items()
            .iter()
            .filter(|item| item.kind == EditorSemanticKind::Class && item.name == id)
            .collect::<Vec<_>>();
        assert_eq!(symbols.len(), expected.len());
        assert_eq!(symbols[0].role, EditorSemanticRole::Entity);
        for symbol in symbols {
            assert_eq!(&source[symbol.selection.start..symbol.selection.end], id);
        }
        for range in &expected {
            assert_eq!(
                goto_definition(&snapshot, range.start).unwrap().range,
                expected[0]
            );
            let locations = references(&snapshot, range.start, true).unwrap();
            assert_eq!(
                locations
                    .iter()
                    .map(|location| location.range)
                    .collect::<Vec<_>>(),
                expected
            );
            let prepared = prepare_rename(&snapshot, range.start).unwrap();
            assert_eq!(prepared.placeholder, id);
            assert_eq!(prepared.range, *range);
            let edit = rename(&snapshot, range.start, "Renamed").unwrap().unwrap();
            let changes = &edit.changes[snapshot.uri()];
            assert_eq!(
                changes
                    .iter()
                    .map(|change| change.range)
                    .collect::<Vec<_>>(),
                expected
            );
            let mut changed = source.to_string();
            for change in changes.iter().rev() {
                let start = snapshot
                    .byte_offset_for_position(change.range.start)
                    .unwrap();
                let end = snapshot.byte_offset_for_position(change.range.end).unwrap();
                changed.replace_range(start..end, &change.new_text);
            }
            assert_eq!(changed, source.replace(token, "`Renamed`"));
        }
    }
}

#[test]
#[cfg(feature = "diagram-class")]
fn class_note_payloads_match_canonical_text_and_crlf_unicode_source_selections() {
    let payload = " \t名𠀀\n尾 \t";
    let source = format!(
        "classDiagram\r\nnamespace Outer {{\r\nnamespace Inner {{\r\nclass Owner\r\nnote for Owner \"{payload}\"\r\nnote \"{payload}\"\r\n}}\r\n}}\r\n"
    );
    let snapshot = SnapshotHarness::new()
        .analyze(
            "file:///tmp/class-note-payloads.mmd",
            1,
            source.clone(),
            DocumentKind::Diagram,
        )
        .expect("raw Class note text remains valid editor input");
    let index = snapshot.fences()[0].text_index();
    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    let semantic = SemanticParser::new().parse_diagram_sync(&source).unwrap();
    let SemanticDiagramModel::Class(model) = semantic.model else {
        panic!("expected canonical Class model");
    };
    let notes = index
        .semantic_items()
        .iter()
        .filter(|item| item.detail.as_deref() == Some("class note"))
        .collect::<Vec<_>>();
    assert_eq!(notes.len(), 2);
    assert_eq!(model.notes.len(), 2);
    for (symbol, note) in notes.into_iter().zip(&model.notes) {
        assert_eq!(note.text, payload);
        assert_eq!(symbol.name, note.text);
        assert_eq!(symbol.role, EditorSemanticRole::Payload);
        assert_eq!(symbol.kind, EditorSemanticKind::String);
        assert_eq!(
            &source[symbol.selection.start..symbol.selection.end],
            note.text
        );
        assert_eq!(
            &source[symbol.span.start..symbol.span.end],
            format!("\"{payload}\"")
        );
        let position = snapshot
            .source_map()
            .utf16_position(symbol.selection.start + " \t".len())
            .unwrap();
        let position = Position::new(position.line, position.character);
        assert!(references(&snapshot, position, true).is_none());
        assert!(prepare_rename(&snapshot, position).is_none());
    }
    assert_eq!(model.notes[0].class_id.as_deref(), Some("Owner"));
    assert_eq!(model.notes[1].class_id, None);
    assert_eq!(model.namespaces["Outer.Inner"].note_ids, ["note0", "note1"]);
    assert!(
        model
            .notes
            .iter()
            .all(|note| note.parent.as_deref() == Some("Outer.Inner"))
    );
}

#[test]
fn product_families_are_parser_backed_and_role_aware() {
    struct CapabilityCase<'a> {
        label: &'a str,
        expected_source: FenceTextIndexSource,
        snippet: &'a str,
        required_ids: &'a [&'a str],
        required_outline: &'a [&'a str],
        required_prefixes: &'a [&'a str],
        forbidden_ids: &'a [&'a str],
    }

    let cases = [
        CapabilityCase {
            label: "agentflow",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: "agentflow-beta\nflow Team\nA[Worker]-->B[Result]\nend\n",
            required_ids: &["A", "B"],
            required_outline: &["Team"],
            required_prefixes: &[],
            forbidden_ids: &[],
        },
        CapabilityCase {
            label: "usecase",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: "usecase-beta\nactor User\nsystemBoundary System\nLogin(Sign in)\nend\nUser --> Login\n",
            required_ids: &["User", "Login"],
            required_outline: &["System"],
            required_prefixes: &[],
            forbidden_ids: &[],
        },
        CapabilityCase {
            label: "agentflow-recovered",
            expected_source: FenceTextIndexSource::ParserRecovered,
            snippet: "agentflow-beta\nflow Team\nA[Worker]-->B[Result]\nend\nC -->",
            required_ids: &["A", "B"],
            required_outline: &["Team"],
            required_prefixes: &[],
            forbidden_ids: &["Worker", "Result"],
        },
        CapabilityCase {
            label: "usecase-recovered",
            expected_source: FenceTextIndexSource::ParserRecovered,
            snippet: "usecase-beta\nactor User\nsystemBoundary System\nLogin(Sign in)\nend\nUser --> Login\nUser -->",
            required_ids: &["User", "Login"],
            required_outline: &["System"],
            required_prefixes: &[],
            forbidden_ids: &["Sign in"],
        },
        CapabilityCase {
            label: "flowchart",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: "flowchart TD\nA-->B\n",
            required_ids: &["A", "B"],
            required_outline: &[],
            required_prefixes: &[],
            forbidden_ids: &[],
        },
        CapabilityCase {
            label: "flowchart-recovered",
            expected_source: FenceTextIndexSource::ParserRecovered,
            snippet: "flowchart TD\nsubgraph group\nA-->B\nC-->",
            required_ids: &["A", "C"],
            required_outline: &["group"],
            required_prefixes: &[],
            forbidden_ids: &[],
        },
        CapabilityCase {
            label: "sequence",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: "sequenceDiagram\nAlice->>Bob: Hi\n",
            required_ids: &["Alice", "Bob"],
            required_outline: &[],
            required_prefixes: &[],
            forbidden_ids: &[],
        },
        CapabilityCase {
            label: "sequence-recovered",
            expected_source: FenceTextIndexSource::ParserRecovered,
            snippet: "sequenceDiagram\nAlice->>Bob: Hi\nBob->>",
            required_ids: &["Alice", "Bob"],
            required_outline: &[],
            required_prefixes: &[],
            forbidden_ids: &[],
        },
        CapabilityCase {
            label: "state",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: concat!(
                "stateDiagram-v2\n",
                "Idle --> Running\n",
                "state \"Paused State\" as Paused\n",
                "classDef activeStyle fill:#0f0,border:#333\n",
                "class Idle, Running activeStyle\n",
                "style Running fill:#f00\n",
            ),
            required_ids: &["Idle", "Running"],
            required_outline: &["activeStyle"],
            required_prefixes: &[],
            forbidden_ids: &[],
        },
        CapabilityCase {
            label: "class",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: concat!(
                "classDiagram\n",
                "class User {\n",
                "  +login()\n",
                "}\n",
                "class User:::service\n",
                "classDef service fill:#eee\n",
                "User <|-- Admin\n",
            ),
            required_ids: &["User", "Admin"],
            required_outline: &["+login()", "service"],
            required_prefixes: &[],
            forbidden_ids: &[],
        },
        CapabilityCase {
            label: "er",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: "erDiagram\nCUSTOMER ||--o{ ORDER : places\n",
            required_ids: &["CUSTOMER", "ORDER"],
            required_outline: &[],
            required_prefixes: &[],
            forbidden_ids: &[],
        },
        CapabilityCase {
            label: "gantt",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: concat!(
                "gantt\n",
                "title Roadmap\n",
                "accTitle: Roadmap chart\n",
                "accDescr: Shows release tasks\n",
                "section Demo\n",
                "Task 1: id1,2014-01-01,1d\n",
                "Task 2: id2,after id1,2d\n",
            ),
            required_ids: &["id1", "id2"],
            required_outline: &["Demo"],
            required_prefixes: &["title", "accTitle", "accDescr", "section"],
            forbidden_ids: &[],
        },
        CapabilityCase {
            label: "mindmap",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: "mindmap\nroot(Root Node)\n child1(Child 1)\n child2\n",
            required_ids: &["root", "child1", "child2"],
            required_outline: &[],
            required_prefixes: &[],
            forbidden_ids: &["Root", "Node", "Child", "1"],
        },
        CapabilityCase {
            label: "gitGraph",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: concat!(
                "gitGraph\n",
                "commit id:\"C1\"\n",
                "branch feature\n",
                "checkout feature\n",
                "commit id:\"F1\"\n",
                "merge main id:\"M1\"\n",
            ),
            required_ids: &["C1", "feature", "F1", "M1"],
            required_outline: &[],
            required_prefixes: &[],
            forbidden_ids: &["main"],
        },
        CapabilityCase {
            label: "radar",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: concat!(
                "radar-beta\n",
                "title Radar diagram\n",
                "accTitle: Radar accTitle\n",
                "accDescr: Radar accDescription\n",
                "axis A[\"Axis A\"], B[\"Axis B\"], C[\"Axis C\"]\n",
                "curve mycurve[\"My Curve\"]{1,2,3}\n",
            ),
            required_ids: &["A", "B", "C", "mycurve"],
            required_outline: &[],
            required_prefixes: &["title", "accTitle", "accDescr"],
            forbidden_ids: &[],
        },
        CapabilityCase {
            label: "kanban",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: "kanban\n    root\n      child1\n",
            required_ids: &["child1"],
            required_outline: &["root"],
            required_prefixes: &[],
            forbidden_ids: &[],
        },
        CapabilityCase {
            label: "treemap",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: concat!(
                "treemap\n",
                "title Treemap Title\n",
                "accTitle: Treemap accTitle\n",
                "accDescr: Treemap accDescr\n",
                "\"Root\"\n",
                "  \"Leaf\": 42 :::highlight\n",
                "classDef highlight fill:#f00\n",
            ),
            required_ids: &["Root", "Leaf"],
            required_outline: &["highlight"],
            required_prefixes: &["title", "accTitle", "accDescr"],
            forbidden_ids: &[],
        },
        CapabilityCase {
            label: "block",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: concat!(
                "block\n",
                "  columns 2\n",
                "  block:group[\"Group label\"]\n",
                "    A[\"Start\"] -- \"edge label\" --> B[\"End\"]\n",
                "  end\n",
                "  classDef hot fill:#f00\n",
                "  class A,B hot\n",
                "  style B stroke:#333\n",
            ),
            required_ids: &["group", "A", "B"],
            required_outline: &["hot"],
            required_prefixes: &["classDef", "class", "style"],
            forbidden_ids: &["Start", "End", "edge", "label"],
        },
        CapabilityCase {
            label: "c4",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: concat!(
                "C4Context\n",
                "title Banking Context\n",
                "accTitle: Banking accessibility title\n",
                "accDescr: Banking accessibility description\n",
                "Boundary(bank, \"Bank\") {\n",
                "Person(customer, \"Customer\", \"Uses the system\")\n",
                "System(system, \"Internet Banking\", \"Core system\")\n",
                "}\n",
                "Rel(customer, system, \"Uses\", \"HTTPS\")\n",
                "UpdateElementStyle(system, $bgColor=\"red\")\n",
                "UpdateRelStyle(customer, system, $lineColor=\"blue\")\n",
            ),
            required_ids: &["bank", "customer", "system"],
            required_outline: &[],
            required_prefixes: &["title", "accTitle", "accDescr"],
            forbidden_ids: &[
                "Banking", "Context", "Bank", "Customer", "Uses", "Internet", "Core", "red", "blue",
            ],
        },
        CapabilityCase {
            label: "zenuml",
            expected_source: FenceTextIndexSource::ParserComplete,
            snippet: concat!(
                "zenuml\n",
                "title Login Flow\n",
                "Alice\n",
                "Bob\n",
                "A as API\n",
                "accTitle: Login accessibility title\n",
                "accDescr: Login accessibility description\n",
                "Alice->Bob: Login\n",
                "SomeType result = A.SyncMessage()\n",
                "new Session(with, params)\n",
            ),
            required_ids: &["Alice", "Bob", "A", "accTitle", "accDescr", "Session"],
            required_outline: &[],
            required_prefixes: &["title"],
            forbidden_ids: &[
                "Login",
                "Flow",
                "accessibility",
                "description",
                "API",
                "SyncMessage",
                "result",
                "params",
            ],
        },
    ];

    for case in cases {
        let harness = SnapshotHarness::new();
        let snapshot = harness
            .analyze(
                "file:///tmp/example.mmd",
                1,
                case.snippet.to_string(),
                DocumentKind::Diagram,
            )
            .expect("test source should be accepted");
        let index = snapshot.fences()[0].text_index();

        assert_eq!(
            index.source(),
            case.expected_source,
            "unexpected provenance for {}",
            case.label
        );
        for id in case.required_ids {
            assert!(
                index.node_ids().any(|candidate| candidate == id),
                "missing node id {id:?} for {}",
                case.label
            );
        }
        for name in case.required_outline {
            assert!(
                outline_items(index).any(|item| item.name == *name),
                "missing outline item {name:?} for {}",
                case.label
            );
        }
        for prefix in case.required_prefixes {
            assert!(
                index.has_directive_prefix(prefix),
                "missing directive prefix {prefix:?} for {}",
                case.label
            );
        }
        for forbidden in case.forbidden_ids {
            assert!(
                !index.node_ids().any(|candidate| candidate == forbidden),
                "forbidden node id {forbidden:?} leaked for {}",
                case.label
            );
        }
    }
}

#[test]
fn capability_matrix_families_are_parser_backed_in_editor_core() {
    let harness = SnapshotHarness::new();

    for (label, snippet) in [
        ("info", "info showInfo\n"),
        ("timeline", "timeline\nsection Alpha\ntask1\n"),
        ("packet", "packet\n0-10: \"first\"\n"),
        ("sankey", "sankey\nsource,target,1\n"),
        ("treeView", "treeView-beta\n\"Root\"\n    \"Child\"\n"),
        (
            "ishikawa",
            "ishikawa-beta\n  Problem\nCause A\n  Subcause A1\n",
        ),
        (
            "eventmodeling",
            concat!(
                "eventmodeling\n",
                "tf 01 cmd AddItem { productId: 7 }\n",
                "tf 02 evt ItemAdded ->> 01 [[ItemAddedData]]\n",
                "\n",
                "data ItemAddedData {\n",
                "  productId: 7\n",
                "}\n",
            ),
        ),
        ("gitGraph", "gitGraph\ncommit id:\"C1\"\nbranch feature\n"),
        ("kanban", "kanban\n    root\n      child1\n"),
        (
            "architecture",
            concat!(
                "architecture-beta\n",
                "group platform(cloud)[Platform]\n",
                "service api(server)[API] in platform\n",
                "junction hub in platform\n",
                "api:R -- L:hub\n",
            ),
        ),
        (
            "radar",
            concat!(
                "radar-beta\n",
                "title Radar diagram\n",
                "accTitle: Radar accTitle\n",
                "accDescr: Radar accDescription\n",
                "axis A[\"Axis A\"], B[\"Axis B\"], C[\"Axis C\"]\n",
                "curve mycurve[\"My Curve\"]{1,2,3}\n",
            ),
        ),
        (
            "treemap",
            concat!(
                "treemap\n",
                "title Treemap Title\n",
                "accTitle: Treemap accTitle\n",
                "accDescr: Treemap accDescr\n",
                "\"Root\"\n",
                "  \"Leaf\": 42 :::highlight\n",
                "classDef highlight fill:#f00\n",
            ),
        ),
        (
            "block",
            concat!(
                "block\n",
                "  A[\"Start\"] -- \"edge label\" --> B[\"End\"]\n",
                "  classDef hot fill:#f00\n",
                "  class A,B hot\n",
            ),
        ),
        (
            "c4",
            concat!(
                "C4Context\n",
                "Person(customer, \"Customer\")\n",
                "System(system, \"System\")\n",
                "Rel(customer, system, \"Uses\")\n",
            ),
        ),
        (
            "zenuml",
            concat!("zenuml\n", "Alice\n", "Bob\n", "Alice->Bob: Hi\n",),
        ),
        ("venn", "venn-beta\nset A\nset B\nunion A,B\n"),
    ] {
        let snapshot = harness
            .analyze(
                "file:///tmp/capability-matrix.mmd",
                1,
                snippet.to_string(),
                DocumentKind::Diagram,
            )
            .expect("test source should be accepted");
        assert_eq!(
            snapshot.fences()[0].text_index().source(),
            FenceTextIndexSource::ParserComplete,
            "unexpected parser provenance for {label}"
        );
    }

    let snapshot = harness
        .analyze(
            "file:///tmp/capability-matrix.mmd",
            1,
            "flowchart TD\nA-->B\n".to_string(),
            DocumentKind::Diagram,
        )
        .expect("test source should be accepted");
    let index = snapshot.fences()[0].text_index();
    assert_eq!(index.source(), FenceTextIndexSource::ParserComplete);
    assert!(index.node_ids().any(|id| id == "A"));
    assert!(index.node_ids().any(|id| id == "B"));
    assert!(
        index
            .semantic_items()
            .iter()
            .any(|item| item.name == "A" && item.role == EditorSemanticRole::Entity)
    );
}

#[cfg(feature = "diagram-pie")]
#[test]
fn pie_parse_failure_keeps_recovered_outline_and_completion_context() {
    let harness = SnapshotHarness::new();
    let source = "pie showData\n\"negative\": -1\n\"valid\": 2\n";
    let snapshot = harness
        .analyze(
            "file:///tmp/pie-recovered.mmd",
            1,
            source,
            DocumentKind::Diagram,
        )
        .expect("recoverable pie syntax remains available to editors");
    let index = snapshot.fences()[0].text_index();
    assert_eq!(index.source(), FenceTextIndexSource::ParserRecovered);
    for name in ["negative", "valid"] {
        assert!(outline_items(index).any(|item| item.name == name));
    }
    assert!(index.has_directive_prefix("showData"));
    assert!(!index.expected_syntax().is_empty());
}
