use crate::environment::RenderSession;
use crate::{Error, RenderCapability, Result};
use merman_core::OperationPhase;
use merman_core::diagrams;
use merman_core::{DiagramFamilyId, ParsedDiagramRender, RenderSemanticModel};

/// Capabilities required by one parsed typed render operation before layout starts.
///
/// Requirements come from the canonically paired semantic model and effective Mermaid config;
/// availability comes from the compiled layout backends and the operation's render session.
#[must_use]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderCapabilityPlan {
    diagram_type: String,
    family_id: DiagramFamilyId,
    required: Vec<RenderCapability>,
    missing: Vec<RenderCapability>,
}

impl RenderCapabilityPlan {
    /// Returns the detected Mermaid diagram type used by render dispatch.
    pub fn diagram_type(&self) -> &str {
        &self.diagram_type
    }

    /// Returns the authoritative typed render family selected before layout starts.
    pub const fn family_id(&self) -> DiagramFamilyId {
        self.family_id
    }

    /// Returns every optional capability this operation requires.
    pub fn required_capabilities(&self) -> &[RenderCapability] {
        &self.required
    }

    /// Returns the required capabilities unavailable in the planned render session.
    pub fn missing_capabilities(&self) -> &[RenderCapability] {
        &self.missing
    }

    /// Iterates over stable semantic IDs for every required capability.
    pub fn required_capability_ids(&self) -> impl ExactSizeIterator<Item = &'static str> + '_ {
        self.required.iter().copied().map(RenderCapability::id)
    }

    /// Iterates over stable semantic IDs for every missing capability.
    pub fn missing_capability_ids(&self) -> impl ExactSizeIterator<Item = &'static str> + '_ {
        self.missing.iter().copied().map(RenderCapability::id)
    }

    /// Reports whether the planned render session satisfies every requirement.
    pub fn is_ready(&self) -> bool {
        self.missing.is_empty()
    }

    pub(super) fn ensure_available(&self) -> Result<()> {
        let Some(capability) = self.missing.first().copied() else {
            return Ok(());
        };
        Err(Error::MissingCapability {
            capability,
            diagram_type: self.diagram_type.clone(),
        })
    }
}

fn semantic_flowchart_requires_math(model: &diagrams::flowchart::FlowchartModel) -> bool {
    model
        .nodes
        .iter()
        .filter_map(|node| node.label.as_deref())
        .chain(model.edges.iter().filter_map(|edge| edge.label.as_deref()))
        .chain(
            model
                .subgraphs
                .iter()
                .map(|subgraph| subgraph.title.as_str()),
        )
        .any(crate::math::contains_delimited_math)
}

fn sequence_requires_math(model: &diagrams::sequence::SequenceDiagramRenderModel) -> bool {
    model
        .actors
        .values()
        .map(|actor| actor.description.as_str())
        .chain(model.messages.iter().map(|message| message.message_text()))
        .chain(model.notes.iter().map(|note| note.message.as_str()))
        .any(crate::math::contains_delimited_math)
}

fn mindmap_requires_math(model: &diagrams::mindmap::MindmapDiagramRenderModel) -> bool {
    model
        .nodes
        .iter()
        .map(|node| node.label.as_str())
        .any(crate::math::contains_delimited_math)
}

fn parsed_render_requires_math(parsed: &ParsedDiagramRender) -> bool {
    match parsed.model() {
        RenderSemanticModel::Class(model) => crate::class::class_requires_math(model),
        RenderSemanticModel::Flowchart(model) => {
            parsed.flowchart_render_label_sources().map_or_else(
                || semantic_flowchart_requires_math(model),
                |label_sources| {
                    crate::flowchart::FlowchartRenderModelRef::new(model, label_sources)
                        .requires_math()
                },
            )
        }
        RenderSemanticModel::Mindmap(model) => mindmap_requires_math(model),
        RenderSemanticModel::Sequence(model) => sequence_requires_math(model),
        _ => false,
    }
}

fn capability_is_available(capability: RenderCapability, session: &RenderSession) -> bool {
    session.supports_capability(capability)
}

fn required_capabilities(parsed: &ParsedDiagramRender) -> Vec<RenderCapability> {
    let mut required = Vec::with_capacity(2);
    let meta = parsed.metadata();
    let model = parsed.model();
    let effective_config = &meta.effective_config;
    match model {
        RenderSemanticModel::Architecture(_) => {
            required.push(RenderCapability::LayoutCytoscape);
        }
        RenderSemanticModel::Mindmap(_)
            if !crate::mindmap::uses_tidy_tree_layout(effective_config.as_value()) =>
        {
            required.push(RenderCapability::LayoutCytoscape);
        }
        RenderSemanticModel::Flowchart(_) | RenderSemanticModel::Class(_)
            if crate::uses_elk_layout(effective_config) =>
        {
            required.push(RenderCapability::LayoutElk);
        }
        RenderSemanticModel::Er(_) if crate::er::uses_elk_layout(effective_config.as_value()) => {
            required.push(RenderCapability::LayoutElk);
        }
        _ => {}
    }

    if parsed_render_requires_math(parsed) {
        required.push(RenderCapability::Math);
    }
    required
}

fn validate_render_input(
    parsed: &ParsedDiagramRender,
    session: &RenderSession,
) -> Result<DiagramFamilyId> {
    let meta = parsed.metadata();
    let model = parsed.model();
    if !merman_core::__private::theme_parse_evidence(meta)
        .matches_recipe(session.theme_compatibility_recipe())
    {
        return Err(Error::ThemeParseBindingMismatch);
    }
    let diagram_type = meta.diagram_type.as_str();
    if let RenderSemanticModel::CustomJson(custom) = model {
        return Err(Error::NonRenderableCustomModel {
            diagram_type: meta.diagram_type.clone(),
            model_name: custom.model_name().to_string(),
            provenance: custom.provenance(),
        });
    }

    if !model.supports_diagram_type(diagram_type) {
        return Err(Error::InvalidModel {
            message: format!(
                "unexpected render model variant {} for diagram type: {diagram_type}",
                model.kind()
            ),
        });
    }

    session.resource_policy().check_parsed_render(parsed)?;
    parsed.family_id().ok_or_else(|| Error::InvalidModel {
        message: format!(
            "render model variant {} has no built-in family for diagram type: {diagram_type}",
            model.kind()
        ),
    })
}

/// Plans capability admission for a canonically paired typed render model without running layout.
pub fn plan_render(
    parsed: &ParsedDiagramRender,
    session: &RenderSession,
) -> Result<RenderCapabilityPlan> {
    session.checkpoint(OperationPhase::Layout)?;
    let meta = parsed.metadata();
    let family_id = validate_render_input(parsed, session)?;
    let required = required_capabilities(parsed);
    let missing = required
        .iter()
        .copied()
        .filter(|capability| !capability_is_available(*capability, session))
        .collect();
    let plan = RenderCapabilityPlan {
        diagram_type: meta.diagram_type.clone(),
        family_id,
        required,
        missing,
    };
    session.checkpoint(OperationPhase::Layout)?;
    Ok(plan)
}
