//! Paint the selected provider's prepared lanes and label waypoints.

use super::*;
use crate::er::ErSwimlanePaintLayout;
use crate::model::{LayoutEdge, SwimlaneDirection, SwimlaneLaneLayout};
use crate::svg::parity::line_hops::{LineHopConfig, LineHopEdge, LineHopStyle};
use std::collections::HashMap;

pub(super) fn render_cluster(
    out: &mut impl SvgOutput,
    cluster: &LayoutCluster,
    lane: &SwimlaneLaneLayout,
    paint: &ErSwimlanePaintLayout,
    subgraph: Option<&crate::er::ErSubgraph>,
    context: ErSubgraphRenderContext<'_>,
    receipt: &mut crate::er::ErEntityThemeReceipt<'_>,
) -> Result<()> {
    let binding = context.theme.css_binding();
    let source = subgraph
        .map(|group| {
            context
                .theme
                .subgraph_source_style(&group.id)
                .ok_or_else(|| Error::InvalidModel {
                    message: format!("ER prepared source style is missing Swimlane {}", group.id),
                })
        })
        .transpose()?;
    let style_attr = source.map_or(r#"style="""#, |source| {
        source.visual.rect_style_attr.as_str()
    });
    let mut classes = String::from("cluster swimlane");
    if let Some(group) = subgraph {
        for class in &group.classes {
            classes.push(' ');
            classes.push_str(class);
        }
    }
    let dom_id = subgraph.map_or_else(
        || lane.id.clone(),
        |_| format!("{}-{}", context.diagram_id, lane.id),
    );
    let _ = write!(
        out,
        r#"<g class="{}" id="{}" data-id="{}" data-et="cluster" data-look="{}""#,
        escape_attr(&classes),
        escape_attr(&dom_id),
        escape_attr(&lane.id),
        escape_attr(context.data_look),
    );
    if subgraph.is_none() && !binding.redux_border_colors.is_empty() {
        let slot = context.synthetic_color_ordinal % binding.redux_border_colors.len();
        let _ = write!(out, r#" data-color-id="color-{slot}""#);
    }
    out.push('>');
    let geometry = paint.lane_geometry(lane);
    let left = lane.x - geometry.width / 2.0 + context.translate_x;
    let top = lane.y - geometry.height / 2.0 + context.translate_y;
    let is_lr = paint.direction == SwimlaneDirection::Lr;
    let (title_rect, body_rect) = if is_lr {
        (
            crate::geom::Box2::from_min_max(
                left,
                top,
                left + geometry.title_band_width,
                top + geometry.height,
            ),
            crate::geom::Box2::from_min_max(
                left + geometry.title_band_width,
                top,
                (left + geometry.width).max(left + geometry.title_band_width),
                top + geometry.height,
            ),
        )
    } else {
        (
            crate::geom::Box2::from_min_max(
                left,
                top,
                left + geometry.width,
                top + geometry.title_band_height,
            ),
            crate::geom::Box2::from_min_max(
                left,
                top + geometry.title_band_height,
                left + geometry.width,
                top + geometry.height,
            ),
        )
    };
    let source_fill = source.and_then(|source| source.fill());
    write_lane_rect(
        out,
        "swimlane-body",
        body_rect,
        source_fill,
        style_attr,
        source,
        context,
    )?;
    write_lane_rect(
        out,
        "swimlane-title",
        title_rect,
        Some(source_fill.unwrap_or(&binding.cluster_bkg)),
        style_attr,
        source,
        context,
    )?;

    let width = lane.title_label_width.max(0.0);
    let height = lane.title_label_height.max(0.0);
    let cx = cluster.title_label.x + context.translate_x;
    let cy = cluster.title_label.y + context.translate_y;
    let transform = if is_lr {
        format!(
            "translate({}, {}) rotate(-90) translate({}, {})",
            fmt(cx),
            fmt(cy),
            fmt(-width / 2.0),
            fmt(-height / 2.0)
        )
    } else {
        format!(
            "translate({}, {})",
            fmt(cx - width / 2.0),
            fmt(cy - height / 2.0)
        )
    };
    let _ = write!(
        out,
        r#"<g class="cluster-label swimlane-label" transform="{}">"#,
        escape_attr(&transform)
    );
    if context.swimlane_html_labels {
        let prepared =
            context
                .theme
                .html_subgraph_label(&lane.id)
                .ok_or_else(|| Error::InvalidModel {
                    message: format!("ER prepared HTML title is missing Swimlane {}", lane.id),
                })?;
        let text_attr = source.map_or(r#"style="""#, |source| {
            source.visual.source_text_attr.as_str()
        });
        let _ = write!(
            out,
            r#"<foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {}px; text-align: center;"><span class="nodeLabel" {}>{}</span></div></foreignObject>"#,
            fmt(width),
            fmt(height),
            fmt(geometry.width),
            text_attr,
            prepared.fragment(),
        );
        if context.record_text {
            receipt.record_html_subgraph_label(&lane.id);
        }
    } else {
        let text_style = source.map_or("", |source| source.visual.subgraph_text_style.as_str());
        out.push_str("<g>");
        write_er_svg_label_background(out);
        let _ = write!(out, r#"<g transform="translate({}, 0)">"#, fmt(width / 2.0));
        crate::svg::parity::label::write_svg_text_markdown_wrapped_centered_with_style(
            out,
            &lane.title,
            text_style,
            context.measurer,
            context.label_style,
            None,
        );
        out.push_str("</g></g>");
        if context.record_text
            && let Some(group) = subgraph
        {
            receipt.record_subgraph_label(
                &group.id,
                &crate::er::subgraph_svg_text_facts(group, context.label_style.font_size),
            );
        }
    }
    out.push_str("</g></g>");
    out.checkpoint()
}

fn write_lane_rect(
    out: &mut impl SvgOutput,
    class: &str,
    rect: crate::geom::Box2,
    fill: Option<&str>,
    style_attr: &str,
    source: Option<&crate::er::ErEntitySourceStyle>,
    context: ErSubgraphRenderContext<'_>,
) -> Result<()> {
    let stroke = source
        .and_then(|source| source.rect_value("stroke"))
        .unwrap_or(&context.theme.css_binding().cluster_border);
    if context.data_look == "handDrawn" {
        let stroke_width = source
            .and_then(|source| source.rect_value("stroke-width"))
            .and_then(|value| {
                value
                    .trim()
                    .trim_end_matches("px")
                    .trim()
                    .parse::<f32>()
                    .ok()
            })
            .filter(|value| value.is_finite() && *value >= 0.0)
            .unwrap_or(1.3);
        let dash = source
            .and_then(|source| source.rect_value("stroke-dasharray"))
            .unwrap_or("0 0");
        let dash_values = dash
            .split(|ch: char| ch == ',' || ch.is_ascii_whitespace())
            .filter_map(|value| value.parse::<f64>().ok())
            .collect::<Vec<_>>();
        let mut builder = roughr::core::OptionsBuilder::default();
        builder
            .randomness(context.rough_randomness.clone())
            .roughness(0.7)
            .stroke(roughr::Srgba::new(0.0, 0.0, 0.0, 1.0))
            .stroke_width(stroke_width)
            .stroke_line_dash(dash_values)
            .fill_style(roughr::core::FillStyle::Hachure)
            .fill_weight(3.0)
            .hachure_gap(1.5);
        if fill.is_some() {
            builder.fill(roughr::Srgba::new(0.0, 0.0, 0.0, 1.0));
        }
        let options = builder.build().map_err(|error| Error::InvalidModel {
            message: format!("invalid ER Swimlane rough rectangle options: {error}"),
        })?;
        let drawable = crate::svg::parity::roughjs_common::controlled::generate(
            context.work_meter,
            merman_model::OperationPhase::Emit,
            |control| {
                roughr::generator::Generator::default().rectangle_with_control::<f64>(
                    rect.min_x(),
                    rect.min_y(),
                    rect.width(),
                    rect.height(),
                    &Some(options),
                    control,
                )
            },
        )?;
        let _ = write!(out, r#"<g class="{}" {}>"#, escape_attr(class), style_attr);
        for set in drawable.sets {
            let outline = set.op_set_type == roughr::core::OpSetType::Path;
            let color = if outline {
                stroke
            } else {
                fill.unwrap_or("none")
            };
            let path = crate::svg::parity::roughjs_common::ops_to_svg_path_d(&set);
            let _ = write!(
                out,
                r#"<path d="{}" stroke="{}" stroke-width="{}" stroke-dasharray="{}" fill="none"/>"#,
                escape_attr(&path),
                escape_attr(color),
                fmt(if outline {
                    f64::from(stroke_width)
                } else {
                    3.0
                }),
                escape_attr(if outline { dash } else { "0 0" })
            );
        }
        out.push_str("</g>");
    } else {
        let _ = write!(
            out,
            r#"<rect class="{}" {} x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}"/>"#,
            escape_attr(class),
            style_attr,
            fmt(rect.min_x()),
            fmt(rect.min_y()),
            fmt(rect.width()),
            fmt(rect.height()),
            escape_attr(fill.unwrap_or("none")),
            escape_attr(stroke),
        );
    }
    out.checkpoint()
}

#[allow(
    clippy::too_many_arguments,
    reason = "The prepared Swimlane label keeps geometry, source identity, foreground, and backing evidence together."
)]
pub(super) fn render_relationship_label(
    out: &mut impl SvgOutput,
    edge: &LayoutEdge,
    index: usize,
    model: &merman_model::families::er::ErDiagramModel,
    layout: &ErDiagramLayout,
    context: ErSubgraphRenderContext<'_>,
    receipt: &mut crate::er::ErEntityThemeReceipt<'_>,
    background: [Option<&str>; 2],
) -> Result<()> {
    let Some(label) = &edge.label else {
        return Ok(());
    };
    let prepared_label =
        layout
            .prepared_labels
            .relationship(index)
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing prepared Swimlane relationship label {index}"),
            })?;
    let prepared = prepared_label
        .swimlane()
        .ok_or_else(|| Error::InvalidModel {
            message: format!("missing prepared Swimlane relationship terminal {index}"),
        })?;
    let visual = context.theme.relation_label_visual(index);
    let dom_id = er_edge_dom_id(&edge.id, &model.relationships);
    let label_id = format!("edge-label-{}-{}-{dom_id}", edge.from, edge.to);
    let _ = write!(
        out,
        r#"<g class="label edgeLabel" id="{}" transform="translate({}, {})"><rect width="0.1" height="0.1"/><g class="label"{} transform="translate({}, {})"><rect/>"#,
        escape_attr(&label_id),
        fmt(label.x + context.translate_x),
        fmt(label.y + context.translate_y),
        visual.map_or(r#" style="""#, |visual| {
            if visual.wrapper_attr.is_empty() {
                r#" style="""#
            } else {
                visual.wrapper_attr.as_str()
            }
        }),
        fmt(-label.width / 2.0),
        fmt(-label.height / 2.0),
    );
    if let Some((fragment, wrapper_style)) = prepared.html() {
        let wrapper_background =
            background[0].map(|paint| format!("{wrapper_style};background-color:{paint};"));
        let wrapper_style = wrapper_background.as_deref().unwrap_or(wrapper_style);
        let span_style = background[1].map(|paint| {
            format!(
                r#" style="{};background-color:{};""#,
                escape_attr(visual.map_or("", |visual| visual.typed_style.as_str())),
                escape_attr(paint)
            )
        });
        let _ = write!(
            out,
            r#"<foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="{}"><span class="nodeLabel"{}>{}</span></div></foreignObject>"#,
            fmt(label.width),
            fmt(label.height),
            escape_attr(wrapper_style),
            span_style
                .as_deref()
                .unwrap_or_else(|| visual.map_or("", |visual| visual.inline_attr.as_str())),
            fragment,
        );
    } else {
        let lines = prepared.svg_lines().ok_or_else(|| Error::InvalidModel {
            message: format!("missing prepared Swimlane SVG rows for relationship {index}"),
        })?;
        if let Some(paint) = background[1] {
            let _ = write!(
                out,
                r#"<rect class="background" style="fill:{} !important;stroke:none" x="0" y="0" width="{}" height="{}"/>"#,
                escape_attr(paint),
                fmt(label.width),
                fmt(label.height)
            );
        } else {
            write_er_svg_label_background(out);
        }
        crate::svg::parity::label::write_svg_text_source_word_lines_with_style(
            out,
            lines,
            visual.map_or("", |visual| visual.typed_style.as_str()),
            false,
            None,
        );
    }
    out.push_str("</g></g>");
    out.checkpoint()?;
    let facts = prepared_label.visible_style_facts();
    if receipt.records_text_terminals() && facts.parse_valid() && facts.has_visible_runs() {
        receipt.record_relation_label_text(index, facts);
    }
    Ok(())
}

pub(super) fn line_hop_paths<'a>(
    config: &serde_json::Value,
    edges: &[LineHopEdge<'a>],
    work_meter: &crate::resources::OperationWorkMeter,
) -> Result<HashMap<&'a str, String>> {
    let paths = crate::svg::parity::line_hops::process_edges_with_line_hops(
        edges,
        LineHopConfig {
            enabled: true,
            jump_radius: 6.0,
            jump_style: if config
                .pointer("/swimlane/lineHops")
                .and_then(serde_json::Value::as_str)
                == Some("gap")
            {
                LineHopStyle::Gap
            } else {
                LineHopStyle::Arc
            },
        },
        work_meter,
    )?;
    Ok(paths
        .into_iter()
        .filter(|path| path.was_rewritten)
        .map(|path| (path.edge_id, path.path))
        .collect())
}
