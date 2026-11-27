//! Flowchart edge label renderer.

use merman_render_flow::edge_labels::svg::{
    FLOWCHART_EDGE_LABEL_WRAP_WIDTH, SvgEdgeLabel, SvgEdgeLabelText, write_empty_svg_edge_label,
};

use super::super::*;
use crate::svg::parity::flowchart::util::HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR;
use std::borrow::Cow;

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
        geom.label_anchor.clone().unwrap_or(dagre_anchor),
        Some(geom.original_label_path_points.as_slice()),
        &geom.label_path_points,
        rendered_d,
        geom.label_path_was_explicitly_updated || always_recompute,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "The position resolver consumes cached geometry with its edge identity, local origin, and explicit recomputation policy."
)]
pub(in crate::svg::parity::flowchart) fn resolve_flowchart_edge_label_position(
    ctx: &FlowchartRenderCtx<'_>,
    key: crate::flowchart::FlowchartEdgeKey,
    layout_edge: FlowchartLayoutEdgeRef<'_>,
    label: &crate::model::LayoutLabel,
    origin_x: f64,
    origin_y: f64,
    edge_cache: &FxHashMap<crate::flowchart::FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
    always_recompute: bool,
) -> crate::model::LayoutPoint {
    let layout_anchor = crate::model::LayoutPoint {
        x: label.x + ctx.tx - origin_x,
        y: label.y + ctx.ty - origin_y,
    };
    if let Some(geom) = edge_cache
        .get(&key)
        .filter(|entry| {
            (entry.origin_x - origin_x).abs() <= 1e-9 && (entry.origin_y - origin_y).abs() <= 1e-9
        })
        .map(|entry| &entry.geom)
    {
        return position_flowchart_edge_label(layout_anchor, geom, always_recompute);
    }

    let points = layout_edge.points();
    if let Some(midpoint) =
        edge_geom::missing_section_label_position(ctx, key, points, origin_x, origin_y)
    {
        return midpoint;
    }

    // Geometry caching only skips routes with fewer than two points. For that degenerate case,
    // `calcLabelPosition` either keeps the anchor (empty) or returns the sole waypoint.
    if always_recompute
        || layout_edge
            .materialized()
            .is_some_and(|edge| edge.to_cluster.is_some() || edge.from_cluster.is_some())
    {
        let points = points
            .iter()
            .map(|point| crate::model::LayoutPoint {
                x: point.x + ctx.tx - origin_x,
                y: point.y + ctx.ty - origin_y,
            })
            .collect::<Vec<_>>();
        return crate::svg::parity::edge_label_geometry::position_edge_label(
            layout_anchor,
            (!always_recompute).then_some(points.as_slice()),
            &points,
            "",
            true,
        );
    }

    layout_anchor
}

fn render_native_label_background(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    hierarchy: &FlowchartHierarchyPlan<'_>,
    position: crate::model::LayoutPoint,
    (origin_x, origin_y): (f64, f64),
    (width, height): (f64, f64),
) -> crate::Result<Option<crate::theme::Color>> {
    use merman_theme_contract::v2::EdgeLabelBackground;

    let Some(native) = ctx.native else {
        return Ok(None);
    };
    let mode = native.form.edge_label_bg;
    let fill = match mode {
        EdgeLabelBackground::Pill => native.node,
        EdgeLabelBackground::Canvas => native.canvas,
        EdgeLabelBackground::Auto | EdgeLabelBackground::Transparent => {
            let layers = match ctx.container_label_layers.get() {
                Some(layers) => layers,
                None => {
                    let layers = ContainerLabelLayers::prepare(ctx, native, hierarchy)?;
                    ctx.container_label_layers.get_or_init(|| layers)
                }
            };
            let point = kurbo::Point::new(
                position.x + origin_x - ctx.tx,
                position.y + origin_y - ctx.ty,
            );
            layers.fill_at(ctx, point, native.canvas)?
        }
    };
    let bounds = kurbo::Rect::from_center_size((position.x, position.y), (width, height));
    let _ = crate::theme::edge_label_bg::write_backplate(out, bounds, mode, fill, native.border);
    out.checkpoint()?;
    Ok(Some(fill))
}

pub(in crate::svg::parity::flowchart) struct ContainerLabelLayers(Vec<ContainerLabelLayer>);

struct ContainerLabelLayer {
    bounds: kurbo::Rect,
    title: Option<kurbo::Rect>,
    paint: std::result::Result<ContainerLabelPaint, String>,
}

struct ContainerLabelPaint {
    body: crate::theme::Color,
    title: crate::theme::Color,
    opacity: f64,
}

impl ContainerLabelLayers {
    fn prepare(
        ctx: &FlowchartRenderCtx<'_>,
        native: &crate::flowchart::native_theme::FlowchartNativeStyle,
        hierarchy: &FlowchartHierarchyPlan<'_>,
    ) -> crate::Result<Self> {
        let meter = ctx.work_meter;
        let clusters = hierarchy.painted_clusters();
        meter.charge_emit_work(clusters.len())?;
        let mut layers = Vec::with_capacity(clusters.len());
        let canonical = ctx.layout.route_contract.group_parents().is_some();
        for (index, cluster) in clusters.iter().enumerate() {
            if index != 0 && index.is_multiple_of(256) {
                meter.charge_emit_work(0)?;
            }
            let id = cluster.id.as_str();
            if ctx.is_subgraph_collapsed(id) {
                continue;
            }
            let (bounds, title) = if let Some(lane) = ctx.swimlane_lanes_by_id.get(id)
                && super::super::swimlane::is_lane_or_pool_cluster(ctx, id)
                && let Some(direction) = ctx.swimlane_direction
            {
                let geometry =
                    super::super::swimlane::swimlane_terminal_geometry(lane, direction, canonical);
                let (body, title) = geometry.frame_parts((lane.x, lane.y), direction, canonical);
                (body.union(title), Some(title))
            } else {
                if !ctx.subgraph_has_children(id)
                    && !super::flowchart_elk_renders_empty_subgraph_as_cluster(ctx)
                {
                    continue;
                }
                (
                    super::super::cluster_geometry::painted_cluster_frame(ctx, cluster).bounds(),
                    None,
                )
            };
            let paint = match ctx.prepared_nodes.cluster(id) {
                Ok(terminal) => {
                    let authored = terminal
                        .compiled
                        .fill
                        .as_deref()
                        .and_then(crate::theme::edge_label_bg::literal_fill);
                    Ok(ContainerLabelPaint {
                        body: authored.unwrap_or(native.container),
                        title: authored.unwrap_or(native.emphasis),
                        opacity: terminal.compiled.fill_opacity,
                    })
                }
                Err(_) => Err(id.to_owned()),
            };
            layers.push(ContainerLabelLayer {
                bounds,
                title,
                paint,
            });
        }
        meter.charge_emit_work(0)?;
        Ok(Self(layers))
    }

    fn fill_at(
        &self,
        ctx: &FlowchartRenderCtx<'_>,
        point: kurbo::Point,
        mut backdrop: crate::theme::Color,
    ) -> crate::Result<crate::theme::Color> {
        ctx.work_meter.charge_emit_work(self.0.len())?;
        for layer in &self.0 {
            if !layer.bounds.contains(point) {
                continue;
            }
            // A frame without a prepared terminal only matters when it is sampled.
            let paint = layer
                .paint
                .as_ref()
                .map_err(|id| crate::Error::InvalidModel {
                    message: format!("missing prepared Flowchart cluster `{id}`"),
                })?;
            let fill = if layer.title.is_some_and(|title| title.contains(point)) {
                paint.title
            } else {
                paint.body
            };
            backdrop = fill.composite(backdrop, paint.opacity);
        }
        Ok(backdrop)
    }
}

fn html_edge_label_div_style(prefix: &str, width: f64, inset: f64) -> String {
    let wraps = width - 2.0 * inset >= FLOWCHART_EDGE_LABEL_WRAP_WIDTH - 0.01;
    if inset > 0.0 {
        let white_space = if wraps { "break-spaces" } else { "nowrap" };
        // Keep the content wrap limit for the SVG text fallback. Center the span even when
        // its browser font is slightly wider than the layout measurement.
        format!(
            "{prefix}display: flex; justify-content: center; white-space: {white_space}; line-height: 1.5; max-width: {mw}px; text-align: center; width: calc(100% - {insets}px); box-sizing: content-box; padding-inline: {inset}px;",
            mw = fmt_display(FLOWCHART_EDGE_LABEL_WRAP_WIDTH),
            insets = fmt_display(2.0 * inset),
            inset = fmt_display(inset),
        )
    } else if wraps {
        format!(
            "{prefix}display: table; white-space: break-spaces; line-height: 1.5; max-width: {mw}px; text-align: center; width: {mw}px;",
            mw = fmt_display(FLOWCHART_EDGE_LABEL_WRAP_WIDTH)
        )
    } else {
        format!(
            "{prefix}display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"
        )
    }
}

fn has_explicit_css_color(style: &str) -> bool {
    let mut found = false;
    crate::mermaid_style::visit_parsed_style_declarations(style, |decl| {
        if decl.property() == "color" {
            found = true;
        }
    });
    found
}

fn flowchart_svg_edge_label_text_style<'a>(
    base_label_style: &'a str,
    adaptive_ink: Option<&'static str>,
    source_label_empty: bool,
) -> Cow<'a, str> {
    if source_label_empty {
        return Cow::Borrowed(base_label_style);
    }
    let has_color = has_explicit_css_color(base_label_style);
    if !has_color && adaptive_ink.is_none() {
        return Cow::Borrowed(base_label_style);
    }
    let mut out = String::with_capacity(base_label_style.len() + 16);
    crate::mermaid_style::visit_parsed_style_declarations(base_label_style, |decl| {
        if !out.is_empty() {
            out.push(';');
        }
        if decl.property() == "color" {
            out.push_str("fill:");
            out.push_str(decl.source_value());
        } else {
            out.push_str(decl.property_css());
            out.push(':');
            out.push_str(decl.source_value());
        }
    });
    if let Some(ink) = adaptive_ink {
        if !out.is_empty() {
            out.push(';');
        }
        out.push_str("fill:");
        out.push_str(ink);
    }
    Cow::Owned(out)
}

fn swimlane_html_edge_label_style(
    first_label_style: &str,
    adaptive_ink: Option<&'static str>,
) -> String {
    let mut out = String::with_capacity(first_label_style.len() + 24);
    let mut replaced_fill = false;
    crate::mermaid_style::visit_parsed_style_declarations(first_label_style, |decl| {
        if !out.is_empty() {
            out.push(';');
        }
        if !replaced_fill && decl.property() == "fill" {
            replaced_fill = true;
            out.push_str("color:");
            out.push_str(decl.source_value());
        } else {
            out.push_str(decl.property_css());
            out.push(':');
            out.push_str(decl.source_value());
        }
    });
    if let Some(ink) = adaptive_ink {
        if !out.is_empty() {
            out.push(';');
        }
        let _ = write!(&mut out, "color:{ink} !important");
    }
    out
}

fn swimlane_svg_edge_label_text_style(
    first_label_style: &str,
    adaptive_ink: Option<&'static str>,
) -> String {
    let mut out = String::with_capacity(first_label_style.len() + 16);
    crate::mermaid_style::visit_parsed_style_declarations(first_label_style, |decl| {
        let property = decl.property();
        if property == "color" || property == "fill" {
            if !out.is_empty() {
                out.push(';');
            }
            out.push_str("fill:");
            out.push_str(decl.source_value());
        } else if crate::flowchart::flowchart_is_source_spelled_label_style_key(property) {
            if !out.is_empty() {
                out.push(';');
            }
            out.push_str(decl.property_css());
            out.push(':');
            out.push_str(decl.source_value());
        }
    });
    if let Some(ink) = adaptive_ink {
        if !out.is_empty() {
            out.push(';');
        }
        out.push_str("fill:");
        out.push_str(ink);
    }
    out
}

pub(in crate::svg::parity::flowchart) fn render_flowchart_edge_label(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    edge_ref: super::super::render_input::FlowchartRenderEdgeRef<'_>,
    origin_x: f64,
    origin_y: f64,
    hierarchy: &FlowchartHierarchyPlan<'_>,
    placements: &super::super::paint::FlowchartEdgeLabels,
) -> crate::Result<()> {
    let key = edge_ref.key;
    let edge = edge_ref.edge;
    let label_text = ctx
        .model
        .edge_label_for_occurrence(key, edge)
        .unwrap_or_default();
    let source_label_empty = crate::flowchart::flowchart_label_is_empty_for_render(label_text);
    let mut sampled_backdrop = None;
    if ctx.native.is_some()
        && !source_label_empty
        && let Some(label) = ctx
            .layout_edges_by_key
            .get(&key)
            .and_then(|edge| edge.label())
    {
        let position = placements
            .local_position(key, ctx, origin_x, origin_y)
            .unwrap_or(crate::model::LayoutPoint {
                x: label.x + ctx.tx - origin_x,
                y: label.y + ctx.ty - origin_y,
            });
        sampled_backdrop = render_native_label_background(
            out,
            ctx,
            hierarchy,
            position,
            (origin_x, origin_y),
            (label.width, label.height),
        )?;
    }
    let compiled_label_styles = ctx.edge_style_plan.edge_for(key)?;
    if source_label_empty && !ctx.edge_html_labels {
        write_empty_svg_edge_label(out, &edge.id);
        return Ok(());
    }

    let label_type = edge.label_type.as_deref().unwrap_or("text");
    let edge_wrap_style = ctx.text_style;
    let edge_metrics_style = if source_label_empty {
        ctx.text_style
    } else {
        ctx.edge_style_plan.terminal_label_text_style(key)?
    };
    let base_label_style = compiled_label_styles.label_style.as_str();
    let adaptive_label_ink = sampled_backdrop
        .filter(|_| {
            !compiled_label_styles.has_source_label_foreground()
                && !has_explicit_css_color(base_label_style)
        })
        .and_then(|backdrop| {
            let default_ink =
                crate::theme::edge_label_bg::literal_fill(&ctx.compatibility.text_color)?;
            crate::theme::edge_label_bg::adaptive_text_ink(backdrop, default_ink)
        });
    let effective_label_style: Cow<'_, str> = match adaptive_label_ink {
        Some(ink) if base_label_style.is_empty() => Cow::Owned(format!("color:{ink} !important;")),
        Some(ink) => Cow::Owned(format!("{base_label_style};color:{ink} !important;")),
        None => Cow::Borrowed(base_label_style),
    };
    let svg_edge_label_text_style = flowchart_svg_edge_label_text_style(
        base_label_style,
        adaptive_label_ink,
        source_label_empty,
    );
    let owner = Some(crate::flowchart::FlowchartSvgLabelOwner::Edge(
        key.semantic_index(),
    ));
    let prepared_svg_label = (!ctx.edge_html_labels && label_type != "markdown").then(|| {
        crate::flowchart::FlowchartSvgLabelRenderPlan::new_with_metrics_style(
            ctx.svg_label_sidecar,
            owner,
            label_text,
            ctx.measurer,
            edge_wrap_style,
            edge_metrics_style,
            Some(FLOWCHART_EDGE_LABEL_WRAP_WIDTH),
            true,
            crate::flowchart::FlowchartSvgWidthMode::Bbox,
        )
    });
    let label_text_plain: Cow<'_, str> = prepared_svg_label.as_ref().map_or_else(
        || {
            if source_label_empty {
                Cow::Borrowed("")
            } else {
                Cow::Owned(flowchart_label_plain_text(
                    label_text,
                    label_type,
                    ctx.edge_html_labels,
                ))
            }
        },
        |prepared| Cow::Borrowed(prepared.plain_text()),
    );
    let span_style_attr = OptionalStyleAttr(effective_label_style.as_ref());
    let div_style_prefix = crate::svg::parity::flowchart::style::flowchart_label_div_style_prefix(
        compiled_label_styles,
        false,
    );
    if !ctx.edge_html_labels {
        if let Some(le) = ctx.layout_edges_by_key.get(&key) {
            let layout_label = le.label();
            let text_is_empty =
                crate::flowchart::flowchart_label_text_is_empty_for_mode(&label_text_plain, false);
            if layout_label.is_some() || !text_is_empty {
                let position = placements
                    .local_position(key, ctx, origin_x, origin_y)
                    .unwrap_or_else(|| {
                        let lbl = layout_label.expect("visible fallback label is prepared");
                        crate::model::LayoutPoint {
                            x: lbl.x + ctx.tx - origin_x,
                            y: lbl.y + ctx.ty - origin_y,
                        }
                    });
                let layout_size = layout_label.map(|lbl| {
                    if text_is_empty {
                        kurbo::Size::new(4.0, 4.0)
                    } else {
                        kurbo::Size::new(lbl.width, lbl.height)
                    }
                });
                use crate::flowchart::FlowchartSvgLabelRenderPlan;
                let text = match prepared_svg_label.as_ref() {
                    Some(FlowchartSvgLabelRenderPlan::Prepared { measured, .. }) => {
                        SvgEdgeLabelText::Wrapped(measured.wrapped_lines())
                    }
                    Some(FlowchartSvgLabelRenderPlan::Source { source, style, .. }) => {
                        SvgEdgeLabelText::Source {
                            source: source.as_ref(),
                            wrap_style: style,
                        }
                    }
                    None => SvgEdgeLabelText::Markdown(label_text),
                };
                SvgEdgeLabel {
                    edge_id: &edge.id,
                    position,
                    layout_size,
                    text,
                    plain_text: &label_text_plain,
                    css: svg_edge_label_text_style.as_ref(),
                    measurer: ctx.measurer,
                    metrics_style: edge_metrics_style,
                }
                .write(out);
                return Ok(());
            }
        }

        write_empty_svg_edge_label(out, &edge.id);
        return Ok(());
    }

    let label_html = if source_label_empty {
        Cow::Borrowed("")
    } else {
        let prepared_label = ctx
            .svg_label_sidecar
            .zip(owner)
            .map_or(Default::default(), |(sidecar, owner)| {
                sidecar.prepared_label_for_terminal(owner, label_text)
            });
        flowchart_label_html_with_prepared_label(
            label_text,
            label_type,
            ctx.config,
            ctx.math_renderer,
            prepared_label,
        )
    };

    #[cfg(feature = "layout-shoal")]
    let html_label_inset = if !source_label_empty
        && ctx.native.is_none()
        && ctx.layout.route_contract.is_canonical_axis()
    {
        crate::flowchart::shoal::HTML_EDGE_LABEL_INSET_PX
    } else {
        0.0
    };
    #[cfg(not(feature = "layout-shoal"))]
    let html_label_inset = 0.0;

    if let Some(le) = ctx.layout_edges_by_key.get(&key) {
        if let Some(lbl) = le.label() {
            let position = placements
                .local_position(key, ctx, origin_x, origin_y)
                .unwrap_or(crate::model::LayoutPoint {
                    x: lbl.x + ctx.tx - origin_x,
                    y: lbl.y + ctx.ty - origin_y,
                });
            let x = position.x;
            let y = position.y;

            let width = lbl.width.max(0.0);
            let height = lbl.height.max(0.0);
            let inset = if width > 0.0 && height > 0.0 {
                html_label_inset
            } else {
                0.0
            };
            let div_style = html_edge_label_div_style(&div_style_prefix, width, inset);
            let _ = write!(
                out,
                r#"<g class="edgeLabel" transform="translate({},{})"><g class="label" data-id="{}" transform="translate({},{})">"#,
                fmt_display(x),
                fmt_display(y),
                escape_xml_display(&edge.id),
                fmt_display(-width / 2.0),
                fmt_display(-height / 2.0),
            );
            let _ = write!(
                out,
                r#"<foreignObject width="{}" height="{}"{}><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="{}"><span class="edgeLabel"{}>{}</span></div></foreignObject>"#,
                fmt_display(width),
                fmt_display(height),
                HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR,
                escape_attr_display(&div_style),
                span_style_attr,
                label_html,
            );
            out.push_str("</g></g>");
            return Ok(());
        }

        if !crate::flowchart::flowchart_label_text_is_empty_for_mode(
            &label_text_plain,
            ctx.edge_html_labels,
        ) {
            let position = placements
                .local_position(key, ctx, origin_x, origin_y)
                .expect("visible fallback label is prepared");
            let (x, y) = (position.x, position.y);
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
                    edge_metrics_style,
                    Some(FLOWCHART_EDGE_LABEL_WRAP_WIDTH),
                    ctx.edge_wrap_mode,
                )
            } else if has_inline_style_tags {
                crate::text::measure_html_with_inline_styles(
                    ctx.measurer,
                    label_text,
                    edge_metrics_style,
                    Some(FLOWCHART_EDGE_LABEL_WRAP_WIDTH),
                    ctx.edge_wrap_mode,
                )
            } else {
                ctx.measurer.measure_wrapped(
                    &label_text_plain,
                    edge_metrics_style,
                    Some(FLOWCHART_EDGE_LABEL_WRAP_WIDTH),
                    ctx.edge_wrap_mode,
                )
            };
            let width = metrics.width.max(1.0) + 2.0 * html_label_inset;
            let height = metrics.height.max(1.0);
            let div_style = html_edge_label_div_style(&div_style_prefix, width, html_label_inset);
            let _ = write!(
                out,
                r#"<g class="edgeLabel" transform="translate({},{})"><g class="label" data-id="{}" transform="translate({},{})"><foreignObject width="{}" height="{}"{}><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="{}"><span class="edgeLabel"{}>{}</span></div></foreignObject></g></g>"#,
                fmt_display(x),
                fmt_display(y),
                escape_xml_display(&edge.id),
                fmt_display(-width / 2.0),
                fmt_display(-height / 2.0),
                fmt_display(width),
                fmt_display(height),
                HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR,
                escape_attr_display(&div_style),
                span_style_attr,
                label_html
            );
            return Ok(());
        }
    }

    let _ = write!(
        out,
        r#"<g class="edgeLabel"><g class="label" data-id="{}" transform="translate(0,0)"><foreignObject width="0" height="0"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="{}display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="edgeLabel"{}></span></div></foreignObject></g></g>"#,
        escape_xml_display(&edge.id),
        escape_attr_display(&div_style_prefix),
        span_style_attr
    );
    Ok(())
}

pub(in crate::svg::parity::flowchart) fn render_swimlane_edge_label_node(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    node_id: &str,
    edge_ref: super::super::render_input::FlowchartRenderEdgeRef<'_>,
    (origin_x, origin_y): (f64, f64),
    hierarchy: &FlowchartHierarchyPlan<'_>,
    placements: &super::super::paint::FlowchartEdgeLabels,
) -> crate::Result<()> {
    let key = edge_ref.key;
    let edge = edge_ref.edge;
    let label_text = ctx
        .model
        .edge_label_for_occurrence(key, edge)
        .unwrap_or_default();
    // Mermaid's Swimlane adapter creates a fresh `labelRect` without copying `edge.labelType`.
    // The node renderer therefore follows ordinary non-Markdown createText semantics.
    let label_type = "text";
    let first_label_style = ctx
        .default_edge_style
        .first()
        .or_else(|| edge.style.first());
    let compiled_label_styles = ctx.edge_style_plan.swimlane_label_for(key)?;
    let first_label_style = first_label_style.map_or("", String::as_str);
    let label_text_style = if crate::flowchart::flowchart_label_is_empty_for_render(label_text) {
        ctx.text_style
    } else {
        ctx.edge_style_plan.terminal_label_text_style(key)?
    };

    let Some(layout_edge) = ctx.layout_edges_by_key.get(&key) else {
        return Ok(());
    };
    let Some(label) = layout_edge.label() else {
        return Ok(());
    };

    // Mermaid 12.2's common renderer uses resolveEdgeLabelPosition: preserve the
    // layout anchor unless insertEdge marks the returned path as updated.
    let position = placements
        .local_position(key, ctx, origin_x, origin_y)
        .ok_or_else(|| crate::Error::InvalidModel {
            message: "missing prepared Swimlane label position".to_owned(),
        })?;
    let x = position.x;
    let y = position.y;
    let width = label.width.max(0.0);
    let height = label.height.max(0.0);
    let sampled_backdrop = if ctx.native.is_some() {
        render_native_label_background(
            out,
            ctx,
            hierarchy,
            position,
            (origin_x, origin_y),
            (width, height),
        )?
    } else {
        None
    };
    let mut has_swimlane_fg =
        compiled_label_styles.is_some_and(FlowchartCompiledStyles::has_source_label_foreground);
    crate::mermaid_style::visit_parsed_style_declarations(first_label_style, |decl| {
        if matches!(decl.property(), "color" | "fill") {
            has_swimlane_fg = true;
        }
    });
    let swimlane_adaptive_ink =
        sampled_backdrop
            .filter(|_| !has_swimlane_fg)
            .and_then(|backdrop| {
                let default_ink =
                    crate::theme::edge_label_bg::literal_fill(&ctx.compatibility.text_color)?;
                crate::theme::edge_label_bg::adaptive_text_ink(backdrop, default_ink)
            });
    let trimmed_first_style = first_label_style.trim().trim_end_matches(';');
    let swimlane_group_style: Cow<'_, str> = match swimlane_adaptive_ink {
        Some(ink) if trimmed_first_style.is_empty() => Cow::Owned(format!("fill:{ink}")),
        Some(ink) => Cow::Owned(format!("{trimmed_first_style};fill:{ink}")),
        None => Cow::Borrowed(first_label_style),
    };
    let edge_label_dom_id =
        ctx.document_ids()
            .synthetic_label(node_id)
            .ok_or_else(|| crate::Error::InvalidModel {
                message: format!("missing prepared Swimlane edge-label DOM id for `{node_id}`"),
            })?;

    let _ = write!(
        out,
        r#"<g class="label edgeLabel" id="{}" data-id="{}" data-et="edge-label" transform="translate({}, {})"><rect width="0.1" height="0.1"/><g class="label"{} transform="translate({}, {})"><rect/>"#,
        edge_label_dom_id,
        escape_xml_display(&edge.id),
        fmt_display(x),
        fmt_display(y),
        OptionalStyleAttr(swimlane_group_style.as_ref()),
        fmt_display(-width / 2.0),
        fmt_display(-height / 2.0),
    );

    if ctx.node_html_labels {
        let owner = Some(crate::flowchart::FlowchartSvgLabelOwner::SwimlaneEdgeLabel(
            key.semantic_index(),
        ));
        let prepared_label = ctx
            .svg_label_sidecar
            .zip(owner)
            .map_or(Default::default(), |(sidecar, owner)| {
                sidecar.prepared_label_for_terminal(owner, label_text)
            });
        let label_html = flowchart_label_html_with_prepared_label(
            label_text,
            label_type,
            ctx.config,
            ctx.math_renderer,
            prepared_label,
        );
        let needs_wrap = ctx.node_wrap_mode == crate::text::WrapMode::HtmlLike
            && crate::graph_label::flowchart_html_label_metrics_with_geometry(
                crate::flowchart::FlowchartLabelMetricsRequest {
                    measurer: ctx.measurer,
                    raw_label: label_text,
                    label_type,
                    style: label_text_style,
                    max_width_px: None,
                    wrap_mode: ctx.node_wrap_mode,
                    config: ctx.config,
                    math_renderer: None,
                },
                &label_html,
                compiled_label_styles.map_or(
                    Default::default(),
                    FlowchartCompiledStyles::html_label_geometry,
                ),
            )
            .width
                > ctx.wrapping_width;
        // `createText` maps only the first `fill:` declaration to `color:` before applying
        // the copied label style to the HTML span and div. Later fixed declarations override the
        // corresponding inline properties exactly as Mermaid's chained D3 `.style()` calls do.
        let html_label_style =
            swimlane_html_edge_label_style(first_label_style, swimlane_adaptive_ink);
        let mut div_style = String::new();
        let trimmed_html_style = html_label_style.trim().trim_end_matches(';');
        if !trimmed_html_style.is_empty() {
            div_style.push_str(trimmed_html_style);
            div_style.push(';');
        }
        if needs_wrap {
            let _ = write!(
                &mut div_style,
                "display: table; white-space: break-spaces; line-height: 1.5; max-width: {width}px; text-align: center; width: {width}px;",
                width = fmt_display(ctx.wrapping_width),
            );
        } else {
            let _ = write!(
                &mut div_style,
                "display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {}px; text-align: center;",
                fmt_display(ctx.wrapping_width),
            );
        }
        let _ = write!(
            out,
            r#"<foreignObject width="{}" height="{}"{}><div xmlns="http://www.w3.org/1999/xhtml" style="{}"><span class="nodeLabel"{}>{}</span></div></foreignObject></g></g>"#,
            fmt_display(width),
            fmt_display(height),
            HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR,
            escape_attr_display(&div_style),
            OptionalStyleAttr(&html_label_style),
            label_html,
        );
        return Ok(());
    }

    out.push_str(r#"<g><rect class="background" style="stroke: none"/>"#);
    let owner = Some(crate::flowchart::FlowchartSvgLabelOwner::SwimlaneEdgeLabel(
        key.semantic_index(),
    ));
    let prepared = crate::flowchart::FlowchartSvgLabelRenderPlan::new(
        ctx.svg_label_sidecar,
        owner,
        label_text,
        ctx.measurer,
        label_text_style,
        Some(ctx.wrapping_width),
        true,
        crate::flowchart::FlowchartSvgWidthMode::Bbox,
    );
    let swimlane_svg_text_style =
        swimlane_svg_edge_label_text_style(first_label_style, swimlane_adaptive_ink);
    write_flowchart_svg_label_plan_with_style(
        out,
        &prepared,
        true,
        (!swimlane_svg_text_style.is_empty()).then_some(swimlane_svg_text_style.as_str()),
    );
    out.push_str("</g></g></g>");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(all(feature = "diagram-flowchart", feature = "layout-shoal"))]
    #[test]
    fn shoal_mermaid_html_edge_label_inset_matches_layout_and_preserves_wrapping() {
        for theme in ["default", "dark"] {
            let source = format!(
                "---\nconfig:\n  layout: shoal\n  theme: {theme}\n  htmlLabels: true\n---\n\
                 flowchart LR\nA near@-->|{}| B\n\
                 B long@-->|A long explanation with several words that should wrap across multiple lines| C\n\
                 C --> D",
                "i".repeat(43)
            );
            let parsed = merman_core::MermaidEngine::new()
                .parse_diagram_for_render_model_sync(&source, merman_core::ParseOptions::strict())
                .unwrap()
                .unwrap();
            let session = crate::environment::RenderEnvironment::deterministic()
                .begin_session()
                .unwrap();
            let artifact = crate::family::prepare(
                parsed.presentation(),
                &crate::LayoutOptions::default(),
                session,
            )
            .unwrap();
            let layout = artifact.layout_json().unwrap();
            let svg = artifact
                .render_svg(&Default::default(), &Default::default())
                .unwrap();
            let doc = crate::xml_dom::Document::parse(svg.svg()).unwrap();
            let labels = layout["layout"]["FlowchartV2"]["edges"].as_array().unwrap();
            for (id, wrapping) in [("near", "nowrap"), ("long", "break-spaces")] {
                let label = labels.iter().find(|edge| edge["id"] == id).unwrap();
                let width = label["label"]["width"].as_f64().unwrap();
                let box_node = doc
                    .descendants()
                    .find(|node| {
                        node.has_tag_name("foreignObject")
                            && node.parent_element().unwrap().attribute("data-id") == Some(id)
                    })
                    .unwrap();
                assert!(width > 200.0 && width <= 208.0, "{theme} {id} {width}");
                assert!(
                    (width - box_node.attribute("width").unwrap().parse::<f64>().unwrap()).abs()
                        < 1e-9
                );
                let style = box_node
                    .first_element_child()
                    .unwrap()
                    .attribute("style")
                    .unwrap();
                assert!(style.contains("padding-inline: 4px;"));
                assert!(style.contains(&format!("white-space: {wrapping};")));
            }
            let empty = doc
                .descendants()
                .find(|node| {
                    node.has_tag_name("foreignObject")
                        && node.parent_element().unwrap().attribute("data-id") == Some("L_C_D_0")
                })
                .unwrap();
            assert_eq!(empty.attribute("width"), Some("0"));
            assert_eq!(empty.attribute("height"), Some("0"));
            assert!(
                !empty
                    .first_element_child()
                    .unwrap()
                    .attribute("style")
                    .unwrap()
                    .contains("padding-inline")
            );
        }
    }

    #[cfg(all(feature = "diagram-flowchart", feature = "layout-shoal"))]
    #[test]
    fn auto_backplates_composite_nested_container_paint_for_each_label() {
        let source = "---\nconfig:\n  layout: shoal\n---\nflowchart LR
subgraph Outer
    subgraph Inner
        A inner@-->|inner text| B
        B next@-->|second inner text| C
    end
    D outer@-->|outer text| E
end
X canvas@-->|canvas text| Y
style Outer fill:#523A60
style Inner fill:#88CCCC80,fill-opacity:0.5";
        let parsed = merman_core::MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, merman_core::ParseOptions::strict())
            .unwrap()
            .unwrap();
        let theme = crate::theme::ThemeCompiler::new()
            .compile_json(
                br#"{"schema_version":2,"extends":"studio","form":{"edge_label_bg":"auto"}}"#,
                None,
            )
            .unwrap();
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_native_theme(crate::theme::NativeTheme::new(theme))
            .unwrap()
            .with_canvas_background(crate::theme::Color::parse("#F1EEE3").unwrap());
        let svg = crate::family::prepare(
            parsed.presentation(),
            &crate::LayoutOptions::default(),
            session,
        )
        .unwrap()
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
        let doc = crate::xml_dom::Document::parse(svg.svg()).unwrap();
        for (id, color) in [
            ("inner", "#605F7B"),
            ("next", "#605F7B"),
            ("outer", "#523A60"),
            ("canvas", "#F1EEE3"),
        ] {
            let label = doc
                .descendants()
                .find(|node| {
                    node.has_tag_name("g")
                        && node.attribute("class") == Some("label")
                        && node.attribute("data-id") == Some(id)
                })
                .unwrap();
            let plate = label
                .parent_element()
                .unwrap()
                .prev_sibling_element()
                .unwrap();
            assert_eq!(
                plate.attribute("class"),
                Some("merman-edge-label-background")
            );
            assert_eq!(plate.attribute("fill"), Some(color));
        }
    }

    fn point(x: f64, y: f64) -> crate::model::LayoutPoint {
        crate::model::LayoutPoint { x, y }
    }

    fn geom(d: &str, points: Vec<crate::model::LayoutPoint>) -> FlowchartEdgePathGeom {
        FlowchartEdgePathGeom {
            d: d.to_string(),
            pb: None,
            neo_length: None,
            data_points: points.clone(),
            pre_hop_dash_mask_length: None,
            line_hop_applied: false,
            original_label_path_points: points.clone(),
            label_path_points: points,
            label_anchor: None,
            label_path_was_explicitly_updated: false,
            emitted_d_for_label: None,
            bounds_skipped_for_viewbox: false,
        }
    }

    #[test]
    fn ordinary_dagre_keeps_anchor_when_paint_does_not_move_the_route_midpoint() {
        let anchor = point(4.0, 5.0);
        let mut geometry = geom(
            "M0,0L10,0L20,0",
            vec![point(0.0, 0.0), point(10.0, 0.0), point(20.0, 0.0)],
        );

        let unchanged = position_flowchart_edge_label(anchor.clone(), &geometry, false);
        assert_eq!((unchanged.x, unchanged.y), (4.0, 5.0));

        geometry.label_path_was_explicitly_updated = true;
        let updated = position_flowchart_edge_label(anchor, &geometry, false);
        assert_eq!((updated.x, updated.y), (4.0, 5.0));
    }

    #[test]
    fn cluster_cut_moves_the_anchor_by_the_midpoint_delta() {
        let mut geometry = geom("M0,0L100,0", vec![point(0.0, 0.0), point(100.0, 0.0)]);
        geometry.original_label_path_points = vec![point(-20.0, 0.0), point(100.0, 0.0)];
        geometry.label_path_was_explicitly_updated = true;
        let label = position_flowchart_edge_label(point(12.0, 8.0), &geometry, false);
        assert_eq!((label.x, label.y), (22.0, 8.0));
    }

    #[test]
    fn elk_paints_the_projected_anchor_without_recentering_on_the_whole_route() {
        let mut geometry = geom(
            "M0,0L100,0L100,100",
            vec![point(0.0, 0.0), point(100.0, 0.0), point(100.0, 100.0)],
        );
        geometry.label_anchor = Some(point(60.0, 0.0));
        let label = position_flowchart_edge_label(point(60.0, 10.0), &geometry, false);
        assert_eq!((label.x, label.y), (60.0, 0.0));
        assert_ne!((label.x, label.y), (100.0, 0.0));
    }

    #[test]
    fn swimlane_updated_waypoints_still_apply_the_midpoint_delta() {
        let mut geometry = geom(
            "M0,0L10,0L20,0",
            vec![point(0.0, 0.0), point(10.0, 0.0), point(20.0, 0.0)],
        );
        geometry.original_label_path_points =
            vec![point(-20.0, 0.0), point(0.0, 0.0), point(20.0, 0.0)];
        geometry.label_path_was_explicitly_updated = true;
        let positioned = position_flowchart_edge_label(point(4.0, 5.0), &geometry, false);
        assert_eq!((positioned.x, positioned.y), (14.0, 5.0));
    }

    #[test]
    fn swimlane_unchanged_clipped_pins_preserve_the_layout_label_anchor() {
        let mut geometry = geom(
            "M403,212.5L667.929,212.5Q675,212.5 675,219.571L675,561.429Q675,568.5 667.929,568.5L407,568.5",
            vec![
                point(403.0, 212.5),
                point(675.0, 212.5),
                point(675.0, 568.5),
                point(403.0, 568.5),
            ],
        );
        geometry.original_label_path_points = vec![
            point(403.0, 212.5),
            point(403.0, 212.5),
            point(675.0, 212.5),
            point(675.0, 568.5),
            point(403.0, 568.5),
            point(403.0, 568.5),
        ];
        // upstream_7_car_sales_constr: clipping removes the duplicate terminal
        // pins, but insertEdge returns no updatedPath for this rounded route.
        let positioned = position_flowchart_edge_label(point(539.0, 212.5), &geometry, false);
        assert_eq!((positioned.x, positioned.y), (539.0, 212.5));
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
        assert_eq!((rough_curve.x, rough_curve.y), (4.0, 5.0));
    }

    #[test]
    fn shared_geometry_handles_empty_single_degenerate_and_polyline_paths() {
        let anchor = point(4.0, 5.0);
        let empty = geom("", Vec::new());
        let positioned = position_flowchart_edge_label(anchor.clone(), &empty, true);
        assert_eq!((positioned.x, positioned.y), (4.0, 5.0));

        let single = geom("M2,3", vec![point(2.0, 3.0)]);
        let positioned = position_flowchart_edge_label(anchor.clone(), &single, true);
        assert_eq!((positioned.x, positioned.y), (4.0, 5.0));

        let degenerate = geom("M2,3L2,3", vec![point(2.0, 3.0), point(2.0, 3.0)]);
        let positioned = position_flowchart_edge_label(anchor.clone(), &degenerate, true);
        assert_eq!((positioned.x, positioned.y), (4.0, 5.0));

        let polyline = geom(
            "M0,0L6,0L6,8",
            vec![point(0.0, 0.0), point(6.0, 0.0), point(6.0, 8.0)],
        );
        let positioned = position_flowchart_edge_label(anchor, &polyline, true);
        assert_eq!((positioned.x, positioned.y), (4.0, 5.0));
    }

    #[cfg(all(feature = "diagram-flowchart", feature = "layout-shoal"))]
    #[test]
    fn adaptive_edge_label_ink_ignores_background_color_and_styles_svg_markdown_labels() {
        let source = "---\nconfig:\n  layout: shoal\n  htmlLabels: false\n  flowchart:\n    htmlLabels: false\n---\nflowchart LR\n\
            subgraph Dark[Dark Zone]\n\
                A plain@-->|Plain text| B\n\
                B md@-->|\"`**Bold** edge`\"| C\n\
            end\n\
            style Dark fill:#0b1120,stroke:#334155\n\
            linkStyle 0 background-color:transparent\n";
        let parsed = merman_core::MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, merman_core::ParseOptions::strict())
            .unwrap()
            .unwrap();
        let theme = crate::theme::ThemeCompiler::new()
            .compile_json(
                br#"{"schema_version":2,"extends":"studio","form":{"edge_label_bg":"auto"}}"#,
                None,
            )
            .unwrap();
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_native_theme(crate::theme::NativeTheme::new(theme))
            .unwrap();
        let svg = crate::family::prepare(
            parsed.presentation(),
            &crate::LayoutOptions::default(),
            session,
        )
        .unwrap()
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
        let out = svg.svg();
        assert!(
            !out.contains("background-fill:"),
            "background-color must never be rewritten into background-fill: {out}"
        );
        let doc = crate::xml_dom::Document::parse(out).unwrap();
        for id in ["plain", "md"] {
            let label = doc
                .descendants()
                .find(|node| {
                    node.has_tag_name("g")
                        && node.attribute("class") == Some("label")
                        && node.attribute("data-id") == Some(id)
                })
                .unwrap_or_else(|| panic!("missing edge label {id}"));
            let text = label
                .descendants()
                .find(|node| node.has_tag_name("text"))
                .unwrap_or_else(|| panic!("missing SVG <text> for {id}"));
            let style = text.attribute("style").unwrap_or("");
            assert!(
                style.contains("fill:#FFFFFF"),
                "expected adaptive white fill on {id} over dark subgraph, got style={style:?}"
            );
        }
    }

    #[test]
    fn cssparser_label_style_rewrites_preserve_quoted_semicolons_and_avoid_double_semicolons() {
        assert_eq!(
            flowchart_svg_edge_label_text_style(
                r#"font-family:"Foo;color:red" !important;color:#ff0 !important"#,
                None,
                false,
            ),
            r#"font-family:"Foo;color:red" !important;fill:#ff0 !important"#
        );
        assert_eq!(
            flowchart_svg_edge_label_text_style(
                r#"font-family:"Foo;color:red" !important"#,
                Some("#FFFFFF"),
                false,
            ),
            r#"font-family:"Foo;color:red" !important;fill:#FFFFFF"#
        );
        assert_eq!(
            swimlane_html_edge_label_style(r#"font-family:"Foo;fill:red";FILL:#333;"#, None,),
            r#"font-family:"Foo;fill:red";color:#333"#
        );
        assert_eq!(
            swimlane_html_edge_label_style("font-size:14px;", Some("#FFFFFF")),
            "font-size:14px;color:#FFFFFF !important"
        );
        assert_eq!(
            swimlane_svg_edge_label_text_style(r#"font-family:"Foo;color:red";COLOR:yellow"#, None,),
            r#"font-family:"Foo;color:red";fill:yellow"#
        );
        assert_eq!(
            swimlane_svg_edge_label_text_style("stroke:#2563eb", Some("#FFFFFF")),
            "fill:#FFFFFF"
        );
    }

    #[cfg(all(feature = "diagram-swimlane", feature = "layout-shoal"))]
    #[test]
    fn swimlane_svg_and_html_edge_labels_apply_adaptive_ink_and_preserve_hex_styles() {
        let theme = crate::theme::ThemeCompiler::new()
            .compile_json(
                br#"{"schema_version":2,"extends":"studio","form":{"edge_label_bg":"auto"}}"#,
                None,
            )
            .unwrap();

        let svg_source = "---\nconfig:\n  htmlLabels: false\n  flowchart:\n    htmlLabels: false\n---\nswimlane-beta LR\n\
            subgraph Dark[Dark Lane]\n\
                A -->|Route| B\n\
            end\n\
            style Dark fill:#0b1120,stroke:#334155\n";
        let parsed = merman_core::MermaidEngine::new()
            .parse_diagram_for_render_model_sync(svg_source, merman_core::ParseOptions::strict())
            .unwrap()
            .unwrap();
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_native_theme(crate::theme::NativeTheme::new(theme.clone()))
            .unwrap();
        let rendered = crate::family::prepare(
            parsed.presentation(),
            &crate::LayoutOptions::default(),
            session,
        )
        .unwrap()
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
        let doc = crate::xml_dom::Document::parse(rendered.svg()).unwrap();
        let label = doc
            .descendants()
            .find(|node| node.has_tag_name("g") && node.attribute("data-et") == Some("edge-label"))
            .expect("swimlane edge-label group");
        let text = label
            .descendants()
            .find(|node| node.has_tag_name("text"))
            .expect("swimlane SVG edge label text");
        assert!(
            text.attribute("style")
                .unwrap_or("")
                .contains("fill:#FFFFFF"),
            "swimlane SVG-text edge label must set fill:#FFFFFF directly on <text>: {:?}",
            text.attribute("style")
        );

        let html_source = "---\nconfig:\n  htmlLabels: true\n---\nswimlane-beta LR\n\
            subgraph Dark[Dark Lane]\n\
                A -->|Styled| B\n\
                B -->|Adaptive| C\n\
            end\n\
            style Dark fill:#0b1120,stroke:#334155\n\
            linkStyle 0 fill:#333;\n";
        let parsed = merman_core::MermaidEngine::new()
            .parse_diagram_for_render_model_sync(html_source, merman_core::ParseOptions::strict())
            .unwrap()
            .unwrap();
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_native_theme(crate::theme::NativeTheme::new(theme))
            .unwrap();
        let rendered = crate::family::prepare(
            parsed.presentation(),
            &crate::LayoutOptions::default(),
            session,
        )
        .unwrap()
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
        let out = rendered.svg();
        assert!(
            !out.contains("&amp;333;"),
            "hex color followed by `;` in swimlane style must not be Mermaid-entity decoded: {out}"
        );
        assert!(
            !out.contains("color:#FFFFFF !important;;"),
            "swimlane HTML div style must not emit double semicolons: {out}"
        );
    }

    #[cfg(all(feature = "diagram-flowchart", feature = "layout-shoal"))]
    #[test]
    fn transparent_edge_label_bg_case_insensitive_color_and_hex_edge_style_escaping() {
        let source = "---\nconfig:\n  layout: shoal\n  htmlLabels: true\n---\nflowchart LR\n\
            subgraph Dark[Dark Zone]\n\
                A auto@-->|Auto Ink| B\n\
                B authored@-->|Authored Upper| C\n\
            end\n\
            style Dark fill:#0b1120,stroke:#334155\n\
            linkStyle 0 stroke:#333;\n\
            linkStyle 1 COLOR:yellow\n";
        let parsed = merman_core::MermaidEngine::new()
            .parse_diagram_for_render_model_sync(source, merman_core::ParseOptions::strict())
            .unwrap()
            .unwrap();
        let theme = crate::theme::ThemeCompiler::new()
            .compile_json(
                br#"{"schema_version":2,"extends":"studio","form":{"edge_label_bg":"transparent"}}"#,
                None,
            )
            .unwrap();
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session_with_native_theme(crate::theme::NativeTheme::new(theme))
            .unwrap();
        let rendered = crate::family::prepare(
            parsed.presentation(),
            &crate::LayoutOptions::default(),
            session,
        )
        .unwrap()
        .render_svg(&Default::default(), &Default::default())
        .unwrap();
        let out = rendered.svg();
        assert!(
            !out.contains("merman-edge-label-background"),
            "transparent edge_label_bg must not emit backplate rects: {out}"
        );
        assert!(
            !out.contains("&amp;333;"),
            "edge path style `#333;` must not be Mermaid-entity corrupted: {out}"
        );
        let doc = crate::xml_dom::Document::parse(out).unwrap();
        let auto_span = doc
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && node.attribute("class") == Some("label")
                    && node.attribute("data-id") == Some("auto")
            })
            .and_then(|label| {
                label.descendants().find(|node| {
                    node.has_tag_name("span") && node.attribute("class") == Some("edgeLabel")
                })
            })
            .expect("auto edge label span");
        assert!(
            auto_span
                .attribute("style")
                .unwrap_or("")
                .contains("color:#FFFFFF !important"),
            "transparent edge_label_bg must still select adaptive white ink over dark cluster: {:?}",
            auto_span.attribute("style")
        );
        let authored_span = doc
            .descendants()
            .find(|node| {
                node.has_tag_name("g")
                    && node.attribute("class") == Some("label")
                    && node.attribute("data-id") == Some("authored")
            })
            .and_then(|label| {
                label.descendants().find(|node| {
                    node.has_tag_name("span") && node.attribute("class") == Some("edgeLabel")
                })
            })
            .expect("authored edge label span");
        assert!(
            !authored_span
                .attribute("style")
                .unwrap_or("")
                .contains("color:#FFFFFF"),
            "uppercase COLOR:yellow must suppress adaptive ink override: {:?}",
            authored_span.attribute("style")
        );
    }
}
