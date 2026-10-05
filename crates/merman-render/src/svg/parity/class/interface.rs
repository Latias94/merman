use super::super::SvgDiagramId;
use super::super::{SvgOutput, escape_attr_display, escape_xml_into, fmt};
use super::ClassSvgInterface;
use super::bounds::include_xywh;
use super::context::ClassEmitCheckpoint;
use super::label::{class_html_div_style, class_math_html_label, class_node_label_style};
use super::node::ClassNodeRenderPosition;
use crate::entities::decode_entities_minimal_cow;
use crate::model::{Bounds, LayoutNode};
use crate::text::{TextMeasurer, TextStyle, WrapMode};

pub(super) struct ClassInterfaceRenderContext<'a> {
    pub diagram_id: SvgDiagramId<'a>,
    pub measurer: &'a dyn TextMeasurer,
    pub text_style: &'a TextStyle,
    pub use_html_labels: bool,
    pub wrapping_width: f64,
    pub look: &'a str,
    pub mermaid_config: Option<&'a merman_core::MermaidConfig>,
    pub math_renderer: Option<&'a (dyn crate::math::MathRenderer + Send + Sync)>,
    pub theme_expectation: &'a crate::class::ClassNodeTerminalExpectation,
    pub emit: ClassEmitCheckpoint<'a>,
}

pub(super) struct ClassInterfaceRenderState<'a, O: SvgOutput> {
    pub out: &'a mut O,
    pub content_bounds: &'a mut Option<Bounds>,
}

pub(super) struct ClassInterfaceRenderResult {
    pub theme_emission: crate::class::ClassNodeTerminalEmission,
    pub typography: crate::class::ClassTextTerminalFacts,
}

pub(super) fn render_class_interface_node<O: SvgOutput>(
    state: ClassInterfaceRenderState<'_, O>,
    iface: &ClassSvgInterface,
    layout_node: &LayoutNode,
    position: ClassNodeRenderPosition,
    ctx: &ClassInterfaceRenderContext<'_>,
) -> crate::Result<ClassInterfaceRenderResult> {
    let out = &mut *state.out;
    let content_bounds = &mut *state.content_bounds;

    let label_text = decode_entities_minimal_cow(iface.label.trim());
    let (fo_w_raw, fo_h_raw) = match (layout_node.label_width, layout_node.label_height) {
        (Some(w), Some(h)) => (w, h),
        _ => {
            let mode = if ctx.use_html_labels {
                WrapMode::HtmlLike
            } else {
                WrapMode::SvgLike
            };
            let metrics = ctx.measurer.measure_wrapped(
                &label_text,
                ctx.text_style,
                Some(ctx.wrapping_width),
                mode,
            );
            (metrics.width, metrics.height)
        }
    };
    let fo_w = fo_w_raw.max(1.0);
    let fo_h = fo_h_raw.max(1.0);

    let w = layout_node.width.max(1.0);
    let h = layout_node.height.max(1.0);
    let left = -w / 2.0;
    let top = -h / 2.0;
    let label_source_owned = crate::class::class_text_is_math_only(label_text.as_ref());
    let label_fill_verified = !crate::math::contains_delimited_math(label_text.as_ref());
    let emitted_label_fill = ctx.theme_expectation.typed_label_fill(label_source_owned);
    let container_style = "opacity:0; !important";
    let label_style = class_node_label_style("", emitted_label_fill.as_ref().map(|(_, css)| *css));
    let label_style_attr = if !label_style.is_empty() {
        format!(r#" style="{}""#, escape_attr_display(&label_style))
    } else {
        String::new()
    };

    include_xywh(
        content_bounds,
        position.node_bounds_tx + left,
        position.node_bounds_ty + top,
        w,
        h,
    );
    include_xywh(
        content_bounds,
        position.node_bounds_tx - if ctx.use_html_labels { fo_w / 2.0 } else { 0.0 },
        position.node_bounds_ty - fo_h / 2.0,
        fo_w,
        fo_h,
    );

    out.push_str(r#"<g class="node undefined" id=""#);
    let _ = write!(out, "{}", ctx.diagram_id);
    ctx.emit.checkpoint()?;
    let _ = write!(
        out,
        r#"-{}" data-look="{}" transform="translate({}, {})"><rect class="basic label-container" style="{}" x="{}" y="{}" width="{}" height="{}"/><g class="label" style="{}" transform="translate({}, {})"><rect/>"#,
        escape_attr_display(&iface.id),
        escape_attr_display(ctx.look),
        fmt(position.node_tx),
        fmt(position.node_ty),
        escape_attr_display(container_style),
        fmt(left),
        fmt(top),
        fmt(w),
        fmt(h),
        escape_attr_display(&label_style),
        fmt(if ctx.use_html_labels {
            -fo_w / 2.0
        } else {
            0.0
        }),
        fmt(-fo_h / 2.0),
    );
    let math_html = ctx
        .use_html_labels
        .then(|| class_math_html_label(label_text.as_ref(), ctx.mermaid_config, ctx.math_renderer))
        .flatten();
    if ctx.use_html_labels {
        let _ = write!(
            out,
            r#"<foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="{}"><span class="nodeLabel"{}>"#,
            fmt(fo_w),
            fmt(fo_h),
            class_html_div_style(fo_w, ctx.wrapping_width as i64),
            label_style_attr,
        );
        if let Some(math_html) = math_html.as_deref() {
            out.push_str(math_html);
        } else {
            out.push_str("<p>");
            for (idx, line) in label_text.split('\n').enumerate() {
                if idx > 0 {
                    out.push_str("<br />");
                }
                escape_xml_into(out, line);
            }
            out.push_str("</p>");
        }
        out.push_str("</span></div></foreignObject>");
    } else {
        let source = crate::graph_label::FlowchartSvgLabelSource::new(&label_text);
        let lines =
            source.wrapped_lines(ctx.measurer, ctx.text_style, Some(ctx.wrapping_width), true);
        out.push_str(r#"<g><rect class="background" style="stroke: none"/>"#);
        super::super::label::write_svg_text_source_word_lines(out, &lines, true, false);
        out.push_str("</g>");
    }
    out.push_str("</g></g>");
    out.checkpoint()?;
    Ok(ClassInterfaceRenderResult {
        theme_emission: crate::class::ClassNodeTerminalEmission::new(
            &iface.id,
            crate::class::ClassNodePaintTerminalEmission::not_applicable(),
            crate::class::ClassNodePaintTerminalEmission::not_applicable(),
            crate::class::ClassNodePaintTerminalEmission::new(
                label_source_owned,
                emitted_label_fill,
                &label_style,
            )
            .with_terminal_verified(label_fill_verified),
        ),
        typography: (if math_html.is_some() {
            crate::class::ClassTextTerminalFacts::unverified_text(label_text.as_ref())
        } else {
            crate::class::ClassTextTerminalFacts::inherited_text(label_text.as_ref())
        })
        .with_paint(emitted_label_fill, &label_style),
    })
}
