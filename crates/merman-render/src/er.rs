use crate::model::{Bounds, ErDiagramLayout, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint};
use crate::text::{
    TextMeasurer, TextMetrics, TextStyle, WrapMode, measure_mermaid_text_dimensions,
};
use crate::{Error, Result};
use dugong::graphlib::{Graph, GraphOptions};
use dugong::{EdgeLabel, GraphLabel, LabelPos, NodeLabel};
#[cfg(feature = "layout-elk")]
use merman_layout_elk as elk;
use serde_json::Value;
use std::collections::HashMap;

mod config;

pub(crate) use config::{ErConfigView, ErEntityMeasurementSettings};
use config::{ErLayoutAlgorithm, ErLayoutSettings};

pub(crate) type ErEntity = merman_core::diagrams::er::ErEntityRenderModel;
pub(crate) type ErRelationship = merman_core::diagrams::er::ErRelationshipRenderModel;
pub(crate) type ErClassDef = merman_core::diagrams::er::ErClassDefRenderModel;

pub(crate) fn uses_elk_layout(effective_config: &Value) -> bool {
    ErConfigView::new(effective_config).is_elk_layout()
}

#[derive(Debug, Clone)]
pub(crate) struct ErBoxLabel {
    markdown_input: String,
    rendered_text: String,
    xhtml_fragment: String,
    generic_workaround: bool,
}

impl ErBoxLabel {
    pub(crate) fn from_source(source: &str) -> Self {
        let decoded = merman_core::entities::decode_mermaid_entities_to_unicode(source);
        let source = decoded.as_ref().trim();
        let parsed = merman_core::common::parse_generic_types(source);
        let generic_workaround = parsed != source;
        let markdown_input = if generic_workaround {
            parsed.replace('<', "&lt;").replace('>', "&gt;")
        } else {
            source.to_string()
        };
        let xhtml_fragment =
            crate::text::mermaid_markdown_to_xhtml_label_fragment(&markdown_input, true);
        let rendered_text = if generic_workaround {
            crate::text::mermaid_xhtml_label_text_content(&xhtml_fragment).unwrap_or(parsed)
        } else {
            source.to_string()
        };

        Self {
            markdown_input,
            rendered_text,
            xhtml_fragment,
            generic_workaround,
        }
    }

    pub(crate) fn markdown_input(&self) -> &str {
        &self.markdown_input
    }

    pub(crate) fn rendered_text(&self) -> &str {
        &self.rendered_text
    }

    pub(crate) fn xhtml_fragment(&self) -> &str {
        &self.xhtml_fragment
    }

    pub(crate) fn uses_generic_workaround(&self) -> bool {
        self.generic_workaround
    }
}

pub(crate) fn er_box_label_metrics(
    label: &ErBoxLabel,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
) -> TextMetrics {
    if label.rendered_text().is_empty() {
        return TextMetrics {
            width: 0.0,
            height: 0.0,
            line_count: 0,
        };
    }

    if label.uses_generic_workaround() {
        measurer.measure_wrapped(label.rendered_text(), style, None, WrapMode::HtmlLike)
    } else {
        crate::text::measure_xhtml_label_fragment(
            measurer,
            label.xhtml_fragment(),
            style,
            None,
            WrapMode::HtmlLike,
        )
    }
}

pub(crate) fn calculate_text_width_like_mermaid_px(
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    text: &str,
) -> i64 {
    measure_mermaid_text_dimensions(measurer, text, style).width
}

#[derive(Debug, Clone)]
pub(crate) struct ErEntityMeasureRow {
    pub type_label: ErBoxLabel,
    pub name_label: ErBoxLabel,
    pub key_label: ErBoxLabel,
    pub comment_label: ErBoxLabel,
    pub height: f64,
}

#[derive(Debug, Clone)]
pub(crate) struct ErEntityMeasure {
    pub width: f64,
    pub height: f64,
    pub text_padding: f64,
    pub label: ErBoxLabel,
    pub label_html_width: f64,
    pub label_height: f64,
    pub label_max_width_px: i64,
    pub has_key: bool,
    pub has_comment: bool,
    pub type_col_w: f64,
    pub name_col_w: f64,
    pub key_col_w: f64,
    pub comment_col_w: f64,
    pub rows: Vec<ErEntityMeasureRow>,
}

pub(crate) fn measure_entity_box(
    entity: &ErEntity,
    measurer: &dyn TextMeasurer,
    label_style: &TextStyle,
    attr_style: &TextStyle,
    settings: ErEntityMeasurementSettings,
) -> ErEntityMeasure {
    // Mermaid measures ER attribute-table text through HTML labels (`foreignObject`). Consume the
    // operation-owned measurement directly: browser hosts provide DOM metrics and the vendored
    // profile is the explicit headless fallback.

    // Mermaid's ER renderer (erBox.ts) uses `config.htmlLabels` inconsistently:
    // - It passes `useHtmlLabels: config.htmlLabels` into `createText`, where `undefined`
    //   effectively behaves as `true` due to JS default parameters.
    // - It uses `if (!config.htmlLabels) { PADDING *= 1.25; TEXT_PADDING *= 1.25; }`, where
    //   `undefined` behaves as `false` and triggers the multiplier even when HTML labels are used.
    //
    // Upstream SVG fixtures at Mermaid@11.12.2 reflect this quirk. The padding multiplier still
    // keys off the raw truthiness (`undefined` behaves like `false`) even though the rendered labels
    // use HTML `<foreignObject>` output by default.
    let html_labels_raw = settings.html_labels_raw;

    // Mermaid ER unified shape (`erBox.ts`) uses:
    // - PADDING = config.er.diagramPadding (default 20 in Mermaid 11.12.2 schema defaults)
    // - TEXT_PADDING = config.er.entityPadding (default 15)
    let mut padding = settings.diagram_padding;
    let mut text_padding = settings.entity_padding;
    let min_w = settings.min_entity_width;
    let wrapping_width_px = settings.wrapping_width_px;

    let label_source = if entity.alias.trim().is_empty() {
        entity.label.as_str()
    } else {
        entity.alias.as_str()
    };
    let label = ErBoxLabel::from_source(label_source);
    let label_metrics = er_box_label_metrics(&label, measurer, label_style);
    let label_html_width = label_metrics.width.max(0.0);

    // No attributes: use `drawRect`-like padding rules from Mermaid erBox.ts.
    if entity.attributes.is_empty() {
        let label_pad_x = padding;
        let label_pad_y = padding * 1.5;
        // Mermaid's `drawRect` branch clamps to `minEntityWidth` based on `calculateTextWidth()`,
        // not on the HTML label bbox. Preserve that quirk: upstream can end up with nodes that are
        // narrower than `minEntityWidth` when `calculateTextWidth()` is larger than the HTML bbox
        // used by `drawRect`.
        let calc_w =
            calculate_text_width_like_mermaid_px(measurer, label_style, label.markdown_input());
        let clamp_to_min_w = (calc_w as f64 + label_pad_x * 2.0) < min_w;
        let width = if clamp_to_min_w {
            min_w
        } else {
            label_html_width + label_pad_x * 2.0
        };
        let height = label_metrics.height + label_pad_y * 2.0;
        return ErEntityMeasure {
            width: width.max(1.0),
            height: height.max(1.0),
            text_padding,
            label,
            label_html_width,
            label_height: label_metrics.height.max(0.0),
            label_max_width_px: if clamp_to_min_w {
                min_w.round().max(0.0) as i64
            } else {
                wrapping_width_px
            },
            has_key: false,
            has_comment: false,
            type_col_w: 0.0,
            name_col_w: 0.0,
            key_col_w: 0.0,
            comment_col_w: 0.0,
            rows: Vec::new(),
        };
    }

    // Mermaid erBox.ts only applies the `* 1.25` multiplier after the "drawRect" early-return.
    // Keep that behavior: nodes without an attribute table should *not* inherit the multiplier.
    if !html_labels_raw {
        padding *= 1.25;
        text_padding *= 1.25;
    }

    let mut rows: Vec<ErEntityMeasureRow> = Vec::new();

    let mut max_type_raw_w: f64 = 0.0;
    let mut max_name_raw_w: f64 = 0.0;
    let mut max_keys_raw_w: f64 = 0.0;
    let mut max_comment_raw_w: f64 = 0.0;

    let mut max_type_col_w: f64 = 0.0;
    let mut max_name_col_w: f64 = 0.0;
    let mut max_keys_col_w: f64 = 0.0;
    let mut max_comment_col_w: f64 = 0.0;

    let mut total_rows_h = 0.0;

    for a in &entity.attributes {
        let type_label = ErBoxLabel::from_source(&a.ty);
        let name_label = ErBoxLabel::from_source(&a.name);
        let type_m = er_box_label_metrics(&type_label, measurer, attr_style);
        let name_m = er_box_label_metrics(&name_label, measurer, attr_style);

        let type_w = type_m.width;
        let name_w = name_m.width;
        max_type_raw_w = max_type_raw_w.max(type_w);
        max_name_raw_w = max_name_raw_w.max(name_w);
        max_type_col_w = max_type_col_w.max(type_w + padding);
        max_name_col_w = max_name_col_w.max(name_w + padding);

        let key_label = ErBoxLabel::from_source(&a.keys.join(","));
        let keys_m = er_box_label_metrics(&key_label, measurer, attr_style);
        let keys_w = keys_m.width;
        max_keys_raw_w = max_keys_raw_w.max(keys_w);
        max_keys_col_w = max_keys_col_w.max(keys_w + padding);

        let comment_label = ErBoxLabel::from_source(&a.comment);
        let comment_m = er_box_label_metrics(&comment_label, measurer, attr_style);
        let comment_w = comment_m.width;
        max_comment_raw_w = max_comment_raw_w.max(comment_w);
        max_comment_col_w = max_comment_col_w.max(comment_w + padding);

        let row_h = type_m
            .height
            .max(name_m.height)
            .max(keys_m.height)
            .max(comment_m.height)
            + text_padding;

        rows.push(ErEntityMeasureRow {
            type_label,
            name_label,
            key_label,
            comment_label,
            height: row_h.max(1.0),
        });
        total_rows_h += row_h.max(1.0);
    }

    let mut total_width_sections = 4usize;
    let mut has_key = true;
    let mut has_comment = true;
    if max_keys_col_w <= padding {
        has_key = false;
        max_keys_col_w = 0.0;
        total_width_sections = total_width_sections.saturating_sub(1);
    }
    if max_comment_col_w <= padding {
        has_comment = false;
        max_comment_col_w = 0.0;
        total_width_sections = total_width_sections.saturating_sub(1);
    }

    // Mermaid adds extra padding to attribute components to accommodate the entity name width.
    // Mermaid uses the HTML label bbox (`getBoundingClientRect`) as `nameBBox.width`.
    let name_w_min = label_html_width + padding * 2.0;
    let mut max_width = max_type_col_w + max_name_col_w + max_keys_col_w + max_comment_col_w;
    if name_w_min - max_width > 0.0 && total_width_sections > 0 {
        let diff = name_w_min - max_width;
        let per = diff / total_width_sections as f64;
        max_type_col_w += per;
        max_name_col_w += per;
        if has_key {
            max_keys_col_w += per;
        }
        if has_comment {
            max_comment_col_w += per;
        }
        max_width = max_type_col_w + max_name_col_w + max_keys_col_w + max_comment_col_w;
    }

    let shape_bbox_w = label_html_width
        .max(max_type_raw_w)
        .max(max_name_raw_w)
        .max(max_keys_raw_w)
        .max(max_comment_raw_w);

    let width = (shape_bbox_w + padding * 2.0).max(max_width);
    let name_h = label_metrics.height + text_padding;
    let height = total_rows_h + name_h;

    ErEntityMeasure {
        width: width.max(1.0),
        height: height.max(1.0),
        text_padding,
        label,
        label_html_width,
        label_height: label_metrics.height.max(0.0),
        label_max_width_px: wrapping_width_px,
        has_key,
        has_comment,
        type_col_w: max_type_col_w.max(0.0),
        name_col_w: max_name_col_w.max(0.0),
        key_col_w: max_keys_col_w.max(0.0),
        comment_col_w: max_comment_col_w.max(0.0),
        rows,
    }
}

fn entity_box_dimensions(
    entity: &ErEntity,
    measurer: &dyn TextMeasurer,
    label_style: &TextStyle,
    attr_style: &TextStyle,
    settings: ErEntityMeasurementSettings,
) -> (f64, f64) {
    let m = measure_entity_box(entity, measurer, label_style, attr_style, settings);
    (m.width, m.height)
}

fn edge_label_metrics(
    text: &str,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    html_labels: bool,
) -> (f64, f64) {
    let text = text.trim();
    if text.is_empty() {
        return (0.0, 0.0);
    }

    let wrap_mode = if html_labels {
        WrapMode::HtmlLike
    } else {
        WrapMode::SvgLike
    };

    // Mermaid ER relationship labels follow Mermaid's effective HTML-label resolution:
    // root `htmlLabels` first, then `flowchart.htmlLabels`, then default `true`.
    // - HTML mode uses the generic HTML edge-label path (`foreignObject`, line-height 1.5)
    // - SVG mode uses `createFormattedText(...)` (`<text>/<tspan>`, line-height 1.1)
    // Markdown emphasis is tokenized in both branches before the final DOM shape is emitted.
    let fragment = crate::text::mermaid_markdown_to_xhtml_label_fragment(text, true);
    let m = if html_labels {
        crate::text::measure_xhtml_label_fragment(measurer, &fragment, style, None, wrap_mode)
    } else if let Some(plain_text) = crate::text::mermaid_xhtml_label_plain_text(&fragment) {
        measurer.measure_wrapped(&plain_text, style, None, wrap_mode)
    } else {
        crate::text::measure_markdown_with_inline_styles(measurer, text, style, None, wrap_mode)
    };
    (m.width.max(0.0), m.height.max(0.0))
}

fn parse_er_rel_idx_from_edge_name(name: &str) -> Option<usize> {
    let rest = name.strip_prefix("er-rel-")?;
    let mut end = 0usize;
    for (idx, ch) in rest.char_indices() {
        if !ch.is_ascii_digit() {
            break;
        }
        end = idx + ch.len_utf8();
    }
    if end == 0 {
        return None;
    }
    rest[..end].parse::<usize>().ok()
}

fn is_er_self_loop_dummy_node_id(id: &str) -> bool {
    // Mermaid's dagre renderer creates self-loop helper nodes using `${nodeId}---${nodeId}---{1|2}`.
    id.contains("---")
}

#[derive(Debug, Clone)]
struct LayoutEdgeParts {
    id: String,
    from: String,
    to: String,
    points: Vec<LayoutPoint>,
    label: Option<LayoutLabel>,
    start_marker: Option<String>,
    end_marker: Option<String>,
    stroke_dasharray: Option<String>,
}

fn calc_label_position(points: &[LayoutPoint]) -> Option<(f64, f64)> {
    if points.is_empty() {
        return None;
    }
    if points.len() == 1 {
        return Some((points[0].x, points[0].y));
    }

    let mut total = 0.0;
    for i in 1..points.len() {
        let dx = points[i].x - points[i - 1].x;
        let dy = points[i].y - points[i - 1].y;
        total += (dx * dx + dy * dy).sqrt();
    }
    let mut remaining = total / 2.0;
    for i in 1..points.len() {
        let p0 = &points[i - 1];
        let p1 = &points[i];
        let dx = p1.x - p0.x;
        let dy = p1.y - p0.y;
        let seg = (dx * dx + dy * dy).sqrt();
        if seg == 0.0 {
            continue;
        }
        if seg < remaining {
            remaining -= seg;
            continue;
        }
        let t = (remaining / seg).clamp(0.0, 1.0);
        return Some((p0.x + t * dx, p0.y + t * dy));
    }
    Some((points.last()?.x, points.last()?.y))
}

type Rect = merman_core::geom::Box2;

fn intersect_segment_with_rect(
    p0: &LayoutPoint,
    p1: &LayoutPoint,
    rect: Rect,
) -> Option<LayoutPoint> {
    let dx = p1.x - p0.x;
    let dy = p1.y - p0.y;
    if dx == 0.0 && dy == 0.0 {
        return None;
    }

    let mut candidates: Vec<(f64, LayoutPoint)> = Vec::new();
    let eps = 1e-9;
    let min_x = rect.min_x();
    let max_x = rect.max_x();
    let min_y = rect.min_y();
    let max_y = rect.max_y();

    if dx.abs() > eps {
        for x_edge in [min_x, max_x] {
            let t = (x_edge - p0.x) / dx;
            if t < -eps || t > 1.0 + eps {
                continue;
            }
            let y = p0.y + t * dy;
            if y + eps >= min_y && y <= max_y + eps {
                candidates.push((t, LayoutPoint { x: x_edge, y }));
            }
        }
    }

    if dy.abs() > eps {
        for y_edge in [min_y, max_y] {
            let t = (y_edge - p0.y) / dy;
            if t < -eps || t > 1.0 + eps {
                continue;
            }
            let x = p0.x + t * dx;
            if x + eps >= min_x && x <= max_x + eps {
                candidates.push((t, LayoutPoint { x, y: y_edge }));
            }
        }
    }

    candidates.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    candidates
        .into_iter()
        .find(|(t, _)| *t >= 0.0)
        .map(|(_, p)| p)
}

fn clip_edge_endpoints(points: &mut [LayoutPoint], from: Rect, to: Rect) {
    if points.len() < 2 {
        return;
    }
    if from.contains_point(points[0].x, points[0].y)
        && let Some(p) = intersect_segment_with_rect(&points[0], &points[1], from)
    {
        points[0] = p;
    }
    let last = points.len() - 1;
    if to.contains_point(points[last].x, points[last].y)
        && let Some(p) = intersect_segment_with_rect(&points[last], &points[last - 1], to)
    {
        points[last] = p;
    }
}

fn er_marker_id(card: &str, suffix: &str) -> Option<String> {
    match card {
        "ONLY_ONE" => Some(format!("ONLY_ONE_{suffix}")),
        "ZERO_OR_ONE" => Some(format!("ZERO_OR_ONE_{suffix}")),
        "ONE_OR_MORE" => Some(format!("ONE_OR_MORE_{suffix}")),
        "ZERO_OR_MORE" => Some(format!("ZERO_OR_MORE_{suffix}")),
        // Mermaid CLI ER output does not emit a dedicated MD_PARENT marker.
        "MD_PARENT" => None,
        _ => None,
    }
}

#[cfg(not(feature = "layout-elk"))]
pub(crate) fn layout_er_diagram_typed(
    model: &merman_core::diagrams::er::ErDiagramRenderModel,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
) -> Result<ErDiagramLayout> {
    layout_er_diagram_typed_with_elk_authority(
        model,
        effective_config,
        measurer,
        ErElkAuthority::Raw,
    )
}

#[cfg(feature = "layout-elk")]
/// Lays out an ER diagram through ELK using the render operation's captured seed.
///
/// This remains crate-private so the public typed API stays fail-closed for ELK's unseeded
/// `randomSeed = 0` source sentinel.
pub(crate) fn layout_er_diagram_typed_with_elk_operation_seed(
    model: &merman_core::diagrams::er::ErDiagramRenderModel,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
    operation_seed: elk::ElkOperationSeed,
) -> Result<ErDiagramLayout> {
    layout_er_diagram_typed_with_elk_authority(
        model,
        effective_config,
        measurer,
        ErElkAuthority::Operation(operation_seed),
    )
}

#[derive(Debug, Clone, Copy)]
enum ErElkAuthority {
    #[cfg(not(feature = "layout-elk"))]
    Raw,
    #[cfg(feature = "layout-elk")]
    Operation(elk::ElkOperationSeed),
}

fn layout_er_diagram_typed_with_elk_authority(
    model: &merman_core::diagrams::er::ErDiagramRenderModel,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
    elk_authority: ErElkAuthority,
) -> Result<ErDiagramLayout> {
    let settings = ErConfigView::new(effective_config).layout_settings(&model.direction);

    if settings.algorithm == ErLayoutAlgorithm::Elk {
        #[cfg(feature = "layout-elk")]
        {
            let operation_seed = match elk_authority {
                ErElkAuthority::Operation(operation_seed) => Some(operation_seed),
            };
            return layout_er_diagram_elk_typed(
                model,
                effective_config,
                measurer,
                settings,
                operation_seed,
            );
        }
        #[cfg(not(feature = "layout-elk"))]
        {
            let _ = elk_authority;
            return Err(Error::MissingCapability {
                capability: crate::RenderCapability::LayoutElk,
                diagram_type: "er".to_string(),
            });
        }
    }

    layout_er_diagram_dagre_typed(model, measurer, settings)
}

fn layout_er_diagram_dagre_typed(
    model: &merman_core::diagrams::er::ErDiagramRenderModel,
    measurer: &dyn TextMeasurer,
    settings: ErLayoutSettings,
) -> Result<ErDiagramLayout> {
    let ErLayoutSettings {
        algorithm: _,
        graph: graph_label,
        label_style,
        attr_style,
        relationship_label_style,
        relationship_html_labels,
        entity_measurement,
    } = settings;

    let mut g = Graph::<NodeLabel, EdgeLabel, GraphLabel>::new(GraphOptions {
        directed: true,
        multigraph: true,
        // Mermaid's Dagre adapter always enables `compound: true`, even without clusters.
        compound: true,
    });
    g.set_graph(graph_label);

    fn parse_entity_counter_from_id(id: &str) -> Option<usize> {
        let (_prefix, tail) = id.rsplit_once('-')?;
        tail.parse::<usize>().ok()
    }

    // Nodes.
    let mut entities_in_layout_order: Vec<&ErEntity> = model.entities.values().collect();
    entities_in_layout_order.sort_by(|a, b| {
        let a_key = (parse_entity_counter_from_id(&a.id), a.id.as_str());
        let b_key = (parse_entity_counter_from_id(&b.id), b.id.as_str());
        a_key.cmp(&b_key)
    });

    for e in entities_in_layout_order {
        let (w, h) =
            entity_box_dimensions(e, measurer, &label_style, &attr_style, entity_measurement);
        g.set_node(
            e.id.clone(),
            NodeLabel {
                width: w,
                height: h,
                ..Default::default()
            },
        );
    }

    // Edges. Mermaid ER uses edge labels ("roleA") and the unified renderer routes through the
    // generic dagre pipeline, which accounts for label bbox in spacing. Mirror that by giving
    // dagre real label sizes here.
    for (idx, r) in model.relationships.iter().enumerate() {
        if g.node(&r.entity_a).is_none() || g.node(&r.entity_b).is_none() {
            return Err(Error::InvalidModel {
                message: format!(
                    "relationship references missing entities: {} -> {}",
                    r.entity_a, r.entity_b
                ),
            });
        }

        // Mermaid's dagre renderer splits self-loops into three edges and introduces two helper
        // nodes (labelRect). Mermaid initializes them at 10x10, but after `updateNodeBounds(...)`
        // an empty labelRect collapses to ~0.1x0.1 and that is what Dagre uses for spacing.
        // Match that here for layout parity.
        if r.entity_a == r.entity_b {
            let node_id = r.entity_a.as_str();
            let special_1 = format!("{node_id}---{node_id}---1");
            let special_2 = format!("{node_id}---{node_id}---2");

            if g.node(&special_1).is_none() {
                g.set_node(
                    special_1.clone(),
                    NodeLabel {
                        width: 0.1,
                        height: 0.1,
                        ..Default::default()
                    },
                );
            }
            if g.node(&special_2).is_none() {
                g.set_node(
                    special_2.clone(),
                    NodeLabel {
                        width: 0.1,
                        height: 0.1,
                        ..Default::default()
                    },
                );
            }

            let (label_w, label_h) = if r.role_a.trim().is_empty() {
                (0.0, 0.0)
            } else {
                edge_label_metrics(
                    &r.role_a,
                    measurer,
                    &relationship_label_style,
                    relationship_html_labels,
                )
            };

            // First segment: keep start marker, no label.
            g.set_edge_named(
                r.entity_a.clone(),
                special_1.clone(),
                Some(format!("er-rel-{idx}-cyclic-0")),
                Some(EdgeLabel {
                    width: 0.0,
                    height: 0.0,
                    labelpos: LabelPos::C,
                    labeloffset: 10.0,
                    minlen: 1,
                    weight: 1.0,
                    ..Default::default()
                }),
            );

            // Mid segment: carries the relationship label, no markers.
            g.set_edge_named(
                special_1.clone(),
                special_2.clone(),
                Some(format!("er-rel-{idx}")),
                Some(EdgeLabel {
                    width: label_w.max(0.0),
                    height: label_h.max(0.0),
                    labelpos: LabelPos::C,
                    labeloffset: 10.0,
                    minlen: 1,
                    weight: 1.0,
                    ..Default::default()
                }),
            );

            // Last segment: keep end marker, no label.
            g.set_edge_named(
                special_2.clone(),
                r.entity_a.clone(),
                Some(format!("er-rel-{idx}-cyclic-2")),
                Some(EdgeLabel {
                    width: 0.0,
                    height: 0.0,
                    labelpos: LabelPos::C,
                    labeloffset: 10.0,
                    minlen: 1,
                    weight: 1.0,
                    ..Default::default()
                }),
            );

            continue;
        }

        let name = format!("er-rel-{idx}");
        let (label_w, label_h) = if r.role_a.trim().is_empty() {
            (0.0, 0.0)
        } else {
            edge_label_metrics(
                &r.role_a,
                measurer,
                &relationship_label_style,
                relationship_html_labels,
            )
        };
        g.set_edge_named(
            r.entity_a.clone(),
            r.entity_b.clone(),
            Some(name),
            Some(EdgeLabel {
                width: label_w.max(0.0),
                height: label_h.max(0.0),
                labelpos: LabelPos::C,
                labeloffset: 10.0,
                minlen: 1,
                weight: 1.0,
                ..Default::default()
            }),
        );
    }

    dugong::layout(&mut g);

    let mut nodes: Vec<LayoutNode> = Vec::new();
    for id in g.node_ids() {
        let Some(n) = g.node(&id) else {
            continue;
        };
        nodes.push(LayoutNode {
            id: id.clone(),
            x: n.x.unwrap_or(0.0),
            y: n.y.unwrap_or(0.0),
            width: n.width,
            height: n.height,
            is_cluster: false,
            label_width: None,
            label_height: None,
        });
    }
    nodes.sort_by(|a, b| a.id.cmp(&b.id));

    let mut node_rect_by_id: HashMap<String, Rect> = HashMap::new();
    for n in &nodes {
        node_rect_by_id.insert(n.id.clone(), Rect::from_center(n.x, n.y, n.width, n.height));
    }

    let mut edges: Vec<LayoutEdgeParts> = Vec::new();
    for key in g.edge_keys() {
        let Some(e) = g.edge_by_key(&key) else {
            continue;
        };
        let mut points = e
            .points
            .iter()
            .map(|p| LayoutPoint { x: p.x, y: p.y })
            .collect::<Vec<_>>();

        let id = key
            .name
            .clone()
            .unwrap_or_else(|| format!("edge:{}:{}", key.v, key.w));

        let rel_idx = key
            .name
            .as_ref()
            .and_then(|name| parse_er_rel_idx_from_edge_name(name))
            .and_then(|idx| model.relationships.get(idx).map(|_| idx));

        let rel = rel_idx.and_then(|idx| model.relationships.get(idx));
        let role = rel.map(|r| r.role_a.clone()).unwrap_or_default();

        let (base_start_marker, base_end_marker, stroke_dasharray) = if let Some(rel) = rel {
            let card_a = rel.rel_spec.card_a.as_str();
            let card_b = rel.rel_spec.card_b.as_str();
            let rel_type = rel.rel_spec.rel_type.as_str();
            let start_marker = er_marker_id(card_b, "START");
            let end_marker = er_marker_id(card_a, "END");
            let stroke_dasharray = if rel_type == "NON_IDENTIFYING" {
                Some("8,8".to_string())
            } else {
                None
            };
            (start_marker, end_marker, stroke_dasharray)
        } else {
            (None, None, None)
        };

        if !is_er_self_loop_dummy_node_id(&key.v)
            && !is_er_self_loop_dummy_node_id(&key.w)
            && let (Some(from_rect), Some(to_rect)) = (
                node_rect_by_id.get(&key.v).copied(),
                node_rect_by_id.get(&key.w).copied(),
            )
        {
            clip_edge_endpoints(&mut points, from_rect, to_rect);
        }

        let (start_marker, end_marker) =
            if is_er_self_loop_dummy_node_id(&key.v) && is_er_self_loop_dummy_node_id(&key.w) {
                (None, None)
            } else if id.ends_with("-cyclic-0") {
                (base_start_marker, None)
            } else if id.ends_with("-cyclic-2") {
                (None, base_end_marker)
            } else {
                (base_start_marker, base_end_marker)
            };

        let label =
            if role.trim().is_empty() || id.ends_with("-cyclic-0") || id.ends_with("-cyclic-2") {
                None
            } else {
                let (w, h) = edge_label_metrics(
                    &role,
                    measurer,
                    &relationship_label_style,
                    relationship_html_labels,
                );
                // Mermaid uses Dagre's computed edge label center (`edge.x/edge.y`) rather than a
                // polyline midpoint. Prefer those coordinates when present.
                let (x, y) =
                    e.x.zip(e.y)
                        .or_else(|| calc_label_position(&points))
                        .unwrap_or((0.0, 0.0));
                Some(LayoutLabel {
                    x,
                    y,
                    width: w.max(1.0),
                    height: h.max(1.0),
                })
            };

        edges.push(LayoutEdgeParts {
            id,
            from: key.v.clone(),
            to: key.w.clone(),
            points,
            label,
            start_marker,
            end_marker,
            stroke_dasharray,
        });
    }
    edges.sort_by(|a, b| a.id.cmp(&b.id));

    let mut out_edges: Vec<LayoutEdge> = Vec::new();
    for e in edges {
        out_edges.push(LayoutEdge {
            id: e.id,
            from: e.from,
            to: e.to,
            from_cluster: None,
            to_cluster: None,
            points: e.points,
            label: e.label,
            start_label_left: None,
            start_label_right: None,
            end_label_left: None,
            end_label_right: None,
            start_marker: e.start_marker,
            end_marker: e.end_marker,
            stroke_dasharray: e.stroke_dasharray,
        });
    }

    let bounds = er_layout_bounds(&nodes, &out_edges);

    Ok(ErDiagramLayout {
        nodes,
        edges: out_edges,
        bounds,
    })
}

fn er_layout_bounds(nodes: &[LayoutNode], edges: &[LayoutEdge]) -> Option<Bounds> {
    let mut points: Vec<(f64, f64)> = Vec::new();
    for node in nodes {
        let half_width = node.width / 2.0;
        let half_height = node.height / 2.0;
        points.push((node.x - half_width, node.y - half_height));
        points.push((node.x + half_width, node.y + half_height));
    }
    for edge in edges {
        points.extend(edge.points.iter().map(|point| (point.x, point.y)));
        if let Some(label) = &edge.label {
            let half_width = label.width / 2.0;
            let half_height = label.height / 2.0;
            points.push((label.x - half_width, label.y - half_height));
            points.push((label.x + half_width, label.y + half_height));
        }
    }
    Bounds::from_points(points)
}

#[cfg(feature = "layout-elk")]
fn layout_er_diagram_elk_typed(
    model: &merman_core::diagrams::er::ErDiagramRenderModel,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
    settings: ErLayoutSettings,
    operation_seed: Option<elk::ElkOperationSeed>,
) -> Result<ErDiagramLayout> {
    let elk_graph = er_elk_graph(model, effective_config, measurer, &settings)?;
    let source_edge_by_id = elk_graph
        .edges
        .iter()
        .map(|edge| (edge.id.as_str(), edge))
        .collect::<HashMap<_, _>>();
    let elk_layout = match operation_seed {
        Some(operation_seed) => elk::layout_with_operation_seed(&elk_graph, operation_seed),
        None => elk::layout(&elk_graph),
    }
    .map_err(|err| Error::InvalidModel {
        message: format!("ER ELK layout failed: {err}"),
    })?;

    let mut out_nodes = elk_layout
        .nodes
        .into_iter()
        .map(|node| LayoutNode {
            id: node.id,
            x: node.x,
            y: node.y,
            width: node.width,
            height: node.height,
            is_cluster: false,
            label_width: None,
            label_height: None,
        })
        .collect::<Vec<_>>();

    let mut out_edges = Vec::with_capacity(elk_layout.edges.len());
    for edge in elk_layout.edges {
        let Some(source) = source_edge_by_id.get(edge.id.as_str()).copied() else {
            return Err(Error::InvalidModel {
                message: format!("ELK layout returned unknown ER edge {}", edge.id),
            });
        };
        let Some(index) = parse_er_rel_idx_from_edge_name(&edge.id) else {
            return Err(Error::InvalidModel {
                message: format!("ELK layout returned malformed ER edge id {}", edge.id),
            });
        };
        let Some(relationship) = model.relationships.get(index) else {
            return Err(Error::InvalidModel {
                message: format!("ELK layout returned out-of-range ER edge id {}", edge.id),
            });
        };
        let points = edge
            .points
            .into_iter()
            .map(|point| LayoutPoint {
                x: point.x,
                y: point.y,
            })
            .collect::<Vec<_>>();
        let label = source.label.and_then(|source_label| {
            edge.labels
                .first()
                .map(|label| LayoutLabel {
                    x: label.x + label.width / 2.0,
                    y: label.y + label.height / 2.0,
                    width: label.width,
                    height: label.height,
                })
                .or_else(|| {
                    calc_label_position(&points).map(|(x, y)| LayoutLabel {
                        x,
                        y,
                        width: source_label.width,
                        height: source_label.height,
                    })
                })
        });
        let rel_type = relationship.rel_spec.rel_type.as_str();
        out_edges.push(LayoutEdge {
            id: edge.id,
            from: source.source.clone(),
            to: source.target.clone(),
            from_cluster: None,
            to_cluster: None,
            points,
            label,
            start_label_left: None,
            start_label_right: None,
            end_label_left: None,
            end_label_right: None,
            start_marker: er_marker_id(relationship.rel_spec.card_b.as_str(), "START"),
            end_marker: er_marker_id(relationship.rel_spec.card_a.as_str(), "END"),
            stroke_dasharray: (rel_type == "NON_IDENTIFYING").then(|| "8,8".to_string()),
        });
    }

    out_nodes.sort_by(|left, right| left.id.cmp(&right.id));
    out_edges.sort_by(|left, right| left.id.cmp(&right.id));
    let bounds = er_layout_bounds(&out_nodes, &out_edges);
    Ok(ErDiagramLayout {
        nodes: out_nodes,
        edges: out_edges,
        bounds,
    })
}

#[cfg(feature = "layout-elk")]
fn er_elk_graph(
    model: &merman_core::diagrams::er::ErDiagramRenderModel,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
    settings: &ErLayoutSettings,
) -> Result<elk::Graph> {
    let ErLayoutSettings {
        algorithm: _,
        graph,
        label_style,
        attr_style,
        relationship_label_style,
        relationship_html_labels,
        entity_measurement,
    } = settings;

    let mut entities: Vec<&ErEntity> = model.entities.values().collect();
    entities.sort_by(|left, right| {
        fn counter(id: &str) -> Option<usize> {
            id.rsplit_once('-')?.1.parse().ok()
        }
        (counter(&left.id), left.id.as_str()).cmp(&(counter(&right.id), right.id.as_str()))
    });

    let nodes = entities
        .into_iter()
        .map(|entity| {
            let (width, height) = entity_box_dimensions(
                entity,
                measurer,
                label_style,
                attr_style,
                *entity_measurement,
            );
            elk::Node {
                id: entity.id.clone(),
                kind: elk::NodeKind::Leaf,
                width,
                height,
                parent: None,
                direction: None,
                hierarchy_handling: None,
                layer_constraint: None,
                label: None,
            }
        })
        .collect::<Vec<_>>();

    let entity_ids = nodes
        .iter()
        .map(|node| node.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let mut edges = Vec::with_capacity(model.relationships.len());
    for (index, relationship) in model.relationships.iter().enumerate() {
        if !entity_ids.contains(relationship.entity_a.as_str())
            || !entity_ids.contains(relationship.entity_b.as_str())
        {
            return Err(Error::InvalidModel {
                message: format!(
                    "relationship references missing entities: {} -> {}",
                    relationship.entity_a, relationship.entity_b
                ),
            });
        }
        let (label_width, label_height) = edge_label_metrics(
            &relationship.role_a,
            measurer,
            relationship_label_style,
            *relationship_html_labels,
        );
        edges.push(elk::Edge {
            id: format!("er-rel-{index}"),
            source: relationship.entity_a.clone(),
            target: relationship.entity_b.clone(),
            label: (!relationship.role_a.trim().is_empty()).then_some(elk::Label {
                width: label_width,
                height: label_height,
            }),
            minlen: 1,
            inside_self_loops_yo: false,
        });
    }

    Ok(elk::Graph {
        id: "root".to_string(),
        direction: match graph.rankdir {
            dugong::RankDir::LR => elk::Direction::Right,
            dugong::RankDir::RL => elk::Direction::Left,
            dugong::RankDir::BT => elk::Direction::Up,
            dugong::RankDir::TB => elk::Direction::Down,
        },
        nodes,
        edges,
        // Mermaid's ELK adapter sets `spacing.baseValue = 40`; the source-backed adapter owns
        // that exact option projection, so family-specific Dagre spacing does not leak into ELK.
        spacing: elk::Spacing::default(),
        options: er_elk_layout_options(effective_config),
    })
}

#[cfg(feature = "layout-elk")]
fn er_elk_layout_options(effective_config: &Value) -> elk::LayoutOptions {
    use crate::config::{config_bool, config_string};

    let model_order = config_string(effective_config, &["elk", "considerModelOrder"])
        .map(
            |strategy| match strategy.trim().to_ascii_uppercase().as_str() {
                "NONE" => elk::ModelOrderStrategy::None,
                "PREFER_EDGES" => elk::ModelOrderStrategy::PreferEdges,
                "PREFER_NODES" => elk::ModelOrderStrategy::PreferNodes,
                _ => elk::ModelOrderStrategy::NodesAndEdges,
            },
        )
        .unwrap_or_default();
    let cycle_breaking = config_string(effective_config, &["elk", "cycleBreakingStrategy"])
        .map(
            |strategy| match strategy.trim().to_ascii_uppercase().as_str() {
                "DEPTH_FIRST" => elk::CycleBreakingStrategy::DepthFirst,
                "INTERACTIVE" => elk::CycleBreakingStrategy::Interactive,
                "MODEL_ORDER" => elk::CycleBreakingStrategy::ModelOrder,
                "GREEDY_MODEL_ORDER" => elk::CycleBreakingStrategy::GreedyModelOrder,
                _ => elk::CycleBreakingStrategy::Greedy,
            },
        )
        .unwrap_or_default();
    let node_placement = config_string(effective_config, &["elk", "nodePlacementStrategy"])
        .map(
            |strategy| match strategy.trim().to_ascii_uppercase().as_str() {
                "SIMPLE" => elk::NodePlacementStrategy::Simple,
                "NETWORK_SIMPLEX" => elk::NodePlacementStrategy::NetworkSimplex,
                "LINEAR_SEGMENTS" => elk::NodePlacementStrategy::LinearSegments,
                _ => elk::NodePlacementStrategy::BrandesKoepf,
            },
        )
        .unwrap_or_default();
    let node_placement_alignment =
        config_string(effective_config, &["elk", "nodePlacementAlignment"])
            .map(
                |alignment| match alignment.trim().to_ascii_uppercase().as_str() {
                    "LEFTUP" => elk::NodePlacementAlignment::LeftUp,
                    "LEFTDOWN" => elk::NodePlacementAlignment::LeftDown,
                    "RIGHTUP" => elk::NodePlacementAlignment::RightUp,
                    "RIGHTDOWN" => elk::NodePlacementAlignment::RightDown,
                    "BALANCED" => elk::NodePlacementAlignment::Balanced,
                    _ => elk::NodePlacementAlignment::None,
                },
            )
            .unwrap_or_default();

    elk::LayoutOptions {
        layered: elk::LayeredOptions {
            merge_edges: config_bool(effective_config, &["elk", "mergeEdges"]).unwrap_or(false),
            merge_hierarchy_edges: true,
            unnecessary_bendpoints: true,
            inside_self_loops_activate: config_bool(
                effective_config,
                &["elk", "insideSelfLoops", "activate"],
            )
            .unwrap_or(false),
            self_loop_distribution: elk::SelfLoopDistributionStrategy::Equally,
            force_node_model_order: config_bool(effective_config, &["elk", "forceNodeModelOrder"])
                .unwrap_or(false),
            consider_model_order: model_order != elk::ModelOrderStrategy::None,
            model_order,
            cycle_breaking,
            node_placement,
            node_placement_alignment,
            ..Default::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use crate::text::{
        TextMeasurer, TextMetrics, TextStyle, VendoredFontMetricsTextMeasurer, WrapMode,
    };

    #[cfg(feature = "layout-elk")]
    #[test]
    fn er_elk_keeps_the_source_default_nonzero_seed() {
        assert_eq!(
            super::er_elk_layout_options(&serde_json::Value::Null)
                .layered
                .random_seed,
            1
        );
    }

    #[cfg(feature = "layout-elk")]
    #[test]
    fn er_elk_zero_seed_adapter_graph_requires_an_operation_seed() {
        use std::num::NonZeroU64;

        let mut model = merman_core::diagrams::er::ErDiagramRenderModel {
            direction: "TB".to_string(),
            ..Default::default()
        };
        for id in ["CUSTOMER", "ORDER"] {
            model.entities.insert(
                id.to_string(),
                merman_core::diagrams::er::ErEntityRenderModel {
                    id: id.to_string(),
                    label: id.to_string(),
                    ..Default::default()
                },
            );
        }
        model
            .relationships
            .push(merman_core::diagrams::er::ErRelationshipRenderModel {
                entity_a: "CUSTOMER".to_string(),
                role_a: "places".to_string(),
                entity_b: "ORDER".to_string(),
                ..Default::default()
            });

        let effective_config = serde_json::json!({ "layout": "elk" });
        let settings =
            super::ErConfigView::new(&effective_config).layout_settings(&model.direction);
        let mut graph = super::er_elk_graph(
            &model,
            &effective_config,
            &VendoredFontMetricsTextMeasurer::default(),
            &settings,
        )
        .expect("ER ELK adapter graph");
        graph.options.layered.random_seed = 0;

        assert!(super::elk::layout(&graph).is_err());

        let operation_seed = super::elk::ElkOperationSeed::from_operation_seed(
            NonZeroU64::new(0x6572_2d73_6565_6421).expect("nonzero operation seed"),
        );
        let first = super::elk::layout_with_operation_seed(&graph, operation_seed)
            .expect("seeded ER layout");
        let replayed = super::elk::layout_with_operation_seed(&graph, operation_seed)
            .expect("replayed seeded ER layout");

        assert_eq!(first, replayed);
    }

    struct ErProbeMeasurer;

    struct ErPrecisionMeasurer;

    impl TextMeasurer for ErProbeMeasurer {
        fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
            TextMetrics {
                width: 73.25,
                height: 22.5,
                line_count: 1,
            }
        }

        fn measure_svg_simple_text_bbox_width_px(&self, _text: &str, style: &TextStyle) -> f64 {
            if style.font_family.as_deref() == Some("sans-serif") {
                120.0
            } else {
                80.0
            }
        }

        fn measure_svg_simple_text_bbox_height_px(&self, _text: &str, _style: &TextStyle) -> f64 {
            17.0
        }
    }

    impl TextMeasurer for ErPrecisionMeasurer {
        fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
            TextMetrics {
                width: 73.123_456_789,
                height: 17.25,
                line_count: 1,
            }
        }
    }

    fn default_style() -> TextStyle {
        TextStyle {
            font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        }
    }

    fn measure_box_label(
        source: &str,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
    ) -> TextMetrics {
        let label = super::ErBoxLabel::from_source(source);
        super::er_box_label_metrics(&label, measurer, style)
    }

    #[test]
    fn er_html_label_metrics_use_the_selected_html_measurer() {
        let metrics = measure_box_label("type~T~", &ErProbeMeasurer, &default_style());

        assert_eq!(metrics.width, 73.25);
        assert_eq!(metrics.height, 22.5);
    }

    #[test]
    fn er_inline_code_label_metrics_stay_on_the_html_measurement_path() {
        let metrics =
            measure_box_label("inline: `**not bold**`", &ErProbeMeasurer, &default_style());

        assert_eq!(metrics.width, 73.25);
        assert_eq!(metrics.height, 22.5);
    }

    #[test]
    fn er_inline_html_metrics_measure_visible_runs_instead_of_tag_source() {
        let metrics = measure_box_label(
            "short<br><strong>bold</strong>",
            &ErProbeMeasurer,
            &default_style(),
        );

        assert_eq!(metrics.width, 73.25);
        assert_eq!(metrics.height, 48.0);
        assert_eq!(metrics.line_count, 2);
    }

    #[test]
    fn er_raw_code_and_anchor_metrics_measure_the_rendered_dom() {
        let measurer = VendoredFontMetricsTextMeasurer::default();
        let style = default_style();
        let source = "<a href='https://example.com'><code>Entity</code></a>";
        let fragment = crate::text::mermaid_markdown_to_xhtml_label_fragment(source, true);
        let expected = crate::text::measure_html_with_inline_styles(
            &measurer,
            &fragment,
            &style,
            None,
            WrapMode::HtmlLike,
        );

        let actual = measure_box_label(source, &measurer, &style);
        let literal = measurer.measure_wrapped(source, &style, None, WrapMode::HtmlLike);

        assert_eq!(actual.width, expected.width);
        assert_eq!(actual.height, expected.height);
        assert_eq!(actual.line_count, expected.line_count);
        assert!(
            actual.width < literal.width,
            "actual={actual:?}, literal={literal:?}"
        );
    }

    #[test]
    fn er_plain_underscore_edge_label_preserves_host_precision() {
        assert_eq!(
            super::edge_label_metrics(
                "driver_license",
                &ErPrecisionMeasurer,
                &default_style(),
                true,
            ),
            (73.123_456_789, 17.25)
        );
    }

    #[test]
    fn er_calculate_text_width_uses_shared_mermaid_family_selection() {
        let width = super::calculate_text_width_like_mermaid_px(
            &ErProbeMeasurer,
            &default_style(),
            "DRIVER",
        );

        assert_eq!(width, 80);
    }

    #[test]
    fn er_generic_workaround_is_derived_from_the_source_transformation() {
        let generic = super::ErBoxLabel::from_source("*string(99)~T~~~~~~*");
        assert!(generic.uses_generic_workaround());
        assert_eq!(generic.rendered_text(), "string(99)<T<<~>>>");
        assert_eq!(
            generic.markdown_input(),
            "*string(99)&lt;T&lt;&lt;~&gt;&gt;&gt;*"
        );

        let raw_html = super::ErBoxLabel::from_source("<code>type<T></code>");
        assert!(!raw_html.uses_generic_workaround());
        assert!(raw_html.xhtml_fragment().contains("<code>"));
    }
}
