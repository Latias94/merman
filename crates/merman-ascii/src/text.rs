use unicode_width::UnicodeWidthStr;

pub(crate) fn display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}
