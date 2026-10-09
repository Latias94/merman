// Shared SVG utility helpers (split from parity.rs).
//
// Keep behavior identical; these helpers are used across multiple diagram renderers.

use merman_core::theme_color::{ColorChannel, ColorSourceFormat, ThemeColor, rgba};
use std::fmt::Write as _;

use super::SvgOutput;

/// CSS selector identity that preserves the renderer-owned diagram-id projection for every
/// selector occurrence while still supporting raw test identifiers.
#[derive(Clone, Copy)]
pub(super) struct CssSelectorDiagramId<I>(I);

impl<I> std::fmt::Display for CssSelectorDiagramId<I>
where
    I: super::SvgDiagramIdValue,
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let semantic = self.0.semantic_value();
        let normalized = semantic
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic)
            && semantic
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
        if normalized {
            std::fmt::Display::fmt(&self.0, formatter)
        } else {
            formatter.write_str(&crate::svg::escape_css_identifier(semantic))
        }
    }
}

pub(super) const fn css_selector_diagram_id<I>(diagram_id: I) -> CssSelectorDiagramId<I> {
    CssSelectorDiagramId(diagram_id)
}

/// Returns only the style attribute accepted by the output sink.
/// Final document checkpoints still own failures in subsequent writes.
pub(super) fn write_style_attribute<'a>(
    out: &mut impl SvgOutput,
    style: Option<&'a str>,
) -> Option<&'a str> {
    let style = style?;
    write!(out, r#" style="{}""#, escape_attr_display(style))
        .ok()
        .map(|()| style)
}

pub(super) use crate::config::{config_diagram_look, config_f64};

pub(super) fn config_string(cfg: &serde_json::Value, path: &[&str]) -> Option<String> {
    let mut cur = cfg;
    for key in path {
        cur = cur.get(*key)?;
    }
    cur.as_str().map(|s| s.to_string())
}

pub(super) use crate::config::json_bool;

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

pub(super) fn theme_token(
    effective_config: &serde_json::Value,
    key: &str,
    fallback: &str,
) -> String {
    config_string(effective_config, &["themeVariables", key])
        .unwrap_or_else(|| fallback.to_string())
}

pub(super) fn css_rgba_fade(color: &str, opacity: f64) -> crate::Result<String> {
    let color = ThemeColor::parse(color.trim())?;
    let faded = rgba(
        color.channel(ColorChannel::Red),
        color.channel(ColorChannel::Green),
        color.channel(ColorChannel::Blue),
        opacity,
    )?;
    Ok(faded)
}

pub(crate) fn cssom_color_value(value: &str) -> String {
    let value = value.trim();
    let Ok(color) = ThemeColor::parse(value) else {
        // Preserve CSS variables and browser-supported syntaxes outside Khroma's parser surface.
        return value.to_string();
    };

    if color.source_format() == ColorSourceFormat::Keyword {
        return value.to_ascii_lowercase();
    }

    let red = color.channel(ColorChannel::Red).round() as i64;
    let green = color.channel(ColorChannel::Green).round() as i64;
    let blue = color.channel(ColorChannel::Blue).round() as i64;
    let alpha = color.channel(ColorChannel::Alpha);
    if alpha < 1.0 {
        let alpha = (alpha * 1000.0).round() / 1000.0;
        format!("rgba({red}, {green}, {blue}, {})", fmt(alpha))
    } else {
        format!("rgb({red}, {green}, {blue})")
    }
}

#[derive(Clone, Copy)]
pub(super) struct ScopedSvgId<'a, I> {
    diagram_id: I,
    local_id: &'a str,
}

impl<I: std::fmt::Display> std::fmt::Display for ScopedSvgId<'_, I> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}-{}", self.diagram_id, self.local_id)
    }
}

pub(super) fn scoped_svg_id<I>(diagram_id: I, local_id: &str) -> ScopedSvgId<'_, I> {
    ScopedSvgId {
        diagram_id,
        local_id,
    }
}

#[derive(Clone, Copy)]
pub(super) struct ScopedSvgUrl<'a, I> {
    diagram_id: I,
    local_id: &'a str,
}

impl<I: std::fmt::Display> std::fmt::Display for ScopedSvgUrl<'_, I> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "url(#{})",
            scoped_svg_id(&self.diagram_id, self.local_id)
        )
    }
}

pub(super) fn scoped_svg_url<I>(diagram_id: I, local_id: &str) -> ScopedSvgUrl<'_, I> {
    ScopedSvgUrl {
        diagram_id,
        local_id,
    }
}

#[derive(Clone, Copy)]
pub(super) struct ScopedDropShadow<'a, I> {
    diagram_id: I,
    source: &'a str,
}

impl<I> std::fmt::Display for ScopedDropShadow<'_, I>
where
    I: Copy + std::fmt::Display,
{
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut parts = self.source.split("url(#drop-shadow)");
        formatter.write_str(parts.next().unwrap_or_default())?;
        for suffix in parts {
            write!(
                formatter,
                "{}",
                scoped_svg_url(self.diagram_id, "drop-shadow")
            )?;
            formatter.write_str(suffix)?;
        }
        Ok(())
    }
}

pub(super) fn scoped_drop_shadow<I>(diagram_id: I, source: &str) -> ScopedDropShadow<'_, I> {
    ScopedDropShadow { diagram_id, source }
}

pub(super) fn fmt_string(v: f64) -> String {
    let mut out = String::new();
    fmt_into(&mut out, v);
    out
}

pub(super) fn fmt_display(v: f64) -> crate::number_format::CanonicalNumber {
    fmt(v)
}

pub(super) fn fmt_points(points: &[crate::model::LayoutPoint]) -> String {
    let mut out = String::new();
    push_points_attr(&mut out, points);
    out
}

pub(super) fn push_points_attr(out: &mut impl SvgOutput, points: &[crate::model::LayoutPoint]) {
    for (idx, point) in points.iter().enumerate() {
        if idx > 0 {
            out.push(' ');
        }
        push_point_pair(out, point.x, point.y);
    }
}

pub(super) fn push_point_pair(out: &mut impl SvgOutput, x: f64, y: f64) {
    let _ = write!(out, "{},{}", fmt_display(x), fmt_display(y));
}

pub(super) fn fmt(v: f64) -> crate::number_format::CanonicalNumber {
    crate::number_format::canonical_number(v)
}

pub(super) fn fmt_into(out: &mut impl SvgOutput, v: f64) {
    // Match how Mermaid/D3 generally stringify numbers for SVG attributes:
    // use a round-trippable decimal form (similar to JS `Number#toString()`),
    // but avoid `-0` and tiny float noise from our own calculations.
    let _ = write!(out, "{}", crate::number_format::canonical_number(v));
}

pub(super) fn fmt_path(v: f64) -> String {
    let mut out = String::new();
    fmt_path_into(&mut out, v);
    out
}

pub(super) fn fmt_path_into(out: &mut impl SvgOutput, v: f64) {
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

fn append_fixed_3dp_trimmed(out: &mut impl SvgOutput, k: i64) {
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

pub(crate) fn escape_xml(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    escape_xml_into(&mut out, text);
    out
}

pub(super) use crate::entities::decode_mermaid_entities_for_render_text;

fn xml_text_is_plain_ascii(text: &str) -> bool {
    !text.contains("]]>")
        && text.bytes().all(|b| {
            matches!(b, b'\t' | b'\n' | b'\r' | 0x20..=0x7f)
                && !matches!(b, b'&' | b'<' | b'"' | b'\'' | b'#')
        })
}

fn xml_raw_text_is_plain_ascii(text: &str) -> bool {
    !text.contains("]]>")
        && text.bytes().all(|b| {
            matches!(b, b'\t' | b'\n' | b'\r' | 0x20..=0x7f)
                && !matches!(b, b'&' | b'<' | b'"' | b'\'')
        })
}

fn xml_text_replacement(ch: char) -> Option<&'static str> {
    if !crate::xml::is_xml_1_0_char(ch) {
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
    if !crate::xml::is_xml_1_0_char(ch) {
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

pub(super) fn escape_xml_into(out: &mut impl SvgOutput, text: &str) {
    if xml_text_is_plain_ascii(text) {
        out.push_str(text);
        return;
    }

    let decoded = decode_mermaid_entities_for_render_text(text);
    escape_xml_raw_into(out, decoded.as_ref());
}

pub(super) fn escape_xml_raw_into(out: &mut impl SvgOutput, text: &str) {
    escape_xml_raw_into_mode(out, text, false);
}

/// Serialize a text node with the same conservative escaping used by browser XMLSerializer.
/// Chromium emits `&gt;` for every literal greater-than character in text content, even though
/// XML only requires that escape when it would close a CDATA section.
pub(super) fn escape_xml_serialized_text_into(out: &mut impl SvgOutput, text: &str) {
    escape_xml_raw_into_mode(out, text, true);
}

fn escape_xml_raw_into_mode(out: &mut impl SvgOutput, text: &str, escape_greater_than: bool) {
    let plain_ascii = if escape_greater_than {
        text.bytes().all(|b| {
            matches!(b, b'\t' | b'\n' | b'\r' | 0x20..=0x7f)
                && !matches!(b, b'&' | b'<' | b'"' | b'\'' | b'>')
        })
    } else {
        xml_raw_text_is_plain_ascii(text)
    };
    if plain_ascii && !text.contains("]]>") {
        out.push_str(text);
        return;
    }

    let mut start = 0usize;
    for (i, ch) in text.char_indices() {
        let replacement = if ch == '>' && (escape_greater_than || text[..i].ends_with("]]")) {
            Some("&gt;")
        } else {
            xml_text_replacement(ch)
        };
        let Some(replacement) = replacement else {
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

pub(super) fn escaped_xml_len(text: &str) -> usize {
    if xml_text_is_plain_ascii(text) {
        return text.len();
    }

    let decoded = decode_mermaid_entities_for_render_text(text);
    let text = decoded.as_ref();
    text.char_indices().fold(0usize, |len, (index, ch)| {
        let replacement = if ch == '>' && text[..index].ends_with("]]") {
            Some("&gt;")
        } else {
            xml_text_replacement(ch)
        };
        len.saturating_add(replacement.map_or(ch.len_utf8(), str::len))
    })
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
            let replacement = if ch == '>' && text[..i].ends_with("]]") {
                Some("&gt;")
            } else {
                xml_text_replacement(ch)
            };
            let Some(replacement) = replacement else {
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

pub(crate) fn escape_attr(text: &str) -> String {
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

pub(super) fn escape_attr_into(out: &mut impl SvgOutput, text: &str) {
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

pub(super) fn escape_attr_display<T: std::fmt::Display>(value: T) -> EscapeAttrDisplay<T> {
    EscapeAttrDisplay(value)
}

pub(super) struct EscapeAttrDisplay<T>(T);

impl<T: std::fmt::Display> std::fmt::Display for EscapeAttrDisplay<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        struct EscapedAttrWriter<'formatter, 'output> {
            formatter: &'formatter mut std::fmt::Formatter<'output>,
        }

        impl std::fmt::Write for EscapedAttrWriter<'_, '_> {
            fn write_str(&mut self, text: &str) -> std::fmt::Result {
                let mut start = 0usize;
                for (i, ch) in text.char_indices() {
                    let Some(replacement) = xml_attr_replacement(ch) else {
                        continue;
                    };
                    if start < i {
                        self.formatter.write_str(&text[start..i])?;
                    }
                    self.formatter.write_str(replacement)?;
                    start = i + ch.len_utf8();
                }
                if start < text.len() {
                    self.formatter.write_str(&text[start..])?;
                }
                Ok(())
            }
        }

        write!(EscapedAttrWriter { formatter: f }, "{}", self.0)
    }
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
    fn serialized_xml_escape_matches_browser_text_node_serialization() {
        let mut out = String::new();
        escape_xml_serialized_text_into(&mut out, "x > y < z & 'quoted'");
        assert_eq!(out, "x &gt; y &lt; z &amp; &#39;quoted&#39;");
    }

    #[test]
    fn raw_xml_escape_preserves_entity_spelling() {
        let mut out = String::new();
        escape_xml_raw_into(&mut out, "&nbsp; &#160; &amp; &lt; #quot; <literal>");
        assert_eq!(
            out,
            "&amp;nbsp; &amp;#160; &amp;amp; &amp;lt; #quot; &lt;literal>"
        );
    }

    #[test]
    fn xml_text_escape_breaks_forbidden_cdata_terminators_after_entity_decode() {
        let source = "A]]#gt;B]]>C";
        let expected = "A]]&gt;B]]&gt;C";

        assert_eq!(escape_xml(source), expected);
        assert_eq!(escape_xml_display(source).to_string(), expected);

        let mut raw = String::new();
        escape_xml_raw_into(&mut raw, "A]]>B");
        assert_eq!(raw, "A]]&gt;B");

        let xml = format!("<text>{expected}</text>");
        let document =
            roxmltree::Document::parse(&xml).expect("escaped XML text must remain parseable");
        assert_eq!(document.root_element().text(), Some("A]]>B]]>C"));
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
    fn css_rgba_fade_matches_khroma_color_semantics() {
        assert_eq!(
            css_rgba_fade("#8090a0", 0.5).expect("valid hex color"),
            "rgba(128, 144, 160, 0.5)"
        );
        assert_eq!(
            css_rgba_fade("rebeccapurple", 0.5).expect("valid named color"),
            "rgba(102, 51, 153, 0.5)"
        );
        assert_eq!(
            css_rgba_fade("rgb(20% 40% 60% / 25%)", 0.2).expect("valid modern RGB color"),
            "rgba(51, 102, 153, 0.2)"
        );
        assert_eq!(
            css_rgba_fade("hsl(80, 100%, 96%)", 0.5).expect("valid HSL color"),
            "rgba(248.2, 255, 234.6, 0.5)"
        );
        assert_eq!(
            css_rgba_fade("#8090a080", 0.5).expect("valid alpha hex color"),
            "rgba(128, 144, 160, 0.5)"
        );
    }

    #[test]
    fn css_rgba_fade_rejects_unsupported_css_color() {
        let error = css_rgba_fade("var(--not-runtime-resolved)", 0.5)
            .expect_err("unresolved CSS variables are not khroma colors");

        assert!(error.to_string().contains("var(--not-runtime-resolved)"));
    }

    #[test]
    fn cssom_color_value_matches_browser_serialization_boundaries() {
        assert_eq!(cssom_color_value("#8090a0"), "rgb(128, 144, 160)");
        assert_eq!(cssom_color_value("hsl(80 100% 96%)"), "rgb(248, 255, 235)");
        assert_eq!(cssom_color_value("#8090a080"), "rgba(128, 144, 160, 0.502)");
        assert_eq!(cssom_color_value("ReBeccAPurple"), "rebeccapurple");
        assert_eq!(cssom_color_value("var(--MyColor)"), "var(--MyColor)");
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
}
