use std::sync::OnceLock;

use merman_core::__private::ThemeParseEvidence;
use merman_core::MermaidConfig;

use crate::Result;
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, ResolvedDiagramTheme,
    ThemeCapability, ThemeTypographyProperty,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, InheritedFontStackOutcome,
    InheritedFontStackPlan, unsupported_residual_for_facet,
};
use crate::model::ErrorDiagramLayout;
use crate::text::TextMeasurer;

pub const UPSTREAM_MERMAID_VERSION: &str = merman_core::baseline::PINNED_MERMAID_BASELINE_VERSION;

const LEGACY_FAMILY_THEME_CONTRIBUTION_PREFIX: &str = "merman.legacy-family-theme.v1.";

#[derive(Debug, Clone, Copy)]
pub(crate) enum ErrorTextRole {
    Message,
    Version,
}

#[derive(Debug)]
pub(crate) struct ErrorSurfaceReceipt {
    expected_font_family_css: Box<str>,
    root_font_family_css: Option<Box<str>>,
    inherited_font_family_css: Option<Box<str>>,
    root_variable_font_family_css: Option<Box<str>>,
    css_emission_unique: bool,
    message_text_count: usize,
    version_text_count: usize,
    terminal_matches: bool,
}

impl ErrorSurfaceReceipt {
    fn new(expected_font_family_css: &str) -> Self {
        Self {
            expected_font_family_css: expected_font_family_css.into(),
            root_font_family_css: None,
            inherited_font_family_css: None,
            root_variable_font_family_css: None,
            css_emission_unique: true,
            message_text_count: 0,
            version_text_count: 0,
            terminal_matches: true,
        }
    }

    pub(crate) fn record_css_emission(
        &mut self,
        root_font_family_css: &str,
        inherited_font_family_css: &str,
        root_variable_font_family_css: &str,
    ) {
        if self.root_font_family_css.is_some()
            || self.inherited_font_family_css.is_some()
            || self.root_variable_font_family_css.is_some()
        {
            self.css_emission_unique = false;
            return;
        }
        let expected = self.expected_font_family_css.as_ref();
        self.terminal_matches &= !expected.trim().is_empty();
        self.terminal_matches &= root_font_family_css == expected;
        self.terminal_matches &= inherited_font_family_css == expected;
        self.terminal_matches &= root_variable_font_family_css == expected;
        self.root_font_family_css = Some(root_font_family_css.into());
        self.inherited_font_family_css = Some(inherited_font_family_css.into());
        self.root_variable_font_family_css = Some(root_variable_font_family_css.into());
    }

    pub(crate) fn record_error_text(
        &mut self,
        role: ErrorTextRole,
        emitted_class: &str,
        text: &str,
    ) {
        self.terminal_matches &= emitted_class == "error-text";
        self.terminal_matches &= !text.trim().is_empty();
        if text.trim().is_empty() {
            return;
        }
        match role {
            ErrorTextRole::Message => {
                self.message_text_count = self.message_text_count.saturating_add(1);
            }
            ErrorTextRole::Version => {
                self.version_text_count = self.version_text_count.saturating_add(1);
            }
        }
    }

    fn proves_font_stack(&self) -> bool {
        self.css_emission_unique
            && self.root_font_family_css.as_deref() == Some(self.expected_font_family_css.as_ref())
            && self.inherited_font_family_css.as_deref()
                == Some(self.expected_font_family_css.as_ref())
            && self.root_variable_font_family_css.as_deref()
                == Some(self.expected_font_family_css.as_ref())
            && self.message_text_count == 1
            && self.version_text_count == 1
            && self.terminal_matches
    }
}

/// Final inherited Error font shared by terminal CSS emission and family evidence.
#[derive(Debug)]
pub(crate) struct ErrorTypographyThemePlan {
    inherited_font_stack: InheritedFontStackPlan,
    evidence: FamilyThemeEvidence,
    unsupported_routes: Box<[ErrorUnsupportedRoute]>,
    terminal_receipt: OnceLock<ErrorSurfaceReceipt>,
}

#[derive(Debug)]
struct ErrorUnsupportedRoute {
    key: FamilyThemeMechanismKey,
    reason: FamilyThemeResidualReason,
}

impl ErrorTypographyThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
    ) -> Self {
        let unsupported_routes = error_unsupported_routes(theme);
        Self {
            inherited_font_stack: InheritedFontStackPlan::resolve(theme, effective_config),
            evidence: FamilyThemeEvidence::from_theme(theme),
            unsupported_routes,
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.inherited_font_stack.font_family_css()
    }

    pub(crate) fn begin_terminal_receipt(&self) -> ErrorSurfaceReceipt {
        ErrorSurfaceReceipt::new(self.font_family_css())
    }

    pub(crate) fn record_terminal(&self, receipt: ErrorSurfaceReceipt) -> bool {
        self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        for route in &self.unsupported_routes {
            evidence.mark_residual(route.key.clone(), route.reason);
        }
        let Some(receipt) = self.terminal_receipt.get() else {
            return evidence;
        };
        let key = FamilyThemeMechanismKey::Typography;
        match self.inherited_font_stack.outcome() {
            InheritedFontStackOutcome::Typed if receipt.proves_font_stack() => {
                evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
            }
            InheritedFontStackOutcome::ConfigOwned if receipt.proves_font_stack() => {
                evidence.mark_not_applicable(key);
            }
            InheritedFontStackOutcome::Typed
            | InheritedFontStackOutcome::ConfigOwned
            | InheritedFontStackOutcome::Unsupported => {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
            }
            InheritedFontStackOutcome::Inactive => {}
        }
        evidence
    }
}

fn error_unsupported_routes(theme: Option<&ResolvedDiagramTheme>) -> Box<[ErrorUnsupportedRoute]> {
    let Some(theme) = theme else {
        return Box::new([]);
    };
    theme
        .family_mechanism_routes()
        .iter()
        .copied()
        .filter_map(|route| {
            if route.disposition() != FamilyThemeDisposition::Unsupported {
                return None;
            }
            let reason = match route.mechanism() {
                FamilyThemeMechanism::RuleFacet { facet, .. } => {
                    unsupported_residual_for_facet(facet)
                }
                FamilyThemeMechanism::OrdinalPalette { .. } => {
                    FamilyThemeResidualReason::UnsupportedOrdinalPalette
                }
                FamilyThemeMechanism::EffectBinding { .. } => {
                    FamilyThemeResidualReason::UnsupportedEffect
                }
                FamilyThemeMechanism::BaseTypography(_) => return None,
            };
            Some(ErrorUnsupportedRoute {
                key: theme.family_mechanism_key(route),
                reason,
            })
        })
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

pub(crate) fn remaining_legacy_compatibility_residual_count(
    theme: Option<&ResolvedDiagramTheme>,
    evidence: &ThemeParseEvidence,
) -> usize {
    let Some(theme) = theme else {
        return evidence.fallback_contribution_count();
    };

    evidence
        .fallback_contributions()
        .filter(|contribution| {
            !is_exact_legacy_family_typography_contribution(contribution.opaque_id())
                || contribution.surviving_assignment_paths().len() == 0
                || !contribution
                    .surviving_assignment_paths()
                    .all(|path| error_typography_path_is_accounted(theme, path))
        })
        .count()
}

fn is_exact_legacy_family_typography_contribution(opaque_id: &str) -> bool {
    let Some(suffix) = opaque_id.strip_prefix(LEGACY_FAMILY_THEME_CONTRIBUTION_PREFIX) else {
        return false;
    };
    let Some((family, mapping)) = suffix.split_once('.') else {
        return false;
    };
    mapping == "typography" && crate::DiagramFamilyId::from_id(family).is_some()
}

fn error_typography_path_is_accounted(theme: &ResolvedDiagramTheme, path: &str) -> bool {
    let expected = match path {
        "fontFamily" | "themeVariables.fontFamily" => (
            ThemeTypographyProperty::FontStack,
            FamilyThemeDisposition::TypedAdapter,
        ),
        "themeVariables.fontSize" => (
            ThemeTypographyProperty::FontSize,
            FamilyThemeDisposition::Unsupported,
        ),
        _ => return false,
    };
    theme
        .family_mechanism_routes()
        .iter()
        .copied()
        .any(|route| {
            route.mechanism() == FamilyThemeMechanism::BaseTypography(expected.0)
                && route.disposition() == expected.1
        })
}

pub(crate) fn layout_error_diagram_typed(
    _semantic: &merman_core::diagrams::error_diagram::ErrorDiagramRenderModel,
    _effective_config: &serde_json::Value,
    _measurer: &dyn TextMeasurer,
) -> Result<ErrorDiagramLayout> {
    Ok(ErrorDiagramLayout {
        viewbox_width: 2412.0,
        viewbox_height: 512.0,
        max_width_px: 512.0,
    })
}
