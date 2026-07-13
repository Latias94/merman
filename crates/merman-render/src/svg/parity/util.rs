// Shared SVG utility helpers (split from parity.rs).
//
// Keep behavior identical; these helpers are used across multiple diagram renderers.

use std::borrow::Cow;
use std::str::FromStr as _;

use roughr::Color;

pub(super) use crate::config::{config_diagram_look, config_f64, config_f64_css_px};

pub(super) fn config_string(cfg: &serde_json::Value, path: &[&str]) -> Option<String> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_str().map(|s| s.to_string())
}

pub(super) fn json_bool(v: &serde_json::Value) -> Option<bool> {
    v.as_bool()
        .or_else(|| v.as_i64().map(|n| n != 0))
        .or_else(|| v.as_u64().map(|n| n != 0))
        .or_else(|| {
            v.as_str()
                .and_then(|s| match s.trim().to_ascii_lowercase().as_str() {
                    "true" | "yes" | "on" | "1" => Some(true),
                    "false" | "no" | "off" | "0" => Some(false),
                    _ => None,
                })
        })
}

pub(super) fn config_bool(cfg: &serde_json::Value, path: &[&str]) -> Option<bool> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    json_bool(cur)
}

pub(super) fn normalize_css_font_family(font_family: &str) -> String {
    crate::config::normalize_css_font_family(font_family)
}

pub(super) fn theme_color(
    effective_config: &serde_json::Value,
    key: &str,
    fallback: &str,
) -> String {
    config_string(effective_config, &["themeVariables", key])
        .unwrap_or_else(|| fallback.to_string())
}

pub(super) struct SvgTheme<'a> {
    effective_config: &'a serde_json::Value,
}

impl<'a> SvgTheme<'a> {
    pub(super) fn new(effective_config: &'a serde_json::Value) -> Self {
        Self { effective_config }
    }

    pub(super) fn optional_color(&self, key: &str) -> Option<String> {
        config_string(self.effective_config, &["themeVariables", key])
    }

    pub(super) fn optional_nested_color(&self, diagram_key: &str, key: &str) -> Option<String> {
        config_string(self.effective_config, &["themeVariables", diagram_key, key])
    }

    pub(super) fn optional_nested_css_value(&self, diagram_key: &str, key: &str) -> Option<String> {
        crate::config::config_css_number_or_string(
            self.effective_config,
            &["themeVariables", diagram_key, key],
        )
    }

    pub(super) fn optional_nested_css_px(&self, diagram_key: &str, key: &str) -> Option<f64> {
        crate::config::config_f64_css_px(
            self.effective_config,
            &["themeVariables", diagram_key, key],
        )
    }

    pub(super) fn optional_scoped_string(&self, scope: &str, key: &str) -> Option<String> {
        config_string(self.effective_config, &[scope, key])
            .or_else(|| config_string(self.effective_config, &["themeVariables", scope, key]))
    }

    pub(super) fn optional_root_scoped_string(&self, scope: &str, key: &str) -> Option<String> {
        config_string(self.effective_config, &[scope, key])
    }

    pub(super) fn optional_root_scoped_css_value(&self, scope: &str, key: &str) -> Option<String> {
        crate::config::config_css_number_or_string(self.effective_config, &[scope, key])
    }

    pub(super) fn root_or_theme_string(&self, key: &str, fallback: &str) -> String {
        config_string(self.effective_config, &[key])
            .or_else(|| config_string(self.effective_config, &["themeVariables", key]))
            .unwrap_or_else(|| fallback.to_string())
    }

    pub(super) fn optional_scoped_f64(&self, scope: &str, key: &str) -> Option<f64> {
        crate::config::config_f64(self.effective_config, &[scope, key]).or_else(|| {
            crate::config::config_f64(self.effective_config, &["themeVariables", scope, key])
        })
    }

    pub(super) fn bool_root_or_theme(&self, key: &str) -> Option<bool> {
        config_bool(self.effective_config, &[key])
            .or_else(|| config_bool(self.effective_config, &["themeVariables", key]))
    }

    pub(super) fn optional_value(&self, key: &str) -> Option<String> {
        crate::config::config_css_number_or_string(self.effective_config, &["themeVariables", key])
    }

    pub(super) fn optional_f64(&self, key: &str) -> Option<f64> {
        crate::config::config_f64(self.effective_config, &["themeVariables", key])
    }

    pub(super) fn string_array(&self, key: &str) -> Vec<String> {
        crate::config::config_string_vec(self.effective_config, &["themeVariables", key])
    }

    pub(super) fn color(&self, key: &str, fallback: &str) -> String {
        theme_color(self.effective_config, key, fallback)
    }

    pub(super) fn theme_name(&self) -> String {
        config_string(self.effective_config, &["theme"]).unwrap_or_else(|| "default".to_string())
    }

    pub(super) fn look(&self) -> String {
        crate::config::config_diagram_look(self.effective_config)
            .as_str()
            .to_string()
    }

    pub(super) fn css_value(&self, key: &str, fallback: &str) -> String {
        crate::config::config_css_number_or_string(self.effective_config, &["themeVariables", key])
            .unwrap_or_else(|| fallback.to_string())
    }

    pub(super) fn font_family_css(&self) -> String {
        crate::config::config_font_family_css(self.effective_config)
    }

    pub(super) fn font_family_css_root_first(&self) -> String {
        let font_family = config_string(self.effective_config, &["fontFamily"])
            .or_else(|| config_string(self.effective_config, &["themeVariables", "fontFamily"]))
            .unwrap_or_else(|| crate::config::MERMAID_DEFAULT_FONT_FAMILY_CSS.to_string());
        normalize_css_font_family(font_family.as_str())
    }

    pub(super) fn font_size_px(&self) -> f64 {
        crate::config::config_theme_font_size_css_or_root_number_px(self.effective_config, 16.0)
            .max(1.0)
    }
}

pub(super) fn css_rgba_fade(color: &str, opacity: f64) -> Option<String> {
    let color = Color::from_str(color.trim()).ok()?;
    Some(format!(
        "rgba({}, {}, {}, {})",
        color.red,
        color.green,
        color.blue,
        fmt(opacity)
    ))
}

pub(super) fn scoped_svg_id(diagram_id: &str, local_id: &str) -> String {
    format!("{diagram_id}-{local_id}")
}

pub(super) fn scoped_svg_url(diagram_id: &str, local_id: &str) -> String {
    format!("url(#{})", scoped_svg_id(diagram_id, local_id))
}

pub(super) fn fmt_debug_3dp(v: f64) -> String {
    let mut out = String::new();
    fmt_debug_3dp_into(&mut out, v);
    out
}

use std::fmt::Write as _;

fn trim_trailing_zeros_and_dot(out: &mut String, start: usize) {
    while out.len() > start && out.as_bytes()[out.len() - 1] == b'0' {
        out.pop();
    }
    if out.len() > start && out.as_bytes()[out.len() - 1] == b'.' {
        out.pop();
    }
}

pub(super) fn fmt_debug_3dp_into(out: &mut String, v: f64) {
    if !v.is_finite() || v.abs() < 0.0005 {
        out.push('0');
        return;
    }

    let scaled = v * 1000.0;
    let k = scaled.round() as i64;
    if k == 0 {
        out.push('0');
        return;
    }

    append_fixed_3dp_trimmed(out, k);
}

pub(super) fn fmt_string(v: f64) -> String {
    let mut out = String::new();
    fmt_into(&mut out, v);
    out
}

pub(super) fn fmt_display(v: f64) -> FmtDisplay {
    fmt(v)
}

pub(super) fn fmt_points(points: &[crate::model::LayoutPoint]) -> String {
    let mut out = String::new();
    push_points_attr(&mut out, points);
    out
}

pub(super) fn push_points_attr(out: &mut String, points: &[crate::model::LayoutPoint]) {
    for (idx, point) in points.iter().enumerate() {
        if idx > 0 {
            out.push(' ');
        }
        push_point_pair(out, point.x, point.y);
    }
}

pub(super) fn push_point_pair(out: &mut String, x: f64, y: f64) {
    let _ = write!(out, "{},{}", fmt_display(x), fmt_display(y));
}

pub(super) fn fmt(v: f64) -> FmtDisplay {
    FmtDisplay(v)
}

const MAX_SAFE_INTEGER_F64: f64 = 9_007_199_254_740_991.0;

fn fmt_fast_integer(v: f64) -> Option<i64> {
    (v.fract() == 0.0 && v.abs() <= MAX_SAFE_INTEGER_F64).then_some(v as i64)
}

#[derive(Debug, Clone, Copy)]
pub(super) struct FmtDisplay(f64);

impl std::fmt::Display for FmtDisplay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut v = self.0;
        if !v.is_finite() {
            return f.write_str("0");
        }

        if v.abs() < 1e-9 {
            v = 0.0;
        }
        let nearest = v.round();
        if (v - nearest).abs() < 1e-6 {
            v = nearest;
        }
        if v == -0.0 {
            v = 0.0;
        }
        if let Some(i) = fmt_fast_integer(v) {
            return write!(f, "{i}");
        }

        write!(f, "{v}")
    }
}

pub(super) fn fmt_into(out: &mut String, v: f64) {
    // Match how Mermaid/D3 generally stringify numbers for SVG attributes:
    // use a round-trippable decimal form (similar to JS `Number#toString()`),
    // but avoid `-0` and tiny float noise from our own calculations.
    if !v.is_finite() {
        out.push('0');
        return;
    }

    let mut v = if v.abs() < 1e-9 { 0.0 } else { v };
    let nearest = v.round();
    if (v - nearest).abs() < 1e-6 {
        v = nearest;
    }
    if v == -0.0 {
        v = 0.0;
    }
    if let Some(i) = fmt_fast_integer(v) {
        let _ = write!(out, "{i}");
        return;
    }

    let _ = write!(out, "{v}");
}

pub(super) fn fmt_path(v: f64) -> String {
    let mut out = String::new();
    fmt_path_into(&mut out, v);
    out
}

pub(super) fn fmt_path_into(out: &mut String, v: f64) {
    // D3's `d3-path` defaults to 3 fractional digits when stringifying path commands.
    // Upstream Mermaid fixtures match a `toFixed(3)`-like rounding behavior: round to nearest with
    // ties away from zero (not `Math.round`, which rounds negative halves toward +∞).
    if !v.is_finite() || v.abs() < 0.0005 {
        out.push('0');
        return;
    }

    let scaled = v * 1000.0;
    let k = if scaled < 0.0 {
        (scaled - 0.5).ceil() as i64
    } else {
        (scaled + 0.5).floor() as i64
    };
    if k == 0 {
        out.push('0');
        return;
    }
    append_fixed_3dp_trimmed(out, k);
}

fn append_fixed_3dp_trimmed(out: &mut String, k: i64) {
    if k == 0 {
        out.push('0');
        return;
    }

    let neg = k.is_negative();
    let abs = k.unsigned_abs();
    let int_part = abs / 1000;
    let frac = abs % 1000;

    if neg {
        out.push('-');
    }

    use std::fmt::Write as _;
    let _ = write!(out, "{int_part}");

    if frac == 0 {
        return;
    }

    let mut frac_str = [b'0'; 3];
    frac_str[0] = b'0' + ((frac / 100) as u8);
    frac_str[1] = b'0' + (((frac / 10) % 10) as u8);
    frac_str[2] = b'0' + ((frac % 10) as u8);

    let mut end = 3usize;
    while end > 0 && frac_str[end - 1] == b'0' {
        end -= 1;
    }
    if end == 0 {
        return;
    }

    out.push('.');
    for &b in &frac_str[..end] {
        out.push(b as char);
    }
}

pub(super) fn json_stringify_points(points: &[crate::model::LayoutPoint]) -> String {
    // Mermaid encodes `data-points` as Base64(JSON.stringify(points)).
    // JS `JSON.stringify` prints whole numbers without a `.0` suffix.
    //
    // For strict SVG XML parity we must also match V8's number-to-string behavior, including
    // tie-breaking cases where Rust's default float formatting can pick a different shortest
    // round-trippable decimal (e.g. `...0312` vs `...0313`).
    let mut out = String::new();
    let mut buf = ryu_js::Buffer::new();
    json_stringify_points_into(&mut out, points, &mut buf);
    out
}

pub(super) fn json_stringify_points_into(
    out: &mut String,
    points: &[crate::model::LayoutPoint],
    buf: &mut ryu_js::Buffer,
) {
    out.push('[');
    for (i, p) in points.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(r#"{"x":"#);
        out.push_str(js_number_to_string(p.x, buf));
        out.push_str(r#","y":"#);
        out.push_str(js_number_to_string(p.y, buf));
        out.push('}');
    }
    out.push(']');
}

fn js_number_to_string(mut v: f64, buf: &mut ryu_js::Buffer) -> &str {
    if !v.is_finite() {
        return "0";
    }
    if v == -0.0 {
        v = 0.0;
    }
    buf.format_finite(v)
}

pub(super) fn fmt_max_width_px(v: f64) -> String {
    let mut out = String::new();
    fmt_max_width_px_into(&mut out, v);
    out
}

pub(super) fn apply_root_viewport_override(
    diagram_id: &str,
    viewbox_attr: &mut String,
    width_attr: &mut String,
    height_attr: &mut String,
    max_width_style: &mut String,
    lookup: fn(&str) -> Option<(&'static str, &'static str)>,
) {
    let Some((viewbox, max_w)) = lookup(diagram_id) else {
        return;
    };

    if std::env::var_os("MERMAN_DISABLE_ROOT_VIEWPORT_OVERRIDES").is_some() {
        return;
    }

    *viewbox_attr = viewbox.to_string();
    let mut it = viewbox.split_whitespace();
    let _ = it.next(); // min-x
    let _ = it.next(); // min-y
    *width_attr = it.next().unwrap_or("0").to_string();
    *height_attr = it.next().unwrap_or("0").to_string();
    *max_width_style = max_w.to_string();
}

pub(super) fn fmt_max_width_px_into(out: &mut String, v: f64) {
    // Mermaid's `max-width: ...px` strings are effectively rendered with ~6 significant digits,
    // trimming trailing zeros (see upstream fixtures: `1184.88`, `432.812`, `85.4375`, `2019.2`).
    if !v.is_finite() || v.abs() < 0.0005 {
        out.push('0');
        return;
    }

    let abs = v.abs().max(0.0005);
    let exp10 = abs.log10().floor() as i32;
    let sig = 6i32;
    let decimals = (sig - 1 - exp10).clamp(0, 6) as usize;

    fn round_ties_to_even(x: f64) -> f64 {
        if !x.is_finite() {
            return 0.0;
        }
        let sign = if x.is_sign_negative() { -1.0 } else { 1.0 };
        let ax = x.abs();
        let f = ax.floor();
        let frac = ax - f;
        let i = if frac < 0.5 {
            f
        } else if frac > 0.5 {
            f + 1.0
        } else {
            // exactly halfway: choose the even integer
            let fi = f as i64;
            if fi % 2 == 0 { f } else { f + 1.0 }
        };
        sign * i
    }

    let scale = 10f64.powi(decimals as i32);
    let mut rounded = round_ties_to_even(v * scale) / scale;
    if rounded.abs() < 0.0005 {
        rounded = 0.0;
    }

    let start = out.len();
    let _ = write!(out, "{:.*}", decimals, rounded);
    if out.as_bytes()[start..].contains(&b'.') {
        trim_trailing_zeros_and_dot(out, start);
    }
    if out.len() == start + 2 && &out[start..] == "-0" {
        out.truncate(start);
        out.push('0');
    }
}

pub(super) fn escape_xml(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    escape_xml_into(&mut out, text);
    out
}

pub(super) fn decode_mermaid_entities_for_render_text(text: &str) -> Cow<'_, str> {
    // Mermaid preprocesses diagrams with `encodeEntities(...)`, rewriting `#...;` sequences into
    // `ﬂ°...¶ß` placeholders so grammars that treat `#` / `;` specially do not break.
    //
    // In headless SVG output we must decode those placeholders back into Unicode so text labels
    // match upstream Mermaid's browser-decoded output.
    if !text.contains('ﬂ') && !text.contains('¶') && !text.contains('#') {
        return Cow::Borrowed(text);
    }
    merman_core::entities::decode_mermaid_entities_to_unicode(text)
}

fn xml_text_is_plain_ascii(text: &str) -> bool {
    text.bytes().all(|b| {
        matches!(b, b'\t' | b'\n' | b'\r' | 0x20..=0x7f)
            && !matches!(b, b'&' | b'<' | b'"' | b'\'' | b'#')
    })
}

fn is_xml_10_char(ch: char) -> bool {
    // XML 1.0 excludes C0 controls except tab, LF, and CR.
    matches!(
        ch,
        '\u{9}'
            | '\u{A}'
            | '\u{D}'
            | '\u{20}'..='\u{D7FF}'
            | '\u{E000}'..='\u{FFFD}'
            | '\u{10000}'..='\u{10FFFF}'
    )
}

fn xml_text_replacement(ch: char) -> Option<&'static str> {
    if !is_xml_10_char(ch) {
        return Some("");
    }
    match ch {
        '&' => Some("&amp;"),
        '<' => Some("&lt;"),
        '"' => Some("&quot;"),
        '\'' => Some("&#39;"),
        _ => None,
    }
}

fn xml_attr_replacement(ch: char) -> Option<&'static str> {
    if !is_xml_10_char(ch) {
        return Some("");
    }
    match ch {
        '\n' => Some("&#10;"),
        '\r' => Some("&#13;"),
        '\t' => Some("&#9;"),
        '&' => Some("&amp;"),
        '<' => Some("&lt;"),
        '"' => Some("&quot;"),
        '\'' => Some("&#39;"),
        _ => None,
    }
}

pub(super) fn escape_xml_into(out: &mut String, text: &str) {
    if xml_text_is_plain_ascii(text) {
        out.push_str(text);
        return;
    }

    let decoded = decode_mermaid_entities_for_render_text(text);
    let text = decoded.as_ref();
    let mut start = 0usize;
    for (i, ch) in text.char_indices() {
        let Some(replacement) = xml_text_replacement(ch) else {
            continue;
        };
        if start < i {
            out.push_str(&text[start..i]);
        }
        out.push_str(replacement);
        start = i + ch.len_utf8();
    }
    if start < text.len() {
        out.push_str(&text[start..]);
    }
}

pub(super) fn escape_xml_display(text: &str) -> EscapeXmlDisplay<'_> {
    EscapeXmlDisplay(text)
}

pub(super) struct EscapeXmlDisplay<'a>(&'a str);

impl std::fmt::Display for EscapeXmlDisplay<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if xml_text_is_plain_ascii(self.0) {
            return f.write_str(self.0);
        }

        let decoded = decode_mermaid_entities_for_render_text(self.0);
        let text = decoded.as_ref();
        let mut start = 0usize;
        for (i, ch) in text.char_indices() {
            let Some(replacement) = xml_text_replacement(ch) else {
                continue;
            };
            if start < i {
                f.write_str(&text[start..i])?;
            }
            f.write_str(replacement)?;
            start = i + ch.len_utf8();
        }
        if start < text.len() {
            f.write_str(&text[start..])?;
        }
        Ok(())
    }
}

pub(super) fn escape_attr(text: &str) -> String {
    // Note: XML parsers normalize literal newlines/carriage-returns/tabs inside attribute values
    // into spaces. Mermaid's serialized SVGs typically encode those characters as numeric
    // character references (e.g. `&#10;`) to keep the attribute value stable across parsers.
    //
    // We mirror that behavior here to preserve parity for diagrams that embed newlines in IDs
    // (e.g. backtick-quoted multiline class names).
    let mut out = String::with_capacity(text.len());
    escape_attr_into(&mut out, text);
    out
}

pub(super) fn escape_attr_into(out: &mut String, text: &str) {
    let mut start = 0usize;
    for (i, ch) in text.char_indices() {
        let Some(replacement) = xml_attr_replacement(ch) else {
            continue;
        };
        if start < i {
            out.push_str(&text[start..i]);
        }
        out.push_str(replacement);
        start = i + ch.len_utf8();
    }
    if start < text.len() {
        out.push_str(&text[start..]);
    }
}

pub(super) fn escape_attr_display(text: &str) -> EscapeAttrDisplay<'_> {
    EscapeAttrDisplay(text)
}

pub(super) struct EscapeAttrDisplay<'a>(&'a str);

impl std::fmt::Display for EscapeAttrDisplay<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = self.0;
        let mut start = 0usize;
        for (i, ch) in text.char_indices() {
            let Some(replacement) = xml_attr_replacement(ch) else {
                continue;
            };
            if start < i {
                f.write_str(&text[start..i])?;
            }
            f.write_str(replacement)?;
            start = i + ch.len_utf8();
        }
        if start < text.len() {
            f.write_str(&text[start..])?;
        }
        Ok(())
    }
}

pub(super) fn replace_placeholders_once(out: &str, replacements: &[(&str, &str)]) -> String {
    if replacements.is_empty() {
        return out.to_string();
    }

    let mut hits: Vec<(usize, &str, &str)> = Vec::with_capacity(replacements.len());
    for (needle, value) in replacements {
        let Some(pos) = out.find(needle) else {
            continue;
        };
        hits.push((pos, *needle, *value));
    }

    if hits.is_empty() {
        return out.to_string();
    }

    hits.sort_by_key(|(pos, _, _)| *pos);

    let mut cap = out.len();
    for (_pos, needle, value) in &hits {
        cap = cap.saturating_sub(needle.len()).saturating_add(value.len());
    }

    let mut rebuilt = String::with_capacity(cap);
    let mut cursor: usize = 0;
    for (pos, needle, value) in hits {
        if pos < cursor {
            continue;
        }
        rebuilt.push_str(&out[cursor..pos]);
        rebuilt.push_str(value);
        cursor = pos + needle.len();
    }
    rebuilt.push_str(&out[cursor..]);
    rebuilt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fmt_into_matches_expected() {
        fn fmt_into_string(v: f64) -> String {
            let mut s = String::new();
            fmt_into(&mut s, v);
            s
        }

        assert_eq!(fmt_into_string(f64::NAN), "0");
        assert_eq!(fmt_into_string(f64::INFINITY), "0");
        assert_eq!(fmt_into_string(-0.0), "0");
        assert_eq!(fmt_into_string(0.0), "0");
        assert_eq!(fmt_into_string(1.0), "1");
        assert_eq!(fmt_into_string(1.0000004), "1");
        assert_eq!(fmt_into_string(-1.0000004), "-1");
    }

    #[test]
    fn fmt_display_matches_fmt() {
        let samples = [
            f64::NAN,
            f64::INFINITY,
            -f64::INFINITY,
            -0.0,
            0.0,
            1.0,
            -1.0,
            1.0000004,
            -1.0000004,
            1234.5678,
            -1234.5678,
        ];
        for v in samples {
            assert_eq!(fmt_display(v).to_string(), fmt_string(v));
            assert_eq!(fmt(v).to_string(), fmt_string(v));
        }
    }

    #[test]
    fn escape_xml_into_fast_path_matches_display_and_preserves_slow_paths() {
        fn escaped_into(text: &str) -> String {
            let mut out = String::new();
            escape_xml_into(&mut out, text);
            out
        }

        let samples = [
            ("plain-id_123", "plain-id_123"),
            (
                "x < y & \"z\" 'q'",
                "x &lt; y &amp; &quot;z&quot; &#39;q&#39;",
            ),
            ("#quot;", "&quot;"),
            ("ﬂ°quot¶ß", "&quot;"),
            ("café", "café"),
        ];

        for (src, expected) in samples {
            assert_eq!(escaped_into(src), expected);
            assert_eq!(escape_xml_display(src).to_string(), expected);
            assert_eq!(escape_xml(src), expected);
        }
    }

    #[test]
    fn escape_helpers_drop_xml_forbidden_control_chars() {
        let text = "A\u{1f}B\u{0}C\u{fffe}D";
        let expected_text = "ABCD";
        assert_eq!(escape_xml(text), expected_text);
        assert_eq!(escape_xml_display(text).to_string(), expected_text);

        let mut escaped_text = String::new();
        escape_xml_into(&mut escaped_text, text);
        assert_eq!(escaped_text, expected_text);

        let attr = "A\nB\rC\tD\u{1f}E\u{0}F\u{fffe}G";
        let expected_attr = "A&#10;B&#13;C&#9;DEFG";
        assert_eq!(escape_attr(attr), expected_attr);
        assert_eq!(escape_attr_display(attr).to_string(), expected_attr);

        let mut escaped_attr = String::new();
        escape_attr_into(&mut escaped_attr, attr);
        assert_eq!(escaped_attr, expected_attr);
    }

    #[test]
    fn css_rgba_fade_parses_css_colors() {
        assert_eq!(
            css_rgba_fade("#8090a0", 0.5).as_deref(),
            Some("rgba(128, 144, 160, 0.5)")
        );
        assert_eq!(
            css_rgba_fade("hsl(80, 100%, 96.2745098039%)", 0.5).as_deref(),
            Some("rgba(249, 255, 236, 0.5)")
        );
        assert!(css_rgba_fade("var(--not-runtime-resolved)", 0.5).is_none());
    }

    #[test]
    fn fmt_points_matches_expected() {
        let points = [
            crate::model::LayoutPoint { x: -0.0, y: 0.0 },
            crate::model::LayoutPoint {
                x: 1.0000004,
                y: -2.5,
            },
            crate::model::LayoutPoint {
                x: 3.25,
                y: f64::NAN,
            },
        ];

        assert_eq!(fmt_points(&points), "0,0 1,-2.5 3.25,0");
    }

    #[test]
    fn fmt_path_into_matches_expected() {
        fn fmt_path_into_string(v: f64) -> String {
            let mut s = String::new();
            fmt_path_into(&mut s, v);
            s
        }

        assert_eq!(fmt_path_into_string(f64::NAN), "0");
        assert_eq!(fmt_path_into_string(f64::INFINITY), "0");
        assert_eq!(fmt_path_into_string(0.0004), "0");
        assert_eq!(fmt_path_into_string(-0.0004), "0");
        assert_eq!(fmt_path_into_string(1.23456), "1.235");
        assert_eq!(fmt_path_into_string(1.0), "1");
        assert_eq!(fmt_path_into_string(-1.2345), "-1.235");
    }

    #[test]
    fn fmt_debug_3dp_into_matches_expected() {
        fn fmt_debug_3dp_into_string(v: f64) -> String {
            let mut s = String::new();
            fmt_debug_3dp_into(&mut s, v);
            s
        }

        assert_eq!(fmt_debug_3dp_into_string(f64::NAN), "0");
        assert_eq!(fmt_debug_3dp_into_string(0.0004), "0");
        assert_eq!(fmt_debug_3dp_into_string(1.0), "1");
        assert_eq!(fmt_debug_3dp_into_string(1.23), "1.23");
        assert_eq!(fmt_debug_3dp_into_string(1.2346), "1.235");
    }

    #[test]
    fn fmt_max_width_px_into_matches_expected() {
        fn fmt_max_width_px_into_string(v: f64) -> String {
            let mut s = String::new();
            fmt_max_width_px_into(&mut s, v);
            s
        }

        assert_eq!(fmt_max_width_px_into_string(f64::NAN), "0");
        assert_eq!(fmt_max_width_px_into_string(0.0004), "0");
        assert_eq!(fmt_max_width_px_into_string(1184.88), "1184.88");
        assert_eq!(fmt_max_width_px_into_string(2019.2), "2019.2");
    }
}
