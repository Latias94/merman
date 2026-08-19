//! Catalog-bound text preparation and native shaping.
//!
//! The ordinary [`TextMeasurer`] trait is intentionally kept compatible with Mermaid's host
//! measurement protocol.  This module adds the stronger contract used by compiled custom themes:
//! a catalog is prepared once, its fingerprint is attested by the backend, and all subsequent
//! measurements use the retained catalog rather than rediscovering a CSS font family.

use super::{TextMeasurer, TextMetrics, TextStyle, WrapMode, split_html_br_lines};
use crate::diagram_theme::{
    FontAssetFingerprint, FontCatalog, FontCatalogFingerprint, FontFaceMetadata, FontSource,
    FontSourcePolicy, FontStack, FontStyle, GenericFontFamily, HostMeasurementFallback, LineHeight,
    MAX_FONT_FACES_HARD_CAP, TextTransform as ThemeTextTransform, ThemePortabilityRequirement,
    ThemeTextStyle, WhiteSpace,
};
use crate::resources::OperationWorkMeter;
use cssparser::{Delimiter, Parser, ParserInput};
use rustybuzz::{
    Direction as BuzzDirection, Feature, Language, Script as BuzzScript, UnicodeBuffer, Variation,
};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use unicode_script::{Script as UnicodeScript, UnicodeScript as _};
use unicode_segmentation::UnicodeSegmentation;

/// Version of the native/host catalog preparation contract.
pub const TEXT_LAYOUT_CONTRACT_VERSION: u32 = 1;

pub(crate) const MAX_TEXT_PROJECTION_BYTES: usize = 1_048_576;
const TEXT_PROJECTION_FINGERPRINT_VERSION: u32 = 1;
const TEXT_REQUEST_DIGEST_VERSION: u32 = 1;
const MAX_PREPARED_TEXT_DIAGNOSTICS: usize = 32;
const MAX_PREPARED_TEXT_DIAGNOSTIC_BYTES: usize = 1_024;
const MAX_PREPARED_TEXT_LINES: usize = MAX_TEXT_PROJECTION_BYTES.saturating_add(1);
const MAX_PREPARED_TEXT_RUNS: usize = MAX_TEXT_PROJECTION_BYTES;
const MAX_PREPARED_TEXT_GEOMETRY_PX: f64 = 1_000_000_000.0;
const MAX_PREPARED_TEXT_RESPONSE_BYTES: usize = 16 * 1024 * 1024;
const MAX_TEXT_PROJECTION_SPANS: usize = MAX_TEXT_PROJECTION_BYTES;
// Stable 64-bit upper-bound accounting keeps native and wasm32 admission identical.
const TEXT_PROJECTION_SPAN_RECORD_BYTES: usize = 32;
const MAX_TEXT_PROJECTION_SPAN_BYTES: usize =
    MAX_TEXT_PROJECTION_SPANS * TEXT_PROJECTION_SPAN_RECORD_BYTES;
// Replacing the former scalar bbox height with top/bottom extents adds one `f64` to its 96-byte
// stable upper bound.
const PREPARED_TEXT_LINE_RECORD_BYTES: usize = 104;
const PREPARED_TEXT_RUN_RECORD_BYTES: usize = 64;
const TEXT_BYTE_RANGE_RECORD_BYTES: usize = 16;
const PREPARED_TEXT_LABEL_EVIDENCE_RECORD_BYTES: usize = 128;
const PREPARED_TEXT_LINE_TEXT_RECORD_BYTES: usize = 32;
const PREPARED_TEXT_DIAGNOSTIC_RECORD_BYTES: usize = 32;
const PREPARED_TEXT_OWNER_RECORD_BYTES: usize = 512;
const PREPARED_TEXT_LEDGER_ENTRY_RECORD_BYTES: usize = 192;
const CATALOG_ADMITTED_TEXT_STYLE_RECORD_BYTES: usize = 128;
const FONT_STACK_FAMILY_RECORD_BYTES: usize = 32;
const MAX_STRUCTURED_COVERAGE_INPUT_BYTES: usize = MAX_TEXT_PROJECTION_BYTES * 16;
const MAX_STRUCTURED_FACE_INSPECTIONS: usize = MAX_TEXT_PROJECTION_BYTES * 32;
const STRUCTURED_COVERAGE_CACHE_LANES: usize = 2;
const MAX_STRUCTURED_COVERAGE_CACHE_ENTRIES: usize =
    MAX_TEXT_PROJECTION_BYTES * STRUCTURED_COVERAGE_CACHE_LANES;
const MAX_STRUCTURED_SPAN_VISITS: usize = MAX_TEXT_PROJECTION_BYTES * 4;
const MAX_STRUCTURED_GLYPH_VISITS: usize = MAX_TEXT_PROJECTION_BYTES * 64;
const MAX_STRUCTURED_CLUSTER_VISITS: usize = MAX_TEXT_PROJECTION_BYTES * 32;
const MAX_STRUCTURED_WRAP_BOUNDARY_VISITS: usize = MAX_TEXT_PROJECTION_BYTES * 32;

#[derive(Debug, Clone, Copy)]
struct ModeledRetainedBytes(usize);

impl ModeledRetainedBytes {
    const fn new(bytes: usize) -> Self {
        Self(bytes)
    }

    fn add_bytes(&mut self, bytes: usize) {
        self.0 = self.0.saturating_add(bytes);
    }

    fn add_records(&mut self, count: usize, record_bytes: usize) {
        self.add_bytes(count.saturating_mul(record_bytes));
    }

    const fn finish(self) -> usize {
        self.0
    }
}

pub(crate) fn merge_prepared_text_typography(
    base: &ThemeTextStyle,
    legacy: &TextStyle,
) -> Result<ThemeTextStyle, TextLayoutError> {
    let mut typography = base.clone();
    // `TextStyle.font_family` also carries Mermaid's compatibility base font. Callers that know
    // a family came from an explicit source declaration apply it through the provenance-aware
    // helper below instead of treating every compatibility value as user-authored.
    if !legacy.font_size.is_finite() || legacy.font_size <= 0.0 {
        return Err(TextLayoutError::InvalidRequest("font_size"));
    }
    typography = typography
        .with_font_size_px(legacy.font_size as f32)
        .map_err(|_| TextLayoutError::InvalidRequest("font_size"))?;
    if let Some(font_weight) = legacy.font_weight.as_deref() {
        let inherited_weight = typography.font_weight();
        typography = typography
            .with_font_weight(
                resolve_css_font_weight(font_weight, inherited_weight)
                    .ok_or(TextLayoutError::InvalidRequest("font_weight"))?,
            )
            .map_err(|_| TextLayoutError::InvalidRequest("font_weight"))?;
    }
    if let Some(font_style) = legacy.font_style.as_deref() {
        let style = match font_style.trim().to_ascii_lowercase().as_str() {
            "normal" => FontStyle::Normal,
            "italic" => FontStyle::Italic,
            "oblique" => FontStyle::Oblique,
            _ => return Err(TextLayoutError::InvalidRequest("font_style")),
        };
        typography = typography.with_font_style(style);
    }
    Ok(typography)
}

/// Final source-owned CSS typography declarations that affect prepared text geometry or content.
///
/// Values remain in CSS form until the inherited/theme typography is known. This preserves the
/// cascade while keeping CSS parsing and admission inside the text-layout boundary.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PreparedTextCssTypographyOverrides {
    font_family: Option<String>,
    font_size: Option<String>,
    line_height: Option<String>,
    letter_spacing: Option<String>,
    word_spacing: Option<String>,
    text_transform: Option<String>,
    unsupported_layout_property: Option<&'static str>,
}

impl PreparedTextCssTypographyOverrides {
    pub(crate) fn observe_declaration(&mut self, property: &str, value: &str) {
        match property.trim().to_ascii_lowercase().as_str() {
            "font-family" if crate::mermaid_style::is_safe_css_font_family_value(value) => {
                self.font_family = Some(value.trim().to_string());
            }
            "font-size" => self.font_size = Some(value.trim().to_string()),
            "line-height" => self.line_height = Some(value.trim().to_string()),
            "letter-spacing" => self.letter_spacing = Some(value.trim().to_string()),
            "word-spacing" => self.word_spacing = Some(value.trim().to_string()),
            "text-transform" => self.text_transform = Some(value.trim().to_string()),
            "white-space" | "word-wrap" | "word-break" | "overflow-wrap" | "hyphens" => {
                self.unsupported_layout_property = Some("source_text_layout");
            }
            _ => {}
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    pub(crate) fn has_font_size(&self) -> bool {
        self.font_size.is_some()
    }

    #[cfg(test)]
    pub(crate) fn font_family(&self) -> Option<&str> {
        self.font_family.as_deref()
    }

    #[cfg(test)]
    pub(crate) fn font_size(&self) -> Option<&str> {
        self.font_size.as_deref()
    }
}

pub(crate) fn merge_prepared_text_typography_with_css_overrides(
    base: &ThemeTextStyle,
    legacy: &TextStyle,
    overrides: Option<&PreparedTextCssTypographyOverrides>,
) -> Result<PreparedTextTypographyRequest, TextLayoutError> {
    let mut typography = merge_prepared_text_typography(base, legacy)?;
    let overrides = overrides.filter(|overrides| !overrides.is_empty());
    if overrides.is_some_and(|overrides| overrides.unsupported_layout_property.is_some()) {
        return Err(TextLayoutError::UnsupportedPreparedTextPath(
            "source_text_layout",
        ));
    }
    let css_font_stack = overrides
        .and_then(|overrides| overrides.font_family.as_deref())
        .map(|font_family| {
            parse_css_font_stack(font_family).ok_or(TextLayoutError::InvalidRequest("font_family"))
        })
        .transpose()?;
    if let Some(stack) = &css_font_stack {
        typography = typography.with_font_stack(stack.font_stack().clone());
    }
    let font_size_px = typography.font_size_px();
    if let Some(value) = overrides.and_then(|overrides| overrides.line_height.as_deref()) {
        typography = typography
            .with_line_height(parse_css_line_height(value)?)
            .map_err(|_| TextLayoutError::InvalidRequest("line_height"))?;
    }
    if let Some(value) = overrides.and_then(|overrides| overrides.letter_spacing.as_deref()) {
        typography = typography
            .with_letter_spacing_px(parse_css_spacing_px(value, font_size_px, "letter_spacing")?)
            .map_err(|_| TextLayoutError::InvalidRequest("letter_spacing"))?;
    }
    if let Some(value) = overrides.and_then(|overrides| overrides.word_spacing.as_deref()) {
        typography = typography
            .with_word_spacing_px(parse_css_spacing_px(value, font_size_px, "word_spacing")?)
            .map_err(|_| TextLayoutError::InvalidRequest("word_spacing"))?;
    }
    if let Some(value) = overrides.and_then(|overrides| overrides.text_transform.as_deref()) {
        typography = typography.with_transform(parse_css_text_transform(value)?);
    }
    Ok(PreparedTextTypographyRequest {
        typography,
        css_font_stack,
    })
}

#[derive(Debug, Clone, PartialEq)]
enum CssTypographyScalar {
    Number(f32),
    Percentage(f32),
    Px(f32),
    Em(f32),
    Rem(f32),
    Ident(String),
}

fn parse_css_typography_scalar(value: &str) -> Option<CssTypographyScalar> {
    use cssparser::Token;

    let mut input = ParserInput::new(value);
    let mut parser = Parser::new(&mut input);
    let scalar = match parser.next().ok()?.clone() {
        Token::Number { value, .. } => CssTypographyScalar::Number(value),
        Token::Percentage { unit_value, .. } => CssTypographyScalar::Percentage(unit_value),
        Token::Dimension { value, unit, .. } if unit.eq_ignore_ascii_case("px") => {
            CssTypographyScalar::Px(value)
        }
        Token::Dimension { value, unit, .. } if unit.eq_ignore_ascii_case("em") => {
            CssTypographyScalar::Em(value)
        }
        Token::Dimension { value, unit, .. } if unit.eq_ignore_ascii_case("rem") => {
            CssTypographyScalar::Rem(value)
        }
        Token::Ident(value) => CssTypographyScalar::Ident(value.to_ascii_lowercase()),
        _ => return None,
    };
    parser.expect_exhausted().ok()?;
    Some(scalar)
}

fn parse_css_line_height(value: &str) -> Result<LineHeight, TextLayoutError> {
    let line_height = match parse_css_typography_scalar(value) {
        Some(CssTypographyScalar::Ident(value)) if value == "normal" => LineHeight::Normal,
        Some(CssTypographyScalar::Number(value)) if value.is_finite() && value > 0.0 => {
            LineHeight::Multiplier(value)
        }
        Some(CssTypographyScalar::Percentage(value)) if value.is_finite() && value > 0.0 => {
            LineHeight::Multiplier(value)
        }
        Some(CssTypographyScalar::Px(value)) if value.is_finite() && value > 0.0 => {
            LineHeight::Px(value)
        }
        Some(CssTypographyScalar::Em(value)) if value.is_finite() && value > 0.0 => {
            LineHeight::Multiplier(value)
        }
        _ => return Err(TextLayoutError::InvalidRequest("line_height")),
    };
    Ok(line_height)
}

fn parse_css_spacing_px(
    value: &str,
    font_size_px: f32,
    field: &'static str,
) -> Result<f32, TextLayoutError> {
    let value = match parse_css_typography_scalar(value) {
        Some(CssTypographyScalar::Ident(value)) if value == "normal" => 0.0,
        Some(CssTypographyScalar::Number(value)) if value == 0.0 => 0.0,
        Some(CssTypographyScalar::Px(value)) => value,
        Some(CssTypographyScalar::Em(value)) => value * font_size_px,
        _ => return Err(TextLayoutError::InvalidRequest(field)),
    };
    value
        .is_finite()
        .then_some(value)
        .ok_or(TextLayoutError::InvalidRequest(field))
}

fn parse_css_text_transform(value: &str) -> Result<ThemeTextTransform, TextLayoutError> {
    match parse_css_typography_scalar(value) {
        Some(CssTypographyScalar::Ident(value)) if value == "none" => Ok(ThemeTextTransform::None),
        Some(CssTypographyScalar::Ident(value)) if value == "uppercase" => {
            Ok(ThemeTextTransform::Uppercase)
        }
        Some(CssTypographyScalar::Ident(value)) if value == "lowercase" => {
            Ok(ThemeTextTransform::Lowercase)
        }
        Some(CssTypographyScalar::Ident(value)) if value == "capitalize" => {
            Ok(ThemeTextTransform::Capitalize)
        }
        _ => Err(TextLayoutError::InvalidRequest("text_transform")),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ParsedCssFontStack {
    font_stack: FontStack,
    generic_families: BTreeMap<usize, GenericFontFamily>,
}

impl ParsedCssFontStack {
    pub(crate) const fn font_stack(&self) -> &FontStack {
        &self.font_stack
    }

    pub(crate) fn families(&self) -> &[String] {
        self.font_stack.families()
    }

    pub(crate) fn contains_named_family(&self) -> bool {
        self.font_stack
            .families()
            .iter()
            .enumerate()
            .any(|(index, _)| self.generic_family(index).is_none())
    }

    fn generic_family(&self, index: usize) -> Option<GenericFontFamily> {
        self.generic_families.get(&index).copied()
    }

    pub(crate) fn as_css(&self) -> String {
        let mut out = String::new();
        for (index, family) in self.font_stack.families().iter().enumerate() {
            if index > 0 {
                out.push_str(", ");
            }
            if self.generic_family(index).is_some() {
                out.push_str(family);
            } else {
                cssparser::serialize_string(family, &mut out)
                    .expect("serializing a CSS string into String cannot fail");
            }
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PreparedTextTypographyRequest {
    typography: ThemeTextStyle,
    css_font_stack: Option<ParsedCssFontStack>,
}

impl PreparedTextTypographyRequest {
    const fn typography(&self) -> &ThemeTextStyle {
        &self.typography
    }

    const fn css_font_stack(&self) -> Option<&ParsedCssFontStack> {
        self.css_font_stack.as_ref()
    }
}

pub(crate) fn parse_css_font_stack(value: &str) -> Option<ParsedCssFontStack> {
    let mut input = ParserInput::new(value);
    let mut parser = Parser::new(&mut input);
    let mut families = Vec::new();
    let mut generic_families = BTreeMap::new();

    while !parser.is_exhausted() {
        let family_index = families.len();
        let family = if let Ok(value) = parser.try_parse(|input| input.expect_string_cloned()) {
            value.to_string()
        } else {
            let first = parser.expect_ident_cloned().ok()?;
            let mut components = vec![first.to_string()];
            while let Ok(component) = parser.try_parse(|input| input.expect_ident_cloned()) {
                components.push(component.to_string());
            }
            if components.iter().any(|component| {
                matches!(
                    component.to_ascii_lowercase().as_str(),
                    "inherit" | "initial" | "revert" | "revert-layer" | "unset"
                )
            }) {
                return None;
            }
            if components.len() == 1
                && let Some(generic) = GenericFontFamily::from_css_keyword(&components[0])
            {
                generic_families.insert(family_index, generic);
            }
            components.join(" ")
        };
        families.push(family);
        if parser.is_exhausted() {
            break;
        }
        parser.expect_comma().ok()?;
        if parser.is_exhausted() {
            return None;
        }
    }

    Some(ParsedCssFontStack {
        font_stack: FontStack::new(families).ok()?,
        generic_families,
    })
}

/// One catalog-admitted font decision shared by native shaping and label emission.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CatalogAdmittedTextStyle {
    typography: ThemeTextStyle,
    catalog_fingerprint: FontCatalogFingerprint,
}

impl CatalogAdmittedTextStyle {
    pub(crate) const fn typography(&self) -> &ThemeTextStyle {
        &self.typography
    }

    pub(crate) const fn catalog_fingerprint(&self) -> FontCatalogFingerprint {
        self.catalog_fingerprint
    }

    pub(crate) fn line_height_em(&self) -> f64 {
        let value = match self.typography.line_height() {
            LineHeight::Normal => 1.2,
            LineHeight::Multiplier(value) => f64::from(value),
            LineHeight::Px(value) => f64::from(value) / f64::from(self.typography.font_size_px()),
        };
        (value * 1_000_000.0).round() / 1_000_000.0
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        let mut retained = ModeledRetainedBytes::new(CATALOG_ADMITTED_TEXT_STYLE_RECORD_BYTES);
        for family in self.typography.font_stack().families() {
            retained.add_records(1, FONT_STACK_FAMILY_RECORD_BYTES);
            retained.add_bytes(family.len());
        }
        retained.finish()
    }

    pub(crate) fn merge_emission_font_style(&self, existing: Option<&str>) -> String {
        self.merge_emission_font_style_with_transform(existing, false)
    }

    /// Merges admitted typography after Flowchart has materialized its visible text transform.
    pub(crate) fn merge_materialized_emission_font_style(&self, existing: Option<&str>) -> String {
        self.merge_emission_font_style_with_transform(existing, true)
    }

    fn merge_emission_font_style_with_transform(
        &self,
        existing: Option<&str>,
        remove_text_transform: bool,
    ) -> String {
        use std::fmt::Write as _;

        let mut style =
            retain_non_font_declarations(existing.unwrap_or_default(), remove_text_transform);
        if !style.is_empty() {
            style.push(';');
        }
        let _ = write!(
            style,
            "font-family:{} !important;font-size:{}px !important;font-weight:{} !important;font-style:{} !important;line-height:{} !important;letter-spacing:{}px !important;word-spacing:{}px !important",
            catalog_font_stack_css(self.typography.font_stack()),
            self.typography.font_size_px(),
            self.typography.font_weight(),
            self.typography.font_style().id(),
            self.line_height_em(),
            self.typography.letter_spacing_px(),
            self.typography.word_spacing_px(),
        );
        style
    }
}

fn catalog_font_stack_css(stack: &FontStack) -> String {
    let mut out = String::new();
    for (index, family) in stack.families().iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        cssparser::serialize_string(family, &mut out)
            .expect("serializing a CSS string into String cannot fail");
    }
    out
}

fn retain_non_font_declarations(existing: &str, remove_text_transform: bool) -> String {
    let mut input = ParserInput::new(existing);
    let mut parser = Parser::new(&mut input);
    let mut retained = String::new();

    while !parser.is_exhausted() {
        let start = parser.position();
        let property = parser.parse_until_after(Delimiter::Semicolon, |declaration| {
            let property = declaration.expect_ident_cloned()?.to_string();
            declaration.expect_colon()?;
            while declaration.next_including_whitespace().is_ok() {}
            Ok::<_, cssparser::ParseError<'_, ()>>(property)
        });
        let raw = parser
            .slice(start..parser.position())
            .trim()
            .trim_end_matches(';')
            .trim();
        let is_admitted_property = property.as_ref().is_ok_and(|property| {
            matches!(
                property.to_ascii_lowercase().as_str(),
                "font-family"
                    | "font-size"
                    | "font-weight"
                    | "font-style"
                    | "line-height"
                    | "letter-spacing"
                    | "word-spacing"
            )
        }) || (remove_text_transform
            && property
                .as_ref()
                .is_ok_and(|property| property.eq_ignore_ascii_case("text-transform")));
        if !raw.is_empty() && !is_admitted_property {
            if !retained.is_empty() {
                retained.push(';');
            }
            retained.push_str(raw);
        }
    }
    retained
}

pub(crate) fn resolve_css_font_weight(value: &str, inherited: u16) -> Option<u16> {
    match value.trim().to_ascii_lowercase().as_str() {
        "normal" => Some(400),
        "bold" => Some(700),
        "bolder" => Some(relative_font_weight(inherited, true)),
        "lighter" => Some(relative_font_weight(inherited, false)),
        value => value
            .parse::<u16>()
            .ok()
            .filter(|weight| (1..=1000).contains(weight)),
    }
}

fn relative_font_weight(inherited: u16, bolder: bool) -> u16 {
    match (inherited, bolder) {
        (1..=99, true) => 400,
        (1..=99, false) => inherited,
        (100..=349, true) => 400,
        (100..=349, false) => 100,
        (350..=549, true) => 700,
        (350..=549, false) => 100,
        (550..=749, true) => 900,
        (550..=749, false) => 400,
        (750..=899, true) => 900,
        (750..=899, false) => 700,
        (900..=1000, true) => inherited,
        (900..=1000, false) => 700,
        _ => inherited,
    }
}

/// A half-open UTF-8 byte range in either projection coordinate space.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextByteRange {
    start: usize,
    end: usize,
}

impl TextByteRange {
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub const fn start(self) -> usize {
        self.start
    }

    pub const fn end(self) -> usize {
        self.end
    }

    pub fn as_range(&self) -> std::ops::Range<usize> {
        self.start..self.end
    }
}

/// One ordered mapping atom between source and visible UTF-8 coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceVisibleSpan {
    source: TextByteRange,
    visible: TextByteRange,
}

const _: () =
    assert!(std::mem::size_of::<SourceVisibleSpan>() <= TEXT_PROJECTION_SPAN_RECORD_BYTES);

impl SourceVisibleSpan {
    pub const fn new(source: TextByteRange, visible: TextByteRange) -> Self {
        Self { source, visible }
    }

    pub const fn source(self) -> TextByteRange {
        self.source
    }

    pub const fn visible(self) -> TextByteRange {
        self.visible
    }
}

/// Canonical identity of one validated source-to-visible projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct TextProjectionFingerprint([u8; 32]);

impl TextProjectionFingerprint {
    pub(crate) const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Canonical identity of one prepared-label request within a catalog session.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextLayoutRequestDigest([u8; 32]);

impl TextLayoutRequestDigest {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for TextLayoutRequestDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("TextLayoutRequestDigest(\"")?;
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        formatter.write_str("\")")
    }
}

const PREPARED_TEXT_LABEL_ID_PREFIX: &str = "merman-prepared-";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PreparedTextLabelFamily {
    Flowchart,
    Swimlane,
    State,
    Sequence,
}

impl PreparedTextLabelFamily {
    const fn id(self) -> &'static str {
        match self {
            Self::Flowchart => "flowchart",
            Self::Swimlane => "swimlane",
            Self::State => "state",
            Self::Sequence => "sequence",
        }
    }

    fn from_id(id: &str) -> Option<Self> {
        match id {
            "flowchart" => Some(Self::Flowchart),
            "swimlane" => Some(Self::Swimlane),
            "state" => Some(Self::State),
            "sequence" => Some(Self::Sequence),
            _ => None,
        }
    }
}

/// Operation-local identity attached only to SVG text emitted from admitted prepared evidence.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PreparedTextLabelId {
    family: PreparedTextLabelFamily,
    key: u32,
}

impl PreparedTextLabelId {
    pub(crate) const fn new(family: PreparedTextLabelFamily, key: u32) -> Self {
        Self { family, key }
    }

    /// Parses a prepared SVG text id, aggregating an optional `-line-N` suffix to its base label.
    pub fn from_svg_id(svg_id: &str) -> Option<Self> {
        let body = svg_id.strip_prefix(PREPARED_TEXT_LABEL_ID_PREFIX)?;
        let body = match body.rsplit_once("-line-") {
            Some((base, line)) => {
                parse_canonical_u32(line)?;
                base
            }
            None => body,
        };
        let (family, key) = body.rsplit_once('-')?;
        Some(Self {
            family: PreparedTextLabelFamily::from_id(family)?,
            key: parse_canonical_u32(key)?,
        })
    }

    /// Returns whether an SVG id belongs to the reserved prepared-text token namespace.
    #[doc(hidden)]
    pub fn is_svg_id_candidate(svg_id: &str) -> bool {
        svg_id.starts_with(PREPARED_TEXT_LABEL_ID_PREFIX)
    }

    /// Returns the stable family identifier embedded in the SVG token.
    pub const fn family(self) -> &'static str {
        self.family.id()
    }

    /// Returns the operation-local key assigned in fixed family/semantic order.
    pub const fn key(self) -> u32 {
        self.key
    }

    /// Returns the canonical SVG id for this prepared label.
    pub fn as_svg_id(self) -> String {
        self.to_string()
    }

    /// Returns a line-scoped SVG id that still parses back to this base label.
    pub fn as_svg_line_id(self, line: u32) -> String {
        format!("{self}-line-{line}")
    }
}

impl fmt::Display for PreparedTextLabelId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{PREPARED_TEXT_LABEL_ID_PREFIX}{}-{}",
            self.family.id(),
            self.key
        )
    }
}

fn parse_canonical_u32(value: &str) -> Option<u32> {
    if value.is_empty() || (value.len() > 1 && value.starts_with('0')) {
        return None;
    }
    value.parse().ok()
}

/// Errors raised while constructing an internally owned text projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum TextProjectionError {
    #[error("text projection source exceeds its bounded byte limit")]
    SourceLimitExceeded,
    #[error("text projection visible text exceeds its bounded byte limit")]
    VisibleLimitExceeded,
    #[error("text projection spans exceed their bounded retained-byte limit")]
    SpanLimitExceeded,
    #[error("text projection exceeded the operation work budget")]
    OperationWorkExceeded,
    #[error("text projection spans contain invalid or incomplete UTF-8 ranges")]
    InvalidRanges,
}

struct TextProjectionSpanBudget<'a> {
    maximum: usize,
    retained: usize,
    work_meter: Option<&'a OperationWorkMeter>,
}

impl<'a> TextProjectionSpanBudget<'a> {
    fn new(maximum: usize, work_meter: Option<&'a OperationWorkMeter>) -> Self {
        Self {
            maximum,
            retained: 0,
            work_meter,
        }
    }

    fn admit_existing(&mut self, span_count: usize) -> Result<(), TextProjectionError> {
        let retained = span_count
            .checked_mul(TEXT_PROJECTION_SPAN_RECORD_BYTES)
            .ok_or(TextProjectionError::SpanLimitExceeded)?;
        self.preflight(retained)?;
        self.retained = retained;
        Ok(())
    }

    fn push(
        &mut self,
        spans: &mut Vec<SourceVisibleSpan>,
        span: SourceVisibleSpan,
    ) -> Result<(), TextProjectionError> {
        let retained = self
            .retained
            .checked_add(TEXT_PROJECTION_SPAN_RECORD_BYTES)
            .ok_or(TextProjectionError::SpanLimitExceeded)?;
        self.preflight(retained)?;
        charge_projection_work(self.work_meter, 1)?;
        spans.push(span);
        self.retained = retained;
        Ok(())
    }

    fn preflight(&self, retained: usize) -> Result<(), TextProjectionError> {
        if retained > self.maximum {
            return Err(TextProjectionError::SpanLimitExceeded);
        }
        Ok(())
    }
}

/// Validated source text, visible text, and their ordered grapheme-level mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TextProjection {
    source: Arc<str>,
    visible: Arc<str>,
    spans: Arc<[SourceVisibleSpan]>,
    fingerprint: TextProjectionFingerprint,
}

impl TextProjection {
    pub(crate) fn new(
        source: impl Into<Arc<str>>,
        transform: ThemeTextTransform,
    ) -> Result<Self, TextProjectionError> {
        Self::build_with_options_and_budget(
            source.into(),
            transform,
            MAX_TEXT_PROJECTION_BYTES,
            true,
            MAX_TEXT_PROJECTION_SPAN_BYTES,
            None,
        )
    }

    pub(crate) fn new_family_normalized(
        source: impl Into<Arc<str>>,
        transform: ThemeTextTransform,
    ) -> Result<Self, TextProjectionError> {
        Self::build_with_options_and_budget(
            source.into(),
            transform,
            MAX_TEXT_PROJECTION_BYTES,
            false,
            MAX_TEXT_PROJECTION_SPAN_BYTES,
            None,
        )
    }

    fn build_with_limit(
        source: Arc<str>,
        transform: ThemeTextTransform,
        max_bytes: usize,
    ) -> Result<Self, TextProjectionError> {
        Self::build_with_options_and_budget(
            source,
            transform,
            max_bytes,
            true,
            MAX_TEXT_PROJECTION_SPAN_BYTES,
            None,
        )
    }

    fn build_with_options_and_budget(
        source: Arc<str>,
        transform: ThemeTextTransform,
        max_bytes: usize,
        recognize_html_breaks: bool,
        max_span_bytes: usize,
        work_meter: Option<&OperationWorkMeter>,
    ) -> Result<Self, TextProjectionError> {
        if source.len() > max_bytes {
            return Err(TextProjectionError::SourceLimitExceeded);
        }

        let mut visible = String::with_capacity(source.len());
        let mut spans = Vec::new();
        let mut span_budget = TextProjectionSpanBudget::new(max_span_bytes, work_meter);
        let mut source_cursor = 0;
        let mut capitalize_at_word_start = true;

        while source_cursor < source.len() {
            let (plain_end, break_end) =
                next_explicit_break(&source, source_cursor, recognize_html_breaks)
                    .unwrap_or((source.len(), source.len()));
            append_transformed_graphemes(
                &source,
                source_cursor..plain_end,
                transform,
                &mut capitalize_at_word_start,
                max_bytes,
                &mut visible,
                &mut spans,
                &mut span_budget,
            )?;
            if plain_end == source.len() {
                break;
            }

            let visible_start = visible.len();
            if visible_start >= max_bytes {
                return Err(TextProjectionError::VisibleLimitExceeded);
            }
            visible.push('\n');
            span_budget.push(
                &mut spans,
                SourceVisibleSpan::new(
                    TextByteRange::new(plain_end, break_end),
                    TextByteRange::new(visible_start, visible.len()),
                ),
            )?;
            capitalize_at_word_start = true;
            source_cursor = break_end;
        }

        Self::finish_parts(source, Arc::from(visible), spans, max_bytes, work_meter)
    }

    /// Admits an already composed family projection after validating both coordinate spaces.
    pub(crate) fn from_parts(
        source: Arc<str>,
        visible: Arc<str>,
        spans: Vec<SourceVisibleSpan>,
    ) -> Result<Self, TextProjectionError> {
        Self::from_parts_with_limits(
            source,
            visible,
            spans,
            MAX_TEXT_PROJECTION_BYTES,
            MAX_TEXT_PROJECTION_SPAN_BYTES,
            None,
        )
    }

    fn from_parts_with_limits(
        source: Arc<str>,
        visible: Arc<str>,
        spans: Vec<SourceVisibleSpan>,
        max_bytes: usize,
        max_span_bytes: usize,
        work_meter: Option<&OperationWorkMeter>,
    ) -> Result<Self, TextProjectionError> {
        let mut span_budget = TextProjectionSpanBudget::new(max_span_bytes, work_meter);
        span_budget.admit_existing(spans.len())?;
        Self::finish_parts(source, visible, spans, max_bytes, work_meter)
    }

    fn finish_parts(
        source: Arc<str>,
        visible: Arc<str>,
        spans: Vec<SourceVisibleSpan>,
        max_bytes: usize,
        work_meter: Option<&OperationWorkMeter>,
    ) -> Result<Self, TextProjectionError> {
        if source.len() > max_bytes {
            return Err(TextProjectionError::SourceLimitExceeded);
        }
        if visible.len() > max_bytes {
            return Err(TextProjectionError::VisibleLimitExceeded);
        }
        let validation_and_fingerprint_work = source
            .len()
            .checked_add(visible.len())
            .and_then(|work| {
                spans
                    .len()
                    .checked_mul(2)
                    .and_then(|spans| work.checked_add(spans))
            })
            .ok_or(TextProjectionError::SpanLimitExceeded)?;
        charge_projection_work(work_meter, validation_and_fingerprint_work)?;
        validate_projection_spans(&source, &visible, &spans)?;
        let fingerprint = fingerprint_text_projection(&source, &visible, &spans);
        Ok(Self {
            source,
            visible,
            spans: spans.into(),
            fingerprint,
        })
    }

    pub(crate) fn source(&self) -> &str {
        &self.source
    }

    pub(crate) fn visible(&self) -> &str {
        &self.visible
    }

    pub(crate) fn spans(&self) -> &[SourceVisibleSpan] {
        &self.spans
    }

    pub(crate) const fn fingerprint(&self) -> TextProjectionFingerprint {
        self.fingerprint
    }
}

fn charge_projection_work(
    work_meter: Option<&OperationWorkMeter>,
    units: usize,
) -> Result<(), TextProjectionError> {
    if let Some(work_meter) = work_meter {
        work_meter
            .charge(units)
            .map_err(|_| TextProjectionError::OperationWorkExceeded)?;
    }
    Ok(())
}

fn next_explicit_break(
    source: &str,
    start: usize,
    recognize_html_breaks: bool,
) -> Option<(usize, usize)> {
    let bytes = source.as_bytes();
    let mut cursor = start;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\n' => return Some((cursor, cursor + 1)),
            b'<' if recognize_html_breaks => {
                if let Some(end) = html_break_end(bytes, cursor) {
                    return Some((cursor, end));
                }
            }
            _ => {}
        }
        cursor += 1;
    }
    None
}

fn html_break_end(bytes: &[u8], start: usize) -> Option<usize> {
    if bytes.get(start) != Some(&b'<')
        || !matches!(bytes.get(start + 1), Some(b'b' | b'B'))
        || !matches!(bytes.get(start + 2), Some(b'r' | b'R'))
    {
        return None;
    }

    let mut cursor = start + 3;
    while matches!(bytes.get(cursor), Some(b' ' | b'\t' | b'\r' | b'\n')) {
        cursor += 1;
    }
    if bytes.get(cursor) == Some(&b'/') {
        cursor += 1;
    }
    (bytes.get(cursor) == Some(&b'>')).then_some(cursor + 1)
}

fn append_transformed_graphemes(
    source: &str,
    source_range: std::ops::Range<usize>,
    transform: ThemeTextTransform,
    capitalize_at_word_start: &mut bool,
    max_bytes: usize,
    visible: &mut String,
    spans: &mut Vec<SourceVisibleSpan>,
    span_budget: &mut TextProjectionSpanBudget,
) -> Result<(), TextProjectionError> {
    let source_start = source_range.start;
    for (offset, grapheme) in source[source_range].grapheme_indices(true) {
        let transformed = transform_grapheme(grapheme, transform, capitalize_at_word_start);
        let visible_start = visible.len();
        let visible_end = visible_start
            .checked_add(transformed.len())
            .filter(|end| *end <= max_bytes)
            .ok_or(TextProjectionError::VisibleLimitExceeded)?;
        visible.push_str(&transformed);
        let grapheme_start = source_start + offset;
        span_budget.push(
            spans,
            SourceVisibleSpan::new(
                TextByteRange::new(grapheme_start, grapheme_start + grapheme.len()),
                TextByteRange::new(visible_start, visible_end),
            ),
        )?;
    }
    Ok(())
}

fn transform_grapheme<'a>(
    grapheme: &'a str,
    transform: ThemeTextTransform,
    capitalize_at_word_start: &mut bool,
) -> Cow<'a, str> {
    match transform {
        ThemeTextTransform::None => Cow::Borrowed(grapheme),
        ThemeTextTransform::Uppercase => Cow::Owned(grapheme.to_uppercase()),
        ThemeTextTransform::Lowercase => Cow::Owned(grapheme.to_lowercase()),
        ThemeTextTransform::Capitalize => {
            let is_word_grapheme = grapheme.chars().any(char::is_alphanumeric);
            if !is_word_grapheme {
                *capitalize_at_word_start = true;
                Cow::Borrowed(grapheme)
            } else if std::mem::replace(capitalize_at_word_start, false) {
                Cow::Owned(grapheme.to_uppercase())
            } else {
                Cow::Borrowed(grapheme)
            }
        }
    }
}

fn validate_projection_spans(
    source: &str,
    visible: &str,
    spans: &[SourceVisibleSpan],
) -> Result<(), TextProjectionError> {
    let mut source_cursor = 0;
    let mut visible_cursor = 0;
    for span in spans {
        let source_range = span.source();
        let visible_range = span.visible();
        if source_range.start() != source_cursor
            || visible_range.start() != visible_cursor
            || source_range.start() > source_range.end()
            || visible_range.start() > visible_range.end()
            || source_range.end() > source.len()
            || visible_range.end() > visible.len()
            || !source.is_char_boundary(source_range.start())
            || !source.is_char_boundary(source_range.end())
            || !visible.is_char_boundary(visible_range.start())
            || !visible.is_char_boundary(visible_range.end())
            || (source_range.start() == source_range.end()
                && visible_range.start() == visible_range.end())
        {
            return Err(TextProjectionError::InvalidRanges);
        }
        source_cursor = source_range.end();
        visible_cursor = visible_range.end();
    }
    if source_cursor != source.len() || visible_cursor != visible.len() {
        return Err(TextProjectionError::InvalidRanges);
    }
    Ok(())
}

fn fingerprint_text_projection(
    source: &str,
    visible: &str,
    spans: &[SourceVisibleSpan],
) -> TextProjectionFingerprint {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(b"merman-text-projection");
    update_hash_field(&mut hasher, b"version");
    hasher.update(TEXT_PROJECTION_FINGERPRINT_VERSION.to_le_bytes());
    update_hash_field(&mut hasher, b"source");
    update_hash_field(&mut hasher, source.as_bytes());
    update_hash_field(&mut hasher, b"visible");
    update_hash_field(&mut hasher, visible.as_bytes());
    update_hash_field(&mut hasher, b"spans");
    hasher.update((spans.len() as u64).to_le_bytes());
    for span in spans {
        update_hash_field(&mut hasher, b"span");
        update_projection_range(&mut hasher, b"source-range", span.source());
        update_projection_range(&mut hasher, b"visible-range", span.visible());
    }
    TextProjectionFingerprint(hasher.finalize().into())
}

fn update_projection_range(hasher: &mut impl sha2::Digest, tag: &[u8], range: TextByteRange) {
    update_hash_field(hasher, tag);
    hasher.update((range.start() as u64).to_le_bytes());
    hasher.update((range.end() as u64).to_le_bytes());
}

/// Wrapping semantics requested for one prepared label.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum PreparedTextWrap {
    /// Preserve explicit line breaks and keep each source line as one shaping run.
    SingleRun,
    /// Apply Mermaid's SVG word wrapping semantics to each explicit source line.
    SvgLike {
        max_width_px: Option<f64>,
        break_long_words: bool,
    },
    /// Apply HTML-like collapsible whitespace wrapping.
    HtmlLike { max_width_px: Option<f64> },
}

impl Default for PreparedTextWrap {
    fn default() -> Self {
        Self::SvgLike {
            max_width_px: None,
            break_long_words: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrepareTextProjectionInput {
    MermaidSource,
    FamilyNormalized,
}

/// A bounded, structured request for preparing one visible label.
#[derive(Debug, Clone)]
pub struct PrepareTextRequest {
    text: Arc<str>,
    wrapping_typography: ThemeTextStyle,
    metrics_typography: ThemeTextStyle,
    direction: TextLayoutDirection,
    script: Option<Arc<str>>,
    language: Option<Arc<str>>,
    features: Arc<[Arc<str>]>,
    variations: Arc<[Arc<str>]>,
    wrap: PreparedTextWrap,
    projection_input: PrepareTextProjectionInput,
}

impl PrepareTextRequest {
    pub fn new(text: impl Into<String>, typography: ThemeTextStyle) -> Self {
        Self {
            text: Arc::from(text.into()),
            wrapping_typography: typography.clone(),
            metrics_typography: typography,
            direction: TextLayoutDirection::Auto,
            script: None,
            language: None,
            features: Arc::from([]),
            variations: Arc::from([]),
            wrap: PreparedTextWrap::default(),
            projection_input: PrepareTextProjectionInput::MermaidSource,
        }
    }

    pub fn with_direction(mut self, direction: TextLayoutDirection) -> Self {
        self.direction = direction;
        self
    }

    /// Sets the typography used for final bbox, advance, and line-height metrics.
    ///
    /// Flowchart edge labels wrap a temporary SVG text run before applying their final label
    /// style, so the wrapping and metrics styles are intentionally independent.
    pub fn with_metrics_typography(mut self, typography: ThemeTextStyle) -> Self {
        self.metrics_typography = typography;
        self
    }

    pub(crate) fn with_script(
        mut self,
        script: impl Into<String>,
    ) -> Result<Self, TextLayoutError> {
        let script = script.into();
        validate_tag(&script, "script")?;
        self.script = Some(Arc::from(script));
        Ok(self)
    }

    pub(crate) fn with_language(
        mut self,
        language: impl Into<String>,
    ) -> Result<Self, TextLayoutError> {
        let language = language.into();
        if language.trim().is_empty() || language.len() > 64 {
            return Err(TextLayoutError::InvalidRequest("language"));
        }
        self.language = Some(Arc::from(language));
        Ok(self)
    }

    pub(crate) fn with_features(
        mut self,
        features: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, TextLayoutError> {
        self.features = parse_features(features)?;
        Ok(self)
    }

    pub(crate) fn with_variations(
        mut self,
        variations: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, TextLayoutError> {
        self.variations = parse_variations(variations)?;
        Ok(self)
    }

    #[cfg(feature = "fuzzing")]
    #[doc(hidden)]
    pub fn for_fuzz_probe(
        text: impl Into<String>,
        typography: ThemeTextStyle,
        direction: TextLayoutDirection,
        features: impl IntoIterator<Item = impl Into<String>>,
        variations: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, String> {
        Self::new(text, typography)
            .with_direction(direction)
            .with_features(features)
            .and_then(|request| request.with_variations(variations))
            .map_err(|error| error.to_string())
    }

    pub fn with_wrap(mut self, wrap: PreparedTextWrap) -> Self {
        self.wrap = wrap;
        self
    }

    pub(crate) fn with_family_normalized_projection(mut self) -> Self {
        self.projection_input = PrepareTextProjectionInput::FamilyNormalized;
        self
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub const fn typography(&self) -> &ThemeTextStyle {
        self.wrapping_typography()
    }

    pub const fn wrapping_typography(&self) -> &ThemeTextStyle {
        &self.wrapping_typography
    }

    pub const fn metrics_typography(&self) -> &ThemeTextStyle {
        &self.metrics_typography
    }

    pub const fn direction(&self) -> TextLayoutDirection {
        self.direction
    }

    pub fn script(&self) -> Option<&str> {
        self.script.as_deref()
    }

    pub fn language(&self) -> Option<&str> {
        self.language.as_deref()
    }

    pub fn features(&self) -> impl ExactSizeIterator<Item = &str> + '_ {
        self.features.iter().map(Arc::as_ref)
    }

    pub fn variations(&self) -> impl ExactSizeIterator<Item = &str> + '_ {
        self.variations.iter().map(Arc::as_ref)
    }

    pub const fn wrap(&self) -> PreparedTextWrap {
        self.wrap
    }

    /// Returns a versioned digest over every field that can affect prepared label geometry.
    pub fn digest(&self) -> TextLayoutRequestDigest {
        self.digest_with_work_meter(None)
            .expect("unmetered request digests cannot exhaust operation work")
    }

    fn digest_with_work_meter(
        &self,
        work_meter: Option<&OperationWorkMeter>,
    ) -> Result<TextLayoutRequestDigest, TextLayoutError> {
        let mut digest = PreparedTextRequestDigestBuilder::new(work_meter);
        digest.update(b"merman-prepared-text-request")?;
        digest.update_field(b"version")?;
        digest.update(TEXT_REQUEST_DIGEST_VERSION.to_le_bytes())?;
        digest.update_field(b"text")?;
        digest.update_field(self.text.as_bytes())?;
        digest.update_field(b"projection-input")?;
        digest.update([projection_input_id(self.projection_input)])?;
        digest.update_field(b"wrapping-typography")?;
        hash_text_style(&mut digest, &self.wrapping_typography)?;
        digest.update_field(b"metrics-typography")?;
        hash_text_style(&mut digest, &self.metrics_typography)?;
        digest.update_field(b"direction")?;
        digest.update([direction_id(self.direction)])?;
        hash_optional_str(&mut digest, b"script", self.script.as_deref())?;
        hash_optional_str(&mut digest, b"language", self.language.as_deref())?;
        hash_string_sequence(&mut digest, b"features", &self.features)?;
        hash_string_sequence(&mut digest, b"variations", &self.variations)?;
        digest.update_field(b"wrap")?;
        hash_prepared_wrap(&mut digest, self.wrap)?;
        Ok(digest.finish())
    }
}

struct PreparedTextRequestDigestBuilder<'a> {
    hasher: sha2::Sha256,
    work_meter: Option<&'a OperationWorkMeter>,
}

impl<'a> PreparedTextRequestDigestBuilder<'a> {
    fn new(work_meter: Option<&'a OperationWorkMeter>) -> Self {
        use sha2::Digest;

        Self {
            hasher: sha2::Sha256::new(),
            work_meter,
        }
    }

    fn update(&mut self, value: impl AsRef<[u8]>) -> Result<(), TextLayoutError> {
        use sha2::Digest;

        let value = value.as_ref();
        charge_text_operation_work(self.work_meter, value.len())?;
        self.hasher.update(value);
        Ok(())
    }

    fn update_field(&mut self, value: &[u8]) -> Result<(), TextLayoutError> {
        self.update((value.len() as u64).to_le_bytes())?;
        self.update(value)
    }

    fn finish(self) -> TextLayoutRequestDigest {
        use sha2::Digest;

        TextLayoutRequestDigest(self.hasher.finalize().into())
    }
}

fn text_projection_for_request(
    request: &PrepareTextRequest,
) -> Result<TextProjection, TextLayoutError> {
    text_projection_for_request_with_meter(request, None)
}

fn text_projection_for_request_with_meter(
    request: &PrepareTextRequest,
    work_meter: Option<&OperationWorkMeter>,
) -> Result<TextProjection, TextLayoutError> {
    validate_prepare_text_request(request)?;
    charge_text_operation_work(work_meter, request.text().len())?;
    match request.projection_input {
        PrepareTextProjectionInput::MermaidSource => TextProjection::build_with_options_and_budget(
            Arc::from(request.text()),
            request.wrapping_typography().transform(),
            MAX_TEXT_PROJECTION_BYTES,
            true,
            MAX_TEXT_PROJECTION_SPAN_BYTES,
            work_meter,
        ),
        PrepareTextProjectionInput::FamilyNormalized => {
            TextProjection::build_with_options_and_budget(
                Arc::from(request.text()),
                request.wrapping_typography().transform(),
                MAX_TEXT_PROJECTION_BYTES,
                false,
                MAX_TEXT_PROJECTION_SPAN_BYTES,
                work_meter,
            )
        }
    }
    .map_err(text_projection_layout_error)
}

fn charge_text_operation_work(
    work_meter: Option<&OperationWorkMeter>,
    units: usize,
) -> Result<(), TextLayoutError> {
    if let Some(work_meter) = work_meter {
        work_meter
            .charge(units)
            .map_err(|_| TextLayoutError::LimitExceeded("operation_work"))?;
    }
    Ok(())
}

fn validate_prepare_text_request(request: &PrepareTextRequest) -> Result<(), TextLayoutError> {
    if request.text().len() > MAX_TEXT_PROJECTION_BYTES {
        return Err(TextLayoutError::LimitExceeded("text"));
    }
    for style in [request.wrapping_typography(), request.metrics_typography()] {
        let font_size = f64::from(style.font_size_px());
        let letter_spacing = f64::from(style.letter_spacing_px());
        let word_spacing = f64::from(style.word_spacing_px());
        if !font_size.is_finite()
            || font_size <= 0.0
            || font_size > MAX_PREPARED_TEXT_GEOMETRY_PX
            || !letter_spacing.is_finite()
            || letter_spacing.abs() > MAX_PREPARED_TEXT_GEOMETRY_PX
            || !word_spacing.is_finite()
            || word_spacing.abs() > MAX_PREPARED_TEXT_GEOMETRY_PX
        {
            return Err(TextLayoutError::InvalidRequest("typography"));
        }
    }
    let max_width_px = match request.wrap() {
        PreparedTextWrap::SingleRun => None,
        PreparedTextWrap::SvgLike { max_width_px, .. }
        | PreparedTextWrap::HtmlLike { max_width_px } => max_width_px,
    };
    if max_width_px.is_some_and(|width| {
        !width.is_finite() || width <= 0.0 || width > MAX_PREPARED_TEXT_GEOMETRY_PX
    }) {
        return Err(TextLayoutError::InvalidRequest("max_width_px"));
    }
    Ok(())
}

fn hash_text_style(
    digest: &mut PreparedTextRequestDigestBuilder<'_>,
    style: &ThemeTextStyle,
) -> Result<(), TextLayoutError> {
    digest.update_field(b"font-stack")?;
    digest.update((style.font_stack().families().len() as u64).to_le_bytes())?;
    for family in style.font_stack().families() {
        digest.update_field(family.as_bytes())?;
    }
    digest.update_field(b"font-size-px")?;
    digest.update(normalized_f32_bits(style.font_size_px()).to_le_bytes())?;
    digest.update_field(b"font-weight")?;
    digest.update(style.font_weight().to_le_bytes())?;
    digest.update_field(b"font-style")?;
    digest.update_field(style.font_style().id().as_bytes())?;
    digest.update_field(b"line-height")?;
    match style.line_height() {
        LineHeight::Normal => digest.update([0])?,
        LineHeight::Multiplier(value) => {
            digest.update([1])?;
            digest.update(normalized_f32_bits(value).to_le_bytes())?;
        }
        LineHeight::Px(value) => {
            digest.update([2])?;
            digest.update(normalized_f32_bits(value).to_le_bytes())?;
        }
    }
    digest.update_field(b"letter-spacing-px")?;
    digest.update(normalized_f32_bits(style.letter_spacing_px()).to_le_bytes())?;
    digest.update_field(b"word-spacing-px")?;
    digest.update(normalized_f32_bits(style.word_spacing_px()).to_le_bytes())?;
    digest.update_field(b"transform")?;
    digest.update([text_transform_id(style.transform())])?;
    digest.update_field(b"decoration")?;
    digest.update([text_decoration_id(style.decoration())])?;
    digest.update_field(b"text-align")?;
    digest.update([text_align_id(style.text_align())])?;
    digest.update_field(b"white-space")?;
    digest.update([white_space_id(style.white_space())])?;
    digest.update_field(b"theme-wrap")?;
    digest.update([theme_wrap_id(style.wrap())])?;
    Ok(())
}

fn hash_optional_str(
    digest: &mut PreparedTextRequestDigestBuilder<'_>,
    tag: &[u8],
    value: Option<&str>,
) -> Result<(), TextLayoutError> {
    digest.update_field(tag)?;
    match value {
        Some(value) => {
            digest.update([1])?;
            digest.update_field(value.as_bytes())?;
        }
        None => digest.update([0])?,
    }
    Ok(())
}

fn hash_string_sequence(
    digest: &mut PreparedTextRequestDigestBuilder<'_>,
    tag: &[u8],
    values: &[Arc<str>],
) -> Result<(), TextLayoutError> {
    digest.update_field(tag)?;
    digest.update((values.len() as u64).to_le_bytes())?;
    for value in values {
        digest.update_field(value.as_bytes())?;
    }
    Ok(())
}

fn hash_prepared_wrap(
    digest: &mut PreparedTextRequestDigestBuilder<'_>,
    wrap: PreparedTextWrap,
) -> Result<(), TextLayoutError> {
    match wrap {
        PreparedTextWrap::SingleRun => digest.update([0])?,
        PreparedTextWrap::SvgLike {
            max_width_px,
            break_long_words,
        } => {
            digest.update([1, u8::from(break_long_words)])?;
            hash_optional_f64(digest, max_width_px)?;
        }
        PreparedTextWrap::HtmlLike { max_width_px } => {
            digest.update([2])?;
            hash_optional_f64(digest, max_width_px)?;
        }
    }
    Ok(())
}

fn hash_optional_f64(
    digest: &mut PreparedTextRequestDigestBuilder<'_>,
    value: Option<f64>,
) -> Result<(), TextLayoutError> {
    match value {
        Some(value) => {
            digest.update([1])?;
            digest.update(normalized_f64_bits(value).to_le_bytes())?;
        }
        None => digest.update([0])?,
    }
    Ok(())
}

const fn normalized_f32_bits(value: f32) -> u32 {
    if value == 0.0 { 0 } else { value.to_bits() }
}

const fn normalized_f64_bits(value: f64) -> u64 {
    if value == 0.0 { 0 } else { value.to_bits() }
}

const fn projection_input_id(input: PrepareTextProjectionInput) -> u8 {
    match input {
        PrepareTextProjectionInput::MermaidSource => 0,
        PrepareTextProjectionInput::FamilyNormalized => 1,
    }
}

const fn direction_id(direction: TextLayoutDirection) -> u8 {
    match direction {
        TextLayoutDirection::Auto => 0,
        TextLayoutDirection::LeftToRight => 1,
        TextLayoutDirection::RightToLeft => 2,
    }
}

const fn text_transform_id(transform: ThemeTextTransform) -> u8 {
    match transform {
        ThemeTextTransform::None => 0,
        ThemeTextTransform::Uppercase => 1,
        ThemeTextTransform::Lowercase => 2,
        ThemeTextTransform::Capitalize => 3,
    }
}

const fn text_decoration_id(decoration: crate::diagram_theme::TextDecoration) -> u8 {
    match decoration {
        crate::diagram_theme::TextDecoration::None => 0,
        crate::diagram_theme::TextDecoration::Underline => 1,
        crate::diagram_theme::TextDecoration::Overline => 2,
        crate::diagram_theme::TextDecoration::LineThrough => 3,
    }
}

const fn text_align_id(align: crate::diagram_theme::TextAlign) -> u8 {
    match align {
        crate::diagram_theme::TextAlign::Start => 0,
        crate::diagram_theme::TextAlign::Center => 1,
        crate::diagram_theme::TextAlign::End => 2,
    }
}

const fn white_space_id(white_space: crate::diagram_theme::WhiteSpace) -> u8 {
    match white_space {
        crate::diagram_theme::WhiteSpace::Normal => 0,
        crate::diagram_theme::WhiteSpace::Pre => 1,
        crate::diagram_theme::WhiteSpace::NoWrap => 2,
        crate::diagram_theme::WhiteSpace::PreWrap => 3,
        crate::diagram_theme::WhiteSpace::PreLine => 4,
    }
}

const fn theme_wrap_id(wrap: crate::diagram_theme::ThemeWrapMode) -> u8 {
    match wrap {
        crate::diagram_theme::ThemeWrapMode::Normal => 0,
        crate::diagram_theme::ThemeWrapMode::BreakWord => 1,
        crate::diagram_theme::ThemeWrapMode::Anywhere => 2,
    }
}

/// Ink bounds for one prepared line relative to its alphabetic baseline.
///
/// Coordinates follow SVG's y-down axis: `top_px <= bottom_px`, with ordinary ascenders above
/// the baseline represented by a negative `top_px` and descenders below it represented by a
/// positive `bottom_px`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreparedTextVerticalExtents {
    top_px: f64,
    bottom_px: f64,
}

impl PreparedTextVerticalExtents {
    pub fn new(top_px: f64, bottom_px: f64) -> Result<Self, TextLayoutError> {
        if !top_px.is_finite()
            || !bottom_px.is_finite()
            || top_px < -MAX_PREPARED_TEXT_GEOMETRY_PX
            || top_px > MAX_PREPARED_TEXT_GEOMETRY_PX
            || bottom_px < -MAX_PREPARED_TEXT_GEOMETRY_PX
            || bottom_px > MAX_PREPARED_TEXT_GEOMETRY_PX
            || top_px > bottom_px
            || bottom_px - top_px > MAX_PREPARED_TEXT_GEOMETRY_PX
        {
            return Err(TextLayoutError::InvalidPreparedText);
        }
        Ok(Self { top_px, bottom_px })
    }

    pub const fn top_px(self) -> f64 {
        self.top_px
    }

    pub const fn bottom_px(self) -> f64 {
        self.bottom_px
    }

    pub const fn height_px(self) -> f64 {
        self.bottom_px - self.top_px
    }

    fn translated(self, offset_y_px: f64) -> Result<Self, TextLayoutError> {
        Self::new(self.top_px + offset_y_px, self.bottom_px + offset_y_px)
    }

    fn union(self, other: Self) -> Result<Self, TextLayoutError> {
        Self::new(
            self.top_px.min(other.top_px),
            self.bottom_px.max(other.bottom_px),
        )
    }
}

fn prepared_text_line_can_have_no_ink(text: &str) -> bool {
    text.chars()
        .all(|character| character.is_whitespace() || is_default_ignorable(character))
}

/// Prepared geometry for one visible output line.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PreparedTextLine {
    text: Arc<str>,
    visible_range: TextByteRange,
    computed_length_px: f64,
    bbox_x: (f64, f64),
    vertical_extents: PreparedTextVerticalExtents,
}

impl PreparedTextLine {
    pub(crate) fn new(
        text: impl Into<String>,
        visible_range: TextByteRange,
        computed_length_px: f64,
        bbox_x: (f64, f64),
        vertical_extents: PreparedTextVerticalExtents,
    ) -> Result<Self, TextLayoutError> {
        let text = text.into();
        if !computed_length_px.is_finite()
            || computed_length_px < 0.0
            || computed_length_px > MAX_PREPARED_TEXT_GEOMETRY_PX
            || !bbox_x.0.is_finite()
            || !bbox_x.1.is_finite()
            || bbox_x.0 < 0.0
            || bbox_x.1 < 0.0
            || bbox_x.0 > MAX_PREPARED_TEXT_GEOMETRY_PX
            || bbox_x.1 > MAX_PREPARED_TEXT_GEOMETRY_PX
            || visible_range.start() > visible_range.end()
            || (vertical_extents.height_px() == 0.0 && !prepared_text_line_can_have_no_ink(&text))
        {
            return Err(TextLayoutError::InvalidPreparedText);
        }
        Ok(Self {
            text: Arc::from(text),
            visible_range,
            computed_length_px,
            bbox_x,
            vertical_extents,
        })
    }

    fn from_response(response: &PreparedTextLineResponse) -> Result<Self, TextLayoutError> {
        if !response.computed_length_px.is_finite()
            || response.computed_length_px < 0.0
            || response.computed_length_px > MAX_PREPARED_TEXT_GEOMETRY_PX
            || !response.bbox_x.0.is_finite()
            || !response.bbox_x.1.is_finite()
            || response.bbox_x.0 < 0.0
            || response.bbox_x.1 < 0.0
            || response.bbox_x.0 > MAX_PREPARED_TEXT_GEOMETRY_PX
            || response.bbox_x.1 > MAX_PREPARED_TEXT_GEOMETRY_PX
            || response.visible_range.start() > response.visible_range.end()
            || (response.vertical_extents.height_px() == 0.0
                && !prepared_text_line_can_have_no_ink(&response.text))
        {
            return Err(TextLayoutError::InvalidPreparedText);
        }
        Ok(Self {
            text: Arc::clone(&response.text),
            visible_range: response.visible_range,
            computed_length_px: response.computed_length_px,
            bbox_x: response.bbox_x,
            vertical_extents: response.vertical_extents,
        })
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn visible_range(&self) -> std::ops::Range<usize> {
        self.visible_range.as_range()
    }

    pub const fn computed_length_px(&self) -> f64 {
        self.computed_length_px
    }

    pub const fn bbox_x(&self) -> (f64, f64) {
        self.bbox_x
    }

    pub fn bbox_width_px(&self) -> f64 {
        self.bbox_x.0 + self.bbox_x.1
    }

    pub const fn bbox_height_px(&self) -> f64 {
        self.vertical_extents.height_px()
    }

    pub(crate) const fn vertical_extents(&self) -> PreparedTextVerticalExtents {
        self.vertical_extents
    }
}

/// Raw line geometry returned by an external prepared-text backend.
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedTextLineResponse {
    text: Arc<str>,
    visible_range: TextByteRange,
    computed_length_px: f64,
    bbox_x: (f64, f64),
    vertical_extents: PreparedTextVerticalExtents,
}

impl PreparedTextLineResponse {
    pub fn new(
        text: impl Into<String>,
        visible_range: TextByteRange,
        computed_length_px: f64,
        bbox_x: (f64, f64),
        vertical_extents: PreparedTextVerticalExtents,
    ) -> Result<Self, TextLayoutError> {
        let text = text.into();
        if text.len() > MAX_TEXT_PROJECTION_BYTES
            || !computed_length_px.is_finite()
            || computed_length_px < 0.0
            || computed_length_px > MAX_PREPARED_TEXT_GEOMETRY_PX
            || !bbox_x.0.is_finite()
            || !bbox_x.1.is_finite()
            || bbox_x.0 < 0.0
            || bbox_x.1 < 0.0
            || bbox_x.0 > MAX_PREPARED_TEXT_GEOMETRY_PX
            || bbox_x.1 > MAX_PREPARED_TEXT_GEOMETRY_PX
            || visible_range.start() > visible_range.end()
            || (vertical_extents.height_px() == 0.0 && !prepared_text_line_can_have_no_ink(&text))
        {
            return Err(TextLayoutError::InvalidPreparedText);
        }
        Ok(Self {
            text: Arc::from(text),
            visible_range,
            computed_length_px,
            bbox_x,
            vertical_extents,
        })
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub const fn visible_range(&self) -> TextByteRange {
        self.visible_range
    }

    pub const fn computed_length_px(&self) -> f64 {
        self.computed_length_px
    }

    pub const fn bbox_x(&self) -> (f64, f64) {
        self.bbox_x
    }

    pub const fn bbox_height_px(&self) -> f64 {
        self.vertical_extents.height_px()
    }

    pub(crate) const fn vertical_extents(&self) -> PreparedTextVerticalExtents {
        self.vertical_extents
    }
}

/// Stable slot of one face in the catalog admitted for a prepared session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PreparedTextFaceSlot(u32);

impl PreparedTextFaceSlot {
    pub const fn new(index: u32) -> Self {
        Self(index)
    }

    pub const fn index(self) -> u32 {
        self.0
    }
}

/// Raw face and source claim for one contiguous visible-text range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedTextRunResponse {
    visible_range: TextByteRange,
    face_slot: PreparedTextFaceSlot,
    font_source: FontSource,
}

impl PreparedTextRunResponse {
    pub const fn new(
        visible_range: TextByteRange,
        face_slot: PreparedTextFaceSlot,
        font_source: FontSource,
    ) -> Self {
        Self {
            visible_range,
            face_slot,
            font_source,
        }
    }

    pub const fn visible_range(&self) -> TextByteRange {
        self.visible_range
    }

    pub const fn face_slot(&self) -> PreparedTextFaceSlot {
        self.face_slot
    }

    pub const fn font_source(&self) -> FontSource {
        self.font_source
    }
}

/// Stable catalog face key retained for one prepared-text run.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PreparedTextFaceKey {
    asset_fingerprint: FontAssetFingerprint,
    face_index: u32,
}

impl PreparedTextFaceKey {
    fn new(asset_fingerprint: FontAssetFingerprint, face_index: u32) -> Self {
        Self {
            asset_fingerprint,
            face_index,
        }
    }

    pub const fn asset_fingerprint(self) -> FontAssetFingerprint {
        self.asset_fingerprint
    }

    pub const fn face_index(self) -> u32 {
        self.face_index
    }
}

/// Provenance of the backend that produced one admitted prepared label.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PreparedTextLabelProvenance {
    Native,
    HostDependent,
}

impl PreparedTextLabelProvenance {
    pub const fn is_host_dependent(self) -> bool {
        matches!(self, Self::HostDependent)
    }
}

/// Face/source evidence retained for one contiguous prepared-text run.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedTextLabelEvidence {
    source_range: TextByteRange,
    visible_range: TextByteRange,
    face_key: PreparedTextFaceKey,
    font_source: FontSource,
}

impl PreparedTextLabelEvidence {
    fn new(
        source_range: TextByteRange,
        visible_range: TextByteRange,
        face_key: PreparedTextFaceKey,
        font_source: FontSource,
    ) -> Self {
        Self {
            source_range,
            visible_range,
            face_key,
            font_source,
        }
    }

    pub(crate) const fn source_range(&self) -> TextByteRange {
        self.source_range
    }

    pub(crate) const fn visible_range(&self) -> TextByteRange {
        self.visible_range
    }

    pub const fn face_key(&self) -> PreparedTextFaceKey {
        self.face_key
    }

    pub const fn font_source(&self) -> FontSource {
        self.font_source
    }
}

/// Frozen per-label evidence consumed by native SVG/export admission.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedTextLabelLedgerEntry {
    id: PreparedTextLabelId,
    catalog_fingerprint: FontCatalogFingerprint,
    request_digest: TextLayoutRequestDigest,
    provenance: PreparedTextLabelProvenance,
    projection_spans: Arc<[SourceVisibleSpan]>,
    line_ranges: Arc<[TextByteRange]>,
    line_texts: Arc<[Arc<str>]>,
    runs: Arc<[PreparedTextLabelEvidence]>,
}

impl PreparedTextLabelLedgerEntry {
    pub const fn id(&self) -> PreparedTextLabelId {
        self.id
    }

    pub const fn catalog_fingerprint(&self) -> FontCatalogFingerprint {
        self.catalog_fingerprint
    }

    pub(crate) const fn request_digest(&self) -> TextLayoutRequestDigest {
        self.request_digest
    }

    pub const fn provenance(&self) -> PreparedTextLabelProvenance {
        self.provenance
    }

    pub(crate) fn projection_spans(&self) -> &[SourceVisibleSpan] {
        &self.projection_spans
    }

    pub(crate) fn line_ranges(&self) -> &[TextByteRange] {
        &self.line_ranges
    }

    pub(crate) fn line_texts(&self) -> &[Arc<str>] {
        &self.line_texts
    }

    pub fn evidence(&self) -> &[PreparedTextLabelEvidence] {
        &self.runs
    }

    /// Returns the number of prepared output lines retained for native export verification.
    pub fn line_count(&self) -> usize {
        self.line_ranges.len()
    }

    /// Returns the admitted font sources actually used by this emitted label.
    pub fn used_font_sources(&self) -> impl Iterator<Item = FontSource> + '_ {
        self.runs.iter().map(PreparedTextLabelEvidence::font_source)
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        prepared_text_label_ledger_retained_bytes(
            self.projection_spans.len(),
            self.line_ranges.len(),
            self.line_texts.iter().map(|text| text.len()).sum(),
            self.runs.len(),
        )
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PendingPreparedTextLabelLedgerEntry {
    catalog_fingerprint: FontCatalogFingerprint,
    request_digest: TextLayoutRequestDigest,
    provenance: PreparedTextLabelProvenance,
    projection_spans: Arc<[SourceVisibleSpan]>,
    line_ranges: Arc<[TextByteRange]>,
    line_texts: Arc<[Arc<str>]>,
    runs: Arc<[PreparedTextLabelEvidence]>,
}

#[derive(Debug, Clone, Copy)]
struct PreparedTextLabelEvidenceContext {
    catalog_fingerprint: FontCatalogFingerprint,
    request_digest: TextLayoutRequestDigest,
    provenance: PreparedTextLabelProvenance,
}

impl PendingPreparedTextLabelLedgerEntry {
    pub(crate) fn retained_bytes(&self) -> usize {
        prepared_text_label_ledger_retained_bytes(
            self.projection_spans.len(),
            self.line_ranges.len(),
            self.line_texts.iter().map(|text| text.len()).sum(),
            self.runs.len(),
        )
    }

    pub(crate) fn bind(self, id: PreparedTextLabelId) -> PreparedTextLabelLedgerEntry {
        PreparedTextLabelLedgerEntry {
            id,
            catalog_fingerprint: self.catalog_fingerprint,
            request_digest: self.request_digest,
            provenance: self.provenance,
            projection_spans: self.projection_spans,
            line_ranges: self.line_ranges,
            line_texts: self.line_texts,
            runs: self.runs,
        }
    }
}

fn prepared_text_label_ledger_retained_bytes(
    projection_span_count: usize,
    line_range_count: usize,
    line_text_bytes: usize,
    run_count: usize,
) -> usize {
    let mut retained = ModeledRetainedBytes::new(PREPARED_TEXT_LEDGER_ENTRY_RECORD_BYTES);
    retained.add_records(projection_span_count, TEXT_PROJECTION_SPAN_RECORD_BYTES);
    retained.add_records(line_range_count, TEXT_BYTE_RANGE_RECORD_BYTES);
    retained.add_records(line_range_count, PREPARED_TEXT_LINE_TEXT_RECORD_BYTES);
    retained.add_bytes(line_text_bytes);
    retained.add_records(run_count, PREPARED_TEXT_LABEL_EVIDENCE_RECORD_BYTES);
    retained.finish()
}

/// Identity echoed by a prepared-text response so stale or cross-session results can be rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedTextCallBinding {
    catalog_fingerprint: FontCatalogFingerprint,
    contract_version: u32,
    backend: TextLayoutBackendIdentity,
    session_token: TextLayoutSessionToken,
    request_digest: TextLayoutRequestDigest,
}

impl PreparedTextCallBinding {
    pub fn new(
        catalog_fingerprint: FontCatalogFingerprint,
        contract_version: u32,
        backend: TextLayoutBackendIdentity,
        session_token: TextLayoutSessionToken,
        request_digest: TextLayoutRequestDigest,
    ) -> Result<Self, TextLayoutError> {
        session_token.validate()?;
        Ok(Self {
            catalog_fingerprint,
            contract_version,
            backend,
            session_token,
            request_digest,
        })
    }

    pub const fn catalog_fingerprint(&self) -> FontCatalogFingerprint {
        self.catalog_fingerprint
    }

    pub const fn contract_version(&self) -> u32 {
        self.contract_version
    }

    pub const fn backend(&self) -> &TextLayoutBackendIdentity {
        &self.backend
    }

    pub const fn session_token(&self) -> TextLayoutSessionToken {
        self.session_token
    }

    pub const fn request_digest(&self) -> TextLayoutRequestDigest {
        self.request_digest
    }
}

/// Merman-owned, normalized request delivered to one catalog-bound backend session.
#[derive(Debug, Clone)]
pub struct PreparedTextBackendRequest {
    binding: PreparedTextCallBinding,
    request: PrepareTextRequest,
    projection: TextProjection,
}

impl PreparedTextBackendRequest {
    fn new(
        binding: PreparedTextCallBinding,
        request: &PrepareTextRequest,
    ) -> Result<Self, TextLayoutError> {
        let projection = text_projection_for_request(request)?;
        Self::from_projection_with_digest(binding, request, projection, request.digest())
    }

    fn from_projection(
        binding: PreparedTextCallBinding,
        request: &PrepareTextRequest,
        projection: TextProjection,
    ) -> Result<Self, TextLayoutError> {
        Self::from_projection_with_digest(binding, request, projection, request.digest())
    }

    fn from_projection_with_digest(
        binding: PreparedTextCallBinding,
        request: &PrepareTextRequest,
        projection: TextProjection,
        request_digest: TextLayoutRequestDigest,
    ) -> Result<Self, TextLayoutError> {
        if binding.request_digest() != request_digest || projection.source() != request.text() {
            return Err(TextLayoutError::RequestDigestMismatch);
        }
        Ok(Self {
            binding,
            request: request.clone(),
            projection,
        })
    }

    pub const fn binding(&self) -> &PreparedTextCallBinding {
        &self.binding
    }

    pub const fn request_digest(&self) -> TextLayoutRequestDigest {
        self.binding.request_digest()
    }

    pub fn source_text(&self) -> &str {
        self.projection.source()
    }

    pub fn visible_text(&self) -> &str {
        self.projection.visible()
    }

    pub fn source_visible_spans(&self) -> &[SourceVisibleSpan] {
        self.projection.spans()
    }

    pub const fn wrapping_typography(&self) -> &ThemeTextStyle {
        self.request.wrapping_typography()
    }

    pub const fn metrics_typography(&self) -> &ThemeTextStyle {
        self.request.metrics_typography()
    }

    pub const fn direction(&self) -> TextLayoutDirection {
        self.request.direction()
    }

    pub fn script(&self) -> Option<&str> {
        self.request.script()
    }

    pub fn language(&self) -> Option<&str> {
        self.request.language()
    }

    pub fn features(&self) -> impl ExactSizeIterator<Item = &str> + '_ {
        self.request.features()
    }

    pub fn variations(&self) -> impl ExactSizeIterator<Item = &str> + '_ {
        self.request.variations()
    }

    pub const fn wrap(&self) -> PreparedTextWrap {
        self.request.wrap()
    }

    fn request(&self) -> &PrepareTextRequest {
        &self.request
    }

    fn projection(&self) -> &TextProjection {
        &self.projection
    }
}

/// Bounded host response awaiting admission against one request and prepared catalog session.
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedTextResponse {
    binding: PreparedTextCallBinding,
    lines: Arc<[PreparedTextLineResponse]>,
    runs: Arc<[PreparedTextRunResponse]>,
    line_height_px: f64,
    raw_width_px: Option<f64>,
    diagnostics: Arc<[Arc<str>]>,
    invalidated: bool,
}

impl PreparedTextResponse {
    pub fn new(
        binding: PreparedTextCallBinding,
        lines: impl IntoIterator<Item = PreparedTextLineResponse>,
        runs: impl IntoIterator<Item = PreparedTextRunResponse>,
        line_height_px: f64,
        raw_width_px: Option<f64>,
    ) -> Result<Self, TextLayoutError> {
        // External producers may already have allocated each yielded DTO. This constructor still
        // stops pulling immediately at the hard retained-byte boundary; request-relative and
        // operation-wide retained budgets remain a deferred C4b host-protocol concern.
        let mut builder = PreparedTextResponseBuilder::new(
            binding,
            line_height_px,
            raw_width_px,
            MAX_PREPARED_TEXT_RESPONSE_BYTES,
        )?;
        for line in lines {
            builder.push_line(line)?;
        }
        for run in runs {
            builder.push_run(run)?;
        }
        builder.finish()
    }

    pub fn invalidated(
        binding: PreparedTextCallBinding,
        diagnostics: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, TextLayoutError> {
        let mut builder =
            PreparedTextResponseBuilder::new(binding, 1.0, None, MAX_PREPARED_TEXT_RESPONSE_BYTES)?;
        builder.invalidated = true;
        for diagnostic in diagnostics {
            builder.push_diagnostic(diagnostic)?;
        }
        builder.finish()
    }

    pub fn with_diagnostics(
        mut self,
        diagnostics: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Self, TextLayoutError> {
        let retained = prepared_text_response_bytes(&self.lines, &self.runs, &[])
            .ok_or(TextLayoutError::LimitExceeded("response.bytes"))?;
        let mut budget = PreparedTextResponseRetainedBudget::with_retained(
            MAX_PREPARED_TEXT_RESPONSE_BYTES,
            retained,
        )?;
        let mut admitted = Vec::new();
        for diagnostic in diagnostics {
            push_prepared_text_diagnostic(&mut admitted, diagnostic, &mut budget)?;
        }
        self.diagnostics = admitted.into();
        Ok(self)
    }

    pub const fn binding(&self) -> &PreparedTextCallBinding {
        &self.binding
    }

    pub const fn request_digest(&self) -> TextLayoutRequestDigest {
        self.binding.request_digest()
    }

    pub const fn session_token(&self) -> TextLayoutSessionToken {
        self.binding.session_token()
    }

    pub fn lines(&self) -> &[PreparedTextLineResponse] {
        &self.lines
    }

    pub fn runs(&self) -> &[PreparedTextRunResponse] {
        &self.runs
    }

    pub const fn line_height_px(&self) -> f64 {
        self.line_height_px
    }

    pub const fn raw_width_px(&self) -> Option<f64> {
        self.raw_width_px
    }

    pub fn diagnostics(&self) -> impl ExactSizeIterator<Item = &str> + '_ {
        self.diagnostics.iter().map(Arc::as_ref)
    }

    pub const fn is_invalidated(&self) -> bool {
        self.invalidated
    }
}

struct PreparedTextResponseRetainedBudget {
    maximum: usize,
    retained: usize,
}

impl PreparedTextResponseRetainedBudget {
    fn new(maximum: usize) -> Self {
        Self {
            maximum,
            retained: 0,
        }
    }

    fn with_retained(maximum: usize, retained: usize) -> Result<Self, TextLayoutError> {
        if retained > maximum {
            return Err(TextLayoutError::LimitExceeded("response.bytes"));
        }
        Ok(Self { maximum, retained })
    }

    fn preflight(&self, additional: usize) -> Result<(), TextLayoutError> {
        let retained = self
            .retained
            .checked_add(additional)
            .ok_or(TextLayoutError::LimitExceeded("response.bytes"))?;
        if retained > self.maximum {
            return Err(TextLayoutError::LimitExceeded("response.bytes"));
        }
        Ok(())
    }

    fn reserve(&mut self, additional: usize) -> Result<(), TextLayoutError> {
        self.preflight(additional)?;
        self.retained = self
            .retained
            .checked_add(additional)
            .ok_or(TextLayoutError::LimitExceeded("response.bytes"))?;
        Ok(())
    }
}

struct PreparedTextResponseBuilder {
    binding: PreparedTextCallBinding,
    lines: Vec<PreparedTextLineResponse>,
    runs: Vec<PreparedTextRunResponse>,
    line_height_px: f64,
    raw_width_px: Option<f64>,
    diagnostics: Vec<Arc<str>>,
    invalidated: bool,
    budget: PreparedTextResponseRetainedBudget,
}

impl PreparedTextResponseBuilder {
    fn new(
        binding: PreparedTextCallBinding,
        line_height_px: f64,
        raw_width_px: Option<f64>,
        maximum_bytes: usize,
    ) -> Result<Self, TextLayoutError> {
        if !line_height_px.is_finite()
            || line_height_px <= 0.0
            || line_height_px > MAX_PREPARED_TEXT_GEOMETRY_PX
            || raw_width_px.is_some_and(|width| {
                !width.is_finite() || width < 0.0 || width > MAX_PREPARED_TEXT_GEOMETRY_PX
            })
        {
            return Err(TextLayoutError::InvalidPreparedText);
        }
        Ok(Self {
            binding,
            lines: Vec::new(),
            runs: Vec::new(),
            line_height_px,
            raw_width_px,
            diagnostics: Vec::new(),
            invalidated: false,
            budget: PreparedTextResponseRetainedBudget::new(maximum_bytes),
        })
    }

    fn reserve_line(&mut self, text_bytes: usize) -> Result<(), TextLayoutError> {
        if self.lines.len() == MAX_PREPARED_TEXT_LINES {
            return Err(TextLayoutError::LimitExceeded("lines"));
        }
        self.budget.reserve(
            PREPARED_TEXT_LINE_RECORD_BYTES
                .checked_add(text_bytes)
                .ok_or(TextLayoutError::LimitExceeded("response.bytes"))?,
        )
    }

    fn preflight_runs(&self, additional_runs: usize) -> Result<(), TextLayoutError> {
        if self.runs.len().saturating_add(additional_runs) > MAX_PREPARED_TEXT_RUNS {
            return Err(TextLayoutError::LimitExceeded("runs"));
        }
        self.budget.preflight(
            additional_runs
                .checked_mul(PREPARED_TEXT_RUN_RECORD_BYTES)
                .ok_or(TextLayoutError::LimitExceeded("response.bytes"))?,
        )
    }

    fn push_line(&mut self, line: PreparedTextLineResponse) -> Result<(), TextLayoutError> {
        self.reserve_line(line.text().len())?;
        self.push_reserved_line(line);
        Ok(())
    }

    fn push_reserved_line(&mut self, line: PreparedTextLineResponse) {
        self.lines.push(line);
    }

    fn push_run(&mut self, run: PreparedTextRunResponse) -> Result<(), TextLayoutError> {
        self.preflight_runs(1)?;
        self.budget.reserve(PREPARED_TEXT_RUN_RECORD_BYTES)?;
        self.runs.push(run);
        Ok(())
    }

    fn push_diagnostic(&mut self, diagnostic: impl Into<String>) -> Result<(), TextLayoutError> {
        push_prepared_text_diagnostic(&mut self.diagnostics, diagnostic, &mut self.budget)
    }

    fn finish(self) -> Result<PreparedTextResponse, TextLayoutError> {
        if !self.invalidated && self.lines.is_empty() {
            return Err(TextLayoutError::InvalidPreparedText);
        }
        Ok(PreparedTextResponse {
            binding: self.binding,
            lines: self.lines.into(),
            runs: self.runs.into(),
            line_height_px: self.line_height_px,
            raw_width_px: self.raw_width_px,
            diagnostics: self.diagnostics.into(),
            invalidated: self.invalidated,
        })
    }
}

fn push_prepared_text_diagnostic(
    admitted: &mut Vec<Arc<str>>,
    diagnostic: impl Into<String>,
    budget: &mut PreparedTextResponseRetainedBudget,
) -> Result<(), TextLayoutError> {
    if admitted.len() == MAX_PREPARED_TEXT_DIAGNOSTICS {
        return Err(TextLayoutError::LimitExceeded("diagnostics"));
    }
    let diagnostic = diagnostic.into();
    if diagnostic.len() > MAX_PREPARED_TEXT_DIAGNOSTIC_BYTES {
        return Err(TextLayoutError::LimitExceeded("diagnostics"));
    }
    budget.reserve(diagnostic.len())?;
    admitted.push(Arc::<str>::from(diagnostic));
    Ok(())
}

fn prepared_text_response_bytes(
    lines: &[PreparedTextLineResponse],
    runs: &[PreparedTextRunResponse],
    diagnostics: &[Arc<str>],
) -> Option<usize> {
    let line_text_bytes = lines
        .iter()
        .try_fold(0usize, |total, line| total.checked_add(line.text().len()))?;
    let diagnostic_bytes = diagnostics.iter().try_fold(0usize, |total, diagnostic| {
        total.checked_add(diagnostic.len())
    })?;
    line_text_bytes
        .checked_add(diagnostic_bytes)?
        .checked_add(lines.len().checked_mul(PREPARED_TEXT_LINE_RECORD_BYTES)?)?
        .checked_add(runs.len().checked_mul(PREPARED_TEXT_RUN_RECORD_BYTES)?)
}

fn prepared_text_request_response_budget(visible_bytes: usize) -> usize {
    visible_bytes
        .checked_mul(
            1usize
                .saturating_add(PREPARED_TEXT_LINE_RECORD_BYTES)
                .saturating_add(PREPARED_TEXT_RUN_RECORD_BYTES),
        )
        .and_then(|bytes| bytes.checked_add(PREPARED_TEXT_LINE_RECORD_BYTES))
        .and_then(|bytes| bytes.checked_add(4_096))
        .unwrap_or(MAX_PREPARED_TEXT_RESPONSE_BYTES)
        .min(MAX_PREPARED_TEXT_RESPONSE_BYTES)
}

/// Immutable output of one label preparation.
#[derive(Debug, Clone)]
pub(crate) struct PreparedText {
    projection: TextProjection,
    lines: Arc<[PreparedTextLine]>,
    runs: Arc<[PreparedTextLabelEvidence]>,
    label_ledger_entry: Option<PendingPreparedTextLabelLedgerEntry>,
    metrics: TextMetrics,
    raw_width_px: Option<f64>,
    line_height_px: f64,
    computed_length_px: f64,
    bbox_x: (f64, f64),
    bbox_width_px: f64,
    vertical_extents: PreparedTextVerticalExtents,
    bbox_height_px: f64,
    diagnostics: Arc<[Arc<str>]>,
}

/// Catalog-bound backend session that prepares fallible structured labels.
pub trait PreparedTextBackendSession: Send + Sync {
    fn prepare_text(
        &self,
        request: &PreparedTextBackendRequest,
    ) -> Result<PreparedTextResponse, TextLayoutError>;
}

impl PreparedText {
    pub(crate) fn new(
        projection: TextProjection,
        lines: impl IntoIterator<Item = PreparedTextLine>,
        line_height_px: f64,
        raw_width_px: Option<f64>,
    ) -> Result<Self, TextLayoutError> {
        Self::new_with_evidence(
            projection,
            lines,
            [],
            line_height_px,
            raw_width_px,
            Arc::from([]),
            None,
        )
    }

    fn new_with_evidence(
        projection: TextProjection,
        lines: impl IntoIterator<Item = PreparedTextLine>,
        runs: impl IntoIterator<Item = PreparedTextLabelEvidence>,
        line_height_px: f64,
        raw_width_px: Option<f64>,
        diagnostics: Arc<[Arc<str>]>,
        evidence_context: Option<PreparedTextLabelEvidenceContext>,
    ) -> Result<Self, TextLayoutError> {
        let lines = lines.into_iter().collect::<Vec<_>>();
        let runs = runs.into_iter().collect::<Vec<_>>();
        let budget = PreparedTextAdmissionBudget::new(None);
        validate_prepared_text_lines(&projection, &lines, &budget)?;
        let summary = summarize_prepared_text_lines(&lines, &budget)?;
        Self::new_with_validated_evidence(
            projection,
            lines,
            runs,
            summary,
            line_height_px,
            raw_width_px,
            diagnostics,
            evidence_context,
            &budget,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_with_validated_evidence(
        projection: TextProjection,
        lines: Vec<PreparedTextLine>,
        runs: Vec<PreparedTextLabelEvidence>,
        summary: PreparedTextLineSummary,
        line_height_px: f64,
        raw_width_px: Option<f64>,
        diagnostics: Arc<[Arc<str>]>,
        evidence_context: Option<PreparedTextLabelEvidenceContext>,
        budget: &PreparedTextAdmissionBudget<'_>,
    ) -> Result<Self, TextLayoutError> {
        if lines.is_empty()
            || lines.len() > projection.visible().len().saturating_add(1)
            || runs.len() > projection.visible().len().saturating_add(1)
            || !line_height_px.is_finite()
            || line_height_px <= 0.0
            || line_height_px > MAX_PREPARED_TEXT_GEOMETRY_PX
            || raw_width_px.is_some_and(|width| {
                !width.is_finite() || width < 0.0 || width > MAX_PREPARED_TEXT_GEOMETRY_PX
            })
        {
            return Err(TextLayoutError::InvalidPreparedText);
        }
        let line_count = lines.len();
        let vertical_extents = summarize_prepared_text_vertical_extents(&lines, line_height_px)?;
        let metrics = TextMetrics {
            width: summary.computed_length_px.max(summary.bbox_width_px),
            height: line_height_px * line_count as f64,
            line_count,
        };
        let bbox_height_px = vertical_extents.height_px();
        if !metrics.height.is_finite()
            || metrics.height > MAX_PREPARED_TEXT_GEOMETRY_PX
            || !bbox_height_px.is_finite()
            || bbox_height_px > MAX_PREPARED_TEXT_GEOMETRY_PX
        {
            return Err(TextLayoutError::InvalidPreparedText);
        }
        budget.charge(lines.len().saturating_add(runs.len()))?;
        let lines: Arc<[PreparedTextLine]> = lines.into();
        let runs: Arc<[PreparedTextLabelEvidence]> = runs.into();
        if evidence_context.is_some() {
            budget.charge(lines.len())?;
        }
        let label_ledger_entry =
            evidence_context.map(|context| PendingPreparedTextLabelLedgerEntry {
                catalog_fingerprint: context.catalog_fingerprint,
                request_digest: context.request_digest,
                provenance: context.provenance,
                projection_spans: Arc::clone(&projection.spans),
                line_ranges: lines
                    .iter()
                    .map(|line| line.visible_range)
                    .collect::<Vec<_>>()
                    .into(),
                line_texts: lines
                    .iter()
                    .map(|line| Arc::clone(&line.text))
                    .collect::<Vec<_>>()
                    .into(),
                runs: Arc::clone(&runs),
            });
        Ok(Self {
            projection,
            lines,
            runs,
            label_ledger_entry,
            metrics,
            raw_width_px,
            line_height_px,
            computed_length_px: summary.computed_length_px,
            bbox_x: summary.bbox_x,
            bbox_width_px: summary.bbox_width_px,
            vertical_extents,
            bbox_height_px,
            diagnostics,
        })
    }

    pub fn source_text(&self) -> &str {
        self.projection.source()
    }

    pub fn visible_text(&self) -> &str {
        self.projection.visible()
    }

    pub fn lines(&self) -> &[PreparedTextLine] {
        &self.lines
    }

    pub(crate) fn run_evidence(&self) -> &[PreparedTextLabelEvidence] {
        &self.runs
    }

    pub(crate) fn label_ledger_entry(&self) -> Option<PendingPreparedTextLabelLedgerEntry> {
        self.label_ledger_entry.clone()
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        let mut retained = ModeledRetainedBytes::new(PREPARED_TEXT_OWNER_RECORD_BYTES);

        retained.add_bytes(self.projection.source.len());
        retained.add_bytes(self.projection.visible.len());
        retained.add_records(
            self.projection.spans.len(),
            TEXT_PROJECTION_SPAN_RECORD_BYTES,
        );

        retained.add_records(self.lines.len(), PREPARED_TEXT_LINE_RECORD_BYTES);
        for line in self.lines.iter() {
            retained.add_bytes(line.text.len());
        }

        retained.add_records(self.runs.len(), PREPARED_TEXT_LABEL_EVIDENCE_RECORD_BYTES);

        retained.add_records(
            self.diagnostics.len(),
            PREPARED_TEXT_DIAGNOSTIC_RECORD_BYTES,
        );
        for diagnostic in self.diagnostics.iter() {
            retained.add_bytes(diagnostic.len());
        }

        // The pending ledger shares projection spans, line text, and run evidence with this owner.
        // Only its independently allocated line-range and line-text-reference slices are charged
        // here.
        if let Some(entry) = &self.label_ledger_entry {
            retained.add_records(entry.line_ranges.len(), TEXT_BYTE_RANGE_RECORD_BYTES);
            retained.add_records(entry.line_texts.len(), PREPARED_TEXT_LINE_TEXT_RECORD_BYTES);
        }

        retained.finish()
    }

    pub fn wrapped_lines(&self) -> impl ExactSizeIterator<Item = &str> + '_ {
        self.lines.iter().map(PreparedTextLine::text)
    }

    pub const fn metrics(&self) -> TextMetrics {
        self.metrics
    }

    pub const fn raw_width_px(&self) -> Option<f64> {
        self.raw_width_px
    }

    pub(crate) const fn line_height_px(&self) -> f64 {
        self.line_height_px
    }

    pub const fn computed_length_px(&self) -> f64 {
        self.computed_length_px
    }

    pub const fn bbox_x(&self) -> (f64, f64) {
        self.bbox_x
    }

    pub const fn bbox_width_px(&self) -> f64 {
        self.bbox_width_px
    }

    pub const fn bbox_height_px(&self) -> f64 {
        self.bbox_height_px
    }

    pub(crate) const fn vertical_extents(&self) -> PreparedTextVerticalExtents {
        self.vertical_extents
    }

    pub fn diagnostics(&self) -> impl ExactSizeIterator<Item = &str> + '_ {
        self.diagnostics.iter().map(Arc::as_ref)
    }
}

/// Geometry summary exposed only to the dedicated text-layout fuzz harness.
#[cfg(feature = "fuzzing")]
#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct PreparedTextFuzzProbe {
    metrics: TextMetrics,
    raw_width_px: Option<f64>,
    bbox_width_px: f64,
    bbox_height_px: f64,
    vertical_extents: PreparedTextVerticalExtents,
}

#[cfg(feature = "fuzzing")]
impl PreparedTextFuzzProbe {
    pub const fn metrics(self) -> TextMetrics {
        self.metrics
    }

    pub const fn raw_width_px(self) -> Option<f64> {
        self.raw_width_px
    }

    pub const fn bbox_width_px(self) -> f64 {
        self.bbox_width_px
    }

    pub const fn bbox_height_px(self) -> f64 {
        self.bbox_height_px
    }

    pub const fn vertical_extents_px(self) -> (f64, f64) {
        (
            self.vertical_extents.top_px(),
            self.vertical_extents.bottom_px(),
        )
    }
}

fn classify_prepared_text_fuzz_probe<T>(
    result: Result<T, TextLayoutError>,
) -> Result<Option<T>, String> {
    match result {
        Ok(probe) => Ok(Some(probe)),
        Err(TextLayoutError::GlyphUnavailable) => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

#[cfg(feature = "fuzzing")]
impl NativeTextLayoutBackend {
    /// Runs the fuzz-only probe while treating a missing catalog glyph as an expected input case.
    #[doc(hidden)]
    pub fn prepare_text_probe_for_fuzz(
        &self,
        catalog_request: &PrepareCatalogRequest,
        text_request: &PrepareTextRequest,
    ) -> Result<Option<PreparedTextFuzzProbe>, String> {
        classify_prepared_text_fuzz_probe(self.prepare_text_probe(catalog_request, text_request))
    }
}

#[derive(Clone, Copy)]
struct PreparedTextLineSummary {
    computed_length_px: f64,
    bbox_x: (f64, f64),
    bbox_width_px: f64,
}

struct PreparedTextAdmissionBudget<'a> {
    work_meter: Option<&'a OperationWorkMeter>,
}

impl<'a> PreparedTextAdmissionBudget<'a> {
    const fn new(work_meter: Option<&'a OperationWorkMeter>) -> Self {
        Self { work_meter }
    }

    fn charge(&self, units: usize) -> Result<(), TextLayoutError> {
        charge_text_operation_work(self.work_meter, units)
    }
}

fn summarize_prepared_text_lines(
    lines: &[PreparedTextLine],
    budget: &PreparedTextAdmissionBudget<'_>,
) -> Result<PreparedTextLineSummary, TextLayoutError> {
    budget.charge(lines.len())?;
    let mut summary = PreparedTextLineSummary {
        computed_length_px: 0.0,
        bbox_x: (0.0, 0.0),
        bbox_width_px: 0.0,
    };
    for line in lines {
        summary.computed_length_px = summary.computed_length_px.max(line.computed_length_px());
        let line_bbox_x = line.bbox_x();
        summary.bbox_x.0 = summary.bbox_x.0.max(line_bbox_x.0);
        summary.bbox_x.1 = summary.bbox_x.1.max(line_bbox_x.1);
        summary.bbox_width_px = summary.bbox_width_px.max(line.bbox_width_px());
    }
    Ok(summary)
}

fn summarize_prepared_text_vertical_extents(
    lines: &[PreparedTextLine],
    line_height_px: f64,
) -> Result<PreparedTextVerticalExtents, TextLayoutError> {
    let mut lines = lines.iter().enumerate();
    let Some((first_index, first)) = lines.next() else {
        return Err(TextLayoutError::InvalidPreparedText);
    };
    let first_offset = line_height_px * first_index as f64;
    let mut extents = first.vertical_extents().translated(first_offset)?;
    for (line_index, line) in lines {
        let offset = line_height_px * line_index as f64;
        extents = extents.union(line.vertical_extents().translated(offset)?)?;
    }
    Ok(extents)
}

struct OrderedProjectionRangeCursor<'a> {
    projection: &'a TextProjection,
    next_span: usize,
    previous_end: usize,
}

impl<'a> OrderedProjectionRangeCursor<'a> {
    const fn new(projection: &'a TextProjection) -> Self {
        Self {
            projection,
            next_span: 0,
            previous_end: 0,
        }
    }

    fn admit(
        &mut self,
        range: TextByteRange,
        budget: &PreparedTextAdmissionBudget<'_>,
        invalid: TextLayoutError,
    ) -> Result<std::ops::Range<usize>, TextLayoutError> {
        budget.charge(1)?;
        let start = range.start();
        let end = range.end();
        if start > end
            || start < self.previous_end
            || end > self.projection.visible().len()
            || !self.projection.visible().is_char_boundary(start)
            || !self.projection.visible().is_char_boundary(end)
        {
            return Err(invalid);
        }

        let spans = self.projection.spans();
        while let Some(span) = spans.get(self.next_span) {
            budget.charge(1)?;
            if span.visible().end() <= start {
                self.next_span = self.next_span.saturating_add(1);
                continue;
            }
            break;
        }

        let first = self.next_span;
        let mut atom_cursor = start;
        while let Some(span) = spans.get(self.next_span) {
            budget.charge(1)?;
            let visible = span.visible();
            if visible.start() >= end {
                break;
            }
            if visible.start() < start
                || visible.end() > end
                || visible.start() != atom_cursor
                || visible.end() < visible.start()
            {
                return Err(invalid);
            }
            atom_cursor = visible.end();
            self.next_span = self.next_span.saturating_add(1);
        }
        if atom_cursor != end {
            return Err(invalid);
        }
        self.previous_end = end;
        Ok(first..self.next_span)
    }
}

fn validate_prepared_text_lines(
    projection: &TextProjection,
    lines: &[PreparedTextLine],
    budget: &PreparedTextAdmissionBudget<'_>,
) -> Result<(), TextLayoutError> {
    budget.charge(lines.len())?;
    let mut cursor = OrderedProjectionRangeCursor::new(projection);
    for line in lines {
        let range = line.visible_range;
        cursor.admit(range, budget, TextLayoutError::InvalidPreparedText)?;
        budget.charge(line.text().len())?;
        if projection.visible().get(range.as_range()) != Some(line.text()) {
            return Err(TextLayoutError::InvalidPreparedText);
        }
    }
    Ok(())
}

fn validate_prepared_text_response_budget(
    projection: &TextProjection,
    response: &PreparedTextResponse,
    budget: &PreparedTextAdmissionBudget<'_>,
) -> Result<PreparedTextLineSummary, TextLayoutError> {
    let visible_bytes = projection.visible().len();
    if response.lines.len() > visible_bytes.saturating_add(1) || response.runs.len() > visible_bytes
    {
        return Err(TextLayoutError::LimitExceeded("response.records"));
    }
    budget.charge(response.lines.len())?;
    let mut line_text_bytes = 0usize;
    let mut computed_length_px = 0.0_f64;
    let mut bbox_x = (0.0_f64, 0.0_f64);
    let mut bbox_width_px = 0.0_f64;
    let mut vertical_extents: Option<PreparedTextVerticalExtents> = None;
    for (line_index, line) in response.lines.iter().enumerate() {
        line_text_bytes = line_text_bytes
            .checked_add(line.text().len())
            .ok_or(TextLayoutError::LimitExceeded("response.text_bytes"))?;
        computed_length_px = computed_length_px.max(line.computed_length_px());
        let line_bbox_x = line.bbox_x();
        bbox_x.0 = bbox_x.0.max(line_bbox_x.0);
        bbox_x.1 = bbox_x.1.max(line_bbox_x.1);
        bbox_width_px = bbox_width_px.max(line_bbox_x.0 + line_bbox_x.1);
        let line_offset = response.line_height_px * line_index as f64;
        let line_extents = line.vertical_extents().translated(line_offset)?;
        vertical_extents = Some(match vertical_extents {
            Some(current) => current.union(line_extents)?,
            None => line_extents,
        });
    }
    if line_text_bytes > visible_bytes {
        return Err(TextLayoutError::LimitExceeded("response.text_bytes"));
    }
    budget.charge(response.diagnostics.len())?;
    let diagnostic_bytes = response
        .diagnostics
        .iter()
        .try_fold(0usize, |total, diagnostic| {
            total.checked_add(diagnostic.len())
        })
        .ok_or(TextLayoutError::LimitExceeded("response.bytes"))?;
    let estimated = line_text_bytes
        .checked_add(diagnostic_bytes)
        .and_then(|bytes| {
            response
                .lines
                .len()
                .checked_mul(PREPARED_TEXT_LINE_RECORD_BYTES)
                .and_then(|records| bytes.checked_add(records))
        })
        .and_then(|bytes| {
            response
                .runs
                .len()
                .checked_mul(PREPARED_TEXT_RUN_RECORD_BYTES)
                .and_then(|records| bytes.checked_add(records))
        })
        .ok_or(TextLayoutError::LimitExceeded("response.bytes"))?;
    let request_budget = prepared_text_request_response_budget(visible_bytes);
    if estimated > request_budget {
        return Err(TextLayoutError::LimitExceeded("response.bytes"));
    }

    let line_count = response.lines.len() as f64;
    let total_height = response.line_height_px * line_count;
    let bbox_height = vertical_extents
        .ok_or(TextLayoutError::InvalidPreparedText)?
        .height_px();
    if !total_height.is_finite()
        || total_height > MAX_PREPARED_TEXT_GEOMETRY_PX
        || !bbox_height.is_finite()
        || bbox_height > MAX_PREPARED_TEXT_GEOMETRY_PX
    {
        return Err(TextLayoutError::InvalidPreparedText);
    }
    Ok(PreparedTextLineSummary {
        computed_length_px,
        bbox_x,
        bbox_width_px,
    })
}

fn validate_prepared_text_coverage(
    projection: &TextProjection,
    lines: &[PreparedTextLine],
    wrap: PreparedTextWrap,
    white_space: WhiteSpace,
    raw_width_px: Option<f64>,
    budget: &PreparedTextAdmissionBudget<'_>,
) -> Result<(), TextLayoutError> {
    let expects_raw_width = matches!(wrap, PreparedTextWrap::HtmlLike { .. });
    if expects_raw_width != raw_width_px.is_some() {
        return Err(TextLayoutError::InvalidPreparedText);
    }

    let exact_explicit_lines = matches!(
        wrap,
        PreparedTextWrap::SingleRun
            | PreparedTextWrap::SvgLike {
                max_width_px: None,
                ..
            }
            | PreparedTextWrap::HtmlLike { max_width_px: None }
    );
    if exact_explicit_lines {
        budget.charge(projection.visible().len())?;
        budget.charge(lines.len())?;
        let mut expected = projected_visible_lines(projection.visible());
        for actual in lines {
            if expected
                .next()
                .is_none_or(|expected| expected.visible_range() != actual.visible_range)
            {
                return Err(TextLayoutError::InvalidPreparedText);
            }
        }
        if expected.next().is_some() {
            return Err(TextLayoutError::InvalidPreparedText);
        }
        return Ok(());
    }

    budget.charge(lines.len())?;
    let mut previous_end = 0;
    for line in lines {
        budget.charge(line.text().len())?;
        if line.text().contains('\n') {
            return Err(TextLayoutError::InvalidPreparedText);
        }
        let gap = projection
            .visible()
            .get(previous_end..line.visible_range.start())
            .ok_or(TextLayoutError::InvalidPreparedText)?;
        budget.charge(gap.len())?;
        if !omittable_wrapped_gap(gap, white_space) {
            return Err(TextLayoutError::InvalidPreparedText);
        }
        previous_end = line.visible_range.end();
    }
    let suffix = projection
        .visible()
        .get(previous_end..)
        .ok_or(TextLayoutError::InvalidPreparedText)?;
    budget.charge(suffix.len())?;
    if !omittable_wrapped_gap(suffix, white_space) {
        return Err(TextLayoutError::InvalidPreparedText);
    }
    Ok(())
}

fn omittable_wrapped_gap(gap: &str, white_space: WhiteSpace) -> bool {
    if gap.is_empty() {
        return true;
    }

    // A wrapped line may omit only collapsible ASCII spacing. Non-breaking and line-separator
    // characters carry visible semantics and must remain covered by an admitted line range.
    if !matches!(white_space, WhiteSpace::Normal | WhiteSpace::PreLine)
        || !gap
            .chars()
            .all(|character| matches!(character, ' ' | '\t' | '\r' | '\u{000b}' | '\u{000c}'))
    {
        return false;
    }
    true
}

/// Direction requested for a prepared shaping session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
pub enum TextLayoutDirection {
    #[default]
    Auto,
    LeftToRight,
    RightToLeft,
}

impl TextLayoutDirection {
    fn to_rustybuzz(self) -> Option<BuzzDirection> {
        match self {
            Self::Auto => None,
            Self::LeftToRight => Some(BuzzDirection::LeftToRight),
            Self::RightToLeft => Some(BuzzDirection::RightToLeft),
        }
    }
}

/// Backend identity included in every prepared session attestation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TextLayoutBackendIdentity {
    name: Arc<str>,
    version: Arc<str>,
}

impl TextLayoutBackendIdentity {
    pub(crate) fn new(
        name: impl Into<String>,
        version: impl Into<String>,
    ) -> Result<Self, TextLayoutError> {
        let name = name.into();
        let version = version.into();
        if name.trim().is_empty() || version.trim().is_empty() {
            return Err(TextLayoutError::InvalidBackendIdentity);
        }
        Ok(Self {
            name: Arc::from(name.trim()),
            version: Arc::from(version.trim()),
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn version(&self) -> &str {
        &self.version
    }
}

/// Capabilities that a prepared backend actually attests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextLayoutCapabilities {
    pub supports_catalog_faces: bool,
    pub supports_cluster_fallback: bool,
    pub supports_features: bool,
    pub supports_variations: bool,
    pub supports_direction: bool,
    pub supports_script: bool,
    pub supports_language: bool,
}

impl TextLayoutCapabilities {
    pub const fn native() -> Self {
        Self {
            supports_catalog_faces: true,
            supports_cluster_fallback: true,
            supports_features: true,
            supports_variations: true,
            supports_direction: true,
            supports_script: true,
            supports_language: true,
        }
    }
}

/// Bounded evidence for the faces loaded by a backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextLayoutFaceEvidence {
    faces: Arc<[FontFaceMetadata]>,
}

impl TextLayoutFaceEvidence {
    /// Creates an explicit list of faces loaded by a backend.
    ///
    /// The list is deliberately supplied by the backend instead of being synthesized from the
    /// request.  The render environment compares it with the requested catalog before admitting
    /// the prepared session, so a host cannot accidentally claim that an unloaded face is usable.
    pub fn new(faces: impl IntoIterator<Item = FontFaceMetadata>) -> Result<Self, TextLayoutError> {
        let mut bounded_faces: Vec<FontFaceMetadata> = Vec::new();
        for face in faces {
            if bounded_faces.len() == MAX_FONT_FACES_HARD_CAP {
                return Err(TextLayoutError::LimitExceeded("face_evidence"));
            }
            bounded_faces.push(face);
        }
        if bounded_faces.is_empty() {
            return Err(TextLayoutError::NoUsableFace);
        }
        for (index, face) in bounded_faces.iter().enumerate() {
            if bounded_faces[..index].iter().any(|previous| {
                previous.asset_id() == face.asset_id() && previous.face_index() == face.face_index()
            }) {
                return Err(TextLayoutError::InvalidRequest("face_evidence"));
            }
        }
        Ok(Self {
            faces: Arc::from(bounded_faces),
        })
    }

    pub fn faces(&self) -> &[FontFaceMetadata] {
        &self.faces
    }
}

/// A request to prepare one immutable catalog-backed shaping session.
#[derive(Debug, Clone)]
pub struct PrepareCatalogRequest {
    catalog: FontCatalog,
    catalog_fingerprint: FontCatalogFingerprint,
    contract_version: u32,
    font_source_policy: FontSourcePolicy,
}

impl PrepareCatalogRequest {
    pub fn new(catalog: FontCatalog, font_source_policy: FontSourcePolicy) -> Self {
        let catalog_fingerprint = catalog.fingerprint();
        Self {
            catalog,
            catalog_fingerprint,
            contract_version: TEXT_LAYOUT_CONTRACT_VERSION,
            font_source_policy,
        }
    }

    pub fn with_contract_version(mut self, version: u32) -> Self {
        self.contract_version = version;
        self
    }

    pub fn catalog(&self) -> &FontCatalog {
        &self.catalog
    }

    pub const fn catalog_fingerprint(&self) -> FontCatalogFingerprint {
        self.catalog_fingerprint
    }

    pub const fn contract_version(&self) -> u32 {
        self.contract_version
    }

    pub const fn font_source_policy(&self) -> &FontSourcePolicy {
        &self.font_source_policy
    }
}

/// A reusable, fingerprint-bound layout result.
#[derive(Clone)]
pub(crate) struct PreparedTextLayout {
    catalog: FontCatalog,
    catalog_fingerprint: FontCatalogFingerprint,
    contract_version: u32,
    candidates: Arc<[PreparedTextLayoutCandidate]>,
    usage: Arc<Mutex<PreparedTextLayoutUsage>>,
}

#[derive(Clone)]
struct PreparedTextLayoutCandidate {
    backend: TextLayoutBackendIdentity,
    capabilities: TextLayoutCapabilities,
    font_source: FontSource,
    face_evidence: TextLayoutFaceEvidence,
    face_keys: Arc<[PreparedTextFaceKey]>,
    session_token: TextLayoutSessionToken,
    session: Arc<dyn PreparedTextBackendSession>,
    native_session: Option<Arc<NativeCatalogTextMeasurer>>,
    fallback: Option<HostMeasurementFallback>,
    host_dependent: bool,
    evidence_provenance: PreparedTextLabelProvenance,
    relaxed_capabilities: bool,
    disabled: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Copy, Default)]
struct PreparedTextCandidateUsage {
    successes: u64,
    failures: u64,
    fallback_selections: u64,
}

#[derive(Debug, Default)]
struct PreparedTextLayoutUsage {
    candidates: Vec<PreparedTextCandidateUsage>,
    catalog_failure_count: u64,
}

/// Internal builder that admits an ordered set of immutable layout candidates.
pub(crate) struct PreparedTextLayoutBuilder {
    request: PrepareCatalogRequest,
    candidates: Vec<PreparedTextLayoutCandidate>,
    catalog_failure_count: u64,
}

/// Backend response awaiting admission against the operation's prepare request.
///
/// Backends explicitly return their attestation and opaque prepared-text session. The render
/// environment remains the only owner that can bind those claims to the retained catalog and
/// construct a [`PreparedTextLayout`].
#[derive(Clone)]
pub struct PreparedTextLayoutResponse {
    catalog_fingerprint: FontCatalogFingerprint,
    contract_version: u32,
    backend: TextLayoutBackendIdentity,
    capabilities: TextLayoutCapabilities,
    font_source: FontSource,
    face_evidence: TextLayoutFaceEvidence,
    session_token: TextLayoutSessionToken,
    session: Arc<dyn PreparedTextBackendSession>,
    native_session: Option<Arc<NativeCatalogTextMeasurer>>,
}

impl PreparedTextLayoutResponse {
    pub fn new(
        catalog_fingerprint: FontCatalogFingerprint,
        contract_version: u32,
        backend: TextLayoutBackendIdentity,
        capabilities: TextLayoutCapabilities,
        font_source: FontSource,
        face_evidence: TextLayoutFaceEvidence,
        session_token: TextLayoutSessionToken,
        session: Arc<dyn PreparedTextBackendSession>,
    ) -> Result<Self, TextLayoutError> {
        session_token.validate()?;
        Ok(Self {
            catalog_fingerprint,
            contract_version,
            backend,
            capabilities,
            font_source,
            face_evidence,
            session_token,
            session,
            native_session: None,
        })
    }

    fn with_native_session(mut self, session: Arc<NativeCatalogTextMeasurer>) -> Self {
        self.native_session = Some(session);
        self
    }

    pub const fn catalog_fingerprint(&self) -> FontCatalogFingerprint {
        self.catalog_fingerprint
    }

    pub const fn contract_version(&self) -> u32 {
        self.contract_version
    }

    pub const fn backend(&self) -> &TextLayoutBackendIdentity {
        &self.backend
    }

    pub const fn capabilities(&self) -> TextLayoutCapabilities {
        self.capabilities
    }

    pub const fn font_source(&self) -> FontSource {
        self.font_source
    }

    pub const fn face_evidence(&self) -> &TextLayoutFaceEvidence {
        &self.face_evidence
    }

    pub const fn session_token(&self) -> TextLayoutSessionToken {
        self.session_token
    }
}

impl fmt::Debug for PreparedTextLayoutResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedTextLayoutResponse")
            .field("catalog_fingerprint", &self.catalog_fingerprint)
            .field("contract_version", &self.contract_version)
            .field("backend", &self.backend)
            .field("capabilities", &self.capabilities)
            .field("font_source", &self.font_source)
            .field("face_count", &self.face_evidence.faces.len())
            .field("session_token", &self.session_token)
            .finish_non_exhaustive()
    }
}

/// Frozen evidence retained in a render-session report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedTextLayoutReport {
    catalog_fingerprint: FontCatalogFingerprint,
    face_count: usize,
    used_font_sources: Arc<[FontSource]>,
    used_fallbacks: Arc<[HostMeasurementFallback]>,
    fallback_count: u64,
    failed_attempt_count: u64,
    host_dependent: bool,
}

impl PreparedTextLayoutReport {
    pub const fn catalog_fingerprint(&self) -> FontCatalogFingerprint {
        self.catalog_fingerprint
    }

    pub const fn face_count(&self) -> usize {
        self.face_count
    }

    pub fn used_font_sources(&self) -> &[FontSource] {
        &self.used_font_sources
    }

    pub fn uses_font_source(&self, source: FontSource) -> bool {
        self.used_font_sources.contains(&source)
    }

    pub fn used_fallbacks(&self) -> &[HostMeasurementFallback] {
        &self.used_fallbacks
    }

    pub const fn fallback_count(&self) -> u64 {
        self.fallback_count
    }

    pub const fn failed_attempt_count(&self) -> u64 {
        self.failed_attempt_count
    }

    pub const fn is_host_dependent(&self) -> bool {
        self.host_dependent
    }
}

impl fmt::Debug for PreparedTextLayout {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let primary = self.primary_candidate();
        formatter
            .debug_struct("PreparedTextLayout")
            .field("catalog_fingerprint", &self.catalog_fingerprint)
            .field("candidate_count", &self.candidates.len())
            .field("primary_backend", &primary.backend)
            .field("primary_capabilities", &primary.capabilities)
            .field("primary_font_source", &primary.font_source)
            .field("face_count", &self.catalog.faces().len())
            .field("primary_session_token", &primary.session_token)
            .finish_non_exhaustive()
    }
}

impl PreparedTextLayoutBuilder {
    pub(crate) fn new(request: PrepareCatalogRequest) -> Self {
        Self {
            request,
            candidates: Vec::new(),
            catalog_failure_count: 0,
        }
    }

    pub(crate) fn record_catalog_failure(&mut self) {
        self.catalog_failure_count = self.catalog_failure_count.saturating_add(1);
    }

    pub(crate) fn admit_strict_response(
        &mut self,
        expected_backend: &TextLayoutBackendIdentity,
        advertised_capabilities: TextLayoutCapabilities,
        response: PreparedTextLayoutResponse,
        fallback: Option<HostMeasurementFallback>,
        portability: ThemePortabilityRequirement,
    ) -> Result<(), TextLayoutError> {
        self.admit_response(
            expected_backend,
            advertised_capabilities,
            response,
            fallback,
            portability,
            false,
            PreparedTextLabelProvenance::HostDependent,
        )
    }

    pub(crate) fn admit_portable_response(
        &mut self,
        expected_backend: &TextLayoutBackendIdentity,
        advertised_capabilities: TextLayoutCapabilities,
        response: PreparedTextLayoutResponse,
        fallback: Option<HostMeasurementFallback>,
    ) -> Result<(), TextLayoutError> {
        self.admit_response(
            expected_backend,
            advertised_capabilities,
            response,
            fallback,
            ThemePortabilityRequirement::RequirePortable,
            false,
            PreparedTextLabelProvenance::Native,
        )
    }

    pub(crate) fn admit_host_dependent_response(
        &mut self,
        expected_backend: &TextLayoutBackendIdentity,
        advertised_capabilities: TextLayoutCapabilities,
        response: PreparedTextLayoutResponse,
        portability: ThemePortabilityRequirement,
    ) -> Result<(), TextLayoutError> {
        self.admit_response(
            expected_backend,
            advertised_capabilities,
            response,
            Some(HostMeasurementFallback::AcceptHostDependent),
            portability,
            true,
            PreparedTextLabelProvenance::HostDependent,
        )
    }

    fn admit_response(
        &mut self,
        expected_backend: &TextLayoutBackendIdentity,
        advertised_capabilities: TextLayoutCapabilities,
        response: PreparedTextLayoutResponse,
        fallback: Option<HostMeasurementFallback>,
        portability: ThemePortabilityRequirement,
        relaxed_capabilities: bool,
        evidence_provenance: PreparedTextLabelProvenance,
    ) -> Result<(), TextLayoutError> {
        validate_catalog_response_binding(
            &self.request,
            expected_backend,
            advertised_capabilities,
            &response,
        )?;
        if relaxed_capabilities {
            validate_face_evidence_subset(&self.request, &response.face_evidence)?;
        } else {
            validate_capabilities(&self.request, response.capabilities)?;
            if response.face_evidence.faces() != self.request.catalog.faces() {
                return Err(TextLayoutError::LoadedFaceEvidenceMismatch);
            }
        }
        let host_dependent = evidence_provenance.is_host_dependent()
            || relaxed_capabilities
            || response.font_source == FontSource::System;
        if host_dependent && portability == ThemePortabilityRequirement::RequirePortable {
            return Err(TextLayoutError::HostDependentNotPortable);
        }
        let face_keys = prepared_text_face_keys(&self.request.catalog, &response.face_evidence)?;
        self.candidates.push(PreparedTextLayoutCandidate {
            backend: response.backend,
            capabilities: response.capabilities,
            font_source: response.font_source,
            face_evidence: response.face_evidence,
            face_keys,
            session_token: response.session_token,
            session: response.session,
            native_session: response.native_session,
            fallback,
            host_dependent,
            evidence_provenance,
            relaxed_capabilities,
            disabled: Arc::new(AtomicBool::new(false)),
        });
        Ok(())
    }

    pub(crate) fn has_backend(&self, backend: &TextLayoutBackendIdentity) -> bool {
        self.candidates
            .iter()
            .any(|candidate| &candidate.backend == backend)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }

    pub(crate) fn build(self) -> Result<PreparedTextLayout, TextLayoutError> {
        if self.candidates.is_empty() {
            return Err(TextLayoutError::NoUsableFace);
        }
        let candidate_count = self.candidates.len();
        Ok(PreparedTextLayout {
            catalog: self.request.catalog,
            catalog_fingerprint: self.request.catalog_fingerprint,
            contract_version: self.request.contract_version,
            candidates: self.candidates.into(),
            usage: Arc::new(Mutex::new(PreparedTextLayoutUsage {
                candidates: vec![PreparedTextCandidateUsage::default(); candidate_count],
                catalog_failure_count: self.catalog_failure_count,
            })),
        })
    }
}

fn prepared_text_face_keys(
    catalog: &FontCatalog,
    evidence: &TextLayoutFaceEvidence,
) -> Result<Arc<[PreparedTextFaceKey]>, TextLayoutError> {
    let assets = catalog
        .assets()
        .iter()
        .map(|asset| (asset.id(), asset.fingerprint()))
        .collect::<BTreeMap<_, _>>();
    evidence
        .faces()
        .iter()
        .map(|face| {
            assets
                .get(face.asset_id())
                .copied()
                .map(|fingerprint| PreparedTextFaceKey::new(fingerprint, face.face_index()))
                .ok_or(TextLayoutError::LoadedFaceEvidenceMismatch)
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Arc::from)
}

fn validate_catalog_response_binding(
    request: &PrepareCatalogRequest,
    expected_backend: &TextLayoutBackendIdentity,
    advertised_capabilities: TextLayoutCapabilities,
    response: &PreparedTextLayoutResponse,
) -> Result<(), TextLayoutError> {
    if request.catalog.fingerprint() != request.catalog_fingerprint
        || response.catalog_fingerprint != request.catalog_fingerprint
    {
        return Err(TextLayoutError::CatalogFingerprintMismatch);
    }
    if request.contract_version != TEXT_LAYOUT_CONTRACT_VERSION {
        return Err(TextLayoutError::UnsupportedContract {
            requested: request.contract_version,
            supported: TEXT_LAYOUT_CONTRACT_VERSION,
        });
    }
    if response.contract_version != request.contract_version {
        return Err(TextLayoutError::UnsupportedContract {
            requested: response.contract_version,
            supported: request.contract_version,
        });
    }
    if &response.backend != expected_backend {
        return Err(TextLayoutError::BackendIdentityMismatch);
    }
    if !capabilities_are_subset(response.capabilities, advertised_capabilities) {
        return Err(TextLayoutError::CapabilityNotAttested("backend"));
    }
    if !request.font_source_policy.contains(response.font_source) {
        return Err(TextLayoutError::FontSourceUnavailable);
    }
    response.session_token.validate()
}

fn validate_face_evidence_subset(
    request: &PrepareCatalogRequest,
    evidence: &TextLayoutFaceEvidence,
) -> Result<(), TextLayoutError> {
    if evidence
        .faces()
        .iter()
        .any(|face| !request.catalog.faces().contains(face))
    {
        return Err(TextLayoutError::LoadedFaceEvidenceMismatch);
    }
    Ok(())
}

impl PreparedTextLayout {
    pub(crate) fn admit_backend_response(
        request: &PrepareCatalogRequest,
        expected_backend: &TextLayoutBackendIdentity,
        advertised_capabilities: TextLayoutCapabilities,
        response: PreparedTextLayoutResponse,
    ) -> Result<Self, TextLayoutError> {
        let mut builder = PreparedTextLayoutBuilder::new(request.clone());
        builder.admit_strict_response(
            expected_backend,
            advertised_capabilities,
            response,
            None,
            ThemePortabilityRequirement::BestEffort,
        )?;
        builder.build()
    }

    fn admit_native_backend_response(
        request: &PrepareCatalogRequest,
        expected_backend: &TextLayoutBackendIdentity,
        advertised_capabilities: TextLayoutCapabilities,
        response: PreparedTextLayoutResponse,
    ) -> Result<Self, TextLayoutError> {
        let mut builder = PreparedTextLayoutBuilder::new(request.clone());
        builder.admit_portable_response(
            expected_backend,
            advertised_capabilities,
            response,
            None,
        )?;
        builder.build()
    }

    fn primary_candidate(&self) -> &PreparedTextLayoutCandidate {
        self.candidates
            .first()
            .expect("prepared text layout retains at least one candidate")
    }

    pub(crate) fn catalog(&self) -> &FontCatalog {
        &self.catalog
    }

    pub(crate) fn admit_typography(
        &self,
        requested: &ThemeTextStyle,
    ) -> Result<CatalogAdmittedTextStyle, TextLayoutError> {
        self.admit_typography_with_css_font_stack(requested, None)
    }

    pub(crate) fn admit_typography_request(
        &self,
        requested: &PreparedTextTypographyRequest,
    ) -> Result<CatalogAdmittedTextStyle, TextLayoutError> {
        self.admit_typography_with_css_font_stack(
            requested.typography(),
            requested.css_font_stack(),
        )
    }

    pub(crate) fn admit_typography_with_css_font_stack(
        &self,
        requested: &ThemeTextStyle,
        css_font_stack: Option<&ParsedCssFontStack>,
    ) -> Result<CatalogAdmittedTextStyle, TextLayoutError> {
        let font_stack = if let Some(css_font_stack) = css_font_stack {
            let mut admitted = Vec::new();
            let mut admitted_keys = BTreeSet::new();
            for (index, family) in css_font_stack.families().iter().enumerate() {
                let canonical = if let Some(generic) = css_font_stack.generic_family(index) {
                    self.catalog.generic_family(generic)
                } else {
                    self.catalog.canonical_named_family_name(family)
                };
                let Some(canonical) = canonical else {
                    continue;
                };
                let key = canonical.to_lowercase();
                if admitted_keys.insert(key) {
                    admitted.push(canonical.to_string());
                }
            }
            FontStack::new(admitted).ok()
        } else {
            self.catalog.admit_font_stack(requested.font_stack())
        }
        .ok_or(TextLayoutError::FontFamilyUnavailable)?;
        Ok(CatalogAdmittedTextStyle {
            typography: requested.clone().with_font_stack(font_stack),
            catalog_fingerprint: self.catalog_fingerprint,
        })
    }

    pub(crate) const fn catalog_fingerprint(&self) -> FontCatalogFingerprint {
        self.catalog_fingerprint
    }

    pub(crate) const fn contract_version(&self) -> u32 {
        self.contract_version
    }

    pub(crate) fn backend(&self) -> &TextLayoutBackendIdentity {
        &self.primary_candidate().backend
    }

    pub(crate) fn capabilities(&self) -> TextLayoutCapabilities {
        self.primary_candidate().capabilities
    }

    pub(crate) fn font_source(&self) -> FontSource {
        self.primary_candidate().font_source
    }

    pub(crate) fn face_evidence(&self) -> &TextLayoutFaceEvidence {
        &self.primary_candidate().face_evidence
    }

    pub(crate) fn session_token(&self) -> TextLayoutSessionToken {
        self.primary_candidate().session_token
    }

    pub(crate) fn prepare_text(
        &self,
        request: &PrepareTextRequest,
    ) -> Result<PreparedText, TextLayoutError> {
        self.prepare_text_internal(request, None)
    }

    pub(crate) fn prepare_text_with_work_meter(
        &self,
        request: &PrepareTextRequest,
        work_meter: &OperationWorkMeter,
    ) -> Result<PreparedText, TextLayoutError> {
        self.prepare_text_internal(request, Some(work_meter))
    }

    fn prepare_text_internal(
        &self,
        request: &PrepareTextRequest,
        work_meter: Option<&OperationWorkMeter>,
    ) -> Result<PreparedText, TextLayoutError> {
        let projection = text_projection_for_request_with_meter(request, work_meter)?;
        let request_digest = request.digest_with_work_meter(work_meter)?;
        let mut last_error = None;
        for (candidate_index, candidate) in self.candidates.iter().enumerate() {
            if candidate.disabled.load(Ordering::Acquire) {
                continue;
            }
            match candidate.prepare_text(
                &self.catalog,
                self.contract_version,
                request_digest,
                request,
                projection.clone(),
                work_meter,
            ) {
                Ok(prepared) => {
                    self.record_candidate_success(candidate_index, candidate.fallback.is_some());
                    return Ok(prepared);
                }
                Err(error) => {
                    self.record_candidate_failure(candidate_index);
                    if text_layout_error_invalidates_candidate(&error) {
                        candidate.disabled.store(true, Ordering::Release);
                    }
                    let can_advance = candidate_index + 1 < self.candidates.len()
                        && text_layout_error_allows_fallback(&error);
                    if !can_advance {
                        return Err(error);
                    }
                    last_error = Some(error);
                }
            }
        }
        Err(last_error.unwrap_or(TextLayoutError::NoUsableFace))
    }

    fn record_candidate_success(&self, candidate_index: usize, fallback: bool) {
        let mut usage = self
            .usage
            .lock()
            .expect("prepared text usage recorder is not poisoned");
        let candidate = &mut usage.candidates[candidate_index];
        candidate.successes = candidate.successes.saturating_add(1);
        if fallback {
            candidate.fallback_selections = candidate.fallback_selections.saturating_add(1);
        }
    }

    fn record_candidate_failure(&self, candidate_index: usize) {
        let mut usage = self
            .usage
            .lock()
            .expect("prepared text usage recorder is not poisoned");
        let candidate = &mut usage.candidates[candidate_index];
        candidate.failures = candidate.failures.saturating_add(1);
    }

    fn admit_text_response(
        &self,
        request: &PreparedTextBackendRequest,
        response: PreparedTextResponse,
    ) -> Result<PreparedText, TextLayoutError> {
        self.primary_candidate().admit_text_response(
            &self.catalog,
            self.contract_version,
            request,
            response,
            None,
        )
    }

    pub(crate) fn attests_catalog(&self, fingerprint: FontCatalogFingerprint) -> bool {
        self.catalog_fingerprint == fingerprint
    }

    pub(crate) fn report(&self) -> PreparedTextLayoutReport {
        let usage = self
            .usage
            .lock()
            .expect("prepared text usage recorder is not poisoned");
        let mut used_font_sources = BTreeSet::new();
        let mut used_fallbacks = BTreeSet::new();
        let mut fallback_count = 0_u64;
        let mut failed_attempt_count = usage.catalog_failure_count;
        let mut host_dependent = false;
        for (candidate, usage) in self.candidates.iter().zip(&usage.candidates) {
            failed_attempt_count = failed_attempt_count.saturating_add(usage.failures);
            fallback_count = fallback_count.saturating_add(usage.fallback_selections);
            if usage.successes == 0 {
                continue;
            }
            used_font_sources.insert(candidate.font_source);
            if let Some(fallback) = candidate.fallback {
                used_fallbacks.insert(fallback);
            }
            host_dependent |= candidate.host_dependent;
        }
        PreparedTextLayoutReport {
            catalog_fingerprint: self.catalog_fingerprint,
            face_count: self.catalog.faces().len(),
            used_font_sources: used_font_sources.into_iter().collect::<Vec<_>>().into(),
            used_fallbacks: used_fallbacks.into_iter().collect::<Vec<_>>().into(),
            fallback_count,
            failed_attempt_count,
            host_dependent,
        }
    }
}

impl PreparedTextLayoutCandidate {
    fn prepare_text(
        &self,
        catalog: &FontCatalog,
        contract_version: u32,
        request_digest: TextLayoutRequestDigest,
        request: &PrepareTextRequest,
        projection: TextProjection,
        work_meter: Option<&OperationWorkMeter>,
    ) -> Result<PreparedText, TextLayoutError> {
        if !self.relaxed_capabilities {
            validate_text_capabilities(request, self.capabilities)?;
        }
        let binding = PreparedTextCallBinding::new(
            catalog.fingerprint(),
            contract_version,
            self.backend.clone(),
            self.session_token,
            request_digest,
        )?;
        let backend_request = PreparedTextBackendRequest::from_projection_with_digest(
            binding,
            request,
            projection,
            request_digest,
        )?;
        let response = if let (Some(native), Some(work_meter)) = (&self.native_session, work_meter)
        {
            let (response, _) = native
                .prepare_structured_text_attempt_with_work_meter(&backend_request, work_meter);
            response?
        } else {
            self.session.prepare_text(&backend_request)?
        };
        self.admit_text_response(
            catalog,
            contract_version,
            &backend_request,
            response,
            work_meter,
        )
    }

    fn admit_text_response(
        &self,
        catalog: &FontCatalog,
        contract_version: u32,
        request: &PreparedTextBackendRequest,
        response: PreparedTextResponse,
        work_meter: Option<&OperationWorkMeter>,
    ) -> Result<PreparedText, TextLayoutError> {
        let binding = response.binding();
        if binding.catalog_fingerprint() != catalog.fingerprint() {
            return Err(TextLayoutError::CatalogFingerprintMismatch);
        }
        if binding.contract_version() != contract_version {
            return Err(TextLayoutError::UnsupportedContract {
                requested: binding.contract_version(),
                supported: contract_version,
            });
        }
        if binding.backend() != &self.backend {
            return Err(TextLayoutError::BackendIdentityMismatch);
        }
        if binding.session_token() != self.session_token {
            return Err(TextLayoutError::SessionTokenMismatch);
        }
        if binding.request_digest() != request.request_digest() {
            return Err(TextLayoutError::RequestDigestMismatch);
        }
        if response.is_invalidated() {
            return Err(TextLayoutError::BackendInvalidated);
        }

        let projection = request.projection().clone();
        let budget = PreparedTextAdmissionBudget::new(work_meter);
        let line_summary = validate_prepared_text_response_budget(&projection, &response, &budget)?;
        budget.charge(response.lines().len())?;
        let mut lines = Vec::with_capacity(response.lines().len());
        for line in response.lines() {
            lines.push(PreparedTextLine::from_response(line)?);
        }
        validate_prepared_text_lines(&projection, &lines, &budget)?;
        validate_prepared_text_coverage(
            &projection,
            &lines,
            request.wrap(),
            request.wrapping_typography().white_space(),
            response.raw_width_px(),
            &budget,
        )?;
        let runs = admit_prepared_text_runs(
            &projection,
            &lines,
            response.runs(),
            &self.face_keys,
            self.font_source,
            &budget,
        )?;

        PreparedText::new_with_validated_evidence(
            projection,
            lines,
            runs,
            line_summary,
            response.line_height_px(),
            response.raw_width_px(),
            response.diagnostics.clone(),
            Some(PreparedTextLabelEvidenceContext {
                catalog_fingerprint: catalog.fingerprint(),
                request_digest: binding.request_digest(),
                provenance: self.evidence_provenance,
            }),
            &budget,
        )
    }
}

fn text_layout_error_allows_fallback(error: &TextLayoutError) -> bool {
    !matches!(
        error,
        TextLayoutError::InvalidRequest(_)
            | TextLayoutError::InvalidFeature
            | TextLayoutError::InvalidVariation
            | TextLayoutError::LimitExceeded(_)
            | TextLayoutError::UnsupportedPreparedTextPath(_)
    )
}

fn text_layout_error_invalidates_candidate(error: &TextLayoutError) -> bool {
    matches!(
        error,
        TextLayoutError::CatalogFingerprintMismatch
            | TextLayoutError::BackendIdentityMismatch
            | TextLayoutError::InvalidSessionToken
            | TextLayoutError::RequestDigestMismatch
            | TextLayoutError::SessionTokenMismatch
            | TextLayoutError::BackendInvalidated
            | TextLayoutError::RunEvidenceMismatch
            | TextLayoutError::UnsupportedContract { .. }
            | TextLayoutError::InvalidPreparedText
    )
}

fn admit_prepared_text_runs(
    projection: &TextProjection,
    lines: &[PreparedTextLine],
    runs: &[PreparedTextRunResponse],
    face_keys: &[PreparedTextFaceKey],
    font_source: FontSource,
    budget: &PreparedTextAdmissionBudget<'_>,
) -> Result<Vec<PreparedTextLabelEvidence>, TextLayoutError> {
    budget.charge(runs.len())?;
    let mut admitted = Vec::with_capacity(runs.len());
    let mut span_cursor = OrderedProjectionRangeCursor::new(projection);
    for run in runs {
        let range = run.visible_range();
        let Some(face_key) = usize::try_from(run.face_slot().index())
            .ok()
            .and_then(|slot| face_keys.get(slot))
            .copied()
        else {
            return Err(TextLayoutError::RunEvidenceMismatch);
        };
        if range.start() >= range.end() || run.font_source() != font_source {
            return Err(TextLayoutError::RunEvidenceMismatch);
        }
        let span_range = span_cursor.admit(range, budget, TextLayoutError::RunEvidenceMismatch)?;
        let Some((first, last)) = projection
            .spans()
            .get(span_range.clone())
            .and_then(|spans| spans.first().zip(spans.last()))
        else {
            return Err(TextLayoutError::RunEvidenceMismatch);
        };
        let source_range = TextByteRange::new(first.source().start(), last.source().end());
        admitted.push(PreparedTextLabelEvidence::new(
            source_range,
            range,
            face_key,
            run.font_source(),
        ));
    }

    budget.charge(lines.len().saturating_add(runs.len()))?;
    let mut run_index = 0;
    for line in lines {
        let line_range = line.visible_range;
        if line_range.start() == line_range.end() {
            continue;
        }
        let mut cursor = line_range.start();
        while let Some(run) = runs.get(run_index) {
            let range = run.visible_range();
            if range.start() >= line_range.end() {
                break;
            }
            if range.start() != cursor || range.end() > line_range.end() {
                return Err(TextLayoutError::RunEvidenceMismatch);
            }
            cursor = range.end();
            run_index += 1;
        }
        if cursor != line_range.end() {
            return Err(TextLayoutError::RunEvidenceMismatch);
        }
    }
    if run_index != runs.len() {
        return Err(TextLayoutError::RunEvidenceMismatch);
    }
    Ok(admitted)
}

/// Opaque token identifying one prepared backend session.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextLayoutSessionToken([u8; 16]);

impl TextLayoutSessionToken {
    pub fn from_bytes(bytes: [u8; 16]) -> Result<Self, TextLayoutError> {
        if bytes.iter().all(|byte| *byte == 0) {
            return Err(TextLayoutError::InvalidSessionToken);
        }
        Ok(Self(bytes))
    }

    fn validate(self) -> Result<(), TextLayoutError> {
        Self::from_bytes(self.0).map(|_| ())
    }

    fn derive(
        fingerprint: FontCatalogFingerprint,
        backend: &TextLayoutBackendIdentity,
        font_source: FontSource,
    ) -> Self {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(b"merman-text-layout-session-v1");
        update_hash_field(&mut hasher, fingerprint.as_bytes());
        update_hash_field(&mut hasher, backend.name.as_bytes());
        update_hash_field(&mut hasher, backend.version.as_bytes());
        update_hash_field(&mut hasher, font_source.id().as_bytes());
        let digest: [u8; 32] = hasher.finalize().into();
        Self(digest[..16].try_into().expect("fixed token length"))
    }

    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

impl fmt::Debug for TextLayoutSessionToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("TextLayoutSessionToken")
            .field(&self.0)
            .finish()
    }
}

/// Stable, coarse failure category exposed by render-session reports.
///
/// Backend protocol details remain internal until the optional host-assurance boundary is proven.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, thiserror::Error)]
#[non_exhaustive]
pub enum TextLayoutFailure {
    #[error("text layout input is invalid")]
    InvalidInput,
    #[error("text layout exceeded a resource limit")]
    ResourceLimitExceeded,
    #[error("text layout could not resolve an admitted font or glyph")]
    FontUnavailable,
    #[error("the selected label mode is not supported by prepared text layout")]
    UnsupportedLabelMode,
    #[error("host-dependent text layout cannot satisfy strict portability")]
    HostDependentNotPortable,
    #[error("the text layout backend is unavailable")]
    BackendUnavailable,
    #[error("the text layout backend returned invalid evidence")]
    InvalidBackendEvidence,
}

impl TextLayoutFailure {
    pub const fn id(self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid-input",
            Self::ResourceLimitExceeded => "resource-limit-exceeded",
            Self::FontUnavailable => "font-unavailable",
            Self::UnsupportedLabelMode => "unsupported-label-mode",
            Self::HostDependentNotPortable => "host-dependent-not-portable",
            Self::BackendUnavailable => "backend-unavailable",
            Self::InvalidBackendEvidence => "invalid-backend-evidence",
        }
    }
}

/// Internal errors raised before a catalog-backed layout session is admitted.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum TextLayoutError {
    #[error("text layout backend name and version must be non-empty")]
    InvalidBackendIdentity,
    #[error("text layout request field `{0}` is invalid")]
    InvalidRequest(&'static str),
    #[error("text layout feature string is invalid")]
    InvalidFeature,
    #[error("text layout variation string is invalid")]
    InvalidVariation,
    #[error("text layout request field `{0}` exceeds its bounded limit")]
    LimitExceeded(&'static str),
    #[error("text layout catalog fingerprint does not match the retained catalog")]
    CatalogFingerprintMismatch,
    #[error("text layout backend identity does not match the backend that prepared the session")]
    BackendIdentityMismatch,
    #[error("text layout capability `{0}` was not attested")]
    CapabilityNotAttested(&'static str),
    #[error("text layout backend did not attest the complete requested face catalog")]
    LoadedFaceEvidenceMismatch,
    #[error("text layout session token must not be all zeroes")]
    InvalidSessionToken,
    #[error("prepared text response does not match the current request digest")]
    RequestDigestMismatch,
    #[error("prepared text response does not belong to the current layout session")]
    SessionTokenMismatch,
    #[error("prepared text backend invalidated the current label result")]
    BackendInvalidated,
    #[error("prepared text run evidence does not match the admitted catalog and visible ranges")]
    RunEvidenceMismatch,
    #[error("text layout contract version {requested} is unsupported (supported: {supported})")]
    UnsupportedContract { requested: u32, supported: u32 },
    #[error("prepared text layout has no usable font face")]
    NoUsableFace,
    #[error("prepared text layout cannot use any allowed font source")]
    FontSourceUnavailable,
    #[error("host-dependent text layout cannot satisfy strict portability")]
    HostDependentNotPortable,
    #[error("prepared text layout cannot resolve the requested font stack")]
    FontFamilyUnavailable,
    #[error("prepared text layout cannot resolve a glyph from the requested font stack")]
    GlyphUnavailable,
    #[error("prepared text layout does not support the `{0}` label path")]
    UnsupportedPreparedTextPath(&'static str),
    #[error("prepared text layout returned an invalid result")]
    InvalidPreparedText,
    #[error("text layout backend rejected catalog preparation")]
    BackendRejected,
    #[error("text layout backend timed out while preparing the catalog")]
    BackendTimeout,
}

impl From<&TextLayoutError> for TextLayoutFailure {
    fn from(error: &TextLayoutError) -> Self {
        match error {
            TextLayoutError::InvalidRequest(_)
            | TextLayoutError::InvalidFeature
            | TextLayoutError::InvalidVariation => Self::InvalidInput,
            TextLayoutError::LimitExceeded(_) => Self::ResourceLimitExceeded,
            TextLayoutError::NoUsableFace
            | TextLayoutError::FontSourceUnavailable
            | TextLayoutError::FontFamilyUnavailable
            | TextLayoutError::GlyphUnavailable => Self::FontUnavailable,
            TextLayoutError::UnsupportedPreparedTextPath(_) => Self::UnsupportedLabelMode,
            TextLayoutError::HostDependentNotPortable => Self::HostDependentNotPortable,
            TextLayoutError::BackendRejected | TextLayoutError::BackendTimeout => {
                Self::BackendUnavailable
            }
            TextLayoutError::InvalidBackendIdentity
            | TextLayoutError::CatalogFingerprintMismatch
            | TextLayoutError::BackendIdentityMismatch
            | TextLayoutError::CapabilityNotAttested(_)
            | TextLayoutError::LoadedFaceEvidenceMismatch
            | TextLayoutError::InvalidSessionToken
            | TextLayoutError::RequestDigestMismatch
            | TextLayoutError::SessionTokenMismatch
            | TextLayoutError::BackendInvalidated
            | TextLayoutError::RunEvidenceMismatch
            | TextLayoutError::UnsupportedContract { .. }
            | TextLayoutError::InvalidPreparedText => Self::InvalidBackendEvidence,
        }
    }
}

pub(crate) fn text_projection_layout_error(error: TextProjectionError) -> TextLayoutError {
    match error {
        TextProjectionError::SourceLimitExceeded | TextProjectionError::VisibleLimitExceeded => {
            TextLayoutError::LimitExceeded("text")
        }
        TextProjectionError::SpanLimitExceeded => {
            TextLayoutError::LimitExceeded("projection_spans")
        }
        TextProjectionError::OperationWorkExceeded => {
            TextLayoutError::LimitExceeded("operation_work")
        }
        TextProjectionError::InvalidRanges => TextLayoutError::InvalidPreparedText,
    }
}

fn capabilities_are_subset(
    actual: TextLayoutCapabilities,
    advertised: TextLayoutCapabilities,
) -> bool {
    (!actual.supports_catalog_faces || advertised.supports_catalog_faces)
        && (!actual.supports_cluster_fallback || advertised.supports_cluster_fallback)
        && (!actual.supports_features || advertised.supports_features)
        && (!actual.supports_variations || advertised.supports_variations)
        && (!actual.supports_direction || advertised.supports_direction)
        && (!actual.supports_script || advertised.supports_script)
        && (!actual.supports_language || advertised.supports_language)
}

fn validate_capabilities(
    request: &PrepareCatalogRequest,
    capabilities: TextLayoutCapabilities,
) -> Result<(), TextLayoutError> {
    if !capabilities.supports_catalog_faces {
        return Err(TextLayoutError::CapabilityNotAttested("catalog_faces"));
    }
    if request.catalog.faces().len() > 1 && !capabilities.supports_cluster_fallback {
        return Err(TextLayoutError::CapabilityNotAttested("cluster_fallback"));
    }
    Ok(())
}

fn validate_text_capabilities(
    request: &PrepareTextRequest,
    capabilities: TextLayoutCapabilities,
) -> Result<(), TextLayoutError> {
    if request.direction != TextLayoutDirection::Auto && !capabilities.supports_direction {
        return Err(TextLayoutError::CapabilityNotAttested("direction"));
    }
    if request.script.is_some() && !capabilities.supports_script {
        return Err(TextLayoutError::CapabilityNotAttested("script"));
    }
    if request.language.is_some() && !capabilities.supports_language {
        return Err(TextLayoutError::CapabilityNotAttested("language"));
    }
    if !request.features.is_empty() && !capabilities.supports_features {
        return Err(TextLayoutError::CapabilityNotAttested("features"));
    }
    if !request.variations.is_empty() && !capabilities.supports_variations {
        return Err(TextLayoutError::CapabilityNotAttested("variations"));
    }
    Ok(())
}

fn parse_features(
    features: impl IntoIterator<Item = impl Into<String>>,
) -> Result<Arc<[Arc<str>]>, TextLayoutError> {
    let mut values = Vec::new();
    for feature in features {
        let feature = feature.into();
        if feature.trim().is_empty() || feature.len() > 64 {
            return Err(TextLayoutError::InvalidRequest("features"));
        }
        Feature::from_str(&feature).map_err(|_| TextLayoutError::InvalidFeature)?;
        values.push(Arc::from(feature));
    }
    if values.len() > 64 {
        return Err(TextLayoutError::LimitExceeded("features"));
    }
    Ok(values.into())
}

fn parse_variations(
    variations: impl IntoIterator<Item = impl Into<String>>,
) -> Result<Arc<[Arc<str>]>, TextLayoutError> {
    let mut values = Vec::new();
    for variation in variations {
        let variation = variation.into();
        if variation.trim().is_empty() || variation.len() > 64 {
            return Err(TextLayoutError::InvalidRequest("variations"));
        }
        Variation::from_str(&variation).map_err(|_| TextLayoutError::InvalidVariation)?;
        values.push(Arc::from(variation));
    }
    if values.len() > 64 {
        return Err(TextLayoutError::LimitExceeded("variations"));
    }
    Ok(values.into())
}

fn update_hash_field(hasher: &mut impl sha2::Digest, value: &[u8]) {
    hasher.update((value.len() as u64).to_le_bytes());
    hasher.update(value);
}

/// Backend API for preparing a catalog once per render session.
pub trait TextLayoutBackend: Send + Sync {
    fn identity(&self) -> &TextLayoutBackendIdentity;

    fn capabilities(&self) -> TextLayoutCapabilities;

    fn prepare_catalog(
        &self,
        request: &PrepareCatalogRequest,
    ) -> Result<PreparedTextLayoutResponse, TextLayoutError>;
}

/// The built-in, pure-Rust shaping backend used by custom font catalogs.
#[derive(Debug, Clone)]
pub struct NativeTextLayoutBackend {
    identity: TextLayoutBackendIdentity,
}

impl Default for NativeTextLayoutBackend {
    fn default() -> Self {
        Self {
            identity: TextLayoutBackendIdentity::new("merman.native-rustybuzz", "contract-1")
                .expect("static native text backend identity is valid"),
        }
    }
}

impl NativeTextLayoutBackend {
    pub fn new(identity: TextLayoutBackendIdentity) -> Self {
        Self { identity }
    }

    pub(crate) fn prepare(
        &self,
        request: &PrepareCatalogRequest,
    ) -> Result<PreparedTextLayout, TextLayoutError> {
        let response = <Self as TextLayoutBackend>::prepare_catalog(self, request)?;
        PreparedTextLayout::admit_native_backend_response(
            request,
            self.identity(),
            self.capabilities(),
            response,
        )
    }

    #[cfg(feature = "fuzzing")]
    #[doc(hidden)]
    fn prepare_text_probe(
        &self,
        catalog_request: &PrepareCatalogRequest,
        text_request: &PrepareTextRequest,
    ) -> Result<PreparedTextFuzzProbe, TextLayoutError> {
        let prepared = self.prepare(catalog_request)?.prepare_text(text_request)?;
        Ok(PreparedTextFuzzProbe {
            metrics: prepared.metrics(),
            raw_width_px: prepared.raw_width_px(),
            bbox_width_px: prepared.bbox_width_px(),
            bbox_height_px: prepared.bbox_height_px(),
            vertical_extents: prepared.vertical_extents(),
        })
    }
}

impl TextLayoutBackend for NativeTextLayoutBackend {
    fn identity(&self) -> &TextLayoutBackendIdentity {
        &self.identity
    }

    fn capabilities(&self) -> TextLayoutCapabilities {
        TextLayoutCapabilities::native()
    }

    fn prepare_catalog(
        &self,
        request: &PrepareCatalogRequest,
    ) -> Result<PreparedTextLayoutResponse, TextLayoutError> {
        if request.catalog().faces().is_empty() {
            return Err(TextLayoutError::NoUsableFace);
        }
        if !request.font_source_policy().contains(FontSource::Embedded) {
            return Err(TextLayoutError::FontSourceUnavailable);
        }
        let session_token = TextLayoutSessionToken::derive(
            request.catalog_fingerprint,
            &self.identity,
            FontSource::Embedded,
        );
        let native = Arc::new(NativeCatalogTextMeasurer::new(
            request,
            FontSource::Embedded,
        )?);
        let face_evidence =
            TextLayoutFaceEvidence::new(native.faces.iter().map(|face| face.metadata.clone()))?;
        let session: Arc<dyn PreparedTextBackendSession> = native.clone();
        PreparedTextLayoutResponse::new(
            request.catalog_fingerprint,
            request.contract_version,
            self.identity.clone(),
            self.capabilities(),
            FontSource::Embedded,
            face_evidence,
            session_token,
            session,
        )
        .map(|response| response.with_native_session(native))
    }
}

#[derive(Clone)]
struct PreparedFace {
    data: Arc<[u8]>,
    face_index: u32,
    metadata: FontFaceMetadata,
    family_key: String,
}

struct NativeCatalogTextMeasurer {
    faces: Vec<PreparedFace>,
    catalog: FontCatalog,
    font_source: FontSource,
    // These fields are retained only for the legacy TextMeasurer compatibility adapter. New
    // custom-font callers use PrepareTextRequest, which carries all shaping settings per label.
    direction: TextLayoutDirection,
    script: Option<BuzzScript>,
    language: Option<Language>,
    features: Vec<Feature>,
    variations: Vec<Variation>,
}

#[derive(Clone)]
struct FaceSelector {
    resolved_families: Vec<String>,
    requested_weight: u16,
    requested_style: String,
}

struct CompiledStructuredTextRequest {
    features: Vec<Feature>,
    variations: Vec<Variation>,
    requested_script: Option<BuzzScript>,
    language: Option<Language>,
    wrapping: CompiledStructuredTextStyle,
    metrics: CompiledStructuredTextStyle,
}

#[derive(Clone)]
struct CompiledStructuredTextStyle {
    candidate_faces: Arc<[usize]>,
    coverage_lane: usize,
}

const STRUCTURED_COVERAGE_UNCHECKED: u32 = u32::MAX;
const STRUCTURED_COVERAGE_MISSING: u32 = u32::MAX - 1;

struct StructuredCoverageCache {
    entries: Vec<[u32; STRUCTURED_COVERAGE_CACHE_LANES]>,
}

impl StructuredCoverageCache {
    fn new(span_count: usize) -> Result<Self, TextLayoutError> {
        let entry_count = span_count
            .checked_mul(STRUCTURED_COVERAGE_CACHE_LANES)
            .ok_or(TextLayoutError::LimitExceeded("coverage_cache"))?;
        if entry_count > MAX_STRUCTURED_COVERAGE_CACHE_ENTRIES {
            return Err(TextLayoutError::LimitExceeded("coverage_cache"));
        }
        Ok(Self {
            entries: vec![
                [STRUCTURED_COVERAGE_UNCHECKED; STRUCTURED_COVERAGE_CACHE_LANES];
                span_count
            ],
        })
    }

    fn get(
        &self,
        span_index: usize,
        lane: usize,
    ) -> Result<Option<Option<usize>>, TextLayoutError> {
        let value = *self
            .entries
            .get(span_index)
            .and_then(|entry| entry.get(lane))
            .ok_or(TextLayoutError::InvalidPreparedText)?;
        Ok(match value {
            STRUCTURED_COVERAGE_UNCHECKED => None,
            STRUCTURED_COVERAGE_MISSING => Some(None),
            face_index => Some(Some(face_index as usize)),
        })
    }

    fn insert(
        &mut self,
        span_index: usize,
        lane: usize,
        face_index: Option<usize>,
    ) -> Result<(), TextLayoutError> {
        let slot = self
            .entries
            .get_mut(span_index)
            .and_then(|entry| entry.get_mut(lane))
            .ok_or(TextLayoutError::InvalidPreparedText)?;
        if *slot != STRUCTURED_COVERAGE_UNCHECKED {
            return Err(TextLayoutError::InvalidPreparedText);
        }
        *slot = match face_index {
            Some(face_index) => u32::try_from(face_index)
                .ok()
                .filter(|face_index| *face_index < STRUCTURED_COVERAGE_MISSING)
                .ok_or(TextLayoutError::InvalidPreparedText)?,
            None => STRUCTURED_COVERAGE_MISSING,
        };
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct StructuredShapingWork {
    candidate_compilation_units: usize,
    coverage_input_bytes: usize,
    face_inspections: usize,
    coverage_cache_insertions: usize,
    coverage_cache_slots: usize,
    source_line_scan_bytes: usize,
    source_line_visits: usize,
    wrapped_line_emissions: usize,
    wrapping_input_bytes: usize,
    metrics_input_bytes: usize,
    wrapping_span_visits: usize,
    metrics_span_visits: usize,
    wrapping_line_ranges: usize,
    metrics_line_ranges: usize,
    glyph_visits: usize,
    cluster_visits: usize,
    wrap_boundary_visits: usize,
}

#[derive(Debug, Clone, Copy)]
enum StructuredShapingPass {
    Coverage,
    Wrapping,
    Metrics,
}

impl StructuredShapingWork {
    fn total_units(self) -> Result<usize, TextLayoutError> {
        [
            self.candidate_compilation_units,
            self.coverage_input_bytes,
            self.face_inspections,
            self.coverage_cache_insertions,
            self.coverage_cache_slots,
            self.source_line_scan_bytes,
            self.source_line_visits,
            self.wrapped_line_emissions,
            self.wrapping_input_bytes,
            self.metrics_input_bytes,
            self.wrapping_span_visits,
            self.metrics_span_visits,
            self.wrapping_line_ranges,
            self.metrics_line_ranges,
            self.glyph_visits,
            self.cluster_visits,
            self.wrap_boundary_visits,
        ]
        .into_iter()
        .try_fold(0usize, |total, units| total.checked_add(units))
        .ok_or(TextLayoutError::LimitExceeded("operation_work"))
    }
}

struct StructuredShapingBudget<'a> {
    work: StructuredShapingWork,
    work_meter: Option<&'a OperationWorkMeter>,
}

impl<'a> StructuredShapingBudget<'a> {
    fn new(work_meter: Option<&'a OperationWorkMeter>) -> Self {
        Self {
            work: StructuredShapingWork::default(),
            work_meter,
        }
    }

    fn charge(&self, units: usize) -> Result<(), TextLayoutError> {
        if let Some(work_meter) = self.work_meter {
            work_meter
                .charge(units)
                .map_err(|_| TextLayoutError::LimitExceeded("operation_work"))?;
        }
        Ok(())
    }

    fn record_candidate_compilation(&mut self, units: usize) -> Result<(), TextLayoutError> {
        let next = self
            .work
            .candidate_compilation_units
            .checked_add(units)
            .ok_or(TextLayoutError::LimitExceeded("candidate_compilation"))?;
        self.charge(units)?;
        self.work.candidate_compilation_units = next;
        Ok(())
    }

    fn record(
        &mut self,
        pass: StructuredShapingPass,
        input_bytes: usize,
    ) -> Result<(), TextLayoutError> {
        let (current, limit) = match pass {
            StructuredShapingPass::Coverage => (
                self.work.coverage_input_bytes,
                MAX_STRUCTURED_COVERAGE_INPUT_BYTES,
            ),
            StructuredShapingPass::Wrapping => {
                (self.work.wrapping_input_bytes, MAX_TEXT_PROJECTION_BYTES)
            }
            StructuredShapingPass::Metrics => {
                (self.work.metrics_input_bytes, MAX_TEXT_PROJECTION_BYTES)
            }
        };
        let next = current
            .checked_add(input_bytes)
            .ok_or(TextLayoutError::LimitExceeded("shaping_work"))?;
        if next > limit {
            return Err(TextLayoutError::LimitExceeded("shaping_work"));
        }
        self.charge(input_bytes)?;
        match pass {
            StructuredShapingPass::Coverage => self.work.coverage_input_bytes = next,
            StructuredShapingPass::Wrapping => self.work.wrapping_input_bytes = next,
            StructuredShapingPass::Metrics => self.work.metrics_input_bytes = next,
        }
        Ok(())
    }

    fn record_span_visit(&mut self, pass: StructuredShapingPass) -> Result<(), TextLayoutError> {
        let current = match pass {
            StructuredShapingPass::Wrapping => self.work.wrapping_span_visits,
            StructuredShapingPass::Metrics => self.work.metrics_span_visits,
            StructuredShapingPass::Coverage => return Ok(()),
        };
        let next = current
            .checked_add(1)
            .ok_or(TextLayoutError::LimitExceeded("span_work"))?;
        if next > MAX_STRUCTURED_SPAN_VISITS {
            return Err(TextLayoutError::LimitExceeded("span_work"));
        }
        self.charge(1)?;
        match pass {
            StructuredShapingPass::Wrapping => self.work.wrapping_span_visits = next,
            StructuredShapingPass::Metrics => self.work.metrics_span_visits = next,
            StructuredShapingPass::Coverage => {}
        }
        Ok(())
    }

    fn record_face_inspection(&mut self) -> Result<(), TextLayoutError> {
        let next = self
            .work
            .face_inspections
            .checked_add(1)
            .ok_or(TextLayoutError::LimitExceeded("coverage_work"))?;
        if next > MAX_STRUCTURED_FACE_INSPECTIONS {
            return Err(TextLayoutError::LimitExceeded("coverage_work"));
        }
        self.charge(1)?;
        self.work.face_inspections = next;
        Ok(())
    }

    fn record_coverage_cache_insertion(&mut self) -> Result<(), TextLayoutError> {
        let next = self
            .work
            .coverage_cache_insertions
            .checked_add(1)
            .ok_or(TextLayoutError::LimitExceeded("coverage_cache"))?;
        if next > MAX_STRUCTURED_COVERAGE_CACHE_ENTRIES {
            return Err(TextLayoutError::LimitExceeded("coverage_cache"));
        }
        self.charge(1)?;
        self.work.coverage_cache_insertions = next;
        Ok(())
    }

    fn record_coverage_cache_slots(&mut self, slots: usize) -> Result<(), TextLayoutError> {
        let next = self
            .work
            .coverage_cache_slots
            .checked_add(slots)
            .ok_or(TextLayoutError::LimitExceeded("coverage_cache"))?;
        if next > MAX_STRUCTURED_COVERAGE_CACHE_ENTRIES {
            return Err(TextLayoutError::LimitExceeded("coverage_cache"));
        }
        self.charge(slots)?;
        self.work.coverage_cache_slots = next;
        Ok(())
    }

    fn record_source_line_scan(&mut self, bytes: usize) -> Result<(), TextLayoutError> {
        let next = self
            .work
            .source_line_scan_bytes
            .checked_add(bytes)
            .ok_or(TextLayoutError::LimitExceeded("line_work"))?;
        if next > MAX_TEXT_PROJECTION_BYTES {
            return Err(TextLayoutError::LimitExceeded("line_work"));
        }
        self.charge(bytes)?;
        self.work.source_line_scan_bytes = next;
        Ok(())
    }

    fn record_source_line_visit(&mut self) -> Result<(), TextLayoutError> {
        let next = self
            .work
            .source_line_visits
            .checked_add(1)
            .ok_or(TextLayoutError::LimitExceeded("line_work"))?;
        if next > MAX_PREPARED_TEXT_LINES {
            return Err(TextLayoutError::LimitExceeded("lines"));
        }
        self.charge(1)?;
        self.work.source_line_visits = next;
        Ok(())
    }

    fn record_wrapped_line_emission(&mut self) -> Result<(), TextLayoutError> {
        let next = self
            .work
            .wrapped_line_emissions
            .checked_add(1)
            .ok_or(TextLayoutError::LimitExceeded("line_work"))?;
        if next > MAX_PREPARED_TEXT_LINES {
            return Err(TextLayoutError::LimitExceeded("lines"));
        }
        self.charge(1)?;
        self.work.wrapped_line_emissions = next;
        Ok(())
    }

    fn record_glyph_visits(&mut self, visits: usize) -> Result<(), TextLayoutError> {
        let next = self
            .work
            .glyph_visits
            .checked_add(visits)
            .ok_or(TextLayoutError::LimitExceeded("glyph_work"))?;
        if next > MAX_STRUCTURED_GLYPH_VISITS {
            return Err(TextLayoutError::LimitExceeded("glyph_work"));
        }
        self.charge(visits)?;
        self.work.glyph_visits = next;
        Ok(())
    }

    fn record_cluster_visits(&mut self, visits: usize) -> Result<(), TextLayoutError> {
        let next = self
            .work
            .cluster_visits
            .checked_add(visits)
            .ok_or(TextLayoutError::LimitExceeded("cluster_work"))?;
        if next > MAX_STRUCTURED_CLUSTER_VISITS {
            return Err(TextLayoutError::LimitExceeded("cluster_work"));
        }
        self.charge(visits)?;
        self.work.cluster_visits = next;
        Ok(())
    }

    fn record_wrap_boundary_visits(&mut self, visits: usize) -> Result<(), TextLayoutError> {
        let next = self
            .work
            .wrap_boundary_visits
            .checked_add(visits)
            .ok_or(TextLayoutError::LimitExceeded("wrap_work"))?;
        if next > MAX_STRUCTURED_WRAP_BOUNDARY_VISITS {
            return Err(TextLayoutError::LimitExceeded("wrap_work"));
        }
        self.charge(visits)?;
        self.work.wrap_boundary_visits = next;
        Ok(())
    }

    fn record_line_range(&mut self, pass: StructuredShapingPass) -> Result<(), TextLayoutError> {
        let current = match pass {
            StructuredShapingPass::Wrapping => self.work.wrapping_line_ranges,
            StructuredShapingPass::Metrics => self.work.metrics_line_ranges,
            StructuredShapingPass::Coverage => return Ok(()),
        };
        let next = current
            .checked_add(1)
            .ok_or(TextLayoutError::LimitExceeded("span_work"))?;
        if next > MAX_PREPARED_TEXT_LINES {
            return Err(TextLayoutError::LimitExceeded("span_work"));
        }
        self.charge(1)?;
        match pass {
            StructuredShapingPass::Wrapping => self.work.wrapping_line_ranges = next,
            StructuredShapingPass::Metrics => self.work.metrics_line_ranges = next,
            StructuredShapingPass::Coverage => {}
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
struct ProjectionSpanCursor {
    next_index: usize,
    previous_line_end: usize,
}

impl ProjectionSpanCursor {
    fn range_for_line(
        &mut self,
        projection: &TextProjection,
        line_range: TextByteRange,
        pass: StructuredShapingPass,
        budget: &mut StructuredShapingBudget<'_>,
    ) -> Result<std::ops::Range<usize>, TextLayoutError> {
        budget.record_line_range(pass)?;
        let start = line_range.start();
        let end = line_range.end();
        if start > end || start < self.previous_line_end || end > projection.visible().len() {
            return Err(TextLayoutError::InvalidPreparedText);
        }

        let spans = projection.spans();
        while let Some(span) = spans.get(self.next_index) {
            budget.record_span_visit(pass)?;
            if span.visible().end() <= start {
                self.next_index = self.next_index.saturating_add(1);
                continue;
            }
            break;
        }

        let first = self.next_index;
        let mut atom_cursor = start;
        while let Some(span) = spans.get(self.next_index) {
            budget.record_span_visit(pass)?;
            let visible = span.visible();
            if visible.start() >= end {
                break;
            }
            if visible.start() < start
                || visible.end() > end
                || visible.start() != atom_cursor
                || visible.end() < visible.start()
            {
                return Err(TextLayoutError::InvalidPreparedText);
            }
            atom_cursor = visible.end();
            self.next_index = self.next_index.saturating_add(1);
        }
        if atom_cursor != end {
            return Err(TextLayoutError::InvalidPreparedText);
        }
        self.previous_line_end = end;
        Ok(first..self.next_index)
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct ShapedLineMetrics {
    advance: f64,
    min_x: f64,
    max_x: f64,
    min_y: f64,
    max_y: f64,
    has_bounds: bool,
}

struct ShapedStructuredLine {
    metrics: ShapedLineMetrics,
    clusters: Vec<ShapedClusterMetric>,
    span_range: std::ops::Range<usize>,
}

struct ShapedStructuredRun {
    metrics: ShapedLineMetrics,
    clusters: Vec<ShapedClusterMetric>,
}

#[derive(Debug, Clone, Copy)]
struct ShapedClusterMetric {
    range: TextByteRange,
    advance: f64,
    min_x: f64,
    max_x: f64,
    has_bounds: bool,
}

impl ShapedClusterMetric {
    fn include_glyph(&mut self, advance: f64, bounds: Option<(f64, f64)>) {
        self.advance += advance;
        let Some((min_x, max_x)) = bounds else {
            return;
        };
        if self.has_bounds {
            self.min_x = self.min_x.min(min_x);
            self.max_x = self.max_x.max(max_x);
        } else {
            self.min_x = min_x;
            self.max_x = max_x;
            self.has_bounds = true;
        }
    }

    fn offset(mut self, byte_offset: usize, x_offset: f64) -> Self {
        self.range = TextByteRange::new(
            self.range.start().saturating_add(byte_offset),
            self.range.end().saturating_add(byte_offset),
        );
        if self.has_bounds {
            self.min_x += x_offset;
            self.max_x += x_offset;
        }
        self
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct WrapMeasure {
    advance: f64,
    min_x: f64,
    max_x: f64,
    grapheme_count: usize,
    ascii_space_count: usize,
    has_bounds: bool,
}

impl WrapMeasure {
    fn include(&mut self, other: Self) {
        self.advance += other.advance;
        self.grapheme_count = self.grapheme_count.saturating_add(other.grapheme_count);
        self.ascii_space_count = self
            .ascii_space_count
            .saturating_add(other.ascii_space_count);
        if !other.has_bounds {
            return;
        }
        if self.has_bounds {
            self.min_x = self.min_x.min(other.min_x);
            self.max_x = self.max_x.max(other.max_x);
        } else {
            self.min_x = other.min_x;
            self.max_x = other.max_x;
            self.has_bounds = true;
        }
    }

    fn width(self, typography: &ThemeTextStyle) -> f64 {
        let letter_spacing = f64::from(typography.letter_spacing_px())
            * self.grapheme_count.saturating_sub(1) as f64;
        let word_spacing = f64::from(typography.word_spacing_px()) * self.ascii_space_count as f64;
        let advance = (self.advance + letter_spacing + word_spacing).abs();
        let ink = if self.has_bounds {
            (self.max_x - self.min_x).max(0.0)
        } else {
            0.0
        };
        advance.max(ink)
    }
}

#[derive(Debug, Clone, Copy)]
struct WrapAtomMetric {
    range: TextByteRange,
    measure: WrapMeasure,
    whitespace: bool,
}

#[derive(Debug, Clone)]
struct ShapedWrapLine {
    atoms: Vec<WrapAtomMetric>,
    measure: WrapMeasure,
}

#[derive(Debug, Clone, Copy)]
struct WrapSegment {
    atom_start: usize,
    atom_end: usize,
}

#[derive(Debug, Clone, Copy, Default)]
struct WrapLineState {
    atom_start: Option<usize>,
    atom_end: usize,
    content_end: usize,
    content: WrapMeasure,
    trailing: WrapMeasure,
}

impl ShapedLineMetrics {
    fn include_run(&mut self, run: Self, offset_x: f64) {
        self.advance += run.advance;
        if !run.has_bounds {
            return;
        }
        let min_x = run.min_x + offset_x;
        let max_x = run.max_x + offset_x;
        if !self.has_bounds {
            self.min_x = min_x;
            self.max_x = max_x;
            self.min_y = run.min_y;
            self.max_y = run.max_y;
            self.has_bounds = true;
        } else {
            self.min_x = self.min_x.min(min_x);
            self.max_x = self.max_x.max(max_x);
            self.min_y = self.min_y.min(run.min_y);
            self.max_y = self.max_y.max(run.max_y);
        }
    }

    fn bbox_width(self) -> f64 {
        if self.has_bounds {
            (self.max_x - self.min_x).max(0.0)
        } else {
            0.0
        }
    }

    fn bbox_height(self) -> f64 {
        if self.has_bounds {
            (self.max_y - self.min_y).max(0.0)
        } else {
            0.0
        }
    }

    fn vertical_extents(self) -> Result<PreparedTextVerticalExtents, TextLayoutError> {
        if self.has_bounds {
            // Font coordinates are y-up around the alphabetic baseline. SVG uses y-down, so the
            // upper outline edge becomes `-max_y` and the lower edge becomes `-min_y`.
            PreparedTextVerticalExtents::new(-self.max_y, -self.min_y)
        } else {
            // Empty, whitespace-only, and default-ignorable lines have no ink. Retaining their
            // baseline as a zero-height point lets multiline aggregation preserve line advance.
            PreparedTextVerticalExtents::new(0.0, 0.0)
        }
    }

    fn bbox_x(self) -> (f64, f64) {
        if self.has_bounds {
            ((-self.min_x).max(0.0), self.max_x.max(0.0))
        } else {
            (0.0, 0.0)
        }
    }

    fn layout_width(self) -> f64 {
        self.advance.abs().max(self.bbox_width())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProjectedVisibleLine {
    visible_range: TextByteRange,
}

impl ProjectedVisibleLine {
    const fn new(visible_range: TextByteRange) -> Self {
        Self { visible_range }
    }

    const fn visible_range(self) -> TextByteRange {
        self.visible_range
    }

    fn text<'a>(self, visible: &'a str) -> Result<&'a str, TextLayoutError> {
        visible
            .get(self.visible_range.as_range())
            .ok_or(TextLayoutError::InvalidPreparedText)
    }

    fn from_local_range(source: Self, local_range: TextByteRange) -> Result<Self, TextLayoutError> {
        let source_len = source
            .visible_range
            .end()
            .saturating_sub(source.visible_range.start());
        if local_range.start() > local_range.end() || local_range.end() > source_len {
            return Err(TextLayoutError::InvalidPreparedText);
        }
        let start = source
            .visible_range
            .start()
            .checked_add(local_range.start())
            .ok_or(TextLayoutError::InvalidPreparedText)?;
        let end = source
            .visible_range
            .start()
            .checked_add(local_range.end())
            .ok_or(TextLayoutError::InvalidPreparedText)?;
        Ok(Self::new(TextByteRange::new(start, end)))
    }
}

fn split_projected_visible_lines(visible: &str) -> Vec<ProjectedVisibleLine> {
    projected_visible_lines(visible).collect()
}

fn projected_visible_lines(visible: &str) -> impl Iterator<Item = ProjectedVisibleLine> + '_ {
    let mut start: usize = 0;
    visible.split('\n').map(move |line| {
        let end = start.saturating_add(line.len());
        let projected = ProjectedVisibleLine::new(TextByteRange::new(start, end));
        start = end.saturating_add(1);
        projected
    })
}

impl WrapLineState {
    const fn is_empty(self) -> bool {
        self.atom_start.is_none()
    }

    fn has_visible_content(self) -> bool {
        self.atom_start
            .is_some_and(|start| self.content_end > start)
    }

    fn append_atom(&mut self, atom_index: usize, atom: WrapAtomMetric) {
        self.atom_start.get_or_insert(atom_index);
        self.atom_end = atom_index.saturating_add(1);
        if atom.whitespace {
            self.trailing.include(atom.measure);
        } else {
            self.content.include(self.trailing);
            self.trailing = WrapMeasure::default();
            self.content.include(atom.measure);
            self.content_end = self.atom_end;
        }
    }

    fn append_segment(&mut self, segment: WrapSegment, atoms: &[WrapAtomMetric]) {
        for (atom_index, atom) in atoms
            .iter()
            .copied()
            .enumerate()
            .take(segment.atom_end)
            .skip(segment.atom_start)
        {
            self.append_atom(atom_index, atom);
        }
    }

    fn width(self, typography: &ThemeTextStyle) -> f64 {
        self.content.width(typography)
    }

    fn trimmed_range(self, atoms: &[WrapAtomMetric]) -> Result<TextByteRange, TextLayoutError> {
        let Some(start) = self.atom_start else {
            return Ok(TextByteRange::new(0, 0));
        };
        let local_start = atoms
            .get(start)
            .ok_or(TextLayoutError::InvalidPreparedText)?
            .range
            .start();
        let local_end = if self.has_visible_content() {
            atoms
                .get(self.content_end.saturating_sub(1))
                .ok_or(TextLayoutError::InvalidPreparedText)?
                .range
                .end()
        } else {
            local_start
        };
        Ok(TextByteRange::new(local_start, local_end))
    }
}

fn coalesce_wrap_atoms(
    text: &str,
    visible_start: usize,
    projection: &TextProjection,
    span_range: std::ops::Range<usize>,
    clusters: &[ShapedClusterMetric],
    budget: &mut StructuredShapingBudget<'_>,
) -> Result<Vec<WrapAtomMetric>, TextLayoutError> {
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let mut grapheme_boundaries = text
        .grapheme_indices(true)
        .map(|(offset, _)| offset)
        .skip(1)
        .chain(std::iter::once(text.len()))
        .peekable();
    let mut projection_boundaries = projection.spans()[span_range]
        .iter()
        .filter(|span| span.visible().start() < span.visible().end())
        .map(|span| span.visible().end())
        .peekable();
    let mut atoms = Vec::new();
    let mut current_start = 0;
    let mut current_end = 0;
    let mut current_grapheme_count: usize = 0;
    let mut current_measure = WrapMeasure::default();
    for cluster in clusters {
        budget.record_cluster_visits(1)?;
        if cluster.range.start() != current_end
            || cluster.range.end() <= cluster.range.start()
            || cluster.range.end() > text.len()
        {
            return Err(TextLayoutError::InvalidPreparedText);
        }
        current_end = cluster.range.end();
        current_measure.include(WrapMeasure {
            advance: cluster.advance,
            min_x: cluster.min_x,
            max_x: cluster.max_x,
            grapheme_count: 0,
            ascii_space_count: 0,
            has_bounds: cluster.has_bounds,
        });
        let global_end = visible_start
            .checked_add(current_end)
            .ok_or(TextLayoutError::InvalidPreparedText)?;
        let mut grapheme_boundary = false;
        while grapheme_boundaries
            .peek()
            .is_some_and(|boundary| *boundary <= current_end)
        {
            let boundary = grapheme_boundaries
                .next()
                .expect("peeked grapheme boundary is present");
            budget.record_wrap_boundary_visits(1)?;
            current_grapheme_count = current_grapheme_count.saturating_add(1);
            if boundary == current_end {
                grapheme_boundary = true;
                break;
            }
        }
        while projection_boundaries
            .peek()
            .is_some_and(|boundary| *boundary < global_end)
        {
            projection_boundaries.next();
            budget.record_wrap_boundary_visits(1)?;
        }
        let projection_boundary = projection_boundaries
            .peek()
            .is_some_and(|boundary| *boundary == global_end);
        if projection_boundary {
            projection_boundaries.next();
            budget.record_wrap_boundary_visits(1)?;
        }
        if !grapheme_boundary || !projection_boundary {
            continue;
        }

        let atom_text = text
            .get(current_start..current_end)
            .ok_or(TextLayoutError::InvalidPreparedText)?;
        budget.record_wrap_boundary_visits(atom_text.len())?;
        current_measure.grapheme_count = current_grapheme_count;
        current_measure.ascii_space_count = atom_text.bytes().filter(|byte| *byte == b' ').count();
        if !current_measure.advance.is_finite()
            || current_measure.advance.abs() > MAX_PREPARED_TEXT_GEOMETRY_PX
            || (current_measure.has_bounds
                && (!current_measure.min_x.is_finite()
                    || !current_measure.max_x.is_finite()
                    || current_measure.min_x.abs() > MAX_PREPARED_TEXT_GEOMETRY_PX
                    || current_measure.max_x.abs() > MAX_PREPARED_TEXT_GEOMETRY_PX))
        {
            return Err(TextLayoutError::LimitExceeded("geometry"));
        }
        atoms.push(WrapAtomMetric {
            range: TextByteRange::new(current_start, current_end),
            measure: current_measure,
            whitespace: atom_text.chars().all(char::is_whitespace),
        });
        current_start = current_end;
        current_grapheme_count = 0;
        current_measure = WrapMeasure::default();
    }
    if current_end != text.len() || current_start != text.len() {
        return Err(TextLayoutError::InvalidPreparedText);
    }
    Ok(atoms)
}

fn svg_wrap_segment_ranges(text: &str) -> Vec<TextByteRange> {
    let mut ranges = Vec::new();
    let mut cursor = 0;
    while cursor < text.len() {
        let end = if text.as_bytes()[cursor] == b' ' {
            cursor + 1
        } else {
            text.as_bytes()[cursor..]
                .iter()
                .position(|byte| *byte == b' ')
                .map(|offset| cursor + offset)
                .unwrap_or(text.len())
        };
        ranges.push(TextByteRange::new(cursor, end));
        cursor = end;
    }
    ranges
}

fn html_wrap_segment_ranges(text: &str) -> Vec<TextByteRange> {
    let mut cursor: usize = 0;
    crate::text::line_break::html_break_spaces_segments(text)
        .into_iter()
        .map(|segment| {
            let start = cursor;
            cursor = cursor.saturating_add(segment.len());
            TextByteRange::new(start, cursor)
        })
        .collect()
}

fn align_wrap_segments(
    atoms: &[WrapAtomMetric],
    ranges: &[TextByteRange],
    budget: &mut StructuredShapingBudget<'_>,
) -> Result<Vec<WrapSegment>, TextLayoutError> {
    let mut segments = Vec::new();
    let mut atom_cursor = 0;
    let mut previous_end = 0;
    for range in ranges {
        budget.record_wrap_boundary_visits(1)?;
        if range.start() < previous_end || range.end() < range.start() {
            return Err(TextLayoutError::InvalidPreparedText);
        }
        previous_end = range.end();
        while atom_cursor < atoms.len() && atoms[atom_cursor].range.end() <= range.start() {
            budget.record_wrap_boundary_visits(1)?;
            atom_cursor += 1;
        }
        let atom_start = atom_cursor;
        while atom_cursor < atoms.len() && atoms[atom_cursor].range.start() < range.end() {
            budget.record_wrap_boundary_visits(1)?;
            atom_cursor += 1;
        }
        if atom_start < atom_cursor {
            segments.push(WrapSegment {
                atom_start,
                atom_end: atom_cursor,
            });
        }
    }
    if atom_cursor != atoms.len() {
        return Err(TextLayoutError::InvalidPreparedText);
    }
    Ok(segments)
}

fn segment_is_whitespace(
    segment: WrapSegment,
    atoms: &[WrapAtomMetric],
    budget: &mut StructuredShapingBudget<'_>,
) -> Result<bool, TextLayoutError> {
    budget.record_wrap_boundary_visits(segment.atom_end.saturating_sub(segment.atom_start))?;
    Ok(atoms[segment.atom_start..segment.atom_end]
        .iter()
        .all(|atom| atom.whitespace))
}

fn push_wrapped_state(
    wrapped: &mut Vec<ProjectedVisibleLine>,
    source: ProjectedVisibleLine,
    state: WrapLineState,
    atoms: &[WrapAtomMetric],
    budget: &mut StructuredShapingBudget<'_>,
) -> Result<(), TextLayoutError> {
    let line = ProjectedVisibleLine::from_local_range(source, state.trimmed_range(atoms)?)?;
    push_projected_visible_line(wrapped, line, budget)?;
    Ok(())
}

fn push_projected_visible_line(
    wrapped: &mut Vec<ProjectedVisibleLine>,
    line: ProjectedVisibleLine,
    budget: &mut StructuredShapingBudget<'_>,
) -> Result<(), TextLayoutError> {
    budget.record_wrapped_line_emission()?;
    wrapped.push(line);
    Ok(())
}

impl NativeCatalogTextMeasurer {
    fn new(
        request: &PrepareCatalogRequest,
        font_source: FontSource,
    ) -> Result<Self, TextLayoutError> {
        let mut faces = Vec::new();
        for metadata in request.catalog().faces() {
            let Some(asset) = request
                .catalog()
                .assets()
                .iter()
                .find(|asset| asset.id() == metadata.asset_id())
            else {
                continue;
            };
            if rustybuzz::Face::from_slice(asset.canonical_bytes(), metadata.face_index()).is_none()
            {
                continue;
            }
            faces.push(PreparedFace {
                data: asset.canonical_data(),
                face_index: metadata.face_index(),
                metadata: metadata.clone(),
                family_key: normalize_family(metadata.family_name()),
            });
        }
        if faces.is_empty() {
            return Err(TextLayoutError::NoUsableFace);
        }
        Ok(Self {
            faces,
            catalog: request.catalog().clone(),
            font_source,
            direction: TextLayoutDirection::Auto,
            script: None,
            language: None,
            features: Vec::new(),
            variations: Vec::new(),
        })
    }

    fn selector(&self, style: &TextStyle) -> FaceSelector {
        let resolved_families = style
            .font_family
            .as_deref()
            .map(|family| {
                family
                    .split(',')
                    .map(|value| self.resolve_family(&normalize_family(value.trim_matches('"'))))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let requested_weight = style
            .font_weight
            .as_deref()
            .map(parse_font_weight)
            .unwrap_or(400);
        let requested_style = style
            .font_style
            .as_deref()
            .map(normalize_family)
            .unwrap_or_else(|| "normal".to_string());
        FaceSelector {
            resolved_families,
            requested_weight,
            requested_style,
        }
    }

    fn structured_selector(
        &self,
        typography: &ThemeTextStyle,
        budget: &mut StructuredShapingBudget<'_>,
    ) -> Result<FaceSelector, TextLayoutError> {
        let families = typography.font_stack().families();
        budget.record_candidate_compilation(families.len())?;
        let mut resolved_families = Vec::with_capacity(families.len());
        for family in families {
            budget.record_candidate_compilation(family.len())?;
            resolved_families.push(
                self.catalog
                    .canonical_named_family_name(family)
                    .map(normalize_family)
                    .unwrap_or_else(|| normalize_family(family)),
            );
        }
        let family_face_comparisons = resolved_families
            .len()
            .checked_mul(self.faces.len())
            .ok_or(TextLayoutError::LimitExceeded("candidate_compilation"))?;
        budget.record_candidate_compilation(family_face_comparisons)?;
        let has_catalog_family = resolved_families
            .iter()
            .any(|family| self.faces.iter().any(|face| &face.family_key == family));
        if !has_catalog_family {
            return Err(TextLayoutError::FontFamilyUnavailable);
        }
        budget.record_candidate_compilation(1)?;
        Ok(FaceSelector {
            resolved_families,
            requested_weight: typography.font_weight(),
            requested_style: typography.font_style().id().to_string(),
        })
    }

    fn compile_structured_request(
        &self,
        request: &PrepareTextRequest,
        budget: &mut StructuredShapingBudget<'_>,
    ) -> Result<CompiledStructuredTextRequest, TextLayoutError> {
        let features = request
            .features()
            .map(|feature| {
                budget.record_candidate_compilation(1usize.saturating_add(feature.len()))?;
                Feature::from_str(feature).map_err(|_| TextLayoutError::InvalidFeature)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let variations = request
            .variations()
            .map(|variation| {
                budget.record_candidate_compilation(1usize.saturating_add(variation.len()))?;
                Variation::from_str(variation).map_err(|_| TextLayoutError::InvalidVariation)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let requested_script = request
            .script()
            .map(|value| {
                budget.record_candidate_compilation(1usize.saturating_add(value.len()))?;
                BuzzScript::from_str(value).map_err(|_| TextLayoutError::InvalidRequest("script"))
            })
            .transpose()?;
        let language = request
            .language()
            .map(|value| {
                budget.record_candidate_compilation(1usize.saturating_add(value.len()))?;
                Language::from_str(value).map_err(|_| TextLayoutError::InvalidRequest("language"))
            })
            .transpose()?;
        let wrapping = self.compile_structured_style(request.wrapping_typography(), budget)?;
        let mut metrics = if request.metrics_typography() == request.wrapping_typography() {
            wrapping.clone()
        } else {
            self.compile_structured_style(request.metrics_typography(), budget)?
        };
        if metrics.candidate_faces != wrapping.candidate_faces {
            metrics.coverage_lane = 1;
        }
        Ok(CompiledStructuredTextRequest {
            features,
            variations,
            requested_script,
            language,
            wrapping,
            metrics,
        })
    }

    fn compile_structured_style(
        &self,
        typography: &ThemeTextStyle,
        budget: &mut StructuredShapingBudget<'_>,
    ) -> Result<CompiledStructuredTextStyle, TextLayoutError> {
        let selector = self.structured_selector(typography, budget)?;
        let candidate_faces = self.strict_candidate_face_indices(&selector, budget)?;
        if candidate_faces.is_empty() {
            return Err(TextLayoutError::FontFamilyUnavailable);
        }
        Ok(CompiledStructuredTextStyle {
            candidate_faces: candidate_faces.into(),
            coverage_lane: 0,
        })
    }

    fn candidate_face_indices(&self, selector: &FaceSelector) -> Vec<usize> {
        let score = |face: &PreparedFace| {
            let family_rank = if selector.resolved_families.is_empty() {
                0
            } else {
                selector
                    .resolved_families
                    .iter()
                    .position(|family| family == &face.family_key)
                    .unwrap_or(selector.resolved_families.len() + 1)
            };
            (
                family_rank,
                usize::from(face.metadata.style().id() != selector.requested_style),
                face.metadata.weight().abs_diff(selector.requested_weight),
                face.metadata.width(),
            )
        };

        let mut candidates = self
            .faces
            .iter()
            .enumerate()
            .map(|(index, face)| (index, score(face)))
            .collect::<Vec<_>>();
        candidates.sort_by_key(|(_, score)| *score);
        if !selector.resolved_families.is_empty()
            && candidates
                .first()
                .is_some_and(|(_, score)| score.0 > selector.resolved_families.len())
        {
            return Vec::new();
        }
        candidates.into_iter().map(|(index, _)| index).collect()
    }

    fn strict_candidate_face_indices(
        &self,
        selector: &FaceSelector,
        budget: &mut StructuredShapingBudget<'_>,
    ) -> Result<Vec<usize>, TextLayoutError> {
        let face_count = self.faces.len();
        let family_count = selector.resolved_families.len();
        let family_comparisons = face_count
            .checked_mul(family_count)
            .ok_or(TextLayoutError::LimitExceeded("candidate_compilation"))?;
        let sort_comparisons = face_count
            .checked_mul(usize::BITS as usize - face_count.max(1).leading_zeros() as usize)
            .ok_or(TextLayoutError::LimitExceeded("candidate_compilation"))?;
        budget.record_candidate_compilation(
            family_comparisons
                .checked_add(face_count)
                .and_then(|units| units.checked_add(sort_comparisons))
                .ok_or(TextLayoutError::LimitExceeded("candidate_compilation"))?,
        )?;
        let mut candidates = self
            .faces
            .iter()
            .enumerate()
            .filter_map(|(index, face)| {
                let family_rank = selector
                    .resolved_families
                    .iter()
                    .position(|family| family == &face.family_key)?;
                Some((
                    index,
                    (
                        family_rank,
                        usize::from(face.metadata.style().id() != selector.requested_style),
                        face.metadata.weight().abs_diff(selector.requested_weight),
                        face.metadata.width().abs_diff(5),
                    ),
                ))
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|(_, score)| *score);
        Ok(candidates.into_iter().map(|(index, _)| index).collect())
    }

    fn choose_face_index(&self, text: &str, selector: &FaceSelector) -> Option<usize> {
        let script = self.script.or_else(|| cluster_script(text));
        // Keep legacy measurement on the same cluster-shaping coverage rule as prepared text.
        // A separate scalar CMap cache cannot prove shaped coverage and grows across labels.
        self.candidate_face_indices(selector)
            .into_iter()
            .find(|index| {
                self.face_shapes_cluster(
                    *index,
                    text,
                    self.direction,
                    script,
                    self.language.clone(),
                    &self.features,
                    &self.variations,
                )
                .unwrap_or(false)
            })
    }

    fn choose_structured_face_index(
        &self,
        text: &str,
        span_index: usize,
        coverage_lane: usize,
        style: &CompiledStructuredTextStyle,
        request: &PrepareTextRequest,
        compiled: &CompiledStructuredTextRequest,
        script: Option<BuzzScript>,
        coverage_cache: &mut StructuredCoverageCache,
        budget: &mut StructuredShapingBudget<'_>,
    ) -> Result<Option<usize>, TextLayoutError> {
        if let Some(face_index) = coverage_cache.get(span_index, coverage_lane)? {
            return Ok(face_index);
        }

        let mut selected = None;
        for index in style.candidate_faces.iter().copied() {
            budget.record_face_inspection()?;
            budget.record(StructuredShapingPass::Coverage, text.len())?;
            let covers = self.face_shapes_cluster(
                index,
                text,
                request.direction(),
                script,
                compiled.language.clone(),
                &compiled.features,
                &compiled.variations,
            )?;
            if covers {
                selected = Some(index);
                break;
            }
        }
        budget.record_coverage_cache_insertion()?;
        coverage_cache.insert(span_index, coverage_lane, selected)?;
        Ok(selected)
    }

    #[allow(clippy::too_many_arguments)]
    fn face_shapes_cluster(
        &self,
        face_index: usize,
        text: &str,
        direction: TextLayoutDirection,
        script: Option<BuzzScript>,
        language: Option<Language>,
        features: &[Feature],
        variations: &[Variation],
    ) -> Result<bool, TextLayoutError> {
        let face = &self.faces[face_index];
        let mut font = rustybuzz::Face::from_slice(&face.data, face.face_index)
            .ok_or(TextLayoutError::NoUsableFace)?;
        font.set_variations(variations);
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.guess_segment_properties();
        if let Some(direction) = direction.to_rustybuzz() {
            buffer.set_direction(direction);
        }
        if let Some(script) = script {
            buffer.set_script(script);
        }
        if let Some(language) = language {
            buffer.set_language(language);
        }
        let glyphs = rustybuzz::shape(&font, features, buffer);
        Ok(shaped_glyph_coverage_is_complete(
            text,
            glyphs.glyph_infos().iter().map(|info| info.glyph_id),
        ))
    }

    fn resolve_family<'a>(&'a self, family: &'a str) -> String {
        self.catalog
            .canonical_family_name(family)
            .map(normalize_family)
            .unwrap_or_else(|| family.to_string())
    }

    fn shape_line(&self, text: &str, style: &TextStyle) -> ShapedLineMetrics {
        if text.is_empty() {
            return ShapedLineMetrics::default();
        }
        let selector = self.selector(style);
        let mut total = ShapedLineMetrics::default();
        let mut offset_x = 0.0;
        let mut current_face = None;
        let mut current_script = None;
        let mut run = String::new();
        for cluster in text.graphemes(true) {
            let face_index = self.choose_face_index(cluster, &selector);
            let script = self
                .script
                .or_else(|| cluster_script(cluster).or(current_script));
            if (face_index != current_face || script != current_script) && !run.is_empty() {
                if let Some(face_index) = current_face {
                    let shaped = self.shape_run(face_index, current_script, &run, style);
                    total.include_run(shaped, offset_x);
                    offset_x += shaped.advance;
                }
                run.clear();
            }
            current_face = face_index;
            current_script = script;
            run.push_str(cluster);
        }
        if let Some(face_index) = current_face {
            let shaped = self.shape_run(face_index, current_script, &run, style);
            total.include_run(shaped, offset_x);
        }
        total
    }

    fn shape_structured_line_with_evidence(
        &self,
        text: &str,
        line_range: TextByteRange,
        projection: &TextProjection,
        request: &PrepareTextRequest,
        typography: &ThemeTextStyle,
        compiled: &CompiledStructuredTextRequest,
        style: &CompiledStructuredTextStyle,
        coverage_cache: &mut StructuredCoverageCache,
        span_cursor: &mut ProjectionSpanCursor,
        budget: &mut StructuredShapingBudget<'_>,
        response_builder: &mut PreparedTextResponseBuilder,
    ) -> Result<ShapedStructuredLine, TextLayoutError> {
        self.shape_structured_line_internal(
            text,
            projection,
            line_range,
            request,
            typography,
            compiled,
            style,
            true,
            StructuredShapingPass::Metrics,
            coverage_cache,
            span_cursor,
            budget,
            Some(response_builder),
        )
    }

    fn shape_structured_line_internal(
        &self,
        text: &str,
        projection: &TextProjection,
        line_range: TextByteRange,
        request: &PrepareTextRequest,
        typography: &ThemeTextStyle,
        compiled: &CompiledStructuredTextRequest,
        style: &CompiledStructuredTextStyle,
        emit_evidence: bool,
        pass: StructuredShapingPass,
        coverage_cache: &mut StructuredCoverageCache,
        span_cursor: &mut ProjectionSpanCursor,
        budget: &mut StructuredShapingBudget<'_>,
        mut response_builder: Option<&mut PreparedTextResponseBuilder>,
    ) -> Result<ShapedStructuredLine, TextLayoutError> {
        let line_visible_start = line_range.start();
        let line_visible_end = line_range.end();
        if line_visible_end.saturating_sub(line_visible_start) != text.len()
            || !projection.visible().is_char_boundary(line_visible_start)
            || !projection.visible().is_char_boundary(line_visible_end)
        {
            return Err(TextLayoutError::InvalidPreparedText);
        }
        let span_range = span_cursor.range_for_line(projection, line_range, pass, budget)?;
        if text.is_empty() {
            return Ok(ShapedStructuredLine {
                metrics: ShapedLineMetrics::default(),
                clusters: Vec::new(),
                span_range,
            });
        }

        let mut total = ShapedLineMetrics::default();
        let mut offset_x = 0.0;
        let mut current_face = None;
        let mut current_script = None;
        let mut run = String::new();
        let mut run_start = 0;
        let mut clusters = Vec::new();
        let mut atom_cursor = 0;
        for span_index in span_range.clone() {
            let span = &projection.spans()[span_index];
            let visible = span.visible();
            if visible.start() < line_visible_start || visible.end() > line_visible_end {
                return Err(TextLayoutError::InvalidPreparedText);
            }
            if visible.start() == visible.end() {
                continue;
            }
            let atom_start = visible.start() - line_visible_start;
            let atom_end = visible.end() - line_visible_start;
            if atom_start != atom_cursor || atom_end <= atom_start {
                return Err(TextLayoutError::InvalidPreparedText);
            }
            let atom = text
                .get(atom_start..atom_end)
                .ok_or(TextLayoutError::InvalidPreparedText)?;
            let atom_script = cluster_script(atom);
            let script = compiled
                .requested_script
                .or_else(|| atom_script.or(current_script));
            let context_dependent_script =
                compiled.requested_script.is_none() && atom_script.is_none();
            let coverage_lane = usize::from(
                matches!(pass, StructuredShapingPass::Metrics)
                    && (style.coverage_lane != 0 || context_dependent_script),
            );
            let face_index = self
                .choose_structured_face_index(
                    atom,
                    span_index,
                    coverage_lane,
                    style,
                    request,
                    compiled,
                    script,
                    coverage_cache,
                    budget,
                )?
                .ok_or(TextLayoutError::GlyphUnavailable)?;
            if (Some(face_index) != current_face || script != current_script) && !run.is_empty() {
                let current_face = current_face.expect("non-empty run has a face");
                let shaped = self.shape_structured_run(
                    current_face,
                    current_script,
                    &run,
                    request,
                    typography,
                    compiled,
                    pass,
                    budget,
                )?;
                let run_metrics = shaped.metrics;
                total.include_run(run_metrics, offset_x);
                clusters.extend(
                    shaped
                        .clusters
                        .into_iter()
                        .map(|cluster| cluster.offset(run_start, offset_x)),
                );
                offset_x += run_metrics.advance;
                if emit_evidence {
                    response_builder
                        .as_deref_mut()
                        .expect("evidence shaping requires a response builder")
                        .push_run(PreparedTextRunResponse::new(
                            TextByteRange::new(
                                line_visible_start + run_start,
                                line_visible_start + atom_start,
                            ),
                            PreparedTextFaceSlot::new(
                                u32::try_from(current_face)
                                    .expect("prepared catalog face count fits in a face slot"),
                            ),
                            self.font_source,
                        ))?;
                }
                run.clear();
                run_start = atom_start;
            } else if run.is_empty() {
                run_start = atom_start;
            }
            current_face = Some(face_index);
            current_script = script;
            run.push_str(atom);
            atom_cursor = atom_end;
        }
        if atom_cursor != text.len() {
            return Err(TextLayoutError::InvalidPreparedText);
        }
        if let Some(face_index) = current_face {
            let shaped = self.shape_structured_run(
                face_index,
                current_script,
                &run,
                request,
                typography,
                compiled,
                pass,
                budget,
            )?;
            let run_metrics = shaped.metrics;
            total.include_run(run_metrics, offset_x);
            clusters.extend(
                shaped
                    .clusters
                    .into_iter()
                    .map(|cluster| cluster.offset(run_start, offset_x)),
            );
            if emit_evidence {
                response_builder
                    .as_deref_mut()
                    .expect("evidence shaping requires a response builder")
                    .push_run(PreparedTextRunResponse::new(
                        TextByteRange::new(
                            line_visible_start + run_start,
                            line_visible_start + text.len(),
                        ),
                        PreparedTextFaceSlot::new(
                            u32::try_from(face_index)
                                .expect("prepared catalog face count fits in a face slot"),
                        ),
                        self.font_source,
                    ))?;
            }
        }

        let grapheme_count = text.graphemes(true).count();
        let letter_spacing =
            f64::from(typography.letter_spacing_px()) * grapheme_count.saturating_sub(1) as f64;
        let word_spacing = f64::from(typography.word_spacing_px())
            * text.chars().filter(|character| *character == ' ').count() as f64;
        total.advance += letter_spacing + word_spacing;
        Ok(ShapedStructuredLine {
            metrics: total,
            clusters,
            span_range,
        })
    }

    fn shape_structured_run(
        &self,
        face_index: usize,
        script: Option<BuzzScript>,
        text: &str,
        request: &PrepareTextRequest,
        typography: &ThemeTextStyle,
        compiled: &CompiledStructuredTextRequest,
        pass: StructuredShapingPass,
        budget: &mut StructuredShapingBudget<'_>,
    ) -> Result<ShapedStructuredRun, TextLayoutError> {
        budget.record(pass, text.len())?;
        let face = &self.faces[face_index];
        let mut font = rustybuzz::Face::from_slice(&face.data, face.face_index)
            .ok_or(TextLayoutError::NoUsableFace)?;
        font.set_variations(&compiled.variations);
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.guess_segment_properties();
        if let Some(direction) = request.direction().to_rustybuzz() {
            buffer.set_direction(direction);
        }
        if let Some(script) = script {
            buffer.set_script(script);
        }
        if let Some(language) = compiled.language.clone() {
            buffer.set_language(language);
        }
        let glyph_direction = buffer.direction();
        let glyphs = rustybuzz::shape(&font, &compiled.features, buffer);
        let glyph_infos = glyphs.glyph_infos();
        if glyph_infos.is_empty() && !text.chars().all(is_default_ignorable) {
            return Err(TextLayoutError::GlyphUnavailable);
        }
        let units_per_em = f64::from(font.units_per_em().max(1));
        let scale = f64::from(typography.font_size_px()) / units_per_em;
        let mut metrics = ShapedLineMetrics::default();
        let backward = matches!(
            glyph_direction,
            BuzzDirection::RightToLeft | BuzzDirection::BottomToTop
        );
        budget.record_glyph_visits(glyph_infos.len())?;
        let mut glyph_order_cluster_starts = Vec::with_capacity(glyph_infos.len().min(text.len()));
        let mut previous_cluster_start = None;
        for info in glyph_infos {
            if info.glyph_id == 0 {
                return Err(TextLayoutError::GlyphUnavailable);
            }
            let cluster_start =
                usize::try_from(info.cluster).map_err(|_| TextLayoutError::InvalidPreparedText)?;
            if cluster_start >= text.len() || !text.is_char_boundary(cluster_start) {
                return Err(TextLayoutError::InvalidPreparedText);
            }
            if let Some(previous) = previous_cluster_start {
                let monotonic = if backward {
                    cluster_start <= previous
                } else {
                    cluster_start >= previous
                };
                if !monotonic {
                    return Err(TextLayoutError::InvalidPreparedText);
                }
            }
            if previous_cluster_start != Some(cluster_start) {
                glyph_order_cluster_starts.push(cluster_start);
                previous_cluster_start = Some(cluster_start);
            }
        }
        if backward {
            budget.record_cluster_visits(glyph_order_cluster_starts.len())?;
            glyph_order_cluster_starts.reverse();
        }
        let mut cluster_starts = Vec::with_capacity(
            glyph_order_cluster_starts
                .len()
                .saturating_add(usize::from(glyph_order_cluster_starts.first() != Some(&0))),
        );
        if glyph_order_cluster_starts.first() != Some(&0) {
            cluster_starts.push(0);
        }
        cluster_starts.extend(glyph_order_cluster_starts);
        budget.record_cluster_visits(cluster_starts.len().saturating_sub(1))?;
        for window in cluster_starts.windows(2) {
            if window[0] >= window[1] {
                return Err(TextLayoutError::InvalidPreparedText);
            }
        }
        budget.record_cluster_visits(cluster_starts.len())?;
        let mut clusters = Vec::with_capacity(cluster_starts.len());
        for (index, start) in cluster_starts.iter().copied().enumerate() {
            clusters.push(ShapedClusterMetric {
                range: TextByteRange::new(
                    start,
                    cluster_starts.get(index + 1).copied().unwrap_or(text.len()),
                ),
                advance: 0.0,
                min_x: 0.0,
                max_x: 0.0,
                has_bounds: false,
            });
        }
        let mut pen_x = 0.0;
        let first_glyph_start = glyph_infos
            .first()
            .and_then(|info| usize::try_from(info.cluster).ok());
        let mut cluster_index = if backward {
            cluster_starts.len().saturating_sub(1)
        } else if first_glyph_start == Some(0) {
            0
        } else {
            usize::from(!cluster_starts.is_empty())
        };
        let mut active_glyph_cluster = None;
        budget.record_glyph_visits(glyph_infos.len())?;
        for (info, position) in glyph_infos.iter().zip(glyphs.glyph_positions()) {
            let x_advance = f64::from(position.x_advance) * scale;
            let glyph_id = ttf_parser::GlyphId(info.glyph_id as u16);
            let glyph_bounds = font.glyph_bounding_box(glyph_id).map(|bounds| {
                let origin_x = pen_x + f64::from(position.x_offset) * scale;
                let origin_y = f64::from(position.y_offset) * scale;
                let min_x = origin_x + f64::from(bounds.x_min) * scale;
                let max_x = origin_x + f64::from(bounds.x_max) * scale;
                let min_y = origin_y + f64::from(bounds.y_min) * scale;
                let max_y = origin_y + f64::from(bounds.y_max) * scale;
                if !metrics.has_bounds {
                    metrics.min_x = min_x;
                    metrics.max_x = max_x;
                    metrics.min_y = min_y;
                    metrics.max_y = max_y;
                    metrics.has_bounds = true;
                } else {
                    metrics.min_x = metrics.min_x.min(min_x);
                    metrics.max_x = metrics.max_x.max(max_x);
                    metrics.min_y = metrics.min_y.min(min_y);
                    metrics.max_y = metrics.max_y.max(max_y);
                }
                (min_x, max_x)
            });
            let cluster_start =
                usize::try_from(info.cluster).map_err(|_| TextLayoutError::InvalidPreparedText)?;
            if active_glyph_cluster != Some(cluster_start) {
                if active_glyph_cluster.is_some() {
                    cluster_index = if backward {
                        cluster_index
                            .checked_sub(1)
                            .ok_or(TextLayoutError::InvalidPreparedText)?
                    } else {
                        cluster_index
                            .checked_add(1)
                            .ok_or(TextLayoutError::InvalidPreparedText)?
                    };
                }
                if cluster_starts.get(cluster_index) != Some(&cluster_start) {
                    return Err(TextLayoutError::InvalidPreparedText);
                }
                active_glyph_cluster = Some(cluster_start);
            }
            clusters[cluster_index].include_glyph(x_advance, glyph_bounds);
            pen_x += x_advance;
        }
        metrics.advance = pen_x;
        Ok(ShapedStructuredRun { metrics, clusters })
    }

    fn structured_line_height(&self, typography: &ThemeTextStyle) -> f64 {
        let font_size = f64::from(typography.font_size_px()).max(0.1);
        match typography.line_height() {
            LineHeight::Normal => font_size * 1.2,
            LineHeight::Multiplier(value) => font_size * f64::from(value),
            LineHeight::Px(value) => f64::from(value),
        }
    }

    fn shape_wrap_line(
        &self,
        source: ProjectedVisibleLine,
        projection: &TextProjection,
        request: &PrepareTextRequest,
        compiled: &CompiledStructuredTextRequest,
        coverage_cache: &mut StructuredCoverageCache,
        span_cursor: &mut ProjectionSpanCursor,
        budget: &mut StructuredShapingBudget<'_>,
    ) -> Result<ShapedWrapLine, TextLayoutError> {
        let text = source.text(projection.visible())?;
        let shaped = self.shape_structured_line_internal(
            text,
            projection,
            source.visible_range(),
            request,
            request.wrapping_typography(),
            compiled,
            &compiled.wrapping,
            false,
            StructuredShapingPass::Wrapping,
            coverage_cache,
            span_cursor,
            budget,
            None,
        )?;
        let atoms = coalesce_wrap_atoms(
            text,
            source.visible_range().start(),
            projection,
            shaped.span_range,
            &shaped.clusters,
            budget,
        )?;
        let mut measure = WrapMeasure::default();
        budget.record_wrap_boundary_visits(atoms.len())?;
        for atom in &atoms {
            measure.include(atom.measure);
        }
        Ok(ShapedWrapLine { atoms, measure })
    }

    fn wrap_svg_structured_line(
        &self,
        source: ProjectedVisibleLine,
        projection: &TextProjection,
        shaped: &ShapedWrapLine,
        typography: &ThemeTextStyle,
        max_width_px: f64,
        break_long_words: bool,
        wrapped: &mut Vec<ProjectedVisibleLine>,
        budget: &mut StructuredShapingBudget<'_>,
    ) -> Result<(), TextLayoutError> {
        let line = source.text(projection.visible())?;
        if !max_width_px.is_finite() || max_width_px <= 0.0 || line.is_empty() {
            return push_projected_visible_line(wrapped, source, budget);
        }
        budget.record_wrap_boundary_visits(line.len())?;
        let ranges = svg_wrap_segment_ranges(line);
        let segments = align_wrap_segments(&shaped.atoms, &ranges, budget)?;
        let output_start = wrapped.len();
        let mut current = WrapLineState::default();
        let mut segment_index = 0;
        while let Some(segment) = segments.get(segment_index).copied() {
            budget.record_wrap_boundary_visits(1)?;
            let whitespace = segment_is_whitespace(segment, &shaped.atoms, budget)?;
            if current.is_empty() && whitespace {
                segment_index += 1;
                continue;
            }
            let mut candidate = current;
            budget
                .record_wrap_boundary_visits(segment.atom_end.saturating_sub(segment.atom_start))?;
            candidate.append_segment(segment, &shaped.atoms);
            if candidate.width(typography) <= max_width_px {
                current = candidate;
                segment_index += 1;
                continue;
            }
            if current.has_visible_content() {
                push_wrapped_state(wrapped, source, current, &shaped.atoms, budget)?;
                current = WrapLineState::default();
                continue;
            }
            if whitespace {
                segment_index += 1;
                continue;
            }
            if !break_long_words {
                current = candidate;
                segment_index += 1;
                continue;
            }

            for atom_index in segment.atom_start..segment.atom_end {
                budget.record_wrap_boundary_visits(1)?;
                let mut candidate = current;
                candidate.append_atom(atom_index, shaped.atoms[atom_index]);
                if current.has_visible_content() && candidate.width(typography) > max_width_px {
                    push_wrapped_state(wrapped, source, current, &shaped.atoms, budget)?;
                    current = WrapLineState::default();
                }
                current.append_atom(atom_index, shaped.atoms[atom_index]);
            }
            segment_index += 1;
        }
        if current.has_visible_content() || wrapped.len() == output_start {
            push_wrapped_state(wrapped, source, current, &shaped.atoms, budget)?;
        }
        Ok(())
    }

    fn wrap_html_structured_line(
        &self,
        source: ProjectedVisibleLine,
        projection: &TextProjection,
        shaped: &ShapedWrapLine,
        typography: &ThemeTextStyle,
        max_width_px: f64,
        wrapped: &mut Vec<ProjectedVisibleLine>,
        budget: &mut StructuredShapingBudget<'_>,
    ) -> Result<(), TextLayoutError> {
        let line = source.text(projection.visible())?;
        if !max_width_px.is_finite() || max_width_px <= 0.0 || line.is_empty() {
            return push_projected_visible_line(wrapped, source, budget);
        }
        budget.record_wrap_boundary_visits(line.len())?;
        let ranges = html_wrap_segment_ranges(line);
        let segments = align_wrap_segments(&shaped.atoms, &ranges, budget)?;
        let output_start = wrapped.len();
        let mut current = WrapLineState::default();
        for segment in segments {
            budget.record_wrap_boundary_visits(
                1usize.saturating_add(segment.atom_end.saturating_sub(segment.atom_start)),
            )?;
            let mut candidate = current;
            candidate.append_segment(segment, &shaped.atoms);
            if current.is_empty() || candidate.width(typography) <= max_width_px {
                current = candidate;
            } else {
                push_wrapped_state(wrapped, source, current, &shaped.atoms, budget)?;
                current = WrapLineState::default();
                current.append_segment(segment, &shaped.atoms);
            }
        }
        if !current.is_empty() || wrapped.len() == output_start {
            push_wrapped_state(wrapped, source, current, &shaped.atoms, budget)?;
        }
        Ok(())
    }

    fn prepare_structured_text(
        &self,
        backend_request: &PreparedTextBackendRequest,
    ) -> Result<PreparedTextResponse, TextLayoutError> {
        self.prepare_structured_text_with_work(backend_request)
            .map(|(response, _)| response)
    }

    fn prepare_structured_text_with_work(
        &self,
        backend_request: &PreparedTextBackendRequest,
    ) -> Result<(PreparedTextResponse, StructuredShapingWork), TextLayoutError> {
        let (response, work) = self.prepare_structured_text_attempt(backend_request);
        Ok((response?, work))
    }

    fn prepare_structured_text_attempt(
        &self,
        backend_request: &PreparedTextBackendRequest,
    ) -> (
        Result<PreparedTextResponse, TextLayoutError>,
        StructuredShapingWork,
    ) {
        self.prepare_structured_text_attempt_internal(backend_request, None)
    }

    fn prepare_structured_text_attempt_with_work_meter(
        &self,
        backend_request: &PreparedTextBackendRequest,
        work_meter: &OperationWorkMeter,
    ) -> (
        Result<PreparedTextResponse, TextLayoutError>,
        StructuredShapingWork,
    ) {
        self.prepare_structured_text_attempt_internal(backend_request, Some(work_meter))
    }

    fn prepare_structured_text_attempt_internal(
        &self,
        backend_request: &PreparedTextBackendRequest,
        work_meter: Option<&OperationWorkMeter>,
    ) -> (
        Result<PreparedTextResponse, TextLayoutError>,
        StructuredShapingWork,
    ) {
        let mut budget = StructuredShapingBudget::new(work_meter);
        let response = (|| {
            let request = backend_request.request();
            let projection = backend_request.projection();
            let compiled = self.compile_structured_request(request, &mut budget)?;
            let coverage_cache_slots = projection
                .spans()
                .len()
                .checked_mul(STRUCTURED_COVERAGE_CACHE_LANES)
                .ok_or(TextLayoutError::LimitExceeded("coverage_cache"))?;
            budget.record_coverage_cache_slots(coverage_cache_slots)?;
            let mut coverage_cache = StructuredCoverageCache::new(projection.spans().len())?;
            let mut wrapping_span_cursor = ProjectionSpanCursor::default();
            budget.record_source_line_scan(projection.visible().len())?;
            let mut wrapped_lines = Vec::new();
            let mut raw_width_px: f64 = 0.0;
            for source in projected_visible_lines(projection.visible()) {
                budget.record_source_line_visit()?;
                match request.wrap() {
                    PreparedTextWrap::SingleRun
                    | PreparedTextWrap::SvgLike {
                        max_width_px: None, ..
                    } => {
                        push_projected_visible_line(&mut wrapped_lines, source, &mut budget)?;
                    }
                    PreparedTextWrap::SvgLike {
                        max_width_px: Some(max_width_px),
                        break_long_words,
                    } => {
                        let shaped = self.shape_wrap_line(
                            source,
                            projection,
                            request,
                            &compiled,
                            &mut coverage_cache,
                            &mut wrapping_span_cursor,
                            &mut budget,
                        )?;
                        self.wrap_svg_structured_line(
                            source,
                            projection,
                            &shaped,
                            request.wrapping_typography(),
                            max_width_px,
                            break_long_words,
                            &mut wrapped_lines,
                            &mut budget,
                        )?;
                    }
                    PreparedTextWrap::HtmlLike { max_width_px } => {
                        let shaped = self.shape_wrap_line(
                            source,
                            projection,
                            request,
                            &compiled,
                            &mut coverage_cache,
                            &mut wrapping_span_cursor,
                            &mut budget,
                        )?;
                        raw_width_px =
                            raw_width_px.max(shaped.measure.width(request.wrapping_typography()));
                        if let Some(max_width_px) = max_width_px {
                            self.wrap_html_structured_line(
                                source,
                                projection,
                                &shaped,
                                request.wrapping_typography(),
                                max_width_px,
                                &mut wrapped_lines,
                                &mut budget,
                            )?;
                        } else {
                            push_projected_visible_line(&mut wrapped_lines, source, &mut budget)?;
                        }
                    }
                }
            }
            let line_height_px = self.structured_line_height(request.metrics_typography());
            let raw_width_px =
                matches!(request.wrap(), PreparedTextWrap::HtmlLike { .. }).then_some(raw_width_px);
            let mut response_builder = PreparedTextResponseBuilder::new(
                backend_request.binding().clone(),
                line_height_px,
                raw_width_px,
                prepared_text_request_response_budget(projection.visible().len()),
            )?;
            let mut metrics_span_cursor = ProjectionSpanCursor::default();
            for line in &wrapped_lines {
                let text = line.text(projection.visible())?;
                response_builder.reserve_line(text.len())?;
                let shaped = self.shape_structured_line_with_evidence(
                    text,
                    line.visible_range(),
                    projection,
                    request,
                    request.metrics_typography(),
                    &compiled,
                    &compiled.metrics,
                    &mut coverage_cache,
                    &mut metrics_span_cursor,
                    &mut budget,
                    &mut response_builder,
                )?;
                response_builder.push_reserved_line(PreparedTextLineResponse::new(
                    text,
                    line.visible_range(),
                    shaped.metrics.advance.abs(),
                    shaped.metrics.bbox_x(),
                    shaped.metrics.vertical_extents()?,
                )?);
            }
            response_builder.finish()
        })();
        (response, budget.work)
    }

    fn shape_run(
        &self,
        face_index: usize,
        script: Option<BuzzScript>,
        text: &str,
        style: &TextStyle,
    ) -> ShapedLineMetrics {
        let face = &self.faces[face_index];
        let Some(mut font) = rustybuzz::Face::from_slice(&face.data, face.face_index) else {
            return ShapedLineMetrics::default();
        };
        font.set_variations(&self.variations);
        let mut buffer = UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.guess_segment_properties();
        if let Some(direction) = self.direction.to_rustybuzz() {
            buffer.set_direction(direction);
        }
        if let Some(script) = script {
            buffer.set_script(script);
        }
        if let Some(language) = self.language.clone() {
            buffer.set_language(language);
        }
        let glyphs = rustybuzz::shape(&font, &self.features, buffer);
        let units_per_em = f64::from(font.units_per_em().max(1));
        let scale = f64::from(style.font_size.max(0.1)) / units_per_em;
        let mut metrics = ShapedLineMetrics::default();
        let mut pen_x = 0.0;
        for (info, position) in glyphs.glyph_infos().iter().zip(glyphs.glyph_positions()) {
            let x_advance = f64::from(position.x_advance) * scale;
            let glyph_id = ttf_parser::GlyphId(info.glyph_id as u16);
            if let Some(bounds) = font.glyph_bounding_box(glyph_id) {
                let origin_x = pen_x + f64::from(position.x_offset) * scale;
                let origin_y = f64::from(position.y_offset) * scale;
                let min_x = origin_x + f64::from(bounds.x_min) * scale;
                let max_x = origin_x + f64::from(bounds.x_max) * scale;
                let min_y = origin_y + f64::from(bounds.y_min) * scale;
                let max_y = origin_y + f64::from(bounds.y_max) * scale;
                if !metrics.has_bounds {
                    metrics.min_x = min_x;
                    metrics.max_x = max_x;
                    metrics.min_y = min_y;
                    metrics.max_y = max_y;
                    metrics.has_bounds = true;
                } else {
                    metrics.min_x = metrics.min_x.min(min_x);
                    metrics.max_x = metrics.max_x.max(max_x);
                    metrics.min_y = metrics.min_y.min(min_y);
                    metrics.max_y = metrics.max_y.max(max_y);
                }
            }
            pen_x += x_advance;
        }
        metrics.advance = pen_x.abs();
        metrics
    }

    fn line_height(&self, style: &TextStyle, wrap_mode: WrapMode) -> f64 {
        let factor = match wrap_mode {
            WrapMode::HtmlLike => 1.5,
            WrapMode::SvgLike | WrapMode::SvgLikeSingleRun => 1.1,
        };
        (style.font_size.max(1.0) * factor).max(1.0)
    }

    fn line_metrics(
        &self,
        lines: &[String],
        style: &TextStyle,
        wrap_mode: WrapMode,
    ) -> TextMetrics {
        let width = lines
            .iter()
            .map(|line| self.shape_line(line, style).layout_width())
            .fold(0.0, f64::max);
        TextMetrics {
            width,
            height: self.line_height(style, wrap_mode) * lines.len().max(1) as f64,
            line_count: lines.len().max(1),
        }
    }

    fn wrap(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> (TextMetrics, Option<f64>) {
        let raw_lines = split_html_br_lines(text)
            .into_iter()
            .flat_map(|line| line.split('\n'))
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        let raw_metrics = self.line_metrics(&raw_lines, style, wrap_mode);
        let Some(max_width) = max_width.filter(|width| width.is_finite() && *width > 0.0) else {
            return (
                raw_metrics,
                (wrap_mode == WrapMode::HtmlLike).then_some(raw_metrics.width),
            );
        };

        let lines = match wrap_mode {
            WrapMode::SvgLikeSingleRun => raw_lines,
            WrapMode::SvgLike => raw_lines
                .iter()
                .flat_map(|line| {
                    crate::text::wrap_label_like_mermaid_lines(line, self, style, max_width)
                })
                .collect::<Vec<_>>(),
            WrapMode::HtmlLike => {
                crate::text::wrap_text_lines_measurer(text, self, style, Some(max_width))
            }
        };
        let mut metrics = self.line_metrics(&lines, style, wrap_mode);
        if wrap_mode == WrapMode::HtmlLike {
            let needs_wrap = raw_metrics.width > max_width;
            metrics.width = if needs_wrap {
                metrics.width.max(max_width)
            } else {
                metrics.width.min(max_width)
            };
        }
        (
            metrics,
            (wrap_mode == WrapMode::HtmlLike).then_some(raw_metrics.width),
        )
    }
}

impl PreparedTextBackendSession for NativeCatalogTextMeasurer {
    fn prepare_text(
        &self,
        request: &PreparedTextBackendRequest,
    ) -> Result<PreparedTextResponse, TextLayoutError> {
        self.prepare_structured_text(request)
    }
}

impl TextMeasurer for NativeCatalogTextMeasurer {
    fn measure(&self, text: &str, style: &TextStyle) -> TextMetrics {
        self.wrap(text, style, None, WrapMode::SvgLike).0
    }

    fn measure_svg_text_computed_length_px(&self, text: &str, style: &TextStyle) -> f64 {
        text.split('\n')
            .map(|line| self.shape_line(line, style).advance)
            .fold(0.0, f64::max)
    }

    fn measure_svg_text_bbox_x(&self, text: &str, style: &TextStyle) -> (f64, f64) {
        text.split('\n')
            .map(|line| self.shape_line(line, style).bbox_x())
            .fold(
                (0.0_f64, 0.0_f64),
                |(left, right), (line_left, line_right)| {
                    (left.max(line_left), right.max(line_right))
                },
            )
    }

    fn measure_svg_text_bbox_x_with_ascii_overhang(
        &self,
        text: &str,
        style: &TextStyle,
    ) -> (f64, f64) {
        self.measure_svg_text_bbox_x(text, style)
    }

    fn measure_svg_simple_text_bbox_width_px(&self, text: &str, style: &TextStyle) -> f64 {
        text.split('\n')
            .map(|line| self.shape_line(line, style).bbox_width())
            .fold(0.0, f64::max)
    }

    fn measure_svg_raw_text_bbox_width_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.measure_svg_simple_text_bbox_width_px(text, style)
    }

    fn measure_svg_raw_text_bbox_height_px(&self, text: &str, style: &TextStyle) -> f64 {
        text.split('\n')
            .map(|line| self.shape_line(line, style).bbox_height())
            .fold(0.0, f64::max)
    }

    fn measure_svg_simple_text_bbox_height_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.measure_svg_raw_text_bbox_height_px(text, style)
    }

    fn measure_svg_tspan_text_bbox_height_px(&self, text: &str, style: &TextStyle) -> f64 {
        self.measure_svg_raw_text_bbox_height_px(text, style)
    }

    fn measure_wrapped(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> TextMetrics {
        self.wrap(text, style, max_width, wrap_mode).0
    }

    fn measure_wrapped_with_raw_width(
        &self,
        text: &str,
        style: &TextStyle,
        max_width: Option<f64>,
        wrap_mode: WrapMode,
    ) -> (TextMetrics, Option<f64>) {
        self.wrap(text, style, max_width, wrap_mode)
    }
}

fn normalize_family(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn parse_font_weight(value: &str) -> u16 {
    match value.trim().to_ascii_lowercase().as_str() {
        "normal" => 400,
        "bold" | "bolder" => 700,
        "lighter" => 300,
        value => value.parse::<u16>().unwrap_or(400).clamp(1, 1000),
    }
}

fn is_default_ignorable(character: char) -> bool {
    matches!(
        character as u32,
        0x00AD
            | 0x034F
            | 0x061C
            | 0x115F..=0x1160
            | 0x17B4..=0x17B5
            | 0x180B..=0x180F
            | 0x200B..=0x200F
            | 0x202A..=0x202E
            | 0x2060..=0x206F
            | 0x3164
            | 0xFE00..=0xFE0F
            | 0xFEFF
            | 0xFFA0
            | 0x1BCA0..=0x1BCAF
            | 0x1D173..=0x1D17A
            | 0xE0000..=0xE0FFF
    )
}

fn shaped_glyph_coverage_is_complete(text: &str, glyph_ids: impl IntoIterator<Item = u32>) -> bool {
    let mut glyph_ids = glyph_ids.into_iter().peekable();
    if glyph_ids.peek().is_none() {
        return text.chars().all(is_default_ignorable);
    }
    glyph_ids.all(|glyph_id| glyph_id != 0)
}

fn cluster_script(value: &str) -> Option<BuzzScript> {
    value
        .chars()
        .map(|character| character.script())
        .find(|script| {
            !matches!(
                script,
                UnicodeScript::Common | UnicodeScript::Inherited | UnicodeScript::Unknown
            )
        })
        .and_then(|script| BuzzScript::from_str(script.short_name()).ok())
}

fn validate_tag(value: &str, field: &'static str) -> Result<(), TextLayoutError> {
    if value.len() != 4
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b' ')
    {
        return Err(TextLayoutError::InvalidRequest(field));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{
        FontAssetSpec, FontCatalogSpec, FontSourcePolicy, FontStack, ThemeResourcePolicy,
        ThemeTextStyle,
    };
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};
    use crate::text::terminal_receipt::PreparedTextTerminalReceipt;

    fn projection_span_text(projection: &TextProjection) -> Vec<(&str, &str)> {
        projection
            .spans()
            .iter()
            .map(|span| {
                (
                    &projection.source()[span.source().as_range()],
                    &projection.visible()[span.visible().as_range()],
                )
            })
            .collect()
    }

    #[test]
    fn prepared_text_label_id_roundtrips_base_and_line_tokens() {
        let id = PreparedTextLabelId::new(PreparedTextLabelFamily::State, 17);

        assert_eq!(id.as_svg_id(), "merman-prepared-state-17");
        assert_eq!(PreparedTextLabelId::from_svg_id(&id.as_svg_id()), Some(id));
        assert_eq!(
            PreparedTextLabelId::from_svg_id(&id.as_svg_line_id(3)),
            Some(id)
        );
        assert_eq!(id.family(), "state");
        assert_eq!(id.key(), 17);

        for invalid in [
            "merman-prepared-state-017",
            "merman-prepared-state-17-line-03",
            "merman-prepared-unknown-17",
            "merman-prepared-state",
            "merman-text-state-17",
        ] {
            assert_eq!(PreparedTextLabelId::from_svg_id(invalid), None, "{invalid}");
        }
    }

    #[test]
    fn prepared_vertical_extents_reject_non_finite_or_inverted_bounds() {
        assert_eq!(
            PreparedTextVerticalExtents::new(f64::NAN, 1.0),
            Err(TextLayoutError::InvalidPreparedText)
        );
        assert_eq!(
            PreparedTextVerticalExtents::new(-1.0, f64::INFINITY),
            Err(TextLayoutError::InvalidPreparedText)
        );
        assert_eq!(
            PreparedTextVerticalExtents::new(2.0, 1.0),
            Err(TextLayoutError::InvalidPreparedText)
        );
        let no_ink = PreparedTextVerticalExtents::new(0.0, 0.0)
            .expect("a zero-height interval is valid for lines without ink");
        assert_eq!(
            PreparedTextLineResponse::new("A", TextByteRange::new(0, 1), 1.0, (0.0, 1.0), no_ink,),
            Err(TextLayoutError::InvalidPreparedText)
        );
        assert!(
            PreparedTextLineResponse::new(" ", TextByteRange::new(0, 1), 1.0, (0.0, 1.0), no_ink,)
                .is_ok()
        );
    }

    #[test]
    fn prepared_vertical_extents_keep_empty_lines_as_baseline_points() {
        let empty = PreparedTextLine::new(
            "",
            TextByteRange::new(0, 0),
            0.0,
            (0.0, 0.0),
            PreparedTextVerticalExtents::new(0.0, 0.0)
                .expect("an empty line has no ink around its baseline"),
        )
        .expect("empty line geometry is valid");
        let ink = PreparedTextLine::new(
            "Ag",
            TextByteRange::new(1, 3),
            10.0,
            (0.0, 10.0),
            PreparedTextVerticalExtents::new(-7.0, 2.0).expect("fixed ink extents are valid"),
        )
        .expect("ink line geometry is valid");

        assert_eq!(
            summarize_prepared_text_vertical_extents(&[empty, ink], 12.0)
                .expect("bounded multiline extents are valid"),
            PreparedTextVerticalExtents::new(0.0, 14.0)
                .expect("the first baseline point and second-line ink form one interval")
        );
    }

    #[test]
    fn prepared_text_retained_bytes_count_shared_evidence_once() {
        let catalog = mixed_catalog();
        let face = &catalog.faces()[0];
        let asset = catalog
            .assets()
            .iter()
            .find(|asset| asset.id() == face.asset_id())
            .expect("the fixture face belongs to one retained asset");
        let range = TextByteRange::new(0, 2);
        let projection = TextProjection::from_parts(
            Arc::from("ab"),
            Arc::from("AB"),
            vec![SourceVisibleSpan::new(range, range)],
        )
        .expect("the fixed projection is valid");
        let line = PreparedTextLine::new(
            "AB",
            range,
            10.0,
            (0.0, 10.0),
            PreparedTextVerticalExtents::new(-6.0, 2.0).expect("fixed vertical extents are valid"),
        )
        .expect("the fixed line geometry is valid");
        let run = PreparedTextLabelEvidence::new(
            range,
            range,
            PreparedTextFaceKey::new(asset.fingerprint(), face.face_index()),
            FontSource::Embedded,
        );
        let prepared = PreparedText::new_with_evidence(
            projection,
            [line],
            [run],
            12.0,
            None,
            Arc::from([Arc::<str>::from("note")]),
            Some(PreparedTextLabelEvidenceContext {
                catalog_fingerprint: catalog.fingerprint(),
                request_digest: TextLayoutRequestDigest::from_bytes([7; 32]),
                provenance: PreparedTextLabelProvenance::Native,
            }),
        )
        .expect("the fixed prepared label is valid");

        let pending = prepared
            .label_ledger_entry()
            .expect("the prepared label retains pending export evidence");
        let pending_bytes = pending.retained_bytes();
        let final_entry = pending.bind(PreparedTextLabelId::new(
            PreparedTextLabelFamily::Flowchart,
            1,
        ));
        let final_bytes = final_entry.retained_bytes();
        let expected_ledger_bytes = PREPARED_TEXT_LEDGER_ENTRY_RECORD_BYTES
            + TEXT_PROJECTION_SPAN_RECORD_BYTES
            + TEXT_BYTE_RANGE_RECORD_BYTES
            + PREPARED_TEXT_LINE_TEXT_RECORD_BYTES
            + 2
            + PREPARED_TEXT_LABEL_EVIDENCE_RECORD_BYTES;
        let expected_prepared_bytes = PREPARED_TEXT_OWNER_RECORD_BYTES
            + 2
            + 2
            + TEXT_PROJECTION_SPAN_RECORD_BYTES
            + PREPARED_TEXT_LINE_RECORD_BYTES
            + 2
            + PREPARED_TEXT_LABEL_EVIDENCE_RECORD_BYTES
            + PREPARED_TEXT_DIAGNOSTIC_RECORD_BYTES
            + 4
            + TEXT_BYTE_RANGE_RECORD_BYTES
            + PREPARED_TEXT_LINE_TEXT_RECORD_BYTES;

        assert_eq!(pending_bytes, expected_ledger_bytes);
        assert_eq!(final_bytes, expected_ledger_bytes);
        assert_eq!(prepared.retained_bytes(), expected_prepared_bytes);
        assert!(prepared.retained_bytes() >= final_bytes);
    }

    fn terminal_receipt_entry() -> PreparedTextLabelLedgerEntry {
        let catalog = mixed_catalog();
        let evidence = catalog
            .faces()
            .iter()
            .take(2)
            .enumerate()
            .map(|(index, face)| {
                let asset = catalog
                    .assets()
                    .iter()
                    .find(|asset| asset.id() == face.asset_id())
                    .expect("the fixture face belongs to one retained asset");
                let range = TextByteRange::new(index, index + 1);
                PreparedTextLabelEvidence::new(
                    range,
                    range,
                    PreparedTextFaceKey::new(asset.fingerprint(), face.face_index()),
                    FontSource::Embedded,
                )
            })
            .collect::<Vec<_>>();
        PreparedTextLabelLedgerEntry {
            id: PreparedTextLabelId::new(PreparedTextLabelFamily::State, 4),
            catalog_fingerprint: catalog.fingerprint(),
            request_digest: TextLayoutRequestDigest::from_bytes([7; 32]),
            provenance: PreparedTextLabelProvenance::Native,
            projection_spans: Arc::from([SourceVisibleSpan::new(
                TextByteRange::new(0, 2),
                TextByteRange::new(0, 2),
            )]),
            line_ranges: Arc::from([TextByteRange::new(0, 1), TextByteRange::new(1, 2)]),
            line_texts: Arc::from([Arc::<str>::from("A"), Arc::<str>::from("B")]),
            runs: evidence.into(),
        }
    }

    #[test]
    fn terminal_receipt_binds_artifact_request_lines_and_ordered_runs() {
        let svg = r#"<svg><text id="merman-prepared-state-4">AB</text></svg>"#;
        let entry = terminal_receipt_entry();
        let receipt = PreparedTextTerminalReceipt::from_ledger(svg, &[entry.clone()])
            .expect("the valid ledger should freeze a terminal receipt");

        assert!(receipt.artifact_matches(svg));
        assert!(
            !receipt.artifact_matches(r#"<svg><text id="merman-prepared-state-4">BA</text></svg>"#)
        );
        assert!(receipt.labels()[0].native_terminal_proof_is_incomplete());

        let mut changed_request = entry.clone();
        changed_request.request_digest = TextLayoutRequestDigest::from_bytes([8; 32]);
        let request_receipt =
            PreparedTextTerminalReceipt::from_ledger(svg, &[changed_request]).unwrap();
        assert_ne!(
            receipt.labels()[0].identity_digest(),
            request_receipt.labels()[0].identity_digest()
        );

        let mut changed_lines = entry.clone();
        changed_lines.line_texts = Arc::from([Arc::<str>::from("B"), Arc::<str>::from("A")]);
        let line_receipt = PreparedTextTerminalReceipt::from_ledger(svg, &[changed_lines]).unwrap();
        assert_ne!(
            receipt.labels()[0].identity_digest(),
            line_receipt.labels()[0].identity_digest()
        );

        let mut changed_run_assignment = entry.clone();
        let mut runs = changed_run_assignment.runs.to_vec();
        runs.swap(0, 1);
        changed_run_assignment.runs = runs.into();
        assert!(
            PreparedTextTerminalReceipt::from_ledger(svg, &[changed_run_assignment]).is_none(),
            "non-monotonic ordered run evidence must fail closed"
        );

        let mut missing_runs = entry.clone();
        missing_runs.runs = Arc::from([]);
        assert!(PreparedTextTerminalReceipt::from_ledger(svg, &[missing_runs]).is_none());

        let mut native_system_face = entry;
        Arc::make_mut(&mut native_system_face.runs)[0].font_source = FontSource::System;
        assert!(PreparedTextTerminalReceipt::from_ledger(svg, &[native_system_face]).is_none());
    }

    #[test]
    fn retained_byte_model_is_logical_and_target_stable() {
        let fingerprint = FontCatalog::default_parity().fingerprint();
        let first = CatalogAdmittedTextStyle {
            typography: ThemeTextStyle::default().with_font_stack(
                FontStack::new(["Excalifont", "sans-serif"]).expect("valid font stack"),
            ),
            catalog_fingerprint: fingerprint,
        };
        let repeated = CatalogAdmittedTextStyle {
            typography: ThemeTextStyle::default().with_font_stack(
                FontStack::new(["Excalifont", "sans-serif"]).expect("valid font stack"),
            ),
            catalog_fingerprint: fingerprint,
        };
        let expected = CATALOG_ADMITTED_TEXT_STYLE_RECORD_BYTES
            + 2 * FONT_STACK_FAMILY_RECORD_BYTES
            + "Excalifont".len()
            + "sans-serif".len();

        assert_eq!(first.retained_bytes(), expected);
        assert_eq!(repeated.retained_bytes(), expected);
    }

    #[test]
    fn retained_byte_model_saturates_instead_of_wrapping() {
        let mut retained = ModeledRetainedBytes::new(usize::MAX - 1);
        retained.add_records(usize::MAX, PREPARED_TEXT_LABEL_EVIDENCE_RECORD_BYTES);
        retained.add_bytes(1);

        assert_eq!(retained.finish(), usize::MAX);
        assert_eq!(
            prepared_text_label_ledger_retained_bytes(
                usize::MAX,
                usize::MAX,
                usize::MAX,
                usize::MAX,
            ),
            usize::MAX
        );
    }

    #[test]
    fn shaped_coverage_requires_real_glyphs_for_visible_text() {
        assert!(shaped_glyph_coverage_is_complete("A", [7]));
        assert!(!shaped_glyph_coverage_is_complete("A", [0]));
        assert!(!shaped_glyph_coverage_is_complete("A", []));
        assert!(shaped_glyph_coverage_is_complete("\u{200d}", []));
    }

    #[test]
    fn fuzz_probe_skips_only_glyph_unavailable() {
        assert_eq!(
            classify_prepared_text_fuzz_probe::<()>(Err(TextLayoutError::GlyphUnavailable)),
            Ok(None)
        );
        assert!(
            classify_prepared_text_fuzz_probe::<()>(Err(TextLayoutError::InvalidPreparedText))
                .is_err()
        );
    }

    #[test]
    fn text_projection_none_preserves_visible_text_and_grapheme_spans() {
        let projection = TextProjection::new("Aé", ThemeTextTransform::None)
            .expect("bounded source should project");

        assert_eq!(projection.source(), "Aé");
        assert_eq!(projection.visible(), "Aé");
        assert_eq!(projection_span_text(&projection), [("A", "A"), ("é", "é")]);

        let entity = TextProjection::new("&amp;", ThemeTextTransform::None)
            .expect("family-owned entities should remain encoded here");
        assert_eq!(entity.visible(), "&amp;");
    }

    #[test]
    fn text_projection_applies_all_theme_transforms() {
        let uppercase = TextProjection::new("Straße", ThemeTextTransform::Uppercase)
            .expect("uppercase projection should succeed");
        let lowercase = TextProjection::new("İSTANBUL", ThemeTextTransform::Lowercase)
            .expect("lowercase projection should succeed");
        let capitalize = TextProjection::new("hello WORLD", ThemeTextTransform::Capitalize)
            .expect("capitalize projection should succeed");

        assert_eq!(uppercase.visible(), "STRASSE");
        assert_eq!(lowercase.visible(), "i\u{307}stanbul");
        assert_eq!(capitalize.visible(), "Hello WORLD");
        assert_eq!(lowercase.spans()[0].source(), TextByteRange::new(0, 2));
        assert_eq!(lowercase.spans()[0].visible(), TextByteRange::new(0, 3));
    }

    #[test]
    fn text_projection_maps_case_expansion_to_one_source_grapheme() {
        let projection = TextProjection::new("ß", ThemeTextTransform::Uppercase)
            .expect("uppercase expansion should project");

        assert_eq!(projection.visible(), "SS");
        assert_eq!(projection.visible().chars().count(), 2);
        assert_eq!(projection.spans().len(), 1);
        assert_eq!(projection_span_text(&projection), [("ß", "SS")]);
    }

    #[test]
    fn text_projection_keeps_combining_variation_and_zwj_sequences_atomic() {
        let source = "e\u{301}✈\u{fe0f}👩\u{200d}💻";
        let projection = TextProjection::new(source, ThemeTextTransform::Uppercase)
            .expect("Unicode graphemes should project");

        assert_eq!(projection.visible(), "E\u{301}✈\u{fe0f}👩\u{200d}💻");
        assert_eq!(
            projection_span_text(&projection),
            [
                ("e\u{301}", "E\u{301}"),
                ("✈\u{fe0f}", "✈\u{fe0f}"),
                ("👩\u{200d}💻", "👩\u{200d}💻"),
            ]
        );
    }

    #[test]
    fn text_projection_normalizes_mermaid_breaks_with_atomic_mappings() {
        let projection = TextProjection::new("a<br>b<br/>c<br />d\ne", ThemeTextTransform::None)
            .expect("explicit breaks should project");

        assert_eq!(projection.visible(), "a\nb\nc\nd\ne");
        assert_eq!(
            projection_span_text(&projection),
            [
                ("a", "a"),
                ("<br>", "\n"),
                ("b", "b"),
                ("<br/>", "\n"),
                ("c", "c"),
                ("<br />", "\n"),
                ("d", "d"),
                ("\n", "\n"),
                ("e", "e"),
            ]
        );
    }

    #[test]
    fn text_projection_fingerprint_is_stable_and_mapping_sensitive() {
        let first =
            TextProjection::new("ab", ThemeTextTransform::None).expect("projection should succeed");
        let repeated = TextProjection::new("ab", ThemeTextTransform::None)
            .expect("projection should be repeatable");
        let transformed = TextProjection::new("ab", ThemeTextTransform::Uppercase)
            .expect("transformed projection should succeed");
        let coalesced = TextProjection::from_parts(
            Arc::from("ab"),
            Arc::from("ab"),
            vec![SourceVisibleSpan::new(
                TextByteRange::new(0, 2),
                TextByteRange::new(0, 2),
            )],
        )
        .expect("coalesced coverage is valid");

        assert_eq!(first.fingerprint(), repeated.fingerprint());
        assert_ne!(first.fingerprint(), transformed.fingerprint());
        assert_ne!(first.fingerprint(), coalesced.fingerprint());
    }

    #[test]
    fn text_projection_rejects_limits_and_invalid_internal_ranges() {
        let oversized = "x".repeat(MAX_TEXT_PROJECTION_BYTES + 1);
        assert_eq!(
            TextProjection::new(oversized, ThemeTextTransform::None),
            Err(TextProjectionError::SourceLimitExceeded)
        );
        assert_eq!(
            TextProjection::build_with_limit(Arc::from("İİ"), ThemeTextTransform::Lowercase, 4,),
            Err(TextProjectionError::VisibleLimitExceeded)
        );
        assert_eq!(
            TextProjection::from_parts(
                Arc::from("é"),
                Arc::from("é"),
                vec![SourceVisibleSpan::new(
                    TextByteRange::new(0, 1),
                    TextByteRange::new(0, 2),
                )],
            ),
            Err(TextProjectionError::InvalidRanges)
        );
    }

    #[test]
    fn text_projection_rejects_span_retention_before_the_next_record_is_pushed() {
        assert!(TEXT_PROJECTION_SPAN_RECORD_BYTES > 0);
        assert_eq!(
            TextProjection::build_with_options_and_budget(
                Arc::from("ab"),
                ThemeTextTransform::None,
                2,
                true,
                TEXT_PROJECTION_SPAN_RECORD_BYTES,
                None,
            ),
            Err(TextProjectionError::SpanLimitExceeded)
        );
    }

    #[test]
    fn text_projection_span_admission_is_target_stable_at_the_text_limit() {
        assert_eq!(MAX_TEXT_PROJECTION_SPANS, MAX_TEXT_PROJECTION_BYTES);
        assert_eq!(TEXT_PROJECTION_SPAN_RECORD_BYTES, 32);
        assert_eq!(
            MAX_TEXT_PROJECTION_SPAN_BYTES,
            MAX_TEXT_PROJECTION_BYTES * 32
        );
        assert!(std::mem::size_of::<SourceVisibleSpan>() <= TEXT_PROJECTION_SPAN_RECORD_BYTES);

        let projection = TextProjection::new(
            "a".repeat(MAX_TEXT_PROJECTION_BYTES),
            ThemeTextTransform::None,
        )
        .expect("the documented text ceiling must admit the densest projection on every target");
        assert_eq!(projection.spans().len(), MAX_TEXT_PROJECTION_SPANS);
    }

    #[test]
    fn operation_budget_rejects_dense_projection_before_span_allocation() {
        let request = prepared_test_request(&"\n".repeat(64 * 1024));
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, 1)
            .expect("the narrow projection budget is valid");
        let meter = OperationWorkMeter::new(policy);

        assert_eq!(
            text_projection_for_request_with_meter(&request, Some(&meter))
                .expect_err("the source scan must be charged before projection allocation"),
            TextLayoutError::LimitExceeded("operation_work")
        );
        assert_eq!(meter.used(), 0);
    }

    #[test]
    fn local_visible_ranges_are_monotonic_for_repeated_text() {
        let source = ProjectedVisibleLine::new(TextByteRange::new(20, 29));
        let wrapped = [TextByteRange::new(0, 4), TextByteRange::new(5, 9)]
            .into_iter()
            .map(|range| ProjectedVisibleLine::from_local_range(source, range))
            .collect::<Result<Vec<_>, _>>()
            .expect("explicit local ranges disambiguate repeated fragments");

        assert_eq!(wrapped[0].visible_range(), TextByteRange::new(20, 24));
        assert_eq!(wrapped[1].visible_range(), TextByteRange::new(25, 29));
    }

    #[test]
    fn prepared_text_rejects_line_text_outside_its_visible_range() {
        let projection = TextProjection::new("alpha", ThemeTextTransform::None)
            .expect("projection should build");
        let line = PreparedTextLine::new(
            "alpha",
            TextByteRange::new(0, 4),
            1.0,
            (0.0, 1.0),
            PreparedTextVerticalExtents::new(-0.8, 0.2).expect("fixed vertical extents are valid"),
        )
        .expect("line metrics are individually valid");

        assert_eq!(
            PreparedText::new(projection, [line], 1.0, None)
                .expect_err("line text and admitted visible range must agree"),
            TextLayoutError::InvalidPreparedText
        );
    }

    #[test]
    fn prepared_text_rejects_a_line_that_splits_a_projection_atom() {
        let projection = TextProjection::new("ß", ThemeTextTransform::Uppercase)
            .expect("expanding projection should build");
        let line = PreparedTextLine::new(
            "S",
            TextByteRange::new(0, 1),
            1.0,
            (0.0, 1.0),
            PreparedTextVerticalExtents::new(-0.8, 0.2).expect("fixed vertical extents are valid"),
        )
        .expect("line metrics are individually valid");

        assert_eq!(
            PreparedText::new(projection, [line], 1.0, None)
                .expect_err("a visible line cannot split one source-visible atom"),
            TextLayoutError::InvalidPreparedText
        );
    }

    fn mixed_catalog() -> FontCatalog {
        let latin = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
        ));
        let cjk = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/themes/assets/fonts/Xiaolai-Regular-CJK-Test.woff2"
        ));
        FontCatalogSpec::new([
            FontAssetSpec::new("latin", latin),
            FontAssetSpec::new("cjk", cjk),
        ])
        .with_alias("Xiaolai", "Xiaolai SC")
        .compile(&ThemeResourcePolicy::interactive())
        .expect("fixture catalog should compile")
    }

    #[test]
    fn css_font_admission_distinguishes_generic_keywords_from_named_families() {
        let latin = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
        ));
        let catalog = FontCatalogSpec::new([FontAssetSpec::new("latin", latin)])
            .with_generic_family(GenericFontFamily::Cursive, "Excalifont")
            .compile(&ThemeResourcePolicy::interactive())
            .expect("fixture catalog should compile");
        let layout = NativeTextLayoutBackend::default()
            .prepare(&PrepareCatalogRequest::new(
                catalog,
                FontSourcePolicy::embedded_only(),
            ))
            .expect("fixture catalog should prepare");
        let unquoted = parse_css_font_stack("cursive").expect("generic CSS family");
        let quoted = parse_css_font_stack("\"cursive\"").expect("named CSS family");
        let requested = ThemeTextStyle::default().with_font_stack(unquoted.font_stack().clone());

        let raw_request =
            PrepareTextRequest::new("alpha", requested.clone()).with_family_normalized_projection();
        assert!(matches!(
            layout.prepare_text(&raw_request),
            Err(TextLayoutError::FontFamilyUnavailable)
        ));

        let admitted = layout
            .admit_typography_with_css_font_stack(&requested, Some(&unquoted))
            .expect("unquoted generic should use the catalog mapping");
        assert_eq!(
            admitted.typography().font_stack().families(),
            &["Excalifont".to_string()]
        );
        assert_eq!(
            layout.admit_typography_with_css_font_stack(&requested, Some(&quoted)),
            Err(TextLayoutError::FontFamilyUnavailable),
            "quoted generic text is a named family and must not use the generic mapping"
        );
        layout
            .prepare_text(
                &PrepareTextRequest::new("alpha", admitted.typography().clone())
                    .with_family_normalized_projection(),
            )
            .expect("admitted generic typography should shape through its canonical family");
    }

    #[test]
    fn admitted_typography_serializes_named_fonts_and_all_measured_spacing() {
        let catalog = mixed_catalog();
        let layout = NativeTextLayoutBackend::default()
            .prepare(&PrepareCatalogRequest::new(
                catalog,
                FontSourcePolicy::embedded_only(),
            ))
            .expect("fixture catalog should prepare");
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").unwrap())
            .with_font_size_px(20.0)
            .unwrap()
            .with_line_height(LineHeight::Multiplier(1.4))
            .unwrap()
            .with_letter_spacing_px(1.5)
            .unwrap()
            .with_word_spacing_px(2.5)
            .unwrap();
        let admitted = layout
            .admit_typography(&typography)
            .expect("catalog owns the requested family");
        let style = admitted.merge_emission_font_style(Some(
            "font-family:Arial;line-height:9;letter-spacing:8px;color:#123456",
        ));

        assert!(style.contains("color:#123456"), "{style}");
        assert!(
            style.contains("font-family:\"Excalifont\" !important"),
            "{style}"
        );
        assert!(style.contains("line-height:1.4 !important"), "{style}");
        assert!(style.contains("letter-spacing:1.5px !important"), "{style}");
        assert!(style.contains("word-spacing:2.5px !important"), "{style}");
        assert!(!style.contains("font-family:Arial"), "{style}");
        assert!(!style.contains("line-height:9"), "{style}");
        assert!((admitted.line_height_em() - 1.4).abs() < 1e-6);
    }

    fn distinct_catalog_fingerprint() -> FontCatalogFingerprint {
        let latin = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/themes/assets/fonts/Excalifont-Regular-Latin.woff2"
        ));
        FontCatalogSpec::new([FontAssetSpec::new("different-latin", latin)])
            .compile(&ThemeResourcePolicy::interactive())
            .expect("distinct fixture catalog should compile")
            .fingerprint()
    }

    fn prepared_test_request(text: &str) -> PrepareTextRequest {
        PrepareTextRequest::new(
            text,
            ThemeTextStyle::default()
                .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid")),
        )
    }

    fn backend_request_for(
        layout: &PreparedTextLayout,
        request: &PrepareTextRequest,
    ) -> PreparedTextBackendRequest {
        let binding = PreparedTextCallBinding::new(
            layout.catalog_fingerprint(),
            layout.contract_version(),
            layout.backend().clone(),
            layout.session_token(),
            request.digest(),
        )
        .expect("admitted layout identity is a valid call binding");
        PreparedTextBackendRequest::new(binding, request)
            .expect("bounded test request should normalize")
    }

    fn metered_projection_and_digest_work(request: &PrepareTextRequest) -> usize {
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        text_projection_for_request_with_meter(request, Some(&meter))
            .expect("bounded test request should build its metered projection");
        request
            .digest_with_work_meter(Some(&meter))
            .expect("bounded request digest work should be admitted");
        meter.used()
    }

    fn valid_raw_response(request: &PreparedTextBackendRequest) -> PreparedTextResponse {
        let lines = split_projected_visible_lines(request.visible_text())
            .into_iter()
            .map(|line| {
                PreparedTextLineResponse::new(
                    line.text(request.visible_text())
                        .expect("split line range belongs to the visible projection"),
                    line.visible_range,
                    10.0,
                    (0.0, 10.0),
                    PreparedTextVerticalExtents::new(-6.0, 2.0)
                        .expect("fixed vertical extents are valid"),
                )
                .expect("fixed raw line geometry is valid")
            })
            .collect::<Vec<_>>();
        let runs = lines
            .iter()
            .filter(|line| line.visible_range().start() < line.visible_range().end())
            .map(|line| {
                PreparedTextRunResponse::new(
                    line.visible_range(),
                    PreparedTextFaceSlot::new(0),
                    FontSource::Embedded,
                )
            })
            .collect::<Vec<_>>();
        PreparedTextResponse::new(request.binding().clone(), lines, runs, 12.0, None)
            .expect("fixed raw response is structurally bounded")
    }

    #[test]
    fn external_prepared_text_response_is_admitted_and_retains_actual_font_evidence() {
        #[derive(Clone)]
        struct EchoPreparedTextSession;

        impl PreparedTextBackendSession for EchoPreparedTextSession {
            fn prepare_text(
                &self,
                request: &PreparedTextBackendRequest,
            ) -> Result<PreparedTextResponse, TextLayoutError> {
                valid_raw_response(request).with_diagnostics(["host-shaped"])
            }
        }

        let catalog = mixed_catalog();
        let catalog_request =
            PrepareCatalogRequest::new(catalog.clone(), FontSourcePolicy::embedded_only());
        let backend =
            TextLayoutBackendIdentity::new("test.echo", "v1").expect("test backend identity");
        let token = TextLayoutSessionToken::from_bytes([11; 16]).expect("nonzero token");
        let response = PreparedTextLayoutResponse::new(
            catalog.fingerprint(),
            TEXT_LAYOUT_CONTRACT_VERSION,
            backend.clone(),
            TextLayoutCapabilities::native(),
            FontSource::Embedded,
            TextLayoutFaceEvidence::new(catalog.faces().iter().cloned())
                .expect("complete face evidence"),
            token,
            Arc::new(EchoPreparedTextSession),
        )
        .expect("catalog response should decode");
        let layout = PreparedTextLayout::admit_backend_response(
            &catalog_request,
            &backend,
            TextLayoutCapabilities::native(),
            response,
        )
        .expect("complete host catalog evidence should be admitted");
        let request = PrepareTextRequest::new(
            "ß<br>a",
            ThemeTextStyle::default()
                .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"))
                .with_transform(ThemeTextTransform::Uppercase),
        );

        let admitted = layout
            .prepare_text(&request)
            .expect("bound host response should be admitted");

        assert_eq!(admitted.source_text(), "ß<br>a");
        assert_eq!(admitted.visible_text(), "SS\nA");
        assert_eq!(admitted.wrapped_lines().collect::<Vec<_>>(), ["SS", "A"]);
        assert_eq!(admitted.run_evidence().len(), 2);
        assert!(
            admitted
                .run_evidence()
                .iter()
                .all(|run| run.font_source() == FontSource::Embedded)
        );
        let selected_face = &catalog.faces()[0];
        let selected_asset = catalog
            .assets()
            .iter()
            .find(|asset| asset.id() == selected_face.asset_id())
            .expect("selected face belongs to the retained catalog");
        assert!(admitted.run_evidence().iter().all(|run| {
            run.face_key()
                == PreparedTextFaceKey::new(
                    selected_asset.fingerprint(),
                    selected_face.face_index(),
                )
        }));
        let ledger_entry = admitted
            .label_ledger_entry()
            .expect("admitted backend result retains per-label evidence")
            .bind(PreparedTextLabelId::new(
                PreparedTextLabelFamily::Flowchart,
                4,
            ));
        assert_eq!(ledger_entry.id().as_svg_id(), "merman-prepared-flowchart-4");
        assert_eq!(ledger_entry.catalog_fingerprint(), catalog.fingerprint());
        assert_eq!(ledger_entry.request_digest(), request.digest());
        assert_eq!(
            ledger_entry.provenance(),
            PreparedTextLabelProvenance::HostDependent
        );
        assert_eq!(ledger_entry.projection_spans().len(), 3);
        assert_eq!(
            ledger_entry.line_ranges(),
            [TextByteRange::new(0, 2), TextByteRange::new(3, 4)]
        );
        assert_eq!(ledger_entry.evidence().len(), 2);
        assert_eq!(
            ledger_entry.evidence()[0].source_range(),
            TextByteRange::new(0, 2)
        );
        assert_eq!(
            ledger_entry.evidence()[0].visible_range(),
            TextByteRange::new(0, 2)
        );
        assert_eq!(
            ledger_entry.evidence()[1].source_range(),
            TextByteRange::new(6, 7)
        );
        assert_eq!(
            ledger_entry.evidence()[1].visible_range(),
            TextByteRange::new(3, 4)
        );
        assert!(ledger_entry.evidence().iter().all(|run| {
            run.face_key()
                == PreparedTextFaceKey::new(
                    selected_asset.fingerprint(),
                    selected_face.face_index(),
                )
                && run.font_source() == FontSource::Embedded
        }));
        assert_eq!(admitted.diagnostics().collect::<Vec<_>>(), ["host-shaped"]);
    }

    #[test]
    fn prepared_text_admission_rejects_stale_or_cross_session_bindings() {
        let catalog = mixed_catalog();
        let layout = NativeTextLayoutBackend::default()
            .prepare(&PrepareCatalogRequest::new(
                catalog,
                FontSourcePolicy::embedded_only(),
            ))
            .expect("native catalog should prepare");
        let request = prepared_test_request("alpha");
        let backend_request = backend_request_for(&layout, &request);
        let response = valid_raw_response(&backend_request);

        let mut mismatched = response.clone();
        mismatched.binding.catalog_fingerprint = distinct_catalog_fingerprint();
        assert_eq!(
            layout
                .admit_text_response(&backend_request, mismatched)
                .expect_err("cross-catalog response must be rejected"),
            TextLayoutError::CatalogFingerprintMismatch
        );

        let mut mismatched = response.clone();
        mismatched.binding.request_digest = prepared_test_request("beta").digest();
        assert_eq!(
            layout
                .admit_text_response(&backend_request, mismatched)
                .expect_err("stale request digest must be rejected"),
            TextLayoutError::RequestDigestMismatch
        );

        let mut mismatched = response.clone();
        mismatched.binding.session_token =
            TextLayoutSessionToken::from_bytes([12; 16]).expect("nonzero token");
        assert_eq!(
            layout
                .admit_text_response(&backend_request, mismatched)
                .expect_err("cross-session response must be rejected"),
            TextLayoutError::SessionTokenMismatch
        );

        let mut mismatched = response.clone();
        mismatched.binding.contract_version = TEXT_LAYOUT_CONTRACT_VERSION + 1;
        assert!(matches!(
            layout
                .admit_text_response(&backend_request, mismatched)
                .expect_err("cross-contract response must be rejected"),
            TextLayoutError::UnsupportedContract { .. }
        ));

        let mut mismatched = response;
        mismatched.binding.backend =
            TextLayoutBackendIdentity::new("test.other", "v1").expect("valid identity");
        assert_eq!(
            layout
                .admit_text_response(&backend_request, mismatched)
                .expect_err("cross-backend response must be rejected"),
            TextLayoutError::BackendIdentityMismatch
        );
    }

    #[test]
    fn prepared_text_admission_rejects_invalid_geometry_ranges_and_run_evidence() {
        let catalog = mixed_catalog();
        let layout = NativeTextLayoutBackend::default()
            .prepare(&PrepareCatalogRequest::new(
                catalog,
                FontSourcePolicy::embedded_only(),
            ))
            .expect("native catalog should prepare");
        let request = prepared_test_request("alpha");
        let backend_request = backend_request_for(&layout, &request);
        let response = valid_raw_response(&backend_request);

        let mut invalid = response.clone();
        Arc::make_mut(&mut invalid.lines)[0].computed_length_px = f64::NAN;
        assert_eq!(
            layout
                .admit_text_response(&backend_request, invalid)
                .expect_err("non-finite raw geometry must be rejected"),
            TextLayoutError::InvalidPreparedText
        );

        let mut invalid = response.clone();
        Arc::make_mut(&mut invalid.lines)[0].vertical_extents = PreparedTextVerticalExtents {
            top_px: 2.0,
            bottom_px: 1.0,
        };
        assert_eq!(
            layout
                .admit_text_response(&backend_request, invalid)
                .expect_err("missing a valid baseline-relative ink interval must fail closed"),
            TextLayoutError::InvalidPreparedText
        );

        let mut invalid = response.clone();
        Arc::make_mut(&mut invalid.lines)[0].visible_range = TextByteRange::new(0, 4);
        assert_eq!(
            layout
                .admit_text_response(&backend_request, invalid)
                .expect_err("line ranges must cover the exact canonical text"),
            TextLayoutError::InvalidPreparedText
        );

        let mut invalid = response.clone();
        Arc::make_mut(&mut invalid.runs)[0].face_slot = PreparedTextFaceSlot::new(u32::MAX);
        assert_eq!(
            layout
                .admit_text_response(&backend_request, invalid)
                .expect_err("unknown face slots must be rejected"),
            TextLayoutError::RunEvidenceMismatch
        );

        let mut invalid = response.clone();
        Arc::make_mut(&mut invalid.runs)[0].font_source = FontSource::System;
        assert_eq!(
            layout
                .admit_text_response(&backend_request, invalid)
                .expect_err("run source must match the admitted session source"),
            TextLayoutError::RunEvidenceMismatch
        );

        let mut invalid = response;
        invalid.runs = Arc::from([]);
        assert_eq!(
            layout
                .admit_text_response(&backend_request, invalid)
                .expect_err("non-empty lines require complete run coverage"),
            TextLayoutError::RunEvidenceMismatch
        );
    }

    #[test]
    fn prepared_text_admission_rejects_incomplete_or_mode_inconsistent_content() {
        let layout = NativeTextLayoutBackend::default()
            .prepare(&PrepareCatalogRequest::new(
                mixed_catalog(),
                FontSourcePolicy::embedded_only(),
            ))
            .expect("native catalog should prepare");
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"));

        let wrapped_request = PrepareTextRequest::new("alpha beta", typography.clone()).with_wrap(
            PreparedTextWrap::SvgLike {
                max_width_px: Some(40.0),
                break_long_words: true,
            },
        );
        let backend_request = backend_request_for(&layout, &wrapped_request);
        let partial = PreparedTextResponse::new(
            backend_request.binding().clone(),
            [PreparedTextLineResponse::new(
                "alpha",
                TextByteRange::new(0, 5),
                10.0,
                (0.0, 10.0),
                PreparedTextVerticalExtents::new(-6.0, 2.0)
                    .expect("fixed vertical extents are valid"),
            )
            .expect("partial line geometry is structurally valid")],
            [PreparedTextRunResponse::new(
                TextByteRange::new(0, 5),
                PreparedTextFaceSlot::new(0),
                FontSource::Embedded,
            )],
            12.0,
            None,
        )
        .expect("partial response is structurally bounded");
        assert_eq!(
            layout
                .admit_text_response(&backend_request, partial)
                .expect_err("a backend cannot omit non-whitespace visible content"),
            TextLayoutError::InvalidPreparedText
        );

        let html_request = PrepareTextRequest::new("alpha beta", typography).with_wrap(
            PreparedTextWrap::HtmlLike {
                max_width_px: Some(40.0),
            },
        );
        let backend_request = backend_request_for(&layout, &html_request);
        let missing_raw_width = valid_raw_response(&backend_request);
        assert_eq!(
            layout
                .admit_text_response(&backend_request, missing_raw_width)
                .expect_err("HTML preparation must retain its unwrapped width"),
            TextLayoutError::InvalidPreparedText
        );
    }

    #[test]
    fn prepared_text_admission_rejects_backend_invalidation_and_unbounded_requests() {
        let layout = NativeTextLayoutBackend::default()
            .prepare(&PrepareCatalogRequest::new(
                mixed_catalog(),
                FontSourcePolicy::embedded_only(),
            ))
            .expect("native catalog should prepare");
        let request = prepared_test_request("alpha");
        let backend_request = backend_request_for(&layout, &request);
        let invalidated = PreparedTextResponse::invalidated(
            backend_request.binding().clone(),
            ["font cache evicted"],
        )
        .expect("bounded invalidation response should decode");
        assert_eq!(
            layout
                .admit_text_response(&backend_request, invalidated)
                .expect_err("invalidated response must not enter layout"),
            TextLayoutError::BackendInvalidated
        );

        let oversized = prepared_test_request(&"x".repeat(MAX_TEXT_PROJECTION_BYTES + 1));
        assert_eq!(
            layout
                .prepare_text(&oversized)
                .expect_err("oversized text must be rejected before reaching the backend"),
            TextLayoutError::LimitExceeded("text")
        );

        let invalid_width = prepared_test_request("alpha").with_wrap(PreparedTextWrap::SvgLike {
            max_width_px: Some(f64::NAN),
            break_long_words: false,
        });
        assert_eq!(
            layout
                .prepare_text(&invalid_width)
                .expect_err("invalid widths must be rejected before reaching the backend"),
            TextLayoutError::InvalidRequest("max_width_px")
        );
    }

    #[test]
    fn prepared_text_response_enforces_hard_request_and_geometry_budgets() {
        let layout = NativeTextLayoutBackend::default()
            .prepare(&PrepareCatalogRequest::new(
                mixed_catalog(),
                FontSourcePolicy::embedded_only(),
            ))
            .expect("native catalog should prepare");
        let request = prepared_test_request("a");
        let backend_request = backend_request_for(&layout, &request);
        let oversized_line = PreparedTextLineResponse::new(
            "x".repeat(MAX_TEXT_PROJECTION_BYTES),
            TextByteRange::new(0, MAX_TEXT_PROJECTION_BYTES),
            1.0,
            (0.0, 1.0),
            PreparedTextVerticalExtents::new(-0.8, 0.2).expect("fixed vertical extents are valid"),
        )
        .expect("one projection-sized line remains within the per-line hard cap");
        assert_eq!(
            PreparedTextResponse::new(
                backend_request.binding().clone(),
                vec![oversized_line; 17],
                [],
                1.0,
                None,
            )
            .expect_err("aggregate response bytes must have a hard cap"),
            TextLayoutError::LimitExceeded("response.bytes")
        );

        let mut record_amplification = valid_raw_response(&backend_request);
        record_amplification.lines = vec![record_amplification.lines[0].clone(); 3].into();
        assert_eq!(
            layout
                .admit_text_response(&backend_request, record_amplification)
                .expect_err("response records must scale with the bound projection"),
            TextLayoutError::LimitExceeded("response.records")
        );

        let multiline = prepared_test_request("a\nb");
        let backend_request = backend_request_for(&layout, &multiline);
        let mut oversized_geometry = valid_raw_response(&backend_request);
        oversized_geometry.line_height_px = 600_000_000.0;
        assert_eq!(
            layout
                .admit_text_response(&backend_request, oversized_geometry)
                .expect_err("aggregate geometry must remain within the global ceiling"),
            TextLayoutError::InvalidPreparedText
        );
    }

    #[test]
    fn prepared_text_response_stops_pulling_lines_at_the_first_retained_byte_overflow() {
        let layout = NativeTextLayoutBackend::default()
            .prepare(&PrepareCatalogRequest::new(
                mixed_catalog(),
                FontSourcePolicy::embedded_only(),
            ))
            .expect("native catalog should prepare");
        let request = prepared_test_request("a");
        let backend_request = backend_request_for(&layout, &request);
        let line = PreparedTextLineResponse::new(
            "x".repeat(MAX_TEXT_PROJECTION_BYTES),
            TextByteRange::new(0, MAX_TEXT_PROJECTION_BYTES),
            1.0,
            (0.0, 1.0),
            PreparedTextVerticalExtents::new(-0.8, 0.2).expect("fixed vertical extents are valid"),
        )
        .expect("one projection-sized line remains within the per-line hard cap");
        let line_bytes = MAX_TEXT_PROJECTION_BYTES + PREPARED_TEXT_LINE_RECORD_BYTES;
        let admitted_lines = MAX_PREPARED_TEXT_RESPONSE_BYTES / line_bytes;
        let line_pulls = std::rc::Rc::new(std::cell::Cell::new(0usize));
        let run_pulls = std::rc::Rc::new(std::cell::Cell::new(0usize));
        let counted_lines = {
            let line_pulls = std::rc::Rc::clone(&line_pulls);
            std::iter::from_fn(move || {
                line_pulls.set(line_pulls.get().saturating_add(1));
                Some(line.clone())
            })
        };
        let counted_runs = {
            let run_pulls = std::rc::Rc::clone(&run_pulls);
            std::iter::from_fn(move || {
                run_pulls.set(run_pulls.get().saturating_add(1));
                Some(PreparedTextRunResponse::new(
                    TextByteRange::new(0, 1),
                    PreparedTextFaceSlot::new(0),
                    FontSource::Embedded,
                ))
            })
        };

        assert_eq!(
            PreparedTextResponse::new(
                backend_request.binding().clone(),
                counted_lines,
                counted_runs,
                1.0,
                None,
            )
            .expect_err("the first line beyond the retained-byte budget must stop collection"),
            TextLayoutError::LimitExceeded("response.bytes")
        );
        assert_eq!(line_pulls.get(), admitted_lines.saturating_add(1));
        assert_eq!(
            run_pulls.get(),
            0,
            "runs must never be pulled after line overflow"
        );
    }

    #[test]
    fn native_prepare_freezes_catalog_identity_and_session_token() {
        let catalog = mixed_catalog();
        let request =
            PrepareCatalogRequest::new(catalog.clone(), FontSourcePolicy::embedded_only());
        let backend = NativeTextLayoutBackend::default();
        let prepared = backend
            .prepare(&request)
            .expect("native backend should prepare fixture catalog");

        assert_eq!(prepared.catalog_fingerprint(), catalog.fingerprint());
        assert!(prepared.attests_catalog(catalog.fingerprint()));
        assert_eq!(
            prepared.face_evidence().faces().len(),
            catalog.faces().len()
        );
        assert_eq!(prepared.report().face_count(), catalog.faces().len());

        let repeated = backend
            .prepare(&request)
            .expect("repeated preparation should succeed");
        assert_eq!(prepared.session_token(), repeated.session_token());
    }

    #[test]
    fn native_prepare_shapes_latin_and_cjk_without_rediscovery() {
        let catalog = mixed_catalog();
        let request = PrepareCatalogRequest::new(catalog, FontSourcePolicy::embedded_only());
        let prepared = NativeTextLayoutBackend::default()
            .prepare(&request)
            .expect("native backend should prepare fixture catalog");
        let typography = ThemeTextStyle::default().with_font_stack(
            FontStack::new(["Excalifont", "Xiaolai"]).expect("fixture stack is valid"),
        );

        let latin = prepared
            .prepare_text(&PrepareTextRequest::new("portable", typography.clone()))
            .expect("Latin label should use the first catalog family");
        let mixed = prepared
            .prepare_text(&PrepareTextRequest::new("portable 图", typography))
            .expect("CJK cluster should fall through to the second catalog family");
        assert!(latin.metrics().width.is_finite() && latin.metrics().width > 0.0);
        assert!(
            mixed.metrics().width.is_finite() && mixed.metrics().width >= latin.metrics().width
        );
        assert_eq!(latin.metrics().line_count, 1);
    }

    #[test]
    fn prepared_text_rejects_stack_external_families_and_missing_glyphs() {
        let prepared = NativeTextLayoutBackend::default()
            .prepare(&PrepareCatalogRequest::new(
                mixed_catalog(),
                FontSourcePolicy::embedded_only(),
            ))
            .expect("native backend should prepare fixture catalog");
        let unknown = ThemeTextStyle::default().with_font_stack(
            FontStack::single("Not In Catalog").expect("bounded unknown family is valid input"),
        );
        assert_eq!(
            prepared
                .prepare_text(&PrepareTextRequest::new("portable", unknown))
                .expect_err("an unknown family must not fall back to the first catalog face"),
            TextLayoutError::FontFamilyUnavailable
        );

        let latin_only = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"));
        assert_eq!(
            prepared
                .prepare_text(&PrepareTextRequest::new("图", latin_only))
                .expect_err("a missing glyph must not silently become zero width"),
            TextLayoutError::GlyphUnavailable
        );
    }

    #[test]
    fn native_backend_requires_embedded_source_authority() {
        let request = PrepareCatalogRequest::new(mixed_catalog(), FontSourcePolicy::system_only());
        assert_eq!(
            NativeTextLayoutBackend::default()
                .prepare(&request)
                .expect_err("native catalog shaping cannot claim a system-only session"),
            TextLayoutError::FontSourceUnavailable
        );
    }

    #[test]
    fn native_measurement_separates_advance_from_outline_bounds() {
        let request =
            PrepareCatalogRequest::new(mixed_catalog(), FontSourcePolicy::embedded_only());
        let prepared = NativeTextLayoutBackend::default()
            .prepare(&request)
            .expect("native backend should prepare fixture catalog");
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"))
            .with_font_size_px(32.0)
            .expect("fixture font size is valid");

        let witness = ["j", "f", "Tj", "Á", "Wj"]
            .into_iter()
            .find(|text| {
                let result = prepared
                    .prepare_text(&PrepareTextRequest::new(*text, typography.clone()))
                    .expect("fixture witness should prepare");
                (result.computed_length_px() - result.bbox_width_px()).abs() > 0.01
            })
            .expect("fixture font should expose an advance/bbox distinction");
        let result = prepared
            .prepare_text(&PrepareTextRequest::new(witness, typography))
            .expect("fixture witness should prepare");
        let (left, right) = result.bbox_x();
        let bbox = result.bbox_width_px();
        let height = result.bbox_height_px();

        assert!(left.is_finite() && right.is_finite());
        assert!(bbox.is_finite() && bbox > 0.0);
        assert!(height.is_finite() && height > 0.0);
    }

    #[test]
    fn native_prepared_text_retains_baseline_relative_extents_across_lines_and_sizes() {
        let request =
            PrepareCatalogRequest::new(mixed_catalog(), FontSourcePolicy::embedded_only());
        let prepared = NativeTextLayoutBackend::default()
            .prepare(&request)
            .expect("native backend should prepare fixture catalog");
        let mut observed_heights = Vec::new();

        for font_size in [12.0, 32.0] {
            let typography = ThemeTextStyle::default()
                .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"))
                .with_font_size_px(font_size)
                .expect("fixture font size is valid")
                .with_line_height(LineHeight::Multiplier(1.25))
                .expect("fixture line height is valid");
            let single = prepared
                .prepare_text(&PrepareTextRequest::new("Agjp", typography.clone()))
                .expect("single-line fixture should prepare");
            let single_extents = single.vertical_extents();
            assert!(single_extents.top_px() < 0.0);
            assert!(single_extents.bottom_px() > 0.0);
            assert_eq!(single.bbox_height_px(), single_extents.height_px());

            let multiline = prepared
                .prepare_text(&PrepareTextRequest::new("Ag\njp", typography))
                .expect("multi-line fixture should prepare");
            assert_eq!(multiline.lines().len(), 2);
            let expected = multiline.lines()[0]
                .vertical_extents()
                .union(
                    multiline.lines()[1]
                        .vertical_extents()
                        .translated(multiline.line_height_px())
                        .expect("bounded line advance remains valid"),
                )
                .expect("bounded line union remains valid");
            assert_eq!(multiline.vertical_extents(), expected);
            assert_eq!(multiline.bbox_height_px(), expected.height_px());
            assert!(multiline.bbox_height_px() > single.bbox_height_px());
            observed_heights.push(single.bbox_height_px());
        }

        assert!(observed_heights[1] > observed_heights[0] * 2.0);
    }

    #[test]
    fn native_wrapping_preserves_html_svg_and_single_run_modes() {
        let request =
            PrepareCatalogRequest::new(mixed_catalog(), FontSourcePolicy::embedded_only());
        let prepared = NativeTextLayoutBackend::default()
            .prepare(&request)
            .expect("native backend should prepare fixture catalog");
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"));
        let max_width = prepared
            .prepare_text(&PrepareTextRequest::new("alpha", typography.clone()))
            .expect("probe should prepare")
            .computed_length_px()
            + 1.0;

        let svg = prepared
            .prepare_text(
                &PrepareTextRequest::new("alpha beta gamma", typography.clone()).with_wrap(
                    PreparedTextWrap::SvgLike {
                        max_width_px: Some(max_width),
                        break_long_words: true,
                    },
                ),
            )
            .expect("SVG wrapping should prepare");
        let single_run = prepared
            .prepare_text(
                &PrepareTextRequest::new("alpha beta gamma", typography.clone())
                    .with_wrap(PreparedTextWrap::SingleRun),
            )
            .expect("single run should prepare");
        let html = prepared
            .prepare_text(
                &PrepareTextRequest::new("alpha beta gamma", typography.clone()).with_wrap(
                    PreparedTextWrap::HtmlLike {
                        max_width_px: Some(max_width),
                    },
                ),
            )
            .expect("HTML wrapping should prepare");
        let explicit_break = prepared
            .prepare_text(
                &PrepareTextRequest::new("alpha<br/>beta", typography)
                    .with_wrap(PreparedTextWrap::SingleRun),
            )
            .expect("explicit break should prepare");

        assert!(svg.metrics().line_count > 1);
        assert_eq!(single_run.metrics().line_count, 1);
        assert!(html.metrics().line_count > 1);
        assert!(html.raw_width_px().is_some_and(|width| width > max_width));
        assert_eq!(explicit_break.metrics().line_count, 2);
    }

    #[test]
    fn native_wrapping_shapes_each_visible_byte_at_most_once_per_pass() {
        let catalog_request =
            PrepareCatalogRequest::new(mixed_catalog(), FontSourcePolicy::embedded_only());
        let layout = NativeTextLayoutBackend::default()
            .prepare(&catalog_request)
            .expect("native backend should prepare fixture catalog");
        let measurer = NativeCatalogTextMeasurer::new(&catalog_request, FontSource::Embedded)
            .expect("fixture catalog should construct the native measurer");
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"));

        for (text, wrap, expects_metrics_fanout) in [
            (
                "a".repeat(8_192),
                PreparedTextWrap::SvgLike {
                    max_width_px: Some(32.0),
                    break_long_words: true,
                },
                true,
            ),
            (
                "alpha ".repeat(1_365),
                PreparedTextWrap::SvgLike {
                    max_width_px: Some(1_000_000.0),
                    break_long_words: true,
                },
                false,
            ),
            (
                "alpha-beta ".repeat(744),
                PreparedTextWrap::HtmlLike {
                    max_width_px: Some(1_000_000.0),
                },
                false,
            ),
        ] {
            let request = PrepareTextRequest::new(text, typography.clone()).with_wrap(wrap);
            let backend_request = backend_request_for(&layout, &request);
            let visible_bytes = backend_request.visible_text().len();
            let projection_spans = backend_request.projection().spans().len();
            let (response, work) = measurer
                .prepare_structured_text_with_work(&backend_request)
                .expect("bounded fixture text should prepare linearly");

            assert_eq!(work.wrapping_input_bytes, visible_bytes);
            assert!(work.metrics_input_bytes <= visible_bytes);
            assert!(
                work.wrapping_input_bytes
                    .saturating_add(work.metrics_input_bytes)
                    <= visible_bytes.saturating_mul(2)
            );
            assert_eq!(work.wrapping_line_ranges, 1);
            assert_eq!(work.metrics_line_ranges, response.lines().len());
            if expects_metrics_fanout {
                assert!(work.metrics_line_ranges > work.wrapping_line_ranges);
            }
            assert!(
                work.wrapping_span_visits
                    <= projection_spans.saturating_add(work.wrapping_line_ranges.saturating_mul(2))
            );
            assert!(
                work.metrics_span_visits
                    <= projection_spans.saturating_add(work.metrics_line_ranges.saturating_mul(2))
            );
        }
    }

    #[test]
    fn native_wrapping_span_cursor_visits_multiline_projection_linearly() {
        let catalog_request =
            PrepareCatalogRequest::new(mixed_catalog(), FontSourcePolicy::embedded_only());
        let layout = NativeTextLayoutBackend::default()
            .prepare(&catalog_request)
            .expect("native backend should prepare fixture catalog");
        let measurer = NativeCatalogTextMeasurer::new(&catalog_request, FontSource::Embedded)
            .expect("fixture catalog should construct the native measurer");
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"));
        let source_line_count = 512;
        let text = vec!["alpha"; source_line_count].join("\n");
        let request =
            PrepareTextRequest::new(text, typography).with_wrap(PreparedTextWrap::SvgLike {
                max_width_px: Some(1_000_000.0),
                break_long_words: true,
            });
        let backend_request = backend_request_for(&layout, &request);
        let projection_spans = backend_request.projection().spans().len();
        let (response, work) = measurer
            .prepare_structured_text_with_work(&backend_request)
            .expect("multiline fixture should prepare linearly");

        assert_eq!(work.wrapping_line_ranges, source_line_count);
        assert_eq!(response.lines().len(), source_line_count);
        assert!(
            work.wrapping_span_visits
                <= projection_spans.saturating_add(work.wrapping_line_ranges.saturating_mul(2))
        );
    }

    #[test]
    fn projection_span_cursor_preserves_gaps_empty_lines_and_expanded_atoms() {
        let source = "a  b\n\nß";
        let projection = TextProjection::from_parts(
            Arc::from(source),
            Arc::from("A B\n\nSS"),
            vec![
                SourceVisibleSpan::new(TextByteRange::new(0, 1), TextByteRange::new(0, 1)),
                SourceVisibleSpan::new(TextByteRange::new(1, 2), TextByteRange::new(1, 2)),
                SourceVisibleSpan::new(TextByteRange::new(2, 3), TextByteRange::new(2, 2)),
                SourceVisibleSpan::new(TextByteRange::new(3, 4), TextByteRange::new(2, 3)),
                SourceVisibleSpan::new(TextByteRange::new(4, 5), TextByteRange::new(3, 4)),
                SourceVisibleSpan::new(TextByteRange::new(5, 6), TextByteRange::new(4, 5)),
                SourceVisibleSpan::new(TextByteRange::new(6, 8), TextByteRange::new(5, 7)),
            ],
        )
        .expect("projection with folded whitespace should build");
        assert_eq!(projection.visible(), "A B\n\nSS");
        let lines = split_projected_visible_lines(projection.visible());
        assert_eq!(
            lines
                .iter()
                .map(|line| line.visible_range())
                .collect::<Vec<_>>(),
            vec![
                TextByteRange::new(0, 3),
                TextByteRange::new(4, 4),
                TextByteRange::new(5, 7),
            ]
        );

        let mut cursor = ProjectionSpanCursor::default();
        let mut budget = StructuredShapingBudget::new(None);
        let ranges = lines
            .iter()
            .map(|line| {
                cursor.range_for_line(
                    &projection,
                    line.visible_range(),
                    StructuredShapingPass::Metrics,
                    &mut budget,
                )
            })
            .collect::<Result<Vec<_>, _>>()
            .expect("ordered projected lines should admit");

        assert_eq!(ranges[0], 0..4);
        assert!(ranges[1].is_empty());
        assert_eq!(ranges[2], 6..7);
        let work = budget.work;
        assert_eq!(work.metrics_line_ranges, lines.len());
        assert!(
            work.metrics_span_visits
                <= projection
                    .spans()
                    .len()
                    .saturating_add(lines.len().saturating_mul(2))
        );

        let catalog_request =
            PrepareCatalogRequest::new(mixed_catalog(), FontSourcePolicy::embedded_only());
        let layout = NativeTextLayoutBackend::default()
            .prepare(&catalog_request)
            .expect("native backend should prepare fixture catalog");
        let measurer = NativeCatalogTextMeasurer::new(&catalog_request, FontSource::Embedded)
            .expect("fixture catalog should construct the native measurer");
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"))
            .with_transform(ThemeTextTransform::Uppercase);
        let request =
            PrepareTextRequest::new(source, typography).with_wrap(PreparedTextWrap::SvgLike {
                max_width_px: Some(1_000_000.0),
                break_long_words: true,
            });
        let binding = backend_request_for(&layout, &request).binding().clone();
        let backend_request =
            PreparedTextBackendRequest::from_projection(binding, &request, projection)
                .expect("custom test projection should match the request source");
        let projection_spans = backend_request.projection().spans().len();
        let (response, work) = measurer
            .prepare_structured_text_with_work(&backend_request)
            .expect("zero-length projection spans should not be shaped as visible atoms");

        assert_eq!(
            response
                .lines()
                .iter()
                .map(PreparedTextLineResponse::text)
                .collect::<Vec<_>>(),
            ["A B", "", "SS"]
        );
        assert_eq!(work.wrapping_line_ranges, 3);
        assert_eq!(work.metrics_line_ranges, 3);
        assert!(
            work.wrapping_span_visits
                <= projection_spans.saturating_add(work.wrapping_line_ranges.saturating_mul(2))
        );
        assert!(
            work.metrics_span_visits
                <= projection_spans.saturating_add(work.metrics_line_ranges.saturating_mul(2))
        );
    }

    #[test]
    fn native_cluster_coverage_shapes_complex_graphemes_once_across_wrap_and_metrics() {
        let catalog_request =
            PrepareCatalogRequest::new(mixed_catalog(), FontSourcePolicy::embedded_only());
        let layout = NativeTextLayoutBackend::default()
            .prepare(&catalog_request)
            .expect("native backend should prepare fixture catalog");
        let measurer = NativeCatalogTextMeasurer::new(&catalog_request, FontSource::Embedded)
            .expect("fixture catalog should construct the native measurer");
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"));
        let request =
            PrepareTextRequest::new("e\u{301}", typography).with_wrap(PreparedTextWrap::SvgLike {
                max_width_px: Some(1_000.0),
                break_long_words: true,
            });
        let backend_request = backend_request_for(&layout, &request);
        let (_, work) = measurer
            .prepare_structured_text_with_work(&backend_request)
            .expect("combining-mark cluster should be shaped by the native backend");

        assert_eq!(work.coverage_input_bytes, "e\u{301}".len());
        assert_eq!(work.coverage_cache_insertions, 1);
        assert_eq!(work.face_inspections, 1);
    }

    #[test]
    fn native_prepared_text_charges_exact_structured_work_to_the_operation_meter() {
        let catalog_request =
            PrepareCatalogRequest::new(mixed_catalog(), FontSourcePolicy::embedded_only());
        let layout = NativeTextLayoutBackend::default()
            .prepare(&catalog_request)
            .expect("native backend should prepare fixture catalog");
        let typography = ThemeTextStyle::default().with_font_stack(
            FontStack::new(["Excalifont", "Xiaolai"]).expect("fixture families are valid"),
        );
        let request = PrepareTextRequest::new("portable 图", typography).with_wrap(
            PreparedTextWrap::SvgLike {
                max_width_px: Some(64.0),
                break_long_words: true,
            },
        );
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());

        layout
            .prepare_text_with_work_meter(&request, &meter)
            .expect("the operation meter should admit one complete native preparation");
        let one_label_work = meter.used();
        assert!(one_label_work > 0);
        layout
            .prepare_text_with_work_meter(&request, &meter)
            .expect("later labels should share the same operation meter");

        assert_eq!(meter.used(), one_label_work.saturating_mul(2));
    }

    #[test]
    fn request_digest_precharges_non_text_geometry_inputs() {
        let base_typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"));
        let base = PrepareTextRequest::new("", base_typography.clone());
        let metrics_typography = base_typography.with_font_stack(
            FontStack::new(["Excalifont", "Xiaolai"]).expect("fixture families are valid"),
        );
        let enriched = PrepareTextRequest::new("", metrics_typography.clone())
            .with_metrics_typography(metrics_typography)
            .with_script("Latn")
            .expect("Latn is a valid script tag")
            .with_language("en")
            .expect("en is a valid language tag")
            .with_features(["kern"])
            .expect("kern is a valid OpenType feature");
        let measure_digest = |request: &PrepareTextRequest| {
            let meter =
                OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
            request
                .digest_with_work_meter(Some(&meter))
                .expect("bounded request digest should be admitted");
            meter.used()
        };

        let base_work = measure_digest(&base);
        let enriched_work = measure_digest(&enriched);
        assert!(enriched_work > base_work);

        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, base_work)
            .expect("the measured base digest limit is valid");
        let meter = OperationWorkMeter::new(policy);
        assert_eq!(
            enriched
                .digest_with_work_meter(Some(&meter))
                .expect_err("non-text digest inputs must consume operation work"),
            TextLayoutError::LimitExceeded("operation_work")
        );
        assert!(meter.used() <= base_work);
    }

    #[test]
    fn structured_shaping_work_total_includes_every_bounded_lane() {
        let work = StructuredShapingWork {
            candidate_compilation_units: 65_536,
            coverage_input_bytes: 1,
            face_inspections: 2,
            coverage_cache_insertions: 4,
            coverage_cache_slots: 8,
            source_line_scan_bytes: 16,
            source_line_visits: 32,
            wrapped_line_emissions: 64,
            wrapping_input_bytes: 128,
            metrics_input_bytes: 256,
            wrapping_span_visits: 512,
            metrics_span_visits: 1_024,
            wrapping_line_ranges: 2_048,
            metrics_line_ranges: 4_096,
            glyph_visits: 8_192,
            cluster_visits: 16_384,
            wrap_boundary_visits: 32_768,
        };

        assert_eq!(work.total_units(), Ok(131_071));
    }

    #[test]
    fn native_prepared_text_rejects_an_operation_budget_below_exact_work() {
        let catalog_request =
            PrepareCatalogRequest::new(mixed_catalog(), FontSourcePolicy::embedded_only());
        let layout = NativeTextLayoutBackend::default()
            .prepare(&catalog_request)
            .expect("native backend should prepare fixture catalog");
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"));
        let request =
            PrepareTextRequest::new("bounded", typography).with_wrap(PreparedTextWrap::SvgLike {
                max_width_px: Some(48.0),
                break_long_words: true,
            });
        let exact_meter =
            OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        layout
            .prepare_text_with_work_meter(&request, &exact_meter)
            .expect("the unbounded meter should measure one complete preparation");
        let exact = exact_meter.used();
        assert!(exact > 1);
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, exact - 1)
            .expect("a positive narrow work limit is valid");
        let meter = OperationWorkMeter::new(policy);

        assert_eq!(
            layout
                .prepare_text_with_work_meter(&request, &meter)
                .expect_err("one work unit below the exact cost must reject"),
            TextLayoutError::LimitExceeded("operation_work")
        );
        assert!(
            meter.used() > 0 && meter.used() <= exact - 1,
            "completed shaping actions must remain charged when a later action is rejected"
        );
    }

    #[test]
    fn prepared_text_admission_work_scales_linearly_with_lines_and_runs() {
        let catalog_request =
            PrepareCatalogRequest::new(mixed_catalog(), FontSourcePolicy::embedded_only());
        let layout = NativeTextLayoutBackend::default()
            .prepare(&catalog_request)
            .expect("native backend should prepare fixture catalog");
        let measure_admission = |line_count: usize| {
            let text = (0..line_count).map(|_| "a").collect::<Vec<_>>().join("\n");
            let request = prepared_test_request(&text);
            let backend_request = backend_request_for(&layout, &request);
            let response = valid_raw_response(&backend_request);
            let meter =
                OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());

            layout
                .primary_candidate()
                .admit_text_response(
                    layout.catalog(),
                    layout.contract_version(),
                    &backend_request,
                    response,
                    Some(&meter),
                )
                .expect("bounded multiline response should be admitted");
            meter.used()
        };

        let small = measure_admission(128);
        let large = measure_admission(256);
        let larger = measure_admission(384);
        let first_delta = large.saturating_sub(small);
        let second_delta = larger.saturating_sub(large);
        assert!(large > small);
        assert!(
            first_delta.abs_diff(second_delta) <= 16,
            "equal line/run increments must have linear admission cost: {small} -> {large} -> {larger}"
        );
    }

    #[test]
    fn native_right_to_left_shaping_preserves_projection_order() {
        let catalog_request =
            PrepareCatalogRequest::new(mixed_catalog(), FontSourcePolicy::embedded_only());
        let layout = NativeTextLayoutBackend::default()
            .prepare(&catalog_request)
            .expect("native backend should prepare fixture catalog");
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"));
        let request = PrepareTextRequest::new("alpha beta", typography)
            .with_direction(TextLayoutDirection::RightToLeft)
            .with_wrap(PreparedTextWrap::SvgLike {
                max_width_px: Some(1_000.0),
                break_long_words: true,
            });
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());

        let prepared = layout
            .prepare_text_with_work_meter(&request, &meter)
            .expect("right-to-left glyph clusters should admit in source projection order");

        assert_eq!(prepared.visible_text(), "alpha beta");
        assert_eq!(prepared.wrapped_lines().collect::<Vec<_>>(), ["alpha beta"]);
        assert_eq!(prepared.run_evidence().len(), 1);
        assert_eq!(
            prepared.run_evidence()[0].visible_range(),
            TextByteRange::new(0, "alpha beta".len())
        );
        assert!(meter.used() > 0);
    }

    #[test]
    fn native_operation_budget_stops_before_the_next_coverage_shape() {
        let catalog_request =
            PrepareCatalogRequest::new(mixed_catalog(), FontSourcePolicy::embedded_only());
        let layout = NativeTextLayoutBackend::default()
            .prepare(&catalog_request)
            .expect("native backend should prepare fixture catalog");
        let measurer = NativeCatalogTextMeasurer::new(&catalog_request, FontSource::Embedded)
            .expect("fixture catalog should construct the native measurer");
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"));
        let request =
            PrepareTextRequest::new("a", typography).with_wrap(PreparedTextWrap::SvgLike {
                max_width_px: Some(32.0),
                break_long_words: true,
            });
        let backend_request = backend_request_for(&layout, &request);
        let (_, complete_work) = measurer
            .prepare_structured_text_with_work(&backend_request)
            .expect("fixture should expose deterministic native work");
        let admitted_before_coverage_shape = complete_work
            .candidate_compilation_units
            .saturating_add(complete_work.coverage_cache_slots)
            .saturating_add(complete_work.source_line_scan_bytes)
            .saturating_add(complete_work.source_line_visits)
            .saturating_add(complete_work.wrapping_line_ranges)
            .saturating_add(complete_work.wrapping_span_visits)
            .saturating_add(complete_work.face_inspections);
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(
                ResourceLimitId::MaxLayoutWorkUnits,
                admitted_before_coverage_shape,
            )
            .expect("the narrow shaping budget is valid");
        let meter = OperationWorkMeter::new(policy);

        let (attempt, work) =
            measurer.prepare_structured_text_attempt_with_work_meter(&backend_request, &meter);

        assert_eq!(
            attempt.expect_err("coverage shaping must not begin after its pre-charge is rejected"),
            TextLayoutError::LimitExceeded("operation_work")
        );
        assert_eq!(meter.used(), admitted_before_coverage_shape);
        assert_eq!(
            work.candidate_compilation_units,
            complete_work.candidate_compilation_units
        );
        assert_eq!(work.coverage_cache_slots, 2);
        assert_eq!(work.source_line_scan_bytes, 1);
        assert_eq!(work.source_line_visits, 1);
        assert_eq!(work.wrapping_line_ranges, 1);
        assert_eq!(work.wrapping_span_visits, 2);
        assert_eq!(work.face_inspections, 1);
        assert_eq!(work.coverage_input_bytes, 0);
        assert_eq!(work.coverage_cache_insertions, 0);
        assert_eq!(work.wrapping_input_bytes, 0);
        assert_eq!(work.metrics_input_bytes, 0);
        assert_eq!(work.metrics_line_ranges, 0);
    }

    #[test]
    fn failed_native_coverage_is_still_charged_to_the_operation_meter() {
        let catalog_request =
            PrepareCatalogRequest::new(mixed_catalog(), FontSourcePolicy::embedded_only());
        let layout = NativeTextLayoutBackend::default()
            .prepare(&catalog_request)
            .expect("native backend should prepare fixture catalog");
        let measurer = NativeCatalogTextMeasurer::new(&catalog_request, FontSource::Embedded)
            .expect("fixture catalog should construct the native measurer");
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"));
        let request = PrepareTextRequest::new("图", typography);
        let backend_request = backend_request_for(&layout, &request);
        let (attempt, work) = measurer.prepare_structured_text_attempt(&backend_request);
        assert_eq!(
            attempt.expect_err("the Latin-only face does not cover the CJK witness"),
            TextLayoutError::GlyphUnavailable
        );
        let expected = work
            .total_units()
            .expect("bounded failed work should fit in usize")
            .saturating_add(metered_projection_and_digest_work(&request));
        assert!(expected > 0);
        let meter = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());

        assert_eq!(
            layout
                .prepare_text_with_work_meter(&request, &meter)
                .expect_err("the missing glyph should remain the semantic error"),
            TextLayoutError::GlyphUnavailable
        );
        assert_eq!(meter.used(), expected);
    }

    #[test]
    fn legacy_native_prepare_entry_remains_unmetered_and_compatible() {
        let layout = NativeTextLayoutBackend::default()
            .prepare(&PrepareCatalogRequest::new(
                mixed_catalog(),
                FontSourcePolicy::embedded_only(),
            ))
            .expect("native backend should prepare fixture catalog");
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"));

        let prepared = layout
            .prepare_text(&PrepareTextRequest::new("compatible", typography))
            .expect("the compatibility entry should retain its existing behavior");

        assert!(prepared.metrics().width > 0.0);
    }

    #[test]
    fn resource_limit_errors_never_advance_to_another_backend_candidate() {
        assert!(!text_layout_error_allows_fallback(
            &TextLayoutError::LimitExceeded("operation_work")
        ));
    }

    #[test]
    fn native_scalar_coverage_confirms_cmap_candidates_with_shaping() {
        let catalog_request =
            PrepareCatalogRequest::new(mixed_catalog(), FontSourcePolicy::embedded_only());
        let layout = NativeTextLayoutBackend::default()
            .prepare(&catalog_request)
            .expect("native backend should prepare fixture catalog");
        let measurer = NativeCatalogTextMeasurer::new(&catalog_request, FontSource::Embedded)
            .expect("fixture catalog should construct the native measurer");
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"));
        let request = PrepareTextRequest::new("A", typography);
        let backend_request = backend_request_for(&layout, &request);
        let (_, work) = measurer
            .prepare_structured_text_with_work(&backend_request)
            .expect("scalar coverage should be confirmed by shaping");

        assert_eq!(work.coverage_input_bytes, 1);
        assert_eq!(work.coverage_cache_insertions, 1);
        assert_eq!(work.face_inspections, 1);
    }

    #[test]
    fn native_coverage_cache_retains_one_face_decision_per_span() {
        let catalog_request =
            PrepareCatalogRequest::new(mixed_catalog(), FontSourcePolicy::embedded_only());
        let layout = NativeTextLayoutBackend::default()
            .prepare(&catalog_request)
            .expect("native backend should prepare fixture catalog");
        let measurer = NativeCatalogTextMeasurer::new(&catalog_request, FontSource::Embedded)
            .expect("fixture catalog should construct the native measurer");
        let typography = ThemeTextStyle::default().with_font_stack(
            FontStack::new(["Excalifont", "Xiaolai"]).expect("fixture families are valid"),
        );
        let request = PrepareTextRequest::new("图", typography);
        let backend_request = backend_request_for(&layout, &request);
        let (_, work) = measurer
            .prepare_structured_text_with_work(&backend_request)
            .expect("CJK fallback should select the second catalog face");

        assert_eq!(work.face_inspections, 2);
        assert_eq!(work.coverage_cache_insertions, 1);
        assert_eq!(work.coverage_input_bytes, "图".len() * 2);
    }

    #[test]
    fn prepared_coverage_does_not_drop_non_breaking_or_preserved_whitespace() {
        let budget = PreparedTextAdmissionBudget::new(None);
        let projection = TextProjection::new("alpha\u{00a0}beta", ThemeTextTransform::None)
            .expect("projection should build");
        let vertical_extents =
            PreparedTextVerticalExtents::new(-0.8, 0.2).expect("fixed vertical extents are valid");
        let alpha = PreparedTextLine::new(
            "alpha",
            TextByteRange::new(0, 5),
            1.0,
            (0.0, 1.0),
            vertical_extents,
        )
        .expect("line should be valid");
        let beta_after_nbsp = PreparedTextLine::new(
            "beta",
            TextByteRange::new(7, 11),
            1.0,
            (0.0, 1.0),
            vertical_extents,
        )
        .expect("line should be valid");
        assert_eq!(
            validate_prepared_text_coverage(
                &projection,
                &[alpha.clone(), beta_after_nbsp],
                PreparedTextWrap::HtmlLike {
                    max_width_px: Some(10.0),
                },
                WhiteSpace::Normal,
                Some(10.0),
                &budget,
            ),
            Err(TextLayoutError::InvalidPreparedText)
        );

        let collapsible_projection = TextProjection::new("alpha beta", ThemeTextTransform::None)
            .expect("projection should build");
        let beta_after_space = PreparedTextLine::new(
            "beta",
            TextByteRange::new(6, 10),
            1.0,
            (0.0, 1.0),
            vertical_extents,
        )
        .expect("line should be valid");
        assert_eq!(
            validate_prepared_text_coverage(
                &collapsible_projection,
                &[alpha.clone(), beta_after_space.clone()],
                PreparedTextWrap::HtmlLike {
                    max_width_px: Some(10.0),
                },
                WhiteSpace::Normal,
                Some(10.0),
                &budget,
            ),
            Ok(())
        );
        assert_eq!(
            validate_prepared_text_coverage(
                &collapsible_projection,
                &[alpha, beta_after_space],
                PreparedTextWrap::HtmlLike {
                    max_width_px: Some(10.0),
                },
                WhiteSpace::PreWrap,
                Some(10.0),
                &budget,
            ),
            Err(TextLayoutError::InvalidPreparedText)
        );
    }

    #[test]
    fn native_wrapping_never_splits_a_transform_expansion_atom() {
        let layout = NativeTextLayoutBackend::default()
            .prepare(&PrepareCatalogRequest::new(
                mixed_catalog(),
                FontSourcePolicy::embedded_only(),
            ))
            .expect("native backend should prepare fixture catalog");
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"))
            .with_transform(ThemeTextTransform::Uppercase);
        let atom_width = layout
            .prepare_text(&PrepareTextRequest::new("ß", typography.clone()))
            .expect("expanded atom probe should prepare")
            .computed_length_px();
        let prepared = layout
            .prepare_text(&PrepareTextRequest::new("ßßß", typography).with_wrap(
                PreparedTextWrap::SvgLike {
                    max_width_px: Some(atom_width + 0.01),
                    break_long_words: true,
                },
            ))
            .expect("expanded atoms should wrap at source-visible boundaries");

        assert_eq!(prepared.visible_text(), "SSSSSS");
        assert_eq!(
            prepared.wrapped_lines().collect::<Vec<_>>(),
            ["SS", "SS", "SS"]
        );
        assert!(prepared.lines().iter().all(|line| {
            let range = line.visible_range();
            range.start % 2 == 0 && range.end % 2 == 0
        }));
    }

    #[test]
    fn prepared_constructor_rejects_under_attested_capabilities_and_faces() {
        struct RejectingTextSession;

        impl PreparedTextBackendSession for RejectingTextSession {
            fn prepare_text(
                &self,
                _request: &PreparedTextBackendRequest,
            ) -> Result<PreparedTextResponse, TextLayoutError> {
                Err(TextLayoutError::BackendRejected)
            }
        }

        let catalog = mixed_catalog();
        let request =
            PrepareCatalogRequest::new(catalog.clone(), FontSourcePolicy::embedded_only());
        let backend =
            TextLayoutBackendIdentity::new("test.host", "v1").expect("test backend identity");
        let token = TextLayoutSessionToken::from_bytes([7; 16]).expect("nonzero token");
        let partial_faces = TextLayoutFaceEvidence::new([catalog.faces()[0].clone()])
            .expect("one face is valid evidence by itself");
        let session: Arc<dyn PreparedTextBackendSession> = Arc::new(RejectingTextSession);

        let partial_response = PreparedTextLayoutResponse::new(
            catalog.fingerprint(),
            TEXT_LAYOUT_CONTRACT_VERSION,
            backend.clone(),
            TextLayoutCapabilities::native(),
            FontSource::Embedded,
            partial_faces,
            token,
            session.clone(),
        )
        .expect("partial response can be decoded before request admission");
        let missing_face = PreparedTextLayout::admit_backend_response(
            &request,
            &backend,
            TextLayoutCapabilities::native(),
            partial_response,
        )
        .expect_err("partial loaded-face evidence must be rejected");
        assert_eq!(missing_face, TextLayoutError::LoadedFaceEvidenceMismatch);

        let mut capabilities = TextLayoutCapabilities::native();
        capabilities.supports_features = false;
        let all_faces = TextLayoutFaceEvidence::new(catalog.faces().iter().cloned())
            .expect("complete face evidence");
        let under_attested_response = PreparedTextLayoutResponse::new(
            catalog.fingerprint(),
            TEXT_LAYOUT_CONTRACT_VERSION,
            backend.clone(),
            capabilities,
            FontSource::Embedded,
            all_faces,
            token,
            session,
        )
        .expect("under-attested response can be decoded before request admission");
        let prepared = PreparedTextLayout::admit_backend_response(
            &request,
            &backend,
            TextLayoutCapabilities::native(),
            under_attested_response,
        )
        .expect("catalog admission does not depend on label-specific feature settings");
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Excalifont").expect("fixture family is valid"));
        let text_request = PrepareTextRequest::new("portable", typography)
            .with_features(["kern"])
            .expect("kern is a valid OpenType feature");
        let missing_capability = prepared
            .prepare_text(&text_request)
            .expect_err("requested OpenType features require per-label capability evidence");
        assert_eq!(
            missing_capability,
            TextLayoutError::CapabilityNotAttested("features")
        );
    }

    #[test]
    fn preparation_rejects_unknown_contract_versions() {
        let catalog = mixed_catalog();
        let request = PrepareCatalogRequest::new(catalog, FontSourcePolicy::embedded_only())
            .with_contract_version(999);
        let error = NativeTextLayoutBackend::default()
            .prepare(&request)
            .expect_err("unknown protocol versions must not be silently accepted");
        assert_eq!(
            error,
            TextLayoutError::UnsupportedContract {
                requested: 999,
                supported: TEXT_LAYOUT_CONTRACT_VERSION,
            }
        );
    }

    #[test]
    fn relative_legacy_font_weights_resolve_against_the_theme_weight() {
        for (inherited, bolder, lighter) in [
            (1, 400, 1),
            (99, 400, 99),
            (100, 400, 100),
            (349, 400, 100),
            (350, 700, 100),
            (549, 700, 100),
            (550, 900, 400),
            (749, 900, 400),
            (750, 900, 700),
            (899, 900, 700),
            (900, 900, 700),
            (1000, 1000, 700),
        ] {
            assert_eq!(resolve_css_font_weight("bolder", inherited), Some(bolder));
            assert_eq!(resolve_css_font_weight("lighter", inherited), Some(lighter));
        }
        assert_eq!(resolve_css_font_weight("calc(400)", 400), None);
    }
}
