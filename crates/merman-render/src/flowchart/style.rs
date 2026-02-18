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
    // Mermaid `classDef` declarations are commonly comma-separated:
    //   font-size:30px,fill:yellow
    //
    // Values may legitimately contain commas (e.g. `font-family:"Open Sans", sans-serif` or
    // `fill:hsl(0, 100%, 50%)`). Split only on commas that are followed by a new `key:` token.
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

pub(crate) fn flowchart_effective_text_style_for_classes<'a>(
    base: &'a TextStyle,
    class_defs: &IndexMap<String, Vec<String>>,
    classes: &[String],
    inline_styles: &[String],
) -> std::borrow::Cow<'a, TextStyle> {
    if classes.is_empty() && inline_styles.is_empty() {
        return std::borrow::Cow::Borrowed(base);
    }

    let mut style = std::borrow::Cow::Borrowed(base);

    for class in classes {
        let Some(decls) = class_defs.get(class) else {
            continue;
        };
        for d in decls {
            for d in split_mermaid_style_decls(d) {
                let Some((k, v)) = parse_style_decl(d) else {
                    continue;
                };
                match k {
                    "font-size" => {
                        if let Some(px) = parse_css_px_f64(v) {
                            style.to_mut().font_size = px;
                        }
                    }
                    "font-family" => {
                        style.to_mut().font_family = Some(normalize_css_font_family(v));
                    }
                    "font-weight" => {
                        style.to_mut().font_weight = Some(v.trim().to_string());
                    }
                    _ => {}
                }
            }
        }
    }

    for d in inline_styles {
        for d in split_mermaid_style_decls(d) {
            let Some((k, v)) = parse_style_decl(d) else {
                continue;
            };
            match k {
                "font-size" => {
                    if let Some(px) = parse_css_px_f64(v) {
                        style.to_mut().font_size = px;
                    }
                }
                "font-family" => {
                    style.to_mut().font_family = Some(normalize_css_font_family(v));
                }
                "font-weight" => {
                    style.to_mut().font_weight = Some(v.trim().to_string());
                }
                _ => {}
            }
        }
    }

    style
}
