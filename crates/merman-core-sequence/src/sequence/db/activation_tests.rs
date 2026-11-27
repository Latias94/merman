use super::*;
use merman_model::{
    CancelReason,
    families::sequence::{LINETYPE_CENTRAL_CONNECTION_DUAL, LINETYPE_LOOP_START, LINETYPE_SOLID},
};

// Independent reference for the historical message recount, including empty IDs and negative sums.
fn recount_activation(db: &SequenceDb, actor: &str) -> i32 {
    if actor.is_empty() {
        return 0;
    }
    db.messages
        .iter()
        .filter(|message| message.from.as_deref() == Some(actor))
        .map(|message| match message.message_type {
            LINETYPE_ACTIVE_START => 1,
            LINETYPE_ACTIVE_END => -1,
            _ => 0,
        })
        .sum()
}

fn apply_checked(db: &mut SequenceDb, action: Action) -> Result<(), String> {
    let expected_end = match &action {
        Action::ActiveEnd { actor } if recount_activation(db, actor) < 1 => Some(Err(format!(
            "Trying to inactivate an inactive participant ({actor})"
        ))),
        Action::ActiveEnd { .. } => Some(Ok(())),
        _ => None,
    };
    let before = format!("{db:?}");
    let result = db
        .apply_controlled(action, &OperationControl::new())
        .expect("an active control must not cancel");
    if let Some(expected) = expected_end {
        assert_eq!(result, expected);
    }
    if result.is_err() {
        assert_eq!(format!("{db:?}"), before, "failed action changed DB state");
    }
    for actor in db
        .messages
        .iter()
        .filter_map(|message| message.from.as_deref())
    {
        assert_eq!(
            db.activation_count(actor),
            i64::from(recount_activation(db, actor)),
            "raw actor ID: {actor:?}"
        );
    }
    for actor in db.activation_depths.keys() {
        assert_eq!(
            db.activation_count(actor),
            i64::from(recount_activation(db, actor))
        );
    }
    assert_eq!(db.activation_count(""), 0);
    assert!(!db.activation_depths.contains_key(""));
    result
}

fn message(from: &str, to: &str, signal_type: i32) -> Action {
    Action::AddMessage {
        from: from.to_string(),
        to: to.to_string(),
        signal_type,
        text: "payload".to_string(),
        activate: false,
        central_connection: 0,
    }
}

#[test]
fn mixed_nested_and_sequential_activations_match_historical_recount() {
    let mut db = SequenceDb::new(Some(true));
    for actor in ["A", "B"] {
        apply_checked(
            &mut db,
            Action::EnsureParticipant {
                id: actor.to_string(),
            },
        )
        .unwrap();
    }
    for _ in 0..16 {
        for action in [
            Action::ActiveStart { actor: "A".into() },
            message("A", "B", LINETYPE_SOLID),
            Action::AddNote {
                actors: vec!["A".into(), "B".into()],
                placement: 2,
                text: "wrap: note".into(),
            },
            Action::Autonumber {
                start: Some(1.0),
                step: Some(2.0),
                visible: true,
            },
            Action::ActiveStart { actor: "B".into() },
            Action::ActiveStart { actor: "A".into() },
            Action::ControlSignal {
                signal_type: LINETYPE_LOOP_START,
                text: Some("loop".into()),
            },
            Action::ActiveEnd { actor: "A".into() },
            Action::ActiveEnd { actor: "B".into() },
            Action::ActiveEnd { actor: "A".into() },
        ] {
            apply_checked(&mut db, action).unwrap();
        }
    }
    apply_checked(&mut db, Action::ActiveEnd { actor: "B".into() }).unwrap_err();
    assert_eq!(db.activation_depths.len(), 2);
    assert!(db.activation_depths.values().all(|depth| *depth == 0));
    let model = db.into_model();
    assert_eq!(model.notes.len(), 16);
    assert_eq!(model.messages.len(), 160);
    for (index, message) in model.messages.iter().enumerate() {
        assert_eq!(message.id, index.to_string());
    }
}

#[test]
fn raw_ids_and_empty_ids_match_recount_before_and_after_declaration() {
    let mut db = SequenceDb::new(None);
    let ids = ["A", " A", "A ", "a", " ", "é", "e\u{301}", "😀", ""];
    for actor in ids {
        apply_checked(
            &mut db,
            Action::ActiveStart {
                actor: actor.into(),
            },
        )
        .unwrap();
    }
    assert!(db.actors.is_empty());
    assert_eq!(db.activation_depths.len(), ids.len() - 1);
    apply_checked(&mut db, Action::ActiveEnd { actor: "".into() }).unwrap_err();
    apply_checked(
        &mut db,
        Action::ActiveEnd {
            actor: "A\u{a0}".into(),
        },
    )
    .unwrap_err();

    for actor in ids {
        apply_checked(
            &mut db,
            Action::AddParticipant {
                id: actor.into(),
                description: Some("shared display name".into()),
                draw: "actor".into(),
                config: None,
            },
        )
        .unwrap();
        if actor.is_empty() {
            apply_checked(
                &mut db,
                Action::ActiveEnd {
                    actor: actor.into(),
                },
            )
            .unwrap_err();
        } else {
            apply_checked(
                &mut db,
                Action::ActiveEnd {
                    actor: actor.into(),
                },
            )
            .unwrap();
        }
    }
    assert_eq!(db.actor_order, ids.map(str::to_string).to_vec());
    assert!(db.activation_depths.values().all(|depth| *depth == 0));
}

#[test]
fn activation_survives_redeclaration_and_lifecycle_errors_leave_state_unchanged() {
    let mut db = SequenceDb::new(None);
    apply_checked(&mut db, Action::ActiveStart { actor: "B".into() }).unwrap();
    apply_checked(
        &mut db,
        Action::CreateParticipant {
            id: "B".into(),
            description: None,
            draw: "participant".into(),
            config: None,
        },
    )
    .unwrap();
    let error = apply_checked(&mut db, message("A", "C", LINETYPE_ACTIVE_START)).unwrap_err();
    assert_eq!(
        error,
        "The created participant B does not have an associated creating message after its declaration. Please check the sequence diagram."
    );
    apply_checked(&mut db, Action::ActiveEnd { actor: "C".into() }).unwrap_err();
    apply_checked(&mut db, message("A", "B", LINETYPE_SOLID)).unwrap();
    for description in [Some("first alias"), None] {
        apply_checked(
            &mut db,
            Action::AddParticipant {
                id: "B".into(),
                description: description.map(str::to_string),
                draw: "actor".into(),
                config: None,
            },
        )
        .unwrap();
    }
    apply_checked(&mut db, Action::DestroyParticipant { id: "B".into() }).unwrap();
    apply_checked(&mut db, Action::ActiveStart { actor: "B".into() }).unwrap();
    let error = apply_checked(&mut db, message("A", "C", LINETYPE_ACTIVE_END)).unwrap_err();
    assert_eq!(
        error,
        "The destroyed participant B does not have an associated destroying message after its declaration. Please check the sequence diagram."
    );
    apply_checked(&mut db, message("B", "A", LINETYPE_SOLID)).unwrap();
    apply_checked(
        &mut db,
        Action::AddParticipant {
            id: "B".into(),
            description: Some("final alias".into()),
            draw: "actor".into(),
            config: None,
        },
    )
    .unwrap();
    apply_checked(&mut db, Action::ActiveEnd { actor: "B".into() }).unwrap();
    apply_checked(&mut db, Action::ActiveEnd { actor: "B".into() }).unwrap();
    apply_checked(&mut db, Action::ActiveEnd { actor: "B".into() }).unwrap_err();
    let model = db.into_model();
    assert_eq!(model.created_actors.get("B"), Some(&1));
    assert_eq!(model.destroyed_actors.get("B"), Some(&2));
    assert_eq!(model.created_actor_message_index("B"), Some(1));
    assert_eq!(model.destroyed_actor_message_index("B"), Some(3));
    assert_eq!(model.actors["B"].description, "final alias");
    assert_eq!(model.messages.len(), 6);
}

#[test]
fn activation_follows_record_type_and_from_id_even_for_nonstandard_actions() {
    let mut db = SequenceDb::new(None);
    for signal_type in [LINETYPE_ACTIVE_START, LINETYPE_ACTIVE_END] {
        apply_checked(
            &mut db,
            Action::ControlSignal {
                signal_type,
                text: None,
            },
        )
        .unwrap();
    }
    assert!(db.activation_depths.is_empty());
    apply_checked(&mut db, message("A", "B", LINETYPE_ACTIVE_END)).unwrap();
    assert_eq!(db.activation_count("A"), -1);
    apply_checked(&mut db, Action::ActiveEnd { actor: "A".into() }).unwrap_err();
    apply_checked(&mut db, Action::ActiveStart { actor: "A".into() }).unwrap();
    apply_checked(&mut db, Action::ActiveEnd { actor: "A".into() }).unwrap_err();
    apply_checked(&mut db, message("A", "B", LINETYPE_ACTIVE_START)).unwrap();
    apply_checked(&mut db, Action::ActiveEnd { actor: "A".into() }).unwrap();
    apply_checked(&mut db, message("B", "A", LINETYPE_ACTIVE_START)).unwrap();
    apply_checked(&mut db, Action::ActiveEnd { actor: "A".into() }).unwrap_err();
    apply_checked(&mut db, Action::ActiveEnd { actor: "B".into() }).unwrap();
    apply_checked(&mut db, message("", "B", LINETYPE_ACTIVE_START)).unwrap();
    apply_checked(&mut db, message("", "B", LINETYPE_ACTIVE_END)).unwrap();
    apply_checked(&mut db, Action::ActiveEnd { actor: "".into() }).unwrap_err();
    for signal_type in [-1, i32::MAX] {
        apply_checked(&mut db, message("C", "A", signal_type)).unwrap();
    }
    assert_eq!(db.activation_depths.len(), 2);
}

#[test]
fn central_connections_and_activate_flags_do_not_change_activation_depth() {
    let mut db = SequenceDb::new(None);
    for central_connection in [
        0,
        LINETYPE_CENTRAL_CONNECTION,
        LINETYPE_CENTRAL_CONNECTION_REVERSE,
        LINETYPE_CENTRAL_CONNECTION_DUAL,
    ] {
        apply_checked(
            &mut db,
            Action::AddMessage {
                from: "A".into(),
                to: "B".into(),
                signal_type: LINETYPE_SOLID,
                text: "central".into(),
                activate: true,
                central_connection,
            },
        )
        .unwrap();
    }
    apply_checked(&mut db, Action::CentralConnection { actor: "B".into() }).unwrap();
    apply_checked(
        &mut db,
        Action::CentralConnectionReverse { actor: "A".into() },
    )
    .unwrap();
    for actor in ["A", "B"] {
        apply_checked(
            &mut db,
            Action::ActiveEnd {
                actor: actor.into(),
            },
        )
        .unwrap_err();
    }
    assert_eq!(db.activation_depths.capacity(), 0);
    apply_checked(&mut db, Action::ActiveStart { actor: "B".into() }).unwrap();
    apply_checked(&mut db, Action::CentralConnection { actor: "B".into() }).unwrap();
    apply_checked(&mut db, Action::ActiveEnd { actor: "B".into() }).unwrap();
    apply_checked(&mut db, Action::ActiveEnd { actor: "B".into() }).unwrap_err();
}

#[test]
fn ordinary_messages_notes_and_autonumber_leave_depth_storage_unallocated() {
    let mut db = SequenceDb::new(None);
    for _ in 0..32 {
        for action in [
            message("A", "B", LINETYPE_SOLID),
            Action::AddNote {
                actors: vec!["A".into(), "B".into()],
                placement: 2,
                text: "note".into(),
            },
            Action::Autonumber {
                start: None,
                step: None,
                visible: false,
            },
        ] {
            apply_checked(&mut db, action).unwrap();
        }
    }
    apply_checked(&mut db, Action::ActiveEnd { actor: "A".into() }).unwrap_err();
    assert!(db.activation_depths.is_empty());
    assert_eq!(db.activation_depths.capacity(), 0);
    assert_eq!(db.messages.len(), 96);
}

#[test]
fn activation_actions_preserve_both_checkpoints_and_cancel_before_mutation() {
    for successful_checkpoints in 0..=2 {
        for action in [
            Action::ActiveStart { actor: "A".into() },
            Action::ActiveEnd { actor: "A".into() },
            message("A", "B", LINETYPE_ACTIVE_START),
            message("A", "B", LINETYPE_ACTIVE_END),
        ] {
            let mut db = SequenceDb::new(None);
            apply_checked(&mut db, Action::ActiveStart { actor: "A".into() }).unwrap();
            let before = format!("{db:?}");
            let control = OperationControl::new();
            control.cancel_after_checkpoints(successful_checkpoints);
            let result = db.apply_controlled(action, &control);
            if successful_checkpoints < 2 {
                assert_eq!(result.unwrap_err().reason, CancelReason::Requested);
                assert_eq!(format!("{db:?}"), before);
                apply_checked(&mut db, Action::ActiveEnd { actor: "A".into() }).unwrap();
            } else {
                result.unwrap().unwrap();
                assert_eq!(
                    db.activation_count("A"),
                    i64::from(recount_activation(&db, "A"))
                );
                assert_eq!(
                    control.checkpoint().unwrap_err().reason,
                    CancelReason::Requested
                );
            }
        }
    }
    let mut db = SequenceDb::new(None);
    let control = OperationControl::new();
    control.cancel();
    let before = format!("{db:?}");
    let result = db.apply_controlled(Action::ActiveEnd { actor: "".into() }, &control);
    assert_eq!(result.unwrap_err().reason, CancelReason::Requested);
    assert_eq!(format!("{db:?}"), before);
}

#[test]
fn activation_depth_covers_counts_beyond_the_old_signed_32_bit_boundary() {
    let mut db = SequenceDb::new(None);
    // Seed the boundary state rather than retaining billions of synthetic message records.
    db.activation_depths.insert("A".into(), i64::from(i32::MAX));
    db.apply_controlled(
        Action::ActiveStart { actor: "A".into() },
        &OperationControl::new(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(db.activation_count("A"), i64::from(i32::MAX) + 1);
    db.apply_controlled(
        Action::ActiveEnd { actor: "A".into() },
        &OperationControl::new(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(db.activation_count("A"), i64::from(i32::MAX));

    db.activation_depths.insert("B".into(), i64::from(i32::MIN));
    db.apply_controlled(
        message("B", "A", LINETYPE_ACTIVE_END),
        &OperationControl::new(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(db.activation_count("B"), i64::from(i32::MIN) - 1);
    let before = format!("{db:?}");
    let error = db
        .apply_controlled(
            Action::ActiveEnd { actor: "B".into() },
            &OperationControl::new(),
        )
        .unwrap()
        .unwrap_err();
    assert_eq!(error, "Trying to inactivate an inactive participant (B)");
    assert_eq!(format!("{db:?}"), before);
}
