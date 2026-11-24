use crate::environment::RenderSession;
use crate::{Error, LayoutAdaptation, LayoutOptions, RenderCapability, Result};
use merman_model::OperationPhase;
use merman_model::families as diagrams;
use merman_model::family::DiagramFamilyId;
use merman_model::presentation::{PresentationDiagram, RenderSemanticModel};

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

    /// Placement modes supported by the selected family and backend.
    pub fn supported_layout_adaptations(&self) -> &'static [LayoutAdaptation] {
        supported_layout_adaptations(
            self.family_id,
            self.required.contains(&RenderCapability::LayoutShoal),
        )
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
fn sequence_requires_math(model: &diagrams::sequence::SequenceDiagramModel) -> bool {
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

fn parsed_render_requires_math(parsed: &PresentationDiagram) -> bool {
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
        RenderSemanticModel::Flowchart(model) => parsed
            .render_context()
            .flowchart_render_context()
            .map_or_else(
                || semantic_flowchart_requires_math(model),
                |render_context| {
                    crate::flowchart::FlowchartRenderModelRef::new(model, render_context)
                        .requires_math()
                },
            ),
        #[cfg(feature = "diagram-mindmap")]
        RenderSemanticModel::Mindmap(model) => {
            crate::config::config_effective_html_labels(
                parsed.metadata().effective_config.as_value(),
            ) && mindmap_requires_math(model)
        }
        #[cfg(feature = "diagram-sequence")]
        RenderSemanticModel::Sequence(model) => sequence_requires_math(model),
        _ => false,
    }
}

fn required_layout_capability(
    parsed: &PresentationDiagram,
    family: DiagramFamilyId,
) -> Result<Option<RenderCapability>> {
    let meta = parsed.metadata();
    match family {
        DiagramFamilyId::ARCHITECTURE => return Ok(Some(RenderCapability::LayoutCytoscape)),
        DiagramFamilyId::AGENTFLOW
        | DiagramFamilyId::CLASS
        | DiagramFamilyId::ER
        | DiagramFamilyId::FLOWCHART
        | DiagramFamilyId::MINDMAP
        | DiagramFamilyId::REQUIREMENT
        | DiagramFamilyId::STATE
        | DiagramFamilyId::SWIMLANE
        | DiagramFamilyId::USECASE => {}
        _ => return Ok(None),
    }

    let selection = crate::layout_backend::resolve_graph_layout(&meta.effective_config);
    if selection.backend == crate::layout_backend::GraphLayoutBackend::Shoal
        && !matches!(
            family,
            DiagramFamilyId::AGENTFLOW
                | DiagramFamilyId::CLASS
                | DiagramFamilyId::ER
                | DiagramFamilyId::REQUIREMENT
                | DiagramFamilyId::STATE
                | DiagramFamilyId::USECASE
        )
        && !(family == DiagramFamilyId::FLOWCHART
            && matches!(meta.diagram_type.as_str(), "flowchart" | "flowchart-v2"))
    {
        return Err(Error::UnsupportedLayoutBackend {
            backend: "shoal",
            diagram_type: meta.diagram_type.clone(),
        });
    }
    #[cfg(feature = "diagram-mindmap")]
    if family == DiagramFamilyId::MINDMAP {
        return Ok(crate::mindmap::required_layout_capability(
            &meta.effective_config,
        ));
    }
    Ok(selection.required_capability())
}

/// Reports whether this renderer contains the logical family handler for a known diagram type.
/// Optional layout and math capabilities are admitted separately during planning.
#[allow(
    clippy::match_like_matches_macro,
    reason = "Each arm has a distinct feature condition that only coincides in all-family builds."
)]
pub fn supports_diagram_type(diagram_type: &str) -> bool {
    match merman_model::family::diagram_type_family_kind(diagram_type) {
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

fn validate_paired_render_model(parsed: &PresentationDiagram) -> Result<DiagramFamilyId> {
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

    if !model.matches_diagram_type(diagram_type) {
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
    parsed.family_id().ok_or_else(|| Error::InvalidModel {
        message: format!(
            "render model variant {} has no built-in family for diagram type: {diagram_type}",
            model.kind()
        ),
    })
}

fn supported_layout_adaptations(
    family: DiagramFamilyId,
    shoal: bool,
) -> &'static [LayoutAdaptation] {
    if family == DiagramFamilyId::KANBAN || shoal {
        &[LayoutAdaptation::None, LayoutAdaptation::FitWidth]
    } else {
        &[LayoutAdaptation::None]
    }
}

/// Admits explicit placement requests without starting layout or installing render services.
#[doc(hidden)]
pub fn validate_layout_options(
    parsed: &PresentationDiagram,
    options: &LayoutOptions,
) -> Result<()> {
    resolve_layout_adaptation(parsed, options).map(|_| ())
}

pub(super) fn resolve_layout_adaptation(
    parsed: &PresentationDiagram,
    options: &LayoutOptions,
) -> Result<LayoutAdaptation> {
    if options.adaptation == LayoutAdaptation::None {
        return Ok(LayoutAdaptation::None);
    }
    let family = validate_paired_render_model(parsed)?;
    let shoal = required_layout_capability(parsed, family)? == Some(RenderCapability::LayoutShoal);
    if !supported_layout_adaptations(family, shoal).contains(&options.adaptation) {
        if options.adaptation_if_supported {
            return Ok(LayoutAdaptation::None);
        }
        return Err(Error::UnsupportedLayoutAdaptation {
            adaptation: options.adaptation.id(),
            diagram_type: parsed.metadata().diagram_type.clone(),
        });
    }
    if !options.container_width.is_finite() || options.container_width <= 0.0 {
        return Err(Error::InvalidAdaptationWidth);
    }
    Ok(options.adaptation)
}

/// Identifies the existing edge-geometry projection after closed typed model admission.
#[doc(hidden)]
pub fn has_edge_geometry(parsed: &PresentationDiagram) -> Result<bool> {
    let family = validate_paired_render_model(parsed)?;
    Ok(matches!(
        family,
        DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE | DiagramFamilyId::AGENTFLOW
    ))
}

#[cfg(all(
    test,
    any(feature = "diagram-requirement", feature = "diagram-usecase")
))]
#[test]
fn shoal_admission_for_requirement_and_usecase_is_typed() {
    use crate::RenderCapabilityPolicy;
    use crate::environment::RenderEnvironment;
    use merman_core::{MermaidEngine, ParseOptions};

    let sources = [
        #[cfg(feature = "diagram-requirement")]
        "requirementDiagram\nrequirement r {\nid: 1\ntext: Demo\nrisk: low\nverifymethod: test\n}",
        #[cfg(feature = "diagram-usecase")]
        "usecase-beta\nactor User\nA[Sign in]\nUser --> A",
    ];
    let engine = MermaidEngine::new();
    for source in sources {
        let source = format!("---\nconfig:\n  layout: shoal\n---\n{source}");
        let parsed = engine
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .unwrap()
            .unwrap()
            .into_presentation();
        for (policy, ready) in [
            (
                RenderCapabilityPolicy::unrestricted(),
                cfg!(feature = "layout-shoal"),
            ),
            (RenderCapabilityPolicy::deny_all(), false),
        ] {
            let session = RenderEnvironment::deterministic()
                .with_capability_policy(policy)
                .begin_session()
                .unwrap();
            let plan = super::plan_render(&parsed, &session).unwrap();
            assert_eq!(
                plan.required_capabilities(),
                [RenderCapability::LayoutShoal]
            );
            assert_eq!(plan.is_ready(), ready);
            let result = super::prepare(&parsed, &LayoutOptions::default(), session);
            if ready {
                result.unwrap();
            } else {
                assert!(matches!(
                    result,
                    Err(Error::MissingCapability {
                        capability: RenderCapability::LayoutShoal,
                        ..
                    })
                ));
            }
        }
    }
}

fn validate_render_input(
    parsed: &PresentationDiagram,
    session: &RenderSession,
) -> Result<DiagramFamilyId> {
    let family = validate_paired_render_model(parsed)?;
    let meta = parsed.metadata();
    if !meta
        .effective_config
        .theme_compatibility()
        .matches_recipe(session.theme_compatibility_recipe())
    {
        return Err(Error::ThemeParseBindingMismatch);
    }
    session
        .work_meter()
        .preflight_parsed_render(parsed, OperationPhase::Layout)?;
    Ok(family)
}

/// Plans capability admission for a canonically paired typed render model without running layout.
pub fn plan_render(
    parsed: &PresentationDiagram,
    session: &RenderSession,
) -> Result<RenderCapabilityPlan> {
    session.checkpoint(OperationPhase::Layout)?;
    let meta = parsed.metadata();
    let family_id = validate_render_input(parsed, session)?;
    let mut required = Vec::with_capacity(2);
    required.extend(required_layout_capability(parsed, family_id)?);
    if parsed_render_requires_math(parsed) {
        required.push(RenderCapability::Math);
    }
    let missing = required
        .iter()
        .copied()
        .filter(|capability| !session.supports_capability(*capability))
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
