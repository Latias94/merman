//! Text extraction from rendered XHTML label fragments.

use std::borrow::Cow;

/// Returns the rendered text when an XHTML label fragment is structurally plain.
///
/// Mermaid's HTML-label path always wraps ordinary inline text in one `<p>`. Parsing the
/// generated fragment lets callers preserve the host measurer's native precision for that common
/// case without guessing from Markdown punctuation. Any nested element, additional root node, or
/// malformed fragment remains on the structured HTML path.
/// Borrowed text is the literal content of one paragraph and needs no XML normalization.
pub(crate) fn mermaid_xhtml_label_plain_text(fragment: &str) -> Option<Cow<'_, str>> {
    if let Some(text) = fragment
        .strip_prefix("<p>")
        .and_then(|text| text.strip_suffix("</p>"))
    {
        // Printable XML text needs no entity decoding or newline normalization.
        if text
            .bytes()
            .all(|byte| (b' '..=b'~').contains(&byte) && !matches!(byte, b'<' | b'&' | b']'))
        {
            return Some(Cow::Borrowed(text));
        }
    }

    let wrapped = format!("<merman-fragment>{fragment}</merman-fragment>");
    let document = crate::xml_dom::Document::parse(&wrapped).ok()?;
    let root = document.root_element();

    let mut children = root.children();
    let paragraph = children.next()?;
    if children.next().is_some()
        || !paragraph.is_element()
        || paragraph.tag_name().name() != "p"
        || paragraph.children().any(|child| !child.is_text())
    {
        return None;
    }

    Some(Cow::Owned(
        paragraph
            .children()
            .filter_map(|child| child.text())
            .collect(),
    ))
}

pub(crate) fn mermaid_xhtml_label_text_content(fragment: &str) -> Option<String> {
    let wrapped = format!("<merman-fragment>{fragment}</merman-fragment>");
    let document = crate::xml_dom::Document::parse(&wrapped).ok()?;
    Some(
        document
            .root_element()
            .descendants()
            .filter_map(|node| node.text().filter(|_| node.is_text()))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xhtml_plain_text_is_decided_from_the_rendered_structure() {
        assert_eq!(
            mermaid_xhtml_label_plain_text(r#"<p>literal > "double" 'single'</p>"#).as_deref(),
            Some(r#"literal > "double" 'single'"#)
        );
        assert_eq!(
            mermaid_xhtml_label_plain_text("<p></p>").as_deref(),
            Some("")
        );
        assert_eq!(
            mermaid_xhtml_label_plain_text("<p>first\r\nsecond\rthird</p>").as_deref(),
            Some("first\nsecond\nthird")
        );
        assert_eq!(
            mermaid_xhtml_label_plain_text("<p>first]]>second</p>"),
            None
        );
        assert_eq!(
            mermaid_xhtml_label_plain_text("<p>first<!-- middle -->second</p>"),
            None
        );
        assert_eq!(
            mermaid_xhtml_label_plain_text("<p>Generic&lt;T&gt; driver_license</p>").as_deref(),
            Some("Generic<T> driver_license")
        );
        assert_eq!(
            mermaid_xhtml_label_plain_text("<p><a href='/'><code>Entity</code></a></p>"),
            None
        );
        assert_eq!(
            mermaid_xhtml_label_plain_text("<p>first<br/>second</p>"),
            None
        );
        assert_eq!(
            mermaid_xhtml_label_plain_text("<p>first</p><p>second</p>"),
            None
        );
        assert_eq!(
            mermaid_xhtml_label_text_content("<p><em>Result&lt;<strong>T</strong>&gt;</em></p>")
                .as_deref(),
            Some("Result<T>")
        );
    }
}
