#![forbid(unsafe_code)]
//! Terminal-friendly ASCII and Unicode rendering for Mermaid typed models.
//!
//! `merman-ascii` is deliberately model-driven: callers parse Mermaid text with `merman-core`, then
//! pass the resulting typed render model into this crate. The renderer does not own Mermaid syntax
//! parsing. Rendering requires the caller's operation control, runtime context, and resource
//! policy so this backend cannot create a second source-to-output operation.

#[cfg_attr(
    not(feature = "all-diagrams"),
    allow(
        dead_code,
        reason = "Shared terminal infrastructure has different users in each family subset"
    )
)]
mod canvas;
mod capability;
#[cfg(feature = "diagram-class")]
mod class;
mod color;
#[cfg(feature = "diagram-er")]
mod er;
mod error;
#[cfg(feature = "diagram-gantt")]
mod gantt;
#[cfg(feature = "diagram-git-graph")]
mod git_graph;
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-state"
))]
mod graph;
#[cfg(feature = "diagram-journey")]
mod journey;
#[cfg(feature = "diagram-kanban")]
mod kanban;
mod layout_selection;
#[cfg(feature = "diagram-mindmap")]
mod mindmap;
#[cfg_attr(
    not(feature = "all-diagrams"),
    allow(
        dead_code,
        reason = "Shared terminal infrastructure has different users in each family subset"
    )
)]
mod operation;
#[cfg_attr(
    not(feature = "all-diagrams"),
    allow(
        dead_code,
        reason = "Shared terminal infrastructure has different users in each family subset"
    )
)]
mod options;
mod output;
#[cfg(feature = "diagram-packet")]
mod packet;
#[cfg(any(feature = "diagram-class", feature = "diagram-er"))]
#[cfg_attr(
    not(all(feature = "diagram-class", feature = "diagram-er")),
    allow(
        dead_code,
        reason = "Class and ER use different operations of the shared relation layout"
    )
)]
mod relation_graph;
#[cfg_attr(
    not(feature = "all-diagrams"),
    allow(
        dead_code,
        reason = "Shared terminal infrastructure has different users in each family subset"
    )
)]
mod resource;
#[cfg_attr(
    not(feature = "all-diagrams"),
    allow(
        dead_code,
        reason = "Shared terminal infrastructure has different users in each family subset"
    )
)]
mod safe_text;
#[cfg(any(feature = "diagram-journey", feature = "diagram-timeline"))]
mod sectioned_text;
#[cfg(feature = "diagram-sequence")]
mod sequence;
#[cfg(feature = "diagram-state")]
mod state;
#[cfg_attr(
    not(feature = "all-diagrams"),
    allow(
        dead_code,
        reason = "Shared terminal infrastructure has different users in each family subset"
    )
)]
mod style_color;
#[cfg_attr(
    not(feature = "all-diagrams"),
    allow(
        dead_code,
        reason = "Shared terminal infrastructure has different users in each family subset"
    )
)]
mod terminal;
#[cfg_attr(
    not(feature = "all-diagrams"),
    allow(
        dead_code,
        reason = "Shared terminal infrastructure has different users in each family subset"
    )
)]
mod text;
#[cfg(feature = "diagram-timeline")]
mod timeline;
#[cfg(feature = "diagram-tree-view")]
mod tree_view;
#[cfg(feature = "diagram-xychart")]
mod xychart;

pub use capability::{
    AsciiCapability, AsciiCapabilityEvidence, AsciiEvidenceKind, AsciiPrimaryProjection,
    AsciiSemanticCoverage, AsciiSupportLevel, ascii_capabilities, ascii_diagrammatic_diagram_types,
    ascii_supported_diagram_types,
};
pub use color::{AsciiColorMode, AsciiColorRole, AsciiColorTheme, AsciiRgb, AsciiTerminalPalette};
pub use error::{AsciiError, Result};
pub use options::{
    AsciiCharset, AsciiDirection, AsciiLayoutProfile, AsciiRenderOptions, TerminalWidthProfile,
};
pub use output::{
    ASCII_OUTPUT_SCHEMA_VERSION, AsciiExtent, AsciiFallbackCapability, AsciiFallbackReason,
    AsciiOutput, AsciiOutputEncoding, AsciiOutputMetadata, AsciiOutputOutcome, AsciiOutputReport,
    AsciiOverflowPolicy, AsciiProjection, AsciiTrimPolicy, AsciiViewportPolicy, FallbackMetadata,
    Lossiness, OverflowPolicy,
};
pub use resource::{
    ASCII_RESOURCE_LIMIT_COUNT, ASCII_RESOURCE_LIMIT_DESCRIPTORS, AsciiResourceLimitCause,
    AsciiResourceLimitDescriptor, AsciiResourceLimitExceeded, AsciiResourceLimitId,
    AsciiResourceLimitOverrideError, AsciiResourceLimitPhase, AsciiResourcePolicy,
    MAX_ASCII_DOCUMENT_CELLS_RESOURCE_LIMIT_ID, MAX_ASCII_GRAPHEME_BYTES_RESOURCE_LIMIT_ID,
    MAX_ASCII_GRID_CELLS_RESOURCE_LIMIT_ID, MAX_ASCII_LAYOUT_WORK_UNITS_RESOURCE_LIMIT_ID,
    MAX_ASCII_NESTING_DEPTH_RESOURCE_LIMIT_ID, MAX_ASCII_OUTPUT_BYTES_RESOURCE_LIMIT_ID,
    ascii_resource_profile_value,
};
pub use safe_text::{normalize_terminal_diagnostic, normalize_terminal_text};

use merman_core::diagram::{ParsedDiagramRender, RenderSemanticModel};
#[cfg(feature = "diagram-er")]
use merman_core::diagrams::er::ErDiagramRenderModel;
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
use merman_core::diagrams::flowchart::{FlowchartModel, FlowchartRenderContext};
#[cfg(feature = "diagram-gantt")]
use merman_core::diagrams::gantt::GanttDiagramRenderModel;
#[cfg(feature = "diagram-git-graph")]
use merman_core::diagrams::git_graph::GitGraphRenderModel;
#[cfg(feature = "diagram-journey")]
use merman_core::diagrams::journey::JourneyDiagramRenderModel;
#[cfg(feature = "diagram-kanban")]
use merman_core::diagrams::kanban::KanbanDiagramRenderModel;
#[cfg(feature = "diagram-mindmap")]
use merman_core::diagrams::mindmap::MindmapDiagramRenderModel;
#[cfg(feature = "diagram-packet")]
use merman_core::diagrams::packet::PacketDiagramRenderModel;
#[cfg(feature = "diagram-sequence")]
use merman_core::diagrams::sequence::SequenceDiagramRenderModel;
#[cfg(feature = "diagram-state")]
use merman_core::diagrams::state::StateDiagramRenderModel;
#[cfg(feature = "diagram-timeline")]
use merman_core::diagrams::timeline::TimelineDiagramRenderModel;
#[cfg(feature = "diagram-tree-view")]
use merman_core::diagrams::tree_view::TreeViewDiagramRenderModel;
#[cfg(feature = "diagram-xychart")]
use merman_core::diagrams::xychart::XyChartDiagramRenderModel;
#[cfg(feature = "diagram-class")]
use merman_core::models::class_diagram::ClassDiagram;
use merman_core::runtime::OperationContext;
use merman_core::{MermaidConfig, ParseMetadata};
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
use options::FlowchartLayoutPolicy;
#[cfg(feature = "diagram-state")]
use options::GraphLayoutPolicy;
use options::ResolvedAsciiPolicies;
#[cfg(feature = "diagram-sequence")]
use options::SequenceLayoutPolicy;
#[cfg(feature = "diagram-xychart")]
use options::XyChartLayoutPolicy;

#[derive(Debug, Clone, Default)]
pub struct AsciiRenderer {
    options: AsciiRenderOptions,
}

struct AsciiRenderRequest<'a> {
    viewport: AsciiViewportPolicy,
    control: &'a merman_core::OperationControl,
    context: &'a OperationContext,
    resources: AsciiResourcePolicy,
}

impl<'a> AsciiRenderRequest<'a> {
    const fn new(
        viewport: AsciiViewportPolicy,
        control: &'a merman_core::OperationControl,
        context: &'a OperationContext,
        resources: AsciiResourcePolicy,
    ) -> Self {
        Self {
            viewport,
            control,
            context,
            resources,
        }
    }
}

impl AsciiRenderer {
    pub fn new(options: AsciiRenderOptions) -> Result<Self> {
        options.validate()?;
        Ok(Self { options })
    }

    pub fn options(&self) -> &AsciiRenderOptions {
        &self.options
    }

    /// Renders a typed model using caller-owned operation control, runtime context, and resources.
    ///
    /// This convenience entrypoint projects the canonical report down to text. It never creates
    /// a replacement operation, deadline, runtime context, or resource policy.
    ///
    /// Detached models use their model-kind identity. In particular, the shared Flowchart
    /// payload requires `diagram-flowchart`; use a parser-owned value with [`Self::render_parsed`]
    /// to preserve Swimlane's distinct language identity.
    pub fn render_model(
        &self,
        model: &RenderSemanticModel,
        control: &merman_core::OperationControl,
        context: &OperationContext,
        resources: AsciiResourcePolicy,
    ) -> Result<String> {
        let execution = operation::AsciiExecution::new(control, &resources);
        execution.checkpoint(merman_core::OperationPhase::Admission)?;
        layout_selection::validate_viewport(
            self.options.layout_profile,
            AsciiViewportPolicy::unrestricted(),
        )?;
        let policies = self.options.resolve_policies();
        render_model_with_execution(
            model,
            None,
            &policies.options,
            &policies,
            execution,
            context.local_time_zone(),
        )
    }

    /// Renders a typed model and returns logical extent/projection/overflow metadata.
    pub fn render_model_report(
        &self,
        model: &RenderSemanticModel,
        viewport: AsciiViewportPolicy,
        control: &merman_core::OperationControl,
        context: &OperationContext,
        resources: AsciiResourcePolicy,
    ) -> Result<AsciiOutput> {
        let metadata = ParseMetadata {
            diagram_type: model.kind().to_string(),
            config: MermaidConfig::default(),
            effective_config: MermaidConfig::default(),
            title: None,
        };
        self.render_report(
            model,
            None,
            &metadata,
            AsciiRenderRequest::new(viewport, control, context, resources),
        )
    }

    fn render_report(
        &self,
        model: &RenderSemanticModel,
        parsed: Option<&ParsedDiagramRender>,
        metadata: &ParseMetadata,
        request: AsciiRenderRequest<'_>,
    ) -> Result<AsciiOutput> {
        let AsciiRenderRequest {
            viewport,
            control,
            context,
            resources,
        } = request;
        viewport.validate()?;
        let render_ledger = resource::ResourceContext::new(resources);
        let execution = operation::AsciiExecution::new(control, &resources)
            .with_viewport(viewport)
            .with_render_ledger(&render_ledger);
        let requested_policies = self.options.resolve_policies();
        let capability = capability::capability_for_diagram_type(&metadata.diagram_type);
        execution.checkpoint(merman_core::OperationPhase::Admission)?;
        validate_local_handler(&metadata.diagram_type)?;
        layout_selection::validate_viewport(self.options.layout_profile, viewport)?;
        validate_primary_request(capability, &requested_policies)?;
        validate_fallback_request(capability, &requested_policies, viewport)?;
        let selected = layout_selection::select(
            self.options,
            viewport,
            execution,
            |options, candidate_execution| {
                let policies = options.resolve_policies();
                render_model_with_execution(
                    model,
                    parsed,
                    &options,
                    &policies,
                    candidate_execution,
                    context.local_time_zone(),
                )
            },
        )?;
        self.finish_report(model, metadata, viewport, execution, selected)
    }

    fn finish_report(
        &self,
        model: &RenderSemanticModel,
        metadata: &ParseMetadata,
        viewport: AsciiViewportPolicy,
        execution: operation::AsciiExecution<'_>,
        selected: layout_selection::SelectedLayout,
    ) -> Result<AsciiOutput> {
        let capability = capability::capability_for_diagram_type(&metadata.diagram_type);
        let policies = self
            .options
            .with_layout_profile(selected.profile)
            .resolve_policies();
        let projection = output::projection_for(capability);
        let fallback_capability = capability.is_some_and(|capability| {
            capability.supports_fallback_encoding(policies.output.encoding)
        });
        let output_context = output::OutputBuildContext {
            color_mode: policies.output.color_mode,
            profile: policies.output.terminal_width_profile,
            layout_profile: selected.profile,
            policy: viewport,
            execution,
        };
        let requested_profile = self.options.layout_profile;
        let compact_attempted = selected.compact_attempted;
        let annotate = |mut output: AsciiOutput| {
            output.requested_layout_profile = requested_profile;
            output.compact_attempted = compact_attempted;
            output
        };
        let primary = match selected.candidate {
            layout_selection::LayoutCandidate::Measured(primary) => primary,
            layout_selection::LayoutCandidate::Overflow(primary_extent) => {
                if viewport.overflow == OverflowPolicy::Error {
                    return Err(AsciiError::WidthOverflow {
                        max_width: viewport.max_width.expect("overflow requires a width bound"),
                        actual_width: primary_extent.width,
                        profile: policies.output.terminal_width_profile,
                    });
                }
                return output::build_semantic_fallback(
                    model,
                    metadata,
                    primary_extent,
                    output_context,
                )
                .map(annotate);
            }
        };
        let primary_extent = primary.metrics().extent;
        let overflowed = viewport
            .max_width
            .is_some_and(|max_width| primary_extent.width > max_width);
        if overflowed && viewport.overflow == output::OverflowPolicy::Fallback {
            if !fallback_capability {
                return Err(AsciiError::FallbackUnavailable {
                    diagram_type: model.kind().to_string(),
                    max_width: viewport.max_width.expect("fallback requires a width bound"),
                    actual_width: primary_extent.width,
                });
            }
            if projection == AsciiProjection::StructuredText {
                return output::build_structured_fallback(model.kind(), primary, output_context)
                    .map(annotate);
            }
            drop(primary);
            return output::build_semantic_fallback(
                model,
                metadata,
                primary_extent,
                output_context,
            )
            .map(annotate);
        }
        let mut output = output::build_output(model.kind(), primary, projection, output_context)?;
        output.fallback.capability = if fallback_capability {
            output::AsciiFallbackCapability::Available
        } else {
            output::AsciiFallbackCapability::Unsupported
        };
        Ok(annotate(output))
    }

    /// Renders one parser-owned model together with its render-only semantic context.
    ///
    /// Admission uses the parsed logical family, so enabling the shared Flowchart implementation
    /// for Swimlane does not admit Flowchart source, or vice versa.
    #[doc(hidden)]
    pub fn render_parsed(
        &self,
        parsed: &ParsedDiagramRender,
        control: &merman_core::OperationControl,
        context: &OperationContext,
        resources: AsciiResourcePolicy,
    ) -> Result<String> {
        let execution = operation::AsciiExecution::new(control, &resources);
        execution.checkpoint(merman_core::OperationPhase::Admission)?;
        layout_selection::validate_viewport(
            self.options.layout_profile,
            AsciiViewportPolicy::unrestricted(),
        )?;
        let policies = self.options.resolve_policies();
        render_model_with_execution(
            parsed.model(),
            Some(parsed),
            &policies.options,
            &policies,
            execution,
            context.local_time_zone(),
        )
    }

    /// Renders a parser-owned model and returns logical extent/projection/overflow metadata.
    #[doc(hidden)]
    pub fn render_parsed_report(
        &self,
        parsed: &ParsedDiagramRender,
        viewport: AsciiViewportPolicy,
        control: &merman_core::OperationControl,
        context: &OperationContext,
        resources: AsciiResourcePolicy,
    ) -> Result<AsciiOutput> {
        self.render_report(
            parsed.model(),
            Some(parsed),
            parsed.metadata(),
            AsciiRenderRequest::new(viewport, control, context, resources),
        )
    }
}

fn render_model_with_execution(
    model: &RenderSemanticModel,
    parsed: Option<&ParsedDiagramRender>,
    options: &AsciiRenderOptions,
    policies: &ResolvedAsciiPolicies,
    execution: operation::AsciiExecution<'_>,
    _local_time_zone: &merman_core::time::LocalTimeZone,
) -> Result<String> {
    execution.checkpoint(merman_core::OperationPhase::Admission)?;
    options.validate()?;
    let diagram_type = parsed.map_or_else(
        || model.kind(),
        |parsed| parsed.metadata().diagram_type.as_str(),
    );
    validate_local_handler(diagram_type)?;
    validate_primary_request(
        capability::capability_for_diagram_type(diagram_type),
        policies,
    )?;

    let rendered = match model {
        #[cfg(feature = "diagram-class")]
        RenderSemanticModel::Class(model) => render_class_model(model, options, &execution),
        #[cfg(feature = "diagram-er")]
        RenderSemanticModel::Er(model) => render_er_model(model, options, &execution),
        #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
        RenderSemanticModel::Flowchart(model) => render_flowchart_model(
            model,
            parsed.and_then(ParsedDiagramRender::flowchart_render_context),
            options,
            policies.layout.flowchart,
            &execution,
        ),
        #[cfg(feature = "diagram-gantt")]
        RenderSemanticModel::Gantt(model) => {
            render_gantt_model(model, options, _local_time_zone, &execution)
        }
        #[cfg(feature = "diagram-git-graph")]
        RenderSemanticModel::GitGraph(model) => render_git_graph_model(model, options, &execution),
        #[cfg(feature = "diagram-journey")]
        RenderSemanticModel::Journey(model) => render_journey_model(model, options, &execution),
        #[cfg(feature = "diagram-kanban")]
        RenderSemanticModel::Kanban(model) => render_kanban_model(model, options, &execution),
        #[cfg(feature = "diagram-mindmap")]
        RenderSemanticModel::Mindmap(model) => render_mindmap_model(model, options, &execution),
        #[cfg(feature = "diagram-packet")]
        RenderSemanticModel::Packet(model) => render_packet_model(model, options, &execution),
        #[cfg(feature = "diagram-sequence")]
        RenderSemanticModel::Sequence(model) => {
            render_sequence_model(model, options, policies.layout.sequence, &execution)
        }
        #[cfg(feature = "diagram-state")]
        RenderSemanticModel::State(model) => {
            render_state_model(model, options, policies.layout.state, &execution)
        }
        #[cfg(feature = "diagram-timeline")]
        RenderSemanticModel::Timeline(model) => render_timeline_model(model, options, &execution),
        #[cfg(feature = "diagram-xychart")]
        RenderSemanticModel::XyChart(model) => {
            render_xychart_model(model, options, policies.layout.xychart, &execution)
        }
        #[cfg(feature = "diagram-tree-view")]
        RenderSemanticModel::TreeView(model) => render_tree_view_model(model, options, &execution),
        _ => Err(AsciiError::UnsupportedDiagram {
            diagram_type: model.kind().to_string(),
        }),
    }?;
    execution.checkpoint(merman_core::OperationPhase::Emit)?;
    Ok(rendered)
}

fn validate_local_handler(diagram_type: &str) -> Result<()> {
    if capability::local_handler_available(diagram_type) {
        Ok(())
    } else {
        Err(AsciiError::UnsupportedDiagram {
            diagram_type: diagram_type.to_string(),
        })
    }
}

fn validate_primary_request(
    capability: Option<AsciiCapability>,
    policies: &ResolvedAsciiPolicies,
) -> Result<()> {
    let Some(capability) = capability.filter(|capability| capability.is_supported()) else {
        return Ok(());
    };
    if !capability.supports_layout_profile(policies.layout.profile) {
        return Err(AsciiError::InvalidOption {
            field: "layout_profile",
            message: "is not admitted for this diagram family",
        });
    }
    if !capability.supports_width_profile(policies.output.terminal_width_profile) {
        return Err(AsciiError::InvalidOption {
            field: "terminal_width_profile",
            message: "is not admitted for this diagram family",
        });
    }
    if !capability.supports_encoding(policies.output.encoding) {
        return Err(AsciiError::InvalidOption {
            field: "color_mode",
            message: "is not admitted for this diagram family",
        });
    }
    Ok(())
}

fn validate_fallback_request(
    capability: Option<AsciiCapability>,
    policies: &ResolvedAsciiPolicies,
    viewport: AsciiViewportPolicy,
) -> Result<()> {
    if viewport.overflow != OverflowPolicy::Fallback {
        return Ok(());
    }
    let Some(capability) = capability.filter(|capability| capability.is_supported()) else {
        return Ok(());
    };
    if !capability.supports_fallback_encoding(policies.output.encoding) {
        return Err(AsciiError::InvalidOption {
            field: "ascii_viewport.overflow",
            message: "fallback is not admitted for the selected output encoding",
        });
    }
    Ok(())
}

#[cfg(feature = "diagram-class")]
fn render_class_model(
    model: &ClassDiagram,
    options: &AsciiRenderOptions,
    execution: &operation::AsciiExecution<'_>,
) -> Result<String> {
    class::render_class_diagram_with_execution(model, options, *execution)
}

#[cfg(feature = "diagram-er")]
fn render_er_model(
    model: &ErDiagramRenderModel,
    options: &AsciiRenderOptions,
    execution: &operation::AsciiExecution<'_>,
) -> Result<String> {
    er::render_er_diagram_with_execution(model, options, *execution)
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
fn render_flowchart_model(
    model: &FlowchartModel,
    render_context: Option<&FlowchartRenderContext>,
    options: &AsciiRenderOptions,
    layout: FlowchartLayoutPolicy,
    execution: &operation::AsciiExecution<'_>,
) -> Result<String> {
    execution.checkpoint(merman_core::OperationPhase::Semantic)?;
    let transaction = execution.new_resource_context(merman_core::OperationPhase::Semantic);
    transaction.transaction(|_| {
        let result = transaction.transaction_preserving_layout_work(|_| {
            render_flowchart_model_inner(model, render_context, options, layout, *execution)
        });
        match result {
            // A viewport fallback reuses the render-wide ledger. Preserve the work spent proving
            // that the primary graph is too wide, but discard its speculative document cells.
            Err(error @ AsciiError::PrimaryViewportOverflow { .. }) => Ok(Err(error)),
            Err(error @ AsciiError::UnsupportedFeature { .. })
                if execution.is_optional_layout_candidate() =>
            {
                Ok(Err(error))
            }
            Ok(rendered) => Ok(Ok(rendered)),
            Err(error) => Err(error),
        }
    })?
}

#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
fn render_flowchart_model_inner(
    model: &FlowchartModel,
    render_context: Option<&FlowchartRenderContext>,
    options: &AsciiRenderOptions,
    layout: FlowchartLayoutPolicy,
    execution: operation::AsciiExecution<'_>,
) -> Result<String> {
    let mut semantic_resources =
        execution.new_resource_context(merman_core::OperationPhase::Semantic);
    let graph = graph::from_flowchart_model_with_execution(
        model,
        render_context,
        layout,
        &mut semantic_resources,
        execution,
    )?;
    execution.checkpoint(merman_core::OperationPhase::Layout)?;
    let mut layout_resources =
        execution.resource_context(&semantic_resources, merman_core::OperationPhase::Layout);
    graph::render_graph_with_resolved_policy_and_execution(
        &graph,
        options,
        layout.graph_policy(),
        &mut layout_resources,
        execution,
    )
}

#[cfg(feature = "diagram-gantt")]
fn render_gantt_model(
    model: &GanttDiagramRenderModel,
    options: &AsciiRenderOptions,
    local_time_zone: &merman_core::time::LocalTimeZone,
    execution: &operation::AsciiExecution<'_>,
) -> Result<String> {
    execution.checkpoint(merman_core::OperationPhase::Layout)?;
    gantt::render_gantt_diagram(model, options, local_time_zone, *execution)
}

#[cfg(feature = "diagram-git-graph")]
fn render_git_graph_model(
    model: &GitGraphRenderModel,
    options: &AsciiRenderOptions,
    execution: &operation::AsciiExecution<'_>,
) -> Result<String> {
    execution.checkpoint(merman_core::OperationPhase::Layout)?;
    git_graph::render_git_graph_diagram(model, options, *execution)
}

#[cfg(feature = "diagram-journey")]
fn render_journey_model(
    model: &JourneyDiagramRenderModel,
    options: &AsciiRenderOptions,
    execution: &operation::AsciiExecution<'_>,
) -> Result<String> {
    execution.checkpoint(merman_core::OperationPhase::Layout)?;
    journey::render_journey_diagram(model, options, *execution)
}

#[cfg(feature = "diagram-kanban")]
fn render_kanban_model(
    model: &KanbanDiagramRenderModel,
    options: &AsciiRenderOptions,
    execution: &operation::AsciiExecution<'_>,
) -> Result<String> {
    execution.checkpoint(merman_core::OperationPhase::Layout)?;
    kanban::render_kanban_diagram(model, options, *execution)
}

#[cfg(feature = "diagram-mindmap")]
fn render_mindmap_model(
    model: &MindmapDiagramRenderModel,
    options: &AsciiRenderOptions,
    execution: &operation::AsciiExecution<'_>,
) -> Result<String> {
    execution.checkpoint(merman_core::OperationPhase::Layout)?;
    mindmap::render_mindmap_diagram(model, options, *execution)
}

#[cfg(feature = "diagram-packet")]
fn render_packet_model(
    model: &PacketDiagramRenderModel,
    options: &AsciiRenderOptions,
    execution: &operation::AsciiExecution<'_>,
) -> Result<String> {
    execution.checkpoint(merman_core::OperationPhase::Layout)?;
    packet::render_packet_diagram(model, options, *execution)
}

#[cfg(feature = "diagram-sequence")]
fn render_sequence_model(
    model: &SequenceDiagramRenderModel,
    options: &AsciiRenderOptions,
    layout: SequenceLayoutPolicy,
    execution: &operation::AsciiExecution<'_>,
) -> Result<String> {
    execution.checkpoint(merman_core::OperationPhase::Semantic)?;
    let mut resources = execution.new_resource_context(merman_core::OperationPhase::Semantic);
    let diagram = sequence::from_sequence_model(model, layout, &mut resources, *execution)?;
    sequence::render_sequence_diagram_with_resolved_policy(
        &diagram,
        model.title.as_deref().filter(|title| !title.is_empty()),
        options,
        layout,
        &mut resources,
        *execution,
    )
}

#[cfg(feature = "diagram-state")]
fn render_state_model(
    model: &StateDiagramRenderModel,
    options: &AsciiRenderOptions,
    layout: GraphLayoutPolicy,
    execution: &operation::AsciiExecution<'_>,
) -> Result<String> {
    execution.checkpoint(merman_core::OperationPhase::Semantic)?;
    let transaction = execution.new_resource_context(merman_core::OperationPhase::Semantic);
    transaction.transaction(|_| {
        let result = transaction.transaction_preserving_layout_work(|_| {
            render_state_model_inner(model, options, layout, *execution)
        });
        match result {
            // State diagrams share the graph renderer and the same render-wide fallback ledger.
            Err(error @ AsciiError::PrimaryViewportOverflow { .. }) => Ok(Err(error)),
            Err(error @ AsciiError::UnsupportedFeature { .. })
                if execution.is_optional_layout_candidate() =>
            {
                Ok(Err(error))
            }
            Ok(rendered) => Ok(Ok(rendered)),
            Err(error) => Err(error),
        }
    })?
}

#[cfg(feature = "diagram-state")]
fn render_state_model_inner(
    model: &StateDiagramRenderModel,
    options: &AsciiRenderOptions,
    layout: GraphLayoutPolicy,
    execution: operation::AsciiExecution<'_>,
) -> Result<String> {
    let mut semantic_resources =
        execution.new_resource_context(merman_core::OperationPhase::Semantic);
    let graph = state::from_state_model_with_context_and_execution(
        model,
        options.terminal_width_profile,
        &mut semantic_resources,
        execution,
    )?;
    execution.checkpoint(merman_core::OperationPhase::Layout)?;
    let mut layout_resources =
        execution.resource_context(&semantic_resources, merman_core::OperationPhase::Layout);
    graph::render_graph_with_resolved_policy_and_execution(
        &graph,
        options,
        layout,
        &mut layout_resources,
        execution,
    )
}

#[cfg(feature = "diagram-timeline")]
fn render_timeline_model(
    model: &TimelineDiagramRenderModel,
    options: &AsciiRenderOptions,
    execution: &operation::AsciiExecution<'_>,
) -> Result<String> {
    execution.checkpoint(merman_core::OperationPhase::Layout)?;
    timeline::render_timeline_diagram(model, options, *execution)
}

#[cfg(feature = "diagram-xychart")]
fn render_xychart_model(
    model: &XyChartDiagramRenderModel,
    options: &AsciiRenderOptions,
    layout: XyChartLayoutPolicy,
    execution: &operation::AsciiExecution<'_>,
) -> Result<String> {
    execution.checkpoint(merman_core::OperationPhase::Semantic)?;
    xychart::render_xychart_diagram_with_resolved_policy(model, options, layout, *execution)
}

#[cfg(feature = "diagram-tree-view")]
fn render_tree_view_model(
    model: &TreeViewDiagramRenderModel,
    options: &AsciiRenderOptions,
    execution: &operation::AsciiExecution<'_>,
) -> Result<String> {
    execution.checkpoint(merman_core::OperationPhase::Layout)?;
    tree_view::render_tree_view_diagram(model, options, *execution)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "diagram-flowchart")]
    use merman_core::diagrams::flowchart::{
        FlowEdge, FlowEdgeMarker, FlowEdgeStroke, FlowEdgeVisibility, FlowNode, FlowSubgraph,
        FlowchartModel,
    };
    #[cfg(feature = "diagram-mindmap")]
    use merman_core::diagrams::mindmap::{MindmapDiagramRenderModel, MindmapDiagramRenderNode};
    #[cfg(feature = "diagram-tree-view")]
    use merman_core::diagrams::tree_view::{TreeViewDiagramRenderModel, TreeViewNodeRenderModel};

    #[cfg(any(
        feature = "diagram-flowchart",
        feature = "diagram-mindmap",
        feature = "diagram-tree-view"
    ))]
    fn render_model(model: &RenderSemanticModel, options: &AsciiRenderOptions) -> Result<String> {
        render_model_with_resources(model, options, AsciiResourcePolicy::default())
    }

    #[cfg(any(
        feature = "diagram-flowchart",
        feature = "diagram-mindmap",
        feature = "diagram-tree-view"
    ))]
    fn render_model_with_resources(
        model: &RenderSemanticModel,
        options: &AsciiRenderOptions,
        resources: AsciiResourcePolicy,
    ) -> Result<String> {
        let context = merman_core::runtime::RuntimePolicy::deterministic()
            .begin_operation()
            .expect("deterministic test operation context");
        AsciiRenderer::new(*options)?.render_model(
            model,
            &merman_core::OperationControl::new(),
            &context,
            resources,
        )
    }

    #[cfg(feature = "diagram-flowchart")]
    fn render_flowchart(model: &FlowchartModel, options: &AsciiRenderOptions) -> Result<String> {
        render_model(&RenderSemanticModel::Flowchart(model.clone()), options)
    }

    #[cfg(feature = "diagram-flowchart")]
    fn render_flowchart_with_resources(
        model: &FlowchartModel,
        options: &AsciiRenderOptions,
        resources: AsciiResourcePolicy,
    ) -> Result<String> {
        render_model_with_resources(
            &RenderSemanticModel::Flowchart(model.clone()),
            options,
            resources,
        )
    }

    #[cfg(feature = "diagram-flowchart")]
    fn empty_flowchart() -> FlowchartModel {
        FlowchartModel {
            keyword: "graph".to_string(),
            acc_descr: None,
            acc_title: None,
            class_defs: Default::default(),
            direction: None,
            edge_defaults: None,
            vertex_calls: Vec::new(),
            nodes: Vec::new(),
            edges: Vec::new(),
            subgraphs: Vec::new(),
            tooltips: Default::default(),
            warning_facts: Vec::new(),
        }
    }

    #[cfg(feature = "diagram-flowchart")]
    fn node(id: &str) -> FlowNode {
        FlowNode {
            id: id.to_string(),
            provenance: Default::default(),
            label: Some(id.to_string()),
            label_type: None,
            layout_shape: None,
            shape: None,
            icon: None,
            form: None,
            pos: None,
            img: None,
            constraint: None,
            asset_width: None,
            asset_height: None,
            classes: Vec::new(),
            styles: Vec::new(),
            link: None,
            link_target: None,
            have_callback: false,
        }
    }

    #[cfg(feature = "diagram-flowchart")]
    fn edge(from: &str, to: &str) -> FlowEdge {
        FlowEdge {
            id: format!("{from}-{to}"),
            from: from.to_string(),
            to: to.to_string(),
            label: None,
            label_type: None,
            edge_type: None,
            arrow: "-->".to_string(),
            start_marker: FlowEdgeMarker::None,
            end_marker: FlowEdgeMarker::Point,
            is_user_defined_id: false,
            stroke: None,
            stroke_kind: FlowEdgeStroke::Normal,
            visibility: FlowEdgeVisibility::Visible,
            interpolate: None,
            classes: Vec::new(),
            style: Vec::new(),
            animate: None,
            animation: None,
            length: 1,
        }
    }

    #[test]
    fn default_options_match_initial_reference_defaults() {
        let options = AsciiRenderOptions::default();
        assert_eq!(options.charset, AsciiCharset::Unicode);
        assert_eq!(
            options.terminal_width_profile,
            TerminalWidthProfile::Unicode
        );
        assert_eq!(options.default_direction, AsciiDirection::LeftRight);
        assert_eq!(options.color_mode, AsciiColorMode::Plain);
        assert_eq!(options.color_theme, AsciiColorTheme::default_light());
        assert_eq!(options.box_border_padding, 1);
        assert_eq!(options.graph_padding_x, 5);
        assert_eq!(options.graph_padding_y, 5);
        assert_eq!(options.flowchart_node_label_wrap_width, 40);
        assert_eq!(options.sequence_participant_spacing, 5);
        assert_eq!(options.sequence_message_spacing, 1);
        assert_eq!(options.sequence_self_message_width, 4);
        assert!(!options.sequence_mirror_actors);
        assert_eq!(options.xychart_vertical_plot_height, 5);
        assert_eq!(options.xychart_category_band_width, 3);
        assert_eq!(options.xychart_horizontal_plot_width, 10);
        assert!(!options.relation_summary_diagnostics);
    }

    #[test]
    fn options_builder_sets_color_mode_and_theme() {
        let edge_arrow = AsciiRgb::from_hex24(0x7aa2f7);
        let theme =
            AsciiColorTheme::default_dark().with_role(AsciiColorRole::EdgeArrow, edge_arrow);

        let options = AsciiRenderOptions::unicode()
            .with_color_mode(AsciiColorMode::TrueColor)
            .with_color_theme(theme);

        assert_eq!(options.color_mode, AsciiColorMode::TrueColor);
        assert_eq!(
            options.color_theme.color_for(AsciiColorRole::EdgeArrow),
            edge_arrow
        );
        assert_eq!(
            options
                .color_theme
                .color_for(AsciiColorRole::ChartSeries(9)),
            AsciiColorTheme::default_dark().color_for(AsciiColorRole::ChartSeries(1))
        );
    }

    #[test]
    fn options_builder_sets_terminal_width_profile() {
        let options =
            AsciiRenderOptions::unicode().with_terminal_width_profile(TerminalWidthProfile::Cjk);

        assert_eq!(options.terminal_width_profile, TerminalWidthProfile::Cjk);
    }

    #[test]
    fn options_builder_sets_sequence_mirror_actors() {
        let options = AsciiRenderOptions::unicode().with_sequence_mirror_actors(true);

        assert!(options.sequence_mirror_actors);
    }

    #[test]
    fn options_builder_sets_flowchart_node_label_wrap_width() {
        let options = AsciiRenderOptions::unicode().with_flowchart_node_label_wrap_width(24);

        assert_eq!(options.flowchart_node_label_wrap_width, 24);
    }

    #[test]
    fn options_builder_sets_xychart_plot_area_dimensions() {
        let options = AsciiRenderOptions::ascii()
            .with_xychart_vertical_plot_height(8)
            .with_xychart_category_band_width(4)
            .with_xychart_horizontal_plot_width(24);

        assert_eq!(options.xychart_vertical_plot_height, 8);
        assert_eq!(options.xychart_category_band_width, 4);
        assert_eq!(options.xychart_horizontal_plot_width, 24);
    }

    #[test]
    fn resource_policy_sets_typed_grid_cell_limit() {
        let mut resources = AsciiResourcePolicy::default();
        resources
            .apply_limit(AsciiResourceLimitId::MaxGridCells, 42)
            .unwrap();

        assert_eq!(
            resources.value(AsciiResourceLimitId::MaxGridCells),
            Some(42)
        );
    }

    #[test]
    fn options_builder_sets_relation_summary_diagnostics() {
        let options = AsciiRenderOptions::ascii().with_relation_summary_diagnostics(true);

        assert!(options.relation_summary_diagnostics);
    }

    #[test]
    fn validates_sequence_self_message_width() {
        let options = AsciiRenderOptions {
            sequence_self_message_width: 1,
            ..AsciiRenderOptions::default()
        };

        assert_eq!(
            options.validate(),
            Err(AsciiError::InvalidOption {
                field: "sequence_self_message_width",
                message: "must be at least 2",
            })
        );
    }

    #[test]
    fn validates_flowchart_node_label_wrap_width() {
        let options = AsciiRenderOptions {
            flowchart_node_label_wrap_width: 0,
            ..AsciiRenderOptions::default()
        };

        assert_eq!(
            options.validate(),
            Err(AsciiError::InvalidOption {
                field: "flowchart_node_label_wrap_width",
                message: "must be greater than 0",
            })
        );
    }

    #[test]
    fn validates_xychart_plot_area_dimensions() {
        let cases = [
            (
                AsciiRenderOptions {
                    xychart_vertical_plot_height: 1,
                    ..AsciiRenderOptions::default()
                },
                "xychart_vertical_plot_height",
                "must be at least 2",
            ),
            (
                AsciiRenderOptions {
                    xychart_category_band_width: 0,
                    ..AsciiRenderOptions::default()
                },
                "xychart_category_band_width",
                "must be greater than 0",
            ),
            (
                AsciiRenderOptions {
                    xychart_horizontal_plot_width: 1,
                    ..AsciiRenderOptions::default()
                },
                "xychart_horizontal_plot_width",
                "must be at least 2",
            ),
        ];

        for (options, field, message) in cases {
            assert_eq!(
                options.validate(),
                Err(AsciiError::InvalidOption { field, message })
            );
        }
    }

    #[test]
    #[cfg(feature = "diagram-flowchart")]
    fn render_model_routes_basic_flowchart_to_graph_renderer() {
        let model = RenderSemanticModel::Flowchart(empty_flowchart());

        let rendered = render_model(&model, &AsciiRenderOptions::default()).unwrap();

        assert_eq!(rendered, "");
    }

    #[cfg(feature = "diagram-tree-view")]
    fn tree_view_model() -> TreeViewDiagramRenderModel {
        TreeViewDiagramRenderModel {
            acc_title: None,
            acc_descr: None,
            title: None,
            root: TreeViewNodeRenderModel {
                id: 0,
                level: -1,
                name: "/".to_string(),
                children: vec![TreeViewNodeRenderModel {
                    id: 1,
                    level: 0,
                    name: "Root".to_string(),
                    children: vec![
                        TreeViewNodeRenderModel {
                            id: 2,
                            level: 1,
                            name: "Child1".to_string(),
                            children: Vec::new(),
                            ..Default::default()
                        },
                        TreeViewNodeRenderModel {
                            id: 3,
                            level: 1,
                            name: "Child2".to_string(),
                            children: Vec::new(),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                }],
                ..Default::default()
            },
        }
    }

    #[cfg(feature = "diagram-mindmap")]
    fn mindmap_model() -> MindmapDiagramRenderModel {
        MindmapDiagramRenderModel {
            nodes: vec![
                MindmapDiagramRenderNode {
                    id: "0".to_string(),
                    dom_id: "node_0".to_string(),
                    label: "Root".to_string(),
                    label_type: String::new(),
                    is_group: false,
                    shape: "defaultMindmapNode".to_string(),
                    width: 40.0,
                    height: 24.0,
                    padding: 10.0,
                    css_classes: "mindmap-node section-root section--1".to_string(),
                    css_styles: Vec::new(),
                    look: "classic".to_string(),
                    icon: None,
                    x: None,
                    y: None,
                    level: 0,
                    node_id: "0".to_string(),
                    node_type: 0,
                    section: None,
                },
                MindmapDiagramRenderNode {
                    id: "1".to_string(),
                    dom_id: "node_1".to_string(),
                    label: "Child".to_string(),
                    label_type: String::new(),
                    is_group: false,
                    shape: "defaultMindmapNode".to_string(),
                    width: 40.0,
                    height: 24.0,
                    padding: 10.0,
                    css_classes: "mindmap-node section-0".to_string(),
                    css_styles: Vec::new(),
                    look: "classic".to_string(),
                    icon: None,
                    x: None,
                    y: None,
                    level: 1,
                    node_id: "1".to_string(),
                    node_type: 0,
                    section: None,
                },
            ],
            edges: Vec::new(),
        }
    }

    #[test]
    #[cfg(feature = "diagram-tree-view")]
    fn render_model_routes_tree_view_to_hierarchy_renderer() {
        let model = RenderSemanticModel::TreeView(tree_view_model());

        let rendered = render_model(&model, &AsciiRenderOptions::ascii()).unwrap();

        assert!(rendered.contains("Root"));
        assert!(rendered.contains("Child1"));
        assert!(rendered.contains("Child2"));
    }

    #[test]
    #[cfg(feature = "diagram-mindmap")]
    fn render_model_routes_mindmap_to_hierarchy_renderer() {
        let model = RenderSemanticModel::Mindmap(mindmap_model());

        let rendered = render_model(&model, &AsciiRenderOptions::ascii()).unwrap();

        assert!(rendered.contains("Root"));
        assert!(rendered.contains("Child"));
    }

    #[test]
    #[cfg(feature = "diagram-flowchart")]
    fn render_flowchart_renders_basic_left_right_chain() {
        let mut model = empty_flowchart();
        model.direction = Some("LR".to_string());
        model.nodes = vec![node("A"), node("B")];
        model.edges = vec![edge("A", "B")];

        let rendered = render_flowchart(&model, &AsciiRenderOptions::ascii()).unwrap();

        assert_eq!(
            rendered,
            "+---+     +---+\n|   |     |   |\n| A |---->| B |\n|   |     |   |\n+---+     +---+\n"
        );
    }

    #[test]
    #[cfg(feature = "diagram-flowchart")]
    fn render_flowchart_respects_grid_cell_limit() {
        let mut model = empty_flowchart();
        model.nodes = vec![node("A"), node("B")];
        model.edges = vec![edge("A", "B")];
        let options = AsciiRenderOptions::ascii();
        let mut resources = AsciiResourcePolicy::default();
        resources
            .apply_limit(AsciiResourceLimitId::MaxGridCells, 1)
            .unwrap();

        let err = render_flowchart_with_resources(&model, &options, resources).unwrap_err();

        assert!(matches!(
            err,
            AsciiError::ResourceLimitExceeded(details)
                if details.limit == AsciiResourceLimitId::MaxGridCells
                    && details.actual > details.max
                    && details.max == 1
        ));
    }

    #[test]
    #[cfg(feature = "diagram-flowchart")]
    fn render_flowchart_renders_model_edge_labels() {
        let mut model = empty_flowchart();
        model.nodes = vec![node("A"), node("B")];
        model.edges = vec![FlowEdge {
            label: Some("label".to_string()),
            ..edge("A", "B")
        }];

        let rendered = render_flowchart(&model, &AsciiRenderOptions::ascii()).unwrap();

        assert_eq!(
            rendered,
            "+---+          +---+\n|   |          |   |\n| A |--label-->| B |\n|   |          |   |\n+---+          +---+\n"
        );
    }

    #[test]
    #[cfg(feature = "diagram-flowchart")]
    fn render_flowchart_supports_invisible_constraints_and_cross_markers() {
        let mut invisible = empty_flowchart();
        invisible.nodes = vec![node("A"), node("B")];
        invisible.edges = vec![FlowEdge {
            stroke: Some("invisible".to_string()),
            visibility: FlowEdgeVisibility::Invisible,
            ..edge("A", "B")
        }];

        let invisible_rendered =
            render_flowchart(&invisible, &AsciiRenderOptions::ascii()).unwrap();
        assert!(
            invisible_rendered.contains("| A |") && invisible_rendered.contains("| B |"),
            "invisible constraints should retain both positioned nodes:\n{invisible_rendered}"
        );
        assert!(
            !invisible_rendered.contains("| A |-") && !invisible_rendered.contains(">| B |"),
            "invisible constraints must not paint a terminal edge:\n{invisible_rendered}"
        );

        let mut cross = empty_flowchart();
        cross.nodes = vec![node("A"), node("B")];
        cross.edges = vec![FlowEdge {
            edge_type: Some("arrow_cross".to_string()),
            end_marker: FlowEdgeMarker::Cross,
            ..edge("A", "B")
        }];

        let cross_rendered = render_flowchart(&cross, &AsciiRenderOptions::ascii()).unwrap();
        assert!(
            cross_rendered.contains('x'),
            "cross marker should remain visible:\n{cross_rendered}"
        );
    }

    #[test]
    #[cfg(feature = "diagram-flowchart")]
    fn render_flowchart_renders_model_subgraphs() {
        let mut model = empty_flowchart();
        model.nodes = vec![node("A")];
        model.subgraphs = vec![FlowSubgraph {
            metadata: None,
            id: "cluster".to_string(),
            title: "cluster".to_string(),
            dir: None,
            has_explicit_dir: false,
            label_type: None,
            classes: Vec::new(),
            styles: Vec::new(),
            nodes: vec!["A".to_string()],
        }];

        let rendered = render_flowchart(&model, &AsciiRenderOptions::ascii()).unwrap();

        assert_eq!(
            rendered,
            concat!(
                "+-------+\n",
                "|cluster|\n",
                "|       |\n",
                "|       |\n",
                "| +---+ |\n",
                "| |   | |\n",
                "| | A | |\n",
                "| |   | |\n",
                "| +---+ |\n",
                "|       |\n",
                "+-------+\n",
            )
        );
    }

    #[test]
    #[cfg(feature = "diagram-flowchart")]
    fn render_flowchart_renders_model_multiline_subgraph_titles() {
        let mut model = empty_flowchart();
        model.nodes = vec![node("A")];
        model.subgraphs = vec![FlowSubgraph {
            metadata: None,
            id: "cluster".to_string(),
            title: "Line\nTwo".to_string(),
            dir: None,
            has_explicit_dir: false,
            label_type: None,
            classes: Vec::new(),
            styles: Vec::new(),
            nodes: vec!["A".to_string()],
        }];

        let rendered = render_flowchart(&model, &AsciiRenderOptions::ascii()).unwrap();

        assert_eq!(
            rendered,
            concat!(
                "+-------+\n",
                "| Line  |\n",
                "|       |\n",
                "|  Two  |\n",
                "|       |\n",
                "|       |\n",
                "| +---+ |\n",
                "| |   | |\n",
                "| | A | |\n",
                "| |   | |\n",
                "| +---+ |\n",
                "|       |\n",
                "+-------+\n",
            )
        );
    }

    #[test]
    #[cfg(feature = "diagram-flowchart")]
    fn render_flowchart_rejects_unsupported_directions() {
        let mut model = empty_flowchart();
        model.direction = Some("XX".to_string());
        model.nodes = vec![node("A")];

        let err = render_flowchart(&model, &AsciiRenderOptions::ascii()).unwrap_err();

        assert_eq!(
            err,
            AsciiError::UnsupportedFeature {
                diagram_type: "flowchart",
                feature: "unsupported graph directions",
            }
        );
    }

    #[test]
    #[cfg(feature = "diagram-flowchart")]
    fn render_flowchart_rejects_edges_with_missing_endpoint_nodes() {
        let mut model = empty_flowchart();
        model.nodes = vec![node("A")];
        model.edges = vec![edge("A", "B")];

        let err = render_flowchart(&model, &AsciiRenderOptions::ascii()).unwrap_err();

        assert_eq!(
            err,
            AsciiError::UnsupportedFeature {
                diagram_type: "flowchart",
                feature: "edges with missing endpoint nodes",
            }
        );
    }
}
