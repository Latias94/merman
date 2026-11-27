// Shared SVG utility helpers (split from parity.rs).
//
// Keep behavior identical; these helpers are used across multiple diagram renderers.

use merman_markup::theme_color::{ColorChannel, ThemeColor, rgba};
use merman_render_core::svg_css::CssSelectorDiagramId;

use super::SvgOutput;

#[cfg(feature = "diagram-state")]
pub(super) use crate::entities::decode_mermaid_entities_for_render_text;
pub(crate) use merman_render_core::theme_svg::cssom_color_value;
pub(super) use merman_render_core::xml::{
    EscapeAttrDisplay, escape_attr_display, escape_attr_into, escape_xml_display, escape_xml_into,
    escape_xml_serialized_text_into, escaped_xml_len,
};
pub(crate) use merman_render_core::xml::{escape_attr, escape_xml};

pub(super) use merman_render_core::svg_css::{scoped_drop_shadow, scoped_svg_id, scoped_svg_url};

pub(super) const fn css_selector_diagram_id<I>(diagram_id: I) -> CssSelectorDiagramId<I> {
    CssSelectorDiagramId(diagram_id)
}

#[cfg(feature = "diagram-class")]
pub(super) use merman_render_core::text::svg_label::write_style_attribute;

pub(super) use crate::config::config_f64;

pub(super) use crate::config::config_string;

pub(super) use crate::config::json_bool;

pub(super) fn config_bool(
    cfg: &(impl crate::config::ConfigLookup + ?Sized),
    path: &[&str],
) -> Option<bool> {
    crate::config::value_at(cfg, path).and_then(json_bool)
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

pub(super) fn fmt_string(v: f64) -> String {
    let mut out = String::new();
    fmt_into(&mut out, v);
    out
}

pub(super) fn fmt_display(v: f64) -> crate::number_format::CanonicalNumber {
    fmt(v)
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
    append_fixed_3dp_trimmed(out, path_coordinate_thousandths(v));
}

/// Write a path coordinate and return the value represented by those same bytes.
pub(super) fn fmt_path_coordinate_into(out: &mut String, v: f64) -> f64 {
    let thousandths = path_coordinate_thousandths(v);
    let start = out.len();
    append_fixed_3dp_trimmed(out, thousandths);
    let numerator = thousandths as f64;
    if thousandths.unsigned_abs() <= (1 << 53) {
        numerator / 1000.0
    } else {
        // Converting the numerator to f64 can round before the division at this magnitude.
        out[start..].parse().unwrap_or(numerator / 1000.0)
    }
}

pub(super) fn path_coordinate_thousandths(v: f64) -> i64 {
    // D3's `d3-path` defaults to 3 fractional digits when stringifying path commands.
    // Upstream Mermaid fixtures match a `toFixed(3)`-like rounding behavior: round to nearest with
    // ties away from zero (not `Math.round`, which rounds negative halves toward +∞).
    if !v.is_finite() || v.abs() < 0.0005 {
        return 0;
    }

    let scaled = v * 1000.0;
    if scaled < 0.0 {
        (scaled - 0.5).ceil() as i64
    } else {
        (scaled + 0.5).floor() as i64
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use merman_render_core::xml::escape_xml_raw_into;

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
            crate::xml_dom::Document::parse(&xml).expect("escaped XML text must remain parseable");
        assert_eq!(document.root_element().text(), Some("A]]>B]]>C"));
    }

    #[test]
    fn xml_text_escape_tracks_retained_brackets_when_forbidden_scalars_are_removed() {
        for (source, expected) in [
            ("]]\u{1f}>", "]]&gt;"),
            ("]\0]\u{ffff}>", "]]&gt;"),
            ("]]\u{fffe}\0\u{1f}>", "]]&gt;"),
            ("]]\0字>", "]]字>"),
            ("字😀]]\u{fffe}>\0 Ω >", "字😀]]&gt; Ω >"),
        ] {
            assert_eq!(escape_xml(source), expected);
            assert_eq!(escape_xml_display(source).to_string(), expected);
            assert_eq!(escaped_xml_len(source), expected.len());

            let mut raw = String::new();
            escape_xml_raw_into(&mut raw, source);
            assert_eq!(raw, expected);

            let policy = crate::resources::RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(
                    crate::resources::ResourceLimitId::MaxSvgBytes,
                    expected.len(),
                )
                .unwrap();
            let meter = crate::resources::OperationWorkMeter::new(policy);
            let mut bounded = crate::svg::parity::output::BoundedSvgOutput::new(&meter);
            escape_xml_into(&mut bounded, source);
            assert_eq!(bounded.finish().unwrap(), expected);

            let xml = format!("<text>{expected}</text>");
            crate::xml_dom::Document::parse(&xml).expect("the producer must emit valid XML");
        }

        let source = "Ω]]\u{1f}#gt;β >";
        let expected = "Ω]]&gt;β >";
        assert_eq!(escape_xml(source), expected);
        assert_eq!(escape_xml_display(source).to_string(), expected);
        assert_eq!(escaped_xml_len(source), expected.len());
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
