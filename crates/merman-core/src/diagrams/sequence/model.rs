pub(crate) use merman_model::families::sequence::{
    SequenceActor, SequenceActorLifecycle, SequenceAutonumber, SequenceBox, SequenceDiagramModel,
    SequenceMessage, SequenceMessagePayload, SequenceNote,
};

pub(crate) fn sanitize_common_db_fields(
    model: &mut SequenceDiagramModel,
    config: &crate::MermaidConfig,
) {
    crate::common_db::sanitize_optional_title(&mut model.title, config);
    crate::common_db::sanitize_optional_acc_title(&mut model.acc_title, config);
    crate::common_db::sanitize_optional_acc_descr(&mut model.acc_descr, config);
}
