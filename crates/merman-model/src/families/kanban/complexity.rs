use super::KanbanDiagramModel;
#[cfg(feature = "presentation-data")]
use super::KanbanDiagramRenderModel;
use crate::resources::{ModelComplexity, json_value_complexity};

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

#[cfg(feature = "presentation-data")]
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
