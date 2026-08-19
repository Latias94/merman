use merman_core::MermaidConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, ResolvedDiagramTheme, ThemeTypographyProperty,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InheritedFontStackOutcome {
    Inactive,
    Typed,
    ConfigOwned,
    Unsupported,
}

/// Shared resolution core for families whose terminal text inherits one root font stack.
///
/// The owning family remains responsible for its writer and occurrence receipts. This core shares
/// only the final value and the typed/config/unsupported ownership outcome.
#[derive(Debug)]
pub(crate) struct InheritedFontStackPlan {
    font_family_css: Box<str>,
    outcome: InheritedFontStackOutcome,
    typed_font_stack_requested: bool,
}

impl InheritedFontStackPlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
    ) -> Self {
        let configured_font = crate::config::config_font_family_css(effective_config.as_value());
        let Some(theme) = theme else {
            return Self {
                font_family_css: configured_font.into_boxed_str(),
                outcome: InheritedFontStackOutcome::Inactive,
                typed_font_stack_requested: false,
            };
        };

        let config_owns_font_stack =
            ["themeVariables.fontFamily", "fontFamily"]
                .into_iter()
                .any(|path| {
                    merman_core::__private::config_path_overrides_typed_default(
                        effective_config,
                        path,
                    )
                });
        let mut typed_font_stack = false;
        let mut unsupported_typography = false;
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontStack)
                    if route.disposition() == FamilyThemeDisposition::TypedAdapter =>
                {
                    typed_font_stack = true;
                }
                FamilyThemeMechanism::BaseTypography(_) => unsupported_typography = true,
                FamilyThemeMechanism::RuleFacet { .. }
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {}
            }
        }

        let font_family_css = if typed_font_stack && !config_owns_font_stack {
            theme.typography().font_stack().as_css()
        } else {
            configured_font
        };
        let outcome = if unsupported_typography {
            InheritedFontStackOutcome::Unsupported
        } else if typed_font_stack && config_owns_font_stack {
            InheritedFontStackOutcome::ConfigOwned
        } else if typed_font_stack {
            InheritedFontStackOutcome::Typed
        } else {
            InheritedFontStackOutcome::Inactive
        };

        Self {
            font_family_css: font_family_css.into_boxed_str(),
            outcome,
            typed_font_stack_requested: typed_font_stack,
        }
    }

    pub(crate) fn font_family_css(&self) -> &str {
        &self.font_family_css
    }

    pub(crate) const fn outcome(&self) -> InheritedFontStackOutcome {
        self.outcome
    }

    /// Whether this operation requested the directly owned inherited font-stack route.
    ///
    /// Families whose unthemed baseline has no font writer use this bit to avoid changing their
    /// terminal DOM unless the typed route was actually requested. It remains true when explicit
    /// configuration owns the winning value or a sibling typography property makes the combined
    /// mechanism fail closed.
    pub(crate) const fn typed_font_stack_requested(&self) -> bool {
        self.typed_font_stack_requested
    }
}
