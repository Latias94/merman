//! Mindmap labels, layout projections, theme binding, and balanced SVG emission.

use merman_model::presentation::PresentationConfig;
use merman_render_core::config::config_f64_css_px;
use merman_render_core::{
    Bounds, Error, LayoutCluster, LayoutEdge, LayoutNode, LayoutPoint, RenderCapability, Result,
    SwimlaneLaneLayout, config, html,
    layout_work::OperationLayoutWorkControl,
    math::{self, MathRenderer},
    resources::{self, OperationWorkMeter},
    text::{
        self, TextMeasurer, TextStyle, WrapMode, create_text_html::FlowchartLabelMetricsRequest,
    },
};
use merman_theme_contract::v2::FamilyTheme;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, sync::Arc};

mod css_binding;
#[cfg(feature = "layout-grid")]
mod grid;
mod registered;
mod svg;
pub(crate) use css_binding::MindmapCssBinding;
mod native_theme;
#[cfg(test)]
mod tests;
mod theme;
mod tidy_tree;

pub use svg::{MindmapSvgContext, render_mindmap_diagram_svg};

pub(crate) use native_theme::MindmapNativeStyle;
pub(crate) use theme::MindmapAppearance;

pub(crate) fn mindmap_label_wrapping_width_px(
    node: &MindmapNodeModel,
    effective_config: &Value,
) -> f64 {
    // mindmapRenderer.ts clears the requested width for these shapes. labelHelper
    // then falls back to the shared Flowchart wrapping width (default 120px).
    let clears_width = matches!(node.shape.as_str(), "rect" | "rounded" | "hexagon");
    if !clears_width && node.width.is_finite() && node.width != 0.0 {
        node.width.max(1.0)
    } else {
        config_f64_css_px(effective_config, &["flowchart", "wrappingWidth"])
            .unwrap_or(120.0)
            .max(1.0)
    }
}

pub(crate) fn mindmap_node_look<'a>(
    model_look: &'a str,
    effective_config: &'a Value,
) -> config::DiagramLook<'a> {
    let model_look = model_look.trim();
    if model_look.is_empty() || model_look == "default" {
        config::config_diagram_look(effective_config)
    } else {
        config::DiagramLook::from_raw(Some(model_look))
    }
}

type MindmapModel = merman_model::families::mindmap::MindmapDiagramRenderModel;
type MindmapNodeModel = merman_model::families::mindmap::MindmapDiagramRenderNode;
type MindmapEdgeModel = merman_model::families::mindmap::MindmapDiagramRenderEdge;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MindmapDiagramLayout {
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
    pub bounds: Option<Bounds>,
    /// Swimlane decoration owned by the selected renderer, not semantic Mindmap nodes.
    #[serde(skip)]
    pub swimlane_lanes: Vec<LayoutCluster>,
}

/// The existing Mermaid label measurement used when Mindmap HTML contains math.
pub type MindmapMathLabelMetrics = fn(FlowchartLabelMetricsRequest<'_>) -> text::TextMetrics;

/// Geometry returned by the shared Swimlane loader for already-measured Mindmap nodes.
pub struct MindmapSwimlaneGeometry {
    pub nodes: Vec<LayoutNode>,
    pub edge_points: rustc_hash::FxHashMap<String, Vec<LayoutPoint>>,
    pub lanes: Vec<SwimlaneLaneLayout>,
    pub bounds: Option<Bounds>,
}

pub type MindmapSwimlaneSolver = fn(
    &[(&LayoutNode, &str)],
    &[LayoutEdge],
    &PresentationConfig,
    Arc<OperationWorkMeter>,
) -> Result<MindmapSwimlaneGeometry>;

/// The facade selects a Mermaid loader before preparing the family.
pub enum MindmapLayoutBackend<'a> {
    TidyTree,
    #[cfg(feature = "layout-grid")]
    Grid,
    #[cfg(feature = "layout-dagre")]
    Dagre,
    #[cfg(feature = "layout-cytoscape")]
    CoseBilkent,
    Swimlane(MindmapSwimlaneSolver),
    #[cfg(feature = "layout-elk")]
    Elk {
        algorithm: merman_layout_elk::Algorithm,
        operation_seed: merman_layout_elk::ElkOperationSeed,
    },
    Unavailable(&'a dyn Fn() -> Error),
}

pub struct MindmapLayoutContext<'a> {
    pub backend: MindmapLayoutBackend<'a>,
    pub text_measurer: &'a dyn TextMeasurer,
    pub math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
    pub measure_math_label: MindmapMathLabelMetrics,
    pub work_meter: &'a Arc<OperationWorkMeter>,
}

#[derive(Debug, Clone, Copy)]
enum MindmapEdgeRouting {
    TidyTree,
    #[cfg(feature = "layout-grid")]
    Grid(merman_render_core::graph_layout::mermaid_grid::GridPaint),
    #[cfg(feature = "layout-dagre")]
    Dagre,
    #[cfg(feature = "layout-cytoscape")]
    CoseBilkent,
    Swimlane,
    #[cfg(feature = "layout-elk")]
    Elk,
}

impl MindmapEdgeRouting {
    fn clips_circle_endpoints(self) -> bool {
        match self {
            Self::Swimlane => true,
            #[cfg(feature = "layout-dagre")]
            Self::Dagre => true,
            _ => false,
        }
    }
}

#[derive(Debug)]
pub struct MindmapPreparedArtifact {
    layout: MindmapDiagramLayout,
    appearance: MindmapAppearance,
    prepared_labels: MindmapPreparedLabels,
    routing: MindmapEdgeRouting,
}

impl MindmapPreparedArtifact {
    pub const fn layout(&self) -> &MindmapDiagramLayout {
        &self.layout
    }
}

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
    _retained_reservation: resources::PreparedTextRetainedReservation,
}

impl MindmapPreparedLabels {
    pub(crate) fn prepare(
        model: &MindmapModel,
        config: &PresentationConfig,
        work_meter: &Arc<resources::OperationWorkMeter>,
    ) -> Result<Self> {
        let mut prepared = Self::default();
        if !config::config_effective_html_labels(config.as_value()) {
            return Ok(prepared);
        }
        for node in &model.nodes {
            work_meter.checkpoint(merman_model::OperationPhase::Layout)?;
            if !math::contains_delimited_math(&node.label) {
                let label =
                    MindmapPreparedLabel::prepare(&node.id, &node.label, config, work_meter)?;
                prepared.labels.insert(node.id.clone(), label);
            }
        }
        Ok(prepared)
    }

    pub(crate) fn xhtml_for(&self, node_id: &str) -> Option<&str> {
        self.labels.get(node_id).map(|label| label.xhtml.as_str())
    }
}

impl MindmapPreparedLabel {
    fn prepare(
        node_id: &str,
        text: &str,
        config: &PresentationConfig,
        work_meter: &Arc<resources::OperationWorkMeter>,
    ) -> Result<Self> {
        Self::prepare_at_phase(
            node_id,
            text,
            config,
            work_meter,
            merman_model::OperationPhase::Layout,
        )
    }

    fn prepare_at_phase(
        node_id: &str,
        text: &str,
        config: &PresentationConfig,
        work_meter: &Arc<resources::OperationWorkMeter>,
        phase: merman_model::OperationPhase,
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
        let sink = html::HtmlSanitizeSink::new(max, work_meter, &parse_work, phase);
        // createText restores entity references before HTML consumption, not after serialization.
        let source = merman_model::text::restore_mermaid_entity_spelling(text);
        let html = text::replace_fontawesome_icons(
            &text::mermaid_markdown_to_xhtml_label_fragment(source.as_ref(), true),
        );
        let xhtml = match merman_markup::sanitize::sanitize_xhtml_with_sink(
            &html,
            config.as_config(),
            &sink,
        ) {
            Ok(xhtml) => xhtml,
            Err(merman_markup::sanitize::SanitizeFailure::Output(
                html::HtmlOutputError::Operation(error),
            )) => return Err(error.into()),
            Err(merman_markup::sanitize::SanitizeFailure::Output(
                html::HtmlOutputError::Limit { attempted },
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
        let retained_bytes = owner_bytes.saturating_add(xhtml.capacity());
        let reservation = work_meter.reserve_prepared_text_retained_bytes(retained_bytes)?;
        Ok(Self {
            xhtml,
            _retained_reservation: reservation,
        })
    }
}

/// Native admission uses the same Markdown, entity and sanitizer path as prepared labels.
pub fn native_html_label_source(
    text: &str,
    config: &PresentationConfig,
    work_meter: &Arc<resources::OperationWorkMeter>,
) -> Result<String> {
    MindmapPreparedLabel::prepare_at_phase(
        "",
        text,
        config,
        work_meter,
        merman_model::OperationPhase::Semantic,
    )
    .map(|label| label.xhtml)
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
        config::config_font_family_css(effective_config),
    )
}

fn mindmap_text_style_with_font_family(
    effective_config: &Value,
    font_family_css: String,
) -> TextStyle {
    // HTML labels and SVG text inherit the same root CSS font size.
    let font_size =
        config::config_theme_font_size_css_or_root_number_px(effective_config, 16.0).max(1.0);
    TextStyle {
        font_family: Some(font_family_css),
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
    merman_model::text::restore_mermaid_entity_spelling(text)
}

#[cfg(test)]
fn mindmap_label_bbox_px(
    text: &str,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    max_node_width_px: f64,
) -> (f64, f64) {
    let config = tests::presentation_config(serde_json::json!({
        "securityLevel": "strict",
        "htmlLabels": true
    }));
    let work_meter = Arc::new(resources::OperationWorkMeter::new(
        resources::RenderResourcePolicy::unbounded_for_trusted_input(),
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
        |_| unreachable!("test label does not contain math"),
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
    measure_math_label: MindmapMathLabelMetrics,
) -> Result<(f64, f64)> {
    if !config::config_effective_html_labels(config.as_value()) {
        let source = mindmap_svg_label_source(text);
        let lines = text::mermaid_markdown_to_wrapped_word_lines(
            measurer,
            source.as_ref(),
            style,
            Some(max_node_width_px.max(1.0)),
            WrapMode::SvgLike,
        );
        let metrics = text::measure_svg_markdown_word_lines(
            measurer,
            &lines,
            style,
            text::inline_html::SvgTextEntityMode::CreateTextSource,
        );
        return Ok((metrics.width.max(0.0), metrics.height.max(0.0)));
    }

    if !math::contains_delimited_math(text) {
        let xhtml = prepared_xhtml.ok_or_else(|| Error::InvalidModel {
            message: "Mindmap HTML label is missing its prepared fragment".to_string(),
        })?;
        // Preserve Mindmap's HTML min-content and wrapping rules while measuring the exact
        // recovered structure retained for emission, including sanitizer removals.
        let metrics = text::inline_html::measure_html_with_document_root(
            measurer,
            xhtml,
            style,
            Some(max_node_width_px.max(1.0)),
            // HTML rem uses the document root, independently of the diagram's inherited font.
            16.0,
        );
        return Ok((metrics.width.max(0.0), metrics.height.max(0.0)));
    }

    let math_renderer = math_renderer.ok_or_else(|| Error::MissingCapability {
        capability: RenderCapability::Math,
        diagram_type: "mindmap".to_string(),
    })?;
    let metrics = measure_math_label(FlowchartLabelMetricsRequest {
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
pub(crate) fn mindmap_rect_label_padding_px(look: config::DiagramLook<'_>) -> (f64, f64) {
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
    look: config::DiagramLook<'_>,
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
            svg::mindmap_cloud_rendered_bbox_size_px(shape_w, shape_h).unwrap_or((shape_w, shape_h))
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
        config::DiagramLook::from_raw(Some(&node.look)),
    )
}

fn compute_bounds(nodes: &[LayoutNode], edges: &[LayoutEdge]) -> Option<Bounds> {
    let corners = nodes.iter().flat_map(|node| {
        let half_width = node.width / 2.0;
        let half_height = node.height / 2.0;
        [
            (node.x - half_width, node.y - half_height),
            (node.x + half_width, node.y + half_height),
        ]
    });
    let points = edges
        .iter()
        .flat_map(|edge| edge.points.iter().map(|point| (point.x, point.y)));
    Bounds::from_points(corners.chain(points))
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

pub fn prepare_mindmap_diagram(
    model: &MindmapModel,
    config: &PresentationConfig,
    native_theme: Option<&FamilyTheme>,
    context: MindmapLayoutContext<'_>,
) -> Result<MindmapPreparedArtifact> {
    let appearance = match native_theme {
        Some(theme) => MindmapAppearance::Native(MindmapNativeStyle::resolve(theme)),
        None => {
            context
                .work_meter
                .checkpoint(merman_model::OperationPhase::Layout)?;
            let binding = MindmapCssBinding::new(config);
            context
                .work_meter
                .checkpoint(merman_model::OperationPhase::Layout)?;
            MindmapAppearance::Mermaid(Box::new(binding))
        }
    };
    let text_style = appearance.text_style();
    let backend = context.backend;
    let work_meter = Arc::clone(context.work_meter);
    let mut work = OperationLayoutWorkControl::new(Arc::clone(&work_meter));
    let adapter_work = mindmap_layout_adapter_work(
        model,
        matches!(&backend, MindmapLayoutBackend::TidyTree),
        &work,
    )?;
    work.charge_adapter(adapter_work)?;

    let effective_config = config.as_value();
    let text_measurer = context.text_measurer;
    let math_renderer = context.math_renderer;

    let mut nodes: Vec<LayoutNode> = Vec::with_capacity(model.nodes.len());
    let prepared_labels = MindmapPreparedLabels::prepare(model, config, &work_meter)?;
    // Mermaid's Mindmap DB emits nodes in parser preorder, and Cytoscape consumes that order
    // directly. The typed model already preserves it; re-sorting numeric-looking ids here added
    // O(N log N) work and changed the geometry of manually constructed source-order models.
    for n in &model.nodes {
        work_meter.checkpoint(merman_model::OperationPhase::Layout)?;
        let (label_width, label_height) = mindmap_label_bbox_px_with_math(
            &n.label,
            text_measurer,
            text_style,
            mindmap_label_wrapping_width_px(n, effective_config),
            config,
            math_renderer,
            prepared_labels.xhtml_for(&n.id),
            context.measure_math_label,
        )?;
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

    let (layout, routing) = match backend {
        #[cfg(feature = "layout-grid")]
        MindmapLayoutBackend::Grid => {
            let (layout, paint) = grid::layout(model, nodes, &edge_indices, config, &work_meter)?;
            (layout, MindmapEdgeRouting::Grid(paint))
        }
        MindmapLayoutBackend::Swimlane(solve) => (
            registered::swimlane(model, nodes, config, work_meter, solve)?,
            MindmapEdgeRouting::Swimlane,
        ),
        #[cfg(feature = "layout-dagre")]
        MindmapLayoutBackend::Dagre => (
            registered::dagre(model, nodes, effective_config, &mut work)?,
            MindmapEdgeRouting::Dagre,
        ),
        #[cfg(feature = "layout-elk")]
        MindmapLayoutBackend::Elk {
            algorithm,
            operation_seed,
        } => (
            registered::elk(
                model,
                nodes,
                effective_config,
                algorithm,
                operation_seed,
                &mut work,
            )?,
            MindmapEdgeRouting::Elk,
        ),
        #[cfg(feature = "layout-cytoscape")]
        MindmapLayoutBackend::CoseBilkent => (
            registered::cose_bilkent(model, nodes, &edge_indices, &mut work)?,
            MindmapEdgeRouting::CoseBilkent,
        ),
        MindmapLayoutBackend::TidyTree => {
            let edges = tidy_tree::layout(
                &mut nodes,
                &model.nodes,
                &model.edges,
                &edge_indices,
                work_meter.as_ref(),
            )?;
            let bounds = compute_bounds(&nodes, &edges);
            (
                MindmapDiagramLayout {
                    swimlane_lanes: Vec::new(),
                    nodes,
                    edges,
                    bounds,
                },
                MindmapEdgeRouting::TidyTree,
            )
        }
        MindmapLayoutBackend::Unavailable(error) => return Err(error()),
    };
    Ok(MindmapPreparedArtifact {
        layout,
        appearance,
        prepared_labels,
        routing,
    })
}
