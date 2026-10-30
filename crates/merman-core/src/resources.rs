//! Backend-independent Mermaid source and semantic-model resource policy.

#[cfg(feature = "presentation")]
use crate::diagram::{ParsedDiagramRender, RenderSemanticModel};
#[cfg(feature = "diagram-ishikawa")]
use crate::diagrams::ishikawa::IshikawaDiagramModel;
#[cfg(feature = "diagram-kanban")]
use crate::diagrams::kanban::KanbanDiagramModel;
#[cfg(all(feature = "diagram-kanban", feature = "presentation"))]
use crate::diagrams::kanban::KanbanDiagramRenderModel;
#[cfg(feature = "diagram-radar")]
use crate::diagrams::radar::RadarDiagramModel;
#[cfg(all(test, feature = "diagram-requirement"))]
use crate::diagrams::requirement::RequirementDiagramModel;
#[cfg(feature = "diagram-treemap")]
use crate::diagrams::treemap::TreemapDiagramModel;

#[cfg(feature = "diagram-mindmap")]
pub use merman_model::resources::MindmapComplexity;

// Temporary resource forwarding while source and output consumers migrate.
pub use merman_model::resources::{
    CLI_DEFAULT_RESOURCE_PROFILE, GENERAL_BINDING_DEFAULT_RESOURCE_PROFILE,
    INPUT_RESOURCE_LIMIT_COUNT, INPUT_RESOURCE_LIMIT_DESCRIPTORS, InputResourceLimitDescriptor,
    InputResourceLimitExceeded, InputResourceLimitId, InputResourceLimitOverride,
    InputResourceLimitOverrideError, InputResourceLimitPhase, InputResourcePolicy,
    InputResourcePolicyRestrictionError, RESOURCE_PROFILE_COUNT, RESOURCE_PROFILE_DESCRIPTORS,
    ResourceProfile, ResourceProfileDescriptor,
};

pub use merman_model::resources::ModelComplexity;
#[cfg(any(feature = "diagram-kanban", feature = "diagram-treemap"))]
use merman_model::resources::json_value_complexity;

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub use merman_model::resources::FlowchartComplexity;
#[cfg(feature = "diagram-sequence")]
pub use merman_model::resources::SequenceComplexity;
#[cfg(feature = "diagram-zenuml")]
pub use merman_model::resources::ZenumlComplexity;
#[cfg(feature = "diagram-class")]
pub use merman_model::resources::{ClassComplexity, class_namespace_depth};

/// Counts canonical semantics and their paired parser-owned rendering text.
#[cfg(feature = "presentation")]
pub fn model_complexity_from_parsed_render(parsed: &ParsedDiagramRender) -> ModelComplexity {
    let mut complexity = model_complexity_from_render_model(parsed.model());
    complexity.text_bytes = complexity
        .text_bytes
        .saturating_add(parsed.retained_render_context_bytes());
    complexity
}

#[cfg(feature = "presentation")]
pub fn model_complexity_from_render_model(model: &RenderSemanticModel) -> ModelComplexity {
    match model {
        RenderSemanticModel::Error(model) => ModelComplexity::from_serializable(model),
        RenderSemanticModel::CustomJson(model) => {
            let mut complexity = ModelComplexity::from_json(model.value());
            complexity.text_bytes = complexity
                .text_bytes
                .saturating_add(model.model_name().len());
            complexity
        }
        #[cfg(feature = "diagram-mindmap")]
        RenderSemanticModel::Mindmap(model) => {
            MindmapComplexity::from_model(model).as_model_complexity()
        }
        #[cfg(feature = "diagram-state")]
        RenderSemanticModel::State(model) => model.model_complexity(),
        #[cfg(feature = "diagram-sequence")]
        RenderSemanticModel::Sequence(model) => {
            SequenceComplexity::from_model(model).as_model_complexity()
        }
        #[cfg(feature = "diagram-zenuml")]
        RenderSemanticModel::Zenuml(model) => {
            ZenumlComplexity::from_model(model).as_model_complexity()
        }
        #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
        RenderSemanticModel::Flowchart(model) => {
            FlowchartComplexity::from_model(model).as_model_complexity()
        }
        #[cfg(feature = "diagram-architecture")]
        RenderSemanticModel::Architecture(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-class")]
        RenderSemanticModel::Class(model) => {
            let mut complexity = ClassComplexity::topology_from_model(model).as_model_complexity();
            complexity.text_bytes = ModelComplexity::from_serializable(model.model())
                .text_bytes
                .saturating_add(model.presentation().retained_bytes());
            complexity
        }
        #[cfg(feature = "diagram-c4")]
        RenderSemanticModel::C4(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-cynefin")]
        RenderSemanticModel::Cynefin(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-railroad")]
        RenderSemanticModel::Railroad(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-kanban")]
        RenderSemanticModel::Kanban(model) => model_complexity_from_kanban(model),
        #[cfg(feature = "diagram-gantt")]
        RenderSemanticModel::Gantt(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-pie")]
        RenderSemanticModel::Pie(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-packet")]
        RenderSemanticModel::Packet(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-timeline")]
        RenderSemanticModel::Timeline(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-journey")]
        RenderSemanticModel::Journey(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-requirement")]
        RenderSemanticModel::Requirement(model) => ModelComplexity::from_requirement(model),
        #[cfg(feature = "diagram-sankey")]
        RenderSemanticModel::Sankey(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-radar")]
        RenderSemanticModel::Radar(model) => model_complexity_from_radar(model.as_semantic_model()),
        #[cfg(feature = "diagram-info")]
        RenderSemanticModel::Info(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-treemap")]
        RenderSemanticModel::Treemap(model) => {
            TreemapComplexity::from_render_model(model).as_model_complexity()
        }
        #[cfg(feature = "diagram-block")]
        RenderSemanticModel::Block(model) => model.model_complexity(),
        #[cfg(feature = "diagram-er")]
        RenderSemanticModel::Er(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-quadrant-chart")]
        RenderSemanticModel::QuadrantChart(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-xychart")]
        RenderSemanticModel::XyChart(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-git-graph")]
        RenderSemanticModel::GitGraph(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-tree-view")]
        RenderSemanticModel::TreeView(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-ishikawa")]
        RenderSemanticModel::Ishikawa(model) => {
            IshikawaComplexity::from_model(model).as_model_complexity()
        }
        #[cfg(feature = "diagram-event-modeling")]
        RenderSemanticModel::EventModeling(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-venn")]
        RenderSemanticModel::Venn(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-wardley")]
        RenderSemanticModel::Wardley(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-usecase")]
        RenderSemanticModel::Usecase(model) => ModelComplexity::from_serializable(model),
        #[cfg(feature = "diagram-agentflow")]
        RenderSemanticModel::Agentflow(model) => ModelComplexity::from_serializable(model),
    }
}

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
        #[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
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

#[cfg(feature = "diagram-block")]
pub fn model_complexity_from_block(
    model: &crate::diagrams::block::BlockDiagramModel,
) -> ModelComplexity {
    let mut complexity = ModelComplexity::from_serializable(model);
    let mut pending = model
        .root
        .map(|root| vec![(root, 0usize)])
        .unwrap_or_default();
    let mut visited = vec![false; model.blocks_flat.len()];
    while let Some((index, depth)) = pending.pop() {
        let Some(node) = model.blocks_flat.get(index) else {
            continue;
        };
        if std::mem::replace(&mut visited[index], true) {
            continue;
        }
        complexity.nesting_depth = complexity.nesting_depth.max(depth);
        pending.extend(
            node.children
                .iter()
                .map(|child| (*child, depth.saturating_add(1))),
        );
    }
    complexity
}

#[cfg(feature = "diagram-kanban")]
pub fn model_complexity_from_kanban_semantic(model: &KanbanDiagramModel) -> ModelComplexity {
    let mut complexity = ModelComplexity::new(
        model.nodes.len(),
        0,
        kanban_nesting_depth(
            model
                .nodes
                .iter()
                .map(|node| (node.id.as_str(), node.parent_id.as_deref())),
        ),
    );
    for node in &model.nodes {
        for text in [
            Some(node.id.as_str()),
            Some(node.label.as_str()),
            node.parent_id.as_deref(),
            node.ticket.as_deref(),
            node.priority.as_deref(),
            node.assigned.as_deref(),
            node.icon.as_deref(),
            node.css_classes.as_deref(),
            node.shape.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            complexity.text_bytes = complexity.text_bytes.saturating_add(text.len());
        }
        if let Some(metadata) = &node.metadata {
            let metadata = json_value_complexity(metadata.as_value(), 0);
            complexity.items = complexity.items.saturating_add(metadata.items);
            complexity.text_bytes = complexity.text_bytes.saturating_add(metadata.text_bytes);
            complexity.nesting_depth = complexity.nesting_depth.max(metadata.nesting_depth);
        }
    }
    complexity
}

#[cfg(all(feature = "diagram-kanban", feature = "presentation"))]
pub fn model_complexity_from_kanban(model: &KanbanDiagramRenderModel) -> ModelComplexity {
    let text_bytes = model.nodes.iter().fold(0usize, |total, node| {
        [
            Some(node.id.as_str()),
            Some(node.label.as_str()),
            node.parent_id.as_deref(),
            node.ticket.as_deref(),
            node.priority.as_deref(),
            node.assigned.as_deref(),
            node.icon.as_deref(),
        ]
        .into_iter()
        .flatten()
        .fold(total, |subtotal, value| {
            subtotal.saturating_add(value.len())
        })
    });
    let nesting_depth = kanban_nesting_depth(
        model
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node.parent_id.as_deref())),
    );
    ModelComplexity::new(model.nodes.len(), text_bytes, nesting_depth)
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

/// Computes Treemap model complexity iteratively without serializing a user-controlled node tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(feature = "diagram-treemap")]
pub struct TreemapComplexity {
    pub nodes: usize,
    pub classes: usize,
    pub label_bytes: usize,
    pub nesting_depth: usize,
}

#[cfg(feature = "diagram-treemap")]
impl TreemapComplexity {
    pub fn from_model(model: &TreemapDiagramModel) -> Self {
        Self::from_model_with_styles(model, |_| None)
    }

    #[cfg(feature = "presentation")]
    pub fn from_render_model(model: &crate::diagrams::treemap::TreemapDiagramRenderModel) -> Self {
        Self::from_model_with_styles(&model.semantic, |index| model.compiled_styles(index))
    }

    fn from_model_with_styles<'a>(
        model: &'a TreemapDiagramModel,
        styles: impl Fn(Option<usize>) -> Option<&'a [String]>,
    ) -> Self {
        let mut nodes = 0usize;
        let mut classes = 0usize;
        let mut label_bytes = [
            model.acc_title.as_deref(),
            model.acc_descr.as_deref(),
            model.title.as_deref(),
        ]
        .into_iter()
        .flatten()
        .fold(0usize, |total, value| total.saturating_add(value.len()));
        let mut nesting_depth = 0usize;

        let mut pending = vec![(&model.root, None, 0usize)];
        let mut visited = vec![false; model.nodes.len()];
        while let Some((node, index, depth)) = pending.pop() {
            nodes = nodes.saturating_add(1);
            nesting_depth = nesting_depth.max(depth);
            label_bytes = label_bytes
                .saturating_add(node.name.len())
                .saturating_add(node.class_selector.as_deref().map_or(0, str::len))
                .saturating_add(
                    styles(index)
                        .unwrap_or(&[])
                        .iter()
                        .map(String::len)
                        .sum::<usize>(),
                );
            if let Some(value) = node.value.as_ref() {
                let value = json_value_complexity(value.as_value(), depth);
                label_bytes = label_bytes.saturating_add(value.text_bytes);
                nodes = nodes.saturating_add(value.items);
                nesting_depth = nesting_depth.max(value.nesting_depth);
            }
            if let Some(children) = node.children.as_ref() {
                for child in children.iter().rev() {
                    if let Some(record) = model.nodes.get(*child)
                        && !visited[*child]
                    {
                        visited[*child] = true;
                        pending.push((record, Some(*child), depth.saturating_add(1)));
                    }
                }
            }
        }

        for (class_name, class_def) in &model.classes {
            classes = classes.saturating_add(1);
            label_bytes = label_bytes
                .saturating_add(class_name.len())
                .saturating_add(class_def.id.len())
                .saturating_add(class_def.styles.iter().map(String::len).sum::<usize>())
                .saturating_add(class_def.text_styles.iter().map(String::len).sum::<usize>());
        }

        Self {
            nodes,
            classes,
            label_bytes,
            nesting_depth,
        }
    }

    pub fn as_model_complexity(self) -> ModelComplexity {
        ModelComplexity::new(
            self.nodes.saturating_add(self.classes),
            self.label_bytes,
            self.nesting_depth,
        )
    }
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

#[cfg(feature = "diagram-kanban")]
fn kanban_nesting_depth<'a>(
    nodes: impl Iterator<Item = (&'a str, Option<&'a str>)> + Clone,
) -> usize {
    let indices = nodes
        .clone()
        .enumerate()
        .map(|(index, (id, _))| (id, index))
        .collect::<std::collections::HashMap<_, _>>();
    let parents = nodes
        .map(|(_, parent)| parent.and_then(|parent| indices.get(parent).copied()))
        .collect::<Vec<_>>();
    let mut depths = vec![0usize; parents.len()];
    let mut completed = vec![false; parents.len()];
    for start in 0..parents.len() {
        if completed[start] {
            continue;
        }
        let mut path = Vec::new();
        let mut positions = std::collections::HashMap::new();
        let mut cursor = Some(start);
        while let Some(index) = cursor {
            if completed[index] || positions.contains_key(&index) {
                break;
            }
            positions.insert(index, path.len());
            path.push(index);
            cursor = parents[index];
        }
        let mut depth = cursor
            .filter(|index| completed[*index])
            .map(|index| depths[index])
            .unwrap_or(0);
        for index in path.into_iter().rev() {
            depth = depth.saturating_add(1);
            depths[index] = depth;
            completed[index] = true;
        }
    }
    depths.into_iter().max().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iterative_json_complexity_matches_existing_serialized_shape() {
        for value in [
            serde_json::json!(null),
            serde_json::json!([]),
            serde_json::json!({"empty": [], "nested": {"name": ["value", {}]}}),
            serde_json::json!([1, "text", [[], {}]]),
        ] {
            assert_eq!(
                ModelComplexity::from_json(&value),
                ModelComplexity::from_serializable(&value),
            );
        }
    }

    #[test]
    #[cfg(feature = "compat-json")]
    fn generic_complexity_rejects_managed_json_depth_without_panicking() {
        let mut value = serde_json::Value::Null;
        for _ in 0..129 {
            value = serde_json::Value::Array(vec![value]);
        }
        let managed = crate::ManagedSemanticJson::from_value(value);
        assert_eq!(
            ModelComplexity::from_serializable(&managed),
            ModelComplexity::new(usize::MAX, usize::MAX, usize::MAX)
        );
    }

    #[test]
    #[cfg(feature = "presentation")]
    fn custom_json_complexity_preserves_deep_payload_and_model_name() {
        let mut value = serde_json::json!("leaf");
        for _ in 0..5_000 {
            value = serde_json::Value::Array(vec![value]);
        }
        let model =
            RenderSemanticModel::CustomJson(crate::CustomJsonRenderModel::new("custom", value));
        assert_eq!(
            model_complexity_from_render_model(&model),
            ModelComplexity::new(5_000, 10, 4_999),
        );
        drop(model);
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn constrained_policy_rejects_flowchart_cardinality() {
        let source = "flowchart TD\nA --> B";
        let parsed = crate::SemanticParser::new()
            .parse_diagram_sync(source)
            .unwrap();
        let policy = InputResourcePolicy::for_profile(ResourceProfile::Constrained)
            .with_limit(InputResourceLimitId::MaxModelItems, 2)
            .unwrap();
        let model_error = policy
            .check_model_complexity(model_complexity_from_semantic_model(&parsed.model))
            .unwrap_err();
        assert_eq!(model_error.limit, "max_model_items");

        #[cfg(feature = "presentation")]
        {
            let parsed = crate::MermaidEngine::new()
                .parse_diagram_for_render_model_sync(source, crate::ParseOptions::strict())
                .unwrap()
                .unwrap();
            assert_eq!(
                policy
                    .check_model_complexity(model_complexity_from_render_model(parsed.model()))
                    .unwrap_err(),
                model_error
            );
        }
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn flowchart_subgraph_vertex_css_is_charged_once_by_its_canonical_owner() {
        let source =
            "flowchart TD\nsubgraph G\n A\nend\nclass G base\nstyle G fill:#f00\nclass G hot\n";
        let parsed = crate::SemanticParser::new()
            .parse_diagram_sync(source)
            .unwrap();
        let crate::SemanticDiagramModel::Flowchart(model) = &parsed.model else {
            panic!("expected Flowchart model");
        };
        let complexity = FlowchartComplexity::from_model(model).as_model_complexity();
        let mut without_vertex_css = model.clone();
        without_vertex_css.subgraph_vertex_styles = Default::default();
        assert_eq!(
            complexity.text_bytes,
            FlowchartComplexity::from_model(&without_vertex_css)
                .as_model_complexity()
                .text_bytes
                + "G".len()
                + "hot".len()
                + "fill:#f00".len(),
        );
        let exact = InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(
                InputResourceLimitId::MaxModelTextBytes,
                complexity.text_bytes,
            )
            .unwrap();
        exact
            .check_model_complexity(model_complexity_from_semantic_model(&parsed.model))
            .unwrap();
        let too_small = InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(
                InputResourceLimitId::MaxModelTextBytes,
                complexity.text_bytes - 1,
            )
            .unwrap();
        let error = too_small
            .check_model_complexity(model_complexity_from_semantic_model(&parsed.model))
            .unwrap_err();
        assert_eq!(error.limit, "max_model_text_bytes");
        assert_eq!(error.actual, complexity.text_bytes);

        #[cfg(feature = "presentation")]
        {
            let full = crate::MermaidEngine::new()
                .parse_diagram_for_render_model_sync(source, crate::ParseOptions::strict())
                .unwrap()
                .unwrap();
            assert_eq!(model_complexity_from_render_model(full.model()), complexity);
            assert_eq!(
                full.retained_render_context_bytes(),
                "G".len() + std::mem::size_of::<usize>(),
                "the presentation context retains only the group color ordinal, not another CSS owner",
            );
            assert_eq!(
                model_complexity_from_parsed_render(&full).text_bytes,
                complexity.text_bytes + full.retained_render_context_bytes(),
            );
        }
    }

    #[cfg(all(feature = "diagram-flowchart", feature = "presentation"))]
    #[test]
    fn parsed_render_resource_checks_include_parser_owned_flowchart_label_sources() {
        let parsed = crate::MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "flowchart LR\nA[\"&nbsp;entity&nbsp;\"] --> B\n",
                crate::ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let model_complexity = model_complexity_from_render_model(parsed.model());
        assert!(parsed.retained_render_context_bytes() > 0);

        let policy = InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(
                InputResourceLimitId::MaxModelTextBytes,
                model_complexity.text_bytes,
            )
            .unwrap();
        policy
            .check_model_complexity(model_complexity_from_render_model(parsed.model()))
            .expect("the public model alone fits its exact text budget");
        let error = policy
            .check_model_complexity(model_complexity_from_parsed_render(&parsed))
            .unwrap_err();
        assert_eq!(error.limit, "max_model_text_bytes");
        assert!(error.actual > model_complexity.text_bytes);
    }

    #[cfg(feature = "diagram-sequence")]
    #[test]
    fn sequence_complexity_bounds_messages_and_nested_frames() {
        let source = "sequenceDiagram\nloop outer\nrect rgb(240,240,240)\nA->>B: hi\nend\nend";
        let parsed = crate::SemanticParser::new()
            .parse_diagram_sync(source)
            .unwrap();
        let crate::SemanticDiagramModel::Sequence(model) = &parsed.model else {
            panic!("expected Sequence model");
        };
        let complexity = SequenceComplexity::from_model(model);
        assert_eq!(complexity.block_depth, 2);
        assert_eq!(complexity.messages, model.messages.len());

        let policy = InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(InputResourceLimitId::MaxModelNestingDepth, 1)
            .unwrap();
        let error = policy
            .check_model_complexity(model_complexity_from_semantic_model(&parsed.model))
            .unwrap_err();
        assert_eq!(error.limit, "max_model_nesting_depth");
        assert_eq!(error.actual, 2);
        assert_eq!(error.max, 1);

        #[cfg(feature = "presentation")]
        {
            let parsed = crate::MermaidEngine::new()
                .parse_diagram_for_render_model_sync(source, crate::ParseOptions::strict())
                .unwrap()
                .unwrap();
            assert_eq!(
                policy
                    .check_model_complexity(model_complexity_from_render_model(parsed.model()))
                    .unwrap_err(),
                error
            );
        }
    }

    #[cfg(feature = "diagram-sequence")]
    #[test]
    fn sequence_complexity_counts_arbitrary_json_items_and_depth_iteratively() {
        let mut parsed = crate::SemanticParser::new()
            .parse_diagram_sync("sequenceDiagram\nparticipant A")
            .unwrap();
        let crate::SemanticDiagramModel::Sequence(model) = &mut parsed.model else {
            panic!("expected Sequence model");
        };
        let mut nested = serde_json::Value::Null;
        for _ in 0..300 {
            nested = serde_json::Value::Array(vec![nested]);
        }
        model
            .actors
            .get_mut("A")
            .unwrap()
            .properties
            .insert("payload".to_string(), nested);

        let complexity = SequenceComplexity::from_model(model);
        assert_eq!(complexity.nesting_depth, 304);
        assert_eq!(complexity.items, 302);

        let exact = InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(
                InputResourceLimitId::MaxModelNestingDepth,
                complexity.nesting_depth,
            )
            .unwrap();
        exact
            .check_model_complexity(model_complexity_from_semantic_model(&parsed.model))
            .expect("the exact JSON depth boundary should pass");

        let below = InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(
                InputResourceLimitId::MaxModelNestingDepth,
                complexity.nesting_depth - 1,
            )
            .unwrap();
        let error = below
            .check_model_complexity(model_complexity_from_semantic_model(&parsed.model))
            .expect_err("the N-1 JSON depth boundary should fail");
        assert_eq!(error.limit, "max_model_nesting_depth");
        assert_eq!(error.actual, complexity.nesting_depth);
        assert_eq!(error.max, complexity.nesting_depth - 1);

        #[cfg(feature = "presentation")]
        {
            let crate::SemanticDiagramModel::Sequence(model) = parsed.model else {
                panic!("expected Sequence model");
            };
            let render_model = RenderSemanticModel::Sequence(model);
            exact
                .check_model_complexity(model_complexity_from_render_model(&render_model))
                .expect("the exact JSON depth boundary should pass");
            assert_eq!(
                below
                    .check_model_complexity(model_complexity_from_render_model(&render_model))
                    .unwrap_err(),
                error
            );
        }
    }

    #[cfg(feature = "diagram-state")]
    #[test]
    fn state_complexity_bounds_semantic_nesting_in_flat_documents() {
        for depth in [32, 130] {
            let mut source = String::from("stateDiagram-v2\n");
            for index in 0..depth {
                source.push_str(&format!("state S{index} {{\n"));
            }
            source.push_str("Leaf\n");
            for _ in 0..depth {
                source.push_str("}\n");
            }
            let parsed = crate::SemanticParser::new()
                .parse_diagram_sync(&source)
                .unwrap();
            let complexity = model_complexity_from_semantic_model(&parsed.model);
            assert_eq!(complexity.nesting_depth, depth);
            let exact = InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
                .with_limit(InputResourceLimitId::MaxModelNestingDepth, depth)
                .unwrap();
            exact
                .check_model_complexity(model_complexity_from_semantic_model(&parsed.model))
                .unwrap();
            let below = InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
                .with_limit(InputResourceLimitId::MaxModelNestingDepth, depth - 1)
                .unwrap();
            let error = below
                .check_model_complexity(model_complexity_from_semantic_model(&parsed.model))
                .unwrap_err();
            assert_eq!(error.limit, "max_model_nesting_depth");
            assert_eq!(error.actual, depth);
            assert_eq!(error.max, depth - 1);

            #[cfg(feature = "presentation")]
            {
                let full = crate::MermaidEngine::new()
                    .parse_diagram_for_render_model_sync(&source, crate::ParseOptions::strict())
                    .unwrap()
                    .unwrap();
                assert_eq!(
                    below
                        .check_model_complexity(model_complexity_from_render_model(full.model()))
                        .unwrap_err()
                        .limit,
                    "max_model_nesting_depth"
                );
            }
        }
    }

    #[cfg(all(feature = "diagram-mindmap", feature = "presentation"))]
    #[test]
    fn mindmap_model_limits_cover_pre_layout_cardinality_and_labels() {
        let constrained = InputResourcePolicy::for_profile(ResourceProfile::Constrained);
        assert_eq!(
            constrained.value(InputResourceLimitId::MaxModelItems),
            Some(16_000)
        );

        let parsed = crate::MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "mindmap\n  Root\n    First child\n    Second child\n",
                crate::ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let RenderSemanticModel::Mindmap(model) = parsed.model() else {
            panic!("expected Mindmap model");
        };
        let complexity = MindmapComplexity::from_model(model);
        assert_eq!(complexity.nodes, 3);
        assert_eq!(complexity.edges, 2);
        assert!(complexity.label_bytes >= "RootFirst childSecond child".len());

        for (id, max, expected_limit) in [
            (InputResourceLimitId::MaxModelItems, 4, "max_model_items"),
            (
                InputResourceLimitId::MaxModelTextBytes,
                1,
                "max_model_text_bytes",
            ),
        ] {
            let error = InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
                .with_limit(id, max)
                .unwrap()
                .check_model_complexity(model_complexity_from_render_model(parsed.model()))
                .unwrap_err();
            assert_eq!(error.phase, InputResourceLimitPhase::Model);
            assert_eq!(error.limit, expected_limit);
        }
    }

    #[cfg(all(
        feature = "presentation",
        any(
            feature = "diagram-kanban",
            feature = "diagram-radar",
            feature = "diagram-requirement"
        )
    ))]
    #[test]
    fn generic_model_budget_covers_new_family_models_without_family_knobs() {
        let models = [
            #[cfg(feature = "diagram-kanban")]
            RenderSemanticModel::Kanban(KanbanDiagramRenderModel {
                nodes: vec![crate::diagrams::kanban::KanbanRenderNode::new(
                    "todo", "Todo",
                )],
            }),
            #[cfg(feature = "diagram-radar")]
            RenderSemanticModel::Radar({
                let mut model = crate::diagrams::radar::RadarDiagramRenderModel::default();
                model.axes = vec![crate::diagrams::radar::RadarAxis {
                    name: "speed".to_string(),
                    label: "Speed".to_string(),
                }];
                model
            }),
            #[cfg(feature = "diagram-requirement")]
            RenderSemanticModel::Requirement(RequirementDiagramModel {
                direction: "TB".to_string(),
                requirements: vec![crate::diagrams::requirement::RequirementNode {
                    name: "R1".to_string(),
                    node_type: "requirement".to_string(),
                    requirement_id: "1".to_string(),
                    text: "Must render".to_string(),
                    risk: "Low".to_string(),
                    verify_method: "Test".to_string(),
                    css_styles: Vec::new(),
                    classes: Vec::new(),
                }],
                acc_title: None,
                acc_descr: None,
                elements: Vec::new(),
                relationships: Vec::new(),
                classes: Default::default(),
            }),
        ];
        for model in models {
            let error = InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
                .with_limit(InputResourceLimitId::MaxModelTextBytes, 1)
                .unwrap()
                .check_model_complexity(model_complexity_from_render_model(&model))
                .unwrap_err();
            assert_eq!(error.limit, "max_model_text_bytes");
        }
    }

    #[cfg(all(feature = "diagram-kanban", feature = "presentation"))]
    #[test]
    fn kanban_complexity_accounts_for_parent_chain_depth_without_quadratic_scans() {
        let root = KanbanDiagramRenderModel {
            nodes: ["section", "lane", "card"]
                .into_iter()
                .enumerate()
                .map(|(index, id)| {
                    let mut node =
                        crate::diagrams::kanban::KanbanRenderNode::new(id, format!("Node {id}"));
                    if index > 0 {
                        node.parent_id = Some(["section", "lane"][index - 1].to_string());
                    }
                    node
                })
                .collect(),
        };

        assert_eq!(model_complexity_from_kanban(&root).nesting_depth, 3);
    }

    #[cfg(all(feature = "diagram-gantt", feature = "presentation"))]
    #[test]
    fn gantt_full_wrappers_preserve_static_field_resource_accounting() {
        use crate::diagrams::gantt::{
            GanttDiagramModel, GanttDiagramRenderModel, GanttRenderTask, GanttTask,
        };

        let empty = RenderSemanticModel::Gantt(GanttDiagramRenderModel::default());
        let complexity = model_complexity_from_render_model(&empty);
        assert_eq!(complexity.items, 1);
        assert_eq!(complexity.text_bytes, 0);
        InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(InputResourceLimitId::MaxModelItems, 1)
            .unwrap()
            .check_model_complexity(model_complexity_from_render_model(&empty))
            .unwrap();

        let task = GanttRenderTask {
            semantic: GanttTask {
                id: "build".to_string(),
                task: "Build".to_string(),
                ..Default::default()
            },
            render_end_ms: Some(42),
        };
        let expected = ModelComplexity::from_serializable(&GanttDiagramModel {
            tasks: vec![task.semantic.clone()],
            ..Default::default()
        });
        let mut full = GanttDiagramRenderModel::default();
        full.tasks.push(task);
        let full = RenderSemanticModel::Gantt(full);
        assert_eq!(model_complexity_from_render_model(&full), expected);
        InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(InputResourceLimitId::MaxModelItems, expected.items)
            .unwrap()
            .check_model_complexity(model_complexity_from_render_model(&full))
            .unwrap();
    }

    #[cfg(feature = "diagram-class")]
    #[test]
    fn class_semantic_complexity_counts_raw_namespace_depth() {
        let namespace = (0..20)
            .map(|index| format!("n{index}"))
            .collect::<Vec<_>>()
            .join(".");
        let source = format!("classDiagram\nnamespace {namespace} {{\nclass Service\n}}\n");
        let parsed = crate::SemanticParser::new()
            .parse_diagram_sync(&source)
            .unwrap();
        let complexity = model_complexity_from_semantic_model(&parsed.model);
        assert_eq!(complexity.nesting_depth, 20);
        let error = InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(InputResourceLimitId::MaxModelNestingDepth, 19)
            .unwrap()
            .check_model_complexity(model_complexity_from_semantic_model(&parsed.model))
            .unwrap_err();
        assert_eq!(error.limit, "max_model_nesting_depth");
        assert_eq!(error.actual, 20);
    }

    #[cfg(all(feature = "diagram-class", feature = "presentation"))]
    #[test]
    fn class_full_complexity_preserves_projection_and_charges_owned_text_once() {
        let source = concat!(
            "%%{init: {\"class\":{\"hierarchicalNamespaces\":false}}}%%\n",
            "classDiagram\nnamespace A.B {\nclass Service\nnote \"namespace note\"\n}\n",
            "class note0\nclass interface0\nIService ()-- Service\n",
        );
        let parsed = crate::MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, crate::ParseOptions::strict())
            .unwrap()
            .unwrap();
        let RenderSemanticModel::Class(model) = parsed.model() else {
            panic!("Class model");
        };
        let projected = ClassComplexity::from_model(model);
        assert_eq!(projected.nodes, 6);
        assert_eq!(projected.edges, 1);
        assert_eq!(projected.namespaces, 1);
        assert_eq!(projected.namespace_depth, 1);
        let canonical_text = ModelComplexity::from_serializable(model.model()).text_bytes;
        // The published DOM ordinals, collapsed membership and collision remaps own 194 bytes.
        assert_eq!(
            model_complexity_from_render_model(parsed.model()).text_bytes,
            canonical_text + 194
        );
        let exact_text = model_complexity_from_parsed_render(&parsed).text_bytes;
        let policy = InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput);
        policy
            .with_limit(InputResourceLimitId::MaxModelTextBytes, exact_text)
            .unwrap()
            .check_model_complexity(model_complexity_from_parsed_render(&parsed))
            .unwrap();
        let error = policy
            .with_limit(InputResourceLimitId::MaxModelTextBytes, exact_text - 1)
            .unwrap()
            .check_model_complexity(model_complexity_from_parsed_render(&parsed))
            .unwrap_err();
        assert_eq!(error.limit, "max_model_text_bytes");
        assert_eq!(error.actual, exact_text);
    }

    #[cfg(all(feature = "diagram-xychart", feature = "presentation"))]
    #[test]
    fn xychart_full_wrapper_preserves_static_field_resource_accounting() {
        let parsed = crate::MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "xychart-beta\nx-axis [a, b]\nbar [1, 2]\n",
                crate::ParseOptions::strict(),
            )
            .unwrap()
            .unwrap();
        let RenderSemanticModel::XyChart(model) = parsed.model() else {
            panic!("XYChart model")
        };
        let expected = ModelComplexity::from_serializable(&model.semantic);
        assert_eq!(model_complexity_from_render_model(parsed.model()), expected);
        InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
            .with_limit(InputResourceLimitId::MaxModelItems, expected.items)
            .unwrap()
            .check_model_complexity(model_complexity_from_render_model(parsed.model()))
            .unwrap();
    }

    #[cfg(feature = "diagram-kanban")]
    #[test]
    fn kanban_semantic_limits_count_arbitrary_metadata_iteratively() {
        use crate::diagrams::kanban::KanbanNodeModel;

        for depth in [32usize, 130] {
            let mut value = serde_json::Value::String("payload".to_string());
            for _ in 0..depth {
                value = serde_json::Value::Array(vec![value]);
            }
            let value = serde_json::Value::Object(serde_json::Map::from_iter([(
                "custom".to_string(),
                value,
            )]));
            let model = KanbanDiagramModel {
                nodes: vec![KanbanNodeModel {
                    id: "card".to_string(),
                    label: String::new(),
                    level: 0,
                    node_type: 0,
                    span: crate::SourceSpan::new(0, 0),
                    is_group: false,
                    parent_id: None,
                    ticket: None,
                    priority: None,
                    assigned: None,
                    icon: None,
                    css_classes: None,
                    shape: None,
                    metadata: Some(value.into()),
                }],
            };
            let complexity = model_complexity_from_kanban_semantic(&model);
            assert_eq!(complexity.items, depth + 2);
            assert_eq!(complexity.text_bytes, 17);
            assert_eq!(complexity.nesting_depth, depth + 1);
            InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
                .with_limit(InputResourceLimitId::MaxModelNestingDepth, depth + 1)
                .unwrap()
                .check_model_complexity(complexity)
                .unwrap();
            assert!(
                InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
                    .with_limit(InputResourceLimitId::MaxModelNestingDepth, depth)
                    .unwrap()
                    .check_model_complexity(complexity)
                    .is_err()
            );
        }
    }

    #[cfg(feature = "diagram-treemap")]
    #[test]
    fn treemap_limits_include_arbitrary_value_nesting() {
        for depth in [32usize, 130] {
            let mut value = serde_json::Value::Null;
            for _ in 0..depth {
                value = serde_json::Value::Array(vec![value]);
            }
            let model = TreemapDiagramModel {
                root: crate::diagrams::treemap::TreemapNode {
                    value: Some(value.into()),
                    ..Default::default()
                },
                ..Default::default()
            };
            let complexity = TreemapComplexity::from_model(&model).as_model_complexity();
            assert_eq!(complexity.nesting_depth, depth);
            InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
                .with_limit(InputResourceLimitId::MaxModelNestingDepth, depth)
                .unwrap()
                .check_model_complexity(complexity)
                .unwrap();
            assert!(
                InputResourcePolicy::for_profile(ResourceProfile::UnboundedForTrustedInput)
                    .with_limit(InputResourceLimitId::MaxModelNestingDepth, depth - 1)
                    .unwrap()
                    .check_model_complexity(complexity)
                    .is_err()
            );
            #[cfg(feature = "presentation")]
            {
                let full =
                    crate::diagrams::treemap::TreemapDiagramRenderModel::from_semantic_model(model);
                assert_eq!(
                    TreemapComplexity::from_render_model(&full).as_model_complexity(),
                    complexity
                );
            }
        }
    }

    #[cfg(feature = "diagram-treemap")]
    #[test]
    fn treemap_complexity_handles_deep_typed_trees_without_serde_recursion() {
        let mut nodes = vec![crate::diagrams::treemap::TreemapNode {
            name: "leaf".to_string(),
            ..Default::default()
        }];
        for index in (0..1_500).rev() {
            nodes.push(crate::diagrams::treemap::TreemapNode {
                name: format!("section{index}"),
                children: Some(vec![nodes.len() - 1]),
                ..Default::default()
            });
        }
        let model = TreemapDiagramModel {
            root: nodes.pop().unwrap(),
            nodes,
            ..Default::default()
        };

        let complexity = TreemapComplexity::from_model(&model);

        assert_eq!(complexity.nodes, 1_501);
        assert_eq!(complexity.nesting_depth, 1_500);
        assert!(complexity.label_bytes >= "leaf".len());
    }

    #[cfg(all(feature = "diagram-treemap", feature = "presentation"))]
    #[test]
    fn treemap_complexity_counts_reachable_presentation_overrides_only_for_full_models() {
        use crate::diagrams::treemap::{TreemapDiagramRenderModel, TreemapNode};

        let mut model = TreemapDiagramRenderModel::from_semantic_model(TreemapDiagramModel {
            root: TreemapNode {
                children: Some(vec![0]),
                ..Default::default()
            },
            nodes: vec![TreemapNode::default(), TreemapNode::default()],
            ..Default::default()
        });
        model.visuals.root_styles = Some(vec!["fill:red".to_string()]);
        model.visuals.node_styles = vec![
            Some(vec!["stroke:blue".to_string()]),
            Some(vec!["font-size:99px".to_string()]),
        ];
        let semantic = TreemapComplexity::from_model(&model.semantic);
        let full = TreemapComplexity::from_render_model(&model);

        assert_eq!(full.nodes, semantic.nodes);
        assert_eq!(full.nesting_depth, semantic.nesting_depth);
        assert_eq!(full.label_bytes, semantic.label_bytes + 19);
    }

    #[cfg(feature = "diagram-block")]
    #[test]
    fn block_semantic_complexity_retains_logical_nesting_after_flattening() {
        for depth in [32usize, 130] {
            let mut source = String::from("block\n");
            for index in 0..depth {
                source.push_str(&format!("block:G{index}\n"));
            }
            source.push_str("A\n");
            for _ in 0..depth {
                source.push_str("end\n");
            }
            let parsed = crate::SemanticParser::new()
                .parse_diagram_sync(&source)
                .unwrap();
            let complexity = model_complexity_from_semantic_model(&parsed.model);

            assert_eq!(complexity.nesting_depth, depth + 1);
        }
    }

    #[cfg(feature = "diagram-ishikawa")]
    #[test]
    fn ishikawa_complexity_handles_deep_typed_trees_without_serde_recursion() {
        let mut nodes = vec![crate::diagrams::ishikawa::IshikawaNode {
            text: "leaf".to_string(),
            children: Vec::new(),
        }];
        for index in (0..1_500).rev() {
            nodes.push(crate::diagrams::ishikawa::IshikawaNode {
                text: format!("cause{index}"),
                children: vec![nodes.len() - 1],
            });
        }
        let model = IshikawaDiagramModel {
            root: Some(nodes.len() - 1),
            nodes,
            ..Default::default()
        };

        let complexity = IshikawaComplexity::from_model(&model);

        assert_eq!(complexity.nodes, 1_501);
        assert_eq!(complexity.nesting_depth, 1_500);
        assert!(complexity.label_bytes >= "leaf".len());
    }
}
