//! Shared selector escaping and base styles for Mermaid SVG.

use std::borrow::Cow;
use std::fmt::Write;

use crate::{Result, svg_output::SvgOutput};

/// Escapes one identifier for use in a CSS selector embedded in SVG XML text.
///
/// This follows the CSSOM identifier serialization rules while using hexadecimal escapes for XML
/// text metacharacters. The latter keeps the serialized stylesheet well-formed without changing
/// the identifier CSS resolves after parsing. This boundary is only for CSS identifiers; XML
/// attributes and CSS `url(#fragment)` values use their own escaping paths.
///
/// Borrows the identifier when serialization preserves its spelling.
pub fn escape_css_identifier(identifier: &str) -> Cow<'_, str> {
    let first = identifier.chars().next();
    let mut escaped: Option<String> = None;

    for (index, (byte_index, character)) in identifier.char_indices().enumerate() {
        let code_point = character as u32;
        let is_null = character == '\0';
        let is_control = code_point <= 0x1f || code_point == 0x7f;
        let is_leading_digit =
            character.is_ascii_digit() && (index == 0 || (index == 1 && first == Some('-')));
        let is_xml_text_metacharacter = matches!(character, '&' | '<' | '>');
        let is_hex_escape = is_control || is_leading_digit || is_xml_text_metacharacter;
        let is_single_hyphen = index == 0 && character == '-' && identifier == "-";
        let is_literal = character.is_ascii_alphanumeric()
            || character == '-'
            || character == '_'
            || !character.is_ascii();

        if !is_null && !is_hex_escape && !is_single_hyphen && is_literal {
            if let Some(escaped) = &mut escaped {
                escaped.push(character);
            }
            continue;
        }

        let escaped = escaped.get_or_insert_with(|| {
            let mut escaped = String::with_capacity(identifier.len());
            escaped.push_str(&identifier[..byte_index]);
            escaped
        });
        if is_null {
            escaped.push('\u{fffd}');
        } else if is_hex_escape {
            write!(escaped, "\\{code_point:x} ")
                .expect("writing a CSS identifier escape to String cannot fail");
        } else if is_single_hyphen {
            escaped.push_str("\\-");
        } else {
            escaped.push('\\');
            escaped.push(character);
        }
    }

    match escaped {
        Some(escaped) => Cow::Owned(escaped),
        None => Cow::Borrowed(identifier),
    }
}

pub trait SvgDiagramIdValue: Copy + std::fmt::Display {
    fn semantic_value(&self) -> &str;
}

impl SvgDiagramIdValue for &str {
    fn semantic_value(&self) -> &str {
        self
    }
}

#[derive(Clone, Copy)]
pub struct MermaidBaseCss<'a> {
    pub font_family: &'a str,
    pub font_size_css: &'a str,
    pub normal_edge_stroke_width_css: &'a str,
    pub text_color: &'a str,
    pub line_color: &'a str,
    pub error_bkg: &'a str,
    pub error_text: &'a str,
}

pub fn write_mermaid_base_css_prefix<I>(
    out: &mut impl SvgOutput,
    id: I,
    css: MermaidBaseCss<'_>,
) -> Result<()>
where
    I: Copy + std::fmt::Display,
{
    emit_mermaid_base_css_prefix(out, &id, css)
}

#[inline(never)]
fn emit_mermaid_base_css_prefix(
    out: &mut dyn SvgOutput,
    id: &dyn std::fmt::Display,
    css: MermaidBaseCss<'_>,
) -> Result<()> {
    let _ = write!(
        out,
        r#"#{}{{font-family:{};font-size:{};fill:{};}}"#,
        id, css.font_family, css.font_size_css, css.text_color
    );
    out.checkpoint()?;
    out.push_str(
        r#"@keyframes edge-animation-frame{from{stroke-dashoffset:0;}}@keyframes dash{to{stroke-dashoffset:0;}}"#,
    );
    out.checkpoint()?;
    let _ = write!(
        out,
        r#"#{} .edge-animation-slow{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 50s linear infinite;stroke-linecap:round;}}#{} .edge-animation-fast{{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 20s linear infinite;stroke-linecap:round;}}"#,
        id, id
    );
    out.checkpoint()?;
    let _ = write!(
        out,
        r#"#{} .error-icon{{fill:{};}}#{} .error-text{{fill:{};stroke:{};}}"#,
        id, css.error_bkg, id, css.error_text, css.error_text
    );
    out.checkpoint()?;
    let _ = write!(
        out,
        r#"#{} .edge-thickness-normal{{stroke-width:{};}}#{} .edge-thickness-thick{{stroke-width:3.5px;}}#{} .edge-pattern-solid{{stroke-dasharray:0;}}#{} .edge-thickness-invisible{{stroke-width:0;fill:none;}}#{} .edge-pattern-dashed{{stroke-dasharray:3;}}#{} .edge-pattern-dotted{{stroke-dasharray:2;}}"#,
        id, css.normal_edge_stroke_width_css, id, id, id, id, id
    );
    out.checkpoint()?;
    let _ = write!(
        out,
        r#"#{} .marker{{fill:{};stroke:{};}}#{} .marker.cross{{stroke:{};}}"#,
        id, css.line_color, css.line_color, id, css.line_color
    );
    out.checkpoint()?;
    let _ = write!(
        out,
        r#"#{} svg{{font-family:{};font-size:{};}}#{} p{{margin:0;}}"#,
        id, css.font_family, css.font_size_css, id
    );
    out.checkpoint()
}

#[cfg(test)]
mod tests {
    use super::escape_css_identifier;
    use std::borrow::Cow;

    #[test]
    fn css_identifier_escape_handles_selector_and_xml_boundaries() {
        for (identifier, expected) in [
            ("seq:prod", r"seq\:prod"),
            ("1diagram", r"\31 diagram"),
            ("-1diagram", r"-\31 diagram"),
            ("-", r"\-"),
            ("a<b&c>d", r"a\3c b\26 c\3e d"),
            ("diagram\0id", "diagram\u{fffd}id"),
            ("主题:prod", r"主题\:prod"),
            (
                "a\0\u{1}\u{1f}\u{7f}<&>",
                "a\u{fffd}\\1 \\1f \\7f \\3c \\26 \\3e ",
            ),
            ("a b\\c", r"a\ b\\c"),
        ] {
            let escaped = escape_css_identifier(identifier);
            assert_eq!(escaped, expected, "identifier: {identifier:?}");
            assert!(
                matches!(escaped, Cow::Owned(_)),
                "identifier: {identifier:?}"
            );
        }
    }

    #[test]
    fn css_identifier_escape_preserves_the_existing_safe_id_spelling() {
        for identifier in [
            "diagram-1_name",
            "",
            "_1--主题",
            "--1",
            "é1",
            "\u{85}",
            "主题",
            "e\u{301}_💡\u{200f}",
        ] {
            let escaped = escape_css_identifier(identifier);
            assert_eq!(escaped, identifier);
            assert!(
                matches!(escaped, Cow::Borrowed(_)),
                "identifier: {identifier:?}"
            );
        }
    }
}
