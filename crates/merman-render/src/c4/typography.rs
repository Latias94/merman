use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::config::config_theme_font_size_css_or_root_number_px_opt;
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, ResolvedDiagramTheme,
    ThemeCapability, ThemeTypographyProperty,
};
use crate::family::{FamilyThemeEvidence, FamilyThemeResidualReason, InheritedFontStackPlan};

/// Resolves C4's inherited base typography for the diagram title and root stylesheet.
///
/// C4 shape, boundary, and relationship text keep their independent `c4.*Font*` owners. This
/// plan intentionally proves only the inherited title surface and makes no layout claim.
#[derive(Debug)]
pub(crate) struct C4TypographyThemePlan {
    inherited_font_stack: InheritedFontStackPlan,
    font_size_css: Box<str>,
    typed_font_size_requested: bool,
    typed_font_size_active: bool,
    title: Option<Box<str>>,
    evidence: FamilyThemeEvidence,
    terminal_seal: OnceLock<C4TypographyTerminalSeal>,
}

#[derive(Debug, Clone, Copy)]
struct C4TypographyTerminalSeal {
    has_visible_title: bool,
}

/// Writer-owned proof for C4's inherited stylesheet and optional diagram title.
#[derive(Debug)]
pub(crate) struct C4TypographyThemeReceipt<'a> {
    expected_font_family_css: &'a str,
    expected_font_size_css: &'a str,
    expected_title: Option<&'a str>,
    css_emitted: bool,
    css_matches: bool,
    title_seen: bool,
    valid: bool,
}

impl C4TypographyThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
        title: Option<&str>,
    ) -> Self {
        let inherited_font_stack =
            InheritedFontStackPlan::resolve_property_local(theme, effective_config);
        let typed_font_size_requested = theme.is_some_and(|theme| {
            theme.family_mechanism_routes().iter().any(|route| {
                route.mechanism()
                    == FamilyThemeMechanism::BaseTypography(ThemeTypographyProperty::FontSize)
                    && route.disposition() == FamilyThemeDisposition::TypedAdapter
            })
        });
        // The theme-variable path is the typed base-size owner. A top-level `fontSize` remains a
        // Mermaid-compatible fallback and must not shadow an active typed value.
        let config_owns_font_size = typed_font_size_requested
            && merman_core::__private::config_path_overrides_typed_default(
                effective_config,
                "themeVariables.fontSize",
            );
        let typed_font_size_active = typed_font_size_requested && !config_owns_font_size;
        let font_size_css = match (theme, typed_font_size_active) {
            (Some(theme), true) => {
                let font_size_px = f64::from(theme.typography().font_size_px()).max(1.0);
                format!("{font_size_px}px").into_boxed_str()
            }
            _ => {
                let font_size_px =
                    config_theme_font_size_css_or_root_number_px_opt(effective_config.as_value())
                        .unwrap_or(16.0)
                        .max(1.0);
                format!("{font_size_px}px").into_boxed_str()
            }
        };

        Self {
            inherited_font_stack,
            font_size_css,
            typed_font_size_requested,
            typed_font_size_active,
            title: normalize_title(title),
            evidence: FamilyThemeEvidence::from_theme(theme),
            terminal_seal: OnceLock::new(),
        }
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.inherited_font_stack.font_family_css()
    }

    pub(crate) fn font_size_css(&self) -> &str {
        &self.font_size_css
    }

    pub(crate) fn typography_requested(&self) -> bool {
        self.inherited_font_stack.typography_requested() || self.typed_font_size_requested
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<C4TypographyThemeReceipt<'_>> {
        self.typography_requested()
            .then(|| C4TypographyThemeReceipt {
                expected_font_family_css: self.font_family_css(),
                expected_font_size_css: self.font_size_css(),
                expected_title: self.title.as_deref(),
                css_emitted: false,
                css_matches: false,
                title_seen: false,
                valid: true,
            })
    }

    pub(crate) fn record_terminal(&self, receipt: C4TypographyThemeReceipt<'_>) -> bool {
        receipt
            .seal()
            .is_some_and(|seal| self.terminal_seal.set(seal).is_ok())
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        if !self.typography_requested() {
            return evidence;
        }

        let Some(seal) = self.terminal_seal.get() else {
            self.mark_typed_property_evidence(
                &mut evidence,
                ThemeTypographyProperty::FontStack,
                self.inherited_font_stack.typed_font_stack_requested(),
                self.inherited_font_stack.typed_font_stack_active(),
                false,
            );
            self.mark_typed_property_evidence(
                &mut evidence,
                ThemeTypographyProperty::FontSize,
                self.typed_font_size_requested,
                self.typed_font_size_active,
                false,
            );
            return evidence;
        };

        self.mark_typed_property_evidence(
            &mut evidence,
            ThemeTypographyProperty::FontStack,
            self.inherited_font_stack.typed_font_stack_requested(),
            self.inherited_font_stack.typed_font_stack_active(),
            seal.has_visible_title,
        );
        self.mark_typed_property_evidence(
            &mut evidence,
            ThemeTypographyProperty::FontSize,
            self.typed_font_size_requested,
            self.typed_font_size_active,
            seal.has_visible_title,
        );
        evidence
    }

    fn mark_typed_property_evidence(
        &self,
        evidence: &mut FamilyThemeEvidence,
        property: ThemeTypographyProperty,
        requested: bool,
        active: bool,
        has_visible_title: bool,
    ) {
        if !requested {
            return;
        }
        let key = FamilyThemeMechanismKey::Typography(property);
        if !active {
            evidence.mark_not_applicable(key);
        } else if self.terminal_seal.get().is_none() {
            evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
        } else if !has_visible_title {
            evidence.mark_not_applicable(key);
        } else {
            evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
        }
    }
}

impl<'a> C4TypographyThemeReceipt<'a> {
    pub(crate) fn record_css_emission(
        &mut self,
        emitted_font_family_css: &str,
        emitted_font_size_css: &str,
        base_typography_emitted: bool,
        root_typography_emitted: bool,
    ) {
        self.valid &= !self.css_emitted
            && base_typography_emitted
            && root_typography_emitted
            && emitted_font_family_css == self.expected_font_family_css
            && emitted_font_size_css == self.expected_font_size_css;
        self.css_emitted = true;
        self.css_matches = emitted_font_family_css == self.expected_font_family_css
            && emitted_font_size_css == self.expected_font_size_css;
    }

    pub(crate) fn record_title_text(&mut self, text: &str) {
        self.valid &= !self.title_seen && self.expected_title == Some(text);
        self.title_seen = true;
    }

    fn seal(self) -> Option<C4TypographyTerminalSeal> {
        (self.valid
            && self.css_emitted
            && self.css_matches
            && self.title_seen == self.expected_title.is_some())
        .then_some(C4TypographyTerminalSeal {
            has_visible_title: self.expected_title.is_some(),
        })
    }
}

fn normalize_title(title: Option<&str>) -> Option<Box<str>> {
    title
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(Into::into)
}

#[cfg(test)]
mod tests {
    use super::{C4TypographyThemePlan, C4TypographyThemeReceipt};
    use crate::diagram_theme::{
        DiagramThemeCompiler, DiagramThemeSpec, FontStack, ThemeTextStyle, TypographySpec,
    };
    use merman_core::MermaidConfig;

    #[test]
    fn title_receipt_requires_matching_css_and_optional_title() {
        let mut receipt = C4TypographyThemeReceipt {
            expected_font_family_css: "Inter",
            expected_font_size_css: "18px",
            expected_title: Some("Context"),
            css_emitted: false,
            css_matches: false,
            title_seen: false,
            valid: true,
        };
        receipt.record_css_emission("Inter", "18px", true, true);
        receipt.record_title_text("Context");
        assert!(receipt.seal().is_some());

        let mut missing_title = C4TypographyThemeReceipt {
            expected_font_family_css: "Inter",
            expected_font_size_css: "18px",
            expected_title: Some("Context"),
            css_emitted: false,
            css_matches: false,
            title_seen: false,
            valid: true,
        };
        missing_title.record_css_emission("Inter", "18px", true, true);
        assert!(missing_title.seal().is_none());
    }

    #[test]
    fn typed_values_are_resolved_for_c4_title_surface() {
        let typography = ThemeTextStyle::default()
            .with_font_stack(FontStack::single("Inter").expect("font stack"))
            .with_font_size_px(18.0)
            .expect("font size");
        let spec = DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(crate::DiagramFamilyId::C4, typography),
        );
        let theme = DiagramThemeCompiler::new()
            .compile(spec)
            .expect("compile C4 theme")
            .resolve(crate::DiagramFamilyId::C4);
        let plan = C4TypographyThemePlan::resolve(
            Some(&theme),
            &MermaidConfig::empty_object(),
            Some("Context"),
        );

        assert_eq!(plan.font_family_css(), "Inter");
        assert_eq!(plan.font_size_css(), "18px");
        assert!(plan.begin_terminal_receipt().is_some());
    }
}
