//! Balanced SVG for a prepared Swimlane lane or pool.

use std::borrow::Cow;

use kurbo::{Point, Vec2};
use merman_model::{families::flowchart::FlowSubgraph, presentation::PresentationConfig};
use merman_render_core::{
    Error, Result, SwimlaneDirection, SwimlaneLaneLayout,
    config::DiagramLook,
    html::HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR,
    math::MathRenderer,
    number_format::canonical_number as fmt_display,
    svg_output::SvgOutput,
    text::{
        TextMeasurer,
        inline_html::SvgTextEntityMode,
        literal_label::FlowchartSvgWidthMode,
        mermaid_markdown_svg_plain_text,
        svg_label::{
            write_svg_markdown_word_lines_centered_with_style,
            write_svg_markdown_word_lines_with_style,
            write_svg_text_markdown_wrapped_from_create_text_source_with_style,
            write_svg_text_source_word_lines_with_style,
        },
    },
    xml::escape_attr_display,
};

use super::{FlowchartClusterShape, swimlane_terminal_geometry, write_swimlane_shell};
use crate::{
    document::FlowchartDocumentIds,
    graph::FlowchartClusterPass,
    lane_title::zero_width_lane_title_rows,
    layout_config::FLOWCHART_FIXED_LABEL_WRAP_WIDTH,
    projection::FlowchartRenderModelRef,
    stylesheet::ContainerPalette,
    svg_label::{
        FlowchartSvgLabelRenderPlan, FlowchartSvgLabelSidecar, OptionalStyleAttr,
        append_native_label_color, flowchart_label_html_with_prepared_label,
        write_flowchart_svg_label_plan, write_flowchart_svg_text_markdown,
    },
};

/// Borrowed geometry, source projection, paint, and label services for one lane or pool.
pub struct SwimlaneClusterSvg<'a> {
    pub lane: &'a SwimlaneLaneLayout,
    pub cluster_id: &'a str,
    pub subgraph: Option<&'a FlowSubgraph>,
    pub subgraph_index: Option<usize>,
    pub model: &'a FlowchartRenderModelRef<'a>,
    pub document_ids: &'a FlowchartDocumentIds,
    pub palette: &'a ContainerPalette<'a>,
    pub diagram_type: &'a str,
    pub direction: SwimlaneDirection,
    pub canonical: bool,
    pub is_pool: bool,
    pub look: DiagramLook<'a>,
    pub shape: FlowchartClusterShape<'a>,
    pub translation: Vec2,
    pub group_origin: Point,
    pub html_labels: bool,
    pub svg_font_fallback: Option<&'a str>,
    pub svg_label_sidecar: Option<&'a FlowchartSvgLabelSidecar>,
    pub config: &'a PresentationConfig,
    pub measurer: &'a dyn TextMeasurer,
    pub math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
}

impl FlowchartClusterPass {
    /// Lanes emit their title with the shell, including in a background pass.
    pub fn draws_swimlane(self) -> bool {
        self != Self::Label
    }
}

pub(crate) fn swimlane_color_slot(
    model: &FlowchartRenderModelRef<'_>,
    palette: &ContainerPalette<'_>,
    diagram_type: &str,
    id: &str,
    synthetic: bool,
) -> Option<usize> {
    model
        .subgraph_color_ordinal(id)
        .and_then(|ordinal| palette.slot_for_ordinal(diagram_type, ordinal))
        .or_else(|| {
            let palette_len = palette.color_count();
            (synthetic && palette_len > 0).then(|| {
                // The synthetic lane follows the highest visible source container color ordinal.
                model
                    .subgraphs
                    .iter()
                    .filter(|subgraph| model.collapsed_replacement(&subgraph.id).is_none())
                    .filter_map(|subgraph| model.subgraph_color_ordinal(&subgraph.id))
                    .max()
                    .map_or(0, |ordinal| ordinal.saturating_add(1))
                    % palette_len
            })
        })
}

/// Write the group, body and title shells, and title label into the operation sink.
/// The caller checks lane membership and performs its Emit checkpoint before calling this writer.
pub fn write_swimlane_cluster(
    out: &mut impl SvgOutput,
    input: SwimlaneClusterSvg<'_>,
) -> Result<()> {
    let color_slot = swimlane_color_slot(
        input.model,
        input.palette,
        input.diagram_type,
        &input.lane.id,
        input.subgraph.is_none(),
    );
    write_swimlane_cluster_with_label_ink(out, input, color_slot, None)
}

pub(crate) fn write_swimlane_cluster_with_label_ink(
    out: &mut impl SvgOutput,
    input: SwimlaneClusterSvg<'_>,
    color_slot: Option<usize>,
    label_ink: Option<&str>,
) -> Result<()> {
    let lane = input.lane;
    let model = input.model;
    let source = input.subgraph.zip(input.subgraph_index);
    let class_names = source
        .map(|(subgraph, index)| model.effective_subgraph_css(index, subgraph).0)
        .unwrap_or_default();
    let label_style = input.shape.terminal.compiled.label_style.trim();
    let label_style = if let Some(ink) = label_ink {
        let mut style = label_style.to_owned();
        append_native_label_color(&mut style, ink);
        Cow::Owned(style)
    } else {
        Cow::Borrowed(label_style)
    };
    let render_title = source.map_or(lane.title.as_str(), |(subgraph, index)| {
        model.subgraph_title_for_render(index, subgraph)
    });
    let direction = input.direction;
    let canonical = input.canonical;
    let geometry = swimlane_terminal_geometry(lane, direction, canonical);
    let label_width = lane.title_label_width.max(0.0);
    let label_height = lane.title_label_height.max(0.0);
    let title_owner = input
        .svg_label_sidecar
        .and_then(|sidecar| sidecar.swimlane_group_title_owner(input.cluster_id));
    let width = geometry.width;
    let (body, title) = geometry.frame_parts((lane.x, lane.y), direction, canonical);
    use SwimlaneDirection::{Lr, Rl};
    let horizontal = direction == Lr || (canonical && direction == Rl);

    let is_pool = input.is_pool;
    let label_class = if is_pool {
        "pool-label swimlane-label"
    } else {
        "swimlane-label"
    };
    let mut classes = String::from(if is_pool {
        "cluster pool swimlane"
    } else {
        "cluster swimlane"
    });
    for class in class_names {
        let class = class.trim();
        if !class.is_empty() {
            classes.push(' ');
            classes.push_str(class);
        }
    }
    let lane_dom_id = input
        .document_ids
        .lane(&lane.id)
        .ok_or_else(|| Error::InvalidModel {
            message: format!("missing prepared Swimlane DOM id for lane `{}`", lane.id),
        })?;
    let _ = write!(
        out,
        r#"<g class="{}" id="{}" data-id="{}" data-et="cluster""#,
        escape_attr_display(&classes),
        lane_dom_id,
        escape_attr_display(&lane.id),
    );
    let data_look = input.look.as_str();
    let _ = write!(out, r#" data-look="{}""#, escape_attr_display(data_look));
    if let Some(color_slot) = color_slot {
        let _ = write!(out, r#" data-color-id="color-{color_slot}""#);
    }
    out.push('>');

    write_swimlane_shell(
        out,
        [body, title],
        input.translation,
        input.group_origin,
        is_pool,
        input.shape.paint(),
        input.shape.hand_drawn(input.look, canonical),
    );

    let label_left = -label_width / 2.0;
    let label_top = -label_height / 2.0;
    let label_transform = |x, y| {
        if horizontal {
            let center_x = geometry.title_label.x + input.translation.x - input.group_origin.x;
            let center_y = geometry.title_label.y + input.translation.y - input.group_origin.y;
            format!(
                "translate({}, {}) rotate(-90) translate({}, {})",
                fmt_display(center_x),
                fmt_display(center_y),
                fmt_display(x),
                fmt_display(y),
            )
        } else {
            format!(
                "translate({}, {})",
                fmt_display(
                    geometry.title_label.x + x + input.translation.x - input.group_origin.x
                ),
                fmt_display(
                    geometry.title_label.y + y + input.translation.y - input.group_origin.y
                ),
            )
        }
    };
    if input.html_labels {
        let prepared_label = input
            .svg_label_sidecar
            .and_then(|sidecar| {
                sidecar
                    .swimlane_group_title_owner(input.cluster_id)
                    .map(|owner| sidecar.prepared_label_for_terminal(owner, render_title))
            })
            .unwrap_or_default();
        let title_html = flowchart_label_html_with_prepared_label(
            render_title,
            "markdown",
            input.config,
            input.math_renderer,
            prepared_label,
        );
        let transform = label_transform(label_left, label_top);
        let wrap_width = FLOWCHART_FIXED_LABEL_WRAP_WIDTH;
        let uses_wrapped_box = label_width >= wrap_width - 1e-3;
        let div_style = if uses_wrapped_box {
            format!(
                "display: table; white-space: break-spaces; line-height: 1.5; max-width: {mw}px; text-align: center; width: {mw}px;",
                mw = fmt_display(wrap_width),
            )
        } else {
            format!(
                "display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {}px; text-align: center;",
                fmt_display(width.max(label_width)),
            )
        };
        let overflow_attr = if label_width > 0.0 || label_height > 0.0 {
            HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR
        } else {
            ""
        };
        let _ = write!(
            out,
            r#"<g class="cluster-label {}" transform="{}"><foreignObject width="{}" height="{}"{}><div xmlns="http://www.w3.org/1999/xhtml" style="{}"><span class="nodeLabel"{}>{}</span></div></foreignObject></g>"#,
            label_class,
            escape_attr_display(&transform),
            fmt_display(label_width),
            fmt_display(label_height),
            overflow_attr,
            escape_attr_display(&div_style),
            OptionalStyleAttr(label_style.as_ref()),
            title_html,
        );
    } else {
        let title_text_style = input
            .svg_label_sidecar
            .zip(title_owner)
            .and_then(|(sidecar, owner)| sidecar.node_source_style(owner))
            .filter(|_| canonical)
            .unwrap_or(&input.shape.terminal.title_text_style)
            .as_ref();
        let svg_style = if canonical { label_style.as_ref() } else { "" };
        let svg_style =
            crate::svg_label::with_svg_font_fallback(svg_style, input.svg_font_fallback);
        let prepared = FlowchartSvgLabelRenderPlan::new(
            input.svg_label_sidecar,
            title_owner,
            render_title,
            input.measurer,
            title_text_style,
            Some(FLOWCHART_FIXED_LABEL_WRAP_WIDTH),
            true,
            FlowchartSvgWidthMode::Bbox,
        );
        let transform = if canonical {
            let plain_text = if prepared.is_prepared() {
                Cow::Borrowed(prepared.plain_text())
            } else {
                let markdown = render_title
                    .strip_prefix('`')
                    .and_then(|title| title.strip_suffix('`'))
                    .unwrap_or(render_title);
                Cow::Owned(mermaid_markdown_svg_plain_text(markdown))
            };
            let bbox_y = input
                .measurer
                .measure_svg_create_text_bbox_y_offset_px(&plain_text, title_text_style);
            label_transform(0.0, label_top - bbox_y)
        } else {
            label_transform(label_left, label_top)
        };
        let _ = write!(
            out,
            r#"<g class="cluster-label {}" transform="{}"><g><rect class="background" style="stroke: none"/>"#,
            label_class,
            escape_attr_display(&transform),
        );
        let zero_width_title = lane.width == 0.0;
        if zero_width_title {
            let rows = zero_width_lane_title_rows(render_title);
            let write_rows = if canonical {
                write_svg_markdown_word_lines_centered_with_style
            } else {
                write_svg_markdown_word_lines_with_style
            };
            write_rows(
                out,
                &rows,
                Some(svg_style.as_ref()),
                SvgTextEntityMode::DecodedModel,
            );
        } else if prepared.is_prepared() {
            if canonical {
                write_svg_text_source_word_lines_with_style(
                    out,
                    &prepared.wrapped_lines(),
                    &svg_style,
                    true,
                    None,
                );
            } else {
                write_flowchart_svg_label_plan(out, &prepared, true);
            }
        } else if canonical {
            write_svg_text_markdown_wrapped_from_create_text_source_with_style(
                out,
                render_title,
                &svg_style,
                input.measurer,
                title_text_style,
                Some(FLOWCHART_FIXED_LABEL_WRAP_WIDTH),
                true,
            );
        } else {
            write_flowchart_svg_text_markdown(out, render_title, true);
        }
        out.push_str("</g></g>");
    }
    out.push_str("</g>");
    Ok(())
}
