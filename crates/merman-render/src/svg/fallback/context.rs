use super::attr::{parse_attr_str, parse_class_tokens};
use super::css::{
    extract_css_root_style_property, extract_css_root_text_fill,
    extract_css_style_property_for_class, extract_css_text_fill_for_class, extract_style_property,
};
use super::xml::escape_xml_attr;

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Translate {
    pub(super) x: f64,
    pub(super) y: f64,
}

#[derive(Clone, Debug, Default)]
pub(super) struct GFrame {
    translate: Translate,
    class_tokens: Vec<String>,
    fill: Option<String>,
    font_size: Option<String>,
    font_family: Option<String>,
    font_weight: Option<String>,
    font_style: Option<String>,
}

impl GFrame {
    pub(super) fn from_g_tag(tag: &str) -> Self {
        let style = parse_attr_str(tag, "style");
        Self {
            translate: parse_attr_str(tag, "transform")
                .map(parse_translate)
                .unwrap_or_default(),
            class_tokens: parse_class_tokens(tag),
            fill: parse_attr_str(tag, "fill")
                .map(ToOwned::to_owned)
                .or_else(|| style.and_then(|style| extract_style_property(style, "fill"))),
            font_size: style.and_then(|style| extract_style_property(style, "font-size")),
            font_family: style.and_then(|style| extract_style_property(style, "font-family")),
            font_weight: style.and_then(|style| extract_style_property(style, "font-weight")),
            font_style: style.and_then(|style| extract_style_property(style, "font-style")),
        }
    }
}

fn parse_translate(transform: &str) -> Translate {
    let lower = transform.to_ascii_lowercase();
    let Some(i) = lower.find("translate(") else {
        return Translate::default();
    };
    let after = &transform[i + "translate(".len()..];
    let Some(end) = after.find(')') else {
        return Translate::default();
    };
    let args = &after[..end];

    let mut nums = Vec::<f64>::with_capacity(2);
    let mut cur = String::new();
    for ch in args.chars() {
        if ch.is_ascii_digit() || ch == '.' || ch == '-' || ch == '+' || ch == 'e' || ch == 'E' {
            cur.push(ch);
        } else if !cur.is_empty() {
            if let Ok(v) = cur.parse::<f64>() {
                nums.push(v);
            }
            cur.clear();
        }
    }
    if !cur.is_empty()
        && let Ok(v) = cur.parse::<f64>()
    {
        nums.push(v);
    }

    Translate {
        x: *nums.first().unwrap_or(&0.0),
        y: *nums.get(1).unwrap_or(&0.0),
    }
}

pub(super) fn sum_translate(stack: &[GFrame]) -> Translate {
    let mut acc = Translate::default();
    for t in stack {
        acc.x += t.translate.x;
        acc.y += t.translate.y;
    }
    acc
}

pub(super) fn extract_svg_text_fill_from_ancestors(
    svg: &str,
    g_stack: &[GFrame],
) -> Option<String> {
    // Prefer the closest ancestor's classes (more specific) by scanning frames from inner -> outer.
    for frame in g_stack.iter().rev() {
        for token in frame.class_tokens.iter().rev() {
            if let Some(fill) = extract_css_text_fill_for_class(svg, token) {
                return Some(fill);
            }
        }
        if let Some(fill) = &frame.fill {
            return Some(fill.clone());
        }
    }
    extract_css_root_text_fill(svg)
}

fn extract_svg_font_style_from_ancestors(g_stack: &[GFrame], property: &str) -> Option<String> {
    for frame in g_stack.iter().rev() {
        let value = match property {
            "font-size" => &frame.font_size,
            "font-family" => &frame.font_family,
            "font-weight" => &frame.font_weight,
            "font-style" => &frame.font_style,
            _ => return None,
        };
        if let Some(value) = value {
            return Some(value.clone());
        }
    }
    None
}

pub(super) fn extract_svg_font_style_from_context(
    svg: &str,
    g_stack: &[GFrame],
    property: &str,
) -> Option<String> {
    extract_svg_font_style_from_ancestors(g_stack, property).or_else(|| {
        for frame in g_stack.iter().rev() {
            for token in frame.class_tokens.iter().rev() {
                if let Some(value) = extract_css_style_property_for_class(svg, token, property) {
                    return Some(value);
                }
            }
        }
        extract_css_root_style_property(svg, &[property])
    })
}

pub(super) fn class_attr_tokens(g_stack: &[GFrame], inner: &str, base_class: &str) -> String {
    let mut tokens = vec![base_class.to_string()];
    for frame in g_stack {
        for token in &frame.class_tokens {
            if !tokens.iter().any(|existing| existing == token) {
                tokens.push(token.clone());
            }
        }
    }
    for token in parse_class_tokens(inner) {
        if !tokens.iter().any(|existing| existing == &token) {
            tokens.push(token);
        }
    }
    escape_xml_attr(&tokens.join(" "))
}

pub(super) fn fallback_text_class_attr_tokens(g_stack: &[GFrame], inner: &str) -> String {
    let mut tokens = vec!["merman-foreignobject-fallback-text".to_string()];
    for frame in g_stack {
        for token in &frame.class_tokens {
            if is_fallback_text_safe_class(token)
                && !tokens.iter().any(|existing| existing == token)
            {
                tokens.push(token.clone());
            }
        }
    }
    for token in parse_class_tokens(inner) {
        if is_fallback_text_safe_class(&token) && !tokens.iter().any(|existing| existing == &token)
        {
            tokens.push(token);
        }
    }
    escape_xml_attr(&tokens.join(" "))
}

fn is_fallback_text_safe_class(class_name: &str) -> bool {
    !matches!(class_name, "label")
}
