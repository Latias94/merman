use std::collections::BTreeMap;

use serde::de::{Error as _, IgnoredAny};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::ThemeRuleFacetV1;

/// Schema version used by the version 1 theme-support discovery query.
pub const THEME_SUPPORT_SCHEMA_VERSION_V1: u32 = 1;

/// Schema version used by the version 2 subject-based theme-support discovery query.
pub const THEME_SUPPORT_SCHEMA_VERSION_V2: u32 = 2;

/// Maximum number of stable reason identifiers in one coarse support descriptor.
pub const MAX_THEME_SUPPORT_REASON_IDS_V1: usize = 8;

/// A versioned, forward-compatible query for coarse theme support.
///
/// Catalog identifiers remain strings so older consumers can preserve and inspect identifiers
/// added by newer Merman releases. Unknown identifiers are valid discovery input and resolve to an
/// unverified descriptor rather than becoming executable theme identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeSupportQueryV1 {
    schema_version: u32,
    family: String,
    output: String,
    target: String,
    facet: String,
}

impl ThemeSupportQueryV1 {
    /// Builds a query from known output and facet identifiers.
    pub fn known<F>(
        family: impl Into<String>,
        output: ThemeSupportOutputV1,
        target: impl Into<String>,
        facet: F,
    ) -> Self
    where
        F: Into<ThemeSupportFacetV1>,
    {
        let facet = facet.into();
        Self {
            schema_version: THEME_SUPPORT_SCHEMA_VERSION_V1,
            family: family.into(),
            output: output.id().to_owned(),
            target: target.into(),
            facet: facet.id().to_owned(),
        }
    }

    /// Returns the query schema version.
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the preserved diagram-family identifier.
    pub fn family_id(&self) -> &str {
        &self.family
    }

    /// Returns the preserved output-target identifier.
    pub fn output_id(&self) -> &str {
        &self.output
    }

    /// Returns the preserved semantic-target identifier.
    pub fn target_id(&self) -> &str {
        &self.target
    }

    /// Returns the preserved facet identifier.
    pub fn facet_id(&self) -> &str {
        &self.facet
    }
}

/// A version 2 theme-support query with an explicit tagged subject.
///
/// Unlike V1, family-wide base typography is not projected through a synthetic semantic target.
/// Unknown subject and property identifiers remain decodable discovery input and resolve to an
/// unverified renderer claim.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeSupportQueryV2 {
    schema_version: u32,
    family: String,
    output: String,
    subject: ThemeSupportSubjectV2,
}

impl ThemeSupportQueryV2 {
    /// Builds a V2 query for one semantic-target rule facet.
    pub fn rule(
        family: impl Into<String>,
        output: ThemeSupportOutputV1,
        target: impl Into<String>,
        facet: ThemeRuleFacetV1,
    ) -> Self {
        Self {
            schema_version: THEME_SUPPORT_SCHEMA_VERSION_V2,
            family: family.into(),
            output: output.id().to_owned(),
            subject: ThemeSupportSubjectV2::Rule {
                target: target.into(),
                facet: facet.id().to_owned(),
            },
        }
    }

    /// Builds a V2 query for one semantic target's ordinal palette.
    pub fn ordinal_palette(
        family: impl Into<String>,
        output: ThemeSupportOutputV1,
        target: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: THEME_SUPPORT_SCHEMA_VERSION_V2,
            family: family.into(),
            output: output.id().to_owned(),
            subject: ThemeSupportSubjectV2::OrdinalPalette {
                target: target.into(),
            },
        }
    }

    /// Builds a V2 query for one family-wide base typography property.
    pub fn base_typography(
        family: impl Into<String>,
        output: ThemeSupportOutputV1,
        property: ThemeSupportBaseTypographyPropertyV2,
    ) -> Self {
        Self {
            schema_version: THEME_SUPPORT_SCHEMA_VERSION_V2,
            family: family.into(),
            output: output.id().to_owned(),
            subject: ThemeSupportSubjectV2::BaseTypography {
                property: property.id().to_owned(),
            },
        }
    }

    /// Returns the query schema version.
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the preserved diagram-family identifier.
    pub fn family_id(&self) -> &str {
        &self.family
    }

    /// Returns the preserved output-target identifier.
    pub fn output_id(&self) -> &str {
        &self.output
    }

    /// Returns the explicit discovery subject.
    pub const fn subject(&self) -> &ThemeSupportSubjectV2 {
        &self.subject
    }
}

/// The subject of a version 2 theme-support query.
///
/// The wire representation is tagged by `kind`. Known subjects preserve unknown target, facet,
/// and property identifiers as strings so the renderer can return `Unverified` instead of
/// rejecting forward-compatible discovery input.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ThemeSupportSubjectV2 {
    /// One rule facet on a semantic target.
    Rule {
        /// Preserved semantic-target identifier.
        target: String,
        /// Preserved rule-facet identifier.
        facet: String,
    },
    /// The ordinal palette of one semantic target.
    OrdinalPalette {
        /// Preserved semantic-target identifier.
        target: String,
    },
    /// One family-wide base typography property.
    BaseTypography {
        /// Preserved base typography property identifier.
        property: String,
    },
    /// A subject tag unknown to this contract version.
    Unknown {
        /// Preserved unknown subject tag.
        kind: String,
    },
}

impl Serialize for ThemeSupportSubjectV2 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let field_count = match self {
            Self::Rule { .. } => 3,
            Self::OrdinalPalette { .. } | Self::BaseTypography { .. } => 2,
            Self::Unknown { .. } => 1,
        };
        let mut map = serializer.serialize_map(Some(field_count))?;
        match self {
            Self::Rule { target, facet } => {
                map.serialize_entry("kind", "rule")?;
                map.serialize_entry("target", target)?;
                map.serialize_entry("facet", facet)?;
            }
            Self::OrdinalPalette { target } => {
                map.serialize_entry("kind", "ordinal-palette")?;
                map.serialize_entry("target", target)?;
            }
            Self::BaseTypography { property } => {
                map.serialize_entry("kind", "base-typography")?;
                map.serialize_entry("property", property)?;
            }
            Self::Unknown { kind } => {
                map.serialize_entry("kind", kind)?;
            }
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for ThemeSupportSubjectV2 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct SubjectWire {
            kind: String,
            #[serde(default)]
            target: Option<String>,
            #[serde(default)]
            facet: Option<String>,
            #[serde(default)]
            property: Option<String>,
            #[serde(flatten)]
            extra: BTreeMap<String, IgnoredAny>,
        }

        let SubjectWire {
            kind,
            target,
            facet,
            property,
            extra,
        } = SubjectWire::deserialize(deserializer)?;

        let reject_extra = |kind: &str| {
            if extra.is_empty() {
                Ok(())
            } else {
                Err(D::Error::custom(format!(
                    "unexpected fields for theme support subject `{kind}`"
                )))
            }
        };
        match kind.as_str() {
            "rule" => {
                reject_extra(&kind)?;
                if property.is_some() {
                    return Err(D::Error::custom(
                        "theme support subject `rule` does not accept `property`",
                    ));
                }
                Ok(Self::Rule {
                    target: target.ok_or_else(|| {
                        D::Error::custom("theme support subject `rule` requires `target`")
                    })?,
                    facet: facet.ok_or_else(|| {
                        D::Error::custom("theme support subject `rule` requires `facet`")
                    })?,
                })
            }
            "ordinal-palette" => {
                reject_extra(&kind)?;
                if facet.is_some() || property.is_some() {
                    return Err(D::Error::custom(
                        "theme support subject `ordinal-palette` only accepts `target`",
                    ));
                }
                Ok(Self::OrdinalPalette {
                    target: target.ok_or_else(|| {
                        D::Error::custom(
                            "theme support subject `ordinal-palette` requires `target`",
                        )
                    })?,
                })
            }
            "base-typography" => {
                reject_extra(&kind)?;
                if target.is_some() || facet.is_some() {
                    return Err(D::Error::custom(
                        "theme support subject `base-typography` only accepts `property`",
                    ));
                }
                Ok(Self::BaseTypography {
                    property: property.ok_or_else(|| {
                        D::Error::custom(
                            "theme support subject `base-typography` requires `property`",
                        )
                    })?,
                })
            }
            _ => Ok(Self::Unknown { kind }),
        }
    }
}

/// A public family-wide typography property available to V2 support discovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ThemeSupportBaseTypographyPropertyV2 {
    /// Font-family stack.
    FontStack,
    /// Font size.
    FontSize,
    /// Font weight.
    FontWeight,
    /// Font style.
    FontStyle,
    /// Line height.
    LineHeight,
    /// Letter spacing.
    LetterSpacing,
}

impl ThemeSupportBaseTypographyPropertyV2 {
    /// Stable enumeration view for catalogs and contract round-trip checks.
    pub const ALL: &'static [Self] = &[
        Self::FontStack,
        Self::FontSize,
        Self::FontWeight,
        Self::FontStyle,
        Self::LineHeight,
        Self::LetterSpacing,
    ];

    /// Stable wire identifier for this base typography property.
    pub const fn id(self) -> &'static str {
        match self {
            Self::FontStack => "font-stack",
            Self::FontSize => "font-size",
            Self::FontWeight => "font-weight",
            Self::FontStyle => "font-style",
            Self::LineHeight => "line-height",
            Self::LetterSpacing => "letter-spacing",
        }
    }

    /// Parses a known property without rejecting unknown discovery identifiers.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|property| property.id() == id)
    }
}

/// Coarse support state for one public theme capability query.
///
/// This static state is an upper bound. Only a concrete render result can report actual
/// application, residuals, portability, or target admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ThemeSupportStateV1 {
    /// The complete public value domain is supported under the declared output contract.
    Unconditional,
    /// Some supported route exists, but selector, value, document, source, or output conditions
    /// remain.
    Conditional,
    /// The semantic target or facet does not apply to the queried family or output.
    NotApplicable,
    /// The complete known public value domain is explicitly unsupported.
    Unsupported,
    /// The current build cannot make a safe positive or negative claim.
    Unverified,
}

/// A versioned coarse theme-support result envelope.
///
/// V1 is an output-only renderer report. Persisted or untrusted decoding remains deferred until a
/// bounded consumer owns the forward-compatibility policy for result fields and reason IDs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ThemeCapabilityDescriptorV1 {
    schema_version: u32,
    claim_revision: u32,
    query: ThemeSupportQueryV1,
    state: ThemeSupportStateV1,
    reason_ids: Vec<String>,
}

impl ThemeCapabilityDescriptorV1 {
    /// Builds a descriptor from one renderer-owned support claim.
    ///
    /// The renderer must pass a deterministic, bounded list of stable reason identifiers.
    #[doc(hidden)]
    pub fn from_renderer_claim<const N: usize>(
        claim_revision: u32,
        query: ThemeSupportQueryV1,
        state: ThemeSupportStateV1,
        reason_ids: [&'static str; N],
    ) -> Self {
        assert!(
            N <= MAX_THEME_SUPPORT_REASON_IDS_V1,
            "renderer theme-support reasons must remain bounded"
        );
        Self {
            schema_version: THEME_SUPPORT_SCHEMA_VERSION_V1,
            claim_revision,
            query,
            state,
            reason_ids: reason_ids.into_iter().map(str::to_owned).collect(),
        }
    }

    /// Returns the result schema version.
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the renderer-owned support-claim revision.
    pub const fn claim_revision(&self) -> u32 {
        self.claim_revision
    }

    /// Returns the original, identifier-preserving query.
    pub const fn query(&self) -> &ThemeSupportQueryV1 {
        &self.query
    }

    /// Returns the coarse support state.
    pub const fn state(&self) -> ThemeSupportStateV1 {
        self.state
    }

    /// Returns the stable reason identifiers in deterministic priority order.
    pub fn reason_ids(&self) -> &[String] {
        &self.reason_ids
    }
}

/// A version 2 coarse theme-support result envelope.
///
/// V2 preserves the explicit subject from [`ThemeSupportQueryV2`] while retaining the same coarse
/// state and bounded stable-reason contract as V1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ThemeCapabilityDescriptorV2 {
    schema_version: u32,
    claim_revision: u32,
    query: ThemeSupportQueryV2,
    state: ThemeSupportStateV1,
    reason_ids: Vec<String>,
}

impl ThemeCapabilityDescriptorV2 {
    /// Builds a descriptor from one renderer-owned V2 support claim.
    ///
    /// The renderer must pass a deterministic, bounded list of stable reason identifiers.
    #[doc(hidden)]
    pub fn from_renderer_claim<const N: usize>(
        claim_revision: u32,
        query: ThemeSupportQueryV2,
        state: ThemeSupportStateV1,
        reason_ids: [&'static str; N],
    ) -> Self {
        assert!(
            N <= MAX_THEME_SUPPORT_REASON_IDS_V1,
            "renderer theme-support reasons must remain bounded"
        );
        Self {
            schema_version: THEME_SUPPORT_SCHEMA_VERSION_V2,
            claim_revision,
            query,
            state,
            reason_ids: reason_ids.into_iter().map(str::to_owned).collect(),
        }
    }

    /// Returns the result schema version.
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the renderer-owned support-claim revision.
    pub const fn claim_revision(&self) -> u32 {
        self.claim_revision
    }

    /// Returns the original V2 query.
    pub const fn query(&self) -> &ThemeSupportQueryV2 {
        &self.query
    }

    /// Returns the coarse support state.
    pub const fn state(&self) -> ThemeSupportStateV1 {
        self.state
    }

    /// Returns the stable reason identifiers in deterministic priority order.
    pub fn reason_ids(&self) -> &[String] {
        &self.reason_ids
    }
}

/// A known visual output target for theme-support discovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ThemeSupportOutputV1 {
    /// A sealed standalone SVG document.
    StandaloneSvg,
    /// An SVG document intended for a browser host.
    BrowserSvg,
    /// A PNG raster export.
    Png,
    /// A JPEG raster export.
    Jpeg,
    /// A PDF export.
    Pdf,
    /// Terminal or plain-text output, which has a separate styling contract.
    Ascii,
}

impl ThemeSupportOutputV1 {
    /// Stable enumeration view for catalogs and contract round-trip checks.
    pub const ALL: &'static [Self] = &[
        Self::StandaloneSvg,
        Self::BrowserSvg,
        Self::Png,
        Self::Jpeg,
        Self::Pdf,
        Self::Ascii,
    ];

    /// Stable wire identifier for this output target.
    pub const fn id(self) -> &'static str {
        match self {
            Self::StandaloneSvg => "standalone-svg",
            Self::BrowserSvg => "browser-svg",
            Self::Png => "png",
            Self::Jpeg => "jpeg",
            Self::Pdf => "pdf",
            Self::Ascii => "ascii",
        }
    }

    /// Parses a known output target without rejecting unknown discovery identifiers.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|output| output.id() == id)
    }
}

/// One public facet category used by coarse support discovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum ThemeSupportFacetV1 {
    /// One atomic rule facet.
    Rule(ThemeRuleFacetV1),
    /// Ordinal palette lookup for the semantic target.
    OrdinalPalette,
}

impl ThemeSupportFacetV1 {
    /// Stable wire identifier for this facet.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Rule(facet) => facet.id(),
            Self::OrdinalPalette => "ordinal-palette",
        }
    }

    /// Parses a known facet without rejecting unknown discovery identifiers.
    pub fn from_id(id: &str) -> Option<Self> {
        if id == "ordinal-palette" {
            Some(Self::OrdinalPalette)
        } else {
            ThemeRuleFacetV1::from_id(id).map(Self::from)
        }
    }
}

impl From<ThemeRuleFacetV1> for ThemeSupportFacetV1 {
    fn from(facet: ThemeRuleFacetV1) -> Self {
        Self::Rule(facet)
    }
}
