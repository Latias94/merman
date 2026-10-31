//! Checked immutable presentation input, independent of source construction.

use crate::family::{DiagramFamilyId, diagram_type_family_id};
use crate::resources::{InputResourceLimitExceeded, InputResourcePolicy};
use crate::{MermaidConfig, OperationControl, OperationControlResult};

use super::{
    PresentationConfig, PresentationConfigError, PresentationContext, PresentationMetadata,
    RenderSemanticModel, ThemeCompatibilityRecipe,
};

/// Caller-authored configuration and identity for an ordinary model-to-output operation.
///
/// All provided values are caller-owned. This route declares no source-derived compatibility
/// residuals or palette authority, including when values equal a selected theme recipe.
#[derive(Debug, Clone)]
pub struct AuthoredPresentationMetadata {
    pub diagram_type: String,
    pub config: MermaidConfig,
    pub effective_config: MermaidConfig,
    pub title: Option<String>,
    pub theme_recipe: Option<ThemeCompatibilityRecipe>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PresentationDataError {
    #[error(transparent)]
    Config(#[from] PresentationConfigError),
    #[error(transparent)]
    ResourceLimit(#[from] InputResourceLimitExceeded),
    #[error("model `{model_kind}` does not represent diagram type `{diagram_type}`")]
    ModelKind {
        model_kind: &'static str,
        diagram_type: String,
    },
    #[error("{context_kind} output facts cannot be paired with a {model_kind} model")]
    ContextKind {
        context_kind: &'static str,
        model_kind: &'static str,
    },
    #[error("{family} presentation context is invalid: {message}")]
    FamilyContext {
        family: &'static str,
        message: &'static str,
    },
    #[error("{paint_family} paint facts cannot be paired with a {model_kind} model")]
    PaintFamily {
        paint_family: &'static str,
        model_kind: &'static str,
    },
    #[error("{family} presentation data requires a nonempty declared output identity")]
    MissingOutputIdentity { family: &'static str },
}

/// Immutable full model, metadata, resolved output family, and paired context declarations.
///
/// This type validates data relationships. It does not authenticate its producer or certify that
/// source preprocessing, parsing, or configuration admission ran. Core's closed parsed-result
/// wrapper retains that distinct guarantee. Output owners still perform their actual admission.
#[derive(Debug, Clone)]
pub struct PresentationDiagram {
    metadata: PresentationMetadata,
    model: RenderSemanticModel,
    context: PresentationContext,
    family_id: Option<DiagramFamilyId>,
}

impl PresentationDiagram {
    /// Constructs an ordinary caller-authored model with no inferred source ownership.
    pub fn try_new(
        metadata: AuthoredPresentationMetadata,
        model: RenderSemanticModel,
        context: PresentationContext,
        construction_policy: &InputResourcePolicy,
        control: &OperationControl,
    ) -> OperationControlResult<Result<Self, PresentationDataError>> {
        control.checkpoint()?;
        let config = match PresentationConfig::from_config_controlled(
            metadata.effective_config,
            construction_policy,
            control,
        )? {
            Ok(config) => config,
            Err(error) => return Ok(Err(error.into())),
        };
        let effective_config = if let Some(recipe) = metadata.theme_recipe {
            let parts = config
                .frozen_parts()
                .with_theme_compatibility(Some(recipe), std::sync::Arc::from([]));
            match PresentationConfig::try_from_frozen_parts_controlled(
                parts,
                construction_policy,
                control,
            )? {
                Ok(config) => config,
                Err(error) => return Ok(Err(error.into())),
            }
        } else {
            config
        };
        Self::try_from_frozen(
            PresentationMetadata {
                diagram_type: metadata.diagram_type,
                config: metadata.config,
                effective_config,
                title: metadata.title,
            },
            model,
            context,
            construction_policy,
            control,
        )
    }

    /// Checks explicitly declared frozen data without reconstructing a producer's history.
    ///
    /// Construction budgets bound validation scratch before family indexes are allocated. They
    /// are independent of later output admission. A source producer with already-admitted data
    /// may select an unbounded construction policy without granting any source authority here.
    pub fn try_from_frozen(
        metadata: PresentationMetadata,
        model: RenderSemanticModel,
        context: PresentationContext,
        construction_policy: &InputResourcePolicy,
        control: &OperationControl,
    ) -> OperationControlResult<Result<Self, PresentationDataError>> {
        control.checkpoint()?;
        let custom = matches!(model, RenderSemanticModel::CustomJson(_));
        if !custom && !model.matches_diagram_type(&metadata.diagram_type) {
            return Ok(Err(PresentationDataError::ModelKind {
                model_kind: model.kind(),
                diagram_type: metadata.diagram_type,
            }));
        }
        if let Some(context_kind) = context.model_kind()
            && context_kind != model.kind()
        {
            return Ok(Err(PresentationDataError::ContextKind {
                context_kind,
                model_kind: model.kind(),
            }));
        }
        if let Some(inputs) = metadata.effective_config.family_paint_inputs()
            && inputs.render_model_kind() != model.kind()
        {
            return Ok(Err(PresentationDataError::PaintFamily {
                paint_family: inputs.family_id().as_str(),
                model_kind: model.kind(),
            }));
        }
        if let Err(error) = crate::resources::check_presentation_construction(
            &model,
            &context,
            construction_policy,
            control,
        )? {
            return Ok(Err(error.into()));
        }
        control.checkpoint()?;
        if let Err(error) = validate_family_context(&model, &context, control)? {
            return Ok(Err(error));
        }
        let family_id = if custom {
            None
        } else {
            diagram_type_family_id(&metadata.diagram_type).map(|family| {
                if matches!(
                    family,
                    DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE
                ) {
                    if metadata.effective_config.get_str("layout") == Some("swimlane") {
                        DiagramFamilyId::SWIMLANE
                    } else {
                        DiagramFamilyId::FLOWCHART
                    }
                } else {
                    family
                }
            })
        };
        control.checkpoint()?;
        Ok(Ok(Self {
            metadata,
            model,
            context,
            family_id,
        }))
    }

    pub fn metadata(&self) -> &PresentationMetadata {
        &self.metadata
    }

    pub fn model(&self) -> &RenderSemanticModel {
        &self.model
    }

    pub fn render_context(&self) -> &PresentationContext {
        &self.context
    }

    pub const fn family_id(&self) -> Option<DiagramFamilyId> {
        self.family_id
    }

    pub fn retained_render_context_bytes(&self) -> usize {
        self.context.retained_text_bytes()
    }

    pub fn into_parts(
        self,
    ) -> (
        PresentationMetadata,
        RenderSemanticModel,
        PresentationContext,
    ) {
        (self.metadata, self.model, self.context)
    }
}

fn validate_family_context(
    model: &RenderSemanticModel,
    context: &PresentationContext,
    control: &OperationControl,
) -> OperationControlResult<Result<(), PresentationDataError>> {
    control.checkpoint()?;
    match (model, context) {
        #[cfg(feature = "diagram-mindmap")]
        (RenderSemanticModel::Mindmap(model), context)
            if !model.nodes.is_empty()
                && context.mindmap_diagram_id().is_none_or(str::is_empty) =>
        {
            return Ok(Err(PresentationDataError::MissingOutputIdentity {
                family: "mindmap",
            }));
        }
        #[cfg(any(
            feature = "diagram-flowchart",
            feature = "diagram-swimlane",
            feature = "diagram-agentflow"
        ))]
        (RenderSemanticModel::Flowchart(model), context) => {
            let validation = match context.flowchart_render_context() {
                Some(context) => context.validate_for_model_controlled(model, control)?,
                None => model
                    .subgraph_vertex_styles
                    .validate_for_model_controlled(model, control)?,
            };
            if let Err(message) = validation {
                return Ok(Err(PresentationDataError::FamilyContext {
                    family: "flowchart",
                    message,
                }));
            }
        }
        #[cfg(feature = "diagram-class")]
        (RenderSemanticModel::Class(model), PresentationContext::Class(facts)) => {
            if let Err(message) = facts.validate_for_model_controlled(model, control)? {
                return Ok(Err(PresentationDataError::FamilyContext {
                    family: "class",
                    message,
                }));
            }
        }
        _ => {}
    }
    control.checkpoint()?;
    Ok(Ok(()))
}
