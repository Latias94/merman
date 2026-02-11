use crate::model::{TreemapDiagramLayout, TreemapLeafLayout, TreemapSectionLayout};
use crate::{Error, Result};
use serde::Deserialize;
use serde_json::Value;

const SECTION_INNER_PADDING: f64 = 10.0;
const SECTION_HEADER_HEIGHT: f64 = 25.0;

#[derive(Debug, Clone, Deserialize)]
struct TreemapModel {
    #[serde(rename = "accTitle")]
    acc_title: Option<String>,
    #[serde(rename = "accDescr")]
    acc_descr: Option<String>,
    title: Option<String>,
    root: TreemapNode,
}

#[derive(Debug, Clone, Deserialize)]
struct TreemapNode {
    name: String,
    #[serde(default)]
    children: Option<Vec<TreemapNode>>,
    #[serde(default)]
    value: Option<Value>,
    #[serde(default, rename = "classSelector")]
    class_selector: Option<String>,
    #[serde(default, rename = "cssCompiledStyles")]
    css_compiled_styles: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
struct HierNode {
    name: String,
    own_value: f64,
    value: f64,
    class_selector: Option<String>,
    css_compiled_styles: Option<Vec<String>>,
    parent: Option<usize>,
    children: Vec<usize>,
    depth: usize,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
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

fn config_bool(cfg: &Value, path: &[&str]) -> Option<bool> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_bool()
}

fn config_string(cfg: &Value, path: &[&str]) -> Option<String> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_str().map(|s| s.to_string())
}

#[derive(Debug, Clone)]
struct TreemapConfig {
    use_max_width: bool,
    padding: f64,
    diagram_padding: f64,
    show_values: bool,
    node_width: f64,
    node_height: f64,
    value_format: String,
}

fn treemap_config(effective_config: &Value) -> TreemapConfig {
    // Mermaid 11.12.2 treemap defaults live in `defaultConfig.ts`, not in the YAML schema.
    // Keep these in sync with:
    // - `repo-ref/mermaid/packages/mermaid/src/defaultConfig.ts`
    // - `repo-ref/mermaid/packages/mermaid/src/diagrams/treemap/renderer.ts`
    let use_max_width = config_bool(effective_config, &["treemap", "useMaxWidth"]).unwrap_or(true);
    let padding = config_f64(effective_config, &["treemap", "padding"]).unwrap_or(10.0);
    let diagram_padding =
        config_f64(effective_config, &["treemap", "diagramPadding"]).unwrap_or(8.0);
    let show_values = config_bool(effective_config, &["treemap", "showValues"]).unwrap_or(true);
    let node_width = config_f64(effective_config, &["treemap", "nodeWidth"]).unwrap_or(100.0);
    let node_height = config_f64(effective_config, &["treemap", "nodeHeight"]).unwrap_or(40.0);
    let value_format = config_string(effective_config, &["treemap", "valueFormat"])
        .unwrap_or_else(|| ",".to_string());

    TreemapConfig {
        use_max_width,
        padding,
        diagram_padding,
        show_values,
        node_width,
        node_height,
        value_format,
    }
}

fn push_node(nodes: &mut Vec<HierNode>, node: &TreemapNode, parent: Option<usize>, depth: usize) {
    let own_value = node.value.as_ref().and_then(json_f64).unwrap_or(0.0);
    let idx = nodes.len();
    nodes.push(HierNode {
        name: node.name.clone(),
        own_value,
        value: 0.0,
        class_selector: node.class_selector.clone(),
        css_compiled_styles: node.css_compiled_styles.clone(),
        parent,
        children: Vec::new(),
        depth,
        x0: 0.0,
        y0: 0.0,
        x1: 0.0,
        y1: 0.0,
    });

    if let Some(parent_idx) = parent {
        nodes[parent_idx].children.push(idx);
    }

    if let Some(children) = node.children.as_ref() {
        for child in children {
            push_node(nodes, child, Some(idx), depth + 1);
        }
    }
}

fn compute_sum(nodes: &mut [HierNode], idx: usize) -> f64 {
    let mut sum = nodes[idx].own_value;
    let children = nodes[idx].children.clone();
    for c in children {
        sum += compute_sum(nodes, c);
    }
    nodes[idx].value = sum;
    sum
}

fn sort_children_by_value(nodes: &mut [HierNode], idx: usize) {
    let mut items = nodes[idx]
        .children
        .iter()
        .copied()
        .enumerate()
        .map(|(pos, child)| (child, pos))
        .collect::<Vec<_>>();
    items.sort_by(|(a, a_pos), (b, b_pos)| {
        let av = nodes[*a].value;
        let bv = nodes[*b].value;
        bv.partial_cmp(&av)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a_pos.cmp(b_pos))
    });
    nodes[idx].children = items.into_iter().map(|(child, _pos)| child).collect();

    let children = nodes[idx].children.clone();
    for c in children {
        sort_children_by_value(nodes, c);
    }
}

fn each_before(nodes: &[HierNode], root: usize) -> Vec<usize> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(idx) = stack.pop() {
        out.push(idx);
        let children = &nodes[idx].children;
        for &c in children.iter().rev() {
            stack.push(c);
        }
    }
    out
}

fn descendants_bfs(nodes: &[HierNode], root: usize) -> Vec<usize> {
    let mut out = Vec::new();
    let mut next = vec![root];
    while !next.is_empty() {
        let mut current = next;
        current.reverse();
        next = Vec::new();
        while let Some(idx) = current.pop() {
            out.push(idx);
            for &c in &nodes[idx].children {
                next.push(c);
            }
        }
    }
    out
}

fn leaves_each_before(nodes: &[HierNode], root: usize) -> Vec<usize> {
    let mut out = Vec::new();
    for idx in each_before(nodes, root) {
        if nodes[idx].children.is_empty() {
            out.push(idx);
        }
    }
    out
}

fn treemap_round_node(nodes: &mut [HierNode], idx: usize) {
    nodes[idx].x0 = nodes[idx].x0.round();
    nodes[idx].y0 = nodes[idx].y0.round();
    nodes[idx].x1 = nodes[idx].x1.round();
    nodes[idx].y1 = nodes[idx].y1.round();
}

fn treemap_dice(
    nodes: &mut [HierNode],
    children: &[usize],
    row_value: f64,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
) {
    let mut x = x0;
    let k = if row_value != 0.0 {
        (x1 - x0) / row_value
    } else {
        0.0
    };
    for &child in children {
        nodes[child].y0 = y0;
        nodes[child].y1 = y1;
        nodes[child].x0 = x;
        x += nodes[child].value * k;
        nodes[child].x1 = x;
    }
}

fn treemap_slice(
    nodes: &mut [HierNode],
    children: &[usize],
    row_value: f64,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
) {
    let mut y = y0;
    let k = if row_value != 0.0 {
        (y1 - y0) / row_value
    } else {
        0.0
    };
    for &child in children {
        nodes[child].x0 = x0;
        nodes[child].x1 = x1;
        nodes[child].y0 = y;
        y += nodes[child].value * k;
        nodes[child].y1 = y;
    }
}

fn squarify(nodes: &mut [HierNode], parent: usize, mut x0: f64, mut y0: f64, x1: f64, y1: f64) {
    const PHI: f64 = (1.0 + 2.23606797749979) / 2.0;
    let ratio = PHI;

    let children = nodes[parent].children.clone();
    if children.is_empty() {
        return;
    }

    let n = children.len();
    let mut i0 = 0usize;
    let mut i1 = 0usize;
    let mut value = nodes[parent].value;

    while i0 < n {
        let dx = x1 - x0;
        let dy = y1 - y0;

        let mut sum_value;
        loop {
            if i1 >= n {
                return;
            }
            sum_value = nodes[children[i1]].value;
            i1 += 1;
            if sum_value != 0.0 || i1 >= n {
                break;
            }
        }

        let mut min_value = sum_value;
        let mut max_value = sum_value;

        let alpha = (dy / dx).max(dx / dy) / (value * ratio);
        let mut beta = sum_value * sum_value * alpha;
        let mut min_ratio = (max_value / beta).max(beta / min_value);

        while i1 < n {
            let node_value = nodes[children[i1]].value;
            sum_value += node_value;
            if node_value < min_value {
                min_value = node_value;
            }
            if node_value > max_value {
                max_value = node_value;
            }
            beta = sum_value * sum_value * alpha;
            let new_ratio = (max_value / beta).max(beta / min_value);
            if new_ratio > min_ratio {
                sum_value -= node_value;
                break;
            }
            min_ratio = new_ratio;
            i1 += 1;
        }

        let dice = dx < dy;
        let row_children = &children[i0..i1];
        if dice {
            let y2 = if value != 0.0 {
                y0 + dy * sum_value / value
            } else {
                y1
            };
            treemap_dice(nodes, row_children, sum_value, x0, y0, x1, y2);
            y0 = y2;
        } else {
            let x2 = if value != 0.0 {
                x0 + dx * sum_value / value
            } else {
                x1
            };
            treemap_slice(nodes, row_children, sum_value, x0, y0, x2, y1);
            x0 = x2;
        }

        value -= sum_value;
        i0 = i1;
    }
}

fn position_node(
    nodes: &mut [HierNode],
    idx: usize,
    padding_stack: &mut Vec<f64>,
    padding_inner: f64,
) {
    let depth = nodes[idx].depth;
    if padding_stack.len() <= depth {
        padding_stack.resize(depth + 1, 0.0);
    }
    let mut p = padding_stack[depth];
    let mut x0 = nodes[idx].x0 + p;
    let mut y0 = nodes[idx].y0 + p;
    let mut x1 = nodes[idx].x1 - p;
    let mut y1 = nodes[idx].y1 - p;
    if x1 < x0 {
        x0 = (x0 + x1) / 2.0;
        x1 = x0;
    }
    if y1 < y0 {
        y0 = (y0 + y1) / 2.0;
        y1 = y0;
    }
    nodes[idx].x0 = x0;
    nodes[idx].y0 = y0;
    nodes[idx].x1 = x1;
    nodes[idx].y1 = y1;

    if nodes[idx].children.is_empty() {
        return;
    }

    p = padding_inner / 2.0;
    if padding_stack.len() <= depth + 1 {
        padding_stack.resize(depth + 2, 0.0);
    }
    padding_stack[depth + 1] = p;

    let has_children = true;
    let padding_top = if has_children {
        SECTION_HEADER_HEIGHT + SECTION_INNER_PADDING
    } else {
        0.0
    };
    let padding_lr = if has_children {
        SECTION_INNER_PADDING
    } else {
        0.0
    };
    let padding_bottom = if has_children {
        SECTION_INNER_PADDING
    } else {
        0.0
    };

    x0 += padding_lr - p;
    y0 += padding_top - p;
    x1 -= padding_lr - p;
    y1 -= padding_bottom - p;
    if x1 < x0 {
        x0 = (x0 + x1) / 2.0;
        x1 = x0;
    }
    if y1 < y0 {
        y0 = (y0 + y1) / 2.0;
        y1 = y0;
    }

    squarify(nodes, idx, x0, y0, x1, y1);
}

pub fn layout_treemap_diagram(
    semantic: &Value,
    effective_config: &Value,
    _measurer: &dyn crate::text::TextMeasurer,
) -> Result<TreemapDiagramLayout> {
    let model: TreemapModel = crate::json::from_value_ref(semantic)?;
    let cfg = treemap_config(effective_config);

    let title_height = if model.title.as_deref().is_some_and(|t| !t.trim().is_empty()) {
        30.0
    } else {
        0.0
    };

    let width = if cfg.node_width > 0.0 {
        cfg.node_width * SECTION_INNER_PADDING
    } else {
        960.0
    };
    let height = if cfg.node_height > 0.0 {
        cfg.node_height * SECTION_INNER_PADDING
    } else {
        500.0
    };

    let mut nodes: Vec<HierNode> = Vec::new();
    push_node(&mut nodes, &model.root, None, 0);
    if nodes.is_empty() {
        return Err(Error::InvalidModel {
            message: "treemap root produced no nodes".to_string(),
        });
    }
    let root_idx = 0usize;

    compute_sum(&mut nodes, root_idx);
    sort_children_by_value(&mut nodes, root_idx);

    nodes[root_idx].x0 = 0.0;
    nodes[root_idx].y0 = 0.0;
    nodes[root_idx].x1 = width;
    nodes[root_idx].y1 = height;

    let mut padding_stack = vec![0.0];
    for idx in each_before(&nodes, root_idx) {
        position_node(&mut nodes, idx, &mut padding_stack, cfg.padding.max(0.0));
    }

    for idx in each_before(&nodes, root_idx) {
        treemap_round_node(&mut nodes, idx);
    }

    let branch_nodes = descendants_bfs(&nodes, root_idx)
        .into_iter()
        .filter(|&idx| !nodes[idx].children.is_empty())
        .collect::<Vec<_>>();

    let leaf_nodes = leaves_each_before(&nodes, root_idx);

    let mut sections = Vec::new();
    for idx in &branch_nodes {
        let n = &nodes[*idx];
        sections.push(TreemapSectionLayout {
            name: n.name.clone(),
            depth: n.depth as i64,
            value: n.value,
            x0: n.x0,
            y0: n.y0,
            x1: n.x1,
            y1: n.y1,
            class_selector: n.class_selector.clone(),
            css_compiled_styles: n.css_compiled_styles.clone(),
        });
    }

    let mut leaves = Vec::new();
    for idx in &leaf_nodes {
        let n = &nodes[*idx];
        leaves.push(TreemapLeafLayout {
            name: n.name.clone(),
            value: n.value,
            parent_name: n.parent.map(|p| nodes[p].name.clone()),
            x0: n.x0,
            y0: n.y0,
            x1: n.x1,
            y1: n.y1,
            class_selector: n.class_selector.clone(),
            css_compiled_styles: n.css_compiled_styles.clone(),
        });
    }

    Ok(TreemapDiagramLayout {
        title_height,
        width,
        height,
        use_max_width: cfg.use_max_width,
        diagram_padding: cfg.diagram_padding.max(0.0),
        show_values: cfg.show_values,
        value_format: cfg.value_format,
        acc_title: model.acc_title,
        acc_descr: model.acc_descr,
        title: model.title,
        sections,
        leaves,
    })
}
