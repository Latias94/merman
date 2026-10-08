use crate::config::json_f64;
use crate::model::{TreemapDiagramLayout, TreemapLeafLayout, TreemapSectionLayout};
use crate::resources::OperationWorkMeter;
use crate::{Error, Result};
use merman_core::diagrams::treemap::{
    TreemapDiagramRenderModel, TreemapNodeRenderModel as TreemapNode,
};
use serde_json::Value;

pub(crate) const TREEMAP_SECTION_INNER_PADDING_PX: f64 = 10.0;
pub(crate) const TREEMAP_SECTION_HEADER_HEIGHT_PX: f64 = 25.0;
pub(crate) const TREEMAP_TITLE_CLASS: &str = "treemapTitle";

mod config;
mod theme;

use config::TreemapConfigView;

pub(crate) use theme::{
    TreemapResolvedTextStyle, TreemapTextFillOwnership, TreemapTextRole, TreemapTitleThemePlan,
    TreemapTypographyCssEmission, TreemapTypographyThemePlan,
};

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

fn push_node(
    nodes: &mut Vec<HierNode>,
    node: &TreemapNode,
    parent: Option<usize>,
    depth: usize,
    work_meter: &OperationWorkMeter,
) -> Result<()> {
    let mut stack = vec![(node, parent, depth)];
    while let Some((current, parent_idx, current_depth)) = stack.pop() {
        let children = current.children.as_deref().unwrap_or(&[]);
        work_meter.charge(
            1usize
                .checked_add(children.len())
                .ok_or_else(|| work_meter.arithmetic_overflow())?,
        )?;
        let own_value = current.value.as_ref().and_then(json_f64).unwrap_or(0.0);
        let css_compiled_styles =
            clone_styles_with_work(current.css_compiled_styles.as_deref(), work_meter)?;
        let idx = nodes.len();
        nodes.push(HierNode {
            name: current.name.clone(),
            own_value,
            value: 0.0,
            class_selector: current.class_selector.clone(),
            css_compiled_styles,
            parent: parent_idx,
            children: Vec::new(),
            depth: current_depth,
            x0: 0.0,
            y0: 0.0,
            x1: 0.0,
            y1: 0.0,
        });

        if let Some(parent_idx) = parent_idx
            && let Some(parent_node) = nodes.get_mut(parent_idx)
        {
            parent_node.children.push(idx);
        }

        for child in children.iter().rev() {
            stack.push((child, Some(idx), current_depth.saturating_add(1)));
        }
    }

    Ok(())
}

fn compute_sum(nodes: &mut [HierNode], idx: usize, work_meter: &OperationWorkMeter) -> Result<f64> {
    let mut stack = vec![(idx, false)];
    while let Some((node_idx, visited)) = stack.pop() {
        work_meter.charge(1)?;
        let Some(node) = nodes.get(node_idx) else {
            continue;
        };

        if visited {
            work_meter.charge(node.children.len())?;
            let sum = node.own_value
                + node
                    .children
                    .iter()
                    .filter_map(|&child_idx| nodes.get(child_idx).map(|child| child.value))
                    .sum::<f64>();
            if let Some(node) = nodes.get_mut(node_idx) {
                node.value = sum;
            }
        } else {
            stack.push((node_idx, true));
            for &child_idx in node.children.iter().rev() {
                stack.push((child_idx, false));
            }
        }
    }

    Ok(nodes.get(idx).map(|node| node.value).unwrap_or(0.0))
}

fn checked_sort_work(item_count: usize, work_meter: &OperationWorkMeter) -> Result<usize> {
    if item_count <= 1 {
        return Ok(0);
    }
    item_count
        .checked_mul(item_count.ilog2() as usize + 1)
        .ok_or_else(|| work_meter.arithmetic_overflow().into())
}

fn sort_children_by_value(
    nodes: &mut [HierNode],
    idx: usize,
    work_meter: &OperationWorkMeter,
) -> Result<()> {
    let mut stack = vec![idx];
    while let Some(node_idx) = stack.pop() {
        work_meter.charge(1)?;
        if node_idx >= nodes.len() {
            continue;
        }

        let child_count = nodes[node_idx].children.len();
        let sort_work = checked_sort_work(child_count, work_meter)?;
        let preparation_work = child_count
            .checked_mul(2)
            .and_then(|work| work.checked_add(sort_work))
            .ok_or_else(|| work_meter.arithmetic_overflow())?;
        work_meter.charge(preparation_work)?;
        let mut items = nodes[node_idx]
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
        nodes[node_idx].children = items.into_iter().map(|(child, _pos)| child).collect();

        let children = nodes[node_idx].children.clone();
        for child_idx in children.into_iter().rev() {
            stack.push(child_idx);
        }
    }

    Ok(())
}

fn each_before(
    nodes: &[HierNode],
    root: usize,
    work_meter: &OperationWorkMeter,
) -> Result<Vec<usize>> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(idx) = stack.pop() {
        let children = &nodes[idx].children;
        work_meter.charge(
            1usize
                .checked_add(children.len())
                .ok_or_else(|| work_meter.arithmetic_overflow())?,
        )?;
        out.push(idx);
        for &c in children.iter().rev() {
            stack.push(c);
        }
    }
    Ok(out)
}

fn hier_node_materialization_work(
    node: &HierNode,
    work_meter: &OperationWorkMeter,
) -> Result<usize> {
    let work = 1usize
        .checked_add(node.name.len().div_ceil(64))
        .and_then(|value| {
            value.checked_add(
                node.class_selector
                    .as_deref()
                    .map_or(0, |selector| selector.len().div_ceil(64)),
            )
        })
        .ok_or_else(|| work_meter.arithmetic_overflow())?;
    Ok(work)
}

fn clone_styles_with_work(
    styles: Option<&[String]>,
    work_meter: &OperationWorkMeter,
) -> Result<Option<Vec<String>>> {
    let Some(styles) = styles else {
        return Ok(None);
    };
    let mut cloned = Vec::new();
    for style in styles {
        let work = 1usize
            .checked_add(style.len().div_ceil(64))
            .ok_or_else(|| work_meter.arithmetic_overflow())?;
        // Charge before cloning so a rejected expansion never allocates the current declaration.
        work_meter.charge(work)?;
        cloned.push(style.clone());
    }
    Ok(Some(cloned))
}

fn join_class_styles_with_work(
    styles: &[String],
    work_meter: &OperationWorkMeter,
) -> Result<Option<Vec<String>>> {
    if styles.is_empty() {
        return Ok(None);
    }

    let total_bytes = styles.iter().try_fold(0usize, |total, style| {
        total
            .checked_add(style.len())
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| work_meter.arithmetic_overflow())
    })?;
    work_meter.charge(
        1usize
            .checked_add(total_bytes.div_ceil(64))
            .ok_or_else(|| work_meter.arithmetic_overflow())?,
    )?;

    let mut joined = String::with_capacity(total_bytes.saturating_sub(1));
    for (index, style) in styles.iter().enumerate() {
        if index != 0 {
            joined.push(';');
        }
        joined.push_str(style);
    }
    Ok(Some(vec![joined]))
}

fn clone_node_styles_with_work(
    node: &HierNode,
    classes: &std::collections::BTreeMap<
        String,
        merman_core::diagrams::treemap::TreemapClassDefRenderModel,
    >,
    work_meter: &OperationWorkMeter,
) -> Result<Option<Vec<String>>> {
    if let Some(styles) = node.css_compiled_styles.as_deref() {
        return clone_styles_with_work(Some(styles), work_meter);
    }

    let Some(selector) = node.class_selector.as_deref() else {
        return Ok(None);
    };
    let Some(class_def) = classes.get(selector) else {
        return Ok(None);
    };
    if class_def.styles.is_empty() {
        return Ok(None);
    }
    // Mermaid's render model stores a classDef's compiled declaration list as one
    // CSS string.  Keep that shape at the output boundary; the renderer's
    // declaration parser still resolves individual properties later.
    join_class_styles_with_work(class_def.styles.as_slice(), work_meter)
}

fn descendants_bfs(
    nodes: &[HierNode],
    root: usize,
    work_meter: &OperationWorkMeter,
) -> Result<Vec<usize>> {
    let mut out = Vec::new();
    let mut next = vec![root];
    while !next.is_empty() {
        let mut current = next;
        current.reverse();
        next = Vec::new();
        while let Some(idx) = current.pop() {
            work_meter.charge(
                1usize
                    .checked_add(nodes[idx].children.len())
                    .ok_or_else(|| work_meter.arithmetic_overflow())?,
            )?;
            out.push(idx);
            for &c in &nodes[idx].children {
                next.push(c);
            }
        }
    }
    Ok(out)
}

fn treemap_round_node(nodes: &mut [HierNode], idx: usize) {
    nodes[idx].x0 = nodes[idx].x0.round();
    nodes[idx].y0 = nodes[idx].y0.round();
    nodes[idx].x1 = nodes[idx].x1.round();
    nodes[idx].y1 = nodes[idx].y1.round();
}

#[allow(
    clippy::too_many_arguments,
    reason = "The row tiler takes explicit rectangular bounds and the operation work meter."
)]
fn treemap_dice(
    nodes: &mut [HierNode],
    children: &[usize],
    row_value: f64,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    work_meter: &OperationWorkMeter,
) -> Result<()> {
    work_meter.charge(children.len())?;
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

    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "The row tiler takes explicit rectangular bounds and the operation work meter."
)]
fn treemap_slice(
    nodes: &mut [HierNode],
    children: &[usize],
    row_value: f64,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    work_meter: &OperationWorkMeter,
) -> Result<()> {
    work_meter.charge(children.len())?;
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

    Ok(())
}

fn squarify(
    nodes: &mut [HierNode],
    parent: usize,
    mut x0: f64,
    mut y0: f64,
    x1: f64,
    y1: f64,
    work_meter: &OperationWorkMeter,
) -> Result<()> {
    const PHI: f64 = (1.0 + 2.23606797749979) / 2.0;
    let ratio = PHI;

    work_meter.charge(nodes[parent].children.len())?;
    let children = nodes[parent].children.clone();
    if children.is_empty() {
        return Ok(());
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
                return Ok(());
            }
            work_meter.charge(1)?;
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
            work_meter.charge(1)?;
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
            treemap_dice(nodes, row_children, sum_value, x0, y0, x1, y2, work_meter)?;
            y0 = y2;
        } else {
            let x2 = if value != 0.0 {
                x0 + dx * sum_value / value
            } else {
                x1
            };
            treemap_slice(nodes, row_children, sum_value, x0, y0, x2, y1, work_meter)?;
            x0 = x2;
        }

        value -= sum_value;
        i0 = i1;
    }

    Ok(())
}

fn position_node(
    nodes: &mut [HierNode],
    idx: usize,
    padding_stack: &mut Vec<f64>,
    padding_inner: f64,
    work_meter: &OperationWorkMeter,
) -> Result<()> {
    work_meter.charge(1)?;
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
        return Ok(());
    }

    p = padding_inner / 2.0;
    if padding_stack.len() <= depth + 1 {
        padding_stack.resize(depth + 2, 0.0);
    }
    padding_stack[depth + 1] = p;

    let padding_top = TREEMAP_SECTION_HEADER_HEIGHT_PX + TREEMAP_SECTION_INNER_PADDING_PX;
    let padding_lr = TREEMAP_SECTION_INNER_PADDING_PX;
    let padding_bottom = TREEMAP_SECTION_INNER_PADDING_PX;

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

    squarify(nodes, idx, x0, y0, x1, y1, work_meter)
}

pub(crate) fn layout_treemap_diagram_typed_with_work_meter(
    model: &TreemapDiagramRenderModel,
    diagram_title: Option<&str>,
    effective_config: &Value,
    work_meter: &OperationWorkMeter,
) -> Result<TreemapDiagramLayout> {
    work_meter.charge(1)?;
    let cfg = TreemapConfigView::new(effective_config).layout_settings();
    let title = model
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .or_else(|| {
            diagram_title
                .map(str::trim)
                .filter(|title| !title.is_empty())
        })
        .map(str::to_owned);

    let title_height = if title.is_some() { 30.0 } else { 0.0 };

    let width = if cfg.node_width > 0.0 {
        cfg.node_width * TREEMAP_SECTION_INNER_PADDING_PX
    } else {
        960.0
    };
    let height = if cfg.node_height > 0.0 {
        cfg.node_height * TREEMAP_SECTION_INNER_PADDING_PX
    } else {
        500.0
    };

    let mut nodes: Vec<HierNode> = Vec::new();
    push_node(&mut nodes, &model.root, None, 0, work_meter)?;
    if nodes.is_empty() {
        return Err(Error::InvalidModel {
            message: "treemap root produced no nodes".to_string(),
        });
    }
    let root_idx = 0usize;

    compute_sum(&mut nodes, root_idx, work_meter)?;
    sort_children_by_value(&mut nodes, root_idx, work_meter)?;

    nodes[root_idx].x0 = 0.0;
    nodes[root_idx].y0 = 0.0;
    nodes[root_idx].x1 = width;
    nodes[root_idx].y1 = height;

    let preorder = each_before(&nodes, root_idx, work_meter)?;
    let mut padding_stack = vec![0.0];
    for &idx in &preorder {
        position_node(
            &mut nodes,
            idx,
            &mut padding_stack,
            cfg.padding.max(0.0),
            work_meter,
        )?;
    }

    work_meter.charge(preorder.len())?;
    for &idx in &preorder {
        treemap_round_node(&mut nodes, idx);
    }

    let branch_nodes = descendants_bfs(&nodes, root_idx, work_meter)?
        .into_iter()
        .filter(|&idx| !nodes[idx].children.is_empty())
        .collect::<Vec<_>>();

    let mut sections = Vec::with_capacity(branch_nodes.len());
    for idx in &branch_nodes {
        work_meter.charge(hier_node_materialization_work(&nodes[*idx], work_meter)?)?;
        let n = &nodes[*idx];
        let css_compiled_styles = clone_node_styles_with_work(n, &model.classes, work_meter)?;
        sections.push(TreemapSectionLayout {
            name: n.name.clone(),
            depth: n.depth as i64,
            value: n.value,
            x0: n.x0,
            y0: n.y0,
            x1: n.x1,
            y1: n.y1,
            class_selector: n.class_selector.clone(),
            css_compiled_styles,
        });
    }

    let mut leaves = Vec::with_capacity(preorder.len().saturating_sub(branch_nodes.len()));
    for &idx in &preorder {
        if !nodes[idx].children.is_empty() {
            continue;
        }
        work_meter.charge(hier_node_materialization_work(&nodes[idx], work_meter)?)?;
        let n = &nodes[idx];
        let css_compiled_styles = clone_node_styles_with_work(n, &model.classes, work_meter)?;
        leaves.push(TreemapLeafLayout {
            name: n.name.clone(),
            value: n.value,
            parent_name: n.parent.map(|p| nodes[p].name.clone()),
            x0: n.x0,
            y0: n.y0,
            x1: n.x1,
            y1: n.y1,
            class_selector: n.class_selector.clone(),
            css_compiled_styles,
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
        acc_title: model.acc_title.clone(),
        acc_descr: model.acc_descr.clone(),
        title,
        sections,
        leaves,
    })
}

#[cfg(test)]
mod tests {
    use merman_core::diagrams::treemap::{
        TreemapClassDefRenderModel, TreemapDiagramRenderModel, TreemapNodeRenderModel,
    };

    use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};

    fn work_budget_fixture() -> TreemapDiagramRenderModel {
        TreemapDiagramRenderModel {
            root: TreemapNodeRenderModel {
                name: "root".to_string(),
                children: Some(vec![TreemapNodeRenderModel {
                    name: "Section".to_string(),
                    children: Some(vec![
                        TreemapNodeRenderModel {
                            name: "Alpha".to_string(),
                            value: Some(serde_json::json!(3)),
                            ..TreemapNodeRenderModel::default()
                        },
                        TreemapNodeRenderModel {
                            name: "Beta".to_string(),
                            value: Some(serde_json::json!(2)),
                            ..TreemapNodeRenderModel::default()
                        },
                    ]),
                    ..TreemapNodeRenderModel::default()
                }]),
                ..TreemapNodeRenderModel::default()
            },
            ..TreemapDiagramRenderModel::default()
        }
    }

    fn shared_class_fixture(styles: Vec<String>) -> TreemapDiagramRenderModel {
        let mut model = work_budget_fixture();
        model.root.children.as_mut().unwrap()[0].class_selector = Some("important".to_string());
        model.classes.insert(
            "important".to_string(),
            TreemapClassDefRenderModel {
                id: "important".to_string(),
                styles,
                text_styles: Vec::new(),
            },
        );
        model
    }

    #[test]
    fn treemap_geometry_constants_match_mermaid() {
        assert_eq!(super::TREEMAP_SECTION_INNER_PADDING_PX, 10.0);
        assert_eq!(super::TREEMAP_SECTION_HEADER_HEIGHT_PX, 25.0);
    }

    #[test]
    fn treemap_layout_resolves_class_styles_at_the_output_boundary() {
        let model = shared_class_fixture(vec!["fill:#f96".to_string(), "stroke:#333".to_string()]);
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let layout = super::layout_treemap_diagram_typed_with_work_meter(
            &model,
            None,
            &serde_json::json!({}),
            &meter,
        )
        .expect("Treemap layout with shared class styles");

        let section = layout
            .sections
            .iter()
            .find(|section| section.name == "Section")
            .expect("styled section");
        assert_eq!(
            section.css_compiled_styles.as_ref(),
            Some(&vec!["fill:#f96;stroke:#333".to_string()])
        );
    }

    #[test]
    fn treemap_layout_charges_class_style_materialization() {
        let styles = vec![format!("fill:{}", "#abcdef".repeat(128))];
        let model = shared_class_fixture(styles);
        let mut without_class_styles = model.clone();
        without_class_styles.classes.clear();

        let baseline_meter =
            OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        super::layout_treemap_diagram_typed_with_work_meter(
            &without_class_styles,
            None,
            &serde_json::json!({}),
            &baseline_meter,
        )
        .expect("unstyled Treemap layout");

        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, baseline_meter.used())
            .expect("valid exact Treemap work ceiling");
        let meter = OperationWorkMeter::new(policy);
        let error = super::layout_treemap_diagram_typed_with_work_meter(
            &model,
            None,
            &serde_json::json!({}),
            &meter,
        )
        .expect_err("class style materialization must be budgeted");

        let crate::Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected Treemap layout work rejection, got {error}");
        };
        assert_eq!(limit.limit, ResourceLimitId::MaxLayoutWorkUnits.as_str());
        assert!(limit.actual > limit.max);
    }

    #[test]
    fn treemap_layout_obeys_the_cumulative_work_budget_at_the_exact_boundary() {
        let model = work_budget_fixture();
        let config = serde_json::json!({});
        let baseline_meter =
            OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let baseline = super::layout_treemap_diagram_typed_with_work_meter(
            &model,
            None,
            &config,
            &baseline_meter,
        )
        .expect("unbounded Treemap layout");
        let exact_work = baseline_meter.used();
        assert!(exact_work > 1);
        assert_eq!(baseline.sections.len(), 2);
        assert_eq!(baseline.leaves.len(), 2);

        let exact_policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, exact_work)
            .expect("valid exact Treemap work ceiling");
        let exact_meter = OperationWorkMeter::new(exact_policy);
        let exact = super::layout_treemap_diagram_typed_with_work_meter(
            &model,
            None,
            &config,
            &exact_meter,
        )
        .expect("exact Treemap work ceiling");
        assert_eq!(exact.sections.len(), baseline.sections.len());
        assert_eq!(exact.leaves.len(), baseline.leaves.len());
        assert_eq!(exact_meter.used(), exact_work);

        let below_policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, exact_work - 1)
            .expect("valid below-exact Treemap work ceiling");
        let below_meter = OperationWorkMeter::new(below_policy);
        let error = super::layout_treemap_diagram_typed_with_work_meter(
            &model,
            None,
            &config,
            &below_meter,
        )
        .expect_err("below-exact Treemap work ceiling must fail");
        let crate::Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected Treemap layout work rejection, got {error}");
        };
        assert_eq!(limit.limit, ResourceLimitId::MaxLayoutWorkUnits.as_str());
        assert_eq!(limit.max, exact_work - 1);
        assert!(limit.actual > limit.max);
    }
}
