use super::*;
use crate::options::AsciiRenderOptions;
use crate::resource::ResourceContext;
use crate::sequence::model::{
    SequenceArrowHead, SequenceCentralDecoration, SequenceLineStyle, SequenceMessage,
    SequenceMessageDirection,
};

#[test]
fn builder_owns_nested_controls_and_explicit_sections_before_layout() {
    let options = AsciiRenderOptions::ascii();
    let resources = ResourceContext::new(options.resources);
    let mut builder = SequenceTreeBuilder::new(6, &resources).unwrap();
    builder
        .start_control(
            0,
            SequenceControlKind::Loop,
            "outer".to_string(),
            None,
            &resources,
        )
        .unwrap();
    builder
        .start_control(
            1,
            SequenceControlKind::Alt,
            "choice".to_string(),
            None,
            &resources,
        )
        .unwrap();
    builder
        .push_event(message_event(2, 0, 1, "yes"), &resources)
        .unwrap();
    builder
        .start_section(
            3,
            SequenceControlKind::Alt,
            "otherwise".to_string(),
            &resources,
        )
        .unwrap();
    builder
        .push_event(message_event(4, 1, 0, "no"), &resources)
        .unwrap();
    builder
        .end_control(5, SequenceControlKind::Alt, &resources)
        .unwrap();
    builder
        .end_control(6, SequenceControlKind::Loop, &resources)
        .unwrap();

    let body = builder.finish().unwrap();
    assert_eq!(body.roots.len(), 1);
    let outer_id = body.roots[0];
    let SequenceItem::Control(outer) = &body.items[outer_id.0] else {
        panic!("the root item should be the outer control")
    };
    assert_eq!(outer.kind, SequenceControlKind::Loop);
    assert_eq!(outer.sections.len(), 1);
    assert_eq!(outer.sections[0].children.len(), 1);

    let inner_id = outer.sections[0].children[0];
    let SequenceItem::Control(inner) = &body.items[inner_id.0] else {
        panic!("the outer section should own the inner control")
    };
    assert_eq!(inner.kind, SequenceControlKind::Alt);
    assert_eq!(inner.sections.len(), 2);
    assert_eq!(
        inner.sections[1]
            .separator
            .as_ref()
            .map(|separator| separator.label.as_str()),
        Some("otherwise")
    );
    assert_eq!(
        inner.participant_span,
        Some(SequenceParticipantSpan { first: 0, last: 1 })
    );
}

#[test]
fn builder_admits_both_input_sized_containers_before_allocation() {
    use std::cell::Cell;

    const EXPECTED_ITEMS: usize = 3;
    const REQUIRED_WORK: usize = EXPECTED_ITEMS * 2;

    let exact_options = AsciiRenderOptions::ascii()
        .with_resource_limit(
            crate::resource::AsciiResourceLimitId::MaxLayoutWorkUnits,
            REQUIRED_WORK,
        )
        .unwrap();
    let exact_resources = ResourceContext::new(exact_options.resources);
    let exact_allocated = Cell::new(false);
    SequenceTreeBuilder::new_with_probe(EXPECTED_ITEMS, &exact_resources, || {
        exact_allocated.set(true)
    })
    .expect("the exact two-container work budget should be admitted");
    assert!(exact_allocated.get());
    assert_eq!(exact_resources.layout_work_used(), REQUIRED_WORK);

    let below_options = AsciiRenderOptions::ascii()
        .with_resource_limit(
            crate::resource::AsciiResourceLimitId::MaxLayoutWorkUnits,
            REQUIRED_WORK - 1,
        )
        .unwrap();
    let below_resources = ResourceContext::new(below_options.resources);
    let below_allocated = Cell::new(false);
    let error = match SequenceTreeBuilder::new_with_probe(EXPECTED_ITEMS, &below_resources, || {
        below_allocated.set(true)
    }) {
        Ok(_) => panic!("the limit-minus-one budget should reject before allocation"),
        Err(error) => error,
    };

    assert!(matches!(
        error,
        AsciiError::ResourceLimitExceeded(details)
            if details.limit == crate::resource::AsciiResourceLimitId::MaxLayoutWorkUnits
                && details.actual == REQUIRED_WORK
                && details.max == REQUIRED_WORK - 1
    ));
    assert!(!below_allocated.get());
    assert_eq!(below_resources.layout_work_used(), 0);
}

#[test]
fn nesting_limit_rejects_before_attaching_the_child_control() {
    let options = AsciiRenderOptions::ascii()
        .with_resource_limit(crate::resource::AsciiResourceLimitId::MaxNestingDepth, 1)
        .unwrap();
    let resources = ResourceContext::new(options.resources);
    let mut builder = SequenceTreeBuilder::new(2, &resources).unwrap();
    builder
        .start_control(
            0,
            SequenceControlKind::Loop,
            "outer".to_string(),
            None,
            &resources,
        )
        .unwrap();
    let work_checkpoint = resources.layout_work_used();

    let error = builder
        .start_control(
            1,
            SequenceControlKind::Opt,
            "inner".to_string(),
            None,
            &resources,
        )
        .unwrap_err();

    assert!(matches!(
        error,
        AsciiError::ResourceLimitExceeded(details)
            if details.limit == crate::resource::AsciiResourceLimitId::MaxNestingDepth
                && details.actual == 2
                && details.max == 1
    ));
    assert_eq!(resources.layout_work_used(), work_checkpoint);
    assert_eq!(builder.body.items.len(), 1);
    assert_eq!(builder.body.roots, vec![SequenceItemId(0)]);
    assert_eq!(builder.stack.len(), 1);
    assert!(
        builder.body.control(SequenceItemId(0)).unwrap().sections[0]
            .children
            .is_empty()
    );
}

fn message_event(model_index: usize, from: usize, to: usize, label: &str) -> SequenceEvent {
    SequenceEvent::Message(SequenceMessage {
        model_index,
        from,
        to,
        label: label.to_string(),
        wrap: false,
        style: SequenceLineStyle::Solid,
        source_marker: SequenceArrowHead::None,
        target_marker: SequenceArrowHead::Filled,
        direction: SequenceMessageDirection::Forward,
        central_decoration: SequenceCentralDecoration::None,
    })
}
