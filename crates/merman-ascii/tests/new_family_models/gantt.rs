use super::*;

#[test]
fn gantt_render_model_renders_sections_tasks_and_flags() {
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
        id: "task-1".to_string(),
        task: "Implement".to_string(),
        section: "Build".to_string(),
        task_type: "Build".to_string(),
        classes: Vec::new(),
        active: true,
        done: true,
        crit: true,
        milestone: true,
        vert: true,
        order: 0,
        start_ms: 9_223_372_036_854_775_000,
        end_ms: 9_223_372_036_854_775_001,
        render_end_ms: Some(9_223_372_036_854_775_002),
        ..Default::default()
    }];

    let rendered = render(RenderSemanticModel::Gantt(model));

    assert_eq!(
        rendered,
        concat!(
            "title(bytes=5)=\"Gantt\"\n",
            "accTitle(bytes=11)=\"Gantt title\"\n",
            "accDescr(bytes=17)=\"Gantt description\"\n",
            "dateFormat(bytes=10)=\"YYYY-MM-DD\"\n",
            "axisFormat(bytes=2)=\"%d\"\n",
            "section(bytes=5)=\"Empty\"\n",
            "section(bytes=5)=\"Empty\"\n",
            "section(bytes=5)=\"Build\"\n",
            "  - task(bytes=9)=\"Implement\" [id(bytes=6)=\"task-1\", order=0,\n",
            "    range=+292278994-08-17T07:12:55.000 -> +292278994-08-17T07:12:55.001,\n",
            "    renderEnd=+292278994-08-17T07:12:55.002, flags=milestone, active, done,\n",
            "    crit, vert]",
        )
    );
}
#[test]
fn gantt_direct_model_discloses_task_order() {
    let mut model = GanttDiagramRenderModel::default();
    model.tasks = vec![
        GanttRenderTask {
            id: "first".to_string(),
            task: "First".to_string(),
            order: 7,
            ..Default::default()
        },
        GanttRenderTask {
            id: "second".to_string(),
            task: "Second".to_string(),
            order: 3,
            ..Default::default()
        },
    ];

    let rendered = render(RenderSemanticModel::Gantt(model.clone()));
    assert!(rendered.contains("task(bytes=5)=\"First\" [id(bytes=5)=\"first\", order=7,"));
    assert!(rendered.contains("task(bytes=6)=\"Second\" [id(bytes=6)=\"second\", order=3,"));

    model.tasks[0].order = 3;
    model.tasks[1].order = 7;
    let reordered = render(RenderSemanticModel::Gantt(model));
    assert_ne!(rendered, reordered, "task order must remain recoverable");
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

    assert!(
        rendered.contains("after=[bytes=6 \"design\", bytes=6 \"review\"]"),
        "structured Gantt output should disclose the dependency source expression:\n{rendered}"
    );
    assert!(rendered.contains("until=[bytes=6 \"design\", bytes=6 \"review\"]"));
    assert!(rendered.contains("start(bytes=10)=\"2026-01-01\""));
    assert!(rendered.contains("duration(bytes=2)=\"1d\""));
    assert!(rendered.contains("end(bytes=10)=\"2026-01-06\""));
    assert!(rendered.contains("id(bytes=6)=\"design\""));
    assert!(rendered.contains("id(bytes=6)=\"review\""));
    assert!(rendered.contains("id(bytes=14)=\"implementation\""));
    assert!(rendered.contains("id(bytes=7)=\"release\""));
}

#[test]
fn gantt_direct_model_renders_only_typed_constraints() {
    let mut model = GanttDiagramRenderModel::default();
    model.sections = vec!["Build".to_string()];
    model.tasks = vec![GanttRenderTask {
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
        raw: GanttRenderTaskRaw {
            data: "implementation,after raw-start,until raw-end".to_string(),
            start_time: GanttRenderTaskStart::GetStartDate {
                start_data: "after raw-start".to_string(),
            },
            end_time: GanttRenderTaskEnd {
                data: "until raw-end".to_string(),
            },
        },
        ..Default::default()
    }];

    let rendered = render(RenderSemanticModel::Gantt(model));
    assert!(rendered.contains("after=[bytes=6 \"design\", bytes=6 \"review\"]"));
    assert!(rendered.contains("until=[bytes=7 \"release\"]"));
    for legacy in ["raw-start", "raw-end", "legacy-previous"] {
        assert!(
            !rendered.contains(legacy),
            "ASCII must not recover constraints from legacy raw fields:\n{rendered}"
        );
    }
}

#[test]
fn gantt_direct_model_distinguishes_fixed_and_relative_end_constraints() {
    let mut model = GanttDiagramRenderModel::default();
    model.tasks = vec![
        GanttRenderTask {
            id: "fixed".to_string(),
            task: "Fixed".to_string(),
            start_constraint: GanttTaskStartConstraint::Fixed {
                value: "2026-01-01 08:30".to_string(),
            },
            end_constraint: GanttTaskEndConstraint::Fixed {
                value: "2026-01-01 10:45".to_string(),
            },
            ..Default::default()
        },
        GanttRenderTask {
            id: "relative".to_string(),
            task: "Relative".to_string(),
            start_constraint: GanttTaskStartConstraint::PreviousTaskEnd {
                dependency_id: Some("fixed".to_string()),
            },
            end_constraint: GanttTaskEndConstraint::Duration {
                value: "2.5h".to_string(),
            },
            ..Default::default()
        },
    ];

    let rendered = render(RenderSemanticModel::Gantt(model));
    assert!(rendered.contains("start(bytes=16)=\"2026-01-01 08:30\""));
    assert!(rendered.contains("end(bytes=16)=\"2026-01-01 10:45\""));
    assert!(rendered.contains("after(bytes=5)=\"fixed\""));
    assert!(rendered.contains("duration(bytes=4)=\"2.5h\""));
}

#[test]
fn gantt_structured_text_preserves_time_precision_and_render_end() {
    let mut model = GanttDiagramRenderModel::default();
    model.sections = vec!["Build".to_string()];
    model.tasks = vec![GanttRenderTask {
        id: "timed".to_string(),
        task: "Timed".to_string(),
        section: "Build".to_string(),
        start_ms: 1,
        end_ms: 2,
        render_end_ms: Some(3),
        ..Default::default()
    }];

    let rendered = render(RenderSemanticModel::Gantt(model));

    assert!(rendered.contains("id(bytes=5)=\"timed\""));
    assert!(
        rendered.contains("T"),
        "time-of-day precision must be visible:\n{rendered}"
    );
    assert!(rendered.contains("renderEnd="));
}

#[test]
fn gantt_direct_model_rejects_duplicate_task_ids() {
    let task = GanttRenderTask {
        id: "same".to_string(),
        task: "Task".to_string(),
        section: "Build".to_string(),
        ..Default::default()
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
