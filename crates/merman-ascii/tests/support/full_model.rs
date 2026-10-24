use merman_ascii::{AsciiDiagram, AsciiSourceContext};
use merman_core::{PresentationMetadata, RenderSemanticModel};

pub(crate) fn view(model: &RenderSemanticModel) -> AsciiDiagram<'_> {
    match model {
        #[cfg(feature = "diagram-class")]
        RenderSemanticModel::Class(model) => AsciiDiagram::Class(Box::new(model.topology_view())),
        #[cfg(feature = "diagram-er")]
        RenderSemanticModel::Er(model) => AsciiDiagram::Er(model),
        #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
        RenderSemanticModel::Flowchart(model) => AsciiDiagram::Flowchart(model),
        #[cfg(feature = "diagram-gantt")]
        RenderSemanticModel::Gantt(model) => {
            AsciiDiagram::Gantt(merman_core::diagrams::gantt::GanttTerminalView::from_render(model))
        }
        #[cfg(feature = "diagram-git-graph")]
        RenderSemanticModel::GitGraph(model) => AsciiDiagram::GitGraph(model),
        #[cfg(feature = "diagram-journey")]
        RenderSemanticModel::Journey(model) => AsciiDiagram::Journey(&model.semantic),
        #[cfg(feature = "diagram-kanban")]
        RenderSemanticModel::Kanban(model) => AsciiDiagram::Kanban(
            merman_core::diagrams::kanban::KanbanTerminalView::from_render(model),
        ),
        #[cfg(feature = "diagram-mindmap")]
        RenderSemanticModel::Mindmap(model) => AsciiDiagram::Mindmap(
            merman_core::diagrams::mindmap::MindmapTerminalView::from_render(model),
        ),
        #[cfg(feature = "diagram-packet")]
        RenderSemanticModel::Packet(model) => AsciiDiagram::Packet(&model.semantic),
        #[cfg(feature = "diagram-sequence")]
        RenderSemanticModel::Sequence(model) => AsciiDiagram::Sequence(model),
        #[cfg(feature = "diagram-state")]
        RenderSemanticModel::State(model) => {
            AsciiDiagram::State(merman_core::diagrams::state::StateTerminalView::from_render(model))
        }
        #[cfg(feature = "diagram-timeline")]
        RenderSemanticModel::Timeline(model) => AsciiDiagram::Timeline(&model.semantic),
        #[cfg(feature = "diagram-tree-view")]
        RenderSemanticModel::TreeView(model) => AsciiDiagram::TreeView(model),
        #[cfg(feature = "diagram-xychart")]
        RenderSemanticModel::XyChart(model) => AsciiDiagram::XyChart {
            model: &model.semantic,
            display: model.display,
        },
        _ => AsciiDiagram::Unsupported(model.kind()),
    }
}

pub(crate) fn source(metadata: &PresentationMetadata) -> AsciiSourceContext<'_> {
    AsciiSourceContext {
        diagram_type: &metadata.diagram_type,
        config: &metadata.config,
        semantic_config: metadata.effective_config.as_config(),
        title: metadata.title.as_deref(),
    }
}
