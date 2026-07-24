use crate::cli::SvgPipelineKind;
use merman::svg::{SvgOutputPolicy, SvgPipelinePreset};

pub(super) fn svg_output_policy(
    kind: SvgPipelineKind,
    background: Option<&str>,
    css: Option<&str>,
) -> SvgOutputPolicy {
    SvgOutputPolicy {
        preset: match kind {
            SvgPipelineKind::Parity => SvgPipelinePreset::Parity,
            SvgPipelineKind::Readable => SvgPipelinePreset::Readable,
            SvgPipelineKind::ResvgSafe => SvgPipelinePreset::ResvgSafe,
        },
        root_background_color: background.map(str::to_owned),
        scoped_css: css.map(str::to_owned),
        ..SvgOutputPolicy::default()
    }
}

#[cfg(feature = "analysis")]
pub(super) fn svg_metadata(svg: &str) -> (Option<String>, Option<String>) {
    (
        first_svg_element_text(svg, "title"),
        first_svg_element_text(svg, "desc"),
    )
}

#[cfg(feature = "analysis")]
fn first_svg_element_text(svg: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let start = svg.find(&open)?;
    let content_start = svg[start..].find('>')? + start + 1;
    let content_end = svg[content_start..].find(&close)? + content_start;
    let value = svg[content_start..content_end].trim();
    (!value.is_empty()).then(|| decode_basic_xml_entities(value))
}

#[cfg(feature = "analysis")]
fn decode_basic_xml_entities(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}
