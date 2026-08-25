use super::*;

#[test]
fn sequence_note_projection_must_match_the_typed_note_facts() {
    let mut base = basic_sequence_model();
    add_sequence_participant(&mut base, "B");
    base.notes.push(SequenceNote {
        actor: vec!["A", "B"].into(),
        message: "typed note".to_string(),
        placement: 2,
        wrap: true,
    });
    base.messages.push(SequenceMessage {
        id: "n0".to_string(),
        from: Some("A".to_string()),
        to: Some("B".to_string()),
        message_type: 2,
        message: SequenceMessagePayload::Text("typed note".to_string()),
        wrap: true,
        activate: false,
        placement: Some(2),
        central_connection: 0,
    });

    let mut wrong_text = base.clone();
    wrong_text.messages[0].message = SequenceMessagePayload::Text("other note".to_string());

    let mut wrong_actor = base.clone();
    wrong_actor.messages[0].to = Some("A".to_string());

    let mut wrong_placement = base.clone();
    wrong_placement.messages[0].placement = Some(1);

    let mut missing_message = base.clone();
    missing_message.messages.clear();

    for model in [wrong_text, wrong_actor, wrong_placement, missing_message] {
        assert_unsupported_sequence_model(model, "note model consistency");
    }
}

#[test]
fn sequence_note_messages_are_the_compatibility_source_when_notes_are_absent() {
    let mut model = basic_sequence_model();
    let mut note = message(Some("A"), Some("A"), 2);
    note.message = SequenceMessagePayload::Text("message-owned note".to_string());
    note.placement = Some(1);
    model.messages.push(note);

    let rendered = render_sequence_model(&model, &AsciiRenderOptions::ascii())
        .expect("ordered note messages should remain drawable without the duplicate notes vector");
    assert!(rendered.contains("message-owned note"));
}

#[test]
fn sequence_message_payload_must_match_its_semantic_kind() {
    let autonumber = SequenceMessagePayload::Autonumber(SequenceAutonumber {
        start: Some(5.0),
        step: Some(2.0),
        visible: true,
    });

    let mut signal_payload = basic_sequence_model();
    let mut signal = message(Some("A"), Some("A"), 0);
    signal.message = autonumber.clone();
    signal_payload.messages.push(signal);

    let mut note_payload = basic_sequence_model();
    let mut note = message(Some("A"), Some("A"), 2);
    note.message = autonumber;
    note.placement = Some(1);
    note_payload.messages.push(note);

    let mut autonumber_payload = basic_sequence_model();
    autonumber_payload
        .messages
        .push(message(None, None, LINETYPE_AUTONUMBER));

    for model in [signal_payload, note_payload, autonumber_payload] {
        assert_unsupported_sequence_model(model, "message payload shape");
    }
}

#[test]
fn sequence_structural_message_records_reject_unconsumed_fields() {
    let mut activation_with_target = basic_sequence_model();
    let mut activation = message(Some("A"), Some("A"), 17);
    activation.message = SequenceMessagePayload::Text(String::new());
    activation_with_target.messages.push(activation);

    let mut activation_with_text = basic_sequence_model();
    activation_with_text
        .messages
        .push(message(Some("A"), None, 17));

    let mut control_end_with_text = basic_sequence_model();
    control_end_with_text
        .messages
        .push(message(None, None, LINETYPE_LOOP_END));

    let mut note_without_placement = basic_sequence_model();
    note_without_placement
        .messages
        .push(message(Some("A"), Some("A"), 2));

    for model in [
        activation_with_target,
        activation_with_text,
        control_end_with_text,
        note_without_placement,
    ] {
        assert_unsupported_sequence_model(model, "message record shape");
    }
}

#[test]
fn sequence_autonumber_values_must_be_finite() {
    for (start, step) in [
        (Some(f64::NAN), Some(1.0)),
        (Some(1.0), Some(f64::INFINITY)),
        (Some(f64::NEG_INFINITY), None),
    ] {
        let mut model = basic_sequence_model();
        model.messages.push(SequenceMessage {
            id: "auto".to_string(),
            from: None,
            to: None,
            message_type: LINETYPE_AUTONUMBER,
            message: SequenceMessagePayload::Autonumber(SequenceAutonumber {
                start,
                step,
                visible: true,
            }),
            wrap: false,
            activate: false,
            placement: None,
            central_connection: 0,
        });
        assert_unsupported_sequence_model(model, "autonumber values");
    }
}

#[test]
fn sequence_central_records_must_bind_their_visible_signal() {
    let central_record = || {
        let mut record = message(Some("A"), None, 59);
        record.message = SequenceMessagePayload::Text(String::new());
        record
    };

    let mut orphan = basic_sequence_model();
    orphan.messages.push(central_record());
    assert_unsupported_sequence_model(orphan, "central connection records");

    let mut missing = basic_sequence_model();
    let mut decorated = message(Some("A"), Some("A"), 0);
    decorated.central_connection = 59;
    decorated.activate = true;
    missing.messages.push(decorated.clone());
    assert_unsupported_sequence_model(missing, "central connection records");

    let mut valid = basic_sequence_model();
    valid.messages.push(decorated);
    valid.messages.push(central_record());
    let rendered = render_sequence_model(&valid, &AsciiRenderOptions::unicode())
        .expect("a central record bound to its visible signal should render");
    assert!(rendered.contains("Hi") && rendered.contains('○'));
}

#[test]
fn sequence_activated_signals_must_bind_their_target_state_event() {
    let mut missing = basic_sequence_model();
    add_sequence_participant(&mut missing, "B");
    missing.messages.push(SequenceMessage {
        activate: true,
        ..message(Some("A"), Some("B"), 0)
    });

    let mut wrong_actor = missing.clone();
    let mut wrong_start = message(Some("A"), None, 17);
    wrong_start.message = SequenceMessagePayload::Text(String::new());
    wrong_actor.messages.push(wrong_start);

    for model in [missing, wrong_actor] {
        assert_unsupported_sequence_model(model, "activation state events");
    }

    let mut valid = basic_sequence_model();
    add_sequence_participant(&mut valid, "B");
    valid.messages.push(SequenceMessage {
        activate: true,
        ..message(Some("A"), Some("B"), 0)
    });
    let mut start = message(Some("B"), None, 17);
    start.message = SequenceMessagePayload::Text(String::new());
    valid.messages.push(start);
    render_sequence_model(&valid, &AsciiRenderOptions::unicode())
        .expect("a matching activation record should satisfy direct-model admission");
}

#[test]
fn sequence_other_model_features_are_explicitly_unsupported() {
    let mut cases = Vec::new();

    let mut model = basic_sequence_model();
    model.messages.push(SequenceMessage {
        placement: Some(0),
        ..message(Some("A"), Some("A"), 0)
    });
    cases.push((model, "message placement"));

    let mut model = basic_sequence_model();
    model.messages.push(message(None, None, 0));
    cases.push((model, "message record shape"));

    let mut model = basic_sequence_model();
    model.messages.push(message(Some("A"), Some("B"), 0));
    cases.push((model, "messages with unknown actors"));

    let mut model = basic_sequence_model();
    model.messages.push(message(Some("A"), Some("A"), 99));
    cases.push((model, "message types"));

    for (model, feature) in cases {
        assert_unsupported_sequence_model(model, feature);
    }
}
