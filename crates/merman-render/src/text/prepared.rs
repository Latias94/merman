//! Source-owned CSS font decisions and bounded text projection evidence.

use crate::diagram_theme::{
    FontAssetFingerprint, FontCatalogFingerprint, FontSource, FontStack, GenericFontFamily,
    HostMeasurementFallback,
};
use cssparser::Parser;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ParsedCssFontStack {
    font_stack: FontStack,
    generic_families: BTreeMap<usize, GenericFontFamily>,
}

impl ParsedCssFontStack {
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

pub(crate) fn parse_css_font_stack(value: &str) -> Option<ParsedCssFontStack> {
    let mut parser = Parser::new(value);
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
    #[cfg(test)]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub const fn start(self) -> usize {
        self.start
    }

    pub const fn end(self) -> usize {
        self.end
    }
}

/// One ordered mapping atom between source and visible UTF-8 coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SourceVisibleSpan {
    source: TextByteRange,
    visible: TextByteRange,
}

impl SourceVisibleSpan {
    #[cfg(test)]
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

/// Canonical identity of one prepared-label request within a catalog session.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct TextLayoutRequestDigest([u8; 32]);

impl TextLayoutRequestDigest {
    #[cfg(test)]
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

pub(crate) const PREPARED_TEXT_LABEL_ID_PREFIX: &str = "merman-prepared-";

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
    #[cfg(test)]
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

/// Stable catalog face key retained for one prepared-text run.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PreparedTextFaceKey {
    asset_fingerprint: FontAssetFingerprint,
    face_index: u32,
}

impl PreparedTextFaceKey {
    #[cfg(test)]
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
    #[cfg(test)]
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
    /// A valid host-dependent label for internal projection-boundary tests, without a font backend.
    #[cfg(test)]
    pub(crate) fn for_test(id: PreparedTextLabelId, text: &str) -> Self {
        let range = TextByteRange::new(0, text.len());
        Self {
            id,
            catalog_fingerprint: crate::diagram_theme::FontCatalog::system_fonts().fingerprint(),
            request_digest: TextLayoutRequestDigest::from_bytes([1; 32]),
            provenance: PreparedTextLabelProvenance::HostDependent,
            projection_spans: vec![SourceVisibleSpan::new(range, range)].into(),
            line_ranges: vec![range].into(),
            line_texts: vec![Arc::from(text)].into(),
            runs: vec![PreparedTextLabelEvidence::new(
                range,
                range,
                PreparedTextFaceKey::new(FontAssetFingerprint::for_test([2; 32]), 0),
                FontSource::System,
            )]
            .into(),
        }
    }

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

#[cfg(test)]
mod tests {
    use super::*;

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
