//! Usecase label preparation, graph placement, and complete Mermaid-compatible SVG.

use merman_model::{
    families::usecase::{UsecaseDiagramModel, UsecaseLabelType},
    presentation::PresentationConfig,
};
use merman_render_core::{
    Bounds, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint, Result,
    layout_work::OperationLayoutWorkControl,
    math::MathRenderer,
    text::{TextMeasurer, TextMetrics, TextStyle},
};
use merman_theme_contract::v2::{Color, FamilyTheme};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, sync::Arc};

pub mod actor;
#[cfg(feature = "layout-dagre")]
mod dagre;
#[cfg(feature = "layout-elk")]
mod elk;
#[cfg(feature = "layout-grid")]
mod grid;
#[cfg(feature = "layout-layered")]
mod layered;
#[cfg(any(
    feature = "layout-dagre",
    feature = "layout-elk",
    feature = "layout-grid",
    feature = "layout-layered",
    feature = "layout-shoal"
))]
pub mod layout;
#[cfg(feature = "layout-shoal")]
mod marker;
mod measure;
#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
mod parity_routes;
mod svg;
mod theme;

#[cfg(feature = "layout-elk")]
pub use elk::prepare as prepare_elk;
#[cfg(feature = "layout-grid")]
pub use grid::prepare as prepare_grid;
#[cfg(feature = "layout-layered")]
pub use layered::{UsecaseLayeredError, prepare as prepare_layered};
#[cfg(feature = "layout-shoal")]
pub use marker::UsecaseMarker;
#[cfg(any(feature = "layout-elk", feature = "layout-shoal"))]
pub use measure::spacing as configured_usecase_spacing;
#[cfg(feature = "layout-shoal")]
pub use svg::{UsecaseCanonicalSvg, UsecaseEdgePainter, UsecasePaintedEdge};
pub use svg::{UsecaseSvgContext, render_usecase_diagram_svg_model};
use theme::UsecaseCssBinding;

/// Mermaid's nonMarkdownToHTML/nonMarkdownToLines treat a literal \n as a line break.
/// Keep the source label intact for accessibility while sharing its rendered text with measurement.
pub(crate) fn normalize_plain_label_line_breaks(text: &str) -> std::borrow::Cow<'_, str> {
    if text.contains("\\n") {
        std::borrow::Cow::Owned(text.replace("\\n", "\n"))
    } else {
        std::borrow::Cow::Borrowed(text)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsecaseDiagramLayout {
    pub nodes: Vec<LayoutNode>,
    pub edges: Vec<LayoutEdge>,
    pub bounds: Option<Bounds>,
}

#[derive(Debug, Clone)]
pub struct UsecaseLabelPlan {
    pub text: String,
    pub(crate) label_type: UsecaseLabelType,
    /// Text bbox before the SVG edge background or node minimum-width box is applied.
    pub(crate) text_metrics: TextMetrics,
    pub metrics: TextMetrics,
    pub(crate) style: TextStyle,
    pub(crate) max_width: Option<f64>,
    pub(crate) styles: indexmap::IndexMap<String, String>,
    /// Final HTML terminal shared by measurement and emission, including effective Markdown config.
    pub(crate) html: Option<Arc<measure::UsecasePreparedHtmlLabel>>,
    #[cfg(feature = "layout-layered")]
    pub(crate) layered_svg: Option<Arc<measure::UsecasePreparedSvgLabel>>,
}

pub fn package_tab_size(label: &UsecaseLabelPlan) -> (f64, f64) {
    const MIN_WIDTH: f64 = 80.0;
    const PADDING_X: f64 = 10.0;
    const PADDING_Y: f64 = 5.0;
    (
        MIN_WIDTH.max(label.metrics.width + 2.0 * PADDING_X),
        label.metrics.height + 2.0 * PADDING_Y,
    )
}

/// Left edge, width and height of the drawn tab measured from the boundary's top.
pub fn package_tab_geometry(
    left: f64,
    top: f64,
    width: f64,
    label: &UsecaseLabelPlan,
    placed_title: Option<&LayoutLabel>,
) -> (f64, f64, f64) {
    let (tab_width, tab_height) = package_tab_size(label);
    let tab_width = width.min(placed_title.map_or(tab_width, |title| title.width));
    let x = placed_title.map_or(left, |title| {
        (title.x - tab_width / 2.0).clamp(left, (left + width - tab_width).max(left))
    });
    let height = placed_title.map_or(tab_height, |title| title.y - top + title.height / 2.0);
    (x, tab_width, height)
}

#[derive(Debug, Clone)]
pub(crate) struct UsecaseJsonRowPlan {
    pub key: UsecaseLabelPlan,
    pub value: UsecaseLabelPlan,
    pub accessible_key: String,
    pub height: f64,
}

#[derive(Debug, Clone)]
pub(crate) struct UsecaseJsonTablePlan {
    pub rows: Vec<UsecaseJsonRowPlan>,
    pub key_width: f64,
    pub value_width: f64,
    pub title_height: f64,
    pub border_width: f64,
}

#[derive(Debug, Clone)]
pub struct UsecaseNodePlan {
    pub id: String,
    pub(crate) source_style: String,
    pub parent: Option<String>,
    pub(crate) source_label: String,
    pub label: UsecaseLabelPlan,
    pub stereotype: Option<UsecaseLabelPlan>,
    pub(crate) folded_stereotype: bool,
    pub width: f64,
    pub height: f64,
    pub is_boundary: bool,
    pub package: bool,
    pub ellipse: bool,
    #[cfg(feature = "layout-layered")]
    pub(crate) layered_ellipse: Option<kurbo::Size>,
    pub(crate) table: Option<UsecaseJsonTablePlan>,
    pub(crate) dagre_helper: bool,
}

#[derive(Debug, Clone)]
pub struct UsecaseEdgePlan {
    pub id: String,
    pub source_style: String,
    #[cfg(feature = "layout-layered")]
    pub(crate) layered_marker_stroke: Option<String>,
    pub animated: bool,
    pub(crate) original_id: Option<String>,
    pub(crate) self_loop_node: Option<String>,
    pub(crate) dagre_recursive: bool,
    pub source: String,
    pub target: String,
    pub label: Option<UsecaseLabelPlan>,
    pub minlen: usize,
    pub start_marker: Option<String>,
    pub end_marker: Option<String>,
    pub(crate) dotted: bool,
    pub(crate) internal: bool,
}

impl UsecaseEdgePlan {
    pub fn dagre_self_loop_endpoint(&self) -> Option<&str> {
        self.self_loop_node
            .as_deref()
            .filter(|_| !self.dagre_recursive)
    }
}

/// The prepared styles, labels and graph inputs shared by layout and emission.
#[derive(Debug)]
pub struct UsecaseMeasured {
    css: UsecaseCssBinding,
    pub nodes: Vec<UsecaseNodePlan>,
    pub edges: Vec<UsecaseEdgePlan>,
}

#[cfg(feature = "layout-shoal")]
impl UsecaseMeasured {
    pub fn canonical_route_style(
        &self,
        config: &Value,
    ) -> merman_render_core::route_paint::RouteStyle {
        let curve = self.css.native_curve().unwrap_or_else(|| {
            config
                .pointer("/flowchart/curve")
                .and_then(Value::as_str)
                .unwrap_or("basis")
        });
        let radius = config
            .pointer("/flowchart/roundedCornerRadius")
            .and_then(Value::as_f64)
            .filter(|radius| radius.is_finite() && *radius >= 0.0)
            .unwrap_or(8.0);
        merman_render_core::route_paint::RouteStyle::from_interpolate(curve, radius)
    }

    pub fn marker_for_edge(&self, plan: &UsecaseEdgePlan, start: bool) -> Option<UsecaseMarker> {
        UsecaseMarker::for_edge(plan, start, self.css.look == "neo", self.css.marker_style)
    }

    pub fn connector_width(&self) -> f64 {
        self.css.connector_width
    }
}

/// Measured Usecase inputs paired with the backend's accepted geometry.
#[derive(Debug)]
pub struct UsecasePrepared {
    measured: UsecaseMeasured,
    layout: UsecaseDiagramLayout,
    edge_paths: HashMap<String, Vec<LayoutPoint>>,
    elk: bool,
    #[cfg(feature = "layout-grid")]
    grid: Option<grid::UsecaseGridPaint>,
    #[cfg(feature = "layout-layered")]
    layered: Option<layered::Paint>,
}

impl UsecasePrepared {
    pub fn new(
        measured: UsecaseMeasured,
        layout: UsecaseDiagramLayout,
        edge_paths: HashMap<String, Vec<LayoutPoint>>,
        elk: bool,
    ) -> Self {
        Self {
            measured,
            layout,
            edge_paths,
            elk,
            #[cfg(feature = "layout-grid")]
            grid: None,
            #[cfg(feature = "layout-layered")]
            layered: None,
        }
    }

    pub fn layout(&self) -> &UsecaseDiagramLayout {
        &self.layout
    }

    #[cfg(all(feature = "layout-layered", feature = "layout-dagre"))]
    pub fn mark_layered_dagre_fallback(&mut self) {
        self.layered = Some(layered::Paint::DagreFallback);
    }

    #[cfg(feature = "layout-shoal")]
    pub fn measured(&self) -> &UsecaseMeasured {
        &self.measured
    }

    #[cfg(feature = "layout-shoal")]
    pub fn rough_edge_seed(&self) -> Option<f64> {
        let css = &self.measured.css;
        (css.look == "handDrawn" || css.sketches_edges()).then_some(css.hand_drawn_seed)
    }

    fn edge_points(&self, edge: &LayoutEdge) -> Vec<LayoutPoint> {
        self.edge_paths
            .get(&edge.id)
            .cloned()
            .unwrap_or_else(|| edge.points.clone())
    }
}

pub fn prepare_usecase_measurements(
    model: &UsecaseDiagramModel,
    config: &PresentationConfig,
    native: Option<&FamilyTheme>,
    canvas: Option<Color>,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    work: &mut OperationLayoutWorkControl,
) -> Result<UsecaseMeasured> {
    let css = resolve_css(config, native, canvas);
    let (nodes, edges) = measure::measure(
        model,
        config,
        &css,
        measurer,
        math_renderer,
        work,
        #[cfg(feature = "layout-grid")]
        None,
    )?;
    Ok(UsecaseMeasured { css, nodes, edges })
}

fn resolve_css(
    config: &PresentationConfig,
    native: Option<&FamilyTheme>,
    canvas: Option<Color>,
) -> UsecaseCssBinding {
    let mut css = UsecaseCssBinding::new(config.as_value());
    css.canvas_background = canvas;
    if let Some(theme) = native {
        css.apply_native(config, theme, canvas);
    }
    css
}

#[cfg(feature = "layout-dagre")]
pub fn layout_dagre(
    model: &UsecaseDiagramModel,
    config: &Value,
    measured: &mut UsecaseMeasured,
    work: &mut OperationLayoutWorkControl,
) -> Result<UsecaseDiagramLayout> {
    dagre::expand_self_loops(
        &mut measured.nodes,
        &mut measured.edges,
        &measured.css.generic_font,
        work,
    )?;
    dagre::layout(model, config, &measured.nodes, &mut measured.edges, work)
}

#[cfg(feature = "layout-dagre")]
pub fn prepare_dagre(
    model: &UsecaseDiagramModel,
    config: &Value,
    mut measured: UsecaseMeasured,
    work: &mut OperationLayoutWorkControl,
) -> Result<UsecasePrepared> {
    let mut layout = layout_dagre(model, config, &mut measured, work)?;
    let edge_paths = parity_routes::prepare_edge_paths(
        &mut layout,
        &measured.nodes,
        &measured.edges,
        config,
        false,
        work,
    )?;
    Ok(UsecasePrepared::new(measured, layout, edge_paths, false))
}

/// The literal source Mermaid passes to createText, before selecting its math branch.
pub(crate) fn create_text_source(text: &str, kind: UsecaseLabelType) -> std::borrow::Cow<'_, str> {
    match kind {
        UsecaseLabelType::Markdown => std::borrow::Cow::Borrowed(text),
        UsecaseLabelType::Text => std::borrow::Cow::Owned(
            normalize_plain_label_line_breaks(text)
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;"),
        ),
    }
}

fn shoal_html_labels(config: &PresentationConfig) -> bool {
    cfg!(feature = "layout-shoal")
        && config.get_str("layout") == Some("shoal")
        && config.as_value().get("htmlLabels").and_then(Value::as_bool) != Some(false)
}

fn label_source(
    text: &str,
    kind: UsecaseLabelType,
    authored_html: bool,
) -> std::borrow::Cow<'_, str> {
    if authored_html && kind == UsecaseLabelType::Text {
        std::borrow::Cow::Borrowed(text)
    } else {
        create_text_source(text, kind)
    }
}

/// Reserializes admitted HTML as XHTML without interpreting entity-authored tags as markup.
fn html_text_label_fragment(
    sanitized: &str,
    charge: &mut impl FnMut(usize) -> Result<()>,
) -> Result<String> {
    charge(sanitized.len().max(1))?;
    let text = normalize_plain_label_line_breaks(sanitized);
    let paragraph = format!("<p>{}</p>", text.replace('\n', "<br/>"));
    charge(paragraph.len())?;
    Ok(merman_markup::sanitize::html_fragment_to_xhtml(&paragraph).unwrap_or_default())
}

fn accessible_html_text(
    sanitized: &str,
    mut charge: impl FnMut(usize) -> Result<()>,
) -> Result<String> {
    charge(sanitized.len().max(1))?;
    if !sanitized.contains(['<', '&', '\n']) && !sanitized.contains("\\n") {
        return Ok(sanitized.to_owned());
    }
    let fragment = html_text_label_fragment(sanitized, &mut charge)?;
    charge(fragment.len().max(1))?;
    let lines = merman_render_core::text::line_break::mermaid_html_breaks_to_newlines(&fragment);
    Ok(
        merman_render_core::text::mermaid_xhtml_label_text_content(&lines)
            .unwrap_or_default()
            .replace('\n', " "),
    )
}

fn sanitize_source_label(
    text: &str,
    kind: UsecaseLabelType,
    config: &PresentationConfig,
) -> String {
    let text = merman_markup::sanitize::sanitize_text(text, config.as_config());
    if cfg!(feature = "layout-shoal")
        && config.get_str("layout") == Some("shoal")
        && kind == UsecaseLabelType::Text
        && !shoal_html_labels(config)
        && let std::borrow::Cow::Owned(decoded) =
            merman_render_core::entities::decode_svg_text_content_entities(&text)
    {
        return decoded;
    }
    text
}

pub fn requires_math(
    model: &UsecaseDiagramModel,
    config: &merman_model::presentation::PresentationConfig,
) -> bool {
    if config.as_value().get("htmlLabels").and_then(Value::as_bool) == Some(false) {
        return false;
    }
    let has_math = |text: &str, kind, authored_html| {
        let sanitized = merman_markup::sanitize::sanitize_text(text, config.as_config());
        merman_render_core::math::contains_delimited_math(&label_source(
            &sanitized,
            kind,
            authored_html,
        ))
    };
    let mut labels = model
        .nodes
        .iter()
        .flat_map(|node| {
            std::iter::once((node.label.as_str(), node.label_type)).chain(
                node.stereotype
                    .as_deref()
                    .map(|text| (text, UsecaseLabelType::Text)),
            )
        })
        .chain(
            model
                .boundaries
                .iter()
                .map(|boundary| (boundary.label.as_str(), boundary.label_type)),
        )
        .chain(
            model
                .notes
                .iter()
                .map(|note| (note.label.as_str(), note.label_type)),
        )
        .chain(model.relationships.iter().filter_map(|edge| {
            edge.label
                .as_deref()
                .map(|text| (text, edge.label_type.unwrap_or_default()))
        }));
    let authored_html = shoal_html_labels(config);
    if labels.any(|(text, kind)| has_math(text, kind, authored_html)) {
        return true;
    }
    // JSON cells show the complete leaf path, not each object key in isolation.
    let mut pending: Vec<_> = model
        .json_nodes
        .iter()
        .map(|node| (node, &node.value, String::new()))
        .collect();
    let literal_has_math = |text: &str| has_math(text, UsecaseLabelType::Text, false);
    while let Some((node, value, path)) = pending.pop() {
        let display_has_math = |text: &str| {
            node.display_string(text)
                .is_ok_and(|text| literal_has_math(&text))
        };
        match value {
            Value::Array(values)
                if !values.is_empty()
                    && values
                        .iter()
                        .all(|value| !value.is_array() && !value.is_object()) =>
            {
                if literal_has_math(&path)
                    || values
                        .iter()
                        .filter_map(Value::as_str)
                        .any(display_has_math)
                {
                    return true;
                }
            }
            Value::Array(values) if !values.is_empty() => {
                pending.extend(
                    values
                        .iter()
                        .enumerate()
                        .map(|(index, value)| (node, value, format!("{path}[{index}]"))),
                );
            }
            Value::Object(values) if !values.is_empty() => {
                pending.extend(values.iter().filter_map(|(key, value)| {
                    let key = node.display_string(key).ok()?;
                    let path = if path.is_empty() {
                        key.into_owned()
                    } else {
                        format!("{path}.{key}")
                    };
                    Some((node, value, path))
                }));
            }
            _ => {
                if literal_has_math(&path) || value.as_str().is_some_and(display_has_math) {
                    return true;
                }
            }
        }
    }
    false
}
