use crate::resources::RenderResourcePolicy;
#[cfg(test)]
use crate::resources::ResourceLimitId;
use crate::{Error, Result};
use quick_xml::XmlVersion;
use quick_xml::events::{BytesDecl, BytesRef, BytesStart, Event};
use quick_xml::name::{NamespaceResolver, ResolveResult};
use quick_xml::reader::NsReader;
use std::collections::{HashMap, HashSet};
use svgtypes::{FuncIRI, IRI, Length};

use super::SvgReferencePlan;
use super::builtin::attr_sanitize::{
    attribute_violates_resvg_contract, matches_active_svg_element,
};
use super::builtin::css_sanitize::{
    validate_resvg_css_declaration_list, validate_resvg_css_stylesheet,
};

const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";
const XLINK_NAMESPACE: &str = "http://www.w3.org/1999/xlink";
const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";
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
    let mut additional_namespaced_attributes = HashSet::new();
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
            || (first_namespaced_attribute.is_some()
                && !additional_namespaced_attributes.insert(expanded_name))
        {
            return Err(xml_validation_error(
                "attributes must have unique expanded names",
            ));
        }
        if first_namespaced_attribute.is_none() {
            first_namespaced_attribute = Some(expanded_name);
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
pub(crate) fn validate_resvg_compatible_svg(
    svg: &str,
    limits: RenderResourcePolicy,
) -> Result<SvgReferencePlan> {
    validate_well_formed_svg(svg, limits)?;
    let mut reader = NsReader::from_str(svg);
    let mut depth = 0usize;
    let mut root_seen = false;
    let mut root_closed = false;
    let mut style_text = None::<String>;
    let mut reference_nodes = Vec::new();
    let mut reference_stack = Vec::new();

    loop {
        let event = reader
            .read_event()
            .map_err(|error| validation_error(format!("invalid XML: {error}")))?;
        match event {
            Event::Start(element) => {
                if style_text.is_some() {
                    return Err(validation_error(
                        "a <style> element contains nested XML elements",
                    ));
                }
                let is_root = depth == 0;
                reject_additional_root(is_root, root_seen, root_closed)?;
                let validated = validate_element(&element, reader.resolver(), is_root)?;
                append_reference_node(
                    &mut reference_nodes,
                    reference_stack.last().copied(),
                    validated,
                );
                if is_root {
                    root_seen = true;
                }
                depth += 1;
                if reference_nodes.last().is_some_and(|node| node.is_style) {
                    style_text = Some(String::new());
                }
                reference_stack.push(reference_nodes.len() - 1);
            }
            Event::Empty(element) => {
                if style_text.is_some() {
                    return Err(validation_error(
                        "a <style> element contains nested XML elements",
                    ));
                }
                let is_root = depth == 0;
                reject_additional_root(is_root, root_seen, root_closed)?;
                let validated = validate_element(&element, reader.resolver(), is_root)?;
                append_reference_node(
                    &mut reference_nodes,
                    reference_stack.last().copied(),
                    validated,
                );
                if reference_nodes.last().is_some_and(|node| node.is_style) {
                    validate_style_text("")?;
                }
                if is_root {
                    root_seen = true;
                    root_closed = true;
                }
            }
            Event::End(element) => {
                let (namespace, _) = reader.resolver().resolve_element(element.name());
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
                reference_stack
                    .pop()
                    .ok_or_else(|| validation_error("an end tag has no matching reference node"))?;
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
    let reference_plan = plan_svg_reference_expansion(&reference_nodes)?;
    limits.check_svg_structure(
        reference_plan.expanded_elements(),
        reference_plan.max_tree_depth(),
    )?;
    Ok(reference_plan)
}

fn reject_additional_root(is_root: bool, root_seen: bool, root_closed: bool) -> Result<()> {
    if is_root && (root_seen || root_closed) {
        return Err(validation_error(
            "the document contains more than one root element",
        ));
    }
    Ok(())
}

struct ValidatedElement {
    is_style: bool,
    is_marker: bool,
    may_repeat_per_element: bool,
    use_id: Option<String>,
    parsed_id: Option<String>,
    references: Vec<ElementReference>,
}

struct ReferenceNode {
    children: Vec<usize>,
    is_style: bool,
    is_marker: bool,
    may_repeat_per_element: bool,
    use_id: Option<String>,
    parsed_id: Option<String>,
    references: Vec<ElementReference>,
}

#[derive(Clone, Copy)]
enum ReferenceTargetKind {
    UseElement,
    ParsedElement,
    Marker,
}

struct ElementReference {
    target: String,
    multiplicity: usize,
    target_kind: ReferenceTargetKind,
}

struct ReferenceDependencyGraph {
    // Real document nodes keep their encounter-order indexes. Trailing nodes are transparent
    // candidate groups: they share duplicate-ID edges without contributing an element or depth.
    dependencies: Vec<Vec<(usize, usize)>>,
    real_nodes: usize,
}

fn append_reference_node(
    nodes: &mut Vec<ReferenceNode>,
    parent: Option<usize>,
    validated: ValidatedElement,
) {
    let index = nodes.len();
    if let Some(parent) = parent {
        nodes[parent].children.push(index);
    }
    nodes.push(ReferenceNode {
        children: Vec::new(),
        is_style: validated.is_style,
        is_marker: validated.is_marker,
        may_repeat_per_element: validated.may_repeat_per_element,
        use_id: validated.use_id,
        parsed_id: validated.parsed_id,
        references: validated.references,
    });
}

fn validate_element(
    element: &BytesStart<'_>,
    resolver: &NamespaceResolver,
    is_root: bool,
) -> Result<ValidatedElement> {
    let (namespace, local_name) = resolver.resolve_element(element.name());
    let is_svg_element = is_svg_element_namespace(&namespace);
    validate_namespace(namespace, is_root)?;
    let element_name = xml_name(local_name.as_ref())?;
    if is_root && element_name != "svg" {
        return Err(validation_error("the document root is not an SVG element"));
    }
    if matches_active_svg_element(element_name) {
        return Err(validation_error(format!(
            "active element <{element_name}> survived terminal sanitization"
        )));
    }

    let is_use = is_svg_element && element_name.eq_ignore_ascii_case("use");
    let is_fe_image = is_svg_element && element_name.eq_ignore_ascii_case("feImage");
    let is_marker = is_svg_element && element_name.eq_ignore_ascii_case("marker");
    let mut use_id = None;
    let mut parsed_id = None;
    let mut parsed_id_seen = false;
    let mut use_href = None;
    let mut use_xlink_href = None;
    let mut fe_image_href = None;
    let mut fe_image_href_seen = false;
    let mut geometry_source_len = 0usize;
    let mut geometry_source_seen = false;
    let mut marker_start = None;
    let mut marker_start_seen = false;
    let mut marker_mid = None;
    let mut marker_mid_seen = false;
    let mut marker_end = None;
    let mut marker_end_seen = false;
    let mut root_width_seen = false;
    let mut root_height_seen = false;
    for attribute in element.attributes() {
        let attribute = attribute
            .map_err(|error| validation_error(format!("invalid XML attribute: {error}")))?;
        let qualified_name = xml_name(attribute.key.as_ref())?;
        let value = attribute
            .normalized_value(XmlVersion::Implicit1_0)
            .map_err(|error| validation_error(format!("invalid XML attribute value: {error}")))?;
        if attribute.key.as_namespace_binding().is_some() {
            continue;
        }

        let (attribute_namespace, local_name) = resolver.resolve_attribute(attribute.key);
        let is_unbound_attribute = matches!(&attribute_namespace, ResolveResult::Unbound);
        let is_xlink_attribute = matches!(
            &attribute_namespace,
            ResolveResult::Bound(namespace) if namespace.as_ref() == XLINK_NAMESPACE.as_bytes()
        );
        if !usvg_consumes_attribute_namespace(attribute_namespace)? {
            continue;
        }
        let semantic_name = xml_name(local_name.as_ref())?;
        if attribute_violates_resvg_contract(element_name, qualified_name, &value) {
            return Err(validation_error(format!(
                "attribute {qualified_name:?} on <{element_name}> violates the resvg-safe contract"
            )));
        }
        if is_root && semantic_name == "width" && !root_width_seen {
            root_width_seen = true;
            validate_positive_root_dimension("width", &value)?;
        }
        if is_root && semantic_name == "height" && !root_height_seen {
            root_height_seen = true;
            validate_positive_root_dimension("height", &value)?;
        }
        if semantic_name == "style" {
            validate_resvg_css_declaration_list(&value).map_err(|error| {
                validation_error(format!(
                    "invalid style attribute on <{element_name}>: {error}"
                ))
            })?;
        }
        if is_svg_element
            && !geometry_source_seen
            && ((element_name == "path" && semantic_name == "d")
                || (matches!(element_name, "polyline" | "polygon") && semantic_name == "points"))
        {
            geometry_source_seen = true;
            geometry_source_len = value.len();
        }
        if is_svg_element && semantic_name == "id" && !parsed_id_seen {
            parsed_id_seen = true;
            parsed_id = Some(value.to_string());
        }
        if is_svg_element && is_unbound_attribute && semantic_name == "id" {
            use_id = Some(value.to_string());
        }
        if is_use && semantic_name == "href" {
            if is_xlink_attribute {
                use_xlink_href.get_or_insert_with(|| value.to_string());
            } else if is_unbound_attribute {
                use_href.get_or_insert_with(|| value.to_string());
            }
        }
        if is_fe_image && semantic_name == "href" && !fe_image_href_seen {
            fe_image_href_seen = true;
            fe_image_href = Some(value.to_string());
        }
        let marker_slot = if semantic_name == "marker-start" && !marker_start_seen {
            marker_start_seen = true;
            Some(&mut marker_start)
        } else if semantic_name == "marker-mid" && !marker_mid_seen {
            marker_mid_seen = true;
            Some(&mut marker_mid)
        } else if semantic_name == "marker-end" && !marker_end_seen {
            marker_end_seen = true;
            Some(&mut marker_end)
        } else {
            None
        };
        if let Some(slot) = marker_slot {
            let target = same_document_marker_target(&value)?;
            if target.is_some() && !is_marker_capable_svg_element(element_name) {
                return Err(validation_error(format!(
                    "marker references on <{element_name}> cannot be bounded before usvg parsing"
                )));
            }
            *slot = target.map(str::to_owned);
        }
    }

    let mut references = Vec::new();
    if let Some(target) = use_xlink_href
        .as_deref()
        .or(use_href.as_deref())
        .and_then(same_document_use_target)
    {
        references.push(ElementReference {
            target: target.to_owned(),
            multiplicity: 1,
            target_kind: ReferenceTargetKind::UseElement,
        });
    }
    if let Some(target) = fe_image_href.as_deref().and_then(same_document_use_target) {
        references.push(ElementReference {
            target: target.to_owned(),
            multiplicity: 1,
            target_kind: ReferenceTargetKind::ParsedElement,
        });
    }
    if let Some(target) = marker_start {
        references.push(ElementReference {
            target,
            multiplicity: 1,
            target_kind: ReferenceTargetKind::Marker,
        });
    }
    if let Some(target) = marker_mid {
        references.push(ElementReference {
            target,
            multiplicity: marker_mid_instance_upper_bound(element_name, geometry_source_len),
            target_kind: ReferenceTargetKind::Marker,
        });
    }
    if let Some(target) = marker_end {
        references.push(ElementReference {
            target,
            multiplicity: 1,
            target_kind: ReferenceTargetKind::Marker,
        });
    }

    Ok(ValidatedElement {
        is_style: element_name.eq_ignore_ascii_case("style"),
        is_marker,
        may_repeat_per_element: is_svg_element
            && matches!(element_name, "filter" | "mask" | "clipPath"),
        use_id,
        parsed_id,
        references,
    })
}

fn validate_positive_root_dimension(name: &str, value: &str) -> Result<()> {
    let Ok(length) = value.parse::<Length>() else {
        return Ok(());
    };
    if length.number.is_finite() && length.number > 0.0 {
        return Ok(());
    }
    Err(validation_error(format!(
        "root SVG {name} must be a positive length"
    )))
}

fn is_svg_element_namespace(namespace: &ResolveResult<'_>) -> bool {
    matches!(namespace, ResolveResult::Unbound)
        || matches!(
            namespace,
            ResolveResult::Bound(namespace) if namespace.as_ref() == SVG_NAMESPACE.as_bytes()
        )
}

fn same_document_use_target(value: &str) -> Option<&str> {
    IRI::from_str(value).ok().map(|iri| iri.0)
}

fn same_document_marker_target(value: &str) -> Result<Option<&str>> {
    if value == "none" {
        return Ok(None);
    }
    FuncIRI::from_str(value)
        .map(|iri| Some(iri.0))
        .map_err(|_| validation_error("marker reference is not a bounded same-document url()"))
}

fn is_marker_capable_svg_element(element_name: &str) -> bool {
    matches!(
        element_name,
        "path" | "line" | "polyline" | "polygon" | "rect" | "circle" | "ellipse"
    )
}

fn marker_mid_instance_upper_bound(element_name: &str, geometry_source_len: usize) -> usize {
    let cap = crate::resources::MAX_RESVG_TREE_NODES.saturating_add(1);
    let estimate = match element_name {
        // One arc command may be lowered to multiple cubic segments. Four per source byte remains
        // a conservative bound even for compressed implicit command repetition.
        "path" => geometry_source_len.saturating_mul(4).saturating_add(16),
        "polyline" | "polygon" => geometry_source_len.saturating_add(4),
        "line" => 2,
        "rect" | "circle" | "ellipse" => 16,
        _ => 0,
    };
    estimate.min(cap)
}

fn plan_svg_reference_expansion(nodes: &[ReferenceNode]) -> Result<SvgReferencePlan> {
    let Some(_) = nodes.first() else {
        return Err(validation_error(
            "the document does not contain an SVG reference root",
        ));
    };

    let mut dependencies = build_svg_reference_dependencies(nodes);
    let baseline_plan = plan_svg_reference_dependencies(&dependencies)?;
    let application_upper_bound = baseline_plan.expanded_elements();
    for (index, node) in nodes.iter().enumerate() {
        if node.may_repeat_per_element {
            // Filter, mask, and clip-path definitions may be selected from inline attributes or
            // CSS and are evaluated in a caller-specific context. Charge each definition once per
            // `<use>`-expanded source element to bound nested image decoding without
            // reimplementing CSS selector matching or usvg's private effect cache policy.
            dependencies.dependencies[0].push((index, application_upper_bound));
        }
    }

    plan_svg_reference_dependencies(&dependencies)
}

fn build_svg_reference_dependencies(nodes: &[ReferenceNode]) -> ReferenceDependencyGraph {
    let real_nodes = nodes.len();

    let mut use_ids = HashMap::new();
    let mut parsed_ids: HashMap<&str, Vec<(usize, bool)>> = HashMap::new();
    for (index, node) in nodes.iter().enumerate() {
        if let Some(id) = node.use_id.as_deref() {
            // Raw `<use>` expansion resolves the first unqualified XML `id`.
            use_ids.entry(id).or_insert(index);
        }
        if let Some(id) = node.parsed_id.as_deref() {
            // Keep every candidate conservatively. usvg ignores unknown elements and otherwise
            // resolves duplicate parsed IDs to one element; charging the union avoids mirroring
            // its private element whitelist while ensuring an ignored duplicate cannot shadow a
            // resource-bearing target in this preflight graph.
            parsed_ids
                .entry(id)
                .or_default()
                .push((index, node.is_marker));
        }
    }

    let mut dependencies = nodes
        .iter()
        .map(|node| {
            node.children
                .iter()
                .map(|&index| (index, 1_usize))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut parsed_dependencies = HashMap::new();
    let mut marker_dependencies = HashMap::new();
    for (index, node) in nodes.iter().enumerate() {
        for reference in &node.references {
            let target_index = match reference.target_kind {
                ReferenceTargetKind::UseElement => use_ids.get(reference.target.as_str()).copied(),
                ReferenceTargetKind::ParsedElement => resolve_parsed_id_dependency(
                    reference.target.as_str(),
                    false,
                    &parsed_ids,
                    &mut parsed_dependencies,
                    &mut dependencies,
                ),
                ReferenceTargetKind::Marker => resolve_parsed_id_dependency(
                    reference.target.as_str(),
                    true,
                    &parsed_ids,
                    &mut marker_dependencies,
                    &mut dependencies,
                ),
            };
            if let Some(target_index) = target_index {
                dependencies[index].push((target_index, reference.multiplicity));
            }
        }
    }

    ReferenceDependencyGraph {
        dependencies,
        real_nodes,
    }
}

fn resolve_parsed_id_dependency<'a>(
    target: &str,
    marker_only: bool,
    parsed_ids: &HashMap<&'a str, Vec<(usize, bool)>>,
    resolved: &mut HashMap<&'a str, Option<usize>>,
    dependencies: &mut Vec<Vec<(usize, usize)>>,
) -> Option<usize> {
    let (id, candidates) = parsed_ids.get_key_value(target)?;
    if let [(index, is_marker)] = candidates.as_slice() {
        return (!marker_only || *is_marker).then_some(*index);
    }
    if let Some(&dependency) = resolved.get(target) {
        return dependency;
    }
    let dependency = if marker_only {
        resolve_reference_candidates(
            dependencies,
            candidates
                .iter()
                .filter_map(|&(index, is_marker)| is_marker.then_some(index)),
        )
    } else {
        resolve_reference_candidates(dependencies, candidates.iter().map(|&(index, _)| index))
    };
    resolved.insert(*id, dependency);
    dependency
}

fn resolve_reference_candidates(
    dependencies: &mut Vec<Vec<(usize, usize)>>,
    mut candidates: impl Iterator<Item = usize>,
) -> Option<usize> {
    let Some(first) = candidates.next() else {
        return None;
    };
    let Some(second) = candidates.next() else {
        return Some(first);
    };

    // A duplicate-ID candidate union is materialized once. Every parsed-tree reference then owns
    // one edge to this transparent group instead of copying every candidate edge.
    let group_index = dependencies.len();
    let mut group = Vec::with_capacity(candidates.size_hint().0.saturating_add(2));
    group.push((first, 1));
    group.push((second, 1));
    group.extend(candidates.map(|index| (index, 1)));
    dependencies.push(group);
    Some(group_index)
}

fn plan_svg_reference_dependencies(graph: &ReferenceDependencyGraph) -> Result<SvgReferencePlan> {
    let cap = crate::resources::MAX_RESVG_TREE_NODES.saturating_add(1);
    let dependencies = &graph.dependencies;
    let nodes_len = dependencies.len();
    let mut states = vec![0_u8; nodes_len];
    let mut expanded_elements = vec![0_usize; nodes_len];
    let mut expanded_depths = vec![0_usize; nodes_len];
    let mut postorder = Vec::with_capacity(nodes_len);
    let mut stack = vec![(0_usize, false)];

    while let Some((index, complete)) = stack.pop() {
        if complete {
            let is_group = index >= graph.real_nodes;
            let mut elements = if is_group { 0 } else { 1 };
            let mut depth = 0_usize;
            for &(dependency, multiplicity) in &dependencies[index] {
                let dependency_elements =
                    capped_svg_reference_mul(expanded_elements[dependency], multiplicity, cap);
                elements = capped_svg_reference_add(elements, dependency_elements, cap);
                let dependency_depth = expanded_depths[dependency];
                depth = depth.max(if is_group {
                    dependency_depth
                } else {
                    dependency_depth.saturating_add(1)
                });
            }
            expanded_elements[index] = elements;
            expanded_depths[index] = depth;
            states[index] = 2;
            postorder.push(index);
            continue;
        }

        match states[index] {
            0 => {
                states[index] = 1;
                stack.push((index, true));
                for &(dependency, _) in dependencies[index].iter().rev() {
                    match states[dependency] {
                        0 => stack.push((dependency, false)),
                        1 => {
                            return Err(validation_error(
                                "same-document SVG expansion references contain a cycle",
                            ));
                        }
                        2 => {}
                        _ => unreachable!("SVG reference traversal state is bounded"),
                    }
                }
            }
            1 => {
                return Err(validation_error(
                    "same-document SVG expansion references contain a cycle",
                ));
            }
            2 => {}
            _ => unreachable!("SVG reference traversal state is bounded"),
        }
    }

    let mut raw_element_occurrences = vec![0_usize; nodes_len];
    raw_element_occurrences[0] = 1;
    for index in postorder.into_iter().rev() {
        let occurrences = raw_element_occurrences[index];
        for &(dependency, multiplicity) in &dependencies[index] {
            let added = capped_svg_reference_mul(occurrences, multiplicity, cap);
            raw_element_occurrences[dependency] =
                capped_svg_reference_add(raw_element_occurrences[dependency], added, cap);
        }
    }
    raw_element_occurrences.truncate(graph.real_nodes);

    Ok(SvgReferencePlan {
        expanded_elements: expanded_elements[0],
        max_tree_depth: expanded_depths[0],
        raw_element_occurrences: raw_element_occurrences.into_boxed_slice(),
    })
}

fn capped_svg_reference_add(value: usize, additional: usize, cap: usize) -> usize {
    value.saturating_add(additional).min(cap)
}

fn capped_svg_reference_mul(value: usize, multiplier: usize, cap: usize) -> usize {
    value.saturating_mul(multiplier).min(cap)
}

// usvg maps attributes from these namespaces to SVG attribute ids by local name.
fn usvg_consumes_attribute_namespace(namespace: ResolveResult<'_>) -> Result<bool> {
    match namespace {
        ResolveResult::Unknown(prefix) => Err(validation_error(format!(
            "attribute uses an unknown namespace prefix {:?}",
            String::from_utf8_lossy(&prefix)
        ))),
        ResolveResult::Unbound => Ok(true),
        ResolveResult::Bound(namespace) => {
            let namespace = namespace.as_ref();
            Ok(namespace == SVG_NAMESPACE.as_bytes()
                || namespace == XLINK_NAMESPACE.as_bytes()
                || namespace == XML_NAMESPACE.as_bytes())
        }
    }
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
        validate_resvg_compatible_svg(svg, RenderResourcePolicy::trusted_native()).map(|_| ())
    }

    #[test]
    fn accepts_structural_fragments_and_raster_data_images() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"><defs><linearGradient id="paint"/><rect id="filter-source"/></defs><circle fill="url(#paint)" style="clip-path:url(#clip);content:&quot;45deg&quot;"/><image href="data:image/png;base64,AAAA"/><filter><feImage href="#filter-source"/><feImage href="data:image/png;base64,BBBB"/></filter></svg>"##;

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
            "<svg><path style=\"background:url(../image.png)\"/></svg>",
            "<svg><style>.node{background:url(https://example.com/image.png)}</style></svg>",
            "<svg><path href=\"java&#x73;cript:alert(1)\"/></svg>",
            "<svg><path onclick=\"alert(1)\"/></svg>",
        ] {
            assert!(validate(svg).is_err(), "{svg}");
        }
    }

    #[test]
    fn rejects_guarded_attributes_in_namespaces_consumed_by_usvg() {
        let namespaces = [
            ("s", r#"xmlns:s="http://www.w3.org/2000/svg""#),
            ("q", r#"xmlns:q="http://www.w3.org/1999/xlink""#),
            ("xml", ""),
        ];
        let guarded_attributes = [
            ("style", "animation:spin 1s"),
            ("fill", "url(file:///tmp/paint.svg#paint)"),
            ("width", "NaN"),
            ("transform", "rotate(NaN)"),
            ("d", "M 0 NaN"),
        ];

        for (prefix, declaration) in namespaces {
            for (name, value) in guarded_attributes {
                let svg = format!(r#"<svg {declaration}><path {prefix}:{name}="{value}"/></svg>"#);
                assert!(validate(&svg).is_err(), "{svg}");
            }
        }
    }

    #[test]
    fn ignores_guarded_attribute_names_in_namespaces_ignored_by_usvg() {
        let svg = r#"<svg xmlns:style="urn:example:ignored"><path style:style="animation:spin 1s" style:fill="url(file:///tmp/paint.svg#paint)" style:width="NaN" style:transform="rotate(NaN)" style:d="M 0 NaN"/></svg>"#;

        validate(svg).unwrap();
    }

    #[test]
    fn rejects_non_positive_root_dimensions_from_the_first_usvg_projection() {
        for (name, value) in [
            ("width", "0"),
            ("width", "-1px"),
            ("height", "0%"),
            ("height", "-0.5em"),
        ] {
            let svg = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" {name}="{value}" viewBox="0 0 10 10"/>"#
            );
            let error = validate(&svg).expect_err("non-positive root dimensions must be rejected");
            assert!(
                error
                    .to_string()
                    .contains(&format!("root SVG {name} must be a positive length")),
                "{error}"
            );
        }

        let error = validate(
            r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:s="http://www.w3.org/2000/svg" s:width="0" width="200" viewBox="0 0 10 10"/>"#,
        )
        .expect_err("the first usvg-projected width must define the terminal contract");
        assert!(
            error
                .to_string()
                .contains("root SVG width must be a positive length"),
            "{error}"
        );
    }

    #[test]
    fn root_dimension_validation_ignores_other_namespaces_and_child_geometry() {
        validate(
            r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:i="urn:ignored" i:width="0" width="100%" height="10em" viewBox="0 0 10 10"><rect width="0" height="-1"/></svg>"#,
        )
        .unwrap();
    }

    #[test]
    fn rejects_malformed_approved_data_image_payloads() {
        for value in [
            "data:image/png;base64,AA*A",
            "data:image/png;base64,A===",
            "data:image/png,%",
            "data:image/png,%GG",
            "data:image/png;base64,AA%GG",
        ] {
            let svg = format!(r#"<svg><image href="{value}"/></svg>"#);
            assert!(validate(&svg).is_err(), "{svg}");
        }
    }

    #[test]
    fn rejects_external_render_resources_that_survive_terminal_sanitization() {
        for svg in [
            r#"<svg><image href="/tmp/secret.png"/></svg>"#,
            r#"<svg><image href="../secret.png"/></svg>"#,
            r#"<svg><image href="bare.png"/></svg>"#,
            r#"<svg><image href="//example.com/image.png"/></svg>"#,
            r#"<svg><image href="\\server\share\secret.png"/></svg>"#,
            r#"<svg><image href="https://example.com/image.png"/></svg>"#,
            r##"<svg><image href="#image"/></svg>"##,
            r#"<svg><image href="data:image/png"/></svg>"#,
            r#"<svg><image href="d a t a:image/png;base64,AAAA"/></svg>"#,
            r#"<svg><image href="data:image /png;base64,AAAA"/></svg>"#,
            r#"<svg><image href="data:image/svg+xml;base64,PHN2Zy8+"/></svg>"#,
            r#"<svg><feImage href="./filter.png"/></svg>"#,
            r#"<svg><use href="sprites.svg#shape"/></svg>"#,
            r#"<svg><use href="data:image/png;base64,AAAA"/></svg>"#,
            r#"<svg><textPath href="text.svg#path">text</textPath></svg>"#,
            r##"<svg xml:base="/tmp/"><image href="#image"/></svg>"##,
            r##"<svg xmlns:b="http://www.w3.org/XML/1998/namespace"><g b:base="../nested/"><image href="#image"/></g></svg>"##,
            r#"<svg xmlns:q="http://www.w3.org/1999/xlink"><image q:href="../secret.png"/></svg>"#,
            r#"<svg><a href="data:image/png;base64,AAAA"><text>data link</text></a></svg>"#,
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

    fn branching_use_svg(levels: usize) -> String {
        let mut svg = String::from(
            r#"<svg xmlns="http://www.w3.org/2000/svg"><defs><g id="leaf"><rect/></g>"#,
        );
        for index in 0..levels {
            let target = if index + 1 == levels {
                "leaf".to_owned()
            } else {
                format!("use-{}", index + 1)
            };
            svg.push_str(&format!(
                r##"<g id="use-{index}"><use href="#{target}"/><use href="#{target}"/></g>"##
            ));
        }
        svg.push_str(r##"</defs><use href="#use-0"/></svg>"##);
        svg
    }

    #[test]
    fn rejects_branching_use_expansion_before_usvg_parsing() {
        let svg = branching_use_svg(6);
        let unrestricted = validate_resvg_compatible_svg(
            &svg,
            RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .unwrap();
        assert!(unrestricted.expanded_elements() > 100);
        assert!(
            unrestricted
                .raw_element_occurrences()
                .iter()
                .any(|occurrences| *occurrences > 1)
        );

        let limits = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgElements, 100)
            .unwrap();
        let error = validate_resvg_compatible_svg(&svg, limits).unwrap_err();

        assert!(error.to_string().contains("max_svg_elements"), "{error}");
    }

    #[test]
    fn local_filter_images_and_marker_fanout_contribute_to_the_pre_usvg_plan() {
        let filter_svg = r##"<svg xmlns="http://www.w3.org/2000/svg"><defs><g id="source"><image href="data:image/png;base64,AAAA"/></g><filter id="filter"><feImage href="#source"/><feImage href="#source"/></filter></defs></svg>"##;
        let filter_plan = validate_resvg_compatible_svg(
            filter_svg,
            RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .unwrap();
        assert!(
            filter_plan
                .raw_element_occurrences()
                .iter()
                .any(|occurrences| *occurrences >= 3)
        );

        let marker_svg = r##"<svg xmlns="http://www.w3.org/2000/svg"><defs><marker id="marker"><image href="data:image/png;base64,AAAA"/></marker></defs><path d="M0 0L1 1L2 2L3 3L4 4" marker-mid="url(#marker)"/></svg>"##;
        let marker_plan = validate_resvg_compatible_svg(
            marker_svg,
            RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .unwrap();
        assert!(
            marker_plan
                .raw_element_occurrences()
                .iter()
                .any(|occurrences| *occurrences > 4)
        );
    }

    #[test]
    fn reference_plan_matches_usvg_namespace_alias_and_first_attribute_semantics() {
        let fe_image_svg = r##"<svg xmlns="http://www.w3.org/2000/svg"><defs><g id="source"><rect/><rect/><rect/><rect/><rect/></g><g id="other"/><filter><feImage xml:href="#source" href="#other"/></filter></defs></svg>"##;
        let fe_image_plan = validate_resvg_compatible_svg(
            fe_image_svg,
            RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .unwrap();
        assert!(
            fe_image_plan.expanded_elements() >= 17,
            "xml:href must be the first projected AId::Href: {fe_image_plan:?}"
        );

        let marker_svg = r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:s="http://www.w3.org/2000/svg"><defs><marker id="marker"><image href="data:image/png;base64,AAAA"/></marker></defs><path d="M0 0L1 1L2 2L3 3L4 4" marker-mid="url(#marker)" s:d="M0 0" s:marker-mid="none"/></svg>"##;
        let marker_plan = validate_resvg_compatible_svg(
            marker_svg,
            RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .unwrap();
        assert!(
            marker_plan
                .raw_element_occurrences()
                .iter()
                .any(|occurrences| *occurrences > 4),
            "later namespace aliases must not override the first projected geometry or marker attribute"
        );
    }

    #[test]
    fn reference_plan_matches_usvg_use_and_parsed_tree_id_resolution() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:s="http://www.w3.org/2000/svg"><defs><g id="shared"/><marker id="shared"><image href="data:image/png;base64,AAAA"/></marker><unknown id="shared"/><g s:id="aliased"><image href="data:image/png;base64,BBBB"/></g><filter><feImage href="#aliased"/></filter></defs><path d="M0 0L1 1L2 2L3 3L4 4" marker-mid="url(#shared)"/><use href="#shared"/></svg>"##;
        let plan =
            validate_resvg_compatible_svg(svg, RenderResourcePolicy::unbounded_for_trusted_input())
                .unwrap();
        let occurrences = plan.raw_element_occurrences();

        assert_eq!(
            occurrences[2], 2,
            "<use> must resolve the first unqualified XML id"
        );
        assert!(
            occurrences[4] > 4,
            "parsed duplicate ids must not shadow a resource-bearing marker"
        );
        assert!(
            occurrences[7] >= 2,
            "parsed-tree references must resolve namespace-aliased ids"
        );
    }

    #[test]
    fn duplicate_parsed_id_dependencies_are_shared_without_changing_plan_semantics() {
        const DUPLICATES: usize = 64;
        const MARKERS: usize = 32;
        const REFERENCES: usize = 64;

        let real_nodes = 1 + DUPLICATES + REFERENCES;
        let node = |is_marker, parsed_id: Option<&str>, references| ReferenceNode {
            children: Vec::new(),
            is_style: false,
            is_marker,
            may_repeat_per_element: false,
            use_id: None,
            parsed_id: parsed_id.map(str::to_owned),
            references,
        };
        let references = || {
            vec![
                ElementReference {
                    target: "shared".to_owned(),
                    multiplicity: 2,
                    target_kind: ReferenceTargetKind::ParsedElement,
                },
                ElementReference {
                    target: "shared".to_owned(),
                    multiplicity: 3,
                    target_kind: ReferenceTargetKind::Marker,
                },
            ]
        };
        let mut nodes = Vec::with_capacity(real_nodes);
        nodes.push(node(false, None, Vec::new()));
        nodes
            .extend((0..DUPLICATES).map(|index| node(index < MARKERS, Some("shared"), Vec::new())));
        nodes.extend((0..REFERENCES).map(|_| node(false, None, references())));
        nodes[0].children = (1..real_nodes).collect();

        let graph = build_svg_reference_dependencies(&nodes);
        let dependency_edges = graph.dependencies.iter().map(Vec::len).sum::<usize>();
        assert_eq!(graph.real_nodes, real_nodes);
        assert_eq!(graph.dependencies.len(), real_nodes + 2);
        assert_eq!(dependency_edges, 2 * DUPLICATES + MARKERS + 3 * REFERENCES);

        let plan = plan_svg_reference_dependencies(&graph).unwrap();
        assert_eq!(
            plan.expanded_elements(),
            1 + DUPLICATES + REFERENCES * (1 + 2 * DUPLICATES + 3 * MARKERS)
        );
        assert_eq!(plan.max_tree_depth(), 2);
        assert_eq!(plan.raw_element_occurrences().len(), real_nodes);
        for (index, &occurrences) in plan.raw_element_occurrences()[1..=DUPLICATES]
            .iter()
            .enumerate()
        {
            let expected = if index < MARKERS {
                1 + 5 * REFERENCES
            } else {
                1 + 2 * REFERENCES
            };
            assert_eq!(occurrences, expected);
        }
        assert!(
            plan.raw_element_occurrences()[1 + DUPLICATES..]
                .iter()
                .all(|&occurrences| occurrences == 1)
        );
    }

    #[test]
    fn effect_definitions_charge_resource_descendants_for_every_possible_application() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"><defs><image id="source" href="data:image/png;base64,AAAA"/><filter id="filter"><feImage href="#source"/></filter><mask id="mask"><image href="data:image/png;base64,BBBB"/></mask></defs><style>.all{filter:url(#filter);mask:url(#mask)}</style><rect class="all"/><rect class="all"/><rect class="all"/></svg>"##;
        let plan =
            validate_resvg_compatible_svg(svg, RenderResourcePolicy::unbounded_for_trusted_input())
                .unwrap();

        assert!(
            plan.raw_element_occurrences()
                .iter()
                .filter(|&&occurrences| occurrences > 1)
                .count()
                >= 4,
            "filter and mask descendants must be charged beyond their one source occurrence"
        );
    }

    #[test]
    fn effect_definitions_charge_use_expanded_application_instances() {
        let mut svg = String::from(
            r##"<svg xmlns="http://www.w3.org/2000/svg"><defs><image id="source" href="data:image/png;base64,AAAA"/><filter id="filter"><feImage href="#source"/></filter><g id="leaf"><rect filter="url(#filter)"/></g>"##,
        );
        for index in 0..5 {
            let target = if index == 0 {
                "leaf".to_owned()
            } else {
                format!("fanout-{}", index - 1)
            };
            svg.push_str(&format!(
                r##"<g id="fanout-{index}"><use href="#{target}"/><use href="#{target}"/></g>"##
            ));
        }
        svg.push_str(r##"</defs><use href="#fanout-4"/></svg>"##);

        let plan = validate_resvg_compatible_svg(
            &svg,
            RenderResourcePolicy::unbounded_for_trusted_input(),
        )
        .unwrap();
        let raw_nodes = plan.raw_element_occurrences().len();

        assert!(
            plan.raw_element_occurrences()[2] > raw_nodes.saturating_mul(2),
            "filter image must be charged for <use>-expanded effect applications: {plan:?}"
        );
    }

    #[test]
    fn same_document_reference_parsing_delegates_to_svgtypes() {
        assert_eq!(same_document_use_target("#target\t"), Some("target\t"));
        assert_eq!(
            same_document_marker_target("url(#target\t)").unwrap(),
            Some("target\t")
        );
        assert_eq!(
            same_document_marker_target("url('#target\u{a0}')").unwrap(),
            Some("target")
        );
    }

    #[test]
    fn rejects_css_and_inherited_marker_references_that_bypass_static_fanout() {
        let css_error = validate(
            r##"<svg><defs><marker id="m"><path d="M0 0L1 1"/></marker></defs><style>.edge{marker-mid:url(#m)}</style><path class="edge" d="M0 0L1 1L2 2"/></svg>"##,
        )
        .unwrap_err();
        assert!(css_error.to_string().contains("CSS marker"), "{css_error}");

        let inherited_error = validate(
            r##"<svg><defs><marker id="m"><path d="M0 0L1 1"/></marker></defs><g marker-end="url(#m)"><path d="M0 0L1 1"/></g></svg>"##,
        )
        .unwrap_err();
        assert!(
            inherited_error
                .to_string()
                .contains("cannot be bounded before usvg"),
            "{inherited_error}"
        );
    }

    #[test]
    fn rejects_cyclic_same_document_expansion_references() {
        for svg in [
            r##"<svg xmlns="http://www.w3.org/2000/svg"><defs><g id="first"><use href="#second"/></g><g id="second"><use href="#first"/></g></defs><use href="#first"/></svg>"##,
            r##"<svg xmlns="http://www.w3.org/2000/svg"><defs><g id="loop"><filter><feImage href="#loop"/></filter></g><unknown id="loop"/></defs></svg>"##,
        ] {
            let error = validate(svg).unwrap_err();

            assert!(
                error
                    .to_string()
                    .contains("same-document SVG expansion references contain a cycle"),
                "{error}"
            );
        }
    }

    #[test]
    fn does_not_claim_a_browser_dom_sanitizer_policy() {
        let svg = r#"<svg xmlns:q="http://www.w3.org/1999/xlink"><a href="https://example.com" q:href="../guide"><text>link</text></a></svg>"#;

        validate(svg).unwrap();
    }
}
