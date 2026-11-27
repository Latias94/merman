// Shared SVG utility helpers (split from parity.rs).
//
// Keep behavior identical; these helpers are used across multiple diagram renderers.

use merman_markup::theme_color::{ColorChannel, ThemeColor, rgba};
use merman_render_core::svg_css::CssSelectorDiagramId;
#[cfg(test)]
use merman_render_core::svg_edge_parity::path_numbers::fmt_path_into;

use super::SvgOutput;

pub(super) use merman_render_core::html::{
    HTML_LABEL_FOREIGN_OBJECT_OVERFLOW_ATTR, html_label_overflow_attr,
};
pub(crate) use merman_render_core::theme_svg::cssom_color_value;
pub(super) use merman_render_core::xml::{
    EscapeAttrDisplay, escape_attr_display, escape_attr_into, escape_xml_display, escape_xml_into,
    escape_xml_serialized_text_into, escaped_attr_len, escaped_xml_len,
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

pub(super) use merman_render_core::svg_edge_parity::path_numbers::{
    fmt_path, json_stringify_points, json_stringify_points_into,
};

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
