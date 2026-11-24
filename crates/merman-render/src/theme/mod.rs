//! Native theme selection and visual settings used by family renderers.

use std::sync::Arc;

pub(crate) mod canvas_svg;

pub use merman_theme_contract::v2::{
    Appearance, BuiltinPreset, Color, FamilyTheme, ResolvedThemeV2, ThemeCapabilityFlags,
    ThemeCompiler, ThemeDocumentV2, ThemeError, ThemeNotice,
};

/// Immutable native theme and an optional host canvas color.
#[derive(Clone, Debug)]
pub struct NativeTheme {
    resolved: Arc<ResolvedThemeV2>,
    background: Option<Color>,
}

impl NativeTheme {
    /// Uses the theme's resolved canvas paint and appearance.
    pub fn new(resolved: Arc<ResolvedThemeV2>) -> Self {
        Self {
            resolved,
            background: None,
        }
    }

    /// Replaces the canvas base paint while retaining its textures.
    pub fn with_background(mut self, background: Color) -> Self {
        self.background = Some(background);
        self
    }

    /// Returns the shared theme data.
    pub fn resolved(&self) -> &ResolvedThemeV2 {
        &self.resolved
    }

    /// Returns the explicitly selected base color, if supplied.
    pub const fn background(&self) -> Option<Color> {
        self.background
    }
}

/// Connects a diagram family's measurement and rendering to its native theme values.
pub trait FamilyThemeAdapter {
    /// Logical family that consumes the values.
    const FAMILY: crate::DiagramFamilyId;
    /// Visual capabilities implemented by this family.
    const CAPS: ThemeCapabilityFlags;

    /// Returns the family-specific settings or shared theme defaults.
    fn resolve(theme: &ResolvedThemeV2) -> &FamilyTheme {
        theme.family(Self::FAMILY.as_str())
    }
}

#[cfg(any(feature = "diagram-info", feature = "diagram-pie"))]
pub(crate) fn font_family_css(families: &[String]) -> String {
    let mut css = String::new();
    for (index, family) in families.iter().enumerate() {
        if index != 0 {
            css.push_str(", ");
        }
        if crate::diagram_theme::GenericFontFamily::from_css_keyword(family).is_some() {
            css.push_str(family);
        } else {
            cssparser::serialize_string(family, &mut css)
                .expect("writing a CSS string to String cannot fail");
        }
    }
    css
}
