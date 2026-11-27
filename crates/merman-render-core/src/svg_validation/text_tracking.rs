/// An asset-free SVG is font-independent only when no authored text may use fonts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgFontSeal {
    text_element_count: usize,
}

impl SvgFontSeal {
    pub(super) const fn from_text_count(text_element_count: usize) -> Self {
        Self { text_element_count }
    }

    pub const fn is_complete(&self) -> bool {
        self.text_element_count == 0
    }
}

/// Counts each authored text element once, without determining its renderability.
#[derive(Debug, Default)]
pub(super) struct SvgTextContentTracker {
    open_text: Vec<OpenText>,
    count: usize,
}

#[derive(Debug)]
struct OpenText {
    depth: usize,
    has_content: bool,
}

impl SvgTextContentTracker {
    pub(super) fn observe_start(&mut self, depth: usize) {
        self.open_text.push(OpenText {
            depth,
            has_content: false,
        });
    }

    pub(super) fn observe_end(&mut self, depth: usize) {
        if let Some(text) = self.open_text.pop_if(|text| text.depth == depth)
            && text.has_content
        {
            self.count += 1;
            self.observe_character();
        }
    }

    pub(super) fn observe_content(&mut self, content: &str) {
        if !content.is_empty() {
            self.observe_character();
        }
    }

    pub(super) fn observe_character(&mut self) {
        if let Some(text) = self.open_text.last_mut() {
            text.has_content = true;
        }
    }

    pub(super) fn text_element_count(&self) -> usize {
        self.count
    }
}
