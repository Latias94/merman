//! Participation facts from the prepared ER terminal, with known zero-sized runs excluded.

use crate::text::VisibleTextStyleFacts;

#[derive(Debug, Clone)]
pub(super) struct ErTextFacts {
    pub(super) has_content: bool,
    pub(super) visible: VisibleTextStyleFacts,
}

impl ErTextFacts {
    pub(super) fn plain(text: &str, size: Option<f64>) -> Self {
        let visible = VisibleTextStyleFacts::plain_text_at_font_size(text, size);
        Self {
            has_content: visible.has_content_runs(),
            visible,
        }
    }

    pub(super) fn svg(markup: &str, size: Option<f64>) -> Self {
        // SVG words have already been escaped and the generic first row transformed. Literal
        // HTML spelling in these text nodes cannot introduce HTML font ownership.
        let document =
            crate::xml_dom::Document::parse(markup).expect("prepared ER SVG text is valid XML");
        let has_content = document.descendants().any(|node| {
            node.is_text()
                && node.text().is_some_and(|text| {
                    !crate::text::trim_html_collapsible_ascii_whitespace(text).is_empty()
                })
        });
        Self::plain(if has_content { "text" } else { "" }, size)
    }

    pub(super) fn html(fragment: &str, size: Option<f64>) -> Self {
        let visible = VisibleTextStyleFacts::from_xhtml_fragment_at_font_size(fragment, size);
        Self {
            has_content: visible.has_content_runs(),
            visible,
        }
    }
}
