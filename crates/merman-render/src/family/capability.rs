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

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
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

#[cfg(feature = "diagram-sequence")]
fn sequence_requires_math(model: &diagrams::sequence::SequenceDiagramRenderModel) -> bool {
    model
        .actors
        .values()
        .map(|actor| actor.description.as_str())
        .chain(model.messages.iter().map(|message| message.message_text()))
        .chain(model.notes.iter().map(|note| note.message.as_str()))
        .any(crate::math::contains_delimited_math)
}

#[cfg(feature = "diagram-mindmap")]
fn mindmap_requires_math(model: &diagrams::mindmap::MindmapDiagramRenderModel) -> bool {
    model
        .nodes
        .iter()
        .map(|node| node.label.as_str())
        .any(crate::math::contains_delimited_math)
}

fn parsed_render_requires_math(parsed: &ParsedDiagramRender) -> bool {
    match parsed.model() {
        #[cfg(feature = "diagram-usecase")]
        RenderSemanticModel::Usecase(model) => {
            crate::usecase::requires_math(model, &parsed.metadata().effective_config)
        }
        #[cfg(feature = "diagram-agentflow")]
        RenderSemanticModel::Agentflow(model) => model
            .vertices
            .iter()
            .filter_map(|node| node.label.as_deref())
            .chain(model.edges.iter().filter_map(|edge| edge.label.as_deref()))
            .chain(
                model
                    .sub_graphs
                    .iter()
                    .filter_map(|graph| graph.title.as_deref()),
            )
            .chain(
                model
                    .connectors
                    .iter()
                    .filter_map(|connector| connector.title.as_deref()),
            )
            .any(crate::math::contains_delimited_math),
        #[cfg(feature = "diagram-class")]
        RenderSemanticModel::Class(model) => crate::class::class_requires_math(model),
        #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
        RenderSemanticModel::Flowchart(model) => parsed.flowchart_render_context().map_or_else(
            || semantic_flowchart_requires_math(model),
            |render_context| {
                crate::flowchart::FlowchartRenderModelRef::new(model, render_context)
                    .requires_math()
            },
        ),
        #[cfg(feature = "diagram-mindmap")]
        RenderSemanticModel::Mindmap(model) => mindmap_requires_math(model),
        #[cfg(feature = "diagram-sequence")]
        RenderSemanticModel::Sequence(model) => sequence_requires_math(model),
        _ => false,
    }
}

fn capability_is_available(capability: RenderCapability, session: &RenderSession) -> bool {
    session.supports_capability(capability)
}

#[allow(
    unused_variables,
    reason = "Dispatch inputs depend on the selected diagram families."
)]
fn required_capabilities(parsed: &ParsedDiagramRender) -> Vec<RenderCapability> {
    let mut required = Vec::with_capacity(2);
    let meta = parsed.metadata();
    let model = parsed.model();
    let effective_config = &meta.effective_config;
    match model {
        #[cfg(feature = "diagram-architecture")]
        RenderSemanticModel::Architecture(_) => {
            required.push(RenderCapability::LayoutCytoscape);
        }
        #[cfg(feature = "diagram-mindmap")]
        RenderSemanticModel::Mindmap(_) => {
            if let Some(capability) = crate::mindmap::required_layout_capability(effective_config) {
                required.push(capability);
            }
        }
        #[cfg(feature = "diagram-agentflow")]
        RenderSemanticModel::Agentflow(_) => {
            required.extend(
                crate::layout_backend::resolve_graph_layout(effective_config.as_value())
                    .required_capability(),
            );
        }
        #[cfg(feature = "diagram-usecase")]
        RenderSemanticModel::Usecase(_) => {
            required.extend(
                crate::layout_backend::resolve_graph_layout(effective_config.as_value())
                    .required_capability(),
            );
        }
        #[cfg(feature = "diagram-state")]
        RenderSemanticModel::State(_) => {
            required.extend(
                crate::layout_backend::resolve_graph_layout(effective_config.as_value())
                    .required_capability(),
            );
        }
        #[cfg(feature = "diagram-requirement")]
        RenderSemanticModel::Requirement(_) => {
            required.extend(
                crate::layout_backend::resolve_graph_layout(effective_config.as_value())
                    .required_capability(),
            );
        }
        #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
        RenderSemanticModel::Flowchart(_) => {
            required.extend(
                crate::layout_backend::resolve_graph_layout(effective_config.as_value())
                    .required_capability(),
            );
        }
        #[cfg(feature = "diagram-class")]
        RenderSemanticModel::Class(_) => {
            required.extend(
                crate::layout_backend::resolve_graph_layout(effective_config.as_value())
                    .required_capability(),
            );
        }
        #[cfg(feature = "diagram-er")]
        RenderSemanticModel::Er(_) => {
            required.extend(
                crate::layout_backend::resolve_graph_layout(effective_config.as_value())
                    .required_capability(),
            );
        }
        _ => {}
    }

    if parsed_render_requires_math(parsed) {
        required.push(RenderCapability::Math);
    }
    required
}

/// Reports whether this renderer contains the logical family handler for a known diagram type.
/// Optional layout and math capabilities are admitted separately during planning.
#[allow(
    clippy::match_like_matches_macro,
    reason = "Each arm has a distinct feature condition that only coincides in all-family builds."
)]
pub fn supports_diagram_type(diagram_type: &str) -> bool {
    match merman_core::diagram_type_family_kind(diagram_type) {
        Some("error") => true,
        Some("agentflow") => cfg!(feature = "diagram-agentflow"),
        Some("usecase") => cfg!(feature = "diagram-usecase"),
        Some("flowchart") => cfg!(feature = "diagram-flowchart"),
        Some("swimlane") => cfg!(feature = "diagram-swimlane"),
        Some("mindmap") => cfg!(feature = "diagram-mindmap"),
        Some("architecture") => cfg!(feature = "diagram-architecture"),
        Some("zenuml") => cfg!(feature = "diagram-zenuml"),
        Some("sequence") => cfg!(feature = "diagram-sequence"),
        Some("c4") => cfg!(feature = "diagram-c4"),
        Some("kanban") => cfg!(feature = "diagram-kanban"),
        Some("class") => cfg!(feature = "diagram-class"),
        Some("er") => cfg!(feature = "diagram-er"),
        Some("gantt") => cfg!(feature = "diagram-gantt"),
        Some("info") => cfg!(feature = "diagram-info"),
        Some("pie") => cfg!(feature = "diagram-pie"),
        Some("requirement") => cfg!(feature = "diagram-requirement"),
        Some("timeline") => cfg!(feature = "diagram-timeline"),
        Some("gitGraph") => cfg!(feature = "diagram-git-graph"),
        Some("state") => cfg!(feature = "diagram-state"),
        Some("journey") => cfg!(feature = "diagram-journey"),
        Some("quadrantChart") => cfg!(feature = "diagram-quadrant-chart"),
        Some("sankey") => cfg!(feature = "diagram-sankey"),
        Some("packet") => cfg!(feature = "diagram-packet"),
        Some("xychart") => cfg!(feature = "diagram-xychart"),
        Some("block") => cfg!(feature = "diagram-block"),
        Some("eventmodeling") => cfg!(feature = "diagram-event-modeling"),
        Some("treeView") => cfg!(feature = "diagram-tree-view"),
        Some("radar") => cfg!(feature = "diagram-radar"),
        Some("ishikawa") => cfg!(feature = "diagram-ishikawa"),
        Some("treemap") => cfg!(feature = "diagram-treemap"),
        Some("railroad") => cfg!(feature = "diagram-railroad"),
        Some("venn") => cfg!(feature = "diagram-venn"),
        Some("wardley") => cfg!(feature = "diagram-wardley"),
        Some("cynefin") => cfg!(feature = "diagram-cynefin"),
        _ => false,
    }
}

fn validate_render_input(
    parsed: &ParsedDiagramRender,
    session: &RenderSession,
) -> Result<DiagramFamilyId> {
    let meta = parsed.metadata();
    let model = parsed.model();
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

    if !supports_diagram_type(diagram_type) {
        return Err(Error::UnsupportedDiagram {
            diagram_type: diagram_type.to_owned(),
        });
    }
    if !merman_core::__private::theme_parse_evidence(meta)
        .matches_recipe(session.theme_compatibility_recipe())
    {
        return Err(Error::ThemeParseBindingMismatch);
    }
    session
        .work_meter()
        .preflight_parsed_render(parsed, OperationPhase::Layout)?;
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
