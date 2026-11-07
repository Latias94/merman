use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use serde::de::{self, Visitor};
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::finite::ContainsNonFiniteNumber;
use crate::wire::deserialize_optional_non_null;
use crate::{
    DiagramThemeSpecWireV1, MermaidThemeCompatibilityWireV1, ThemeDesignCanvasV1,
    ThemeDesignRuleV1, ThemeDesignStyleV1, ThemeDesignTokenV1, ThemeDesignTypographyV1,
    ThemeDesignValueDomain, ThemeValueV1,
};

/// A concrete authored appearance, independent of host preference and semantic variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeAppearance {
    /// An appearance designed for a light reading environment.
    Light,
    /// An appearance designed for a dark reading environment.
    Dark,
}

impl ThemeAppearance {
    /// The complete concrete appearance vocabulary in deterministic order.
    pub const ALL: [Self; 2] = [Self::Light, Self::Dark];

    /// Returns the stable JSON and command-line spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

impl fmt::Display for ThemeAppearance {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for ThemeAppearance {
    type Err = ThemeAppearanceParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "light" => Ok(Self::Light),
            "dark" => Ok(Self::Dark),
            _ => Err(ThemeAppearanceParseError),
        }
    }
}

impl<'de> Deserialize<'de> for ThemeAppearance {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct AppearanceVisitor;

        impl Visitor<'_> for AppearanceVisitor {
            type Value = ThemeAppearance;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("the string `light` or `dark`")
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                value.parse().map_err(E::custom)
            }
        }

        deserializer.deserialize_str(AppearanceVisitor)
    }
}

/// A string was not one of the concrete appearance identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("theme appearance must be `light` or `dark`; resolve host preferences before rendering")]
pub struct ThemeAppearanceParseError;

/// The closed appearance-local layer of a shared theme design.
///
/// Tokens and palettes replace entries by exact key. Local styles and ordered rules form the
/// second style layer. Canvas and typography replace their complete section. Mermaid values
/// retain their property-specific configuration ownership. Assets and admission requirements
/// belong to the shared design.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct ThemeAppearanceOverridesV1 {
    /// Appearance-local token replacements and additions.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tokens: BTreeMap<String, ThemeDesignTokenV1>,
    /// Semantic styles applied after the shared ordered rules.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub styles: BTreeMap<String, ThemeDesignStyleV1>,
    /// Ordered exceptions applied after the local semantic styles.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<ThemeDesignRuleV1>,
    /// Complete palette replacements keyed by semantic target.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub palettes: BTreeMap<String, ThemeValueV1<Vec<ThemeValueV1<String>>>>,
    /// A complete local canvas, or inheritance when omitted.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub canvas: Option<ThemeDesignCanvasV1>,
    /// A complete local typography section, or inheritance when omitted.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub typography: Option<ThemeDesignTypographyV1>,
    /// Restricted Mermaid compatibility overrides.
    #[serde(
        default,
        deserialize_with = "deserialize_optional_non_null",
        skip_serializing_if = "Option::is_none"
    )]
    pub mermaid: Option<MermaidThemeCompatibilityWireV1>,
}

impl ContainsNonFiniteNumber for ThemeAppearanceOverridesV1 {
    fn contains_non_finite_number(&self) -> bool {
        self.tokens.contains_non_finite_number()
            || self.styles.contains_non_finite_number()
            || self.rules.contains_non_finite_number()
            || self.palettes.contains_non_finite_number()
            || self.canvas.contains_non_finite_number()
            || self.typography.contains_non_finite_number()
            || self.mermaid.contains_non_finite_number()
    }
}

impl Serialize for ThemeAppearanceOverridesV1 {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.contains_non_finite_number() {
            return Err(S::Error::custom(
                "theme appearance contains a non-finite number",
            ));
        }
        Self::serialize(self, serializer)
    }
}

impl<'de> Deserialize<'de> for ThemeAppearanceOverridesV1 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ObjectVisitor;

        impl<'de> Visitor<'de> for ObjectVisitor {
            type Value = ThemeAppearanceOverridesV1;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("an appearance override object")
            }

            fn visit_map<M: de::MapAccess<'de>>(self, map: M) -> Result<Self::Value, M::Error> {
                ThemeAppearanceOverridesV1::deserialize(de::value::MapAccessDeserializer::new(map))
            }
        }

        deserializer.deserialize_map(ObjectVisitor)
    }
}

fn deserialize_schema_version_v1<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<u32, D::Error> {
    struct VersionVisitor;

    impl<'de> Visitor<'de> for VersionVisitor {
        type Value = u32;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("schema_version 1 as an integer")
        }

        fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
            if value == 1 {
                Ok(1)
            } else {
                Err(E::invalid_value(de::Unexpected::Unsigned(value), &self))
            }
        }
    }

    deserializer.deserialize_u32(VersionVisitor)
}

/// A declared binding with consumer and token locations in the complete editable document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeRecipeReferenceUseV1 {
    /// Exact document-local token identifier.
    pub token: String,
    /// JSON Pointer to the original shared or appearance-local consumer.
    pub path: String,
    /// JSON Pointer to the token entry supplying this appearance's value.
    pub token_path: String,
    /// Consumer-owned units, choices, and numeric bounds.
    pub domain: ThemeDesignValueDomain,
}

/// Binding inspection for one concrete appearance, or one unclassified concrete document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeAppearanceInspectionV1 {
    /// The declared appearance, or null for an unclassified concrete recipe.
    pub appearance: Option<ThemeAppearance>,
    /// Declared bindings; these do not identify winning rendered properties.
    pub references: Vec<ThemeRecipeReferenceUseV1>,
}

/// Complete-document admission and binding inspection without selecting a host appearance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeRecipeInspectionV1 {
    #[serde(deserialize_with = "deserialize_schema_version_v1")]
    schema_version: u32,
    /// Version of the inspected editable document.
    pub recipe_schema_version: u32,
    /// Declared concrete appearances, empty for an unclassified document.
    pub supported_appearances: Vec<ThemeAppearance>,
    /// The authored deterministic default, if classified.
    pub default_appearance: Option<ThemeAppearance>,
    /// Inspection of every admitted composition in deterministic appearance order.
    pub appearances: Vec<ThemeAppearanceInspectionV1>,
}

impl ThemeRecipeInspectionV1 {
    /// Constructs an inspection result after the compiler admits the complete document.
    pub fn new(
        recipe_schema_version: u32,
        supported_appearances: Vec<ThemeAppearance>,
        default_appearance: Option<ThemeAppearance>,
        appearances: Vec<ThemeAppearanceInspectionV1>,
    ) -> Self {
        Self {
            schema_version: 1,
            recipe_schema_version,
            supported_appearances,
            default_appearance,
            appearances,
        }
    }

    /// Returns the inspection result schema version.
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
}

/// One selected concrete specification after admission of the entire editable document.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterializedThemeRecipeV1 {
    #[serde(deserialize_with = "deserialize_schema_version_v1")]
    schema_version: u32,
    /// Version of the editable source document.
    pub recipe_schema_version: u32,
    /// All appearances declared by that document.
    pub supported_appearances: Vec<ThemeAppearance>,
    /// Its authored default, independent of the requested selection.
    pub default_appearance: Option<ThemeAppearance>,
    /// The selected concrete appearance, or null for an unclassified recipe.
    pub effective_appearance: Option<ThemeAppearance>,
    /// The concrete version-1 specification consumed by the existing compiler.
    pub spec: DiagramThemeSpecWireV1,
    /// Declared references in the selected composition.
    pub references: Vec<ThemeRecipeReferenceUseV1>,
}

impl MaterializedThemeRecipeV1 {
    /// Constructs a selected result after whole-document compiler admission.
    pub fn new(
        inspection: &ThemeRecipeInspectionV1,
        effective_appearance: Option<ThemeAppearance>,
        spec: DiagramThemeSpecWireV1,
        references: Vec<ThemeRecipeReferenceUseV1>,
    ) -> Self {
        Self {
            schema_version: 1,
            recipe_schema_version: inspection.recipe_schema_version,
            supported_appearances: inspection.supported_appearances.clone(),
            default_appearance: inspection.default_appearance,
            effective_appearance,
            spec,
            references,
        }
    }

    /// Returns the materialization result schema version.
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }
}
