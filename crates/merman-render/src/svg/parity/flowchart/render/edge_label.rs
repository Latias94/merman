//! Flowchart edge label renderer.

use super::super::*;
use super::root::flowchart_wrap_svg_text_lines;
use crate::svg::parity::flowchart::util::HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR;

fn padded_html_edge_label_background(padding: f64, width: f64, height: f64) -> String {
    if padding <= 0.0 || width <= 0.0 || height <= 0.0 {
        return String::new();
    }

    let width = width + 2.0 * padding;
    let height = height + 2.0 * padding;
    format!(
        r#"<rect class="background" x="{}" y="{}" width="{}" height="{}" rx="{}" ry="{}"/>"#,
        fmt_display(-width / 2.0),
        fmt_display(-height / 2.0),
        fmt_display(width),
        fmt_display(height),
        fmt_display(padding),
        fmt_display(padding),
    )
}

fn position_flowchart_edge_label(
    dagre_anchor: crate::model::LayoutPoint,
    geom: &FlowchartEdgePathGeom,
    always_recompute: bool,
) -> crate::model::LayoutPoint {
    let rendered_d = geom
        .emitted_d_for_label
        .as_deref()
        .unwrap_or(geom.d.as_str());
    crate::svg::parity::edge_label_geometry::position_edge_label(
        dagre_anchor,
        &geom.label_path_points,
        rendered_d,
        geom.label_path_was_explicitly_updated || always_recompute,
    )
}

fn resolve_flowchart_edge_label_position(
    ctx: &FlowchartRenderCtx<'_>,
    layout_edge: &crate::model::LayoutEdge,
    label: &crate::model::LayoutLabel,
    origin_x: f64,
    origin_y: f64,
    edge_cache: &FxHashMap<&str, FlowchartEdgePathCacheEntry>,
    always_recompute: bool,
) -> crate::model::LayoutPoint {
    let dagre_anchor = crate::model::LayoutPoint {
        x: label.x + ctx.tx - origin_x,
        y: label.y + ctx.ty - origin_y,
    };

    if let Some(geom) = edge_cache
        .get(layout_edge.id.as_str())
        .filter(|entry| {
            (entry.origin_x - origin_x).abs() <= 1e-9 && (entry.origin_y - origin_y).abs() <= 1e-9
        })
        .map(|entry| &entry.geom)
    {
        return position_flowchart_edge_label(dagre_anchor, geom, always_recompute);
    }

    // Geometry caching only skips routes with fewer than two points. For that degenerate case,
    // `calcLabelPosition` either keeps the anchor (empty) or returns the sole waypoint.
    if always_recompute || layout_edge.to_cluster.is_some() || layout_edge.from_cluster.is_some() {
        let points = layout_edge
            .points
            .iter()
            .map(|point| crate::model::LayoutPoint {
                x: point.x + ctx.tx - origin_x,
                y: point.y + ctx.ty - origin_y,
            })
            .collect::<Vec<_>>();
        return crate::svg::parity::edge_label_geometry::position_edge_label(
            dagre_anchor.clone(),
            &points,
            "",
            true,
        );
    }

    dagre_anchor
}

pub(in crate::svg::parity) fn render_flowchart_edge_label(
    out: &mut String,
    ctx: &FlowchartRenderCtx<'_>,
    edge: &crate::flowchart::FlowEdge,
    origin_x: f64,
    origin_y: f64,
    edge_cache: &FxHashMap<&str, FlowchartEdgePathCacheEntry>,
) {
    let label_text = edge.label.as_deref().unwrap_or_default();
    let label_type = edge.label_type.as_deref().unwrap_or("text");
    let label_text_plain = flowchart_label_plain_text(label_text, label_type, ctx.edge_html_labels);
    let compiled_label_styles = flowchart_compile_styles(
        ctx.class_defs,
        &edge.classes,
        &ctx.default_edge_style,
        &edge.style,
    );
    let span_style_attr = OptionalStyleXmlAttr(compiled_label_styles.label_style.as_str());
    let div_style_prefix = crate::svg::parity::flowchart::style::flowchart_label_div_style_prefix(
        &compiled_label_styles,
        false,
    );

    fn fallback_midpoint(
        le: &crate::model::LayoutEdge,
        ctx: &FlowchartRenderCtx<'_>,
        origin_x: f64,
        origin_y: f64,
    ) -> (f64, f64) {
        let anchor = crate::model::LayoutPoint {
            x: ctx.tx - origin_x,
            y: ctx.ty - origin_y,
        };
        let points = le
            .points
            .iter()
            .map(|point| crate::model::LayoutPoint {
                x: point.x + ctx.tx - origin_x,
                y: point.y + ctx.ty - origin_y,
            })
            .collect::<Vec<_>>();
        let position =
            crate::svg::parity::edge_label_geometry::position_edge_label(anchor, &points, "", true);
        (position.x, position.y)
    }

    if !ctx.edge_html_labels {
        if let Some(le) = ctx.layout_edges_by_id.get(edge.id.as_str()) {
            if let Some(lbl) = le.label.as_ref() {
                let position = resolve_flowchart_edge_label_position(
                    ctx, le, lbl, origin_x, origin_y, edge_cache, false,
                );
                let x = position.x;
                let y = position.y;

                if label_text_plain.trim().is_empty() {
                    if !label_text.trim().is_empty() {
                        let _ = write!(
                            out,
                            r#"<g class="edgeLabel" transform="translate({},{})"><g class="label" data-id="{}" transform="translate(-2,-2)"><g><rect class="background" style="" x="-2" y="-2" width="4" height="4"/>"#,
                            fmt_display(x),
                            fmt_display(y),
                            escape_xml_display(&edge.id),
                        );
                        write_flowchart_svg_text_centered(out, "", true);
                        out.push_str("</g></g></g>");
                        return;
                    }
                } else {
                    let w = lbl.width.max(0.0);
                    let h = lbl.height.max(0.0);
                    let (dx, dy) = if w > 0.0 && h > 0.0 {
                        (-w / 2.0, -h / 2.0)
                    } else {
                        (0.0, 0.0)
                    };
                    let background_y =
                        crate::text::flowchart_svg_edge_label_background_y_px(&ctx.text_style);
                    let _ = write!(
                        out,
                        r#"<g class="edgeLabel" transform="translate({},{})"><g class="label" data-id="{}" transform="translate({},{})"><g><rect class="background" style="" x="-2" y="{}" width="{}" height="{}"/>"#,
                        fmt_display(x),
                        fmt_display(y),
                        escape_xml_display(&edge.id),
                        fmt_display(dx),
                        fmt_display(dy),
                        fmt_display(background_y),
                        fmt_display(w),
                        fmt_display(h)
                    );
                    let wrapped = flowchart_wrap_svg_text_lines(
                        ctx.measurer,
                        &label_text_plain,
                        &ctx.text_style,
                        Some(FLOWCHART_EDGE_LABEL_WRAP_WIDTH),
                        true,
                    )
                    .join("\n");
                    if label_type == "markdown" {
                        write_flowchart_svg_text_markdown_wrapped_centered(
                            out,
                            label_text,
                            true,
                            ctx.measurer,
                            &ctx.text_style,
                            Some(FLOWCHART_EDGE_LABEL_WRAP_WIDTH),
                        );
                    } else {
                        write_flowchart_svg_text_centered(out, &wrapped, true);
                    }
                    out.push_str("</g></g></g>");
                    return;
                }
            }

            if !label_text_plain.trim().is_empty() {
                let (x, y) = fallback_midpoint(le, ctx, origin_x, origin_y);
                let metrics = ctx.measurer.measure_wrapped(
                    &label_text_plain,
                    &ctx.text_style,
                    Some(FLOWCHART_EDGE_LABEL_WRAP_WIDTH),
                    crate::text::WrapMode::SvgLike,
                );
                let w = (metrics.width + 4.0).max(1.0);
                let h = (metrics.height + 4.0).max(1.0);
                let background_y =
                    crate::text::flowchart_svg_edge_label_background_y_px(&ctx.text_style);
                let _ = write!(
                    out,
                    r#"<g class="edgeLabel" transform="translate({},{})"><g class="label" data-id="{}" transform="translate({},{})"><g><rect class="background" style="" x="-2" y="{}" width="{}" height="{}"/>"#,
                    fmt_display(x),
                    fmt_display(y),
                    escape_xml_display(&edge.id),
                    fmt_display(-w / 2.0),
                    fmt_display(-h / 2.0),
                    fmt_display(background_y),
                    fmt_display(w),
                    fmt_display(h)
                );
                let wrapped = flowchart_wrap_svg_text_lines(
                    ctx.measurer,
                    &label_text_plain,
                    &ctx.text_style,
                    Some(FLOWCHART_EDGE_LABEL_WRAP_WIDTH),
                    true,
                )
                .join("\n");
                if label_type == "markdown" {
                    write_flowchart_svg_text_markdown_wrapped_centered(
                        out,
                        label_text,
                        true,
                        ctx.measurer,
                        &ctx.text_style,
                        Some(FLOWCHART_EDGE_LABEL_WRAP_WIDTH),
                    );
                } else {
                    write_flowchart_svg_text_centered(out, &wrapped, true);
                }
                out.push_str("</g></g></g>");
                return;
            }
        }

        let _ = write!(
            out,
            r#"<g class="edgeLabel"><g class="label" data-id="{}" transform="translate(0,0)">"#,
            escape_xml_display(&edge.id)
        );
        write_flowchart_svg_text_centered(out, "", false);
        out.push_str("</g></g>");
        return;
    }

    let label_html = if label_text.trim().is_empty() {
        String::new()
    } else {
        flowchart_label_html(label_text, label_type, ctx.config, ctx.math_renderer)
    };

    if let Some(le) = ctx.layout_edges_by_id.get(edge.id.as_str()) {
        if let Some(lbl) = le.label.as_ref() {
            let position = resolve_flowchart_edge_label_position(
                ctx, le, lbl, origin_x, origin_y, edge_cache, false,
            );
            let x = position.x;
            let y = position.y;

            let layout_w = lbl.width.max(0.0);
            let h = lbl.height.max(0.0);
            let background = padded_html_edge_label_background(ctx.edge_label_padding, layout_w, h);
            let wrapped_style = if layout_w >= FLOWCHART_EDGE_LABEL_WRAP_WIDTH - 0.01 {
                format!(
                    "display: table; white-space: break-spaces; line-height: 1.5; max-width: {mw}px; text-align: center; width: {mw}px;",
                    mw = fmt_display(FLOWCHART_EDGE_LABEL_WRAP_WIDTH)
                )
            } else {
                "display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;".to_string()
            };
            let div_style = if div_style_prefix.is_empty() {
                wrapped_style
            } else {
                format!("{div_style_prefix}{wrapped_style}")
            };
            let _ = write!(
                out,
                r#"<g class="edgeLabel" transform="translate({},{})">{}<g class="label" data-id="{}" transform="translate({},{})"><foreignObject width="{}" height="{}"{}><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="{}"><span class="edgeLabel"{}>{}</span></div></foreignObject></g></g>"#,
                fmt_display(x),
                fmt_display(y),
                background,
                escape_xml_display(&edge.id),
                fmt_display(-layout_w / 2.0),
                fmt_display(-h / 2.0),
                fmt_display(layout_w),
                fmt_display(h),
                HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR,
                escape_xml_display(&div_style),
                span_style_attr,
                label_html
            );
            return;
        }

        if !label_text_plain.trim().is_empty() {
            let (x, y) = fallback_midpoint(le, ctx, origin_x, origin_y);
            let has_inline_style_tags = if label_type == "markdown" {
                false
            } else {
                let lower = label_text.to_ascii_lowercase();
                crate::text::flowchart_html_has_inline_style_tags(&lower)
            };

            let metrics = if label_type == "markdown" {
                crate::text::measure_markdown_with_inline_styles(
                    ctx.measurer,
                    label_text,
                    &ctx.text_style,
                    Some(FLOWCHART_EDGE_LABEL_WRAP_WIDTH),
                    ctx.edge_wrap_mode,
                )
            } else if has_inline_style_tags {
                crate::text::measure_html_with_inline_styles(
                    ctx.measurer,
                    label_text,
                    &ctx.text_style,
                    Some(FLOWCHART_EDGE_LABEL_WRAP_WIDTH),
                    ctx.edge_wrap_mode,
                )
            } else {
                ctx.measurer.measure_wrapped(
                    &label_text_plain,
                    &ctx.text_style,
                    Some(FLOWCHART_EDGE_LABEL_WRAP_WIDTH),
                    ctx.edge_wrap_mode,
                )
            };
            let layout_w = metrics.width.max(1.0);
            let h = metrics.height.max(1.0);
            let background = padded_html_edge_label_background(ctx.edge_label_padding, layout_w, h);
            let wrapped_style = if layout_w >= FLOWCHART_EDGE_LABEL_WRAP_WIDTH - 0.01 {
                format!(
                    "display: table; white-space: break-spaces; line-height: 1.5; max-width: {mw}px; text-align: center; width: {mw}px;",
                    mw = fmt_display(FLOWCHART_EDGE_LABEL_WRAP_WIDTH)
                )
            } else {
                "display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;".to_string()
            };
            let div_style = if div_style_prefix.is_empty() {
                wrapped_style
            } else {
                format!("{div_style_prefix}{wrapped_style}")
            };
            let _ = write!(
                out,
                r#"<g class="edgeLabel" transform="translate({},{})">{}<g class="label" data-id="{}" transform="translate({},{})"><foreignObject width="{}" height="{}"{}><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="{}"><span class="edgeLabel"{}>{}</span></div></foreignObject></g></g>"#,
                fmt_display(x),
                fmt_display(y),
                background,
                escape_xml_display(&edge.id),
                fmt_display(-layout_w / 2.0),
                fmt_display(-h / 2.0),
                fmt_display(layout_w),
                fmt_display(h.max(0.0)),
                HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR,
                escape_xml_display(&div_style),
                span_style_attr,
                label_html
            );
            return;
        }
    }

    let _ = write!(
        out,
        r#"<g class="edgeLabel"><g class="label" data-id="{}" transform="translate(0,0)"><foreignObject width="0" height="0"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="{}display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="edgeLabel"{}></span></div></foreignObject></g></g>"#,
        escape_xml_display(&edge.id),
        escape_xml_display(&div_style_prefix),
        span_style_attr
    );
}

pub(in crate::svg::parity::flowchart) fn render_swimlane_edge_label_node(
    out: &mut String,
    ctx: &FlowchartRenderCtx<'_>,
    node_id: &str,
    edge: &crate::flowchart::FlowEdge,
    origin_x: f64,
    origin_y: f64,
    edge_cache: &FxHashMap<&str, FlowchartEdgePathCacheEntry>,
) {
    let Some(layout_edge) = ctx.layout_edges_by_id.get(edge.id.as_str()) else {
        return;
    };
    let Some(label) = layout_edge.label.as_ref() else {
        return;
    };

    let label_text = edge.label.as_deref().unwrap_or_default();
    let label_type = edge.label_type.as_deref().unwrap_or("text");
    let label_text_plain = flowchart_label_plain_text(label_text, label_type, ctx.node_html_labels);
    let compiled = flowchart_compile_styles(
        ctx.class_defs,
        &edge.classes,
        &ctx.default_edge_style,
        &edge.style,
    );
    // Swimlane's private `positionEdgeLabel` recomputes whenever `insertEdge` returns a paths
    // object, independent of the ordinary updated-path heuristic.
    let position = resolve_flowchart_edge_label_position(
        ctx,
        layout_edge,
        label,
        origin_x,
        origin_y,
        edge_cache,
        true,
    );
    let x = position.x;
    let y = position.y;
    let width = label.width.max(0.0);
    let height = label.height.max(0.0);

    let _ = write!(
        out,
        r#"<g class="label edgeLabel" id="{}" transform="translate({}, {})"><rect width="0.1" height="0.1"/><g class="label"{} transform="translate({}, {})"><rect/>"#,
        escape_xml_display(node_id),
        fmt_display(x),
        fmt_display(y),
        OptionalStyleXmlAttr(&compiled.label_style),
        fmt_display(-width / 2.0),
        fmt_display(-height / 2.0),
    );

    if ctx.node_html_labels {
        let label_html =
            flowchart_label_html(label_text, label_type, ctx.config, ctx.math_renderer);
        let mut div_style =
            crate::svg::parity::flowchart::style::flowchart_label_div_style_prefix(&compiled, true);
        let _ = write!(
            &mut div_style,
            "display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {}px; text-align: center;",
            fmt_display(ctx.wrapping_width),
        );
        let _ = write!(
            out,
            r#"<foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="{}"><span class="nodeLabel"{}>{}</span></div></foreignObject></g></g>"#,
            fmt_display(width),
            fmt_display(height),
            escape_xml_display(&div_style),
            OptionalStyleXmlAttr(&compiled.label_style),
            label_html,
        );
        return;
    }

    out.push_str(r#"<g><rect class="background" style="stroke: none"/>"#);
    if label_type == "markdown" {
        write_flowchart_svg_text_markdown_wrapped(
            out,
            label_text,
            true,
            ctx.measurer,
            &ctx.text_style,
            Some(ctx.wrapping_width),
        );
    } else {
        let wrapped = flowchart_wrap_svg_text_lines(
            ctx.measurer,
            &label_text_plain,
            &ctx.text_style,
            Some(ctx.wrapping_width),
            true,
        )
        .join("\n");
        write_flowchart_svg_text(out, &wrapped, true);
    }
    out.push_str("</g></g></g>");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: f64, y: f64) -> crate::model::LayoutPoint {
        crate::model::LayoutPoint { x, y }
    }

    fn geom(d: &str, points: Vec<crate::model::LayoutPoint>) -> FlowchartEdgePathGeom {
        FlowchartEdgePathGeom {
            d: d.to_string(),
            pb: None,
            data_points: points.clone(),
            data_points_b64: String::new(),
            original_path_length: None,
            path_length: None,
            line_hop_applied: false,
            label_path_points: points,
            label_path_was_explicitly_updated: false,
            emitted_d_for_label: None,
            bounds_skipped_for_viewbox: false,
        }
    }

    #[test]
    fn ordinary_dagre_keeps_anchor_until_insert_edge_marks_path_updated() {
        let anchor = point(4.0, 5.0);
        let mut geometry = geom(
            "M0,0L10,0L20,0",
            vec![point(0.0, 0.0), point(10.0, 0.0), point(20.0, 0.0)],
        );

        let unchanged = position_flowchart_edge_label(anchor.clone(), &geometry, false);
        assert_eq!((unchanged.x, unchanged.y), (4.0, 5.0));

        geometry.label_path_was_explicitly_updated = true;
        let updated = position_flowchart_edge_label(anchor, &geometry, false);
        assert_eq!((updated.x, updated.y), (10.0, 0.0));
    }

    #[test]
    fn swimlane_always_recomputes_from_returned_waypoints() {
        let geometry = geom(
            "M0,0L10,0L20,0",
            vec![point(0.0, 0.0), point(10.0, 0.0), point(20.0, 0.0)],
        );

        let positioned = position_flowchart_edge_label(point(4.0, 5.0), &geometry, true);
        assert_eq!((positioned.x, positioned.y), (10.0, 0.0));
    }

    #[test]
    fn rough_actual_emitted_path_drives_updated_path_detection() {
        let anchor = point(4.0, 5.0);
        let mut geometry = geom(
            "M0,0L10,20L30,20",
            vec![point(0.0, 0.0), point(10.0, 20.0), point(30.0, 20.0)],
        );

        let logical_curve = position_flowchart_edge_label(anchor.clone(), &geometry, false);
        assert_eq!((logical_curve.x, logical_curve.y), (4.0, 5.0));

        geometry.emitted_d_for_label = Some("M1.1,2.2C3.3,4.4,5.5,6.6,7.7,8.8".to_string());
        let rough_curve = position_flowchart_edge_label(anchor, &geometry, false);
        assert_ne!((rough_curve.x, rough_curve.y), (4.0, 5.0));
    }

    #[test]
    fn shared_geometry_handles_empty_single_degenerate_and_polyline_paths() {
        let anchor = point(4.0, 5.0);
        let empty = geom("", Vec::new());
        let positioned = position_flowchart_edge_label(anchor.clone(), &empty, true);
        assert_eq!((positioned.x, positioned.y), (4.0, 5.0));

        let single = geom("M2,3", vec![point(2.0, 3.0)]);
        let positioned = position_flowchart_edge_label(anchor.clone(), &single, true);
        assert_eq!((positioned.x, positioned.y), (2.0, 3.0));

        let degenerate = geom("M2,3L2,3", vec![point(2.0, 3.0), point(2.0, 3.0)]);
        let positioned = position_flowchart_edge_label(anchor.clone(), &degenerate, true);
        assert_eq!((positioned.x, positioned.y), (2.0, 3.0));

        let polyline = geom(
            "M0,0L6,0L6,8",
            vec![point(0.0, 0.0), point(6.0, 0.0), point(6.0, 8.0)],
        );
        let positioned = position_flowchart_edge_label(anchor, &polyline, true);
        assert_eq!((positioned.x, positioned.y), (6.0, 1.0));
    }
}
