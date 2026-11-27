use merman_model::presentation::PresentationConfig;

use crate::mermaid_style::HtmlTextGeometryDeclarations;
use crate::text::{
    WrapMode,
    create_text_html::{
        FlowchartLabelMetricsRequest, flowchart_label_html_source, flowchart_label_sanitize_config,
    },
    literal_label::{FlowchartSvgLabelSource, FlowchartSvgWidthMode},
};

/// Measures a raw createText label, including its optional host math and HTML projection.
/// The caller supplies whether its diagram family supports intrinsic raster image sizing.
pub fn measure_flowchart_label_for_layout(
    request: FlowchartLabelMetricsRequest<'_>,
    geometry: HtmlTextGeometryDeclarations,
    probe_intrinsic_images: bool,
) -> crate::text::TextMetrics {
    if request.raw_label.is_empty() {
        return crate::text::TextMetrics {
            width: 0.0,
            height: 0.0,
            line_count: 0,
        };
    }
    if let Some(metrics) = flowchart_math_label_metrics_for_layout(&request) {
        return metrics;
    }
    if request.wrap_mode == WrapMode::HtmlLike {
        let source =
            flowchart_label_html_source(request.raw_label, request.label_type, request.config);
        let sanitize_config = flowchart_label_sanitize_config(request.config);
        let xhtml = merman_markup::sanitize::sanitize_xhtml(&source, &sanitize_config);
        return super::measure_flowchart_html_label(
            request,
            &xhtml,
            geometry,
            probe_intrinsic_images,
        );
    }
    if request.label_type == "markdown" {
        crate::text::measure_wrapped_markdown_with_inline_styles(
            request.measurer,
            request.raw_label,
            request.style,
            request.max_width_px,
            request.wrap_mode,
        )
    } else {
        FlowchartSvgLabelSource::new(request.raw_label).metrics(
            request.measurer,
            request.style,
            request.max_width_px,
            FlowchartSvgWidthMode::Bbox,
        )
    }
}

fn flowchart_math_label_metrics_for_layout(
    request: &FlowchartLabelMetricsRequest<'_>,
) -> Option<crate::text::TextMetrics> {
    if request.wrap_mode != WrapMode::HtmlLike || !request.raw_label.contains("$$") {
        return None;
    }
    let math_renderer = request.math_renderer?;
    let resolved_config;
    let config = if request.config.get_str("securityLevel").is_none() {
        let mut values = request.config.as_config().clone();
        values.set_value("securityLevel", serde_json::json!("strict"));
        let parts = merman_model::presentation::FrozenPresentationConfigParts::new(values)
            .with_explicit_config_paths(std::sync::Arc::clone(
                request.config.shared_explicit_config_paths(),
            ));
        // Flowchart's standalone measurement defaults to strict without declaring an assignment.
        // Construction validates the derived values without charging the request's model limits.
        resolved_config = PresentationConfig::try_from_frozen_parts_controlled(
            parts,
            &merman_model::resources::InputResourcePolicy::for_profile(
                merman_model::resources::ResourceProfile::UnboundedForTrustedInput,
            ),
            &merman_model::OperationControl::new(),
        )
        .ok()?
        .ok()?;
        &resolved_config
    } else {
        request.config
    };
    crate::math::math_label_metrics_for_layout(crate::math::MathLabelMetricsRequest {
        measurer: request.measurer,
        raw_label: request.raw_label,
        style: request.style,
        max_width_px: request.max_width_px,
        wrap_mode: request.wrap_mode,
        config,
        math_renderer: Some(math_renderer),
    })
}
