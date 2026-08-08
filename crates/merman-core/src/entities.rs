use std::borrow::Cow;

/// Reverts only the placeholder spelling introduced by Mermaid's `encodeEntities` preprocessing.
///
/// This is the exact representation transition used by Mermaid's final `cleanUpSvgCode` pass. It
/// deliberately does not parse the resulting HTML character references; callers that operate on
/// serialized markup must leave that work to the browser parser.
pub(crate) fn decode_mermaid_entity_placeholders(input: &str) -> Cow<'_, str> {
    if !input.contains('\u{fb02}') && !input.contains('\u{00b6}') {
        return Cow::Borrowed(input);
    }

    Cow::Owned(
        input
            .replace("ﬂ°°", "&#")
            .replace("ﬂ°", "&")
            .replace("¶ß", ";"),
    )
}

/// Decodes Mermaid's `encodeEntities` placeholders and shorthand `#...;` sequences into Unicode.
///
/// Upstream Mermaid runs `encodeEntities(text)` before parsing, and later uses `decodeEntities`
/// + browser `entityDecode(...)` to turn placeholders into actual characters.
///
/// In `merman` we decode these into Unicode as part of headless parsing so that:
/// - layout measurements operate on the same final text
/// - SVG output matches upstream DOM output
pub fn decode_mermaid_entities_to_unicode(input: &str) -> Cow<'_, str> {
    let entities = restore_mermaid_entity_spelling(input);
    if !entities.contains('&') {
        return entities;
    }

    Cow::Owned(decode_html_entities_to_unicode(entities.as_ref()).into_owned())
}

/// Restores Mermaid preprocessor placeholders and `#...;` shorthand to HTML entity spelling.
///
/// This intentionally stops before browser entity decoding. Flowchart SVG text labels need the
/// literal `&nbsp;` spelling, while HTML labels decode the same spelling when inserted as markup.
pub fn restore_mermaid_entity_spelling(input: &str) -> Cow<'_, str> {
    if !input.contains('#') && !input.contains('ﬂ') && !input.contains('¶') {
        return Cow::Borrowed(input);
    }

    // Step 1: Mermaid placeholders -> `&...;` / `&#...;`
    let mut s = decode_mermaid_entity_placeholders(input).into_owned();

    // Step 2 (shorthand): `#...;` -> `&...;` / `&#...;`
    //
    // This is primarily for older headless code paths / fixtures that bypass upstream-like
    // preprocessing. It is intentionally conservative and only rewrites `#\w+;` patterns.
    if s.contains('#') {
        let mut out = String::with_capacity(s.len());
        let mut it = s.chars().peekable();
        let mut prev: Option<char> = None;
        while let Some(ch) = it.next() {
            if ch != '#' {
                out.push(ch);
                prev = Some(ch);
                continue;
            }

            // Do not treat `&#...;` as Mermaid shorthand `#...;`.
            if prev == Some('&') {
                out.push('#');
                prev = Some('#');
                continue;
            }

            let mut entity = String::new();
            let mut ok = false;
            for _ in 0..64 {
                match it.peek().copied() {
                    Some(';') => {
                        it.next();
                        ok = true;
                        break;
                    }
                    Some(c) if c.is_ascii_alphanumeric() || c == '_' || c == '+' => {
                        entity.push(c);
                        it.next();
                    }
                    _ => break,
                }
            }

            if !ok {
                out.push('#');
                out.push_str(&entity);
                continue;
            }

            let is_int = entity.chars().all(|c| c.is_ascii_digit() || c == '+')
                && entity.chars().any(|c| c.is_ascii_digit());
            if is_int {
                out.push('&');
                out.push('#');
                out.push_str(&entity);
                out.push(';');
            } else {
                out.push('&');
                out.push_str(&entity);
                out.push(';');
            }
            prev = Some(';');
        }
        s = out;
    }

    Cow::Owned(s)
}

/// Decodes browser-facing HTML entities into Unicode without Mermaid shorthand handling.
pub fn decode_html_entities_to_unicode(input: &str) -> Cow<'_, str> {
    if !input.contains('&') {
        return Cow::Borrowed(input);
    }

    htmlize::unescape(input)
}

#[cfg(test)]
mod tests {
    use super::{
        decode_html_entities_to_unicode, decode_mermaid_entities_to_unicode,
        decode_mermaid_entity_placeholders, restore_mermaid_entity_spelling,
    };

    #[test]
    fn html_entity_decode_does_not_apply_mermaid_shorthand() {
        assert_eq!(
            decode_html_entities_to_unicode("Tom &amp; Jerry &lt;ok&gt; &#39;x&#39;"),
            "Tom & Jerry <ok> 'x'"
        );
        assert_eq!(decode_html_entities_to_unicode("#quot;"), "#quot;");
    }

    #[test]
    fn mermaid_entity_decode_keeps_shorthand_and_placeholder_semantics() {
        assert_eq!(decode_mermaid_entities_to_unicode("#quot;"), "\"");
        assert_eq!(decode_mermaid_entities_to_unicode("ﬂ°quot¶ß"), "\"");
        assert_eq!(decode_mermaid_entities_to_unicode("ﬂ°°39¶ß"), "'");
    }

    #[test]
    fn mermaid_entity_spelling_restoration_stops_before_browser_decode() {
        assert_eq!(restore_mermaid_entity_spelling("#nbsp;"), "&nbsp;");
        assert_eq!(restore_mermaid_entity_spelling("ﬂ°nbsp¶ß"), "&nbsp;");
        assert_eq!(restore_mermaid_entity_spelling("ﬂ°°160¶ß"), "&#160;");
        assert_eq!(restore_mermaid_entity_spelling("&nbsp;"), "&nbsp;");
    }

    #[test]
    fn placeholder_decode_preserves_the_serialized_entity_layer() {
        assert_eq!(
            decode_mermaid_entity_placeholders("javascriptﬂ°colon¶ßalert(1)"),
            "javascript&colon;alert(1)"
        );
        assert_eq!(
            decode_mermaid_entity_placeholders("ticket&amp;value"),
            "ticket&amp;value"
        );
    }
}
