use super::super::roughjs_common::RoughRectSpec;
use super::super::roughjs_common::parse_hex_color_to_srgba as roughjs_parse_hex_color_to_srgba;
use super::super::*;
use merman_core::diagrams::requirement::RequirementDiagramRenderModel;

// Requirement diagram SVG renderer implementation (split from parity.rs).

const REQUIREMENT_ROUGH_GEOMETRY_COLOR: &str = "#000000";

fn requirement_color_id(border_colors: &[String], color_index: usize) -> Option<String> {
    (!border_colors.is_empty()).then(|| format!("color-{}", color_index % border_colors.len()))
}

fn requirement_color_indices(
    model: &RequirementDiagramRenderModel,
) -> std::collections::HashMap<String, usize> {
    let mut indices = std::collections::HashMap::new();
    let mut index = 0usize;
    for node in &model.requirements {
        indices.insert(node.name.clone(), index);
        index += 1;
    }
    for node in &model.elements {
        indices.insert(node.name.clone(), index);
        index += 1;
    }
    indices
}

fn requirement_color_css<I>(
    diagram_id: I,
    data_look: &str,
    border_colors: &[String],
    background_colors: &[String],
    theme_color_limit: usize,
) -> String
where
    I: SvgDiagramIdValue,
{
    let mut out = String::new();
    let diagram_id = crate::svg::escape_css_identifier(diagram_id.semantic_value());
    for index in 0..theme_color_limit {
        let Some(border_color) = border_colors.get(index) else {
            continue;
        };
        let fill = background_colors
            .get(index)
            .map(|color| format!("fill:{color};"))
            .unwrap_or_default();
        let _ = write!(
            &mut out,
            r#"#{} [data-look="{}"][data-color-id="color-{}"].node path{{stroke:{};{}}}#{} [data-look="{}"][data-color-id="color-{}"].node rect{{stroke:{};{}}}"#,
            diagram_id.as_str(),
            escape_xml(data_look),
            index,
            border_color,
            fill,
            diagram_id.as_str(),
            escape_xml(data_look),
            index,
            border_color,
            fill,
        );
    }
    out
}

fn insert_requirement_color_css<I>(css: &mut String, diagram_id: I, color_css: &str)
where
    I: SvgDiagramIdValue,
{
    if color_css.is_empty() {
        return;
    }

    let escaped_id = crate::svg::escape_css_identifier(diagram_id.semantic_value());
    let family_rule = format!("#{escaped_id} marker");
    let root_rule = format!("#{escaped_id} :root");
    let insertion_point = css
        .find(&family_rule)
        .or_else(|| css.find(&root_rule))
        .unwrap_or(css.len());
    css.insert_str(insertion_point, color_css);
}

pub(crate) fn render_requirement_diagram_svg_model(
    prepared: &crate::requirement::RequirementPreparedArtifact,
    model: &RequirementDiagramRenderModel,
    paint_theme: &crate::requirement::RequirementPaintThemePlan,
    sanitize_config: &merman_core::MermaidConfig,
    diagram_title: Option<&str>,
    measurer: &dyn TextMeasurer,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let (layout, prepared_nodes, prepared_edges) = prepared.render_parts();
    let effective_config = sanitize_config.as_value();
    let label_measurements =
        prepared.label_measurements_for_render_with_typography(measurer, paint_theme.css());

    fn mermaid_markdown_to_html(raw: &str, sanitize_config: &merman_core::MermaidConfig) -> String {
        let sanitized = requirement_label_source(raw, sanitize_config);
        crate::text::mermaid_markdown_to_xhtml_label_fragment(&sanitized, true)
    }

    fn requirement_label_source(raw: &str, sanitize_config: &merman_core::MermaidConfig) -> String {
        let decoded = raw
            .replace("ﬂ°°", "&#")
            .replace("ﬂ°", "&")
            .replace("¶ß", ";");
        merman_core::sanitize::sanitize_text(&decoded, sanitize_config)
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
            .map(|s| format!(r#" style="{}""#, escape_xml(s)))
            .unwrap_or_default();
        let div_style_prefix = div_style_prefix.unwrap_or("");
        let constrained_width = max_width_px > 0 && (width - max_width_px as f64).abs() <= 1.0e-6;
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
            span_class = escape_xml(span_class),
            span_style_attr = span_style_attr,
            div_style_prefix = escape_xml(div_style_prefix),
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

    let diagram_id = options.diagram_id_or("requirement");
    let render_settings = crate::requirement::RequirementConfigView::new(effective_config)
        .render_settings_with_binding(paint_theme.css());
    let look = render_settings.look;
    let look = look.as_str();
    let look_attr = format!(r#" data-look="{}""#, escape_xml(look));
    let binding = paint_theme.css();
    let border_colors = &binding.border_colors;
    let background_colors = &binding.background_colors;
    let theme_color_limit = binding.theme_color_limit;
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
    let hand_drawn_seed = options.rough_randomness(
        render_settings.hand_drawn_seed,
        "render.requirement.roughjs",
    );
    let html_style_regular = TextStyle {
        font_family: font_family.clone(),
        font_size,
        font_weight: None,
        font_style: None,
    };

    let edge_identity = |edge: &crate::model::LayoutEdge| {
        dugong::graphlib::EdgeKey::new(&edge.from, &edge.to, Some(&edge.id))
    };
    let mut rendered_edge_paths = std::collections::HashMap::new();
    let mut rendered_edge_label_positions = std::collections::HashMap::new();
    for edge in &layout.edges {
        let identity = edge_identity(edge);
        let prepared_edge = prepared_edges
            .get(&identity)
            .ok_or_else(|| Error::InvalidModel {
                message: format!(
                    "missing prepared Requirement edge label for {} -> {} ({})",
                    edge.from, edge.to, edge.id
                ),
            })?;
        let rendered_d = if prepared.uses_elk() {
            if edge.points.len() <= 2 {
                super::super::curve::curve_linear_path_d(&edge.points)
            } else {
                super::super::curve::curve_rounded_path_d_and_bounds(&edge.points, 5.0, false, None)
                    .0
            }
        } else {
            curve_basis_path_d(&edge.points)
        };
        if rendered_edge_paths
            .insert(identity.clone(), rendered_d.clone())
            .is_some()
        {
            return Err(Error::InvalidModel {
                message: format!(
                    "duplicate rendered Requirement edge identity for {} -> {} ({})",
                    edge.from, edge.to, edge.id
                ),
            });
        }
        if prepared_edge.has_label {
            let label = edge.label.as_ref().ok_or_else(|| Error::InvalidModel {
                message: format!(
                    "missing Requirement edge label geometry for {} -> {} ({})",
                    edge.from, edge.to, edge.id
                ),
            })?;
            let label_position = super::super::edge_label_geometry::position_edge_label(
                crate::model::LayoutPoint {
                    x: label.x,
                    y: label.y,
                },
                Some(&edge.points),
                &edge.points,
                &rendered_d,
                false,
            );
            rendered_edge_label_positions.insert(identity, label_position);
        }
    }

    let hop_paths = if prepared.uses_elk()
        && effective_config
            .pointer("/elk/lineHops")
            .and_then(serde_json::Value::as_bool)
            != Some(false)
    {
        options.work_meter().charge(layout.edges.len())?;
        let hop_edges = layout
            .edges
            .iter()
            .map(|edge| super::super::line_hops::LineHopEdge {
                id: edge.id.as_str(),
                points: &edge.points,
                curve: Some(if edge.points.len() <= 2 {
                    "linear"
                } else {
                    "rounded"
                }),
                arrow_type_start: None,
                arrow_type_end: None,
            })
            .collect::<Vec<_>>();
        super::super::line_hops::elk_line_hop_paths(
            effective_config,
            &hop_edges,
            options.work_meter(),
        )?
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
        if let Some(path) = hop_paths.get(edge.id.as_str()) {
            options.work_meter().charge(path.len())?;
            if let Some(bounds) = super::super::svg_path_bounds_from_d(path) {
                min_x = min_x.min(bounds.min_x);
                min_y = min_y.min(bounds.min_y);
                max_x = max_x.max(bounds.max_x);
                max_y = max_y.max(bounds.max_y);
            }
        }
        for point in &edge.points {
            min_x = min_x.min(point.x);
            min_y = min_y.min(point.y);
            max_x = max_x.max(point.x);
            max_y = max_y.max(point.y);
        }
        let identity = edge_identity(edge);
        if let (Some(label), Some(position)) = (
            edge.label.as_ref(),
            rendered_edge_label_positions.get(&identity),
        ) {
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
        let (left, right) = measurer.measure_svg_title_bbox_x(title, &html_style_regular);
        let (ascent, descent) =
            crate::text::svg_title_bbox_vertical_extents_px(&html_style_regular);
        content_bounds.min_x = content_bounds.min_x.min(x - left);
        content_bounds.max_x = content_bounds.max_x.max(x + right);
        content_bounds.min_y = content_bounds.min_y.min(y - ascent);
        content_bounds.max_y = content_bounds.max_y.max(y + descent);
        (title, x, y)
    });
    let viewport_padding = render_settings.viewport_padding;
    let vb_x = content_bounds.min_x - viewport_padding;
    let vb_y = content_bounds.min_y - viewport_padding;
    let vb_w = ((content_bounds.max_x - content_bounds.min_x) + 2.0 * viewport_padding).max(1.0);
    let vb_h = ((content_bounds.max_y - content_bounds.min_y) + 2.0 * viewport_padding).max(1.0);
    let mut out = BoundedSvgOutput::new(options.work_meter());
    let mut aria_labelledby: Option<String> = None;
    let mut aria_describedby: Option<String> = None;
    let mut a11y_nodes = String::new();
    if let Some(t) = model
        .acc_title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        aria_labelledby = Some(format!("chart-title-{diagram_id}"));
        let _ = write!(
            &mut a11y_nodes,
            r#"<title id="chart-title-{}">{}</title>"#,
            diagram_id,
            escape_xml(t)
        );
    }
    if let Some(d) = model
        .acc_descr
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty())
    {
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
    let root_document =
        root_svg::RootViewportContext::new(crate::DiagramFamilyId::REQUIREMENT, diagram_id)
            .write_open(
                &mut out,
                root_svg::RootViewportSpec::mermaid(
                    root_svg::DiagramBounds::from_view_box(vb_x, vb_y, vb_w, vb_h),
                    render_settings.use_max_width,
                )
                .with_max_width(root_svg::RootMaxWidth::Precision {
                    value: vb_w,
                    significant_digits: 6,
                }),
                root_chrome,
            )?;

    out.push_str(&a11y_nodes);

    let relation_paint = paint_theme.relation_paint();
    let mut relation_receipt =
        relation_paint.and_then(|plan| plan.begin_terminal_receipt(layout.edges.len()));
    let mut expected_text_terminals = Vec::new();
    if paint_theme.text_paint().requested() {
        for edge in &layout.edges {
            options.work_meter().charge(1)?;
            if let Some(label) = prepared_edges
                .get(&edge_identity(edge))
                .filter(|label| label.has_label)
            {
                expected_text_terminals.push(crate::requirement::RequirementTextTerminalId::edge(
                    &label.rendered_id,
                ));
            }
        }
        for node in &layout.nodes {
            options.work_meter().charge(1)?;
            if let Some(crate::requirement::RequirementNodeRenderPlan::Semantic(labels)) =
                prepared_nodes.get(&node.id)
            {
                for line in &labels.lines {
                    expected_text_terminals.push(
                        crate::requirement::RequirementTextTerminalId::node(
                            &node.id,
                            line.source_index,
                        ),
                    );
                }
            }
        }
    }
    let mut text_paint_receipt = paint_theme
        .text_paint()
        .begin_terminal_receipt(expected_text_terminals, options.work_meter())?;
    let mut css_emission =
        super::css::write_requirement_css(diagram_id, binding, text_paint_receipt.as_mut());
    let color_css = requirement_color_css(
        diagram_id,
        look,
        border_colors,
        background_colors,
        theme_color_limit,
    );
    insert_requirement_color_css(&mut css_emission.css, diagram_id, &color_css);
    let _ = write!(&mut out, r#"<style>{}</style>"#, css_emission.css);
    if let Some(receipt) = relation_receipt.as_mut() {
        out.checkpoint()?;
        receipt.record_css(css_emission.typed_relation_color());
    }

    let mut paint_theme_receipt = paint_theme.begin_terminal_receipt();
    if let Some(receipt) = paint_theme_receipt.as_mut() {
        receipt.record_typography(css_emission.font_family(), css_emission.font_size());
    }
    out.push_str("<g>");

    // Mermaid 12 selects the Neo variants in requirementRenderer.ts.
    let marker_units = if look == "neo" {
        r#" markerUnits="userSpaceOnUse""#
    } else {
        ""
    };
    let marker_stroke = if look == "neo" {
        let width = &binding.marker_stroke_width;
        format!(r#" stroke-width="{}""#, escape_xml(width))
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
    if let Some(receipt) = relation_receipt.as_mut() {
        out.checkpoint()?;
        receipt.record_marker(0);
    }
    let _ = write!(
        &mut out,
        r#"<defs><marker id="{diagram_id}_requirement-requirement_arrowEnd" refX="20" refY="10" markerWidth="20" markerHeight="20" orient="auto"{marker_units}{marker_stroke}{arrow_view_box}><path d="M0,0&#10;      L20,10&#10;      M20,10&#10;      L0,20"{arrow_join}/></marker></defs>"#,
    );
    if let Some(receipt) = relation_receipt.as_mut() {
        out.checkpoint()?;
        receipt.record_marker(1);
    }
    options.checkpoint_emit()?;

    out.push_str(r#"<g class="root">"#);
    out.push_str(r#"<g class="clusters"/>"#);

    out.push_str(if prepared.uses_elk() {
        r#"<g class="edges edgePaths">"#
    } else {
        r#"<g class="edgePaths">"#
    });
    for (edge_index, e) in layout.edges.iter().enumerate() {
        let identity = edge_identity(e);
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
        let style = if is_contains {
            "fill:none;;;;fill:none;"
        } else {
            "fill:none;stroke-dasharray: 10,7;;;fill:none;stroke-dasharray: 10,7"
        };

        let d = rendered_edge_paths
            .get(&identity)
            .expect("Requirement edge paths were validated before root rendering");
        let mut masked_style = String::new();
        if look == "neo"
            && let Some(length) = super::super::svg_path_length_from_d(d)
        {
            super::super::edge_path::write_neo_edge_mask(
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
            super::super::line_hops::rewrite_style_after_line_hop(
                &masked_style,
                path,
                options.work_meter(),
            )?
        } else {
            std::borrow::Cow::Borrowed(masked_style.as_str())
        };
        let data_points_b64 =
            base64::engine::general_purpose::STANDARD.encode(json_stringify_points(&e.points));

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

        let dom_id = if has_diagram_id {
            format!("{diagram_id}-{}", prepared_label.rendered_id)
        } else {
            prepared_label.rendered_id.clone()
        };
        options.checkpoint_emit()?;

        let _ = write!(
            &mut out,
            r#"<path d="{d}" id="{dom_id}" class="{class}" style="{style}" data-edge="true" data-et="edge" data-id="{id}" data-points="{data_points}"{look_attr}{marker_attr}/>"#,
            d = escape_xml(hopped_d.unwrap_or(d)),
            dom_id = escape_xml(&dom_id),
            id = escape_xml(&prepared_label.rendered_id),
            class = escape_xml(&class),
            style = escape_xml(&style),
            data_points = escape_xml(&data_points_b64),
            look_attr = look_attr.as_str(),
            marker_attr = marker_attr,
        );
        if let Some(receipt) = relation_receipt.as_mut() {
            out.checkpoint()?;
            receipt.record_path(edge_index);
        }
    }
    out.push_str("</g>");

    out.push_str(r#"<g class="edgeLabels">"#);
    for e in &layout.edges {
        let identity = edge_identity(e);
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
            let label_position = rendered_edge_label_positions
                .get(&identity)
                .expect("Requirement label positions were validated before root rendering");
            let _ = write!(
                &mut out,
                r#"<g class="edgeLabel" transform="translate({x}, {y})"><g class="label" data-id="{id}" transform="translate({lx}, {ly})">"#,
                x = fmt(label_position.x),
                y = fmt(label_position.y),
                id = escape_xml(&prepared_label.rendered_id),
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
                id = escape_xml(&prepared_label.rendered_id),
            );
        }
        let mut text_facts = None;
        if render_settings.edge_html_labels {
            let label_html = mermaid_markdown_to_html(label_text, sanitize_config);
            if text_paint_receipt.is_some() && prepared_label.has_label {
                options.work_meter().charge(label_html.len())?;
                text_facts = Some(crate::text::VisibleTextStyleFacts::from_xhtml_fragment(
                    &label_html,
                ));
            }
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
            if text_paint_receipt.is_some() && prepared_label.has_label {
                options.work_meter().charge(label_text.len())?;
                text_facts = Some(
                    crate::text::VisibleTextStyleFacts::from_svg_markdown_projection(label_text),
                );
            }
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
            super::super::label::write_svg_text_markdown_wrapped_centered_from_create_text_source(
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
        if prepared_label.has_label
            && let Some(receipt) = paint_theme_receipt.as_mut()
        {
            receipt.record_edge_text(label_text);
        }
        if let Some(receipt) = text_paint_receipt.as_mut()
            && let Some(facts) = text_facts
        {
            out.checkpoint()?;
            receipt.record_label(
                crate::requirement::RequirementTextTerminalId::edge(&prepared_label.rendered_id),
                !render_settings.edge_html_labels,
                &facts,
                crate::requirement::RequirementTextColorOwner::Inherited,
                options.work_meter(),
            )?;
        }
    }
    out.push_str("</g>");

    out.push_str(r#"<g class="nodes">"#);
    for n in &layout.nodes {
        let cx = n.x + n.width / 2.0;
        let cy = n.y + n.height / 2.0;
        let prepared_node = prepared_nodes
            .get(&n.id)
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing prepared Requirement node plan for {}", n.id),
            })?;
        if matches!(
            prepared_node,
            crate::requirement::RequirementNodeRenderPlan::EdgeLabelAnchor
        ) {
            let _ = write!(
                &mut out,
                r#"<g class="label edgeLabel" id="{id}" transform="translate({cx}, {cy})"><rect width="0.1" height="0.1"/><g class="label" style="" transform="translate(0, 0)"><rect/>"#,
                id = escape_xml(&n.id),
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
                super::super::label::write_svg_text_markdown_from_create_text_source(
                    &mut out, "", true,
                );
                out.push_str("</g>");
            }
            out.push_str("</g></g>");
            continue;
        }
        let crate::requirement::RequirementNodeRenderPlan::Semantic(prepared_node) = prepared_node
        else {
            unreachable!("edge label anchors return before semantic rendering");
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
        let paint_theme_index = paint_theme_receipt
            .as_ref()
            .map(|_| {
                paint_theme
                    .index_for_node_id(&n.id)
                    .ok_or_else(|| Error::InvalidModel {
                        message: format!(
                            "Requirement paint theme plan is missing semantic node {}",
                            n.id
                        ),
                    })
            })
            .transpose()?;

        if !node_classes.contains(&"default") {
            node_classes.insert(0, "default");
        }
        let classes_str = if node_classes.is_empty() {
            "node default".to_string()
        } else {
            format!("node {}", node_classes.join(" "))
        };
        let id_attr = if has_diagram_id {
            format!(r#" id="{diagram_id}-{}""#, escape_xml(&n.id))
        } else {
            format!(r#" id="{}""#, escape_xml(&n.id))
        };
        options.checkpoint_emit()?;
        let color_id_attr = color_indices
            .get(n.id.as_str())
            .and_then(|index| requirement_color_id(border_colors, *index))
            .map(|color_id| format!(r#" data-color-id="{}""#, escape_xml(&color_id)))
            .unwrap_or_default();

        let _ = write!(
            &mut out,
            r#"<g class="{class}"{id_attr}{look_attr}{color_id_attr} transform="translate({cx}, {cy})">"#,
            class = escape_xml(&classes_str),
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
        let typed_fill = visual
            .typed_fill
            .as_ref()
            .map(|(rule, css)| (*rule, css.as_str()));
        let typed_stroke = visual
            .typed_stroke
            .as_ref()
            .map(|(rule, css)| (*rule, css.as_str()));
        let fill_color = visual.fill.as_str();
        let stroke_color = visual.stroke.as_str();
        let stroke_width = visual.stroke_width;
        let fill_style_attr = &visual.fill_style_attr;
        let stroke_style_declaration = &visual.stroke_style_declaration;
        let stroke_style_attr = &visual.stroke_style_attr;

        // RoughJS path geometry does not depend on RGB values. Keep paint-only values such as
        // `transparent` from selecting a different geometry fallback than an opaque color.
        let geometry_stroke_color = roughjs_parse_hex_color_to_srgba(stroke_color)
            .map_or(REQUIREMENT_ROUGH_GEOMETRY_COLOR, |_| stroke_color);

        let x = -n.width / 2.0;
        let y = -n.height / 2.0;
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
            randomness: &hand_drawn_seed,
        })
        .map(|(_, stroke_d)| stroke_d)
        .unwrap_or_else(|| rough_rect_stroke_path_d(x, y, n.width, n.height));

        let _ = write!(
            &mut out,
            r#"<g class="basic label-container outer-path" style="{style}">"#,
            style = escape_xml(node_styles)
        );
        let _ = write!(
            &mut out,
            r##"<path d="{d}" stroke="none" stroke-width="0" fill="{fill}"{style_attr}/>"##,
            d = escape_xml(&fill_path),
            fill = escape_xml(fill_color),
            style_attr = fill_style_attr,
        );
        let _ = write!(
            &mut out,
            r##"<path d="{d}" stroke="{stroke}" stroke-width="{stroke_width}" fill="none" stroke-dasharray="0 0"{style_attr}/>"##,
            d = escape_xml(&stroke_path),
            stroke = escape_xml(stroke_color),
            stroke_width = fmt(stroke_width),
            style_attr = stroke_style_attr,
        );
        out.push_str("</g>");

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
            let style = if line.bold {
                format!("{label_styles}; font-weight: bold;")
            } else {
                label_styles.to_owned()
            };
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
                if p.is_empty() { None } else { Some(p) }
            };
            let div_style_prefix = div_style_prefix.as_deref();
            let _ = write!(
                &mut out,
                r#"<g class="label" style="{style}" transform="translate({x}, {y})">"#,
                style = escape_xml(&style),
                x = fmt(label_x),
                y = fmt(label_y),
            );
            let mut text_facts = None;
            if render_settings.html_labels {
                let display_html = mermaid_markdown_to_html(&line.display_text, sanitize_config);
                if text_paint_receipt.is_some() {
                    options.work_meter().charge(display_html.len())?;
                    text_facts = Some(crate::text::VisibleTextStyleFacts::from_xhtml_fragment(
                        &display_html,
                    ));
                }
                mk_label_foreign_object(
                    &mut out,
                    LabelForeignObject {
                        html: &display_html,
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
                if text_paint_receipt.is_some() {
                    options.work_meter().charge(source.len())?;
                    text_facts = Some(
                        crate::text::VisibleTextStyleFacts::from_svg_markdown_projection(&source),
                    );
                }
                let svg_style = if line.bold {
                    format!("{svg_text_styles}; font-weight: bold;")
                } else {
                    svg_text_styles.to_owned()
                };
                let base_style = TextStyle {
                    font_weight: line.measurement_bold.then(|| "bold".to_owned()),
                    ..html_style_regular.clone()
                };
                let text_style = rendered_node.typography.resolve_text_style(&base_style);
                let node_measurer = rendered_node
                    .typography
                    .node_measurer(measurer, crate::text::WrapMode::SvgLike);
                out.push_str(r#"<g><rect class="background" style="stroke: none"/>"#);
                super::super::label::write_svg_text_markdown_wrapped_with_row_style(
                    &mut out,
                    &source,
                    &svg_style,
                    super::super::label::SvgTextRowStyle {
                        style: &style,
                        first_row_start: render_settings.body_text_start && !line.keep_centered,
                    },
                    node_measurer.as_measurer(),
                    &text_style,
                    Some(metrics.max_width_px as f64),
                );
                out.push_str("</g>");
            }
            out.push_str("</g>");
            if let Some(receipt) = text_paint_receipt.as_mut()
                && let Some(facts) = text_facts
            {
                out.checkpoint()?;
                let facts = if rendered_node.typography.has_zero_font_size() {
                    crate::text::VisibleTextStyleFacts::plain_text("")
                } else {
                    facts
                };
                receipt.record_label(
                    crate::requirement::RequirementTextTerminalId::node(&n.id, line.source_index),
                    false,
                    &facts,
                    crate::requirement::RequirementTextColorOwner::from_source_color(
                        source_text_color.as_deref(),
                    ),
                    options.work_meter(),
                )?;
            }
        }

        let divider_expected = rendered_node.divider_y_offset.is_some();
        let mut divider_emissions = Vec::new();
        if let Some(divider_y_offset) = rendered_node.divider_y_offset {
            let divider_y = y + divider_y_offset;
            if look == "neo" {
                let (fill_d, stroke_d) = roughjs_paths_for_rect(RoughRectSpec {
                    x,
                    y: divider_y,
                    w: n.width,
                    h: 0.001,
                    stroke_width: stroke_width as f32,
                    randomness: &hand_drawn_seed,
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
                write!(
                    &mut out,
                    r#"<g class="divider"><path d="{}" stroke="none" stroke-width="0" fill="{}" fill-rule="evenodd"{fill_style_attr}/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="0 0"{stroke_style_attr}/></g>"#,
                    escape_xml(&fill_d), escape_xml(fill_color), escape_xml(&stroke_d),
                    escape_xml(stroke_color), fmt(stroke_width),
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
                style = escape_xml(node_styles),
                d = escape_xml(&divider_d),
                stroke = escape_xml(stroke_color),
                stroke_width = fmt(stroke_width),
                style_attr = stroke_style_attr,
            )
            .map_err(|_| Error::InvalidModel {
                message: "Requirement divider terminal emission failed".to_string(),
            })?;
            }
            out.checkpoint()?;
            if let Some(index) = paint_theme_index {
                divider_emissions.push(
                    crate::requirement::RequirementDividerEmission::from_successful_write(
                        index,
                        stroke_color,
                        stroke_style_declaration.as_deref().unwrap_or_default(),
                    ),
                );
            }
        }

        if let Some(receipt) = paint_theme_receipt.as_mut() {
            receipt.record_checkpointed_node(
                paint_theme_index.expect("themed node index validated before emission"),
                rendered_node
                    .lines
                    .iter()
                    .any(|line| !line.display_text.trim().is_empty()),
                &rendered_node.typography,
                label_styles,
                visual.source_owns_fill,
                typed_fill,
                fill_color,
                visual.source_owns_stroke,
                typed_stroke,
                stroke_color,
                stroke_style_declaration.as_deref().unwrap_or_default(),
                divider_expected,
                &divider_emissions,
            );
        }

        out.push_str("</g>");
    }
    out.push_str("</g>");

    out.push_str("</g></g>");

    if let Some((title, title_x, title_y)) = title_position {
        let _ = write!(
            &mut out,
            r#"<text text-anchor="middle" x="{x}" y="{y}" class="requirementDiagramTitleText">{txt}</text>"#,
            x = fmt(title_x),
            y = fmt(title_y),
            txt = escape_xml(title),
        );
        if let Some(receipt) = paint_theme_receipt.as_mut() {
            receipt.record_title_text("requirementDiagramTitleText", title);
        }
    }

    push_requirement_shadow_defs(
        &mut out,
        diagram_id.semantic_str(),
        binding.shadow_flood_color,
    );

    out.push_str("</svg>\n");
    let rooted_svg = root_document.complete(out.finish()?)?;
    if let Some(receipt) = relation_receipt
        && !relation_paint
            .expect("relation receipt requires a plan")
            .record_terminal(receipt)
    {
        return Err(Error::InvalidModel {
            message: "incomplete Requirement relation paint terminal receipt".to_owned(),
        });
    }
    if !paint_theme.text_paint().record_terminal(text_paint_receipt) {
        return Err(Error::InvalidModel {
            message: "Requirement text paint terminal evidence could not be sealed".to_string(),
        });
    }
    if !paint_theme.record_terminal(paint_theme_receipt) {
        return Err(Error::InvalidModel {
            message: "Requirement paint theme terminal evidence could not be sealed".to_string(),
        });
    }
    Ok(rooted_svg)
}

fn push_requirement_shadow_defs(out: &mut impl SvgOutput, diagram_id: &str, flood_color: &str) {
    let _ = write!(
        out,
        r#"<defs><filter id="{}-drop-shadow" height="130%" width="130%"><feDropShadow dx="4" dy="4" stdDeviation="0" flood-opacity="0.06" flood-color="{}"/></filter></defs><defs><filter id="{}-drop-shadow-small" height="150%" width="150%"><feDropShadow dx="2" dy="2" stdDeviation="0" flood-opacity="0.06" flood-color="{}"/></filter></defs>"#,
        diagram_id, flood_color, diagram_id, flood_color
    );
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::DiagramFamilyId;
    use crate::environment::{RenderEnvironment, TextMeasurementPhase};
    use crate::resources::{
        RenderResourcePolicy, ResourceLimitCause, ResourceLimitId, ResourceLimitPhase,
    };
    use crate::svg::{SvgDebugOptions, SvgExecution, SvgRenderOptions, with_test_svg_execution};
    use crate::text::{DeterministicTextMeasurer, TextMeasurer, TextMetrics, TextStyle, WrapMode};
    use merman_core::diagrams::requirement::{
        RequirementDiagramRenderModel, RequirementRenderElement, RequirementRenderNode,
        RequirementRenderRelationship,
    };
    use std::cell::Cell;
    use std::collections::BTreeMap;

    fn report_call_count(session: &crate::environment::RenderSession) -> u64 {
        session
            .text_measurement_report()
            .entries()
            .iter()
            .map(crate::environment::TextMeasurementSummary::count)
            .sum()
    }

    #[derive(Default)]
    struct CountingRequirementMeasurer {
        inner: DeterministicTextMeasurer,
        mermaid_dimensions: Cell<usize>,
        wrapped: Cell<usize>,
    }

    impl TextMeasurer for CountingRequirementMeasurer {
        fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
            self.inner.measure(text, style)
        }

        fn measure_mermaid_calculate_text_dimensions(
            &self,
            text: &str,
            style: &TextStyle,
        ) -> TextMetrics {
            self.mermaid_dimensions
                .set(self.mermaid_dimensions.get() + 1);
            self.inner
                .measure_mermaid_calculate_text_dimensions(text, style)
        }

        fn measure_wrapped(
            &self,
            text: &str,
            style: &TextStyle,
            max_width: Option<f64>,
            wrap_mode: WrapMode,
        ) -> TextMetrics {
            self.wrapped.set(self.wrapped.get() + 1);
            self.inner
                .measure_wrapped(text, style, max_width, wrap_mode)
        }
    }

    fn empty_requirement_model() -> RequirementDiagramRenderModel {
        RequirementDiagramRenderModel {
            acc_title: None,
            acc_descr: None,
            direction: String::new(),
            requirements: Vec::new(),
            elements: Vec::new(),
            relationships: Vec::new(),
            classes: BTreeMap::new(),
        }
    }

    fn prepared_requirement_model() -> RequirementDiagramRenderModel {
        RequirementDiagramRenderModel {
            requirements: vec![RequirementRenderNode {
                name: "requirement-a".to_string(),
                node_type: "Requirement".to_string(),
                requirement_id: "REQ-1".to_string(),
                text: "The **prepared** requirement".to_string(),
                risk: "Low".to_string(),
                verify_method: "Test".to_string(),
                css_styles: Vec::new(),
                classes: Vec::new(),
            }],
            elements: vec![RequirementRenderElement {
                name: "element-b".to_string(),
                element_type: "System".to_string(),
                doc_ref: "DOC-1".to_string(),
                css_styles: Vec::new(),
                classes: Vec::new(),
            }],
            relationships: vec![RequirementRenderRelationship {
                rel_type: "satisfies".to_string(),
                src: "element-b".to_string(),
                dst: "requirement-a".to_string(),
            }],
            ..empty_requirement_model()
        }
    }

    fn requirement_node(name: &str) -> RequirementRenderNode {
        RequirementRenderNode {
            name: name.to_string(),
            node_type: "Requirement".to_string(),
            requirement_id: String::new(),
            text: String::new(),
            risk: String::new(),
            verify_method: String::new(),
            css_styles: Vec::new(),
            classes: Vec::new(),
        }
    }

    #[test]
    fn requirement_redux_colors_follow_requirement_then_element_order() {
        let model = RequirementDiagramRenderModel {
            requirements: vec![
                RequirementRenderNode {
                    name: "requirement-a".to_string(),
                    node_type: "Requirement".to_string(),
                    requirement_id: String::new(),
                    text: String::new(),
                    risk: String::new(),
                    verify_method: String::new(),
                    css_styles: Vec::new(),
                    classes: Vec::new(),
                },
                RequirementRenderNode {
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
            elements: vec![RequirementRenderElement {
                name: "element-c".to_string(),
                element_type: "Element".to_string(),
                doc_ref: String::new(),
                css_styles: Vec::new(),
                classes: Vec::new(),
            }],
            ..empty_requirement_model()
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

        let css = super::requirement_color_css("requirement", "classic", &borders, &backgrounds, 3);
        assert!(css.contains(
            r##"#requirement [data-look="classic"][data-color-id="color-0"].node path{stroke:#e879f9;fill:#fdf4ff;}"##
        ));
        assert!(css.contains(
            r##"#requirement [data-look="classic"][data-color-id="color-1"].node rect{stroke:#2dd4bf;fill:#f0fdfa;}"##
        ));

        let dark_css = super::requirement_color_css("requirement", "classic", &borders, &[], 3);
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
        let core_config = merman_core::MermaidConfig::from_value(config.clone());
        let meter = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let plan = crate::requirement::RequirementPaintThemePlan::resolve_with_title(
            None,
            &core_config,
            &model,
            None,
            &meter,
        )
        .unwrap();
        let binding = plan.css();
        assert_eq!(binding.theme_color_limit, 3);

        let mut merged =
            super::super::css::write_requirement_css("requirement-colors", binding, None).css;
        let generated = super::requirement_color_css(
            "requirement-colors",
            "classic",
            &binding.border_colors,
            &binding.background_colors,
            3,
        );
        super::insert_requirement_color_css(&mut merged, "requirement-colors", &generated);
        let colors = merged.find("[data-color-id=").unwrap();
        let family = merged.find("#requirement-colors marker").unwrap();
        let root = merged.find("#requirement-colors :root").unwrap();
        assert!(colors < family && family < root, "{merged}");

        let options = SvgRenderOptions {
            diagram_id: Some("requirement-colors".to_string()),
            ..SvgRenderOptions::default()
        };
        let svg = render_requirement_for_test(
            &model,
            &config,
            None,
            &crate::text::DeterministicTextMeasurer::default(),
            &options,
        )
        .unwrap();

        assert!(svg.contains(
            r#"id="requirement-colors-requirement-a" data-look="classic" data-color-id="color-0""#
        ));
        assert!(svg.contains(
            r#"id="requirement-colors-requirement-b" data-look="classic" data-color-id="color-1""#
        ));
        assert!(svg.contains(
            r#"id="requirement-colors-element-c" data-look="classic" data-color-id="color-2""#
        ));
    }

    fn root_view_box(svg: &str) -> Vec<f64> {
        let root_open = svg.split_once('>').expect("root svg open tag").0;
        let value = root_open
            .split_once("viewBox=\"")
            .and_then(|(_, rest)| rest.split_once('"'))
            .map(|(value, _)| value)
            .expect("root viewBox");
        value
            .split_whitespace()
            .map(|part| part.parse::<f64>().expect("numeric viewBox part"))
            .collect()
    }

    fn render_requirement_for_test(
        model: &RequirementDiagramRenderModel,
        effective_config: &serde_json::Value,
        diagram_title: Option<&str>,
        measurer: &dyn TextMeasurer,
        request: &SvgRenderOptions,
    ) -> crate::Result<String> {
        let effective_config = merman_core::MermaidConfig::from_value(effective_config.clone());
        let prepared = crate::requirement::layout_requirement_diagram_typed_with_resource_policy(
            model,
            effective_config.as_value(),
            measurer,
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        )?;
        render_prepared_requirement_for_test(
            &prepared,
            model,
            &effective_config,
            diagram_title,
            measurer,
            request,
        )
    }

    fn render_prepared_requirement_for_test(
        prepared: &crate::requirement::RequirementPreparedArtifact,
        model: &RequirementDiagramRenderModel,
        effective_config: &merman_core::MermaidConfig,
        diagram_title: Option<&str>,
        measurer: &dyn TextMeasurer,
        request: &SvgRenderOptions,
    ) -> crate::Result<String> {
        with_test_svg_execution(DiagramFamilyId::REQUIREMENT, request, |options| {
            let paint_theme = crate::requirement::RequirementPaintThemePlan::resolve_with_title(
                None,
                effective_config,
                model,
                diagram_title,
                options.work_meter(),
            )?;
            render_requirement_diagram_svg_model(
                prepared,
                model,
                &paint_theme,
                effective_config,
                diagram_title,
                measurer,
                options,
            )
        })
        .and_then(|svg| svg.into_string_for(DiagramFamilyId::REQUIREMENT))
    }

    fn render_requirement_with_policy(policy: RenderResourcePolicy) -> crate::Result<String> {
        let model = prepared_requirement_model();
        let effective_config = merman_core::MermaidConfig::from_value(serde_json::json!({}));
        let measurer = DeterministicTextMeasurer::default();
        let prepared = crate::requirement::layout_requirement_diagram_typed_with_resource_policy(
            &model,
            effective_config.as_value(),
            &measurer,
            policy,
        )?;
        let session = RenderEnvironment::deterministic()
            .with_resource_policy(policy)
            .begin_session()
            .expect("render session");
        let request = SvgRenderOptions {
            diagram_id: Some("requirement-bounded".to_string()),
            ..SvgRenderOptions::default()
        };
        let debug = SvgDebugOptions::default();
        let execution = SvgExecution::unthemed_for_test(
            &request,
            &debug,
            &session,
            DiagramFamilyId::REQUIREMENT,
        )
        .expect("SVG execution");
        let paint_theme = crate::requirement::RequirementPaintThemePlan::resolve_with_title(
            None,
            &effective_config,
            &model,
            None,
            execution.work_meter(),
        )?;

        render_requirement_diagram_svg_model(
            &prepared,
            &model,
            &paint_theme,
            &effective_config,
            None,
            &measurer,
            &execution,
        )?
        .into_string_for(DiagramFamilyId::REQUIREMENT)
    }

    #[test]
    fn requirement_family_svg_accepts_exact_max_svg_bytes_and_rejects_one_byte_less() {
        let baseline =
            render_requirement_with_policy(RenderResourcePolicy::unbounded_for_trusted_input())
                .expect("render unbounded Requirement SVG");
        let exact_bytes = baseline.len();
        assert!(exact_bytes > 1);

        let exact = render_requirement_with_policy(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxSvgBytes, exact_bytes)
                .expect("valid exact Requirement SVG ceiling"),
        )
        .expect("exact Requirement SVG ceiling must succeed");
        assert_eq!(exact.as_bytes(), baseline.as_bytes());

        let below_exact = exact_bytes - 1;
        let error = render_requirement_with_policy(
            RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxSvgBytes, below_exact)
                .expect("valid below-exact Requirement SVG ceiling"),
        )
        .expect_err("one byte below the Requirement SVG size must fail");
        let crate::Error::ResourceLimitExceeded(limit) = error else {
            panic!("expected Requirement MaxSvgBytes rejection, got {error}");
        };
        assert_eq!(limit.cause, ResourceLimitCause::Ceiling);
        assert_eq!(limit.phase, ResourceLimitPhase::SvgOutput);
        assert_eq!(limit.limit, ResourceLimitId::MaxSvgBytes.as_str());
        assert_eq!(limit.max, below_exact);
        assert!(limit.actual > limit.max);
    }

    #[test]
    fn requirement_opaque_measurer_preserves_render_stage_label_measurements() {
        let model = prepared_requirement_model();
        let config = merman_core::MermaidConfig::from_value(serde_json::json!({}));
        let measurer = CountingRequirementMeasurer::default();
        let prepared = crate::requirement::layout_requirement_diagram_typed_with_resource_policy(
            &model,
            config.as_value(),
            &measurer,
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .unwrap();
        let dimensions_after_prepare = measurer.mermaid_dimensions.get();
        let wrapped_after_prepare = measurer.wrapped.get();

        assert!(dimensions_after_prepare > 0);
        assert!(wrapped_after_prepare > 0);

        render_prepared_requirement_for_test(
            &prepared,
            &model,
            &config,
            None,
            &measurer,
            &SvgRenderOptions::default(),
        )
        .unwrap();

        assert_eq!(
            measurer.mermaid_dimensions.get() - dimensions_after_prepare,
            dimensions_after_prepare
        );
        assert_eq!(
            measurer.wrapped.get() - wrapped_after_prepare,
            wrapped_after_prepare
        );
    }

    #[test]
    fn requirement_routed_builtin_measurements_are_reused_during_svg_emission() {
        let model = prepared_requirement_model();
        let config = merman_core::MermaidConfig::from_value(serde_json::json!({}));
        let environment = RenderEnvironment::deterministic().with_resource_policy(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let session = environment.begin_session().expect("deterministic session");
        let measurer = session.text_measurer(TextMeasurementPhase::Layout);
        let prepared = crate::requirement::layout_requirement_diagram_typed_with_resource_policy(
            &model,
            config.as_value(),
            &measurer,
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .unwrap();
        let calls_after_prepare = report_call_count(&session);

        assert!(calls_after_prepare > 0);

        render_prepared_requirement_for_test(
            &prepared,
            &model,
            &config,
            None,
            &measurer,
            &SvgRenderOptions::default(),
        )
        .unwrap();

        assert_eq!(report_call_count(&session), calls_after_prepare);
    }

    #[test]
    fn requirement_prepared_measurements_are_not_reused_across_style_changes() {
        let model = prepared_requirement_model();
        let prepare_config = merman_core::MermaidConfig::from_value(serde_json::json!({}));
        let render_config = merman_core::MermaidConfig::from_value(serde_json::json!({
            "themeVariables": {
                "fontFamily": "Courier New",
                "fontSize": "24px"
            }
        }));
        let environment = RenderEnvironment::deterministic().with_resource_policy(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let session = environment.begin_session().expect("deterministic session");
        let measurer = session.text_measurer(TextMeasurementPhase::Layout);
        let prepared = crate::requirement::layout_requirement_diagram_typed_with_resource_policy(
            &model,
            prepare_config.as_value(),
            &measurer,
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .unwrap();
        let calls_after_prepare = report_call_count(&session);

        render_prepared_requirement_for_test(
            &prepared,
            &model,
            &render_config,
            None,
            &measurer,
            &SvgRenderOptions::default(),
        )
        .unwrap();

        assert!(report_call_count(&session) > calls_after_prepare);
    }

    #[test]
    fn requirement_prepared_measurements_are_not_reused_across_label_modes() {
        let model = prepared_requirement_model();
        let prepare_config =
            merman_core::MermaidConfig::from_value(serde_json::json!({"htmlLabels":true}));
        let render_config =
            merman_core::MermaidConfig::from_value(serde_json::json!({"htmlLabels":false}));
        let session = RenderEnvironment::deterministic()
            .with_resource_policy(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            )
            .begin_session()
            .unwrap();
        let measurer = session.text_measurer(TextMeasurementPhase::Layout);
        let prepared = crate::requirement::layout_requirement_diagram_typed_with_resource_policy(
            &model,
            prepare_config.as_value(),
            &measurer,
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .unwrap();
        let calls_after_prepare = report_call_count(&session);
        let svg = render_prepared_requirement_for_test(
            &prepared,
            &model,
            &render_config,
            None,
            &measurer,
            &SvgRenderOptions::default(),
        )
        .unwrap();
        assert!(report_call_count(&session) > calls_after_prepare);
        assert!(!svg.contains("foreignObject"));
    }

    #[test]
    fn requirement_prepared_labels_keep_markdown_and_strict_sanitization_at_render_time() {
        let mut model = prepared_requirement_model();
        model.requirements[0].text = concat!(
            "**safe** ",
            r#"<a href="jav&#x61;script:alert(1)" onclick="alert(2)">bad</a>"#,
            "<script>alert(3)</script>"
        )
        .to_string();
        let measurer = DeterministicTextMeasurer::default();
        for html_labels in [true, false] {
            let config = merman_core::MermaidConfig::from_value(serde_json::json!({
                "securityLevel": "strict", "htmlLabels": html_labels
            }));
            let prepared =
                crate::requirement::layout_requirement_diagram_typed_with_resource_policy(
                    &model,
                    config.as_value(),
                    &measurer,
                    crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
                )
                .unwrap();

            let svg = render_prepared_requirement_for_test(
                &prepared,
                &model,
                &config,
                None,
                &measurer,
                &SvgRenderOptions::default(),
            )
            .unwrap();

            if html_labels {
                assert!(svg.contains("<strong>safe</strong>"), "{svg}");
            } else {
                let document = roxmltree::Document::parse(&svg).expect("valid SVG labels");
                assert!(
                    !document
                        .descendants()
                        .any(|node| node.has_tag_name("foreignObject"))
                );
                assert!(document.descendants().any(|node| node.has_tag_name("tspan")
                    && node.attribute("font-weight") == Some("bold")
                    && node.text().is_some_and(|text| text.trim() == "safe")));
            }
            assert!(!svg.contains("<script"), "{svg}");
            assert!(!svg.contains("onclick="), "{svg}");
            assert!(!svg.contains("<a href="), "{svg}");
        }
    }

    #[test]
    fn requirement_prepared_edge_plan_keeps_parallel_relationships() {
        let mut model = prepared_requirement_model();
        model.relationships = vec![
            RequirementRenderRelationship {
                rel_type: "satisfies".to_string(),
                src: "element-b".to_string(),
                dst: "requirement-a".to_string(),
            },
            RequirementRenderRelationship {
                rel_type: "contains".to_string(),
                src: "element-b".to_string(),
                dst: "requirement-a".to_string(),
            },
        ];
        let config = merman_core::MermaidConfig::from_value(serde_json::json!({}));
        let measurer = DeterministicTextMeasurer::default();
        let prepared = crate::requirement::layout_requirement_diagram_typed_with_resource_policy(
            &model,
            config.as_value(),
            &measurer,
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .unwrap();

        let svg = render_prepared_requirement_for_test(
            &prepared,
            &model,
            &config,
            None,
            &measurer,
            &SvgRenderOptions::default(),
        )
        .unwrap();

        assert!(svg.contains("&lt;&lt;contains&gt;&gt;"), "{svg}");
        assert!(svg.contains("&lt;&lt;satisfies&gt;&gt;"), "{svg}");
        assert!(svg.contains("edge-pattern-solid relationshipLine"), "{svg}");
        assert!(
            svg.contains("edge-pattern-dashed relationshipLine"),
            "{svg}"
        );
        assert!(
            svg.contains(r#"data-id="element-b-requirement-a-0""#),
            "{svg}"
        );
        assert!(
            svg.contains(r#"data-id="element-b-requirement-a-1""#),
            "{svg}"
        );
    }

    #[test]
    fn requirement_prepared_edge_plan_disambiguates_colliding_endpoint_text() {
        let model = RequirementDiagramRenderModel {
            requirements: vec![
                requirement_node("a-b"),
                requirement_node("c"),
                requirement_node("a"),
                requirement_node("b-c"),
            ],
            relationships: vec![
                RequirementRenderRelationship {
                    rel_type: "contains".to_string(),
                    src: "a-b".to_string(),
                    dst: "c".to_string(),
                },
                RequirementRenderRelationship {
                    rel_type: "satisfies".to_string(),
                    src: "a".to_string(),
                    dst: "b-c".to_string(),
                },
            ],
            ..empty_requirement_model()
        };
        let config = merman_core::MermaidConfig::from_value(serde_json::json!({}));
        let measurer = DeterministicTextMeasurer::default();
        let prepared = crate::requirement::layout_requirement_diagram_typed_with_resource_policy(
            &model,
            config.as_value(),
            &measurer,
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .unwrap();

        let (layout, _, edge_plans) = prepared.render_parts();
        assert_eq!(layout.edges.len(), 2);
        for edge in &layout.edges {
            let identity =
                dugong::graphlib::EdgeKey::new(&edge.from, &edge.to, Some(edge.id.as_str()));
            let label_plan = edge_plans.get(&identity).expect("prepared edge identity");
            match (edge.from.as_str(), edge.to.as_str()) {
                ("a-b", "c") => {
                    assert_eq!(edge.id, "a-b-c-0");
                    assert_eq!(label_plan.relationship_type, "contains");
                }
                ("a", "b-c") => {
                    assert_eq!(edge.id, "a-b-c-1");
                    assert_eq!(label_plan.relationship_type, "satisfies");
                }
                endpoints => panic!("unexpected Requirement edge: {endpoints:?}"),
            }
        }

        let svg = render_prepared_requirement_for_test(
            &prepared,
            &model,
            &config,
            None,
            &measurer,
            &SvgRenderOptions::default(),
        )
        .unwrap();

        assert_eq!(svg.matches("&lt;&lt;contains&gt;&gt;").count(), 1, "{svg}");
        assert_eq!(svg.matches("&lt;&lt;satisfies&gt;&gt;").count(), 1, "{svg}");
        assert_eq!(
            svg.matches("edge-pattern-solid relationshipLine").count(),
            1,
            "{svg}"
        );
        assert_eq!(
            svg.matches("edge-pattern-dashed relationshipLine").count(),
            1,
            "{svg}"
        );
        assert_eq!(svg.matches(r#"marker-start="url(#"#).count(), 1, "{svg}");
        assert_eq!(svg.matches(r#"marker-end="url(#"#).count(), 1, "{svg}");

        let document = roxmltree::Document::parse(&svg).expect("valid Requirement SVG");
        let edge_label_max_widths = document
            .descendants()
            .filter(|node| {
                node.has_tag_name("div")
                    && node
                        .attribute("class")
                        .unwrap_or_default()
                        .split_whitespace()
                        .any(|class| class == "labelBkg")
            })
            .map(|node| node.attribute("style").unwrap_or_default())
            .collect::<Vec<_>>();
        assert_eq!(edge_label_max_widths.len(), 2, "{svg}");
        assert!(
            edge_label_max_widths
                .iter()
                .all(|style| style.contains("max-width: 200px;")),
            "Requirement edge labels keep Mermaid's fixed 200px wrap cap: {svg}"
        );
    }

    #[test]
    fn requirement_prepared_source_paints_reach_actual_paths_in_both_label_modes() {
        let mut node = requirement_node("source-node");
        node.css_styles = vec![
            "fill:#112233 !important".to_owned(),
            "stroke:#445566 !important".to_owned(),
            "stroke-width:2px".to_owned(),
            "color:#778899".to_owned(),
        ];
        let model = RequirementDiagramRenderModel {
            requirements: vec![node],
            ..empty_requirement_model()
        };
        for html in [false, true] {
            let svg = render_requirement_for_test(
                &model,
                &serde_json::json!({"htmlLabels": html, "layout": "dagre"}),
                None,
                &crate::text::DeterministicTextMeasurer::default(),
                &SvgRenderOptions::default(),
            )
            .expect("render prepared source paints");
            let document = roxmltree::Document::parse(&svg).expect("valid SVG");
            assert!(
                document
                    .descendants()
                    .any(|element| element.has_tag_name("path")
                        && element.attribute("fill") == Some("#112233")),
                "{svg}"
            );
            assert!(
                document
                    .descendants()
                    .any(|element| element.has_tag_name("path")
                        && element.attribute("stroke") == Some("#445566")
                        && element.attribute("stroke-width") == Some("2")),
                "{svg}"
            );
            assert!(!svg.contains("!important !important"), "{svg}");
        }
    }

    #[test]
    fn requirement_self_loop_preserves_pinned_segment_identity_and_dom() {
        let model = RequirementDiagramRenderModel {
            requirements: vec![requirement_node("req1")],
            relationships: vec![RequirementRenderRelationship {
                rel_type: "satisfies".to_string(),
                src: "req1".to_string(),
                dst: "req1".to_string(),
            }],
            ..empty_requirement_model()
        };
        let config = merman_core::MermaidConfig::from_value(serde_json::json!({}));
        let measurer = DeterministicTextMeasurer::default();
        let prepared = crate::requirement::layout_requirement_diagram_typed_with_resource_policy(
            &model,
            config.as_value(),
            &measurer,
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .unwrap();

        let (layout, node_plans, edge_plans) = prepared.render_parts();
        assert_eq!(layout.nodes.len(), 3);
        assert!(matches!(
            node_plans.get("req1---req1---1"),
            Some(crate::requirement::RequirementNodeRenderPlan::EdgeLabelAnchor)
        ));
        assert!(matches!(
            node_plans.get("req1---req1---2"),
            Some(crate::requirement::RequirementNodeRenderPlan::EdgeLabelAnchor)
        ));
        assert_eq!(
            layout
                .edges
                .iter()
                .map(|edge| edge.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "req1-cyclic-special-0",
                "req1-cyclic-special-1",
                "req1-cyclic-special-2",
            ]
        );
        let rendered_segments = layout
            .edges
            .iter()
            .map(|edge| {
                let identity = dugong::graphlib::EdgeKey::new(&edge.from, &edge.to, Some(&edge.id));
                let plan = edge_plans.get(&identity).expect("self-loop edge plan");
                (
                    plan.rendered_id.as_str(),
                    plan.has_label,
                    plan.marker_start,
                    plan.marker_end,
                    edge.label.is_some(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            rendered_segments,
            vec![
                ("req1-cyclic-special-1", false, false, false, false),
                ("req1-cyclic-special-mid", true, false, false, true),
                ("req1-cyclic-special-2", false, false, true, false),
            ]
        );

        let options = SvgRenderOptions {
            diagram_id: Some("requirement-self-loop".to_string()),
            ..SvgRenderOptions::default()
        };
        let svg = render_prepared_requirement_for_test(
            &prepared, &model, &config, None, &measurer, &options,
        )
        .unwrap();

        for rendered_id in [
            "req1-cyclic-special-1",
            "req1-cyclic-special-mid",
            "req1-cyclic-special-2",
        ] {
            assert!(
                svg.contains(&format!(r#"data-id="{rendered_id}""#)),
                "{svg}"
            );
        }
        assert!(!svg.contains("req1-req1-0"), "{svg}");
        assert_eq!(svg.matches("&lt;&lt;satisfies&gt;&gt;").count(), 1, "{svg}");
        assert_eq!(svg.matches(r#"marker-start="url(#"#).count(), 0, "{svg}");
        assert_eq!(svg.matches(r#"marker-end="url(#"#).count(), 1, "{svg}");
        assert!(svg.contains(
            r#"<g class="edgeLabel"><g class="label" data-id="req1-cyclic-special-1" transform="translate(0, 0)">"#
        ));
        assert!(
            svg.contains(
                r#"<g class="label edgeLabel" id="req1---req1---1" transform="translate("#
            )
        );
        assert!(svg.contains(r#"<rect width="0.1" height="0.1"/>"#));

        let middle_edge = layout
            .edges
            .iter()
            .find(|edge| edge.id == "req1-cyclic-special-1")
            .expect("middle self-loop segment");
        let middle_label = middle_edge.label.as_ref().expect("middle label geometry");
        let middle_path = crate::svg::parity::curve_basis_path_d(&middle_edge.points);
        let final_position = crate::svg::parity::edge_label_geometry::position_edge_label(
            crate::model::LayoutPoint {
                x: middle_label.x,
                y: middle_label.y,
            },
            Some(&middle_edge.points),
            &middle_edge.points,
            &middle_path,
            false,
        );
        let view_box = root_view_box(&svg);
        assert!(
            final_position.x - middle_label.width / 2.0 >= view_box[0]
                && final_position.x + middle_label.width / 2.0 <= view_box[0] + view_box[2]
                && final_position.y - middle_label.height / 2.0 >= view_box[1]
                && final_position.y + middle_label.height / 2.0 <= view_box[1] + view_box[3],
            "final label bounds must be inside the root viewport: {view_box:?}"
        );
    }

    #[test]
    fn requirement_contains_self_loop_keeps_only_the_first_segment_start_marker() {
        let model = RequirementDiagramRenderModel {
            requirements: vec![requirement_node("req1")],
            relationships: vec![RequirementRenderRelationship {
                rel_type: "contains".to_string(),
                src: "req1".to_string(),
                dst: "req1".to_string(),
            }],
            ..empty_requirement_model()
        };
        let config = merman_core::MermaidConfig::from_value(serde_json::json!({}));
        let measurer = DeterministicTextMeasurer::default();
        let prepared = crate::requirement::layout_requirement_diagram_typed_with_resource_policy(
            &model,
            config.as_value(),
            &measurer,
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .unwrap();
        let svg = render_prepared_requirement_for_test(
            &prepared,
            &model,
            &config,
            None,
            &measurer,
            &SvgRenderOptions::default(),
        )
        .unwrap();

        assert_eq!(svg.matches(r#"marker-start="url(#"#).count(), 1, "{svg}");
        assert_eq!(svg.matches(r#"marker-end="url(#"#).count(), 0, "{svg}");
        let document = roxmltree::Document::parse(&svg).expect("valid Requirement SVG");
        let path_for = |id: &str| {
            document
                .descendants()
                .find(|node| node.has_tag_name("path") && node.attribute("data-id") == Some(id))
                .unwrap_or_else(|| panic!("missing self-loop path {id}"))
        };
        assert!(
            path_for("req1-cyclic-special-1")
                .attribute("marker-start")
                .is_some_and(|value| value.ends_with("requirement_containsStart)"))
        );
        assert_eq!(
            path_for("req1-cyclic-special-mid").attribute("marker-start"),
            None
        );
        assert_eq!(
            path_for("req1-cyclic-special-2").attribute("marker-end"),
            None
        );
    }

    #[test]
    fn requirement_root_honors_disabled_max_width() {
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let options = SvgRenderOptions {
            diagram_id: Some("requirementFixed".to_string()),
            ..SvgRenderOptions::default()
        };

        let svg = render_requirement_for_test(
            &empty_requirement_model(),
            &serde_json::json!({"requirement": {"useMaxWidth": false}}),
            None,
            &measurer,
            &options,
        )
        .unwrap();
        let root_open = svg.split_once('>').expect("root svg open tag").0;

        assert!(root_open.contains(r#"width="16""#), "{root_open}");
        assert!(root_open.contains(r#"height="16""#), "{root_open}");
        assert!(
            root_open.contains(r#"viewBox="-8 -8 16 16""#),
            "{root_open}"
        );
        assert!(!root_open.contains("style="), "{root_open}");
        assert!(!root_open.contains("max-width"), "{root_open}");
    }

    #[test]
    fn requirement_root_is_derived_for_formerly_pinned_fixture_ids() {
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let pinned_options = SvgRenderOptions {
            diagram_id: Some(
                "upstream_cypress_requirementdiagram_unified_spec_example_025".to_string(),
            ),
            ..SvgRenderOptions::default()
        };
        let control_options = SvgRenderOptions {
            diagram_id: Some("requirement-control".to_string()),
            ..SvgRenderOptions::default()
        };
        let model = prepared_requirement_model();
        let config = serde_json::json!({});

        let pinned_svg =
            render_requirement_for_test(&model, &config, None, &measurer, &pinned_options).unwrap();
        let control_svg =
            render_requirement_for_test(&model, &config, None, &measurer, &control_options)
                .unwrap();
        let root_open = pinned_svg.split_once('>').expect("root svg open tag").0;

        assert_eq!(root_view_box(&pinned_svg), root_view_box(&control_svg));
        assert!(root_open.contains("max-width:"), "{root_open}");
    }

    #[test]
    fn requirement_title_uses_pre_title_bounds_and_state_margin() {
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let options = SvgRenderOptions {
            diagram_id: Some("requirementTitle".to_string()),
            ..SvgRenderOptions::default()
        };

        let svg = render_requirement_for_test(
            &empty_requirement_model(),
            &serde_json::json!({"state": {"titleTopMargin": 33}}),
            Some("simple Requirement diagram"),
            &measurer,
            &options,
        )
        .unwrap();
        let view_box = root_view_box(&svg);

        assert!(svg.contains(r#"x="0" y="-33" class="requirementDiagramTitleText""#));
        assert!(view_box[0] < 0.0, "{view_box:?}");
        assert!(view_box[1] < -33.0, "{view_box:?}");
        assert!(view_box[2] > 16.0, "{view_box:?}");
        assert!(view_box[3] > 41.0, "{view_box:?}");
    }

    #[test]
    fn requirement_html_labels_use_xhtml_and_deterministic_wrap_styles() {
        let measurer = crate::text::DeterministicTextMeasurer::default();
        let config = serde_json::json!({
            "fontFamily": "trebuchet ms, verdana, arial, sans-serif",
            "fontSize": 10,
            "themeVariables": {"fontSize": "24px"}
        });
        let model = RequirementDiagramRenderModel {
            requirements: vec![RequirementRenderNode {
                name: "req_font_size".to_string(),
                node_type: "Requirement".to_string(),
                requirement_id: "req_font_size".to_string(),
                text: "font size precedence should be deterministic".to_string(),
                risk: "`Low`".to_string(),
                verify_method: "Test".to_string(),
                css_styles: Vec::new(),
                classes: Vec::new(),
            }],
            ..empty_requirement_model()
        };
        let options = SvgRenderOptions {
            diagram_id: Some("requirementLabels".to_string()),
            ..SvgRenderOptions::default()
        };

        let svg = render_requirement_for_test(&model, &config, None, &measurer, &options).unwrap();
        let settings = crate::requirement::RequirementConfigView::new(&config).layout_settings();
        let calculation_style = crate::text::TextStyle {
            font_family: Some(settings.calculation_font_family),
            font_size: settings.calculation_font_size,
            ..crate::text::TextStyle::default()
        };
        let expected_max_width = crate::requirement::calculate_text_width_like_mermaid_px(
            &measurer,
            &calculation_style,
            "Text: font size precedence should be deterministic",
        ) + 50;

        assert!(svg.contains(r#"<div xmlns="http://www.w3.org/1999/xhtml""#));
        assert!(svg.contains("display: table; white-space: break-spaces;"));
        assert!(svg.contains(&format!("max-width: {expected_max_width}px;")));
        assert!(svg.contains(&format!("width: {expected_max_width}px;")));
        assert!(svg.contains(r#"class="nodeLabel markdown-node-label""#));
        assert!(svg.contains("<p>Risk: `Low`</p>"), "{svg}");
    }
}
