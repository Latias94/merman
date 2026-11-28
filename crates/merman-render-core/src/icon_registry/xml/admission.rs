use super::super::error::IconRegistryBuildError;
use super::super::limits::{IconRegistryBuildLimits, IconRegistryResourceLimitId};
use super::projection::{RewriteSources, XmlProjection};
use super::{SVG_NAMESPACE, XLINK_NAMESPACE, arithmetic_error, invalid_xml_error, limit_error};
use quick_xml::XmlVersion;
use quick_xml::escape::resolve_xml_entity;
use quick_xml::events::attributes::Attributes;
use quick_xml::events::{BytesRef, Event};
use quick_xml::name::{NamespaceResolver, ResolveResult};
use quick_xml::reader::NsReader;
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
) -> Result<(XmlStats, Option<RewriteSources<'xml>>), IconRegistryBuildError> {
    let mut reader = NsReader::from_str(wrapped);
    reader.config_mut().enable_all_checks(true);

    let mut stats = XmlStats::default();
    let mut projection = (remaining_build_work > 0).then(|| XmlProjection::new(wrapped));
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
                let (namespace, local_name) = reader.resolver().resolve_element(element.name());
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
                    // The next admitted element and the wrapper each cost one unit. Keep
                    // checking the stream for its own first error after that budget is exceeded.
                    if stats.element_count.saturating_add(1) >= remaining_build_work {
                        projection = None;
                    }
                }

                let tag = &wrapped[start + 1..start + 1 + element.len()];
                debug_assert_eq!(tag, &*element);
                let name = &tag[..element.name().as_ref().len()];
                observe(&mut projection, |xml| {
                    xml.begin_element(start + 1 + name.len())
                });
                let uses_xlink = validate_element(
                    tag,
                    name,
                    namespace,
                    reader.resolver(),
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
                if !empty {
                    depth = depth.checked_add(1).ok_or_else(|| {
                        arithmetic_error(pack_index, "icon XML nesting depth overflowed")
                    })?;
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
                let content = text.xml10_content();
                validate_xml_chars(&content, pack_index, "icon body contains invalid XML text")?;
                if let Some(xml) = projection.as_mut() {
                    xml.text(start..end);
                }
            }
            Event::CData(text) => {
                ensure_inside_wrapper(depth, root_closed, pack_index)?;
                let content = text.xml10_content();
                validate_xml_chars(&content, pack_index, "icon body contains invalid CDATA")?;
                observe(&mut projection, |xml| xml.cdata(start..end, content));
            }
            Event::GeneralRef(reference) => {
                ensure_inside_wrapper(depth, root_closed, pack_index)?;
                validate_general_reference(&reference, pack_index)?;
                if let Some(xml) = projection.as_mut() {
                    xml.text(start..end);
                }
            }
            Event::Comment(comment) => {
                ensure_inside_wrapper(depth, root_closed, pack_index)?;
                let content = comment.xml10_content();
                validate_xml_chars(
                    &content,
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
    projection: &mut Option<XmlProjection<'xml>>,
    event: impl FnOnce(&mut XmlProjection<'xml>) -> Option<()>,
) {
    if let Some(xml) = projection
        && event(xml).is_none()
    {
        *projection = None;
    }
}

fn validate_element<'xml>(
    tag: &'xml str,
    name: &str,
    namespace: ResolveResult<'_>,
    resolver: &NamespaceResolver,
    is_wrapper: bool,
    pack_index: usize,
    projection: &mut Option<XmlProjection<'xml>>,
) -> Result<bool, IconRegistryBuildError> {
    let mut uses_xlink = matches!(
        &namespace,
        ResolveResult::Bound(namespace) if namespace.as_ref() == XLINK_NAMESPACE
    ) && !is_wrapper;
    match namespace {
        ResolveResult::Unknown(_) => {
            return Err(invalid_xml_error(
                pack_index,
                "icon body uses an undeclared namespace prefix",
            ));
        }
        ResolveResult::Bound(namespace) if is_wrapper && namespace.as_ref() != SVG_NAMESPACE => {
            return Err(invalid_xml_error(
                pack_index,
                "icon XML wrapper uses a non-SVG namespace",
            ));
        }
        ResolveResult::Unbound if is_wrapper => {
            return Err(invalid_xml_error(
                pack_index,
                "icon XML wrapper is missing the SVG namespace",
            ));
        }
        ResolveResult::Unbound | ResolveResult::Bound(_) => {}
    }

    validate_xml_qname(name, pack_index)?;
    let mut expanded_attributes = HashSet::new();

    for attribute in Attributes::new(tag, name.len()) {
        let attribute = attribute.map_err(|_| {
            invalid_xml_error(pack_index, "icon body contains an invalid XML attribute")
        })?;
        validate_xml_qname(attribute.key.as_ref(), pack_index)?;
        if attribute.value.contains('<') {
            return Err(invalid_xml_error(
                pack_index,
                "icon body contains an invalid XML attribute value",
            ));
        }
        let normalized =
            match attribute.normalized_value_with(XmlVersion::Implicit1_0, 1, resolve_xml_entity) {
                Ok(value) => value,
                Err(_) => {
                    // Cargo feature unification can enable HTML entities in the reader. Preserve
                    // its streaming error order while rejecting those entities at the strict gate.
                    *projection = None;
                    attribute
                        .normalized_value(XmlVersion::Implicit1_0)
                        .map_err(|_| {
                            invalid_xml_error(
                                pack_index,
                                "icon body contains an invalid XML attribute value",
                            )
                        })?
                }
            };
        validate_xml_chars(
            &normalized,
            pack_index,
            "icon body contains an invalid XML attribute value",
        )?;
        observe(projection, |xml| {
            xml.attribute(attribute.key.into_inner(), &attribute.value, normalized)
        });

        if attribute.key.as_namespace_binding().is_some() {
            continue;
        }

        let (attribute_namespace, local_name) = resolver.resolve_attribute(attribute.key);
        let attribute_namespace = match attribute_namespace {
            ResolveResult::Unknown(_) => {
                return Err(invalid_xml_error(
                    pack_index,
                    "icon body uses an undeclared attribute namespace prefix",
                ));
            }
            ResolveResult::Bound(namespace) => {
                uses_xlink |= namespace.as_ref() == XLINK_NAMESPACE;
                Some(namespace.into_inner())
            }
            ResolveResult::Unbound => None,
        };
        if !expanded_attributes.insert((attribute_namespace, local_name.into_inner())) {
            return Err(invalid_xml_error(
                pack_index,
                "icon body contains duplicate expanded XML attributes",
            ));
        }
    }

    Ok(uses_xlink)
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
