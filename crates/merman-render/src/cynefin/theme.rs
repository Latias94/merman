use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, ResolvedDiagramTheme,
    ThemeCapability,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, InheritedFontStackOutcome,
    InheritedFontStackPlan, unsupported_residual_for_facet,
};
use crate::model::CynefinDiagramLayout;

const CYNEFIN_TEXT_ROLE_COUNT: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CynefinTextRole {
    DomainLabel,
    Subtitle,
    Item,
    TransitionLabel,
    Title,
}

impl CynefinTextRole {
    const fn index(self) -> usize {
        match self {
            Self::DomainLabel => 0,
            Self::Subtitle => 1,
            Self::Item => 2,
            Self::TransitionLabel => 3,
            Self::Title => 4,
        }
    }

    const fn class(self) -> &'static str {
        match self {
            Self::DomainLabel => "cynefinDomainLabel",
            Self::Subtitle => "cynefinSubtitle",
            Self::Item => "cynefinItemText",
            Self::TransitionLabel => "cynefinArrowLabel",
            Self::Title => "cynefinTitle",
        }
    }
}

/// Writer-owned proof that one resolved font stack reached every inherited Cynefin surface.
#[derive(Debug)]
pub(crate) struct CynefinSurfaceReceipt {
    expected_font_family_css: Box<str>,
    root_font_family_css: Option<Box<str>>,
    inherited_font_family_css: Option<Box<str>>,
    root_variable_font_family_css: Option<Box<str>>,
    css_emission_unique: bool,
    expected_text_counts: [usize; CYNEFIN_TEXT_ROLE_COUNT],
    emitted_text_counts: [usize; CYNEFIN_TEXT_ROLE_COUNT],
    terminal_matches: bool,
}

impl CynefinSurfaceReceipt {
    fn new(
        expected_font_family_css: &str,
        expected_text_counts: [usize; CYNEFIN_TEXT_ROLE_COUNT],
    ) -> Self {
        Self {
            expected_font_family_css: expected_font_family_css.into(),
            root_font_family_css: None,
            inherited_font_family_css: None,
            root_variable_font_family_css: None,
            css_emission_unique: true,
            expected_text_counts,
            emitted_text_counts: [0; CYNEFIN_TEXT_ROLE_COUNT],
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

    pub(crate) fn record_text(&mut self, role: CynefinTextRole, emitted_class: &str, text: &str) {
        if text.trim().is_empty() {
            return;
        }
        self.terminal_matches &= emitted_class == role.class();
        let count = &mut self.emitted_text_counts[role.index()];
        *count = count.saturating_add(1);
    }

    fn has_visible_text(&self) -> bool {
        self.expected_text_counts.iter().copied().sum::<usize>() != 0
    }

    fn proves_font_stack(&self) -> bool {
        self.css_emission_unique
            && self.root_font_family_css.as_deref() == Some(self.expected_font_family_css.as_ref())
            && self.inherited_font_family_css.as_deref()
                == Some(self.expected_font_family_css.as_ref())
            && self.root_variable_font_family_css.as_deref()
                == Some(self.expected_font_family_css.as_ref())
            && self.emitted_text_counts == self.expected_text_counts
            && self.terminal_matches
    }
}

/// Final inherited Cynefin font shared by layout measurement, terminal CSS, and evidence.
#[derive(Debug)]
pub(crate) struct CynefinTypographyThemePlan {
    inherited_font_stack: InheritedFontStackPlan,
    evidence: FamilyThemeEvidence,
    unsupported_routes: Box<[CynefinUnsupportedRoute]>,
    terminal_receipt: OnceLock<CynefinSurfaceReceipt>,
}

#[derive(Debug)]
struct CynefinUnsupportedRoute {
    key: FamilyThemeMechanismKey,
    reason: FamilyThemeResidualReason,
}

impl CynefinTypographyThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
    ) -> Self {
        Self {
            inherited_font_stack: InheritedFontStackPlan::resolve(theme, effective_config),
            evidence: FamilyThemeEvidence::from_theme(theme),
            unsupported_routes: cynefin_unsupported_routes(theme),
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.inherited_font_stack.font_family_css()
    }

    pub(crate) fn begin_terminal_receipt(
        &self,
        layout: &CynefinDiagramLayout,
        title: Option<&str>,
    ) -> CynefinSurfaceReceipt {
        let quadrant_count = crate::cynefin::quadrant_domains()
            .iter()
            .copied()
            .filter(|domain_name| {
                layout
                    .domain_layouts
                    .iter()
                    .any(|domain| domain.name == *domain_name)
            })
            .count();
        let expected_text_counts = [
            quadrant_count.saturating_add(1),
            if layout.show_domain_descriptions {
                quadrant_count.saturating_mul(2).saturating_add(1)
            } else {
                0
            },
            layout
                .items
                .iter()
                .filter(|item| !item.label.trim().is_empty())
                .count(),
            layout
                .transitions
                .iter()
                .filter(|transition| {
                    transition
                        .label
                        .as_deref()
                        .is_some_and(|label| !label.trim().is_empty())
                })
                .count(),
            if title.is_some_and(|title| !title.trim().is_empty()) {
                1
            } else {
                0
            },
        ];
        CynefinSurfaceReceipt::new(self.font_family_css(), expected_text_counts)
    }

    pub(crate) fn record_terminal(&self, receipt: CynefinSurfaceReceipt) -> bool {
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
        if !receipt.has_visible_text() {
            evidence.mark_not_applicable(key);
            return evidence;
        }
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

fn cynefin_unsupported_routes(
    theme: Option<&ResolvedDiagramTheme>,
) -> Box<[CynefinUnsupportedRoute]> {
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
            Some(CynefinUnsupportedRoute {
                key: theme.family_mechanism_key(route),
                reason,
            })
        })
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        DiagramThemeCompiler, DiagramThemeSpec, FontStack, ThemeTextStyle, TypographySpec,
    };

    fn typed_plan() -> CynefinTypographyThemePlan {
        let typography = ThemeTextStyle::default().with_font_stack(
            FontStack::single("monospace").expect("valid Cynefin receipt font stack"),
        );
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_typography(
                TypographySpec::default().with_family_style(DiagramFamilyId::CYNEFIN, typography),
            ))
            .expect("compile Cynefin receipt theme");
        let resolved = theme.resolve(DiagramFamilyId::CYNEFIN);
        CynefinTypographyThemePlan::resolve(
            Some(&resolved),
            &MermaidConfig::from_value(serde_json::json!({})),
        )
    }

    #[test]
    fn cynefin_font_stack_receipt_rejects_missing_and_mismatched_writer_events() {
        let mut receipt = CynefinSurfaceReceipt::new("monospace", [1, 0, 0, 0, 0]);
        receipt.record_css_emission("monospace", "monospace", "monospace");
        receipt.record_text(CynefinTextRole::DomainLabel, "wrong-class", "Complex");
        assert!(!receipt.proves_font_stack());

        let mut missing_css = CynefinSurfaceReceipt::new("monospace", [1, 0, 0, 0, 0]);
        missing_css.record_text(
            CynefinTextRole::DomainLabel,
            "cynefinDomainLabel",
            "Complex",
        );
        assert!(!missing_css.proves_font_stack());
    }

    #[test]
    fn unverified_cynefin_writer_receipt_is_a_strict_typography_residual() {
        let plan = typed_plan();
        let mut receipt = CynefinSurfaceReceipt::new("monospace", [1, 0, 0, 0, 0]);
        receipt.record_css_emission("monospace", "monospace", "serif");
        receipt.record_text(
            CynefinTextRole::DomainLabel,
            "cynefinDomainLabel",
            "Complex",
        );
        assert!(plan.record_terminal(receipt));

        let evidence = plan.finish_evidence();
        assert!(evidence.applied().is_empty());
        assert_eq!(evidence.residuals().len(), 1);
        assert_eq!(
            evidence.residuals()[0].reason(),
            FamilyThemeResidualReason::UnsupportedTypography
        );
    }
}
