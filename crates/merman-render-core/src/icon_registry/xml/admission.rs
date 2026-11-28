use super::super::error::IconRegistryBuildError;
use super::super::limits::{IconRegistryBuildLimits, IconRegistryResourceLimitId};
use super::projection::{
    Namespace, NamespaceScopes, ProjectionError, ProjectionResult, RewriteSources, XmlProjection,
};
use super::{allocation_error, arithmetic_error, invalid_xml_error, limit_error};
use quick_xml::escape::{resolve_predefined_entity, resolve_xml_entity};
use quick_xml::events::attributes::Attributes;
use quick_xml::events::{BytesRef, Event};
use quick_xml::name::{Prefix, PrefixDeclaration, QName};
use quick_xml::reader::NsReader;
use std::borrow::Cow;
use std::collections::HashSet;

#[derive(Debug, Default)]
pub(super) struct XmlStats {
    pub(super) element_count: usize,
    pub(super) uses_xlink: bool,
}

pub(super) fn admit_wrapped_fragment<'xml>(
    wrapped: &'xml str,
    pack_index: usize,
    limits: &IconRegistryBuildLimits,
    remaining_build_work: usize,
) -> Result<(XmlStats, ProjectionResult<RewriteSources<'xml>>), IconRegistryBuildError> {
    let mut reader = NsReader::from_str(wrapped);
    reader.config_mut().enable_all_checks(true);

    let mut stats = XmlStats::default();
    let mut projection = Ok(XmlProjection::new(wrapped, remaining_build_work));
    let mut raw_namespaces = NamespaceScopes::default();
    let mut raw_scopes = Vec::new();
    let mut depth = 0usize;
    let mut root_seen = false;
    let mut root_closed = false;

    loop {
        let start = reader.buffer_position() as usize;
        let event = reader
            .read_event()
            .map_err(|_| invalid_xml_error(pack_index, "icon body is not well-formed XML"))?;
        let end = reader.buffer_position() as usize;
        let empty = matches!(&event, Event::Empty(_));
        match event {
            Event::Start(element) | Event::Empty(element) => {
                let is_wrapper = !root_seen;
                if is_wrapper {
                    if empty {
                        return Err(invalid_xml_error(
                            pack_index,
                            "icon XML wrapper root is unexpectedly empty",
                        ));
                    }
                    if depth != 0 || root_closed {
                        return Err(invalid_xml_error(
                            pack_index,
                            "icon XML wrapper has an invalid root structure",
                        ));
                    }
                } else {
                    ensure_inside_wrapper(depth, root_closed, pack_index)?;
                }

                let tag = &wrapped[start + 1..start + 1 + element.len()];
                debug_assert_eq!(tag, &*element);
                let name = &tag[..element.name().as_ref().len()];
                observe(&mut projection, |xml| {
                    xml.begin_element(start + 1 + name.len())
                });
                let scope = raw_namespaces.scope_start();
                collect_raw_namespaces(
                    tag,
                    name.len(),
                    &mut raw_namespaces,
                    pack_index,
                    &mut projection,
                )?;
                let (local_name, prefix) = QName(name).decompose();
                let namespace = resolve_raw_namespace(&raw_namespaces, prefix, true, pack_index)?;
                let uses_xlink = validate_element(
                    tag,
                    name,
                    namespace,
                    &raw_namespaces,
                    is_wrapper,
                    pack_index,
                    &mut projection,
                )?;
                if is_wrapper {
                    if local_name.as_ref() != "svg" {
                        return Err(invalid_xml_error(
                            pack_index,
                            "icon XML wrapper root is not SVG",
                        ));
                    }
                } else {
                    stats.uses_xlink |= uses_xlink;
                    record_element(&mut stats, depth, pack_index, limits)?;
                }
                root_seen = true;
                if empty {
                    raw_namespaces.restore(scope);
                } else {
                    depth = depth.checked_add(1).ok_or_else(|| {
                        arithmetic_error(pack_index, "icon XML nesting depth overflowed")
                    })?;
                    raw_scopes.try_reserve(1).map_err(|_| {
                        allocation_error(pack_index, "icon XML namespace scope allocation failed")
                    })?;
                    raw_scopes.push(scope);
                }
                observe(&mut projection, |xml| {
                    xml.finish_element(name, start..end, empty)
                });
            }
            Event::End(_) => {
                if !root_seen || root_closed {
                    return Err(invalid_xml_error(
                        pack_index,
                        "icon body closes an element outside its XML wrapper",
                    ));
                }
                depth = depth.checked_sub(1).ok_or_else(|| {
                    invalid_xml_error(pack_index, "icon body contains an unmatched end tag")
                })?;
                let scope = raw_scopes.pop().ok_or_else(|| {
                    invalid_xml_error(pack_index, "icon body contains an unmatched end tag")
                })?;
                raw_namespaces.restore(scope);
                if depth == 0 {
                    root_closed = true;
                }
                observe(&mut projection, |xml| xml.close_element(end));
            }
            Event::Text(text) => {
                ensure_inside_wrapper(depth, root_closed, pack_index)?;
                if text.contains("]]>") {
                    return Err(invalid_xml_error(
                        pack_index,
                        "icon body contains a forbidden XML text terminator",
                    ));
                }
                validate_xml_chars(&text, pack_index, "icon body contains invalid XML text")?;
                if let Ok(xml) = projection.as_mut() {
                    xml.text(start..end);
                }
            }
            Event::CData(text) => {
                ensure_inside_wrapper(depth, root_closed, pack_index)?;
                validate_xml_chars(&text, pack_index, "icon body contains invalid CDATA")?;
                observe(&mut projection, |xml| xml.cdata(start..end));
            }
            Event::GeneralRef(reference) => {
                ensure_inside_wrapper(depth, root_closed, pack_index)?;
                validate_general_reference(&reference, pack_index)?;
                if let Ok(xml) = projection.as_mut() {
                    xml.text(start..end);
                }
            }
            Event::Comment(comment) => {
                ensure_inside_wrapper(depth, root_closed, pack_index)?;
                validate_xml_chars(
                    &comment,
                    pack_index,
                    "icon body contains an invalid XML comment",
                )?;
                observe(&mut projection, XmlProjection::break_text);
            }
            Event::Decl(_) => {
                return Err(invalid_xml_error(
                    pack_index,
                    "XML declarations are not allowed in icon bodies",
                ));
            }
            Event::PI(_) => {
                return Err(invalid_xml_error(
                    pack_index,
                    "processing instructions are not allowed in icon bodies",
                ));
            }
            Event::DocType(_) => {
                return Err(invalid_xml_error(
                    pack_index,
                    "document type and entity declarations are not allowed in icon bodies",
                ));
            }
            Event::Eof => break,
        }
    }

    if !root_seen || !root_closed || depth != 0 {
        return Err(invalid_xml_error(
            pack_index,
            "icon XML wrapper is not completely closed",
        ));
    }
    Ok((stats, projection.map(XmlProjection::into_sources)))
}

fn observe<'xml>(
    projection: &mut ProjectionResult<XmlProjection<'xml>>,
    event: impl FnOnce(&mut XmlProjection<'xml>) -> ProjectionResult<()>,
) {
    if let Ok(xml) = projection
        && let Err(error) = event(xml)
    {
        *projection = Err(error);
    }
}

fn collect_raw_namespaces<'xml>(
    tag: &'xml str,
    name_len: usize,
    namespaces: &mut NamespaceScopes<'xml>,
    pack_index: usize,
    projection: &mut ProjectionResult<XmlProjection<'xml>>,
) -> Result<(), IconRegistryBuildError> {
    // NsReader has checked its raw declaration rules and active-binding ceiling. Reuse the
    // same lexical iteration for source-backed keys and values. Raw names see the entire tag
    // up to the first lexer error, even when a declaration follows the name that uses it.
    for attribute in Attributes::new(tag, name_len).with_checks(false) {
        let Ok(attribute) = attribute else { break };
        let raw_binding = attribute.key.as_namespace_binding();
        if raw_binding.is_some() {
            observe(projection, XmlProjection::preflight_tag_item);
        }
        let prefix = match raw_binding {
            Some(PrefixDeclaration::Default) => "",
            Some(PrefixDeclaration::Named("xml")) | None => continue,
            Some(PrefixDeclaration::Named(prefix)) => prefix,
        };
        namespaces.bind(prefix, attribute.value).map_err(|_| {
            allocation_error(pack_index, "icon XML namespace index allocation failed")
        })?;
    }
    Ok(())
}

fn resolve_raw_namespace(
    namespaces: &NamespaceScopes<'_>,
    prefix: Option<Prefix<'_>>,
    use_default: bool,
    pack_index: usize,
) -> Result<Option<Namespace>, IconRegistryBuildError> {
    let Some(prefix) = prefix else {
        return Ok(if use_default {
            namespaces
                .lookup("")
                .filter(|namespace| *namespace != Namespace::Empty)
        } else {
            None
        });
    };
    let namespace = match prefix.into_inner() {
        "xml" => Some(Namespace::Xml),
        "xmlns" => Some(Namespace::Xmlns),
        "" => None,
        prefix => namespaces
            .lookup(prefix)
            .filter(|namespace| *namespace != Namespace::Empty),
    };
    namespace.map(Some).ok_or_else(|| {
        invalid_xml_error(
            pack_index,
            if use_default {
                "icon body uses an undeclared namespace prefix"
            } else {
                "icon body uses an undeclared attribute namespace prefix"
            },
        )
    })
}

fn validate_element<'xml>(
    tag: &'xml str,
    name: &str,
    namespace: Option<Namespace>,
    namespaces: &NamespaceScopes<'_>,
    is_wrapper: bool,
    pack_index: usize,
    projection: &mut ProjectionResult<XmlProjection<'xml>>,
) -> Result<bool, IconRegistryBuildError> {
    let mut uses_xlink = namespace == Some(Namespace::Xlink) && !is_wrapper;
    match namespace {
        Some(namespace) if is_wrapper && namespace != Namespace::Svg => {
            return Err(invalid_xml_error(
                pack_index,
                "icon XML wrapper uses a non-SVG namespace",
            ));
        }
        None if is_wrapper => {
            return Err(invalid_xml_error(
                pack_index,
                "icon XML wrapper is missing the SVG namespace",
            ));
        }
        _ => {}
    }

    validate_xml_qname(name, pack_index)?;
    let mut raw_attributes = HashSet::new();
    let mut expanded_attributes = HashSet::new();

    for attribute in Attributes::new(tag, name.len()).with_checks(false) {
        let attribute = attribute.map_err(|_| {
            invalid_xml_error(pack_index, "icon body contains an invalid XML attribute")
        })?;
        observe(projection, XmlProjection::preflight_tag_item);
        raw_attributes.try_reserve(1).map_err(|_| {
            allocation_error(pack_index, "icon XML attribute name allocation failed")
        })?;
        if !raw_attributes.insert(attribute.key.into_inner()) {
            return Err(invalid_xml_error(
                pack_index,
                "icon body contains an invalid XML attribute",
            ));
        }
        validate_xml_qname(attribute.key.as_ref(), pack_index)?;
        let Cow::Borrowed(raw_value) = attribute.value else {
            // Attributes::new slices the original tag, including for empty values.
            return Err(invalid_attribute_value(pack_index));
        };
        if validate_attribute_value(raw_value, pack_index)? {
            observe(projection, |xml| {
                xml.attribute(attribute.key.into_inner(), raw_value)
            });
        } else {
            observe(projection, |_| Err(ProjectionError::InvalidXml));
        }

        if attribute.key.as_namespace_binding().is_some() {
            continue;
        }

        let (local_name, prefix) = attribute.key.decompose();
        let attribute_namespace = resolve_raw_namespace(namespaces, prefix, false, pack_index)?;
        uses_xlink |= attribute_namespace == Some(Namespace::Xlink);
        expanded_attributes.try_reserve(1).map_err(|_| {
            allocation_error(
                pack_index,
                "icon XML expanded attribute name allocation failed",
            )
        })?;
        if !expanded_attributes.insert((attribute_namespace, local_name.into_inner())) {
            return Err(invalid_xml_error(
                pack_index,
                "icon body contains duplicate expanded XML attributes",
            ));
        }
    }

    Ok(uses_xlink)
}

fn invalid_attribute_value(pack_index: usize) -> IconRegistryBuildError {
    invalid_xml_error(
        pack_index,
        "icon body contains an invalid XML attribute value",
    )
}

/// Returns whether the value also passes the XML-only entity check. Raw admission
/// needs validity, so it can leave whitespace unchanged and check references in place.
fn validate_attribute_value(
    mut raw: &str,
    pack_index: usize,
) -> Result<bool, IconRegistryBuildError> {
    let valid_char = crate::xml_chars::is_xml_1_0_char;
    if raw.contains('<') || !raw.chars().all(valid_char) {
        return Err(invalid_attribute_value(pack_index));
    }

    let mut strict = true;
    while let Some((_, after_amp)) = raw.split_once('&') {
        let (reference, rest) = after_amp
            .split_once(';')
            .ok_or_else(|| invalid_attribute_value(pack_index))?;
        if reference.starts_with('#') {
            let character = BytesRef::new(reference)
                .resolve_char_ref()
                .map_err(|_| invalid_attribute_value(pack_index))?;
            if !character.is_some_and(valid_char) {
                return Err(invalid_attribute_value(pack_index));
            }
        } else if resolve_xml_entity(reference).is_none() {
            // The reader supports HTML names when escape-html is unified. Its depth-one
            // normalization only accepts replacement text that needs no further decoding.
            strict = false;
            let replacement = resolve_predefined_entity(reference)
                .ok_or_else(|| invalid_attribute_value(pack_index))?;
            if replacement.contains(['&', '\t', '\r', '\n']) || !replacement.chars().all(valid_char)
            {
                return Err(invalid_attribute_value(pack_index));
            }
        }
        raw = rest;
    }
    Ok(strict)
}

fn record_element(
    stats: &mut XmlStats,
    fragment_depth: usize,
    pack_index: usize,
    limits: &IconRegistryBuildLimits,
) -> Result<(), IconRegistryBuildError> {
    let elements = stats
        .element_count
        .checked_add(1)
        .ok_or_else(|| arithmetic_error(pack_index, "icon XML element count overflowed"))?;
    if elements > limits.max_xml_elements_per_body {
        return Err(limit_error(
            pack_index,
            IconRegistryResourceLimitId::MaxXmlElementsPerBody,
            elements,
            limits.max_xml_elements_per_body,
            "icon body exceeds the XML element limit",
        ));
    }
    if fragment_depth > limits.max_xml_depth_per_body {
        return Err(limit_error(
            pack_index,
            IconRegistryResourceLimitId::MaxXmlDepthPerBody,
            fragment_depth,
            limits.max_xml_depth_per_body,
            "icon body exceeds the XML nesting-depth limit",
        ));
    }

    stats.element_count = elements;
    Ok(())
}

fn ensure_inside_wrapper(
    depth: usize,
    root_closed: bool,
    pack_index: usize,
) -> Result<(), IconRegistryBuildError> {
    if depth == 0 || root_closed {
        return Err(invalid_xml_error(
            pack_index,
            "icon body contains XML outside its fragment wrapper",
        ));
    }
    Ok(())
}

fn validate_xml_chars(
    content: &str,
    pack_index: usize,
    message: &'static str,
) -> Result<(), IconRegistryBuildError> {
    if content.chars().all(crate::xml_chars::is_xml_1_0_char) {
        Ok(())
    } else {
        Err(invalid_xml_error(pack_index, message))
    }
}

fn validate_general_reference(
    reference: &BytesRef<'_>,
    pack_index: usize,
) -> Result<(), IconRegistryBuildError> {
    if let Some(value) = reference.resolve_char_ref().map_err(|_| {
        invalid_xml_error(
            pack_index,
            "icon body contains an invalid XML character reference",
        )
    })? {
        if crate::xml_chars::is_xml_1_0_char(value) {
            return Ok(());
        }
        return Err(invalid_xml_error(
            pack_index,
            "icon body contains an XML 1.0-forbidden character reference",
        ));
    }

    if matches!(reference.as_ref(), "amp" | "apos" | "gt" | "lt" | "quot") {
        Ok(())
    } else {
        Err(invalid_xml_error(
            pack_index,
            "icon body contains an undeclared XML entity reference",
        ))
    }
}

fn validate_xml_qname(name: &str, pack_index: usize) -> Result<(), IconRegistryBuildError> {
    let mut components = name.split(':');
    let first = components.next().unwrap_or_default();
    let second = components.next();
    if components.next().is_some()
        || !is_valid_xml_ncname(first)
        || second.is_some_and(|component| !is_valid_xml_ncname(component))
    {
        return Err(invalid_xml_error(
            pack_index,
            "icon body contains an invalid XML qualified name",
        ));
    }
    Ok(())
}

fn is_valid_xml_ncname(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(is_xml_name_start_char)
        && chars.all(|character| character != ':' && is_xml_name_char(character))
}

fn is_xml_name_start_char(character: char) -> bool {
    matches!(
        character,
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

fn is_xml_name_char(character: char) -> bool {
    is_xml_name_start_char(character)
        || matches!(
            character,
            '-' | '.' | '0'..='9' | '\u{b7}' | '\u{300}'..='\u{36f}' | '\u{203f}'..='\u{2040}'
        )
}
