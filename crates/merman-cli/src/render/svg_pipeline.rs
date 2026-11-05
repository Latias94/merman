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
    let Ok(document) = roxmltree::Document::parse(svg) else {
        return (None, None);
    };
    let root = document.root_element();
    let element_text = |tag| {
        let node = root.children().find(|node| {
            node.is_element()
                && node.tag_name().name() == tag
                && node
                    .tag_name()
                    .namespace()
                    .is_none_or(|namespace| namespace == "http://www.w3.org/2000/svg")
        })?;
        let text: String = node
            .descendants()
            .filter(|child| child.is_text())
            .filter_map(|child| child.text())
            .collect();
        let text = text.trim();
        (!text.is_empty()).then(|| text.to_owned())
    };
    (element_text("title"), element_text("desc"))
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

    #[cfg(feature = "markdown")]
    #[test]
    fn svg_metadata_decodes_xml_text_content_once() {
        let svg = concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg">"#,
            "<title>Title &#34;quoted&#34; &amp; &#x5B;one&#x5D;</title>",
            "<desc>Before <![CDATA[[raw] & literal]]><tspan> &#65;</tspan> &amp;lt;</desc>",
            "</svg>",
        );

        assert_eq!(
            svg_metadata(svg),
            (
                Some("Title \"quoted\" & [one]".to_string()),
                Some("Before [raw] & literal A &lt;".to_string()),
            )
        );
    }

    #[cfg(feature = "markdown")]
    #[test]
    fn svg_metadata_reads_namespaced_root_children() {
        let svg = concat!(
            r#"<svg:svg xmlns:svg="http://www.w3.org/2000/svg" xmlns:html="http://www.w3.org/1999/xhtml">"#,
            "<svg:title>SVG title</svg:title>",
            "<html:desc>HTML description</html:desc>",
            "<svg:g><svg:desc>Nested description</svg:desc></svg:g>",
            "<svg:desc>SVG description</svg:desc>",
            "</svg:svg>",
        );

        assert_eq!(
            svg_metadata(svg),
            (
                Some("SVG title".to_string()),
                Some("SVG description".to_string()),
            )
        );
    }

    #[cfg(feature = "markdown")]
    #[test]
    fn svg_metadata_ignores_comments_cdata_and_nested_descriptions() {
        let svg = concat!(
            "<!-- <title>Comment title</title><desc>Comment description</desc> -->",
            r#"<svg xmlns="http://www.w3.org/2000/svg">"#,
            "<title>Root title</title>",
            "<style><![CDATA[/* <desc>CSS description</desc> */]]></style>",
            "<g><desc>Nested description</desc></g>",
            "<desc>Root description</desc>",
            "</svg>",
        );

        assert_eq!(
            svg_metadata(svg),
            (
                Some("Root title".to_string()),
                Some("Root description".to_string()),
            )
        );
    }

    #[cfg(feature = "markdown")]
    #[test]
    fn svg_metadata_does_not_promote_descendant_accessibility_to_the_root() {
        let svg = concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg"><g>"#,
            "<title>Node title</title><desc>Node description</desc>",
            "</g></svg>",
        );

        assert_eq!(svg_metadata(svg), (None, None));
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
