//! Renderer-neutral font-family values.
//!
//! Mermaid accepts CSS font-family declarations, while a DrawingList consumer must receive a
//! deterministic list of actual family names.  This module deliberately implements the small
//! portable subset needed by the renderer-neutral contract.  SVG-only CSS sanitisation remains a
//! separate concern: a value that is safe to preserve in a browser is not necessarily resolvable
//! by a non-CSS host.

use std::fmt;

/// A validated, renderer-neutral font-family list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PortableFontFamilies {
    families: Vec<String>,
}

/// Why a CSS font-family declaration cannot be represented portably.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PortableFontFamilyError {
    reason: String,
}

impl PortableFontFamilyError {
    fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

impl fmt::Display for PortableFontFamilyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.reason)
    }
}

impl std::error::Error for PortableFontFamilyError {}

impl PortableFontFamilies {
    /// Parses a CSS font-family list into portable logical family names.
    ///
    /// The parser accepts quoted names, identifier sequences, and one optional declaration
    /// terminator (`Arial;`).  It intentionally rejects CSS functions, comments, escapes,
    /// declarations, and CSS-wide keywords because those values need a CSS evaluation context.
    pub(crate) fn parse(raw: &str) -> Result<Self, PortableFontFamilyError> {
        let value = raw.trim();
        if value.is_empty() {
            return Err(PortableFontFamilyError::new("is empty"));
        }
        if value.chars().any(char::is_control) {
            return Err(PortableFontFamilyError::new("contains control characters"));
        }
        if value.contains("/*") || value.contains("*/") {
            return Err(PortableFontFamilyError::new("contains CSS comments"));
        }
        if value.contains('\\') {
            return Err(PortableFontFamilyError::new("uses CSS escapes"));
        }
        if value.chars().any(is_markup_delimiter) {
            return Err(PortableFontFamilyError::new(
                "contains markup delimiters not portable to SVG styles",
            ));
        }

        let mut members = Vec::new();
        let mut member_start = 0;
        let mut quote = None;
        let mut end = value.len();
        for (index, character) in value.char_indices() {
            match character {
                '\'' | '"' if quote == Some(character) => quote = None,
                '\'' | '"' if quote.is_none() => quote = Some(character),
                ',' if quote.is_none() => {
                    members.push(&value[member_start..index]);
                    member_start = index + character.len_utf8();
                }
                ';' if quote.is_none() => {
                    if !value[index + character.len_utf8()..].trim().is_empty() {
                        return Err(PortableFontFamilyError::new(
                            "contains a non-terminal declaration separator",
                        ));
                    }
                    end = index;
                    break;
                }
                _ => {}
            }
        }
        if quote.is_some() {
            return Err(PortableFontFamilyError::new(
                "contains an unterminated quoted family",
            ));
        }
        members.push(&value[member_start..end]);

        let families = members
            .into_iter()
            .map(parse_member)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { families })
    }

    /// Validates logical family names already present in a DrawingList document.
    ///
    /// Unlike [`Self::parse`], this method receives names after their source quoting has been
    /// removed.  Punctuation such as a semicolon is therefore retained and quoted by
    /// [`Self::to_css`] when the SVG projection needs to spell the list again.
    pub(crate) fn from_resolved(families: &[String]) -> Result<Self, PortableFontFamilyError> {
        if families.is_empty() {
            return Err(PortableFontFamilyError::new("is empty"));
        }
        let mut validated = Vec::with_capacity(families.len());
        for family in families {
            if family.trim().is_empty() {
                return Err(PortableFontFamilyError::new(
                    "contains an empty family entry",
                ));
            }
            if family.chars().any(char::is_control) {
                return Err(PortableFontFamilyError::new("contains control characters"));
            }
            if family.contains('\\') {
                return Err(PortableFontFamilyError::new("uses CSS escapes"));
            }
            if family.chars().any(is_markup_delimiter) {
                return Err(PortableFontFamilyError::new(
                    "contains markup delimiters not portable to SVG styles",
                ));
            }
            if family.contains("/*") || family.contains("*/") {
                return Err(PortableFontFamilyError::new("contains CSS comments"));
            }
            if family
                .chars()
                .any(|character| matches!(character, '\'' | '"'))
            {
                return Err(PortableFontFamilyError::new(
                    "contains an unsupported quote",
                ));
            }
            if family.contains('(') || family.contains(')') {
                return Err(PortableFontFamilyError::new(
                    "contains unsupported CSS function syntax",
                ));
            }
            if family.to_ascii_lowercase().contains("!important") {
                return Err(PortableFontFamilyError::new("uses !important"));
            }
            validated.push(family.clone());
        }
        Ok(Self {
            families: validated,
        })
    }

    pub(crate) fn into_vec(self) -> Vec<String> {
        self.families
    }

    /// Spells the validated list as a deterministic CSS value for the SVG projection.
    pub(crate) fn to_css(&self) -> String {
        self.families
            .iter()
            .map(|family| {
                if is_plain_identifier(family) && !is_css_wide_keyword(family) {
                    family.clone()
                } else {
                    format!("\"{family}\"")
                }
            })
            .collect::<Vec<_>>()
            .join(",")
    }
}

fn parse_member(raw: &str) -> Result<String, PortableFontFamilyError> {
    let token = raw.trim();
    if token.is_empty() {
        return Err(PortableFontFamilyError::new(
            "contains an empty family entry",
        ));
    }

    let first = token.as_bytes()[0] as char;
    if matches!(first, '\'' | '"') {
        let last = token.chars().last();
        if last != Some(first) || token.len() <= first.len_utf8() * 2 {
            return Err(PortableFontFamilyError::new(
                "contains an unterminated or empty quoted family",
            ));
        }
        let inner = &token[first.len_utf8()..token.len() - first.len_utf8()];
        if inner.trim().is_empty() {
            return Err(PortableFontFamilyError::new(
                "contains an empty quoted family",
            ));
        }
        if inner
            .chars()
            .any(|character| matches!(character, '\'' | '"'))
        {
            return Err(PortableFontFamilyError::new(
                "contains nested or multiple quoted families",
            ));
        }
        if inner.contains('(') || inner.contains(')') {
            return Err(PortableFontFamilyError::new(
                "contains unsupported CSS function syntax",
            ));
        }
        if inner.to_ascii_lowercase().contains("!important") {
            return Err(PortableFontFamilyError::new("uses !important"));
        }
        return Ok(inner.to_string());
    }

    if token
        .chars()
        .any(|character| matches!(character, '\'' | '"'))
    {
        return Err(PortableFontFamilyError::new(
            "contains quotes outside a complete family token",
        ));
    }

    let normalized = token.split_ascii_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return Err(PortableFontFamilyError::new(
            "contains an empty family entry",
        ));
    }
    if is_css_wide_keyword(&normalized) {
        return Err(PortableFontFamilyError::new(format!(
            "uses the CSS-wide keyword `{normalized}`"
        )));
    }
    if !normalized.split(' ').all(is_plain_identifier) {
        return Err(PortableFontFamilyError::new(
            "contains unsupported CSS syntax or a non-portable identifier",
        ));
    }
    Ok(normalized)
}

fn is_plain_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    if first == '-' {
        let Some(second) = characters.next() else {
            return false;
        };
        if !(second == '-' || second == '_' || second.is_alphabetic()) {
            return false;
        }
    } else if !(first == '_' || first.is_alphabetic()) {
        return false;
    }
    characters.all(|character| character == '-' || character == '_' || character.is_alphanumeric())
}

fn is_css_wide_keyword(value: &str) -> bool {
    ["inherit", "initial", "revert", "revert-layer", "unset"]
        .into_iter()
        .any(|keyword| value.eq_ignore_ascii_case(keyword))
}

fn is_markup_delimiter(character: char) -> bool {
    matches!(character, '<' | '>' | '&')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_quoted_spacing_semicolons_and_one_terminal_separator() {
        let families =
            PortableFontFamilies::parse(r#"' Foo ', 'A;B', Arial;"#).expect("portable font list");
        assert_eq!(
            families.families,
            [" Foo ".to_string(), "A;B".to_string(), "Arial".to_string()]
        );
        assert_eq!(families.to_css(), r#"" Foo ","A;B",Arial"#);
    }

    #[test]
    fn rejects_css_evaluation_and_malformed_members() {
        for value in [
            "",
            "Arial,,sans-serif",
            "Arial,",
            "var(--font)",
            "env(--font)",
            "font-family()",
            "Arial/**/,sans-serif",
            "Arial !important",
            "Arial\\ Black",
            "'font & text'",
            "'font</style>'",
            "Arial, 'unterminated",
            "inherit",
            "Arial; color: red",
        ] {
            assert!(PortableFontFamilies::parse(value).is_err(), "{value:?}");
        }
    }

    #[test]
    fn quoted_css_wide_keyword_is_a_literal_family_name() {
        let families = PortableFontFamilies::parse("'inherit'").expect("quoted family");
        assert_eq!(families.families, ["inherit"]);
        assert_eq!(families.to_css(), r#""inherit""#);
    }
}
