//! Source ranges and normalized names used by the icon rewrite planner.

use super::{SVG_NAMESPACE, XLINK_NAMESPACE, decode_xml_reference};
use std::borrow::Cow;
use std::collections::HashSet;
use std::ops::Range;

const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";
const XMLNS_NAMESPACE: &str = "http://www.w3.org/2000/xmlns/";

#[derive(Default)]
pub(super) struct RewriteSources<'xml> {
    pub(super) defs: Vec<Range<usize>>,
    pub(super) attributes: Vec<RewriteAttribute<'xml>>,
    pub(super) style_text: Vec<(Range<usize>, Cow<'xml, str>)>,
}

pub(super) struct RewriteAttribute<'xml> {
    pub(super) kind: AttributeKind,
    pub(super) value: Cow<'xml, str>,
    pub(super) value_range: Range<usize>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum AttributeKind {
    Id,
    Fragment,
    Smil,
    IdRefList,
    CssUrl,
}

impl AttributeKind {
    fn for_name(namespace: Option<&str>, local: &str) -> Option<Self> {
        let kind = match (namespace, local) {
            (None, "id") => Self::Id,
            (None | Some(XLINK_NAMESPACE), "href") => Self::Fragment,
            (None, "begin" | "end") => Self::Smil,
            (
                None,
                "aria-activedescendant"
                | "aria-controls"
                | "aria-describedby"
                | "aria-details"
                | "aria-errormessage"
                | "aria-flowto"
                | "aria-labelledby"
                | "aria-owns",
            ) => Self::IdRefList,
            (
                None,
                "clip-path" | "color-profile" | "cursor" | "fill" | "filter" | "marker"
                | "marker-end" | "marker-mid" | "marker-start" | "mask" | "stroke" | "style",
            ) => Self::CssUrl,
            _ => return None,
        };
        Some(kind)
    }
}

struct Namespace<'xml> {
    prefix: &'xml str,
    uri: Cow<'xml, str>,
}

struct OpenElement {
    scope: usize,
    hoisted_defs_start: Option<usize>,
    style: bool,
}

struct Attribute<'xml> {
    prefix: &'xml str,
    local: &'xml str,
    value: Cow<'xml, str>,
    value_range: Range<usize>,
}

/// Observes one event stream alongside the raw namespace and structural checks. The caller
/// reports strict projection failures only after structural admission and its work charge.
pub(super) struct XmlProjection<'xml> {
    document: &'xml str,
    sources: RewriteSources<'xml>,
    namespaces: Vec<Namespace<'xml>>,
    elements: Vec<OpenElement>,
    attributes: Vec<Attribute<'xml>>,
    scope_start: usize,
    previous_attribute_end: usize,
    inside_hoisted_defs: bool,
    pending_text: Option<Range<usize>>,
    style_run: Option<(Range<usize>, Cow<'xml, str>)>,
}

impl<'xml> XmlProjection<'xml> {
    pub(super) fn new(document: &'xml str) -> Self {
        Self {
            document,
            sources: RewriteSources::default(),
            namespaces: Vec::new(),
            elements: Vec::new(),
            attributes: Vec::new(),
            scope_start: 0,
            previous_attribute_end: 0,
            inside_hoisted_defs: false,
            pending_text: None,
            style_run: None,
        }
    }

    pub(super) fn into_sources(self) -> RewriteSources<'xml> {
        self.sources
    }

    pub(super) fn begin_element(&mut self, name_end: usize) -> Option<()> {
        self.break_text()?;
        self.previous_attribute_end = name_end;
        self.scope_start = self.namespaces.len();
        Some(())
    }

    pub(super) fn attribute(
        &mut self,
        name: &'xml str,
        raw_value: &str,
        value: Cow<'xml, str>,
    ) -> Option<()> {
        // These slices come from the same source-backed quick-xml attribute iterator.
        let document_start = self.document.as_ptr() as usize;
        let start = (name.as_ptr() as usize).checked_sub(document_start)?;
        let value_start = (raw_value.as_ptr() as usize).checked_sub(document_start)?;
        if start == self.previous_attribute_end {
            return None;
        }
        self.previous_attribute_end = value_start + raw_value.len() + 1;
        let (prefix, local) = split_name(name);
        if prefix == "xmlns" {
            if value == XMLNS_NAMESPACE || (local == "xml") != (value == XML_NAMESPACE) {
                return None;
            }
            if local != "xml" {
                self.namespaces.push(Namespace {
                    prefix: local,
                    uri: value,
                });
            }
        } else if local == "xmlns" {
            if matches!(value.as_ref(), XML_NAMESPACE | XMLNS_NAMESPACE) {
                return None;
            }
            // The previous strict parser treats a qualified local `xmlns` as a default
            // declaration too and uses the first default declared on this element.
            if !self.namespaces[self.scope_start..]
                .iter()
                .any(|ns| ns.prefix.is_empty())
            {
                self.namespaces.push(Namespace {
                    prefix: "",
                    uri: value,
                });
            }
        } else {
            // Preserve historical source ranges for declarations wider than the strict
            // parser's stored name length or equals span.
            let equals_len = value_start - start - name.len() - 1;
            self.attributes.push(Attribute {
                prefix,
                local,
                value,
                value_range: start
                    + name.len().min(u16::MAX as usize)
                    + equals_len.min(u8::MAX as usize)
                    + 1..value_start + raw_value.len(),
            });
        }
        Some(())
    }

    pub(super) fn finish_element(
        &mut self,
        name: &str,
        range: Range<usize>,
        empty: bool,
    ) -> Option<()> {
        let (prefix, local) = split_name(name);
        let namespace = resolve_namespace(&self.namespaces, prefix);
        if !prefix.is_empty() && namespace.is_none() {
            return None;
        }
        let style = namespace == Some(SVG_NAMESPACE) && local == "style";
        let hoist_defs =
            namespace == Some(SVG_NAMESPACE) && local == "defs" && !self.inside_hoisted_defs;

        // Raw expanded names were already checked. Normalized namespace URIs can cause
        // two different prefixes to resolve to the same expanded attribute name.
        let mut prefixed_names = HashSet::new();
        for attribute in self.attributes.drain(..) {
            let namespace = match attribute.prefix {
                "" => None,
                "xml" => Some(XML_NAMESPACE),
                prefix => Some(resolve_namespace(&self.namespaces, prefix)?),
            };
            if let Some(uri) = namespace
                && !prefixed_names.insert((uri, attribute.local))
            {
                return None;
            }
            if let Some(kind) = AttributeKind::for_name(namespace, attribute.local) {
                self.sources.attributes.push(RewriteAttribute {
                    kind,
                    value: attribute.value,
                    value_range: attribute.value_range,
                });
            }
        }

        if empty {
            if hoist_defs {
                self.sources.defs.push(range);
            }
            self.namespaces.truncate(self.scope_start);
        } else {
            self.inside_hoisted_defs |= hoist_defs;
            self.elements.push(OpenElement {
                scope: self.scope_start,
                hoisted_defs_start: hoist_defs.then_some(range.start),
                style,
            });
        }
        Some(())
    }

    pub(super) fn close_element(&mut self, end: usize) -> Option<()> {
        self.break_text()?;
        let element = self.elements.pop()?;
        if let Some(start) = element.hoisted_defs_start {
            self.sources.defs.push(start..end);
            self.inside_hoisted_defs = false;
        }
        self.namespaces.truncate(element.scope);
        Some(())
    }

    pub(super) fn text(&mut self, range: Range<usize>) {
        if self.in_style() {
            // Character references are separate reader events, but share a single text span.
            match &mut self.pending_text {
                Some(pending) => pending.end = range.end,
                None => self.pending_text = Some(range),
            }
        }
    }

    pub(super) fn cdata(&mut self, range: Range<usize>, value: Cow<'xml, str>) -> Option<()> {
        if self.in_style() {
            self.finish_text()?;
            self.style_text(range, value);
        }
        Some(())
    }

    pub(super) fn break_text(&mut self) -> Option<()> {
        self.finish_text()?;
        if let Some(run) = self.style_run.take() {
            self.sources.style_text.push(run);
        }
        Some(())
    }

    fn in_style(&self) -> bool {
        self.elements.last().is_some_and(|element| element.style)
    }

    fn finish_text(&mut self) -> Option<()> {
        if let Some(range) = self.pending_text.take() {
            let value = normalized_style_text(&self.document[range.clone()])?;
            self.style_text(range, value);
        }
        Some(())
    }

    fn style_text(&mut self, range: Range<usize>, value: Cow<'xml, str>) {
        if let Some((_, previous)) = self.style_run.as_mut() {
            // Adjacent text and CDATA shared a value but used the first token's range.
            previous.to_mut().push_str(&value);
        } else {
            self.style_run = Some((range, value));
        }
    }
}

fn split_name(name: &str) -> (&str, &str) {
    name.split_once(':').unwrap_or(("", name))
}

fn resolve_namespace<'a>(namespaces: &'a [Namespace<'_>], prefix: &str) -> Option<&'a str> {
    namespaces
        .iter()
        .rfind(|ns| ns.prefix == prefix)
        .map(|ns| ns.uri.as_ref())
}

fn normalized_style_text(raw: &str) -> Option<Cow<'_, str>> {
    if !raw.contains(['&', '\r']) {
        return Some(Cow::Borrowed(raw));
    }

    let mut result = String::with_capacity(raw.len());
    let mut offset = 0;
    let mut after_reference = false;
    while offset < raw.len() {
        if raw.as_bytes()[offset] == b'&' {
            let (character, end) = decode_xml_reference(raw, offset)?;
            result.push(character);
            offset = end;
            after_reference = true;
            continue;
        }

        let character = raw[offset..].chars().next()?;
        offset += character.len_utf8();
        if after_reference {
            // The strict parser copied the byte after an expanded reference verbatim.
            result.push(character);
            after_reference = false;
        } else {
            if result.ends_with('\r') {
                result.pop();
                result.push('\n');
                if character == '\n' {
                    continue;
                }
            }
            result.push(if character == '\r' && offset == raw.len() {
                '\n'
            } else {
                character
            });
        }
    }
    Some(Cow::Owned(result))
}
