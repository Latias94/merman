#![cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-class",
    feature = "diagram-gantt"
))]

mod support;

use merman_analysis::FenceTextIndexSource;
use merman_editor_core::{DocumentKind, Position, goto_definition, references, rename};
use support::SnapshotHarness;

#[cfg(feature = "diagram-flowchart")]
#[test]
fn early_style_targets_keep_navigation_and_rename_after_recovery() {
    for broken in [false, true] {
        for markdown in [false, true] {
            let diagram = format!(
                concat!(
                    "\u{feff}---\r\ntitle: Diagram 😀\r\n---\r\nflowchart TD\r\n",
                    "style 雪 fill:red\r\nA-->雪[Later]\r\n",
                    "class 雪 accent\r\nclick 雪 \"https://example.test\"\r\n{}",
                ),
                if broken { "C-->\r\n" } else { "" },
            );
            let source = if markdown {
                format!("# Diagram\r\n\r\n```mermaid\r\n{diagram}```\r\n")
            } else {
                diagram
            };
            let kind = if markdown {
                DocumentKind::Markdown
            } else {
                DocumentKind::Diagram
            };
            let snapshot = SnapshotHarness::new()
                .analyze("file:///tmp/flow-style.mmd", 1, source.as_str(), kind)
                .unwrap();
            assert_eq!(
                snapshot.fences()[0].text_index().source(),
                if broken {
                    FenceTextIndexSource::ParserRecovered
                } else {
                    FenceTextIndexSource::ParserComplete
                },
            );
            let offsets = source
                .match_indices('雪')
                .map(|(offset, _)| offset)
                .collect::<Vec<_>>();
            let position = snapshot.source_map().utf16_position(offsets[0]).unwrap();
            let position = Position::new(position.line, position.character);
            let definition = goto_definition(&snapshot, position).unwrap();
            assert_eq!(
                snapshot.byte_offset_for_position(definition.range.start),
                Some(offsets[1])
            );
            let locations = references(&snapshot, position, true).unwrap();
            assert_eq!(locations.len(), 4);
            assert_eq!(
                locations
                    .iter()
                    .map(|location| snapshot
                        .byte_offset_for_position(location.range.start)
                        .unwrap())
                    .collect::<Vec<_>>(),
                offsets
            );
            let edits = rename(&snapshot, position, "Renamed").unwrap().unwrap();
            let mut changed = source.clone();
            for edit in edits.changes[snapshot.uri()].iter().rev() {
                let start = snapshot.byte_offset_for_position(edit.range.start).unwrap();
                let end = snapshot.byte_offset_for_position(edit.range.end).unwrap();
                changed.replace_range(start..end, &edit.new_text);
            }
            assert_eq!(changed, source.replace('雪', "Renamed"));
            let reparsed = SnapshotHarness::new()
                .analyze("file:///tmp/flow-style.mmd", 2, changed.as_str(), kind)
                .unwrap();
            assert_eq!(
                reparsed.fences()[0].text_index().source(),
                snapshot.fences()[0].text_index().source()
            );
        }
    }
}

#[cfg(feature = "diagram-class")]
#[test]
fn adjacent_aggregation_rename_preserves_markers_quotes_and_generic_suffixes() {
    for newline in ["\n", "\r\n"] {
        let source = concat!(
            "\u{feff}---\ntitle: Classes 😀\n---\nclassDiagram\n",
            "class `目标😀`~T~\nRoot --o`目标😀`~T~\nRoot ..o`目标😀`~T~\n",
        )
        .replace('\n', newline);
        let snapshot = SnapshotHarness::new()
            .analyze(
                "file:///tmp/class-aggregation.mmd",
                1,
                source.as_str(),
                DocumentKind::Diagram,
            )
            .unwrap();
        assert_eq!(
            snapshot.fences()[0].text_index().source(),
            FenceTextIndexSource::ParserComplete
        );
        let position = snapshot
            .source_map()
            .utf16_position(source.rfind("目标😀").unwrap())
            .unwrap();
        let position = Position::new(position.line, position.character);
        let definition = goto_definition(&snapshot, position).unwrap();
        assert_eq!(
            snapshot.byte_offset_for_position(definition.range.start),
            source.find("目标😀")
        );
        assert_eq!(references(&snapshot, position, true).unwrap().len(), 3);
        let edits = rename(&snapshot, position, "Renamed").unwrap().unwrap();
        let mut changed = source.clone();
        for edit in edits.changes[snapshot.uri()].iter().rev() {
            let start = snapshot.byte_offset_for_position(edit.range.start).unwrap();
            let end = snapshot.byte_offset_for_position(edit.range.end).unwrap();
            changed.replace_range(start..end, &edit.new_text);
        }
        assert_eq!(changed, source.replace("目标😀", "Renamed"));
        assert!(changed.contains("--o`Renamed`~T~"));
        assert!(changed.contains("..o`Renamed`~T~"));
        let reparsed = SnapshotHarness::new()
            .analyze(
                "file:///tmp/class-aggregation.mmd",
                2,
                changed.as_str(),
                DocumentKind::Diagram,
            )
            .unwrap();
        assert_eq!(
            reparsed.fences()[0].text_index().source(),
            FenceTextIndexSource::ParserComplete
        );
    }
}

#[cfg(feature = "diagram-gantt")]
#[test]
fn literal_gantt_end_fields_stay_out_of_dependency_rename() {
    let source = concat!(
        "\u{feff}---\r\ntitle: Tasks 😀\r\n---\r\n",
        "gantt\r\ndateFormat YYYY-MM-DD\r\n",
        "Base界:a,2026-01-01,1d\r\n",
        "Task😀:b,2026-01-02,\u{0085}until a\r\n",
        "Next:c,after b,until a\r\n",
    );
    let snapshot = SnapshotHarness::new()
        .analyze(
            "file:///tmp/gantt-literal-end.mmd",
            1,
            source,
            DocumentKind::Diagram,
        )
        .unwrap();
    assert_eq!(
        snapshot.fences()[0].text_index().source(),
        FenceTextIndexSource::ParserComplete
    );
    let position = snapshot
        .source_map()
        .utf16_position(source.rfind("a\r\n").unwrap())
        .unwrap();
    let position = Position::new(position.line, position.character);
    let definition = goto_definition(&snapshot, position).unwrap();
    assert_eq!(
        snapshot.byte_offset_for_position(definition.range.start),
        source.find("a,2026")
    );
    assert_eq!(references(&snapshot, position, true).unwrap().len(), 2);
    let edits = rename(&snapshot, position, "base").unwrap().unwrap();
    let mut changed = source.to_string();
    for edit in edits.changes[snapshot.uri()].iter().rev() {
        let start = snapshot.byte_offset_for_position(edit.range.start).unwrap();
        let end = snapshot.byte_offset_for_position(edit.range.end).unwrap();
        changed.replace_range(start..end, &edit.new_text);
    }
    assert!(changed.contains("Base界:base,2026"));
    assert!(changed.contains("\u{0085}until a\r\n"));
    assert!(changed.contains("after b,until base\r\n"));
    let reparsed = SnapshotHarness::new()
        .analyze(
            "file:///tmp/gantt-literal-end.mmd",
            2,
            changed.as_str(),
            DocumentKind::Diagram,
        )
        .unwrap();
    assert_eq!(
        reparsed.fences()[0].text_index().source(),
        FenceTextIndexSource::ParserComplete
    );
}

#[cfg(feature = "diagram-class")]
#[test]
fn class_signature_failure_keeps_later_navigation_and_member_outline_ranges() {
    for broken in [false, true] {
        let member = "\u{85}+run(\u{feff}arg\u{feff}) Result\u{85}";
        let source = format!(
            concat!(
                "# Classes\r\n\r\n```mermaid\r\n\u{feff}---\r\ntitle: Classes 😀\r\n---\r\n",
                "classDiagram\r\nclass C {{\r\n\u{feff}{member}\u{feff}\r\n{}\r\n}}\r\n",
                "class Later\r\nLater --> C\r\n```\r\n",
            ),
            if broken { "invalid)" } else { "+ok() Result" },
            member = member
        );
        let snapshot = SnapshotHarness::new()
            .analyze(
                "file:///tmp/class-method.md",
                1,
                source.as_str(),
                DocumentKind::Markdown,
            )
            .unwrap();
        let index = snapshot.fences()[0].text_index();
        assert_eq!(
            index.source(),
            if broken {
                FenceTextIndexSource::ParserRecovered
            } else {
                FenceTextIndexSource::ParserComplete
            }
        );
        let symbol = index
            .semantic_items()
            .iter()
            .find(|symbol| symbol.name == member)
            .expect("authored Class member outline");
        assert_eq!(
            &snapshot.fences()[0].text()[symbol.selection.start..symbol.selection.end],
            member
        );
        let position = snapshot
            .source_map()
            .utf16_position(source.rfind("Later").unwrap())
            .unwrap();
        let position = Position::new(position.line, position.character);
        let definition = goto_definition(&snapshot, position).unwrap();
        assert_eq!(
            snapshot.byte_offset_for_position(definition.range.start),
            source.find("Later")
        );
        let edits = rename(&snapshot, position, "Renamed").unwrap().unwrap();
        let mut changed = source.clone();
        for edit in edits.changes[snapshot.uri()].iter().rev() {
            let start = snapshot.byte_offset_for_position(edit.range.start).unwrap();
            let end = snapshot.byte_offset_for_position(edit.range.end).unwrap();
            changed.replace_range(start..end, &edit.new_text);
        }
        assert_eq!(changed, source.replace("Later", "Renamed"));
        let reparsed = SnapshotHarness::new()
            .analyze(
                "file:///tmp/class-method.md",
                2,
                changed.as_str(),
                DocumentKind::Markdown,
            )
            .unwrap();
        assert_eq!(reparsed.fences()[0].text_index().source(), index.source());
    }
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn comma_click_targets_keep_independent_navigation_and_rename_during_recovery() {
    for broken in [false, true] {
        let source = format!(
            concat!(
                "\u{feff}---\r\ntitle: Flow 😀\r\n---\r\nflowchart TD\r\n甲\r\n乙\r\n",
                "click 甲,乙 {}\r\n甲-->乙\r\n",
            ),
            if broken {
                "href"
            } else {
                "href \"https://example.test\""
            }
        );
        let snapshot = SnapshotHarness::new()
            .analyze(
                "file:///tmp/flow-click.mmd",
                1,
                source.as_str(),
                DocumentKind::Diagram,
            )
            .unwrap();
        assert_eq!(
            snapshot.fences()[0].text_index().source(),
            if broken {
                FenceTextIndexSource::ParserRecovered
            } else {
                FenceTextIndexSource::ParserComplete
            }
        );
        let offset = source.find("click 甲,乙").unwrap() + "click 甲,".len();
        let position = snapshot.source_map().utf16_position(offset).unwrap();
        let position = Position::new(position.line, position.character);
        let definition = goto_definition(&snapshot, position).unwrap();
        assert_eq!(
            snapshot.byte_offset_for_position(definition.range.start),
            source.find("乙")
        );
        assert_eq!(references(&snapshot, position, true).unwrap().len(), 3);
        let edits = rename(&snapshot, position, "Renamed").unwrap().unwrap();
        let mut changed = source.clone();
        for edit in edits.changes[snapshot.uri()].iter().rev() {
            let start = snapshot.byte_offset_for_position(edit.range.start).unwrap();
            let end = snapshot.byte_offset_for_position(edit.range.end).unwrap();
            changed.replace_range(start..end, &edit.new_text);
        }
        assert_eq!(changed, source.replace('乙', "Renamed"));
        assert!(changed.contains("click 甲,Renamed"));
    }
}

#[cfg(feature = "diagram-flowchart")]
#[test]
fn inline_class_keeps_a_reference_to_a_node_sharing_an_existing_edge_id() {
    for styled in ["e:::hot", "e[Ignored]:::hot"] {
        let source = format!("flowchart TD\ne[Before]\nA e@-->B\n{styled}\n");
        let snapshot = SnapshotHarness::new()
            .analyze(
                "file:///tmp/flow-shared.mmd",
                1,
                source.as_str(),
                DocumentKind::Diagram,
            )
            .unwrap();
        let offset = source.rfind(styled).unwrap();
        let position = snapshot.source_map().utf16_position(offset).unwrap();
        let position = Position::new(position.line, position.character);
        let definition = goto_definition(&snapshot, position).unwrap();
        assert_eq!(
            snapshot.byte_offset_for_position(definition.range.start),
            source.find("e[Before]")
        );
        assert_eq!(references(&snapshot, position, true).unwrap().len(), 2);
        let edits = rename(&snapshot, position, "Renamed").unwrap().unwrap();
        let mut changed = source.clone();
        for edit in edits.changes[snapshot.uri()].iter().rev() {
            let start = snapshot.byte_offset_for_position(edit.range.start).unwrap();
            let end = snapshot.byte_offset_for_position(edit.range.end).unwrap();
            changed.replace_range(start..end, &edit.new_text);
        }
        assert_eq!(
            changed,
            source
                .replacen("e[Before]", "Renamed[Before]", 1)
                .replace(styled, &styled.replacen('e', "Renamed", 1))
        );
        assert!(changed.contains("A e@-->B"));
    }
}
