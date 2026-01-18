use crate::Result;
use crate::model::{Bounds, KanbanDiagramLayout, KanbanItemLayout, KanbanSectionLayout};
use crate::text::{TextMeasurer, TextStyle};
use serde::Deserialize;

const SECTION_LABEL_HEIGHT_BASELINE: f64 = 25.0;
const SECTION_LABEL_FO_HEIGHT: f64 = 24.0;

const SECTION_PADDING: f64 = 10.0;
const ITEM_ONE_ROW_HEIGHT: f64 = 44.0;
const ITEM_TWO_ROW_HEIGHT: f64 = 56.0;

#[derive(Debug, Clone, Deserialize)]
struct KanbanNode {
    id: String,
    label: String,
    #[serde(default, rename = "isGroup")]
    is_group: bool,
    #[serde(default, rename = "parentId")]
    parent_id: Option<String>,
    #[serde(default)]
    ticket: Option<String>,
    #[serde(default)]
    priority: Option<String>,
    #[serde(default)]
    assigned: Option<String>,
    #[serde(default)]
    icon: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct KanbanModel {
    #[serde(default)]
    nodes: Vec<KanbanNode>,
    #[serde(rename = "type")]
    diagram_type: String,
}

fn cfg_f64(cfg: &serde_json::Value, path: &[&str]) -> Option<f64> {
    let mut cur = cfg;
    for k in path {
        cur = cur.get(*k)?;
    }
    cur.as_f64()
}

pub fn layout_kanban_diagram(
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    measurer: &dyn TextMeasurer,
) -> Result<KanbanDiagramLayout> {
    let model: KanbanModel = serde_json::from_value(semantic.clone())?;
    let _ = model.diagram_type.as_str();

    let section_width = cfg_f64(effective_config, &["kanban", "sectionWidth"])
        .unwrap_or(200.0)
        .max(1.0);

    // Mermaid 11.12.2 has a bug: `kanbanRenderer` uses `conf.mindmap.padding`/`useMaxWidth` when
    // calling `setupGraphViewbox`. Mirror that behavior for parity.
    let viewbox_padding = cfg_f64(effective_config, &["mindmap", "padding"])
        .or_else(|| cfg_f64(effective_config, &["kanban", "padding"]))
        .unwrap_or(8.0)
        .max(0.0);

    let padding = SECTION_PADDING;
    let section_rect_y = -(section_width * 3.0) / 2.0;

    let legend_style = TextStyle::default();
    let mut max_label_height = SECTION_LABEL_HEIGHT_BASELINE;
    let mut sections: Vec<KanbanSectionLayout> = Vec::new();
    let mut items: Vec<KanbanItemLayout> = Vec::new();

    let section_nodes: Vec<&KanbanNode> = model.nodes.iter().filter(|n| n.is_group).collect();
    for (i, section) in section_nodes.iter().enumerate() {
        let index = (i + 1) as i64;
        let center_x = section_width * (index as f64) + ((index - 1) as f64 * padding) / 2.0;
        let center_y = 0.0;

        let label_metrics = measurer.measure(&section.label, &legend_style);
        max_label_height = max_label_height.max(label_metrics.height.max(SECTION_LABEL_FO_HEIGHT));

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
        });
    }

    for section in sections.iter_mut() {
        let top = section_rect_y + max_label_height;
        let mut y = top;

        let section_items: Vec<&KanbanNode> = model
            .nodes
            .iter()
            .filter(|n| n.parent_id.as_deref() == Some(section.id.as_str()))
            .collect();

        for item in section_items {
            let has_details_row = item.ticket.is_some() || item.assigned.is_some();
            let height = if has_details_row {
                ITEM_TWO_ROW_HEIGHT
            } else {
                ITEM_ONE_ROW_HEIGHT
            };

            let center_x = section.center_x;
            let center_y = y + height / 2.0;

            items.push(KanbanItemLayout {
                id: item.id.clone(),
                label: item.label.clone(),
                parent_id: section.id.clone(),
                center_x,
                center_y,
                width: (section_width - 1.5 * padding).max(1.0),
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

        let height = (y - top + 3.0 * padding).max(50.0) + (max_label_height - 25.0);
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
        sections,
        items,
    })
}
