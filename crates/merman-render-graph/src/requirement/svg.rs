//! Complete, bounded Mermaid-compatible Requirement SVG emission.

use super::{RequirementEdgeIdentity, RequirementNativeStyle, requirement_color_indices};
use base64::Engine as _;
use merman_model::{OperationPhase, families::requirement::RequirementDiagramModel};
#[cfg(feature = "layout-shoal")]
use merman_render_core::LayoutEdge;
use merman_render_core::{
    Bounds, Error, LayoutPoint, Result,
    number_format::canonical_number as fmt,
    resources::OperationWorkMeter,
    root_svg,
    rough_svg::{
        RoughRectSpec, constrained_rough_edge_path, ops_to_svg_path_d as roughjs_ops_to_svg_path_d,
        parse_hex_color_to_srgba as roughjs_parse_hex_color_to_srgba, roughjs_paths_for_rect,
    },
    svg_bounds::svg_path_bounds_from_d,
    svg_css::{
        CssSelectorDiagramId, PreparedLookDefs, SvgDiagramIdValue, SvgId, available_local_svg_id,
        escape_css_identifier,
    },
    svg_edge_parity::{
        curve, edge_label_geometry, edge_path, line_hops,
        path_numbers::{fmt_path, json_stringify_points},
        svg_path_length_from_d,
    },
    svg_output::{BoundedSvgOutput, SvgOutput, XmlTextOutput},
    text::{TextMeasurer, TextStyle, svg_label as label},
    xml::{escape_attr, escape_attr_display, escape_xml},
};
use merman_theme_contract::v2::{EdgeStyle, MarkerStyle};
use roughr::core::RoughRandomness;
use std::fmt::Write as _;

mod css;

const REQUIREMENT_ROUGH_GEOMETRY_COLOR: &str = "#000000";

/// An owned canonical edge, retained across the root and edge-label passes.
pub struct RequirementCanonicalEdge {
    pub points: Vec<LayoutPoint>,
    pub bounds: Bounds,
}

struct RequirementPaintedEdge {
    d: String,
    label_position: Option<LayoutPoint>,
    canonical: Option<RequirementCanonicalEdge>,
}

#[cfg(feature = "layout-shoal")]
pub type RequirementCanonicalEdgePainter<'a> =
    dyn Fn(&LayoutEdge, f64, Option<EdgeStyle>) -> Result<(String, RequirementCanonicalEdge)> + 'a;

/// Supplies the composed viewport, ID projection and operation ledger to the balanced writer.
pub struct RequirementSvgContext<'a> {
    pub diagram_id: SvgId<'a>,
    pub viewport: root_svg::RootViewportContext<'a>,
    pub work_meter: &'a OperationWorkMeter,
    pub rough_randomness: &'a RoughRandomness,
    #[cfg(feature = "layout-shoal")]
    pub canonical_routes: Option<&'a RequirementCanonicalEdgePainter<'a>>,
}

fn requirement_color_id(border_colors: &[String], color_index: usize) -> Option<String> {
    (!border_colors.is_empty()).then(|| format!("color-{}", color_index % border_colors.len()))
}

fn write_requirement_color_css<I>(
    out: &mut impl SvgOutput,
    diagram_id: I,
    data_look: &str,
    border_colors: &[String],
    background_colors: &[String],
) -> Result<()>
where
    I: SvgDiagramIdValue,
{
    let diagram_id = escape_css_identifier(diagram_id.semantic_value());
    for (index, border_color) in border_colors.iter().enumerate() {
        out.checkpoint()?;
        let background =
            background_colors.get(index.checked_rem(background_colors.len()).unwrap_or(0));
        for shape in ["path", "rect"] {
            let _ = write!(out, "#{diagram_id} [data-look=")
                .and_then(|()| cssparser::serialize_string(data_look, out))
                .and_then(|()| {
                    write!(
                        out,
                        r#"][data-color-id="color-{index}"].node {shape}{{stroke:{border_color};"#,
                    )
                });
            out.checkpoint()?;
            if let Some(background) = background {
                let _ = write!(out, "fill:{background};");
            }
            out.push('}');
            out.checkpoint()?;
        }
    }
    Ok(())
}

#[cfg(test)]
fn requirement_color_css<I: SvgDiagramIdValue>(
    diagram_id: I,
    data_look: &str,
    border_colors: &[String],
    background_colors: &[String],
) -> String {
    let mut css = String::new();
    write_requirement_color_css(
        &mut css,
        diagram_id,
        data_look,
        border_colors,
        background_colors,
    )
    .expect("unbounded test string output");
    css
}

fn write_requirement_stylesheet(
    out: &mut impl SvgOutput,
    diagram_id: impl SvgDiagramIdValue,
    binding: &super::RequirementCssBinding,
    look: &str,
    canonical: bool,
) -> Result<()> {
    binding
        .common
        .write_prefix(out, CssSelectorDiagramId(diagram_id))?;
    write_requirement_color_css(
        out,
        diagram_id,
        look,
        &binding.border_colors,
        &binding.background_colors,
    )?;
    css::write_requirement_css(out, diagram_id, binding, canonical)
}

pub fn render_requirement_diagram_svg_model(
    prepared: &super::RequirementPreparedArtifact,
    model: &RequirementDiagramModel,
    paint_theme: &super::RequirementPaintThemePlan,
    sanitize_config: &merman_model::presentation::PresentationConfig,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &RequirementSvgContext<'_>,
) -> Result<root_svg::RootedSvg> {
    let (layout, prepared_nodes, prepared_edges) = prepared.render_parts();
    let effective_config = sanitize_config.as_value();
    let label_measurements =
        prepared.label_measurements_for_render_with_typography(measurer, paint_theme.css());

    fn mermaid_markdown_to_html(
        raw: &str,
        sanitize_config: &merman_model::presentation::PresentationConfig,
    ) -> String {
        let sanitized = requirement_label_source(raw, sanitize_config);
        merman_render_core::text::mermaid_markdown_to_xhtml_label_fragment(&sanitized, true)
    }

    fn requirement_label_source(
        raw: &str,
        sanitize_config: &merman_model::presentation::PresentationConfig,
    ) -> String {
        let decoded = raw
            .replace("ﬂ°°", "&#")
            .replace("ﬂ°", "&")
            .replace("¶ß", ";");
        merman_markup::sanitize::sanitize_text(&decoded, sanitize_config.as_config())
    }

    #[derive(Debug, Clone, Copy)]
    struct LabelForeignObject<'a> {
        html: &'a str,
        width: f64,
        height: f64,
        span_class: &'a str,
        span_style: Option<&'a str>,
        div_class: Option<&'a str>,
        div_style_prefix: Option<&'a str>,
        max_width_px: i64,
        text_align: &'static str,
    }

    fn mk_label_foreign_object(out: &mut impl SvgOutput, spec: LabelForeignObject<'_>) {
        let LabelForeignObject {
            html,
            width,
            height,
            span_class,
            span_style,
            div_class,
            div_style_prefix,
            max_width_px,
            text_align,
        } = spec;
        let div_class_attr = div_class
            .map(|c| format!(r#" class="{c}""#))
            .unwrap_or_default();
        let span_style_attr = span_style
            .map(|s| format!(r#" style="{}""#, escape_attr(s)))
            .unwrap_or_default();
        let div_style_prefix = div_style_prefix.unwrap_or("");
        // A table can grow past its requested width to fit an unbreakable word. That
        // min-content expansion retains createText's constrained wrapping mode.
        let constrained_width = max_width_px > 0 && width >= max_width_px as f64;
        let (display, white_space, width_style) = if constrained_width {
            (
                "table",
                "break-spaces",
                format!(" width: {max_width_px}px;"),
            )
        } else {
            ("table-cell", "nowrap", String::new())
        };
        let _ = write!(
            out,
            r#"<foreignObject height="{h}" width="{w}"><div xmlns="http://www.w3.org/1999/xhtml"{div_class_attr} style="{div_style_prefix}display: {display}; white-space: {white_space}; line-height: 1.5; max-width: {max_width}px; text-align: {text_align};{width_style}"><span class="{span_class}"{span_style_attr}>"#,
            w = fmt(width),
            h = fmt(height),
            div_class_attr = div_class_attr,
            span_class = escape_attr_display(span_class),
            span_style_attr = span_style_attr,
            // CSS is an attribute value: Mermaid's text entity decoder would turn
            // numeric colors such as `#051423;` into a Unicode scalar.
            div_style_prefix = escape_attr(div_style_prefix),
            display = display,
            white_space = white_space,
            max_width = max_width_px,
            width_style = width_style,
        );
        out.push_str(html);
        out.push_str("</span></div></foreignObject>");
    }

    fn rough_double_line_path_d(x1: f64, y1: f64, x2: f64, y2: f64) -> String {
        let cx1 = (x1 + x2) / 2.0;
        let cy1 = (y1 + y2) / 2.0;
        let mut out = String::new();
        let _ = write!(
            &mut out,
            "M{x1} {y1} C{cx0} {cy0} {cx1} {cy1} {x2} {y2} M{x1b} {y1b} C{cx0b} {cy0b} {cx1b} {cy1b} {x2b} {y2b}",
            x1 = fmt_path(x1),
            y1 = fmt_path(y1),
            cx0 = fmt_path((x1 * 2.0 + x2) / 3.0),
            cy0 = fmt_path((y1 * 2.0 + y2) / 3.0),
            cx1 = fmt_path((x1 + x2 * 2.0) / 3.0),
            cy1 = fmt_path((y1 + y2 * 2.0) / 3.0),
            x2 = fmt_path(x2),
            y2 = fmt_path(y2),
            x1b = fmt_path(x1),
            y1b = fmt_path(y1),
            cx0b = fmt_path(cx1),
            cy0b = fmt_path(cy1),
            cx1b = fmt_path(cx1 + (x2 - x1) * 0.1),
            cy1b = fmt_path(cy1 + (y2 - y1) * 0.1),
            x2b = fmt_path(x2),
            y2b = fmt_path(y2),
        );
        out
    }

    fn rough_rect_stroke_path_d(x: f64, y: f64, w: f64, h: f64) -> String {
        let x2 = x + w;
        let y2 = y + h;
        let mut out = String::new();
        out.push_str(&rough_double_line_path_d(x, y, x2, y));
        out.push(' ');
        out.push_str(&rough_double_line_path_d(x2, y, x2, y2));
        out.push(' ');
        out.push_str(&rough_double_line_path_d(x2, y2, x, y2));
        out.push(' ');
        out.push_str(&rough_double_line_path_d(x, y2, x, y));
        out
    }

    let diagram_id = options.diagram_id;
    let requirement_config = super::RequirementConfigView::new(effective_config);
    let binding = paint_theme.css();
    let native = binding.native.as_ref();
    let edge_style = binding.edge_style();
    let render_settings = requirement_config.render_settings_with_binding(binding);
    #[cfg(feature = "layout-shoal")]
    let canonical_paint = options.canonical_routes.map(|paint| {
        let width = binding.canonical_stroke_width(&requirement_config);
        (paint, width / 2.0)
    });
    #[cfg(feature = "layout-shoal")]
    let uses_canonical_routes = canonical_paint.is_some();
    #[cfg(not(feature = "layout-shoal"))]
    let uses_canonical_routes = false;
    let look = render_settings.look;
    let look = look.as_str();
    let look_attr = format!(r#" data-look="{}""#, escape_attr_display(look));
    let border_colors = &binding.border_colors;
    let color_indices = requirement_color_indices(model);
    let has_diagram_id = !diagram_id.semantic_str().is_empty();

    let req_by_id: std::collections::HashMap<&str, _> = model
        .requirements
        .iter()
        .map(|n| (n.name.as_str(), n))
        .collect();
    let el_by_id: std::collections::HashMap<&str, _> = model
        .elements
        .iter()
        .map(|n| (n.name.as_str(), n))
        .collect();

    let font_family = Some(render_settings.font_family.clone());
    let font_size = render_settings.font_size;
    let hand_drawn_seed = options.rough_randomness;
    let compatibility_sketch_markers = (edge_style == Some(EdgeStyle::Sketch)
        && !uses_canonical_routes)
        .then(|| binding.relation_markers(&requirement_config));
    let html_style_regular = TextStyle {
        font_family: font_family.clone(),
        font_size,
        font_weight: None,
        font_style: None,
    };

    let mut painted_edges = std::collections::HashMap::with_capacity(layout.edges.len());
    for edge in &layout.edges {
        let identity = RequirementEdgeIdentity::from(edge);
        let prepared_edge = prepared_edges
            .get(&identity)
            .ok_or_else(|| Error::InvalidModel {
                message: format!(
                    "missing prepared Requirement edge label for {} -> {} ({})",
                    edge.from, edge.to, edge.id
                ),
            })?;
        let compatibility_path = || -> Result<String> {
            Ok(match edge_style {
                Some(EdgeStyle::Orthogonal) => curve::curve_step_path_d_and_bounds(&edge.points).0,
                Some(EdgeStyle::Straight) => curve::curve_linear_path_d(&edge.points),
                Some(EdgeStyle::Sketch) => {
                    let d = curve::curve_linear_path_d(&edge.points);
                    let (contains, arrow) = compatibility_sketch_markers.unwrap_or_default();
                    let markers = (
                        if prepared_edge.marker_start {
                            contains
                        } else {
                            Default::default()
                        },
                        if prepared_edge.marker_end {
                            arrow
                        } else {
                            Default::default()
                        },
                    );
                    return constrained_rough_edge_path(
                        &d,
                        markers,
                        options.work_meter,
                        hand_drawn_seed,
                    );
                }
                Some(EdgeStyle::Curved) => curve::curve_basis_path_d(&edge.points),
                None if prepared.uses_elk() && edge.points.len() <= 2 => {
                    curve::curve_linear_path_d(&edge.points)
                }
                None if prepared.uses_elk() => curve::curve_rounded_path_d(&edge.points, 5.0),
                None => curve::curve_basis_path_d(&edge.points),
            })
        };
        #[cfg(feature = "layout-shoal")]
        let (d, canonical) = if let Some((paint, stroke_outset)) = canonical_paint {
            let (d, geometry) = paint(edge, stroke_outset, edge_style)?;
            (d, Some(geometry))
        } else {
            (compatibility_path()?, None)
        };
        #[cfg(not(feature = "layout-shoal"))]
        let (d, canonical) = (compatibility_path()?, None);
        let label_position = if prepared_edge.has_label {
            let label = edge.label.as_ref().ok_or_else(|| Error::InvalidModel {
                message: format!(
                    "missing Requirement edge label geometry for {} -> {} ({})",
                    edge.from, edge.to, edge.id
                ),
            })?;
            let anchor = LayoutPoint {
                x: label.x,
                y: label.y,
            };
            Some(if uses_canonical_routes {
                anchor
            } else {
                edge_label_geometry::position_edge_label(
                    anchor,
                    Some(&edge.points),
                    &edge.points,
                    &d,
                    false,
                )
            })
        } else {
            None
        };
        if painted_edges
            .insert(
                identity,
                RequirementPaintedEdge {
                    d,
                    label_position,
                    canonical,
                },
            )
            .is_some()
        {
            return Err(Error::InvalidModel {
                message: format!(
                    "duplicate rendered Requirement edge identity for {} -> {} ({})",
                    edge.from, edge.to, edge.id
                ),
            });
        }
    }

    let hop_paths = if prepared.uses_elk()
        && !matches!(edge_style, Some(EdgeStyle::Curved | EdgeStyle::Sketch))
        && effective_config
            .pointer("/elk/lineHops")
            .and_then(serde_json::Value::as_bool)
            != Some(false)
    {
        options.work_meter.charge(layout.edges.len())?;
        let hop_edges = layout
            .edges
            .iter()
            .filter(|edge| {
                edge_style != Some(EdgeStyle::Orthogonal)
                    || edge
                        .points
                        .windows(2)
                        .all(|pair| pair[0].x == pair[1].x || pair[0].y == pair[1].y)
            })
            .map(|edge| line_hops::LineHopEdge {
                id: edge.id.as_str(),
                points: &edge.points,
                curve: Some(if edge_style.is_some() || edge.points.len() <= 2 {
                    "linear"
                } else {
                    "rounded"
                }),
                arrow_type_start: None,
                arrow_type_end: None,
            })
            .collect::<Vec<_>>();
        line_hops::elk_line_hop_paths(effective_config, &hop_edges, options.work_meter)?
    } else {
        std::collections::HashMap::new()
    };

    // Mermaid derives the root viewport from the rendered SVG subtree. Reconstruct those bounds
    // from final paths and post-path label positions instead of stale Dagre label anchors.
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for node in &layout.nodes {
        min_x = min_x.min(node.x);
        min_y = min_y.min(node.y);
        max_x = max_x.max(node.x + node.width);
        max_y = max_y.max(node.y + node.height);
    }
    for edge in &layout.edges {
        for point in &edge.points {
            min_x = min_x.min(point.x);
            min_y = min_y.min(point.y);
            max_x = max_x.max(point.x);
            max_y = max_y.max(point.y);
        }
        let identity = RequirementEdgeIdentity::from(edge);
        let painted = &painted_edges[&identity];
        if let Some(canonical) = &painted.canonical {
            min_x = min_x.min(canonical.bounds.min_x);
            min_y = min_y.min(canonical.bounds.min_y);
            max_x = max_x.max(canonical.bounds.max_x);
            max_y = max_y.max(canonical.bounds.max_y);
        } else if let Some(path) = hop_paths
            .get(edge.id.as_str())
            .or_else(|| native.map(|_| &painted.d))
        {
            options.work_meter.charge(path.len())?;
            if let Some(bounds) = svg_path_bounds_from_d(path) {
                let outset = native.map_or(0.0, |style| style.connector_width / 2.0);
                min_x = min_x.min(bounds.min_x - outset);
                min_y = min_y.min(bounds.min_y - outset);
                max_x = max_x.max(bounds.max_x + outset);
                max_y = max_y.max(bounds.max_y + outset);
            }
        }
        if let (Some(label), Some(position)) = (&edge.label, &painted.label_position) {
            min_x = min_x.min(position.x - label.width / 2.0);
            min_y = min_y.min(position.y - label.height / 2.0);
            max_x = max_x.max(position.x + label.width / 2.0);
            max_y = max_y.max(position.y + label.height / 2.0);
        }
    }
    let mut content_bounds =
        if min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite() {
            Bounds {
                min_x,
                min_y,
                max_x,
                max_y,
            }
        } else {
            Bounds {
                min_x: 0.0,
                min_y: 0.0,
                max_x: 0.0,
                max_y: 0.0,
            }
        };
    let diagram_title = diagram_title
        .map(str::trim)
        .filter(|title| !title.is_empty());
    let title_position = diagram_title.map(|title| {
        let x = (content_bounds.min_x + content_bounds.max_x) / 2.0;
        let y = -render_settings.title_top_margin;
        let title_style = native.map_or_else(
            || html_style_regular.clone(),
            |native| TextStyle {
                font_family: Some(native.title_family.clone()),
                font_weight: Some(native.title_weight.clone()),
                ..html_style_regular.clone()
            },
        );
        let (left, right) = measurer.measure_svg_title_bbox_x(title, &title_style);
        let (ascent, descent) = merman_render_core::text::svg_text_vertical_extents_px(
            measurer,
            title,
            &title_style,
            merman_render_core::text::svg_title_bbox_vertical_extents_px(&title_style),
        );
        content_bounds.min_x = content_bounds.min_x.min(x - left);
        content_bounds.max_x = content_bounds.max_x.max(x + right);
        content_bounds.min_y = content_bounds.min_y.min(y - ascent);
        content_bounds.max_y = content_bounds.max_y.max(y + descent);
        (title, x, y)
    });
    let mut out = BoundedSvgOutput::new(options.work_meter);
    let mut aria_labelledby: Option<String> = None;
    let mut aria_describedby: Option<String> = None;
    let mut a11y_nodes = String::new();
    if let Some(t) = model.acc_title.as_deref().filter(|t| !t.is_empty()) {
        aria_labelledby = Some(format!("chart-title-{diagram_id}"));
        let _ = write!(
            &mut a11y_nodes,
            r#"<title id="chart-title-{}">{}</title>"#,
            diagram_id,
            escape_xml(t)
        );
    }
    if let Some(d) = model.acc_descr.as_deref().filter(|d| !d.is_empty()) {
        aria_describedby = Some(format!("chart-desc-{diagram_id}"));
        let _ = write!(
            &mut a11y_nodes,
            r#"<desc id="chart-desc-{}">{}</desc>"#,
            diagram_id,
            escape_xml(d)
        );
    }

    let mut root_chrome = root_svg::RootChrome::new(diagram_id, "requirement");
    root_chrome.class = Some("requirementDiagram");
    root_chrome.aria_labelledby = aria_labelledby.as_deref();
    root_chrome.aria_describedby = aria_describedby.as_deref();
    root_chrome.dom = root_svg::RootDomProfile {
        fixed_height_placement: root_svg::SvgRootFixedHeightPlacement::AfterXmlns,
        fixed_style_placement: root_svg::RootStylePlacement::Tail,
        ..root_svg::RootDomProfile::default()
    };
    let root_context = &options.viewport;
    let root_document = root_context.begin_document(
        &mut out,
        root_svg::DeferredRootSpec::mermaid(render_settings.use_max_width),
        root_chrome,
    )?;

    out.push_str(&a11y_nodes);

    out.push_str("<style>");
    {
        let mut css_out = XmlTextOutput::new(&mut out);
        write_requirement_stylesheet(
            &mut css_out,
            diagram_id,
            binding,
            look,
            uses_canonical_routes,
        )?;
        if let Some(native) = native {
            let id = escape_css_identifier(diagram_id.semantic_str());
            let _ = write!(
                &mut css_out,
                "#{id} .requirementDiagramTitleText{{font-family:{};font-weight:{};fill:{};}}",
                native.title_family, native.title_weight, native.title_ink,
            );
        }
        css_out.checkpoint()?;
    }
    out.push_str("</style>");
    let look_defs =
        PreparedLookDefs::new(effective_config).with_shadow_flood_color(binding.shadow_flood_color);
    if look_defs.uses_gradient() {
        out.push_str("<defs>");
        look_defs.write_gradient(&mut out, diagram_id)?;
        out.push_str("</defs>");
    }
    out.push_str("<g>");

    // Mermaid 12 selects the Neo variants in requirementRenderer.ts.
    let marker_units = if look == "neo" || uses_canonical_routes || native.is_some() {
        r#" markerUnits="userSpaceOnUse""#
    } else {
        ""
    };
    let marker_stroke = if let Some(native) = native {
        format!(r#" stroke-width="{}""#, fmt(native.connector_width))
    } else if look == "neo" {
        let width = &binding.marker_stroke_width;
        format!(r#" stroke-width="{}""#, escape_attr_display(width))
    } else {
        String::new()
    };
    let arrow_view_box = if look == "neo" {
        r#" viewBox="0 0 25 20""#
    } else {
        ""
    };
    let arrow_join = if look == "neo" {
        r#" stroke-linejoin="miter""#
    } else {
        ""
    };
    let _ = write!(
        &mut out,
        r#"<defs><marker id="{diagram_id}_requirement-requirement_containsStart" refX="0" refY="10" markerWidth="20" markerHeight="20" orient="auto"{marker_units}><g><circle cx="10" cy="10" r="9" fill="none"{marker_stroke}/><line x1="1" x2="19" y1="10" y2="10"{marker_stroke}/><line y1="1" y2="19" x1="10" x2="10"{marker_stroke}/></g></marker></defs>"#,
    );
    if let Some((native, path)) =
        native.and_then(|style| style.arrow_path().map(|path| (style, path)))
    {
        let size = fmt(RequirementNativeStyle::ARROW_SIZE);
        let width = fmt(native.connector_width * 10.0 / RequirementNativeStyle::ARROW_SIZE);
        let open = if native.marker_style == MarkerStyle::Open {
            r#" fill="none" stroke-linecap="round" stroke-linejoin="round""#
        } else {
            ""
        };
        let _ = write!(
            &mut out,
            r#"<defs><marker id="{diagram_id}_requirement-requirement_arrowEnd" refX="10" refY="5" markerWidth="{size}" markerHeight="{size}" viewBox="0 0 10 10" orient="auto"{marker_units}><path d="{path}" stroke-width="{width}"{open}/></marker></defs>"#,
        );
    } else {
        let _ = write!(
            &mut out,
            r#"<defs><marker id="{diagram_id}_requirement-requirement_arrowEnd" refX="20" refY="10" markerWidth="20" markerHeight="20" orient="auto"{marker_units}{marker_stroke}{arrow_view_box}><path d="M0,0&#10;      L20,10&#10;      M20,10&#10;      L0,20"{arrow_join}/></marker></defs>"#,
        );
    }
    options.work_meter.checkpoint(OperationPhase::Emit)?;

    out.push_str(r#"<g class="root">"#);
    out.push_str(r#"<g class="clusters"/>"#);

    out.push_str(if prepared.uses_elk() {
        r#"<g class="edges edgePaths">"#
    } else {
        r#"<g class="edgePaths">"#
    });
    for (edge_index, e) in layout.edges.iter().enumerate() {
        let identity = RequirementEdgeIdentity::from(e);
        let prepared_label = prepared_edges
            .get(&identity)
            .ok_or_else(|| Error::InvalidModel {
                message: format!(
                    "missing prepared Requirement edge label for {} -> {} ({})",
                    e.from, e.to, e.id
                ),
            })?;
        let rel_type = prepared_label.relationship_type.as_str();
        let is_contains = rel_type == "contains";
        let pattern = if is_contains { "solid" } else { "dashed" };
        let class = format!("edge-thickness-normal edge-pattern-{pattern} relationshipLine");
        let style = match (uses_canonical_routes, is_contains) {
            (true, true) => "fill:none;",
            (true, false) => "fill:none;stroke-dasharray:10,7;",
            (false, true) => "fill:none;;;;fill:none;",
            (false, false) => "fill:none;stroke-dasharray: 10,7;;;fill:none;stroke-dasharray: 10,7",
        };

        let painted = painted_edges
            .get(&identity)
            .expect("Requirement edge paths were validated before root rendering");
        let d = painted.d.as_str();
        let mut masked_style = String::new();
        if !uses_canonical_routes
            && look == "neo"
            && let Some(length) = svg_path_length_from_d(d)
        {
            edge_path::write_neo_edge_mask(
                &mut masked_style,
                length,
                None,
                None,
                !is_contains,
                false,
            );
        }
        masked_style.push_str(style);
        let hopped_d = hop_paths.get(e.id.as_str()).map(String::as_str);
        let style = if let Some(path) = hopped_d {
            line_hops::rewrite_style_after_line_hop(&masked_style, path, options.work_meter)?
        } else {
            std::borrow::Cow::Borrowed(masked_style.as_str())
        };
        let data_points = painted
            .canonical
            .as_ref()
            .map_or(e.points.as_slice(), |geometry| geometry.points.as_slice());
        let data_points_b64 =
            base64::engine::general_purpose::STANDARD.encode(json_stringify_points(data_points));

        let mut marker_attr = String::new();
        if prepared_label.marker_start {
            let _ = write!(
                &mut marker_attr,
                r#" marker-start="url(#{diagram_id}_requirement-requirement_containsStart)""#,
            );
        }
        if prepared_label.marker_end {
            let _ = write!(
                &mut marker_attr,
                r#" marker-end="url(#{diagram_id}_requirement-requirement_arrowEnd)""#,
            );
        }

        let dom_id = if uses_canonical_routes {
            format!("{diagram_id}-requirement-edge-{edge_index}")
        } else if has_diagram_id {
            format!("{diagram_id}-{}", prepared_label.rendered_id)
        } else {
            prepared_label.rendered_id.clone()
        };
        options.work_meter.checkpoint(OperationPhase::Emit)?;

        let _ = write!(
            &mut out,
            r#"<path d="{d}" id="{dom_id}" class="{class}" style="{style}" data-edge="true" data-et="edge" data-id="{id}" data-points="{data_points}"{look_attr}{marker_attr}/>"#,
            d = escape_attr_display(hopped_d.unwrap_or(d)),
            dom_id = escape_attr_display(&dom_id),
            id = escape_attr_display(&prepared_label.rendered_id),
            class = escape_attr_display(&class),
            style = escape_attr_display(&style),
            data_points = escape_attr_display(&data_points_b64),
            look_attr = look_attr.as_str(),
            marker_attr = marker_attr,
        );
    }
    out.push_str("</g>");

    out.push_str(r#"<g class="edgeLabels">"#);
    for e in &layout.edges {
        let identity = RequirementEdgeIdentity::from(e);
        let prepared_label = prepared_edges
            .get(&identity)
            .ok_or_else(|| Error::InvalidModel {
                message: format!(
                    "missing prepared Requirement edge label for {} -> {} ({})",
                    e.from, e.to, e.id
                ),
            })?;
        let rel_type = prepared_label.relationship_type.as_str();
        debug_assert!(!rel_type.trim().is_empty());
        let label_text = prepared_label.display_text.as_str();
        if prepared_label.has_label {
            label_measurements
                .measure_edge_label_for_render(prepared_label)
                .ok_or_else(|| Error::InvalidModel {
                    message: format!(
                        "missing Requirement edge label measurement for {} -> {} ({})",
                        e.from, e.to, e.id
                    ),
                })?;
        }

        let (w, h) = e
            .label
            .as_ref()
            .map(|label| (label.width, label.height))
            .unwrap_or((0.0, 0.0));
        let svg_text_height = (h - 4.0).max(0.0);
        let svg_text_y = if !render_settings.edge_html_labels && prepared_label.has_label {
            measurer.measure_svg_create_text_bbox_y_offset_px(label_text, &html_style_regular)
        } else {
            0.0
        };
        if prepared_label.has_label {
            let label_position = painted_edges
                .get(&identity)
                .and_then(|painted| painted.label_position.as_ref())
                .expect("Requirement label positions were validated before root rendering");
            let _ = write!(
                &mut out,
                r#"<g class="edgeLabel" transform="translate({x}, {y})"><g class="label" data-id="{id}" transform="translate({lx}, {ly})">"#,
                x = fmt(label_position.x),
                y = fmt(label_position.y),
                id = escape_attr_display(&prepared_label.rendered_id),
                lx = fmt(if render_settings.edge_html_labels {
                    -w / 2.0
                } else {
                    0.0
                }),
                ly = fmt(if render_settings.edge_html_labels {
                    -h / 2.0
                } else {
                    -svg_text_y - svg_text_height / 2.0
                }),
            );
        } else {
            let _ = write!(
                &mut out,
                r#"<g class="edgeLabel"><g class="label" data-id="{id}" transform="translate(0, 0)">"#,
                id = escape_attr_display(&prepared_label.rendered_id),
            );
        }
        if render_settings.edge_html_labels {
            let label_html = mermaid_markdown_to_html(label_text, sanitize_config);
            mk_label_foreign_object(
                &mut out,
                LabelForeignObject {
                    html: &label_html,
                    width: w,
                    height: h,
                    span_class: "edgeLabel",
                    span_style: None,
                    div_class: Some("labelBkg"),
                    div_style_prefix: None,
                    max_width_px: 200,
                    text_align: "center",
                },
            );
        } else {
            out.push_str("<g>");
            if prepared_label.has_label {
                let _ = write!(
                    &mut out,
                    r#"<rect class="background" style="" x="{}" y="{}" width="{}" height="{}"/>"#,
                    fmt(-w / 2.0),
                    fmt(svg_text_y - 2.0),
                    fmt(w),
                    fmt(h)
                );
            } else {
                out.push_str(r#"<rect class="background" style="stroke: none"/>"#);
            }
            label::write_svg_text_markdown_wrapped_centered_from_create_text_source(
                &mut out,
                label_text,
                true,
                measurer,
                &html_style_regular,
                Some(200.0),
            );
            out.push_str("</g>");
        }
        out.push_str("</g></g>");
    }
    out.push_str("</g>");

    out.push_str(r#"<g class="nodes">"#);
    let mut legacy_edge_ids = None;
    for (node_index, n) in layout.nodes.iter().enumerate() {
        let cx = n.x + n.width / 2.0;
        let cy = n.y + n.height / 2.0;
        let prepared_node = prepared_nodes
            .get(&n.id)
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing prepared Requirement node plan for {}", n.id),
            })?;
        use super::RequirementNodeRenderPlan;
        let prepared_node = match prepared_node {
            RequirementNodeRenderPlan::Semantic(plan) => plan,
            #[cfg(feature = "layout-dagre")]
            RequirementNodeRenderPlan::EdgeLabelAnchor => {
                let _ = write!(
                    &mut out,
                    r#"<g class="label edgeLabel" id="{id}" transform="translate({cx}, {cy})"><rect width="0.1" height="0.1"/><g class="label" style="" transform="translate(0, 0)"><rect/>"#,
                    id = escape_attr_display(&n.id),
                    cx = fmt(cx),
                    cy = fmt(cy),
                );
                if render_settings.html_labels {
                    mk_label_foreign_object(
                        &mut out,
                        LabelForeignObject {
                            html: "",
                            width: 0.0,
                            height: 0.0,
                            span_class: "nodeLabel",
                            span_style: None,
                            div_class: None,
                            div_style_prefix: None,
                            max_width_px: 10,
                            text_align: "center",
                        },
                    );
                } else {
                    out.push_str(r#"<g><rect class="background" style="stroke: none"/>"#);
                    label::write_svg_text_markdown_from_create_text_source(&mut out, "", true);
                    out.push_str("</g>");
                }
                out.push_str("</g></g>");
                continue;
            }
        };
        let rendered_node = label_measurements
            .node_plan_for_render(prepared_node)
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing Requirement node label measurement for {}", n.id),
            })?;

        let mut node_classes: Vec<&str> = Vec::new();
        if let Some(req) = req_by_id.get(n.id.as_str()) {
            node_classes = req.classes.iter().map(String::as_str).collect();
        } else if let Some(el) = el_by_id.get(n.id.as_str()) {
            node_classes = el.classes.iter().map(String::as_str).collect();
        }
        if !node_classes.contains(&"default") {
            node_classes.insert(0, "default");
        }
        let classes_str = if node_classes.is_empty() {
            "node default".to_string()
        } else {
            format!("node {}", node_classes.join(" "))
        };
        let disambiguated = if !uses_canonical_routes
            && has_diagram_id
            && look_defs.defines_local_id(&n.id, true)
        {
            if legacy_edge_ids.is_none() {
                options.work_meter.charge(prepared_edges.len())?;
            }
            let edge_ids = legacy_edge_ids.get_or_insert_with(|| {
                prepared_edges
                    .values()
                    .map(|edge| edge.rendered_id.as_str())
                    .collect::<std::collections::HashSet<_>>()
            });
            Some(available_local_svg_id(
                format!("requirement-node-{node_index}"),
                prepared_nodes.len().saturating_add(edge_ids.len()),
                |id| prepared_nodes.contains_key(id) || edge_ids.contains(id),
                options.work_meter,
            )?)
        } else {
            None
        };
        let node_id = disambiguated.as_deref().unwrap_or(&n.id);
        let id_attr = if uses_canonical_routes {
            format!(r#" id="{diagram_id}-requirement-node-{node_index}""#)
        } else if has_diagram_id {
            format!(r#" id="{diagram_id}-{}""#, escape_attr_display(node_id))
        } else {
            format!(r#" id="{}""#, escape_attr_display(node_id))
        };
        options.work_meter.checkpoint(OperationPhase::Emit)?;
        let color_id_attr = color_indices
            .get(n.id.as_str())
            .and_then(|index| requirement_color_id(border_colors, *index))
            .map(|color_id| format!(r#" data-color-id="{}""#, escape_attr_display(&color_id)))
            .unwrap_or_default();
        let semantic_id_attr = if uses_canonical_routes || disambiguated.is_some() {
            format!(
                r#" data-et="node" data-id="{}""#,
                escape_attr_display(&n.id)
            )
        } else {
            String::new()
        };

        let _ = write!(
            &mut out,
            r#"<g class="{class}"{id_attr}{look_attr}{color_id_attr}{semantic_id_attr} transform="translate({cx}, {cy})">"#,
            class = escape_attr_display(&classes_str),
            id_attr = id_attr,
            look_attr = look_attr.as_str(),
            color_id_attr = color_id_attr,
            cx = fmt(cx),
            cy = fmt(cy),
        );

        let visual = paint_theme
            .node_visual(&n.id)
            .ok_or_else(|| Error::InvalidModel {
                message: format!("Requirement final node visual is missing {}", n.id),
            })?;
        let label_styles = &visual.label_styles;
        let label_div_style_prefix = &visual.label_div_style_prefix;
        let svg_text_styles = &visual.svg_text_styles;
        let node_styles = &visual.node_styles;
        let source_text_color = &visual.source_text_color;
        let fill_color = visual.fill.as_str();
        let stroke_color = visual.stroke.as_str();
        let stroke_width = visual.stroke_width;
        let fill_style_attr = &visual.fill_style_attr;
        let stroke_style_attr = &visual.stroke_style_attr;

        // RoughJS path geometry does not depend on RGB values. Keep paint-only values such as
        // `transparent` from selecting a different geometry fallback than an opaque color.
        let geometry_stroke_color = roughjs_parse_hex_color_to_srgba(stroke_color)
            .map_or(REQUIREMENT_ROUGH_GEOMETRY_COLOR, |_| stroke_color);

        let x = -n.width / 2.0;
        let y = -n.height / 2.0;
        let native_box = native.filter(|_| look != "handDrawn");
        let heading = native_box.and_then(|native| {
            if visual.source_owns_fill {
                return None;
            }
            let fill = native.emphasis.as_deref()?;
            let height = rendered_node.divider_y_offset?.clamp(0.0, n.height);
            (height > 0.0).then_some((fill, height))
        });
        if let Some(native) = native_box {
            let radius = native.radius.min(n.width / 2.0).min(n.height / 2.0);
            let (left, top, width, height, rx) =
                (fmt(x), fmt(y), fmt(n.width), fmt(n.height), fmt(radius));
            let _ = write!(
                &mut out,
                r#"<g class="basic label-container outer-path"><rect class="requirement-native-box" x="{left}" y="{top}" width="{width}" height="{height}" rx="{rx}" style="fill:{};stroke:{};stroke-width:{};{}"/>"#,
                escape_attr(fill_color),
                escape_attr(stroke_color),
                fmt(stroke_width),
                escape_attr(node_styles),
            );
            if let Some((fill, heading_height)) = heading {
                let inset = stroke_width / 2.0;
                let (left, right, top) = (x + inset, x + n.width - inset, y + inset);
                let bottom = y + heading_height;
                let r = (radius - inset)
                    .max(0.0)
                    .min((right - left) / 2.0)
                    .min(bottom - top);
                if r >= 0.0 && bottom > top && right > left {
                    let _ = write!(
                        &mut out,
                        r#"<path class="requirement-native-header" d="M{} {} Q{} {} {} {} H{} Q{} {} {} {} V{} H{} Z" style="fill:{};stroke:none"/>"#,
                        fmt(left),
                        fmt(top + r),
                        fmt(left),
                        fmt(top),
                        fmt(left + r),
                        fmt(top),
                        fmt(right - r),
                        fmt(right),
                        fmt(top),
                        fmt(right),
                        fmt(top + r),
                        fmt(bottom),
                        fmt(left),
                        escape_attr(fill),
                    );
                }
            }
            out.push_str("</g>");
        } else {
            let fill_path = format!(
                "M{} {} L{} {} L{} {} L{} {}",
                fmt(x),
                fmt(y),
                fmt(x + n.width),
                fmt(y),
                fmt(x + n.width),
                fmt(y + n.height),
                fmt(x),
                fmt(y + n.height)
            );
            let stroke_path = roughjs_paths_for_rect(RoughRectSpec {
                x,
                y,
                w: n.width,
                h: n.height,
                stroke_width: stroke_width as f32,
                work_meter: options.work_meter,
                phase: merman_model::OperationPhase::Emit,
                randomness: hand_drawn_seed,
            })
            .map(|(_, stroke_d)| stroke_d)
            .unwrap_or_else(|| rough_rect_stroke_path_d(x, y, n.width, n.height));

            let _ = write!(
                &mut out,
                r#"<g class="basic label-container outer-path" style="{style}">"#,
                style = escape_attr_display(node_styles)
            );
            let _ = write!(
                &mut out,
                r##"<path d="{d}" stroke="none" stroke-width="0" fill="{fill}"{style_attr}/>"##,
                d = escape_attr_display(&fill_path),
                fill = escape_attr_display(fill_color),
                style_attr = fill_style_attr,
            );
            let _ = write!(
                &mut out,
                r##"<path d="{d}" stroke="{stroke}" stroke-width="{stroke_width}" fill="none" stroke-dasharray="0 0"{style_attr}/>"##,
                d = escape_attr_display(&stroke_path),
                stroke = escape_attr_display(stroke_color),
                stroke_width = fmt(stroke_width),
                style_attr = stroke_style_attr,
            );
            out.push_str("</g>");
        }

        // Labels.
        let padding = 20.0;
        for line in &rendered_node.lines {
            let metrics = line.metrics;
            let label_x = if line.keep_centered {
                -metrics.width / 2.0
            } else {
                x + padding / 2.0
            };
            let label_y = y + line.y_offset - metrics.height / 2.0 + padding;
            // Requirement labels can extend above their node after upstream's centering step.
            // Reuse the final render measurement and the coordinates written below; SVG's extra
            // six pixels belong to stacking, while bbox_y retains createText's font-specific top.
            let label_top = cy + label_y + metrics.bbox_y;
            let label_height = if render_settings.html_labels {
                metrics.height
            } else {
                (metrics.height - 6.0).max(0.0)
            };
            content_bounds.min_x = content_bounds.min_x.min(cx + label_x);
            content_bounds.max_x = content_bounds.max_x.max(cx + label_x + metrics.width);
            content_bounds.min_y = content_bounds.min_y.min(label_top);
            content_bounds.max_y = content_bounds.max_y.max(label_top + label_height);
            let row_ink = native
                .filter(|_| source_text_color.is_none())
                .and_then(|native| {
                    if line.keep_centered && heading.is_some() {
                        return native.emphasis_ink.as_deref();
                    }
                    if line.source_index == 4 {
                        let req = req_by_id.get(n.id.as_str())?;
                        if !req.risk.trim().is_empty() {
                            return native.risk_ink(&req.risk);
                        }
                    }
                    None
                });
            let mut style = if line.bold {
                format!("{label_styles}; font-weight: bold;")
            } else {
                label_styles.to_owned()
            };
            if let Some(ink) = row_ink {
                let _ = write!(&mut style, ";color:{ink};");
            }
            let span_style = if style.trim().is_empty() {
                None
            } else {
                Some(style.as_str())
            };
            let div_style_prefix = {
                let mut p = String::new();
                if !label_div_style_prefix.is_empty() {
                    p.push_str(label_div_style_prefix);
                }
                if line.bold {
                    p.push_str("font-weight: bold; ");
                }
                if let Some(ink) = row_ink {
                    let _ = write!(&mut p, "color:{ink}; ");
                }
                if p.is_empty() { None } else { Some(p) }
            };
            let div_style_prefix = div_style_prefix.as_deref();
            let _ = write!(
                &mut out,
                r#"<g class="label" style="{style}" transform="translate({x}, {y})">"#,
                style = escape_attr_display(&style),
                x = fmt(label_x),
                y = fmt(label_y),
            );
            if render_settings.html_labels {
                let display_html = line
                    .html
                    .as_ref()
                    .ok_or_else(|| Error::InvalidModel {
                        message: format!(
                            "missing prepared Requirement XHTML for {} row {}",
                            n.id, line.source_index
                        ),
                    })?
                    .as_str();
                mk_label_foreign_object(
                    &mut out,
                    LabelForeignObject {
                        html: display_html,
                        width: metrics.width,
                        height: metrics.height,
                        span_class: "nodeLabel markdown-node-label",
                        span_style,
                        div_class: None,
                        div_style_prefix,
                        max_width_px: metrics.max_width_px,
                        text_align: if render_settings.body_text_start && !line.keep_centered {
                            "left"
                        } else {
                            "center"
                        },
                    },
                );
            } else {
                let source = requirement_label_source(&line.display_text, sanitize_config);
                let mut svg_style = if line.bold {
                    format!("{svg_text_styles}; font-weight: bold;")
                } else {
                    svg_text_styles.to_owned()
                };
                if let Some(ink) = row_ink {
                    let _ = write!(&mut svg_style, ";fill:{ink};");
                }
                let base_style = TextStyle {
                    font_weight: line.measurement_bold.then(|| "bold".to_owned()),
                    ..html_style_regular.clone()
                };
                let text_style = rendered_node.typography.resolve_text_style(&base_style);
                out.push_str(r#"<g><rect class="background" style="stroke: none"/>"#);
                label::write_svg_text_markdown_wrapped_with_first_row_style(
                    &mut out,
                    &source,
                    &svg_style,
                    label::SvgTextFirstRowStyle {
                        style: &style,
                        first_row_start: render_settings.body_text_start && !line.keep_centered,
                    },
                    measurer,
                    &text_style,
                    Some(metrics.max_width_px as f64),
                );
                out.push_str("</g>");
            }
            out.push_str("</g>");
        }

        if let Some(divider_y_offset) = rendered_node.divider_y_offset {
            let divider_y = y + divider_y_offset;
            if let Some(native) = native_box {
                let stroke = if visual.source_owns_stroke {
                    stroke_color
                } else {
                    &native.guide
                };
                let _ = write!(
                    &mut out,
                    r#"<line class="requirement-native-divider" x1="{}" x2="{}" y1="{}" y2="{}" style="stroke:{};stroke-width:{}"/>"#,
                    fmt(x),
                    fmt(x + n.width),
                    fmt(divider_y),
                    fmt(divider_y),
                    escape_attr(stroke),
                    fmt(stroke_width),
                );
            } else if look == "neo" {
                let (fill_d, stroke_d) = roughjs_paths_for_rect(RoughRectSpec {
                    x,
                    y: divider_y,
                    w: n.width,
                    h: 0.001,
                    stroke_width: stroke_width as f32,
                    work_meter: options.work_meter,
                    phase: merman_model::OperationPhase::Emit,
                    randomness: hand_drawn_seed,
                })
                .unwrap_or_else(|| {
                    (
                        format!(
                            "M{} {} L{} {} L{} {} L{} {}",
                            fmt(x),
                            fmt(divider_y),
                            fmt(x + n.width),
                            fmt(divider_y),
                            fmt(x + n.width),
                            fmt(divider_y + 0.001),
                            fmt(x),
                            fmt(divider_y + 0.001),
                        ),
                        rough_rect_stroke_path_d(x, divider_y, n.width, 0.001),
                    )
                });
                out.push_str(r#"<g class="divider""#);
                if let Some(opacity) = &visual.opacity {
                    let _ = write!(
                        &mut out,
                        r#" style="opacity:{} !important""#,
                        escape_attr_display(opacity)
                    );
                }
                write!(
                    &mut out,
                    r#"><path d="{}" stroke="none" stroke-width="0" fill="{}" fill-rule="evenodd"{fill_style_attr}/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0"{stroke_style_attr}/></g>"#,
                    escape_attr_display(&fill_d), escape_attr_display(fill_color), escape_attr_display(&stroke_d),
                    escape_attr_display(stroke_color), fmt(stroke_width),
                )
                .map_err(|_| Error::InvalidModel {
                    message: "Requirement divider terminal emission failed".to_string(),
                })?;
            } else {
                let divider_d =
                    if let Some(stroke) = roughjs_parse_hex_color_to_srgba(geometry_stroke_color) {
                        if let Ok(mut opts) = roughr::core::OptionsBuilder::default()
                            .randomness(hand_drawn_seed.clone())
                            .roughness(0.0)
                            .fill_style(roughr::core::FillStyle::Solid)
                            .stroke(stroke)
                            .stroke_width(stroke_width as f32)
                            .stroke_line_dash(vec![0.0, 0.0])
                            .stroke_line_dash_offset(0.0)
                            .fill_line_dash(vec![0.0, 0.0])
                            .fill_line_dash_offset(0.0)
                            .disable_multi_stroke(false)
                            .disable_multi_stroke_fill(false)
                            .build()
                        {
                            roughjs_ops_to_svg_path_d(&roughr::renderer::line::<f64>(
                                x,
                                divider_y,
                                x + n.width,
                                divider_y,
                                &mut opts,
                            ))
                        } else {
                            rough_double_line_path_d(x, divider_y, x + n.width, divider_y)
                        }
                    } else {
                        rough_double_line_path_d(x, divider_y, x + n.width, divider_y)
                    };
                write!(
                &mut out,
                r##"<g class="divider" style="{style}"><path d="{d}" stroke="{stroke}" stroke-width="{stroke_width}" fill="none" stroke-dasharray="0 0"{style_attr}/></g>"##,
                style = escape_attr_display(node_styles),
                d = escape_attr_display(&divider_d),
                stroke = escape_attr_display(stroke_color),
                stroke_width = fmt(stroke_width),
                style_attr = stroke_style_attr,
            )
            .map_err(|_| Error::InvalidModel {
                message: "Requirement divider terminal emission failed".to_string(),
            })?;
            }
            out.checkpoint()?;
        }

        out.push_str("</g>");
    }
    out.push_str("</g>");

    out.push_str("</g></g>");

    if let Some((title, title_x, title_y)) = title_position {
        let _ = write!(
            &mut out,
            r#"<text text-anchor="middle" x="{x}" y="{y}" class="requirementDiagramTitleText""#,
            x = fmt(title_x),
            y = fmt(title_y),
        );
        let _ = write!(&mut out, ">{}</text>", escape_xml(title));
        out.checkpoint()?;
    }

    look_defs.write_shadow_defs(&mut out, diagram_id)?;

    out.push_str("</svg>\n");
    let viewport_padding = render_settings.viewport_padding;
    let vb_x = content_bounds.min_x - viewport_padding;
    let vb_y = content_bounds.min_y - viewport_padding;
    let vb_w = ((content_bounds.max_x - content_bounds.min_x) + 2.0 * viewport_padding).max(1.0);
    let vb_h = ((content_bounds.max_y - content_bounds.min_y) + 2.0 * viewport_padding).max(1.0);
    let root_document = root_context.finish_document(
        &mut out,
        root_document,
        root_svg::RootViewportSpec::mermaid(
            root_svg::DiagramBounds::from_view_box(vb_x, vb_y, vb_w, vb_h),
            render_settings.use_max_width,
        )
        .with_max_width(root_svg::RootMaxWidth::Precision {
            value: vb_w,
            significant_digits: 6,
        }),
    )?;
    root_document.complete(out.finish()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_model::families::requirement::{RequirementElement, RequirementNode};
    use merman_render_core::resources::{RenderResourcePolicy, ResourceLimitId};

    #[test]
    fn requirement_color_css_serializes_the_look_as_a_css_string_inside_xml() {
        let mut svg = String::from("<style>");
        write_requirement_color_css(
            &mut XmlTextOutput::new(&mut svg),
            "requirement",
            "neo\"\\\n&<]]>",
            &["#123456".into()],
            &[],
        )
        .unwrap();
        svg.push_str("</style>");

        let parsed = roxmltree::Document::parse(&svg).unwrap();
        let css = parsed.root_element().text().unwrap();
        assert_eq!(css.matches(r#"[data-look="neo\"\\\a &<]]>"]"#).count(), 2);
    }

    #[test]
    fn palette_css_rejects_repeated_color_expansion_before_retaining_it() {
        let borders = vec!["#123456".to_string(); 4096];
        let backgrounds = vec![format!("rgb({}1 2 3)", " ".repeat(64 * 1024))];
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, 1024)
            .unwrap();
        let meter = merman_render_core::resources::OperationWorkMeter::new(policy);
        let mut out = BoundedSvgOutput::new(&meter);
        let error = super::write_requirement_color_css(
            &mut XmlTextOutput::new(&mut out),
            "bounded-palette",
            "classic",
            &borders,
            &backgrounds,
        )
        .expect_err("the expanded palette must fail before its first oversized color is retained");
        assert!(out.len() <= 1024);
        let merman_render_core::Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected SVG byte rejection, got {error}");
        };
        assert_eq!(limit.limit, ResourceLimitId::MaxSvgBytes.as_str());
        assert_eq!(limit.max, 1024);
        assert!(limit.actual > limit.max);
    }

    #[test]
    fn requirement_redux_css_follows_requirement_then_element_order() {
        let model = RequirementDiagramModel {
            requirements: vec![
                RequirementNode {
                    name: "requirement-a".to_string(),
                    node_type: "Requirement".to_string(),
                    requirement_id: String::new(),
                    text: String::new(),
                    risk: String::new(),
                    verify_method: String::new(),
                    css_styles: Vec::new(),
                    classes: Vec::new(),
                },
                RequirementNode {
                    name: "requirement-b".to_string(),
                    node_type: "Requirement".to_string(),
                    requirement_id: String::new(),
                    text: String::new(),
                    risk: String::new(),
                    verify_method: String::new(),
                    css_styles: Vec::new(),
                    classes: Vec::new(),
                },
            ],
            elements: vec![RequirementElement {
                name: "element-c".to_string(),
                element_type: "Element".to_string(),
                doc_ref: String::new(),
                css_styles: Vec::new(),
                classes: Vec::new(),
            }],
            acc_title: None,
            acc_descr: None,
            direction: String::new(),
            relationships: Vec::new(),
            classes: std::collections::BTreeMap::new(),
        };
        let indices = super::requirement_color_indices(&model);
        let borders = vec![
            "#e879f9".to_string(),
            "#2dd4bf".to_string(),
            "#fb923c".to_string(),
        ];
        let backgrounds = vec![
            "#fdf4ff".to_string(),
            "#f0fdfa".to_string(),
            "#fff7ed".to_string(),
        ];

        assert_eq!(indices.get("requirement-a"), Some(&0));
        assert_eq!(indices.get("requirement-b"), Some(&1));
        assert_eq!(indices.get("element-c"), Some(&2));
        assert_eq!(
            super::requirement_color_id(&borders, *indices.get("element-c").unwrap()).as_deref(),
            Some("color-2")
        );

        let css = super::requirement_color_css("requirement", "classic", &borders, &backgrounds);
        assert!(css.contains(
            r##"#requirement [data-look="classic"][data-color-id="color-0"].node path{stroke:#e879f9;fill:#fdf4ff;}"##
        ));
        assert!(css.contains(
            r##"#requirement [data-look="classic"][data-color-id="color-1"].node rect{stroke:#2dd4bf;fill:#f0fdfa;}"##
        ));

        let dark_css = super::requirement_color_css("requirement", "classic", &borders, &[]);
        assert!(dark_css.contains(
            r##"#requirement [data-look="classic"][data-color-id="color-0"].node path{stroke:#e879f9;}"##
        ));
        assert!(!dark_css.contains("fill:"), "{dark_css}");

        let config = serde_json::json!({
            "theme": "redux-color",
            "themeVariables": {
                "borderColorArray": borders,
                "bkgColorArray": backgrounds,
                "THEME_COLOR_LIMIT": 3
            }
        });
        let core_config = crate::requirement::test_config(config.clone());
        let meter = merman_render_core::resources::OperationWorkMeter::new(
            merman_render_core::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let plan = crate::requirement::RequirementPaintThemePlan::resolve(
            None,
            &core_config,
            &model,
            &meter,
        )
        .unwrap();
        let binding = plan.css();
        assert_eq!(
            super::requirement_color_id(&binding.border_colors, 5).as_deref(),
            Some("color-2")
        );

        let mut merged = String::new();
        super::write_requirement_stylesheet(
            &mut merged,
            "requirement-colors",
            binding,
            "classic",
            false,
        )
        .unwrap();
        let colors = merged.find("[data-color-id=").unwrap();
        let family = merged.find("#requirement-colors marker").unwrap();
        let root = merged.find("#requirement-colors :root").unwrap();
        assert!(colors < family && family < root, "{merged}");
    }
}
