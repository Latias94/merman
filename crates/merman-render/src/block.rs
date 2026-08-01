use crate::model::{BlockDiagramLayout, Bounds, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint};
use crate::text::{TextMeasurer, TextStyle, WrapMode};
use crate::{Error, Result};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

mod config;

use config::{BlockConfigView, BlockLayoutSettings};

mod geometry;

pub use geometry::{
    BlockAllocatedBounds, BlockRectangleKind, BlockShapeBoundary, BlockShapeGeometry,
};

pub(crate) type BlockNode = merman_core::diagrams::block::BlockNodeRenderModel;

#[derive(Debug, Clone)]
struct SizedBlock {
    id: String,
    block_type: String,
    children: Vec<SizedBlock>,
    columns: i64,
    width_in_columns: i64,
    width: f64,
    height: f64,
    label_width: f64,
    label_height: f64,
    x: f64,
    y: f64,
}

fn decode_block_label_html(raw: &str) -> String {
    raw.replace("&nbsp;", "\u{00A0}")
}

pub(crate) fn block_label_is_effectively_empty(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|ch| ch != '\u{00A0}' && ch.is_whitespace())
}

fn block_html_label_metrics_px(
    text: &str,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
) -> (f64, f64) {
    let html_metrics = measurer.measure_wrapped(text, style, None, WrapMode::HtmlLike);
    (html_metrics.width.max(0.0), html_metrics.height.max(0.0))
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct BlockArrowPoint {
    pub(crate) x: f64,
    pub(crate) y: f64,
}

pub(crate) fn block_arrow_points_for_width(
    directions: &[String],
    bbox_h: f64,
    node_padding: f64,
    width: f64,
) -> Vec<BlockArrowPoint> {
    fn expand_and_dedup(directions: &[String]) -> std::collections::BTreeSet<String> {
        let mut out = std::collections::BTreeSet::new();
        for d in directions {
            match d.trim() {
                "x" => {
                    out.insert("right".to_string());
                    out.insert("left".to_string());
                }
                "y" => {
                    out.insert("up".to_string());
                    out.insert("down".to_string());
                }
                other if !other.is_empty() => {
                    out.insert(other.to_string());
                }
                _ => {}
            }
        }
        out
    }

    let dirs = expand_and_dedup(directions);
    let height = bbox_h + 2.0 * node_padding;
    let midpoint = height / 2.0;
    let pad = node_padding / 2.0;

    let has = |name: &str| dirs.contains(name);

    if has("right") && has("left") && has("up") && has("down") {
        return vec![
            BlockArrowPoint { x: 0.0, y: 0.0 },
            BlockArrowPoint {
                x: midpoint,
                y: 0.0,
            },
            BlockArrowPoint {
                x: width / 2.0,
                y: 2.0 * pad,
            },
            BlockArrowPoint {
                x: width - midpoint,
                y: 0.0,
            },
            BlockArrowPoint { x: width, y: 0.0 },
            BlockArrowPoint {
                x: width,
                y: -height / 3.0,
            },
            BlockArrowPoint {
                x: width + 2.0 * pad,
                y: -height / 2.0,
            },
            BlockArrowPoint {
                x: width,
                y: (-2.0 * height) / 3.0,
            },
            BlockArrowPoint {
                x: width,
                y: -height,
            },
            BlockArrowPoint {
                x: width - midpoint,
                y: -height,
            },
            BlockArrowPoint {
                x: width / 2.0,
                y: -height - 2.0 * pad,
            },
            BlockArrowPoint {
                x: midpoint,
                y: -height,
            },
            BlockArrowPoint { x: 0.0, y: -height },
            BlockArrowPoint {
                x: 0.0,
                y: (-2.0 * height) / 3.0,
            },
            BlockArrowPoint {
                x: -2.0 * pad,
                y: -height / 2.0,
            },
            BlockArrowPoint {
                x: 0.0,
                y: -height / 3.0,
            },
        ];
    }
    if has("right") && has("left") && has("up") {
        return vec![
            BlockArrowPoint {
                x: midpoint,
                y: 0.0,
            },
            BlockArrowPoint {
                x: width - midpoint,
                y: 0.0,
            },
            BlockArrowPoint {
                x: width,
                y: -height / 2.0,
            },
            BlockArrowPoint {
                x: width - midpoint,
                y: -height,
            },
            BlockArrowPoint {
                x: midpoint,
                y: -height,
            },
            BlockArrowPoint {
                x: 0.0,
                y: -height / 2.0,
            },
        ];
    }
    if has("right") && has("left") && has("down") {
        return vec![
            BlockArrowPoint { x: 0.0, y: 0.0 },
            BlockArrowPoint {
                x: midpoint,
                y: -height,
            },
            BlockArrowPoint {
                x: width - midpoint,
                y: -height,
            },
            BlockArrowPoint { x: width, y: 0.0 },
        ];
    }
    if has("right") && has("up") && has("down") {
        return vec![
            BlockArrowPoint { x: 0.0, y: 0.0 },
            BlockArrowPoint {
                x: width,
                y: -midpoint,
            },
            BlockArrowPoint {
                x: width,
                y: -height + midpoint,
            },
            BlockArrowPoint { x: 0.0, y: -height },
        ];
    }
    if has("left") && has("up") && has("down") {
        return vec![
            BlockArrowPoint { x: width, y: 0.0 },
            BlockArrowPoint {
                x: 0.0,
                y: -midpoint,
            },
            BlockArrowPoint {
                x: 0.0,
                y: -height + midpoint,
            },
            BlockArrowPoint {
                x: width,
                y: -height,
            },
        ];
    }
    if has("right") && has("left") {
        return vec![
            BlockArrowPoint {
                x: midpoint,
                y: 0.0,
            },
            BlockArrowPoint {
                x: midpoint,
                y: -pad,
            },
            BlockArrowPoint {
                x: width - midpoint,
                y: -pad,
            },
            BlockArrowPoint {
                x: width - midpoint,
                y: 0.0,
            },
            BlockArrowPoint {
                x: width,
                y: -height / 2.0,
            },
            BlockArrowPoint {
                x: width - midpoint,
                y: -height,
            },
            BlockArrowPoint {
                x: width - midpoint,
                y: -height + pad,
            },
            BlockArrowPoint {
                x: midpoint,
                y: -height + pad,
            },
            BlockArrowPoint {
                x: midpoint,
                y: -height,
            },
            BlockArrowPoint {
                x: 0.0,
                y: -height / 2.0,
            },
        ];
    }
    if has("up") && has("down") {
        return vec![
            BlockArrowPoint {
                x: width / 2.0,
                y: 0.0,
            },
            BlockArrowPoint { x: 0.0, y: -pad },
            BlockArrowPoint {
                x: midpoint,
                y: -pad,
            },
            BlockArrowPoint {
                x: midpoint,
                y: -height + pad,
            },
            BlockArrowPoint {
                x: 0.0,
                y: -height + pad,
            },
            BlockArrowPoint {
                x: width / 2.0,
                y: -height,
            },
            BlockArrowPoint {
                x: width,
                y: -height + pad,
            },
            BlockArrowPoint {
                x: width - midpoint,
                y: -height + pad,
            },
            BlockArrowPoint {
                x: width - midpoint,
                y: -pad,
            },
            BlockArrowPoint { x: width, y: -pad },
        ];
    }
    if has("right") && has("up") {
        return vec![
            BlockArrowPoint { x: 0.0, y: 0.0 },
            BlockArrowPoint {
                x: width,
                y: -midpoint,
            },
            BlockArrowPoint { x: 0.0, y: -height },
        ];
    }
    if has("right") && has("down") {
        return vec![
            BlockArrowPoint { x: 0.0, y: 0.0 },
            BlockArrowPoint { x: width, y: 0.0 },
            BlockArrowPoint { x: 0.0, y: -height },
        ];
    }
    if has("left") && has("up") {
        return vec![
            BlockArrowPoint { x: width, y: 0.0 },
            BlockArrowPoint {
                x: 0.0,
                y: -midpoint,
            },
            BlockArrowPoint {
                x: width,
                y: -height,
            },
        ];
    }
    if has("left") && has("down") {
        return vec![
            BlockArrowPoint { x: width, y: 0.0 },
            BlockArrowPoint { x: 0.0, y: 0.0 },
            BlockArrowPoint {
                x: width,
                y: -height,
            },
        ];
    }
    if has("right") {
        return vec![
            BlockArrowPoint {
                x: midpoint,
                y: -pad,
            },
            BlockArrowPoint {
                x: midpoint,
                y: -pad,
            },
            BlockArrowPoint {
                x: width - midpoint,
                y: -pad,
            },
            BlockArrowPoint {
                x: width - midpoint,
                y: 0.0,
            },
            BlockArrowPoint {
                x: width,
                y: -height / 2.0,
            },
            BlockArrowPoint {
                x: width - midpoint,
                y: -height,
            },
            BlockArrowPoint {
                x: width - midpoint,
                y: -height + pad,
            },
            BlockArrowPoint {
                x: midpoint,
                y: -height + pad,
            },
            BlockArrowPoint {
                x: midpoint,
                y: -height + pad,
            },
        ];
    }
    if has("left") {
        return vec![
            BlockArrowPoint {
                x: midpoint,
                y: 0.0,
            },
            BlockArrowPoint {
                x: midpoint,
                y: -pad,
            },
            BlockArrowPoint {
                x: width - midpoint,
                y: -pad,
            },
            BlockArrowPoint {
                x: width - midpoint,
                y: -height + pad,
            },
            BlockArrowPoint {
                x: midpoint,
                y: -height + pad,
            },
            BlockArrowPoint {
                x: midpoint,
                y: -height,
            },
            BlockArrowPoint {
                x: 0.0,
                y: -height / 2.0,
            },
        ];
    }
    if has("up") {
        return vec![
            BlockArrowPoint {
                x: midpoint,
                y: -pad,
            },
            BlockArrowPoint {
                x: midpoint,
                y: -height + pad,
            },
            BlockArrowPoint {
                x: 0.0,
                y: -height + pad,
            },
            BlockArrowPoint {
                x: width / 2.0,
                y: -height,
            },
            BlockArrowPoint {
                x: width,
                y: -height + pad,
            },
            BlockArrowPoint {
                x: width - midpoint,
                y: -height + pad,
            },
            BlockArrowPoint {
                x: width - midpoint,
                y: -pad,
            },
        ];
    }
    if has("down") {
        return vec![
            BlockArrowPoint {
                x: width / 2.0,
                y: 0.0,
            },
            BlockArrowPoint { x: 0.0, y: -pad },
            BlockArrowPoint {
                x: midpoint,
                y: -pad,
            },
            BlockArrowPoint {
                x: midpoint,
                y: -height + pad,
            },
            BlockArrowPoint {
                x: width - midpoint,
                y: -height + pad,
            },
            BlockArrowPoint {
                x: width - midpoint,
                y: -pad,
            },
            BlockArrowPoint { x: width, y: -pad },
        ];
    }

    vec![BlockArrowPoint { x: 0.0, y: 0.0 }]
}

fn block_shape_size(
    block_type: &str,
    directions: &[String],
    label_width: f64,
    label_height: f64,
    padding: f64,
    has_label: bool,
) -> Option<(f64, f64)> {
    geometry::natural_shape_size(
        block_type,
        directions,
        label_width,
        label_height,
        padding,
        has_label,
    )
}

fn to_sized_block_shallow(
    node: &BlockNode,
    padding: f64,
    measurer: &dyn TextMeasurer,
    text_style: &TextStyle,
    children: Vec<SizedBlock>,
) -> SizedBlock {
    let columns = node.columns.unwrap_or(-1);
    let width_in_columns = node.width_in_columns.unwrap_or(1).max(1);

    let mut width = 0.0;
    let mut height = 0.0;

    // Mermaid renders block diagram labels via `labelHelper(...)`, which decodes HTML entities
    // and measures the resulting HTML content (`getBoundingClientRect()` for width/height).
    //
    // Block diagrams frequently use `&nbsp;` placeholders (notably for block arrows), so we must
    // decode those before measuring; otherwise node widths drift drastically.
    let label_decoded = decode_block_label_html(&node.label);
    let label_effectively_empty = block_label_is_effectively_empty(&label_decoded);
    let (label_width, label_height) = if label_effectively_empty {
        (0.0, 0.0)
    } else {
        block_html_label_metrics_px(&label_decoded, measurer, text_style)
    };
    let shape_label_height = label_height;

    if let Some((computed_width, computed_height)) = block_shape_size(
        node.block_type.as_str(),
        &node.directions,
        label_width,
        shape_label_height,
        padding,
        !label_effectively_empty && !label_decoded.trim().is_empty(),
    ) {
        width = computed_width;
        height = computed_height;
    }

    SizedBlock {
        id: node.id.clone(),
        block_type: node.block_type.clone(),
        children,
        columns,
        width_in_columns,
        width,
        height,
        label_width,
        label_height,
        x: 0.0,
        y: 0.0,
    }
}

fn to_sized_block(
    node: &BlockNode,
    padding: f64,
    measurer: &dyn TextMeasurer,
    text_style: &TextStyle,
) -> SizedBlock {
    let mut stack: Vec<(&BlockNode, bool)> = vec![(node, false)];
    let mut completed: HashMap<*const BlockNode, SizedBlock> = HashMap::new();

    while let Some((block, visited)) = stack.pop() {
        if visited {
            let children = block
                .children
                .iter()
                .filter_map(|child| completed.remove(&(child as *const BlockNode)))
                .collect();
            completed.insert(
                block as *const BlockNode,
                to_sized_block_shallow(block, padding, measurer, text_style, children),
            );
        } else {
            stack.push((block, true));
            for child in block.children.iter().rev() {
                stack.push((child, false));
            }
        }
    }

    completed
        .remove(&(node as *const BlockNode))
        .unwrap_or_else(|| to_sized_block_shallow(node, padding, measurer, text_style, Vec::new()))
}

fn get_max_child_size(block: &SizedBlock) -> (f64, f64) {
    let mut max_width = 0.0;
    let mut max_height = 0.0;
    for child in &block.children {
        if child.block_type == "space" {
            continue;
        }
        let normalized_width = child.width / (child.width_in_columns as f64);
        if normalized_width > max_width {
            max_width = normalized_width;
        }
        if child.height > max_height {
            max_height = child.height;
        }
    }
    (max_width, max_height)
}

fn block_ref_at_path<'a>(root: &'a SizedBlock, path: &[usize]) -> &'a SizedBlock {
    let mut block = root;
    for &index in path {
        block = &block.children[index];
    }
    block
}

fn block_mut_at_path<'a>(root: &'a mut SizedBlock, path: &[usize]) -> &'a mut SizedBlock {
    let mut block = root;
    for &index in path {
        block = &mut block.children[index];
    }
    block
}

fn set_block_sizes_shallow(block: &mut SizedBlock, padding: f64) {
    if block.width <= 0.0 {
        block.width = 0.0;
        block.height = 0.0;
        block.x = 0.0;
        block.y = 0.0;
    }

    if block.children.is_empty() {
        return;
    }

    let (mut max_width, mut max_height) = get_max_child_size(block);

    for child in &mut block.children {
        child.width = max_width * (child.width_in_columns as f64)
            + padding * ((child.width_in_columns as f64) - 1.0);
        child.height = max_height;
        child.x = 0.0;
        child.y = 0.0;
    }

    for child in &mut block.children {
        child.x = 0.0;
        child.y = 0.0;
    }

    let (x_size, y_size) = block_grid_size(block);

    let mut width = (x_size as f64) * (max_width + padding) + padding;
    let height = (y_size as f64) * (max_height + padding) + padding;

    if width < block.width {
        width = block.width;
        let num = if block.columns > 0 {
            (block.children.len() as i64).min(block.columns)
        } else {
            block.children.len() as i64
        };
        if num > 0 {
            let child_width = (width - (num as f64) * padding - padding) / (num as f64);
            for child in &mut block.children {
                child.width = child_width;
            }
        }
    }

    block.width = width;
    block.height = height;
    block.x = 0.0;
    block.y = 0.0;

    // Keep behavior consistent with Mermaid even when all children were `space`.
    max_width = max_width.max(0.0);
    max_height = max_height.max(0.0);
    let _ = (max_width, max_height);
}

fn block_grid_size(block: &SizedBlock) -> (i64, i64) {
    let columns = block.columns;
    let mut num_items = 0i64;
    for child in &block.children {
        num_items += child.width_in_columns.max(1);
    }

    let mut x_size = block.children.len() as i64;
    if columns > 0 && columns < num_items {
        x_size = columns;
    }
    let y_size = ((num_items as f64) / (x_size.max(1) as f64)).ceil() as i64;
    (x_size, y_size)
}

fn reconcile_block_with_parent_size(block: &mut SizedBlock, padding: f64) {
    if block.children.is_empty() {
        return;
    }

    let inherited_width = block.width;
    let inherited_height = block.height;
    let column_span = block.width_in_columns.max(1) as f64;
    let sibling_width = (inherited_width - padding * (column_span - 1.0)) / column_span;

    let (max_width, max_height) = get_max_child_size(block);
    let (x_size, y_size) = block_grid_size(block);
    let mut width = (x_size as f64) * (max_width + padding) + padding;
    let mut height = (y_size as f64) * (max_height + padding) + padding;

    if width < sibling_width {
        width = sibling_width;
        height = inherited_height;
        let child_width = (sibling_width - (x_size as f64) * padding - padding) / (x_size as f64);
        let child_height =
            (inherited_height - (y_size as f64) * padding - padding) / (y_size as f64);
        for child in &mut block.children {
            child.width = child_width;
            child.height = child_height;
            child.x = 0.0;
            child.y = 0.0;
        }
    }

    if width < inherited_width {
        width = inherited_width;
        let num = if block.columns > 0 {
            (block.children.len() as i64).min(block.columns)
        } else {
            block.children.len() as i64
        };
        if num > 0 {
            let child_width = (width - (num as f64) * padding - padding) / (num as f64);
            for child in &mut block.children {
                child.width = child_width;
            }
        }
    }

    block.width = width;
    block.height = height;
    block.x = 0.0;
    block.y = 0.0;
}

fn set_block_sizes(block: &mut SizedBlock, padding: f64) {
    let mut stack: Vec<(Vec<usize>, bool)> = vec![(Vec::new(), false)];
    while let Some((path, visited)) = stack.pop() {
        if visited {
            let block = block_mut_at_path(block, &path);
            set_block_sizes_shallow(block, padding);
            continue;
        }

        let child_count = block_ref_at_path(block, &path).children.len();
        stack.push((path.clone(), true));
        for index in (0..child_count).rev() {
            let mut child_path = path.clone();
            child_path.push(index);
            stack.push((child_path, false));
        }
    }

    let mut stack: Vec<Vec<usize>> = (0..block.children.len())
        .rev()
        .map(|index| vec![index])
        .collect();
    while let Some(path) = stack.pop() {
        let child_count = {
            let block = block_mut_at_path(block, &path);
            reconcile_block_with_parent_size(block, padding);
            block.children.len()
        };

        for index in (0..child_count).rev() {
            let mut child_path = path.clone();
            child_path.push(index);
            stack.push(child_path);
        }
    }
}

fn invalid_block_columns_error() -> Error {
    Error::InvalidModel {
        // Match Mermaid's layout invariant while returning a typed render failure instead of
        // letting a malformed semantic model reach integer division.
        message: "Columns must be an integer !== 0.".to_string(),
    }
}

fn validate_block_columns(root: &BlockNode) -> Result<()> {
    let mut stack = vec![root];
    while let Some(block) = stack.pop() {
        if block.columns == Some(0) {
            return Err(invalid_block_columns_error());
        }
        stack.extend(block.children.iter());
    }
    Ok(())
}

fn calculate_block_position(columns: i64, position: i64) -> Result<(i64, i64)> {
    if columns == 0 {
        return Err(invalid_block_columns_error());
    }
    if columns < 0 {
        return Ok((position, 0));
    }
    if columns == 1 {
        return Ok((0, position));
    }
    Ok((position % columns, position / columns))
}

fn layout_blocks(block: &mut SizedBlock, padding: f64) -> Result<()> {
    let mut stack: Vec<Vec<usize>> = vec![Vec::new()];
    while let Some(path) = stack.pop() {
        let child_count = {
            let block = block_mut_at_path(block, &path);
            if block.children.is_empty() {
                0
            } else {
                let columns = block.columns;
                let mut row_heights = BTreeMap::<i64, f64>::new();
                let mut height_column_pos = 0i64;
                for child in &block.children {
                    let (_, row) = calculate_block_position(columns, height_column_pos)?;
                    row_heights
                        .entry(row)
                        .and_modify(|height| *height = height.max(child.height))
                        .or_insert(child.height);

                    let mut columns_filled = child.width_in_columns.max(1);
                    if columns > 0 {
                        let remaining = columns - (height_column_pos % columns);
                        columns_filled = columns_filled.min(remaining.max(1));
                    }
                    height_column_pos += columns_filled;
                }

                let mut row_offsets = BTreeMap::<i64, f64>::new();
                let mut offset = 0.0;
                for (&row, &height) in &row_heights {
                    row_offsets.insert(row, offset);
                    offset += height + padding;
                }

                let mut column_pos = 0i64;

                // JS truthiness: treat `0` as falsy (Mermaid uses `block?.size?.x ? ... : -padding`).
                let mut starting_pos_x = if block.x != 0.0 {
                    block.x + (-block.width / 2.0)
                } else {
                    -padding
                };
                let mut row_pos = 0i64;

                for child in &mut block.children {
                    let (px, py) = calculate_block_position(columns, column_pos)?;

                    if py != row_pos {
                        row_pos = py;
                        starting_pos_x = if block.x != 0.0 {
                            block.x + (-block.width / 2.0)
                        } else {
                            -padding
                        };
                    }

                    let half_width = child.width / 2.0;
                    child.x = starting_pos_x + padding + half_width;
                    starting_pos_x = child.x + half_width;

                    let row_offset = row_offsets.get(&py).copied().unwrap_or_default();
                    let row_height = row_heights.get(&py).copied().unwrap_or(child.height);
                    child.y =
                        block.y - block.height / 2.0 + row_offset + row_height / 2.0 + padding;

                    let mut columns_filled = child.width_in_columns.max(1);
                    if columns > 0 {
                        let rem = columns - (column_pos % columns);
                        columns_filled = columns_filled.min(rem.max(1));
                    }
                    column_pos += columns_filled;

                    let _ = px;
                }
                block.children.len()
            }
        };

        for index in (0..child_count).rev() {
            let mut child_path = path.clone();
            child_path.push(index);
            stack.push(child_path);
        }
    }
    Ok(())
}

fn find_bounds(block: &SizedBlock, b: &mut Bounds) {
    let mut stack = vec![block];
    while let Some(block) = stack.pop() {
        if block.id != "root" {
            b.min_x = b.min_x.min(block.x - block.width / 2.0);
            b.min_y = b.min_y.min(block.y - block.height / 2.0);
            b.max_x = b.max_x.max(block.x + block.width / 2.0);
            b.max_y = b.max_y.max(block.y + block.height / 2.0);
        }
        for child in block.children.iter().rev() {
            stack.push(child);
        }
    }
}

fn collect_nodes(block: &SizedBlock, out: &mut Vec<LayoutNode>) {
    let mut stack = vec![block];
    while let Some(block) = stack.pop() {
        if block.id != "root" && block.block_type != "space" {
            out.push(LayoutNode {
                id: block.id.clone(),
                x: block.x,
                y: block.y,
                width: block.width,
                height: block.height,
                is_cluster: false,
                label_width: Some(block.label_width.max(0.0)),
                label_height: Some(block.label_height.max(0.0)),
            });
        }
        for child in block.children.iter().rev() {
            stack.push(child);
        }
    }
}

#[derive(Debug, Clone)]
struct BlockShapeSource {
    block_type: String,
    directions: Vec<String>,
    width_in_columns: i64,
}

fn collect_shape_sources(root: &BlockNode, out: &mut HashMap<String, BlockShapeSource>) {
    let mut stack = vec![root];
    while let Some(block) = stack.pop() {
        let source = out
            .entry(block.id.clone())
            .or_insert_with(|| BlockShapeSource {
                block_type: block.block_type.clone(),
                directions: block.directions.clone(),
                width_in_columns: block.width_in_columns.unwrap_or(1).max(1),
            });
        if !block.block_type.is_empty() && block.block_type != "na" {
            source.block_type = block.block_type.clone();
        }
        if !block.directions.is_empty() {
            source.directions = block.directions.clone();
        }
        if let Some(width_in_columns) = block.width_in_columns {
            source.width_in_columns = width_in_columns.max(1);
        }
        for child in block.children.iter().rev() {
            stack.push(child);
        }
    }
}

pub(crate) fn layout_block_diagram_typed(
    model: &merman_core::diagrams::block::BlockDiagramRenderModel,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
) -> Result<BlockDiagramLayout> {
    let BlockLayoutSettings {
        padding,
        text_style,
    } = BlockConfigView::new(effective_config).layout_settings();

    let root = model
        .blocks_flat
        .iter()
        .find(|b| b.id == "root" && b.block_type == "composite")
        .ok_or_else(|| Error::InvalidModel {
            message: "missing block root composite".to_string(),
        })?;

    validate_block_columns(root)?;

    let mut root = to_sized_block(root, padding, measurer, &text_style);
    set_block_sizes(&mut root, padding);
    layout_blocks(&mut root, padding)?;

    let mut nodes: Vec<LayoutNode> = Vec::new();
    collect_nodes(&root, &mut nodes);

    let mut shape_sources = HashMap::new();
    for block in &model.blocks_flat {
        collect_shape_sources(block, &mut shape_sources);
    }
    let mut shape_geometries = Vec::with_capacity(nodes.len());
    for node in &nodes {
        let source = shape_sources
            .get(&node.id)
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing Block shape source for node `{}`", node.id),
            })?;
        let geometry = BlockShapeGeometry::from_layout_node(
            node,
            &source.block_type,
            &source.directions,
            padding,
            source.width_in_columns,
        )
        .ok_or_else(|| Error::InvalidModel {
            message: format!("missing Block geometry for visible node `{}`", node.id),
        })?;
        shape_geometries.push(geometry);
    }

    let mut bounds = Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 0.0,
        max_y: 0.0,
    };
    find_bounds(&root, &mut bounds);
    let bounds = if nodes.is_empty() { None } else { Some(bounds) };

    let nodes_by_id: HashMap<String, LayoutNode> =
        nodes.iter().cloned().map(|n| (n.id.clone(), n)).collect();

    let mut edges: Vec<LayoutEdge> = Vec::new();
    for e in &model.edges {
        let Some(from) = nodes_by_id.get(&e.start) else {
            continue;
        };
        let Some(to) = nodes_by_id.get(&e.end) else {
            continue;
        };

        let start = LayoutPoint {
            x: from.x,
            y: from.y,
        };
        let end = LayoutPoint { x: to.x, y: to.y };
        let mid = LayoutPoint {
            x: start.x + (end.x - start.x) / 2.0,
            y: start.y + (end.y - start.y) / 2.0,
        };

        let label = if e.label.trim().is_empty() {
            None
        } else {
            let edge_label = decode_block_label_html(&e.label);
            let (label_width, label_height) =
                block_html_label_metrics_px(&edge_label, measurer, &text_style);
            Some(LayoutLabel {
                x: mid.x,
                y: mid.y,
                width: label_width.max(1.0),
                height: label_height.max(1.0),
            })
        };

        edges.push(LayoutEdge {
            id: e.id.clone(),
            from: e.start.clone(),
            to: e.end.clone(),
            from_cluster: None,
            to_cluster: None,
            points: vec![start, mid, end],
            label,
            start_label_left: None,
            start_label_right: None,
            end_label_left: None,
            end_label_right: None,
            start_marker: e.arrow_type_start.clone(),
            end_marker: e.arrow_type_end.clone(),
            stroke_dasharray: None,
        });
    }

    Ok(BlockDiagramLayout {
        nodes,
        edges,
        shape_geometries,
        bounds,
    })
}

#[cfg(test)]
mod tests {
    use crate::text::{TextMeasurer, TextMetrics, TextStyle};

    use super::SizedBlock;

    fn default_style(font_size: f64) -> TextStyle {
        TextStyle {
            font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
            font_size,
            font_weight: None,
            font_style: None,
        }
    }

    fn sized_block(id: &str, width: f64, height: f64, width_in_columns: i64) -> SizedBlock {
        SizedBlock {
            id: id.to_string(),
            block_type: "square".to_string(),
            children: Vec::new(),
            columns: -1,
            width_in_columns,
            width,
            height,
            label_width: 0.0,
            label_height: 0.0,
            x: 0.0,
            y: 0.0,
        }
    }

    #[test]
    fn max_child_width_is_normalized_by_each_child_column_span() {
        let mut parent = sized_block("parent", 0.0, 0.0, 9);
        parent.children = vec![
            sized_block("three-columns", 180.0, 30.0, 3),
            sized_block("one-column", 80.0, 20.0, 1),
        ];

        let (width, height) = super::get_max_child_size(&parent);

        assert_eq!(width, 80.0);
        assert_eq!(height, 30.0);
    }

    #[test]
    fn multi_column_composite_keeps_intrinsic_height_when_only_total_width_grows() {
        let mut composite = sized_block("composite", 0.0, 0.0, 3);
        composite.children = (0..7)
            .map(|index| sized_block(&format!("leaf-{index}"), 18.0, 32.0, 1))
            .collect();

        let mut root = sized_block("root", 0.0, 0.0, 1);
        root.columns = 3;
        root.children = vec![sized_block("tall", 150.0, 88.0, 3), composite];

        super::set_block_sizes(&mut root, 8.0);

        let composite = &root.children[1];
        assert_eq!(composite.height, 48.0);
        assert!(composite.children.iter().all(|child| child.height == 32.0));
    }

    #[test]
    fn heterogeneous_block_rows_use_accumulated_row_heights() {
        let mut root = sized_block("root", 239.0, 296.0, 1);
        root.columns = 3;
        root.children = vec![
            sized_block("a", 223.0, 88.0, 3),
            sized_block("group1", 150.0, 88.0, 2),
            sized_block("g", 71.0, 88.0, 1),
            sized_block("group2", 223.0, 48.0, 3),
        ];

        super::layout_blocks(&mut root, 8.0).expect("valid block layout");

        assert_eq!(root.children[0].y, -96.0);
        assert_eq!(root.children[1].y, 0.0);
        assert_eq!(root.children[2].y, 0.0);
        assert_eq!(root.children[3].y, 76.0);
    }

    #[test]
    fn block_label_metrics_use_the_selected_html_measurer() {
        struct SelectedMeasurer;

        impl TextMeasurer for SelectedMeasurer {
            fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
                TextMetrics {
                    width: 321.25,
                    height: 45.5,
                    line_count: 1,
                }
            }
        }

        let style = default_style(24.0);

        let (width, height) = super::block_html_label_metrics_px(
            "Font size precedence should widen this block",
            &SelectedMeasurer,
            &style,
        );

        assert_eq!(width, 321.25);
        assert_eq!(height, 45.5);
    }

    #[test]
    fn zero_columns_are_a_typed_layout_error_not_a_division_by_zero() {
        let error = super::calculate_block_position(0, 0).expect_err("zero columns must fail");
        assert!(matches!(error, crate::Error::InvalidModel { .. }));
        assert_eq!(
            error.to_string(),
            "invalid semantic model: Columns must be an integer !== 0."
        );
    }
}
