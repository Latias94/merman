//! Paint the selected provider's prepared lanes and label waypoints.

use super::*;
use line_hops::{LineHopConfig, LineHopEdge, LineHopStyle};
use merman_render_core::config::{ConfigLookup, value_at};
use merman_render_core::{LayoutLabel, SwimlaneDirection, SwimlaneLaneLayout};
use std::collections::HashMap;

use crate::label_rect::LabelRect;

pub(super) fn render_cluster(
    out: &mut impl SvgOutput,
    cluster: &LayoutCluster,
    dom_id: &str,
    lane: &SwimlaneLaneLayout,
    paint: &ErSwimlanePaintLayout,
    subgraph: Option<&ErSubgraph>,
    context: ErSubgraphRenderContext<'_>,
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
        source.visual().rect_style_attr.as_str()
    });
    let mut classes = String::from("cluster swimlane");
    if let Some(group) = subgraph {
        for class in &group.classes {
            classes.push(' ');
            classes.push_str(class);
        }
    }
    let _ = write!(
        out,
        r#"<g class="{}" id="{}" data-id="{}" data-et="cluster" data-look="{}""#,
        escape_attr(&classes),
        escape_attr(dom_id),
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
            kurbo::Rect::new(
                left,
                top,
                left + geometry.title_band_width,
                top + geometry.height,
            ),
            kurbo::Rect::new(
                left + geometry.title_band_width,
                top,
                (left + geometry.width).max(left + geometry.title_band_width),
                top + geometry.height,
            ),
        )
    } else {
        (
            kurbo::Rect::new(
                left,
                top,
                left + geometry.width,
                top + geometry.title_band_height,
            ),
            kurbo::Rect::new(
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
            source.visual().source_text_attr.as_str()
        });
        let _ = write!(
            out,
            r#"<foreignObject width="{}" height="{}"{}><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {}px; text-align: center;"><span class="nodeLabel" {}>{}</span></div></foreignObject>"#,
            fmt(width),
            fmt(height),
            html_label_overflow_attr(context.allow_html_label_overflow, width, height),
            fmt(geometry.width),
            text_attr,
            prepared.fragment(),
        );
    } else {
        out.push_str("<g>");
        write_er_svg_label_background(out);
        let _ = write!(out, r#"<g transform="translate({}, 0)">"#, fmt(width / 2.0));
        label::write_svg_text_markdown_wrapped_centered_with_style(
            out,
            &lane.title,
            "",
            context.measurer,
            context.label_style,
            None,
        );
        out.push_str("</g></g>");
    }
    out.push_str("</g></g>");
    out.checkpoint()
}

fn write_lane_rect(
    out: &mut impl SvgOutput,
    class: &str,
    rect: kurbo::Rect,
    fill: Option<&str>,
    style_attr: &str,
    source: Option<&super::super::ErEntitySourceStyle>,
    context: ErSubgraphRenderContext<'_>,
) -> Result<()> {
    use merman_markup::theme_color::{ColorChannel, ThemeColor};

    let stroke = source
        .and_then(|source| source.rect_value("stroke"))
        .unwrap_or(&context.theme.css_binding().cluster_border);
    if context.data_look == "handDrawn" {
        let opacity = |property| source.and_then(|source| source.rect_value(property));
        let fill_opacity = opacity("fill-opacity").unwrap_or("1");
        let opacity_value = |value: &str| {
            let (number, scale) = value.strip_suffix('%').map_or((value, 1.0), |v| (v, 0.01));
            number
                .trim()
                .parse::<f64>()
                .ok()
                .filter(|value| value.is_finite())
                .map_or(1.0, |value| (value * scale).clamp(0.0, 1.0))
        };
        let hatch_opacity = fill
            .filter(|fill| !fill.eq_ignore_ascii_case("none"))
            .map_or(0.0, |fill| {
                let alpha =
                    ThemeColor::parse(fill).map_or(1.0, |color| color.channel(ColorChannel::Alpha));
                0.15 * alpha * opacity_value(fill_opacity)
            });
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
        if hatch_opacity > 0.0 {
            builder.fill(roughr::Srgba::new(0.0, 0.0, 0.0, 1.0));
        }
        let options = builder.build().map_err(|error| Error::InvalidModel {
            message: format!("invalid ER Swimlane rough rectangle options: {error}"),
        })?;
        let drawable = merman_render_core::rough_svg::controlled::generate(
            context.work_meter,
            merman_model::OperationPhase::Emit,
            |control| {
                roughr::generator::Generator::default().rectangle_with_control::<f64>(
                    rect.x0,
                    rect.y0,
                    rect.width(),
                    rect.height(),
                    &Some(options),
                    control,
                )
            },
        )?;
        let _ = write!(out, r#"<g class="{}""#, escape_attr(class));
        if let Some(group_opacity) = opacity("opacity") {
            let _ = write!(out, r#" style="opacity:{}""#, escape_attr(group_opacity));
        }
        out.push('>');
        if let Some(fill) = fill {
            let _ = write!(
                out,
                r#"<rect class="merman-rough-backing" x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="none" style="fill:{} !important;fill-opacity:{};stroke:none !important"/>"#,
                fmt(rect.x0),
                fmt(rect.y0),
                fmt(rect.width()),
                fmt(rect.height()),
                escape_attr(fill),
                escape_attr(fill),
                escape_attr(fill_opacity),
            );
        }
        for set in drawable.sets {
            let outline = set.op_set_type == roughr::core::OpSetType::Path;
            let path = merman_render_core::rough_svg::ops_to_svg_path_d(&set);
            let stroke_opacity = fmt(hatch_opacity).to_string();
            let _ = write!(
                out,
                r#"<path d="{}" stroke="{}" stroke-width="{}" stroke-opacity="{}" stroke-dasharray="{}" fill="none"/>"#,
                escape_attr(&path),
                escape_attr(stroke),
                fmt(if outline {
                    f64::from(stroke_width)
                } else {
                    3.0
                }),
                escape_attr(if outline {
                    opacity("stroke-opacity").unwrap_or("1")
                } else {
                    &stroke_opacity
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
            fmt(rect.x0),
            fmt(rect.y0),
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
    reason = "The prepared Swimlane label keeps geometry and source styling together."
)]
pub(super) fn render_relationship_label(
    out: &mut impl SvgOutput,
    edge: &LayoutEdge,
    index: usize,
    model: &ErDiagramModel,
    prepared_labels: &ErPreparedLabels,
    context: ErSubgraphRenderContext<'_>,
) -> Result<()> {
    let Some(label) = &edge.label else {
        return Ok(());
    };
    let prepared_label =
        prepared_labels
            .relationship(index)
            .ok_or_else(|| Error::InvalidModel {
                message: format!("missing prepared Swimlane relationship label {index}"),
            })?;
    let prepared = prepared_label
        .label_rect()
        .ok_or_else(|| Error::InvalidModel {
            message: format!("missing prepared Swimlane relationship terminal {index}"),
        })?;
    let dom_id = er_edge_dom_id(&edge.id, &model.relationships);
    let label_id = relationship_label_dom_id(edge, &dom_id);
    write_relationship_label_rect(out, label, &label_id, prepared, context)
}

pub(super) fn write_relationship_label_rect(
    out: &mut impl SvgOutput,
    label: &LayoutLabel,
    label_id: &str,
    prepared: &LabelRect,
    context: ErSubgraphRenderContext<'_>,
) -> Result<()> {
    prepared.write(
        out,
        label,
        label_id,
        (context.translate_x, context.translate_y),
        context.allow_html_label_overflow,
    )
}

pub(super) fn relationship_label_dom_id(edge: &LayoutEdge, dom_id: &str) -> String {
    format!("edge-label-{}-{}-{dom_id}", edge.from, edge.to)
}

pub(super) fn line_hop_paths<'a>(
    config: &(impl ConfigLookup + ?Sized),
    edges: &[LineHopEdge<'a>],
    work_meter: &OperationWorkMeter,
) -> Result<HashMap<&'a str, String>> {
    let paths = line_hops::line_hop_overrides(
        edges,
        LineHopConfig {
            jump_radius: 6.0,
            jump_style: if value_at(config, &["swimlane", "lineHops"])
                .and_then(serde_json::Value::as_str)
                == Some("gap")
            {
                LineHopStyle::Gap
            } else {
                LineHopStyle::Arc
            },
        },
        |_| true,
        work_meter,
    )?;
    Ok(paths
        .into_iter()
        .map(|(index, path)| (edges[index].id, path))
        .collect())
}
