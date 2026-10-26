use crate::Error;
#[cfg(feature = "presentation")]
use crate::{
    PresentationMetadata, Result,
    diagram::{ParsedDiagramRender, RenderSemanticModel},
};
#[cfg(feature = "compat-json")]
use crate::{common_db, diagram::ParsedDiagram};
use serde::{Deserialize, Serialize};
#[cfg(feature = "compat-json")]
use serde_json::{Value, json};

/// Typed semantic model for Merman's built-in suppressed-error diagram.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorDiagramModel {
    #[serde(rename = "type")]
    pub diagram_type: String,
    #[serde(
        default,
        rename = "errorMessage",
        skip_serializing_if = "Option::is_none"
    )]
    pub error_message: Option<String>,
}

impl ErrorDiagramModel {
    pub(crate) fn new(diagram_type: &str) -> Self {
        Self {
            diagram_type: diagram_type.to_string(),
            error_message: None,
        }
    }

    pub(crate) fn suppressed(error: &Error) -> Self {
        Self {
            diagram_type: "error".to_string(),
            error_message: Some(error.to_string()),
        }
    }

    #[cfg(feature = "compat-json")]
    fn compatibility_json(&self) -> Value {
        let mut model = json!({ "type": self.diagram_type });
        if let Some(message) = &self.error_message {
            model["errorMessage"] = json!(message);
        }
        model
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
    _meta: &PresentationMetadata,
) -> Result<Value> {
    Ok(model.compatibility_json())
}

#[cfg(feature = "compat-json")]
pub(crate) fn parse_error(_code: &str, meta: &PresentationMetadata) -> Result<Value> {
    render_model_to_compat_json(&ErrorDiagramModel::new(&meta.diagram_type), meta)
}

#[cfg(feature = "presentation")]
pub(crate) fn parse_error_model_for_render(
    _code: &str,
    meta: &PresentationMetadata,
) -> Result<ErrorDiagramModel> {
    Ok(ErrorDiagramModel::new(&meta.diagram_type))
}

#[cfg(feature = "compat-json")]
pub(crate) fn suppressed_error_diagram(
    source_meta: &PresentationMetadata,
    error: &Error,
) -> ParsedDiagram {
    let meta = suppressed_error_metadata(source_meta);
    let typed = ErrorDiagramModel::suppressed(error);
    let mut model = render_model_to_compat_json(&typed, &meta)
        .expect("Error typed model must remain JSON-serializable");
    common_db::apply_common_db_sanitization(&mut model, meta.effective_config.as_config());
    ParsedDiagram {
        meta,
        model: model.into(),
    }
}

#[cfg(feature = "presentation")]
pub(crate) fn suppressed_error_render_diagram(
    source_meta: &PresentationMetadata,
    error: &Error,
) -> ParsedDiagramRender {
    let meta = suppressed_error_metadata(source_meta);
    let model = ErrorDiagramModel::suppressed(error);
    ParsedDiagramRender::new(meta, RenderSemanticModel::Error(model))
}

#[cfg(feature = "presentation")]
fn suppressed_error_metadata(source_meta: &PresentationMetadata) -> PresentationMetadata {
    let mut meta = source_meta.clone();
    meta.diagram_type = "error".to_string();
    meta
}

#[cfg(all(test, feature = "compat-json"))]
mod tests {
    use super::*;

    #[test]
    fn typed_error_model_projects_exact_compatibility_json() {
        let meta = PresentationMetadata {
            diagram_type: "error".to_string(),
            config: crate::MermaidConfig::default(),
            effective_config: crate::presentation::PresentationConfig::default(),
            title: None,
        };
        let compat = parse_error("", &meta).unwrap();
        let typed = parse_error_model_for_render("", &meta).unwrap();

        assert_eq!(render_model_to_compat_json(&typed, &meta).unwrap(), compat);
    }
}
