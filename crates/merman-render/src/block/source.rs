use std::collections::HashMap;

use merman_core::diagrams::block::BlockDiagramRenderModel;

use super::BlockNode;

/// Effective source fields shared by geometry, theme ownership and SVG emission.
///
/// Flat records preserve Mermaid database insertion order.
/// Later non-empty fields replace earlier fields for repeated node IDs.
#[derive(Debug)]
pub(crate) struct BlockNodeSource<'a> {
    pub(crate) color_index: Option<usize>,
    pub(crate) label: &'a str,
    pub(crate) block_type: &'a str,
    pub(crate) classes: &'a [String],
    pub(crate) styles: &'a [String],
    pub(crate) directions: &'a [String],
    pub(crate) width_in_columns: i64,
}

impl<'a> BlockNodeSource<'a> {
    fn new(node: &'a BlockNode, color_index: Option<usize>) -> Self {
        Self {
            color_index,
            label: &node.label,
            block_type: &node.block_type,
            classes: &node.classes,
            styles: &node.styles,
            directions: &node.directions,
            width_in_columns: node.width_in_columns.unwrap_or(1).max(1),
        }
    }

    fn merge(&mut self, node: &'a BlockNode, color_index: Option<usize>) {
        if color_index.is_some() {
            self.color_index = color_index;
        }
        if !node.label.is_empty() {
            self.label = &node.label;
        }
        if !node.block_type.is_empty() && node.block_type != "na" {
            self.block_type = &node.block_type;
        }
        if !node.classes.is_empty() {
            self.classes = &node.classes;
        }
        if !node.styles.is_empty() {
            self.styles = &node.styles;
        }
        if !node.directions.is_empty() {
            self.directions = &node.directions;
        }
        if let Some(width) = node.width_in_columns {
            self.width_in_columns = width.max(1);
        }
    }
}

pub(crate) fn resolve_block_node_sources(
    model: &BlockDiagramRenderModel,
) -> HashMap<&str, BlockNodeSource<'_>> {
    let mut sources = HashMap::new();
    for (index, node) in model.blocks_flat.iter().enumerate() {
        let color_index = model.visuals.color_indices.get(index).copied().flatten();
        sources
            .entry(node.id.as_str())
            .and_modify(|source: &mut BlockNodeSource<'_>| source.merge(node, color_index))
            .or_insert_with(|| BlockNodeSource::new(node, color_index));
    }
    sources
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_nodes_merge_fields_in_database_insertion_order() {
        let mut model = BlockDiagramRenderModel::default();
        let root = BlockNode {
            id: "root".to_string(),
            block_type: "composite".to_string(),
            children: vec![1, 2],
            ..Default::default()
        };
        let first = BlockNode {
            id: "A".to_string(),
            block_type: "square".to_string(),
            label: "First".to_string(),
            classes: vec!["first".to_string()],
            styles: vec!["color:red".to_string()],
            directions: vec!["left".to_string()],
            width_in_columns: Some(3),
            ..Default::default()
        };
        let branch = BlockNode {
            id: "branch".to_string(),
            children: vec![3],
            ..Default::default()
        };
        let nested = BlockNode {
            id: "A".to_string(),
            block_type: "circle".to_string(),
            label: "Nested".to_string(),
            classes: vec!["nested".to_string()],
            width_in_columns: Some(2),
            ..Default::default()
        };
        let last = BlockNode {
            id: "A".to_string(),
            block_type: "na".to_string(),
            label: "Last".to_string(),
            styles: vec!["color:blue".to_string()],
            directions: vec!["right".to_string()],
            width_in_columns: Some(0),
            ..Default::default()
        };
        let empty = BlockNode {
            id: "A".to_string(),
            ..Default::default()
        };
        model.blocks_flat = vec![root, first, branch, nested, last, empty];
        model.visuals.color_indices = vec![None, Some(0), None, Some(1), None, None];
        let sources = resolve_block_node_sources(&model);
        let node = &sources["A"];
        assert_eq!(node.label, "Last");
        assert_eq!(node.block_type, "circle");
        assert_eq!(node.classes, ["nested"]);
        assert_eq!(node.styles, ["color:blue"]);
        assert_eq!(node.directions, ["right"]);
        assert_eq!(node.width_in_columns, 1);
        assert_eq!(node.color_index, Some(1));
        assert_eq!(sources.len(), 3);
    }
}
