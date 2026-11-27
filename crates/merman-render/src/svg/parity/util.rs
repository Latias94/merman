#[cfg(test)]
mod tests {
    use merman_render_core::number_format::canonical_number;
    use merman_render_core::svg_edge_parity::path_numbers::fmt_path_into;
    use merman_render_core::theme_svg::cssom_color_value;
    use merman_render_core::xml::{
        escape_attr, escape_attr_display, escape_attr_into, escape_xml, escape_xml_display,
        escape_xml_into, escape_xml_raw_into, escape_xml_serialized_text_into, escaped_xml_len,
    };

    #[test]
    fn canonical_number_matches_svg_attributes() {
        for (value, expected) in [
            (f64::NAN, "0"),
            (f64::INFINITY, "0"),
            (-f64::INFINITY, "0"),
            (-0.0, "0"),
            (0.0, "0"),
            (1.0, "1"),
            (-1.0, "-1"),
            (1.0000004, "1"),
            (-1.0000004, "-1"),
            (1234.5678, "1234.5678"),
            (-1234.5678, "-1234.5678"),
        ] {
            assert_eq!(canonical_number(value).to_string(), expected);
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
            let mut bounded = merman_render_core::svg_output::BoundedSvgOutput::new(&meter);
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
