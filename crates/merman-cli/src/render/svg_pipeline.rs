use crate::cli::SvgPipelineKind;
use merman::render::{RootBackgroundPostprocessor, ScopedCssPostprocessor, SvgPipeline};

pub(super) fn svg_postprocess_pipeline(
    mut pipeline: SvgPipeline,
    background: Option<&str>,
    css: Option<&str>,
) -> SvgPipeline {
    if let Some(background) = background {
        pipeline.push_postprocessor(RootBackgroundPostprocessor::new(background));
    }
    if let Some(css) = css {
        pipeline.push_postprocessor(ScopedCssPostprocessor::new(css));
    }
    pipeline
}

pub(super) fn svg_pipeline_from_kind(kind: SvgPipelineKind) -> SvgPipeline {
    match kind {
        SvgPipelineKind::Parity => SvgPipeline::parity(),
        SvgPipelineKind::Readable => SvgPipeline::readable(),
        SvgPipelineKind::ResvgSafe => SvgPipeline::resvg_safe(),
    }
}

pub(super) fn svg_metadata(svg: &str) -> (Option<String>, Option<String>) {
    (
        first_svg_element_text(svg, "title"),
        first_svg_element_text(svg, "desc"),
    )
}

fn first_svg_element_text(svg: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let start = svg.find(&open)?;
    let content_start = svg[start..].find('>')? + start + 1;
    let content_end = svg[content_start..].find(&close)? + content_start;
    let value = svg[content_start..content_end].trim();
    (!value.is_empty()).then(|| decode_basic_xml_entities(value))
}

fn decode_basic_xml_entities(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

pub(super) fn escape_xml_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
