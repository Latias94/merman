use std::fmt::Write;

/// Escapes one identifier for use in a CSS selector embedded in SVG XML text.
///
/// This follows the CSSOM identifier serialization rules while using hexadecimal escapes for XML
/// text metacharacters. The latter keeps the serialized stylesheet well-formed without changing
/// the identifier CSS resolves after parsing. This boundary is only for CSS identifiers; XML
/// attributes and CSS `url(#fragment)` values use their own escaping paths.
pub(crate) fn escape_css_identifier(identifier: &str) -> String {
    let first = identifier.chars().next();
    let mut escaped = String::with_capacity(identifier.len());

    for (index, character) in identifier.chars().enumerate() {
        if character == '\0' {
            escaped.push('\u{fffd}');
            continue;
        }

        let code_point = character as u32;
        let is_control = code_point <= 0x1f || code_point == 0x7f;
        let is_leading_digit =
            character.is_ascii_digit() && (index == 0 || (index == 1 && first == Some('-')));
        let is_xml_text_metacharacter = matches!(character, '&' | '<' | '>');
        if is_control || is_leading_digit || is_xml_text_metacharacter {
            write!(&mut escaped, "\\{code_point:x} ")
                .expect("writing a CSS identifier escape to String cannot fail");
            continue;
        }

        if index == 0 && character == '-' && identifier == "-" {
            escaped.push_str("\\-");
        } else if character.is_ascii_alphanumeric()
            || character == '-'
            || character == '_'
            || !character.is_ascii()
        {
            escaped.push(character);
        } else {
            escaped.push('\\');
            escaped.push(character);
        }
    }

    escaped
}

#[cfg(test)]
mod tests {
    use super::escape_css_identifier;

    #[test]
    fn css_identifier_escape_handles_selector_and_xml_boundaries() {
        assert_eq!(escape_css_identifier("seq:prod"), r"seq\:prod");
        assert_eq!(escape_css_identifier("1diagram"), r"\31 diagram");
        assert_eq!(escape_css_identifier("-1diagram"), r"-\31 diagram");
        assert_eq!(escape_css_identifier("-"), r"\-");
        assert_eq!(escape_css_identifier("a<b&c>d"), r"a\3c b\26 c\3e d");
        assert_eq!(escape_css_identifier("diagram\0id"), "diagram\u{fffd}id");
        assert_eq!(escape_css_identifier("主题"), "主题");
    }

    #[test]
    fn css_identifier_escape_preserves_the_existing_safe_id_spelling() {
        assert_eq!(escape_css_identifier("diagram-1_name"), "diagram-1_name");
    }
}
