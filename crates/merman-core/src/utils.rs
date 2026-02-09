use crate::MermaidConfig;
use regex::Regex;
use url::Url;

pub const BLANK_URL: &str = "about:blank";

fn html_ctrl_entity_regex() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)&(newline|tab);").expect("valid regex"))
}

fn whitespace_escape_chars_regex() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(\\|%5c)((%(6e|72|74))|[nrt])").expect("valid regex"))
}

fn is_ctrl_character_like(ch: char) -> bool {
    matches!(ch,
        '\u{0000}'..='\u{001F}'
        | '\u{007F}'..='\u{009F}'
        | '\u{2000}'..='\u{200D}'
        | '\u{FEFF}'
    )
}

fn strip_ctrl_characters_like(input: &str) -> String {
    input
        .chars()
        .filter(|&ch| !is_ctrl_character_like(ch))
        .collect()
}

fn contains_ctrl_characters_like(input: &str) -> bool {
    input.chars().any(is_ctrl_character_like)
}

fn is_ascii_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn decode_html_characters_like(input: &str) -> String {
    let without_ctrl = strip_ctrl_characters_like(input);

    let bytes = without_ctrl.as_bytes();
    let mut out = String::with_capacity(without_ctrl.len());

    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'&' && i + 1 < bytes.len() && bytes[i + 1] == b'#' {
            let mut j = i + 2;
            while j < bytes.len() && is_ascii_word_byte(bytes[j]) {
                j += 1;
            }
            if j > i + 2 {
                let dec = &without_ctrl[i + 2..j];
                let value = dec.parse::<u32>().unwrap_or(0);
                let value = value & 0xFFFF;
                out.push(char::from_u32(value).unwrap_or('\u{0000}'));

                i = j;
                if i < bytes.len() && bytes[i] == b';' {
                    i += 1;
                }
                continue;
            }
        }

        let ch = without_ctrl[i..].chars().next().expect("valid utf-8");
        out.push(ch);
        i += ch.len_utf8();
    }

    out
}

fn decode_uri_component_like(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());

    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len() {
                return input.to_string();
            }
            let hi = bytes[i + 1];
            let lo = bytes[i + 2];
            let Some(hi) = from_hex_byte(hi) else {
                return input.to_string();
            };
            let Some(lo) = from_hex_byte(lo) else {
                return input.to_string();
            };
            out.push((hi << 4) | lo);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }

    String::from_utf8(out).unwrap_or_else(|_| input.to_string())
}

fn from_hex_byte(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn contains_html_entity_like(input: &str) -> bool {
    let bytes = input.as_bytes();
    let mut i = 0usize;
    while i + 2 < bytes.len() {
        if bytes[i] == b'&' && bytes[i + 1] == b'#' && is_ascii_word_byte(bytes[i + 2]) {
            return true;
        }
        i += 1;
    }
    false
}

fn url_scheme_like(input: &str) -> Option<&str> {
    let lower = input.to_ascii_lowercase();

    let last_colon = input.rfind(':');
    let last_entity = lower.rfind("&colon;");

    let mut best_end = None::<usize>;

    if let Some(idx) = last_colon {
        best_end = Some(idx + 1);
    }

    if let Some(idx) = last_entity {
        let end = idx + "&colon;".len();
        if best_end.is_none_or(|cur| end > cur) {
            best_end = Some(end);
        }
    }

    best_end.map(|end| &input[..end])
}

fn is_invalid_protocol_like(url_scheme: &str) -> bool {
    let lower = url_scheme.to_ascii_lowercase();
    let trimmed = lower.trim();
    let trimmed = trimmed.trim_start_matches(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'));

    trimmed.starts_with("javascript")
        || trimmed.starts_with("data")
        || trimmed.starts_with("vbscript")
}

pub fn sanitize_url(url: &str) -> String {
    // Ported to match `@braintree/sanitize-url@7.1.1` behavior (Mermaid dependency).
    // Reference: `repo-ref/sanitize-url@v7.1.1`.
    if url.is_empty() {
        return BLANK_URL.to_string();
    }

    let mut decoded_url = decode_uri_component_like(url.trim());

    loop {
        decoded_url = decode_html_characters_like(&decoded_url);
        decoded_url = html_ctrl_entity_regex()
            .replace_all(&decoded_url, "")
            .to_string();
        decoded_url = strip_ctrl_characters_like(&decoded_url);
        decoded_url = whitespace_escape_chars_regex()
            .replace_all(&decoded_url, "")
            .to_string();
        decoded_url = decoded_url.trim().to_string();
        decoded_url = decode_uri_component_like(&decoded_url);

        let chars_to_decode = contains_ctrl_characters_like(&decoded_url)
            || contains_html_entity_like(&decoded_url)
            || html_ctrl_entity_regex().is_match(&decoded_url)
            || whitespace_escape_chars_regex().is_match(&decoded_url);

        if !chars_to_decode {
            break;
        }
    }

    let sanitized_url = decoded_url;
    if sanitized_url.is_empty() {
        return BLANK_URL.to_string();
    }

    if matches!(sanitized_url.as_bytes().first(), Some(b'.' | b'/')) {
        return sanitized_url;
    }

    let trimmed_url = sanitized_url.trim_start();
    let Some(url_scheme) = url_scheme_like(trimmed_url) else {
        return sanitized_url;
    };

    let url_scheme = url_scheme.to_ascii_lowercase();
    let url_scheme = url_scheme.trim();

    if is_invalid_protocol_like(url_scheme) {
        return BLANK_URL.to_string();
    }

    let back_sanitized = trimmed_url.replace('\\', "/");

    if url_scheme == "mailto:" || url_scheme.contains("://") {
        return back_sanitized;
    }

    if url_scheme == "http:" || url_scheme == "https:" {
        let Ok(mut parsed) = Url::parse(&back_sanitized) else {
            return BLANK_URL.to_string();
        };

        let scheme = parsed.scheme().to_ascii_lowercase();
        let _ = parsed.set_scheme(&scheme);
        if let Some(host) = parsed.host_str() {
            let lower_host = host.to_ascii_lowercase();
            let _ = parsed.set_host(Some(&lower_host));
        }

        return parsed.to_string();
    }

    back_sanitized
}

pub fn format_url(link_str: &str, config: &MermaidConfig) -> Option<String> {
    let url = link_str.trim();
    if url.is_empty() {
        return None;
    }
    if config.get_str("securityLevel") != Some("loose") {
        return Some(sanitize_url(url));
    }
    Some(url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn format_url_matches_mermaid_utils_spec() {
        let url = "https://mermaid-js.github.io/mermaid/#/";
        let cfg_loose = MermaidConfig::from_value(json!({ "securityLevel": "loose" }));
        let cfg_strict = MermaidConfig::from_value(json!({ "securityLevel": "strict" }));
        assert_eq!(format_url(url, &cfg_loose).as_deref(), Some(url));
        assert_eq!(format_url(url, &cfg_strict).as_deref(), Some(url));

        let anchor = "#interaction";
        assert_eq!(format_url(anchor, &cfg_loose).as_deref(), Some(anchor));
        assert_eq!(format_url(anchor, &cfg_strict).as_deref(), Some(anchor));

        let mailto = "mailto:user@user.user";
        assert_eq!(format_url(mailto, &cfg_loose).as_deref(), Some(mailto));
        assert_eq!(format_url(mailto, &cfg_strict).as_deref(), Some(mailto));

        let other = "notes://do-your-thing/id";
        assert_eq!(format_url(other, &cfg_loose).as_deref(), Some(other));
        assert_eq!(format_url(other, &cfg_strict).as_deref(), Some(other));

        let js = r#"javascript:alert("test")"#;
        assert_eq!(format_url(js, &cfg_loose).as_deref(), Some(js));
        assert_eq!(format_url(js, &cfg_strict).as_deref(), Some("about:blank"));
    }

    #[test]
    fn sanitize_url_matches_braintree_sanitize_url_7_1_1() {
        assert_eq!(
            sanitize_url("http://example.com/path/to:something"),
            "http://example.com/path/to:something"
        );
        assert_eq!(
            sanitize_url("http://example.com:4567/path/to:something"),
            "http://example.com:4567/path/to:something"
        );
        assert_eq!(sanitize_url("https://example.com"), "https://example.com/");
        assert_eq!(
            sanitize_url("https://example.com:4567/path/to:something"),
            "https://example.com:4567/path/to:something"
        );
        assert_eq!(sanitize_url("./path/to/my.json"), "./path/to/my.json");
        assert_eq!(sanitize_url("/path/to/my.json"), "/path/to/my.json");
        assert_eq!(
            sanitize_url("//google.com/robots.txt"),
            "//google.com/robots.txt"
        );
        assert_eq!(sanitize_url("www.example.com"), "www.example.com");
        assert_eq!(
            sanitize_url("com.braintreepayments.demo://example"),
            "com.braintreepayments.demo://example"
        );
        assert_eq!(
            sanitize_url("mailto:test@example.com?subject=hello+world"),
            "mailto:test@example.com?subject=hello+world"
        );
        assert_eq!(
            sanitize_url("www.example.com/with-芍cc那nt?"),
            "www.example.com/with-芍cc那nt?"
        );
        assert_eq!(
            sanitize_url("www.example.com/抖抉找.把扳扮我扮抗我邦每"),
            "www.example.com/抖抉找.把扳扮我扮抗我邦每"
        );
        assert_eq!(
            sanitize_url("www.example.com/\u{200D}\u{0000}\u{001F}\u{0000}\u{001F}\u{FEFF}foo"),
            "www.example.com/foo"
        );
        assert_eq!(sanitize_url(""), BLANK_URL);
        assert_eq!(
            sanitize_url("   http://example.com/path/to:something    "),
            "http://example.com/path/to:something"
        );
        assert_eq!(
            sanitize_url("https://example.com&NewLine;&NewLine;/something"),
            "https://example.com/something"
        );

        // all these decode to `javascript:alert('xss');`
        let attack_vectors = [
            "&#0000106&#0000097&#0000118&#0000097&#0000115&#0000099&#0000114&#0000105&#0000112&#0000116&#0000058&#0000097&#0000108&#0000101&#0000114&#0000116&#0000040&#0000039&#0000088&#0000083&#0000083&#0000039&#0000041",
            "&#106;&#97;&#118;&#97;&#115;&#99;&#114;&#105;&#112;&#116;&#58;&#97;&#108;&#101;&#114;&#116;&#40;&#39;&#88;&#83;&#83;&#39;&#41;",
            "&#x6A&#x61&#x76&#x61&#x73&#x63&#x72&#x69&#x70&#x74&#x3A&#x61&#x6C&#x65&#x72&#x74&#x28&#x27&#x58&#x53&#x53&#x27&#x29",
            "jav&#x09;ascript:alert('XSS');",
            " &#14; javascript:alert('XSS');",
            "javasc&Tab;ript: alert('XSS');",
            "javasc&#\u{0000}x09;ript:alert(1)",
            "java&#38;&#38;&#35;78&#59;ewLine&#38;newline&#59;&#59;script&#58;alert&#40;&#39;XSS&#39;&#41;",
            "java&&#78;ewLine&newline;;script:alert('XSS')",
        ];
        for v in attack_vectors {
            assert_eq!(sanitize_url(v), BLANK_URL);
        }

        assert_eq!(
            sanitize_url(
                "&#104;&#116;&#116;&#112;&#115;&#0000058//&#101;&#120;&#97;&#109;&#112;&#108;&#101;&#46;&#99;&#111;&#109;/&#0000106&#0000097&#0000118&#0000097&#0000115&#0000099&#0000114&#0000105&#0000112&#0000116&#0000058&#0000097&#0000108&#0000101&#0000114&#0000116&#0000040&#0000039&#0000088&#0000083&#0000083&#0000039&#0000041"
            ),
            "https://example.com/javascript:alert('XSS')"
        );

        let whitespace_escape_vectors = [
            "javascri\npt:alert('xss')",
            "javascri\rpt:alert('xss')",
            "javascri\tpt:alert('xss')",
            "javascrip\\%74t:alert('XSS')",
            "javascrip%5c%72t:alert()",
            "javascrip%5Ctt:alert()",
            "javascrip%255Ctt:alert()",
            "javascrip%25%35Ctt:alert()",
            "javascrip%25%35%43tt:alert()",
            "javascrip%25%32%35%25%33%35%25%34%33rt:alert()",
            "javascrip%255Crt:alert('%25xss')",
        ];
        for v in whitespace_escape_vectors {
            assert_eq!(sanitize_url(v), BLANK_URL);
        }

        let backslash_prefixed_vectors = [
            "\u{000C}javascript:alert()",
            "\u{000B}javascript:alert()",
            "\tjavascript:alert()",
            "\njavascript:alert()",
            "\rjavascript:alert()",
            "\u{0000}javascript:alert()",
            "\u{0001}javascript:alert()",
        ];
        for v in backslash_prefixed_vectors {
            assert_eq!(sanitize_url(v), BLANK_URL);
        }

        assert_eq!(
            sanitize_url("\\j\\av\\a\\s\\cript:alert()"),
            "/j/av/a/s/cript:alert()"
        );

        for protocol in ["javascript", "data", "vbscript"] {
            assert_eq!(
                sanitize_url(&format!("{protocol}:alert(document.domain)")),
                BLANK_URL
            );
            assert_eq!(
                sanitize_url(&format!("not_{protocol}:alert(document.domain)")),
                format!("not_{protocol}:alert(document.domain)")
            );
            assert_eq!(
                sanitize_url(&format!("&!*{protocol}:alert(document.domain)")),
                BLANK_URL
            );
            assert_eq!(
                sanitize_url(&format!("{protocol}&colon;alert(document.domain)")),
                BLANK_URL
            );
            assert_eq!(
                sanitize_url(&format!("{protocol}&COLON;alert(document.domain)")),
                BLANK_URL
            );

            let mixed = protocol
                .chars()
                .enumerate()
                .map(|(idx, ch)| {
                    if idx % 2 == 0 {
                        ch.to_ascii_uppercase()
                    } else {
                        ch
                    }
                })
                .collect::<String>();
            assert_eq!(
                sanitize_url(&format!("{mixed}:alert(document.domain)")),
                BLANK_URL
            );

            let mut with_ctrl = String::new();
            for (idx, ch) in protocol.chars().enumerate() {
                if idx == 1 {
                    with_ctrl.push(ch);
                    with_ctrl.push_str("%EF%BB%BF%EF%BB%BF");
                } else if idx == 2 {
                    with_ctrl.push(ch);
                    with_ctrl.push_str("%e2%80%8b");
                } else {
                    with_ctrl.push(ch);
                }
            }
            let decoded = decode_uri_component_like(&format!("{with_ctrl}:alert(document.domain)"));
            assert_eq!(sanitize_url(&decoded), BLANK_URL);

            let decoded = decode_uri_component_like(&format!(
                "%20%20%20%20{protocol}:alert(document.domain)"
            ));
            assert_eq!(sanitize_url(&decoded), BLANK_URL);

            assert_eq!(
                sanitize_url(&format!("    {protocol}:alert(document.domain)")),
                BLANK_URL
            );

            assert_eq!(
                sanitize_url(&format!("http://example.com#{protocol}:foo")),
                format!("http://example.com#{protocol}:foo")
            );
        }
    }
}
