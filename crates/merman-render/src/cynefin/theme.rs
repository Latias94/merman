use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, ResolvedDiagramTheme,
    ThemeCapability, ThemeTarget, ThemeTypographyProperty, ThemeVariant,
};
use crate::family::{
    DirectStaticPaint, DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeResidualReason,
    InheritedFontStackOutcome, InheritedFontStackPlan, UnsupportedTerminalDomain,
    reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
    unsupported_residual_for_facet,
};
use crate::model::CynefinDiagramLayout;
use crate::resources::{OperationWorkError, OperationWorkMeter};

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
    text_fill_css: Option<Box<str>>,
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
            text_fill_css: None,
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
        text_fill_css: &str,
    ) {
        if self.root_font_family_css.is_some()
            || self.inherited_font_family_css.is_some()
            || self.root_variable_font_family_css.is_some()
            || self.text_fill_css.is_some()
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
        self.text_fill_css = Some(text_fill_css.into());
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

    fn has_visible_text_fill_surface(&self) -> bool {
        self.expected_text_counts[CynefinTextRole::Subtitle.index()]
            .saturating_add(self.expected_text_counts[CynefinTextRole::Item.index()])
            .saturating_add(self.expected_text_counts[CynefinTextRole::TransitionLabel.index()])
            != 0
    }

    fn text_fill_counts_match(&self) -> bool {
        [
            CynefinTextRole::Subtitle,
            CynefinTextRole::Item,
            CynefinTextRole::TransitionLabel,
        ]
        .into_iter()
        .all(|role| {
            self.emitted_text_counts[role.index()] == self.expected_text_counts[role.index()]
        })
    }

    fn proves_text_fill(&self, expected_fill_css: &str) -> bool {
        self.css_emission_unique
            && self.has_visible_text_fill_surface()
            && self.text_fill_counts_match()
            && self.text_fill_css.as_deref() == Some(expected_fill_css)
            && self.terminal_matches
    }
}

/// Final inherited Cynefin font shared by layout measurement, terminal CSS, and evidence.
#[derive(Debug)]
pub(crate) struct CynefinTypographyThemePlan {
    common_css: crate::svg::PreparedCommonCss,
    colors: super::CynefinTheme,
    inherited_font_stack: InheritedFontStackPlan,
    evidence: FamilyThemeEvidence,
    unsupported_routes: Box<[CynefinUnsupportedRoute]>,
    text_fill: Option<DirectStaticPaint>,
    text_fill_routes: Box<[(FamilyThemeMechanismKey, usize)]>,
    config_owns_text_fill: bool,
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
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let text_fill = theme.and_then(|theme| {
            let style = theme.style(ThemeTarget::Text, ThemeVariant::Default, None);
            resolve_direct_static_fill(
                theme,
                &style,
                &[ThemeTarget::Text],
                DirectStaticSelectorDomain::Default,
            )
        });
        let text_fill_routes = theme
            .map(|theme| {
                theme
                    .family_mechanism_routes()
                    .iter()
                    .copied()
                    .filter_map(|route| match route.mechanism() {
                        FamilyThemeMechanism::RuleFacet {
                            rule_index,
                            target: ThemeTarget::Text,
                            facet: crate::diagram_theme::FamilyThemeRuleFacet::Fill(_),
                            selector,
                            ..
                        } if route.disposition() == FamilyThemeDisposition::TypedAdapter
                            && matches!(
                                selector,
                                crate::diagram_theme::FamilyThemeSelectorShape::Static {
                                    variant: None | Some(ThemeVariant::Default)
                                }
                            ) =>
                        {
                            Some((theme.family_mechanism_key(route), rule_index))
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice()
            })
            .unwrap_or_default();
        let config_owns_text_fill = theme.is_some()
            && (merman_core::__private::config_path_overrides_typed_default(
                effective_config,
                "themeVariables.textColor",
            ) || merman_core::__private::config_path_overrides_typed_default(
                effective_config,
                "themeVariables.cynefin.textColor",
            ));
        let mut evidence = FamilyThemeEvidence::from_theme(theme);
        let mut unsupported_routes = cynefin_unsupported_routes(theme).into_vec();
        if let Some(theme) = theme
            && theme.ordinal_palette_disposition(ThemeTarget::Text)
                == Some(FamilyThemeDisposition::Unsupported)
        {
            // Cynefin writes one shared text fill. Resolve its fallback without assigning
            // ordinal identities to CSS selectors; ordinal rules retain their own residuals.
            let mut fallbacks = FamilyThemeEvidence::from_theme(Some(theme));
            reconcile_unsupported_terminal_domains(
                theme,
                &mut fallbacks,
                &[UnsupportedTerminalDomain::static_fallbacks_only(
                    ThemeTarget::Text,
                    ThemeVariant::Default,
                )
                .with_source_owned_fill(&[config_owns_text_fill])],
                work_meter,
            )?;
            let palette_key = FamilyThemeMechanismKey::OrdinalPalette {
                target: ThemeTarget::Text,
            };
            if fallbacks.not_applicable_mechanisms().contains(&palette_key) {
                evidence.mark_not_applicable(palette_key.clone());
            }
            for residual in fallbacks
                .residuals()
                .iter()
                .filter(|item| item.key() == &palette_key)
            {
                unsupported_routes.push(CynefinUnsupportedRoute {
                    key: residual.key().clone(),
                    reason: residual.reason(),
                });
            }
        }
        let inherited_font_stack =
            InheritedFontStackPlan::resolve_property_local(theme, effective_config);
        let mut colors = super::cynefin_theme(effective_config.as_value());
        if !config_owns_text_fill && let Some(fill) = &text_fill {
            colors.text_color = fill.css().to_owned();
        }
        let common_css = crate::svg::PreparedCommonCss::new(
            effective_config.as_value(),
            Some(inherited_font_stack.font_family_css()),
        );
        Ok(Self {
            common_css,
            colors,
            inherited_font_stack,
            evidence,
            unsupported_routes: unsupported_routes.into_boxed_slice(),
            text_fill,
            text_fill_routes,
            config_owns_text_fill,
            terminal_receipt: OnceLock::new(),
        })
    }

    pub(crate) fn font_family_css(&self) -> &str {
        self.common_css.font_family()
    }

    pub(crate) fn common_css(&self) -> &crate::svg::PreparedCommonCss {
        &self.common_css
    }

    pub(crate) fn colors(&self) -> &super::CynefinTheme {
        &self.colors
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
            let absent_text_fallback = matches!(
                route.key,
                FamilyThemeMechanismKey::OrdinalPalette {
                    target: ThemeTarget::Text
                }
            ) && self.terminal_receipt.get().is_some_and(|receipt| {
                !receipt.has_visible_text_fill_surface() && receipt.text_fill_counts_match()
            });
            if absent_text_fallback {
                evidence.mark_not_applicable(route.key.clone());
            } else {
                evidence.mark_residual(route.key.clone(), route.reason);
            }
        }
        let Some(receipt) = self.terminal_receipt.get() else {
            for (key, _) in &self.text_fill_routes {
                if self.config_owns_text_fill {
                    evidence.mark_not_applicable(key.clone());
                } else {
                    evidence
                        .mark_residual(key.clone(), FamilyThemeResidualReason::UnsupportedPaint);
                }
            }
            return evidence;
        };
        self.inherited_font_stack
            .mark_unsupported_typography_evidence(&mut evidence, receipt.has_visible_text());
        let key = FamilyThemeMechanismKey::Typography(ThemeTypographyProperty::FontStack);
        if !receipt.has_visible_text() {
            evidence.mark_not_applicable(key);
            return evidence;
        }
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
        for (key, rule_index) in &self.text_fill_routes {
            if self.config_owns_text_fill || !receipt.has_visible_text_fill_surface() {
                evidence.mark_not_applicable(key.clone());
            } else if self
                .text_fill
                .as_ref()
                .is_some_and(|fill| fill.rule_index() == *rule_index)
            {
                if let Some(fill) = self.text_fill.as_ref()
                    && receipt.proves_text_fill(fill.css())
                {
                    evidence.mark_applied_with_capabilities(key.clone(), [fill.capability()]);
                } else {
                    evidence
                        .mark_residual(key.clone(), FamilyThemeResidualReason::UnsupportedPaint);
                }
            } else {
                evidence.mark_not_applicable(key.clone());
            }
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
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Text,
                } => return None,
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
            &OperationWorkMeter::new(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            ),
        )
        .expect("resolve Cynefin receipt theme")
    }

    #[test]
    fn binding_preserves_scoped_roles_and_shared_measurement_values() {
        let config = MermaidConfig::from_value(serde_json::json!({
            "themeVariables": {
                "fontFamily": "Config Sans", "textColor": "root-text",
                "lineColor": "root-line", "primaryTextColor": "root-label",
                "cynefin": {
                    "textColor": "var(--text)", "boundaryColor": "currentColor",
                    "complexBg": "var(--complex)", "domainFontSize": "23",
                    "itemFontSize": 15
                }
            }
        }));
        let plan = CynefinTypographyThemePlan::resolve(
            None,
            &config,
            &OperationWorkMeter::new(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            ),
        )
        .unwrap();
        assert_eq!(plan.colors().text_color, "var(--text)");
        assert_eq!(plan.colors().boundary_color, "currentColor");
        assert_eq!(plan.colors().arrow_color, "root-line");
        assert_eq!(plan.colors().label_color, "root-label");
        assert_eq!(plan.colors().complex_bg, "var(--complex)");
        assert_eq!(plan.colors().domain_font_size, 23.0);
        assert_eq!(plan.colors().item_font_size, 15.0);
        assert_eq!(plan.font_family_css(), "Config Sans");
    }

    #[test]
    fn cynefin_font_stack_receipt_rejects_missing_and_mismatched_writer_events() {
        let mut receipt = CynefinSurfaceReceipt::new("monospace", [1, 0, 0, 0, 0]);
        receipt.record_css_emission("monospace", "monospace", "monospace", "#333");
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
    fn cynefin_text_fill_receipt_requires_every_visible_text_role() {
        let mut receipt = CynefinSurfaceReceipt::new("monospace", [1, 1, 1, 0, 0]);
        receipt.record_css_emission("monospace", "monospace", "monospace", "#333");
        receipt.record_text(CynefinTextRole::Subtitle, "cynefinSubtitle", "Model");

        assert!(!receipt.proves_text_fill("#333"));
    }

    #[test]
    fn unverified_cynefin_writer_receipt_is_a_strict_typography_residual() {
        let plan = typed_plan();
        let mut receipt = CynefinSurfaceReceipt::new("monospace", [1, 0, 0, 0, 0]);
        receipt.record_css_emission("monospace", "monospace", "serif", "#333");
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
