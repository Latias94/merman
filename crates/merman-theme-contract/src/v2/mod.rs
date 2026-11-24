//! Portable authoring and compiled values for Merman native themes.

mod color;
mod compile;
mod document;
mod json;
mod presets;
mod resolved;
mod validate;

pub use color::{Color, InvalidColor};
pub use compile::{ThemeCompiler, ThemeError};
pub use document::{
    Appearance, Category, ChartPolish, Density, EdgeLabelBackground, EdgeStyle, FontRole, Form,
    GradientStop, LinearGradient, MarkerStyle, Paint, Roles, ShadowLayer, Texture, ThemeDocumentV2,
    Typography,
};
pub use presets::BuiltinPreset;
pub use resolved::{Canvas, DataPalette, Elevation, FamilyTheme, ResolvedThemeV2, ThemeNotice};

/// Version carried by a native theme document.
pub const SCHEMA_VERSION: u32 = 2;

/// Static visual features supported by a diagram renderer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(transparent)]
pub struct ThemeCapabilityFlags(u64);

impl ThemeCapabilityFlags {
    /// Canvas paint and textures.
    pub const CANVAS: Self = Self(1 << 0);
    /// Ordinary node surfaces.
    pub const NODE_SURFACE: Self = Self(1 << 1);
    /// Group and container surfaces.
    pub const CONTAINER: Self = Self(1 << 2);
    /// Notes and callouts.
    pub const CALLOUT: Self = Self(1 << 3);
    /// Emphasized elements.
    pub const EMPHASIS: Self = Self(1 << 4);
    /// Connectors and guides.
    pub const CONNECTOR: Self = Self(1 << 5);
    /// Series colors for narrow marks.
    pub const DATA_INK: Self = Self(1 << 6);
    /// Series colors for filled areas.
    pub const DATA_FILL: Self = Self(1 << 7);
    /// Status-specific colors.
    pub const STATUS_COLORS: Self = Self(1 << 8);
    /// Surface elevation.
    pub const ELEVATION: Self = Self(1 << 9);
    /// Configurable corner radii.
    pub const RADIUS: Self = Self(1 << 10);
    /// Configurable connector style.
    pub const EDGE_STYLE: Self = Self(1 << 11);
    /// Configurable arrowhead style.
    pub const MARKER_STYLE: Self = Self(1 << 12);
    /// Chart-specific form settings.
    pub const CHART_POLISH: Self = Self(1 << 13);
    /// Theme typography.
    pub const TYPOGRAPHY: Self = Self(1 << 14);
    /// Pattern differentiation for categorical marks.
    pub const PATTERN_ENCODING: Self = Self(1 << 15);

    /// Combines two capability sets.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Returns whether this set contains all of the requested capabilities.
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// Returns the stable wire bit mask.
    pub const fn bits(self) -> u64 {
        self.0
    }
}
