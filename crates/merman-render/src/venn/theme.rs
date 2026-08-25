use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeResidualReason,
    TerminalVariantDomain, UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains,
    resolve_direct_static_fill, resolved_style_property_for_facet, unsupported_residual_for_facet,
};
use crate::resources::OperationWorkMeter;

/// Final Venn title fill shared by stylesheet emission, the title terminal, and evidence.
#[derive(Debug)]
pub(crate) struct VennTitleThemePlan {
    fill_css: Option<Box<str>>,
    typed_fill_capability: Option<ThemeCapability>,
    evidence: FamilyThemeEvidence,
    pending_fill_key: Option<FamilyThemeMechanismKey>,
    terminal_receipt: OnceLock<()>,
}

impl VennTitleThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
        title: Option<&str>,
        work_meter: &OperationWorkMeter,
    ) -> crate::Result<Self> {
        let title_present = title.is_some_and(|title| !title.trim().is_empty());
        let Some(theme) = theme else {
            return Ok(Self::baseline());
        };

        let title_count = usize::from(title_present);
        let config_owns_fill = merman_core::__private::config_path_overrides_typed_default(
            effective_config,
            "themeVariables.vennTitleTextColor",
        );
        let style = title_present.then(|| {
            theme.style_with_work_meter(
                ThemeTarget::Title,
                ThemeVariant::Default,
                Some(1),
                work_meter,
            )
        });
        let style = style.transpose()?;
        let winner_properties = style
            .as_ref()
            .into_iter()
            .flat_map(|style| style.winner_rule_properties())
            .map(|(property, origin)| (origin.rule_index(), property))
            .collect::<BTreeSet<_>>();

        let typed_fill = (!config_owns_fill)
            .then(|| {
                style.as_ref().and_then(|style| {
                    resolve_direct_static_fill(
                        theme,
                        style,
                        &[ThemeTarget::Title],
                        DirectStaticSelectorDomain::Unqualified,
                    )
                })
            })
            .flatten();
        let (fill_css, typed_fill_rule, typed_fill_capability) =
            typed_fill.map_or((None, None, None), |fill| {
                let (css, rule_index, capability) = fill.into_parts();
                (Some(css), Some(rule_index), Some(capability))
            });
        let source_owned_fill = [config_owns_fill];

        let mut evidence = FamilyThemeEvidence::from_theme(Some(theme));
        let mut observations = BTreeMap::<usize, VennTitleRuleObservation>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::Title,
                    selector,
                    facet,
                } => {
                    let observation = observations.entry(rule_index).or_default();
                    if !selector.ordinal_domain_intersects_occurrence_count(title_count) {
                        continue;
                    }
                    let route_won = winner_properties
                        .contains(&(rule_index, resolved_style_property_for_facet(facet)));
                    let qualified_variant = matches!(
                        selector,
                        FamilyThemeSelectorShape::Static { variant: Some(_) }
                            | FamilyThemeSelectorShape::Ordinal {
                                variant: Some(_),
                                ..
                            }
                    );
                    if !route_won && !qualified_variant {
                        continue;
                    }

                    observation.applicable = true;
                    if config_owns_fill && matches!(facet, FamilyThemeRuleFacet::Fill(_)) {
                        observation.fill_config_owned = true;
                        continue;
                    }
                    match (route.disposition(), selector, facet) {
                        (
                            FamilyThemeDisposition::TypedAdapter,
                            FamilyThemeSelectorShape::Static { variant: None },
                            FamilyThemeRuleFacet::Fill(_),
                        ) if typed_fill_rule == Some(rule_index) => {
                            observation.fill_pending = true;
                        }
                        (FamilyThemeDisposition::Unsupported, _, facet) => {
                            observation
                                .residual
                                .get_or_insert(unsupported_residual_for_facet(facet));
                        }
                        (FamilyThemeDisposition::TypedAdapter, _, _)
                        | (FamilyThemeDisposition::LegacyCompatibility, _, _) => {
                            observation.incomplete = true;
                        }
                    }
                }
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Title,
                } => {
                    // Reconciled below from the final title fill winner.
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Title,
                    ..
                } => {
                    // Reconciled below from the final title style.
                }
                FamilyThemeMechanism::BaseTypography(_)
                | FamilyThemeMechanism::RuleFacet { .. }
                | FamilyThemeMechanism::OrdinalPalette { .. }
                | FamilyThemeMechanism::EffectBinding { .. } => {}
            }
        }

        reconcile_unsupported_terminal_domains(
            theme,
            &mut evidence,
            &[UnsupportedTerminalDomain::fallbacks_only(
                ThemeTarget::Title,
                TerminalVariantDomain::uniform(title_count, ThemeVariant::Default),
            )
            .with_source_owned_fill(&source_owned_fill)],
            work_meter,
        )?;

        let mut pending_fill_key = None;
        for (rule_index, observation) in observations {
            let key = FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target: ThemeTarget::Title,
            };
            if !observation.applicable {
                evidence.mark_not_applicable(key);
            } else if let Some(reason) = observation.residual {
                evidence.mark_residual(key, reason);
            } else if observation.incomplete {
                // Mixed rules stay unaccounted until every winning facet has a terminal owner.
            } else if observation.fill_pending {
                debug_assert!(pending_fill_key.is_none());
                pending_fill_key = Some(key);
            } else if observation.fill_config_owned {
                evidence.mark_not_applicable(key);
            } else {
                evidence.mark_not_applicable(key);
            }
        }

        Ok(Self {
            fill_css,
            typed_fill_capability,
            evidence,
            pending_fill_key,
            terminal_receipt: OnceLock::new(),
        })
    }

    pub(crate) fn baseline() -> Self {
        Self {
            fill_css: None,
            typed_fill_capability: None,
            evidence: FamilyThemeEvidence::default(),
            pending_fill_key: None,
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn fill_css(&self) -> Option<&str> {
        self.fill_css.as_deref()
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<VennTitleThemeReceipt> {
        self.fill_css
            .as_ref()
            .map(|fill_css| VennTitleThemeReceipt::new(fill_css.clone()))
    }

    pub(crate) fn record_terminal(&self, receipt: VennTitleThemeReceipt) -> bool {
        self.fill_css.as_deref().is_some_and(|fill_css| {
            receipt.proves(fill_css) && self.terminal_receipt.set(()).is_ok()
        })
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        if let Some(key) = self.pending_fill_key.clone()
            && self.terminal_receipt.get().is_some()
            && let Some(capability) = self.typed_fill_capability
        {
            evidence.mark_applied_with_capabilities(key, [capability]);
        }
        evidence
    }
}

/// Writer-owned proof that the selected title fill reached CSS and the real title terminal.
#[derive(Debug)]
pub(crate) struct VennTitleThemeReceipt {
    expected_fill: Box<str>,
    stylesheet_fill: Option<Box<str>>,
    stylesheet_class: Option<Box<str>>,
    inline_fill: Option<Box<str>>,
    inline_class: Option<Box<str>>,
    title_text_count: usize,
    terminal_matches: bool,
}

impl VennTitleThemeReceipt {
    fn new(expected_fill: Box<str>) -> Self {
        Self {
            expected_fill,
            stylesheet_fill: None,
            stylesheet_class: None,
            inline_fill: None,
            inline_class: None,
            title_text_count: 0,
            terminal_matches: true,
        }
    }

    pub(crate) fn record_stylesheet(&mut self, emitted_class: &str, emitted_fill: &str) {
        if self.stylesheet_fill.is_some() || self.stylesheet_class.is_some() {
            self.terminal_matches = false;
            return;
        }
        self.terminal_matches &= emitted_class == super::VENN_TITLE_CLASS;
        self.terminal_matches &= emitted_fill == self.expected_fill.as_ref();
        self.stylesheet_class = Some(emitted_class.into());
        self.stylesheet_fill = Some(emitted_fill.into());
    }

    pub(crate) fn record_title_text(&mut self, emitted_class: &str, emitted_fill: &str) {
        self.title_text_count = self.title_text_count.saturating_add(1);
        self.terminal_matches &= emitted_class == super::VENN_TITLE_CLASS;
        self.terminal_matches &= emitted_fill == self.expected_fill.as_ref();
        if self.inline_fill.is_some() || self.inline_class.is_some() {
            self.terminal_matches = false;
            return;
        }
        self.inline_class = Some(emitted_class.into());
        self.inline_fill = Some(emitted_fill.into());
    }

    fn proves(&self, expected_fill: &str) -> bool {
        self.expected_fill.as_ref() == expected_fill
            && self.stylesheet_fill.as_deref() == Some(expected_fill)
            && self.stylesheet_class.as_deref() == Some(super::VENN_TITLE_CLASS)
            && self.inline_fill.as_deref() == Some(expected_fill)
            && self.inline_class.as_deref() == Some(super::VENN_TITLE_CLASS)
            && self.title_text_count == 1
            && self.terminal_matches
    }
}

#[derive(Debug, Default)]
struct VennTitleRuleObservation {
    applicable: bool,
    fill_config_owned: bool,
    incomplete: bool,
    residual: Option<FamilyThemeResidualReason>,
    fill_pending: bool,
}

#[cfg(test)]
mod tests {
    use super::{VennTitleThemePlan, VennTitleThemeReceipt};
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, OrdinalPalette, ThemeColorValue,
        ThemeRule, ThemeRuleSet, ThemeStylePatch, ThemeTarget,
    };
    use crate::resources::RenderResourcePolicy;
    use merman_core::MermaidConfig;
    use serde_json::json;

    #[test]
    fn title_receipt_requires_matching_stylesheet_and_inline_terminal() {
        let mut complete = VennTitleThemeReceipt::new("#123456".into());
        complete.record_stylesheet(super::super::VENN_TITLE_CLASS, "#123456");
        complete.record_title_text(super::super::VENN_TITLE_CLASS, "#123456");
        assert!(complete.proves("#123456"));

        let mut missing_inline = VennTitleThemeReceipt::new("#123456".into());
        missing_inline.record_stylesheet(super::super::VENN_TITLE_CLASS, "#123456");
        assert!(!missing_inline.proves("#123456"));

        let mut wrong_stylesheet = VennTitleThemeReceipt::new("#123456".into());
        wrong_stylesheet.record_stylesheet(super::super::VENN_TITLE_CLASS, "#abcdef");
        wrong_stylesheet.record_title_text(super::super::VENN_TITLE_CLASS, "#123456");
        assert!(!wrong_stylesheet.proves("#123456"));

        let mut wrong_inline = VennTitleThemeReceipt::new("#123456".into());
        wrong_inline.record_stylesheet(super::super::VENN_TITLE_CLASS, "#123456");
        wrong_inline.record_title_text(super::super::VENN_TITLE_CLASS, "transparent");
        assert!(!wrong_inline.proves("#123456"));

        let mut duplicate_inline = VennTitleThemeReceipt::new("#123456".into());
        duplicate_inline.record_stylesheet(super::super::VENN_TITLE_CLASS, "#123456");
        duplicate_inline.record_title_text(super::super::VENN_TITLE_CLASS, "#123456");
        duplicate_inline.record_title_text(super::super::VENN_TITLE_CLASS, "#123456");
        assert!(!duplicate_inline.proves("#123456"));
    }

    #[test]
    fn typed_title_fill_shadows_unsupported_ordinal_palette() {
        let palette = OrdinalPalette::new([
            ThemeColorValue::parse("#abcdef").expect("valid Venn ordinal palette color")
        ])
        .expect("non-empty Venn ordinal palette");
        let resolved = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default()
                        .with_rule(ThemeRule::new(
                            ThemeTarget::Title,
                            ThemeStylePatch::default().with_fill(
                                CanvasPaint::solid("#123456").expect("valid Venn title fill"),
                            ),
                        ))
                        .with_ordinal_palette(ThemeTarget::Title, palette),
                ),
            )
            .expect("compile Venn title fill and palette theme")
            .resolve(DiagramFamilyId::VENN);
        let meter = crate::resources::OperationWorkMeter::new(
            RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let config = MermaidConfig::from_value(json!({}));
        let plan = VennTitleThemePlan::resolve(Some(&resolved), &config, Some("Title"), &meter)
            .expect("resolve Venn title theme");
        let mut receipt = plan.begin_terminal_receipt().expect("typed title receipt");
        receipt.record_stylesheet(super::super::VENN_TITLE_CLASS, "#123456");
        receipt.record_title_text(super::super::VENN_TITLE_CLASS, "#123456");
        assert!(plan.record_terminal(receipt));

        let evidence = plan.finish_evidence();
        assert_eq!(resolved.family_mechanism_keys().len(), 2);
        assert_eq!(evidence.applied().len(), 1);
        assert_eq!(evidence.not_applicable_mechanisms().len(), 1);
        assert!(evidence.residuals().is_empty());
    }
}
