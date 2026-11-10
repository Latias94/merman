use crate::model::{BlockDiagramLayout, Bounds, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint};
use crate::resources::{OperationWorkMeter, PreparedTextRetainedReservation};
use crate::text::{TextMeasurer, TextStyle, WrapMode};
use crate::{Error, Result};
use merman_model::presentation::PresentationConfig;
#[cfg(test)]
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

mod config;
mod edge_paint;
mod marker_paint;
mod node_binding;
mod typography;
pub(crate) use edge_paint::BlockEdgePaintPlan;
pub(crate) use marker_paint::BlockMarkerPaintPlan;
pub(crate) use typography::BlockTextParticipation;
mod source;
mod theme;

pub(crate) use config::BlockShapeSettings;
pub(crate) use source::resolve_block_node_sources;

#[cfg(test)]
use config::{BlockConfigView, BlockLayoutSettings};
pub(crate) use theme::{
    BlockCssThemeBinding, BlockLabelBackgroundPlan, BlockNodeLabelPaintPlan,
    BlockNodePaintThemePlan, BlockTypographyThemePlan,
};

mod geometry;

pub use geometry::{
    BlockAllocatedBounds, BlockRectangleKind, BlockShapeBoundary, BlockShapeGeometry,
};

pub(crate) type BlockNode = merman_model::families::block::BlockNode;

// Mermaid createFormattedText pads the SVG edge label's background on all four sides.
pub(crate) const SVG_EDGE_LABEL_BACKGROUND_PADDING: f64 = 2.0;

// Prepared XHTML uses a document root independently of the diagram's font size.
const HTML_DOCUMENT_ROOT_FONT_SIZE_PX: f64 = 16.0;

#[derive(Debug, Clone)]
struct SizedBlock {
    id: String,
    block_type: String,
    children: Vec<usize>,
    columns: f64,
    width_in_columns: f64,
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

/// Operation-owned HTML terminals shared by Block measurement and SVG emission.
#[derive(Debug, Default)]
pub(crate) struct BlockPreparedLabels {
    nodes: HashMap<String, BlockPreparedLabel>,
    edges: Vec<BlockPreparedLabel>,
}

#[derive(Debug)]
pub(crate) struct BlockPreparedLabel {
    html: Option<BlockPreparedHtmlLabel>,
    pub(crate) typography: typography::BlockLabelTypography,
}

impl BlockPreparedLabel {
    pub(crate) fn xhtml(&self) -> Option<&str> {
        self.html.as_ref().map(|html| html.xhtml.as_str())
    }

    fn measurement_xhtml(&self) -> Option<&str> {
        self.typography
            .measurement_xhtml
            .as_deref()
            .or_else(|| self.xhtml())
    }
}

#[derive(Debug)]
struct BlockPreparedHtmlLabel {
    xhtml: String,
    _retained_reservation: PreparedTextRetainedReservation,
}

impl BlockPreparedLabels {
    fn prepare(
        model: &merman_model::families::block::BlockDiagramRenderModel,
        sources: &HashMap<&str, source::BlockNodeSource<'_>>,
        html_labels: bool,
        text_style: &TextStyle,
        config: &PresentationConfig,
        work_meter: &Arc<OperationWorkMeter>,
    ) -> Result<Self> {
        work_meter.checkpoint(merman_model::OperationPhase::Layout)?;
        // Resolve the absent security-level default once for the whole family operation.
        let config = crate::graph_label::flowchart_label_sanitize_config(config);
        let mut prepared = Self::default();
        for node in &model.blocks_flat {
            work_meter.checkpoint(merman_model::OperationPhase::Layout)?;
            if prepared.nodes.contains_key(&node.id) {
                continue;
            }
            // Detached database objects are sized with flat records but never painted.
            // Their own label must not override an attached object's source paint.
            let text = sources
                .get(node.id.as_str())
                .map_or(node.label.as_str(), |source| source.label);
            let html = html_labels
                .then(|| BlockPreparedHtmlLabel::prepare(text, node.id.len(), &config, work_meter))
                .transpose()?;
            let source = sources.get(node.id.as_str());
            let typography = typography::BlockLabelTypography::resolve(
                source.map_or(node.styles.as_slice(), |source| source.styles),
                Some(source.map_or(node.classes.as_slice(), |source| source.classes)),
                &model.class_defs,
                text_style,
                html.as_ref().map(|html| html.xhtml.as_str()),
                text,
                node.id.len(),
                work_meter,
            )?;
            prepared
                .nodes
                .insert(node.id.clone(), BlockPreparedLabel { html, typography });
        }
        for edge in &model.edges {
            let label = if html_labels {
                Some(BlockPreparedHtmlLabel::prepare(
                    &edge.label,
                    0,
                    &config,
                    work_meter,
                )?)
            } else {
                None
            };
            let typography = typography::BlockLabelTypography::resolve(
                &[],
                None,
                &model.class_defs,
                text_style,
                label.as_ref().map(|label| label.xhtml.as_str()),
                &edge.label,
                0,
                work_meter,
            )?;
            prepared.edges.push(BlockPreparedLabel {
                html: label,
                typography,
            });
        }
        Ok(prepared)
    }

    pub(crate) fn node_label(&self, node_id: &str) -> &BlockPreparedLabel {
        self.nodes.get(node_id).expect("prepared Block node label")
    }

    pub(crate) fn edge_label(&self, edge_index: usize) -> &BlockPreparedLabel {
        &self.edges[edge_index]
    }
}

impl BlockPreparedHtmlLabel {
    fn prepare(
        raw: &str,
        key_bytes: usize,
        config: &merman_model::MermaidConfig,
        work_meter: &Arc<OperationWorkMeter>,
    ) -> Result<Self> {
        use crate::html::{HtmlOutputError, HtmlSanitizeSink};
        use merman_markup::sanitize::SanitizeFailure;

        // Include the map/vector owner slot, as in the existing prepared-label sidecars.
        const OWNER_SLOT_BYTES: usize = 192;
        let owner_bytes = std::mem::size_of::<Self>()
            .saturating_add(OWNER_SLOT_BYTES)
            .saturating_add(key_bytes);
        work_meter.preflight_prepared_text_retained_bytes(owner_bytes)?;
        let parse_work = std::cell::Cell::new(0);
        let max = work_meter
            .remaining_prepared_text_retained_bytes()
            .map_or(usize::MAX, |remaining| {
                remaining.saturating_sub(owner_bytes)
            });
        let sink = HtmlSanitizeSink::new(
            max,
            work_meter,
            &parse_work,
            merman_model::OperationPhase::Layout,
        );
        // Keep entity-authored angle brackets as text until HTML parsing consumes them.
        let fragment = (|| {
            if raw.is_empty() {
                return Ok(String::new());
            }
            let text = merman_markup::sanitize::sanitize_text_with_sink(raw, config, &sink)?;
            let html = format!(
                "<p>{}</p>",
                text.replace("\\n", "<br />").replace('\n', "<br />")
            );
            merman_markup::sanitize::sanitize_xhtml_with_sink(&html, config, &sink)
        })();
        let xhtml = match fragment {
            Ok(xhtml) => xhtml,
            Err(SanitizeFailure::Output(HtmlOutputError::Operation(error))) => {
                return Err(error.into());
            }
            Err(SanitizeFailure::Output(HtmlOutputError::Limit { attempted })) => {
                work_meter.preflight_prepared_text_retained_bytes(
                    owner_bytes.saturating_add(attempted),
                )?;
                String::new()
            }
            // Preserve the sanitizer's fail-closed behavior for malformed input/allocation.
            Err(_) => String::new(),
        };
        work_meter.checkpoint(merman_model::OperationPhase::Layout)?;
        let work_control =
            crate::layout_work::OperationLayoutWorkControl::new(Arc::clone(work_meter));
        let reservation = work_control
            .reserve_prepared_text_retained_bytes(owner_bytes.saturating_add(xhtml.capacity()))?;
        Ok(Self {
            xhtml,
            _retained_reservation: reservation,
        })
    }
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
    html_labels: bool,
    prepared_xhtml: Option<&str>,
) -> (f64, f64) {
    let metrics = if html_labels {
        crate::text::measure_unwrapped_xhtml_label_fragment(
            measurer,
            prepared_xhtml.expect("HTML Block labels are prepared before measurement"),
            style,
            HTML_DOCUMENT_ROOT_FONT_SIZE_PX,
        )
    } else {
        measurer.measure_wrapped(text, style, None, WrapMode::SvgLike)
    };
    (metrics.width.max(0.0), metrics.height.max(0.0))
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
    shape_settings: BlockShapeSettings,
) -> Option<(f64, f64)> {
    geometry::natural_shape_size(
        block_type,
        directions,
        label_width,
        label_height,
        padding,
        has_label,
        shape_settings,
    )
}

fn to_sized_block_shallow(
    node: &BlockNode,
    padding: f64,
    shape_settings: BlockShapeSettings,
    measurer: &dyn TextMeasurer,
    prepared_label: &BlockPreparedLabel,
    html_labels: bool,
) -> SizedBlock {
    let columns = node.columns.unwrap_or(-1.0);
    let width_in_columns = node.width_in_columns.unwrap_or(1.0);

    let mut width = 0.0;
    let mut height = 0.0;

    // HTML measurement and emission share the sanitized XHTML projection, including breaks,
    // inline formatting, and entities. Keep the existing SVG text path separate.
    let label_decoded = decode_block_label_html(&node.label);
    let label_effectively_empty = block_label_is_effectively_empty(&label_decoded);
    let (label_width, label_height) = if label_effectively_empty {
        (0.0, 0.0)
    } else {
        block_html_label_metrics_px(
            &label_decoded,
            measurer,
            &prepared_label.typography.style,
            html_labels,
            prepared_label.measurement_xhtml(),
        )
    };
    let shape_label_height = label_height;

    if let Some((computed_width, computed_height)) = block_shape_size(
        node.block_type.as_str(),
        &node.directions,
        label_width,
        shape_label_height,
        padding,
        !label_effectively_empty && !label_decoded.trim().is_empty(),
        shape_settings,
    ) {
        width = computed_width;
        height = computed_height;
    }

    SizedBlock {
        id: node.id.clone(),
        block_type: node.block_type.clone(),
        children: node.children.clone(),
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

fn get_max_child_size(block: &SizedBlock, records: &[SizedBlock]) -> (f64, f64) {
    let mut max_width: f64 = 0.0;
    let mut max_height: f64 = 0.0;
    for &index in &block.children {
        let child = &records[index];
        if child.block_type == "space" {
            continue;
        }
        max_width = max_width.max(child.width / child.width_in_columns);
        max_height = max_height.max(child.height);
    }
    (max_width, max_height)
}

fn block_grid_size(block: &SizedBlock, records: &[SizedBlock]) -> (f64, f64) {
    let num_items = block
        .children
        .iter()
        .map(|&index| records[index].width_in_columns)
        .sum::<f64>();
    let mut x_size = block.children.len() as f64;
    if block.columns > 0.0 && block.columns < num_items {
        x_size = block.columns;
    }
    let y_size = (num_items / x_size).ceil();
    (x_size, y_size)
}

fn set_block_sizes_shallow(index: usize, records: &mut [SizedBlock], padding: f64) {
    if records[index].width == 0.0 || records[index].width.is_nan() {
        let block = &mut records[index];
        block.width = 0.0;
        block.height = 0.0;
        block.x = 0.0;
        block.y = 0.0;
    }
    let block = &records[index];
    if block.children.is_empty() {
        return;
    }
    let (max_width, max_height) = get_max_child_size(block, records);
    let children = block.children.clone();
    for &child in &children {
        let child = &mut records[child];
        child.width = max_width * child.width_in_columns + padding * (child.width_in_columns - 1.0);
        child.height = max_height;
        // The second setBlockSizes call initializes a falsy allocated width from sibling size.
        if child.width == 0.0 || child.width.is_nan() {
            child.width = max_width;
            child.height = max_height;
        }
        child.x = 0.0;
        child.y = 0.0;
    }
    let block = &records[index];
    let (x_size, y_size) = block_grid_size(block, records);
    let mut width = x_size * (max_width + padding) + padding;
    let height = y_size * (max_height + padding) + padding;
    if width < block.width {
        width = block.width;
        let num = if block.columns > 0.0 {
            (children.len() as f64).min(block.columns)
        } else {
            children.len() as f64
        };
        if num > 0.0 {
            let child_width = (width - num * padding - padding) / num;
            for &child in &children {
                records[child].width = child_width;
            }
        }
    }
    let block = &mut records[index];
    block.width = width;
    block.height = height;
    block.x = 0.0;
    block.y = 0.0;
}

fn reconcile_block_with_parent_size(index: usize, records: &mut [SizedBlock], padding: f64) {
    let block = &records[index];
    if block.children.is_empty() {
        return;
    }
    let children = block.children.clone();
    let inherited_width = block.width;
    let inherited_height = block.height;
    let column_span = block.width_in_columns;
    let sibling_width = (inherited_width - padding * (column_span - 1.0)) / column_span;
    let (max_width, max_height) = get_max_child_size(block, records);
    let (x_size, y_size) = block_grid_size(block, records);
    let columns = block.columns;
    let mut width = x_size * (max_width + padding) + padding;
    let mut height = y_size * (max_height + padding) + padding;
    if width < sibling_width {
        width = sibling_width;
        height = inherited_height;
        let child_width = (sibling_width - x_size * padding - padding) / x_size;
        let child_height = (inherited_height - y_size * padding - padding) / y_size;
        for &child in &children {
            let child = &mut records[child];
            child.width = child_width;
            child.height = child_height;
            child.x = 0.0;
            child.y = 0.0;
        }
    }
    if width < inherited_width {
        width = inherited_width;
        let num = if columns > 0.0 {
            (children.len() as f64).min(columns)
        } else {
            children.len() as f64
        };
        if num > 0.0 {
            let child_width = (width - num * padding - padding) / num;
            for &child in &children {
                records[child].width = child_width;
            }
        }
    }
    let block = &mut records[index];
    block.width = width;
    block.height = height;
    block.x = 0.0;
    block.y = 0.0;
}

fn set_block_sizes(root: usize, records: &mut [SizedBlock], padding: f64) {
    let mut stack = vec![(root, false)];
    while let Some((index, complete)) = stack.pop() {
        if complete {
            set_block_sizes_shallow(index, records, padding);
        } else {
            stack.push((index, true));
            stack.extend(
                records[index]
                    .children
                    .iter()
                    .rev()
                    .map(|&child| (child, false)),
            );
        }
    }
    let mut stack = records[root]
        .children
        .iter()
        .rev()
        .copied()
        .collect::<Vec<_>>();
    while let Some(index) = stack.pop() {
        reconcile_block_with_parent_size(index, records, padding);
        stack.extend(records[index].children.iter().rev().copied());
    }
}

fn invalid_block_columns_error() -> Error {
    Error::InvalidModel {
        // Match Mermaid's Number.isInteger invariant before grid arithmetic.
        message: "Columns must be an integer !== 0.".to_string(),
    }
}

fn validate_block_columns(
    root: usize,
    records: &[BlockNode],
    work_meter: &OperationWorkMeter,
) -> Result<()> {
    let mut stack = vec![(root, false)];
    let mut active = vec![false; records.len()];
    let mut seen = vec![false; records.len()];
    while let Some((index, complete)) = stack.pop() {
        work_meter.charge(1)?;
        let Some(block) = records.get(index) else {
            return Err(Error::InvalidModel {
                message: format!("invalid Block child index {index}"),
            });
        };
        if complete {
            active[index] = false;
            continue;
        }
        if active[index] {
            return Err(Error::InvalidModel {
                message: "cyclic Block child indices".to_string(),
            });
        }
        if std::mem::replace(&mut seen[index], true) {
            return Err(Error::InvalidModel {
                message: format!("repeated Block child index {index}"),
            });
        }
        if !block.children.is_empty()
            && block.columns.is_some_and(|columns| {
                columns == 0.0 || !columns.is_finite() || columns.fract() != 0.0
            })
        {
            return Err(invalid_block_columns_error());
        }
        work_meter.charge(block.children.len())?;
        for &child in &block.children {
            let child = records.get(child).ok_or_else(|| Error::InvalidModel {
                message: format!("invalid Block child index {child}"),
            })?;
            if child.width_in_columns.is_some_and(|span| {
                span.is_nan() || span < 0.0 || (span.is_finite() && span.fract() != 0.0)
            }) {
                return Err(Error::InvalidModel {
                    message: "Block column span must be a non-negative integer or Infinity"
                        .to_string(),
                });
            }
        }
        active[index] = true;
        stack.push((index, true));
        stack.extend(block.children.iter().rev().map(|&child| (child, false)));
    }
    Ok(())
}

fn calculate_block_position(columns: f64, position: f64) -> Result<(f64, f64)> {
    if columns == 0.0 || !columns.is_finite() || columns.fract() != 0.0 {
        return Err(invalid_block_columns_error());
    }
    if position < 0.0 || !position.is_finite() || position.fract() != 0.0 {
        return Err(Error::InvalidModel {
            message: format!(
                "Position must be a non-negative integer.{}",
                ryu_js::Buffer::new().format(position)
            ),
        });
    }
    if columns < 0.0 {
        return Ok((position, 0.0));
    }
    if columns == 1.0 {
        return Ok((0.0, position));
    }
    Ok((position % columns, (position / columns).floor()))
}

fn filled_columns(span: f64, columns: f64, position: f64) -> f64 {
    if columns > 0.0 {
        span.min(columns - position % columns)
    } else {
        span
    }
}

fn layout_blocks(root: usize, records: &mut [SizedBlock], padding: f64) -> Result<()> {
    let mut stack = vec![root];
    while let Some(index) = stack.pop() {
        let block = &records[index];
        if block.children.is_empty() {
            continue;
        }
        let children = block.children.clone();
        let columns = block.columns;
        let parent_x = block.x;
        let parent_y = block.y;
        let parent_width = block.width;
        let parent_height = block.height;
        // Positions are validated non-negative Numbers. Their non-negative row bit patterns
        // retain numeric sort order without narrowing rows to a machine integer.
        let mut row_heights = BTreeMap::<u64, f64>::new();
        let mut height_column_pos = 0.0;
        for &index in &children {
            let child = &records[index];
            let (_, row) = calculate_block_position(columns, height_column_pos)?;
            row_heights
                .entry(row.to_bits())
                .and_modify(|height| *height = height.max(child.height))
                .or_insert(child.height);
            height_column_pos += filled_columns(child.width_in_columns, columns, height_column_pos);
        }
        let mut row_offsets = BTreeMap::<u64, f64>::new();
        let mut offset = 0.0;
        for (&row, &height) in &row_heights {
            row_offsets.insert(row, offset);
            offset += height + padding;
        }
        let mut column_pos = 0.0;
        let row_start = if parent_x != 0.0 {
            parent_x - parent_width / 2.0
        } else {
            -padding
        };
        let mut starting_pos_x = row_start;
        let mut row_pos = 0.0;
        for &index in &children {
            let child = &mut records[index];
            let (_, py) = calculate_block_position(columns, column_pos)?;
            if py != row_pos {
                row_pos = py;
                starting_pos_x = row_start;
            }
            let half_width = child.width / 2.0;
            child.x = starting_pos_x + padding + half_width;
            starting_pos_x = child.x + half_width;
            let row_offset = row_offsets.get(&py.to_bits()).copied().unwrap_or_default();
            let row_height = row_heights
                .get(&py.to_bits())
                .copied()
                .unwrap_or(child.height);
            child.y = parent_y - parent_height / 2.0 + row_offset + row_height / 2.0 + padding;
            column_pos += filled_columns(child.width_in_columns, columns, column_pos);
        }
        stack.extend(children.into_iter().rev());
    }
    Ok(())
}

fn find_bounds(root: usize, records: &[SizedBlock], bounds: &mut Bounds) {
    let mut stack = vec![root];
    while let Some(index) = stack.pop() {
        let block = &records[index];
        if block.id != "root" {
            bounds.min_x = bounds.min_x.min(block.x - block.width / 2.0);
            bounds.min_y = bounds.min_y.min(block.y - block.height / 2.0);
            bounds.max_x = bounds.max_x.max(block.x + block.width / 2.0);
            bounds.max_y = bounds.max_y.max(block.y + block.height / 2.0);
        }
        stack.extend(block.children.iter().rev().copied());
    }
}

fn collect_nodes(root: usize, records: &[SizedBlock], out: &mut Vec<LayoutNode>) {
    let mut stack = vec![root];
    while let Some(index) = stack.pop() {
        let block = &records[index];
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
        stack.extend(block.children.iter().rev().copied());
    }
}

#[cfg(test)]
pub(crate) fn layout_block_diagram_typed(
    model: &merman_model::families::block::BlockDiagramRenderModel,
    effective_config: &Value,
    measurer: &dyn TextMeasurer,
) -> Result<BlockDiagramLayout> {
    let BlockLayoutSettings {
        padding,
        text_style,
    } = BlockConfigView::new(effective_config).layout_settings();
    let html_labels = crate::config::config_effective_html_labels(effective_config);
    layout_block_diagram_typed_with_text_style(
        model,
        padding,
        BlockShapeSettings::resolve(effective_config),
        html_labels,
        text_style,
        measurer,
        &crate::test_support::presentation_config(effective_config.clone()),
        &Arc::new(OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        )),
    )
    .map(|(layout, _prepared_labels)| layout)
}

#[allow(
    clippy::too_many_arguments,
    reason = "Block preparation keeps layout settings, typography, and operation services explicit."
)]
pub(crate) fn layout_block_diagram_typed_with_text_style(
    model: &merman_model::families::block::BlockDiagramRenderModel,
    padding: f64,
    shape_settings: BlockShapeSettings,
    html_labels: bool,
    text_style: TextStyle,
    measurer: &dyn TextMeasurer,
    effective_config: &PresentationConfig,
    work_meter: &Arc<OperationWorkMeter>,
) -> Result<(BlockDiagramLayout, BlockPreparedLabels)> {
    let root = model
        .blocks_flat
        .iter()
        .position(|b| b.id == "root" && b.block_type == "composite")
        .ok_or_else(|| Error::InvalidModel {
            message: "missing block root composite".to_string(),
        })?;

    validate_block_columns(root, &model.blocks_flat, work_meter)?;

    let shape_sources = resolve_block_node_sources(model);
    let prepared_labels = BlockPreparedLabels::prepare(
        model,
        &shape_sources,
        html_labels,
        &text_style,
        effective_config,
        work_meter,
    )?;

    let mut records = model
        .blocks_flat
        .iter()
        .map(|block| {
            to_sized_block_shallow(
                block,
                padding,
                shape_settings,
                measurer,
                prepared_labels.node_label(&block.id),
                html_labels,
            )
        })
        .collect::<Vec<_>>();
    set_block_sizes(root, &mut records, padding);
    layout_blocks(root, &mut records, padding)?;

    let mut nodes: Vec<LayoutNode> = Vec::new();
    collect_nodes(root, &records, &mut nodes);

    let mut shape_geometries = Vec::with_capacity(nodes.len());
    for node in &nodes {
        let source = shape_sources
            .get(node.id.as_str())
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing Block shape source for node `{}`", node.id),
            })?;
        let geometry = BlockShapeGeometry::from_layout_node(
            node,
            source.block_type,
            source.directions,
            padding,
            source.width_in_columns,
            shape_settings,
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
    find_bounds(root, &records, &mut bounds);
    let bounds = if nodes.is_empty() { None } else { Some(bounds) };

    let nodes_by_id: HashMap<String, LayoutNode> =
        nodes.iter().cloned().map(|n| (n.id.clone(), n)).collect();

    let mut edges: Vec<LayoutEdge> = Vec::new();
    for (edge_index, e) in model.edges.iter().enumerate() {
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
            let prepared_label = prepared_labels.edge_label(edge_index);
            let (label_width, label_height) = block_html_label_metrics_px(
                &edge_label,
                measurer,
                &prepared_label.typography.style,
                html_labels,
                prepared_label.measurement_xhtml(),
            );
            let background_padding = if html_labels {
                0.0
            } else {
                2.0 * SVG_EDGE_LABEL_BACKGROUND_PADDING
            };
            Some(LayoutLabel {
                x: mid.x,
                y: mid.y,
                width: label_width.max(1.0) + background_padding,
                height: label_height.max(1.0) + background_padding,
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

    work_meter.checkpoint(merman_model::OperationPhase::Layout)?;
    Ok((
        BlockDiagramLayout {
            nodes,
            edges,
            shape_geometries,
            bounds,
        },
        prepared_labels,
    ))
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
            columns: -1.0,
            width_in_columns: width_in_columns as f64,
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
        let mut records = vec![
            sized_block("parent", 0.0, 0.0, 9),
            sized_block("three-columns", 180.0, 30.0, 3),
            sized_block("one-column", 80.0, 20.0, 1),
        ];
        records[0].children = vec![1, 2];
        let (width, height) = super::get_max_child_size(&records[0], &records);
        assert_eq!(width, 80.0);
        assert_eq!(height, 30.0);
    }

    #[test]
    fn multi_column_composite_keeps_intrinsic_height_when_only_total_width_grows() {
        let mut records = vec![
            sized_block("root", 0.0, 0.0, 1),
            sized_block("tall", 150.0, 88.0, 3),
            sized_block("composite", 0.0, 0.0, 3),
        ];
        records.extend((0..7).map(|index| sized_block(&format!("leaf-{index}"), 18.0, 32.0, 1)));
        records[0].columns = 3.0;
        records[0].children = vec![1, 2];
        records[2].children = (3..10).collect();
        super::set_block_sizes(0, &mut records, 8.0);
        assert_eq!(records[2].height, 48.0);
        assert!(
            records[2]
                .children
                .iter()
                .all(|&child| records[child].height == 32.0)
        );
    }

    #[test]
    fn heterogeneous_block_rows_use_accumulated_row_heights() {
        let mut records = vec![
            sized_block("root", 239.0, 296.0, 1),
            sized_block("a", 223.0, 88.0, 3),
            sized_block("group1", 150.0, 88.0, 2),
            sized_block("g", 71.0, 88.0, 1),
            sized_block("group2", 223.0, 48.0, 3),
        ];
        records[0].columns = 3.0;
        records[0].children = vec![1, 2, 3, 4];
        super::layout_blocks(0, &mut records, 8.0).expect("valid block layout");
        assert_eq!(records[1].y, -96.0);
        assert_eq!(records[2].y, 0.0);
        assert_eq!(records[3].y, 0.0);
        assert_eq!(records[4].y, 76.0);
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
            true,
            Some("<p>Font size precedence should widen this block</p>"),
        );

        assert_eq!(width, 321.25);
        assert_eq!(height, 45.5);
    }

    #[test]
    fn block_prepared_html_retains_entities_and_breaks_until_its_owner_is_dropped() {
        let meter = std::sync::Arc::new(crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let config =
            crate::test_support::presentation_config(serde_json::json!({"securityLevel": "loose"}));
        let prepared =
            super::BlockPreparedHtmlLabel::prepare("A&amp;B<br/>C", 1, config.as_config(), &meter)
                .expect("prepare the final Block HTML terminal");
        let fragment = format!("<root>{}</root>", prepared.xhtml);
        let document = crate::xml_dom::Document::parse(&fragment).expect("prepared XHTML");
        assert_eq!(
            document
                .descendants()
                .filter(|node| node.has_tag_name("br"))
                .count(),
            1,
        );
        let text = document
            .descendants()
            .filter(|node| node.is_text())
            .filter_map(|node| node.text())
            .collect::<String>();
        assert_eq!(text, "A&BC");
        assert!(meter.prepared_text_retained_bytes() >= prepared.xhtml.capacity());
        assert!(meter.used() > 0, "HTML parsing is layout work");
        assert_eq!(meter.projected_svg_bytes(), 0);
        drop(prepared);
        assert_eq!(meter.prepared_text_retained_bytes(), 0);
    }

    #[test]
    fn block_prepared_nodes_and_edges_share_the_final_html_projection() {
        let parsed = merman_core::MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "block\n A[\"Alpha<br/>Beta\"] -- \"Alpha<br/>Beta\" --> B[\"Target\"]\n",
                merman_core::ParseOptions::strict(),
            )
            .expect("parse HTML Block labels")
            .expect("detect Block")
            .into_presentation();
        let merman_model::presentation::RenderSemanticModel::Block(model) = parsed.model() else {
            panic!("expected a typed Block model");
        };
        let meter = std::sync::Arc::new(crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let prepared = super::BlockPreparedLabels::prepare(
            model,
            &super::resolve_block_node_sources(model),
            true,
            &default_style(16.0),
            &parsed.metadata().effective_config,
            &meter,
        )
        .expect("prepare node and edge HTML");
        assert_eq!(
            prepared.node_label("A").xhtml(),
            prepared.edge_label(0).xhtml()
        );
        assert!(prepared.node_label("A").xhtml().unwrap().contains("Alpha"));
        assert!(meter.prepared_text_retained_bytes() > 0);
        drop(prepared);
        assert_eq!(meter.prepared_text_retained_bytes(), 0);
    }

    #[test]
    fn block_prepared_html_measures_the_retained_fragment_instead_of_authored_markup() {
        struct VisibleTextMeasurer(std::cell::RefCell<Vec<String>>);

        impl TextMeasurer for VisibleTextMeasurer {
            fn measure(&self, text: &str, _style: &TextStyle) -> TextMetrics {
                self.0.borrow_mut().push(text.to_owned());
                TextMetrics {
                    width: text.chars().count() as f64,
                    height: 10.0,
                    line_count: 1,
                }
            }
        }

        let measurer = VisibleTextMeasurer(std::cell::RefCell::new(Vec::new()));
        let (width, _) = super::block_html_label_metrics_px(
            "authored text must not be measured",
            &measurer,
            &default_style(16.0),
            true,
            Some("<p>A&amp;B<br />C</p>"),
        );
        let measured = measurer.0.borrow();
        assert_eq!(width, 3.0);
        assert!(measured.iter().any(|text| text == "A&B"));
        assert!(measured.iter().any(|text| text == "C"));
        assert!(measured.iter().all(|text| !text.contains("authored")));
    }

    #[test]
    fn block_prepared_html_preserves_retained_and_work_rejections() {
        use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};

        let config =
            crate::test_support::presentation_config(serde_json::json!({"securityLevel": "loose"}));
        for (limit, value, expected) in [
            (
                ResourceLimitId::MaxPreparedTextRetainedBytes,
                1,
                "max_prepared_text_retained_bytes",
            ),
            (
                ResourceLimitId::MaxLayoutWorkUnits,
                1,
                "max_layout_work_units",
            ),
        ] {
            let meter = std::sync::Arc::new(OperationWorkMeter::new(
                RenderResourcePolicy::unbounded_for_trusted_input()
                    .with_limit(limit, value)
                    .unwrap(),
            ));
            let error = super::BlockPreparedHtmlLabel::prepare(
                "<b>label</b>",
                1,
                config.as_config(),
                &meter,
            )
            .expect_err("Block HTML preparation must respect its operation budget");
            let crate::Error::ResourceLimitExceeded(error) = error else {
                panic!("expected the original resource rejection");
            };
            assert_eq!(error.limit, expected);
            assert_eq!(meter.prepared_text_retained_bytes(), 0);
            assert_eq!(meter.projected_svg_bytes(), 0);
        }
    }

    #[test]
    fn block_prepared_html_preserves_layout_cancellation() {
        let control = merman_model::OperationControl::new();
        control.cancel();
        let meter = std::sync::Arc::new(crate::resources::OperationWorkMeter::new_with_control(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            control,
        ));
        let config =
            crate::test_support::presentation_config(serde_json::json!({"securityLevel": "loose"}));
        let error =
            super::BlockPreparedHtmlLabel::prepare("<b>label</b>", 1, config.as_config(), &meter)
                .expect_err("Block HTML preparation must preserve cancellation");
        let crate::Error::Cancelled(error) = error else {
            panic!("expected cancellation");
        };
        assert_eq!(error.phase, merman_model::OperationPhase::Layout);
        assert_eq!(meter.prepared_text_retained_bytes(), 0);
    }

    #[test]
    fn zero_columns_are_a_typed_layout_error_not_a_division_by_zero() {
        let error = super::calculate_block_position(0.0, 0.0).expect_err("zero columns must fail");
        assert!(matches!(error, crate::Error::InvalidModel { .. }));
        assert_eq!(
            error.to_string(),
            "invalid semantic model: Columns must be an integer !== 0."
        );
    }

    #[test]
    fn block_positions_and_grid_sizes_retain_number_precision_beyond_machine_integers() {
        let large = 18_446_744_073_709_551_616.0;
        assert_eq!(
            super::calculate_block_position(-1.0, large).unwrap(),
            (large, 0.0)
        );
        assert_eq!(
            super::calculate_block_position(3.0, large).unwrap(),
            (1.0, (large / 3.0).floor())
        );
        let mut records = vec![
            sized_block("root", 0.0, 0.0, 1),
            sized_block("A", 40.0, 30.0, 1),
            sized_block("B", 50.0, 30.0, 1),
        ];
        records[0].children = vec![1, 2];
        records[1].width_in_columns = large;
        records[2].width_in_columns = large;
        assert_eq!(super::block_grid_size(&records[0], &records), (2.0, large));
        super::set_block_sizes(0, &mut records, 8.0);
        super::layout_blocks(0, &mut records, 8.0).unwrap();
        assert!(records.iter().all(|block| {
            [block.width, block.height, block.x, block.y]
                .iter()
                .all(|value| value.is_finite())
        }));
    }

    #[test]
    fn zero_and_infinite_spans_keep_source_sizing_and_position_failure_stage() {
        // layout.ts divides by the authored span and initializes falsy allocated widths from
        // sibling size. Neither zero nor Infinity is replaced with a one-column span.
        for (span, expected_width, expected_height) in [
            (0.0, f64::INFINITY, 8.0),
            (f64::INFINITY, 16.0, f64::INFINITY),
        ] {
            let mut records = vec![
                sized_block("root", 0.0, 0.0, 1),
                sized_block("A", 40.0, 30.0, 1),
            ];
            records[0].children = vec![1];
            records[1].width_in_columns = span;
            super::set_block_sizes(0, &mut records, 8.0);
            assert_eq!(records[0].width, expected_width);
            assert_eq!(records[0].height, expected_height);
            super::layout_blocks(0, &mut records, 8.0).expect("the first position is finite");
            if span.is_infinite() {
                assert_eq!(records[1].y, f64::NEG_INFINITY);
                records.push(sized_block("B", 50.0, 30.0, 1));
                records[0].children.push(2);
                let error = super::layout_blocks(0, &mut records, 8.0).unwrap_err();
                assert!(
                    error
                        .to_string()
                        .contains("Position must be a non-negative integer")
                );
                records[0].columns = 2.0;
                super::layout_blocks(0, &mut records, 8.0)
                    .expect("finite columns clip filled spans");
            }
        }
    }
}
