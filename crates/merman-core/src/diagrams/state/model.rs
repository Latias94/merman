pub(crate) use merman_model::families::state::StateDiagramModel;

pub(crate) fn sanitize_common_db_fields(
    model: &mut StateDiagramModel,
    config: &crate::MermaidConfig,
) {
    crate::common_db::sanitize_optional_acc_title(&mut model.acc_title, config);
    crate::common_db::sanitize_optional_acc_descr(&mut model.acc_descr, config);
}
