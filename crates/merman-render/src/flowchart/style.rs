use crate::text::TextStyle;
use indexmap::IndexMap;

fn parse_style_decl(s: &str) -> Option<(&str, &str)> {
    let s = s.trim().trim_end_matches(';').trim();
    if s.is_empty() {
        return None;
    }
    let (k, v) = s.split_once(':')?;
    let k = k.trim();
    let v = v.trim();
    if k.is_empty() || v.is_empty() {
        return None;
    }
    Some((k, v))
}

fn parse_css_px_f64(v: &str) -> Option<f64> {
    let v = v.trim().trim_end_matches(';').trim();
    let v = v.trim_end_matches("px").trim();
    if v.is_empty() {
        return None;
    }
    v.parse::<f64>().ok()
}

fn normalize_css_font_family(font_family: &str) -> String {
    font_family.trim().trim_end_matches(';').trim().to_string()
}

fn split_mermaid_style_decls(s: &str) -> impl Iterator<Item = &str> {
    fn looks_like_key_start(s: &str) -> bool {
        let s = s.trim_start();
        let Some((k, _)) = s.split_once(':') else {
            return false;
        };
        let k = k.trim();
        !k.is_empty()
            && k.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
    }

    let mut parts: Vec<&str> = Vec::new();
    let mut start = 0usize;
    for (i, ch) in s.char_indices() {
        if ch != ',' {
            continue;
        }
        if looks_like_key_start(&s[i + 1..]) {
            let p = s[start..i].trim();
            if !p.is_empty() {
                parts.push(p);
            }
            start = i + 1;
        }
    }
    let tail = s[start..].trim();
    if !tail.is_empty() {
        parts.push(tail);
    }
    parts.into_iter()
}

fn apply_text_style_decl(style: &mut std::borrow::Cow<'_, TextStyle>, key: &str, value: &str) {
    match key {
        "font-size" => {
            if let Some(px) = parse_css_px_f64(value) {
                style.to_mut().font_size = px;
            }
        }
        "font-family" => {
            style.to_mut().font_family = Some(normalize_css_font_family(value));
        }
        "font-weight" => {
            style.to_mut().font_weight = Some(value.trim().to_string());
        }
        _ => {}
    }
}

fn flowchart_effective_text_style_for_class_names<'a>(
    base: &'a TextStyle,
    class_defs: &IndexMap<String, Vec<String>>,
    class_names: impl IntoIterator<Item = &'a str>,
    inline_styles: &[String],
) -> std::borrow::Cow<'a, TextStyle> {
    let mut style = std::borrow::Cow::Borrowed(base);

    for class in class_names {
        let Some(decls) = class_defs.get(class) else {
            continue;
        };
        for d in decls {
            for d in split_mermaid_style_decls(d) {
                let Some((k, v)) = parse_style_decl(d) else {
                    continue;
                };
                apply_text_style_decl(&mut style, k, v);
            }
        }
    }

    for d in inline_styles {
        for d in split_mermaid_style_decls(d) {
            let Some((k, v)) = parse_style_decl(d) else {
                continue;
            };
            apply_text_style_decl(&mut style, k, v);
        }
    }

    style
}

pub(crate) fn flowchart_effective_node_class_names<'a>(
    class_defs: &'a IndexMap<String, Vec<String>>,
    classes: &'a [String],
) -> Vec<&'a str> {
    let mut effective: Vec<&'a str> = Vec::with_capacity(classes.len() + 1);
    if classes.is_empty() && class_defs.contains_key("default") {
        effective.push("default");
    }
    effective.extend(classes.iter().map(|class| class.as_str()));
    effective
}

pub(crate) fn flowchart_node_has_span_css_height_parity(
    class_defs: &IndexMap<String, Vec<String>>,
    classes: &[String],
) -> bool {
    flowchart_effective_node_class_names(class_defs, classes)
        .into_iter()
        .any(|class| {
            class_defs.get(class).is_some_and(|styles| {
                styles.iter().any(|style| {
                    split_mermaid_style_decls(style).any(|decl| {
                        matches!(
                            parse_style_decl(decl).map(|(key, _)| key),
                            Some("background" | "border")
                        )
                    })
                })
            })
        })
}

pub(crate) fn flowchart_effective_text_style_for_node_classes<'a>(
    base: &'a TextStyle,
    class_defs: &'a IndexMap<String, Vec<String>>,
    classes: &'a [String],
    inline_styles: &[String],
) -> std::borrow::Cow<'a, TextStyle> {
    let effective_classes = flowchart_effective_node_class_names(class_defs, classes);
    if effective_classes.is_empty() && inline_styles.is_empty() {
        return std::borrow::Cow::Borrowed(base);
    }
    flowchart_effective_text_style_for_class_names(
        base,
        class_defs,
        effective_classes.into_iter(),
        inline_styles,
    )
}

pub(crate) fn flowchart_effective_text_style_for_classes<'a>(
    base: &'a TextStyle,
    class_defs: &IndexMap<String, Vec<String>>,
    classes: &'a [String],
    inline_styles: &[String],
) -> std::borrow::Cow<'a, TextStyle> {
    if classes.is_empty() && inline_styles.is_empty() {
        return std::borrow::Cow::Borrowed(base);
    }

    flowchart_effective_text_style_for_class_names(
        base,
        class_defs,
        classes.iter().map(|class| class.as_str()),
        inline_styles,
    )
}
