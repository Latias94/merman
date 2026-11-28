//! Source ranges and normalized names used by the icon rewrite planner.

use super::{DecodeMode, SVG_NAMESPACE, XLINK_NAMESPACE, next_decoded_char};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet, TryReserveError};
use std::ops::Range;

const XML_NAMESPACE: &str = "http://www.w3.org/XML/1998/namespace";
const XMLNS_NAMESPACE: &str = "http://www.w3.org/2000/xmlns/";

pub(super) type ProjectionResult<T> = Result<T, ProjectionError>;

#[derive(Debug)]
pub(super) enum ProjectionError {
    InvalidXml,
    AllocationFailed,
    WorkOverflow,
    WorkLimit(usize),
}

impl From<TryReserveError> for ProjectionError {
    fn from(_: TryReserveError) -> Self {
        Self::AllocationFailed
    }
}

#[derive(Default)]
pub(super) struct RewriteSources<'xml> {
    pub(super) defs: Vec<Range<usize>>,
    pub(super) attributes: Vec<RewriteAttribute<'xml>>,
    pub(super) style_text: Vec<(Range<usize>, Cow<'xml, str>)>,
    pub(super) work_units: usize,
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
    fn for_name(namespace: Option<Namespace>, local: &str) -> Option<Self> {
        let kind = match (namespace, local) {
            (None, "id") => Self::Id,
            (None | Some(Namespace::Xlink), "href") => Self::Fragment,
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

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Namespace {
    Empty,
    Svg,
    Xlink,
    Xml,
    Xmlns,
    Other(usize),
}

struct NamespaceBinding<'xml> {
    prefix: &'xml str,
    namespace: Namespace,
    previous: Option<usize>,
}

/// One namespace view. Raw and normalized bindings use separate instances.
#[derive(Default)]
pub(super) struct NamespaceScopes<'xml> {
    bindings: Vec<NamespaceBinding<'xml>>,
    active: HashMap<&'xml str, usize>,
    uris: HashMap<Cow<'xml, str>, usize>,
}

impl<'xml> NamespaceScopes<'xml> {
    pub(super) fn scope_start(&self) -> usize {
        self.bindings.len()
    }

    pub(super) fn bind(
        &mut self,
        prefix: &'xml str,
        uri: Cow<'xml, str>,
    ) -> Result<(), TryReserveError> {
        let namespace = match uri.as_ref() {
            "" => Namespace::Empty,
            SVG_NAMESPACE => Namespace::Svg,
            XLINK_NAMESPACE => Namespace::Xlink,
            XML_NAMESPACE => Namespace::Xml,
            XMLNS_NAMESPACE => Namespace::Xmlns,
            uri_text => {
                // Expanded-name comparisons use this identity. A long URI is hashed here,
                // rather than every time an attribute uses its prefix.
                if let Some(&index) = self.uris.get(uri_text) {
                    Namespace::Other(index)
                } else {
                    let index = self.uris.len();
                    self.uris.try_reserve(1)?;
                    self.uris.insert(uri, index);
                    Namespace::Other(index)
                }
            }
        };
        let index = self.bindings.len();
        self.bindings.try_reserve(1)?;
        let previous = if let Some(active) = self.active.get_mut(prefix) {
            Some(std::mem::replace(active, index))
        } else {
            self.active.try_reserve(1)?;
            self.active.insert(prefix, index);
            None
        };
        self.bindings.push(NamespaceBinding {
            prefix,
            namespace,
            previous,
        });
        Ok(())
    }

    pub(super) fn lookup(&self, prefix: &str) -> Option<Namespace> {
        self.active
            .get(prefix)
            .map(|&index| self.bindings[index].namespace)
    }

    pub(super) fn restore(&mut self, scope: usize) {
        for binding in self.bindings.drain(scope..).rev() {
            if let Some(previous) = binding.previous {
                *self
                    .active
                    .get_mut(binding.prefix)
                    .expect("an active binding has a shadowed predecessor") = previous;
            } else {
                self.active.remove(binding.prefix);
            }
        }
    }
}

struct ProjectionWork {
    remaining: usize,
    structure: usize,
    used: usize,
}

impl ProjectionWork {
    fn charge(&mut self, units: usize) -> ProjectionResult<()> {
        let next = self
            .used
            .checked_add(units)
            .ok_or(ProjectionError::WorkOverflow)?;
        if self
            .remaining
            .checked_sub(self.structure)
            .is_none_or(|available| next > available)
        {
            return Err(ProjectionError::WorkLimit(next));
        }
        self.used = next;
        Ok(())
    }
}

struct OpenElement {
    scope: usize,
    hoisted_defs_start: Option<usize>,
    style: bool,
}

struct Attribute<'xml> {
    prefix: &'xml str,
    local: &'xml str,
    raw_value: &'xml str,
    value_range: Range<usize>,
}

/// Observes one event stream alongside the raw namespace and structural checks. The caller
/// reports strict projection failures only after structural admission and its work charge.
pub(super) struct XmlProjection<'xml> {
    document: &'xml str,
    sources: RewriteSources<'xml>,
    namespaces: NamespaceScopes<'xml>,
    elements: Vec<OpenElement>,
    attributes: Vec<Attribute<'xml>>,
    work: ProjectionWork,
    scope_start: usize,
    previous_attribute_end: usize,
    inside_hoisted_defs: bool,
    pending_text: Option<Range<usize>>,
    style_run: Option<(Range<usize>, Cow<'xml, str>)>,
}

impl<'xml> XmlProjection<'xml> {
    pub(super) fn new(document: &'xml str, remaining_build_work: usize) -> Self {
        Self {
            document,
            sources: RewriteSources::default(),
            namespaces: NamespaceScopes::default(),
            elements: Vec::new(),
            attributes: Vec::new(),
            work: ProjectionWork {
                remaining: remaining_build_work,
                structure: 0,
                used: 0,
            },
            scope_start: 0,
            previous_attribute_end: 0,
            inside_hoisted_defs: false,
            pending_text: None,
            style_run: None,
        }
    }

    pub(super) fn into_sources(mut self) -> RewriteSources<'xml> {
        self.sources.work_units = self.work.used;
        self.sources
    }

    pub(super) fn begin_element(&mut self, name_end: usize) -> ProjectionResult<()> {
        self.work.structure = self
            .work
            .structure
            .checked_add(1)
            .ok_or(ProjectionError::WorkOverflow)?;
        self.work.charge(0)?;
        self.break_text()?;
        self.previous_attribute_end = name_end;
        self.scope_start = self.namespaces.scope_start();
        Ok(())
    }

    pub(super) fn preflight_tag_item(&mut self) -> ProjectionResult<()> {
        if self.work.structure == 1 {
            // The compiler-owned wrapper has two fixed declarations.
            return Ok(());
        }
        // Pack ingestion already prices fragment bytes. Attribute checks and binding
        // declarations each add one structural unit.
        self.work.charge(1)
    }

    pub(super) fn attribute(
        &mut self,
        name: &'xml str,
        raw_value: &'xml str,
    ) -> ProjectionResult<()> {
        // These slices come from the same source-backed quick-xml attribute iterator.
        let document_start = self.document.as_ptr() as usize;
        let start = (name.as_ptr() as usize)
            .checked_sub(document_start)
            .ok_or(ProjectionError::InvalidXml)?;
        let value_start = (raw_value.as_ptr() as usize)
            .checked_sub(document_start)
            .ok_or(ProjectionError::InvalidXml)?;
        if start == self.previous_attribute_end {
            return Err(ProjectionError::InvalidXml);
        }
        self.previous_attribute_end = value_start + raw_value.len() + 1;
        let (prefix, local) = split_name(name);
        if prefix == "xmlns" {
            let value = normalized_value(raw_value, DecodeMode::Attribute)?;
            if value.is_empty()
                || value == XMLNS_NAMESPACE
                || (local == "xml") != (value == XML_NAMESPACE)
            {
                return Err(ProjectionError::InvalidXml);
            }
            if local != "xml" {
                self.namespaces.bind(local, value)?;
            }
        } else if name == "xmlns" {
            let value = normalized_value(raw_value, DecodeMode::Attribute)?;
            if matches!(value.as_ref(), XML_NAMESPACE | XMLNS_NAMESPACE) {
                return Err(ProjectionError::InvalidXml);
            }
            self.namespaces.bind("", value)?;
        } else {
            // Preserve historical source ranges for declarations wider than the strict
            // parser's stored name length or equals span.
            let equals_len = value_start - start - name.len() - 1;
            self.attributes.try_reserve(1)?;
            self.attributes.push(Attribute {
                prefix,
                local,
                raw_value,
                value_range: start
                    + name.len().min(u16::MAX as usize)
                    + equals_len.min(u8::MAX as usize)
                    + 1..value_start + raw_value.len(),
            });
        }
        Ok(())
    }

    pub(super) fn finish_element(
        &mut self,
        name: &str,
        range: Range<usize>,
        empty: bool,
    ) -> ProjectionResult<()> {
        let (prefix, local) = split_name(name);
        let namespace = match prefix {
            "xml" => Some(Namespace::Xml),
            prefix => self.namespaces.lookup(prefix),
        };
        if !prefix.is_empty() && namespace.is_none() {
            return Err(ProjectionError::InvalidXml);
        }
        let style = namespace == Some(Namespace::Svg) && local == "style";
        let hoist_defs =
            namespace == Some(Namespace::Svg) && local == "defs" && !self.inside_hoisted_defs;

        // Raw expanded names were already checked. Normalized namespace URIs can cause
        // two different prefixes to resolve to the same expanded attribute name.
        let mut prefixed_names = HashSet::new();
        for attribute in self.attributes.drain(..) {
            let namespace = match attribute.prefix {
                "" => None,
                "xml" => Some(Namespace::Xml),
                prefix => Some(
                    self.namespaces
                        .lookup(prefix)
                        .ok_or(ProjectionError::InvalidXml)?,
                ),
            };
            if let Some(namespace) = namespace {
                prefixed_names.try_reserve(1)?;
                if !prefixed_names.insert((namespace, attribute.local)) {
                    return Err(ProjectionError::InvalidXml);
                }
            }
            if let Some(kind) = AttributeKind::for_name(namespace, attribute.local) {
                let value = normalized_value(attribute.raw_value, DecodeMode::Attribute)?;
                self.sources.attributes.try_reserve(1)?;
                self.sources.attributes.push(RewriteAttribute {
                    kind,
                    value,
                    value_range: attribute.value_range,
                });
            }
        }

        if empty {
            if hoist_defs {
                self.sources.defs.try_reserve(1)?;
                self.sources.defs.push(range);
            }
            self.namespaces.restore(self.scope_start);
        } else {
            self.inside_hoisted_defs |= hoist_defs;
            self.elements.try_reserve(1)?;
            self.elements.push(OpenElement {
                scope: self.scope_start,
                hoisted_defs_start: hoist_defs.then_some(range.start),
                style,
            });
        }
        Ok(())
    }

    pub(super) fn close_element(&mut self, end: usize) -> ProjectionResult<()> {
        self.break_text()?;
        let element = self.elements.pop().ok_or(ProjectionError::InvalidXml)?;
        if let Some(start) = element.hoisted_defs_start {
            self.sources.defs.try_reserve(1)?;
            self.sources.defs.push(start..end);
            self.inside_hoisted_defs = false;
        }
        self.namespaces.restore(element.scope);
        Ok(())
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

    pub(super) fn cdata(&mut self, range: Range<usize>) -> ProjectionResult<()> {
        if self.in_style() {
            self.finish_text()?;
            self.work.charge(1)?;
            let raw = self.document[range.clone()]
                .strip_prefix("<![CDATA[")
                .and_then(|text| text.strip_suffix("]]>"))
                .ok_or(ProjectionError::InvalidXml)?;
            self.style_text(range, normalized_value(raw, DecodeMode::CData)?)?;
        }
        Ok(())
    }

    pub(super) fn break_text(&mut self) -> ProjectionResult<()> {
        self.finish_text()?;
        if let Some(run) = self.style_run.take() {
            self.sources.style_text.try_reserve(1)?;
            self.sources.style_text.push(run);
        }
        Ok(())
    }

    fn in_style(&self) -> bool {
        self.elements.last().is_some_and(|element| element.style)
    }

    fn finish_text(&mut self) -> ProjectionResult<()> {
        if let Some(range) = self.pending_text.take() {
            self.work.charge(1)?;
            let value = normalized_value(&self.document[range.clone()], DecodeMode::Text)?;
            self.style_text(range, value)?;
        }
        Ok(())
    }

    fn style_text(&mut self, range: Range<usize>, value: Cow<'xml, str>) -> ProjectionResult<()> {
        if let Some((_, previous)) = self.style_run.as_mut() {
            // Adjacent text and CDATA shared a value but used the first token's range.
            match previous {
                Cow::Borrowed(previous_text) => {
                    let bytes = previous_text
                        .len()
                        .checked_add(value.len())
                        .ok_or(ProjectionError::WorkOverflow)?;
                    let mut joined = String::new();
                    joined.try_reserve_exact(bytes)?;
                    joined.push_str(previous_text);
                    joined.push_str(&value);
                    *previous = Cow::Owned(joined);
                }
                Cow::Owned(previous_text) => {
                    previous_text.try_reserve(value.len())?;
                    previous_text.push_str(&value);
                }
            }
        } else {
            self.style_run = Some((range, value));
        }
        Ok(())
    }
}

fn split_name(name: &str) -> (&str, &str) {
    name.split_once(':').unwrap_or(("", name))
}

fn normalized_value(raw: &str, mode: DecodeMode) -> ProjectionResult<Cow<'_, str>> {
    let needs_normalization = match mode {
        DecodeMode::Attribute => raw.contains(['&', '\t', '\r', '\n']),
        DecodeMode::Text => raw.contains(['&', '\r']),
        DecodeMode::CData => raw.contains('\r'),
    };
    if !needs_normalization {
        return Ok(Cow::Borrowed(raw));
    }

    // XML 1.0 line endings and the five predefined or numeric references never grow.
    let mut result = String::new();
    result.try_reserve_exact(raw.len())?;
    let mut offset = 0;
    while offset < raw.len() {
        let (character, end) =
            next_decoded_char(raw, offset, mode).ok_or(ProjectionError::InvalidXml)?;
        result.push(character);
        offset = end;
    }
    Ok(Cow::Owned(result))
}

#[cfg(test)]
mod tests {
    use super::{DecodeMode, normalized_value};

    #[test]
    fn attribute_and_cdata_normalization_keep_source_whitespace_semantics() {
        let attribute = "A\r\n\t&#xD;&#xA;&amp;é";
        let cdata = "B\r\né\r&amp;";
        let source = format!("<svg title=\"{attribute}\"><![CDATA[{cdata}]]></svg>");
        let document = roxmltree::Document::parse(&source).unwrap();
        let root = document.root_element();
        assert_eq!(
            normalized_value(attribute, DecodeMode::Attribute)
                .unwrap()
                .as_ref(),
            root.attribute("title").unwrap(),
        );
        assert_eq!(
            normalized_value(cdata, DecodeMode::CData).unwrap().as_ref(),
            root.text().unwrap(),
        );
    }

    #[test]
    fn style_text_normalizes_source_eols_separately_from_character_references() {
        for (raw, expected) in [
            ("a\r\nb", "a\nb"),
            ("a\r&#xA;b", "a\n\nb"),
            ("a&amp;\r", "a&\n"),
            ("a&amp;\r\nb", "a&\nb"),
            ("a&#xD;\nb", "a\r\nb"),
            ("a&#xD;&#10;b", "a\r\nb"),
            ("a&#xD;é\r", "a\ré\n"),
            ("a\r&#x26;\ré", "a\n&\né"),
        ] {
            assert_eq!(
                normalized_value(raw, DecodeMode::Text).unwrap().as_ref(),
                expected,
                "{raw:?}",
            );
        }
    }
}
