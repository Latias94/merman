//! Controlled Mermaid-compatible JSON projection of existing full family data.

use super::compat::{CompatJsonMetadata, CompatProjectionError};
use super::{PresentationContext, PresentationDiagram, PresentationMetadata, RenderSemanticModel};
use crate::{ManagedJson, OperationControl, OperationControlResult, OperationPhase};

mod sealed {
    pub trait Sealed {}
}

/// A pinned family payload with a pure Mermaid-compatible JSON projection.
///
/// Membership is sealed to the canonical family data. Declared context affects projection shape;
/// it does not authenticate source history or provide source-admission authority.
pub trait BuiltinRenderSemantic: sealed::Sealed {
    fn compatibility_json(
        &self,
        meta: &PresentationMetadata,
    ) -> Result<ManagedJson, CompatProjectionError> {
        self.compatibility_json_controlled(meta, &OperationControl::new())
            .expect("a private projection control cannot be cancelled")
    }

    fn compatibility_json_controlled(
        &self,
        meta: &PresentationMetadata,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ManagedJson, CompatProjectionError>>;

    fn compatibility_json_with_context_controlled(
        &self,
        meta: &PresentationMetadata,
        _context: &PresentationContext,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ManagedJson, CompatProjectionError>> {
        self.compatibility_json_controlled(meta, control)
    }
}

macro_rules! simple_projection {
    ($model:path, $project:path) => {
        impl sealed::Sealed for $model {}
        impl BuiltinRenderSemantic for $model {
            fn compatibility_json_controlled(
                &self,
                meta: &PresentationMetadata,
                control: &OperationControl,
            ) -> OperationControlResult<Result<ManagedJson, CompatProjectionError>> {
                control.checkpoint()?;
                let projected =
                    $project(self, &CompatJsonMetadata::from(meta)).map(ManagedJson::from);
                control.checkpoint()?;
                Ok(projected)
            }
        }
    };
}

#[allow(
    unused_macros,
    reason = "Controlled families are independently selected."
)]
macro_rules! controlled_projection {
    ($model:path, $project:path) => {
        impl sealed::Sealed for $model {}
        impl BuiltinRenderSemantic for $model {
            fn compatibility_json_controlled(
                &self,
                meta: &PresentationMetadata,
                control: &OperationControl,
            ) -> OperationControlResult<Result<ManagedJson, CompatProjectionError>> {
                $project(self, &CompatJsonMetadata::from(meta), control)
                    .map(|result| result.map(ManagedJson::from))
            }
        }
    };
}

simple_projection!(
    crate::families::error::ErrorDiagramModel,
    crate::families::error::compat::project
);

#[cfg(feature = "diagram-cynefin")]
simple_projection!(
    crate::families::cynefin::CynefinDiagramModel,
    crate::families::cynefin::compat::project
);

#[cfg(feature = "diagram-railroad")]
simple_projection!(
    crate::families::railroad::RailroadDiagramModel,
    crate::families::railroad::compat::project
);

#[cfg(feature = "diagram-event-modeling")]
simple_projection!(
    crate::families::event_modeling::EventModelingDiagramModel,
    crate::families::event_modeling::compat::project
);

#[cfg(feature = "diagram-venn")]
simple_projection!(
    crate::families::venn::VennDiagramModel,
    crate::families::venn::compat::project
);

#[cfg(feature = "diagram-usecase")]
simple_projection!(
    crate::families::usecase::UsecaseDiagramModel,
    crate::families::usecase::compat::project
);

#[cfg(feature = "diagram-sankey")]
simple_projection!(
    crate::families::sankey::SankeyDiagramModel,
    crate::families::sankey::compat::project
);

#[cfg(feature = "diagram-quadrant-chart")]
simple_projection!(
    crate::families::quadrant_chart::QuadrantChartDiagramModel,
    crate::families::quadrant_chart::compat::project
);

#[cfg(feature = "diagram-requirement")]
simple_projection!(
    crate::families::requirement::RequirementDiagramModel,
    crate::families::requirement::compat::project
);

#[cfg(feature = "diagram-git-graph")]
simple_projection!(
    crate::families::git_graph::GitGraphDiagramModel,
    crate::families::git_graph::compat::project
);

#[cfg(feature = "diagram-c4")]
simple_projection!(
    crate::families::c4::C4DiagramModel,
    crate::families::c4::compat::project
);

#[cfg(feature = "diagram-xychart")]
simple_projection!(
    crate::families::xy_chart::XyChartDiagramRenderModel,
    crate::families::xy_chart::compat::project
);

#[cfg(feature = "diagram-kanban")]
simple_projection!(
    crate::families::kanban::KanbanDiagramRenderModel,
    crate::families::kanban::compat::project
);

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
simple_projection!(
    crate::families::flowchart::FlowchartModel,
    crate::families::flowchart::compat::project
);

#[cfg(feature = "diagram-agentflow")]
simple_projection!(
    crate::families::agentflow::AgentflowDiagramRenderModel,
    crate::families::agentflow::compat::project
);

#[cfg(feature = "diagram-class")]
simple_projection!(
    crate::families::class::ClassDiagram,
    crate::families::class::compat::project
);

#[cfg(feature = "diagram-er")]
simple_projection!(
    crate::families::er::ErDiagramModel,
    crate::families::er::compat::project
);

#[cfg(feature = "diagram-sequence")]
simple_projection!(
    crate::families::sequence::SequenceDiagramModel,
    crate::families::sequence::compat::project
);

#[cfg(feature = "diagram-wardley")]
controlled_projection!(
    crate::families::wardley::WardleyDiagramModel,
    crate::families::wardley::compat::project_controlled
);

#[cfg(feature = "diagram-gantt")]
controlled_projection!(
    crate::families::gantt::GanttDiagramRenderModel,
    crate::families::gantt::compat::project_controlled
);

#[cfg(feature = "diagram-architecture")]
controlled_projection!(
    crate::families::architecture::ArchitectureDiagramModel,
    crate::families::architecture::compat::project_controlled
);

#[cfg(feature = "diagram-zenuml")]
controlled_projection!(
    crate::families::zenuml::ZenumlDiagramModel,
    crate::families::zenuml::compat::project_controlled
);

#[cfg(feature = "diagram-tree-view")]
controlled_projection!(
    crate::families::tree_view::TreeViewDiagramModel,
    crate::families::tree_view::compat::project_controlled
);

#[cfg(feature = "diagram-ishikawa")]
controlled_projection!(
    crate::families::ishikawa::IshikawaDiagramModel,
    crate::families::ishikawa::compat::project_controlled
);

#[cfg(feature = "diagram-block")]
controlled_projection!(
    crate::families::block::BlockDiagramRenderModel,
    crate::families::block::compat::project_controlled
);

#[cfg(feature = "diagram-treemap")]
controlled_projection!(
    crate::families::treemap::TreemapDiagramRenderModel,
    crate::families::treemap::compat::project_controlled
);

#[cfg(feature = "diagram-state")]
controlled_projection!(
    crate::families::state::StateDiagramRenderModel,
    crate::families::state::compat::project_controlled
);

#[cfg(feature = "diagram-info")]
impl sealed::Sealed for crate::families::info::InfoDiagramModel {}
#[cfg(feature = "diagram-info")]
impl BuiltinRenderSemantic for crate::families::info::InfoDiagramModel {
    fn compatibility_json_controlled(
        &self,
        meta: &PresentationMetadata,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ManagedJson, CompatProjectionError>> {
        self.compatibility_json_with_context_controlled(
            meta,
            &PresentationContext::default(),
            control,
        )
    }

    fn compatibility_json_with_context_controlled(
        &self,
        meta: &PresentationMetadata,
        context: &PresentationContext,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ManagedJson, CompatProjectionError>> {
        control.checkpoint()?;
        let projected = ManagedJson::from(crate::families::info::compat::project(
            self,
            context.info_output_data(),
            &meta.diagram_type,
        ));
        control.checkpoint()?;
        Ok(Ok(projected))
    }
}

#[cfg(feature = "diagram-pie")]
impl sealed::Sealed for crate::families::pie::PieDiagramModel {}
#[cfg(feature = "diagram-pie")]
impl BuiltinRenderSemantic for crate::families::pie::PieDiagramModel {
    fn compatibility_json_controlled(
        &self,
        meta: &PresentationMetadata,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ManagedJson, CompatProjectionError>> {
        self.compatibility_json_with_context_controlled(
            meta,
            &PresentationContext::default(),
            control,
        )
    }

    fn compatibility_json_with_context_controlled(
        &self,
        meta: &PresentationMetadata,
        context: &PresentationContext,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ManagedJson, CompatProjectionError>> {
        control.checkpoint()?;
        let projected = ManagedJson::from(crate::families::pie::compat::project(
            self,
            context.pie_output_data(),
            &meta.diagram_type,
        ));
        control.checkpoint()?;
        Ok(Ok(projected))
    }
}

#[cfg(feature = "diagram-journey")]
impl sealed::Sealed for crate::families::journey::JourneyDiagramModel {}
#[cfg(feature = "diagram-journey")]
impl BuiltinRenderSemantic for crate::families::journey::JourneyDiagramModel {
    fn compatibility_json_controlled(
        &self,
        meta: &PresentationMetadata,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ManagedJson, CompatProjectionError>> {
        self.compatibility_json_with_context_controlled(
            meta,
            &PresentationContext::default(),
            control,
        )
    }

    fn compatibility_json_with_context_controlled(
        &self,
        meta: &PresentationMetadata,
        context: &PresentationContext,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ManagedJson, CompatProjectionError>> {
        control.checkpoint()?;
        let projected = ManagedJson::from(crate::families::journey::compat::project(
            self,
            context.journey_output_data(),
            &meta.diagram_type,
        ));
        control.checkpoint()?;
        Ok(Ok(projected))
    }
}

#[cfg(feature = "diagram-timeline")]
impl sealed::Sealed for crate::families::timeline::TimelineDiagramModel {}
#[cfg(feature = "diagram-timeline")]
impl BuiltinRenderSemantic for crate::families::timeline::TimelineDiagramModel {
    fn compatibility_json_controlled(
        &self,
        meta: &PresentationMetadata,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ManagedJson, CompatProjectionError>> {
        self.compatibility_json_with_context_controlled(
            meta,
            &PresentationContext::default(),
            control,
        )
    }

    fn compatibility_json_with_context_controlled(
        &self,
        meta: &PresentationMetadata,
        context: &PresentationContext,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ManagedJson, CompatProjectionError>> {
        control.checkpoint()?;
        let projected = ManagedJson::from(crate::families::timeline::compat::project(
            self,
            context.timeline_output_data(),
            &meta.diagram_type,
        ));
        control.checkpoint()?;
        Ok(Ok(projected))
    }
}

#[cfg(feature = "diagram-packet")]
impl sealed::Sealed for crate::families::packet::PacketDiagramModel {}
#[cfg(feature = "diagram-packet")]
impl BuiltinRenderSemantic for crate::families::packet::PacketDiagramModel {
    fn compatibility_json_controlled(
        &self,
        meta: &PresentationMetadata,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ManagedJson, CompatProjectionError>> {
        self.compatibility_json_with_context_controlled(
            meta,
            &PresentationContext::default(),
            control,
        )
    }

    fn compatibility_json_with_context_controlled(
        &self,
        meta: &PresentationMetadata,
        context: &PresentationContext,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ManagedJson, CompatProjectionError>> {
        control.checkpoint()?;
        let projected = ManagedJson::from(crate::families::packet::compat::project(
            self,
            context.packet_output_data(),
            &meta.diagram_type,
            meta.effective_config.as_config(),
        ));
        control.checkpoint()?;
        Ok(Ok(projected))
    }
}

#[cfg(feature = "diagram-radar")]
impl sealed::Sealed for crate::families::radar::RadarDiagramModel {}
#[cfg(feature = "diagram-radar")]
impl BuiltinRenderSemantic for crate::families::radar::RadarDiagramModel {
    fn compatibility_json_controlled(
        &self,
        meta: &PresentationMetadata,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ManagedJson, CompatProjectionError>> {
        self.compatibility_json_with_context_controlled(
            meta,
            &PresentationContext::default(),
            control,
        )
    }

    fn compatibility_json_with_context_controlled(
        &self,
        meta: &PresentationMetadata,
        context: &PresentationContext,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ManagedJson, CompatProjectionError>> {
        control.checkpoint()?;
        let projected = ManagedJson::from(crate::families::radar::compat::project(
            self,
            context.radar_output_data(),
            &meta.diagram_type,
            meta.effective_config.as_config(),
        ));
        control.checkpoint()?;
        Ok(Ok(projected))
    }
}

#[cfg(feature = "diagram-mindmap")]
impl sealed::Sealed for crate::families::mindmap::MindmapDiagramRenderModel {}
#[cfg(feature = "diagram-mindmap")]
impl BuiltinRenderSemantic for crate::families::mindmap::MindmapDiagramRenderModel {
    fn compatibility_json_controlled(
        &self,
        meta: &PresentationMetadata,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ManagedJson, CompatProjectionError>> {
        self.compatibility_json_with_context_controlled(
            meta,
            &PresentationContext::default(),
            control,
        )
    }
    fn compatibility_json_with_context_controlled(
        &self,
        meta: &PresentationMetadata,
        context: &PresentationContext,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ManagedJson, CompatProjectionError>> {
        let mut projection = CompatJsonMetadata::from(meta);
        projection.diagram_id = context.mindmap_diagram_id();
        crate::families::mindmap::compat::project_controlled(self, &projection, control)
            .map(|result| result.map(ManagedJson::from))
    }
}

impl RenderSemanticModel {
    /// Projects this family-owned typed model into the public compatibility JSON shape.
    ///
    /// Built-in families delegate to their own lossless projector. This never reparses source;
    /// custom adapters retain their explicitly named JSON boundary. Output facts held outside this
    /// value default to their declared model shape. Use `PresentationDiagram::compatibility_json`
    /// to retain the output shape frozen by source construction.
    pub fn compatibility_json(
        &self,
        meta: &PresentationMetadata,
    ) -> Result<ManagedJson, CompatProjectionError> {
        let control = OperationControl::new();
        self.compatibility_json_controlled(meta, &control)
            .expect("a private operation control cannot be cancelled")
    }

    /// Projects the public compatibility JSON while observing the caller's operation control.
    ///
    /// Output facts held in a parsed context use their manual defaults here. Parsed consumers use
    /// `PresentationDiagram::compatibility_json_controlled` to retain the paired output shape.
    pub fn compatibility_json_controlled(
        &self,
        meta: &PresentationMetadata,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ManagedJson, CompatProjectionError>> {
        self.compatibility_json_with_context_controlled(
            meta,
            &PresentationContext::default(),
            control,
        )
    }

    pub fn compatibility_json_with_context_controlled(
        &self,
        meta: &PresentationMetadata,
        context: &PresentationContext,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ManagedJson, CompatProjectionError>> {
        let _ = context;
        let control = control.for_phase(OperationPhase::Semantic);
        control.checkpoint()?;
        let projected = match self {
            Self::Error(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            Self::CustomJson(model) => model.json().clone_controlled(&control).map(Ok),
            #[cfg(feature = "diagram-mindmap")]
            Self::Mindmap(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-state")]
            Self::State(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-sequence")]
            Self::Sequence(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-zenuml")]
            Self::Zenuml(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(any(
                feature = "diagram-flowchart",
                feature = "diagram-swimlane",
                feature = "diagram-agentflow"
            ))]
            Self::Flowchart(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-architecture")]
            Self::Architecture(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-class")]
            Self::Class(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-c4")]
            Self::C4(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-cynefin")]
            Self::Cynefin(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-railroad")]
            Self::Railroad(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-kanban")]
            Self::Kanban(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-gantt")]
            Self::Gantt(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-pie")]
            Self::Pie(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-packet")]
            Self::Packet(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-timeline")]
            Self::Timeline(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-journey")]
            Self::Journey(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-requirement")]
            Self::Requirement(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-sankey")]
            Self::Sankey(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-radar")]
            Self::Radar(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-info")]
            Self::Info(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-treemap")]
            Self::Treemap(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-block")]
            Self::Block(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-er")]
            Self::Er(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-quadrant-chart")]
            Self::QuadrantChart(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-xychart")]
            Self::XyChart(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-git-graph")]
            Self::GitGraph(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-tree-view")]
            Self::TreeView(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-ishikawa")]
            Self::Ishikawa(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-event-modeling")]
            Self::EventModeling(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-venn")]
            Self::Venn(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-wardley")]
            Self::Wardley(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-usecase")]
            Self::Usecase(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
            #[cfg(feature = "diagram-agentflow")]
            Self::Agentflow(model) => {
                model.compatibility_json_with_context_controlled(meta, context, &control)
            }
        }?;
        control.checkpoint()?;
        Ok(projected)
    }
}

impl PresentationDiagram {
    pub fn compatibility_json(&self) -> Result<ManagedJson, CompatProjectionError> {
        self.compatibility_json_controlled(&OperationControl::new())
            .expect("a private projection control cannot be cancelled")
    }

    pub fn compatibility_json_controlled(
        &self,
        control: &OperationControl,
    ) -> OperationControlResult<Result<ManagedJson, CompatProjectionError>> {
        self.model().compatibility_json_with_context_controlled(
            self.metadata(),
            self.render_context(),
            control,
        )
    }
}
