use crate::Result;
use crate::config::{config_f64, config_f64_css_px};
use crate::model::{Bounds, TreeViewDiagramLayout, TreeViewLineLayout, TreeViewNodeLayout};
use crate::text::{TextMeasurer, TextStyle};
use merman_core::diagrams::tree_view::{
    TreeViewDiagramRenderModel, TreeViewNodeRenderModel as TreeViewNode,
};
use serde_json::Value;

#[derive(Debug, Clone)]
struct TreeViewConfig {
    row_indent: f64,
    padding_x: f64,
    padding_y: f64,
    line_thickness: f64,
    use_max_width: bool,
    label_font_size: f64,
}

pub fn layout_tree_view_diagram(
    semantic: &Value,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
) -> Result<TreeViewDiagramLayout> {
    let model: TreeViewDiagramRenderModel = crate::json::from_value_ref(semantic)?;
    layout_tree_view_diagram_typed(&model, effective_config, measurer)
}

pub fn layout_tree_view_diagram_typed(
    model: &TreeViewDiagramRenderModel,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
) -> Result<TreeViewDiagramLayout> {
    let cfg = tree_view_config(effective_config);
    let style = TextStyle {
        font_size: cfg.label_font_size,
        ..Default::default()
    };
    let mut ctx = LayoutCtx {
        cfg: cfg.clone(),
        measurer,
        style,
        total_height: 0.0,
        total_width: 0.0,
        nodes: Vec::new(),
        lines: Vec::new(),
    };

    layout_node(&mut ctx, &model.root, 0)?;

    let min_x = -ctx.cfg.line_thickness / 2.0;
    let total_width = ctx.total_width.max(1.0);
    let total_height = ctx.total_height.max(1.0);
    Ok(TreeViewDiagramLayout {
        bounds: Some(Bounds {
            min_x,
            min_y: 0.0,
            max_x: total_width,
            max_y: total_height,
        }),
        total_width,
        total_height,
        row_indent: ctx.cfg.row_indent,
        padding_x: ctx.cfg.padding_x,
        padding_y: ctx.cfg.padding_y,
        line_thickness: ctx.cfg.line_thickness,
        use_max_width: ctx.cfg.use_max_width,
        label_font_size: ctx.cfg.label_font_size,
        nodes: ctx.nodes,
        lines: ctx.lines,
    })
}

struct LayoutCtx<'a> {
    cfg: TreeViewConfig,
    measurer: &'a dyn TextMeasurer,
    style: TextStyle,
    total_height: f64,
    total_width: f64,
    nodes: Vec<TreeViewNodeLayout>,
    lines: Vec<TreeViewLineLayout>,
}

fn layout_node(ctx: &mut LayoutCtx<'_>, node: &TreeViewNode, depth: usize) -> Result<usize> {
    let indent = depth as f64 * (ctx.cfg.row_indent + ctx.cfg.padding_x);
    let label_width = ctx
        .measurer
        .measure_svg_simple_text_bbox_width_px(&node.name, &ctx.style)
        .max(0.0);
    let label_height = ctx
        .measurer
        .measure_svg_simple_text_bbox_height_px(&node.name, &ctx.style)
        .max(0.0);
    let height = label_height + ctx.cfg.padding_y * 2.0;
    let width = label_width + ctx.cfg.padding_x * 2.0;
    let y = ctx.total_height;
    let idx = ctx.nodes.len();

    ctx.nodes.push(TreeViewNodeLayout {
        id: node.id,
        level: node.level,
        name: node.name.clone(),
        depth,
        x: indent,
        y,
        width,
        height,
        label_x: indent + ctx.cfg.padding_x,
        label_y: y + height / 2.0,
        label_width,
        label_height,
    });
    ctx.lines.push(TreeViewLineLayout {
        x1: indent - ctx.cfg.row_indent,
        y1: y + height / 2.0,
        x2: indent,
        y2: y + height / 2.0,
        stroke_width: ctx.cfg.line_thickness,
        kind: "horizontal".to_string(),
    });

    ctx.total_width = ctx.total_width.max(indent + width);
    ctx.total_height += height;

    let mut direct_child_layouts = Vec::new();
    for child in &node.children {
        direct_child_layouts.push(layout_node(ctx, child, depth + 1)?);
    }

    if let Some(last_child_idx) = direct_child_layouts.last().copied() {
        let current = &ctx.nodes[idx];
        let last_child = &ctx.nodes[last_child_idx];
        ctx.lines.push(TreeViewLineLayout {
            x1: current.x + ctx.cfg.padding_x,
            y1: current.y + current.height,
            x2: current.x + ctx.cfg.padding_x,
            y2: last_child.y + last_child.height / 2.0 + ctx.cfg.line_thickness / 2.0,
            stroke_width: ctx.cfg.line_thickness,
            kind: "vertical".to_string(),
        });
    }

    Ok(idx)
}

fn tree_view_config(effective_config: &Value) -> TreeViewConfig {
    TreeViewConfig {
        row_indent: config_f64(effective_config, &["treeView", "rowIndent"])
            .unwrap_or(10.0)
            .max(0.0),
        padding_x: config_f64(effective_config, &["treeView", "paddingX"])
            .unwrap_or(5.0)
            .max(0.0),
        padding_y: config_f64(effective_config, &["treeView", "paddingY"])
            .unwrap_or(5.0)
            .max(0.0),
        line_thickness: config_f64(effective_config, &["treeView", "lineThickness"])
            .unwrap_or(1.0)
            .max(0.0),
        use_max_width: config_bool(effective_config, &["treeView", "useMaxWidth"]).unwrap_or(true),
        label_font_size: config_f64_css_px(
            effective_config,
            &["themeVariables", "treeView", "labelFontSize"],
        )
        .unwrap_or(16.0)
        .max(1.0),
    }
}

fn config_bool(cfg: &Value, path: &[&str]) -> Option<bool> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_bool()
}
