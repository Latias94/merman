use crate::resources::RenderResourcePolicy;
#[cfg(test)]
use crate::resources::ResourceLimitId;
use crate::{Error, Result};
use quick_xml::XmlVersion;
use quick_xml::events::{BytesDecl, BytesRef, BytesStart, Event};
use quick_xml::name::{NamespaceResolver, ResolveResult};
use quick_xml::reader::NsReader;

use super::builtin::attr_sanitize::{
    attribute_violates_resvg_contract, matches_active_svg_element,
};
use super::builtin::css_sanitize::{
    validate_resvg_css_declaration_list, validate_resvg_css_stylesheet,
};

const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";
const VALIDATION_PASS: &str = "validate-resvg-compatible-svg";
const XML_VALIDATION_PASS: &str = "validate-well-formed-svg";

/// Proves the terminal contract shared by every public SVG output profile.
pub(crate) fn validate_well_formed_svg(svg: &str, limits: RenderResourcePolicy) -> Result<()> {
    let mut reader = NsReader::from_str(svg);
    reader.config_mut().enable_all_checks(true);
    let mut depth = 0usize;
    let mut elements = 0usize;
    let mut max_tree_depth = 0usize;
    let mut root_seen = false;
    let mut root_closed = false;
    let mut document_started = false;

    loop {
        let event = reader
            .read_event()
            .map_err(|error| xml_validation_error(format!("invalid XML: {error}")))?;
        match event {
            Event::Start(element) => {
                document_started = true;
                let is_root = depth == 0;
                if is_root && (root_seen || root_closed) {
                    return Err(xml_validation_error(
                        "the document contains more than one root element",
                    ));
                }
                let (namespace, _) = reader.resolver().resolve_element(element.name());
                validate_well_formed_element(&element, namespace, reader.resolver(), is_root)?;
                if is_root {
                    root_seen = true;
                }
                elements = elements.saturating_add(1);
                depth += 1;
                max_tree_depth = max_tree_depth.max(depth.saturating_sub(1));
                limits.check_svg_structure(elements, max_tree_depth)?;
            }
            Event::Empty(element) => {
                document_started = true;
                let is_root = depth == 0;
                if is_root && (root_seen || root_closed) {
                    return Err(xml_validation_error(
                        "the document contains more than one root element",
                    ));
                }
                let (namespace, _) = reader.resolver().resolve_element(element.name());
                validate_well_formed_element(&element, namespace, reader.resolver(), is_root)?;
                elements = elements.saturating_add(1);
                max_tree_depth = max_tree_depth.max(depth);
                limits.check_svg_structure(elements, max_tree_depth)?;
                if is_root {
                    root_seen = true;
                    root_closed = true;
                }
            }
            Event::End(_) => {
                document_started = true;
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| xml_validation_error("an end tag has no matching start tag"))?;
                if depth == 0 {
                    root_closed = true;
                }
            }
            Event::Text(text) => {
                document_started = true;
                if text.as_ref().windows(3).any(|window| window == b"]]>") {
                    return Err(xml_validation_error(
                        "the sequence ]]> is not allowed in XML text",
                    ));
                }
                let text = text
                    .xml10_content()
                    .map_err(|error| xml_validation_error(format!("invalid XML text: {error}")))?;
                if depth == 0 && !text.trim().is_empty() {
                    return Err(xml_validation_error(
                        "text is not allowed outside the SVG root",
                    ));
                }
            }
            Event::CData(text) => {
                document_started = true;
                text.xml10_content()
                    .map_err(|error| xml_validation_error(format!("invalid CDATA: {error}")))?;
                if depth == 0 {
                    return Err(xml_validation_error(
                        "CDATA is not allowed outside the SVG root",
                    ));
                }
            }
            Event::GeneralRef(reference) => {
                document_started = true;
                let value =
                    resolve_xml_reference_value(&reference).map_err(xml_validation_error)?;
                if depth == 0 {
                    return Err(xml_validation_error(
                        "character references are not allowed outside the SVG root",
                    ));
                }
                let _ = value;
            }
            Event::PI(_) => {
                return Err(xml_validation_error(
                    "processing instructions are not accepted in terminal SVG",
                ));
            }
            Event::DocType(_) => {
                return Err(xml_validation_error(
                    "document type declarations are not accepted in terminal SVG",
                ));
            }
            Event::Decl(declaration) => {
                if document_started || depth != 0 || root_seen {
                    return Err(xml_validation_error(
                        "the XML declaration must be the first document token",
                    ));
                }
                validate_xml_declaration(&declaration)?;
                document_started = true;
            }
            Event::Comment(comment) => {
                comment.xml10_content().map_err(|error| {
                    xml_validation_error(format!("invalid XML comment: {error}"))
                })?;
                document_started = true;
            }
            Event::Eof => break,
        }
    }

    if !root_seen {
        return Err(xml_validation_error(
            "the document does not contain an SVG root",
        ));
    }
    if !root_closed || depth != 0 {
        return Err(xml_validation_error("the SVG root is not closed"));
    }

    Ok(())
}

fn validate_well_formed_element(
    element: &BytesStart<'_>,
    namespace: ResolveResult<'_>,
    resolver: &NamespaceResolver,
    is_root: bool,
) -> Result<()> {
    match namespace {
        ResolveResult::Unknown(prefix) => {
            return Err(xml_validation_error(format!(
                "element uses an unknown namespace prefix {:?}",
                String::from_utf8_lossy(&prefix)
            )));
        }
        ResolveResult::Bound(namespace)
            if is_root && namespace.as_ref() != SVG_NAMESPACE.as_bytes() =>
        {
            return Err(xml_validation_error(
                "the root element uses a non-SVG namespace",
            ));
        }
        ResolveResult::Unbound | ResolveResult::Bound(_) => {}
    }

    validate_xml_qname(element.name().as_ref())?;
    let local_name = element.local_name();
    let element_name = std::str::from_utf8(local_name.as_ref())
        .map_err(|error| xml_validation_error(format!("invalid UTF-8 XML name: {error}")))?;
    if is_root && element_name != "svg" {
        return Err(xml_validation_error(
            "the document root is not an SVG element",
        ));
    }
    let mut first_namespaced_attribute = None;
    let mut additional_namespaced_attributes = Vec::new();
    for attribute in element.attributes() {
        let attribute = attribute
            .map_err(|error| xml_validation_error(format!("invalid XML attribute: {error}")))?;
        validate_xml_qname(attribute.key.as_ref())?;
        if attribute.value.as_ref().contains(&b'<') {
            return Err(xml_validation_error(
                "the character < is not allowed in an XML attribute value",
            ));
        }
        attribute
            .normalized_value(XmlVersion::Implicit1_0)
            .map_err(|error| {
                xml_validation_error(format!("invalid XML attribute value: {error}"))
            })?;

        if attribute.key.as_namespace_binding().is_some() || attribute.key.prefix().is_none() {
            continue;
        }
        let (namespace, local_name) = resolver.resolve_attribute(attribute.key);
        let namespace = match namespace {
            ResolveResult::Unknown(prefix) => {
                return Err(xml_validation_error(format!(
                    "attribute uses an unknown namespace prefix {:?}",
                    String::from_utf8_lossy(&prefix)
                )));
            }
            ResolveResult::Bound(namespace) => Some(namespace.into_inner()),
            ResolveResult::Unbound => None,
        };
        let expanded_name = (namespace, local_name.into_inner());
        if first_namespaced_attribute == Some(expanded_name)
            || additional_namespaced_attributes.contains(&expanded_name)
        {
            return Err(xml_validation_error(
                "attributes must have unique expanded names",
            ));
        }
        if first_namespaced_attribute.is_none() {
            first_namespaced_attribute = Some(expanded_name);
        } else {
            additional_namespaced_attributes.push(expanded_name);
        }
    }
    Ok(())
}

fn validate_xml_declaration(declaration: &BytesDecl<'_>) -> Result<()> {
    let declaration = std::str::from_utf8(declaration.as_ref())
        .map_err(|error| xml_validation_error(format!("invalid UTF-8 XML declaration: {error}")))?;
    let declaration = BytesStart::from_content(declaration, 3);
    let mut attributes = declaration.attributes();

    let version = attributes
        .next()
        .transpose()
        .map_err(|error| xml_validation_error(format!("invalid XML declaration: {error}")))?
        .ok_or_else(|| xml_validation_error("the XML declaration is missing version"))?;
    if version.key.as_ref() != b"version" || version.value.as_ref() != b"1.0" {
        return Err(xml_validation_error(
            "the XML declaration must begin with version=\"1.0\"",
        ));
    }

    let mut expected_attribute = 1usize;
    for attribute in attributes {
        let attribute = attribute
            .map_err(|error| xml_validation_error(format!("invalid XML declaration: {error}")))?;
        match (
            expected_attribute,
            attribute.key.as_ref(),
            attribute.value.as_ref(),
        ) {
            (1, b"encoding", value) if value.eq_ignore_ascii_case(b"utf-8") => {
                expected_attribute = 2;
            }
            (1 | 2, b"standalone", b"yes" | b"no") => {
                expected_attribute = 3;
            }
            _ => {
                return Err(xml_validation_error(
                    "the XML declaration contains unsupported or out-of-order attributes",
                ));
            }
        }
    }
    Ok(())
}

fn validate_xml_qname(name: &[u8]) -> Result<()> {
    let name = std::str::from_utf8(name)
        .map_err(|error| xml_validation_error(format!("invalid UTF-8 XML name: {error}")))?;
    let mut components = name.split(':');
    let first = components.next().unwrap_or_default();
    let second = components.next();
    if components.next().is_some()
        || !is_valid_xml_ncname(first)
        || second.is_some_and(|component| !is_valid_xml_ncname(component))
    {
        return Err(xml_validation_error(format!(
            "invalid XML qualified name {name:?}"
        )));
    }
    Ok(())
}

fn is_valid_xml_ncname(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(is_xml_name_start_char)
        && chars.all(|ch| ch != ':' && is_xml_name_char(ch))
}

fn is_xml_name_start_char(ch: char) -> bool {
    matches!(
        ch,
        'A'..='Z'
            | '_'
            | 'a'..='z'
            | '\u{c0}'..='\u{d6}'
            | '\u{d8}'..='\u{f6}'
            | '\u{f8}'..='\u{2ff}'
            | '\u{370}'..='\u{37d}'
            | '\u{37f}'..='\u{1fff}'
            | '\u{200c}'..='\u{200d}'
            | '\u{2070}'..='\u{218f}'
            | '\u{2c00}'..='\u{2fef}'
            | '\u{3001}'..='\u{d7ff}'
            | '\u{f900}'..='\u{fdcf}'
            | '\u{fdf0}'..='\u{fffd}'
            | '\u{10000}'..='\u{effff}'
    )
}

fn is_xml_name_char(ch: char) -> bool {
    is_xml_name_start_char(ch)
        || matches!(
            ch,
            '-' | '.' | '0'..='9' | '\u{b7}' | '\u{300}'..='\u{36f}' | '\u{203f}'..='\u{2040}'
        )
}

/// Validates the terminal XML contract consumed by usvg/resvg.
///
/// This check deliberately does not claim that an SVG is safe to insert into a browser DOM. DOM
/// embedding needs a separate browser-oriented policy for navigation, network access, and HTML
/// integration.
pub(crate) fn validate_resvg_compatible_svg(svg: &str, limits: RenderResourcePolicy) -> Result<()> {
    validate_well_formed_svg(svg, limits)?;
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
                let value = resolve_xml_reference_value(&reference).map_err(validation_error)?;
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

fn resolve_xml_reference_value(reference: &BytesRef<'_>) -> std::result::Result<char, String> {
    if let Some(value) = reference
        .resolve_char_ref()
        .map_err(|error| format!("invalid XML character reference: {error}"))?
    {
        if crate::xml::is_xml_1_0_char(value) {
            return Ok(value);
        }
        return Err("invalid XML character reference: the scalar is forbidden in XML 1.0".into());
    }

    let name = reference
        .decode()
        .map_err(|error| format!("invalid XML entity reference: {error}"))?;
    match name.as_ref() {
        "amp" => Ok('&'),
        "apos" => Ok('\''),
        "gt" => Ok('>'),
        "lt" => Ok('<'),
        "quot" => Ok('"'),
        _ => Err(format!(
            "invalid XML entity reference: unknown entity &{name};"
        )),
    }
}

fn xml_name(bytes: &[u8]) -> Result<&str> {
    std::str::from_utf8(bytes)
        .map_err(|error| validation_error(format!("invalid UTF-8 XML name: {error}")))
}

fn validation_error(message: impl Into<String>) -> Error {
    Error::svg_postprocess(VALIDATION_PASS, message)
}

fn xml_validation_error(message: impl Into<String>) -> Error {
    Error::svg_postprocess(XML_VALIDATION_PASS, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn validate_xml(svg: &str) -> Result<()> {
        validate_well_formed_svg(svg, RenderResourcePolicy::trusted_native())
    }

    fn validate(svg: &str) -> Result<()> {
        validate_resvg_compatible_svg(svg, RenderResourcePolicy::trusted_native())
    }

    #[test]
    fn accepts_structural_fragments_and_raster_data_images() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"><defs><linearGradient id="paint"/></defs><circle fill="url(#paint)" style="clip-path:url(#clip);content:&quot;45deg&quot;"/><image href="data:image/png;base64,AAAA"/></svg>"##;

        validate(svg).unwrap();
    }

    #[test]
    fn streaming_xml_validation_accepts_a_strict_utf8_declaration_and_namespaces() {
        validate_xml(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><svg xmlns="http://www.w3.org/2000/svg" xmlns:x="urn:x"><x:item x:id="one"/></svg>"#,
        )
        .unwrap();
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
            "<svg><text>]]></text></svg>",
            "<svg><?xml version=\"1.0\"?></svg>",
            "<svg/><?xml version=\"1.0\"?>",
            "<svg><!-- a--b --></svg>",
            "<svg id=\"a<b\"/>",
            "<svg><1bad/></svg>",
            "<svg 1bad=\"value\"/>",
            "<svg bad:name:again=\"value\"/>",
            "<svg value=\"&unknown;\"/>",
            "<svg value=\"one\" value=\"two\"/>",
            "<svg xmlns:a=\"urn:x\" xmlns:b=\"urn:x\" a:id=\"one\" b:id=\"two\"/>",
            "<![CDATA[ ]]><svg/>",
            "&#32;<svg/>",
            " <?xml version=\"1.0\"?><svg/>",
            "<?xml version=\"1.1\"?><svg/>",
            "<?xml version=\"1.0\" encoding=\"UTF-16\"?><svg/>",
            "<?xml version=\"1.0\" extra=\"value\"?><svg/>",
        ] {
            assert!(validate_xml(svg).is_err(), "{svg}");
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
            RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .unwrap_err();

        assert!(
            error.to_string().contains("svg_backend_tree_depth"),
            "{error}"
        );
    }

    #[test]
    fn rejects_svg_element_count_before_usvg_parsing() {
        let svg = "<svg><g/><g/></svg>";
        let limits = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgElements, 2)
            .unwrap();

        let error = validate_resvg_compatible_svg(svg, limits).unwrap_err();

        assert!(error.to_string().contains("max_svg_elements"), "{error}");
    }

    #[test]
    fn does_not_claim_a_browser_dom_sanitizer_policy() {
        let svg = r#"<svg><a href="https://example.com"><text>link</text></a></svg>"#;

        validate(svg).unwrap();
    }
}
