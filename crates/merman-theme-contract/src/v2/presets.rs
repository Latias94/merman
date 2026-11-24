use std::fmt;
use std::sync::{Arc, OnceLock};

use serde::Serialize;

use super::{Appearance, ResolvedThemeV2, ThemeDocumentV2, ThemeError, json};

const LIGHT: &[Appearance] = &[Appearance::Light];
const DARK: &[Appearance] = &[Appearance::Dark];
const PAIRED: &[Appearance] = &[Appearance::Light, Appearance::Dark];

macro_rules! builtin_presets {
    ($($(#[$doc:meta])* $variant:ident => ($id:literal, $appearances:ident, $default:ident)),+ $(,)?) => {
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

            /// Available concrete appearances as declared in the preset document.
            pub const fn supported_appearances(self) -> &'static [Appearance] {
                match self {
                    $(Self::$variant => $appearances,)+
                }
            }

            /// Appearance selected when the caller omits it.
            pub const fn default_appearance(self) -> Appearance {
                match self {
                    $(Self::$variant => Appearance::$default,)+
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
    Studio => ("studio", PAIRED, Light),
    /// Compact data and architecture design.
    Prism => ("prism", PAIRED, Light),
    /// Warm paper with serif titles.
    Paper => ("paper", PAIRED, Light),
    /// Monochrome for reading and print.
    Monograph => ("monograph", PAIRED, Light),
    /// Warm editor palette.
    Gruvbox => ("gruvbox", PAIRED, Dark),
    /// Indigo and daylight editor palette.
    Tokyo => ("tokyo", PAIRED, Dark),
    /// Layered cut paper.
    PaperCut => ("paper-cut", LIGHT, Light),
    /// Two-ink risograph design.
    Riso => ("riso", LIGHT, Light),
    /// Ruled copy paper.
    CarbonCopy => ("carbon-copy", LIGHT, Light),
    /// Cyanotype engineering grid.
    Blueprint => ("blueprint", DARK, Dark),
    /// Dark lacquer and gold.
    Lacquer => ("lacquer", DARK, Dark),
    /// Dark interface with bright accents.
    Cyberpunk => ("cyberpunk", DARK, Dark),
    /// Printed comic colors and dots.
    PopArt => ("pop-art", LIGHT, Light),
}

impl BuiltinPreset {
    /// Returns the decoded editable document supplied by the registry.
    pub fn document(self) -> Result<&'static ThemeDocumentV2, ThemeError> {
        static DOCUMENTS: [OnceLock<Result<ThemeDocumentV2, ThemeError>>;
            BuiltinPreset::ALL.len()] = [const { OnceLock::new() }; BuiltinPreset::ALL.len()];
        DOCUMENTS[self as usize]
            .get_or_init(|| json::parse(self.json().as_bytes()))
            .as_ref()
            .map_err(Clone::clone)
    }

    pub(super) fn resolved(
        self,
        appearance: Appearance,
        compile: impl FnOnce() -> Result<Arc<ResolvedThemeV2>, ThemeError>,
    ) -> Result<Arc<ResolvedThemeV2>, ThemeError> {
        type Slot = OnceLock<Result<Arc<ResolvedThemeV2>, ThemeError>>;
        static THEMES: [[Slot; 2]; BuiltinPreset::ALL.len()] =
            [const { [const { OnceLock::new() }; 2] }; BuiltinPreset::ALL.len()];
        let index = match appearance {
            Appearance::Light => 0,
            Appearance::Dark => 1,
        };
        THEMES[self as usize][index].get_or_init(compile).clone()
    }
}

impl fmt::Display for BuiltinPreset {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
