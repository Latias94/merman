//! Class presentation binding, preparation, and complete SVG emission.

use merman_render_core::entities::decode_entities_minimal;
use merman_render_core::layout_work::OperationLayoutWorkControl;
use merman_render_core::math::MathRenderer;
use merman_render_core::text::{
    MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX, MermaidMarkdownAnalysis, TextMeasurer, TextStyle,
    WrapMode, analyze_mermaid_markdown, measure_mermaid_text_dimensions,
};
use merman_render_core::{Bounds, Error, Result};
#[cfg(any(
    feature = "layout-dagre",
    feature = "layout-elk",
    feature = "layout-grid"
))]
use merman_render_core::{LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint};
use rustc_hash::FxHashMap;
use std::sync::Arc;

pub(crate) mod config;
#[cfg(feature = "layout-dagre")]
pub mod dagre;
#[cfg(feature = "layout-elk")]
pub mod elk_layout;
#[cfg(feature = "layout-grid")]
pub mod grid;
pub mod measured;
mod model;
mod native_theme;
pub(crate) mod node_binding;
mod svg;
pub use self::config::{ClassLayoutSettings, ClassRenderConfig};
pub(crate) use self::native_theme::ClassNativeStyle;
pub use model::{
    ClassDiagramLayout, ClassNodeLabelPlan, ClassNodeRowMetrics, ClassPreparedHtmlLabel,
    ClassPreparedHtmlNodeLabels, ClassRenderItem, ClassRenderRoot, ClassRenderRootId,
    ClassRenderTree,
};
pub use node_binding::ClassNodeVisualPlan;
#[cfg(feature = "layout-shoal")]
pub use svg::{ClassCanonicalEdge, ClassCanonicalRoutes, class_canonical_marker};
pub use svg::{ClassSvgContext, render_class_diagram_svg_model_with_config};
mod theme;
use theme::ClassCssThemeBinding;
pub use theme::{ClassRelationThemePlan, ClassTextThemePlan};

pub type ClassDiagramModel = merman_model::families::class::ClassDiagram;
type ClassNode = merman_model::families::class::ClassNode;

pub(crate) const CLASS_CARDINALITY_FONT_SIZE_PX: f64 = 11.0;
pub(crate) const CLASS_SVG_EDGE_LABEL_PADDING_PX: f64 = 2.0;

/// Returns the fixed Mermaid text style used by class-relation cardinalities.
///
/// Cardinalities inherit the diagram font family, but Mermaid's stylesheet owns their size at
/// 11px independently of the base typography route. Layout and SVG emission must therefore use
/// this same style instead of the base label style.
pub(crate) fn class_cardinality_text_style(base: &TextStyle) -> TextStyle {
    TextStyle {
        font_family: base.font_family.clone(),
        font_size: CLASS_CARDINALITY_FONT_SIZE_PX,
        font_weight: None,
        font_style: None,
    }
}

pub(crate) fn class_node_requires_math(node: &ClassNode) -> bool {
    [
        node.label.as_str(),
        node.text.as_str(),
        node.type_param.as_str(),
    ]
    .into_iter()
    .chain(node.annotations.iter().map(String::as_str))
    .chain(
        node.members
            .iter()
            .chain(node.methods.iter())
            .map(|member| member.display_text.as_str()),
    )
    .any(merman_render_core::math::contains_delimited_math)
}

pub fn class_requires_math(model: &ClassDiagramModel) -> bool {
    model
        .classes()
        .any(|(_, class)| class_node_requires_math(class.node))
        || model.relations().any(|relation| {
            merman_render_core::math::contains_delimited_math(&relation.title)
                || relation
                    .relation_title_1
                    .as_deref()
                    .is_some_and(merman_render_core::math::contains_delimited_math)
                || relation
                    .relation_title_2
                    .as_deref()
                    .is_some_and(merman_render_core::math::contains_delimited_math)
        })
        || model
            .notes()
            .map(|note| note.note.text.as_str())
            .chain(
                model
                    .interfaces()
                    .map(|iface| iface.interface.label.as_str()),
            )
            .chain(model.namespaces().flat_map(|(_, namespace)| {
                [
                    namespace.namespace.label.as_str(),
                    namespace.namespace.id.as_str(),
                ]
            }))
            .any(merman_render_core::math::contains_delimited_math)
}

/// Estimates the source-backed graph preparation work shared by Class Dagre and ELK layouts.
///
/// Both backends share source measurements. The conservative family allowance also covers Dagre
/// namespace extraction, which scans edges while copying nested descendants, in addition to the
/// linear measurement and projection baseline.
pub fn class_layout_work_units(
    model: &ClassDiagramModel,
    work_control: &OperationLayoutWorkControl,
) -> Result<usize> {
    let complexity = merman_model::resources::ClassComplexity::from_model(model);
    let baseline = work_control.checked_mul(
        work_control.checked_add(complexity.nodes, complexity.edges)?,
        4,
    )?;
    if complexity.namespaces == 0 || complexity.edges == 0 {
        return Ok(baseline);
    }

    let depth = complexity.namespace_depth.max(1);
    let namespace_edge_scans = work_control.checked_mul(
        work_control.checked_mul(complexity.namespaces, complexity.edges)?,
        depth,
    )?;
    let extraction_edge_scans = work_control.checked_mul(complexity.nodes, complexity.edges)?;
    work_control.checked_add(
        work_control.checked_add(baseline, namespace_edge_scans)?,
        extraction_edge_scans,
    )
}

pub(crate) fn class_member_display_text(
    member: &merman_model::families::class::ClassMember,
) -> String {
    // Mermaid ClassMember.parseMember preserves internal attribute whitespace in `text`;
    // shapeUtil.addText passes that text directly to createText, including spaces around ':'.
    member.display_text.trim().to_string()
}

pub(crate) fn class_member_create_text_input(
    member: &merman_model::families::class::ClassMember,
) -> String {
    node_binding::class_generic_member_source(member, ["<", ">"])
        .unwrap_or_else(|| class_member_display_text(member))
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

pub fn normalize_dir(direction: &str) -> String {
    match direction.trim().to_uppercase().as_str() {
        "TB" | "TD" => "TB".to_string(),
        "BT" => "BT".to_string(),
        "LR" => "LR".to_string(),
        "RL" => "RL".to_string(),
        other => other.to_string(),
    }
}

fn class_dom_decl_order_index(dom_id: &str) -> usize {
    dom_id
        .rsplit_once('-')
        .and_then(|(_, suffix)| suffix.parse::<usize>().ok())
        .unwrap_or(usize::MAX)
}

pub fn class_namespace_ids_in_decl_order(model: &ClassDiagramModel) -> Vec<&str> {
    let mut namespaces: Vec<_> = model.namespaces().map(|(_, namespace)| namespace).collect();
    namespaces.sort_by(|lhs, rhs| {
        class_dom_decl_order_index(lhs.dom_id)
            .cmp(&class_dom_decl_order_index(rhs.dom_id))
            .then_with(|| lhs.id.cmp(&rhs.id))
    });
    namespaces
        .into_iter()
        .map(|ns| ns.namespace.id.as_str())
        .collect()
}

pub fn class_namespace_label<'a>(model: &'a ClassDiagramModel, id: &'a str) -> &'a str {
    model
        .namespace(id)
        .and_then(|ns| {
            let label = ns.namespace.label.as_str();
            (!label.is_empty()).then_some(label)
        })
        .unwrap_or(id)
}

pub type Rect = merman_render_core::euclid_box::Box2;

#[cfg(any(
    feature = "layout-dagre",
    feature = "layout-elk",
    feature = "layout-grid",
    test
))]
#[derive(Debug)]
pub struct EdgeTerminalMetrics {
    pub start_left: Option<(f64, f64)>,
    pub start_right: Option<(f64, f64)>,
    pub end_left: Option<(f64, f64)>,
    pub end_right: Option<(f64, f64)>,
    pub start_marker: f64,
    pub end_marker: f64,
    #[cfg(feature = "layout-elk")]
    pub start_arrow_type: Option<&'static str>,
    #[cfg(feature = "layout-elk")]
    pub end_arrow_type: Option<&'static str>,
}

#[cfg(any(
    feature = "layout-dagre",
    feature = "layout-elk",
    feature = "layout-grid"
))]
fn round_number(num: f64, precision: i32) -> f64 {
    if !num.is_finite() {
        return 0.0;
    }
    let factor = 10_f64.powi(precision);
    (num * factor).round() / factor
}

#[cfg(any(
    feature = "layout-dagre",
    feature = "layout-elk",
    feature = "layout-grid"
))]
fn distance(a: &LayoutPoint, b: Option<&LayoutPoint>) -> f64 {
    let Some(b) = b else {
        return 0.0;
    };
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    (dx * dx + dy * dy).sqrt()
}

#[cfg(any(
    feature = "layout-dagre",
    feature = "layout-elk",
    feature = "layout-grid"
))]
pub fn calculate_point(points: &[LayoutPoint], distance_to_traverse: f64) -> Option<LayoutPoint> {
    if points.is_empty() {
        return None;
    }
    let mut prev: Option<&LayoutPoint> = None;
    let mut remaining = distance_to_traverse.max(0.0);
    for p in points {
        if let Some(prev_p) = prev {
            let vector_distance = distance(p, Some(prev_p));
            if vector_distance == 0.0 {
                return Some(prev_p.clone());
            }
            if vector_distance < remaining {
                remaining -= vector_distance;
            } else {
                let ratio = remaining / vector_distance;
                if ratio <= 0.0 {
                    return Some(prev_p.clone());
                }
                if ratio >= 1.0 {
                    return Some(p.clone());
                }
                return Some(LayoutPoint {
                    x: round_number((1.0 - ratio) * prev_p.x + ratio * p.x, 5),
                    y: round_number((1.0 - ratio) * prev_p.y + ratio * p.y, 5),
                });
            }
        }
        prev = Some(p);
    }
    None
}

#[cfg(any(
    feature = "layout-dagre",
    feature = "layout-elk",
    feature = "layout-grid"
))]
#[derive(Debug, Clone, Copy)]
pub enum TerminalPos {
    StartLeft,
    StartRight,
    EndLeft,
    EndRight,
}

#[cfg(any(
    feature = "layout-dagre",
    feature = "layout-elk",
    feature = "layout-grid"
))]
fn calc_terminal_label_position(
    terminal_marker_size: f64,
    position: TerminalPos,
    points: &[LayoutPoint],
) -> Option<(f64, f64)> {
    if points.len() < 2 {
        return None;
    }

    let mut pts = points.to_vec();
    match position {
        TerminalPos::StartLeft | TerminalPos::StartRight => {}
        TerminalPos::EndLeft | TerminalPos::EndRight => pts.reverse(),
    }

    let distance_to_cardinality_point = 25.0 + terminal_marker_size;
    let center = calculate_point(&pts, distance_to_cardinality_point)?;
    let d = 10.0 + terminal_marker_size * 0.5;
    let angle = (pts[0].y - center.y).atan2(pts[0].x - center.x);

    let (x, y) = match position {
        TerminalPos::StartLeft => {
            let a = angle + std::f64::consts::PI;
            (
                a.sin() * d + (pts[0].x + center.x) / 2.0,
                -a.cos() * d + (pts[0].y + center.y) / 2.0,
            )
        }
        TerminalPos::StartRight => (
            angle.sin() * d + (pts[0].x + center.x) / 2.0,
            -angle.cos() * d + (pts[0].y + center.y) / 2.0,
        ),
        TerminalPos::EndLeft => (
            angle.sin() * d + (pts[0].x + center.x) / 2.0,
            -angle.cos() * d + (pts[0].y + center.y) / 2.0,
        ),
        TerminalPos::EndRight => {
            let a = angle - std::f64::consts::PI;
            (
                a.sin() * d + (pts[0].x + center.x) / 2.0,
                -a.cos() * d + (pts[0].y + center.y) / 2.0,
            )
        }
    };
    Some((x, y))
}

fn write_class_relation_dom_id(out: &mut String, from: &str, to: &str, ordinal: usize) {
    use std::fmt::Write as _;
    let _ = write!(out, "id_{from}_{to}_{ordinal}");
}

pub(crate) fn class_arrow_type_for_relation_end(ty: i32) -> Option<&'static str> {
    match ty {
        0 => Some("aggregation"),
        1 => Some("extension"),
        2 => Some("composition"),
        3 => Some("dependency"),
        4 => Some("lollipop"),
        _ => None,
    }
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
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

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
pub fn terminal_path_for_edge(
    points: &[LayoutPoint],
    from_rect: Rect,
    to_rect: Rect,
) -> Vec<LayoutPoint> {
    if points.len() < 2 {
        return points.to_vec();
    }
    let mut out = points.to_vec();

    if let Some(p) = intersect_segment_with_rect(&out[0], &out[1], from_rect) {
        out[0] = p;
    }
    let last = out.len() - 1;
    if let Some(p) = intersect_segment_with_rect(&out[last], &out[last - 1], to_rect) {
        out[last] = p;
    }

    out
}

struct ClassBoxMeasureCtx<'a> {
    measurer: &'a dyn TextMeasurer,
    mermaid_config: &'a merman_model::presentation::PresentationConfig,
    math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
    text_style: &'a TextStyle,
    code_text_style: &'a TextStyle,
    native_title_style: Option<&'a TextStyle>,
    html_calc_text_style: &'a TextStyle,
    wrap_probe_font_size: f64,
    wrap_mode: WrapMode,
    padding: f64,
    hide_empty_members_box: bool,
    capture_row_metrics: bool,
}

pub(crate) fn class_math_label_metrics(
    text: &str,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    max_width_px: Option<f64>,
    mermaid_config: &merman_model::presentation::PresentationConfig,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
) -> Option<merman_render_core::text::TextMetrics> {
    if !merman_render_core::math::contains_delimited_math(text) {
        return None;
    }
    let math_renderer = math_renderer?;
    merman_render_core::math::math_label_metrics_for_layout(
        merman_render_core::math::MathLabelMetricsRequest {
            measurer,
            raw_label: text,
            style,
            max_width_px,
            wrap_mode: WrapMode::HtmlLike,
            config: mermaid_config,
            math_renderer: Some(math_renderer),
        },
    )
}

pub(crate) fn class_svg_empty_markdown_fallback(text: &str) -> Option<String> {
    if text.is_empty()
        || analyze_class_svg_markdown(text)
            .lines
            .iter()
            .flatten()
            .any(|(word, _)| !word.is_empty())
    {
        return None;
    }
    // Pinned shapeUtil.addText restores the first SVG row when createText produced no text.
    let first = text.chars().next()?;
    let remaining = text[first.len_utf8()..]
        .replace("&gt;", ">")
        .replace("&lt;", "<");
    Some(format!("{first}{}", remaining.trim()))
}

fn mermaid_class_svg_create_text_width_px(
    measurer: &dyn TextMeasurer,
    text: &str,
    style: &TextStyle,
    wrap_probe_font_size: f64,
) -> Option<f64> {
    let wrap_probe_style = TextStyle {
        font_family: style
            .font_family
            .clone()
            .or_else(|| Some("Arial".to_string())),
        font_size: wrap_probe_font_size.max(1.0),
        font_weight: None,
        font_style: None,
    };
    let width = class_html_create_text_width_px(text, measurer, &wrap_probe_style);
    (width > 0).then_some(width as f64)
}

fn class_box_dimensions(
    node: &ClassNode,
    ctx: &ClassBoxMeasureCtx<'_>,
) -> (
    f64,
    f64,
    Option<ClassNodeLabelPlan>,
    Option<ClassPreparedSvgTitle>,
) {
    let measurer = ctx.measurer;
    let mermaid_config = ctx.mermaid_config;
    let math_renderer = ctx.math_renderer;
    let text_style = ctx.text_style;
    let html_calc_text_style = ctx.html_calc_text_style;
    let wrap_probe_font_size = ctx.wrap_probe_font_size;
    let wrap_mode = ctx.wrap_mode;
    let padding = ctx.padding;
    let hide_empty_members_box = ctx.hide_empty_members_box;
    let capture_row_metrics = ctx.capture_row_metrics;

    // Mermaid class nodes are sized by rendering the label groups (`textHelper(...)`) and taking
    // the resulting SVG bbox (`getBBox()`), then expanding by class padding (see upstream:
    // `rendering-elements/shapes/classBox.ts` + `diagrams/class/shapeUtil.ts`).
    //
    // Emulate that sizing logic deterministically using the same text measurer.
    let node_requires_math = class_node_requires_math(node);
    let use_html_labels = matches!(wrap_mode, WrapMode::HtmlLike) || node_requires_math;
    let svg_font_family = if !use_html_labels && ctx.native_title_style.is_some() {
        class_native_svg_font_family(node)
    } else {
        None
    };
    let svg_font_family = svg_font_family.as_deref();
    let prepare_html_labels = use_html_labels && !node_requires_math;
    let node_label_style = if use_html_labels {
        node_binding::class_apply_inline_styles(node).style_attr
    } else {
        String::new()
    };
    let padding = padding.max(0.0);
    let gap = padding;
    let text_padding = if use_html_labels { 0.0 } else { 3.0 };

    fn wrap_class_svg_text_like_mermaid(
        text: &str,
        probe_text: &str,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
        probe_style: &TextStyle,
        wrap_probe_font_size: f64,
        bold: bool,
    ) -> String {
        let Some(wrap_width_px) = mermaid_class_svg_create_text_width_px(
            measurer,
            probe_text,
            probe_style,
            wrap_probe_font_size,
        ) else {
            return text.to_string();
        };
        let mut lines: Vec<String> = Vec::new();
        for line in merman_render_core::text::DeterministicTextMeasurer::normalized_text_lines(text)
        {
            let mut tokens = std::collections::VecDeque::from(
                merman_render_core::text::DeterministicTextMeasurer::split_line_to_words(&line),
            );
            let mut cur = String::new();

            while let Some(tok) = tokens.pop_front() {
                if cur.is_empty() && tok == " " {
                    continue;
                }

                let candidate = format!("{cur}{tok}");
                let candidate_w = if bold {
                    let bold_style = TextStyle {
                        font_family: style.font_family.clone(),
                        font_size: style.font_size,
                        font_weight: Some("bolder".to_string()),
                        font_style: None,
                    };
                    measurer.measure_svg_text_computed_length_px(candidate.trim_end(), &bold_style)
                } else {
                    measurer.measure_svg_text_computed_length_px(candidate.trim_end(), style)
                };
                if candidate_w <= wrap_width_px {
                    cur = candidate;
                    continue;
                }

                if !cur.trim().is_empty() {
                    lines.push(cur.trim_end().to_string());
                    cur.clear();
                    tokens.push_front(tok);
                    continue;
                }

                if tok == " " {
                    continue;
                }

                // Token itself does not fit on an empty line; split by characters.
                let chars = tok.chars().collect::<Vec<_>>();
                let mut cut = 1usize;
                while cut < chars.len() {
                    let head: String = chars[..cut].iter().collect();
                    let head_w = if bold {
                        let bold_style = TextStyle {
                            font_family: style.font_family.clone(),
                            font_size: style.font_size,
                            font_weight: Some("bolder".to_string()),
                            font_style: None,
                        };
                        measurer.measure_svg_text_computed_length_px(head.as_str(), &bold_style)
                    } else {
                        measurer.measure_svg_text_computed_length_px(head.as_str(), style)
                    };
                    if head_w > wrap_width_px {
                        break;
                    }
                    cut += 1;
                }
                cut = cut.saturating_sub(1).max(1);
                let head: String = chars[..cut].iter().collect();
                let tail: String = chars[cut..].iter().collect();
                lines.push(head);
                if !tail.is_empty() {
                    tokens.push_front(tail);
                }
            }

            if !cur.trim().is_empty() {
                lines.push(cur.trim_end().to_string());
            }
        }

        if lines.len() <= 1 {
            text.to_string()
        } else {
            lines.join("\n")
        }
    }

    let measure_label = |text: &str,
                         css_style: &str,
                         base_style: &TextStyle|
     -> (
        merman_render_core::text::TextMetrics,
        Option<ClassPreparedHtmlLabel>,
    ) {
        let effective_style = if use_html_labels {
            class_html_node_text_style(base_style, css_style, &node_label_style)
        } else {
            class_svg_row_text_style(
                base_style,
                css_style,
                class_svg_empty_markdown_fallback(text).is_some(),
            )
        };
        let effective_style = class_svg_text_with_family(&effective_style, svg_font_family);
        let style = effective_style.as_ref();
        let calc_style = if ctx.native_title_style.is_some() {
            style
        } else {
            html_calc_text_style
        };
        let max_width_px = class_html_create_text_width_px(text, measurer, calc_style);
        if let Some(metrics) = class_math_label_metrics(
            text,
            measurer,
            style,
            Some(max_width_px.max(1) as f64),
            mermaid_config,
            math_renderer,
        ) {
            return (metrics, None);
        }
        if use_html_labels {
            let prepared = class_prepare_html_label(measurer, style, text, max_width_px, "");
            (prepared.metrics, prepare_html_labels.then_some(prepared))
        } else if let Some(fallback) = class_svg_empty_markdown_fallback(text) {
            (
                measurer.measure_wrapped(&fallback, style, None, wrap_mode),
                None,
            )
        } else if analyze_class_svg_markdown(text).has_styled_runs {
            (
                merman_render_core::text::measure_markdown_with_inline_styles(
                    measurer, text, style, None, wrap_mode,
                ),
                None,
            )
        } else {
            let wrapped = if matches!(wrap_mode, WrapMode::SvgLike | WrapMode::SvgLikeSingleRun) {
                wrap_class_svg_text_like_mermaid(
                    text,
                    text,
                    measurer,
                    style,
                    calc_style,
                    wrap_probe_font_size,
                    false,
                )
            } else {
                text.to_string()
            };
            let metrics = if matches!(wrap_mode, WrapMode::SvgLike | WrapMode::SvgLikeSingleRun) {
                // Keep layout sizing aligned with the SVG renderer, which emits labels through
                // Mermaid's Markdown-aware `createText(...)` path even for plain class text.
                merman_render_core::text::measure_markdown_with_inline_styles(
                    measurer, &wrapped, style, None, wrap_mode,
                )
            } else {
                measurer.measure_wrapped(&wrapped, style, None, wrap_mode)
            };
            (metrics, None)
        }
    };

    fn label_rect(m: merman_render_core::text::TextMetrics, y_offset: f64) -> Option<Rect> {
        if !(m.width.is_finite() && m.height.is_finite()) {
            return None;
        }
        let w = m.width.max(0.0);
        let h = m.height.max(0.0);
        if w <= 0.0 || h <= 0.0 {
            return None;
        }
        let lines = m.line_count.max(1) as f64;
        let y = y_offset - (h / (2.0 * lines));
        Some(Rect::from_min_max(0.0, y, w, y + h))
    }

    // Annotation group: Mermaid only renders the first annotation.
    let mut annotation_rect: Option<Rect> = None;
    let mut annotation_group_height = 0.0;
    let mut annotation_prepared = None;
    if let Some(t) = node.annotation_text_for_render() {
        let (m, prepared) = measure_label(&t, "", text_style);
        annotation_prepared = prepared;
        annotation_rect = label_rect(m, 0.0);
        if let Some(r) = annotation_rect {
            annotation_group_height = r.height().max(0.0);
        }
    }

    // shapeUtil sanitizes the createText input but retains the source for its width probe
    // and conditional literal fallback. Keep entities encoded until textContent emission.
    let title_source = if use_html_labels {
        node.text.trim().to_string()
    } else {
        let source = node.text.trim();
        source.strip_prefix('\\').unwrap_or(source).to_string()
    };
    let title_text = if use_html_labels {
        title_source.clone()
    } else {
        merman_markup::sanitize::sanitize_text(&title_source, mermaid_config.as_config())
    };
    // Both pinned shapeUtil implementations apply bolder after createText wrapping.
    let title_markdown_analysis =
        (!use_html_labels).then(|| analyze_class_svg_markdown(&title_text));
    let title_style = class_svg_text_with_family(
        ctx.native_title_style.unwrap_or(text_style),
        svg_font_family,
    );
    let title_style = title_style.as_ref();
    let title_calc_style = if ctx.native_title_style.is_some() {
        title_style
    } else {
        html_calc_text_style
    };
    let svg_title = (!use_html_labels).then(|| {
        if node.type_param.contains('<') && title_source.ends_with("&gt;") {
            let label = ClassSvgLabel::from_type_source(
                &title_text,
                measurer,
                title_style,
                title_calc_style,
                wrap_probe_font_size,
                wrap_mode,
            );
            return ClassSvgTitleLabel {
                label,
                literal_first_row: false,
            };
        }
        let wrapped = if matches!(wrap_mode, WrapMode::SvgLike | WrapMode::SvgLikeSingleRun)
            && title_markdown_analysis
                .as_ref()
                .is_some_and(MermaidMarkdownAnalysis::all_runs_normal)
        {
            wrap_class_svg_text_like_mermaid(
                &title_text,
                &title_source,
                measurer,
                title_style,
                title_calc_style,
                wrap_probe_font_size,
                false,
            )
        } else {
            title_text.clone()
        };
        ClassSvgTitleLabel::new(&title_source, &wrapped)
    });
    let title_max_width_px = use_html_labels.then(|| {
        class_html_create_text_width_px(title_text.as_str(), measurer, title_calc_style).max(1)
    });
    let title_max_width = title_max_width_px.map(|width| width as f64);

    let bold_title_style = if let Some(native) = ctx.native_title_style {
        if use_html_labels {
            class_html_node_text_style(native, "", &node_label_style).into_owned()
        } else {
            native.clone()
        }
    } else if use_html_labels {
        class_html_node_text_style(text_style, "font-weight:bolder", &node_label_style).into_owned()
    } else {
        TextStyle {
            font_family: text_style.font_family.clone(),
            font_size: text_style.font_size,
            font_weight: Some("bolder".to_string()),
            font_style: text_style.font_style.clone(),
        }
    };
    let math_title_metrics =
        merman_render_core::math::contains_delimited_math(&title_text).then(|| {
            let max_width = title_max_width.unwrap_or_else(|| {
                class_html_create_text_width_px(title_text.as_str(), measurer, title_calc_style)
                    .max(1) as f64
            });
            class_math_label_metrics(
                &title_text,
                measurer,
                &bold_title_style,
                Some(max_width),
                mermaid_config,
                math_renderer,
            )
        });
    let math_title_metrics = math_title_metrics.flatten();
    let has_math_title_metrics = math_title_metrics.is_some();
    let title_prepared = (use_html_labels && !has_math_title_metrics).then(|| {
        class_prepare_html_label(
            measurer,
            &bold_title_style,
            &title_text,
            title_max_width_px.unwrap_or(1),
            "",
        )
    });
    let title_metrics = math_title_metrics.unwrap_or_else(|| {
        if let Some(prepared) = &title_prepared {
            prepared.metrics
        } else {
            svg_title
                .as_ref()
                .expect("prepared SVG title")
                .metrics(measurer, title_style)
        }
    });
    let title_rect = label_rect(title_metrics, 0.0);
    let title_group_height = title_rect.map(|r| r.height()).unwrap_or(0.0);
    let capture_fallback_row_metrics = capture_row_metrics && !prepare_html_labels;
    let measure_rows = |rows: &[merman_model::families::class::ClassMember]| {
        let mut rows_rect: Option<Rect> = None;
        let mut metrics_out: Option<Vec<merman_render_core::text::TextMetrics>> =
            capture_fallback_row_metrics.then(|| Vec::with_capacity(rows.len()));
        let mut prepared_out = prepare_html_labels.then(|| Vec::with_capacity(rows.len()));
        let mut y_offset = 0.0;
        for row in rows {
            let (metrics, prepared) = if use_html_labels {
                measure_label(
                    &class_member_create_text_input(row),
                    row.css_style.as_str(),
                    ctx.code_text_style,
                )
            } else if let Some(source) =
                node_binding::class_generic_member_source(row, ["&lt;", "&gt;"])
            {
                let style = class_svg_row_text_style(ctx.code_text_style, &row.css_style, false);
                let style = class_svg_text_with_family(&style, svg_font_family);
                let probe_style = if ctx.native_title_style.is_some() {
                    style.as_ref()
                } else {
                    html_calc_text_style
                };
                let label = ClassSvgLabel::from_type_source(
                    &source,
                    measurer,
                    &style,
                    probe_style,
                    wrap_probe_font_size,
                    wrap_mode,
                );
                (label.type_metrics(measurer, &style), None)
            } else {
                measure_label(
                    &row.display_text_for_render(),
                    row.css_style.as_str(),
                    ctx.code_text_style,
                )
            };
            if let Some(out) = metrics_out.as_mut() {
                out.push(metrics);
            }
            if let (Some(out), Some(prepared)) = (prepared_out.as_mut(), prepared) {
                out.push(prepared);
            }
            if let Some(r) = label_rect(metrics, y_offset) {
                if let Some(ref mut cur) = rows_rect {
                    cur.union(r);
                } else {
                    rows_rect = Some(r);
                }
            }
            y_offset += metrics.height.max(0.0) + text_padding;
        }

        (rows_rect, metrics_out, prepared_out)
    };

    // Members group.
    let (members_rect, members_metrics_out, members_prepared_out) = measure_rows(&node.members);
    let mut members_group_height = members_rect.map(|r| r.height()).unwrap_or(0.0);
    if members_group_height <= 0.0 {
        // Mermaid reserves half a gap when the members group is empty.
        members_group_height = (gap / 2.0).max(0.0);
    }

    // Methods group.
    let (methods_rect, methods_metrics_out, methods_prepared_out) = measure_rows(&node.methods);

    // Combine into the bbox returned by `textHelper(...)`.
    let mut bbox_opt: Option<Rect> = None;

    // annotation-group: centered horizontally (`translate(-w/2, 0)`).
    if let Some(mut r) = annotation_rect {
        let w = r.width();
        r.translate(-w / 2.0, 0.0);
        bbox_opt = Some(if let Some(mut cur) = bbox_opt {
            cur.union(r);
            cur
        } else {
            r
        });
    }

    // label-group: centered and shifted down by annotation height.
    if let Some(mut r) = title_rect {
        let w = r.width();
        r.translate(-w / 2.0, annotation_group_height);
        bbox_opt = Some(if let Some(mut cur) = bbox_opt {
            cur.union(r);
            cur
        } else {
            r
        });
    }

    // members-group: left-aligned, shifted down by label height + gap*2.
    if let Some(mut r) = members_rect {
        let dy = annotation_group_height + title_group_height + gap * 2.0;
        r.translate(0.0, dy);
        bbox_opt = Some(if let Some(mut cur) = bbox_opt {
            cur.union(r);
            cur
        } else {
            r
        });
    }

    // methods-group: left-aligned, shifted down by label height + members height + gap*4.
    if let Some(mut r) = methods_rect {
        let dy = annotation_group_height + title_group_height + (members_group_height + gap * 4.0);
        r.translate(0.0, dy);
        bbox_opt = Some(if let Some(mut cur) = bbox_opt {
            cur.union(r);
            cur
        } else {
            r
        });
    }

    let bbox = bbox_opt.unwrap_or_else(|| Rect::from_min_max(0.0, 0.0, 0.0, 0.0));
    let w = bbox.width().max(0.0);
    let mut h = bbox.height().max(0.0);

    // Mermaid adjusts bbox height depending on which compartments exist.
    if node.members.is_empty() && node.methods.is_empty() {
        h += gap;
    } else if !node.members.is_empty() && node.methods.is_empty() {
        h += gap * 2.0;
    }

    let render_extra_box =
        node.members.is_empty() && node.methods.is_empty() && !hide_empty_members_box;

    // The Dagre node bounds come from the rectangle passed to `updateNodeBounds`.
    let rect_w = w + 2.0 * padding;
    let mut rect_h = h + 2.0 * padding;
    if render_extra_box {
        rect_h += padding * 2.0;
    } else if node.members.is_empty() && node.methods.is_empty() {
        rect_h -= padding;
    }

    let label_plan = if prepare_html_labels {
        Some(ClassNodeLabelPlan::PreparedHtml(
            ClassPreparedHtmlNodeLabels {
                title: title_prepared.expect("prepared Class title XHTML"),
                annotation: annotation_prepared,
                members: members_prepared_out.unwrap_or_default(),
                methods: methods_prepared_out.unwrap_or_default(),
                _retained_reservation: None,
            },
        ))
    } else if capture_row_metrics {
        Some(ClassNodeLabelPlan::RowMetrics(ClassNodeRowMetrics {
            members: members_metrics_out.unwrap_or_default(),
            methods: methods_metrics_out.unwrap_or_default(),
            _retained_reservation: None,
        }))
    } else {
        None
    };

    let svg_title = svg_title.map(|label| ClassPreparedSvgTitle {
        label,
        metrics: title_metrics,
        _retained_reservation: None,
    });
    (rect_w.max(1.0), rect_h.max(1.0), label_plan, svg_title)
}

pub(crate) fn class_calculate_text_width_like_mermaid_px(
    text: &str,
    measurer: &dyn TextMeasurer,
    calc_text_style: &TextStyle,
) -> i64 {
    measure_mermaid_text_dimensions(measurer, text, calc_text_style).width
}

pub(crate) fn class_html_create_text_width_px(
    text: &str,
    measurer: &dyn TextMeasurer,
    calc_text_style: &TextStyle,
) -> i64 {
    class_calculate_text_width_like_mermaid_px(text, measurer, calc_text_style) + 50
}

pub(crate) fn class_effective_text_style<'a>(
    base: &'a TextStyle,
    css_style: &str,
) -> std::borrow::Cow<'a, TextStyle> {
    let mut cascade = merman_render_core::mermaid_style::TextStyleCascade::new(
        base,
        merman_render_core::mermaid_style::CssFontSizeContext::uniform(base.font_size),
    );
    merman_render_core::mermaid_style::visit_parsed_style_declarations(css_style, |declaration| {
        cascade.observe_declaration(&declaration);
    });
    cascade.finish()
}

pub(crate) fn class_native_svg_font_family(node: &ClassNode) -> Option<String> {
    use merman_render_core::mermaid_style::{
        CssFontSizeContext, TextStyleCascade, parse_style_declaration,
    };

    let inherited = TextStyle::default();
    let mut cascade =
        TextStyleCascade::new(&inherited, CssFontSizeContext::uniform(inherited.font_size));
    for declaration in node
        .styles
        .iter()
        .filter_map(|raw| parse_style_declaration(raw))
    {
        if declaration.property() == "font-family" {
            cascade.observe_declaration(&declaration);
        }
    }
    match cascade.finish() {
        std::borrow::Cow::Owned(style) => style.font_family,
        std::borrow::Cow::Borrowed(_) => None,
    }
}

pub(crate) fn class_svg_text_with_family<'a>(
    base: &'a TextStyle,
    family: Option<&str>,
) -> std::borrow::Cow<'a, TextStyle> {
    match family {
        Some(family) => std::borrow::Cow::Owned(TextStyle {
            font_family: Some(family.to_owned()),
            ..base.clone()
        }),
        None => std::borrow::Cow::Borrowed(base),
    }
}

pub(crate) fn class_svg_row_text_style<'a>(
    base: &'a TextStyle,
    css_style: &str,
    literal_fallback: bool,
) -> std::borrow::Cow<'a, TextStyle> {
    let mut style = class_effective_text_style(base, css_style);
    // Pinned createText writes normal on each inner tspan. The empty-Markdown
    // fallback resets style too, but its empty weight attribute inherits the row.
    if style
        .font_style
        .as_deref()
        .is_some_and(|value| value != "normal")
    {
        style.to_mut().font_style = Some("normal".to_owned());
    }
    if !literal_fallback
        && style
            .font_weight
            .as_deref()
            .is_some_and(|value| value != "normal")
    {
        style.to_mut().font_weight = Some("normal".to_owned());
    }
    style
}

pub(crate) fn class_html_node_text_style<'a>(
    base: &'a TextStyle,
    group_style: &str,
    span_style: &str,
) -> std::borrow::Cow<'a, TextStyle> {
    // The nodeLabel span is a child of the row/title group, so its declarations win
    // over inherited group values even when the group's declaration is important.
    let group = class_effective_text_style(base, group_style);
    if span_style.is_empty() {
        group
    } else {
        std::borrow::Cow::Owned(class_effective_text_style(&group, span_style).into_owned())
    }
}

pub(crate) fn class_html_measure_label_metrics(
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    text: &str,
    max_width_px: i64,
    css_style: &str,
) -> merman_render_core::text::TextMetrics {
    class_prepare_html_label(measurer, style, text, max_width_px, css_style).metrics
}

pub(crate) fn class_prepare_html_label(
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    text: &str,
    max_width_px: i64,
    css_style: &str,
) -> ClassPreparedHtmlLabel {
    let max_width = Some(max_width_px.max(1) as f64);
    let effective_style = class_effective_text_style(style, css_style);
    let style = effective_style.as_ref();
    let xhtml = merman_render_core::text::mermaid_markdown_to_xhtml_label_fragment(text, true);
    // HTML rem is relative to the host document, independently of node typography.
    let mut metrics = merman_render_core::text::measure_xhtml_label_fragment_with_terminal_fonts(
        measurer, &xhtml, style, max_width, 16.0,
    );

    let rendered_width = metrics.width;
    if metrics.line_count == 1
        && rendered_width > 0.0
        && rendered_width < max_width_px.max(1) as f64 - 0.01
    {
        metrics.height =
            metrics
                .height
                .max(merman_render_core::text::flowchart_html_line_height_px(
                    style.font_size,
                ));
        metrics.line_count = 1;
    }

    ClassPreparedHtmlLabel {
        metrics,
        max_width_px,
        xhtml,
    }
}

pub(crate) fn class_normalize_xhtml_br_tags(html: &str) -> String {
    html.replace("<br>", "<br />")
        .replace("<br/>", "<br />")
        .replace("<br >", "<br />")
        .replace("</br>", "<br />")
        .replace("</br/>", "<br />")
        .replace("</br />", "<br />")
        .replace("</br >", "<br />")
}

pub(crate) fn class_note_html_fragment(
    note_src: &str,
    mermaid_config: &merman_model::presentation::PresentationConfig,
) -> String {
    let note_html = note_src.replace("\r\n", "\n").replace('\n', "<br />");
    let note_html = merman_markup::sanitize::sanitize_xhtml(&note_html, mermaid_config.as_config());
    class_normalize_xhtml_br_tags(&note_html)
}

fn measure_unwrapped_xhtml_label_fragment(
    measurer: &dyn TextMeasurer,
    fragment: &str,
    style: &TextStyle,
    root_font_size: f64,
) -> merman_render_core::text::TextMetrics {
    merman_render_core::text::measure_xhtml_label_fragment_with_terminal_fonts(
        measurer,
        fragment,
        style,
        None,
        root_font_size,
    )
}

pub(crate) fn class_html_measure_note_metrics(
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    note_src: &str,
    mermaid_config: &merman_model::presentation::PresentationConfig,
) -> merman_render_core::text::TextMetrics {
    let html = class_note_html_fragment(note_src, mermaid_config);
    // Like the other Class XHTML labels, rem resolves against the host document,
    // independently of the diagram's inherited font size.
    measure_unwrapped_xhtml_label_fragment(measurer, &html, style, 16.0)
}

pub(crate) fn analyze_class_svg_markdown(text: &str) -> MermaidMarkdownAnalysis {
    analyze_mermaid_markdown(text, true)
}

/// Prepared createText words shared by Class layout and SVG emission.
#[derive(Debug)]
pub(crate) struct ClassSvgLabel {
    analysis: MermaidMarkdownAnalysis,
}

// Notes retain their existing family boundary and decoded-model provenance.
pub(crate) type ClassSvgNoteLabel = ClassSvgLabel;

#[derive(Debug)]
pub(crate) struct ClassSvgTitleLabel {
    label: ClassSvgLabel,
    literal_first_row: bool,
}

#[derive(Debug)]
pub(crate) struct ClassPreparedSvgTitle {
    pub(crate) label: ClassSvgTitleLabel,
    pub(crate) metrics: merman_render_core::text::TextMetrics,
    _retained_reservation: Option<merman_render_core::resources::PreparedTextRetainedReservation>,
}

/// SVG-only preparations belong to the family operation, not its public layout projection.
#[derive(Debug, Default)]
pub struct ClassPreparedLabels {
    pub(crate) titles_by_id: FxHashMap<String, ClassPreparedSvgTitle>,
    pub(crate) edges_by_id: FxHashMap<String, ClassPreparedSvgEdgeLabel>,
    #[cfg(feature = "layout-grid")]
    grid_by_id: FxHashMap<String, grid::ClassGridLabel>,
}

#[derive(Debug)]
pub(crate) struct ClassPreparedSvgEdgeLabel {
    pub(crate) label: ClassSvgLabel,
    pub(crate) text_bounds: Bounds,
    _retained_reservation: Option<merman_render_core::resources::PreparedTextRetainedReservation>,
}

impl ClassNodeLabelPlan {
    fn reserve_retained_bytes(
        &mut self,
        id_capacity: usize,
        work_control: &OperationLayoutWorkControl,
    ) -> Result<()> {
        let (heap_bytes, reservation) = match self {
            Self::RowMetrics(rows) => (
                rows.members
                    .capacity()
                    .saturating_add(rows.methods.capacity())
                    .saturating_mul(std::mem::size_of::<merman_render_core::text::TextMetrics>()),
                &mut rows._retained_reservation,
            ),
            Self::PreparedHtml(labels) => {
                let row_bytes = labels
                    .members
                    .capacity()
                    .saturating_add(labels.methods.capacity())
                    .saturating_mul(std::mem::size_of::<ClassPreparedHtmlLabel>());
                let heap_bytes = std::iter::once(&labels.title)
                    .chain(labels.annotation.iter())
                    .chain(labels.members.iter())
                    .chain(labels.methods.iter())
                    .fold(row_bytes, |bytes, label| {
                        bytes.saturating_add(label.xhtml.capacity())
                    });
                (heap_bytes, &mut labels._retained_reservation)
            }
        };
        // Include the map entry/control allowance, key, and Arc allocation with both counters.
        // The lease lives inside that allocation, so every layout/plan Arc shares its lifetime.
        let bytes = std::mem::size_of::<(String, Arc<Self>)>()
            .saturating_add(std::mem::size_of::<usize>())
            .saturating_add(id_capacity)
            .saturating_add(std::mem::size_of::<Self>())
            .saturating_add(2 * std::mem::size_of::<usize>())
            .saturating_add(heap_bytes);
        *reservation = Some(work_control.reserve_prepared_text_retained_bytes(bytes)?);
        Ok(())
    }
}

impl ClassPreparedSvgTitle {
    fn reserve_retained_bytes(
        &mut self,
        id: &str,
        work_control: &OperationLayoutWorkControl,
    ) -> Result<()> {
        let bytes = std::mem::size_of::<(String, Self)>()
            .saturating_add(std::mem::size_of::<usize>())
            .saturating_add(id.len())
            .saturating_add(self.label.label.retained_heap_bytes());
        self._retained_reservation =
            Some(work_control.reserve_prepared_text_retained_bytes(bytes)?);
        Ok(())
    }
}

impl ClassSvgTitleLabel {
    pub(crate) fn new(source: &str, wrapped: &str) -> Self {
        let mut label = ClassSvgLabel::new(wrapped);
        for (word, _) in label.analysis.lines.iter_mut().flatten() {
            *word =
                merman_render_core::entities::decode_svg_text_content_entities(word).into_owned();
        }
        let content = label
            .lines()
            .iter()
            .map(|words| {
                words
                    .iter()
                    .map(|(word, _)| word.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect::<String>();
        // shapeUtil.addText replaces only the first outer tspan after createText.
        let literal_first_row = content.is_empty() || content.contains("&gt");
        if literal_first_row {
            let first = source.chars().next();
            let mut fallback = first.map_or_else(String::new, |first| {
                let tail = &source[first.len_utf8()..];
                let rest = tail.replace("&gt;", ">").replace("&lt;", "<");
                format!(
                    "{first}{}{rest}",
                    if tail.starts_with(' ') { " " } else { "" },
                    rest = rest.trim()
                )
            });
            if fallback == "undefined" {
                fallback.clear();
            }
            if label.analysis.lines.is_empty() {
                label.analysis.lines.push(Vec::new());
            }
            label.analysis.lines[0] = vec![(
                fallback,
                merman_render_core::text::inline_html::MermaidMarkdownWordType::Normal,
            )];
        }
        Self {
            label,
            literal_first_row,
        }
    }

    pub(crate) fn lines(
        &self,
    ) -> &[Vec<(
        String,
        merman_render_core::text::inline_html::MermaidMarkdownWordType,
    )>] {
        self.label.lines()
    }

    pub(crate) fn literal_first_row(&self) -> bool {
        self.literal_first_row
    }

    #[cfg(test)]
    fn plain_text(&self) -> String {
        self.label.plain_text()
    }

    pub(crate) fn metrics(
        &self,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
    ) -> merman_render_core::text::TextMetrics {
        let weight = style.font_weight.as_deref().unwrap_or("bolder");
        let role = ClassTitleMeasurer {
            inner: measurer,
            weight,
        };
        let mut metrics = merman_render_core::text::measure_svg_markdown_word_lines(
            &role,
            self.lines(),
            style,
            merman_render_core::text::inline_html::SvgTextEntityMode::DecodedModel,
        );
        if self.lines().iter().flatten().all(|(_, kind)| {
            *kind == merman_render_core::text::inline_html::MermaidMarkdownWordType::Normal
        }) {
            let mut role_style = style.clone();
            role_style.font_weight = Some(weight.to_owned());
            role_style.font_style = Some("normal".to_owned());
            metrics.width = self
                .lines()
                .iter()
                .map(|words| {
                    let row = words
                        .iter()
                        .map(|(word, _)| word.as_str())
                        .collect::<Vec<_>>()
                        .join(" ");
                    measurer.measure_svg_tspan_text_bbox_width_px(&row, &role_style)
                })
                .fold(0.0_f64, f64::max);
        }
        metrics
    }
}

/// addText clears every title tspan's weight, including strong and emphasis runs.
struct ClassTitleMeasurer<'a> {
    inner: &'a dyn TextMeasurer,
    weight: &'a str,
}

impl ClassTitleMeasurer<'_> {
    fn role_style(&self, style: &TextStyle) -> TextStyle {
        TextStyle {
            font_weight: Some(self.weight.to_owned()),
            ..style.clone()
        }
    }
}

impl TextMeasurer for ClassTitleMeasurer<'_> {
    fn cancellation_requested(&self) -> bool {
        self.inner.cancellation_requested()
    }

    fn measure(&self, text: &str, style: &TextStyle) -> merman_render_core::text::TextMetrics {
        self.inner.measure(text, &self.role_style(style))
    }

    fn measure_wrapped(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> merman_render_core::text::TextMetrics {
        self.inner
            .measure_wrapped(text, &self.role_style(style), max_width, wrap_mode)
    }

    fn measure_svg_text_computed_length_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.inner
            .measure_svg_text_computed_length_px(text, &self.role_style(style))
    }

    fn measure_svg_tspan_text_bbox_width_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.inner
            .measure_svg_tspan_text_bbox_width_px(text, &self.role_style(style))
    }

    fn measure_svg_tspan_text_bbox_height_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.inner
            .measure_svg_tspan_text_bbox_height_px(text, &self.role_style(style))
    }

    fn measure_svg_create_text_bbox_y_offset_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.inner
            .measure_svg_create_text_bbox_y_offset_px(text, &self.role_style(style))
    }
}

impl ClassSvgLabel {
    pub(crate) fn from_type_source(
        source: &str,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
        probe_style: &TextStyle,
        wrap_probe_font_size: f64,
        wrap_mode: WrapMode,
    ) -> Self {
        use merman_render_core::text::inline_html::SvgTextEntityMode;

        let source = source.strip_prefix('\\').unwrap_or(source);
        let mut label = Self::new(source);
        // Protect type delimiters while tokenizing Markdown, then wrap the literal words.
        // Splitting the source spelling could split an entity into different display rows.
        for (word, _) in label.analysis.lines.iter_mut().flatten() {
            if let std::borrow::Cow::Owned(decoded) =
                SvgTextEntityMode::CreateTextSource.visible_word(word)
            {
                *word = decoded;
            }
        }
        if label.analysis.all_runs_normal()
            && matches!(wrap_mode, WrapMode::SvgLike | WrapMode::SvgLikeSingleRun)
        {
            let width = mermaid_class_svg_create_text_width_px(
                measurer,
                source,
                probe_style,
                wrap_probe_font_size,
            );
            label.analysis.lines = merman_render_core::text::wrap_markdown_word_lines(
                measurer,
                label.lines(),
                style,
                width,
                wrap_mode,
                true,
                SvgTextEntityMode::DecodedModel,
            );
            label.analysis.line_count = label.analysis.lines.len();
        }
        label
    }

    pub(crate) fn type_metrics(
        &self,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
    ) -> merman_render_core::text::TextMetrics {
        merman_render_core::text::measure_svg_markdown_word_lines(
            measurer,
            self.lines(),
            style,
            merman_render_core::text::inline_html::SvgTextEntityMode::DecodedModel,
        )
    }

    fn retained_heap_bytes(&self) -> usize {
        self.analysis.lines.iter().fold(
            self.analysis
                .lines
                .capacity()
                .saturating_mul(std::mem::size_of::<
                    Vec<(
                        String,
                        merman_render_core::text::inline_html::MermaidMarkdownWordType,
                    )>,
                >()),
            |bytes, line| {
                line.iter().fold(
                    bytes.saturating_add(line.capacity().saturating_mul(std::mem::size_of::<(
                        String,
                        merman_render_core::text::inline_html::MermaidMarkdownWordType,
                    )>())),
                    |bytes, (word, _)| bytes.saturating_add(word.capacity()),
                )
            },
        )
    }

    pub(crate) fn plain_text(&self) -> String {
        self.lines()
            .iter()
            .map(|words| {
                words
                    .iter()
                    .map(|(word, _)| word.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn edge(decoded_text: &str, measurer: &dyn TextMeasurer, style: &TextStyle) -> Self {
        let text = decoded_text
            .strip_prefix('`')
            .and_then(|text| text.strip_suffix('`'))
            .unwrap_or(decoded_text);
        let mut label = Self::new(text);
        label.analysis.lines = merman_render_core::text::wrap_markdown_word_lines(
            measurer,
            label.lines(),
            style,
            Some(MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX),
            WrapMode::SvgLike,
            true,
            merman_render_core::text::inline_html::SvgTextEntityMode::DecodedModel,
        );
        label.analysis.line_count = label.analysis.lines.len();
        label
    }

    pub(crate) fn new(decoded_text: &str) -> Self {
        Self {
            analysis: analyze_class_svg_markdown(decoded_text),
        }
    }

    pub(crate) fn lines(
        &self,
    ) -> &[Vec<(
        String,
        merman_render_core::text::inline_html::MermaidMarkdownWordType,
    )>] {
        &self.analysis.lines
    }

    fn single_line_plain_width(
        &self,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
    ) -> Option<f64> {
        if self.analysis.line_count != 1 || !self.analysis.all_runs_normal() {
            return None;
        }
        let text = self.plain_text();
        if text.is_empty() {
            return None;
        }
        let style = merman_render_core::text::inline_html::svg_markdown_word_style(
            style,
            merman_render_core::text::inline_html::MermaidMarkdownWordType::Normal,
        );
        let (left, right) = measurer.measure_svg_text_bbox_x(&text, &style);
        let width = left + right;
        (width.is_finite() && width > 0.0).then_some(width)
    }

    pub(crate) fn metrics(
        &self,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
    ) -> merman_render_core::text::TextMetrics {
        let mut metrics = merman_render_core::text::measure_svg_markdown_word_lines(
            measurer,
            self.lines(),
            style,
            merman_render_core::text::inline_html::SvgTextEntityMode::DecodedModel,
        );
        if let Some(width) = self.single_line_plain_width(measurer, style) {
            metrics.width = width;
        }
        metrics
    }

    pub(crate) fn bbox_y_offset(&self, measurer: &dyn TextMeasurer, style: &TextStyle) -> f64 {
        self.lines()
            .first()
            .into_iter()
            .flatten()
            .map(|(word, kind)| {
                let style =
                    merman_render_core::text::inline_html::svg_markdown_word_style(style, *kind);
                measurer.measure_svg_create_text_bbox_y_offset_px(word, &style)
            })
            .reduce(f64::min)
            .unwrap_or(0.0)
    }

    fn centered_bounds(&self, measurer: &dyn TextMeasurer, style: &TextStyle) -> Bounds {
        let mut horizontal: Option<(f64, f64)> = None;
        let mut vertical: Option<(f64, f64)> = None;
        for (row_index, row) in self.lines().iter().enumerate() {
            if measurer.cancellation_requested() {
                break;
            }
            let mut advance = 0.0;
            let mut row_bounds: Option<(f64, f64)> = None;
            for (word_index, (word, kind)) in row.iter().enumerate() {
                if measurer.cancellation_requested() {
                    break;
                }
                let text = if word_index == 0 {
                    std::borrow::Cow::Borrowed(word.as_str())
                } else {
                    std::borrow::Cow::Owned(format!(" {word}"))
                };
                let face =
                    merman_render_core::text::inline_html::svg_markdown_word_style(style, *kind);
                let word_advance = measurer.measure_svg_text_computed_length_px(&text, &face);
                if !merman_render_core::text::trim_html_collapsible_ascii_whitespace(word)
                    .is_empty()
                {
                    let (left, right) = measurer.measure_svg_text_bbox_x(&text, &face);
                    let center = advance + word_advance / 2.0;
                    let min = center - left;
                    let max = center + right;
                    row_bounds =
                        Some(row_bounds.map_or((min, max), |(x0, x1)| (x0.min(min), x1.max(max))));
                    let top = measurer.measure_svg_create_text_bbox_y_offset_px(&text, &face)
                        + row_index as f64 * style.font_size * 1.1;
                    let height = measurer.measure_svg_tspan_text_bbox_height_px(&text, &face);
                    if height > 0.0 {
                        let bottom = top + height;
                        vertical = Some(
                            vertical
                                .map_or((top, bottom), |(y0, y1)| (y0.min(top), y1.max(bottom))),
                        );
                    }
                }
                advance += word_advance;
            }
            if measurer.cancellation_requested() {
                break;
            }
            if let Some((mut min, mut max)) = row_bounds {
                min -= advance / 2.0;
                max -= advance / 2.0;
                // A uniform row can use the host's complete centered-text bbox directly.
                if let Some((_, kind)) = row.first()
                    && row.iter().all(|(_, next)| next == kind)
                {
                    let text = row
                        .iter()
                        .map(|(word, _)| word.as_str())
                        .collect::<Vec<_>>()
                        .join(" ");
                    let face = merman_render_core::text::inline_html::svg_markdown_word_style(
                        style, *kind,
                    );
                    let (left, right) = measurer.measure_svg_text_bbox_x(&text, &face);
                    if left.is_finite() && right.is_finite() && left + right > 0.0 {
                        min = -left;
                        max = right;
                    }
                }
                horizontal =
                    Some(horizontal.map_or((min, max), |(x0, x1)| (x0.min(min), x1.max(max))));
            }
        }
        let (min_x, max_x) = horizontal.unwrap_or_default();
        let (min_y, max_y) = vertical.unwrap_or_default();
        Bounds {
            min_x,
            max_x,
            min_y,
            max_y,
        }
    }
}

fn note_dimensions(
    text: &str,
    measurer: &dyn TextMeasurer,
    text_style: &TextStyle,
    wrap_mode: WrapMode,
    padding: f64,
    mermaid_config: Option<&merman_model::presentation::PresentationConfig>,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
) -> (f64, f64, merman_render_core::text::TextMetrics) {
    let p = padding.max(0.0);
    let label = decode_entities_minimal(text);
    let math_metrics = mermaid_config.and_then(|config| {
        class_math_label_metrics(&label, measurer, text_style, None, config, math_renderer)
    });
    let m = if let Some(metrics) = math_metrics {
        metrics
    } else if matches!(wrap_mode, WrapMode::HtmlLike) {
        mermaid_config
            .map(|config| class_html_measure_note_metrics(measurer, text_style, text, config))
            .unwrap_or_else(|| measurer.measure_wrapped(&label, text_style, None, wrap_mode))
    } else {
        ClassSvgNoteLabel::new(&label).metrics(measurer, text_style)
    };
    (m.width + p * 2.0, m.height + p * 2.0, m)
}

fn label_metrics(
    text: &str,
    measurer: &dyn TextMeasurer,
    text_style: &TextStyle,
    wrap_mode: WrapMode,
    mermaid_config: &merman_model::presentation::PresentationConfig,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
) -> (f64, f64) {
    if text.trim().is_empty() {
        return (0.0, 0.0);
    }
    let t = decode_entities_minimal(text);
    let m = class_math_label_metrics(
        &t,
        measurer,
        text_style,
        None,
        mermaid_config,
        math_renderer,
    )
    .unwrap_or_else(|| measurer.measure_wrapped(&t, text_style, None, wrap_mode));
    (m.width.max(0.0), m.height.max(0.0))
}

pub(crate) fn class_plain_html_fragment(
    text: &str,
    mermaid_config: &merman_model::presentation::PresentationConfig,
) -> String {
    // Reuse createText's non-Markdown HTML projection. The sanitized model still contains
    // serialized entities; the XHTML parser owns their single decoding step.
    let html = merman_render_core::text::create_text_html::flowchart_label_html_source(
        text,
        "text",
        mermaid_config,
    );
    merman_markup::sanitize::sanitize_xhtml(&html, mermaid_config.as_config())
}

fn plain_label_metrics(
    text: &str,
    measurer: &dyn TextMeasurer,
    text_style: &TextStyle,
    wrap_mode: WrapMode,
    mermaid_config: &merman_model::presentation::PresentationConfig,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
) -> (f64, f64) {
    if !matches!(wrap_mode, WrapMode::HtmlLike) || text.trim().is_empty() {
        return label_metrics(text, measurer, text_style, wrap_mode, mermaid_config, None);
    }
    let decoded = decode_entities_minimal(text);
    let metrics = class_math_label_metrics(
        &decoded,
        measurer,
        text_style,
        None,
        mermaid_config,
        math_renderer,
    )
    .unwrap_or_else(|| {
        let xhtml = class_plain_html_fragment(text, mermaid_config);
        measure_unwrapped_xhtml_label_fragment(measurer, &xhtml, text_style, text_style.font_size)
    });
    (metrics.width.max(0.0), metrics.height.max(0.0))
}

fn prepare_edge_title(
    text: &str,
    measurer: &dyn TextMeasurer,
    text_style: &TextStyle,
    wrap_mode: WrapMode,
    mermaid_config: &merman_model::presentation::PresentationConfig,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
) -> (f64, f64, Option<ClassPreparedSvgEdgeLabel>) {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return (0.0, 0.0, None);
    }

    let label = decode_entities_minimal(text);
    if let Some(metrics) = class_math_label_metrics(
        &label,
        measurer,
        text_style,
        Some(MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX),
        mermaid_config,
        math_renderer,
    ) {
        return (metrics.width.max(0.0), metrics.height.max(0.0), None);
    }
    if matches!(wrap_mode, WrapMode::HtmlLike)
        || merman_render_core::math::contains_delimited_math(label.trim())
    {
        let metrics = class_html_measure_label_metrics(
            measurer,
            text_style,
            &label,
            MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX as i64,
            "",
        );
        return (metrics.width.max(0.0), metrics.height.max(0.0), None);
    }

    let prepared = ClassSvgLabel::edge(label.trim(), measurer, text_style);
    let text_bounds = prepared.centered_bounds(measurer, text_style);
    (
        (text_bounds.max_x - text_bounds.min_x).max(0.0) + 2.0 * CLASS_SVG_EDGE_LABEL_PADDING_PX,
        (text_bounds.max_y - text_bounds.min_y).max(0.0) + 2.0 * CLASS_SVG_EDGE_LABEL_PADDING_PX,
        Some(ClassPreparedSvgEdgeLabel {
            label: prepared,
            text_bounds,
            _retained_reservation: None,
        }),
    )
}

fn class_layout_parent<'a>(model: &ClassDiagramModel, parent: Option<&'a str>) -> Option<&'a str> {
    let parent = parent?;
    if model.namespace(parent).is_some() {
        Some(parent)
    } else {
        let trimmed = parent.trim();
        (!trimmed.is_empty() && model.namespace(trimmed).is_some()).then_some(trimmed)
    }
}

fn validate_class_namespace_hierarchy(model: &ClassDiagramModel) -> Result<()> {
    const UNVISITED: u8 = 0;
    const VISITING: u8 = 1;
    const COMPLETE: u8 = 2;

    // Every parent target is a namespace key; other graph IDs cannot close a parent cycle.
    let ids = model.namespaces().map(|(id, _)| id).collect::<Vec<_>>();
    let index_by_id = ids
        .iter()
        .copied()
        .enumerate()
        .map(|(index, id)| (id, index))
        .collect::<FxHashMap<_, _>>();
    let mut parents = vec![None; ids.len()];
    // Parent targets are namespace keys, but materialization writes each payload's ID.
    // Reproduce those assignments, including later class/note aliases that replace a parent.
    for (_, namespace) in model.namespaces() {
        let id = namespace.namespace.id.as_str();
        if let Some(&index) = index_by_id.get(id) {
            parents[index] = class_layout_parent(
                model,
                model
                    .namespace(id)
                    .and_then(|ns| ns.namespace.parent.as_deref()),
            )
            .and_then(|parent| index_by_id.get(parent).copied());
        }
    }
    for (id, parent) in model
        .classes()
        .map(|(_, class)| (class.node.id.as_str(), class.parent))
        .chain(model.notes().map(|note| (note.id, note.parent)))
    {
        if let Some(&index) = index_by_id.get(id)
            && let Some(parent) = class_layout_parent(model, parent)
        {
            parents[index] = index_by_id.get(parent).copied();
        }
    }
    let mut state = vec![UNVISITED; ids.len()];
    let mut depth = vec![0_usize; ids.len()];

    for start in 0..ids.len() {
        if state[start] == COMPLETE {
            continue;
        }

        let mut path = Vec::new();
        let mut current = Some(start);
        let inherited_depth = loop {
            let Some(index) = current else {
                break None;
            };
            match state[index] {
                UNVISITED => {
                    state[index] = VISITING;
                    path.push(index);
                    current = parents[index];
                }
                VISITING => {
                    return Err(Error::InvalidModel {
                        message: format!("class namespace parent cycle involving {}", ids[index]),
                    });
                }
                COMPLETE => break Some(depth[index].saturating_add(1)),
                _ => unreachable!("Class namespace traversal state is internal"),
            }
        };

        let mut next_depth = inherited_depth.unwrap_or(0);
        for index in path.into_iter().rev() {
            if next_depth > merman_model::MAX_DIAGRAM_NESTING_DEPTH {
                return Err(Error::InvalidModel {
                    message: format!(
                        "class namespace nesting depth exceeds {} at {}",
                        merman_model::MAX_DIAGRAM_NESTING_DEPTH,
                        ids[index]
                    ),
                });
            }
            depth[index] = next_depth;
            state[index] = COMPLETE;
            next_depth = next_depth.saturating_add(1);
        }
    }
    Ok(())
}

#[cfg(any(
    feature = "layout-dagre",
    feature = "layout-elk",
    feature = "layout-grid"
))]
pub fn required_terminal_label_position(
    edge_id: &str,
    marker: f64,
    position: TerminalPos,
    points: &[LayoutPoint],
) -> Result<(f64, f64)> {
    // Mermaid's calculatePoint throws when the route cannot supply the sample distance.
    calc_terminal_label_position(marker, position, points).ok_or_else(|| Error::InvalidModel {
        message: format!(
            "Class edge {edge_id}: Could not find a suitable point for the given distance"
        ),
    })
}

#[cfg(any(
    feature = "layout-dagre",
    feature = "layout-elk",
    feature = "layout-grid"
))]
pub fn apply_class_terminal_labels(
    edge: &mut LayoutEdge,
    meta: &EdgeTerminalMetrics,
    points: &[LayoutPoint],
) -> Result<()> {
    for (slot, metrics, marker, position) in [
        (
            &mut edge.start_label_left,
            meta.start_left,
            meta.start_marker,
            TerminalPos::StartLeft,
        ),
        (
            &mut edge.start_label_right,
            meta.start_right,
            meta.start_marker,
            TerminalPos::StartRight,
        ),
        (
            &mut edge.end_label_left,
            meta.end_left,
            meta.end_marker,
            TerminalPos::EndLeft,
        ),
        (
            &mut edge.end_label_right,
            meta.end_right,
            meta.end_marker,
            TerminalPos::EndRight,
        ),
    ] {
        if slot.is_none()
            && let Some((width, height)) = metrics
        {
            let (x, y) = required_terminal_label_position(&edge.id, marker, position, points)?;
            *slot = Some(LayoutLabel {
                x,
                y,
                width,
                height,
            });
        }
    }
    Ok(())
}

#[cfg(any(
    feature = "layout-dagre",
    feature = "layout-elk",
    feature = "layout-grid"
))]
pub fn compute_bounds(
    nodes: &[LayoutNode],
    edges: &[LayoutEdge],
    clusters: &[LayoutCluster],
) -> Option<Bounds> {
    let mut points: Vec<(f64, f64)> = Vec::new();

    for c in clusters {
        let r = Rect::from_center(c.x, c.y, c.width, c.height);
        points.push((r.min_x(), r.min_y()));
        points.push((r.max_x(), r.max_y()));
        let lr = Rect::from_center(
            c.title_label.x,
            c.title_label.y,
            c.title_label.width,
            c.title_label.height,
        );
        points.push((lr.min_x(), lr.min_y()));
        points.push((lr.max_x(), lr.max_y()));
    }

    for n in nodes {
        let r = Rect::from_center(n.x, n.y, n.width, n.height);
        points.push((r.min_x(), r.min_y()));
        points.push((r.max_x(), r.max_y()));
    }

    for e in edges {
        for p in &e.points {
            points.push((p.x, p.y));
        }
        for l in [
            e.label.as_ref(),
            e.start_label_left.as_ref(),
            e.start_label_right.as_ref(),
            e.end_label_left.as_ref(),
            e.end_label_right.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            let r = Rect::from_center(l.x, l.y, l.width, l.height);
            points.push((r.min_x(), r.min_y()));
            points.push((r.max_x(), r.max_y()));
        }
    }

    Bounds::from_points(points)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod class_svg_terminal_title_rows;
