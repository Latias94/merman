use cssparser::{BasicParseErrorKind, ParseError, ParseErrorKind, Parser, SourceLocation, Token};

const CSS_RESOURCE_NESTING_HARD_LIMIT: u8 = 64;

/// A lexical observation of a decoded CSS `url(...)` value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CssResourceUrl<'a> {
    /// A leading `#` followed by a nonempty fragment, after trimming the URL value.
    SameDocumentFragment(&'a str),
    /// An ASCII case-insensitive `data:` scheme prefix.
    DataScheme,
}

#[derive(Debug, thiserror::Error)]
#[error("line {line}, column {column}: {kind}")]
pub struct CssResourceScanError {
    line: u32,
    column: u32,
    kind: ParseErrorKind<ScanFailure>,
}

impl CssResourceScanError {
    pub fn is_malformed_token(&self) -> bool {
        matches!(
            self.kind,
            ParseErrorKind::Custom(ScanFailure::MalformedToken)
        )
    }
}

#[derive(Debug, thiserror::Error)]
enum ScanFailure {
    #[error("malformed CSS token")]
    MalformedToken,
    #[error("CSS resource nesting exceeds {}", CSS_RESOURCE_NESTING_HARD_LIMIT)]
    NestingLimit,
}

/// Visits fragment and `data:` URL tokens in source order, including nested component values.
/// The caller owns resource admission and reachability. Observations before a scan error
/// may already have been delivered.
pub fn visit_css_resource_urls(
    css: &str,
    mut observe: impl FnMut(CssResourceUrl<'_>),
) -> Result<(), CssResourceScanError> {
    let mut parser = Parser::new(css);
    let mut location = parser.current_source_location();
    visit_urls(&mut parser, &mut observe, 0, &mut location).map_err(|error| CssResourceScanError {
        line: location.line,
        column: location.column,
        kind: error.kind,
    })
}

fn visit_urls(
    parser: &mut Parser<'_>,
    observe: &mut impl FnMut(CssResourceUrl<'_>),
    depth: u8,
    error_location: &mut SourceLocation,
) -> Result<(), ParseError<ScanFailure>> {
    // cssparser consumes the rest of a nested block even on failure. Capture the location
    // before unwinding so diagnostics still identify the failing token.
    *error_location = parser.current_source_location();
    if depth > CSS_RESOURCE_NESTING_HARD_LIMIT {
        return Err(ParseError::custom(ScanFailure::NestingLimit));
    }

    loop {
        *error_location = parser.current_source_location();
        let token = match parser.next_including_whitespace() {
            Ok(token) => token.clone(),
            Err(error) if matches!(error.kind, BasicParseErrorKind::EndOfInput) => return Ok(()),
            Err(error) => return Err(error.into()),
        };

        match token {
            Token::UnquotedUrl(url) => observe_url(&url, observe),
            Token::Function(name) if name.eq_ignore_ascii_case("url") => {
                let url = parser.parse_nested_block(|nested| {
                    *error_location = nested.current_source_location();
                    let value = nested.expect_string_cloned()?;
                    *error_location = nested.current_source_location();
                    nested.expect_exhausted()?;
                    Ok::<_, ParseError<ScanFailure>>(value)
                })?;
                observe_url(&url, observe);
            }
            Token::Function(_)
            | Token::ParenthesisBlock
            | Token::SquareBracketBlock
            | Token::CurlyBracketBlock => {
                parser.parse_nested_block(|nested| {
                    visit_urls(nested, observe, depth + 1, error_location)
                })?;
            }
            Token::BadUrl(_) | Token::BadString(_) => {
                *error_location = parser.current_source_location();
                return Err(ParseError::custom(ScanFailure::MalformedToken));
            }
            _ => {}
        }
    }
}

fn observe_url(url: &str, observe: &mut impl FnMut(CssResourceUrl<'_>)) {
    let url = url.trim();
    if let Some(fragment) = url
        .strip_prefix('#')
        .filter(|fragment| !fragment.is_empty())
    {
        observe(CssResourceUrl::SameDocumentFragment(fragment));
    } else if url
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("data:"))
    {
        observe(CssResourceUrl::DataScheme);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visits_only_lexical_resource_urls_in_source_order() {
        let css = r#"u\72l(#p\61 int) 'url(#string)' /* url(#comment) */
            fn(url(' #glow '), URL(DaTa:invalid), url(https://example.invalid/x.svg#ext))
            url('') url('#') url(💡x) url(#paint)"#;
        let mut observations = Vec::new();
        visit_css_resource_urls(css, |url| {
            observations.push(match url {
                CssResourceUrl::SameDocumentFragment(fragment) => Some(fragment.to_owned()),
                CssResourceUrl::DataScheme => None,
            });
        })
        .unwrap();
        assert_eq!(
            observations,
            [
                Some("paint".into()),
                Some("glow".into()),
                None,
                Some("paint".into())
            ],
        );
    }
}
