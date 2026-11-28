//! Admission checks shared by native text measurement consumers.

use super::{root_dependent_font_size, visit_style_declaration_boundaries_with_checkpoints};
use merman_model::{OperationControl, OperationControlResult};

#[cfg(feature = "svg-parse")]
mod retained_html;
#[cfg(feature = "svg-parse")]
pub use retained_html::unsupported_retained_html_style;

/// Properties whose geometry needs dedicated support in the native layout path.
pub const UNSUPPORTED_NATIVE_TEXT_STYLES: &[&str] = &[
    "letter-spacing",
    "word-spacing",
    "text-transform",
    "white-space",
    "font",
    "font-variant",
    "font-stretch",
    "font-feature-settings",
    "font-variation-settings",
    "font-kerning",
    "line-height",
    "text-indent",
    "writing-mode",
    "direction",
    "unicode-bidi",
    "text-anchor",
    "dominant-baseline",
    "alignment-baseline",
    "baseline-shift",
    "kerning",
    "text-rendering",
    "font-size-adjust",
];

fn declaration(raw: &str, control: &OperationControl) -> OperationControlResult<Option<String>> {
    control.checkpoint()?;
    // Check the supplied byte volume before handing indivisible tokens or comments to
    // cssparser, whose own cancellation checkpoints are at declaration boundaries.
    for _ in raw.as_bytes().chunks(1024) {
        control.checkpoint()?;
    }
    let mut unsupported = None;
    visit_style_declaration_boundaries_with_checkpoints(
        raw,
        &mut || control.checkpoint(),
        |boundary| {
            let mut parser = cssparser::Parser::new(boundary.raw());
            let Ok(property) = parser.expect_ident_cloned() else {
                return Ok(true);
            };
            // Source-style emission trims Unicode whitespace from property spellings.
            let property = property.trim();
            if parser.expect_colon().is_ok()
                && UNSUPPORTED_NATIVE_TEXT_STYLES
                    .iter()
                    .any(|name| property.eq_ignore_ascii_case(name))
            {
                unsupported = Some(property.to_string());
                return Ok(false);
            }
            Ok(true)
        },
    )?;
    control.checkpoint()?;
    Ok(unsupported)
}

/// Finds an unsupported authored declaration or a winning root-dependent size.
pub fn declarations<'a>(
    styles: impl IntoIterator<Item = &'a str>,
    control: &OperationControl,
) -> OperationControlResult<Option<String>> {
    let mut checked_styles = Vec::new();
    for raw in styles {
        if let Some(property) = declaration(raw, control)? {
            return Ok(Some(property));
        }
        checked_styles.push(raw);
    }
    root_dependent_size(checked_styles, control)
}

/// Reports an active root-dependent size at the native measurement boundary.
pub fn root_dependent_size<'a>(
    styles: impl IntoIterator<Item = &'a str>,
    control: &OperationControl,
) -> OperationControlResult<Option<String>> {
    Ok(root_dependent_font_size(styles, control)?
        .map(|_| "font-size (rem requires a document-root font size)".to_owned()))
}

/// Checks each element's authored styles in traversal order.
pub fn style_lists<'a>(
    lists: impl IntoIterator<Item = &'a [String]>,
    control: &OperationControl,
) -> OperationControlResult<Option<String>> {
    for styles in lists {
        control.checkpoint()?;
        if let Some(property) = declarations(styles.iter().map(String::as_str), control)? {
            return Ok(Some(property));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_model::OperationPhase;

    #[test]
    fn declarations_preserve_property_case_and_ignore_values() {
        let control = OperationControl::new();
        assert_eq!(
            declaration("fill:letter-spacing:4px; stroke:red", &control).unwrap(),
            None
        );
        assert_eq!(
            declaration("fill:red; \tWord-Spacing\n:2px", &control)
                .unwrap()
                .as_deref(),
            Some("Word-Spacing")
        );
    }

    #[test]
    fn declaration_headers_follow_css_tokens_and_boundaries() {
        let control = OperationControl::new();
        for raw in [
            "letter-spacing /* comment */ :20px",
            "/* comment */letter-spacing:20px",
            r"l\65tter-spacing:20px",
            r"fill:red; l\65tter-spacing/**/:20px",
            "--label:';font-size:99px';letter-spacing:20px",
            "letter-spacing\u{2003}:20px",
        ] {
            assert_eq!(
                declaration(raw, &control).unwrap().as_deref(),
                Some("letter-spacing"),
                "{raw}"
            );
        }
        for raw in [
            r#"font-family:"Face;letter-spacing:20px",monospace"#,
            r#"--label:"a\";word-spacing:20px";fill:red"#,
            "/* letter-spacing:20px */fill:red",
            "--label:fn(letter-spacing:20px;word-spacing:20px);fill:red",
        ] {
            assert_eq!(declaration(raw, &control).unwrap(), None, "{raw}");
        }
    }

    #[test]
    fn root_dependent_font_size_follows_the_winning_valid_declaration() {
        let control = OperationControl::new();
        for (styles, rejected) in [
            (vec!["font-size:2rem"], true),
            (vec![r"FONT-SIZE:/* root */2r\65m !important"], true),
            (
                vec![
                    r"FONT-SIZE:/* root */2r\65m !important",
                    "font-size:24px !important",
                ],
                false,
            ),
            (vec!["font-size:2rem", "font-size:24px"], false),
            (vec!["font-size:2rem;font-size:24px"], false),
            (vec!["font-size:2rem !important", "font-size:24px"], true),
            (
                vec!["font-size:2rem !important", "font-size:24px !important"],
                false,
            ),
            (vec!["font-size:24px !important", "font-size:2rem"], false),
            (vec!["font-size:2rem;font-size:inherit"], false),
            (vec!["font-size:2rem;font-size:24px junk !important"], true),
            (vec!["font-size:2rem;font-size:#fff !important"], true),
            (
                vec!["font-size:2rem;font-size:var(--size) !important"],
                true,
            ),
            (vec!["font-size:2rem;font-size:calc(20px + 1px)"], true),
            (vec!["font-size:2rem;font-size:-1rem"], true),
            (vec!["font-size:-1rem", "fill:red"], false),
            (vec!["content:'font-size:2rem'", "fill:red"], false),
        ] {
            assert_eq!(
                declarations(styles.iter().copied(), &control)
                    .unwrap()
                    .is_some(),
                rejected,
                "{styles:?}",
            );
        }
    }

    #[test]
    fn rem_ownership_is_unverified_without_changing_compatibility_resolution() {
        use crate::mermaid_style::{CssFontSizeContext, CssFontSizeOwnership};

        let rem = crate::mermaid_style::parse_style_declaration("font-size:2rem").unwrap();
        assert!(rem.analysis().depends_on_root_font_size());
        assert_eq!(
            rem.resolve_font_size_px(CssFontSizeContext::uniform(18.0)),
            Some(36.0)
        );
        assert_eq!(
            crate::mermaid_style::css_font_size_override("font-size:2rem")
                .unwrap()
                .ownership(),
            CssFontSizeOwnership::Unverified,
        );
        assert_eq!(
            crate::mermaid_style::css_font_size_override("font-size:2rem;font-size:24px")
                .unwrap()
                .ownership(),
            CssFontSizeOwnership::SourceOwned,
        );
    }

    #[test]
    fn native_admission_cancels_during_one_large_declaration() {
        for raw in [
            format!("fill:{};word-spacing:4px", "a".repeat(64 * 1024)),
            format!("/*{}*/", "a".repeat(64 * 1024)),
        ] {
            let control = OperationControl::new();
            control.cancel_after_checkpoints(8);
            let cancelled =
                declaration(&raw, &control.for_phase(OperationPhase::Semantic)).unwrap_err();
            assert_eq!(cancelled.phase, OperationPhase::Semantic);
        }
    }
}
