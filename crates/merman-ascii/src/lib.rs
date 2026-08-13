#![forbid(unsafe_code)]
//! Terminal-friendly ASCII and Unicode rendering for Mermaid typed models.
//!
//! `merman-ascii` is deliberately model-driven: callers parse Mermaid text with `merman-core`, then
//! pass the resulting typed render model into this crate. The renderer does not own Mermaid syntax
//! parsing.

mod canvas;
mod capability;
mod class;
mod color;
mod er;
mod error;
mod gantt;
mod git_graph;
mod graph;
mod journey;
mod kanban;
mod mindmap;
mod operation;
mod options;
mod packet;
mod relation_graph;
mod sequence;
mod state;
mod style_color;
mod terminal;
mod text;
mod timeline;
mod tree_view;
mod xychart;

pub use capability::{
    AsciiCapability, AsciiCapabilityEvidence, AsciiEvidenceKind, AsciiSupportLevel,
    ascii_capabilities, ascii_supported_diagram_types,
};
pub use color::{AsciiColorMode, AsciiColorRole, AsciiColorTheme, AsciiRgb, AsciiTerminalPalette};
pub use error::{AsciiError, Result};
pub use operation::AsciiResourcePolicy;
pub use options::{
    ASCII_RESOURCE_LIMIT_DESCRIPTORS, AsciiCharset, AsciiDirection, AsciiRenderOptions,
    AsciiResourceLimitDescriptor, MAX_ASCII_GRID_CELLS_RESOURCE_LIMIT_ID,
    ascii_resource_profile_value,
};

use merman_core::diagram::RenderSemanticModel;
use merman_core::runtime::OperationContext;

#[derive(Debug, Clone, Default)]
pub struct AsciiRenderer {
    options: AsciiRenderOptions,
}

impl AsciiRenderer {
    pub fn new(options: AsciiRenderOptions) -> Result<Self> {
        options.validate()?;
        Ok(Self { options })
    }

    pub fn options(&self) -> &AsciiRenderOptions {
        &self.options
    }

    pub fn render_model(&self, model: &RenderSemanticModel) -> Result<String> {
        render_model(model, &self.options)
    }

    /// Renders a typed model using caller-owned operation control and runtime context.
    pub fn render_model_with_operation(
        &self,
        model: &RenderSemanticModel,
        control: &merman_core::OperationControl,
        context: &OperationContext,
        resources: AsciiResourcePolicy,
    ) -> Result<String> {
        render_model_with_operation(model, &self.options, control, context, resources)
    }
}

pub fn render_model(model: &RenderSemanticModel, options: &AsciiRenderOptions) -> Result<String> {
    render_model_with_local_time_zone(model, options, &merman_core::time::LocalTimeZone::utc())
}

/// Renders a typed model through the shared operation projection.
///
/// This is the model-level backend seam used by the canonical facade. It never creates a new
/// control, runtime context, or engine operation; callers retain ownership of all three.
pub fn render_model_with_operation(
    model: &RenderSemanticModel,
    options: &AsciiRenderOptions,
    control: &merman_core::OperationControl,
    context: &OperationContext,
    resources: AsciiResourcePolicy,
) -> Result<String> {
    let execution = operation::AsciiExecution::new(control, resources);
    render_model_with_execution(model, options, execution, context.local_time_zone())
}

fn render_model_with_execution(
    model: &RenderSemanticModel,
    options: &AsciiRenderOptions,
    execution: operation::AsciiExecution<'_>,
    local_time_zone: &merman_core::time::LocalTimeZone,
) -> Result<String> {
    execution.checkpoint(merman_core::OperationPhase::Admission)?;
    options.validate()?;

    let rendered = match model {
        RenderSemanticModel::Class(model) => {
            class::render_class_diagram_with_execution(model, options, execution)
        }
        RenderSemanticModel::Er(model) => {
            er::render_er_diagram_with_execution(model, options, execution)
        }
        RenderSemanticModel::Flowchart(model) => {
            execution.checkpoint(merman_core::OperationPhase::Semantic)?;
            let graph = graph::from_flowchart_model(model, options)?;
            execution.checkpoint(merman_core::OperationPhase::Layout)?;
            graph::render_graph_with_execution(&graph, options, execution)
        }
        RenderSemanticModel::Gantt(model) => {
            execution.checkpoint(merman_core::OperationPhase::Layout)?;
            let rendered = gantt::render_gantt_diagram(model, options, local_time_zone);
            execution.checkpoint(merman_core::OperationPhase::Emit)?;
            Ok(rendered)
        }
        RenderSemanticModel::GitGraph(model) => {
            execution.checkpoint(merman_core::OperationPhase::Layout)?;
            let rendered = git_graph::render_git_graph_diagram(model, options);
            execution.checkpoint(merman_core::OperationPhase::Emit)?;
            Ok(rendered)
        }
        RenderSemanticModel::Journey(model) => {
            execution.checkpoint(merman_core::OperationPhase::Layout)?;
            let rendered = journey::render_journey_diagram(model, options);
            execution.checkpoint(merman_core::OperationPhase::Emit)?;
            Ok(rendered)
        }
        RenderSemanticModel::Kanban(model) => {
            execution.checkpoint(merman_core::OperationPhase::Layout)?;
            let rendered = kanban::render_kanban_diagram(model, options);
            execution.checkpoint(merman_core::OperationPhase::Emit)?;
            Ok(rendered)
        }
        RenderSemanticModel::Mindmap(model) => {
            execution.checkpoint(merman_core::OperationPhase::Layout)?;
            let rendered = mindmap::render_mindmap_diagram(model, options);
            execution.checkpoint(merman_core::OperationPhase::Emit)?;
            Ok(rendered)
        }
        RenderSemanticModel::Packet(model) => {
            execution.checkpoint(merman_core::OperationPhase::Layout)?;
            let rendered = packet::render_packet_diagram(model, options);
            execution.checkpoint(merman_core::OperationPhase::Emit)?;
            Ok(rendered)
        }
        RenderSemanticModel::Sequence(model) => {
            execution.checkpoint(merman_core::OperationPhase::Semantic)?;
            let diagram = sequence::from_sequence_model(model)?;
            sequence::render_sequence_diagram_with_execution(&diagram, options, execution)
        }
        RenderSemanticModel::State(model) => {
            execution.checkpoint(merman_core::OperationPhase::Semantic)?;
            let graph = state::from_state_model(model)?;
            execution.checkpoint(merman_core::OperationPhase::Layout)?;
            graph::render_graph_with_execution(&graph, options, execution)
        }
        RenderSemanticModel::Timeline(model) => {
            execution.checkpoint(merman_core::OperationPhase::Layout)?;
            let rendered = timeline::render_timeline_diagram(model, options);
            execution.checkpoint(merman_core::OperationPhase::Emit)?;
            Ok(rendered)
        }
        RenderSemanticModel::XyChart(model) => {
            execution.checkpoint(merman_core::OperationPhase::Semantic)?;
            xychart::render_xychart_diagram_with_execution(model, options, execution)
        }
        RenderSemanticModel::TreeView(model) => {
            execution.checkpoint(merman_core::OperationPhase::Layout)?;
            let rendered = tree_view::render_tree_view_diagram(model, options);
            execution.checkpoint(merman_core::OperationPhase::Emit)?;
            Ok(rendered)
        }
        other => Err(AsciiError::UnsupportedDiagram {
            diagram_type: other.kind().to_string(),
        }),
    }?;
    execution.checkpoint(merman_core::OperationPhase::Emit)?;
    Ok(rendered)
}

/// Renders a typed model with an explicitly captured local-time resolver.
///
/// The resolver is used by Gantt output. This convenience entrypoint owns no operation state;
/// callers that need cancellation or deadlines should use [`render_model_with_operation`].
pub fn render_model_with_local_time_zone(
    model: &RenderSemanticModel,
    options: &AsciiRenderOptions,
    local_time_zone: &merman_core::time::LocalTimeZone,
) -> Result<String> {
    render_model_with_execution(
        model,
        options,
        operation::AsciiExecution::standalone(AsciiResourcePolicy::default()),
        local_time_zone,
    )
}
