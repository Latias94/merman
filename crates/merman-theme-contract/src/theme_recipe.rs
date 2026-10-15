use std::fmt;

use serde::de::{self, Visitor};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{DiagramThemeSpecWireV1, ThemeDefinitionV1};

/// Schema version of the portable theme recipe file.
pub const THEME_RECIPE_SCHEMA_VERSION_V1: u32 = 1;

/// A versioned, self-contained theme recipe for editing and exchange.
///
/// The required root `schema_version: 1` selects this complete recipe contract, including the
/// version 1 complete specification. Definitions also validate their own authoring and expansion
/// versions. The `kind` tag is authoritative; payload kinds are never inferred from missing fields.
#[derive(Debug, Clone, PartialEq)]
#[expect(
    clippy::large_enum_variant,
    reason = "A theme recipe is a single exchange value, not a dense collection; retain its by-value payload"
)]
pub enum ThemeRecipeV1 {
    /// A lossless compact authoring definition.
    Definition {
        /// The complete authoring definition.
        definition: ThemeDefinitionV1,
    },
    /// A lossless complete editable theme specification.
    CompleteSpec {
        /// The complete version 1 specification.
        complete_spec: DiagramThemeSpecWireV1,
    },
}

impl ThemeRecipeV1 {
    /// Returns the persisted schema version for the complete recipe envelope.
    pub const fn schema_version(&self) -> u32 {
        THEME_RECIPE_SCHEMA_VERSION_V1
    }

    /// Returns the stable wire tag for this recipe variant.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Definition { .. } => "definition",
            Self::CompleteSpec { .. } => "complete_spec",
        }
    }
}

impl Serialize for ThemeRecipeV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut recipe = serializer.serialize_struct("ThemeRecipeV1", 3)?;
        recipe.serialize_field("schema_version", &self.schema_version())?;
        recipe.serialize_field("kind", self.kind())?;
        match self {
            Self::Definition { definition } => recipe.serialize_field("definition", definition)?,
            Self::CompleteSpec { complete_spec } => {
                recipe.serialize_field("complete_spec", complete_spec)?;
            }
        }
        recipe.end()
    }
}

struct RecipeSchemaVersionV1;

impl<'de> Deserialize<'de> for RecipeSchemaVersionV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct VersionVisitor;

        impl<'de> Visitor<'de> for VersionVisitor {
            type Value = RecipeSchemaVersionV1;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("theme recipe schema_version 1 as an integer")
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if value == u64::from(THEME_RECIPE_SCHEMA_VERSION_V1) {
                    Ok(RecipeSchemaVersionV1)
                } else {
                    Err(E::invalid_value(de::Unexpected::Unsigned(value), &self))
                }
            }
        }

        deserializer.deserialize_u32(VersionVisitor)
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
#[expect(
    clippy::large_enum_variant,
    reason = "Decode the single exchanged recipe without introducing a boxed public payload"
)]
enum ThemeRecipeV1Decode {
    Definition {
        schema_version: RecipeSchemaVersionV1,
        definition: ThemeDefinitionV1,
    },
    CompleteSpec {
        schema_version: RecipeSchemaVersionV1,
        complete_spec: DiagramThemeSpecWireV1,
    },
}

impl<'de> Deserialize<'de> for ThemeRecipeV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(match ThemeRecipeV1Decode::deserialize(deserializer)? {
            ThemeRecipeV1Decode::Definition {
                schema_version: RecipeSchemaVersionV1,
                definition,
            } => Self::Definition { definition },
            ThemeRecipeV1Decode::CompleteSpec {
                schema_version: RecipeSchemaVersionV1,
                complete_spec,
            } => Self::CompleteSpec { complete_spec },
        })
    }
}
