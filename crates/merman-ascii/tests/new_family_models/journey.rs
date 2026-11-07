use super::*;

#[test]
fn journey_render_model_renders_actors_sections_and_scores() {
    let model = JourneyDiagramModel {
        title: Some("Journey".to_string()),
        acc_title: Some("Journey title".to_string()),
        acc_descr: Some("Journey description".to_string()),
        sections: vec!["Discovery".to_string()],
        tasks: vec![
            JourneyTask {
                score: 5.0,
                score_is_nan: false,
                people: vec!["Alice".to_string(), "Bob".to_string()],
                section: "Discovery".to_string(),
                section_index: Some(0),
                task_type: "Discovery".to_string(),
                task: "Research".to_string(),
            },
            JourneyTask {
                score: 3.0,
                score_is_nan: false,
                people: vec!["Bob".to_string()],
                section: "Discovery".to_string(),
                section_index: Some(0),
                task_type: "Discovery".to_string(),
                task: "Ship".to_string(),
            },
        ],
        ..Default::default()
    };

    let rendered = render(RenderSemanticModel::Journey(model));

    assert_eq!(
        rendered,
        concat!(
            "title(bytes=7)=\"Journey\"\n",
            "accTitle(bytes=13)=\"Journey title\"\n",
            "accDescr(bytes=19)=\"Journey description\"\n",
            "actors=[bytes=5 \"Alice\", bytes=3 \"Bob\"]\n",
            "section(bytes=9)=\"Discovery\"\n",
            "  - task(bytes=8)=\"Research\" score=5 people=[bytes=5 \"Alice\", bytes=3 \"Bob\"]\n",
            "  - task(bytes=4)=\"Ship\" score=3 people=[bytes=3 \"Bob\"]",
        )
    );
}
#[test]
fn journey_structured_text_framing_distinguishes_actor_list_items() {
    let one_actor = JourneyDiagramModel {
        actors: vec!["Alice, Bob".to_string()],
        ..Default::default()
    };

    let two_actors = JourneyDiagramModel {
        actors: vec!["Alice".to_string(), "Bob".to_string()],
        ..Default::default()
    };

    assert_ne!(
        render(RenderSemanticModel::Journey(one_actor)),
        render(RenderSemanticModel::Journey(two_actors)),
        "one authored actor containing a comma must differ from two actor values",
    );
}

#[test]
fn journey_direct_scores_preserve_fractions_integer_text_and_nan_controls() {
    for (score, score_is_nan, expected) in [
        (3.5, false, "3.5"),
        (2.75, false, "2.75"),
        (4.0, false, "4"),
        (4.0, true, "NaN"),
        (f64::NAN, false, "NaN"),
        (f64::INFINITY, false, "NaN"),
        (f64::NEG_INFINITY, false, "NaN"),
    ] {
        let model = JourneyDiagramModel {
            tasks: vec![JourneyTask {
                score,
                score_is_nan,
                people: Vec::new(),
                section: String::new(),
                section_index: None,
                task_type: String::new(),
                task: "Pay".to_string(),
            }],
            ..Default::default()
        };
        let rendered = render(RenderSemanticModel::Journey(model));
        assert!(
            rendered.contains(&format!("score={expected}")),
            "{rendered}"
        );
    }
}
