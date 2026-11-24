use std::fmt;

use serde::Serialize;

use super::{Appearance, ThemeDocumentV2, ThemeError, json};

macro_rules! builtin_presets {
    ($($(#[$doc:meta])* $variant:ident => $id:literal),+ $(,)?) => {
        /// A preset embedded in the native theme registry.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
        #[non_exhaustive]
        pub enum BuiltinPreset {
            $($(#[$doc])* #[serde(rename = $id)] $variant,)+
        }

        impl BuiltinPreset {
            /// Presets in catalog order.
            pub const ALL: &'static [Self] = &[$(Self::$variant,)+];

            /// Parses a known preset identifier.
            pub fn from_id(id: &str) -> Option<Self> {
                match id {
                    $($id => Some(Self::$variant),)+
                    _ => None,
                }
            }

            /// The stable preset identifier used by bindings and authoring documents.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $id,)+
                }
            }

            /// The editable JSON document shipped with the preset.
            pub const fn json(self) -> &'static str {
                match self {
                    $(Self::$variant => include_str!(concat!("../../assets/presets/", $id, ".theme.json")),)+
                }
            }
        }
    };
}

builtin_presets! {
    /// Neutral technical reading.
    Studio => "studio",
    /// Compact data and architecture design.
    Prism => "prism",
    /// Warm paper with serif titles.
    Paper => "paper",
    /// Monochrome for reading and print.
    Monograph => "monograph",
    /// Warm editor palette.
    Gruvbox => "gruvbox",
    /// Indigo and daylight editor palette.
    Tokyo => "tokyo",
    /// Layered cut paper.
    PaperCut => "paper-cut",
    /// Two-ink risograph design.
    Riso => "riso",
    /// Ruled copy paper.
    CarbonCopy => "carbon-copy",
    /// Cyanotype engineering grid.
    Blueprint => "blueprint",
    /// Dark lacquer and gold.
    Lacquer => "lacquer",
    /// Dark interface with bright accents.
    Cyberpunk => "cyberpunk",
    /// Printed comic colors and dots.
    PopArt => "pop-art",
}

impl BuiltinPreset {
    /// Decodes the editable document supplied by the registry.
    pub fn document(self) -> Result<ThemeDocumentV2, ThemeError> {
        json::parse(self.json().as_bytes())
    }

    /// Available concrete appearances as declared in the preset document.
    pub fn appearances(self) -> Result<Vec<Appearance>, ThemeError> {
        Ok(self.document()?.appearances().collect())
    }
}

impl fmt::Display for BuiltinPreset {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
