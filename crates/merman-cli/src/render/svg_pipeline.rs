use crate::cli::SvgPipelineKind;
use merman::svg::{SvgGeneration, SvgOutputPolicy, SvgPipeline, SvgPipelinePreset};

pub(super) fn svg_output_pipeline(
    kind: SvgPipelineKind,
    background: Option<&str>,
    css: Option<&str>,
) -> SvgPipeline {
    let pipeline = SvgOutputPolicy {
        preset: match kind {
            SvgPipelineKind::Parity | SvgPipelineKind::Static => SvgPipelinePreset::Parity,
            SvgPipelineKind::Readable => SvgPipelinePreset::Readable,
            SvgPipelineKind::ResvgSafe => SvgPipelinePreset::ResvgSafe,
        },
        root_background_color: background.map(str::to_owned),
        scoped_css: css.map(str::to_owned),
        ..SvgOutputPolicy::default()
    }
    .pipeline();
    if matches!(kind, SvgPipelineKind::Static) {
        pipeline.with_generation(SvgGeneration::Static)
    } else {
        pipeline
    }
}

#[cfg(feature = "markdown")]
pub(super) fn svg_metadata_for_document(
    svg: &str,
    writes_document: bool,
) -> (Option<String>, Option<String>) {
    if writes_document {
        svg_metadata(svg)
    } else {
        (None, None)
    }
}

#[cfg(feature = "markdown")]
pub(super) fn svg_metadata(svg: &str) -> (Option<String>, Option<String>) {
    let [title, desc] = read_svg_metadata(svg).unwrap_or_default();
    (title, desc)
}

/// Projects the first root title and description from the completed render pipeline output.
#[cfg(feature = "markdown")]
fn read_svg_metadata(svg: &str) -> Option<[Option<String>; 2]> {
    use quick_xml::events::Event;
    use quick_xml::name::Namespace;

    let mut reader = quick_xml::NsReader::from_str(svg);
    reader.config_mut().enable_all_checks(true);
    let mut metadata = [None, None];
    let mut active = None;
    let mut content = String::new();
    let mut depth = 0usize;
    let mut root_seen = false;

    loop {
        let (namespace, event) = reader.read_resolved_event().ok()?;
        let opens = matches!(&event, Event::Start(_));
        match event {
            Event::Start(element) | Event::Empty(element) => {
                let namespace = Option::<Namespace<'_>>::try_from(namespace).ok()?;
                if depth == 0 && std::mem::replace(&mut root_seen, true) {
                    return None;
                }
                if depth == 1 && namespace.is_none_or(|ns| ns.0 == "http://www.w3.org/2000/svg") {
                    let slot = match element.local_name().as_ref() {
                        "title" => Some(0),
                        "desc" => Some(1),
                        _ => None,
                    };
                    if let Some(slot) = slot.filter(|&slot| metadata[slot].is_none()) {
                        if opens {
                            active = Some(slot);
                        } else {
                            metadata[slot] = Some(String::new());
                        }
                    }
                }
                depth += usize::from(opens);
            }
            Event::End(_) => {
                depth = depth.checked_sub(1)?;
                if depth == 1
                    && let Some(slot) = active.take()
                {
                    metadata[slot] = Some(std::mem::take(&mut content));
                }
            }
            Event::Text(text) if active.is_some() => content.push_str(&text.xml10_content()),
            Event::CData(text) if active.is_some() => content.push_str(&text.xml10_content()),
            Event::GeneralRef(reference) if active.is_some() => {
                if let Some(character) = reference.resolve_char_ref().ok()? {
                    content.push(character);
                } else {
                    content.push_str(quick_xml::escape::resolve_xml_entity(reference.as_ref())?);
                }
            }
            Event::Text(text) if depth == 0 => {
                if !text
                    .as_ref()
                    .as_bytes()
                    .iter()
                    .all(|byte| matches!(byte, b' ' | b'\t' | b'\n' | b'\r'))
                {
                    return None;
                }
            }
            Event::CData(_) | Event::GeneralRef(_) if depth == 0 => return None,
            Event::DocType(_) => return None,
            Event::Eof => break,
            _ => {}
        }
    }

    if !root_seen || depth != 0 {
        return None;
    }
    Some(metadata.map(|text| {
        let text = text?;
        let trimmed = text.trim();
        if trimmed.is_empty() {
            None
        } else if trimmed.len() == text.len() {
            Some(text)
        } else {
            Some(trimmed.to_owned())
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_render::environment::RenderEnvironment;

    const SVG_WITH_BROWSER_ONLY_CONTENT: &str = r#"<svg id="diagram" xmlns="http://www.w3.org/2000/svg"><style>@keyframes bad { to { opacity: .5; } } .node { animation: bad 1s; }</style><foreignObject width="40" height="20"><div xmlns="http://www.w3.org/1999/xhtml"><p>Raw</p></div></foreignObject><rect class="node" width="10px" height="12px" stroke=""/></svg>"#;

    fn process(kind: SvgPipelineKind) -> String {
        let pipeline = svg_output_pipeline(kind, Some("#f8fafc"), Some(".node { fill: red; }"));
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

        assert_eq!(svg_metadata_for_document(svg, false), (None, None));
        assert_eq!(
            svg_metadata_for_document(svg, true),
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

    #[cfg(feature = "markdown")]
    #[test]
    fn svg_metadata_respects_empty_first_fields_and_complete_xml() {
        assert_eq!(
            svg_metadata("<svg><title/><title>Later</title><desc> \r\nOne\rTwo </desc></svg>"),
            (None, Some("One\nTwo".into()))
        );
        for invalid in [
            "<svg><title>Title</title>",
            "<svg><title>Title</title><g></svg>",
            "<svg><title>Title</title></svg><other/>",
            "<!DOCTYPE svg [<!ENTITY name 'Title'>]><svg><title>&name;</title></svg>",
        ] {
            assert_eq!(svg_metadata(invalid), (None, None), "{invalid}");
        }
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
