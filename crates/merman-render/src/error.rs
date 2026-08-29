use std::sync::OnceLock;

use merman_core::__private::{ThemeCompatibilityContributionEvidence, ThemeParseEvidence};
use merman_core::MermaidConfig;

use crate::Result;
use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, ResolvedDiagramTheme,
    ThemeCapability, ThemeTypographyProperty,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, InheritedFontStackOutcome,
    InheritedFontStackPlan, InheritedTextRunFacts, InheritedTextRunSpec,
    InheritedTextViewportFacts, unsupported_residual_for_facet,
};
use crate::model::ErrorDiagramLayout;
use crate::text::TextMeasurer;

pub const UPSTREAM_MERMAID_VERSION: &str = merman_core::baseline::PINNED_MERMAID_BASELINE_VERSION;

const LEGACY_FAMILY_THEME_CONTRIBUTION_PREFIX: &str = "merman.legacy-family-theme.v1.";
const ERROR_BASELINE_VIEWBOX_WIDTH: f64 = 2412.0;
const ERROR_BASELINE_VIEWBOX_HEIGHT: f64 = 512.0;
const ERROR_BASELINE_MAX_WIDTH_PX: f64 = 512.0;
const ERROR_MESSAGE: &str = "Syntax error in text";
const ERROR_MESSAGE_FONT_SIZE_PX: f64 = 150.0;
const ERROR_MESSAGE_BASELINE_X_PX: f64 = 1440.0;
const ERROR_MESSAGE_BASELINE_Y_PX: f64 = 250.0;
const ERROR_VERSION_FONT_SIZE_PX: f64 = 100.0;
const ERROR_VERSION_BASELINE_X_PX: f64 = 1250.0;
const ERROR_VERSION_BASELINE_Y_PX: f64 = 400.0;

#[derive(Debug, Clone, Copy)]
pub(crate) enum ErrorTextRole {
    Message,
    Version,
}

#[derive(Debug)]
pub(crate) struct ErrorSurfaceReceipt {
    expected_font_family_css: Box<str>,
    expected_message: InheritedTextRunFacts,
    expected_version: InheritedTextRunFacts,
    root_font_family_css: Option<Box<str>>,
    inherited_font_family_css: Option<Box<str>>,
    root_variable_font_family_css: Option<Box<str>>,
    css_emission_unique: bool,
    message_text_count: usize,
    version_text_count: usize,
    terminal_matches: bool,
}

impl ErrorSurfaceReceipt {
    fn new(
        expected_font_family_css: &str,
        expected_message: &InheritedTextRunFacts,
        expected_version: &InheritedTextRunFacts,
    ) -> Self {
        Self {
            expected_font_family_css: expected_font_family_css.into(),
            expected_message: expected_message.clone(),
            expected_version: expected_version.clone(),
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
        font_size_px: f64,
        x: f64,
        y: f64,
    ) {
        self.terminal_matches &= emitted_class == "error-text";
        let expected = match role {
            ErrorTextRole::Message => &self.expected_message,
            ErrorTextRole::Version => &self.expected_version,
        };
        self.terminal_matches &= expected.matches_terminal(text, font_size_px, x, y);
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
    terminal_geometry: InheritedTextViewportFacts,
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
        measurer: &dyn TextMeasurer,
    ) -> Self {
        let inherited_font_stack =
            InheritedFontStackPlan::resolve_property_local(theme, effective_config);
        let version = format!("mermaid version {UPSTREAM_MERMAID_VERSION}");
        let terminal_geometry = InheritedTextViewportFacts::prepare(
            ERROR_BASELINE_VIEWBOX_WIDTH,
            ERROR_BASELINE_VIEWBOX_HEIGHT,
            [
                InheritedTextRunSpec::new(
                    ERROR_MESSAGE,
                    ERROR_MESSAGE_FONT_SIZE_PX,
                    ERROR_MESSAGE_BASELINE_X_PX,
                    ERROR_MESSAGE_BASELINE_Y_PX,
                ),
                InheritedTextRunSpec::new(
                    &version,
                    ERROR_VERSION_FONT_SIZE_PX,
                    ERROR_VERSION_BASELINE_X_PX,
                    ERROR_VERSION_BASELINE_Y_PX,
                ),
            ],
            inherited_font_stack.font_family_css(),
            measurer,
        );
        let unsupported_routes = error_unsupported_routes(theme);
        Self {
            inherited_font_stack,
            terminal_geometry,
            evidence: FamilyThemeEvidence::from_theme(theme),
            unsupported_routes,
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.inherited_font_stack.font_family_css()
    }

    pub(crate) fn begin_terminal_receipt(&self) -> ErrorSurfaceReceipt {
        ErrorSurfaceReceipt::new(
            self.font_family_css(),
            self.message_geometry(),
            self.version_geometry(),
        )
    }

    pub(crate) const fn viewport_width_px(&self) -> f64 {
        self.terminal_geometry.width_px()
    }

    pub(crate) const fn viewport_height_px(&self) -> f64 {
        self.terminal_geometry.height_px()
    }

    pub(crate) fn message_geometry(&self) -> &InheritedTextRunFacts {
        self.terminal_geometry.run(0)
    }

    pub(crate) fn version_geometry(&self) -> &InheritedTextRunFacts {
        self.terminal_geometry.run(1)
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
        self.inherited_font_stack
            .mark_unsupported_typography_evidence(
                &mut evidence,
                receipt.message_text_count != 0 || receipt.version_text_count != 0,
            );
        let key = FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontStack);
        if self.inherited_font_stack.typed_font_stack_active() {
            if receipt.proves_font_stack() {
                evidence.mark_applied_with_capabilities(key, [ThemeCapability::Typography]);
            } else {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedTypography);
            }
        } else {
            match self.inherited_font_stack.outcome() {
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
    // ErrorSurfaceReceipt independently proves this planned value reached every terminal. Parse
    // compatibility can retire only the contribution assignment that selected the same value.
    let expected_terminal_font_family_css = theme.typography().font_stack().as_css();

    evidence
        .fallback_contributions()
        .filter(|contribution| {
            !is_exact_legacy_family_typography_contribution(contribution.opaque_id())
                || contribution.surviving_assignment_paths().len() == 0
                || !contribution.surviving_assignment_paths().all(|path| {
                    error_typography_assignment_is_accounted(
                        theme,
                        contribution,
                        path,
                        &expected_terminal_font_family_css,
                    )
                })
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

fn error_typography_assignment_is_accounted(
    theme: &ResolvedDiagramTheme,
    contribution: &ThemeCompatibilityContributionEvidence,
    path: &str,
    expected_terminal_font_family_css: &str,
) -> bool {
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
    let route_is_accounted = theme
        .family_mechanism_routes()
        .iter()
        .copied()
        .any(|route| {
            route.mechanism() == FamilyThemeMechanism::BaseTypography(expected.0)
                && route.disposition() == expected.1
        });
    if !route_is_accounted {
        return false;
    }

    let Some(assignment_value) = contribution.surviving_assignment_value(path) else {
        return false;
    };
    match expected.0 {
        ThemeTypographyProperty::FontStack => assignment_value
            .as_str()
            .is_some_and(|value| value == expected_terminal_font_family_css),
        ThemeTypographyProperty::FontSize => true,
        ThemeTypographyProperty::FontWeight
        | ThemeTypographyProperty::FontStyle
        | ThemeTypographyProperty::LineHeight
        | ThemeTypographyProperty::LetterSpacing
        | ThemeTypographyProperty::WordSpacing
        | ThemeTypographyProperty::Transform
        | ThemeTypographyProperty::Decoration
        | ThemeTypographyProperty::TextAlign
        | ThemeTypographyProperty::WhiteSpace
        | ThemeTypographyProperty::Wrap => false,
    }
}

pub(crate) fn layout_error_diagram_typed(
    _semantic: &merman_core::diagrams::error_diagram::ErrorDiagramRenderModel,
    typography_theme: &ErrorTypographyThemePlan,
) -> Result<ErrorDiagramLayout> {
    let viewbox_width = typography_theme.viewport_width_px();
    Ok(ErrorDiagramLayout {
        viewbox_width,
        viewbox_height: typography_theme.viewport_height_px(),
        max_width_px: ERROR_BASELINE_MAX_WIDTH_PX * (viewbox_width / ERROR_BASELINE_VIEWBOX_WIDTH),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::DeterministicTextMeasurer;

    #[test]
    fn error_layout_preserves_upstream_canvas_and_anchors_for_the_default_font() {
        let measurer = DeterministicTextMeasurer::default();
        let config = MermaidConfig::default();
        let typography_theme = ErrorTypographyThemePlan::resolve(None, &config, &measurer);
        let layout = layout_error_diagram_typed(
            &merman_core::diagrams::error_diagram::ErrorDiagramRenderModel {
                diagram_type: "error".to_string(),
            },
            &typography_theme,
        )
        .expect("Error layout");

        assert_eq!(layout.viewbox_width, ERROR_BASELINE_VIEWBOX_WIDTH);
        assert_eq!(layout.viewbox_height, ERROR_BASELINE_VIEWBOX_HEIGHT);
        assert_eq!(layout.max_width_px, ERROR_BASELINE_MAX_WIDTH_PX);
        assert_eq!(
            typography_theme.message_geometry().x(),
            ERROR_MESSAGE_BASELINE_X_PX
        );
        assert_eq!(
            typography_theme.message_geometry().y(),
            ERROR_MESSAGE_BASELINE_Y_PX
        );
        assert_eq!(
            typography_theme.version_geometry().x(),
            ERROR_VERSION_BASELINE_X_PX
        );
        assert_eq!(
            typography_theme.version_geometry().y(),
            ERROR_VERSION_BASELINE_Y_PX
        );
    }
}
