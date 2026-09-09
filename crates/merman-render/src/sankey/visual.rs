//! Family-owned Sankey visual plan shared by SVG and renderer-neutral backends.

use super::SankeyConfigView;
use crate::model::{Bounds, SankeyDiagramLayout};
use crate::{Error, Result};
use serde_json::Value;
use std::collections::HashMap;

pub(crate) const SANKEY_LABEL_FONT_SIZE_PX: f64 = 14.0;
const SANKEY_LABEL_GAP_X_PX: f64 = 6.0;
const SANKEY_LABEL_HIDE_VALUES_DY_EM: f64 = 0.35;
pub(crate) const SANKEY_LABEL_ASCENT_EM: f64 = 0.9285714286;
pub(crate) const SANKEY_LABEL_DESCENT_EM: f64 = 0.262;
const SANKEY_NODE_COLORS: [&str; 10] = [
    "#4e79a7", "#f28e2c", "#e15759", "#76b7b2", "#59a14f", "#edc949", "#af7aa1", "#ff9da7",
    "#9c755f", "#bab0ab",
];

#[derive(Debug, Clone)]
pub(crate) struct SankeyDrawingTheme {
    pub(crate) font_family_css: String,
    pub(crate) text_color: String,
    pub(crate) label_background: String,
}

impl SankeyDrawingTheme {
    pub(crate) fn new(effective_config: &Value) -> Self {
        let theme_variables = effective_config
            .get("themeVariables")
            .unwrap_or(&Value::Null);
        let theme_string = |key: &str| {
            theme_variables
                .get(key)
                .and_then(Value::as_str)
                .map(str::to_string)
        };
        Self {
            font_family_css: crate::config::config_font_family_css(effective_config),
            text_color: theme_string("textColor").unwrap_or_else(|| "#333".to_string()),
            label_background: theme_string("mainBkg")
                .or_else(|| theme_string("background"))
                .unwrap_or_else(|| "#fff".to_string()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SankeyLabelAnchor {
    Start,
    End,
}

impl SankeyLabelAnchor {
    pub(crate) const fn as_svg(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::End => "end",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SankeyVisualNode {
    pub(crate) id: String,
    pub(crate) value: f64,
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) width: f64,
    pub(crate) height: f64,
    pub(crate) fill: String,
}

#[derive(Debug, Clone)]
pub(crate) struct SankeyVisualLabel {
    pub(crate) node_index: usize,
    pub(crate) text: String,
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) dy_em: f64,
    pub(crate) anchor: SankeyLabelAnchor,
}

impl SankeyVisualLabel {
    pub(crate) fn baseline_y(&self) -> f64 {
        self.y + self.dy_em * SANKEY_LABEL_FONT_SIZE_PX
    }
}

#[derive(Debug, Clone)]
pub(crate) enum SankeyVisualLinkPaint {
    Solid(String),
    LinearGradient {
        start_color: String,
        end_color: String,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct SankeyVisualLink {
    pub(crate) index: usize,
    pub(crate) source: String,
    pub(crate) target: String,
    pub(crate) value: f64,
    pub(crate) start_x: f64,
    pub(crate) start_y: f64,
    pub(crate) control_x: f64,
    pub(crate) end_x: f64,
    pub(crate) end_y: f64,
    pub(crate) width: f64,
    pub(crate) paint: SankeyVisualLinkPaint,
}

#[derive(Debug, Clone)]
pub(crate) struct SankeyVisualPlan {
    pub(crate) bounds: Bounds,
    pub(crate) use_max_width: bool,
    pub(crate) show_values: bool,
    pub(crate) outlined_labels: bool,
    pub(crate) link_color: String,
    pub(crate) theme: SankeyDrawingTheme,
    pub(crate) nodes: Vec<SankeyVisualNode>,
    pub(crate) labels: Vec<SankeyVisualLabel>,
    pub(crate) links: Vec<SankeyVisualLink>,
}

pub(crate) fn build_sankey_visual_plan(
    layout: &SankeyDiagramLayout,
    effective_config: &Value,
) -> Result<SankeyVisualPlan> {
    if ![layout.width, layout.height]
        .into_iter()
        .all(f64::is_finite)
    {
        return Err(invalid("Sankey layout has non-finite root geometry"));
    }
    let layout_width = layout.width.max(1.0);
    let layout_height = layout.height.max(1.0);
    let render_settings = SankeyConfigView::new(effective_config).render_settings();
    let theme = SankeyDrawingTheme::new(effective_config);

    let mut min_x: f64 = 0.0;
    let mut min_y: f64 = 0.0;
    let mut max_x = layout_width;
    let mut max_y = layout_height;
    let mut color_domain = HashMap::<String, usize>::new();
    let mut node_index_by_id = HashMap::<String, usize>::new();
    let mut nodes = Vec::with_capacity(layout.nodes.len());

    for (node_index, node) in layout.nodes.iter().enumerate() {
        if ![node.value, node.x0, node.x1, node.y0, node.y1]
            .into_iter()
            .all(f64::is_finite)
            || node.x1 < node.x0
            || node.y1 < node.y0
        {
            return Err(invalid(format!(
                "Sankey node `{}` has invalid geometry",
                node.id
            )));
        }
        if node_index_by_id
            .insert(node.id.clone(), node_index)
            .is_some()
        {
            return Err(invalid(format!(
                "Sankey layout contains duplicate node `{}`",
                node.id
            )));
        }

        min_x = min_x.min(node.x0);
        min_y = min_y.min(node.y0);
        max_x = max_x.max(node.x1);
        max_y = max_y.max(node.y1);

        let fill = render_settings
            .node_colors
            .and_then(|colors| colors.get(&node.id))
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| {
                let color_index = match color_domain.get(&node.id) {
                    Some(index) => *index,
                    None => {
                        let index = color_domain.len();
                        color_domain.insert(node.id.clone(), index);
                        index
                    }
                };
                SANKEY_NODE_COLORS[color_index % SANKEY_NODE_COLORS.len()].to_string()
            });
        nodes.push(SankeyVisualNode {
            id: node.id.clone(),
            value: node.value,
            x: node.x0,
            y: node.y0,
            width: node.x1 - node.x0,
            height: node.y1 - node.y0,
            fill,
        });
    }

    let mut max_value = 0.0;
    let mut central_node_layer = 0usize;
    for node in &layout.nodes {
        if node.value > max_value {
            max_value = node.value;
            central_node_layer = node.layer;
        }
    }

    let dy_em = if render_settings.show_values {
        0.0
    } else {
        SANKEY_LABEL_HIDE_VALUES_DY_EM
    };
    let labels = layout
        .nodes
        .iter()
        .enumerate()
        .map(|(node_index, node)| {
            let y = (node.y0 + node.y1) / 2.0;
            let (x, anchor) = if render_settings.outlined_labels {
                if node.layer < central_node_layer {
                    (node.x0 - SANKEY_LABEL_GAP_X_PX, SankeyLabelAnchor::End)
                } else {
                    (node.x1 + SANKEY_LABEL_GAP_X_PX, SankeyLabelAnchor::Start)
                }
            } else if node.x0 < layout_width / 2.0 {
                (node.x1 + SANKEY_LABEL_GAP_X_PX, SankeyLabelAnchor::Start)
            } else {
                (node.x0 - SANKEY_LABEL_GAP_X_PX, SankeyLabelAnchor::End)
            };
            let value = (node.value * 100.0).round() / 100.0;
            let text = if render_settings.show_values {
                format!(
                    "{}\n{}{}{}",
                    node.id, render_settings.prefix, value, render_settings.suffix
                )
            } else {
                node.id.clone()
            };
            SankeyVisualLabel {
                node_index,
                text,
                x,
                y,
                dy_em,
                anchor,
            }
        })
        .collect::<Vec<_>>();

    let label_ascent = SANKEY_LABEL_FONT_SIZE_PX * SANKEY_LABEL_ASCENT_EM;
    let label_descent = SANKEY_LABEL_FONT_SIZE_PX * SANKEY_LABEL_DESCENT_EM;
    for label in &labels {
        let baseline_y = label.baseline_y();
        min_y = min_y.min(baseline_y - label_ascent);
        max_y = max_y.max(baseline_y + label_descent);
    }

    let mut links = Vec::with_capacity(layout.links.len());
    for link in &layout.links {
        if ![link.value, link.width, link.y0, link.y1]
            .into_iter()
            .all(f64::is_finite)
        {
            return Err(invalid(format!(
                "Sankey link {} has invalid geometry",
                link.index
            )));
        }
        let source_index = node_index_by_id
            .get(&link.source)
            .copied()
            .ok_or_else(|| invalid(format!("missing source node {}", link.source)))?;
        let target_index = node_index_by_id
            .get(&link.target)
            .copied()
            .ok_or_else(|| invalid(format!("missing target node {}", link.target)))?;
        let source = &layout.nodes[source_index];
        let target = &layout.nodes[target_index];
        let start_x = source.x1;
        let end_x = target.x0;
        let control_x = (start_x + end_x) / 2.0;
        let width = link.width.max(1.0);
        let half_width = width / 2.0;
        min_y = min_y.min(link.y0.min(link.y1) - half_width);
        max_y = max_y.max(link.y0.max(link.y1) + half_width);

        let paint = match render_settings.link_color.as_str() {
            "source" => SankeyVisualLinkPaint::Solid(nodes[source_index].fill.clone()),
            "target" => SankeyVisualLinkPaint::Solid(nodes[target_index].fill.clone()),
            "gradient" => SankeyVisualLinkPaint::LinearGradient {
                start_color: nodes[source_index].fill.clone(),
                end_color: nodes[target_index].fill.clone(),
            },
            other => SankeyVisualLinkPaint::Solid(other.to_string()),
        };
        links.push(SankeyVisualLink {
            index: link.index,
            source: link.source.clone(),
            target: link.target.clone(),
            value: link.value,
            start_x,
            start_y: link.y0,
            control_x,
            end_x,
            end_y: link.y1,
            width,
            paint,
        });
    }

    let bounds = Bounds {
        min_x,
        min_y,
        max_x,
        max_y,
    };
    if ![bounds.min_x, bounds.min_y, bounds.max_x, bounds.max_y]
        .into_iter()
        .all(f64::is_finite)
        || bounds.max_x <= bounds.min_x
        || bounds.max_y <= bounds.min_y
    {
        return Err(invalid("Sankey visual bounds are invalid"));
    }

    Ok(SankeyVisualPlan {
        bounds,
        use_max_width: render_settings.use_max_width,
        show_values: render_settings.show_values,
        outlined_labels: render_settings.outlined_labels,
        link_color: render_settings.link_color,
        theme,
        nodes,
        labels,
        links,
    })
}

fn invalid(message: impl Into<String>) -> Error {
    Error::InvalidModel {
        message: message.into(),
    }
}
