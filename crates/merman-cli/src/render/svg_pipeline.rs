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

#[cfg(feature = "markdown")]
pub(super) fn svg_metadata(svg: &str) -> (Option<String>, Option<String>) {
    (
        first_svg_element_text(svg, "title"),
        first_svg_element_text(svg, "desc"),
    )
}

#[cfg(feature = "markdown")]
fn first_svg_element_text(svg: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let start = svg.find(&open)?;
    let content_start = svg[start..].find('>')? + start + 1;
    let content_end = svg[content_start..].find(&close)? + content_start;
    let value = svg[content_start..content_end].trim();
    (!value.is_empty()).then(|| decode_basic_xml_entities(value))
}

#[cfg(feature = "markdown")]
fn decode_basic_xml_entities(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_render::environment::RenderEnvironment;

    const SVG_WITH_BROWSER_ONLY_CONTENT: &str = r#"<svg id="diagram" xmlns="http://www.w3.org/2000/svg"><style>@keyframes bad { to { opacity: .5; } } .node { animation: bad 1s; }</style><foreignObject width="40" height="20"><div xmlns="http://www.w3.org/1999/xhtml"><p>Raw</p></div></foreignObject><rect class="node" width="10px" height="12px" stroke=""/></svg>"#;

    fn process(kind: SvgPipelineKind) -> String {
        let pipeline =
            svg_output_policy(kind, Some("#f8fafc"), Some(".node { fill: red; }")).pipeline();
        let session = RenderEnvironment::deterministic()
            .begin_session()
            .expect("deterministic render session");
        pipeline
            .process_to_string(SVG_WITH_BROWSER_ONLY_CONTENT, &session)
            .expect("pipeline output")
    }

    fn assert_root_background(svg: &str, expected: &str) {
        let document = roxmltree::Document::parse(svg).expect("valid SVG XML");
        let style = document
            .root_element()
            .attribute("style")
            .expect("root style attribute");
        assert!(
            style.split(';').map(str::trim).any(|declaration| {
                declaration
                    .split_once(':')
                    .is_some_and(|(property, value)| {
                        property.trim() == "background-color" && value.trim() == expected
                    })
            }),
            "expected root background {expected:?}, got {style:?}"
        );
    }

    #[test]
    fn parity_keeps_browser_content_before_cli_postprocessors() {
        let output = process(SvgPipelineKind::Parity);

        assert!(output.contains("<foreignObject"));
        assert_root_background(&output, "#f8fafc");
        assert_eq!(
            output
                .matches(r#"data-merman-postprocess="scoped-css""#)
                .count(),
            1
        );
    }

    #[test]
    fn resvg_safe_materializes_fallbacks_before_cli_postprocessors() {
        let output = process(SvgPipelineKind::ResvgSafe);

        assert!(!output.contains("<foreignObject"));
        assert!(!output.contains("@keyframes bad"));
        assert!(!output.contains("animation: bad"));
        assert!(output.contains(r#"data-merman-foreignobject="fallback""#));
        assert_root_background(&output, "#f8fafc");
        assert_eq!(
            output
                .matches(r#"data-merman-postprocess="scoped-css""#)
                .count(),
            1
        );
    }
}
