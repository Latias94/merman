use std::collections::HashMap;

use merman_core::diagrams::block::BlockDiagramRenderModel;

use super::BlockNode;

/// Effective source fields shared by geometry, theme ownership and SVG emission.
///
/// Mermaid's render model can repeat a node in the tree and in `blocks_flat`.
/// Traverse each tree in source order; later non-empty fields replace earlier fields.
#[derive(Debug)]
pub(crate) struct BlockNodeSource<'a> {
    pub(crate) label: &'a str,
    pub(crate) block_type: &'a str,
    pub(crate) classes: &'a [String],
    pub(crate) styles: &'a [String],
    pub(crate) directions: &'a [String],
    pub(crate) width_in_columns: i64,
}

impl<'a> BlockNodeSource<'a> {
    fn new(node: &'a BlockNode) -> Self {
        Self {
            label: &node.label,
            block_type: &node.block_type,
            classes: &node.classes,
            styles: &node.styles,
            directions: &node.directions,
            width_in_columns: node.width_in_columns.unwrap_or(1).max(1),
        }
    }

    fn merge(&mut self, node: &'a BlockNode) {
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
    let mut stack = Vec::new();
    for root in &model.blocks_flat {
        stack.push(root);
        while let Some(node) = stack.pop() {
            sources
                .entry(node.id.as_str())
                .and_modify(|source: &mut BlockNodeSource<'_>| source.merge(node))
                .or_insert_with(|| BlockNodeSource::new(node));
            stack.extend(node.children.iter().rev());
        }
    }
    sources
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_nodes_merge_fields_in_depth_first_source_order() {
        let model = serde_json::from_value(serde_json::json!({"blocksFlat": [
            {"id": "root", "type": "composite", "children": [
                {"id": "A", "type": "square", "label": "First",
                 "classes": ["first"], "styles": ["color:red"],
                 "directions": ["left"], "widthInColumns": 3},
                {"id": "branch", "children": [
                    {"id": "A", "type": "circle", "label": "Nested",
                     "classes": ["nested"], "widthInColumns": 2}
                ]}
            ]},
            {"id": "A", "type": "na", "label": "Last", "styles": ["color:blue"],
             "directions": ["right"], "widthInColumns": 0},
            {"id": "A", "type": "", "label": "", "classes": [], "styles": []}
        ]}))
        .unwrap();
        let sources = resolve_block_node_sources(&model);
        let node = &sources["A"];
        assert_eq!(node.label, "Last");
        assert_eq!(node.block_type, "circle");
        assert_eq!(node.classes, ["nested"]);
        assert_eq!(node.styles, ["color:blue"]);
        assert_eq!(node.directions, ["right"]);
        assert_eq!(node.width_in_columns, 1);
        assert_eq!(sources.len(), 3);
    }
}
