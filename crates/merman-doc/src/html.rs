use std::fmt;

/// SVG inputs must already have passed the caller's output safety policy.
#[derive(Clone, Copy, Debug)]
pub enum SvgVariants<'a> {
    Single(&'a str),
    RustdocTheme { light: &'a str, dark: &'a str },
}

const RUSTDOC_THEME_CSS: &str = r#"<style>
.merman-rustdoc-diagram,
.merman-rustdoc-theme {
  contain: layout paint;
  isolation: isolate;
  max-width: 100%;
  overflow: auto;
  position: relative;
}
.merman-rustdoc-diagram > svg,
.merman-rustdoc-theme > svg {
  display: block;
  max-width: 100%;
}
.merman-rustdoc-theme-dark {
  display: none;
}
:root[data-theme="dark"] .merman-rustdoc-theme-light,
:root[data-theme="ayu"] .merman-rustdoc-theme-light {
  display: none;
}
:root[data-theme="dark"] .merman-rustdoc-theme-dark,
:root[data-theme="ayu"] .merman-rustdoc-theme-dark {
  display: block;
}
</style>"#;

/// Count the exact UTF-8 bytes that `write_diagram_html` will write.
pub fn diagram_html_len(
    wrapper_id: Option<&str>,
    source: &str,
    variants: SvgVariants<'_>,
    show_source: bool,
) -> Option<usize> {
    let mut counter = ByteCounter(Some(0));
    write_diagram_html(&mut counter, wrapper_id, source, variants, show_source).ok()?;
    counter.0
}

/// Write a complete HTML block with Markdown-safe leading and trailing blank lines.
pub fn write_diagram_html(
    output: &mut impl fmt::Write,
    wrapper_id: Option<&str>,
    source: &str,
    variants: SvgVariants<'_>,
    show_source: bool,
) -> fmt::Result {
    output.write_str("\n\n")?;
    // Keep the stylesheet within the same HTML block as the wrapper. A style
    // element before the wrapper has a different CommonMark HTML block boundary.
    output.write_str("<div")?;
    if let Some(id) = wrapper_id {
        output.write_str(" id=\"")?;
        write_escaped_html(output, id)?;
        output.write_str("\"")?;
    }
    output.write_str(" class=\"merman-rustdoc-diagram\" data-merman-rustdoc=\"true\">\n")?;
    output.write_str(RUSTDOC_THEME_CSS)?;
    match variants {
        SvgVariants::Single(svg) => write_svg(output, svg)?,
        SvgVariants::RustdocTheme { light, dark } => {
            for (theme, svg) in [("light", light), ("dark", dark)] {
                write!(
                    output,
                    "\n<div class=\"merman-rustdoc-theme merman-rustdoc-theme-{theme}\" data-merman-rustdoc-theme=\"{theme}\">\n"
                )?;
                write_svg(output, svg)?;
                output.write_str("\n</div>\n")?;
            }
        }
    }
    output.write_str("</div>")?;
    if show_source {
        output.write_str("\n<details class=\"merman-rustdoc-source\"><summary>Mermaid source</summary>\n<pre><code class=\"language-mermaid\">")?;
        write_escaped_html(output, source)?;
        output.write_str("</code></pre>\n</details>")?;
    }
    output.write_str("\n\n")
}

// CommonMark ends a div HTML block at an empty line, including one inside SVG.
// Preserve XML text/attribute values while removing literal line breaks from the
// serialized SVG. This is serialization only; safety policy remains upstream.
fn write_svg(output: &mut impl fmt::Write, svg: &str) -> fmt::Result {
    use quick_xml::events::Event;
    if !svg.contains(['\r', '\n']) {
        return output.write_str(svg);
    }
    let mut reader = quick_xml::Reader::from_str(svg);
    loop {
        let event = reader.read_event().map_err(|_| fmt::Error)?;
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::Start(element) | Event::Empty(element) => {
                output.write_char('<')?;
                output.write_str(
                    std::str::from_utf8(element.name().as_ref()).map_err(|_| fmt::Error)?,
                )?;
                for attribute in element.attributes() {
                    let attribute = attribute.map_err(|_| fmt::Error)?;
                    output.write_char(' ')?;
                    output.write_str(
                        std::str::from_utf8(attribute.key.as_ref()).map_err(|_| fmt::Error)?,
                    )?;
                    output.write_str("=\"")?;
                    let value = attribute
                        .decoded_and_normalized_value(
                            quick_xml::XmlVersion::Implicit1_0,
                            reader.decoder(),
                        )
                        .map_err(|_| fmt::Error)?;
                    write_escaped_html(output, &value)?;
                    output.write_char('"')?;
                }
                output.write_str(if empty { "/>" } else { ">" })?;
            }
            Event::End(element) => {
                output.write_str("</")?;
                output.write_str(
                    std::str::from_utf8(element.name().as_ref()).map_err(|_| fmt::Error)?,
                )?;
                output.write_char('>')?;
            }
            Event::Text(text) => {
                let text = text.xml10_content().map_err(|_| fmt::Error)?;
                // Entity references arrive as separate GeneralRef events.
                write_escaped_html(output, &text)?;
            }
            Event::CData(text) => {
                write_escaped_html(output, &text.xml10_content().map_err(|_| fmt::Error)?)?;
            }
            Event::GeneralRef(reference) => {
                output.write_char('&')?;
                output.write_str(&reference.decode().map_err(|_| fmt::Error)?)?;
                output.write_char(';')?;
            }
            Event::Comment(_) => {}
            Event::Eof => return Ok(()),
            Event::Decl(_) | Event::PI(_) | Event::DocType(_) => return Err(fmt::Error),
        }
    }
}

fn write_escaped_html(output: &mut impl fmt::Write, input: &str) -> fmt::Result {
    for character in input.chars() {
        match character {
            '&' => output.write_str("&amp;")?,
            '<' => output.write_str("&lt;")?,
            '>' => output.write_str("&gt;")?,
            '"' => output.write_str("&quot;")?,
            '\'' => output.write_str("&#39;")?,
            '\n' => output.write_str("&#10;")?,
            '\r' => output.write_str("&#13;")?,
            '\t' => output.write_str("&#9;")?,
            _ => output.write_char(character)?,
        }
    }
    Ok(())
}

struct ByteCounter(Option<usize>);

impl fmt::Write for ByteCounter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.0 = self.0.and_then(|length| length.checked_add(value.len()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shares_theme_markup_and_escapes_untrusted_text() {
        let variants = SvgVariants::RustdocTheme {
            light: "<svg id=\"light\"/>",
            dark: "<svg id=\"dark\"/>",
        };
        let mut html = String::new();
        write_diagram_html(&mut html, Some("id\"<&"), "A[<Start & Go>]", variants, true).unwrap();
        assert!(html.contains("id=\"id&quot;&lt;&amp;\""));
        assert!(html.contains("A[&lt;Start &amp; Go&gt;]"));
        assert!(html.contains("data-merman-rustdoc-theme=\"dark\""));
        assert!(html.contains(":root[data-theme=\"ayu\"]"));
        assert!(html.contains("contain: layout paint"));
        assert_eq!(
            diagram_html_len(Some("id\"<&"), "A[<Start & Go>]", variants, true),
            Some(html.len())
        );
    }

    #[test]
    fn svg_blank_lines_preserve_xml_values_through_markdown() {
        let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\"><text xml:space=\"preserve\" data-literal=\"a\r\n\nb\" data-refs=\"a&#10;&#13;&#9;b\">a\r\n\n**b** &amp; c<![CDATA[\n\n<cdata>]]></text>\n\n<path\n\nd=\"M0 0\" /></svg>";
        let mut markdown = String::new();
        write_diagram_html(
            &mut markdown,
            None,
            "source\n\n**source**",
            SvgVariants::Single(svg),
            true,
        )
        .unwrap();
        markdown.push_str("**after**");
        let mut html = String::new();
        pulldown_cmark::html::push_html(&mut html, pulldown_cmark::Parser::new(&markdown));
        assert!(!html.contains("<strong>b</strong>"), "{html}");
        assert!(html.contains("<strong>after</strong>"), "{html}");
        let svg_start = html.find("<svg").unwrap();
        let svg_end = html.find("</svg>").unwrap() + "</svg>".len();
        let before = roxmltree::Document::parse(svg).unwrap();
        let after = roxmltree::Document::parse(&html[svg_start..svg_end]).unwrap();
        let before_text = before
            .descendants()
            .find(|node| node.has_tag_name("text"))
            .unwrap();
        let after_text = after
            .descendants()
            .find(|node| node.has_tag_name("text"))
            .unwrap();
        assert_eq!(before_text.text(), after_text.text());
        assert_eq!(
            before_text.attribute("data-literal"),
            after_text.attribute("data-literal")
        );
        assert_eq!(
            before_text.attribute("data-refs"),
            after_text.attribute("data-refs")
        );
        assert_eq!(
            before_text.attribute(("http://www.w3.org/XML/1998/namespace", "space")),
            after_text.attribute(("http://www.w3.org/XML/1998/namespace", "space"))
        );
        assert_eq!(
            diagram_html_len(None, "source\n\n**source**", SvgVariants::Single(svg), true),
            Some(markdown.len() - "**after**".len())
        );
    }

    #[test]
    fn multiline_svg_serialization_reports_invalid_xml() {
        let variants = SvgVariants::Single("<svg>\n\n</different>");
        assert!(write_diagram_html(&mut String::new(), None, "", variants, false).is_err());
        assert_eq!(diagram_html_len(None, "", variants, false), None);
        for svg in [
            "<?xml version=\"1.0\"?>\n<svg/>",
            "<?instruction data?>\n<svg/>",
            "<!DOCTYPE svg>\n<svg/>",
        ] {
            assert!(
                write_diagram_html(
                    &mut String::new(),
                    None,
                    "",
                    SvgVariants::Single(svg),
                    false
                )
                .is_err()
            );
        }
    }

    #[test]
    fn surrounding_markdown_keeps_block_boundaries() {
        for show_source in [true, false] {
            let mut markdown = "**before**".to_owned();
            write_diagram_html(
                &mut markdown,
                None,
                "A-->B\n\n**not Markdown**",
                SvgVariants::Single("<svg></svg>"),
                show_source,
            )
            .unwrap();
            markdown.push_str("**after**\n\n- next item");
            let mut html = String::new();
            pulldown_cmark::html::push_html(&mut html, pulldown_cmark::Parser::new(&markdown));
            assert!(html.contains("<strong>before</strong>"), "{html}");
            assert!(html.contains("<strong>after</strong>"), "{html}");
            assert!(html.contains("<li>next item</li>"), "{html}");
            assert!(!html.contains("&lt;svg"), "{html}");
            assert!(!html.contains("<strong>not Markdown</strong>"), "{html}");
        }
    }
}
