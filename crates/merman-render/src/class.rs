#[cfg(feature = "layout-elk")]
use crate::elk_options::layout_options as class_elk_layout_options;
use crate::entities::decode_entities_minimal;
#[cfg(feature = "layout-elk")]
use crate::graph_layout::ir::Direction;
#[cfg(feature = "layout-elk")]
use crate::layout_backend::ElkRootAlgorithm;
use crate::layout_work::OperationLayoutWorkControl;
use crate::math::MathRenderer;
#[cfg(feature = "layout-elk")]
use crate::model::ClassDiagramLayout;
use crate::model::{
    Bounds, ClassNodeLabelPlan, ClassNodeRowMetrics, ClassPreparedHtmlLabel,
    ClassPreparedHtmlNodeLabels,
};
#[cfg(feature = "layout-elk")]
use crate::model::{ClassRenderItem, ClassRenderRoot, ClassRenderRootId, ClassRenderTree};
#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
use crate::model::{LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode, LayoutPoint};
use crate::text::{
    MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX, MermaidMarkdownAnalysis, TextMeasurer, TextStyle,
    WrapMode, analyze_mermaid_markdown, measure_mermaid_text_dimensions,
};
use crate::{Error, Result};
use rustc_hash::FxHashMap;
#[cfg(feature = "layout-elk")]
use serde_json::Value;
#[cfg(feature = "layout-elk")]
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;

pub(crate) mod config;
#[cfg(feature = "layout-dagre")]
mod dagre;
#[cfg(feature = "layout-elk")]
mod elk_terminals;
#[cfg(any(
    feature = "layout-dagre",
    feature = "layout-elk",
    feature = "layout-shoal",
    test
))]
mod measured;
mod native_theme;
pub(crate) mod node_binding;
#[cfg(feature = "layout-shoal")]
pub(crate) mod shoal;
pub(crate) use self::config::ClassRenderConfig;
#[doc(hidden)]
#[cfg(feature = "layout-dagre")]
pub use self::dagre::debug_build_class_diagram_dagre_graph;
#[cfg(feature = "layout-dagre")]
pub(crate) use self::dagre::layout as layout_class_diagram_typed_with_config;
pub(crate) use self::native_theme::{ClassNativeStyle, ClassThemeAdapter};
pub(crate) use node_binding::ClassNodeVisualPlan;
mod theme;
#[cfg(feature = "layout-elk")]
use self::measured::MeasuredClassDiagram;
#[cfg(any(feature = "layout-elk", test))]
use self::measured::measure_class_diagram;
#[cfg(feature = "layout-elk")]
use merman_layout_elk as elk;
pub(crate) use theme::{ClassCssThemeBinding, ClassRelationThemePlan, ClassTextThemePlan};

type ClassDiagramModel = merman_model::families::class::ClassDiagram;
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
    .any(crate::math::contains_delimited_math)
}

pub(crate) fn class_requires_math(model: &ClassDiagramModel) -> bool {
    model
        .classes()
        .any(|(_, class)| class_node_requires_math(class.node))
        || model.relations().any(|relation| {
            crate::math::contains_delimited_math(&relation.title)
                || relation
                    .relation_title_1
                    .as_deref()
                    .is_some_and(crate::math::contains_delimited_math)
                || relation
                    .relation_title_2
                    .as_deref()
                    .is_some_and(crate::math::contains_delimited_math)
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
            .any(crate::math::contains_delimited_math)
}

/// Estimates the source-backed graph preparation work shared by Class Dagre and ELK layouts.
///
/// Both backends share source measurements. The conservative family allowance also covers Dagre
/// namespace extraction, which scans edges while copying nested descendants, in addition to the
/// linear measurement and projection baseline.
pub(crate) fn class_layout_work_units(
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
    class_member_display_text(member)
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn normalize_dir(direction: &str) -> String {
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

pub(crate) fn class_namespace_ids_in_decl_order(model: &ClassDiagramModel) -> Vec<&str> {
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

pub(crate) fn class_namespace_label<'a>(model: &'a ClassDiagramModel, id: &'a str) -> &'a str {
    model
        .namespace(id)
        .and_then(|ns| {
            let label = ns.namespace.label.as_str();
            (!label.is_empty()).then_some(label)
        })
        .unwrap_or(id)
}

type Rect = crate::geom::Box2;

#[cfg(any(feature = "layout-dagre", feature = "layout-elk", test))]
#[derive(Debug)]
struct EdgeTerminalMetrics {
    start_left: Option<(f64, f64)>,
    start_right: Option<(f64, f64)>,
    end_left: Option<(f64, f64)>,
    end_right: Option<(f64, f64)>,
    start_marker: f64,
    end_marker: f64,
    #[cfg(feature = "layout-elk")]
    start_arrow_type: Option<&'static str>,
    #[cfg(feature = "layout-elk")]
    end_arrow_type: Option<&'static str>,
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
fn round_number(num: f64, precision: i32) -> f64 {
    if !num.is_finite() {
        return 0.0;
    }
    let factor = 10_f64.powi(precision);
    (num * factor).round() / factor
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
fn distance(a: &LayoutPoint, b: Option<&LayoutPoint>) -> f64 {
    let Some(b) = b else {
        return 0.0;
    };
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    (dx * dx + dy * dy).sqrt()
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
fn calculate_point(points: &[LayoutPoint], distance_to_traverse: f64) -> Option<LayoutPoint> {
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

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
#[derive(Debug, Clone, Copy)]
enum TerminalPos {
    StartLeft,
    StartRight,
    EndLeft,
    EndRight,
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
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
fn terminal_path_for_edge(
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
) -> Option<crate::text::TextMetrics> {
    if !crate::math::contains_delimited_math(text) {
        return None;
    }
    let math_renderer = math_renderer?;
    crate::math::math_label_metrics_for_layout(crate::math::MathLabelMetricsRequest {
        measurer,
        raw_label: text,
        style,
        max_width_px,
        wrap_mode: WrapMode::HtmlLike,
        config: mermaid_config,
        math_renderer: Some(math_renderer),
    })
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
    let prepare_html_labels = use_html_labels && !node_requires_math;
    let node_label_style = if use_html_labels {
        node_binding::class_apply_inline_styles(node).style_attr
    } else {
        String::new()
    };
    let padding = padding.max(0.0);
    let gap = padding;
    let text_padding = if use_html_labels { 0.0 } else { 3.0 };

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
        let w = class_html_create_text_width_px(text, measurer, &wrap_probe_style) as f64;
        if w.is_finite() && w > 0.0 {
            Some(w)
        } else {
            None
        }
    }

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
        for line in crate::text::DeterministicTextMeasurer::normalized_text_lines(text) {
            let mut tokens = std::collections::VecDeque::from(
                crate::text::DeterministicTextMeasurer::split_line_to_words(&line),
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
     -> (crate::text::TextMetrics, Option<ClassPreparedHtmlLabel>) {
        let effective_style = if use_html_labels {
            class_html_node_text_style(base_style, css_style, &node_label_style)
        } else {
            class_svg_row_text_style(
                base_style,
                css_style,
                class_svg_empty_markdown_fallback(text).is_some(),
            )
        };
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
                crate::text::measure_markdown_with_inline_styles(
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
                crate::text::measure_markdown_with_inline_styles(
                    measurer, &wrapped, style, None, wrap_mode,
                )
            } else {
                measurer.measure_wrapped(&wrapped, style, None, wrap_mode)
            };
            (metrics, None)
        }
    };

    fn label_rect(m: crate::text::TextMetrics, y_offset: f64) -> Option<Rect> {
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
    let title_style = ctx.native_title_style.unwrap_or(text_style);
    let title_calc_style = ctx.native_title_style.unwrap_or(html_calc_text_style);
    let wrapped_title_text = if matches!(wrap_mode, WrapMode::SvgLike | WrapMode::SvgLikeSingleRun)
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
    let svg_title =
        (!use_html_labels).then(|| ClassSvgTitleLabel::new(&title_source, &wrapped_title_text));
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
    let math_title_metrics = crate::math::contains_delimited_math(&title_text).then(|| {
        let max_width = title_max_width.unwrap_or_else(|| {
            class_html_create_text_width_px(title_text.as_str(), measurer, title_calc_style).max(1)
                as f64
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
        let mut metrics_out: Option<Vec<crate::text::TextMetrics>> =
            capture_fallback_row_metrics.then(|| Vec::with_capacity(rows.len()));
        let mut prepared_out = prepare_html_labels.then(|| Vec::with_capacity(rows.len()));
        let mut y_offset = 0.0;
        for row in rows {
            let t = if use_html_labels {
                class_member_create_text_input(row)
            } else {
                row.display_text_for_render()
            };
            let (metrics, prepared) =
                measure_label(&t, row.css_style.as_str(), ctx.code_text_style);
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
    let mut cascade = crate::mermaid_style::TextStyleCascade::new(
        base,
        crate::mermaid_style::CssFontSizeContext::uniform(base.font_size),
    );
    crate::mermaid_style::visit_parsed_style_declarations(css_style, |declaration| {
        cascade.observe_declaration(&declaration);
    });
    cascade.finish()
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
) -> crate::text::TextMetrics {
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
    let xhtml = crate::text::mermaid_markdown_to_xhtml_label_fragment(text, true);
    // HTML rem is relative to the host document, independently of node typography.
    let mut metrics = crate::text::measure_xhtml_label_fragment_with_terminal_fonts(
        measurer, &xhtml, style, max_width, 16.0,
    );

    let rendered_width = metrics.width;
    if metrics.line_count == 1
        && rendered_width > 0.0
        && rendered_width < max_width_px.max(1) as f64 - 0.01
    {
        metrics.height = metrics
            .height
            .max(crate::text::flowchart_html_line_height_px(style.font_size));
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

pub(crate) fn class_html_measure_note_metrics(
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    note_src: &str,
    mermaid_config: &merman_model::presentation::PresentationConfig,
) -> crate::text::TextMetrics {
    let html = class_note_html_fragment(note_src, mermaid_config);
    // Like the other Class XHTML labels, rem resolves against the host document,
    // independently of the diagram's inherited font size.
    crate::text::measure_unwrapped_xhtml_label_fragment(measurer, &html, style, 16.0)
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
    pub(crate) metrics: crate::text::TextMetrics,
    _retained_reservation: Option<crate::resources::PreparedTextRetainedReservation>,
}

/// SVG-only preparations belong to the family operation, not its public layout projection.
#[derive(Debug, Default)]
pub(crate) struct ClassPreparedLabels {
    pub(crate) titles_by_id: FxHashMap<String, ClassPreparedSvgTitle>,
    pub(crate) edges_by_id: FxHashMap<String, ClassPreparedSvgEdgeLabel>,
}

#[derive(Debug)]
pub(crate) struct ClassPreparedSvgEdgeLabel {
    pub(crate) label: ClassSvgLabel,
    pub(crate) text_bounds: Bounds,
    _retained_reservation: Option<crate::resources::PreparedTextRetainedReservation>,
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
                    .saturating_mul(std::mem::size_of::<crate::text::TextMetrics>()),
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
            *word = crate::entities::decode_svg_text_content_entities(word).into_owned();
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
            label.analysis.lines[0] =
                vec![(fallback, crate::text::MermaidMarkdownWordType::Normal)];
        }
        Self {
            label,
            literal_first_row,
        }
    }

    pub(crate) fn lines(&self) -> &[Vec<(String, crate::text::MermaidMarkdownWordType)>] {
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
    ) -> crate::text::TextMetrics {
        let weight = style.font_weight.as_deref().unwrap_or("bolder");
        let role = ClassTitleMeasurer {
            inner: measurer,
            weight,
        };
        let mut metrics = crate::text::measure_svg_markdown_word_lines(
            &role,
            self.lines(),
            style,
            crate::text::SvgTextEntityMode::DecodedModel,
        );
        if self
            .lines()
            .iter()
            .flatten()
            .all(|(_, kind)| *kind == crate::text::MermaidMarkdownWordType::Normal)
        {
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

    fn measure(&self, text: &str, style: &TextStyle) -> crate::text::TextMetrics {
        self.inner.measure(text, &self.role_style(style))
    }

    fn measure_wrapped(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> crate::text::TextMetrics {
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
    fn retained_heap_bytes(&self) -> usize {
        self.analysis.lines.iter().fold(
            self.analysis
                .lines
                .capacity()
                .saturating_mul(std::mem::size_of::<
                    Vec<(String, crate::text::MermaidMarkdownWordType)>,
                >()),
            |bytes, line| {
                line.iter().fold(
                    bytes.saturating_add(line.capacity().saturating_mul(std::mem::size_of::<(
                        String,
                        crate::text::MermaidMarkdownWordType,
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
        label.analysis.lines = crate::text::wrap_markdown_word_lines(
            measurer,
            label.lines(),
            style,
            Some(MERMAID_CREATE_TEXT_DEFAULT_WIDTH_PX),
            WrapMode::SvgLike,
            true,
            crate::text::SvgTextEntityMode::DecodedModel,
        );
        label.analysis.line_count = label.analysis.lines.len();
        label
    }

    pub(crate) fn new(decoded_text: &str) -> Self {
        Self {
            analysis: analyze_class_svg_markdown(decoded_text),
        }
    }

    pub(crate) fn lines(&self) -> &[Vec<(String, crate::text::MermaidMarkdownWordType)>] {
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
        let style = crate::text::svg_markdown_word_style(
            style,
            crate::text::MermaidMarkdownWordType::Normal,
        );
        let (left, right) = measurer.measure_svg_text_bbox_x(&text, &style);
        let width = left + right;
        (width.is_finite() && width > 0.0).then_some(width)
    }

    pub(crate) fn metrics(
        &self,
        measurer: &dyn TextMeasurer,
        style: &TextStyle,
    ) -> crate::text::TextMetrics {
        let mut metrics = crate::text::measure_svg_markdown_word_lines(
            measurer,
            self.lines(),
            style,
            crate::text::SvgTextEntityMode::DecodedModel,
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
                let style = crate::text::svg_markdown_word_style(style, *kind);
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
                let face = crate::text::svg_markdown_word_style(style, *kind);
                let word_advance = measurer.measure_svg_text_computed_length_px(&text, &face);
                if !crate::text::trim_html_collapsible_ascii_whitespace(word).is_empty() {
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
                    let face = crate::text::svg_markdown_word_style(style, *kind);
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
) -> (f64, f64, crate::text::TextMetrics) {
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
    let html = crate::graph_label::flowchart_label_html_source(text, "text", mermaid_config);
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
        crate::text::measure_unwrapped_xhtml_label_fragment(
            measurer,
            &xhtml,
            text_style,
            text_style.font_size,
        )
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
    if matches!(wrap_mode, WrapMode::HtmlLike) || crate::math::contains_delimited_math(label.trim())
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

#[cfg(feature = "layout-elk")]
/// Lays out a Class diagram through ELK using the render operation's captured seed.
///
/// This remains crate-private so direct callers cannot accidentally turn ELK's unseeded
/// `randomSeed = 0` sentinel into a process-random layout.
#[allow(
    clippy::too_many_arguments,
    reason = "Class ELK receives immutable prepared settings and operation-owned execution inputs."
)]
pub(crate) fn layout_class_diagram_elk_typed_with_config_and_operation_seed(
    model: &ClassDiagramModel,
    effective_config: &merman_model::presentation::PresentationConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    operation_seed: elk::ElkOperationSeed,
    algorithm: ElkRootAlgorithm,
    render_config: &ClassRenderConfig,
    work_control: &mut OperationLayoutWorkControl,
) -> Result<(ClassDiagramLayout, ClassPreparedLabels)> {
    let settings = render_config.layout_settings();
    let mut measured = measure_class_diagram(
        model,
        effective_config,
        measurer,
        math_renderer,
        render_config,
        true,
        Some(work_control),
    )?;
    let prepared_labels = std::mem::take(&mut measured.prepared_labels);
    let settings = ClassElkLayoutSettings {
        // ClassDB.getData assigns class.padding ?? 16 to every namespace, independently
        // of class-node padding and ELK's fixed content inset.
        algorithm,
        namespace_padding: settings.namespace_padding,
        title_margin_top: settings.title_margin_top,
        title_margin_bottom: settings.title_margin_bottom,
        effective_config: effective_config.as_value(),
    };
    let mut elk_graph = class_measured_to_elk_graph(&measured, &settings, &mut Some(work_control))?;
    let orientation = crate::elk_feedback_edges::orient_feedback_edges(
        &mut elk_graph,
        settings.effective_config,
        &mut Some(work_control),
    )?;
    let mut layout = elk::layout_with_operation_seed_and_work_control(
        orientation.graph(),
        operation_seed,
        work_control,
    )
    .map_err(|error| work_control.map_elk_error_with_context(error, "Class ELK"))?;
    orientation.restore(&mut layout, &mut Some(work_control))?;
    let layout = class_layout_from_elk(
        model,
        measured,
        &elk_graph,
        layout,
        settings,
        &mut Some(work_control),
    )?;
    Ok((layout, prepared_labels))
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

#[cfg(feature = "layout-elk")]
struct ClassElkLayoutSettings<'a> {
    algorithm: ElkRootAlgorithm,
    namespace_padding: f64,
    title_margin_top: f64,
    title_margin_bottom: f64,
    effective_config: &'a Value,
}

#[cfg(feature = "layout-elk")]
fn class_measured_to_elk_graph(
    measured: &MeasuredClassDiagram,
    settings: &ClassElkLayoutSettings<'_>,
    work: &mut Option<&mut OperationLayoutWorkControl>,
) -> Result<elk::Graph> {
    let graph = &measured.graph;
    let options = class_elk_layout_options(settings.effective_config, settings.algorithm);
    let direction = match graph.direction {
        Direction::Down => elk::Direction::Down,
        Direction::Up => elk::Direction::Up,
        Direction::Right => elk::Direction::Right,
        Direction::Left => elk::Direction::Left,
    };
    let parents = graph
        .parents
        .values()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let mut nodes: Vec<_> = graph
        .nodes
        .iter()
        .map(|(id, node)| {
            let is_group = node.is_namespace || parents.contains(id.as_str());
            let namespace_label =
                measured
                    .namespace_label_metrics_by_id
                    .get(id)
                    .map(|&(width, height)| elk::Label {
                        width: width.max(1.0),
                        height: height.max(1.0),
                    });
            elk::Node {
                id: id.clone(),
                container: elk::ContainerNodeOptions {
                    padding: if is_group {
                        settings.namespace_padding
                    } else {
                        0.0
                    },
                    ..Default::default()
                },
                label_text: is_group.then(|| id.clone()),
                kind: if is_group {
                    elk::NodeKind::Group
                } else {
                    elk::NodeKind::Leaf
                },
                width: node.width.max(if is_group { 0.0 } else { 1.0 }),
                height: node.height.max(if is_group { 0.0 } else { 1.0 }),
                parent: graph.parents.get(id).cloned(),
                direction: is_group.then_some(direction),
                hierarchy_handling: is_group.then_some(elk::HierarchyHandling::IncludeChildren),
                layer_constraint: None,
                port_alignment: None,
                label: namespace_label,
            }
        })
        .collect();
    if let Some(work) = work.as_deref_mut() {
        let terminal_work =
            work.checked_mul(graph.edges.len(), elk::TerminalLabelKey::ALL.len())?;
        work.charge_adapter(terminal_work)?;
    }
    let edges = graph
        .edges
        .iter()
        .map(|((source, target, id), edge)| elk::Edge {
            id: id.clone(),
            source: source.clone(),
            target: target.clone(),
            label: (edge.width > 0.0 && edge.height > 0.0).then_some(elk::Label {
                width: edge.width,
                height: edge.height,
            }),
            terminal_labels: if options.algorithm == elk::Algorithm::Layered {
                edge.terminals
                    .as_ref()
                    .map(|meta| {
                        elk::TerminalLabelKey::ALL
                            .into_iter()
                            .filter_map(|key| {
                                let (width, height) = class_terminal_metrics(meta, key)?;
                                // Mermaid's class markers are 12px wide. Padding reserves half that
                                // width on each side of the measured label beside a real marker.
                                let marker = if key.at_start() {
                                    meta.start_arrow_type
                                } else {
                                    meta.end_arrow_type
                                };
                                let padding = if marker.is_some() { 6.0 } else { 0.0 };
                                Some(elk::TerminalLabel {
                                    key,
                                    label: elk::Label {
                                        width: width + 2.0 * padding,
                                        height: height + 2.0 * padding,
                                    },
                                })
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                Vec::new()
            },
            minlen: 1,
            inside_self_loops_yo: false,
        })
        .collect();
    if settings
        .effective_config
        .pointer("/elk/keepEntryNodeOnTop")
        .and_then(Value::as_bool)
        != Some(false)
    {
        crate::elk_adapter::apply_cyclic_entry_constraints(
            &mut nodes,
            graph
                .edges
                .keys()
                .map(|(source, target, _)| (source.as_str(), target.as_str())),
            work,
        )?;
    }
    Ok(elk::Graph {
        id: "classDiagram".to_string(),
        direction,
        nodes,
        edges,
        spacing: elk::Spacing {
            node_node: graph.nodesep,
            layer_layer: graph.ranksep,
            group_padding_x: settings.namespace_padding,
            group_padding_y: settings.namespace_padding,
            ..Default::default()
        },
        options,
    })
}

#[cfg(feature = "layout-elk")]
fn class_layout_from_elk(
    model: &ClassDiagramModel,
    measured: MeasuredClassDiagram,
    elk_graph: &elk::Graph,
    layout: elk::LayoutResult,
    settings: ClassElkLayoutSettings<'_>,
    work: &mut Option<&mut OperationLayoutWorkControl>,
) -> Result<ClassDiagramLayout> {
    let frames = crate::elk_adapter::drawing_group_frames(
        elk_graph,
        &layout,
        |node| node.label.map_or(0.0, |label| label.width) + node.container.padding,
        work,
    )?;
    let namespace_ids = class_namespace_ids_in_decl_order(model);
    let namespace_set: HashSet<&str> = namespace_ids.iter().copied().collect();
    let source_node_by_id: HashMap<&str, &elk::Node> = elk_graph
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    let source_edge_by_id: HashMap<&str, &elk::Edge> = elk_graph
        .edges
        .iter()
        .map(|edge| (edge.id.as_str(), edge))
        .collect();
    validate_class_elk_output_ids(
        "node",
        source_node_by_id.keys().copied(),
        layout.nodes.iter().map(|node| node.id.as_str()),
    )?;
    validate_class_elk_output_ids(
        "edge",
        source_edge_by_id.keys().copied(),
        layout.edges.iter().map(|edge| edge.id.as_str()),
    )?;

    let mut nodes = Vec::with_capacity(layout.nodes.len());
    for node in layout.nodes {
        let Some(source) = source_node_by_id.get(node.id.as_str()).copied() else {
            return Err(Error::InvalidModel {
                message: format!("ELK layout returned unknown class node {}", node.id),
            });
        };
        let frame = frames.get(node.id.as_str());
        let label_metrics = measured
            .node_label_metrics_by_id
            .get(&node.id)
            .copied()
            .or_else(|| source.label.map(|label| (label.width, label.height)));
        nodes.push(LayoutNode {
            id: node.id,
            x: frame.map_or(node.x, |frame| frame.x),
            y: frame.map_or(node.y, |frame| frame.y),
            width: frame.map_or(node.width, |frame| frame.width),
            height: frame.map_or(node.height, |frame| frame.height),
            is_cluster: source.kind == elk::NodeKind::Group,
            label_width: label_metrics.map(|(width, _)| width),
            label_height: label_metrics.map(|(_, height)| height),
        });
    }

    let node_by_id: HashMap<&str, &LayoutNode> =
        nodes.iter().map(|node| (node.id.as_str(), node)).collect();
    let mut node_rect_by_id: HashMap<&str, Rect> = HashMap::new();
    for node in &nodes {
        node_rect_by_id.insert(
            node.id.as_str(),
            Rect::from_center(node.x, node.y, node.width, node.height),
        );
    }

    let edge_terminals_by_id = measured
        .graph
        .edges
        .iter()
        .filter_map(|((_, _, id), edge)| edge.terminals.as_ref().map(|meta| (id.as_str(), meta)))
        .collect::<HashMap<_, _>>();

    let mut edges = Vec::with_capacity(layout.edges.len());
    let mut placed_terminals = Vec::with_capacity(layout.edges.len());
    for edge in layout.edges {
        let Some(source) = source_edge_by_id.get(edge.id.as_str()).copied() else {
            return Err(Error::InvalidModel {
                message: format!("ELK layout returned unknown class edge {}", edge.id),
            });
        };
        let terminal_meta = edge_terminals_by_id.get(edge.id.as_str());
        if let Some(work) = work.as_deref_mut() {
            work.charge_adapter(
                edge.points
                    .len()
                    .saturating_add(edge.labels.len())
                    .saturating_add(2),
            )?;
        }
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
                .iter()
                .find(|label| label.terminal.is_none())
                .map(|label| LayoutLabel {
                    x: label.x + label.width / 2.0,
                    y: label.y + label.height / 2.0,
                    width: label.width,
                    height: label.height,
                })
                .or_else(|| class_elk_edge_label_position(&points, source_label))
        });
        let terminal_points = if points.is_empty()
            && let (Some(from), Some(to)) = (
                node_by_id.get(source.source.as_str()),
                node_by_id.get(source.target.as_str()),
            ) {
            crate::elk_geometry::missing_rect_section_points(from, to)
        } else if let (Some(from), Some(to)) = (
            node_rect_by_id.get(source.source.as_str()).copied(),
            node_rect_by_id.get(source.target.as_str()).copied(),
        ) {
            terminal_path_for_edge(&points, from, to)
        } else {
            points.clone()
        };

        let mut placed = elk_terminals::PlacedTerminals {
            ports: points
                .first()
                .zip(points.last())
                .map(|(start, end)| (start.clone(), end.clone())),
            ..Default::default()
        };
        let routed_points = if !points.is_empty()
            && let (Some(start), Some(end)) = (
                node_by_id.get(source.source.as_str()),
                node_by_id.get(source.target.as_str()),
            ) {
            use crate::elk_edge_geometry::{self as geometry, Outline, Shape};
            let start_shape = Shape {
                node: start,
                outline: Outline::Rect,
                intersection: None,
            };
            let end_shape = Shape {
                node: end,
                outline: Outline::Rect,
                intersection: None,
            };
            let mut input = Vec::with_capacity(points.len() + 2);
            input.push(LayoutPoint {
                x: start.x,
                y: start.y,
            });
            input.extend_from_slice(&points);
            input.push(LayoutPoint { x: end.x, y: end.y });
            let mut clipped = geometry::sanitize(&input, start_shape, end_shape);
            if let Some(meta) = terminal_meta {
                geometry::marker_segment(&mut clipped, end_shape, meta.end_arrow_type, false);
                geometry::marker_segment(&mut clipped, start_shape, meta.start_arrow_type, true);
            }
            clipped
        } else {
            points
        };
        let mut out_edge = LayoutEdge {
            id: edge.id,
            from: source.source.clone(),
            to: source.target.clone(),
            from_cluster: node_by_id
                .get(source.source.as_str())
                .filter(|node| node.is_cluster)
                .map(|node| node.id.clone()),
            to_cluster: node_by_id
                .get(source.target.as_str())
                .filter(|node| node.is_cluster)
                .map(|node| node.id.clone()),
            points: routed_points,
            label,
            start_label_left: None,
            start_label_right: None,
            end_label_left: None,
            end_label_right: None,
            start_marker: None,
            end_marker: None,
            stroke_dasharray: None,
        };
        if let Some(meta) = terminal_meta {
            for terminal in &edge.labels {
                let Some(key) = terminal.terminal else {
                    continue;
                };
                if terminal.x == 0.0 && terminal.y == 0.0 {
                    continue;
                }
                let Some((width, height)) = class_terminal_metrics(meta, key) else {
                    continue;
                };
                *elk_terminals::label_slot(&mut out_edge, key) = Some(LayoutLabel {
                    x: terminal.x + terminal.width / 2.0,
                    y: terminal.y + terminal.height / 2.0,
                    width,
                    height,
                });
                placed.keys.push(key);
            }
            apply_class_terminal_labels(&mut out_edge, meta, &terminal_points)?;
        }
        edges.push(out_edge);
        placed_terminals.push(placed);
    }

    let mut charge = |units| match work.as_deref_mut() {
        Some(work) => work.charge_adapter(units),
        None => Ok(()),
    };
    if settings
        .effective_config
        .pointer("/elk/straightenEdges")
        .and_then(Value::as_bool)
        != Some(false)
    {
        crate::elk_terminal_jogs::straighten_edge_terminals(&mut edges, &mut charge)?;
    }
    for (edge, placed) in edges.iter_mut().zip(&placed_terminals) {
        charge(1)?;
        elk_terminals::follow_moved_endpoints(edge, placed);
        // Only unplaced/non-layered labels retain Mermaid's legacy path fallback.
        if !edge.points.is_empty() {
            for (key, position) in [
                (elk::TerminalLabelKey::StartLeft, TerminalPos::StartLeft),
                (elk::TerminalLabelKey::StartRight, TerminalPos::StartRight),
                (elk::TerminalLabelKey::EndLeft, TerminalPos::EndLeft),
                (elk::TerminalLabelKey::EndRight, TerminalPos::EndRight),
            ] {
                if !placed.keys.contains(&key) && elk_terminals::label(edge, key).is_some() {
                    let (x, y) =
                        required_terminal_label_position(&edge.id, 10.0, position, &edge.points)?;
                    if let Some(label) = elk_terminals::label_slot(edge, key) {
                        label.x = x;
                        label.y = y;
                    }
                }
            }
        }
    }
    elk_terminals::slide_off_frames(&mut edges, &placed_terminals, &nodes, &mut charge)?;
    elk_terminals::put_on_own_side(&mut edges, &placed_terminals, &nodes, &mut charge)?;
    crate::elk_terminal_jogs::separate_opposite_edge_labels(
        edges
            .iter_mut()
            .map(|edge| (edge.from.as_str(), edge.to.as_str(), edge.label.as_mut())),
        &mut charge,
    )?;

    let mut clusters = Vec::new();
    for &id in &namespace_ids {
        if !namespace_set.contains(id) {
            continue;
        }
        let Some(node) = node_by_id.get(id).copied() else {
            continue;
        };
        let title = class_namespace_label(model, id).to_string();
        let (title_width, title_height) = measured.namespace_label_metrics_by_id[id];
        let title_label = LayoutLabel {
            x: node.x,
            y: node.y - node.height / 2.0 + settings.title_margin_top + title_height / 2.0,
            width: title_width,
            height: title_height,
        };
        // The title minimum is reserved before layout and bounded by the provider frame.
        // Reapplying it here would grow a contracted cluster without updating its node bounds.
        let width = node.width;
        let diff = -settings.namespace_padding;
        clusters.push(LayoutCluster {
            id: id.to_string(),
            x: node.x,
            y: node.y,
            width,
            height: node.height,
            diff,
            offset_y: title_height - settings.namespace_padding / 2.0,
            title,
            title_label,
            requested_dir: None,
            effective_dir: normalize_dir(&model.model().direction),
            padding: settings.namespace_padding,
            title_margin_top: settings.title_margin_top,
            title_margin_bottom: settings.title_margin_bottom,
        });
    }

    let namespace_order: HashMap<&str, usize> = namespace_ids
        .iter()
        .copied()
        .enumerate()
        .map(|(idx, id)| (id, idx))
        .collect();
    clusters.sort_by(|a, b| {
        namespace_order
            .get(a.id.as_str())
            .copied()
            .unwrap_or(usize::MAX)
            .cmp(
                &namespace_order
                    .get(b.id.as_str())
                    .copied()
                    .unwrap_or(usize::MAX),
            )
            .then_with(|| a.id.cmp(&b.id))
    });

    let bounds = compute_bounds(&nodes, &edges, &clusters);
    let render_tree = ClassRenderTree {
        roots: vec![ClassRenderRoot {
            namespace_id: None,
            cluster_ids: clusters.iter().map(|cluster| cluster.id.clone()).collect(),
            edge_ids: edges.iter().map(|edge| edge.id.clone()).collect(),
            items: nodes
                .iter()
                .filter(|node| !node.is_cluster)
                .map(|node| ClassRenderItem::Node(node.id.clone()))
                .collect(),
        }],
        top: ClassRenderRootId(0),
    };
    Ok(ClassDiagramLayout {
        nodes,
        edges,
        clusters,
        bounds,
        uses_elk_adapter_dom: true,
        class_label_plans_by_id: measured.class_label_plans_by_id,
        render_tree,
    })
}

#[cfg(feature = "layout-elk")]
fn validate_class_elk_output_ids<'a>(
    kind: &'static str,
    expected: impl IntoIterator<Item = &'a str>,
    observed: impl IntoIterator<Item = &'a str>,
) -> Result<()> {
    let expected = expected
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    let mut observed_ids = BTreeSet::new();
    for id in observed {
        if !observed_ids.insert(id.to_owned()) {
            return Err(Error::InvalidModel {
                message: format!("ELK layout returned duplicate class {kind} {id}"),
            });
        }
    }
    if let Some(id) = observed_ids.difference(&expected).next() {
        return Err(Error::InvalidModel {
            message: format!("ELK layout returned unknown class {kind} {id}"),
        });
    }
    if let Some(id) = expected.difference(&observed_ids).next() {
        return Err(Error::InvalidModel {
            message: format!("ELK layout omitted class {kind} {id}"),
        });
    }
    Ok(())
}

#[cfg(feature = "layout-elk")]
fn class_terminal_metrics(
    meta: &EdgeTerminalMetrics,
    key: elk::TerminalLabelKey,
) -> Option<(f64, f64)> {
    match key {
        elk::TerminalLabelKey::StartLeft => meta.start_left,
        elk::TerminalLabelKey::StartRight => meta.start_right,
        elk::TerminalLabelKey::EndLeft => meta.end_left,
        elk::TerminalLabelKey::EndRight => meta.end_right,
    }
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
fn required_terminal_label_position(
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

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
fn apply_class_terminal_labels(
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

#[cfg(feature = "layout-elk")]
fn class_elk_edge_label_position(points: &[LayoutPoint], label: elk::Label) -> Option<LayoutLabel> {
    calculate_point(points, class_elk_polyline_len(points) / 2.0).map(|point| LayoutLabel {
        x: point.x,
        y: point.y,
        width: label.width,
        height: label.height,
    })
}

#[cfg(feature = "layout-elk")]
fn class_elk_polyline_len(points: &[LayoutPoint]) -> f64 {
    points
        .windows(2)
        .map(|pair| (pair[1].x - pair[0].x).hypot(pair[1].y - pair[0].y))
        .sum::<f64>()
}

#[cfg(any(feature = "layout-dagre", feature = "layout-elk"))]
fn compute_bounds(
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
mod tests {
    use merman_core::{MermaidEngine, ParseOptions};
    use merman_model::families::class::Namespace;
    use merman_model::presentation::RenderSemanticModel;

    use crate::text::{DeterministicTextMeasurer, TextMeasurer, TextMetrics, TextStyle, WrapMode};

    #[test]
    fn class_html_and_fallback_plans_enforce_retained_limits_and_release_partial_preparation() {
        use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};
        use std::sync::Arc;

        for (source, prepared_html) in [
            (
                "classDiagram\nclass A {\n<<interface>>\n+value: String\n+call()\n}\nclass B\n",
                true,
            ),
            (
                "classDiagram\nclass A[\"$$x$$\"]\nclass A {\n+value\n+call()\n}\nclass B[\"$$y$$\"]\n",
                false,
            ),
        ] {
            let parsed = MermaidEngine::new()
                .parse_diagram_for_render_model_sync(source, ParseOptions::default())
                .unwrap()
                .unwrap()
                .into_presentation();
            let RenderSemanticModel::Class(model) = parsed.model() else {
                panic!("expected Class model");
            };
            let config =
                crate::test_support::presentation_config(serde_json::json!({"htmlLabels":true}));
            let typography = super::ClassTextThemePlan::resolve(None, &config);
            let render_config = super::ClassRenderConfig::resolve(&config, &typography);
            let measurer = DeterministicTextMeasurer::default();
            for uses_elk_adapter in [false, true] {
                let measure = |policy| {
                    let meter = Arc::new(OperationWorkMeter::new(policy));
                    let work = super::OperationLayoutWorkControl::new(Arc::clone(&meter));
                    let result = super::measure_class_diagram(
                        model,
                        &config,
                        &measurer,
                        None,
                        &render_config,
                        uses_elk_adapter,
                        Some(&work),
                    );
                    (result, meter)
                };
                let policy = RenderResourcePolicy::unbounded_for_trusted_input();
                let (measured, meter) = measure(policy);
                let measured = measured.unwrap();
                assert_eq!(measured.class_label_plans_by_id.len(), 2);
                assert_eq!(
                    matches!(
                        measured.class_label_plans_by_id["A"].as_ref(),
                        super::ClassNodeLabelPlan::PreparedHtml(_)
                    ),
                    prepared_html
                );
                let retained = meter.prepared_text_retained_bytes();
                assert!(retained > 0);
                drop(measured);
                assert_eq!(meter.prepared_text_retained_bytes(), 0);

                let (exact, meter) = measure(
                    policy
                        .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, retained)
                        .unwrap(),
                );
                assert!(exact.is_ok());
                assert_eq!(meter.prepared_text_retained_bytes(), retained);
                drop(exact);
                assert_eq!(meter.prepared_text_retained_bytes(), 0);

                let (rejected, meter) = measure(
                    policy
                        .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, retained - 1)
                        .unwrap(),
                );
                let Some(crate::Error::ResourceLimitExceeded(limit)) = rejected.err() else {
                    panic!("expected partial Class plan preparation to exceed the retained limit");
                };
                assert_eq!(limit.limit, "max_prepared_text_retained_bytes");
                assert!(meter.prepared_text_retained_bytes_peak() > 0);
                assert_eq!(meter.prepared_text_retained_bytes(), 0);
            }
        }
    }

    #[test]
    fn class_html_plan_accounts_for_spare_xhtml_and_container_capacity() {
        use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};
        use std::sync::Arc;

        let label = || {
            let mut xhtml = String::with_capacity(4096);
            xhtml.push_str("<p>x</p>");
            super::ClassPreparedHtmlLabel {
                metrics: TextMetrics {
                    width: 1.0,
                    height: 1.0,
                    line_count: 1,
                },
                max_width_px: 100,
                xhtml,
            }
        };
        let mut labels = super::ClassPreparedHtmlNodeLabels {
            title: label(),
            annotation: Some(label()),
            members: Vec::with_capacity(32),
            methods: Vec::with_capacity(64),
            _retained_reservation: None,
        };
        labels.members.push(label());
        labels.methods.push(label());
        let xhtml_capacity = std::iter::once(&labels.title)
            .chain(labels.annotation.iter())
            .chain(labels.members.iter())
            .chain(labels.methods.iter())
            .map(|label| label.xhtml.capacity())
            .sum::<usize>();
        let row_capacity = (labels.members.capacity() + labels.methods.capacity())
            * std::mem::size_of::<super::ClassPreparedHtmlLabel>();
        let mut id = String::with_capacity(2048);
        id.push('A');
        let payload_bytes = xhtml_capacity + row_capacity + id.capacity();
        let meter = Arc::new(OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, payload_bytes)
                .unwrap(),
        ));
        let work = super::OperationLayoutWorkControl::new(Arc::clone(&meter));
        let mut plan = super::ClassNodeLabelPlan::PreparedHtml(labels);
        let Err(crate::Error::ResourceLimitExceeded(limit)) =
            plan.reserve_retained_bytes(id.capacity(), &work)
        else {
            panic!("the plan and Arc/map owners must also fit after admitting payload capacities");
        };
        assert_eq!(limit.limit, "max_prepared_text_retained_bytes");
        assert!(limit.actual > payload_bytes);
        assert_eq!(meter.prepared_text_retained_bytes(), 0);
    }

    #[test]
    fn class_html_prepared_plans_release_on_mid_preparation_cancellation() {
        use crate::resources::{OperationWorkMeter, RenderResourcePolicy};
        use std::sync::Arc;

        struct CancellingMeasurer {
            meter: Arc<OperationWorkMeter>,
            control: merman_model::OperationControl,
        }

        impl TextMeasurer for CancellingMeasurer {
            fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
                self.measure_wrapped(text, style, None, WrapMode::HtmlLike)
            }

            fn measure_wrapped(
                &self,
                text: &str,
                style: &TextStyle,
                max_width: Option<f64>,
                wrap_mode: WrapMode,
            ) -> TextMetrics {
                if self.meter.prepared_text_retained_bytes() > 0 {
                    self.control.cancel();
                }
                DeterministicTextMeasurer::default()
                    .measure_wrapped(text, style, max_width, wrap_mode)
            }
        }

        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "classDiagram\nclass A\nclass B\n",
                ParseOptions::default(),
            )
            .unwrap()
            .unwrap()
            .into_presentation();
        let RenderSemanticModel::Class(model) = parsed.model() else {
            panic!("expected Class model");
        };
        let config =
            crate::test_support::presentation_config(serde_json::json!({"htmlLabels":true}));
        let typography = super::ClassTextThemePlan::resolve(None, &config);
        let render_config = super::ClassRenderConfig::resolve(&config, &typography);
        let control = merman_model::OperationControl::new();
        let meter = Arc::new(OperationWorkMeter::new_with_control(
            RenderResourcePolicy::unbounded_for_trusted_input(),
            control.clone(),
        ));
        let work = super::OperationLayoutWorkControl::new(Arc::clone(&meter));
        let measurer = CancellingMeasurer {
            meter: Arc::clone(&meter),
            control,
        };
        let result = super::measure_class_diagram(
            model,
            &config,
            &measurer,
            None,
            &render_config,
            false,
            Some(&work),
        );
        assert!(matches!(result, Err(crate::Error::Cancelled(_))));
        assert!(meter.prepared_text_retained_bytes_peak() > 0);
        assert_eq!(meter.prepared_text_retained_bytes(), 0);
    }

    #[cfg(feature = "layout-dagre")]
    #[test]
    fn class_html_family_releases_plans_after_svg_failure_or_cancellation() {
        use crate::environment::RenderEnvironment;
        use crate::resources::{RenderResourcePolicy, ResourceLimitId};
        use crate::svg::{SvgDebugOptions, SvgRenderOptions};
        use std::sync::Arc;

        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "---\nconfig:\n  htmlLabels: true\n---\nclassDiagram\nclass A[\"MMMM\"]\n",
                ParseOptions::strict(),
            )
            .unwrap()
            .unwrap()
            .into_presentation();
        for cancel in [false, true] {
            let control = merman_model::OperationControl::new();
            let session = RenderEnvironment::deterministic()
                .with_resource_policy(
                    RenderResourcePolicy::unbounded_for_trusted_input()
                        .with_limit(ResourceLimitId::MaxSvgBytes, 1)
                        .unwrap(),
                )
                .begin_session_with_control(control.clone())
                .unwrap();
            let meter = Arc::clone(session.work_meter());
            let artifact = crate::family::prepare(
                &parsed,
                &crate::LayoutOptions::headless_svg_defaults(),
                session,
            )
            .unwrap();
            assert!(meter.prepared_text_retained_bytes() > 0);
            if cancel {
                control.cancel();
            }
            let error = match artifact
                .render_svg(&SvgRenderOptions::default(), &SvgDebugOptions::default())
            {
                Err(error) => error,
                Ok(_) => panic!("expected SVG failure or cancellation"),
            };
            if cancel {
                assert!(matches!(error, crate::Error::Cancelled(_)));
            } else {
                let crate::Error::ResourceLimitExceeded(limit) = error else {
                    panic!("expected the SVG output ceiling");
                };
                assert_eq!(limit.limit, "max_svg_bytes");
            }
            assert_eq!(meter.prepared_text_retained_bytes(), 0);
        }
    }

    #[test]
    fn class_svg_prepared_labels_own_retained_bytes_and_release_partial_preparation() {
        use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};

        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "classDiagram\nclass A[\"**OwnershipTitle**\"]\nclass B\nA --> B : **OwnershipEdge**\n",
                ParseOptions::default(),
            )
            .unwrap()
            .unwrap().into_presentation();
        let RenderSemanticModel::Class(model) = parsed.model() else {
            panic!("expected Class model");
        };
        let config =
            crate::test_support::presentation_config(serde_json::json!({"htmlLabels":false}));
        let typography = super::ClassTextThemePlan::resolve(None, &config);
        let render_config = super::ClassRenderConfig::resolve(&config, &typography);
        let measurer = DeterministicTextMeasurer::default();
        for uses_elk_adapter in [false, true] {
            let measure = |policy| {
                let meter = std::sync::Arc::new(OperationWorkMeter::new(policy));
                let work = super::OperationLayoutWorkControl::new(std::sync::Arc::clone(&meter));
                let result = super::measure_class_diagram(
                    model,
                    &config,
                    &measurer,
                    None,
                    &render_config,
                    uses_elk_adapter,
                    Some(&work),
                );
                (result, meter)
            };
            let policy = RenderResourcePolicy::unbounded_for_trusted_input();
            let (measured, meter) = measure(policy);
            let mut measured = measured.unwrap();
            let retained = meter.prepared_text_retained_bytes();
            assert!(retained > 0);
            let labels = std::mem::take(&mut measured.prepared_labels);
            assert_eq!(labels.titles_by_id.len(), 2);
            assert_eq!(
                labels.titles_by_id["A"].label.plain_text(),
                "OwnershipTitle"
            );
            assert_eq!(labels.edges_by_id.len(), 1);
            assert_eq!(
                labels
                    .edges_by_id
                    .values()
                    .next()
                    .unwrap()
                    .label
                    .plain_text(),
                "OwnershipEdge"
            );
            drop(measured);
            assert_eq!(meter.prepared_text_retained_bytes(), retained);
            drop(labels);
            assert_eq!(meter.prepared_text_retained_bytes(), 0);

            let (exact, meter) = measure(
                policy
                    .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, retained)
                    .unwrap(),
            );
            assert!(exact.is_ok());
            assert_eq!(meter.prepared_text_retained_bytes(), retained);
            drop(exact);
            assert_eq!(meter.prepared_text_retained_bytes(), 0);

            let (rejected, meter) = measure(
                policy
                    .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, retained - 1)
                    .unwrap(),
            );
            let Some(crate::Error::ResourceLimitExceeded(limit)) = rejected.err() else {
                panic!("expected the prepared-text ceiling to reject partial Class preparation");
            };
            assert_eq!(limit.limit, "max_prepared_text_retained_bytes");
            assert_eq!(meter.prepared_text_retained_bytes(), 0);
        }
    }

    #[test]
    fn class_prepared_reservation_observes_cancellation_before_its_byte_ceiling() {
        use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};

        let control = merman_model::OperationControl::new();
        let meter = std::sync::Arc::new(OperationWorkMeter::new_with_control(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxPreparedTextRetainedBytes, 1)
                .unwrap(),
            control.clone(),
        ));
        let work = super::OperationLayoutWorkControl::new(std::sync::Arc::clone(&meter));
        control.cancel();
        assert!(matches!(
            work.reserve_prepared_text_retained_bytes(2),
            Err(crate::Error::Cancelled(_))
        ));
        assert_eq!(meter.prepared_text_retained_bytes(), 0);
    }

    #[test]
    fn metrics_only_html_measurement_matches_prepared_labels() {
        let measurer = DeterministicTextMeasurer::default();
        let style = TextStyle::default();
        for (label, css, width) in [
            ("plain", "", 200),
            (
                "**bold** and <span style='color:red'>owned</span>",
                "font-size:20px",
                32,
            ),
            ("multiple<br/>lines", "font-family:monospace", 100),
        ] {
            let expected =
                super::class_prepare_html_label(&measurer, &style, label, width, css).metrics;
            let actual =
                super::class_html_measure_label_metrics(&measurer, &style, label, width, css);
            assert_eq!(
                (actual.width, actual.height, actual.line_count),
                (expected.width, expected.height, expected.line_count)
            );
        }
    }

    #[test]
    fn class_member_display_preserves_authored_colon_spaces_and_classifiers() {
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "classDiagram\nclass Box {\n+left :String\n+right: String\n+both : String$\n+tight:String\n+convert(value: String) Result\n}",
                ParseOptions::default(),
            )
            .expect("parse Class member whitespace")
            .expect("detect Class diagram").into_presentation();
        let RenderSemanticModel::Class(model) = parsed.model() else {
            panic!("expected Class model");
        };
        let class = model.class("Box").expect("Box class");
        let members = class
            .members
            .iter()
            .map(super::class_member_display_text)
            .collect::<Vec<_>>();
        assert_eq!(
            members,
            [
                "+left :String",
                "+right: String",
                "+both : String",
                "+tight:String"
            ]
        );
        assert_eq!(class.members[2].classifier, "$");
        assert_eq!(class.members[2].css_style, "text-decoration:underline;");
        assert_eq!(
            super::class_member_display_text(&class.methods[0]),
            "+convert(value: String) : Result"
        );
        assert_eq!(
            super::class_member_create_text_input(&class.members[1]),
            "+right: String"
        );
    }

    #[cfg(feature = "layout-elk")]
    fn class_elk_graph_for_test(source: &str) -> super::elk::Graph {
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, ParseOptions::default())
            .expect("parse Class source")
            .expect("detect Class source")
            .into_presentation();
        let RenderSemanticModel::Class(model) = parsed.model() else {
            panic!("expected Class model");
        };
        let config = &parsed.metadata().effective_config;
        let typography_theme = super::ClassTextThemePlan::resolve(None, config);
        let render_config = super::ClassRenderConfig::resolve(config, &typography_theme);
        let settings = render_config.layout_settings();
        let measured = super::measure_class_diagram(
            model,
            config,
            &DeterministicTextMeasurer::default(),
            None,
            &render_config,
            true,
            None,
        )
        .expect("measure Class diagram");
        super::class_measured_to_elk_graph(
            &measured,
            &super::ClassElkLayoutSettings {
                algorithm: super::ElkRootAlgorithm::for_diagnostic(config.as_value()),
                namespace_padding: settings.namespace_padding,
                title_margin_top: settings.title_margin_top,
                title_margin_bottom: settings.title_margin_bottom,
                effective_config: config.as_value(),
            },
            &mut None,
        )
        .expect("Class ELK graph")
    }

    #[cfg(feature = "layout-elk")]
    #[test]
    fn class_elk_interfaces_remain_root_nodes_outside_nested_namespaces() {
        for relation in ["A --() ProvidedInterface", "ProvidedInterface ()-- A"] {
            let graph = class_elk_graph_for_test(&format!(
                "classDiagram\nnamespace Outer {{\nnamespace Inner {{\nclass A\n}}\n}}\n{relation}\n"
            ));
            let node = |id: &str| graph.nodes.iter().find(|node| node.id == id).expect("node");
            assert_eq!(node("A").parent.as_deref(), Some("Outer.Inner"));
            assert_eq!(node("Outer.Inner").parent.as_deref(), Some("Outer"));
            assert_eq!(node("interface0").parent, None);
            assert!(graph.edges.iter().any(|edge| {
                (edge.source == "A" && edge.target == "interface0")
                    || (edge.source == "interface0" && edge.target == "A")
            }));
        }
    }

    #[cfg(feature = "layout-elk")]
    #[test]
    fn class_elk_namespace_padding_reaches_every_container_without_flowchart_fallback() {
        for (config, expected) in [
            ("flowchart:\n    padding: 83", 16.0),
            (
                "class:\n    padding: 23\n  flowchart:\n    padding: 83",
                23.0,
            ),
            ("class:\n    padding: 0\n  flowchart:\n    padding: 83", 0.0),
        ] {
            let graph = class_elk_graph_for_test(&format!(
                "---\nconfig:\n  {config}\n---\nclassDiagram\nnamespace Outer {{\nnamespace Inner {{\nclass A\n}}\n}}\n"
            ));
            for id in ["Outer", "Outer.Inner"] {
                let container = graph.nodes.iter().find(|node| node.id == id).unwrap();
                assert_eq!(container.container.padding, expected, "{config}: {id}");
            }
            assert_eq!(graph.spacing.group_padding_x, expected, "{config}");
            assert_eq!(graph.spacing.group_padding_y, expected, "{config}");
        }
    }

    #[cfg(feature = "layout-elk")]
    #[test]
    fn class_elk_cycle_entry_follows_relations_independently_of_node_declarations() {
        for declarations in ["class B\nclass A\nclass C", "class C\nclass B\nclass A"] {
            for enabled in [false, true] {
                let graph = class_elk_graph_for_test(&format!(
                    "---\nconfig:\n  elk:\n    keepEntryNodeOnTop: {enabled}\n    cycleBreakingStrategy: GREEDY_MODEL_ORDER\n---\nclassDiagram\n{declarations}\nA --> B\nB --> C\nC --> A\n"
                ));
                let constrained = graph
                    .nodes
                    .iter()
                    .filter(|node| {
                        node.layer_constraint == Some(super::elk::LayerConstraint::First)
                    })
                    .map(|node| node.id.as_str())
                    .collect::<Vec<_>>();
                assert_eq!(constrained, if enabled { vec!["A"] } else { vec![] });
            }
        }
    }

    #[cfg(feature = "layout-elk")]
    #[test]
    fn class_elk_interface_padding_preserves_measured_label_bounds_in_both_modes() {
        for look in ["classic", "neo"] {
            for html in [false, true] {
                for font_size in [14, 23] {
                    let source = format!(
                        "---\nconfig:\n  look: {look}\n  htmlLabels: {html}\n  themeVariables:\n    fontSize: {font_size}px\n---\nclassDiagram\nA --() CompleteInterface\n"
                    );
                    let parsed = MermaidEngine::new()
                        .parse_diagram_for_render_model_sync(&source, ParseOptions::default())
                        .unwrap()
                        .unwrap()
                        .into_presentation();
                    let RenderSemanticModel::Class(model) = parsed.model() else {
                        panic!("Class model")
                    };
                    let config = &parsed.metadata().effective_config;
                    let typography_theme = super::ClassTextThemePlan::resolve(None, config);
                    let render_config =
                        super::ClassRenderConfig::resolve(config, &typography_theme);
                    let layout_settings = render_config.layout_settings();
                    let measured = super::measure_class_diagram(
                        model,
                        config,
                        &DeterministicTextMeasurer::default(),
                        None,
                        &render_config,
                        true,
                        None,
                    )
                    .unwrap();
                    let (label_width, label_height) =
                        measured.node_label_metrics_by_id["interface0"];
                    let (padding_x, padding_y) = if look == "neo" {
                        (32.0, 24.0)
                    } else {
                        (0.0, 0.0)
                    };
                    let settings = super::ClassElkLayoutSettings {
                        algorithm: super::ElkRootAlgorithm::for_diagnostic(config.as_value()),
                        namespace_padding: layout_settings.namespace_padding,
                        title_margin_top: layout_settings.title_margin_top,
                        title_margin_bottom: layout_settings.title_margin_bottom,
                        effective_config: config.as_value(),
                    };
                    let graph = super::class_measured_to_elk_graph(&measured, &settings, &mut None)
                        .unwrap();
                    let interface = graph
                        .nodes
                        .iter()
                        .find(|node| node.id == "interface0")
                        .unwrap();
                    assert!((interface.width - label_width - padding_x).abs() < 1e-6);
                    assert!((interface.height - label_height - padding_y).abs() < 1e-6);
                    let provider = super::elk::LayoutResult {
                        nodes: graph
                            .nodes
                            .iter()
                            .map(|node| super::elk::NodeLayout {
                                id: node.id.clone(),
                                x: 0.0,
                                y: 0.0,
                                width: node.width,
                                height: node.height,
                            })
                            .collect(),
                        edges: graph
                            .edges
                            .iter()
                            .map(|edge| super::elk::EdgeLayout {
                                id: edge.id.clone(),
                                points: vec![
                                    super::elk::Point { x: 0.0, y: 0.0 },
                                    super::elk::Point { x: 40.0, y: 40.0 },
                                ],
                                labels: Vec::new(),
                            })
                            .collect(),
                    };
                    let layout = super::class_layout_from_elk(
                        model, measured, &graph, provider, settings, &mut None,
                    )
                    .unwrap();
                    let painted = layout
                        .nodes
                        .iter()
                        .find(|node| node.id == "interface0")
                        .unwrap();
                    assert_eq!(painted.label_width, Some(label_width));
                    assert_eq!(painted.label_height, Some(label_height));
                    assert_eq!(
                        (painted.width, painted.height),
                        (interface.width, interface.height)
                    );
                }
            }
        }
    }

    #[cfg(feature = "layout-elk")]
    #[test]
    fn class_elk_contracts_namespace_frame_without_rebasing_provider_routes() {
        for title_width in [32.0, 300.0] {
            let parsed = MermaidEngine::new()
                .parse_diagram_for_render_model_sync(
                    "classDiagram\nnamespace Frame {\nclass A\n}\nA --> A : loop\n",
                    ParseOptions::default(),
                )
                .unwrap()
                .unwrap()
                .into_presentation();
            let RenderSemanticModel::Class(model) = parsed.model() else {
                panic!("Class model")
            };
            let config = &parsed.metadata().effective_config;
            let typography_theme = super::ClassTextThemePlan::resolve(None, config);
            let render_config = super::ClassRenderConfig::resolve(config, &typography_theme);
            let layout_settings = render_config.layout_settings();
            let mut measured = super::measure_class_diagram(
                model,
                config,
                &DeterministicTextMeasurer::default(),
                None,
                &render_config,
                true,
                None,
            )
            .unwrap();
            measured
                .namespace_label_metrics_by_id
                .get_mut("Frame")
                .unwrap()
                .0 = title_width;
            let settings = super::ClassElkLayoutSettings {
                algorithm: super::ElkRootAlgorithm::for_diagnostic(config.as_value()),
                namespace_padding: layout_settings.namespace_padding,
                title_margin_top: layout_settings.title_margin_top,
                title_margin_bottom: layout_settings.title_margin_bottom,
                effective_config: config.as_value(),
            };
            let graph =
                super::class_measured_to_elk_graph(&measured, &settings, &mut None).unwrap();
            let provider = super::elk::LayoutResult {
                nodes: vec![
                    super::elk::NodeLayout {
                        id: "Frame".to_string(),
                        x: 100.0,
                        y: 74.0,
                        width: 200.0,
                        height: 148.0,
                    },
                    super::elk::NodeLayout {
                        id: "A".to_string(),
                        x: 90.0,
                        y: 86.0,
                        width: 100.0,
                        height: 76.0,
                    },
                ],
                edges: vec![super::elk::EdgeLayout {
                    id: graph.edges[0].id.clone(),
                    points: vec![
                        super::elk::Point { x: 40.0, y: 60.0 },
                        super::elk::Point { x: 140.0, y: 70.0 },
                    ],
                    labels: vec![super::elk::EdgeLabelLayout {
                        terminal: None,
                        x: 100.0,
                        y: 20.0,
                        width: 30.0,
                        height: 20.0,
                    }],
                }],
            };
            let layout = super::class_layout_from_elk(
                model, measured, &graph, provider, settings, &mut None,
            )
            .unwrap();
            let frame = &layout.clusters[0];
            let expected = if title_width < 200.0 {
                (90.0, 74.0, 148.0, 148.0)
            } else {
                (100.0, 74.0, 200.0, 148.0)
            };
            assert_eq!((frame.x, frame.y, frame.width, frame.height), expected);
            let node = layout.nodes.iter().find(|node| node.id == "Frame").unwrap();
            assert_eq!((node.x, node.y, node.width, node.height), expected);
            assert_eq!(
                layout.edges[0]
                    .points
                    .iter()
                    .map(|p| (p.x, p.y))
                    .collect::<Vec<_>>(),
                [(40.0, 60.0), (140.0, 70.0)]
            );
            let label = layout.edges[0].label.as_ref().unwrap();
            assert_eq!((label.x, label.y), (115.0, 30.0));
        }
    }

    #[cfg(feature = "layout-elk")]
    #[test]
    fn class_elk_terminal_provider_padding_only_reserves_real_markers() {
        use super::elk::TerminalLabelKey;
        let plain = class_elk_graph_for_test("classDiagram\nA \"many\" -- \"many\" B\n");
        let marked = class_elk_graph_for_test("classDiagram\nA \"many\" <|-- \"many\" B\n");
        assert_eq!(plain.edges[0].terminal_labels.len(), 2);
        for key in [TerminalLabelKey::StartRight, TerminalLabelKey::EndLeft] {
            let before = plain.edges[0]
                .terminal_labels
                .iter()
                .find(|label| label.key == key)
                .unwrap()
                .label;
            let after = marked.edges[0]
                .terminal_labels
                .iter()
                .find(|label| label.key == key)
                .unwrap()
                .label;
            let padding = if key.at_start() { 12.0 } else { 0.0 };
            assert!((after.width - before.width - padding).abs() < 1e-9);
            assert!((after.height - before.height - padding).abs() < 1e-9);
            assert_eq!(before.height, 16.5, "measured terminal CSS font is 11px");
        }
        let non_layered = class_elk_graph_for_test(
            "---\nconfig:\n  layout: elk.mrtree\n---\nclassDiagram\nA \"many\" --> \"one\" B\n",
        );
        assert!(non_layered.edges[0].terminal_labels.is_empty());
    }

    #[cfg(feature = "layout-elk")]
    #[test]
    fn class_elk_keeps_the_source_default_nonzero_seed() {
        assert_eq!(
            super::class_elk_layout_options(
                &serde_json::Value::Null,
                super::ElkRootAlgorithm::Layered
            )
            .layered
            .random_seed,
            1
        );
    }

    #[cfg(feature = "layout-elk")]
    #[test]
    fn class_elk_zero_seed_adapter_graph_requires_an_operation_seed() {
        use std::num::NonZeroU64;

        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync("classDiagram\nA --> B", ParseOptions::default())
            .expect("parse Class source")
            .expect("detect Class source")
            .into_presentation();
        let RenderSemanticModel::Class(model) = parsed.model() else {
            panic!("expected Class model");
        };
        let config = &parsed.metadata().effective_config;
        let typography_theme = super::ClassTextThemePlan::resolve(None, config);
        let render_config = super::ClassRenderConfig::resolve(config, &typography_theme);
        let measured = super::measure_class_diagram(
            model,
            config,
            &DeterministicTextMeasurer::default(),
            None,
            &render_config,
            true,
            None,
        )
        .expect("measure Class diagram");
        let settings = super::ClassElkLayoutSettings {
            algorithm: super::ElkRootAlgorithm::Layered,
            namespace_padding: 8.0,
            title_margin_top: 0.0,
            title_margin_bottom: 0.0,
            effective_config: &serde_json::Value::Null,
        };
        let mut graph = super::class_measured_to_elk_graph(&measured, &settings, &mut None)
            .expect("Class ELK graph");
        graph.options.layered.random_seed = 0;

        assert!(super::elk::layout(&graph).is_err());

        let operation_seed = super::elk::ElkOperationSeed::from_operation_seed(
            NonZeroU64::new(0x636c_6173_7365_6c6b).expect("nonzero operation seed"),
        );
        let first = super::elk::layout_with_operation_seed(&graph, operation_seed)
            .expect("seeded Class layout");
        let replayed = super::elk::layout_with_operation_seed(&graph, operation_seed)
            .expect("replayed seeded Class layout");

        assert_eq!(first, replayed);
    }

    struct ClassProbeMeasurer;

    struct ClassPrecisionMeasurer;

    struct ClassMathGeometryMeasurer;

    #[derive(Debug)]
    struct ClassMathGeometryRenderer;

    impl TextMeasurer for ClassProbeMeasurer {
        fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
            TextMetrics {
                width: 0.0,
                height: 0.0,
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

    impl TextMeasurer for ClassPrecisionMeasurer {
        fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
            TextMetrics {
                width: 73.123_456_789,
                height: 17.25,
                line_count: 1,
            }
        }

        fn measure_svg_text_bbox_x(&self, _text: &str, _style: &TextStyle) -> (f64, f64) {
            (31.0, 42.123_456_789)
        }

        fn measure_svg_tspan_text_bbox_width_px(&self, _text: &str, _style: &TextStyle) -> f64 {
            panic!("formatted Class labels must not use the single-tspan operation")
        }
    }

    impl TextMeasurer for ClassMathGeometryMeasurer {
        fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
            TextMetrics {
                width: 29.0,
                height: 19.0,
                line_count: 1,
            }
        }

        fn measure_svg_tspan_text_bbox_width_px(&self, _text: &str, _style: &TextStyle) -> f64 {
            17.0
        }
    }

    impl crate::math::MathRenderer for ClassMathGeometryRenderer {
        fn render_html_label(
            &self,
            text: &str,
            _config: &merman_model::presentation::PresentationConfig,
        ) -> Option<String> {
            text.contains("$$").then(|| text.to_string())
        }

        fn measure_html_label(
            &self,
            text: &str,
            _config: &merman_model::presentation::PresentationConfig,
            _style: &TextStyle,
            _max_width_px: Option<f64>,
            _wrap_mode: WrapMode,
        ) -> Option<TextMetrics> {
            text.contains("$$").then_some(TextMetrics {
                width: 123.0,
                height: 37.0,
                line_count: 1,
            })
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

    fn class_model_with_namespace_chain(count: usize) -> super::ClassDiagramModel {
        let mut model: merman_model::families::class::ClassDiagramModel =
            serde_json::from_value(serde_json::json!({
                "type": "classDiagram",
                "direction": "TB",
                "classes": {},
                "constants": {
                    "lineType": { "line": 0, "dottedLine": 1 },
                    "relationType": {
                        "none": 0,
                        "aggregation": 1,
                        "extension": 2,
                        "composition": 3,
                        "dependency": 4,
                        "lollipop": 5
                    }
                }
            }))
            .expect("minimal Class diagram model");
        for index in 0..count {
            let id = format!("ns{index}");
            model.namespaces.insert(
                id.clone(),
                Namespace {
                    id,
                    label: String::new(),
                    class_ids: Vec::new(),
                    note_ids: Vec::new(),
                    parent: index.checked_sub(1).map(|parent| format!("ns{parent}")),
                    explicit: true,
                },
            );
        }
        super::ClassDiagramModel::new(model, &merman_model::MermaidConfig::default())
    }

    #[test]
    fn class_namespace_hierarchy_accepts_the_shared_depth_boundary() {
        let model = class_model_with_namespace_chain(merman_model::MAX_DIAGRAM_NESTING_DEPTH + 1);

        super::validate_class_namespace_hierarchy(&model)
            .expect("the shared maximum namespace nesting depth is valid");
    }

    #[test]
    fn class_namespace_hierarchy_rejects_excessive_depth_without_recursion() {
        let model = class_model_with_namespace_chain(merman_model::MAX_DIAGRAM_NESTING_DEPTH + 2);

        let error = super::validate_class_namespace_hierarchy(&model)
            .expect_err("namespace nesting beyond the shared limit must be rejected");

        assert_eq!(
            error.to_string(),
            format!(
                "invalid semantic model: class namespace nesting depth exceeds {} at ns{}",
                merman_model::MAX_DIAGRAM_NESTING_DEPTH,
                merman_model::MAX_DIAGRAM_NESTING_DEPTH + 1
            )
        );
    }

    #[test]
    fn class_namespace_hierarchy_rejects_parent_cycles() {
        let mut model = class_model_with_namespace_chain(3).into_model();
        model.namespaces["ns0"].parent = Some("ns2".to_string());
        let model = super::ClassDiagramModel::new(model, &merman_model::MermaidConfig::default());

        let error = super::validate_class_namespace_hierarchy(&model)
            .expect_err("namespace parent cycles must be rejected");

        assert!(error.to_string().contains("namespace parent cycle"));
    }

    #[cfg(feature = "layout-elk")]
    #[test]
    fn class_elk_output_inventory_rejects_missing_duplicate_and_unknown_ids() {
        super::validate_class_elk_output_ids("node", ["A", "B"], ["A", "B"])
            .expect("matching ELK node inventory");

        for (observed, expected_message) in [
            (vec!["A"], "ELK layout omitted class node B"),
            (vec!["A", "A"], "ELK layout returned duplicate class node A"),
            (vec!["A", "C"], "ELK layout returned unknown class node C"),
        ] {
            let error = super::validate_class_elk_output_ids("node", ["A", "B"], observed)
                .expect_err("invalid ELK node inventory");
            assert_eq!(
                error.to_string(),
                format!("invalid semantic model: {expected_message}")
            );
        }
    }

    #[test]
    fn class_create_text_width_uses_shared_mermaid_dimensions() {
        let style = TextStyle {
            font_family: Some("Arial".to_string()),
            font_size: 16.0,
            font_weight: None,
            font_style: None,
        };

        assert_eq!(
            super::class_html_create_text_width_px("unseen label", &ClassProbeMeasurer, &style),
            130
        );
    }

    #[test]
    fn class_effective_text_style_uses_token_aware_declaration_boundaries() {
        let base = default_style();
        let style = super::class_effective_text_style(
            &base,
            r#"font-family:"a;b",sans-serif;font-size:20px !important"#,
        );

        assert_eq!(style.font_family.as_deref(), Some(r#""a;b",sans-serif"#));
        assert_eq!(style.font_size, 20.0);
    }

    #[test]
    fn class_effective_text_style_applies_css_winners_against_the_inherited_base() {
        let base = default_style();
        let style = super::class_effective_text_style(
            &base,
            "font-family:Important !important;font-family:Later;\
             font-size:20px !important;font-size:2em;\
             font-weight:700 !important;font-weight:400;\
             font-style:italic !important;font-style:normal",
        );

        assert_eq!(style.font_family.as_deref(), Some("Important"));
        assert_eq!(style.font_size, 20.0);
        assert_eq!(style.font_weight.as_deref(), Some("700"));
        assert_eq!(style.font_style.as_deref(), Some("italic"));

        let relative = super::class_effective_text_style(&base, "font-size:2em;font-size:2em");
        assert_eq!(relative.font_size, base.font_size * 2.0);

        let same_priority = super::class_effective_text_style(
            &base,
            "font-family:First;font-family:Second;\
             font-size:2em;font-size:150%;\
             font-weight:400;font-weight:500;\
             font-style:italic;font-style:normal",
        );
        assert_eq!(same_priority.font_family.as_deref(), Some("Second"));
        assert_eq!(same_priority.font_size, base.font_size * 1.5);
        assert_eq!(same_priority.font_weight.as_deref(), Some("500"));
        assert_eq!(same_priority.font_style.as_deref(), Some("normal"));

        let invalid = super::class_effective_text_style(
            &base,
            "font-family:Valid;font-family:inherit junk !important;\
             font-size:20px;font-size:30px junk !important;\
             font-weight:700;font-weight:invalid !important;\
             font-style:italic;font-style:invalid !important",
        );
        assert_eq!(invalid.font_family.as_deref(), Some("Valid"));
        assert_eq!(invalid.font_size, 20.0);
        assert_eq!(invalid.font_weight.as_deref(), Some("700"));
        assert_eq!(invalid.font_style.as_deref(), Some("italic"));

        let inherited = super::class_effective_text_style(
            &base,
            "font-family:Other;font-family:unset;\
             font-size:20px;font-size:inherit;\
             font-weight:700;font-weight:unset;\
             font-style:italic;font-style:inherit",
        );
        assert!(matches!(inherited, std::borrow::Cow::Borrowed(_)));
    }

    #[test]
    fn class_html_row_relative_fonts_are_applied_once() {
        struct FontSizeMeasurer;
        impl TextMeasurer for FontSizeMeasurer {
            fn measure(&self, _text: &str, style: &TextStyle) -> crate::text::TextMetrics {
                crate::text::TextMetrics {
                    width: style.font_size,
                    height: style.font_size * 1.5,
                    line_count: 1,
                }
            }
            fn measure_wrapped(
                &self,
                text: &str,
                style: &TextStyle,
                _max_width: Option<f64>,
                _wrap: WrapMode,
            ) -> crate::text::TextMetrics {
                self.measure(text, style)
            }
        }
        let parsed = MermaidEngine::new()
            .parse_diagram_for_render_model_sync(
                "classDiagram\nclass Vehicle {\n+owner\n+drive()\n}",
                ParseOptions::default(),
            )
            .unwrap()
            .unwrap()
            .into_presentation();
        let RenderSemanticModel::Class(model) = parsed.model() else {
            panic!("expected Class model");
        };
        let mut node = model.class("Vehicle").expect("Vehicle class").node.clone();
        node.members[0].css_style = "font-size:2em".to_owned();
        node.methods[0].css_style = "font-size:150%".to_owned();
        let config = merman_model::presentation::PresentationConfig::default();
        let style = TextStyle {
            font_size: 24.0,
            ..default_style()
        };
        let context = super::ClassBoxMeasureCtx {
            measurer: &FontSizeMeasurer,
            mermaid_config: &config,
            math_renderer: None,
            text_style: &style,
            code_text_style: &style,
            native_title_style: None,
            html_calc_text_style: &style,
            wrap_probe_font_size: 16.0,
            wrap_mode: WrapMode::HtmlLike,
            padding: 12.0,
            hide_empty_members_box: false,
            capture_row_metrics: true,
        };
        let (_, _, plan, _) = super::class_box_dimensions(&node, &context);
        let Some(super::ClassNodeLabelPlan::PreparedHtml(labels)) = plan else {
            panic!("expected prepared HTML labels");
        };
        assert_eq!(labels.members[0].metrics.width, 48.0);
        assert_eq!(labels.members[0].metrics.height, 72.0);
        assert_eq!(labels.methods[0].metrics.width, 36.0);
        assert_eq!(labels.methods[0].metrics.height, 54.0);
    }

    #[test]
    fn class_raw_code_and_anchor_metrics_measure_the_rendered_dom() {
        let measurer = DeterministicTextMeasurer::default();
        let style = default_style();
        let source = "<a href='https://example.com'><code>Entity</code></a>";
        let fragment = crate::text::mermaid_markdown_to_xhtml_label_fragment(source, true);
        let expected = crate::text::measure_html_with_inline_styles(
            &measurer,
            &fragment,
            &style,
            Some(500.0),
            WrapMode::HtmlLike,
        );

        let actual = super::class_html_measure_label_metrics(&measurer, &style, source, 500, "");
        let literal = measurer.measure_wrapped(source, &style, Some(500.0), WrapMode::HtmlLike);

        assert_eq!(actual.width, expected.width);
        assert_eq!(actual.height, expected.height);
        assert_eq!(actual.line_count, expected.line_count);
        assert!(
            actual.width < literal.width,
            "actual={actual:?}, literal={literal:?}"
        );
    }

    #[test]
    fn class_plain_underscore_label_preserves_host_precision() {
        let metrics = super::class_html_measure_label_metrics(
            &ClassPrecisionMeasurer,
            &default_style(),
            "driver_license",
            200,
            "",
        );

        assert_eq!(metrics.width, 73.123_456_789);
        assert_eq!(metrics.line_count, 1);
    }

    #[test]
    fn class_svg_plain_underscore_uses_formatted_text_bbox_precision() {
        assert_eq!(
            super::ClassSvgLabel::new("driver_license")
                .single_line_plain_width(&ClassPrecisionMeasurer, &default_style()),
            Some(73.123_456_789)
        );
    }

    #[test]
    fn class_svg_formatted_bbox_uses_semantic_markdown_analysis() {
        let style = default_style();
        for literal in [
            "driver_license",
            "*unclosed",
            "literal ` backtick",
            "plain title",
        ] {
            assert_eq!(
                super::ClassSvgLabel::new(literal)
                    .single_line_plain_width(&ClassPrecisionMeasurer, &style),
                Some(73.123_456_789),
                "literal={literal:?}"
            );
        }

        for non_plain_single_line in ["*emphasis*", "__strong__", "first<br/>second"] {
            assert_eq!(
                super::ClassSvgLabel::new(non_plain_single_line)
                    .single_line_plain_width(&ClassPrecisionMeasurer, &style),
                None,
                "label={non_plain_single_line:?}"
            );
        }
    }

    #[test]
    fn class_svg_math_title_keeps_math_renderer_width_in_box_geometry() {
        let node: super::ClassNode = serde_json::from_value(serde_json::json!({
            "id": "Formula",
            "label": "Formula",
            "text": "$$x^2$$",
            "domId": "classId-Formula-0"
        }))
        .expect("Class node");
        let config = merman_model::presentation::PresentationConfig::default();
        let text_style = default_style();
        let context = super::ClassBoxMeasureCtx {
            measurer: &ClassMathGeometryMeasurer,
            mermaid_config: &config,
            math_renderer: Some(&ClassMathGeometryRenderer),
            text_style: &text_style,
            code_text_style: &text_style,
            native_title_style: None,
            html_calc_text_style: &text_style,
            wrap_probe_font_size: 10.0,
            wrap_mode: WrapMode::SvgLike,
            padding: 8.0,
            hide_empty_members_box: true,
            capture_row_metrics: false,
        };

        let (width, _, _, _) = super::class_box_dimensions(&node, &context);

        assert_eq!(width, 139.0);
    }

    #[test]
    fn class_svg_math_note_keeps_math_renderer_width_in_note_geometry() {
        let config = merman_model::presentation::PresentationConfig::default();
        let style = default_style();

        let (width, height, metrics) = super::note_dimensions(
            "$$x^2$$",
            &ClassMathGeometryMeasurer,
            &style,
            WrapMode::SvgLike,
            8.0,
            Some(&config),
            Some(&ClassMathGeometryRenderer),
        );

        assert_eq!(metrics.width, 123.0);
        assert_eq!(metrics.height, 37.0);
        assert_eq!(width, 139.0);
        assert_eq!(height, 53.0);
    }
}

#[cfg(test)]
mod class_svg_terminal_title_rows {
    #[test]
    fn class_svg_terminal_title_literal_fallback_replaces_only_first_row() {
        let source = "&amp;gt;<br>**MM**";
        let label = super::ClassSvgTitleLabel::new(source, source);
        assert!(label.literal_first_row());
        assert_eq!(label.lines().len(), 2);
        assert_eq!(label.lines()[0][0].0, source);
        assert_eq!(label.lines()[1][0].0, "MM");
        assert_eq!(
            label.lines()[1][0].1,
            crate::text::MermaidMarkdownWordType::Strong
        );
    }
    #[test]
    fn class_svg_terminal_title_literal_bounds_use_retained_rows() {
        struct LiteralRows;
        impl crate::text::TextMeasurer for LiteralRows {
            fn measure(&self, text: &str, _: &crate::text::TextStyle) -> crate::text::TextMetrics {
                crate::text::TextMetrics {
                    width: text.chars().count() as f64,
                    height: if text.contains("<br>") { 100.0 } else { 10.0 },
                    line_count: 1,
                }
            }
            fn measure_svg_tspan_text_bbox_height_px(
                &self,
                text: &str,
                style: &crate::text::TextStyle,
            ) -> f64 {
                assert_eq!(style.font_weight.as_deref(), Some("bolder"));
                if text == "MM" { 20.0 } else { 10.0 }
            }
        }
        let source = "&amp;gt;<br>**MM**";
        let label = super::ClassSvgTitleLabel::new(source, source);
        let metrics = label.metrics(&LiteralRows, &crate::text::TextStyle::default());
        assert_eq!(metrics.line_count, 2);
        assert!((metrics.height - 37.6).abs() < 1e-9, "{metrics:?}");
    }
}
