use super::ClassSvgInterface;
use super::SvgDiagramId;
use super::bounds::include_xywh;
use super::context::ClassEmitCheckpoint;
use super::label::{class_html_div_style, class_math_html_label};
use super::node::ClassNodeRenderPosition;
use super::{SvgOutput, escape_attr_display, escape_xml_into, fmt};
use merman_render_core::entities::decode_entities_minimal_cow;
use merman_render_core::html::html_label_overflow_attr;
use merman_render_core::text::{TextMeasurer, TextStyle, WrapMode};
use merman_render_core::{Bounds, LayoutNode};

pub(super) struct ClassInterfaceRenderContext<'a> {
    pub diagram_id: SvgDiagramId<'a>,
    pub measurer: &'a dyn TextMeasurer,
    pub text_style: &'a TextStyle,
    pub use_html_labels: bool,
    pub uses_canonical_routes: bool,
    pub wrapping_width: f64,
    pub look: &'a str,
    pub mermaid_config: Option<&'a merman_model::presentation::PresentationConfig>,
    pub math_renderer: Option<&'a (dyn merman_render_core::math::MathRenderer + Send + Sync)>,
    pub emit: ClassEmitCheckpoint<'a>,
}

pub(super) struct ClassInterfaceRenderState<'a, O: SvgOutput> {
    pub out: &'a mut O,
    pub content_bounds: &'a mut Option<Bounds>,
}

pub(super) fn render_class_interface_node<O: SvgOutput>(
    state: ClassInterfaceRenderState<'_, O>,
    iface: ClassSvgInterface<'_>,
    layout_node: &LayoutNode,
    position: ClassNodeRenderPosition,
    ctx: &ClassInterfaceRenderContext<'_>,
) -> merman_render_core::Result<()> {
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
    let container_style = "opacity:0; !important";

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
        r#"-{}" data-look="{}" transform="translate({}, {})"><rect class="basic label-container" style="{}" x="{}" y="{}" width="{}" height="{}"/><g class="label" style="" transform="translate({}, {})"><rect/>"#,
        escape_attr_display(&iface.id),
        escape_attr_display(ctx.look),
        fmt(position.node_tx),
        fmt(position.node_ty),
        escape_attr_display(container_style),
        fmt(left),
        fmt(top),
        fmt(w),
        fmt(h),
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
            r#"<foreignObject width="{}" height="{}"{}><div xmlns="http://www.w3.org/1999/xhtml" style="{}"><span class="nodeLabel">"#,
            fmt(fo_w),
            fmt(fo_h),
            html_label_overflow_attr(ctx.uses_canonical_routes, fo_w, fo_h),
            class_html_div_style(fo_w, ctx.wrapping_width as i64),
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
        let source =
            merman_render_core::text::literal_label::FlowchartSvgLabelSource::new(&label_text);
        let lines =
            source.wrapped_lines(ctx.measurer, ctx.text_style, Some(ctx.wrapping_width), true);
        out.push_str(r#"<g><rect class="background" style="stroke: none"/>"#);
        merman_render_core::text::svg_label::write_svg_text_source_word_lines_with_style(
            out, &lines, "", false, None,
        );
        out.push_str("</g>");
    }
    out.push_str("</g></g>");
    out.checkpoint()
}
