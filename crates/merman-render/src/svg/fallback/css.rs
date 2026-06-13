use super::attr::parse_attr_str;

pub(super) fn extract_css_background_color_for_class(
    svg: &str,
    class_name: &str,
) -> Option<String> {
    // Mermaid parity SVGs inline styles in a `<style>` element and typically emit rules like:
    //   #<id> .labelBkg{background-color:rgba(...);}
    // This is a cheap non-validating parser that looks for `.className{...}` and then extracts the
    // first `background-color:` declaration within that block.
    let needle = format!(".{class_name}{{");
    let mut search = 0usize;
    while let Some(rel) = svg[search..].find(&needle) {
        let i = search + rel + needle.len();
        let end_rel = svg[i..].find('}')?;
        let block = &svg[i..i + end_rel];
        if let Some(k) = block.find("background-color:") {
            let after = &block[k + "background-color:".len()..];
            let end = after.find(';').unwrap_or(after.len());
            let value = after[..end].trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
        search = i + end_rel + 1;
    }
    None
}

pub(super) fn extract_css_text_fill_for_class(svg: &str, class_name: &str) -> Option<String> {
    // Mermaid parity SVGs inline styles in a `<style>` element and typically emit rules like:
    //   #<id> .section-root text{fill:#ffffff;}
    //   #<id> .label text,#<id> span,#<id> p{fill:#ffffff;color:#ffffff;}
    // This is a cheap non-validating parser for scoped selector lists. It deliberately avoids
    // shape selectors like `.node rect` so node backgrounds do not become text fallback fills.
    let mut search = 0usize;
    while let Some(open_rel) = svg[search..].find('{') {
        let open = search + open_rel;
        let Some(close_rel) = svg[open + 1..].find('}') else {
            break;
        };
        let close = open + 1 + close_rel;
        let selector_start = svg[..open].rfind('}').map_or(0, |idx| idx + 1);
        let selector = svg[selector_start..open].trim();
        let declarations = &svg[open + 1..close];

        if selector_rule_applies_to_text_class(selector, class_name) {
            let property = extract_style_property(declarations, "fill")
                .or_else(|| extract_style_property(declarations, "color"));
            if property.is_some() {
                return property;
            }
        } else if selector_rule_applies_inherited_color_to_class(selector, class_name)
            && let Some(color) = extract_style_property(declarations, "color")
        {
            return Some(color);
        }

        search = close + 1;
    }

    // Preserve the historical fast path for compact unscoped rules.
    let needle = format!(".{class_name} text{{fill:");
    let mut search = 0usize;
    while let Some(rel) = svg[search..].find(&needle) {
        let i = search + rel + needle.len();
        let after = &svg[i..];
        let end = after
            .find(';')
            .or_else(|| after.find('}'))
            .unwrap_or(after.len());
        let value = after[..end].trim();
        if !value.is_empty() {
            return Some(value.to_string());
        }
        search = i + end;
    }
    None
}

pub(super) fn extract_css_root_text_fill(svg: &str) -> Option<String> {
    extract_css_root_style_property(svg, &["fill", "color"])
}

pub(super) fn extract_css_root_style_property(svg: &str, properties: &[&str]) -> Option<String> {
    let svg_start = svg.find("<svg")?;
    let svg_end = svg[svg_start..].find('>').map(|rel| svg_start + rel + 1)?;
    let root_id = parse_attr_str(&svg[svg_start..svg_end], "id")?;
    let needle = format!("#{root_id}{{");
    let i = svg.find(&needle)? + needle.len();
    let end_rel = svg[i..].find('}')?;
    let declarations = &svg[i..i + end_rel];
    properties
        .iter()
        .find_map(|property| extract_style_property(declarations, property))
}

pub(super) fn extract_css_style_property_for_class(
    svg: &str,
    class_name: &str,
    property: &str,
) -> Option<String> {
    let mut search = 0usize;
    while let Some(open_rel) = svg[search..].find('{') {
        let open = search + open_rel;
        let Some(close_rel) = svg[open + 1..].find('}') else {
            break;
        };
        let close = open + 1 + close_rel;
        let selector_start = svg[..open].rfind('}').map_or(0, |idx| idx + 1);
        let selector = svg[selector_start..open].trim();
        let declarations = &svg[open + 1..close];

        if selector_rule_applies_inherited_color_to_class(selector, class_name)
            && let Some(value) = extract_style_property(declarations, property)
        {
            return Some(value);
        }

        search = close + 1;
    }

    None
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
