//! Usecase label preparation, Dagre placement, and complete Mermaid-compatible SVG.

use merman_model::{
    families::usecase::{UsecaseDiagramModel, UsecaseLabelType},
    presentation::PresentationConfig,
};
use merman_render_core::{
    Bounds, LayoutEdge, LayoutNode, LayoutPoint, Result,
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
#[cfg(any(
    feature = "layout-dagre",
    feature = "layout-elk",
    feature = "layout-shoal"
))]
pub mod layout;
#[cfg(feature = "layout-shoal")]
mod marker;
mod measure;
mod svg;
mod theme;

#[cfg(feature = "layout-shoal")]
pub use marker::UsecaseMarker;
pub use measure::number as configured_usecase_number;
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
    pub(crate) table: Option<UsecaseJsonTablePlan>,
    pub(crate) dagre_helper: bool,
}

#[derive(Debug, Clone)]
pub struct UsecaseEdgePlan {
    pub id: String,
    pub source_style: String,
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

/// Measured Usecase inputs paired with the backend's accepted geometry.
#[derive(Debug)]
pub struct UsecasePrepared {
    measured: UsecaseMeasured,
    layout: UsecaseDiagramLayout,
    edge_paths: HashMap<String, Vec<LayoutPoint>>,
    elk: bool,
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
        }
    }

    pub fn layout(&self) -> &UsecaseDiagramLayout {
        &self.layout
    }

    #[cfg(feature = "layout-shoal")]
    pub fn edges(&self) -> &[UsecaseEdgePlan] {
        &self.measured.edges
    }

    #[cfg(feature = "layout-shoal")]
    pub fn hand_drawn_seed(&self) -> Option<f64> {
        let css = &self.measured.css;
        (css.look == "handDrawn").then_some(css.hand_drawn_seed)
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
    let mut css = UsecaseCssBinding::new(config.as_value());
    if let Some(theme) = native {
        css.apply_native(config, theme, canvas);
    }
    let (nodes, edges) = measure::measure(model, config, &css, measurer, math_renderer, work)?;
    Ok(UsecaseMeasured { css, nodes, edges })
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

pub fn requires_math(
    model: &UsecaseDiagramModel,
    config: &merman_model::presentation::PresentationConfig,
) -> bool {
    if config.as_value().get("htmlLabels").and_then(Value::as_bool) == Some(false) {
        return false;
    }
    let has_math = |(text, kind): (&str, UsecaseLabelType)| {
        let sanitized = merman_markup::sanitize::sanitize_text(text, config.as_config());
        merman_render_core::math::contains_delimited_math(&create_text_source(&sanitized, kind))
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
    if labels.any(has_math) {
        return true;
    }
    // JSON cells show the complete leaf path, not each object key in isolation.
    let mut pending: Vec<_> = model
        .json_nodes
        .iter()
        .map(|node| (node, &node.value, String::new()))
        .collect();
    while let Some((node, value, path)) = pending.pop() {
        let display_has_math = |text: &str| {
            node.display_string(text)
                .is_ok_and(|text| has_math((&text, UsecaseLabelType::Text)))
        };
        match value {
            Value::Array(values)
                if !values.is_empty()
                    && values
                        .iter()
                        .all(|value| !value.is_array() && !value.is_object()) =>
            {
                if has_math((&path, UsecaseLabelType::Text))
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
                if has_math((&path, UsecaseLabelType::Text))
                    || value.as_str().is_some_and(display_has_math)
                {
                    return true;
                }
            }
        }
    }
    false
}
