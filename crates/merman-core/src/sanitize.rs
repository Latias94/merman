use crate::MermaidConfig;
use crate::generated::dompurify_defaults;
use lol_html::{RewriteStrSettings, element, rewrite_str};
use regex::Regex;
use std::collections::HashSet;
use std::sync::OnceLock;

fn line_break_regex() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)<br\s*/?>").expect("valid regex"))
}

fn break_to_placeholder(input: &str) -> String {
    line_break_regex().replace_all(input, "#br#").to_string()
}

fn placeholder_to_break(input: &str) -> String {
    input.replace("#br#", "<br/>")
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

fn dompurify_data_attr_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^data-[\-\w.\u{00B7}-\u{FFFF}]+$").expect("valid regex"))
}

fn dompurify_aria_attr_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^aria-[\-\w]+$").expect("valid regex"))
}

fn dompurify_attr_whitespace_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"[\u{0000}-\u{0020}\u{00A0}\u{1680}\u{180E}\u{2000}-\u{2029}\u{205F}\u{3000}]")
            .expect("valid regex")
    })
}

fn dompurify_is_allowed_uri_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)^(?:(?:(?:f|ht)tps?|mailto|tel|callto|sms|cid|xmpp):|[^a-z]|[a-z+.\-]+(?:[^a-z+.\-:]|$))")
            .expect("valid regex")
    })
}

fn dompurify_is_script_or_data_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^(?:\w+script|data):").expect("valid regex"))
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
    if cfg.allow_data_attr
        && !cfg.forbid_attr.contains(lc_name)
        && dompurify_data_attr_regex().is_match(lc_name)
    {
        return true;
    }

    if cfg.allow_aria_attr && dompurify_aria_attr_regex().is_match(lc_name) {
        return true;
    }

    if !cfg.allowed_attr.contains(lc_name) || cfg.forbid_attr.contains(lc_name) {
        return false;
    }

    if cfg.uri_safe_attr.contains(lc_name) {
        return true;
    }

    let decoded_value = decode_attr_html_entities_minimally(value);
    let value_no_ws = dompurify_attr_whitespace_regex()
        .replace_all(&decoded_value, "")
        .to_string();

    if dompurify_is_allowed_uri_regex().is_match(&value_no_ws) {
        return true;
    }

    if matches!(lc_name, "src" | "xlink:href" | "href")
        && lc_tag != "script"
        && decoded_value.starts_with("data:")
        && cfg.data_uri_tags.contains(lc_tag)
    {
        return true;
    }

    if cfg.allow_unknown_protocols && !dompurify_is_script_or_data_regex().is_match(&value_no_ws) {
        return true;
    }

    value.is_empty()
}

fn decode_attr_html_entities_minimally(input: &str) -> String {
    if input.is_empty() {
        return String::new();
    }

    fn colon_entity_regex() -> &'static Regex {
        static RE: OnceLock<Regex> = OnceLock::new();
        RE.get_or_init(|| Regex::new(r"(?i)&colon;").expect("valid regex"))
    }

    fn newline_entity_regex() -> &'static Regex {
        static RE: OnceLock<Regex> = OnceLock::new();
        RE.get_or_init(|| Regex::new(r"(?i)&newline;").expect("valid regex"))
    }

    fn tab_entity_regex() -> &'static Regex {
        static RE: OnceLock<Regex> = OnceLock::new();
        RE.get_or_init(|| Regex::new(r"(?i)&tab;").expect("valid regex"))
    }

    fn numeric_colon_dec_regex() -> &'static Regex {
        static RE: OnceLock<Regex> = OnceLock::new();
        RE.get_or_init(|| Regex::new(r"(?i)&#0*58;?").expect("valid regex"))
    }

    fn numeric_colon_hex_regex() -> &'static Regex {
        static RE: OnceLock<Regex> = OnceLock::new();
        RE.get_or_init(|| Regex::new(r"(?i)&#x0*3a;?").expect("valid regex"))
    }

    let mut out = input.to_string();
    out = colon_entity_regex().replace_all(&out, ":").to_string();
    out = newline_entity_regex().replace_all(&out, "\n").to_string();
    out = tab_entity_regex().replace_all(&out, "\t").to_string();
    out = numeric_colon_dec_regex().replace_all(&out, ":").to_string();
    out = numeric_colon_hex_regex().replace_all(&out, ":").to_string();
    out
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

    let mut handlers = vec![
        element!("script", |el| {
            el.remove();
            Ok(())
        }),
        element!("iframe", |el| {
            el.remove();
            Ok(())
        }),
        element!("style", |el| {
            el.remove();
            Ok(())
        }),
    ];

    handlers.push(element!("a", |el| {
        // Mirror Mermaid's DOMPurify hooks:
        // - beforeSanitizeAttributes stores the target in a temporary data-* attribute
        // - afterSanitizeAttributes restores it (only if the data-* survived sanitization)
        if let Some(target) = el.get_attribute("target") {
            let _ = el.set_attribute("data-temp-href-target", &target);
        }
        Ok(())
    }));

    handlers.push(element!("*", |el| {
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
            if !dompurify_is_valid_attribute(cfg, &lc_tag, &lc_name, &value) {
                el.remove_attribute(&name);
                continue;
            }

            if matches!(lc_name.as_str(), "href" | "src" | "xlink:href") {
                // DOMPurify validates URI values on parsed DOM values (entities already decoded).
                // `lol_html` gives us raw values, so we decode the minimal subset Mermaid relies on.
                let decoded = decode_attr_html_entities_minimally(&value);
                if decoded != value {
                    let _ = el.set_attribute(&name, &decoded);
                }
            }
        }

        if lc_tag == "a" {
            if let Some(target) = el.get_attribute("data-temp-href-target") {
                let _ = el.set_attribute("target", &target);
                el.remove_attribute("data-temp-href-target");
                if target == "_blank" {
                    let _ = el.set_attribute("rel", "noopener");
                }
            }
        }

        Ok(())
    }));

    rewrite_str(
        text.as_ref(),
        RewriteStrSettings {
            element_content_handlers: handlers,
            ..RewriteStrSettings::new()
        },
    )
    .unwrap_or_else(|_| text.into_owned())
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

fn sanitize_more(text: &str, config: &MermaidConfig) -> String {
    let html_labels_enabled = config.get_bool("flowchart.htmlLabels") != Some(false);
    if !html_labels_enabled {
        return text.to_string();
    }

    let level = config.get_str("securityLevel");
    if matches!(level, Some("antiscript" | "strict")) {
        return remove_script(text);
    }

    if level != Some("loose") {
        let mut t = break_to_placeholder(text);
        t = t.replace('<', "&lt;").replace('>', "&gt;");
        t = t.replace('=', "&equals;");
        return placeholder_to_break(&t);
    }

    text.to_string()
}

pub fn sanitize_text(text: &str, config: &MermaidConfig) -> String {
    if text.is_empty() {
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
    fn sanitize_text_sandbox_escapes_angle_brackets_and_equals() {
        let cfg = MermaidConfig::from_value(json!({
            "securityLevel": "sandbox",
            "flowchart": { "htmlLabels": true }
        }));
        assert_eq!(
            sanitize_text(r#"<b a=1>ok</b><br/>x"#, &cfg),
            "&lt;b a&equals;1&gt;ok&lt;/b&gt;<br/>x"
        );
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
        let out = sanitize_text(r#"<b data-x="1" aria-label="x" foo="bar">ok</b>"#, &cfg);
        assert!(!out.contains("foo="));
        assert!(out.contains(r#"data-x="1""#));
        assert!(out.contains(r#"aria-label="x""#));
        assert!(out.starts_with("<b"));
        assert!(out.ends_with(">ok</b>"));

        let cfg = MermaidConfig::from_value(json!({
            "securityLevel": "loose",
            "flowchart": { "htmlLabels": true },
            "dompurifyConfig": { "ALLOW_DATA_ATTR": false, "ALLOW_ARIA_ATTR": false }
        }));
        assert_eq!(
            sanitize_text(r#"<b data-x="1" aria-label="x">ok</b>"#, &cfg),
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
