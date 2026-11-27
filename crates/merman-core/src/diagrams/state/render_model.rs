pub(crate) use merman_model::families::state::StateDiagramRenderModel;

#[cfg(feature = "compat-json")]
use serde_json::Value;

#[cfg(all(feature = "compat-json", any(test, feature = "editor-facts")))]
use crate::ManagedSemanticJson;
#[cfg(all(feature = "compat-json", any(test, feature = "editor-facts")))]
use crate::{OperationControl, OperationControlResult};
#[cfg(feature = "compat-json")]
use crate::{Result, SourcePresentationMetadata};

#[cfg(all(test, feature = "compat-json"))]
pub(super) use merman_model::families::state::compat::test_support::{
    projected_document_count, reset_projected_document_count,
};

pub(crate) fn sanitize_common_db_fields(
    model: &mut StateDiagramRenderModel,
    config: &crate::MermaidConfig,
) {
    crate::common_db::sanitize_optional_acc_title(&mut model.acc_title, config);
    crate::common_db::sanitize_optional_acc_descr(&mut model.acc_descr, config);
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &StateDiagramRenderModel,
    meta: &SourcePresentationMetadata,
) -> Result<Value> {
    merman_model::families::state::compat::project(model, &meta.compat_json_metadata())
        .map_err(Into::into)
}

#[cfg(all(feature = "compat-json", any(test, feature = "editor-facts")))]
pub(crate) fn render_model_to_compat_json_controlled(
    model: &StateDiagramRenderModel,
    meta: &SourcePresentationMetadata,
    control: &OperationControl,
) -> OperationControlResult<Result<ManagedSemanticJson>> {
    Ok(merman_model::families::state::compat::project_controlled(
        model,
        &meta.compat_json_metadata(),
        control,
    )?
    .map_err(Into::into))
}
