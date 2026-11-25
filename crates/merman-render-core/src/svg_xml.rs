//! XML grammar and raw structure admission for SVG postprocessing.

use crate::{Error, Result};
use quick_xml::XmlVersion;
use quick_xml::events::{BytesDecl, BytesRef, BytesStart, Event};
use quick_xml::name::{NamespaceResolver, ResolveResult};
use quick_xml::reader::NsReader;
use std::borrow::Cow;
use std::collections::HashSet;

pub const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";
pub const XML_VALIDATION_PASS: &str = "validate-well-formed-svg";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SvgStructureMetrics {
    pub elements: usize,
    pub max_tree_depth: usize,
}

/// Receives attribute values after XML normalization and elements after structure admission.
pub trait SvgXmlVisitor {
    type Attributes<'xml>;
    type Element;

    fn begin<'xml>(
        &mut self,
        local_name: &'xml str,
        is_svg: bool,
    ) -> Option<Self::Attributes<'xml>>;

    fn attribute<'xml>(
        &mut self,
        attributes: &mut Self::Attributes<'xml>,
        namespace: ResolveResult<'_>,
        local_name: &str,
        value: Cow<'xml, str>,
        checkpoint: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()>;

    fn finish(&mut self, attributes: Self::Attributes<'_>) -> Self::Element;

    fn element(&mut self, element: Self::Element, depth: usize) -> Result<()>;
}

impl SvgXmlVisitor for () {
    type Attributes<'xml> = ();
    type Element = ();

    fn begin(&mut self, _local_name: &str, _is_svg: bool) -> Option<()> {
        None
    }

    fn attribute(
        &mut self,
        _attributes: &mut (),
        _namespace: ResolveResult<'_>,
        _local_name: &str,
        _value: Cow<'_, str>,
        _checkpoint: &mut dyn FnMut() -> Result<()>,
    ) -> Result<()> {
        Ok(())
    }

    fn finish(&mut self, _attributes: ()) {}

    fn element(&mut self, _element: (), _depth: usize) -> Result<()> {
        Ok(())
    }
}

/// Checks the SVG XML grammar with the caller's operation controls.
pub fn validate_well_formed_svg_with_controls(
    svg: &str,
    checkpoint: &mut dyn FnMut() -> Result<()>,
    check_structure: &mut dyn FnMut(usize, usize) -> Result<()>,
) -> Result<SvgStructureMetrics> {
    visit_well_formed_svg_with_controls(svg, checkpoint, check_structure, &mut ())
}

/// Checks XML grammar and projects elements in preorder during the same walk.
#[inline(never)]
pub fn visit_well_formed_svg_with_controls<V: SvgXmlVisitor>(
    svg: &str,
    checkpoint: &mut dyn FnMut() -> Result<()>,
    check_structure: &mut dyn FnMut(usize, usize) -> Result<()>,
    visitor: &mut V,
) -> Result<SvgStructureMetrics> {
    let mut reader = NsReader::from_str(svg);
    reader.config_mut().enable_all_checks(true);
    let mut depth = 0usize;
    let mut elements = 0usize;
    let mut max_tree_depth = 0usize;
    let mut root_seen = false;
    let mut root_closed = false;
    let mut document_started = false;

    loop {
        checkpoint()?;
        let event = reader.read_event();
        checkpoint()?;
        let event = event.map_err(|error| xml_validation_error(format!("invalid XML: {error}")))?;
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
                let references = validate_well_formed_element(
                    &element,
                    namespace,
                    reader.resolver(),
                    is_root,
                    checkpoint,
                    visitor,
                )?;
                if is_root {
                    root_seen = true;
                }
                elements = elements.saturating_add(1);
                depth += 1;
                max_tree_depth = max_tree_depth.max(depth.saturating_sub(1));
                checkpoint()?;
                check_structure(elements, max_tree_depth)?;
                if let Some(references) = references {
                    visitor.element(references, depth - 1)?;
                }
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
                let references = validate_well_formed_element(
                    &element,
                    namespace,
                    reader.resolver(),
                    is_root,
                    checkpoint,
                    visitor,
                )?;
                elements = elements.saturating_add(1);
                max_tree_depth = max_tree_depth.max(depth);
                checkpoint()?;
                check_structure(elements, max_tree_depth)?;
                if let Some(references) = references {
                    visitor.element(references, depth)?;
                }
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
                let contains_cdata_close = text.contains("]]>");
                checkpoint()?;
                if contains_cdata_close {
                    return Err(xml_validation_error(
                        "the sequence ]]> is not allowed in XML text",
                    ));
                }
                let text = text.xml10_content();
                checkpoint()?;
                validate_xml_characters(&text, checkpoint)?;
                let text_outside_root = depth == 0 && !text.trim().is_empty();
                checkpoint()?;
                if text_outside_root {
                    return Err(xml_validation_error(
                        "text is not allowed outside the SVG root",
                    ));
                }
            }
            Event::CData(text) => {
                document_started = true;
                checkpoint()?;
                if depth == 0 {
                    return Err(xml_validation_error(
                        "CDATA is not allowed outside the SVG root",
                    ));
                }
                validate_xml_characters(text.as_ref(), checkpoint)?;
            }
            Event::GeneralRef(reference) => {
                document_started = true;
                let value = resolve_xml_reference_value(&reference);
                checkpoint()?;
                let value = value.map_err(xml_validation_error)?;
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
            Event::Comment(text) => {
                checkpoint()?;
                validate_xml_characters(text.as_ref(), checkpoint)?;
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

    checkpoint()?;
    Ok(SvgStructureMetrics {
        elements,
        max_tree_depth,
    })
}

#[inline(never)]
fn validate_well_formed_element<V: SvgXmlVisitor>(
    element: &BytesStart<'_>,
    namespace: ResolveResult<'_>,
    resolver: &NamespaceResolver,
    is_root: bool,
    checkpoint: &mut dyn FnMut() -> Result<()>,
    visitor: &mut V,
) -> Result<Option<V::Element>> {
    let is_svg_element = is_svg_element_namespace(&namespace);
    match namespace {
        ResolveResult::Unknown(prefix) => {
            return Err(xml_validation_error(format!(
                "element uses an unknown namespace prefix {prefix:?}"
            )));
        }
        ResolveResult::Bound(namespace) if is_root && namespace.as_ref() != SVG_NAMESPACE => {
            return Err(xml_validation_error(
                "the root element uses a non-SVG namespace",
            ));
        }
        ResolveResult::Unbound | ResolveResult::Bound(_) => {}
    }

    validate_xml_qname(element.name().as_ref())?;
    let local_name = element.local_name();
    let element_name = local_name.as_ref();
    if is_root && element_name != "svg" {
        return Err(xml_validation_error(
            "the document root is not an SVG element",
        ));
    }
    let mut attributes = visitor.begin(element_name, is_svg_element);
    let mut first_namespaced_attribute = None;
    let mut additional_namespaced_attributes = HashSet::new();
    for attribute in element.attributes() {
        checkpoint()?;
        let attribute = attribute
            .map_err(|error| xml_validation_error(format!("invalid XML attribute: {error}")))?;
        let qualified_name = validate_xml_qname(attribute.key.as_ref());
        checkpoint()?;
        qualified_name?;
        let contains_open_angle = attribute.value.contains('<');
        checkpoint()?;
        if contains_open_angle {
            return Err(xml_validation_error(
                "the character < is not allowed in an XML attribute value",
            ));
        }
        let normalized = attribute.normalized_value(XmlVersion::Implicit1_0);
        checkpoint()?;
        let value = normalized.map_err(|error| {
            xml_validation_error(format!("invalid XML attribute value: {error}"))
        })?;
        validate_xml_characters(&value, checkpoint)?;

        if attribute.key.as_namespace_binding().is_some() {
            continue;
        }
        if attribute.key.prefix().is_some() {
            let (namespace, local_name) = resolver.resolve_attribute(attribute.key);
            let namespace = match namespace {
                ResolveResult::Unknown(prefix) => {
                    checkpoint()?;
                    return Err(xml_validation_error(format!(
                        "attribute uses an unknown namespace prefix {prefix:?}"
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
                checkpoint()?;
                return Err(xml_validation_error(
                    "attributes must have unique expanded names",
                ));
            }
            if first_namespaced_attribute.is_none() {
                first_namespaced_attribute = Some(expanded_name);
            }
        }
        if let Some(attributes) = attributes.as_mut() {
            let (namespace, local_name) = resolver.resolve_attribute(attribute.key);
            visitor.attribute(
                attributes,
                namespace,
                local_name.as_ref(),
                value,
                checkpoint,
            )?;
        }
    }
    Ok(attributes.map(|attributes| visitor.finish(attributes)))
}

fn validate_xml_declaration(declaration: &BytesDecl<'_>) -> Result<()> {
    let declaration = BytesStart::from_content(declaration.as_ref(), 3);
    let mut attributes = declaration.attributes();

    let version = attributes
        .next()
        .transpose()
        .map_err(|error| xml_validation_error(format!("invalid XML declaration: {error}")))?
        .ok_or_else(|| xml_validation_error("the XML declaration is missing version"))?;
    if version.key.as_ref() != "version" || version.value.as_ref() != "1.0" {
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
            (1, "encoding", value) if value.eq_ignore_ascii_case("utf-8") => {
                expected_attribute = 2;
            }
            (1 | 2, "standalone", "yes" | "no") => {
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

pub fn validate_xml_characters(
    value: &str,
    checkpoint: &mut dyn FnMut() -> Result<()>,
) -> Result<()> {
    const MAX_BLOCK_BYTES: usize = 256;

    let mut start = 0;
    while start < value.len() {
        checkpoint()?;
        let mut end = start.saturating_add(MAX_BLOCK_BYTES).min(value.len());
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        let block = &value[start..end];
        // In valid UTF-8, XML 1.0 forbids only ASCII controls other than TAB/LF/CR and
        // U+FFFE/U+FFFF. Surrogates cannot occur, and other Unicode noncharacters are legal.
        let (invalid_control, has_ef) =
            block
                .as_bytes()
                .iter()
                .fold((false, false), |(invalid_control, has_ef), &byte| {
                    (
                        invalid_control | (byte < 0x20 && !matches!(byte, b'\t' | b'\n' | b'\r')),
                        has_ef | (byte == 0xef),
                    )
                });
        // Scalar-aligned ends cannot split EF BF BE/BF. Without EF, neither forbidden
        // scalar is present, including in Unicode text. There is no whole-input fallback.
        let invalid_noncharacter =
            has_ef && (block.contains('\u{fffe}') || block.contains('\u{ffff}'));
        if (invalid_control || invalid_noncharacter)
            && let Some(character) = block
                .chars()
                .find(|&character| !crate::xml::is_xml_1_0_char(character))
        {
            checkpoint()?;
            return Err(xml_validation_error(format!(
                "the character U+{:04X} is forbidden in XML 1.0",
                u32::from(character)
            )));
        }
        // Each block bounds the fold, two possible searches and invalid-scalar localization:
        // at most four passes over <=256 bytes (<=1024 logical input bytes), not 256 total work.
        start = end;
    }
    checkpoint()
}

fn validate_xml_qname(name: &str) -> Result<()> {
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

pub fn is_svg_element_namespace(namespace: &ResolveResult<'_>) -> bool {
    matches!(namespace, ResolveResult::Unbound)
        || matches!(
            namespace,
            ResolveResult::Bound(namespace) if namespace.as_ref() == SVG_NAMESPACE
        )
}

pub fn resolve_xml_reference_value(reference: &BytesRef<'_>) -> std::result::Result<char, String> {
    if let Some(value) = reference
        .resolve_char_ref()
        .map_err(|error| format!("invalid XML character reference: {error}"))?
    {
        if crate::xml::is_xml_1_0_char(value) {
            return Ok(value);
        }
        return Err("invalid XML character reference: the scalar is forbidden in XML 1.0".into());
    }

    let name = reference.as_ref();
    match name {
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

pub fn xml_validation_error(message: impl Into<String>) -> Error {
    Error::svg_postprocess(XML_VALIDATION_PASS, message)
}
