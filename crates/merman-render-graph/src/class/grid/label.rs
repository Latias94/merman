//! Class relation text promoted to a plain Mermaid Grid `labelRect`.

use merman_model::{OperationPhase, presentation::PresentationConfig};
use merman_render_core::{
    Result,
    graph_layout::Size,
    layout_work::OperationLayoutWorkControl,
    math::{MathRenderer, mark_math_html_native_unavailable, render_math_html_label},
    resources::{OperationWorkMeter, PreparedTextRetainedReservation},
    text::{
        TextMeasurer, WrapMode,
        create_text_html::FlowchartLabelMetricsRequest,
        html_label_layout::measure_flowchart_html_label,
        literal_label::{FlowchartSvgLabelSource, FlowchartSvgWidthMode},
    },
};

use super::super::{ClassRenderConfig, class_math_label_metrics, class_plain_html_fragment};

#[derive(Debug)]
pub(in crate::class) struct ClassGridLabel {
    pub size: Size,
    pub body: ClassGridLabelBody,
    _reservation: Option<PreparedTextRetainedReservation>,
}

#[derive(Debug)]
pub(in crate::class) enum ClassGridLabelBody {
    Svg {
        rows: Vec<Vec<String>>,
        bbox_top: f64,
    },
    Html {
        fragment: String,
        wrapping_width: f64,
    },
}

#[allow(
    clippy::too_many_arguments,
    reason = "Keep the Class label on the operation's measured text path."
)]
pub(in crate::class) fn prepare_label_rect(
    raw_label: &str,
    id: &str,
    config: &PresentationConfig,
    render: &ClassRenderConfig,
    measurer: &dyn TextMeasurer,
    math_renderer: Option<&(dyn MathRenderer + Send + Sync)>,
    work: &OperationLayoutWorkControl,
    meter: &OperationWorkMeter,
) -> Result<ClassGridLabel> {
    meter.charge(raw_label.len().max(1))?;
    let style = render.text_style();
    let wrapping_width = render.interface_wrapping_width;
    let width = Some(wrapping_width);
    // `labelHelper` uses root htmlLabels and flowchart.wrappingWidth. Grid did not copy
    // the old relation's markdown labelType or its edge-label box padding.
    let (metrics, body, bytes) = if render.diagram_use_html_labels {
        let math = render_math_html_label(raw_label, config, math_renderer)
            .map(mark_math_html_native_unavailable);
        let math_metrics = math.as_ref().and_then(|_| {
            class_math_label_metrics(raw_label, measurer, style, width, config, math_renderer)
        });
        let fragment = math.unwrap_or_else(|| class_plain_html_fragment(raw_label, config));
        meter.charge(fragment.len().max(1))?;
        let metrics = math_metrics.unwrap_or_else(|| {
            measure_flowchart_html_label(
                FlowchartLabelMetricsRequest {
                    measurer,
                    raw_label,
                    label_type: "text",
                    style,
                    max_width_px: width,
                    wrap_mode: WrapMode::HtmlLike,
                    config,
                    math_renderer,
                },
                &fragment,
                Default::default(),
                false,
            )
        });
        let bytes = fragment.capacity();
        (
            metrics,
            ClassGridLabelBody::Html {
                fragment,
                wrapping_width,
            },
            bytes,
        )
    } else {
        let source = FlowchartSvgLabelSource::new(raw_label);
        let rows = source.wrapped_lines(measurer, style, width, true);
        work.checkpoint()?;
        let metrics =
            source.metrics_from_wrapped(measurer, style, &rows, FlowchartSvgWidthMode::Bbox);
        work.checkpoint()?;
        let bbox_top =
            measurer.measure_svg_create_text_bbox_y_offset_px(source.plain_text(), style);
        let bytes = rows.iter().fold(
            rows.capacity()
                .saturating_mul(std::mem::size_of::<Vec<String>>()),
            |bytes, row| {
                row.iter().fold(
                    bytes.saturating_add(
                        row.capacity().saturating_mul(std::mem::size_of::<String>()),
                    ),
                    |bytes, word| bytes.saturating_add(word.capacity()),
                )
            },
        );
        (metrics, ClassGridLabelBody::Svg { rows, bbox_top }, bytes)
    };
    meter.checkpoint(OperationPhase::Layout)?;
    let bytes = bytes
        .saturating_add(id.len())
        .saturating_add(std::mem::size_of::<(String, ClassGridLabel)>())
        .saturating_add(std::mem::size_of::<usize>());
    let reservation = work.reserve_prepared_text_retained_bytes(bytes)?;
    Ok(ClassGridLabel {
        // Shape measurement falls back to the invisible 0.1px anchor for empty ink.
        size: Size {
            width: metrics.width.max(0.1),
            height: metrics.height.max(0.1),
        },
        body,
        _reservation: Some(reservation),
    })
}
