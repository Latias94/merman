use super::checkpoint_loop;
use crate::Result;
use std::ops::Range;

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

/// Counts SVG text elements with character data, including data copied by a `tref`.
/// Empty Mermaid parity text and tspan nodes do not select or paint a font face.
#[derive(Debug, Default)]
pub(super) struct SvgTextContentTracker {
    open_text: Vec<OpenText>,
    references: Vec<String>,
    pending_text: Vec<Range<usize>>,
    count: usize,
}

#[derive(Debug)]
struct OpenText {
    depth: usize,
    open_tref_depth: Option<usize>,
    first_reference: usize,
    has_content: bool,
}

impl SvgTextContentTracker {
    pub(super) fn observe_start(&mut self, depth: usize) {
        self.open_text.push(OpenText {
            depth,
            open_tref_depth: None,
            first_reference: self.references.len(),
            has_content: false,
        });
    }

    pub(super) fn observe_end(&mut self, depth: usize) {
        if let Some(text) = self.open_text.last_mut()
            && text.open_tref_depth == Some(depth)
        {
            text.open_tref_depth = None;
        }
        let Some(text) = self.open_text.pop_if(|text| text.depth == depth) else {
            return;
        };
        if text.has_content {
            self.count += 1;
            self.observe_character();
        } else if text.first_reference < self.references.len() {
            self.pending_text
                .push(text.first_reference..self.references.len());
        }
    }

    pub(super) fn observe_content(&mut self, content: &str) {
        if !content.is_empty() {
            self.observe_character();
        }
    }

    pub(super) fn observe_tref(&mut self, target: Option<String>, open_depth: Option<usize>) {
        if let Some(text) = self.open_text.last_mut()
            && text.open_tref_depth.is_none()
        {
            self.references.extend(target);
            // The renderer uses the referenced text in place of any children of this element.
            text.open_tref_depth = open_depth;
        }
    }

    pub(super) fn observe_character(&mut self) {
        if let Some(text) = self.open_text.last_mut()
            && text.open_tref_depth.is_none()
        {
            text.has_content = true;
        }
    }

    pub(super) fn has_pending_text(&self) -> bool {
        !self.pending_text.is_empty()
    }

    pub(super) fn text_element_count(
        self,
        target_has_text: impl Fn(&str) -> bool,
        mut checkpoint: impl FnMut() -> Result<()>,
    ) -> Result<usize> {
        let mut count = self.count;
        let mut examined = 0;
        let mut last_match = None;
        // Text closes in order of range end. Resolve each reference at most once,
        // even when a forward reference contributes to nested text elements.
        for (iteration, range) in self.pending_text.into_iter().enumerate() {
            checkpoint_loop(iteration, &mut checkpoint)?;
            for index in examined..range.end {
                checkpoint_loop(index, &mut checkpoint)?;
                if target_has_text(&self.references[index]) {
                    last_match = Some(index);
                }
            }
            examined = range.end;
            count += usize::from(last_match.is_some_and(|index| index >= range.start));
        }
        Ok(count)
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
        let mut depth = 0;
        loop {
            match reader.read_event().unwrap() {
                Event::Start(element) => {
                    if element.local_name().as_ref() == "text" {
                        tracker.observe_start(depth);
                    }
                    depth += 1;
                }
                Event::End(_) => {
                    depth -= 1;
                    tracker.observe_end(depth);
                }
                Event::Text(text) => tracker.observe_content(&text.xml10_content()),
                Event::CData(text) => tracker.observe_content(&text.xml10_content()),
                Event::GeneralRef(_) => tracker.observe_character(),
                Event::Eof => break,
                _ => {}
            }
        }
        tracker.text_element_count(|_| false, || Ok(())).unwrap()
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
