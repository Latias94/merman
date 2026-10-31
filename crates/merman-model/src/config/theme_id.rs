use std::fmt;
use std::str::FromStr;

// Source: Mermaid 12.2.0 `packages/mermaid/src/themes/index.js`.
macro_rules! define_mermaid_theme_ids {
    ($(($variant:ident, $name:literal)),+ $(,)?) => {
        /// A theme identifier from the pinned Mermaid theme catalog.
        ///
        /// This type is deliberately separate from Merman's compiled theme presets. It models
        /// only the upstream `MermaidConfig.theme` contract and is therefore also the source of
        /// truth for bindings, CLI discovery, and compatibility projection. Mermaid's `"null"`
        /// configuration sentinel is not a registered theme and is deliberately excluded.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[non_exhaustive]
        pub enum MermaidThemeId {
            $( $variant, )+
        }

        impl MermaidThemeId {
            /// All theme identifiers supported by the pinned Mermaid baseline, in catalog order.
            pub const ALL: &'static [Self] = &[
                $(Self::$variant,)+
            ];

            /// The exact upstream spellings in the same order as [`Self::ALL`].
            pub const NAMES: &'static [&'static str] = &[
                $($name,)+
            ];

            /// Returns the exact upstream spelling used in Mermaid configuration and mmdc files.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $name,)+
                }
            }

            /// Parses an upstream theme identifier without applying a fallback.
            pub fn parse(value: &str) -> Result<Self, MermaidThemeIdParseError> {
                value.parse()
            }

            /// Returns whether the selected upstream theme uses a dark visual palette.
            ///
            /// This classification does not set or imply Mermaid's independent
            /// `themeVariables.darkMode` compatibility value.
            pub const fn is_dark(self) -> bool {
                matches!(
                    self,
                    Self::Dark | Self::NeoDark | Self::ReduxDark | Self::ReduxDarkColor
                )
            }
        }

        impl Default for MermaidThemeId {
            fn default() -> Self {
                Self::Default
            }
        }

        impl fmt::Display for MermaidThemeId {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl FromStr for MermaidThemeId {
            type Err = MermaidThemeIdParseError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                match value {
                    $($name => Ok(Self::$variant),)+
                    _ => Err(MermaidThemeIdParseError::new(value)),
                }
            }
        }

    };
}

define_mermaid_theme_ids![
    (Default, "default"),
    (Base, "base"),
    (Dark, "dark"),
    (Forest, "forest"),
    (Neutral, "neutral"),
    (Neo, "neo"),
    (NeoDark, "neo-dark"),
    (Redux, "redux"),
    (ReduxDark, "redux-dark"),
    (ReduxColor, "redux-color"),
    (ReduxDarkColor, "redux-dark-color"),
];

/// Error returned when a string is not one of the pinned Mermaid theme identifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MermaidThemeIdParseError {
    input: String,
}

impl MermaidThemeIdParseError {
    fn new(input: &str) -> Self {
        Self {
            input: input.to_string(),
        }
    }

    /// Returns the unsupported identifier supplied to the parser.
    pub fn input(&self) -> &str {
        &self.input
    }
}

impl fmt::Display for MermaidThemeIdParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unsupported Mermaid theme `{}`", self.input)
    }
}

impl std::error::Error for MermaidThemeIdParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_theme_names_match_typed_catalog() {
        assert_eq!(
            MermaidThemeId::NAMES,
            &[
                "default",
                "base",
                "dark",
                "forest",
                "neutral",
                "neo",
                "neo-dark",
                "redux",
                "redux-dark",
                "redux-color",
                "redux-dark-color"
            ]
        );
        assert_eq!(MermaidThemeId::ALL.len(), MermaidThemeId::NAMES.len());
        assert_eq!(
            MermaidThemeId::ALL
                .iter()
                .copied()
                .map(MermaidThemeId::as_str)
                .collect::<Vec<_>>(),
            MermaidThemeId::NAMES
        );
    }

    #[test]
    fn typed_theme_ids_round_trip_without_fallback() {
        for &theme in MermaidThemeId::ALL {
            assert_eq!(MermaidThemeId::parse(theme.as_str()).unwrap(), theme);
            assert_eq!(theme.to_string(), theme.as_str());
        }

        for invalid in ["unknown", "null"] {
            let error = MermaidThemeId::parse(invalid).unwrap_err();
            assert_eq!(error.input(), invalid);
            assert_eq!(
                error.to_string(),
                format!("unsupported Mermaid theme `{invalid}`")
            );
        }
    }
}
