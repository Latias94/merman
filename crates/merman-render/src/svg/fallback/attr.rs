pub(super) fn parse_attr_str<'a>(tag: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!(r#"{key}=""#);
    let i = tag.find(&needle)?;
    let rest = &tag[i + needle.len()..];
    let end = rest.find('"')?;
    Some(rest[..end].trim())
}

pub(super) fn parse_attr_f64(tag: &str, key: &str) -> Option<f64> {
    parse_attr_str(tag, key)?.parse::<f64>().ok()
}

pub(super) fn is_self_closing(tag: &str) -> bool {
    tag.trim_end().ends_with("/>")
}

pub(super) fn parse_class_tokens(tag: &str) -> Vec<String> {
    let Some(s) = parse_attr_str(tag, "class") else {
        return Vec::new();
    };
    s.split_whitespace().map(|t| t.to_string()).collect()
}
