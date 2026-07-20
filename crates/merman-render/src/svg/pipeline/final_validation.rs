use crate::resources::RenderResourceLimits;
use crate::{Error, Result};
use quick_xml::XmlVersion;
use quick_xml::events::{BytesRef, BytesStart, Event};
use quick_xml::name::ResolveResult;
use quick_xml::reader::NsReader;

use super::builtin::attr_sanitize::{
    attribute_violates_resvg_contract, matches_active_svg_element,
};
use super::builtin::css_sanitize::{
    validate_resvg_css_declaration_list, validate_resvg_css_stylesheet,
};

const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";
const VALIDATION_PASS: &str = "validate-resvg-compatible-svg";

/// Validates the terminal XML contract consumed by usvg/resvg.
///
/// This check deliberately does not claim that an SVG is safe to insert into a browser DOM. DOM
/// embedding needs a separate browser-oriented policy for navigation, network access, and HTML
/// integration.
pub(crate) fn validate_resvg_compatible_svg(svg: &str, limits: RenderResourceLimits) -> Result<()> {
    let mut reader = NsReader::from_str(svg);
    let mut depth = 0usize;
    let mut max_tree_depth = 0usize;
    let mut elements = 0usize;
    let mut root_seen = false;
    let mut root_closed = false;
    let mut style_text = None::<String>;

    loop {
        let (namespace, event) = reader
            .read_resolved_event()
            .map_err(|error| validation_error(format!("invalid XML: {error}")))?;
        match event {
            Event::Start(element) => {
                elements = elements.saturating_add(1);
                if style_text.is_some() {
                    return Err(validation_error(
                        "a <style> element contains nested XML elements",
                    ));
                }
                let is_root = depth == 0;
                reject_additional_root(is_root, root_seen, root_closed)?;
                let is_style = validate_element(&element, namespace, is_root)?;
                if is_root {
                    root_seen = true;
                }
                depth += 1;
                max_tree_depth = max_tree_depth.max(depth.saturating_sub(1));
                if is_style {
                    style_text = Some(String::new());
                }
            }
            Event::Empty(element) => {
                elements = elements.saturating_add(1);
                if style_text.is_some() {
                    return Err(validation_error(
                        "a <style> element contains nested XML elements",
                    ));
                }
                let is_root = depth == 0;
                reject_additional_root(is_root, root_seen, root_closed)?;
                let is_style = validate_element(&element, namespace, is_root)?;
                if is_style {
                    validate_style_text("")?;
                }
                if is_root {
                    root_seen = true;
                    root_closed = true;
                } else {
                    max_tree_depth = max_tree_depth.max(depth);
                }
            }
            Event::End(element) => {
                reject_unknown_namespace(namespace)?;
                let local_name = element.local_name();
                let element_name = xml_name(local_name.as_ref())?;
                if let Some(css) = style_text.take() {
                    if !element_name.eq_ignore_ascii_case("style") {
                        return Err(validation_error(
                            "a <style> element contains nested XML elements",
                        ));
                    }
                    validate_style_text(&css)?;
                }
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| validation_error("an end tag has no matching start tag"))?;
                if depth == 0 {
                    root_closed = true;
                }
            }
            Event::Text(text) => {
                let text = text
                    .xml10_content()
                    .map_err(|error| validation_error(format!("invalid XML text: {error}")))?;
                if let Some(css) = style_text.as_mut() {
                    css.push_str(&text);
                } else if depth == 0 && !text.trim().is_empty() {
                    return Err(validation_error("text is not allowed outside the SVG root"));
                }
            }
            Event::CData(text) => {
                let text = text
                    .xml10_content()
                    .map_err(|error| validation_error(format!("invalid CDATA: {error}")))?;
                if let Some(css) = style_text.as_mut() {
                    css.push_str(&text);
                } else if depth == 0 && !text.trim().is_empty() {
                    return Err(validation_error(
                        "CDATA is not allowed outside the SVG root",
                    ));
                }
            }
            Event::GeneralRef(reference) => {
                let value = resolve_xml_reference(&reference)?;
                if let Some(css) = style_text.as_mut() {
                    css.push(value);
                } else if depth == 0 && !value.is_ascii_whitespace() {
                    return Err(validation_error(
                        "character references are not allowed outside the SVG root",
                    ));
                }
            }
            Event::PI(_) => {
                return Err(validation_error(
                    "processing instructions are not accepted by the resvg-safe contract",
                ));
            }
            Event::DocType(_) => {
                return Err(validation_error(
                    "document type declarations are not accepted by the resvg-safe contract",
                ));
            }
            Event::Decl(_) | Event::Comment(_) => {}
            Event::Eof => break,
        }
    }

    if !root_seen {
        return Err(validation_error(
            "the document does not contain an SVG root",
        ));
    }
    if !root_closed || depth != 0 || style_text.is_some() {
        return Err(validation_error("the SVG root is not closed"));
    }
    limits.check_svg_structure(elements, max_tree_depth)?;
    Ok(())
}

fn reject_additional_root(is_root: bool, root_seen: bool, root_closed: bool) -> Result<()> {
    if is_root && (root_seen || root_closed) {
        return Err(validation_error(
            "the document contains more than one root element",
        ));
    }
    Ok(())
}

fn validate_element(
    element: &BytesStart<'_>,
    namespace: ResolveResult<'_>,
    is_root: bool,
) -> Result<bool> {
    validate_namespace(namespace, is_root)?;
    let local_name = element.local_name();
    let element_name = xml_name(local_name.as_ref())?;
    if is_root && element_name != "svg" {
        return Err(validation_error("the document root is not an SVG element"));
    }
    if matches_active_svg_element(element_name) {
        return Err(validation_error(format!(
            "active element <{element_name}> survived terminal sanitization"
        )));
    }

    for attribute in element.attributes() {
        let attribute = attribute
            .map_err(|error| validation_error(format!("invalid XML attribute: {error}")))?;
        let name = xml_name(attribute.key.as_ref())?;
        let value = attribute
            .normalized_value(XmlVersion::Implicit1_0)
            .map_err(|error| validation_error(format!("invalid XML attribute value: {error}")))?;
        if attribute_violates_resvg_contract(name, &value) {
            return Err(validation_error(format!(
                "attribute {name:?} on <{element_name}> violates the resvg-safe contract"
            )));
        }
        if name.eq_ignore_ascii_case("style") {
            validate_resvg_css_declaration_list(&value).map_err(|error| {
                validation_error(format!(
                    "invalid style attribute on <{element_name}>: {error}"
                ))
            })?;
        }
    }

    Ok(element_name.eq_ignore_ascii_case("style"))
}

fn validate_namespace(namespace: ResolveResult<'_>, is_root: bool) -> Result<()> {
    match namespace {
        ResolveResult::Unknown(prefix) => Err(validation_error(format!(
            "element uses an unknown namespace prefix {:?}",
            String::from_utf8_lossy(&prefix)
        ))),
        ResolveResult::Bound(namespace)
            if is_root && namespace.as_ref() != SVG_NAMESPACE.as_bytes() =>
        {
            Err(validation_error(
                "the root element uses a non-SVG namespace",
            ))
        }
        ResolveResult::Unbound | ResolveResult::Bound(_) => Ok(()),
    }
}

fn reject_unknown_namespace(namespace: ResolveResult<'_>) -> Result<()> {
    match namespace {
        ResolveResult::Unknown(prefix) => Err(validation_error(format!(
            "element uses an unknown namespace prefix {:?}",
            String::from_utf8_lossy(&prefix)
        ))),
        ResolveResult::Unbound | ResolveResult::Bound(_) => Ok(()),
    }
}

fn validate_style_text(css: &str) -> Result<()> {
    validate_resvg_css_stylesheet(css)
        .map_err(|error| validation_error(format!("invalid <style> content: {error}")))
}

fn resolve_xml_reference(reference: &BytesRef<'_>) -> Result<char> {
    if let Some(value) = reference
        .resolve_char_ref()
        .map_err(|error| validation_error(format!("invalid character reference: {error}")))?
    {
        if is_legal_xml_1_0_char(value) {
            return Ok(value);
        }
        return Err(validation_error("illegal XML character reference"));
    }

    let name = reference
        .decode()
        .map_err(|error| validation_error(format!("invalid entity reference: {error}")))?;
    match name.as_ref() {
        "amp" => Ok('&'),
        "apos" => Ok('\''),
        "gt" => Ok('>'),
        "lt" => Ok('<'),
        "quot" => Ok('"'),
        _ => Err(validation_error(format!(
            "unknown XML entity reference &{name};"
        ))),
    }
}

fn is_legal_xml_1_0_char(value: char) -> bool {
    matches!(value, '\u{9}' | '\u{A}' | '\u{D}')
        || matches!(value, '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}')
}

fn xml_name(bytes: &[u8]) -> Result<&str> {
    std::str::from_utf8(bytes)
        .map_err(|error| validation_error(format!("invalid UTF-8 XML name: {error}")))
}

fn validation_error(message: impl Into<String>) -> Error {
    Error::svg_postprocess(VALIDATION_PASS, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn validate(svg: &str) -> Result<()> {
        validate_resvg_compatible_svg(svg, RenderResourceLimits::trusted_native())
    }

    #[test]
    fn accepts_structural_fragments_and_raster_data_images() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"><defs><linearGradient id="paint"/></defs><circle fill="url(#paint)" style="clip-path:url(#clip);content:&quot;45deg&quot;"/><image href="data:image/png;base64,AAAA"/></svg>"##;

        validate(svg).unwrap();
    }

    #[test]
    fn rejects_smil_and_other_active_elements() {
        for element in [
            "set",
            "animate",
            "animateMotion",
            "animateTransform",
            "discard",
            "mpath",
        ] {
            let svg = format!("<svg><{element}/></svg>");
            let error = validate(&svg).unwrap_err();
            assert!(error.to_string().contains("active element"), "{error}");
        }
    }

    #[test]
    fn rejects_dtd_processing_instructions_and_malformed_xml() {
        for svg in [
            "<!DOCTYPE svg><svg/>",
            "<?merman unsafe?><svg/>",
            "<svg><g></svg>",
            "<SVG/>",
            "<svg/><svg/>",
            "<svg><x:g/></svg>",
            "<svg>&unknown;</svg>",
        ] {
            assert!(validate(svg).is_err(), "{svg}");
        }
    }

    #[test]
    fn rejects_css_that_survived_sanitization() {
        for svg in [
            "<svg><style>@import url('a.css');</style></svg>",
            "<svg><style>.safe{}<!-- split -->.bad{animation:spin 1s}</style></svg>",
            "<svg><style><g/>.safe{fill:red}</style></svg>",
            "<svg><path style=\"animation:spin 1s\"/></svg>",
            "<svg><path style=\"fill:url(javascript:x)\"/></svg>",
            "<svg><path href=\"java&#x73;cript:alert(1)\"/></svg>",
            "<svg><path onclick=\"alert(1)\"/></svg>",
        ] {
            assert!(validate(svg).is_err(), "{svg}");
        }
    }

    #[test]
    fn rejects_svg_deeper_than_downstream_recursive_renderers_support() {
        let depth = crate::resources::MAX_RESVG_TREE_DEPTH + 1;
        let mut svg = String::from("<svg>");
        svg.push_str(&"<g>".repeat(depth));
        svg.push_str(&"</g>".repeat(depth));
        svg.push_str("</svg>");

        let error = validate_resvg_compatible_svg(
            &svg,
            RenderResourceLimits::unbounded_for_trusted_input(),
        )
        .unwrap_err();

        assert!(error.to_string().contains("max_svg_tree_depth"), "{error}");
    }

    #[test]
    fn rejects_svg_element_count_before_usvg_parsing() {
        let svg = "<svg><g/><g/></svg>";
        let limits = RenderResourceLimits {
            max_svg_elements: Some(2),
            ..RenderResourceLimits::unbounded_for_trusted_input()
        };

        let error = validate_resvg_compatible_svg(svg, limits).unwrap_err();

        assert!(error.to_string().contains("max_svg_elements"), "{error}");
    }

    #[test]
    fn does_not_claim_a_browser_dom_sanitizer_policy() {
        let svg = r#"<svg><a href="https://example.com"><text>link</text></a></svg>"#;

        validate(svg).unwrap();
    }
}
