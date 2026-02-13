// Shared SVG utility helpers (split from parity.rs).
//
// Keep behavior identical; these helpers are used across multiple diagram renderers.

pub(super) fn config_string(cfg: &serde_json::Value, path: &[&str]) -> Option<String> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_str().map(|s| s.to_string())
}

pub(super) fn json_f64(v: &serde_json::Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| v.as_i64().map(|n| n as f64))
        .or_else(|| v.as_u64().map(|n| n as f64))
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

pub(super) fn config_f64(cfg: &serde_json::Value, path: &[&str]) -> Option<f64> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    json_f64(cur)
}

pub(super) fn config_bool(cfg: &serde_json::Value, path: &[&str]) -> Option<bool> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    json_bool(cur)
}

pub(super) fn normalize_css_font_family(font_family: &str) -> String {
    let s = font_family.trim().trim_end_matches(';').trim();
    if s.is_empty() {
        return String::new();
    }

    // Mermaid's generated CSS uses a comma-separated `font-family` list with no extra whitespace
    // around commas (e.g. `"trebuchet ms",verdana,arial,sans-serif`). Normalize config-provided
    // values to the same format so strict SVG XML compares are stable.
    let mut parts: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut in_single = false;
    let mut in_double = false;

    for ch in s.chars() {
        match ch {
            '\'' if !in_double => {
                in_single = !in_single;
                cur.push(ch);
            }
            '"' if !in_single => {
                in_double = !in_double;
                cur.push(ch);
            }
            ',' if !in_single && !in_double => {
                let p = cur.trim();
                if !p.is_empty() {
                    parts.push(p.to_string());
                }
                cur.clear();
            }
            _ => cur.push(ch),
        }
    }

    let p = cur.trim();
    if !p.is_empty() {
        parts.push(p.to_string());
    }

    parts.join(",")
}

pub(super) fn theme_color(
    effective_config: &serde_json::Value,
    key: &str,
    fallback: &str,
) -> String {
    config_string(effective_config, &["themeVariables", key])
        .unwrap_or_else(|| fallback.to_string())
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
        out.push_str("0");
        return;
    }

    let scaled = v * 1000.0;
    let k = scaled.round() as i64;
    if k == 0 {
        out.push_str("0");
        return;
    }

    append_fixed_3dp_trimmed(out, k);
}

pub(super) fn fmt(v: f64) -> String {
    let mut out = String::new();
    fmt_into(&mut out, v);
    out
}

pub(super) fn fmt_display(v: f64) -> FmtDisplay {
    FmtDisplay(v)
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

        write!(f, "{v}")
    }
}

pub(super) fn fmt_into(out: &mut String, v: f64) {
    // Match how Mermaid/D3 generally stringify numbers for SVG attributes:
    // use a round-trippable decimal form (similar to JS `Number#toString()`),
    // but avoid `-0` and tiny float noise from our own calculations.
    if !v.is_finite() {
        out.push_str("0");
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

    let _ = write!(out, "{v}");
}

pub(super) fn fmt_path(v: f64) -> String {
    let mut out = String::new();
    fmt_path_into(&mut out, v);
    out
}

pub(super) fn fmt_path_into(out: &mut String, v: f64) {
    // D3's `d3-path` defaults to 3 fractional digits when stringifying path commands.
    // D3 uses `Math.round(x * 1000) / 1000` (ties half-up, including for negatives).
    if !v.is_finite() || v.abs() < 0.0005 {
        out.push_str("0");
        return;
    }

    let scaled = v * 1000.0;
    let k = (scaled + 0.5).floor() as i64;
    if k == 0 {
        out.push_str("0");
        return;
    }
    append_fixed_3dp_trimmed(out, k);
}

fn append_fixed_3dp_trimmed(out: &mut String, k: i64) {
    if k == 0 {
        out.push_str("0");
        return;
    }

    let neg = k.is_negative();
    let abs = k.unsigned_abs();
    let int_part = (abs / 1000) as u64;
    let frac = (abs % 1000) as u64;

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

pub(super) fn fmt_max_width_px_into(out: &mut String, v: f64) {
    // Mermaid's `max-width: ...px` strings are effectively rendered with ~6 significant digits,
    // trimming trailing zeros (see upstream fixtures: `1184.88`, `432.812`, `85.4375`, `2019.2`).
    if !v.is_finite() || v.abs() < 0.0005 {
        out.push_str("0");
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
        out.push_str("0");
    }
}

pub(super) fn escape_xml(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    escape_xml_into(&mut out, text);
    out
}

pub(super) fn escape_xml_into(out: &mut String, text: &str) {
    let bytes = text.as_bytes();
    let mut start = 0usize;
    for (i, &b) in bytes.iter().enumerate() {
        let esc = match b {
            b'&' => Some("&amp;"),
            b'<' => Some("&lt;"),
            b'"' => Some("&quot;"),
            b'\'' => Some("&#39;"),
            _ => None,
        };
        let Some(esc) = esc else {
            continue;
        };
        if start < i {
            out.push_str(&text[start..i]);
        }
        out.push_str(esc);
        start = i + 1;
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
        let text = self.0;
        let bytes = text.as_bytes();
        let mut start = 0usize;
        for (i, &b) in bytes.iter().enumerate() {
            let esc = match b {
                b'&' => Some("&amp;"),
                b'<' => Some("&lt;"),
                b'"' => Some("&quot;"),
                b'\'' => Some("&#39;"),
                _ => None,
            };
            let Some(esc) = esc else {
                continue;
            };
            if start < i {
                f.write_str(&text[start..i])?;
            }
            f.write_str(esc)?;
            start = i + 1;
        }
        if start < text.len() {
            f.write_str(&text[start..])?;
        }
        Ok(())
    }
}

pub(super) fn escape_attr(text: &str) -> String {
    // Attributes in our debug SVG only use escaped XML. No URL encoding here.
    escape_xml(text)
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
            assert_eq!(fmt_display(v).to_string(), fmt(v));
        }
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
        assert_eq!(fmt_path_into_string(-1.2345), "-1.234");
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
