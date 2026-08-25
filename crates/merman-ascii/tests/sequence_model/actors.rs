use super::*;

#[test]
fn sequence_titles_render_above_participants() {
    let rendered = render_sequence(
        "sequenceDiagram\ntitle: Setup\nparticipant A\nparticipant B\nA->>B: Hi",
        &AsciiRenderOptions::ascii(),
    )
    .expect("sequence title should render");

    assert_eq!(
        rendered,
        concat!(
            "     Setup\n",
            "+---+     +---+\n",
            "| A |     | B |\n",
            "+-+-+     +-+-+\n",
            "  |         |\n",
            "  | Hi      |\n",
            "  +-------->|\n",
            "  |         |\n",
        )
    );

    let boxed = render_sequence(
        "sequenceDiagram\ntitle: Setup\nbox Group\nparticipant A\nparticipant B\nend\nA->>B: Hi",
        &AsciiRenderOptions::ascii(),
    )
    .expect("sequence title should render outside boxes");
    let mut boxed_lines = boxed.lines();
    assert_eq!(boxed_lines.next().unwrap().trim(), "Setup");
    assert!(
        boxed_lines.next().unwrap().starts_with("+- Group"),
        "title should stay above sequence boxes:\n{boxed}"
    );
}

#[test]
fn sequence_actor_keyword_renders_as_participant_box() {
    let rendered = render_sequence(
        "sequenceDiagram\nactor U as User\nparticipant S as System\nU->>S: Click",
        &AsciiRenderOptions::ascii(),
    )
    .expect("sequence actor keyword should render in ASCII");

    assert!(
        rendered.contains("| User |") && rendered.contains("| System |"),
        "actor and participant labels should both render as ASCII participant boxes:\n{rendered}"
    );
    assert!(
        rendered.contains("Click"),
        "messages involving actor declarations should keep rendering:\n{rendered}"
    );
}

#[test]
fn sequence_extended_actor_types_render_as_participant_boxes() {
    let rendered = render_sequence(
        concat!(
            "sequenceDiagram\n",
            "participant API@{ \"type\" : \"boundary\", \"alias\": \"Public API\" }\n",
            "participant Auth@{ \"type\" : \"control\" } as Auth Controller\n",
            "participant Entity@{ \"type\" : \"entity\" }\n",
            "participant DB@{ \"type\" : \"database\" }\n",
            "participant Queue@{ \"type\" : \"queue\" }\n",
            "actor Store@{ \"type\" : \"collections\" } as Event Store\n",
            "API->>Auth: Request\n",
            "Auth->>Entity: Validate\n",
            "Entity->>DB: Query\n",
            "DB-->>Queue: Result\n",
            "Queue-->>Store: Publish",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("extended actor types should render as ASCII participant boxes");

    for label in [
        "Public API",
        "Auth Controller",
        "Entity",
        "DB",
        "Queue",
        "Event Store",
    ] {
        assert!(
            rendered.contains(label),
            "extended actor label {label:?} should render:\n{rendered}"
        );
    }
    for message in ["Request", "Validate", "Query", "Result", "Publish"] {
        assert!(
            rendered.contains(message),
            "messages involving extended actor types should render:\n{rendered}"
        );
    }
}

#[test]
fn sequence_unknown_actor_types_are_explicitly_unsupported() {
    let mut model = basic_sequence_model();
    model.actors.get_mut("A").unwrap().actor_type = "gateway".to_string();

    assert_unsupported_sequence_model(model, "actor types");
}

#[test]
fn sequence_wrapped_actor_labels_render_multiline_participant_boxes() {
    let mut model = basic_sequence_model();
    let actor = model.actors.get_mut("A").unwrap();
    actor.description = "Public API Gateway".to_string();
    actor.wrap = true;

    let rendered = render_sequence_model(&model, &AsciiRenderOptions::ascii())
        .expect("sequence should render");
    let normalized = normalize_sequence_output(&rendered);
    let lines = normalized.lines().collect::<Vec<_>>();

    assert_eq!(lines[0], "+------------+");
    assert_eq!(lines[1], "| Public API |");
    assert_eq!(lines[2], "|  Gateway   |");
    assert_eq!(lines[3], "+------+-----+");
    assert_eq!(lines[4], "       |");
}

#[test]
fn sequence_actor_label_html_breaks_render_multiline_participant_boxes() {
    let rendered = render_sequence(
        "sequenceDiagram\nparticipant A as First<br />Line\nA->>A: Hi",
        &AsciiRenderOptions::ascii(),
    )
    .expect("sequence should render");
    let normalized = normalize_sequence_output(&rendered);
    let lines = normalized.lines().collect::<Vec<_>>();

    assert!(
        lines.len() >= 5,
        "rendered sequence should include multiline participant header:\n{rendered}"
    );
    assert_eq!(lines[0], "+-------+");
    assert_eq!(lines[1], "| First |");
    assert_eq!(lines[2], "| Line  |");
    assert_eq!(lines[3], "+---+---+");
    assert_eq!(lines[4], "    |");
}

#[test]
fn sequence_actor_links_do_not_block_ascii_rendering() {
    let rendered = render_sequence(
        concat!(
            "sequenceDiagram\n",
            "participant A\n",
            "participant B\n",
            "links A: { \"Docs\": \"https://example.com/docs\" }\n",
            "link B: Repo @ https://example.com/repo\n",
            "A->>B: Hi",
        ),
        &AsciiRenderOptions::ascii(),
    )
    .expect("sequence actor links should not block ASCII rendering");

    assert!(
        rendered.contains("Hi"),
        "linked actors should keep sequence messages renderable:\n{rendered}"
    );
    assert!(
        !rendered.contains("example.com"),
        "actor link URLs are SVG metadata and should not leak into ASCII output:\n{rendered}"
    );
}

#[test]
fn sequence_actor_properties_are_accepted_as_omitted_metadata() {
    let mut model = basic_sequence_model();
    model
        .actors
        .get_mut("A")
        .unwrap()
        .properties
        .insert("icon".to_string(), "@clock".into());

    let rendered = render_sequence_model(&model, &AsciiRenderOptions::ascii())
        .expect("actor presentation properties should not block ASCII rendering");
    assert!(rendered.contains('A'));
    assert!(!rendered.contains("icon"));
    assert!(!rendered.contains("@clock"));
}

#[test]
fn sequence_actor_order_must_be_a_complete_actor_permutation() {
    let mut unknown = basic_sequence_model();
    unknown.actor_order = vec!["missing".to_string()];

    let mut duplicate = basic_sequence_model();
    add_sequence_participant(&mut duplicate, "B");
    duplicate.actor_order = vec!["A".to_string(), "A".to_string()];

    let mut omitted = basic_sequence_model();
    add_sequence_participant(&mut omitted, "B");
    omitted.actor_order = vec!["A".to_string()];

    for model in [unknown, duplicate, omitted] {
        assert_unsupported_sequence_model(model, "actor order");
    }
}

#[test]
fn sequence_empty_actor_order_uses_deterministic_model_order() {
    let mut model = basic_sequence_model();
    add_sequence_participant(&mut model, "B");
    model.actor_order.clear();
    model.messages.push(message(Some("A"), Some("B"), 0));

    let rendered = render_sequence_model(&model, &AsciiRenderOptions::ascii())
        .expect("an absent compatibility actor order should use the ordered actor map");
    assert!(rendered.contains("A") && rendered.contains("B") && rendered.contains("Hi"));
}
