use std::collections::BTreeMap;
use std::sync::Arc;

use serde::Serialize;

use super::{Appearance, Color, Form, Roles, ShadowLayer, Texture, Typography};

/// Concrete canvas decorations and extra padding. The base paint is in [`Roles::canvas`].
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[non_exhaustive]
pub struct Canvas {
    /// Ordered texture layers.
    pub textures: Vec<Texture>,
    /// Extra canvas padding in pixels.
    pub padding_px: f64,
}

/// Color slots for line marks, filled areas, and labels on filled areas.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[non_exhaustive]
pub struct DataPalette {
    /// Series inks for thin marks.
    pub ink: Arc<[Color]>,
    /// Series area colors.
    pub fill: Arc<[Color]>,
    /// Text colors paired with the fills.
    pub fill_ink: Arc<[Color]>,
    /// Whether marks should also differ by pattern.
    pub pattern_encoding: bool,
}

impl DataPalette {
    /// Selects an ink by ordinal, cycling at the end of the palette.
    pub fn ink(&self, ordinal: usize) -> Color {
        self.ink[ordinal % self.ink.len()]
    }

    /// Selects a fill by ordinal, cycling at the end of the palette.
    pub fn fill(&self, ordinal: usize) -> Color {
        self.fill[ordinal % self.fill.len()]
    }

    /// Selects the corresponding text color by ordinal.
    pub fn fill_ink(&self, ordinal: usize) -> Color {
        self.fill_ink[ordinal % self.fill_ink.len()]
    }
}

/// Concrete depth treatment for a diagram surface.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Elevation {
    /// Draw only the foreground surface.
    #[default]
    Flat,
    /// Draw a colored stack of shadows from back to front.
    Shadows {
        /// Ordered shadow layers.
        layers: Arc<[ShadowLayer]>,
    },
    /// Draw a colored glow around the surface.
    Glow {
        /// Glow color.
        color: Color,
        /// Gaussian blur radius in pixels.
        blur: f64,
        /// Glow opacity.
        opacity: f64,
    },
}

/// A contrast result reported when resolving a theme.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ThemeNotice {
    /// A foreground and background pair missed its target contrast ratio.
    LowContrast {
        /// The foreground's field or data slot.
        foreground: String,
        /// The background's role.
        background: &'static str,
        /// Observed contrast ratio.
        ratio: f64,
        /// Required contrast ratio.
        minimum: f64,
    },
    /// The compiler adjusted a foreground to improve its contrast.
    ContrastAdjusted {
        /// The foreground's field or data slot.
        foreground: String,
        /// The replacement color.
        color: Color,
    },
}

/// Values a family can read directly while measuring or rendering a diagram.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[non_exhaustive]
pub struct FamilyTheme {
    /// Semantic role colors and paints.
    pub roles: Roles,
    /// Categorical data colors.
    pub data: DataPalette,
    /// Font stacks and text settings.
    pub typography: Typography,
    /// Shape, connector, and chart styling.
    pub form: Form,
    /// Surface depth.
    pub elevation: Elevation,
    /// Apply categorical fills to ordinary nodes in source order.
    pub node_ordinal_palette: bool,
}

/// An immutable theme compiled for one appearance, normally shared through `Arc`.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ResolvedThemeV2 {
    pub(super) id: Option<String>,
    pub(super) appearance: Appearance,
    #[serde(flatten)]
    pub(super) base: FamilyTheme,
    pub(super) canvas: Canvas,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub(super) family_overrides: BTreeMap<String, FamilyTheme>,
    #[serde(skip)]
    pub(super) fingerprint: [u8; 32],
    #[serde(skip)]
    pub(super) notices: Vec<ThemeNotice>,
}

impl ResolvedThemeV2 {
    /// The identifier supplied in the theme document.
    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    /// The selected appearance.
    pub const fn appearance(&self) -> Appearance {
        self.appearance
    }

    /// The shared role colors and paints.
    pub const fn roles(&self) -> &Roles {
        &self.base.roles
    }

    /// The shared categorical palette.
    pub const fn data(&self) -> &DataPalette {
        &self.base.data
    }

    /// The shared text settings.
    pub const fn typography(&self) -> &Typography {
        &self.base.typography
    }

    /// The shared shape and connector settings.
    pub const fn form(&self) -> &Form {
        &self.base.form
    }

    /// Canvas decoration and padding.
    pub const fn canvas(&self) -> &Canvas {
        &self.canvas
    }

    /// The shared depth treatment.
    pub const fn elevation(&self) -> &Elevation {
        &self.base.elevation
    }

    /// Returns precomputed overrides for a family, or the shared values.
    pub fn family(&self, id: &str) -> &FamilyTheme {
        self.family_overrides.get(id).unwrap_or(&self.base)
    }

    /// SHA-256 of the canonical compiled theme, excluding notices.
    pub const fn fingerprint(&self) -> &[u8; 32] {
        &self.fingerprint
    }

    /// Contrast results from compilation.
    pub fn notices(&self) -> &[ThemeNotice] {
        &self.notices
    }
}
