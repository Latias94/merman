use crate::model::{BlockDiagramLayout, Bounds, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint};
use crate::text::{TextMeasurer, TextStyle, WrapMode};
use crate::{Error, Result};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BlockDiagramModel {
    // Keep the full upstream semantic model shape for future parity work.
    #[allow(dead_code)]
    #[serde(default)]
    pub blocks: Vec<BlockNode>,
    #[serde(default, rename = "blocksFlat")]
    pub blocks_flat: Vec<BlockNode>,
    #[serde(default)]
    pub edges: Vec<BlockEdge>,
    #[allow(dead_code)]
    #[serde(default)]
    pub warnings: Vec<String>,
    #[allow(dead_code)]
    #[serde(default)]
    pub classes: HashMap<String, BlockClassDef>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BlockClassDef {
    #[allow(dead_code)]
    pub id: String,
    #[allow(dead_code)]
    #[serde(default)]
    pub styles: Vec<String>,
    #[allow(dead_code)]
    #[serde(default, rename = "textStyles")]
    pub text_styles: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BlockNode {
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default, rename = "type")]
    pub block_type: String,
    #[serde(default)]
    pub children: Vec<BlockNode>,
    #[serde(default)]
    pub columns: Option<i64>,
    #[serde(default, rename = "widthInColumns")]
    pub width_in_columns: Option<i64>,
    #[serde(default)]
    pub width: Option<i64>,
    #[serde(default)]
    pub classes: Vec<String>,
    #[allow(dead_code)]
    #[serde(default)]
    pub styles: Vec<String>,
    #[serde(default)]
    pub directions: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct BlockEdge {
    pub id: String,
    pub start: String,
    pub end: String,
    #[serde(default, rename = "arrowTypeEnd")]
    pub arrow_type_end: Option<String>,
    #[serde(default, rename = "arrowTypeStart")]
    pub arrow_type_start: Option<String>,
    #[serde(default)]
    pub label: String,
}

#[derive(Debug, Clone)]
struct SizedBlock {
    id: String,
    block_type: String,
    children: Vec<SizedBlock>,
    columns: i64,
    width_in_columns: i64,
    width: f64,
    height: f64,
    x: f64,
    y: f64,
}

fn json_f64(v: &Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| v.as_i64().map(|n| n as f64))
        .or_else(|| v.as_u64().map(|n| n as f64))
}

fn config_f64(cfg: &Value, path: &[&str]) -> Option<f64> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    json_f64(cur)
}

fn to_sized_block(
    node: &BlockNode,
    padding: f64,
    measurer: &dyn TextMeasurer,
    text_style: &TextStyle,
) -> SizedBlock {
    let columns = node.columns.unwrap_or(-1);
    let width_in_columns = node.width_in_columns.or(node.width).unwrap_or(1).max(1);

    let mut width = 0.0;
    let mut height = 0.0;

    // Mermaid renders block diagram labels via `labelHelper(...)`, which decodes HTML entities
    // and measures the resulting HTML content (`getBoundingClientRect()` for width/height).
    //
    // Block diagrams frequently use `&nbsp;` placeholders (notably for block arrows), so we must
    // decode those before measuring; otherwise node widths drift drastically.
    let label_decoded = node.label.replace("&nbsp;", "\u{00A0}");
    let label_bbox_html =
        measurer.measure_wrapped(&label_decoded, text_style, None, WrapMode::HtmlLike);
    let label_bbox_svg =
        measurer.measure_wrapped(&label_decoded, text_style, None, WrapMode::SvgLike);

    match node.block_type.as_str() {
        // Composite/group blocks can become wider than their children due to their label; Mermaid's
        // `setBlockSizes` grows children to fit when computed width is smaller than the pre-sized
        // label width.
        "composite" | "group" => {
            if !label_decoded.trim().is_empty() {
                // Mermaid uses the measured label helper bbox width directly for composite/group
                // nodes (no extra padding on top of the HTML bbox).
                width = label_bbox_html.width.max(1.0);
                height = (label_bbox_svg.height + padding).max(1.0);
            }
        }
        // Mermaid's dagre wrapper uses a dedicated sizing rule for block arrows:
        // `h = bbox.height + 2 * padding; w = bbox.width + h + padding`.
        "block_arrow" => {
            let h = (label_bbox_svg.height + 2.0 * padding).max(1.0);
            let w = (label_bbox_html.width + h + padding).max(1.0);
            width = w;
            height = h;
        }
        // Regular blocks: `w = bbox.width + padding; h = bbox.height + padding`.
        t if t != "space" => {
            width = (label_bbox_html.width + padding).max(1.0);
            height = (label_bbox_svg.height + padding).max(1.0);
        }
        _ => {}
    }

    let children = node
        .children
        .iter()
        .map(|c| to_sized_block(c, padding, measurer, text_style))
        .collect::<Vec<_>>();

    SizedBlock {
        id: node.id.clone(),
        block_type: node.block_type.clone(),
        children,
        columns,
        width_in_columns,
        width,
        height,
        x: 0.0,
        y: 0.0,
    }
}

fn get_max_child_size(block: &SizedBlock) -> (f64, f64) {
    let mut max_width = 0.0;
    let mut max_height = 0.0;
    for child in &block.children {
        if child.block_type == "space" {
            continue;
        }
        if child.width > max_width {
            max_width = child.width / (block.width_in_columns as f64);
        }
        if child.height > max_height {
            max_height = child.height;
        }
    }
    (max_width, max_height)
}

fn set_block_sizes(block: &mut SizedBlock, padding: f64, sibling_width: f64, sibling_height: f64) {
    if block.width <= 0.0 {
        block.width = sibling_width;
        block.height = sibling_height;
        block.x = 0.0;
        block.y = 0.0;
    }

    if block.children.is_empty() {
        return;
    }

    for child in &mut block.children {
        set_block_sizes(child, padding, 0.0, 0.0);
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
        set_block_sizes(child, padding, max_width, max_height);
    }

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

    let mut width = (x_size as f64) * (max_width + padding) + padding;
    let mut height = (y_size as f64) * (max_height + padding) + padding;

    if width < sibling_width {
        width = sibling_width;
        height = sibling_height;

        let child_width = (sibling_width - (x_size as f64) * padding - padding) / (x_size as f64);
        let child_height = (sibling_height - (y_size as f64) * padding - padding) / (y_size as f64);
        for child in &mut block.children {
            child.width = child_width;
            child.height = child_height;
            child.x = 0.0;
            child.y = 0.0;
        }
    }

    if width < block.width {
        width = block.width;
        let num = if columns > 0 {
            (block.children.len() as i64).min(columns)
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

fn calculate_block_position(columns: i64, position: i64) -> (i64, i64) {
    if columns < 0 {
        return (position, 0);
    }
    if columns == 1 {
        return (0, position);
    }
    (position % columns, position / columns)
}

fn layout_blocks(block: &mut SizedBlock, padding: f64) {
    if block.children.is_empty() {
        return;
    }

    let columns = block.columns;
    let mut column_pos = 0i64;

    // JS truthiness: treat `0` as falsy (Mermaid uses `block?.size?.x ? ... : -padding`).
    let mut starting_pos_x = if block.x != 0.0 {
        block.x + (-block.width / 2.0)
    } else {
        -padding
    };
    let mut row_pos = 0i64;

    for child in &mut block.children {
        let (px, py) = calculate_block_position(columns, column_pos);

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

        child.y = block.y - block.height / 2.0
            + (py as f64) * (child.height + padding)
            + child.height / 2.0
            + padding;

        if !child.children.is_empty() {
            layout_blocks(child, padding);
        }

        let mut columns_filled = child.width_in_columns.max(1);
        if columns > 0 {
            let rem = columns - (column_pos % columns);
            columns_filled = columns_filled.min(rem.max(1));
        }
        column_pos += columns_filled;

        let _ = px;
    }
}

fn find_bounds(block: &SizedBlock, b: &mut Bounds) {
    if block.id != "root" {
        b.min_x = b.min_x.min(block.x - block.width / 2.0);
        b.min_y = b.min_y.min(block.y - block.height / 2.0);
        b.max_x = b.max_x.max(block.x + block.width / 2.0);
        b.max_y = b.max_y.max(block.y + block.height / 2.0);
    }
    for child in &block.children {
        find_bounds(child, b);
    }
}

fn collect_nodes(block: &SizedBlock, out: &mut Vec<LayoutNode>) {
    if block.id != "root" && block.block_type != "space" {
        out.push(LayoutNode {
            id: block.id.clone(),
            x: block.x,
            y: block.y,
            width: block.width,
            height: block.height,
            is_cluster: false,
        });
    }
    for child in &block.children {
        collect_nodes(child, out);
    }
}

pub fn layout_block_diagram(
    semantic: &Value,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
) -> Result<BlockDiagramLayout> {
    let model: BlockDiagramModel = serde_json::from_value(semantic.clone())?;

    let padding = config_f64(effective_config, &["block", "padding"]).unwrap_or(8.0);
    let text_style = crate::text::TextStyle {
        font_family: effective_config
            .get("fontFamily")
            .and_then(|v| v.as_str())
            .or_else(|| {
                effective_config
                    .get("themeVariables")
                    .and_then(|tv| tv.get("fontFamily"))
                    .and_then(|v| v.as_str())
            })
            .map(|s| s.to_string())
            .or_else(|| Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string())),
        font_size: effective_config
            .get("fontSize")
            .and_then(|v| v.as_f64())
            .unwrap_or(16.0)
            .max(1.0),
        font_weight: None,
    };

    let root = model
        .blocks_flat
        .iter()
        .find(|b| b.id == "root" && b.block_type == "composite")
        .ok_or_else(|| Error::InvalidModel {
            message: "missing block root composite".to_string(),
        })?;

    let mut root = to_sized_block(root, padding, measurer, &text_style);
    set_block_sizes(&mut root, padding, 0.0, 0.0);
    layout_blocks(&mut root, padding);

    let mut nodes: Vec<LayoutNode> = Vec::new();
    collect_nodes(&root, &mut nodes);

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
            let metrics =
                measurer.measure_wrapped(&e.label, &TextStyle::default(), None, WrapMode::HtmlLike);
            Some(LayoutLabel {
                x: mid.x,
                y: mid.y,
                width: metrics.width.max(1.0),
                height: metrics.height.max(1.0),
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
        bounds,
    })
}
