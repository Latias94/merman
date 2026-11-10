#![cfg(feature = "diagram-gantt")]

use merman_core::SemanticParser;
use merman_model::{
    families::gantt::{GanttDiagramModel, GanttTaskEndConstraint, GanttTaskStartConstraint},
    semantic::SemanticDiagramModel,
};

fn parse_gantt(source: &str) -> GanttDiagramModel {
    let parsed = SemanticParser::new()
        .parse_diagram_sync(source)
        .expect("valid Gantt source");
    let SemanticDiagramModel::Gantt(model) = parsed.model else {
        panic!("expected the canonical Gantt model");
    };
    model
}

#[test]
fn nel_prefixed_end_fields_remain_literal_and_do_not_move_dependent_tasks() {
    for newline in ["\n", "\r\n"] {
        for padding in ["", " \u{feff}", "\u{00a0}"] {
            for end_data in [
                "\u{0085}until a",
                "\u{0085}1d",
                "\u{0085}2026-01-04",
                "\u{0085}",
            ] {
                let source = format!(
                    concat!(
                        "gantt\ndateFormat YYYY-MM-DD\n",
                        "A:a,2026-01-01,1d\n",
                        "B:b,2026-01-02,{padding}{end_data}{padding}\n",
                        "C:c,after b,1d\n",
                    ),
                    padding = padding,
                    end_data = end_data,
                )
                .replace('\n', newline);
                let model = parse_gantt(&source);
                let [base, literal, dependent] = model.tasks.as_slice() else {
                    panic!("the three authored tasks must be retained");
                };
                assert_ne!(literal.start_ms, base.start_ms, "{source:?}");
                assert_eq!(literal.end_ms, literal.start_ms, "{source:?}");
                assert_eq!(dependent.start_ms, literal.end_ms, "{source:?}");
                assert_eq!(literal.raw.end_time.data, end_data, "{source:?}");
                assert_eq!(
                    literal.raw.data,
                    format!(":b,2026-01-02,{padding}{end_data}{padding}"),
                );
                assert_eq!(
                    literal.end_constraint,
                    GanttTaskEndConstraint::Duration {
                        value: end_data.to_string(),
                    },
                    "{source:?}",
                );
                assert!(!literal.manual_end_time, "{source:?}");
                assert!(model.tasks.iter().all(|task| task.processed));
            }
        }
    }
}

#[test]
fn ecmascript_task_padding_preserves_dependencies_tags_and_authored_order() {
    for padding in ["\u{feff}", "\u{00a0}", "\u{2028}", "\u{2029}"] {
        let source = format!(
            concat!(
                "gantt\ndateFormat YYYY-MM-DD\n",
                "A:a,2026-01-01,1d\n",
                "B:{padding}active{padding},done,crit,b,{padding}after a{padding},",
                "{padding}until v{padding}\n",
                "Vertical:vert,milestone,v,2026-01-03,0d\n",
                "Next:1d\nC:c,after b,until v\n",
            ),
            padding = padding,
        );
        let model = parse_gantt(&source);
        let tasks = &model.tasks;
        assert_eq!(
            tasks
                .iter()
                .map(|task| task.id.as_str())
                .collect::<Vec<_>>(),
            ["a", "b", "v", "task1", "c"],
            "{source:?}",
        );
        assert_eq!(
            tasks.iter().map(|task| task.order).collect::<Vec<_>>(),
            [0, 1, -1, 2, 3],
            "{source:?}",
        );
        assert!(tasks[1].active && tasks[1].done && tasks[1].crit);
        assert!(tasks[2].vert && tasks[2].milestone);
        assert_eq!(tasks[1].start_ms, tasks[0].end_ms);
        assert_eq!(tasks[1].end_ms, tasks[2].start_ms);
        assert_eq!(tasks[3].start_ms, tasks[2].end_ms);
        assert_eq!(tasks[4].start_ms, tasks[1].end_ms);
        assert_eq!(tasks[4].end_ms, tasks[2].start_ms);
        assert_eq!(tasks[4].prev_task_id.as_deref(), Some("task1"));
        assert_eq!(
            tasks[1].start_constraint,
            GanttTaskStartConstraint::After {
                dependency_ids: vec!["a".to_string()],
            },
        );
        assert_eq!(
            tasks[1].end_constraint,
            GanttTaskEndConstraint::Until {
                dependency_ids: vec!["v".to_string()],
            },
        );
    }
}

#[test]
fn nel_prefixed_after_is_not_a_start_dependency() {
    for padding in ["", " \u{feff}"] {
        let source = format!(
            "gantt\ndateFormat YYYY-MM-DD\nA:a,2026-01-01,1d\nB:b,{padding}\u{0085}after a{padding},1d\n"
        );
        let error = SemanticParser::new()
            .parse_diagram_sync(&source)
            .expect_err("NEL cannot turn an invalid start field into an after dependency");
        assert!(error.to_string().contains("Invalid date:"), "{error}");
    }
}

#[cfg(feature = "editor-facts")]
mod editor {
    use super::*;
    use merman_core::{
        EditorSemanticCompleteness, EditorSemanticRole, ParsedEditorFacts, SemanticParseOutcome,
        SemanticSnapshotCapture,
    };
    use merman_model::source::SourceSpan;

    #[test]
    fn literal_end_fields_and_real_dependency_facts_agree_at_original_byte_ranges() {
        let source = concat!(
            "\u{feff}---\r\ntitle: 計画 🦀\r\n---\r\n",
            "gantt\r\ndateFormat YYYY-MM-DD\r\n",
            "Base界:a,2026-01-01,1d\r\n",
            "Task😀:b,2026-01-02,\u{0085}until a\r\n",
            "Next:c,after b,until a\r\n",
        );
        let SemanticSnapshotCapture::Snapshot(snapshot) =
            SemanticParser::new().capture_diagram_snapshot_sync(source)
        else {
            panic!("valid Gantt source must retain a semantic snapshot");
        };
        let SemanticParseOutcome::Parsed(SemanticDiagramModel::Gantt(model)) = snapshot.outcome()
        else {
            panic!("valid Gantt source must retain the canonical model");
        };
        assert_eq!(model.tasks[1].end_ms, model.tasks[1].start_ms);
        assert_eq!(model.tasks[2].start_ms, model.tasks[1].end_ms);
        let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
            panic!("Gantt capture must retain editor facts");
        };
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Complete);
        assert!(facts.diagnostics.is_empty());
        for (name, marker, role) in [
            ("a", "a,2026-01-01", EditorSemanticRole::Entity),
            ("b", "b,2026-01-02", EditorSemanticRole::Entity),
            ("c", "c,after b", EditorSemanticRole::Entity),
            ("b", "b,until a", EditorSemanticRole::Reference),
            ("a", "a\r\n", EditorSemanticRole::Reference),
        ] {
            let start = source.rfind(marker).unwrap();
            let selection = SourceSpan::new(start, start + name.len());
            assert!(facts.symbols.iter().any(|symbol| {
                symbol.name == name && symbol.role == role && symbol.selection == selection
            }));
            assert_eq!(&source[selection.start..selection.end], name);
        }
        let literal_reference = source.find("\u{0085}until a").unwrap() + "\u{0085}until ".len();
        assert!(!facts.symbols.iter().any(|symbol| {
            symbol.role == EditorSemanticRole::Reference
                && symbol.selection.start == literal_reference
        }));
    }

    #[test]
    fn rejected_nel_start_keeps_later_independent_facts_at_original_ranges() {
        let source = concat!(
            "\u{feff}---\r\ntitle: 回復 🦀\r\n---\r\n",
            "gantt\r\ndateFormat YYYY-MM-DD\r\n",
            "Base界:a,2026-01-01,1d\r\n",
            "Broken😀:b,\u{0085}after a,1d\r\n",
            "Later界:c,2026-01-03,1d\r\n",
            "Final:d,after c,1d\r\n",
        );
        let SemanticSnapshotCapture::Snapshot(snapshot) =
            SemanticParser::new().capture_diagram_snapshot_sync(source)
        else {
            panic!("invalid Gantt task dates must retain a semantic snapshot");
        };
        assert!(matches!(
            snapshot.outcome(),
            SemanticParseOutcome::Failed(_)
        ));
        let ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
            panic!("Gantt failure must retain editor recovery facts");
        };
        assert_eq!(facts.completeness, EditorSemanticCompleteness::Recovered);
        assert!(!facts.diagnostics.is_empty());
        for (name, marker, role) in [
            ("c", "c,2026-01-03", EditorSemanticRole::Entity),
            ("d", "d,after c", EditorSemanticRole::Entity),
            ("c", "c,1d", EditorSemanticRole::Reference),
        ] {
            let start = source.rfind(marker).unwrap();
            let selection = SourceSpan::new(start, start + name.len());
            assert!(facts.symbols.iter().any(|symbol| {
                symbol.name == name && symbol.role == role && symbol.selection == selection
            }));
            assert_eq!(&source[selection.start..selection.end], name);
        }
        let literal_reference = source.find("\u{0085}after a").unwrap() + "\u{0085}after ".len();
        assert!(!facts.symbols.iter().any(|symbol| {
            symbol.role == EditorSemanticRole::Reference
                && symbol.selection.start == literal_reference
        }));
    }
}
