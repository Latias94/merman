//! Shared text measurement types.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MermaidMarkdownWordType {
    Normal,
    Strong,
    Em,
}

#[derive(Clone, Copy)]
pub enum SvgTextEntityMode {
    DecodedModel,
    CreateTextSource,
}

impl SvgTextEntityMode {
    pub fn visible_word(self, word: &str) -> std::borrow::Cow<'_, str> {
        match self {
            Self::DecodedModel => std::borrow::Cow::Borrowed(word),
            Self::CreateTextSource => crate::entities::decode_svg_text_content_entities(word),
        }
    }
}
