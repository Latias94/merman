use super::*;

#[test]
fn timeline_render_model_renders_sections_tasks_and_events() {
    let mut model = TimelineDiagramRenderModel::default();
    model.title = Some("Timeline".to_string());
    model.acc_title = Some("Timeline title".to_string());
    model.acc_descr = Some("Timeline description".to_string());
    model.direction = TimelineDirection::TopDown;
    model.sections = vec!["Planning".to_string()];
    model.tasks = vec![
        TimelineRenderTask {
            id: 0,
            section: "Planning".to_string(),
            section_index: Some(0),
            task_type: "Planning".to_string(),
            task: "Design".to_string(),
            score: 0,
            events: vec!["Kickoff".to_string()],
        },
        TimelineRenderTask {
            id: 1,
            section: "Planning".to_string(),
            section_index: Some(0),
            task_type: "Planning".to_string(),
            task: "Implement".to_string(),
            score: 3,
            events: vec!["Build spec".to_string(), "Review".to_string()],
        },
    ];

    let rendered = render(RenderSemanticModel::Timeline(model));

    assert_eq!(
        rendered,
        concat!(
            "title(bytes=8)=\"Timeline\"\n",
            "accTitle(bytes=14)=\"Timeline title\"\n",
            "accDescr(bytes=20)=\"Timeline description\"\n",
            "direction: TD\n",
            "section(bytes=8)=\"Planning\"\n",
            "  - task(bytes=6)=\"Design\"\n",
            "    * event(bytes=7)=\"Kickoff\"\n",
            "  - task(bytes=9)=\"Implement\"\n",
            "    * event(bytes=10)=\"Build spec\"\n",
            "    * event(bytes=6)=\"Review\"",
        )
    );
}
#[test]
fn timeline_render_model_wraps_long_task_and_event_text() {
    let mut model = TimelineDiagramRenderModel::default();
    model.sections = vec!["Planning".to_string()];
    model.tasks = vec![TimelineRenderTask {
            id: 0,
            section: "Planning".to_string(),
            section_index: Some(0),
            task_type: "Planning".to_string(),
            task: "Design a very long integration event stream normalization workflow that still fits readable terminal output".to_string(),
            score: 0,
            events: vec![
                "Capture every upstream payload variant without losing the important operational context".to_string(),
            ],
        }];

    let rendered = render(RenderSemanticModel::Timeline(model));

    assert_eq!(
        rendered,
        concat!(
            "direction: LR\n",
            "section(bytes=8)=\"Planning\"\n",
            "  - task(bytes=107)=\"Design a very long integration event stream normalization w\n",
            "    orkflow that still fits readable terminal output\"\n",
            "    * event(bytes=87)=\"Capture every upstream payload variant without losing the\n",
            "       important operational context\"",
        )
    );
}

#[test]
fn timeline_structured_text_framing_distinguishes_task_text_from_events() {
    let mut embedded_event = TimelineDiagramRenderModel::default();
    embedded_event.tasks = vec![TimelineRenderTask {
        id: 1,
        section: String::new(),
        section_index: None,
        task_type: String::new(),
        task: "Task\n* Event".to_string(),
        score: 0,
        events: Vec::new(),
    }];

    let mut explicit_event = embedded_event.clone();
    explicit_event.tasks[0].task = "Task".to_string();
    explicit_event.tasks[0].events = vec!["Event".to_string()];

    assert_ne!(
        render(RenderSemanticModel::Timeline(embedded_event)),
        render(RenderSemanticModel::Timeline(explicit_event)),
        "authored task text must not be able to forge an event row",
    );
}
