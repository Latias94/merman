#![cfg(feature = "diagram-class")]

mod support;

use merman_analysis::FenceTextIndexSource;
use merman_editor_core::{DocumentKind, Position, completion_for_snapshot};
use support::SnapshotHarness;

#[test]
fn inline_annotations_do_not_offer_standalone_annotation_targets() {
    for newline in ["\n", "\r\n", "\r"] {
        for (declaration, body) in [
            ("class Shape <<interface>>", ""),
            ("class Shape <<interface>>", "\n"),
            ("class Shape <<interface>> ", "\n"),
            ("class Shape <<interface>> {}", "\n"),
            ("class Shape <<interface>> {", "\n+draw()\n}\n"),
        ] {
            let body = body.replace('\n', newline);
            let source = format!("classDiagram{newline}class Known{newline}{declaration}{body}");
            let snapshot = SnapshotHarness::new()
                .analyze(
                    "file:///tmp/class-inline-annotation.mmd",
                    1,
                    source,
                    DocumentKind::Diagram,
                )
                .unwrap();
            let completion = completion_for_snapshot(
                &snapshot,
                Position::new(2, declaration.encode_utf16().count()),
            );
            assert_eq!(
                completion.fact_source,
                Some(FenceTextIndexSource::ParserComplete)
            );
            assert!(
                completion
                    .items
                    .iter()
                    .all(|item| item.label != "Known" && item.label != "Shape"),
                "completed class header must not offer another class target: {declaration:?}"
            );
        }
    }
}

#[test]
fn standalone_annotations_still_offer_class_targets_at_eof() {
    let final_line = "<<interface>>";
    let source = format!("classDiagram\nclass Known\n{final_line}");
    let snapshot = SnapshotHarness::new()
        .analyze(
            "file:///tmp/class-standalone-annotation.mmd",
            1,
            source,
            DocumentKind::Diagram,
        )
        .unwrap();
    let cursor = Position::new(2, final_line.len());
    let completion = completion_for_snapshot(&snapshot, cursor);
    assert_eq!(
        completion.fact_source,
        Some(FenceTextIndexSource::ParserRecovered)
    );
    let item = completion
        .items
        .iter()
        .find(|item| item.label == "Known")
        .unwrap();
    let edit = item.text_edit.as_ref().unwrap();
    assert_eq!(edit.range.start, cursor);
    assert_eq!(edit.range.end, cursor);
    assert_eq!(edit.new_text, "Known");
}

#[test]
fn spaced_generic_declarations_offer_the_canonical_class_id() {
    let source = "classDiagram\nclass Box \t~T~\nclass Item\nItem -->";
    let snapshot = SnapshotHarness::new()
        .analyze(
            "file:///tmp/class-spaced-generic.mmd",
            1,
            source,
            DocumentKind::Diagram,
        )
        .unwrap();
    let cursor = Position::new(3, "Item -->".len());
    let completion = completion_for_snapshot(&snapshot, cursor);
    assert_eq!(
        completion.fact_source,
        Some(FenceTextIndexSource::ParserRecovered)
    );
    let item = completion
        .items
        .iter()
        .find(|item| item.label == "Box")
        .unwrap();
    let edit = item.text_edit.as_ref().unwrap();
    assert_eq!(edit.range.start, cursor);
    assert_eq!(edit.range.end, cursor);
    assert_eq!(edit.new_text, "Box");
    assert!(
        completion
            .items
            .iter()
            .all(|item| !item.label.contains('~'))
    );
}
