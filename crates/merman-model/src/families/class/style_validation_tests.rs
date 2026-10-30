use super::*;
use crate::{MermaidConfig, OperationControl};

fn model() -> ClassDiagram {
    let mut model = super::test_data::authored_model(true);
    for name in ["accent with spaces", "shade"] {
        model.style_classes.insert(
            name.to_owned(),
            StyleClass {
                id: name.to_owned(),
                styles: vec!["fill:red".to_owned(), "font-size:20px".to_owned()],
                text_styles: Vec::new(),
            },
        );
    }
    model.classes["Service"].css_classes = "default accent with spaces".to_owned();
    model.classes["Service"].styles = vec!["fill:red".to_owned(), "fill:blue".to_owned()];
    model.classes["note0"].css_classes = "default shade".to_owned();
    ClassDiagram::new(model, &MermaidConfig::empty_object())
}

fn facts() -> ClassStylePrecedenceFacts {
    ClassStylePrecedenceFacts::from_witnesses(
        Some(ClassStylePrecedenceWitness::new(
            "accent with spaces".to_owned(),
            "Service".to_owned(),
            vec!["fill:red".to_owned()],
            10_000,
            30_000,
        )),
        Some(ClassStylePrecedenceWitness::new(
            "shade".to_owned(),
            "note0".to_owned(),
            vec!["fill:red".to_owned()],
            usize::MAX - 1,
            usize::MAX,
        )),
        Some(ClassStyleDeclarationWitness::new(
            "Service".to_owned(),
            vec!["fill:blue".to_owned()],
            40_000,
        )),
        Some(ClassStyleDeclarationWitness::new(
            "accent with spaces".to_owned(),
            vec!["font-size:20px".to_owned()],
            50_000,
        )),
    )
}

#[test]
fn witnesses_accept_independent_event_ordinals_and_canonical_collision_targets() {
    let model = model();
    let control = OperationControl::new();
    let facts = facts();
    assert_eq!(
        facts
            .validate_for_model_controlled(&model, &control)
            .unwrap(),
        Ok(())
    );
    assert_eq!(
        ClassStylePrecedenceFacts::default()
            .validate_for_model_controlled(&model, &control)
            .unwrap(),
        Ok(())
    );
    assert_eq!(
        facts
            .definition_before_assignment_no_backfill_witness()
            .unwrap()
            .target(),
        "note0"
    );
    assert_eq!(model.notes().next().unwrap().id, "note0$1");
}

#[test]
fn precedence_witnesses_require_class_targets_classdefs_and_ordered_events() {
    let model = model();
    for (class_name, target, earlier, later, styles) in [
        ("missing", "Service", 3, 9, vec!["fill:red".to_owned()]),
        ("shade", "missing", 3, 9, vec!["fill:red".to_owned()]),
        ("shade", "Service", 9, 9, vec!["fill:red".to_owned()]),
        ("shade", "Service", 10, 9, vec!["fill:red".to_owned()]),
        ("shade", "Service", 3, 9, Vec::new()),
    ] {
        let witness = ClassStylePrecedenceWitness::new(
            class_name.to_owned(),
            target.to_owned(),
            styles,
            earlier,
            later,
        );
        for facts in [
            ClassStylePrecedenceFacts::from_witnesses(Some(witness.clone()), None, None, None),
            ClassStylePrecedenceFacts::from_witnesses(None, Some(witness.clone()), None, None),
        ] {
            assert!(
                facts
                    .validate_for_model_controlled(&model, &OperationControl::new())
                    .unwrap()
                    .is_err(),
                "{class_name} -> {target}: {earlier} < {later}"
            );
        }
    }
}

#[test]
fn declaration_witnesses_distinguish_class_targets_from_classdef_targets() {
    let model = model();
    for facts in [
        ClassStylePrecedenceFacts::from_witnesses(
            None,
            None,
            Some(ClassStyleDeclarationWitness::new(
                "shade".to_owned(),
                vec!["fill:red".to_owned()],
                0,
            )),
            None,
        ),
        ClassStylePrecedenceFacts::from_witnesses(
            None,
            None,
            Some(ClassStyleDeclarationWitness::new(
                "Service".to_owned(),
                Vec::new(),
                0,
            )),
            None,
        ),
        ClassStylePrecedenceFacts::from_witnesses(
            None,
            None,
            None,
            Some(ClassStyleDeclarationWitness::new(
                "Service".to_owned(),
                vec!["font-size:20px".to_owned()],
                0,
            )),
        ),
        ClassStylePrecedenceFacts::from_witnesses(
            None,
            None,
            None,
            Some(ClassStyleDeclarationWitness::new(
                "shade".to_owned(),
                Vec::new(),
                0,
            )),
        ),
    ] {
        assert!(
            facts
                .validate_for_model_controlled(&model, &OperationControl::new())
                .unwrap()
                .is_err()
        );
    }
}

#[test]
fn note_and_namespace_ids_do_not_substitute_for_class_targets() {
    let mut canonical = super::test_data::authored_model(false);
    canonical.notes.push(ClassNote {
        id: "only-note".to_owned(),
        class_id: None,
        text: "note".to_owned(),
        parent: None,
    });
    let model = ClassDiagram::new(canonical, &MermaidConfig::empty_object());
    for target in ["only-note", "A.B"] {
        let facts = ClassStylePrecedenceFacts::from_witnesses(
            None,
            None,
            Some(ClassStyleDeclarationWitness::new(
                target.to_owned(),
                vec!["fill:red".to_owned()],
                0,
            )),
            None,
        );
        assert!(
            facts
                .validate_for_model_controlled(&model, &OperationControl::new())
                .unwrap()
                .is_err()
        );
    }
}

#[test]
fn witness_validation_observes_cancellation_before_rejection_and_between_witnesses() {
    let model = model();
    let mut invalid = facts();
    invalid
        .assignment_before_definition_copy
        .as_mut()
        .unwrap()
        .target = "missing".to_owned();
    let control = OperationControl::new();
    control.cancel();
    assert!(
        invalid
            .validate_for_model_controlled(&model, &control)
            .is_err()
    );

    let control = OperationControl::new();
    control.cancel_after_checkpoints(2);
    assert!(
        facts()
            .validate_for_model_controlled(&model, &control)
            .is_err()
    );
}
