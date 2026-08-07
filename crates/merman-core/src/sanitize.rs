use crate::MermaidConfig;
use crate::generated::dompurify_defaults;
use lol_html::{RewriteStrSettings, element, rewrite_str};
use std::collections::HashSet;
use std::sync::OnceLock;

fn break_to_placeholder(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut cursor = 0usize;
    let mut probe = 0usize;

    while let Some(rel_start) = input[probe..].find('<') {
        let start = probe + rel_start;
        let Some(end) = mermaid_line_break_tag_end(input, start) else {
            probe = start + 1;
            continue;
        };

        out.push_str(&input[cursor..start]);
        out.push_str("#br#");
        cursor = end;
        probe = end;
    }

    out.push_str(&input[cursor..]);
    out
}

fn placeholder_to_break(input: &str) -> String {
    input.replace("#br#", "<br/>")
}

fn mermaid_line_break_tag_end(input: &str, start: usize) -> Option<usize> {
    let bytes = input.as_bytes();
    if bytes.get(start) != Some(&b'<')
        || !bytes
            .get(start + 1)
            .is_some_and(|b| b.eq_ignore_ascii_case(&b'b'))
        || !bytes
            .get(start + 2)
            .is_some_and(|b| b.eq_ignore_ascii_case(&b'r'))
    {
        return None;
    }

    let mut cursor = start + 3;
    while cursor < input.len() {
        let ch = input[cursor..].chars().next()?;
        if !is_js_regex_whitespace(ch) {
            break;
        }
        cursor += ch.len_utf8();
    }

    if bytes.get(cursor) == Some(&b'/') {
        cursor += 1;
    }

    (bytes.get(cursor) == Some(&b'>')).then_some(cursor + 1)
}

fn is_js_regex_whitespace(ch: char) -> bool {
    if ('\u{2000}'..='\u{200A}').contains(&ch) {
        return true;
    }

    matches!(
        ch,
        '\u{0009}'
            | '\u{000A}'
            | '\u{000B}'
            | '\u{000C}'
            | '\u{000D}'
            | '\u{0020}'
            | '\u{00A0}'
            | '\u{1680}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202F}'
            | '\u{205F}'
            | '\u{3000}'
            | '\u{FEFF}'
    )
}

fn escape_html_preserving_breaks(text: &str, escape_equals: bool) -> String {
    let with_placeholders = break_to_placeholder(text);
    let mut out = String::with_capacity(with_placeholders.len());
    for ch in with_placeholders.chars() {
        match ch {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '=' if escape_equals => out.push_str("&#61;"),
            _ => out.push(ch),
        }
    }
    placeholder_to_break(&out)
}

fn default_allowed_tags() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| {
        dompurify_defaults::DEFAULT_ALLOWED_TAGS
            .iter()
            .copied()
            .collect()
    })
}

fn default_allowed_attr() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| {
        dompurify_defaults::DEFAULT_ALLOWED_ATTR
            .iter()
            .copied()
            .collect()
    })
}

fn default_uri_safe_attr() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| {
        dompurify_defaults::DEFAULT_URI_SAFE_ATTRIBUTES
            .iter()
            .copied()
            .collect()
    })
}

fn default_data_uri_tags() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| {
        dompurify_defaults::DEFAULT_DATA_URI_TAGS
            .iter()
            .copied()
            .collect()
    })
}

fn is_dompurify_data_attr_name(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("data-") else {
        return false;
    };

    !rest.is_empty() && rest.chars().all(is_dompurify_data_attr_suffix_char)
}

fn is_dompurify_data_attr_suffix_char(ch: char) -> bool {
    // Source: DOMPurify 3.4.12 `DATA_ATTR = /^data-[\-\w.\u00B7-\uFFFF]+$/`.
    matches!(
        ch,
        '-' | '.' | '_' | '0'..='9' | 'A'..='Z' | 'a'..='z'
    ) || ('\u{00B7}'..='\u{FFFF}').contains(&ch)
}

fn is_dompurify_aria_attr_name(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("aria-") else {
        return false;
    };

    !rest.is_empty() && rest.chars().all(is_dompurify_aria_attr_suffix_char)
}

fn is_dompurify_aria_attr_suffix_char(ch: char) -> bool {
    // Source: DOMPurify 3.4.12 `ARIA_ATTR = /^aria-[\-\w]+$/`.
    matches!(ch, '-' | '_' | '0'..='9' | 'A'..='Z' | 'a'..='z')
}

fn remove_dompurify_attr_whitespace(input: &str) -> std::borrow::Cow<'_, str> {
    let Some(first) = input
        .char_indices()
        .find_map(|(idx, ch)| is_dompurify_attr_whitespace(ch).then_some(idx))
    else {
        return std::borrow::Cow::Borrowed(input);
    };

    let mut out = String::with_capacity(input.len());
    out.push_str(&input[..first]);
    out.extend(
        input[first..]
            .chars()
            .filter(|ch| !is_dompurify_attr_whitespace(*ch)),
    );
    std::borrow::Cow::Owned(out)
}

fn is_dompurify_attr_whitespace(ch: char) -> bool {
    // Source: DOMPurify 3.4.12 `ATTR_WHITESPACE`.
    matches!(
        ch,
        '\u{0000}'..='\u{0020}'
            | '\u{00A0}'
            | '\u{1680}'
            | '\u{180E}'
            | '\u{2000}'..='\u{2029}'
            | '\u{205F}'
            | '\u{3000}'
    )
}

fn is_dompurify_script_or_data_uri(value: &str) -> bool {
    // Source: DOMPurify 3.4.12 `IS_SCRIPT_OR_DATA = /^(?:\w+script|data):/i`.
    let Some(colon) = value.find(':') else {
        return false;
    };

    let scheme = &value[..colon];
    if scheme.eq_ignore_ascii_case("data") {
        return true;
    }

    let bytes = scheme.as_bytes();
    let script = b"script";
    if bytes.len() <= script.len()
        || !bytes[bytes.len() - script.len()..].eq_ignore_ascii_case(script)
    {
        return false;
    }

    bytes[..bytes.len() - script.len()]
        .iter()
        .all(|byte| is_js_regex_word_byte(*byte))
}

fn is_dompurify_allowed_uri(value: &str) -> bool {
    // Source: DOMPurify 3.4.12 `IS_ALLOWED_URI`.
    if value.is_empty() {
        return false;
    }

    if has_dompurify_allowed_uri_scheme(value) {
        return true;
    }

    let bytes = value.as_bytes();
    if !bytes[0].is_ascii_alphabetic() {
        return true;
    }

    let mut cursor = 0usize;
    while bytes
        .get(cursor)
        .is_some_and(|byte| is_dompurify_uri_scheme_byte(*byte))
    {
        cursor += 1;
    }

    cursor == bytes.len()
        || bytes
            .get(cursor)
            .is_some_and(|byte| !is_dompurify_uri_scheme_byte(*byte) && *byte != b':')
}

/// Normalizes a URI-bearing attribute value that already exists in the DOM.
///
/// DOMPurify trims every attribute except `value`, validates URI attributes after removing its
/// `ATTR_WHITESPACE` set, and writes the trimmed value back to the DOM. This entry point must not
/// decode character references again: a literal `&colon;` assigned through `setAttribute()` is a
/// literal DOM value, not an encoded colon.
#[doc(hidden)]
pub(crate) fn dompurify_normalize_dom_uri_attribute(value: &str) -> Option<String> {
    let value = dompurify_normalize_dom_attribute_value("href", value);
    let value_no_ws = remove_dompurify_attr_whitespace(value);
    (is_dompurify_allowed_uri(value_no_ws.as_ref()) || value.is_empty()).then(|| value.to_string())
}

/// Applies DOMPurify's pre-validation attribute-value normalization to an existing DOM value.
#[doc(hidden)]
pub(crate) fn dompurify_normalize_dom_attribute_value<'a>(name: &str, value: &'a str) -> &'a str {
    if name.eq_ignore_ascii_case("value") {
        value
    } else {
        trim_ecmascript_whitespace(value)
    }
}

/// Normalizes a URI-bearing attribute read from serialized SVG source.
///
/// Serialized source is decoded exactly once before applying the DOM-value contract.
#[doc(hidden)]
pub(crate) fn dompurify_normalize_serialized_uri_attribute(value: &str) -> Option<String> {
    let decoded_value = decode_attr_html_entities(value);
    dompurify_normalize_dom_uri_attribute(&decoded_value)
}

fn trim_ecmascript_whitespace(value: &str) -> &str {
    value.trim_matches(is_ecmascript_trim_whitespace)
}

fn is_ecmascript_trim_whitespace(ch: char) -> bool {
    matches!(
        ch,
        '\u{0009}'..='\u{000D}'
            | '\u{0020}'
            | '\u{00A0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200A}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202F}'
            | '\u{205F}'
            | '\u{3000}'
            | '\u{FEFF}'
    )
}

fn has_dompurify_allowed_uri_scheme(value: &str) -> bool {
    let bytes = value.as_bytes();
    const ALLOWED_URI_SCHEMES: &[&[u8]] = &[
        b"http:", b"https:", b"ftp:", b"ftps:", b"mailto:", b"tel:", b"callto:", b"sms:", b"cid:",
        b"xmpp:", b"matrix:",
    ];

    ALLOWED_URI_SCHEMES
        .iter()
        .any(|scheme| ascii_case_insensitive_starts_with(bytes, 0, scheme))
}

fn is_dompurify_uri_scheme_byte(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || matches!(byte, b'+' | b'.' | b'-')
}

fn is_js_regex_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

#[derive(Debug, Clone)]
struct DompurifyEffectiveConfig {
    allowed_tags: HashSet<String>,
    allowed_attr: HashSet<String>,
    uri_safe_attr: HashSet<String>,
    data_uri_tags: HashSet<String>,
    forbid_tags: HashSet<String>,
    forbid_attr: HashSet<String>,
    allow_aria_attr: bool,
    allow_data_attr: bool,
    allow_unknown_protocols: bool,
    keep_content: bool,
}

fn dompurify_config_object(
    config: &MermaidConfig,
) -> Option<&serde_json::Map<String, serde_json::Value>> {
    config
        .as_value()
        .as_object()
        .and_then(|o| o.get("dompurifyConfig"))
        .and_then(|v| v.as_object())
}

fn dompurify_extract_string_list(
    dompurify_config: Option<&serde_json::Map<String, serde_json::Value>>,
    key: &str,
) -> Vec<String> {
    dompurify_config
        .and_then(|o| o.get(key))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str())
                .map(|s| s.to_ascii_lowercase())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn dompurify_effective_config(
    config: &MermaidConfig,
    forbid_style_when_unconfigured: bool,
) -> DompurifyEffectiveConfig {
    let dompurify_cfg = dompurify_config_object(config);

    let allow_aria_attr = dompurify_cfg
        .and_then(|o| o.get("ALLOW_ARIA_ATTR"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let allow_data_attr = dompurify_cfg
        .and_then(|o| o.get("ALLOW_DATA_ATTR"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let allow_unknown_protocols = dompurify_cfg
        .and_then(|o| o.get("ALLOW_UNKNOWN_PROTOCOLS"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let keep_content = dompurify_cfg
        .and_then(|o| o.get("KEEP_CONTENT"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let mut allowed_tags: HashSet<String> = if dompurify_cfg
        .and_then(|o| o.get("ALLOWED_TAGS"))
        .and_then(|v| v.as_array())
        .is_some()
    {
        dompurify_extract_string_list(dompurify_cfg, "ALLOWED_TAGS")
            .into_iter()
            .collect()
    } else {
        default_allowed_tags()
            .iter()
            .map(|s| s.to_string())
            .collect()
    };

    for t in dompurify_extract_string_list(dompurify_cfg, "ADD_TAGS") {
        allowed_tags.insert(t);
    }

    if allowed_tags.contains("table") {
        allowed_tags.insert("tbody".to_string());
    }

    let mut allowed_attr: HashSet<String> = if dompurify_cfg
        .and_then(|o| o.get("ALLOWED_ATTR"))
        .and_then(|v| v.as_array())
        .is_some()
    {
        dompurify_extract_string_list(dompurify_cfg, "ALLOWED_ATTR")
            .into_iter()
            .collect()
    } else {
        default_allowed_attr()
            .iter()
            .map(|s| s.to_string())
            .collect()
    };

    for a in dompurify_extract_string_list(dompurify_cfg, "ADD_ATTR") {
        allowed_attr.insert(a);
    }

    let mut uri_safe_attr: HashSet<String> = default_uri_safe_attr()
        .iter()
        .map(|s| s.to_string())
        .collect();
    for a in dompurify_extract_string_list(dompurify_cfg, "ADD_URI_SAFE_ATTR") {
        uri_safe_attr.insert(a);
    }

    let mut data_uri_tags: HashSet<String> = default_data_uri_tags()
        .iter()
        .map(|s| s.to_string())
        .collect();
    for t in dompurify_extract_string_list(dompurify_cfg, "ADD_DATA_URI_TAGS") {
        data_uri_tags.insert(t);
    }

    let mut forbid_tags: HashSet<String> =
        dompurify_extract_string_list(dompurify_cfg, "FORBID_TAGS")
            .into_iter()
            .collect();

    if forbid_style_when_unconfigured && dompurify_cfg.is_none() {
        forbid_tags.insert("style".to_string());
    }

    let forbid_attr: HashSet<String> = dompurify_extract_string_list(dompurify_cfg, "FORBID_ATTR")
        .into_iter()
        .collect();

    DompurifyEffectiveConfig {
        allowed_tags,
        allowed_attr,
        uri_safe_attr,
        data_uri_tags,
        forbid_tags,
        forbid_attr,
        allow_aria_attr,
        allow_data_attr,
        allow_unknown_protocols,
        keep_content,
    }
}

fn dompurify_is_valid_attribute(
    cfg: &DompurifyEffectiveConfig,
    lc_tag: &str,
    lc_name: &str,
    value: &str,
) -> bool {
    // DOMPurify applies FORBID_ATTR before its data-* and aria-* convenience paths.
    // Keeping that priority here makes every accepted attribute route obey one policy.
    if cfg.forbid_attr.contains(lc_name) {
        return false;
    }

    if cfg.allow_data_attr && is_dompurify_data_attr_name(lc_name) {
        return true;
    }

    if cfg.allow_aria_attr && is_dompurify_aria_attr_name(lc_name) {
        return true;
    }

    if !cfg.allowed_attr.contains(lc_name) {
        return false;
    }

    if cfg.uri_safe_attr.contains(lc_name) {
        return true;
    }

    let value_no_ws = remove_dompurify_attr_whitespace(value);

    if is_dompurify_allowed_uri(value_no_ws.as_ref()) {
        return true;
    }

    if matches!(lc_name, "src" | "xlink:href" | "href")
        && lc_tag != "script"
        && value.starts_with("data:")
        && cfg.data_uri_tags.contains(lc_tag)
    {
        return true;
    }

    if cfg.allow_unknown_protocols && !is_dompurify_script_or_data_uri(value_no_ws.as_ref()) {
        return true;
    }

    value.is_empty()
}

fn decode_attr_html_entities(input: &str) -> String {
    // `lol_html` exposes the serialized source spelling. Decode exactly the one layer that a
    // browser parser would consume before DOMPurify reads `Attr.value`.
    htmlize::unescape_attribute(input).into_owned()
}

fn ascii_case_insensitive_starts_with(haystack: &[u8], start: usize, needle: &[u8]) -> bool {
    haystack
        .get(start..start + needle.len())
        .is_some_and(|candidate| {
            candidate
                .iter()
                .zip(needle)
                .all(|(a, b)| a.eq_ignore_ascii_case(b))
        })
}

fn dompurify_like_sanitize_html(text: &str, cfg: &DompurifyEffectiveConfig) -> String {
    if text.is_empty() {
        return text.to_string();
    }

    // `lol_html::rewrite_str` is less permissive than browser parsing (and therefore DOMPurify).
    // In particular, fragments containing a "stray" `<` that does not start a valid tag (e.g.
    // `"foo < bar"` or `"<\""` in a quoted string) can fail to parse. In browsers those `<`
    // tokens are treated as text and serialized as `&lt;`.
    //
    // To stay closer to Mermaid's `sanitizeText` behavior, pre-escape such `<` tokens before
    // running the DOMPurify-like rewrite.
    fn escape_stray_lt(input: &str) -> std::borrow::Cow<'_, str> {
        let bytes = input.as_bytes();
        let mut pos = 0usize;
        while pos < bytes.len() {
            if bytes[pos] == b'<' {
                let next = bytes.get(pos + 1).copied().unwrap_or(b' ');
                let tag_start = next.is_ascii_alphabetic() || matches!(next, b'/' | b'!' | b'?');
                if !tag_start {
                    break;
                }
            }
            pos += 1;
        }
        if pos >= bytes.len() {
            return std::borrow::Cow::Borrowed(input);
        }

        let mut out = String::with_capacity(input.len() + 8);
        let mut last = 0usize;
        let mut i = 0usize;
        while i < bytes.len() {
            if bytes[i] == b'<' {
                let next = bytes.get(i + 1).copied().unwrap_or(b' ');
                let tag_start = next.is_ascii_alphabetic() || matches!(next, b'/' | b'!' | b'?');
                if !tag_start {
                    out.push_str(&input[last..i]);
                    out.push_str("&lt;");
                    i += 1;
                    last = i;
                    continue;
                }
            }
            i += 1;
        }
        out.push_str(&input[last..]);
        std::borrow::Cow::Owned(out)
    }

    let text = escape_stray_lt(text);

    let rewrite_str_settings = RewriteStrSettings::new()
        .append_element_content_handler(element!("script", |el| {
            el.remove();
            Ok(())
        }))
        .append_element_content_handler(element!("iframe", |el| {
            el.remove();
            Ok(())
        }))
        .append_element_content_handler(element!("style", |el| {
            el.remove();
            Ok(())
        }))
        .append_element_content_handler(element!("a", |el| {
            // Mirror Mermaid's DOMPurify hooks:
            // - beforeSanitizeAttributes stores the target in a temporary data-* attribute
            // - afterSanitizeAttributes restores it (only if the data-* survived sanitization)
            if let Some(target) = el.get_attribute("target") {
                let _ = el.set_attribute("data-temp-href-target", &target);
            }
            Ok(())
        }))
        .append_element_content_handler(element!("*", |el| {
            let tag_name = el.tag_name();
            let lc_tag = tag_name.to_ascii_lowercase();

            if !cfg.allowed_tags.contains(&lc_tag) || cfg.forbid_tags.contains(&lc_tag) {
                if cfg.keep_content {
                    el.remove_and_keep_content();
                } else {
                    el.remove();
                }
                return Ok(());
            }

            let attrs: Vec<(String, String)> = el
                .attributes()
                .iter()
                .map(|a| (a.name().to_string(), a.value().to_string()))
                .collect();

            for (name, value) in attrs {
                let lc_name = name.to_ascii_lowercase();
                let parsed_value = decode_attr_html_entities(&value);
                let normalized_value =
                    dompurify_normalize_dom_attribute_value(&lc_name, &parsed_value);
                if !dompurify_is_valid_attribute(cfg, &lc_tag, &lc_name, normalized_value) {
                    el.remove_attribute(&name);
                    continue;
                }

                if normalized_value != value {
                    let _ = el.set_attribute(&name, normalized_value);
                }
            }

            if lc_tag == "a"
                && let Some(target) = el.get_attribute("data-temp-href-target")
            {
                let _ = el.set_attribute("target", &target);
                el.remove_attribute("data-temp-href-target");
                if target == "_blank" {
                    let _ = el.set_attribute("rel", "noopener");
                }
            }

            Ok(())
        }));

    rewrite_str(text.as_ref(), rewrite_str_settings).unwrap_or_else(|_| text.into_owned())
}

pub fn remove_script(text: &str) -> String {
    if text.is_empty() {
        return text.to_string();
    }
    if !text.contains('<') {
        return text.to_string();
    }
    let cfg = dompurify_effective_config(
        &MermaidConfig::from_value(serde_json::Value::Object(serde_json::Map::new())),
        false,
    );
    dompurify_like_sanitize_html(text, &cfg)
}

fn effective_html_labels(config: &MermaidConfig) -> bool {
    config
        .get_bool("htmlLabels")
        .or_else(|| config.get_bool("flowchart.htmlLabels"))
        .unwrap_or(true)
}

fn is_unformatted_ascii_paragraph(text: &str) -> bool {
    let Some(body) = text
        .strip_prefix("<p>")
        .and_then(|text| text.strip_suffix("</p>"))
    else {
        return false;
    };
    let bytes = body.as_bytes();
    bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || *byte == b' ')
}

fn sanitizer_preserves_unformatted_ascii_paragraph(config: &MermaidConfig) -> bool {
    if effective_html_labels(config)
        && !matches!(
            config.get_str("securityLevel"),
            Some("antiscript" | "strict" | "sandbox" | "loose")
        )
    {
        return false;
    }

    let dompurify = dompurify_config_object(config);
    let list_contains = |key: &str, expected: &str| {
        dompurify
            .and_then(|object| object.get(key))
            .and_then(serde_json::Value::as_array)
            .is_some_and(|values| {
                values
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .any(|value| value.eq_ignore_ascii_case(expected))
            })
    };
    let replaces_default_allowed_tags = dompurify
        .and_then(|object| object.get("ALLOWED_TAGS"))
        .and_then(serde_json::Value::as_array)
        .is_some();
    let paragraph_is_allowed = !replaces_default_allowed_tags
        || list_contains("ALLOWED_TAGS", "p")
        || list_contains("ADD_TAGS", "p");

    paragraph_is_allowed && !list_contains("FORBID_TAGS", "p")
}

fn sanitize_more(text: &str, config: &MermaidConfig) -> String {
    let html_labels_enabled = effective_html_labels(config);
    if !html_labels_enabled {
        return text.to_string();
    }

    let level = config.get_str("securityLevel");
    if matches!(level, Some("antiscript" | "strict" | "sandbox")) {
        return remove_script(text);
    }

    if level != Some("loose") {
        return escape_html_preserving_breaks(text, true);
    }

    text.to_string()
}

pub fn sanitize_text(text: &str, config: &MermaidConfig) -> String {
    if text.is_empty() {
        return text.to_string();
    }
    // This grammar contains no user-controlled markup. Skip DOM parsing only when the configured
    // policy also guarantees that the sole generated `<p>` element is preserved unchanged.
    if is_unformatted_ascii_paragraph(text)
        && sanitizer_preserves_unformatted_ascii_paragraph(config)
    {
        return text.to_string();
    }

    let t = sanitize_more(text, config);
    if !t.contains('<') {
        return t;
    }

    let cfg = dompurify_effective_config(config, true);
    dompurify_like_sanitize_html(&t, &cfg)
}

pub fn sanitize_text_or_array(
    value: &serde_json::Value,
    config: &MermaidConfig,
) -> serde_json::Value {
    match value {
        serde_json::Value::String(s) => serde_json::Value::String(sanitize_text(s, config)),
        serde_json::Value::Array(arr) => serde_json::Value::Array(
            arr.iter()
                .flat_map(|v| match v {
                    serde_json::Value::Array(inner) => inner.to_vec(),
                    _ => vec![v.clone()],
                })
                .map(|v| match v {
                    serde_json::Value::String(s) => {
                        serde_json::Value::String(sanitize_text(&s, config))
                    }
                    other => other,
                })
                .collect(),
        ),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn cfg_strict() -> MermaidConfig {
        MermaidConfig::from_value(json!({
            "securityLevel": "strict",
            "flowchart": { "htmlLabels": true }
        }))
    }

    #[test]
    fn break_to_placeholder_matches_mermaid_line_break_regex_shape() {
        assert_eq!(
            break_to_placeholder("A<br>B<BR/>C<br \t/>D<br   >E"),
            "A#br#B#br#C#br#D#br#E"
        );
        assert_eq!(
            break_to_placeholder("<br / > <brx> </br> < br>"),
            "<br / > <brx> </br> < br>"
        );
        assert_eq!(
            break_to_placeholder("A<br\u{00A0}/>B<br\u{FEFF}>C"),
            "A#br#B#br#C"
        );
    }

    #[test]
    fn sanitize_more_uses_root_html_labels_before_deprecated_flowchart_fallback() {
        let root_false = MermaidConfig::from_value(json!({
            "securityLevel": "strict",
            "htmlLabels": false,
            "flowchart": { "htmlLabels": true }
        }));
        assert_eq!(
            sanitize_more(r#"<b a=1>ok</b>"#, &root_false),
            r#"<b a=1>ok</b>"#
        );

        let root_true = MermaidConfig::from_value(json!({
            "securityLevel": "strict",
            "htmlLabels": true,
            "flowchart": { "htmlLabels": false }
        }));
        assert_eq!(
            sanitize_more(r#"<b a=1>ok</b>"#, &root_true),
            r#"<b>ok</b>"#
        );

        let deprecated_false = MermaidConfig::from_value(json!({
            "securityLevel": "strict",
            "flowchart": { "htmlLabels": false }
        }));
        assert_eq!(
            sanitize_more(r#"<b a=1>ok</b>"#, &deprecated_false),
            r#"<b a=1>ok</b>"#
        );
    }

    #[test]
    fn decode_attr_entities_matches_browser_attribute_semantics_without_regex() {
        assert_eq!(
            decode_attr_html_entities("javascript&colon;alert&NewLine;one&Tab;two"),
            "javascript:alert\none\ttwo"
        );
        assert_eq!(
            decode_attr_html_entities("a&#58;b&#00058;c&#058d"),
            "a:b:c:d"
        );
        let high_code_point = char::from_u32(0x3adef).expect("valid HTML numeric reference");
        assert_eq!(
            decode_attr_html_entities("a&#x3a;b&#X0003A;c&#x03adef"),
            format!("a:b:c{high_code_point}")
        );
        assert_eq!(decode_attr_html_entities("&Colon;"), "∷");
        assert_eq!(decode_attr_html_entities("&COLON;"), "&COLON;");
        assert_eq!(
            decode_attr_html_entities("&colon &newline &tab &#59; &#x3b;"),
            "&colon &newline &tab ; ;"
        );
        assert_eq!(decode_attr_html_entities("jav&#x61;script:"), "javascript:");
    }

    #[test]
    fn dompurify_attr_name_matchers_follow_source_regex_boundaries() {
        assert!(is_dompurify_data_attr_name("data-x"));
        assert!(is_dompurify_data_attr_name("data-x.y_9-"));
        assert!(is_dompurify_data_attr_name("data-\u{00B7}"));
        assert!(is_dompurify_data_attr_name("data-\u{FFFF}"));
        assert!(!is_dompurify_data_attr_name("data-"));
        assert!(!is_dompurify_data_attr_name("data-\u{00B6}"));
        assert!(!is_dompurify_data_attr_name("data-x:y"));
        assert!(!is_dompurify_data_attr_name("data-\u{10000}"));

        assert!(is_dompurify_aria_attr_name("aria-label"));
        assert!(is_dompurify_aria_attr_name("aria-foo_bar"));
        assert!(!is_dompurify_aria_attr_name("aria-"));
        assert!(!is_dompurify_aria_attr_name("aria.label"));
        assert!(!is_dompurify_aria_attr_name("aria-\u{00B7}"));
    }

    #[test]
    fn dompurify_attr_whitespace_cleanup_matches_source_regex_boundaries() {
        assert_eq!(
            remove_dompurify_attr_whitespace(
                "java\u{0000}\u{0020}\u{00A0}\u{1680}\u{180E}\u{2000}\u{2029}\u{205F}\u{3000}script:"
            ),
            "javascript:"
        );
        assert_eq!(
            remove_dompurify_attr_whitespace("java\u{0021}script:"),
            "java\u{0021}script:"
        );
        assert_eq!(
            remove_dompurify_attr_whitespace("java\u{202A}script:"),
            "java\u{202A}script:"
        );
        assert_eq!(
            remove_dompurify_attr_whitespace("java\u{FEFF}script:"),
            "java\u{FEFF}script:"
        );
    }

    #[test]
    fn dompurify_script_or_data_uri_matches_source_regex_boundaries() {
        assert!(is_dompurify_script_or_data_uri("javascript:alert(1)"));
        assert!(is_dompurify_script_or_data_uri("JavaSCRIPT:alert(1)"));
        assert!(is_dompurify_script_or_data_uri("vbscript:alert(1)"));
        assert!(is_dompurify_script_or_data_uri("_script:alert(1)"));
        assert!(is_dompurify_script_or_data_uri("1script:alert(1)"));
        assert!(is_dompurify_script_or_data_uri("data:text/html,alert(1)"));
        assert!(is_dompurify_script_or_data_uri("DATA:text/html,alert(1)"));
        assert!(!is_dompurify_script_or_data_uri("script:alert(1)"));
        assert!(!is_dompurify_script_or_data_uri("java-script:alert(1)"));
        assert!(!is_dompurify_script_or_data_uri(
            "jav\u{00E1}script:alert(1)"
        ));
        assert!(!is_dompurify_script_or_data_uri("datax:text/html,alert(1)"));
        assert!(!is_dompurify_script_or_data_uri("javascript"));
    }

    #[test]
    fn dompurify_allowed_uri_matches_source_regex_boundaries() {
        for uri in [
            "http://example.test",
            "https://example.test",
            "ftp://example.test",
            "ftps://example.test",
            "mailto:user@example.test",
            "tel:+123",
            "callto:user",
            "sms:+123",
            "cid:content-id",
            "xmpp:user@example.test",
            "matrix:r/example:example.test",
            "MATRIX:r/example:example.test",
            "/relative",
            "#fragment",
            "?query",
            "1-relative",
            ":colon-relative",
            "abc",
            "abc/path",
            "abc?query",
            "abc123:allowed-by-source-prefix",
            "abc_def:allowed-by-source-prefix",
        ] {
            assert!(is_dompurify_allowed_uri(uri), "{uri}");
        }

        for uri in [
            "",
            "javascript:alert(1)",
            "data:text/html,1",
            "foo:bar",
            "abc+def:bar",
            "abc.def:bar",
            "abc-def:bar",
        ] {
            assert!(!is_dompurify_allowed_uri(uri), "{uri}");
        }
    }

    #[test]
    fn dompurify_uri_attribute_normalization_preserves_the_representation_layer() {
        for value in [
            "https://example.test",
            "jav&#x61;script:alert(1)",
            "javascript&#58;alert(1)",
            "javascript&colon;alert(1)",
        ] {
            assert_eq!(
                dompurify_normalize_dom_uri_attribute(value).as_deref(),
                Some(value),
                "DOM: {value}"
            );
        }
        assert_eq!(
            dompurify_normalize_dom_uri_attribute(""),
            Some(String::new())
        );
        assert_eq!(
            dompurify_normalize_dom_uri_attribute("  https://example.test  "),
            Some("https://example.test".into())
        );
        assert_eq!(
            dompurify_normalize_dom_uri_attribute(" \t\n "),
            Some(String::new())
        );
        for value in [
            "java\nscript:alert(1)",
            "java\tscript:alert(1)",
            "\u{FEFF}javascript:alert(1)",
            "javascript:alert(1)",
            "data:text/html,alert(1)",
            "vbscript:alert(1)",
            "unknown:ticket",
            "about:blank",
        ] {
            assert_eq!(
                dompurify_normalize_dom_uri_attribute(value),
                None,
                "DOM: {value}"
            );
        }

        assert_eq!(
            dompurify_normalize_serialized_uri_attribute("jav&#x61;script:alert(1)"),
            None
        );
        assert_eq!(
            dompurify_normalize_serialized_uri_attribute("jav&amp;#x61;script:alert(1)"),
            Some("jav&#x61;script:alert(1)".into())
        );
        assert_eq!(
            dompurify_normalize_serialized_uri_attribute("javascript&amp;colon;ticket"),
            Some("javascript&colon;ticket".into())
        );
        assert_eq!(
            dompurify_normalize_serialized_uri_attribute("javascript&Colon;ticket"),
            Some("javascript∷ticket".into())
        );
        assert_eq!(
            dompurify_normalize_serialized_uri_attribute(""),
            Some(String::new())
        );
    }

    #[test]
    fn remove_script_strips_script_blocks_and_javascript_urls_and_events() {
        let label_string = r#"1
		Act1: Hello 1<script src="http://abc.com/script1.js"></script>1
		<b>Act2</b>:
		1<script>
			alert('script run......');
		</script>1
	1"#;

        let exactly_string = r#"1
		Act1: Hello 11
		<b>Act2</b>:
		11
	1"#;

        assert_eq!(remove_script(label_string).trim(), exactly_string);

        let url_in = r#"This is a <a href="javascript:runHijackingScript();">clean link</a> + <a href="javascript:runHijackingScript();">clean link</a>
  and <a href="javascript&colon;bypassedMining();">me too</a>"#;
        let url_out = r#"This is a <a>clean link</a> + <a>clean link</a>
  and <a>me too</a>"#;
        assert_eq!(remove_script(url_in).trim(), url_out);

        assert_eq!(
            remove_script(r#"<img onerror="alert('hello');">"#).trim(),
            "<img>"
        );
    }

    #[test]
    fn remove_script_decodes_colon_entities_before_url_validation_without_regex() {
        assert_eq!(
            remove_script(
                r#"<a href="javascript&#58;alert(1)">decimal</a><a href="javascript&#x3A;alert(1)">hex</a>"#
            ),
            "<a>decimal</a><a>hex</a>"
        );
    }

    #[test]
    fn remove_script_preserves_target_and_adds_noopener_for_blank() {
        assert_eq!(
            remove_script(
                r#"<a href="https://mermaid.js.org/" target="_blank">note about mermaid</a>"#
            )
            .trim(),
            r#"<a href="https://mermaid.js.org/" target="_blank" rel="noopener">note about mermaid</a>"#
        );

        assert_eq!(
            remove_script(
                r#"<a href="https://mermaid.js.org/" target="_self">note about mermaid</a>"#
            )
            .trim(),
            r#"<a href="https://mermaid.js.org/" target="_self">note about mermaid</a>"#
        );
    }

    #[test]
    fn remove_script_removes_iframes() {
        let out = remove_script(
            r#"<iframe src="http://abc.com/script1.js"></iframe>
    <iframe src="http://example.com/iframeexample"></iframe>"#,
        );
        assert_eq!(out.trim(), "");
    }

    #[test]
    fn sanitize_text_strict_runs_remove_script_and_forbids_style() {
        let cfg = cfg_strict();
        assert_eq!(
            sanitize_text(r#"<style>.x{color:red}</style><b>ok</b>"#, &cfg),
            "<b>ok</b>"
        );
        assert!(
            !sanitize_text("javajavascript:script:alert(1)", &cfg).contains("javascript:alert(1)")
        );
    }

    #[test]
    fn sanitize_text_matches_mermaid_common_spec_minimally() {
        let cfg = MermaidConfig::from_value(json!({
            "securityLevel": "strict",
            "flowchart": { "htmlLabels": true }
        }));
        let malicious = "javajavascript:script:alert(1)";
        let out = sanitize_text(malicious, &cfg);
        assert!(!out.contains("javascript:alert(1)"));
    }

    #[test]
    fn unformatted_ascii_paragraph_fast_path_obeys_the_effective_sanitizer_policy() {
        let input = "<p>ASCII 123  Label</p>";
        for (config, expected) in [
            (json!({ "securityLevel": "strict" }), input),
            (json!({ "securityLevel": "antiscript" }), input),
            (json!({ "securityLevel": "sandbox" }), input),
            (json!({ "securityLevel": "loose" }), input),
            (
                json!({
                    "securityLevel": "strict",
                    "dompurifyConfig": { "FORBID_TAGS": ["p"] }
                }),
                "ASCII 123  Label",
            ),
            (
                json!({
                    "securityLevel": "loose",
                    "dompurifyConfig": { "ALLOWED_TAGS": [] }
                }),
                "ASCII 123  Label",
            ),
            (
                json!({
                    "securityLevel": "loose",
                    "dompurifyConfig": {
                        "ALLOWED_TAGS": [],
                        "ADD_TAGS": ["P"]
                    }
                }),
                input,
            ),
            (
                json!({
                    "securityLevel": "loose",
                    "dompurifyConfig": {
                        "FORBID_TAGS": ["p"],
                        "KEEP_CONTENT": false
                    }
                }),
                "",
            ),
            (
                json!({
                    "securityLevel": "custom",
                    "htmlLabels": false
                }),
                input,
            ),
        ] {
            let config = MermaidConfig::from_value(config);
            assert_eq!(sanitize_text(input, &config), expected);
        }

        let unknown_security =
            MermaidConfig::from_value(json!({ "securityLevel": "custom", "htmlLabels": true }));
        assert_ne!(sanitize_text(input, &unknown_security), input);
    }

    #[test]
    fn sanitize_text_preserves_mermaid_line_break_tags_without_regex() {
        let cfg = MermaidConfig::from_value(json!({
            "flowchart": { "htmlLabels": true }
        }));
        let out = sanitize_text("A<br \t/>B<BR>C", &cfg);
        assert!(out.contains("A<br"));
        assert!(out.contains(">B<br"));
        assert!(out.ends_with(">C"));
        assert!(!out.contains("&lt;br"));
    }

    #[test]
    fn sanitize_text_sandbox_runs_remove_script_like_mermaid() {
        let cfg = MermaidConfig::from_value(json!({
            "securityLevel": "sandbox",
            "flowchart": { "htmlLabels": true }
        }));
        let out = sanitize_text(r#"<b a=1>ok</b><br/>x"#, &cfg);
        assert!(out.contains("<b"));
        assert!(out.contains("ok"));
        assert!(out.contains("<br"));
        assert!(!out.contains("&lt;"));
        assert!(!out.contains("&equals;"));
    }

    #[test]
    fn sanitize_text_dompurify_config_add_attr_allows_onclick_like_dompurify() {
        // Mermaid supports passing `dompurifyConfig` through to DOMPurify.
        // Reference: `repo-ref/mermaid/packages/mermaid/src/diagrams/common/common.ts`.
        let cfg = MermaidConfig::from_value(json!({
            "securityLevel": "loose",
            "flowchart": { "htmlLabels": true },
            "dompurifyConfig": { "ADD_ATTR": ["onclick"] }
        }));
        assert_eq!(
            sanitize_text(r#"<b onclick="alert(1)">ok</b>"#, &cfg),
            r#"<b onclick="alert(1)">ok</b>"#
        );
    }

    #[test]
    fn sanitize_text_dompurify_config_forbid_attr_removes_href_like_dompurify() {
        let cfg = MermaidConfig::from_value(json!({
            "securityLevel": "loose",
            "flowchart": { "htmlLabels": true },
            "dompurifyConfig": { "FORBID_ATTR": ["href"] }
        }));
        assert_eq!(sanitize_text(r#"<a href="/x">y</a>"#, &cfg), "<a>y</a>");
    }

    #[test]
    fn sanitize_text_dompurify_forbid_attr_overrides_aria_and_data_defaults() {
        let cfg = MermaidConfig::from_value(json!({
            "securityLevel": "loose",
            "flowchart": { "htmlLabels": true },
            "dompurifyConfig": {
                "FORBID_ATTR": ["aria-label", "data-secret"]
            }
        }));

        assert_eq!(
            sanitize_text(
                r#"<span aria-label="visible" data-secret="hidden" title="kept">text</span>"#,
                &cfg,
            ),
            r#"<span title="kept">text</span>"#,
        );
    }

    #[test]
    fn sanitize_text_dompurify_defaults_strip_unknown_attribute_and_keep_style_attr() {
        let cfg = MermaidConfig::from_value(json!({
            "securityLevel": "loose",
            "flowchart": { "htmlLabels": true }
        }));
        assert_eq!(sanitize_text(r#"<b foo="bar">ok</b>"#, &cfg), "<b>ok</b>");
        assert_eq!(
            sanitize_text(r#"<b style="color:red">ok</b>"#, &cfg),
            r#"<b style="color:red">ok</b>"#
        );
    }

    #[test]
    fn sanitize_text_dompurify_defaults_remove_unknown_tag_keep_content() {
        let cfg = MermaidConfig::from_value(json!({
            "securityLevel": "loose",
            "flowchart": { "htmlLabels": true }
        }));
        assert_eq!(
            sanitize_text(r#"<custom-tag onclick="alert(1)">x</custom-tag>"#, &cfg),
            "x"
        );
    }

    #[test]
    fn sanitize_text_dompurify_defaults_allow_aria_and_data_attrs() {
        let cfg = MermaidConfig::from_value(json!({
            "securityLevel": "loose",
            "flowchart": { "htmlLabels": true }
        }));
        let out = sanitize_text(
            r#"<b data-x="1" data-x.y_9-="2" aria-label="x" aria-foo_bar="y" data-x:y="bad" aria.foo="bad" foo="bar">ok</b>"#,
            &cfg,
        );
        assert!(!out.contains("foo="));
        assert!(!out.contains("data-x:y="));
        assert!(!out.contains("aria.foo="));
        assert!(out.contains(r#"data-x="1""#));
        assert!(out.contains(r#"data-x.y_9-="2""#));
        assert!(out.contains(r#"aria-label="x""#));
        assert!(out.contains(r#"aria-foo_bar="y""#));
        assert!(out.starts_with("<b"));
        assert!(out.ends_with(">ok</b>"));

        let cfg = MermaidConfig::from_value(json!({
            "securityLevel": "loose",
            "flowchart": { "htmlLabels": true },
            "dompurifyConfig": { "ALLOW_DATA_ATTR": false, "ALLOW_ARIA_ATTR": false }
        }));
        assert_eq!(
            sanitize_text(
                r#"<b data-x="1" data-x.y_9-="2" aria-label="x">ok</b>"#,
                &cfg
            ),
            "<b>ok</b>"
        );
    }

    #[test]
    fn sanitize_text_allows_svg_elements_inside_svg_container() {
        let cfg = MermaidConfig::from_value(json!({
            "securityLevel": "strict",
            "flowchart": { "htmlLabels": true }
        }));
        let out = sanitize_text(
            r#"<svg><path fill="currentColor" d="M224 0c-17.7 0-32 14.3-32 32v19.2"/></svg>"#,
            &cfg,
        );
        assert!(out.contains("<svg"));
        assert!(out.contains("<path"));
        assert!(out.contains("fill=\"currentColor\""));
        assert!(out.contains("d=\"M224 0c-17.7"));
    }

    #[test]
    fn sanitize_text_strips_javascript_xlink_href_in_svg() {
        let cfg = MermaidConfig::from_value(json!({
            "securityLevel": "strict",
            "flowchart": { "htmlLabels": true }
        }));
        let out = sanitize_text(
            r#"<svg><a xlink:href="javascript:alert(1)">x</a></svg>"#,
            &cfg,
        );
        assert!(out.contains("<svg"));
        assert!(out.contains("<a"));
        assert!(out.contains(">x</a>"));
        assert!(!out.to_ascii_lowercase().contains("javascript:"));
        assert!(!out.to_ascii_lowercase().contains("xlink:href"));
    }

    #[test]
    fn sanitize_text_strips_javascript_href_after_dompurify_attr_whitespace_cleanup() {
        let cfg = MermaidConfig::from_value(json!({
            "securityLevel": "strict",
            "flowchart": { "htmlLabels": true }
        }));
        let out = sanitize_text("<a href=\"java\u{00A0}script:alert(1)\">x</a>", &cfg);
        assert!(out.contains("<a"));
        assert!(out.contains(">x</a>"));
        assert!(!out.to_ascii_lowercase().contains("javascript:"));
        assert!(!out.to_ascii_lowercase().contains("href="));
    }

    #[test]
    fn sanitize_text_strips_script_schemes_obfuscated_with_html_character_references() {
        let cfg = MermaidConfig::from_value(json!({
            "securityLevel": "strict",
            "flowchart": { "htmlLabels": true }
        }));
        let out = sanitize_text(
            concat!(
                r#"<a href="jav&#x61;script:alert(1)">hex</a>"#,
                r#"<a href="java&#115;cript:alert(2)">decimal</a>"#,
                r#"<a href="https://mermaid.js.org/">safe</a>"#,
            ),
            &cfg,
        );

        assert!(out.contains(">hex</a>"));
        assert!(out.contains(">decimal</a>"));
        assert_eq!(out.matches("href=").count(), 1, "{out}");
        assert!(out.contains(r#"href="https://mermaid.js.org/""#));
    }

    #[test]
    fn sanitize_text_dompurify_allowed_uri_matches_pinned_source_schemes() {
        let cfg = MermaidConfig::from_value(json!({
            "securityLevel": "loose",
            "flowchart": { "htmlLabels": true }
        }));
        let out = sanitize_text(
            r#"<a href="matrix:r/example:example.test">matrix</a><a href="foo:bar">foo</a>"#,
            &cfg,
        );
        assert!(out.contains(r#"href="matrix:r/example:example.test""#));
        assert!(out.contains(">matrix</a>"));
        assert!(out.contains(">foo</a>"));
        assert!(!out.contains(r#"href="foo:bar""#));
    }

    #[test]
    fn sanitize_text_allow_unknown_protocols_still_blocks_script_or_data_uri() {
        let cfg = MermaidConfig::from_value(json!({
            "securityLevel": "loose",
            "flowchart": { "htmlLabels": true },
            "dompurifyConfig": { "ALLOW_UNKNOWN_PROTOCOLS": true }
        }));
        let out = sanitize_text(
            r#"<a href="foo:bar">ok</a><a href="javascript:alert(1)">bad</a><a href="data:text/html,1">data</a>"#,
            &cfg,
        );
        assert!(out.contains(r#"href="foo:bar""#));
        assert!(out.contains(">ok</a>"));
        assert!(out.contains(">bad</a>"));
        assert!(out.contains(">data</a>"));
        assert!(!out.to_ascii_lowercase().contains("javascript:"));
        assert!(!out.to_ascii_lowercase().contains("data:text/html"));
    }

    #[test]
    fn sanitize_text_dompurify_hook_target_depends_on_allow_data_attr() {
        let cfg = MermaidConfig::from_value(json!({
            "securityLevel": "strict",
            "flowchart": { "htmlLabels": true }
        }));
        let out = sanitize_text(
            r#"<a href="https://mermaid.js.org/" target="_blank">x</a>"#,
            &cfg,
        );
        assert!(out.contains("target=\"_blank\""));
        assert!(out.contains("rel=\"noopener\""));

        let cfg = MermaidConfig::from_value(json!({
            "securityLevel": "strict",
            "flowchart": { "htmlLabels": true },
            "dompurifyConfig": { "ALLOW_DATA_ATTR": false }
        }));
        let out = sanitize_text(
            r#"<a href="https://mermaid.js.org/" target="_blank">x</a>"#,
            &cfg,
        );
        assert!(!out.contains("target=\"_blank\""));
        // In Mermaid strict mode, the `removeScript()` pass adds `rel=noopener` before the second
        // DOMPurify pass runs, so `rel` can remain even if the target is later removed.
        assert!(out.contains("rel=\"noopener\""));
    }

    #[test]
    fn sanitize_text_dompurify_keep_content_false_removes_custom_element_content() {
        let cfg = MermaidConfig::from_value(json!({
            "securityLevel": "loose",
            "flowchart": { "htmlLabels": true },
            "dompurifyConfig": { "KEEP_CONTENT": false }
        }));
        assert_eq!(sanitize_text("<custom-tag>x</custom-tag>", &cfg), "");
    }
}
