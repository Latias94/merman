use crate::Result;
use crate::model::{Bounds, KanbanDiagramLayout, KanbanItemLayout, KanbanSectionLayout};
use crate::resources::{ModelComplexity, RenderResourcePolicy};
use crate::text::{TextMeasurer, TextMetrics, TextStyle, WrapMode};
use merman_core::diagrams::kanban::{KanbanDiagramRenderModel, KanbanRenderNode};
use std::collections::HashMap;

pub(crate) const KANBAN_SECTION_LABEL_HEIGHT_BASELINE_PX: f64 = 25.0;
pub(crate) const KANBAN_SECTION_PADDING_PX: f64 = 10.0;
pub(crate) const KANBAN_LABEL_FOREIGN_OBJECT_HEIGHT_PX: f64 = 24.0;
const KANBAN_ITEM_ONE_ROW_HEIGHT_PX: f64 = 44.0;
const KANBAN_ITEM_TWO_ROW_HEIGHT_PX: f64 = 56.0;

pub(crate) struct KanbanMarkdown<'a> {
    sanitize_config: &'a merman_core::MermaidConfig,
    auto_wrap: bool,
}

impl<'a> KanbanMarkdown<'a> {
    pub(crate) fn new(effective_config: &'a merman_core::MermaidConfig) -> Self {
        Self {
            sanitize_config: effective_config,
            auto_wrap: crate::config::config_bool(
                effective_config.as_value(),
                &["markdownAutoWrap"],
            )
            .unwrap_or(true),
        }
    }

    pub(crate) fn render(&self, raw: &str) -> String {
        let sanitized = merman_core::sanitize::sanitize_text(raw, self.sanitize_config);
        crate::text::mermaid_markdown_to_xhtml_label_fragment(&sanitized, self.auto_wrap)
    }

    pub(crate) fn measure_html(
        &self,
        measurer: &dyn TextMeasurer,
        html: &str,
        style: &TextStyle,
        max_width: Option<f64>,
    ) -> TextMetrics {
        crate::text::measure_xhtml_label_fragment(
            measurer,
            html,
            style,
            max_width,
            WrapMode::HtmlLike,
        )
    }
}

mod config;

pub(crate) use config::{KanbanConfigView, default_use_max_width};

fn kanban_layout_work_units(model: &KanbanDiagramRenderModel) -> usize {
    let sections = model.nodes.iter().filter(|node| node.is_group).count();
    let items = model
        .nodes
        .iter()
        .filter(|node| node.parent_id.is_some())
        .count();
    model
        .nodes
        .len()
        .saturating_mul(2)
        .saturating_add(sections.saturating_mul(2))
        .saturating_add(items.saturating_mul(3))
}

#[cfg(test)]
pub(crate) fn layout_kanban_diagram_typed(
    model: &KanbanDiagramRenderModel,
    effective_config: &serde_json::Value,
    measurer: &dyn TextMeasurer,
) -> Result<KanbanDiagramLayout> {
    let effective_config = merman_core::MermaidConfig::from_value(effective_config.clone());
    layout_kanban_diagram_typed_with_resource_policy(
        model,
        &effective_config,
        measurer,
        RenderResourcePolicy::interactive(),
    )
}

/// Lays out a Kanban model under the resource policy owned by the render operation.
pub(crate) fn layout_kanban_diagram_typed_with_resource_policy(
    model: &KanbanDiagramRenderModel,
    effective_config: &merman_core::MermaidConfig,
    measurer: &dyn TextMeasurer,
    resource_limits: RenderResourcePolicy,
) -> Result<KanbanDiagramLayout> {
    resource_limits.check_model_complexity(ModelComplexity::from_kanban(model))?;
    resource_limits.check_layout_work_units(kanban_layout_work_units(model))?;
    let cfg = KanbanConfigView::new(effective_config.as_value()).layout_settings();
    let section_width = cfg.section_width;
    let viewbox_padding = cfg.viewbox_padding;
    let padding = KANBAN_SECTION_PADDING_PX;
    let section_rect_y = -(section_width * 3.0) / 2.0;

    let legend_style = cfg.text_style;
    let font_scale = legend_style.font_size / 16.0;
    let section_label_height_baseline = KANBAN_SECTION_LABEL_HEIGHT_BASELINE_PX * font_scale;
    let label_foreign_object_height = KANBAN_LABEL_FOREIGN_OBJECT_HEIGHT_PX * font_scale;
    let item_one_row_height = KANBAN_ITEM_ONE_ROW_HEIGHT_PX * font_scale;
    let item_two_row_height = KANBAN_ITEM_TWO_ROW_HEIGHT_PX * font_scale;
    let markdown = KanbanMarkdown::new(effective_config);

    let mut max_label_height = section_label_height_baseline;
    let mut sections: Vec<KanbanSectionLayout> = Vec::new();
    let mut items: Vec<KanbanItemLayout> = Vec::new();

    let section_nodes: Vec<&KanbanRenderNode> = model.nodes.iter().filter(|n| n.is_group).collect();
    let mut items_by_section: HashMap<&str, Vec<&KanbanRenderNode>> = HashMap::new();
    for node in &model.nodes {
        if let Some(parent_id) = node.parent_id.as_deref() {
            items_by_section.entry(parent_id).or_default().push(node);
        }
    }
    for (i, section) in section_nodes.iter().enumerate() {
        let index = (i + 1) as i64;
        let center_x = section_width * (index as f64) + ((index - 1) as f64 * padding) / 2.0;
        let center_y = 0.0;

        let label_html = markdown.render(&section.label);
        let raw_label_metrics = markdown.measure_html(measurer, &label_html, &legend_style, None);
        let label_metrics = if section_width > 0.0 && raw_label_metrics.width > section_width {
            markdown.measure_html(measurer, &label_html, &legend_style, Some(section_width))
        } else {
            raw_label_metrics
        };
        let label_height = label_metrics.height.max(label_foreign_object_height);
        max_label_height = max_label_height.max(label_height);

        sections.push(KanbanSectionLayout {
            id: section.id.clone(),
            label: section.label.clone(),
            index,
            center_x,
            center_y,
            width: section_width,
            rect_y: section_rect_y,
            rect_height: (section_width * 3.0).max(1.0),
            rx: 5.0,
            ry: 5.0,
            label_width: label_metrics.width.max(0.0),
            label_height,
        });
    }

    for section in sections.iter_mut() {
        let top = section_rect_y + max_label_height;
        let mut y = top;

        for &item in items_by_section
            .get(section.id.as_str())
            .map(Vec::as_slice)
            .unwrap_or_default()
        {
            let width = (section_width - 1.5 * padding).max(1.0);
            let inner_max_w = (width - padding).max(0.0);

            // Mermaid's kanban items are rendered via `kanbanItem.ts`, which uses HTML labels for
            // the title and applies `max-width` clamping when the content needs wrapping. Mirror
            // that behavior so item heights match the upstream bbox-based layout.
            let item_label_style = legend_style.clone();
            let title_html = markdown.render(&item.label);
            let raw_title_metrics =
                markdown.measure_html(measurer, &title_html, &item_label_style, None);
            let title_metrics = if inner_max_w > 0.0 && raw_title_metrics.width > inner_max_w {
                markdown.measure_html(measurer, &title_html, &item_label_style, Some(inner_max_w))
            } else {
                raw_title_metrics
            };

            let has_details_row = item.ticket.is_some() || item.assigned.is_some();
            let base_height = if has_details_row {
                item_two_row_height
            } else {
                item_one_row_height
            };
            let extra_title_height = (title_metrics.height - label_foreign_object_height).max(0.0);
            let height = base_height + extra_title_height;

            let center_x = section.center_x;
            let center_y = y + height / 2.0;

            items.push(KanbanItemLayout {
                id: item.id.clone(),
                label: item.label.clone(),
                parent_id: section.id.clone(),
                center_x,
                center_y,
                width,
                height: height.max(1.0),
                rx: 5.0,
                ry: 5.0,
                ticket: item.ticket.clone(),
                assigned: item.assigned.clone(),
                priority: item.priority.clone(),
                icon: item.icon.clone(),
            });

            y = center_y + height / 2.0 + padding / 2.0;
        }

        let min_section_height = 50.0 * font_scale;
        let height = (y - top + 3.0 * padding).max(min_section_height)
            + (max_label_height - section_label_height_baseline);
        section.rect_height = height.max(1.0);
    }

    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;

    for s in &sections {
        let left = s.center_x - s.width / 2.0;
        let right = left + s.width;
        let top = s.rect_y;
        let bottom = s.rect_y + s.rect_height;
        min_x = min_x.min(left);
        min_y = min_y.min(top);
        max_x = max_x.max(right);
        max_y = max_y.max(bottom);
    }
    for n in &items {
        let left = n.center_x - n.width / 2.0;
        let right = n.center_x + n.width / 2.0;
        let top = n.center_y - n.height / 2.0;
        let bottom = n.center_y + n.height / 2.0;
        min_x = min_x.min(left);
        min_y = min_y.min(top);
        max_x = max_x.max(right);
        max_y = max_y.max(bottom);
    }

    let bounds = if min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite()
    {
        Some(Bounds {
            min_x: min_x - viewbox_padding,
            min_y: min_y - viewbox_padding,
            max_x: max_x + viewbox_padding,
            max_y: max_y + viewbox_padding,
        })
    } else {
        None
    };

    Ok(KanbanDiagramLayout {
        bounds,
        section_width,
        padding,
        max_label_height,
        viewbox_padding,
        use_max_width: cfg.use_max_width,
        sections,
        items,
    })
}

#[cfg(test)]
mod tests {
    use super::layout_kanban_diagram_typed;
    use crate::text::DeterministicTextMeasurer;
    use merman_core::diagrams::kanban::{KanbanDiagramRenderModel, KanbanRenderNode};
    use serde_json::json;

    fn section(id: &str, label: &str) -> KanbanRenderNode {
        let mut node = KanbanRenderNode::new(id, label);
        node.is_group = true;
        node
    }

    fn item(id: &str, label: &str, parent_id: &str) -> KanbanRenderNode {
        let mut node = KanbanRenderNode::new(id, label);
        node.parent_id = Some(parent_id.to_string());
        node
    }

    #[test]
    fn kanban_geometry_constants_match_mermaid() {
        assert_eq!(super::KANBAN_SECTION_LABEL_HEIGHT_BASELINE_PX, 25.0);
        assert_eq!(super::KANBAN_SECTION_PADDING_PX, 10.0);
        assert_eq!(super::KANBAN_LABEL_FOREIGN_OBJECT_HEIGHT_PX, 24.0);
        assert_eq!(super::KANBAN_ITEM_ONE_ROW_HEIGHT_PX, 44.0);
        assert_eq!(super::KANBAN_ITEM_TWO_ROW_HEIGHT_PX, 56.0);
    }

    #[test]
    fn kanban_layout_uses_mermaid_padding() {
        let model = KanbanDiagramRenderModel {
            nodes: vec![
                section("todo", "Todo"),
                section("doing", "Doing"),
                item("task-1", "Task", "todo"),
            ],
        };
        let measurer = DeterministicTextMeasurer {
            char_width_factor: 8.0,
            line_height_factor: 16.0,
        };

        let layout = layout_kanban_diagram_typed(&model, &json!({}), &measurer).unwrap();

        assert_eq!(layout.padding, super::KANBAN_SECTION_PADDING_PX);
        assert!(layout.use_max_width);
        assert_eq!(
            layout.items[0].width,
            layout.section_width - 1.5 * super::KANBAN_SECTION_PADDING_PX
        );
    }

    #[test]
    fn kanban_layout_measures_rendered_markdown_instead_of_source_markers() {
        let markdown_model = KanbanDiagramRenderModel {
            nodes: vec![
                section("todo", "Todo"),
                item("task-1", "*aaaa aaaa aaaaaaa*", "todo"),
                item("task-2", "Next", "todo"),
            ],
        };
        let plain_model = KanbanDiagramRenderModel {
            nodes: vec![
                section("todo", "Todo"),
                item("task-1", "aaaa aaaa aaaaaaa", "todo"),
                item("task-2", "Next", "todo"),
            ],
        };
        let measurer = DeterministicTextMeasurer::default();

        let markdown_layout =
            layout_kanban_diagram_typed(&markdown_model, &json!({}), &measurer).unwrap();
        let plain_layout =
            layout_kanban_diagram_typed(&plain_model, &json!({}), &measurer).unwrap();

        assert_eq!(
            markdown_layout.items[0].height,
            plain_layout.items[0].height
        );
        assert_eq!(
            markdown_layout.items[1].center_y,
            plain_layout.items[1].center_y
        );
        assert_eq!(
            markdown_layout.sections[0].rect_height,
            plain_layout.sections[0].rect_height
        );
        let markdown_bounds = markdown_layout.bounds.as_ref().unwrap();
        let plain_bounds = plain_layout.bounds.as_ref().unwrap();
        assert_eq!(markdown_bounds.min_x, plain_bounds.min_x);
        assert_eq!(markdown_bounds.min_y, plain_bounds.min_y);
        assert_eq!(markdown_bounds.max_x, plain_bounds.max_x);
        assert_eq!(markdown_bounds.max_y, plain_bounds.max_y);
    }

    #[test]
    fn kanban_layout_uses_mermaid_mindmap_viewport_config_precedence() {
        let model = KanbanDiagramRenderModel {
            nodes: vec![section("todo", "Todo")],
        };
        let measurer = DeterministicTextMeasurer {
            char_width_factor: 8.0,
            line_height_factor: 16.0,
        };

        let layout = layout_kanban_diagram_typed(
            &model,
            &json!({
                "mindmap": {
                    "padding": 3,
                    "useMaxWidth": false
                },
                "kanban": {
                    "padding": 12,
                    "useMaxWidth": true
                }
            }),
            &measurer,
        )
        .unwrap();

        assert_eq!(layout.viewbox_padding, 3.0);
        assert!(!layout.use_max_width);
    }
}
