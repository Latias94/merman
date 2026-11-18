use super::*;
use merman_model::families::gantt::GanttTask;

fn unwrapped_records(rendered: &str) -> Vec<String> {
    let mut records: Vec<String> = Vec::new();
    for line in rendered.lines() {
        if let Some(continuation) = line.strip_prefix("  ")
            && let Some(record) = records.last_mut()
        {
            record.push_str(continuation);
        } else {
            records.push(line.to_string());
        }
    }
    records
}

#[test]
fn gantt_render_model_retains_sections_task_fields_and_flags() {
    let mut model = GanttDiagramRenderModel::default();
    model.title = Some("Gantt".to_string());
    model.acc_title = Some("Gantt title".to_string());
    model.acc_descr = Some("Gantt description".to_string());
    model.date_format = "YYYY-MM-DD".to_string();
    model.axis_format = "%d".to_string();
    model.sections = vec![
        "Empty".to_string(),
        "Empty".to_string(),
        "Build".to_string(),
    ];
    model.tasks = vec![GanttRenderTask {
        semantic: GanttTask {
            id: "task-1".to_string(),
            task: "Implement".to_string(),
            section: "Build".to_string(),
            task_type: "Build".to_string(),
            classes: vec!["delivery".to_string()],
            active: true,
            done: true,
            crit: true,
            milestone: true,
            vert: true,
            manual_end_time: true,
            order: 0,
            start_ms: 9_223_372_036_854_775_000,
            end_ms: 9_223_372_036_854_775_001,
            ..GanttTask::default()
        },
        render_end_ms: Some(9_223_372_036_854_775_002),
    }];

    let rendered = render(RenderSemanticModel::Gantt(model));
    let records = unwrapped_records(&rendered);
    let identity = records
        .iter()
        .find(|record| record.starts_with("T1 [task]"))
        .expect("source-indexed task record");
    assert_eq!(
        identity,
        "T1 [task] id=\"task-1\" label=\"Implement\" section=\"Build\" sectionIndex=null order=0"
    );
    let details = records.join("\n");
    for value in [
        "title:\"Gantt\"",
        "accTitle:\"Gantt title\"",
        "accDescr:\"Gantt description\"",
        "dateFormat:\"YYYY-MM-DD\"",
        "axisFormat:\"%d\"",
        "sections:[\"Empty\",\"Empty\",\"Build\"]",
        "type:\"Build\"",
        "classes:[\"delivery\"]",
        "active:true",
        "done:true",
        "crit:true",
        "milestone:true",
        "vert:true",
        "manualEndTime:true",
        "processed:false",
        "startTime:9223372036854775000",
        "endTime:9223372036854775001",
        "renderEndTime:9223372036854775002",
        "+292278994-08-17T07:12:55.000 -> +292278994-08-17T07:12:55.001",
        "renderEnd=+292278994-08-17T07:12:55.002",
        "interval (UTC):",
    ] {
        assert!(details.contains(value), "missing {value}:\n{rendered}");
    }
}

#[test]
fn gantt_repeated_section_names_preserve_authored_task_occurrences() {
    let rendered = render_parsed(concat!(
        "gantt\n",
        "dateFormat YYYY-MM-DD\n",
        "section A\n",
        "First: first, 2026-01-01, 1d\n",
        "section B\n",
        "Middle: middle, 2026-01-02, 1d\n",
        "section A\n",
        "Last: last, 2026-01-03, 1d\n",
    ));

    let records = unwrapped_records(&rendered);
    let structure = records
        .iter()
        .filter(|record| record.contains(" [task] "))
        .collect::<Vec<_>>();
    assert_eq!(
        structure.len(),
        3,
        "unexpected Gantt structure:\n{rendered}"
    );
    for (record, key, label, section, occurrence) in [
        (structure[0], "T1", "First", "A", 0),
        (structure[1], "T2", "Middle", "B", 1),
        (structure[2], "T3", "Last", "A", 2),
    ] {
        assert!(record.starts_with(&format!("{key} [task] ")));
        assert!(record.contains(&format!("label=\"{label}\"")));
        assert!(record.contains(&format!("section=\"{section}\" sectionIndex={occurrence}")));
    }
    assert!(records.join("\n").contains("sections:[\"A\",\"B\",\"A\"]"));
}

#[test]
fn gantt_direct_model_discloses_task_order() {
    let mut model = GanttDiagramRenderModel::default();
    model.tasks = vec![
        GanttRenderTask {
            semantic: GanttTask {
                id: "first".to_string(),
                task: "First".to_string(),
                order: 7,
                ..GanttTask::default()
            },
            ..GanttRenderTask::default()
        },
        GanttRenderTask {
            semantic: GanttTask {
                id: "second".to_string(),
                task: "Second".to_string(),
                order: 3,
                ..GanttTask::default()
            },
            ..GanttRenderTask::default()
        },
    ];

    let rendered = render(RenderSemanticModel::Gantt(model.clone()));
    let identities = unwrapped_records(&rendered)
        .into_iter()
        .filter(|record| record.contains(" [task] "))
        .collect::<Vec<_>>();
    assert_eq!(identities.len(), 2);
    assert!(identities[0].starts_with("T1 [task] id=\"first\" label=\"First\""));
    assert!(identities[0].ends_with("order=7"));
    assert!(identities[1].starts_with("T2 [task] id=\"second\" label=\"Second\""));
    assert!(identities[1].ends_with("order=3"));

    model.tasks[0].order = 3;
    model.tasks[1].order = 7;
    let reordered = render(RenderSemanticModel::Gantt(model));
    let reordered_identities = unwrapped_records(&reordered)
        .into_iter()
        .filter(|record| record.contains(" [task] "))
        .collect::<Vec<_>>();
    assert!(reordered_identities[0].starts_with("T1 [task] id=\"first\""));
    assert!(reordered_identities[0].ends_with("order=3"));
    assert!(reordered_identities[1].starts_with("T2 [task] id=\"second\""));
    assert!(reordered_identities[1].ends_with("order=7"));
    assert_ne!(rendered, reordered, "task order must remain recoverable");
}

#[test]
fn gantt_task_keys_do_not_alias_literal_ids_or_normalize_dependencies() {
    let mut model = GanttDiagramRenderModel::default();
    model.tasks = ["T2", "a-b", "a_b"]
        .into_iter()
        .map(|id| GanttRenderTask {
            semantic: GanttTask {
                id: id.to_string(),
                task: "Same label".to_string(),
                start_constraint: GanttTaskStartConstraint::After {
                    dependency_ids: vec![
                        "T2".to_string(),
                        "a-b".to_string(),
                        "a_b".to_string(),
                        "a b".to_string(),
                    ],
                },
                ..GanttTask::default()
            },
            ..GanttRenderTask::default()
        })
        .collect();

    let rendered = render(RenderSemanticModel::Gantt(model));
    let records = unwrapped_records(&rendered);
    let identities = records
        .iter()
        .filter(|record| record.contains(" [task] "))
        .collect::<Vec<_>>();
    assert_eq!(identities.len(), 3);
    for (record, expected) in identities.iter().zip([
        "T1 [task] id=\"T2\"",
        "T2 [task] id=\"a-b\"",
        "T3 [task] id=\"a_b\"",
    ]) {
        assert!(record.starts_with(expected));
    }
    let task_details = records
        .iter()
        .filter(|record| record.starts_with('T') && record.contains(" details:"))
        .collect::<Vec<_>>();
    assert_eq!(task_details.len(), 3);
    for record in task_details {
        assert!(record.contains(
            "startConstraint:{\"kind\":\"after\",\"dependencyIds\":[\"T2\",\"a-b\",\"a_b\",\"a b\"]}",
        ));
    }
}

#[test]
fn gantt_structured_text_framing_distinguishes_title_from_axis_format() {
    let mut embedded_axis = GanttDiagramRenderModel::default();
    embedded_axis.title = Some("Gantt\naxisFormat: %d".to_string());

    let mut explicit_axis = GanttDiagramRenderModel::default();
    explicit_axis.title = Some("Gantt".to_string());
    explicit_axis.axis_format = "%d".to_string();

    assert_ne!(
        render(RenderSemanticModel::Gantt(embedded_axis)),
        render(RenderSemanticModel::Gantt(explicit_axis)),
        "authored title text must not be able to forge an axisFormat field",
    );
}

#[test]
fn gantt_structured_text_discloses_dependency_source_expressions() {
    let rendered = render_parsed(concat!(
        "gantt\n",
        "dateFormat YYYY-MM-DD\n",
        "section Build\n",
        "Design: design, 2026-01-01, 1d\n",
        "Review: review, 2026-01-02, 1d\n",
        "Implement: implementation, after design review, until design review\n",
        "Release: release, 2026-01-05, 2026-01-06\n",
    ));

    let records = unwrapped_records(&rendered).join("\n");
    for value in [
        "startConstraint:{\"kind\":\"after\",\"dependencyIds\":[\"design\",\"review\"]}",
        "endConstraint:{\"kind\":\"until\",\"dependencyIds\":[\"design\",\"review\"]}",
        "startConstraint:{\"kind\":\"fixed\",\"value\":\"2026-01-01\"}",
        "endConstraint:{\"kind\":\"duration\",\"value\":\"1d\"}",
        "endConstraint:{\"kind\":\"fixed\",\"value\":\"2026-01-06\"}",
        "T1 [task] id=\"design\"",
        "T2 [task] id=\"review\"",
        "T3 [task] id=\"implementation\"",
        "T4 [task] id=\"release\"",
    ] {
        assert!(records.contains(value), "missing {value}:\n{rendered}");
    }
}

#[test]
fn gantt_direct_model_preserves_typed_constraints_and_separate_legacy_raw_data() {
    let mut model = GanttDiagramRenderModel::default();
    model.sections = vec!["Build".to_string()];
    model.tasks = vec![GanttRenderTask {
        semantic: GanttTask {
            id: "implementation".to_string(),
            task: "Implement".to_string(),
            section: "Build".to_string(),
            start_constraint: GanttTaskStartConstraint::After {
                dependency_ids: vec!["design".to_string(), "review".to_string()],
            },
            end_constraint: GanttTaskEndConstraint::Until {
                dependency_ids: vec!["release".to_string()],
            },
            prev_task_id: Some("legacy-previous".to_string()),
            raw: GanttTaskRaw {
                data: "implementation,after raw-start,until raw-end".to_string(),
                start_time: GanttTaskStart::GetStartDate {
                    start_data: "after raw-start".to_string(),
                },
                end_time: GanttTaskEnd {
                    data: "until raw-end".to_string(),
                },
            },
            ..GanttTask::default()
        },
        ..GanttRenderTask::default()
    }];

    let rendered = render(RenderSemanticModel::Gantt(model));
    let records = unwrapped_records(&rendered);
    let details = records
        .iter()
        .find(|record| record.starts_with("T1 details:"))
        .expect("complete task details");
    for value in [
        "startConstraint:{\"kind\":\"after\",\"dependencyIds\":[\"design\",\"review\"]}",
        "endConstraint:{\"kind\":\"until\",\"dependencyIds\":[\"release\"]}",
        "prevTaskId:\"legacy-previous\"",
        "raw:{\"data\":\"implementation,after raw-start,until raw-end\"",
        "\"startTime\":{\"type\":\"getStartDate\",\"startData\":\"after raw-start\"}",
        "\"endTime\":{\"data\":\"until raw-end\"}",
    ] {
        assert!(details.contains(value), "missing {value}:\n{rendered}");
    }
    for legacy in ["raw-start", "raw-end", "legacy-previous"] {
        assert!(
            records
                .iter()
                .filter(|record| !record.starts_with("T1 details:"))
                .all(|record| !record.contains(legacy)),
            "legacy data must not generate task identities, intervals, or dependencies:\n{rendered}"
        );
    }
}

#[test]
fn gantt_direct_model_distinguishes_fixed_and_relative_end_constraints() {
    let mut model = GanttDiagramRenderModel::default();
    model.tasks = vec![
        GanttRenderTask {
            semantic: GanttTask {
                id: "fixed".to_string(),
                task: "Fixed".to_string(),
                start_constraint: GanttTaskStartConstraint::Fixed {
                    value: "2026-01-01 08:30".to_string(),
                },
                end_constraint: GanttTaskEndConstraint::Fixed {
                    value: "2026-01-01 10:45".to_string(),
                },
                ..GanttTask::default()
            },
            ..GanttRenderTask::default()
        },
        GanttRenderTask {
            semantic: GanttTask {
                id: "relative".to_string(),
                task: "Relative".to_string(),
                start_constraint: GanttTaskStartConstraint::PreviousTaskEnd {
                    dependency_id: Some("fixed".to_string()),
                },
                end_constraint: GanttTaskEndConstraint::Duration {
                    value: "2.5h".to_string(),
                },
                ..GanttTask::default()
            },
            ..GanttRenderTask::default()
        },
    ];

    let rendered = render(RenderSemanticModel::Gantt(model));
    let records = unwrapped_records(&rendered).join("\n");
    for value in [
        "startConstraint:{\"kind\":\"fixed\",\"value\":\"2026-01-01 08:30\"}",
        "endConstraint:{\"kind\":\"fixed\",\"value\":\"2026-01-01 10:45\"}",
        "startConstraint:{\"kind\":\"previousTaskEnd\",\"dependencyId\":\"fixed\"}",
        "endConstraint:{\"kind\":\"duration\",\"value\":\"2.5h\"}",
    ] {
        assert!(records.contains(value), "missing {value}:\n{rendered}");
    }
}

#[test]
fn gantt_structured_text_preserves_time_precision_and_render_end() {
    let mut model = GanttDiagramRenderModel::default();
    model.sections = vec!["Build".to_string()];
    model.tasks = vec![GanttRenderTask {
        semantic: GanttTask {
            id: "timed".to_string(),
            task: "Timed".to_string(),
            section: "Build".to_string(),
            start_ms: 1,
            end_ms: 2,
            ..GanttTask::default()
        },
        render_end_ms: Some(3),
    }];

    let rendered = render(RenderSemanticModel::Gantt(model));
    let records = unwrapped_records(&rendered).join("\n");
    for value in [
        "T1 [task] id=\"timed\"",
        "T1 interval (UTC): 1970-01-01T00:00:00.001 -> 1970-01-01T00:00:00.002",
        "renderEnd=1970-01-01T00:00:00.003",
        "startTime:1",
        "endTime:2",
        "renderEndTime:3",
    ] {
        assert!(records.contains(value), "missing {value}:\n{rendered}");
    }
}

#[test]
fn gantt_direct_model_rejects_duplicate_task_ids() {
    let task = GanttRenderTask {
        semantic: GanttTask {
            id: "same".to_string(),
            task: "Task".to_string(),
            section: "Build".to_string(),
            ..GanttTask::default()
        },
        ..GanttRenderTask::default()
    };
    let mut model = GanttDiagramRenderModel::default();
    model.tasks = vec![task.clone(), task];

    let error = render_model(
        &RenderSemanticModel::Gantt(model),
        &AsciiRenderOptions::ascii(),
    )
    .expect_err("duplicate task ids cannot provide stable identity");
    assert!(matches!(
        error,
        AsciiError::UnsupportedFeature {
            diagram_type: "gantt",
            feature: "duplicate task ids",
        }
    ));
}
