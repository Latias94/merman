use quick_xml::events::BytesStart;
use quick_xml::name::{NamespaceResolver, ResolveResult};

const SVG_NAMESPACE: &[u8] = b"http://www.w3.org/2000/svg";

/// An asset-free SVG is font-independent only when it contains no rendered text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SvgFontSeal {
    text_element_count: usize,
}

impl SvgFontSeal {
    pub(super) const fn unsealed(text_element_count: usize) -> Self {
        Self { text_element_count }
    }

    pub(super) const fn not_required() -> Self {
        Self::unsealed(0)
    }

    pub(crate) const fn is_complete(&self) -> bool {
        self.text_element_count == 0
    }
}

/// Counts SVG text elements with character content, including preserved whitespace.
/// Empty Mermaid parity text and tspan nodes do not select or paint a font face.
#[derive(Debug, Default)]
pub(super) struct SvgTextContentTracker {
    element_stack: Vec<bool>,
    open_text_stack: Vec<bool>,
    text_element_count: usize,
}

impl SvgTextContentTracker {
    pub(super) fn observe_start(
        &mut self,
        element: &BytesStart<'_>,
        resolver: &NamespaceResolver,
    ) -> Option<()> {
        let (namespace, local_name) = resolver.resolve_element(element.name());
        let is_svg_element = match namespace {
            ResolveResult::Unknown(_) => return None,
            ResolveResult::Unbound => true,
            ResolveResult::Bound(namespace) => namespace.as_ref() == SVG_NAMESPACE,
        };
        let is_text = is_svg_element && local_name.as_ref().eq_ignore_ascii_case(b"text");
        self.element_stack.push(is_text);
        if is_text {
            self.open_text_stack.push(false);
        }
        Some(())
    }

    pub(super) fn observe_end(&mut self) -> Option<()> {
        if !self.element_stack.pop()? {
            return Some(());
        }
        if self.open_text_stack.pop()? {
            self.text_element_count = self.text_element_count.checked_add(1)?;
            if let Some(parent) = self.open_text_stack.last_mut() {
                *parent = true;
            }
        }
        Some(())
    }

    pub(super) fn observe_content(&mut self, content: &str) {
        if !content.is_empty()
            && let Some(text) = self.open_text_stack.last_mut()
        {
            *text = true;
        }
    }

    pub(super) fn observe_character(&mut self, _character: char) {
        if let Some(text) = self.open_text_stack.last_mut() {
            *text = true;
        }
    }

    pub(super) fn text_element_count(&self) -> usize {
        self.text_element_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quick_xml::events::Event;
    use quick_xml::reader::NsReader;

    fn text_count(svg: &str) -> usize {
        let mut reader = NsReader::from_str(svg);
        let mut tracker = SvgTextContentTracker::default();
        loop {
            match reader.read_event().unwrap() {
                Event::Start(element) => {
                    tracker.observe_start(&element, reader.resolver()).unwrap();
                }
                Event::End(_) => tracker.observe_end().unwrap(),
                Event::Text(text) => tracker.observe_content(&text.xml10_content().unwrap()),
                Event::CData(text) => tracker.observe_content(&text.decode().unwrap()),
                Event::GeneralRef(reference) => {
                    tracker.observe_character(reference.resolve_char_ref().unwrap().unwrap());
                }
                Event::Eof => break,
                _ => {}
            }
        }
        tracker.text_element_count()
    }

    #[test]
    fn empty_parity_nodes_do_not_require_fonts_but_preserved_whitespace_does() {
        assert_eq!(text_count("<svg><text/><text><tspan/></text></svg>"), 0);
        assert_eq!(
            text_count("<svg><text xml:space=\"preserve\"><tspan> </tspan></text></svg>"),
            1
        );
        assert_eq!(
            text_count("<svg><text><tspan>&#xA0;</tspan></text></svg>"),
            1
        );
    }

    #[test]
    fn nested_and_deep_content_reaches_its_text_owners() {
        assert_eq!(
            text_count("<svg><text><g><text><tspan>Alpha</tspan></text></g></text></svg>"),
            2
        );
        let svg = format!(
            "<svg><text>{}Alpha{}</text></svg>",
            "<g>".repeat(1_024),
            "</g>".repeat(1_024)
        );
        assert_eq!(text_count(&svg), 1);
    }
}
