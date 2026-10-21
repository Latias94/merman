use crate::config::config_f64_css_px;
use crate::graph_label::{FlowchartLabelMetricsRequest, flowchart_label_metrics_for_layout};
use crate::layout_work::OperationLayoutWorkControl;
use crate::math::MathRenderer;
use crate::model::{Bounds, LayoutEdge, LayoutNode, LayoutPoint, MindmapDiagramLayout};
use crate::text::WrapMode;
use crate::text::{TextMeasurer, TextStyle};
use crate::{Error, Result};
use merman_core::presentation::PresentationConfig;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::Arc;

mod css_binding;
mod registered;
pub(crate) use css_binding::MindmapCssBinding;
mod theme;
mod tidy_tree;

pub(crate) use theme::{
    MINDMAP_SECTION_COUNT, MindmapNodeFillSource, MindmapNodePalettePlan,
    MindmapNodePaletteTerminalDecision, MindmapWriterThemeToken,
};

pub(crate) fn mindmap_max_node_width_px(effective_config: &Value) -> f64 {
    config_f64_css_px(effective_config, &["mindmap", "maxNodeWidth"])
        .unwrap_or(200.0)
        .max(1.0)
}

pub(crate) fn mindmap_node_look<'a>(
    model_look: &'a str,
    effective_config: &'a Value,
) -> crate::config::DiagramLook<'a> {
    let model_look = model_look.trim();
    if model_look.is_empty() || model_look == "default" {
        crate::config::config_diagram_look(effective_config)
    } else {
        crate::config::DiagramLook::from_raw(Some(model_look))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum MindmapLayoutBackend {
    Cose,
    TidyTree,
    Swimlane,
    Dagre,
    #[cfg(feature = "layout-elk")]
    Elk,
}

pub(crate) fn layout_backend(config: &Value) -> MindmapLayoutBackend {
    let requested = config.get("layout").and_then(Value::as_str).unwrap_or("");
    match requested {
        "tidy-tree" => MindmapLayoutBackend::TidyTree,
        "dagre" => MindmapLayoutBackend::Dagre,
        "swimlane" => MindmapLayoutBackend::Swimlane,
        #[cfg(feature = "layout-elk")]
        _ if crate::layout_backend::resolve_graph_layout(config).backend
            == crate::layout_backend::GraphLayoutBackend::Elk =>
        {
            MindmapLayoutBackend::Elk
        }
        _ if cfg!(feature = "layout-cytoscape") => MindmapLayoutBackend::Cose,
        _ => MindmapLayoutBackend::Dagre,
    }
}

pub(crate) fn required_layout_capability(
    config: &PresentationConfig,
) -> Option<crate::RenderCapability> {
    match layout_backend(config.as_value()) {
        MindmapLayoutBackend::Cose => Some(crate::RenderCapability::LayoutCytoscape),
        #[cfg(feature = "layout-elk")]
        MindmapLayoutBackend::Elk => Some(crate::RenderCapability::LayoutElk),
        _ => None,
    }
}

/// The Dagre renderer clips circular nodes along the endpoint-to-center ray.
pub(crate) fn dagre_shape_points(
    mut points: Vec<LayoutPoint>,
    start: &LayoutNode,
    end: &LayoutNode,
    start_circle: bool,
    end_circle: bool,
) -> Vec<LayoutPoint> {
    if points.len() < 2 {
        return points;
    }
    let intersect = |node: &LayoutNode, point: &LayoutPoint| {
        let dx = point.x - node.x;
        let dy = point.y - node.y;
        let length = dx.hypot(dy);
        if length == 0.0 {
            return point.clone();
        }
        let radius = node.width / 2.0;
        LayoutPoint {
            x: node.x + radius * dx / length,
            y: node.y + radius * dy / length,
        }
    };
    if start_circle {
        points[0] = intersect(start, &points[1]);
    }
    if end_circle {
        let last = points.len() - 1;
        points[last] = intersect(end, &points[last - 1]);
    }
    points
}

type MindmapModel = merman_core::diagrams::mindmap::MindmapDiagramRenderModel;
type MindmapNodeModel = merman_core::diagrams::mindmap::MindmapDiagramRenderNode;
type MindmapEdgeModel = merman_core::diagrams::mindmap::MindmapDiagramRenderEdge;

/// HTML label fragments prepared once during Mindmap layout and reused by SVG emission.
///
/// Keeping this sidecar private to the family artifact prevents the public layout projection
/// from carrying operation-owned XHTML while making layout and emission observe the same
/// sanitized DOM structure.
#[derive(Debug, Default)]
pub(crate) struct MindmapPreparedLabels {
    labels: BTreeMap<String, MindmapPreparedLabel>,
}

#[derive(Debug)]
struct MindmapPreparedLabel {
    xhtml: String,
    _retained_reservation: crate::resources::PreparedTextRetainedReservation,
}

impl MindmapPreparedLabels {
    pub(crate) fn xhtml_for(&self, node_id: &str) -> Option<&str> {
        self.labels.get(node_id).map(|label| label.xhtml.as_str())
    }

    fn insert(&mut self, node_id: String, label: MindmapPreparedLabel) {
        self.labels.insert(node_id, label);
    }
}

impl MindmapPreparedLabel {
    fn prepare(
        node_id: &str,
        text: &str,
        config: &PresentationConfig,
        work_meter: &Arc<crate::resources::OperationWorkMeter>,
    ) -> Result<Self> {
        // Match the conservative owner-slot allowance used by the Flowchart sidecar.
        const OWNER_SLOT_BYTES: usize = 192;
        let owner_bytes = std::mem::size_of::<Self>()
            .saturating_add(OWNER_SLOT_BYTES)
            .saturating_add(node_id.len());
        work_meter.preflight_prepared_text_retained_bytes(owner_bytes)?;
        let parse_work = std::cell::Cell::new(0);
        let max = work_meter
            .remaining_prepared_text_retained_bytes()
            .map_or(usize::MAX, |remaining| {
                remaining.saturating_sub(owner_bytes)
            });
        let sink = crate::html::HtmlSanitizeSink::new(
            max,
            work_meter,
            &parse_work,
            merman_core::OperationPhase::Layout,
        );
        let html = crate::text::replace_fontawesome_icons(
            &crate::text::mermaid_markdown_to_xhtml_label_fragment(text, true),
        );
        let xhtml =
            match merman_core::sanitize::sanitize_xhtml_with_sink(&html, config.as_config(), &sink)
            {
                Ok(xhtml) => xhtml,
                Err(merman_core::sanitize::SanitizeFailure::Output(
                    crate::html::HtmlOutputError::Operation(error),
                )) => return Err(error.into()),
                Err(merman_core::sanitize::SanitizeFailure::Output(
                    crate::html::HtmlOutputError::Limit { attempted },
                )) => {
                    work_meter.preflight_prepared_text_retained_bytes(
                        owner_bytes.saturating_add(attempted),
                    )?;
                    String::new()
                }
                // Preserve the existing sanitizer's fail-closed behavior for malformed input.
                Err(_) => String::new(),
            };
        // Preserve the terminal's normalization order and whitespace. Indented code may own a
        // trailing line box once its container switches to `white-space: break-spaces`.
        let xhtml = single_image_paragraph_inner(&xhtml)
            .map(str::to_owned)
            .unwrap_or(xhtml);
        let decoded = crate::entities::decode_mermaid_entities_for_render_text(&xhtml);
        let xhtml = crate::xml::normalize_html_entities_for_xml(decoded.as_ref()).into_owned();
        let retained_bytes = owner_bytes.saturating_add(xhtml.capacity());
        let reservation = work_meter.reserve_prepared_text_retained_bytes(retained_bytes)?;
        Ok(Self {
            xhtml,
            _retained_reservation: reservation,
        })
    }
}

pub(crate) fn single_image_paragraph_inner(fragment: &str) -> Option<&str> {
    let fragment = fragment.trim();
    let inner = fragment.strip_prefix("<p>")?.strip_suffix("</p>")?.trim();
    let prefix = inner.get(..4)?;
    if !prefix.eq_ignore_ascii_case("<img") {
        return None;
    }
    if inner
        .as_bytes()
        .get(4)
        .is_some_and(|byte| !byte.is_ascii_whitespace() && !matches!(byte, b'/' | b'>'))
    {
        return None;
    }

    let mut quote = None;
    for (index, ch) in inner.char_indices().skip(4) {
        match (quote, ch) {
            (Some(active), candidate) if active == candidate => quote = None,
            (None, '\'' | '"') => quote = Some(ch),
            (None, '>') => return inner[index + 1..].trim().is_empty().then_some(inner),
            _ => {}
        }
    }
    None
}

#[cfg(test)]
fn mindmap_text_style(effective_config: &Value) -> TextStyle {
    mindmap_text_style_with_font_family(
        effective_config,
        &crate::config::config_font_family_css(effective_config),
    )
}

pub(crate) fn mindmap_text_style_with_font_family(
    effective_config: &Value,
    font_family_css: &str,
) -> TextStyle {
    // HTML labels retain their 16px measurement baseline; SVG labels inherit root CSS size.
    let font_size = if crate::config::config_effective_html_labels(effective_config) {
        16.0
    } else {
        crate::config::config_theme_font_size_css_or_root_number_px(effective_config, 16.0).max(1.0)
    };
    TextStyle {
        font_family: Some(font_family_css.to_owned()),
        font_size,
        font_weight: None,
        font_style: None,
    }
}

pub(crate) fn mindmap_label_text_for_layout(text: &str) -> &str {
    if !text.contains('\n') && !text.contains('\r') {
        return text;
    }

    let mut normalized = None;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if normalized.is_some() {
            return text;
        }
        normalized = Some(line);
    }

    normalized.unwrap_or(text)
}

pub(crate) fn mindmap_svg_label_source(text: &str) -> std::borrow::Cow<'_, str> {
    let text = mindmap_label_text_for_layout(text);
    let text = text
        .strip_prefix('`')
        .and_then(|text| text.strip_suffix('`'))
        .unwrap_or(text);
    merman_core::entities::restore_mermaid_entity_spelling(text)
}

#[cfg(test)]
fn mindmap_label_bbox_px(
    text: &str,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    max_node_width_px: f64,
) -> (f64, f64) {
    let config = PresentationConfig::from_value(serde_json::json!({
        "securityLevel": "strict",
        "htmlLabels": true
    }));
    let work_meter = Arc::new(crate::resources::OperationWorkMeter::new(
        crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
    ));
    let prepared = MindmapPreparedLabel::prepare("test-label", text, &config, &work_meter)
        .expect("prepare test label");
    mindmap_label_bbox_px_with_math(
        text,
        measurer,
        style,
        max_node_width_px,
        &config,
        None,
        Some(&prepared.xhtml),
    )
    .expect("measure prepared test label")
}

#[allow(
    clippy::too_many_arguments,
    reason = "The family measurement boundary keeps source, prepared XHTML, and capabilities explicit"
)]
fn mindmap_label_bbox_px_with_math(
    text: &str,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    max_node_width_px: f64,
    config: &PresentationConfig,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    prepared_xhtml: Option<&str>,
) -> Result<(f64, f64)> {
    if !crate::config::config_effective_html_labels(config.as_value()) {
        let source = mindmap_svg_label_source(text);
        let lines = crate::text::mermaid_markdown_to_wrapped_word_lines(
            measurer,
            source.as_ref(),
            style,
            Some(max_node_width_px.max(1.0)),
            WrapMode::SvgLike,
        );
        let metrics = crate::text::measure_svg_markdown_word_lines(measurer, &lines, style);
        return Ok((metrics.width.max(0.0), metrics.height.max(0.0)));
    }

    if !crate::math::contains_delimited_math(text) {
        let xhtml = prepared_xhtml.ok_or_else(|| Error::InvalidModel {
            message: "Mindmap HTML label is missing its prepared fragment".to_string(),
        })?;
        // Preserve Mindmap's HTML min-content and wrapping rules while measuring the exact
        // recovered structure retained for emission, including sanitizer removals.
        let metrics = crate::text::measure_html_with_inline_styles(
            measurer,
            xhtml,
            style,
            Some(max_node_width_px.max(1.0)),
            WrapMode::HtmlLike,
        );
        return Ok((metrics.width.max(0.0), metrics.height.max(0.0)));
    }

    let math_renderer = math_renderer.ok_or_else(|| Error::MissingCapability {
        capability: crate::RenderCapability::Math,
        diagram_type: "mindmap".to_string(),
    })?;
    let metrics = flowchart_label_metrics_for_layout(FlowchartLabelMetricsRequest {
        measurer,
        raw_label: mindmap_label_text_for_layout(text),
        label_type: "markdown",
        style,
        max_width_px: Some(max_node_width_px.max(1.0)),
        wrap_mode: WrapMode::HtmlLike,
        config,
        math_renderer: Some(math_renderer),
    });
    Ok((metrics.width.max(0.0), metrics.height.max(0.0)))
}

/// Total label padding after Mindmap's rect preprocessing and shared squareRect/drawRect sizing.
pub(crate) fn mindmap_rect_label_padding_px(look: crate::config::DiagramLook<'_>) -> (f64, f64) {
    if look.is_neo() {
        (32.0, 24.0)
    } else {
        // mindmapRenderer.ts fixes node.padding at 10; squareRect applies it twice horizontally.
        (40.0, 20.0)
    }
}

fn mindmap_node_dimensions_from_label_bbox(
    node: &MindmapNodeModel,
    bbox_w: f64,
    bbox_h: f64,
    look: crate::config::DiagramLook<'_>,
) -> (f64, f64, f64, f64) {
    // `mindmapRenderer.ts` overrides padding after `getData()`: rect uses 10, rounded uses 15.
    // Hexagon only clears its requested dimensions; mindmapCircle retains the configured padding.
    //
    // Our semantic snapshots keep the DB padding (e.g. doubled padding for `(text)`), but layout
    // should follow the render-time effective padding so layout golden snapshots remain stable.
    let padding = match node.shape.as_str() {
        "rounded" => 15.0,
        _ => node.padding.max(0.0),
    };
    let half_padding = padding / 2.0;

    // Align with Mermaid shape sizing rules for mindmap nodes (via `labelHelper(...)` + shape
    // handlers in `rendering-elements/shapes/*`).
    let (w, h) = match node.shape.as_str() {
        // `defaultMindmapNode.ts`: w = bbox.width + 8 * halfPadding; h = bbox.height + 2 * halfPadding
        "" | "defaultMindmapNode" => (bbox_w + 8.0 * half_padding, bbox_h + 2.0 * half_padding),
        // `squareRect.ts` supplies per-side padding to `drawRect.ts`: classic uses 2*padding
        // horizontally and padding vertically; Neo uses fixed 16px/12px per-side steps.
        "rect" => {
            let (padding_w, padding_h) = mindmap_rect_label_padding_px(look);
            (bbox_w + padding_w, bbox_h + padding_h)
        }
        "rounded" => (bbox_w + 2.0 * padding, bbox_h + 2.0 * padding),
        // `mindmapCircle.ts` passes the full node padding to shared `circle.ts`.
        // Mermaid 12.2 encloses the label diagonal and uses one 16px radius padding for Neo.
        "mindmapCircle" => {
            let radius_padding = if look.is_neo() { 16.0 } else { padding };
            let d = bbox_w.hypot(bbox_h) + 2.0 * radius_padding;
            (d, d)
        }
        // `cloud.ts` first draws a path from w = bbox.width + 2*halfPadding and
        // h = bbox.height + 2*halfPadding, then upstream cose-bilkent lays out the node
        // using the inserted SVG node's rendered path bbox.
        "cloud" => {
            let shape_w = bbox_w + 2.0 * half_padding;
            let shape_h = bbox_h + 2.0 * half_padding;
            crate::svg::mindmap_cloud_rendered_bbox_size_px(shape_w, shape_h)
                .unwrap_or((shape_w, shape_h))
        }
        // `bang.ts`:
        // - w = bbox.width + 10*halfPadding; h = bbox.height + 8*halfPadding
        // - minWidth = bbox.width + 20; minHeight = bbox.height + 20
        // - effectiveWidth/Height = max(w/h, minWidth/Height)
        "bang" => {
            let w = bbox_w + 10.0 * half_padding;
            let h = bbox_h + 8.0 * half_padding;
            let min_w = bbox_w + 20.0;
            let min_h = bbox_h + 20.0;
            (w.max(min_w), h.max(min_h))
        }
        // `hexagon.ts` (classic): h = bbox.height + padding; m = h/4;
        // w = bbox.width + 2*m + padding.
        "hexagon" => {
            let h = bbox_h + padding;
            let m = h / 4.0;
            (bbox_w + 2.0 * m + padding, h)
        }
        _ => (bbox_w + 8.0 * half_padding, bbox_h + 2.0 * half_padding),
    };

    (w, h, bbox_w, bbox_h)
}

#[cfg(all(test, feature = "diagram-mindmap"))]
fn mindmap_node_dimensions_px(
    node: &MindmapNodeModel,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    max_node_width_px: f64,
) -> (f64, f64, f64, f64) {
    let (bbox_w, bbox_h) = mindmap_label_bbox_px(&node.label, measurer, style, max_node_width_px);
    mindmap_node_dimensions_from_label_bbox(
        node,
        bbox_w,
        bbox_h,
        crate::config::DiagramLook::from_raw(Some(&node.look)),
    )
}

fn compute_bounds(nodes: &[LayoutNode], edges: &[LayoutEdge]) -> Option<Bounds> {
    let mut pts: Vec<(f64, f64)> = Vec::new();
    for n in nodes {
        let x0 = n.x - n.width / 2.0;
        let y0 = n.y - n.height / 2.0;
        let x1 = n.x + n.width / 2.0;
        let y1 = n.y + n.height / 2.0;
        pts.push((x0, y0));
        pts.push((x1, y1));
    }
    for e in edges {
        for p in &e.points {
            pts.push((p.x, p.y));
        }
    }
    Bounds::from_points(pts)
}

fn shift_nodes_to_positive_bounds(nodes: &mut [LayoutNode], content_min: f64) {
    if nodes.is_empty() {
        return;
    }
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    for n in nodes.iter() {
        min_x = min_x.min(n.x - n.width / 2.0);
        min_y = min_y.min(n.y - n.height / 2.0);
    }
    if !(min_x.is_finite() && min_y.is_finite()) {
        return;
    }
    let dx = content_min - min_x;
    let dy = content_min - min_y;
    for n in nodes.iter_mut() {
        n.x += dx;
        n.y += dy;
    }
}

fn mindmap_layout_adapter_work(
    model: &MindmapModel,
    use_tidy_tree: bool,
    work_control: &OperationLayoutWorkControl,
) -> Result<usize> {
    let node_work = work_control.checked_mul(model.nodes.len(), 12)?;
    let edge_work = work_control.checked_mul(model.edges.len(), 8)?;
    let backend_work = if use_tidy_tree {
        work_control.checked_add(
            work_control.checked_mul(model.nodes.len(), 16)?,
            work_control.checked_mul(model.edges.len(), 12)?,
        )?
    } else {
        work_control.checked_add(
            work_control.checked_mul(model.nodes.len(), 6)?,
            work_control.checked_mul(model.edges.len(), 4)?,
        )?
    };
    work_control.checked_add(
        node_work,
        work_control.checked_add(edge_work, backend_work)?,
    )
}

pub(crate) fn layout_mindmap_diagram_typed_with_work_meter(
    model: &MindmapModel,
    config: &PresentationConfig,
    font_family_css: &str,
    text_measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    work_meter: Arc<crate::resources::OperationWorkMeter>,
    #[cfg(feature = "layout-elk")] operation_seed: merman_layout_elk::ElkOperationSeed,
) -> Result<(MindmapDiagramLayout, MindmapPreparedLabels)> {
    let mut work_control = OperationLayoutWorkControl::new(work_meter.clone());
    let use_tidy_tree = layout_backend(config.as_value()) == MindmapLayoutBackend::TidyTree;
    let adapter_work = mindmap_layout_adapter_work(model, use_tidy_tree, &work_control)?;
    work_control.charge_adapter(adapter_work)?;
    layout_mindmap_diagram_model(
        model,
        config,
        font_family_css,
        text_measurer,
        math_renderer,
        &mut work_control,
        work_meter,
        #[cfg(feature = "layout-elk")]
        operation_seed,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "Layout inputs and operation controls remain explicit at this family boundary"
)]
fn layout_mindmap_diagram_model(
    model: &MindmapModel,
    config: &PresentationConfig,
    font_family_css: &str,
    text_measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    _work_control: &mut OperationLayoutWorkControl,
    work_meter: Arc<crate::resources::OperationWorkMeter>,
    #[cfg(feature = "layout-elk")] operation_seed: merman_layout_elk::ElkOperationSeed,
) -> Result<(MindmapDiagramLayout, MindmapPreparedLabels)> {
    let effective_config = config.as_value();
    let text_style = mindmap_text_style_with_font_family(effective_config, font_family_css);
    let max_node_width_px = mindmap_max_node_width_px(effective_config);

    let mut nodes: Vec<LayoutNode> = Vec::with_capacity(model.nodes.len());
    let mut prepared_labels = MindmapPreparedLabels::default();
    // Mermaid's Mindmap DB emits nodes in parser preorder, and Cytoscape consumes that order
    // directly. The typed model already preserves it; re-sorting numeric-looking ids here added
    // O(N log N) work and changed the geometry of manually constructed source-order models.
    for n in &model.nodes {
        let prepared = if crate::config::config_effective_html_labels(effective_config)
            && !crate::math::contains_delimited_math(&n.label)
        {
            Some(MindmapPreparedLabel::prepare(
                &n.id,
                &n.label,
                config,
                &work_meter,
            )?)
        } else {
            None
        };
        let (label_width, label_height) = mindmap_label_bbox_px_with_math(
            &n.label,
            text_measurer,
            &text_style,
            max_node_width_px,
            config,
            math_renderer,
            prepared.as_ref().map(|label| label.xhtml.as_str()),
        )?;
        if let Some(prepared) = prepared {
            prepared_labels.insert(n.id.clone(), prepared);
        }
        let (width, height, label_width, label_height) = mindmap_node_dimensions_from_label_bbox(
            n,
            label_width,
            label_height,
            mindmap_node_look(&n.look, effective_config),
        );

        nodes.push(LayoutNode {
            id: n.id.clone(),
            // Mermaid mindmap uses Cytoscape COSE-Bilkent and initializes node positions at (0,0).
            // We keep that behavior so `manatee` can reproduce upstream placements deterministically.
            x: 0.0,
            y: 0.0,
            width: width.max(1.0),
            height: height.max(1.0),
            is_cluster: false,
            label_width: Some(label_width.max(0.0)),
            label_height: Some(label_height.max(0.0)),
        });
    }
    let mut id_to_idx: rustc_hash::FxHashMap<&str, usize> =
        rustc_hash::FxHashMap::with_capacity_and_hasher(nodes.len(), Default::default());
    for (idx, n) in nodes.iter().enumerate() {
        id_to_idx.insert(n.id.as_str(), idx);
    }

    let mut edge_indices: Vec<(usize, usize)> = Vec::with_capacity(model.edges.len());
    for e in &model.edges {
        let Some(&a) = id_to_idx.get(e.start.as_str()) else {
            return Err(Error::InvalidModel {
                message: format!("edge start node not found: {}", e.start),
            });
        };
        let Some(&b) = id_to_idx.get(e.end.as_str()) else {
            return Err(Error::InvalidModel {
                message: format!("edge end node not found: {}", e.end),
            });
        };
        edge_indices.push((a, b));
    }

    let backend = layout_backend(effective_config);
    match backend {
        MindmapLayoutBackend::Swimlane => {
            return Ok((
                registered::swimlane(model, nodes, config, work_meter)?,
                prepared_labels,
            ));
        }
        MindmapLayoutBackend::Dagre => {
            return Ok((
                registered::dagre(model, nodes, effective_config, _work_control)?,
                prepared_labels,
            ));
        }
        #[cfg(feature = "layout-elk")]
        MindmapLayoutBackend::Elk => {
            return Ok((
                registered::elk(
                    model,
                    nodes,
                    effective_config,
                    _work_control,
                    operation_seed,
                )?,
                prepared_labels,
            ));
        }
        _ => {}
    }
    let use_tidy_tree = backend == MindmapLayoutBackend::TidyTree;
    let tidy_tree_edges = if use_tidy_tree {
        Some(tidy_tree::layout(
            &mut nodes,
            &model.nodes,
            &model.edges,
            &edge_indices,
        )?)
    } else {
        #[cfg(feature = "layout-cytoscape")]
        {
            let indexed_nodes: Vec<manatee::algo::cose_bilkent::IndexedNode> = nodes
                .iter()
                .map(|n| manatee::algo::cose_bilkent::IndexedNode {
                    width: n.width,
                    height: n.height,
                    x: n.x,
                    y: n.y,
                })
                .collect();
            let mut indexed_edges: Vec<manatee::algo::cose_bilkent::IndexedEdge> =
                Vec::with_capacity(model.edges.len());
            for (edge_idx, (a, b)) in edge_indices.iter().copied().enumerate() {
                if a == b {
                    continue;
                }
                indexed_edges.push(manatee::algo::cose_bilkent::IndexedEdge { a, b });

                // Keep `edge_idx` referenced so unused warnings don't obscure failures if we ever
                // enhance indexed validation error messages.
                let _ = edge_idx;
            }

            let positions = manatee::algo::cose_bilkent::layout_indexed_with_work_control(
                &indexed_nodes,
                &indexed_edges,
                _work_control,
            )
            .map_err(|error| _work_control.map_manatee_error(error))?;

            for (n, p) in nodes.iter_mut().zip(positions) {
                n.x = p.x;
                n.y = p.y;
            }
            None
        }
        #[cfg(not(feature = "layout-cytoscape"))]
        {
            return Err(Error::MissingCapability {
                capability: crate::RenderCapability::LayoutCytoscape,
                diagram_type: "mindmap".to_string(),
            });
        }
    };

    // Mermaid's COSE-Bilkent post-layout normalizes to a positive coordinate space via
    // `transform(0,0)` (layout-base), yielding a content bbox that starts around (15,15) before
    // the 10px viewport padding is applied (viewBox starts at 5,5).
    //
    // Tidy-tree is centered around x=0 and intentionally grows in both horizontal directions.
    // COSE-Bilkent instead normalizes into Mermaid's positive Cytoscape coordinate space.
    if !use_tidy_tree {
        shift_nodes_to_positive_bounds(&mut nodes, 15.0);
    }

    let edges = if let Some(edges) = tidy_tree_edges {
        edges
    } else {
        model
            .edges
            .iter()
            .zip(edge_indices.iter().copied())
            .map(|(edge, (source_index, target_index))| {
                let source = &nodes[source_index];
                let target = &nodes[target_index];
                LayoutEdge {
                    id: edge.id.clone(),
                    from: edge.start.clone(),
                    to: edge.end.clone(),
                    from_cluster: None,
                    to_cluster: None,
                    points: vec![
                        LayoutPoint {
                            x: source.x,
                            y: source.y,
                        },
                        LayoutPoint {
                            x: target.x,
                            y: target.y,
                        },
                    ],
                    label: None,
                    start_label_left: None,
                    start_label_right: None,
                    end_label_left: None,
                    end_label_right: None,
                    start_marker: None,
                    end_marker: None,
                    stroke_dasharray: None,
                }
            })
            .collect()
    };
    let bounds = compute_bounds(&nodes, &edges);
    Ok((
        MindmapDiagramLayout {
            swimlane_lanes: Vec::new(),
            nodes,
            edges,
            bounds,
        },
        prepared_labels,
    ))
}

#[cfg(all(test, feature = "diagram-mindmap"))]
mod tests {
    #[test]
    fn mindmap_prepared_html_owns_the_sanitized_structure_and_releases_retained_bytes() {
        let meter = std::sync::Arc::new(crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        ));
        let config = merman_core::presentation::PresentationConfig::from_value(serde_json::json!({
            "securityLevel": "loose",
            "dompurifyConfig": {"FORBID_TAGS": ["strong"], "KEEP_CONTENT": false}
        }));
        let prepared = super::MindmapPreparedLabel::prepare(
            "root",
            "Alpha</div><strong>removed</strong>Beta",
            &config,
            &meter,
        )
        .expect("prepare the final HTML label");
        assert_eq!(prepared.xhtml, "<p>AlphaBeta</p>");
        let retained = meter.prepared_text_retained_bytes();
        assert!(retained >= prepared.xhtml.capacity());
        assert!(meter.used() > 0, "HTML parsing belongs to layout work");
        assert_eq!(meter.projected_svg_bytes(), 0);
        drop(prepared);
        assert_eq!(meter.prepared_text_retained_bytes(), 0);
    }

    #[test]
    fn mindmap_prepared_html_preserves_retained_and_work_rejections() {
        use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};
        let config = merman_core::presentation::PresentationConfig::from_value(serde_json::json!({
            "securityLevel": "loose"
        }));
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
            let error =
                super::MindmapPreparedLabel::prepare("root", "<b>label</b>", &config, &meter)
                    .expect_err("prepared HTML must respect its operation budget");
            let crate::Error::ResourceLimitExceeded(error) = error else {
                panic!("expected the original resource terminal");
            };
            assert_eq!(error.limit, expected);
            assert_eq!(meter.prepared_text_retained_bytes(), 0);
            assert_eq!(meter.projected_svg_bytes(), 0);
        }
    }

    #[test]
    fn mindmap_prepared_html_preserves_layout_cancellation() {
        let control = merman_core::OperationControl::new();
        control.cancel();
        let meter = std::sync::Arc::new(crate::resources::OperationWorkMeter::new_with_control(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            control,
        ));
        let config = merman_core::presentation::PresentationConfig::from_value(serde_json::json!({
            "securityLevel": "loose"
        }));
        let error = super::MindmapPreparedLabel::prepare("root", "<b>label</b>", &config, &meter)
            .expect_err("HTML preparation must preserve cancellation");
        let crate::Error::Cancelled(error) = error else {
            panic!("expected cancellation");
        };
        assert_eq!(error.phase, merman_core::OperationPhase::Layout);
        assert_eq!(meter.prepared_text_retained_bytes(), 0);
    }

    #[test]
    fn mindmap_declined_math_measurement_keeps_markdown_fallback() {
        let parsed = merman_core::Engine::new().parse_diagram_for_render_model_sync(
            "---\nconfig:\n  layout: tidy-tree\n---\nmindmap\n  root[Root]\n    child[\"**Bold** $$x$$\"]",
            merman_core::ParseOptions::strict(),
        ).unwrap().unwrap();
        let merman_core::RenderSemanticModel::Mindmap(model) = parsed.model() else {
            panic!("Mindmap render model");
        };
        let padding = model.nodes[1].padding;
        let session = crate::environment::RenderEnvironment::deterministic()
            .with_math_renderer(std::sync::Arc::new(crate::math::NoopMathRenderer))
            .begin_session()
            .unwrap();
        let artifact =
            crate::family::prepare(&parsed, &crate::LayoutOptions::default(), session).unwrap();
        let layout = artifact.layout_json().unwrap();
        let nodes = layout["layout"]["MindmapDiagram"]["nodes"]
            .as_array()
            .unwrap();
        assert_eq!(nodes.len(), 2);
        let child = nodes
            .iter()
            .find(|node| node["id"] == "1")
            .expect("Markdown child node");
        let style = super::mindmap_text_style(&serde_json::json!({}));
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let html = crate::text::mermaid_markdown_to_html_label_fragment("**Bold** $$x$$", true);
        let expected = crate::text::measure_html_with_inline_styles(
            &measurer,
            &html,
            &style,
            Some(200.0),
            crate::text::WrapMode::HtmlLike,
        );
        assert_eq!(
            child["width"].as_f64(),
            Some(expected.width + 2.0 * padding)
        );
        assert_eq!(child["height"].as_f64(), Some(expected.height + padding));
    }

    struct FixedMeasurer;

    impl crate::text::TextMeasurer for FixedMeasurer {
        fn measure(
            &self,
            _text: &str,
            _style: &crate::text::TextStyle,
        ) -> crate::text::TextMetrics {
            crate::text::TextMetrics {
                width: 73.0,
                height: 24.0,
                line_count: 1,
            }
        }
    }

    #[test]
    fn mindmap_max_node_width_accepts_number_and_px_string() {
        let numeric = serde_json::json!({
            "mindmap": {
                "maxNodeWidth": 320
            }
        });
        assert_eq!(super::mindmap_max_node_width_px(&numeric), 320.0);

        let px_string = serde_json::json!({
            "mindmap": {
                "maxNodeWidth": "280px"
            }
        });
        assert_eq!(super::mindmap_max_node_width_px(&px_string), 280.0);

        let plain_string = serde_json::json!({
            "mindmap": {
                "maxNodeWidth": "240"
            }
        });
        assert_eq!(super::mindmap_max_node_width_px(&plain_string), 240.0);

        let fallback = serde_json::json!({});
        assert_eq!(super::mindmap_max_node_width_px(&fallback), 200.0);
    }

    #[test]
    fn mindmap_label_text_for_layout_trims_single_line_delimiter_text() {
        assert_eq!(
            super::mindmap_label_text_for_layout("\n      The root\n    "),
            "The root"
        );
        assert_eq!(
            super::mindmap_label_text_for_layout("\r\nThe root"),
            "The root"
        );
        assert_eq!(super::mindmap_label_text_for_layout("The root"), "The root");
        assert_eq!(
            super::mindmap_label_text_for_layout("\n      first\n      second\n    "),
            "\n      first\n      second\n    "
        );
    }

    #[test]
    fn mindmap_plain_wrapping_label_uses_wrapped_container_width() {
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let style = super::mindmap_text_style(&serde_json::json!({}));
        let (width, height) = super::mindmap_label_bbox_px(
            "A root with a long text that wraps to keep the node size in check",
            &measurer,
            &style,
            200.0,
        );

        assert!(width > 0.0 && width <= 200.0);
        assert!(
            height > style.font_size,
            "long prose should wrap to multiple rows"
        );
    }

    #[test]
    fn mindmap_markdown_wrapping_respects_max_node_width() {
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let style = super::mindmap_text_style(&serde_json::json!({}));
        let (width, height) = super::mindmap_label_bbox_px(
            "The dog in **the** hog... a *very long text* that wraps to a new line",
            &measurer,
            &style,
            200.0,
        );

        assert_eq!(width, 200.0);
        assert_eq!(height, 72.0);
    }

    #[test]
    fn mindmap_break_spaces_preserves_trailing_indentation_line_box() {
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let style = super::mindmap_text_style(&serde_json::json!({}));
        let source = "\n    Multi-line root\n    with three lines\n  ";
        assert_eq!(
            crate::text::mermaid_markdown_to_html_label_fragment(source, true),
            "    Multi-line root\n    with three lines\n  "
        );
        let (width, height) = super::mindmap_label_bbox_px(source, &measurer, &style, 200.0);

        assert_eq!(width, 200.0);
        assert_eq!(height, 72.0);
    }

    #[test]
    fn mindmap_html_labels_measure_visible_content_instead_of_markup() {
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let style = super::mindmap_text_style(&serde_json::json!({}));
        let expected = crate::text::TextMeasurer::measure_wrapped(
            &measurer,
            "docs",
            &style,
            Some(200.0),
            crate::text::WrapMode::HtmlLike,
        );

        let actual = super::mindmap_label_bbox_px(
            r#"<a href='https://mermaid.js.org/' rel="noopener" target="_blank">docs</a>"#,
            &measurer,
            &style,
            200.0,
        );

        assert_eq!(
            actual,
            (
                crate::text::round_to_1_64_px(expected.width),
                expected.height
            )
        );
    }

    #[test]
    fn mindmap_layout_decodes_mermaid_entity_placeholders_before_measurement() {
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let style = super::mindmap_text_style(&serde_json::json!({}));
        let expected = super::mindmap_label_bbox_px("Circle: &♥ ∞", &measurer, &style, 200.0);

        let actual =
            super::mindmap_label_bbox_px("Circle: &ﬂ°°9829¶ß &infin;", &measurer, &style, 200.0);

        assert_eq!(actual, expected);
    }

    #[test]
    fn mindmap_code_only_html_measures_visible_code_content() {
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let style = super::mindmap_text_style(&serde_json::json!({}));
        let expected = crate::text::TextMeasurer::measure_wrapped(
            &measurer,
            "note about mermaid",
            &style,
            Some(200.0),
            crate::text::WrapMode::HtmlLike,
        );

        let actual = super::mindmap_label_bbox_px(
            r#"<a href="https://mermaid.js.org/"><code>note about mermaid</code></a>"#,
            &measurer,
            &style,
            200.0,
        );

        assert_eq!(
            actual,
            (
                crate::text::round_to_1_64_px(expected.width),
                expected.height
            )
        );
    }

    #[test]
    fn mindmap_plain_labels_use_the_selected_measurer_without_content_adjustment() {
        let measurer = FixedMeasurer;
        let style = super::mindmap_text_style(&serde_json::json!({}));

        for text in [
            "String containing []",
            "String containing ()",
            "Waterfall",
            "the root",
            "Root",
            "unseen label",
        ] {
            let (width, height) = super::mindmap_label_bbox_px(text, &measurer, &style, 200.0);
            assert_eq!(width, 73.0);
            assert_eq!(height, 24.0);
        }
    }

    #[test]
    fn mindmap_circle_encloses_label_diagonal_with_actual_node_look() {
        let mut node = super::MindmapNodeModel {
            id: "0".to_string(),
            dom_id: "node_0".to_string(),
            label: "Circle".to_string(),
            label_type: "markdown".to_string(),
            is_group: false,
            shape: "mindmapCircle".to_string(),
            width: 0.0,
            height: 0.0,
            padding: 7.0,
            css_classes: "mindmap-node section-root section--1".to_string(),
            css_styles: Vec::new(),
            look: String::new(),
            icon: None,
            x: None,
            y: None,
            level: 0,
            node_id: "0".to_string(),
            node_type: 3,
            section: None,
        };
        // A 30x40 label has a 50px diagonal. mindmapCircle supplies all 7px padding;
        // Neo instead uses shared circle.ts's single 16px radius padding step.
        for (node_look, config_look, expected_diameter) in [
            ("", "neo", 82.0),
            ("default", "neo", 82.0),
            ("classic", "neo", 64.0),
            ("handDrawn", "neo", 64.0),
            ("neo", "classic", 82.0),
            ("classic", "classic", 64.0),
        ] {
            node.look = node_look.to_string();
            let config = serde_json::json!({ "look": config_look, "theme": "redux" });
            let dimensions = super::mindmap_node_dimensions_from_label_bbox(
                &node,
                30.0,
                40.0,
                super::mindmap_node_look(&node.look, &config),
            );
            assert_eq!(
                dimensions,
                (expected_diameter, expected_diameter, 30.0, 40.0),
                "node look={node_look}, config look={config_look}"
            );
        }
    }

    #[test]
    fn mindmap_svg_text_style_matches_root_css_font_sources() {
        let config = serde_json::json!({
            "htmlLabels": false,
            "fontFamily": "Root font",
            "fontSize": 18,
            "themeVariables": { "fontFamily": "Theme font", "fontSize": "24px" }
        });
        let style = super::mindmap_text_style(&config);
        assert_eq!(style.font_family.as_deref(), Some("Theme font"));
        assert_eq!(style.font_size, 24.0);
        let typed_style =
            super::mindmap_text_style_with_font_family(&config, "Typed font,monospace");
        assert_eq!(
            typed_style.font_family.as_deref(),
            Some("Typed font,monospace")
        );
        assert_eq!(typed_style.font_size, 24.0);
        let html_style = super::mindmap_text_style_with_font_family(
            &serde_json::json!({"htmlLabels": true, "themeVariables": {"fontSize": "24px"}}),
            "Typed font,monospace",
        );
        assert_eq!(html_style.font_family, typed_style.font_family);
        assert_eq!(html_style.font_size, 16.0);
        assert_eq!(
            super::mindmap_text_style(&serde_json::json!({ "htmlLabels": false, "fontSize": 18 }))
                .font_size,
            18.0
        );
        assert_eq!(
            super::mindmap_text_style(&serde_json::json!({ "htmlLabels": false })).font_size,
            16.0
        );
    }

    #[test]
    fn mindmap_cloud_layout_uses_rendered_path_bbox_dimensions() {
        let measurer = FixedMeasurer;
        let style = super::mindmap_text_style(&serde_json::json!({}));
        let node = super::MindmapNodeModel {
            id: "0".to_string(),
            dom_id: "node_0".to_string(),
            label: "arbitrary cloud label".to_string(),
            label_type: String::new(),
            is_group: false,
            shape: "cloud".to_string(),
            width: 0.0,
            height: 0.0,
            padding: 10.0,
            css_classes: "mindmap-node section-root section--1".to_string(),
            css_styles: Vec::new(),
            look: String::new(),
            icon: None,
            x: None,
            y: None,
            level: 0,
            node_id: "0".to_string(),
            node_type: 0,
            section: None,
        };

        let (width, height, label_width, label_height) =
            super::mindmap_node_dimensions_px(&node, &measurer, &style, 200.0);

        let shape_width = label_width + node.padding;
        let shape_height = label_height + node.padding;
        let expected = crate::svg::mindmap_cloud_rendered_bbox_size_px(shape_width, shape_height)
            .expect("cloud path bounds");

        assert_eq!((label_width, label_height), (73.0, 24.0));
        assert_eq!((width, height), expected);
        assert!(width > shape_width && height > shape_height);
    }

    #[test]
    fn mindmap_hexagon_layout_uses_mermaid_11_16_shape_geometry() {
        let measurer = FixedMeasurer;
        let style = super::mindmap_text_style(&serde_json::json!({}));
        let node = super::MindmapNodeModel {
            id: "0".to_string(),
            dom_id: "node_0".to_string(),
            label: "arbitrary hexagon label".to_string(),
            label_type: String::new(),
            is_group: false,
            shape: "hexagon".to_string(),
            width: 0.0,
            height: 0.0,
            padding: 20.0,
            css_classes: "mindmap-node section-root section--1".to_string(),
            css_styles: Vec::new(),
            look: String::new(),
            icon: None,
            x: None,
            y: None,
            level: 0,
            node_id: "0".to_string(),
            node_type: 0,
            section: None,
        };

        let dimensions = super::mindmap_node_dimensions_px(&node, &measurer, &style, 200.0);

        assert_eq!(dimensions, (115.0, 44.0, 73.0, 24.0));
    }
}
