//! Shared Mermaid CSS declaration safety helpers.

/// Parses one Mermaid CSS declaration after applying the same structural safety policy used by
/// renderers and source-backed fixture evidence.
pub fn parse_safe_style_decl(s: &str) -> Option<(&str, &str)> {
    let s = s.trim().trim_end_matches(';').trim();
    if s.is_empty() {
        return None;
    }
    let (key, value) = s.split_once(':')?;
    let key = key.trim();
    let value = value.trim();
    if !is_safe_css_property_name(key) || !is_safe_css_declaration_value(value) {
        return None;
    }
    Some((key, value))
}

/// Returns whether a font-family value is structurally safe for Mermaid's style lane.
pub fn is_safe_css_font_family_value(value: &str) -> bool {
    is_safe_css_declaration_value(value) && !value.contains(':')
}

/// Returns whether a declaration belongs to the paint-only subset used by source-style evidence.
pub fn is_safe_paint_declaration(s: &str) -> bool {
    let Some((property, _)) = parse_safe_style_decl(s) else {
        return false;
    };
    matches!(
        property.trim().to_ascii_lowercase().as_str(),
        "color"
            | "fill"
            | "fill-opacity"
            | "opacity"
            | "stroke"
            | "stroke-dasharray"
            | "stroke-linecap"
            | "stroke-linejoin"
            | "stroke-opacity"
            | "stroke-width"
    )
}

/// Returns whether a declaration belongs to the layout-affecting typography subset.
pub fn is_safe_typography_declaration(s: &str) -> bool {
    let Some((property, _)) = parse_safe_style_decl(s) else {
        return false;
    };
    matches!(
        property.trim().to_ascii_lowercase().as_str(),
        "font-family"
            | "font-size"
            | "font-style"
            | "font-weight"
            | "letter-spacing"
            | "text-transform"
    )
}

fn is_safe_css_property_name(key: &str) -> bool {
    !key.is_empty()
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn is_safe_css_declaration_value(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() {
        return false;
    }

    let lower = value.to_ascii_lowercase();
    if lower.contains("url(") || lower.contains("expression(") {
        return false;
    }

    value
        .chars()
        .all(|ch| !ch.is_control() && !matches!(ch, '<' | '>' | '{' | '}' | ';' | '@'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paint_declarations_share_renderer_safety_and_exclude_typography() {
        assert!(is_safe_paint_declaration("fill: #123456"));
        assert!(!is_safe_paint_declaration("fill: url(javascript:alert(1))"));
        assert!(!is_safe_paint_declaration("font-size: 24px"));
        assert!(is_safe_typography_declaration("font-size: 24px"));
        assert!(!is_safe_typography_declaration("fill: #123456"));
    }
}
