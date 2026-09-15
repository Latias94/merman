use std::collections::BTreeMap;
use std::fmt;

use serde::de::{Error as _, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use crate::ThemeRuleFacetV1;
use crate::canonical_json::canonical_json_bytes;

/// Schema version used by the version 1 subject-based theme-support discovery query.
pub const THEME_SUPPORT_SCHEMA_VERSION_V1: u32 = 1;

/// Maximum number of stable reason identifiers in one coarse support descriptor.
pub const MAX_THEME_SUPPORT_REASON_IDS_V1: usize = 8;

const MAX_UNKNOWN_SUBJECT_FIELDS_V1: usize = 16;
const MAX_UNKNOWN_SUBJECT_KIND_BYTES_V1: usize = 64;
const MAX_UNKNOWN_SUBJECT_FIELD_KEY_BYTES_V1: usize = 64;
const MAX_UNKNOWN_SUBJECT_CONTAINER_ITEMS_V1: usize = 32;
const MAX_UNKNOWN_SUBJECT_VALUE_DEPTH_V1: usize = 4;
const MAX_UNKNOWN_SUBJECT_ENCODED_BYTES_V1: usize = 4 * 1024;

/// A versioned theme-support query with an explicit tagged subject.
///
/// Family-wide base typography is separate from semantic-target rules. Unknown identifiers
/// remain decodable discovery input and resolve to an unverified renderer claim.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeSupportQueryV1 {
    schema_version: u32,
    family: String,
    output: String,
    subject: ThemeSupportSubjectV1,
}

impl ThemeSupportQueryV1 {
    /// Builds a subject query from a known target facet or ordinal palette.
    pub fn for_target<F: Into<ThemeSupportFacetV1>>(
        family: impl Into<String>,
        output: ThemeSupportOutputV1,
        target: impl Into<String>,
        facet: F,
    ) -> Self {
        match facet.into() {
            ThemeSupportFacetV1::Rule(facet) => Self::rule(family, output, target, facet),
            ThemeSupportFacetV1::OrdinalPalette => Self::ordinal_palette(family, output, target),
        }
    }

    /// Builds a query for one semantic-target rule facet.
    pub fn rule(
        family: impl Into<String>,
        output: ThemeSupportOutputV1,
        target: impl Into<String>,
        facet: ThemeRuleFacetV1,
    ) -> Self {
        Self {
            schema_version: THEME_SUPPORT_SCHEMA_VERSION_V1,
            family: family.into(),
            output: output.id().to_owned(),
            subject: ThemeSupportSubjectV1::Rule {
                target: target.into(),
                facet: facet.id().to_owned(),
            },
        }
    }

    /// Builds a query for one semantic target's ordinal palette.
    pub fn ordinal_palette(
        family: impl Into<String>,
        output: ThemeSupportOutputV1,
        target: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: THEME_SUPPORT_SCHEMA_VERSION_V1,
            family: family.into(),
            output: output.id().to_owned(),
            subject: ThemeSupportSubjectV1::OrdinalPalette {
                target: target.into(),
            },
        }
    }

    /// Builds a query for one family-wide base typography property.
    pub fn base_typography(
        family: impl Into<String>,
        output: ThemeSupportOutputV1,
        property: ThemeSupportBaseTypographyPropertyV1,
    ) -> Self {
        Self {
            schema_version: THEME_SUPPORT_SCHEMA_VERSION_V1,
            family: family.into(),
            output: output.id().to_owned(),
            subject: ThemeSupportSubjectV1::BaseTypography {
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
    pub const fn subject(&self) -> &ThemeSupportSubjectV1 {
        &self.subject
    }
}

/// The subject of a version 1 theme-support query.
///
/// The wire representation is tagged by `kind`. Known subjects preserve unknown target, facet,
/// and property identifiers as strings so the renderer can return `Unverified` instead of
/// rejecting forward-compatible discovery input.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ThemeSupportSubjectV1 {
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
    Unknown(ThemeSupportUnknownSubjectV1),
}

/// A bounded, forward-compatible subject payload unknown to this contract version.
///
/// Opaque values are stored as canonical JSON so decoding and re-encoding preserves additive
/// fields without exposing an unbounded arbitrary-value tree as part of the Rust API.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ThemeSupportUnknownSubjectV1 {
    kind: String,
    fields: BTreeMap<String, String>,
}

impl ThemeSupportUnknownSubjectV1 {
    /// Returns the preserved unknown subject tag.
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// Returns one preserved opaque field as canonical JSON.
    pub fn field_json(&self, key: &str) -> Option<&str> {
        self.fields.get(key).map(String::as_str)
    }

    /// Returns the number of preserved opaque fields.
    pub fn field_count(&self) -> usize {
        self.fields.len()
    }
}

impl Serialize for ThemeSupportSubjectV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let field_count = match self {
            Self::Rule { .. } => 3,
            Self::OrdinalPalette { .. } | Self::BaseTypography { .. } => 2,
            Self::Unknown(unknown) => 1 + unknown.fields.len(),
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
            Self::Unknown(unknown) => {
                map.serialize_entry("kind", &unknown.kind)?;
                for (key, encoded) in &unknown.fields {
                    let value = serde_json::from_str::<Value>(encoded).map_err(|error| {
                        serde::ser::Error::custom(format!(
                            "invalid preserved theme support subject field `{key}`: {error}"
                        ))
                    })?;
                    map.serialize_entry(key, &value)?;
                }
            }
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for ThemeSupportSubjectV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut fields = deserializer.deserialize_map(UniqueSubjectFieldsVisitor)?;
        let kind = fields
            .remove("kind")
            .and_then(|value| value.as_str().map(str::to_owned))
            .ok_or_else(|| D::Error::custom("theme support subject requires string `kind`"))?;
        match kind.as_str() {
            "rule" => {
                let target = take_required_string::<D>(&mut fields, "target", &kind)?;
                let facet = take_required_string::<D>(&mut fields, "facet", &kind)?;
                reject_known_subject_extra::<D>(&kind, &fields)?;
                Ok(Self::Rule { target, facet })
            }
            "ordinal-palette" => {
                let target = take_required_string::<D>(&mut fields, "target", &kind)?;
                reject_known_subject_extra::<D>(&kind, &fields)?;
                Ok(Self::OrdinalPalette { target })
            }
            "base-typography" => {
                let property = take_required_string::<D>(&mut fields, "property", &kind)?;
                reject_known_subject_extra::<D>(&kind, &fields)?;
                Ok(Self::BaseTypography { property })
            }
            _ => {
                let fields = preserve_unknown_subject_fields::<D>(&kind, fields)?;
                Ok(Self::Unknown(ThemeSupportUnknownSubjectV1 { kind, fields }))
            }
        }
    }
}

struct UniqueSubjectFieldsVisitor;

impl<'de> Visitor<'de> for UniqueSubjectFieldsVisitor {
    type Value = BTreeMap<String, Value>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a theme support subject object with unique field names")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut fields = BTreeMap::new();
        while let Some(key) = map.next_key::<String>()? {
            if fields.contains_key(&key) {
                return Err(A::Error::custom(format!(
                    "theme support subject contains duplicate field `{key}`"
                )));
            }
            fields.insert(key, map.next_value::<Value>()?);
        }
        Ok(fields)
    }
}

fn take_required_string<'de, D>(
    fields: &mut BTreeMap<String, Value>,
    key: &str,
    kind: &str,
) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    fields
        .remove(key)
        .ok_or_else(|| {
            D::Error::custom(format!("theme support subject `{kind}` requires `{key}`"))
        })?
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| {
            D::Error::custom(format!(
                "theme support subject `{kind}` field `{key}` must be a string"
            ))
        })
}

fn reject_known_subject_extra<'de, D>(
    kind: &str,
    fields: &BTreeMap<String, Value>,
) -> Result<(), D::Error>
where
    D: Deserializer<'de>,
{
    if fields.is_empty() {
        Ok(())
    } else {
        Err(D::Error::custom(format!(
            "unexpected fields for theme support subject `{kind}`"
        )))
    }
}

fn preserve_unknown_subject_fields<'de, D>(
    kind: &str,
    fields: BTreeMap<String, Value>,
) -> Result<BTreeMap<String, String>, D::Error>
where
    D: Deserializer<'de>,
{
    if kind.is_empty() || kind.len() > MAX_UNKNOWN_SUBJECT_KIND_BYTES_V1 {
        return Err(D::Error::custom(
            "theme support subject has an invalid opaque kind",
        ));
    }
    if fields.len() > MAX_UNKNOWN_SUBJECT_FIELDS_V1 {
        return Err(D::Error::custom(format!(
            "theme support subject `{kind}` exceeds the opaque field limit"
        )));
    }
    let mut encoded_bytes = kind.len();
    let mut preserved = BTreeMap::new();
    for (key, value) in fields {
        if key.len() > MAX_UNKNOWN_SUBJECT_FIELD_KEY_BYTES_V1 {
            return Err(D::Error::custom(format!(
                "theme support subject `{kind}` has an oversized opaque field key"
            )));
        }
        validate_unknown_subject_value::<D>(kind, &value, 0)?;
        let encoded = canonical_json_bytes(&value).map_err(|error| {
            D::Error::custom(format!(
                "theme support subject `{kind}` has a non-canonical opaque field: {error}"
            ))
        })?;
        encoded_bytes = encoded_bytes
            .checked_add(key.len())
            .and_then(|bytes| bytes.checked_add(encoded.len()))
            .ok_or_else(|| D::Error::custom("theme support subject opaque payload overflow"))?;
        if encoded_bytes > MAX_UNKNOWN_SUBJECT_ENCODED_BYTES_V1 {
            return Err(D::Error::custom(format!(
                "theme support subject `{kind}` exceeds the opaque byte limit"
            )));
        }
        let encoded = String::from_utf8(encoded)
            .map_err(|_| D::Error::custom("canonical theme support JSON was not UTF-8"))?;
        preserved.insert(key, encoded);
    }
    Ok(preserved)
}

fn validate_unknown_subject_value<'de, D>(
    kind: &str,
    value: &Value,
    depth: usize,
) -> Result<(), D::Error>
where
    D: Deserializer<'de>,
{
    if depth > MAX_UNKNOWN_SUBJECT_VALUE_DEPTH_V1 {
        return Err(D::Error::custom(format!(
            "theme support subject `{kind}` exceeds the opaque nesting limit"
        )));
    }
    match value {
        Value::Array(values) => {
            if values.len() > MAX_UNKNOWN_SUBJECT_CONTAINER_ITEMS_V1 {
                return Err(D::Error::custom(format!(
                    "theme support subject `{kind}` exceeds the opaque array item limit"
                )));
            }
            for value in values {
                validate_unknown_subject_value::<D>(kind, value, depth + 1)?;
            }
        }
        Value::Object(values) => {
            if values.len() > MAX_UNKNOWN_SUBJECT_CONTAINER_ITEMS_V1 {
                return Err(D::Error::custom(format!(
                    "theme support subject `{kind}` exceeds the opaque object field limit"
                )));
            }
            for (key, value) in values {
                if key.len() > MAX_UNKNOWN_SUBJECT_FIELD_KEY_BYTES_V1 {
                    return Err(D::Error::custom(format!(
                        "theme support subject `{kind}` has an oversized nested field key"
                    )));
                }
                validate_unknown_subject_value::<D>(kind, value, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// A family-wide typography property available to support discovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ThemeSupportBaseTypographyPropertyV1 {
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
    /// Word spacing.
    WordSpacing,
    /// Text transform.
    Transform,
    /// Text decoration.
    Decoration,
    /// Text alignment.
    TextAlign,
    /// White-space handling.
    WhiteSpace,
    /// Text wrapping policy.
    Wrap,
}

impl ThemeSupportBaseTypographyPropertyV1 {
    /// Current alpha enumeration view for catalogs and contract round-trip checks.
    pub const ALL: &'static [Self] = &[
        Self::FontStack,
        Self::FontSize,
        Self::FontWeight,
        Self::FontStyle,
        Self::LineHeight,
        Self::LetterSpacing,
        Self::WordSpacing,
        Self::Transform,
        Self::Decoration,
        Self::TextAlign,
        Self::WhiteSpace,
        Self::Wrap,
    ];

    /// Version 1 wire identifier for this base typography property.
    ///
    /// These identifiers remain unfrozen until the C7a rollout gate closes.
    pub const fn id(self) -> &'static str {
        match self {
            Self::FontStack => "font-stack",
            Self::FontSize => "font-size",
            Self::FontWeight => "font-weight",
            Self::FontStyle => "font-style",
            Self::LineHeight => "line-height",
            Self::LetterSpacing => "letter-spacing",
            Self::WordSpacing => "word-spacing",
            Self::Transform => "transform",
            Self::Decoration => "decoration",
            Self::TextAlign => "text-align",
            Self::WhiteSpace => "white-space",
            Self::Wrap => "wrap",
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
/// Preserves the query subject, coarse state, and bounded stable reason identifiers.
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

    /// Returns the original query.
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
