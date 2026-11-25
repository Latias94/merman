use super::checkpoint_loop;
use crate::Result;
use std::collections::{HashMap, HashSet};
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
    references: Vec<TextReference>,
    pending_text: Vec<PendingText>,
    count: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum SvgTextContent {
    #[default]
    Empty,
    Whitespace {
        more_than_two_spaces: bool,
    },
    Characters,
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct SvgCharacterEvents {
    bytes: usize,
    non_whitespace: usize,
}

impl SvgCharacterEvents {
    pub(super) fn observe(&mut self, content: &str) {
        self.bytes += content.len();
        self.non_whitespace += usize::from(
            content
                .bytes()
                .any(|byte| !matches!(byte, b' ' | b'\t' | b'\r' | b'\n')),
        );
    }

    pub(super) fn content_since(self, start: Self) -> SvgTextContent {
        if self.non_whitespace != start.non_whitespace {
            SvgTextContent::Characters
        } else if self.bytes != start.bytes {
            SvgTextContent::Whitespace {
                more_than_two_spaces: self.bytes - start.bytes > 2,
            }
        } else {
            SvgTextContent::Empty
        }
    }
}

#[derive(Debug)]
struct TextReference {
    target: String,
    preserves_in_default_text: bool,
}

#[derive(Debug)]
struct PendingText {
    element: usize,
    preserves_space: bool,
    references: Range<usize>,
}

#[derive(Debug)]
struct OpenText {
    depth: usize,
    element: usize,
    preserves_space: bool,
    open_tref_depth: Option<usize>,
    first_reference: usize,
    has_content: bool,
}

impl SvgTextContentTracker {
    pub(super) fn observe_start(&mut self, depth: usize, element: usize, preserves_space: bool) {
        self.open_text.push(OpenText {
            depth,
            element,
            preserves_space,
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
            self.pending_text.push(PendingText {
                element: text.element,
                preserves_space: text.preserves_space,
                references: text.first_reference..self.references.len(),
            });
        }
    }

    pub(super) fn observe_content(&mut self, content: &str) {
        if !content.is_empty() {
            self.observe_character();
        }
    }

    pub(super) fn observe_tref(
        &mut self,
        target: Option<String>,
        space_override: Option<(usize, bool)>,
        open_depth: Option<usize>,
    ) {
        if let Some(text) = self.open_text.last_mut()
            && text.open_tref_depth.is_none()
        {
            if let Some(target) = target {
                self.references.push(TextReference {
                    target,
                    preserves_in_default_text: space_override
                        .is_some_and(|(depth, preserve)| depth > text.depth && preserve),
                });
            }
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
        target_content: impl Fn(&str) -> SvgTextContent,
        may_inherit_use_space: bool,
        mut checkpoint: impl FnMut() -> Result<()>,
    ) -> Result<(usize, Vec<usize>)> {
        let mut count = self.count;
        let mut examined = 0;
        let mut last_characters = None;
        let mut last_whitespace = None;
        let mut previous_whitespace = None;
        let mut last_locally_preserved = None;
        let mut last_locally_preserved_long = None;
        let mut needs_inherited_space = Vec::new();
        // Text closes in order of range end. Resolve each reference at most once,
        // even when a forward reference contributes to nested text elements.
        for (iteration, text) in self.pending_text.into_iter().enumerate() {
            checkpoint_loop(iteration, &mut checkpoint)?;
            for index in examined..text.references.end {
                checkpoint_loop(index, &mut checkpoint)?;
                let reference = &self.references[index];
                match target_content(&reference.target) {
                    SvgTextContent::Empty => {}
                    SvgTextContent::Characters => last_characters = Some(index),
                    SvgTextContent::Whitespace {
                        more_than_two_spaces,
                    } => {
                        previous_whitespace = last_whitespace.replace(index);
                        if reference.preserves_in_default_text {
                            last_locally_preserved = Some(index);
                            if more_than_two_spaces {
                                last_locally_preserved_long = Some(index);
                            }
                        }
                    }
                }
            }
            examined = text.references.end;
            let includes =
                |last: Option<usize>| last.is_some_and(|index| index >= text.references.start);
            let has_whitespace = includes(last_whitespace);
            if includes(last_characters)
                || (has_whitespace && text.preserves_space)
                || includes(last_locally_preserved_long)
                || (includes(last_locally_preserved) && includes(previous_whitespace))
            {
                count += 1;
            } else if has_whitespace && may_inherit_use_space {
                needs_inherited_space.push(text.element);
            }
        }
        Ok((count, needs_inherited_space))
    }
}

/// Adds the space inherited by cloned subtrees to the lexical space tracked in the XML walk.
#[derive(Default)]
pub(super) struct SvgUseTextSpace {
    resets: HashSet<usize>,
    uses: HashMap<usize, UseSpaceSite>,
    has_preserving_use: bool,
    parents: Vec<usize>,
    inherited: Vec<bool>,
}

struct UseSpaceSite {
    lexically_preserves: bool,
    target: Option<usize>,
}

impl SvgUseTextSpace {
    pub(super) fn observe_reset(&mut self, element: usize) {
        self.resets.insert(element);
    }

    pub(super) fn observe_use(&mut self, element: usize, preserves_space: bool) {
        self.has_preserving_use |= preserves_space;
        self.uses.insert(
            element,
            UseSpaceSite {
                lexically_preserves: preserves_space,
                target: None,
            },
        );
    }

    pub(super) fn has_preserving_use(&self) -> bool {
        self.has_preserving_use
    }

    pub(super) fn resolve_use_target(&mut self, source: usize, target: usize) {
        if let Some(site) = self.uses.get_mut(&source) {
            site.target = Some(target);
        }
    }

    pub(super) fn set_parents(&mut self, parents: Vec<usize>) {
        self.inherited.resize(parents.len(), false);
        self.parents = parents;
    }

    pub(super) fn preserves(&self, element: usize) -> bool {
        self.inherited.get(element).copied().unwrap_or(false)
    }

    pub(super) fn inherit_in_reference_order(&mut self, element: usize) {
        let Some(&parent) = self.parents.get(element) else {
            return;
        };
        let preserves =
            (self.preserves(element) || self.preserves(parent)) && !self.resets.contains(&element);
        self.inherited[element] = preserves;
        if let Some(site) = self.uses.get(&element)
            && (preserves || site.lexically_preserves)
            && let Some(target) = site.target
        {
            self.inherited[target] = true;
        }
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
                        tracker.observe_start(depth, 0, false);
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
        tracker
            .text_element_count(|_| SvgTextContent::Empty, false, || Ok(()))
            .unwrap()
            .0
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
