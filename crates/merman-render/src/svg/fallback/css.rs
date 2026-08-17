use super::attr::parse_attr_str;
use crate::svg::pipeline::{checkpoint_loop, find_with_checkpoints};

pub(super) fn extract_css_background_color_for_class<E>(
    svg: &str,
    class_name: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<String>, E> {
    // Mermaid parity SVGs inline styles in a `<style>` element and typically emit rules like:
    //   #<id> .labelBkg{background-color:rgba(...);}
    // This is a cheap non-validating parser that looks for `.className{...}` and then extracts the
    // first `background-color:` declaration within that block.
    let needle = format!(".{class_name}{{");
    let mut search = 0usize;
    while let Some(rel) = find_with_checkpoints(&svg[search..], &needle, checkpoint)? {
        checkpoint_loop(search, checkpoint)?;
        let i = search + rel + needle.len();
        let Some(end_rel) = find_with_checkpoints(&svg[i..], "}", checkpoint)? else {
            return Ok(None);
        };
        let block = &svg[i..i + end_rel];
        if let Some(k) = find_with_checkpoints(block, "background-color:", checkpoint)? {
            let after = &block[k + "background-color:".len()..];
            let end = find_with_checkpoints(after, ";", checkpoint)?.unwrap_or(after.len());
            let value = after[..end].trim();
            if !value.is_empty() {
                return Ok(Some(value.to_string()));
            }
        }
        search = i + end_rel + 1;
    }
    checkpoint()?;
    Ok(None)
}

pub(super) fn extract_css_text_fill_for_class<E>(
    svg: &str,
    class_name: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<String>, E> {
    // Mermaid parity SVGs inline styles in a `<style>` element and typically emit rules like:
    //   #<id> .section-root text{fill:#ffffff;}
    //   #<id> .label text,#<id> span,#<id> p{fill:#ffffff;color:#ffffff;}
    // This is a cheap non-validating parser for scoped selector lists. It deliberately avoids
    // shape selectors like `.node rect` so node backgrounds do not become text fallback fills.
    let mut search = 0usize;
    while let Some(open_rel) = find_with_checkpoints(&svg[search..], "{", checkpoint)? {
        checkpoint_loop(search, checkpoint)?;
        let open = search + open_rel;
        let Some(close_rel) = find_with_checkpoints(&svg[open + 1..], "}", checkpoint)? else {
            break;
        };
        let close = open + 1 + close_rel;
        let selector_start = search;
        let selector = svg[selector_start..open].trim();
        let declarations = &svg[open + 1..close];

        if selector_rule_applies_to_text_class(selector, class_name) {
            let property = extract_style_property(declarations, "fill")
                .or_else(|| extract_style_property(declarations, "color"));
            if property.is_some() {
                return Ok(property);
            }
        } else if selector_rule_applies_inherited_color_to_class(selector, class_name)
            && let Some(color) = extract_style_property(declarations, "color")
        {
            return Ok(Some(color));
        }

        search = close + 1;
    }

    // Preserve the historical fast path for compact unscoped rules.
    let needle = format!(".{class_name} text{{fill:");
    let mut search = 0usize;
    while let Some(rel) = find_with_checkpoints(&svg[search..], &needle, checkpoint)? {
        checkpoint_loop(search, checkpoint)?;
        let i = search + rel + needle.len();
        let after = &svg[i..];
        let end = find_with_checkpoints(after, ";", checkpoint)?
            .or(find_with_checkpoints(after, "}", checkpoint)?)
            .unwrap_or(after.len());
        let value = after[..end].trim();
        if !value.is_empty() {
            return Ok(Some(value.to_string()));
        }
        search = i + end;
    }
    checkpoint()?;
    Ok(None)
}

pub(super) fn extract_css_root_text_fill<E>(
    svg: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<String>, E> {
    extract_css_root_style_property(svg, &["fill", "color"], checkpoint)
}

pub(super) fn extract_css_root_style_property<E>(
    svg: &str,
    properties: &[&str],
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<String>, E> {
    checkpoint()?;
    let Some(svg_start) = find_with_checkpoints(svg, "<svg", checkpoint)? else {
        return Ok(None);
    };
    let Some(svg_end_rel) = find_with_checkpoints(&svg[svg_start..], ">", checkpoint)? else {
        return Ok(None);
    };
    let svg_end = svg_start + svg_end_rel + 1;
    let Some(root_id) = parse_attr_str(&svg[svg_start..svg_end], "id") else {
        return Ok(None);
    };
    let needle = format!("#{root_id}{{");
    let Some(needle_start) = find_with_checkpoints(svg, &needle, checkpoint)? else {
        return Ok(None);
    };
    let i = needle_start + needle.len();
    let Some(end_rel) = find_with_checkpoints(&svg[i..], "}", checkpoint)? else {
        return Ok(None);
    };
    let declarations = &svg[i..i + end_rel];
    let value = properties
        .iter()
        .find_map(|property| extract_style_property(declarations, property));
    checkpoint()?;
    Ok(value)
}

pub(super) fn extract_css_style_property_for_class<E>(
    svg: &str,
    class_name: &str,
    property: &str,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Option<String>, E> {
    let mut search = 0usize;
    while let Some(open_rel) = find_with_checkpoints(&svg[search..], "{", checkpoint)? {
        checkpoint_loop(search, checkpoint)?;
        let open = search + open_rel;
        let Some(close_rel) = find_with_checkpoints(&svg[open + 1..], "}", checkpoint)? else {
            break;
        };
        let close = open + 1 + close_rel;
        let selector_start = search;
        let selector = svg[selector_start..open].trim();
        let declarations = &svg[open + 1..close];

        if selector_rule_applies_inherited_color_to_class(selector, class_name)
            && let Some(value) = extract_style_property(declarations, property)
        {
            return Ok(Some(value));
        }

        search = close + 1;
    }

    checkpoint()?;
    Ok(None)
}

fn selector_rule_applies_to_text_class(selector_list: &str, class_name: &str) -> bool {
    selector_list.split(',').map(str::trim).any(|selector| {
        selector_has_class(selector, class_name) && selector_targets_text_like(selector)
    })
}

fn selector_rule_applies_inherited_color_to_class(selector_list: &str, class_name: &str) -> bool {
    selector_list.split(',').map(str::trim).any(|selector| {
        selector_has_class(selector, class_name) && !selector_targets_shape(selector)
    })
}

fn selector_has_class(selector: &str, class_name: &str) -> bool {
    let needle = format!(".{class_name}");
    let mut search = 0usize;
    while let Some(rel) = selector[search..].find(&needle) {
        let start = search + rel;
        let after = start + needle.len();
        if selector[after..]
            .chars()
            .next()
            .is_none_or(|ch| !is_css_identifier_char(ch))
        {
            return true;
        }
        search = after;
    }
    false
}

fn is_css_identifier_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '-' || ch == '_'
}

fn selector_targets_text_like(selector: &str) -> bool {
    selector_targets_element(selector, "text")
        || selector_targets_element(selector, "tspan")
        || selector_targets_element(selector, "span")
        || selector_targets_element(selector, "p")
}

fn selector_targets_shape(selector: &str) -> bool {
    selector_targets_element(selector, "rect")
        || selector_targets_element(selector, "circle")
        || selector_targets_element(selector, "ellipse")
        || selector_targets_element(selector, "polygon")
        || selector_targets_element(selector, "path")
        || selector_targets_element(selector, "line")
}

fn selector_targets_element(selector: &str, element: &str) -> bool {
    let lower = selector.to_ascii_lowercase();
    let mut search = 0usize;
    while let Some(rel) = lower[search..].find(element) {
        let start = search + rel;
        let before = lower[..start].chars().next_back();
        let after = lower[start + element.len()..].chars().next();
        let before_ok = before.is_none_or(|ch| !is_css_identifier_char(ch));
        let after_ok = after.is_none_or(|ch| !is_css_identifier_char(ch));
        if before_ok && after_ok {
            return true;
        }
        search = start + element.len();
    }
    false
}

pub(super) fn extract_style_property(style: &str, property: &str) -> Option<String> {
    for decl in style.split(';') {
        let Some((name, value)) = decl.split_once(':') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case(property) {
            let value = strip_important(value.trim());
            if !value.is_empty() {
                return Some(value);
            }
        }
    }
    None
}

fn strip_important(value: &str) -> String {
    let mut value = value.trim().to_string();
    if let Some(v) = value.strip_suffix("!important") {
        value = v.trim().to_string();
    }
    value
}

pub(super) fn parse_css_px_value(value: &str) -> Option<f64> {
    let trimmed = value.trim();
    let trimmed = strip_important(trimmed);
    let number = trimmed.strip_suffix("px").unwrap_or(&trimmed).trim();
    number
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value > 0.0)
}
