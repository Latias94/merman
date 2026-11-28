use super::UNSUPPORTED_NATIVE_TEXT_STYLES;
use crate::mermaid_style::{
    CssFontSizeContext, css_font_size_override, parse_style_declaration, root_dependent_font_size,
    visit_style_declaration_boundaries_with_checkpoints,
};
use merman_model::{OperationControl, OperationControlResult};
use quick_xml::{Reader, XmlVersion, events::Event};

/// Reports unsupported styles unless the fragment contains only empty paragraph and span wrappers.
pub fn unsupported_measurable_html_style(
    html: &str,
    control: &OperationControl,
) -> OperationControlResult<Option<String>> {
    let style = unsupported_retained_html_style(html, control)?;
    if style.is_none() {
        return Ok(None);
    }
    let mut tags = Reader::from_str(html);
    let mut depth = 0usize;
    loop {
        control.checkpoint()?;
        let opening = match tags.read_event() {
            Ok(Event::Start(tag)) => {
                depth += 1;
                tag
            }
            Ok(Event::Empty(tag)) => tag,
            Ok(Event::End(_)) if depth > 0 => {
                depth -= 1;
                continue;
            }
            Ok(Event::Eof) if depth == 0 => return Ok(None),
            _ => return Ok(style),
        };
        if !matches!(opening.name().as_ref(), "p" | "span") {
            return Ok(style);
        }
        for attribute in opening.attributes() {
            control.checkpoint()?;
            if !attribute.is_ok_and(|attribute| {
                attribute.key.as_ref() == "style"
                    && attribute.normalized_value(XmlVersion::Implicit1_0).is_ok()
            }) {
                return Ok(style);
            }
        }
    }
}

/// Inspect retained XHTML. This does not grant permission to emit any CSS.
pub fn unsupported_retained_html_style(
    html: &str,
    control: &OperationControl,
) -> OperationControlResult<Option<String>> {
    let mut tags = Reader::from_str(html);
    loop {
        control.checkpoint()?;
        let opening = match tags.read_event() {
            Ok(Event::Start(opening) | Event::Empty(opening)) => opening,
            Ok(Event::Eof) => break,
            Err(_) => return Ok(Some("HTML label normalization".into())),
            _ => continue,
        };
        let mut style = None;
        for attribute in opening.attributes() {
            control.checkpoint()?;
            let Ok(attribute) = attribute else {
                return Ok(Some("HTML label normalization".into()));
            };
            if attribute.key.as_ref().eq_ignore_ascii_case("style") {
                style = Some(attribute);
            }
        }
        let Some(style) = style else {
            continue;
        };
        let style = merman_model::text::decode_html_entities_to_unicode(style.value.as_ref());
        let mut unsupported = None;
        let mut font_size: Option<(bool, bool)> = None;
        visit_style_declaration_boundaries_with_checkpoints(
            &style,
            &mut || control.checkpoint(),
            |boundary| {
                // A comment in a retained value must not hide its geometry property.
                let raw = boundary.raw();
                let mut declaration =
                    cssparser::Parser::new(raw.strip_suffix(';').unwrap_or(raw).trim());
                let Ok(property) = declaration.expect_ident_cloned() else {
                    return Ok(true);
                };
                if declaration.expect_colon().is_err() {
                    return Ok(true);
                }
                let property = property.to_ascii_lowercase();
                // Both shorthands can reset earlier longhands, including with inherit/unset.
                // Native measurement does not expand them.
                if property == "all" || property == "font" {
                    unsupported = Some(property);
                    return Ok(false);
                }
                if let Some(size) = css_font_size_override(raw)
                    && font_size.is_none_or(|(important, _)| size.important() || !important)
                {
                    let measured = parse_style_declaration(raw).is_some_and(|declaration| {
                        declaration.inherits_property_value()
                            || declaration
                                .resolve_font_size_px(CssFontSizeContext::uniform(16.0))
                                .is_some()
                    });
                    font_size = Some((size.important(), measured));
                }
                if !UNSUPPORTED_NATIVE_TEXT_STYLES.contains(&property.as_str()) {
                    return Ok(true);
                }
                let Some(first) = declaration.next().ok().cloned() else {
                    return Ok(true);
                };
                let inherits = matches!(first, cssparser::Token::Ident(value)
                    if value.eq_ignore_ascii_case("inherit") || value.eq_ignore_ascii_case("unset"))
                    && (declaration.is_exhausted()
                        || (declaration.try_parse(cssparser::parse_important).is_ok()
                            && declaration.is_exhausted()));
                if !inherits {
                    unsupported = Some(property);
                }
                Ok(unsupported.is_none())
            },
        )?;
        if unsupported.is_some() {
            return Ok(unsupported);
        }
        // Font longhands are measured, but rem still needs an independent document root.
        if root_dependent_font_size([style.as_ref()], control)?.is_some() {
            return Ok(Some(
                "font-size (rem requires a document-root font size)".into(),
            ));
        }
        if font_size.is_some_and(|(_, measured)| !measured) {
            return Ok(Some("font-size (unresolved CSS value)".into()));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_model::{CancelReason, OperationPhase};

    #[test]
    fn malformed_retained_xml_or_attributes_are_not_admitted() {
        for html in [
            "<b>x</i><span style='letter-spacing:20px'>AAAA</span>",
            "<span missing style='letter-spacing:20px'>AAAA</span>",
            "<span style='color:red' missing>AAAA</span>",
            "<span style='color:red' style='letter-spacing:20px'>AAAA</span>",
        ] {
            assert_eq!(
                unsupported_retained_html_style(html, &OperationControl::new())
                    .unwrap()
                    .as_deref(),
                Some("HTML label normalization"),
                "{html}",
            );
        }
    }

    #[test]
    fn measurable_style_skips_only_well_formed_empty_wrappers() {
        for (html, expected) in [
            ("<p><span style='letter-spacing:20px'></span></p>", None),
            (
                "<p><span missing></span></p>",
                Some("HTML label normalization"),
            ),
            (
                "<p><span style='letter-spacing:20px'/>",
                Some("letter-spacing"),
            ),
            (
                "<p><span style='letter-spacing:20px'><img/></span></p>",
                Some("letter-spacing"),
            ),
        ] {
            assert_eq!(
                unsupported_measurable_html_style(html, &OperationControl::new())
                    .unwrap()
                    .as_deref(),
                expected,
            );
        }
    }

    #[test]
    fn retained_css_declaration_scanning_remains_cancellable() {
        let html = format!(
            "<span style='{}'>AAAA</span>",
            "color:/* comment */red;".repeat(1024)
        );
        let control = OperationControl::new().for_phase(OperationPhase::Semantic);
        control.cancel_after_checkpoints(8);
        let error = unsupported_retained_html_style(&html, &control).unwrap_err();
        assert_eq!(error.phase, OperationPhase::Semantic);
        assert_eq!(error.reason, CancelReason::Requested);
    }
}
