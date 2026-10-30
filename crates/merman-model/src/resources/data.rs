//! Model accounting independent of source construction and output execution.

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
use super::FlowchartComplexity;
#[cfg(feature = "diagram-mindmap")]
use super::MindmapComplexity;
use super::ModelComplexity;
#[cfg(feature = "diagram-sequence")]
use super::SequenceComplexity;
#[cfg(feature = "diagram-treemap")]
use super::TreemapComplexity;
#[cfg(feature = "diagram-zenuml")]
use super::ZenumlComplexity;
#[cfg(feature = "diagram-class")]
use super::class_namespace_depth;
#[cfg(feature = "diagram-block")]
use super::model_complexity_from_block;
#[cfg(feature = "diagram-kanban")]
use super::model_complexity_from_kanban_semantic;
#[cfg(feature = "diagram-ishikawa")]
use crate::families::ishikawa::IshikawaDiagramModel;
#[cfg(feature = "diagram-radar")]
use crate::families::radar::RadarDiagramModel;

pub fn model_complexity_from_semantic_model(
    model: &crate::SemanticDiagramModel,
) -> ModelComplexity {
    match *model {
        crate::SemanticDiagramModel::Error(ref model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-agentflow")]
        crate::SemanticDiagramModel::Agentflow(ref model) => {
            ModelComplexity::from_serializable(model)
        }
        #[cfg(feature = "diagram-git-graph")]
        crate::SemanticDiagramModel::GitGraph(ref model) => {
            ModelComplexity::from_serializable(model)
        }
        #[cfg(feature = "diagram-class")]
        crate::SemanticDiagramModel::Class(ref model) => {
            let mut complexity = ModelComplexity::from_serializable(model);
            complexity.nesting_depth = complexity
                .nesting_depth
                .max(class_namespace_depth(&model.namespaces));
            complexity
        }
        #[cfg(any(
            feature = "diagram-flowchart",
            feature = "diagram-swimlane",
            feature = "diagram-agentflow"
        ))]
        crate::SemanticDiagramModel::Flowchart(ref model) => {
            FlowchartComplexity::from_model(model).as_model_complexity()
        }
        #[cfg(feature = "diagram-sequence")]
        crate::SemanticDiagramModel::Sequence(ref model) => {
            SequenceComplexity::from_model(model).as_model_complexity()
        }
        #[cfg(feature = "diagram-state")]
        crate::SemanticDiagramModel::State(ref model) => model.model_complexity(),
        #[cfg(feature = "diagram-cynefin")]
        crate::SemanticDiagramModel::Cynefin(ref model) => {
            ModelComplexity::from_serializable(model)
        }
        #[cfg(feature = "diagram-info")]
        crate::SemanticDiagramModel::Info(ref model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-pie")]
        crate::SemanticDiagramModel::Pie(ref model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-packet")]
        crate::SemanticDiagramModel::Packet(ref model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-tree-view")]
        crate::SemanticDiagramModel::TreeView(ref model) => {
            ModelComplexity::from_serializable(model)
        }
        #[cfg(feature = "diagram-quadrant-chart")]
        crate::SemanticDiagramModel::QuadrantChart(ref model) => {
            ModelComplexity::from_serializable(model)
        }
        #[cfg(feature = "diagram-radar")]
        crate::SemanticDiagramModel::Radar(ref model) => model_complexity_from_radar(model),
        #[cfg(feature = "diagram-sankey")]
        crate::SemanticDiagramModel::Sankey(ref model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-venn")]
        crate::SemanticDiagramModel::Venn(ref model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-er")]
        crate::SemanticDiagramModel::Er(ref model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-architecture")]
        crate::SemanticDiagramModel::Architecture(ref model) => {
            ModelComplexity::from_serializable(model)
        }
        #[cfg(feature = "diagram-gantt")]
        crate::SemanticDiagramModel::Gantt(ref model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-c4")]
        crate::SemanticDiagramModel::C4(ref model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-block")]
        crate::SemanticDiagramModel::Block(ref model) => model_complexity_from_block(model),
        #[cfg(feature = "diagram-kanban")]
        crate::SemanticDiagramModel::Kanban(ref model) => {
            model_complexity_from_kanban_semantic(model)
        }
        #[cfg(feature = "diagram-mindmap")]
        crate::SemanticDiagramModel::Mindmap(ref model) => {
            MindmapComplexity::from_semantic_model(model).as_model_complexity()
        }
        #[cfg(feature = "diagram-xychart")]
        crate::SemanticDiagramModel::XyChart(ref model) => {
            ModelComplexity::from_serializable(model)
        }
        #[cfg(feature = "diagram-ishikawa")]
        crate::SemanticDiagramModel::Ishikawa(ref model) => {
            IshikawaComplexity::from_model(model).as_model_complexity()
        }
        #[cfg(feature = "diagram-treemap")]
        crate::SemanticDiagramModel::Treemap(ref model) => {
            TreemapComplexity::from_model(model).as_model_complexity()
        }
        #[cfg(feature = "diagram-requirement")]
        crate::SemanticDiagramModel::Requirement(ref model) => {
            ModelComplexity::from_requirement(model)
        }
        #[cfg(feature = "diagram-usecase")]
        crate::SemanticDiagramModel::Usecase(ref model) => {
            ModelComplexity::from_serializable(model)
        }
        #[cfg(feature = "diagram-event-modeling")]
        crate::SemanticDiagramModel::EventModeling(ref model) => {
            ModelComplexity::from_serializable(model)
        }
        #[cfg(feature = "diagram-wardley")]
        crate::SemanticDiagramModel::Wardley(ref model) => {
            ModelComplexity::from_serializable(model)
        }
        #[cfg(feature = "diagram-railroad")]
        crate::SemanticDiagramModel::Railroad(ref model) => {
            ModelComplexity::from_serializable(model)
        }
        #[cfg(feature = "diagram-zenuml")]
        crate::SemanticDiagramModel::Zenuml(ref model) => {
            ZenumlComplexity::from_model(model).as_model_complexity()
        }
        #[cfg(feature = "diagram-journey")]
        crate::SemanticDiagramModel::Journey(ref model) => {
            ModelComplexity::from_serializable(model)
        }
        #[cfg(feature = "diagram-timeline")]
        crate::SemanticDiagramModel::Timeline(ref model) => {
            ModelComplexity::from_serializable(model)
        }
    }
}

#[cfg(feature = "diagram-radar")]
pub fn model_complexity_from_radar(model: &RadarDiagramModel) -> ModelComplexity {
    let common_text_bytes = [
        model.title.as_deref(),
        model.acc_title.as_deref(),
        model.acc_descr.as_deref(),
        Some(model.options.graticule.as_str()),
    ]
    .into_iter()
    .flatten()
    .fold(0usize, |total, value| total.saturating_add(value.len()));
    let axis_text_bytes = model.axes.iter().fold(0usize, |total, axis| {
        total
            .saturating_add(axis.name.len())
            .saturating_add(axis.label.len())
    });
    let (curve_items, curve_text_bytes) =
        model
            .curves
            .iter()
            .fold((0usize, 0usize), |(items, text_bytes), curve| {
                (
                    items.saturating_add(curve.entries.len()),
                    text_bytes
                        .saturating_add(curve.name.len())
                        .saturating_add(curve.label.len()),
                )
            });
    ModelComplexity::new(
        model
            .axes
            .len()
            .saturating_add(model.curves.len())
            .saturating_add(curve_items),
        common_text_bytes
            .saturating_add(axis_text_bytes)
            .saturating_add(curve_text_bytes),
        0,
    )
}

/// Computes Ishikawa model complexity iteratively without growing the stack with cause depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(feature = "diagram-ishikawa")]
pub struct IshikawaComplexity {
    pub nodes: usize,
    pub label_bytes: usize,
    pub nesting_depth: usize,
}

#[cfg(feature = "diagram-ishikawa")]
impl IshikawaComplexity {
    pub fn from_model(model: &IshikawaDiagramModel) -> Self {
        let mut complexity = Self {
            nodes: 0,
            label_bytes: [
                model.acc_title.as_deref(),
                model.acc_descr.as_deref(),
                model.title.as_deref(),
            ]
            .into_iter()
            .flatten()
            .fold(0usize, |total, value| total.saturating_add(value.len())),
            nesting_depth: 0,
        };
        let Some(root) = model.root.and_then(|id| model.nodes.get(id)) else {
            return complexity;
        };

        let mut pending = vec![(root, 0usize)];
        let mut visited = vec![false; model.nodes.len()];
        if let Some(root) = model.root {
            visited[root] = true;
        }
        while let Some((node, depth)) = pending.pop() {
            complexity.nodes = complexity.nodes.saturating_add(1);
            complexity.label_bytes = complexity.label_bytes.saturating_add(node.text.len());
            complexity.nesting_depth = complexity.nesting_depth.max(depth);
            for child in node.children.iter().rev() {
                if let Some(record) = model.nodes.get(*child)
                    && !visited[*child]
                {
                    visited[*child] = true;
                    pending.push((record, depth.saturating_add(1)));
                }
            }
        }
        complexity
    }

    pub fn as_model_complexity(self) -> ModelComplexity {
        ModelComplexity::new(self.nodes, self.label_bytes, self.nesting_depth)
    }
}
