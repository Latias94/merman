use cssparser::{BasicParseErrorKind, Parser, ParserInput, Token};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fmt;

const SVG_RESOURCE_FINGERPRINT_DOMAIN: &[u8] = b"merman.svg-export-resources.v1";
const CSS_RESOURCE_NESTING_HARD_LIMIT: u8 = 64;

/// Stable identity of the resources retained by one sealed SVG artifact.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SvgResourceFingerprint([u8; 32]);

impl SvgResourceFingerprint {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        let mut out = String::with_capacity(64);
        for byte in self.0 {
            use std::fmt::Write as _;
            write!(&mut out, "{byte:02x}").expect("writing to String cannot fail");
        }
        out
    }
}

impl fmt::Debug for SvgResourceFingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SvgResourceFingerprint")
            .field(&self.to_hex())
            .finish()
    }
}

impl fmt::Display for SvgResourceFingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

/// Terminal inventory of same-document and inline resources referenced by a sealed SVG.
///
/// Construction is private to the terminal validator. Therefore every public value is closed:
/// all referenced fragments exist in the finalized document, and every non-fragment render
/// resource has already passed either the resvg-safe inline-image policy or an exact
/// renderer-owned typed-font embedding plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgResourceClosure {
    available_fragment_ids: Box<[String]>,
    referenced_fragment_ids: Box<[String]>,
    stylesheet_fragment_ids: Box<[String]>,
    inline_data_resources: usize,
}

impl SvgResourceClosure {
    pub fn available_fragment_ids(&self) -> &[String] {
        &self.available_fragment_ids
    }

    pub fn referenced_fragment_ids(&self) -> &[String] {
        &self.referenced_fragment_ids
    }

    /// Returns conditional fragment URLs found in stylesheets.
    ///
    /// These are not closure obligations because determining whether their selectors apply would
    /// require duplicating the downstream CSS engine. Missing values fail closed to the SVG
    /// property's initial or inherited value.
    pub fn stylesheet_fragment_ids(&self) -> &[String] {
        &self.stylesheet_fragment_ids
    }

    pub const fn inline_data_resource_count(&self) -> usize {
        self.inline_data_resources
    }

    pub const fn is_closed(&self) -> bool {
        true
    }
}

#[derive(Debug, Default)]
pub(crate) struct SvgResourceClosureBuilder {
    available_fragment_ids: BTreeSet<String>,
    referenced_fragment_ids: BTreeSet<String>,
    stylesheet_fragment_ids: BTreeSet<String>,
    inline_data_resources: usize,
}

impl SvgResourceClosureBuilder {
    pub(crate) fn observe_fragment_id(&mut self, id: &str) {
        self.available_fragment_ids.insert(id.to_owned());
    }

    pub(crate) fn observe_fragment_reference(&mut self, id: &str) {
        self.referenced_fragment_ids.insert(id.to_owned());
    }

    pub(crate) fn observe_inline_data_resource(&mut self) {
        self.inline_data_resources = self.inline_data_resources.saturating_add(1);
    }

    pub(crate) fn observe_inline_css_urls(&mut self, css: &str) -> Result<(), String> {
        self.observe_css_urls(css, CssReferenceScope::Concrete)
    }

    pub(crate) fn observe_stylesheet_urls(&mut self, css: &str) -> Result<(), String> {
        self.observe_css_urls(css, CssReferenceScope::ConditionalStylesheet)
    }

    fn observe_css_urls(&mut self, css: &str, scope: CssReferenceScope) -> Result<(), String> {
        let mut input = ParserInput::new(css);
        let mut parser = Parser::new(&mut input);
        collect_css_urls(&mut parser, self, scope, 0).map_err(|error| {
            format!(
                "failed to inventory terminal CSS resources at line {}, column {}: {}",
                error.location.line, error.location.column, error.kind
            )
        })
    }

    pub(crate) fn finish(self) -> Result<SvgResourceClosure, String> {
        if let Some(unresolved) = self
            .referenced_fragment_ids
            .difference(&self.available_fragment_ids)
            .next()
        {
            return Err(format!(
                "terminal SVG references missing same-document fragment #{unresolved}"
            ));
        }

        Ok(SvgResourceClosure {
            available_fragment_ids: self
                .available_fragment_ids
                .into_iter()
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            referenced_fragment_ids: self
                .referenced_fragment_ids
                .into_iter()
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            stylesheet_fragment_ids: self
                .stylesheet_fragment_ids
                .into_iter()
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            inline_data_resources: self.inline_data_resources,
        })
    }
}

pub(crate) fn fingerprint_svg_resources(
    svg: &str,
    font_catalog_fingerprint: &[u8; 32],
    font_source_policy: &crate::diagram_theme::FontSourcePolicy,
) -> SvgResourceFingerprint {
    let mut hasher = Sha256::new();
    update_len_prefixed(&mut hasher, SVG_RESOURCE_FINGERPRINT_DOMAIN);
    update_len_prefixed(&mut hasher, svg.as_bytes());
    update_len_prefixed(&mut hasher, font_catalog_fingerprint);
    for source in font_source_policy.priority() {
        update_len_prefixed(&mut hasher, source.id().as_bytes());
    }
    SvgResourceFingerprint(hasher.finalize().into())
}

fn collect_css_urls<'i, 't>(
    parser: &mut Parser<'i, 't>,
    resources: &mut SvgResourceClosureBuilder,
    scope: CssReferenceScope,
    depth: u8,
) -> Result<(), cssparser::ParseError<'i, String>> {
    if depth > CSS_RESOURCE_NESTING_HARD_LIMIT {
        return Err(parser.new_custom_error(format!(
            "CSS resource nesting exceeds {CSS_RESOURCE_NESTING_HARD_LIMIT}"
        )));
    }

    loop {
        let token = match parser.next_including_whitespace() {
            Ok(token) => token.clone(),
            Err(error) if matches!(error.kind, BasicParseErrorKind::EndOfInput) => return Ok(()),
            Err(error) => return Err(error.into()),
        };

        match token {
            Token::UnquotedUrl(url) => observe_url(resources, &url, scope),
            Token::Function(name) if name.eq_ignore_ascii_case("url") => {
                let url = parser.parse_nested_block(|nested| {
                    let value = nested.expect_string_cloned()?;
                    nested.expect_exhausted()?;
                    Ok::<_, cssparser::ParseError<'i, String>>(value)
                })?;
                observe_url(resources, &url, scope);
            }
            Token::Function(_)
            | Token::ParenthesisBlock
            | Token::SquareBracketBlock
            | Token::CurlyBracketBlock => {
                parser.parse_nested_block(|nested| {
                    collect_css_urls(nested, resources, scope, depth.saturating_add(1))
                })?;
            }
            Token::BadUrl(_) | Token::BadString(_) => {
                return Err(parser.new_custom_error(
                    "malformed CSS token survived terminal validation".to_owned(),
                ));
            }
            _ => {}
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CssReferenceScope {
    Concrete,
    ConditionalStylesheet,
}

fn observe_url(resources: &mut SvgResourceClosureBuilder, url: &str, scope: CssReferenceScope) {
    let url = url.trim();
    if let Some(fragment) = url
        .strip_prefix('#')
        .filter(|fragment| !fragment.is_empty())
    {
        match scope {
            CssReferenceScope::Concrete => resources.observe_fragment_reference(fragment),
            CssReferenceScope::ConditionalStylesheet => {
                resources
                    .stylesheet_fragment_ids
                    .insert(fragment.to_owned());
            }
        }
    } else if url
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("data:"))
    {
        resources.observe_inline_data_resource();
    }
}

fn update_len_prefixed(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_inventory_handles_quoted_unquoted_and_nested_urls() {
        let mut resources = SvgResourceClosureBuilder::default();
        resources
            .observe_inline_css_urls(
                ".node{fill:url(#paint);filter:drop-shadow(0 0 2px url('#glow'));--asset:url(data:image/png;base64,AAAA)}",
            )
            .unwrap();
        resources.observe_fragment_id("paint");
        resources.observe_fragment_id("glow");

        let closure = resources.finish().unwrap();
        assert_eq!(closure.referenced_fragment_ids(), &["glow", "paint"]);
        assert_eq!(closure.inline_data_resource_count(), 1);
    }

    #[test]
    fn unresolved_fragment_cannot_become_a_closed_resource_artifact() {
        let mut resources = SvgResourceClosureBuilder::default();
        resources.observe_fragment_reference("missing");

        assert_eq!(
            resources.finish().unwrap_err(),
            "terminal SVG references missing same-document fragment #missing"
        );
    }

    #[test]
    fn stylesheet_fragment_urls_do_not_require_selector_matching() {
        let mut resources = SvgResourceClosureBuilder::default();
        resources
            .observe_stylesheet_urls(".optional{fill:url(#family-gradient)}")
            .unwrap();

        let closure = resources.finish().unwrap();
        assert_eq!(closure.stylesheet_fragment_ids(), &["family-gradient"]);
        assert!(closure.referenced_fragment_ids().is_empty());
    }
}
