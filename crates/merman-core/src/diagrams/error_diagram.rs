use crate::Error;
#[cfg(feature = "presentation")]
use crate::{Result, SourcePresentationMetadata, diagram::ParsedDiagramRender};
#[cfg(feature = "compat-json")]
use crate::{common_db, diagram::ParsedDiagram};
#[cfg(feature = "presentation")]
use merman_model::presentation::RenderSemanticModel;
#[cfg(feature = "compat-json")]
use serde_json::Value;

pub(crate) use merman_model::families::error::ErrorDiagramModel;

pub(crate) fn suppressed_error_model(error: &Error) -> ErrorDiagramModel {
    ErrorDiagramModel {
        diagram_type: "error".to_string(),
        error_message: Some(error.to_string()),
    }
}

pub(crate) fn canonical_error(
    _code: &str,
    metadata: &crate::SemanticMetadata,
    #[cfg(feature = "editor-facts")] _editor_demand: crate::family::EditorFactsDemand,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<crate::family::SemanticConstruction<crate::SemanticDiagramModel>>
{
    control.checkpoint()?;
    Ok(crate::family::SemanticConstruction {
        model: Ok(crate::SemanticDiagramModel::Error(ErrorDiagramModel::new(
            &metadata.diagram_type,
        ))),
        #[cfg(feature = "editor-facts")]
        editor_facts: None,
    })
}

#[cfg(feature = "compat-json")]
pub(crate) fn render_model_to_compat_json(
    model: &ErrorDiagramModel,
    meta: &SourcePresentationMetadata,
) -> Result<Value> {
    merman_model::families::error::compat::project(model, &meta.compat_json_metadata())
        .map_err(crate::Error::from)
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_error(_code: &str, meta: &SourcePresentationMetadata) -> Result<Value> {
    render_model_to_compat_json(&ErrorDiagramModel::new(&meta.diagram_type), meta)
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_error_model_for_render(
    _code: &str,
    meta: &SourcePresentationMetadata,
) -> Result<ErrorDiagramModel> {
    Ok(ErrorDiagramModel::new(&meta.diagram_type))
}

#[cfg(feature = "compat-json")]
pub(crate) fn suppressed_error_diagram(
    source_meta: &merman_model::presentation::PresentationMetadata,
    error: &Error,
) -> ParsedDiagram {
    let meta = suppressed_error_metadata(source_meta);
    let typed = suppressed_error_model(error);
    let mut model = merman_model::families::error::compat::project(
        &typed,
        &merman_model::presentation::compat::CompatJsonMetadata::from(&meta),
    )
    .expect("Error typed model must remain JSON-serializable");
    common_db::apply_common_db_sanitization(&mut model, meta.effective_config.as_config());
    ParsedDiagram {
        meta,
        model: model.into(),
    }
}

#[cfg(feature = "presentation")]
pub(crate) fn suppressed_error_render_diagram_controlled(
    source_meta: &merman_model::presentation::PresentationMetadata,
    error: &Error,
    control: &crate::OperationControl,
) -> crate::OperationControlResult<Result<ParsedDiagramRender>> {
    let meta = suppressed_error_metadata(source_meta);
    let model = suppressed_error_model(error);
    ParsedDiagramRender::from_parse_output_controlled(
        meta,
        crate::diagram::RenderSemanticParseOutput::new(RenderSemanticModel::Error(model)),
        control,
    )
}

#[cfg(feature = "presentation")]
fn suppressed_error_metadata(
    source_meta: &merman_model::presentation::PresentationMetadata,
) -> merman_model::presentation::PresentationMetadata {
    let mut meta = source_meta.clone();
    meta.diagram_type = "error".to_string();
    meta
}

#[cfg(all(test, feature = "compat-json"))]
mod tests {
    use super::*;

    #[test]
    fn typed_error_model_projects_exact_compatibility_json() {
        let meta = SourcePresentationMetadata {
            diagram_type: "error".to_string(),
            config: crate::MermaidConfig::default(),
            effective_config: crate::presentation::SourcePresentationConfig::default(),
            title: None,
        };
        let compat = parse_error("", &meta).unwrap();
        let typed = parse_error_model_for_render("", &meta).unwrap();

        assert_eq!(render_model_to_compat_json(&typed, &meta).unwrap(), compat);
    }
}
