//! Mermaid 12 Usecase paint over its prepared graph and measured labels.

use super::{
    UsecaseLabelPlan, UsecaseNodePlan, UsecasePrepared, create_text_source, package_tab_geometry,
    sanitize_source_label,
};
use base64::Engine as _;
#[cfg(feature = "layout-shoal")]
use kurbo::Vec2;
use kurbo::{Ellipse, Rect, Shape};
use merman_model::families::usecase::{
    UsecaseActorType, UsecaseAnimation, UsecaseDiagramModel, UsecaseLabelType, UsecaseNodeKind,
    UsecaseRelationshipType,
};
use merman_model::runtime::OperationContext;
use merman_render_core::{
    Bounds, Error, LayoutLabel, LayoutNode, Result,
    config::{config_f64, config_string, json_bool, value_at},
    icon_registry::IconRegistry,
    number_format::canonical_number as fmt,
    resources::OperationWorkMeter,
    root_svg::{self, RootViewportContext},
    rough_svg::{constrained_rough_edge_path, operation_randomness},
    svg_css::SvgId,
    svg_edge_parity::{
        edge_path, line_hops, markers::BaseEdgeMarkerKind, path_numbers::json_stringify_points,
    },
    svg_output::{BoundedSvgOutput, SvgOutput, cutout},
    text::{self, TextMeasurer, svg_label as label},
    theme_svg::{elevation, marker::arrow_path},
    xml::{escape_attr, escape_xml},
};
use merman_theme_contract::v2::{Color, EdgeLabelBackground, FamilyTheme, MarkerStyle};
use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::Write as _;

#[cfg(feature = "layout-shoal")]
mod canonical;
#[cfg(feature = "layout-shoal")]
pub use canonical::UsecasePaintedEdge;
#[cfg(feature = "layout-grid")]
mod grid;
#[cfg(feature = "layout-layered")]
mod layered;
mod shapes;
#[cfg(feature = "layout-shoal")]
mod sketch;
mod theme;

#[cfg(feature = "layout-shoal")]
pub type UsecaseEdgePainter<'a> =
    dyn Fn(&mut Bounds) -> Result<Option<Vec<UsecasePaintedEdge>>> + 'a;

/// Canonical geometry provided by the facade without transferring the routing backend.
#[cfg(feature = "layout-shoal")]
pub struct UsecaseCanonicalSvg<'a> {
    pub titles: &'a HashMap<String, LayoutLabel>,
    pub boundary_order: &'a [(usize, usize)],
    pub panel_captions: &'a [bool],
    pub paint_edges: &'a UsecaseEdgePainter<'a>,
}

/// Operation context for complete Usecase SVG emission.
pub struct UsecaseSvgContext<'a> {
    pub diagram_id: SvgId<'a>,
    pub viewport: RootViewportContext<'a>,
    pub work_meter: &'a OperationWorkMeter,
    pub operation: &'a OperationContext,
    pub native_theme: Option<&'a FamilyTheme>,
    pub icon_registry: Option<&'a IconRegistry>,
    #[cfg(feature = "layout-shoal")]
    pub canonical: Option<UsecaseCanonicalSvg<'a>>,
}

fn config_bool(config: &serde_json::Value, path: &[&str]) -> Option<bool> {
    value_at(config, path).and_then(json_bool)
}

fn dom_part(value: &str) -> String {
    let mut result = String::new();
    let mut replacement = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-') {
            result.push(ch);
            replacement = false;
        } else if !replacement {
            result.push('_');
            replacement = true;
        }
    }
    let result = result.trim_matches('_');
    if result.is_empty() {
        "element".into()
    } else {
        result.into()
    }
}

fn node_shape_size(node: &LayoutNode, plan: &UsecaseNodePlan) -> kurbo::Size {
    #[cfg(feature = "layout-layered")]
    if let Some(size) = plan.layered_ellipse {
        return size;
    }
    #[cfg(not(feature = "layout-layered"))]
    let _ = plan;
    kurbo::Size::new(node.width, node.height)
}

fn element_role_classes(plan: &UsecaseNodePlan, business: bool) -> String {
    format!(
        "usecase-element usecase-{}{}",
        if plan.ellipse { "ellipse" } else { "rect" },
        if business && plan.ellipse {
            " usecase-business usecase-business-shape"
        } else if business {
            " usecase-business"
        } else {
            ""
        }
    )
}

fn accessible_label(label: &UsecaseLabelPlan) -> String {
    accessible_text(&label.text, label.label_type)
}

fn accessible_text(text: &str, kind: UsecaseLabelType) -> String {
    match kind {
        UsecaseLabelType::Text => text.to_owned(),
        UsecaseLabelType::Markdown => text::mermaid_markdown_to_lines(text, true)
            .iter()
            .map(|line| {
                line.iter()
                    .map(|word| word.0.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

#[derive(Clone, Copy)]
struct UsecaseLabelRenderContext<'a> {
    config: &'a merman_model::presentation::PresentationConfig,
    measurer: &'a dyn TextMeasurer,
    inline_svg_font: bool,
}

enum UsecaseLabelKind<'a> {
    Node(&'a str),
    PositionedNode(&'a str),
    CenteredNode(&'a str),
    Boundary {
        canonical: bool,
    },
    Edge {
        id: &'a str,
        panel_override: bool,
        plate: Option<Color>,
    },
}

fn write_label(
    out: &mut impl SvgOutput,
    plan: &UsecaseLabelPlan,
    kind: UsecaseLabelKind<'_>,
    center_x: f64,
    center_y: f64,
    context: UsecaseLabelRenderContext<'_>,
) {
    let UsecaseLabelRenderContext {
        config,
        measurer,
        inline_svg_font,
    } = context;
    let html = config_bool(config.as_value(), &["htmlLabels"]).unwrap_or(true);
    let font_family = plan.style.font_family.as_deref().unwrap_or("sans-serif");
    let font_weight = plan.style.font_weight.as_deref().unwrap_or("normal");
    let mut css: String = plan
        .styles
        .iter()
        .map(|(key, value)| {
            let key = if !html && key == "color" {
                "fill"
            } else {
                key.as_str()
            };
            format!(
                "{key}:{} !important;",
                value.trim_end_matches("!important").trim()
            )
        })
        .collect();
    if inline_svg_font && !html {
        // Raster consumers without CSS custom properties must paint the font measured by layout.
        // Append resolved values after any relative source declarations.
        let font_style = plan.style.font_style.as_deref().unwrap_or("normal");
        let _ = write!(
            css,
            "font-family:{font_family} !important;font-size:{}px !important;font-weight:{font_weight} !important;font-style:{font_style} !important;",
            fmt(plan.style.font_size)
        );
    }
    let (is_edge_label, panel_override) = match kind {
        UsecaseLabelKind::Edge { panel_override, .. } => (true, panel_override),
        _ => (false, false),
    };
    let is_boundary = matches!(kind, UsecaseLabelKind::Boundary { .. });
    let canonical_boundary = matches!(kind, UsecaseLabelKind::Boundary { canonical: true });
    let positioned_node = matches!(kind, UsecaseLabelKind::PositionedNode(_));
    let center_text = is_edge_label || matches!(kind, UsecaseLabelKind::CenteredNode(_));
    let svg_bbox_y = if !html && (center_text || positioned_node || canonical_boundary) {
        let measure_bbox_y =
            || measurer.measure_svg_create_text_bbox_y_offset_px(&plan.text, &plan.style);
        #[cfg(feature = "layout-layered")]
        let measure_bbox_y = || {
            plan.layered_svg
                .as_ref()
                .map_or_else(measure_bbox_y, |label| label.bbox_y)
        };
        measure_bbox_y()
    } else {
        0.0
    };
    let class_attr = match kind {
        UsecaseLabelKind::Node(class)
        | UsecaseLabelKind::PositionedNode(class)
        | UsecaseLabelKind::CenteredNode(class) => class,
        UsecaseLabelKind::Boundary { .. } => "cluster-label system-boundary-title",
        UsecaseLabelKind::Edge { .. } => "label",
    };
    if let UsecaseLabelKind::Edge { id, plate, .. } = kind {
        let class = if panel_override {
            "edgeLabel usecase-panel-caption"
        } else {
            "edgeLabel"
        };
        let _ = write!(
            out,
            r#"<g class="{class}" transform="translate({},{})">"#,
            fmt(center_x),
            fmt(center_y),
        );
        if let Some(plate) = plate {
            let _ = write!(
                out,
                r#"<rect class="merman-edge-label-background" x="{}" y="{}" width="{}" height="{}" rx="{}" fill="{plate}"/>"#,
                fmt(-plan.metrics.width / 2.0),
                fmt(-plan.metrics.height / 2.0),
                fmt(plan.metrics.width),
                fmt(plan.metrics.height),
                fmt(plan.metrics.height / 2.0),
            );
        }
        let _ = write!(
            out,
            r#"<g class="label" data-id="{}" transform="translate({}, {})">"#,
            escape_attr(id),
            fmt(if html { -plan.metrics.width / 2.0 } else { 0.0 }),
            // insertEdgeLabel centers the SVG text bbox, including its local Y offset.
            fmt(-svg_bbox_y - plan.text_metrics.height / 2.0),
        );
    } else if html {
        let _ = write!(
            out,
            r#"<g class="{}" transform="translate({},{})" style="font-family:{};font-size:{}px;font-weight:{}">"#,
            escape_attr(class_attr),
            fmt(center_x - plan.metrics.width / 2.0),
            fmt(center_y - plan.metrics.height / 2.0),
            escape_attr(font_family),
            fmt(plan.style.font_size),
            escape_attr(font_weight)
        );
    } else {
        // Plain labelHelper centers only Y; custom shapes reposition the whole local bbox.
        let x = if is_boundary || positioned_node {
            center_x - plan.metrics.width / 2.0
        } else {
            center_x
        };
        let _ = write!(
            out,
            r#"<g class="{}" transform="translate({},{})" style="{}">"#,
            escape_attr(class_attr),
            fmt(x),
            fmt(center_y - plan.metrics.height / 2.0 - svg_bbox_y),
            escape_attr(&css),
        );
    }
    if html {
        let content = plan.html.as_ref().map_or("", |label| label.xhtml.as_str());
        if !is_edge_label && !is_boundary {
            out.push_str("<rect/>");
        }
        let _ = write!(
            out,
            r#"<foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml"{} style="{}display:inline-block;white-space:normal;line-height:1.5;text-align:center;width:{}px"><span class="{}" style="{}">{}</span></div></foreignObject>"#,
            fmt(plan.metrics.width),
            fmt(plan.metrics.height),
            if is_edge_label {
                r#" class="labelBkg""#
            } else {
                ""
            },
            escape_attr(&css),
            fmt(plan.metrics.width),
            if is_edge_label {
                "edgeLabel"
            } else {
                "nodeLabel"
            },
            escape_attr(&css),
            content
        );
    } else {
        if !is_edge_label && !is_boundary {
            out.push_str("<rect/>");
        }
        out.push_str("<g>");
        if is_edge_label && !plan.text.is_empty() {
            let background_style = if panel_override { "fill:none" } else { "" };
            let _ = write!(
                out,
                r#"<rect class="background" style="{background_style}" x="{}" y="{}" width="{}" height="{}"/>"#,
                fmt(-plan.text_metrics.width / 2.0 - 2.0),
                fmt(svg_bbox_y - 2.0),
                fmt(plan.metrics.width),
                fmt(plan.metrics.height),
            );
        } else {
            out.push_str(r#"<rect class="background" style="stroke: none"/>"#);
        }
        // Usecase plain labels are literal text, including strings that resemble HTML.
        match plan.label_type {
            UsecaseLabelType::Text => {
                // prepareUsecaseLayoutData escapes plain labels before createText splits
                // lines and words. Decode those entities only when writing text content,
                // so a literal <br/> remains visible instead of becoming a line break.
                #[cfg(feature = "layout-layered")]
                let prepared_rows = plan.layered_svg.as_ref().map(|label| &label.rows);
                #[cfg(not(feature = "layout-layered"))]
                let prepared_rows: Option<&Vec<Vec<String>>> = None;
                if let Some(rows) = prepared_rows {
                    label::write_svg_text_source_word_lines_with_style(
                        out,
                        rows,
                        &css,
                        center_text,
                        None,
                    );
                } else if canonical_boundary {
                    let source = create_text_source(&plan.text, plan.label_type);
                    let words = text::DeterministicTextMeasurer::normalized_text_lines(&source)
                        .iter()
                        .map(|line| {
                            text::non_markdown_svg_words(line)
                                .map(str::to_owned)
                                .collect()
                        })
                        .collect::<Vec<_>>();
                    label::write_measured_svg_title(out, &words, &css, None, plan.metrics.width);
                } else {
                    label::write_svg_text_from_create_text_source_with_style(
                        out,
                        &create_text_source(&plan.text, plan.label_type),
                        &css,
                        center_text,
                    );
                }
            }
            UsecaseLabelType::Markdown => {
                label::write_svg_text_markdown_wrapped_from_create_text_source_with_style(
                    out,
                    &plan.text,
                    &css,
                    measurer,
                    &plan.style,
                    plan.max_width,
                    center_text,
                );
            }
        }
        out.push_str("</g>");
    }
    out.push_str("</g>");
    if is_edge_label {
        out.push_str("</g>");
    }
}

fn exposed_caption_plate(
    label: &UsecaseLabelPlan,
    binding: &super::UsecaseCssBinding,
    theme: &FamilyTheme,
) -> Option<Color> {
    let paint = label
        .styles
        .get("color")
        .or_else(|| label.styles.get("fill"));
    let ink = paint.map_or(binding.text.css(), |value| {
        value.trim_end_matches("!important")
    });
    let [r, g, b, a] = merman_model::css_color::parse(ink.trim())?.rgba.to_rgba8();
    let ink = Color::rgba(r, g, b, a);
    let contrast = |background| ink.composite(background, 1.0).contrast(background);
    let light = Color::rgb(255, 255, 255);
    if a == 0 || contrast(light) >= 4.5 {
        return None;
    }
    let [r, g, b, _] = theme.roles.canvas.first_color().channels();
    let preferred = Color::rgb(r, g, b);
    let dark = Color::rgb(0, 0, 0);
    Some(if contrast(preferred) >= 4.5 {
        preferred
    } else if contrast(dark) > contrast(light) {
        dark
    } else {
        light
    })
}

fn write_edge_label_mask<'a>(
    out: &mut impl SvgOutput,
    diagram_id: SvgId<'_>,
    bounds: root_svg::DiagramBounds,
    positions: impl Iterator<Item = &'a LayoutLabel>,
    y_offset: f64,
) -> Result<bool> {
    let mut positions = positions.peekable();
    if positions.peek().is_none() {
        return Ok(false);
    }
    let _ = write!(
        out,
        r#"<defs><mask id="{diagram_id}_usecase-edge-label-mask" maskUnits="userSpaceOnUse" maskContentUnits="userSpaceOnUse" x="{}" y="{}" width="{}" height="{}"><rect x="{}" y="{}" width="{}" height="{}" fill="white"/>"#,
        fmt(bounds.min_x),
        fmt(bounds.min_y),
        fmt(bounds.width),
        fmt(bounds.height),
        fmt(bounds.min_x),
        fmt(bounds.min_y),
        fmt(bounds.width),
        fmt(bounds.height),
    );
    // Opaque black rectangles subtract the union of the label boxes, including overlaps.
    for position in positions {
        let rect = Rect::from_center_size(
            (position.x, position.y + y_offset),
            (position.width, position.height),
        );
        let _ = write!(
            out,
            r#"<rect x="{}" y="{}" width="{}" height="{}" fill="black"/>"#,
            fmt(rect.x0),
            fmt(rect.y0),
            fmt(rect.width()),
            fmt(rect.height()),
        );
        out.checkpoint()?;
    }
    out.push_str("</mask></defs>");
    out.checkpoint()?;
    Ok(true)
}

fn write_legacy_base_markers(
    out: &mut impl SvgOutput,
    diagram_id: SvgId<'_>,
    style: MarkerStyle,
) -> Result<()> {
    use BaseEdgeMarkerKind::*;

    for kind in [
        PointEnd,
        PointStart,
        PointEndMargin,
        PointStartMargin,
        CircleEnd,
        CircleStart,
        CircleEndMargin,
        CircleStartMargin,
        CrossEnd,
        CrossStart,
        CrossEndMargin,
        CrossStartMargin,
    ] {
        write_base_marker(out, diagram_id, style, kind, kind.suffix(), None);
    }
    out.checkpoint()
}

fn write_base_marker(
    out: &mut impl SvgOutput,
    diagram_id: SvgId<'_>,
    style: MarkerStyle,
    kind: BaseEdgeMarkerKind,
    suffix: &str,
    paint: Option<&str>,
) {
    use BaseEdgeMarkerKind::*;

    let point = kind.suffix().starts_with("point");
    if let Some(path) = arrow_path(style).filter(|_| point) {
        let transform = if matches!(kind, PointStart | PointStartMargin) {
            r#" transform="translate(10 0) scale(-1 1)""#
        } else {
            ""
        };
        let fill = if style == MarkerStyle::Open {
            "fill:none;"
        } else {
            ""
        };
        let _ = write!(
            out,
            r#"<marker id="{diagram_id}_usecase-{suffix}" class="marker usecase" viewBox="0 0 10 10" refX="5" refY="5" markerWidth="8" markerHeight="8" markerUnits="userSpaceOnUse" orient="auto" overflow="visible"><path d="{path}"{transform} style="stroke-width:1;stroke-dasharray:none;stroke-linecap:round;stroke-linejoin:round;{fill}""#,
        );
    } else {
        let cross = if kind.cross() { "cross " } else { "" };
        let _ = write!(
            out,
            r#"<marker id="{diagram_id}_usecase-{suffix}" class="marker {cross}usecase"{}"#,
            kind.shape_fragment(),
        );
    }
    if kind.geometry().shape != "polygon" || (point && style != MarkerStyle::Classic) {
        write_marker_paint(out, paint, point && style != MarkerStyle::Open);
    }
    out.push_str("/></marker>");
}

fn write_marker_paint(out: &mut impl SvgOutput, paint: Option<&str>, fill: bool) {
    if let Some(paint) = paint {
        let color = escape_attr(paint);
        let _ = write!(out, r#" stroke="{color}""#);
        if fill {
            let _ = write!(out, r#" fill="{color}""#);
        }
    }
}

fn write_extension_marker(
    out: &mut impl SvgOutput,
    diagram_id: SvgId<'_>,
    suffix: &str,
    start: bool,
    margin: bool,
    paint: Option<&str>,
) {
    let _ = write!(
        out,
        r#"<marker id="{diagram_id}_usecase-{suffix}" class="marker extension usecase""#
    );
    match (start, margin) {
        (true, false) => {
            out.push_str(r#" refX="18" refY="7" markerWidth="20" markerHeight="28" orient="auto" markerUnits="userSpaceOnUse"><path d="M 1,7 L18,13 V 1 Z""#);
        }
        (false, false) => {
            out.push_str(r#" refX="1" refY="7" markerWidth="20" markerHeight="28" markerUnits="userSpaceOnUse" orient="auto"><path d="M 1,1 V 13 L18,7 Z""#);
        }
        (true, true) => {
            out.push_str(r#" refX="18" refY="7" markerWidth="20" markerHeight="28" orient="auto" markerUnits="userSpaceOnUse" viewBox="0 0 20 14"><polygon points="10,7 18,13 18,1" style="stroke-width: 2; stroke-dasharray: 0;""#);
        }
        (false, true) => {
            out.push_str(r#" refX="9" refY="7" markerWidth="20" markerHeight="28" orient="auto" markerUnits="userSpaceOnUse" viewBox="0 0 20 14"><polygon points="10,1 10,13 18,7" style="stroke-width: 2; stroke-dasharray: 0;""#);
        }
    }
    if !margin {
        write_marker_paint(out, paint, false);
    }
    out.push_str("/></marker>");
}

pub fn render_usecase_diagram_svg_model(
    prepared: &UsecasePrepared,
    model: &UsecaseDiagramModel,
    config: &merman_model::presentation::PresentationConfig,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &UsecaseSvgContext<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id;
    let cfg = config.as_value();
    let css_binding = &prepared.measured.css;
    let native = options.native_theme;
    let layout = prepared.layout();
    let geometry: HashMap<_, _> = layout
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    let plans: HashMap<_, _> = prepared
        .measured
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    let source_nodes: HashMap<_, _> = model
        .nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    let boundaries: HashMap<_, _> = model
        .boundaries
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    let notes: HashMap<_, _> = model
        .notes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    let json_nodes: HashMap<_, _> = model
        .json_nodes
        .iter()
        .map(|node| (node.id.as_str(), node))
        .collect();
    let relationships: HashMap<_, _> = model
        .relationships
        .iter()
        .map(|edge| (edge.id.as_str(), edge))
        .collect();
    let edge_geometry: HashMap<_, _> = layout
        .edges
        .iter()
        .map(|edge| (edge.id.as_str(), edge))
        .collect();
    let source_labels: HashMap<_, _> = plans
        .iter()
        .map(|(&id, plan)| {
            let kind = source_nodes
                .get(id)
                .map_or(plan.label.label_type, |node| node.label_type);
            (id, accessible_text(&plan.source_label, kind))
        })
        .collect();
    let padding = config_f64(cfg, &["usecase", "diagramPadding"]).unwrap_or(20.0);
    let bounds = layout.bounds.clone().unwrap_or(Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 0.0,
        max_y: 0.0,
    });
    let title = model
        .title
        .as_deref()
        .or(diagram_title)
        .filter(|text| !text.is_empty());
    let title_x = (bounds.min_x + bounds.max_x) / 2.0;
    let mut viewport_bounds = bounds.clone();
    #[cfg(feature = "layout-grid")]
    let grid_edges = prepared
        .grid
        .as_ref()
        .map(|grid| {
            grid::paint_edges(
                prepared,
                grid,
                &geometry,
                &plans,
                &mut viewport_bounds,
                options.work_meter,
            )
        })
        .transpose()?;
    #[cfg(feature = "layout-grid")]
    let uses_grid = grid_edges.is_some();
    #[cfg(not(feature = "layout-grid"))]
    let uses_grid = false;
    #[cfg(feature = "layout-layered")]
    let layered_routes = prepared.layered.as_ref().and_then(|paint| match paint {
        super::layered::Paint::Axes(routes) => Some(routes),
        #[cfg(feature = "layout-dagre")]
        super::layered::Paint::DagreFallback => None,
    });
    #[cfg(feature = "layout-layered")]
    let layered_edges = layered_routes
        .map(|routes| {
            layered::paint_edges(
                prepared,
                routes,
                cfg,
                &geometry,
                &plans,
                &mut viewport_bounds,
                options,
            )
        })
        .transpose()?;
    #[cfg(feature = "layout-layered")]
    let (uses_layered, label_y_offset) = (
        layered_edges.is_some(),
        layered_routes.map_or(0.0, |routes| routes.label_y_offset),
    );
    #[cfg(not(feature = "layout-layered"))]
    let (uses_layered, label_y_offset) = (false, 0.0);
    let elevation = native
        .map(|theme| &theme.elevation)
        .filter(|elevation| **elevation != merman_theme_contract::v2::Elevation::Flat);
    let mut depth = Vec::new();
    if let Some(elevation) = elevation {
        for (index, plan) in prepared.measured.nodes.iter().enumerate() {
            let Some(source) = source_nodes.get(plan.id.as_str()) else {
                continue;
            };
            if source.kind == UsecaseNodeKind::Actor {
                continue;
            }
            let node = geometry[plan.id.as_str()];
            let size = node_shape_size(node, plan);
            let [width, height] = [size.width, size.height];
            let rect = Rect::new(-width / 2.0, -height / 2.0, width / 2.0, height / 2.0);
            let silhouette = if plan.ellipse {
                Ellipse::from_rect(rect).to_path(0.1)
            } else {
                rect.to_path(0.1)
            };
            let bounds = elevation::bounds(elevation, rect) + kurbo::Vec2::new(node.x, node.y);
            viewport_bounds.min_x = viewport_bounds.min_x.min(bounds.x0);
            viewport_bounds.min_y = viewport_bounds.min_y.min(bounds.y0);
            viewport_bounds.max_x = viewport_bounds.max_x.max(bounds.x1);
            viewport_bounds.max_y = viewport_bounds.max_y.max(bounds.y1);
            depth.push((index, silhouette, (node.x, node.y)));
        }
    }
    #[cfg(feature = "layout-shoal")]
    let mut canonical_edges = match &options.canonical {
        Some(canonical) => (canonical.paint_edges)(&mut viewport_bounds)?.map(Vec::into_iter),
        None => None,
    };
    #[cfg(feature = "layout-shoal")]
    if canonical_edges
        .as_ref()
        .is_some_and(|edges| edges.len() != prepared.measured.edges.len())
    {
        return Err(Error::InvalidModel {
            message: "canonical Usecase paint does not match the prepared edges".into(),
        });
    }
    #[cfg(feature = "layout-shoal")]
    let uses_canonical = canonical_edges.is_some();
    #[cfg(not(feature = "layout-shoal"))]
    let uses_canonical = false;
    #[cfg(feature = "layout-shoal")]
    let sketch_shapes =
        (uses_canonical && css_binding.look == "handDrawn").then(|| sketch::UsecaseSketch {
            work_meter: options.work_meter,
            randomness: operation_randomness(
                options.operation,
                css_binding.hand_drawn_seed,
                "usecase-shape",
            ),
            bounds: Default::default(),
        });
    let labels = UsecaseLabelRenderContext {
        config,
        measurer,
        inline_svg_font: uses_canonical || native.is_some(),
    };
    if let Some(title) = title {
        let style = css_binding
            .native_title_font
            .as_ref()
            .unwrap_or(&css_binding.root_font);
        let (left, right) = measurer.measure_svg_title_bbox_x(title, style);
        let (ascent, descent) = text::svg_text_vertical_extents_px(
            measurer,
            title,
            style,
            text::svg_title_bbox_vertical_extents_px(style),
        );
        viewport_bounds.min_x = viewport_bounds.min_x.min(title_x - left);
        viewport_bounds.max_x = viewport_bounds.max_x.max(title_x + right);
        viewport_bounds.min_y = viewport_bounds.min_y.min(-ascent);
        viewport_bounds.max_y = viewport_bounds.max_y.max(descent);
    }
    let viewport = root_svg::DiagramBounds::from_extents(
        viewport_bounds.min_x,
        viewport_bounds.min_y,
        viewport_bounds.max_x,
        viewport_bounds.max_y,
        padding,
    );
    let use_max_width = config_bool(cfg, &["usecase", "useMaxWidth"]).unwrap_or(true);
    let spec = root_svg::RootViewportSpec::mermaid(viewport, use_max_width);
    let acc_title_id = model
        .acc_title
        .as_ref()
        .map(|_| format!("chart-title-{diagram_id}"));
    let acc_descr_id = model
        .acc_description
        .as_ref()
        .map(|_| format!("chart-desc-{diagram_id}"));
    let mut chrome = root_svg::RootChrome::new(diagram_id, "usecase");
    chrome.class = Some("usecaseDiagram");
    chrome.aria_labelledby = acc_title_id.as_deref();
    chrome.aria_describedby = acc_descr_id.as_deref();
    #[cfg(all(feature = "layout-layered", feature = "layout-dagre"))]
    if matches!(
        prepared.layered.as_ref(),
        Some(super::layered::Paint::DagreFallback)
    ) {
        chrome.extra_attrs = &[("data-layout-fallback", "dagre")];
    }
    let actor_font = &css_binding.actor_font;
    let usecase_font = &css_binding.usecase_font;
    let actor_font_size = format!("{}px", fmt(actor_font.font_size));
    let usecase_font_size = format!("{}px", fmt(usecase_font.font_size));
    let font_properties = [
        (
            "--mermaid-usecase-actor-font-size",
            actor_font_size.as_str(),
        ),
        (
            "--mermaid-usecase-actor-font-family",
            actor_font.font_family.as_deref().unwrap_or("sans-serif"),
        ),
        (
            "--mermaid-usecase-actor-font-weight",
            actor_font.font_weight.as_deref().unwrap_or("normal"),
        ),
        ("--mermaid-usecase-font-size", usecase_font_size.as_str()),
        (
            "--mermaid-usecase-font-family",
            usecase_font.font_family.as_deref().unwrap_or("sans-serif"),
        ),
        (
            "--mermaid-usecase-font-weight",
            usecase_font.font_weight.as_deref().unwrap_or("normal"),
        ),
    ];
    chrome.custom_properties = &font_properties;
    let mut out = BoundedSvgOutput::new(options.work_meter);
    #[cfg(feature = "layout-shoal")]
    let document = if sketch_shapes.is_some() {
        options.viewport.begin_document(
            &mut out,
            root_svg::DeferredRootSpec::mermaid(use_max_width),
            chrome,
        )?
    } else {
        options.viewport.write_open(&mut out, spec, chrome)?
    };
    #[cfg(not(feature = "layout-shoal"))]
    let document = options.viewport.write_open(&mut out, spec, chrome)?;
    if let Some(title) = &model.acc_title {
        let _ = write!(
            out,
            r#"<title id="chart-title-{diagram_id}">{}</title>"#,
            escape_xml(title)
        );
    }
    if let Some(description) = &model.acc_description {
        let _ = write!(
            out,
            r#"<desc id="chart-desc-{diagram_id}">{}</desc>"#,
            escape_xml(description)
        );
    }
    theme::write_css(
        &mut out,
        diagram_id,
        css_binding,
        config,
        uses_canonical,
        uses_grid && native.is_none(),
    )?;
    // Mermaid emits marker wrappers that are observable in strict SVG DOM parity.
    out.push_str("<g>");
    if !uses_canonical {
        write_legacy_base_markers(&mut out, diagram_id, css_binding.marker_style)?;
        out.push_str("<defs>");
        write_extension_marker(&mut out, diagram_id, "extensionStart", true, false, None);
        out.push_str("</defs><defs>");
        write_extension_marker(&mut out, diagram_id, "extensionEnd", false, false, None);
        out.push_str("</defs>");
        write_extension_marker(
            &mut out,
            diagram_id,
            "extensionStart-margin",
            true,
            true,
            None,
        );
        out.push_str("<defs>");
        write_extension_marker(
            &mut out,
            diagram_id,
            "extensionEnd-margin",
            false,
            true,
            None,
        );
        out.push_str("</defs>");
    }
    #[cfg(feature = "layout-layered")]
    let layered_markers = uses_layered
        .then(|| {
            layered::write_marker_variants(
                &mut out,
                diagram_id,
                css_binding,
                &prepared.measured.edges,
                options.work_meter,
            )
        })
        .transpose()?;
    out.push_str(r#"<g class="root"><g class="clusters">"#);
    let mut legacy_boundaries = prepared
        .measured
        .nodes
        .iter()
        .filter(|plan| plan.is_boundary)
        .enumerate();
    #[cfg(feature = "layout-shoal")]
    let mut canonical_boundaries = options
        .canonical
        .as_ref()
        .map(|canonical| canonical.boundary_order.iter());
    let boundary_plans = std::iter::from_fn(|| {
        #[cfg(feature = "layout-shoal")]
        if let Some(order) = &mut canonical_boundaries {
            return order
                .next()
                .map(|&(color, plan)| (color, &prepared.measured.nodes[plan]));
        }
        legacy_boundaries.next()
    });
    for (color_index, plan) in boundary_plans {
        out.checkpoint()?;
        let node = geometry[plan.id.as_str()];
        let boundary = boundaries[plan.id.as_str()];
        let kind = if plan.package { "package" } else { "rect" };
        let accessible = if uses_layered {
            format!(
                "{kind} system boundary {}",
                accessible_text(&plan.source_label, boundary.label_type)
            )
        } else {
            format!("{kind} system boundary {}", accessible_label(&plan.label))
        };
        let appearance = theme::appearance_attributes(css_binding, Some(color_index));
        let canonical_attributes = if uses_canonical {
            format!(r#" data-et="cluster" data-id="{}""#, escape_attr(&plan.id))
        } else {
            String::new()
        };
        let _ = write!(
            out,
            r#"<g id="{}-usecase-{}" class="cluster usecase-system-boundary usecase-system-boundary-{kind} default system-boundary system-boundary-{kind} {}" data-boundary-type="{kind}"{appearance} data-usecase-id="{}" data-usecase-kind="boundary" role="img" aria-label="{}"{canonical_attributes}>"#,
            dom_part(diagram_id.semantic_str()),
            dom_part(&plan.id),
            escape_attr(&boundary.classes.join(" ")),
            escape_attr(&plan.id),
            escape_attr(&accessible)
        );
        #[cfg(feature = "layout-shoal")]
        let placed_title = options
            .canonical
            .as_ref()
            .and_then(|canonical| canonical.titles.get(&plan.id));
        #[cfg(not(feature = "layout-shoal"))]
        let placed_title: Option<&LayoutLabel> = None;
        #[cfg(feature = "layout-layered")]
        let height = if uses_layered {
            layered::boundary_height(node, plan)
        } else {
            node.height
        };
        #[cfg(not(feature = "layout-layered"))]
        let height = node.height;
        let left = node.x - node.width / 2.0;
        let top = node.y - height / 2.0;
        let (tab_left, tab_width, tab_height) = if plan.package {
            package_tab_geometry(left, top, node.width, &plan.label, placed_title)
        } else {
            (left, 0.0, 0.0)
        };
        let write_tab = |out: &mut BoundedSvgOutput<'_>| -> Result<()> {
            if plan.package {
                shapes::write_box(
                    out,
                    "boundary-tab system-boundary-package-tab label-container",
                    (tab_left, top),
                    (tab_width, tab_height),
                    &plan.source_style,
                    #[cfg(feature = "layout-shoal")]
                    sketch_shapes.as_ref(),
                    #[cfg(feature = "layout-shoal")]
                    Vec2::ZERO,
                )?;
            }
            Ok(())
        };
        if !uses_canonical {
            write_tab(&mut out)?;
        }
        let body_inset = if uses_canonical { 0.0 } else { tab_height };
        shapes::write_box(
            &mut out,
            "boundary-body label-container",
            (left, top + body_inset),
            (node.width, height - body_inset),
            &plan.source_style,
            #[cfg(feature = "layout-shoal")]
            sketch_shapes.as_ref(),
            #[cfg(feature = "layout-shoal")]
            Vec2::ZERO,
        )?;
        if uses_canonical {
            write_tab(&mut out)?;
        }
        let title_x = placed_title.map_or_else(
            || {
                if plan.package {
                    left + tab_width / 2.0
                } else {
                    node.x
                }
            },
            |title| title.x,
        );
        let title_y = placed_title.map_or_else(
            || {
                top + if plan.package {
                    tab_height / 2.0
                } else {
                    plan.label.metrics.height / 2.0
                        + config_f64(cfg, &["flowchart", "subGraphTitleMargin", "top"])
                            .unwrap_or(0.0)
                }
            },
            |title| title.y,
        );
        write_label(
            &mut out,
            &plan.label,
            UsecaseLabelKind::Boundary {
                canonical: uses_canonical,
            },
            title_x,
            title_y,
            labels,
        );
        out.push_str("</g>");
    }
    out.checkpoint()?;
    let elk = prepared.elk;
    let sketch = (!uses_canonical
        && !uses_layered
        && (css_binding.sketches_edges() || (uses_grid && css_binding.edge_look == "handDrawn")))
        .then(|| {
            operation_randomness(
                options.operation,
                css_binding.hand_drawn_seed,
                "usecase-edge",
            )
        });
    let hop_points =
        if elk && sketch.is_none() && config_bool(cfg, &["elk", "lineHops"]) != Some(false) {
            let clone_work = prepared.measured.edges.iter().fold(0usize, |total, plan| {
                total.saturating_add(
                    edge_geometry[plan.id.as_str()]
                        .points
                        .len()
                        .saturating_add(3),
                )
            });
            options.work_meter.charge(clone_work)?;
            prepared
                .measured
                .edges
                .iter()
                .map(|plan| prepared.edge_points(edge_geometry[plan.id.as_str()]))
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
    let elk_curve = |edge: &merman_render_core::LayoutEdge| {
        css_binding
            .native_curve()
            .unwrap_or(if edge.points.is_empty() {
                "linear"
            } else {
                "rounded"
            })
    };
    let hop_edges = prepared
        .measured
        .edges
        .iter()
        .zip(&hop_points)
        .map(|(plan, points)| line_hops::LineHopEdge {
            id: plan.id.as_str(),
            points,
            curve: Some(elk_curve(edge_geometry[plan.id.as_str()])),
            arrow_type_start: plan.start_marker.as_deref(),
            arrow_type_end: plan.end_marker.as_deref(),
        })
        .collect::<Vec<_>>();
    let hopped_paths = line_hops::elk_line_hop_paths(cfg, &hop_edges, options.work_meter)?;
    out.push_str("</g>");
    if let Some(elevation) = elevation {
        for (index, silhouette, center) in depth {
            let id = format!("{diagram_id}-usecase-{index}");
            let _ = elevation::write_depth(&mut out, elevation, &id, &silhouette, center);
            out.checkpoint()?;
        }
    }
    let transparent = cutout::is_transparent_paint(css_binding.edge_background.css());
    // Mermaid's derived background follows the same automatic panel policy.
    let auto_background = uses_canonical
        && native.is_none_or(|theme| theme.form.edge_label_bg == EdgeLabelBackground::Auto)
        && !config.explicit_config_owns_path("themeVariables.edgeLabelBackground");
    #[cfg(feature = "layout-shoal")]
    let panel_captions = options
        .canonical
        .as_ref()
        .filter(|_| auto_background)
        .map_or(&[][..], |canonical| canonical.panel_captions);
    #[cfg(not(feature = "layout-shoal"))]
    let panel_captions: &[bool] = &[];
    let panel_background_override =
        |index: usize| panel_captions.get(index).copied().unwrap_or(false);
    let transparent_auto_canvas = native.filter(|theme| {
        auto_background
            && transparent
            && css_binding
                .canvas_background
                .unwrap_or_else(|| theme.roles.canvas.first_color())
                .channels()[3]
                == 0
    });
    let has_edge_mask = !uses_canonical
        && transparent
        && write_edge_label_mask(
            &mut out,
            diagram_id,
            viewport,
            layout.edges.iter().filter_map(|edge| {
                edge.label
                    .as_ref()
                    .filter(|label| label.width > 0.0 && label.height > 0.0)
            }),
            label_y_offset,
        )?;
    let edge_layer_class = if uses_grid || uses_layered {
        "edges edgePaths"
    } else {
        "edgePaths edges"
    };
    if has_edge_mask {
        let _ = write!(
            out,
            r#"<g class="{edge_layer_class}" mask="url(#{diagram_id}_usecase-edge-label-mask)">"#
        );
    } else {
        let _ = write!(out, r#"<g class="{edge_layer_class}">"#);
    }
    for (edge_index, plan) in prepared.measured.edges.iter().enumerate() {
        #[cfg(not(any(
            feature = "layout-shoal",
            feature = "layout-grid",
            feature = "layout-layered"
        )))]
        let _ = edge_index;
        out.checkpoint()?;
        let edge = edge_geometry[plan.id.as_str()];
        let data_look = &css_binding.edge_look;
        #[cfg(feature = "layout-shoal")]
        let canonical_edge = canonical_edges.as_mut().and_then(Iterator::next);
        #[cfg(feature = "layout-grid")]
        let grid_edge = grid_edges.as_ref().map(|edges| &edges[edge_index]);
        #[cfg(feature = "layout-layered")]
        let layered_edge = layered_edges.as_ref().map(|edges| &edges[edge_index]);
        let legacy = || -> (Cow<'_, str>, Cow<'_, str>) {
            #[cfg(feature = "layout-layered")]
            if let Some(paint) = layered_edge {
                return (
                    Cow::Borrowed(paint.json.as_str()),
                    Cow::Borrowed(paint.path.as_str()),
                );
            }
            #[cfg(feature = "layout-grid")]
            if let Some(paint) = grid_edge {
                return (
                    Cow::Owned(json_stringify_points(&paint.points)),
                    Cow::Borrowed(paint.path.as_str()),
                );
            }
            let mut points = prepared.edge_points(edge);
            let curve = if elk {
                elk_curve(edge)
            } else {
                css_binding.native_curve().unwrap_or_else(|| {
                    cfg.pointer("/flowchart/curve")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("basis")
                })
            };
            let data_points = json_stringify_points(&points);
            let path = edge_path::render_path(
                &mut points,
                curve,
                plan.start_marker.as_deref(),
                plan.end_marker.as_deref(),
            );
            let path = match hopped_paths.get(plan.id.as_str()) {
                Some(hop) => Cow::Borrowed(hop.as_str()),
                None => Cow::Owned(path),
            };
            (Cow::Owned(data_points), path)
        };
        #[cfg(feature = "layout-shoal")]
        let (data_points, path) = canonical_edge.as_ref().map_or_else(legacy, |painted| {
            (
                Cow::Owned(json_stringify_points(&painted.points)),
                Cow::Borrowed(painted.path.as_str()),
            )
        });
        #[cfg(not(feature = "layout-shoal"))]
        let (data_points, path) = legacy();
        let rough = if let Some(randomness) = sketch.as_ref().filter(|_| !path.is_empty()) {
            Some(constrained_rough_edge_path(
                &path,
                Default::default(),
                options.work_meter,
                randomness,
            )?)
        } else {
            None
        };
        let path = rough.as_deref().unwrap_or(&path);
        let data_points = base64::engine::general_purpose::STANDARD.encode(data_points.as_bytes());
        let relation = relationships
            .get(plan.original_id.as_deref().unwrap_or(&plan.id))
            .copied();
        let label = relation.map(|edge| match edge.relationship_type {
            UsecaseRelationshipType::Association => {
                let label = if uses_grid {
                    edge.label
                        .as_deref()
                        .filter(|label| !label.is_empty())
                        .map(|label| {
                            let kind = edge.label_type.unwrap_or_default();
                            accessible_text(&sanitize_source_label(label, kind, config), kind)
                        })
                } else {
                    plan.label.as_ref().map(accessible_label)
                };
                label.map_or_else(
                    || "association".into(),
                    |label| format!("association {label}"),
                )
            }
            UsecaseRelationshipType::Include => "include".into(),
            UsecaseRelationshipType::Extend => "extend".into(),
            UsecaseRelationshipType::Generalization => "generalization".into(),
        });
        let relation_kind = relation.map_or("note", |edge| match edge.relationship_type {
            UsecaseRelationshipType::Association => "association",
            UsecaseRelationshipType::Include => "include",
            UsecaseRelationshipType::Extend => "extend",
            UsecaseRelationshipType::Generalization => "generalization",
        });
        let margin = if config_string(cfg, &["look"]).as_deref() == Some("neo") && !plan.animated {
            "-margin"
        } else {
            ""
        };
        let mut marker_references = Vec::new();
        for (start, marker) in [(true, &plan.start_marker), (false, &plan.end_marker)] {
            if let Some(marker) = marker {
                let end = if start { "start" } else { "end" };
                #[cfg(feature = "layout-shoal")]
                if let Some(painted) = &canonical_edge {
                    if let Some(marker) = &painted.markers[usize::from(!start)] {
                        let reference = canonical::write_marker(
                            &mut out, diagram_id, marker, start, edge_index,
                        );
                        marker_references.push((end, reference));
                    }
                    continue;
                }
                let name = marker.strip_prefix("arrow_").unwrap_or(marker);
                let side = if start { "Start" } else { "End" };
                #[cfg(feature = "layout-layered")]
                if let Some(suffix) = layered_markers
                    .as_ref()
                    .and_then(|variants| variants[edge_index][usize::from(!start)].as_deref())
                {
                    marker_references.push((end, format!("url(#{diagram_id}_usecase-{suffix})")));
                    continue;
                }
                let reference = format!("url(#{diagram_id}_usecase-{name}{side}{margin})");
                marker_references.push((end, reference));
            }
        }
        let mut classes = format!(
            "edge-thickness-normal edge-pattern-{} default relationship relationship-{relation_kind}",
            if plan.dotted { "dotted" } else { "solid" }
        );
        if uses_layered && data_look == "handDrawn" {
            classes.push_str(" transition");
        }
        if let Some(edge) = relation {
            for class in &edge.classes {
                classes.push(' ');
                classes.push_str(class);
            }
            if edge.animate || edge.animation.is_some() {
                classes.push_str(match edge.animation {
                    Some(UsecaseAnimation::Slow) => " edge-animation-slow",
                    _ => " edge-animation-fast",
                });
            }
        }
        #[cfg(feature = "layout-shoal")]
        let mut caption_cutout = None;
        #[cfg(feature = "layout-shoal")]
        if (transparent || panel_background_override(edge_index))
            && plan.label.is_some()
            && let Some((painted, label)) = canonical_edge.as_ref().zip(edge.label.as_ref())
        {
            let gap = Rect::from_center_size(
                (label.x, label.y + label_y_offset),
                (label.width, label.height),
            );
            if painted.paint_bounds.intersect(gap).area() > 0.0 {
                let id = format!("{diagram_id}_usecase-edge-label-{edge_index}");
                cutout::write_defs(
                    &mut out,
                    &id,
                    painted.paint_bounds,
                    1.0,
                    &[gap],
                    options.work_meter,
                )?;
                caption_cutout = Some(id);
            }
        }
        let _ = write!(
            out,
            r#"<path id="{}{}-{}" data-id="{}" data-et="edge" data-edge="true" data-look="{}" data-points="{}" class="{}" d="{}""#,
            if plan.dagre_recursive { "" } else { "usecase-" },
            dom_part(diagram_id.semantic_str()),
            dom_part(&plan.id),
            escape_attr(&plan.id),
            escape_attr(data_look),
            escape_attr(&data_points),
            escape_attr(&classes),
            path
        );
        if !plan.dagre_recursive {
            let _ = write!(
                out,
                r#" data-usecase-id="{}" data-usecase-kind="{}""#,
                escape_attr(&plan.id),
                if plan.internal {
                    "note-connector"
                } else {
                    "relationship"
                }
            );
        }
        if plan.dagre_recursive {
            // Extracted edges are absent from the top-level LayoutData annotation pass.
        } else if plan.internal {
            out.push_str(" aria-hidden=\"true\"");
        } else {
            let name = if plan.original_id.is_some() {
                plan.id.clone()
            } else {
                format!(
                    "{} from {} to {}",
                    label.unwrap_or_default(),
                    source_labels[plan.source.as_str()],
                    source_labels[plan.target.as_str()]
                )
            };
            let _ = write!(out, r#" role="img" aria-label="{}""#, escape_attr(&name));
        }
        let mut source_style = plan.source_style.clone();
        if plan.dotted {
            source_style.push_str("stroke-dasharray:3;");
        }
        // The authored mask encodes offsets along the path before line hops.
        let source_style = if let Some(hopped_d) = hopped_paths.get(plan.id.as_str()) {
            line_hops::rewrite_style_after_line_hop(&source_style, hopped_d, options.work_meter)?
        } else {
            std::borrow::Cow::Borrowed(source_style.as_str())
        };
        #[cfg(feature = "layout-grid")]
        let source_style = match grid_edge {
            Some(paint) => Cow::Owned(grid::path_style(plan, paint, options.work_meter)?),
            None => source_style,
        };
        #[cfg(feature = "layout-layered")]
        let source_style = match layered_edge {
            Some(paint) => Cow::Owned(layered::path_style(plan, paint, options.work_meter)?),
            None => source_style,
        };
        let _ = write!(out, r#" style="{}""#, escape_attr(&source_style));
        for (end, reference) in marker_references {
            let _ = write!(out, r#" marker-{end}="{}""#, escape_attr(&reference));
        }
        #[cfg(feature = "layout-shoal")]
        if let Some(id) = caption_cutout {
            cutout::write_reference(&mut out, id);
        }
        out.push_str("/>");
    }
    out.push_str("</g><g class=\"edgeLabels\">");
    for (index, plan) in prepared.measured.edges.iter().enumerate() {
        out.checkpoint()?;
        if let Some(label) = &plan.label {
            let edge = edge_geometry[plan.id.as_str()];
            if let Some(position) = &edge.label {
                let panel_override = panel_background_override(index);
                write_label(
                    &mut out,
                    label,
                    UsecaseLabelKind::Edge {
                        id: &plan.id,
                        panel_override,
                        plate: transparent_auto_canvas
                            .filter(|_| !panel_override)
                            .and_then(|theme| exposed_caption_plate(label, css_binding, theme)),
                    },
                    position.x,
                    position.y + label_y_offset,
                    labels,
                );
            }
        } else if plan.original_id.is_some() {
            shapes::write_empty_edge_label(&mut out, &plan.id, config);
        }
    }
    out.push_str("</g><g class=\"nodes\">");
    for (color_index, plan) in prepared
        .measured
        .nodes
        .iter()
        .filter(|plan| !plan.is_boundary)
        .enumerate()
    {
        out.checkpoint()?;
        let node = geometry[plan.id.as_str()];
        if plan.dagre_helper {
            shapes::write_dagre_helper(&mut out, node, config);
            continue;
        }
        let source = source_nodes.get(plan.id.as_str()).copied();
        let note = notes.get(plan.id.as_str()).copied();
        let json = json_nodes.get(plan.id.as_str()).copied();
        let (kind, name, classes) = if let Some(source) = source {
            let label = &source_labels[plan.id.as_str()];
            let stereotype = source
                .stereotype
                .as_ref()
                .map(|value| {
                    format!(
                        ", stereotype {}",
                        sanitize_source_label(value, UsecaseLabelType::Text, config)
                    )
                })
                .unwrap_or_default();
            let business = if source.business { "business " } else { "" };
            let actor = source.kind == UsecaseNodeKind::Actor;
            let variant = match source.actor_type {
                UsecaseActorType::Normal => "",
                UsecaseActorType::Hollow => "hollow ",
                UsecaseActorType::Awesome => "awesome ",
                UsecaseActorType::Icon => "icon ",
            };
            (
                if actor { "actor" } else { "usecase" },
                if actor {
                    format!("{business}{variant}actor {label}{stereotype}")
                } else {
                    format!("{business}use case {label}{stereotype}")
                },
                source.classes.as_slice(),
            )
        } else if let Some(note) = note {
            (
                "note",
                format!(
                    "Note for {}: {}",
                    source_labels[note.target.as_str()],
                    accessible_label(&plan.label)
                ),
                [].as_slice(),
            )
        } else if let Some(json) = json {
            let rows = plan
                .table
                .as_ref()
                .map(|table| {
                    table
                        .rows
                        .iter()
                        .map(|row| format!("{}: {}", row.accessible_key, row.value.text))
                        .collect::<Vec<_>>()
                        .join("; ")
                })
                .unwrap_or_default();
            (
                "json",
                if rows.is_empty() {
                    json.id.clone()
                } else {
                    format!("{}: {rows}", json.id)
                },
                json.classes.as_slice(),
            )
        } else {
            return Err(Error::InvalidModel {
                message: format!("missing Usecase node {}", plan.id),
            });
        };
        let appearance = theme::appearance_attributes(css_binding, source.map(|_| color_index));
        let role_classes = match source {
            Some(node) if node.kind == UsecaseNodeKind::Actor => {
                let variant = match node.actor_type {
                    UsecaseActorType::Normal => "normal",
                    UsecaseActorType::Hollow => "hollow",
                    UsecaseActorType::Awesome => "awesome",
                    UsecaseActorType::Icon => "icon",
                };
                format!(
                    "usecase-actor usecase-actor-{variant}{} usecase-actor-variant usecase-actor-{variant}",
                    if node.business {
                        " usecase-business"
                    } else {
                        ""
                    }
                )
            }
            Some(node) => element_role_classes(plan, node.business),
            None if note.is_some() => "usecase-note".into(),
            None => "usecase-json-table usecase-json-table".into(),
        };
        let canonical_attributes = if uses_canonical {
            format!(r#" data-et="node" data-id="{}""#, escape_attr(&plan.id))
        } else {
            String::new()
        };
        let _ = write!(
            out,
            r#"<g id="{}-usecase-{}"{appearance} class="node default {role_classes} {}" data-usecase-id="{}" data-usecase-kind="{kind}" role="img" aria-label="{}" transform="translate({},{})"{canonical_attributes}>"#,
            dom_part(diagram_id.semantic_str()),
            dom_part(&plan.id),
            escape_attr(&classes.join(" ")),
            escape_attr(&plan.id),
            escape_attr(&name),
            fmt(node.x),
            fmt(node.y)
        );
        shapes::write_node(
            &mut out,
            node,
            plan,
            &shapes::UsecaseNodeRenderContext {
                source,
                note: note.is_some(),
                css_binding,
                labels,
                diagram_id,
                options,
                canonical: uses_canonical,
                #[cfg(feature = "layout-shoal")]
                sketch: sketch_shapes.as_ref(),
            },
        )?;
        out.push_str("</g>");
    }
    #[cfg(feature = "layout-grid")]
    if let Some(grid) = &prepared.grid {
        grid::write_labels(
            &mut out,
            prepared,
            grid,
            diagram_id,
            config_bool(cfg, &["htmlLabels"]).unwrap_or(true),
            options.work_meter,
        )?;
    }
    out.push_str("</g></g></g>");
    if let Some(look) = &css_binding.mermaid_look {
        look.defs.write_shadow_defs(&mut out, diagram_id)?;
        look.defs.write_gradient(&mut out, diagram_id)?;
    }
    if let Some(title) = title {
        let _ = write!(
            out,
            r#"<text class="usecaseDiagramTitleText" x="{}" y="{}" text-anchor="middle">{}</text>"#,
            fmt(title_x),
            fmt(0.0),
            escape_xml(title)
        );
    }
    #[cfg(feature = "layout-shoal")]
    let document = if let Some(sketch) = &sketch_shapes {
        let viewport = if let Some(rough) = sketch.bounds.get() {
            root_svg::DiagramBounds::from_extents(
                viewport_bounds.min_x.min(rough.x0),
                viewport_bounds.min_y.min(rough.y0),
                viewport_bounds.max_x.max(rough.x1),
                viewport_bounds.max_y.max(rough.y1),
                padding,
            )
        } else {
            viewport
        };
        options.viewport.finish_document(
            &mut out,
            document,
            root_svg::RootViewportSpec::mermaid(viewport, use_max_width),
        )?
    } else {
        document
    };
    out.push_str("</svg>");
    document.complete(out.finish()?)
}
