use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    ResolvedDiagramTheme, ResolvedStyleProperty, ResolvedThemeEffect, ResolvedThemeStyle,
    Specified, ThemeTarget, ThemeVariant,
};
use crate::family::{
    DirectStaticPaint, DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeResidualReason,
    resolve_direct_static_fill, resolved_style_property_for_facet, unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};
use crate::text::VisibleTextStyleFacts;

const CLUSTER_TEXT_ROLES: &[ThemeTarget] = &[ThemeTarget::Title, ThemeTarget::ClusterLabel];

pub(super) fn resolve_cluster_text_style(
    theme: &ResolvedDiagramTheme,
    ordinal: Option<usize>,
    work: &OperationWorkMeter,
) -> Result<ResolvedThemeStyle, OperationWorkError> {
    theme.text_style_with_role_override(
        ThemeTarget::Title,
        ThemeTarget::ClusterLabel,
        ThemeVariant::Default,
        ordinal,
        work,
    )
}

pub(super) fn cluster_palette_target(
    theme: &ResolvedDiagramTheme,
    ordinal: usize,
) -> Option<ThemeTarget> {
    [
        ThemeTarget::ClusterLabel,
        ThemeTarget::Title,
        ThemeTarget::Text,
    ]
    .into_iter()
    .find(|target| theme.series_color(*target, ordinal).is_some())
}

pub(super) fn cluster_effect<'a>(
    theme: &'a ResolvedDiagramTheme,
    style: &ResolvedThemeStyle,
) -> Option<ResolvedThemeEffect<'a>> {
    [ThemeTarget::ClusterLabel, ThemeTarget::Title]
        .into_iter()
        .find_map(|target| theme.resolve_effect(target, style.effect_resolution()))
}

/// A shared CSS color is resolved once; actual label writers own applicability and ordinals.
#[derive(Debug)]
pub(crate) struct FlowchartTextSurfacePaintPlan {
    pub(crate) background: super::FlowchartLabelBackgroundPlan,
    pub(crate) generic_text: super::FlowchartTextPaintPlan,
    evidence: FamilyThemeEvidence,
    theme: Option<ResolvedDiagramTheme>,
    style: Option<ResolvedThemeStyle>,
    fill: Option<DirectStaticPaint>,
    config_owned: bool,
    ordinal_rules: bool,
    terminal: Mutex<Option<FlowchartTextSurfacePaintReceipt>>,
}

impl FlowchartTextSurfacePaintPlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        config: &MermaidConfig,
        node_config_owned: bool,
        work: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        // All collaborating consumers retain the same complete family request set, even when
        // their own target filter is empty.
        let evidence = FamilyThemeEvidence::from_theme(theme);
        let background = super::FlowchartLabelBackgroundPlan::resolve(theme, config, work)?;
        let generic_text =
            super::FlowchartTextPaintPlan::resolve(theme, config, node_config_owned, work)?;
        let theme = theme.filter(|theme| {
            theme.family_mechanism_routes().iter().any(|route| {
                matches!(
                    route.mechanism(),
                    FamilyThemeMechanism::RuleFacet {
                        target: ThemeTarget::Title | ThemeTarget::ClusterLabel,
                        ..
                    } | FamilyThemeMechanism::OrdinalPalette {
                        target: ThemeTarget::Title | ThemeTarget::ClusterLabel
                    } | FamilyThemeMechanism::EffectBinding {
                        target: ThemeTarget::Title | ThemeTarget::ClusterLabel,
                        ..
                    }
                )
            })
        });
        let style = theme
            .map(|theme| resolve_cluster_text_style(theme, None, work))
            .transpose()?;
        let fill = theme.zip(style.as_ref()).and_then(|(theme, style)| {
            resolve_direct_static_fill(
                theme,
                style,
                CLUSTER_TEXT_ROLES,
                DirectStaticSelectorDomain::Default,
            )
        });
        Ok(Self {
            background,
            generic_text,
            evidence,
            theme: theme.cloned(),
            style,
            fill,
            config_owned: merman_core::__private::config_path_overrides_typed_default(
                config,
                "themeVariables.titleColor",
            ),
            ordinal_rules: theme.is_some_and(|theme| {
                theme.family_rules().any(|(_, rule)| {
                    matches!(
                        rule.target(),
                        ThemeTarget::Text | ThemeTarget::Title | ThemeTarget::ClusterLabel
                    ) && rule.ordinal().is_some()
                })
            }),
            terminal: Mutex::new(None),
        })
    }

    pub(crate) fn requested(&self) -> bool {
        self.theme.is_some() || self.generic_text.requested() || self.background.requested()
    }

    pub(crate) fn begin_terminal_emission(
        &self,
        expected: impl IntoIterator<Item = impl Into<String>>,
        work: &OperationWorkMeter,
    ) -> Result<(), OperationWorkError> {
        if !self.requested() {
            return Ok(());
        }
        work.charge(
            self.theme
                .as_ref()
                .map_or(0, |theme| theme.family_mechanism_routes().len()),
        )?;
        let mut terminals = BTreeMap::new();
        let mut valid = true;
        for id in expected {
            work.charge(1)?;
            valid &= terminals.insert(id.into(), false).is_none();
        }
        let mut terminal = self
            .terminal
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        valid &= terminal.is_none();
        *terminal = Some(FlowchartTextSurfacePaintReceipt {
            terminals,
            valid,
            css_emitted: [false; 2],
            ordinal: 0,
            winners: BTreeSet::new(),
            unknown_fill_winners: BTreeSet::new(),
            consumed: BTreeSet::new(),
            palettes: BTreeSet::new(),
            bindings: BTreeSet::new(),
        });
        Ok(())
    }

    /// Emit the existing cluster selector declaration, preserving author CSS precedence.
    pub(crate) fn write_css(
        &self,
        out: &mut impl std::fmt::Write,
        configured: &str,
        html: bool,
    ) -> std::fmt::Result {
        let color = if self.config_owned {
            configured
        } else {
            self.fill.as_ref().map_or_else(
                || {
                    self.generic_text
                        .color(super::FlowchartTextPaintChannel::Cluster, configured)
                },
                DirectStaticPaint::css,
            )
        };
        let property = if html { "color" } else { "fill" };
        let result = write!(out, "{property}:{color};");
        if let Some(terminal) = self
            .terminal
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_mut()
        {
            terminal.css_emitted[usize::from(html)] |= result.is_ok();
            terminal.valid &= result.is_ok();
        }
        result
    }

    pub(crate) fn record_label(
        &self,
        id: &str,
        facts: &VisibleTextStyleFacts,
        source: super::FlowchartSourceFacetStatus,
        work: &OperationWorkMeter,
    ) -> Result<(), OperationWorkError> {
        if !self.requested() {
            return Ok(());
        }
        let mut receipt = self
            .terminal
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(terminal) = receipt.as_mut() else {
            return Ok(());
        };
        work.charge(1 + facts.visible_run_count())?;
        let Some(seen) = terminal.terminals.get_mut(id) else {
            terminal.valid = false;
            return Ok(());
        };
        terminal.valid &= !*seen;
        *seen = true;
        if facts.parse_valid() && !facts.has_visible_runs() {
            return Ok(());
        }
        self.generic_text.record_label(
            super::FlowchartTextPaintChannel::Cluster,
            super::FlowchartTextPaintFacts::from_visible(facts),
            source,
            work,
        )?;
        let Some(theme) = self.theme.as_ref() else {
            return Ok(());
        };
        terminal.ordinal += 1;
        let source_owned =
            self.config_owned || source == super::FlowchartSourceFacetStatus::Admitted;
        let unverified = !facts.parse_valid()
            || source == super::FlowchartSourceFacetStatus::Unverified
            || facts.unverified_portable_color_run_count() != 0;
        let inherited = !source_owned && facts.inherited_color_run_count() != 0;
        let fill_applies = !source_owned && (inherited || unverified);
        let ordinal_style;
        let style = if self.ordinal_rules {
            ordinal_style = resolve_cluster_text_style(theme, Some(terminal.ordinal), work)?;
            &ordinal_style
        } else {
            self.style.as_ref().expect("requested static text style")
        };
        for (property, origin) in style.winner_rule_properties() {
            work.charge(1)?;
            if !CLUSTER_TEXT_ROLES.contains(&origin.target())
                || (property == ResolvedStyleProperty::Fill && !fill_applies)
            {
                continue;
            }
            terminal.winners.insert((origin.rule_index(), property));
            if property == ResolvedStyleProperty::Fill {
                if unverified {
                    terminal.unknown_fill_winners.insert(origin.rule_index());
                }
                if inherited
                    && !unverified
                    && self
                        .fill
                        .as_ref()
                        .is_some_and(|fill| fill.rule_index() == origin.rule_index())
                {
                    terminal.consumed.insert(origin.rule_index());
                }
            }
        }
        // Unsupported ordinal paint may leave static CSS visible, but only the actual winner
        // can be certified. Another unshadowed occurrence can still consume the static rule.
        if fill_applies
            && matches!(style.fill_resolution().specified(), Specified::Unspecified)
            && let Some(target) = cluster_palette_target(theme, terminal.ordinal)
            && CLUSTER_TEXT_ROLES.contains(&target)
        {
            terminal.palettes.insert(target);
        }
        if let Some(ResolvedThemeEffect::Binding { binding, .. }) = cluster_effect(theme, style)
            && CLUSTER_TEXT_ROLES.contains(&binding.target())
        {
            terminal
                .bindings
                .insert((binding.target(), binding.effect_id().to_owned()));
        }
        Ok(())
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        let receipt = self
            .terminal
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(terminal) = receipt.as_ref() else {
            return evidence;
        };
        if !terminal.valid
            || terminal.css_emitted != [true, true]
            || !terminal.terminals.values().all(|seen| *seen)
        {
            return evidence;
        }
        if self.background.requested() {
            evidence.merge_accounted_from(self.background.finish_evidence());
        }
        if self.generic_text.requested() {
            evidence.merge_accounted_from(self.generic_text.finish_evidence());
        }
        let Some(theme) = self.theme.as_ref() else {
            return evidence;
        };
        let mut observations =
            BTreeMap::<(usize, ThemeTarget), (bool, Option<FamilyThemeResidualReason>)>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: target @ (ThemeTarget::Title | ThemeTarget::ClusterLabel),
                    facet,
                    ..
                } => {
                    let observation = observations.entry((rule_index, target)).or_default();
                    let property = resolved_style_property_for_facet(facet);
                    if !terminal.winners.contains(&(rule_index, property)) {
                        continue;
                    }
                    if route.disposition() == FamilyThemeDisposition::TypedAdapter
                        && matches!(facet, FamilyThemeRuleFacet::Fill(_))
                        && self
                            .fill
                            .as_ref()
                            .is_some_and(|fill| fill.rule_index() == rule_index)
                        && terminal.consumed.contains(&rule_index)
                        && !terminal.unknown_fill_winners.contains(&rule_index)
                    {
                        observation.0 = true;
                    } else {
                        observation
                            .1
                            .get_or_insert(unsupported_residual_for_facet(facet));
                    }
                }
                FamilyThemeMechanism::OrdinalPalette {
                    target: target @ (ThemeTarget::Title | ThemeTarget::ClusterLabel),
                } => {
                    let key = theme.family_mechanism_key(route);
                    if terminal.palettes.contains(&target) {
                        evidence.mark_residual(
                            key,
                            FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                        );
                    } else {
                        evidence.mark_not_applicable(key);
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: target @ (ThemeTarget::Title | ThemeTarget::ClusterLabel),
                    ..
                } => {
                    let key = theme.family_mechanism_key(route);
                    if matches!(&key, FamilyThemeMechanismKey::EffectBinding { effect_id, .. } if terminal.bindings.contains(&(target, effect_id.clone())))
                    {
                        evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedEffect);
                    } else {
                        evidence.mark_not_applicable(key);
                    }
                }
                _ => {}
            }
        }
        for ((index, target), (applied, residual)) in observations {
            let key = FamilyThemeMechanismKey::Rule { index, target };
            if let Some(reason) = residual {
                evidence.mark_residual(key, reason);
            } else if applied {
                evidence.mark_applied_with_capabilities(
                    key,
                    [self
                        .fill
                        .as_ref()
                        .expect("typed cluster text fill")
                        .capability()],
                );
            } else {
                evidence.mark_not_applicable(key);
            }
        }
        evidence
    }
}

#[derive(Debug)]
struct FlowchartTextSurfacePaintReceipt {
    terminals: BTreeMap<String, bool>,
    valid: bool,
    css_emitted: [bool; 2],
    ordinal: usize,
    winners: BTreeSet<(usize, ResolvedStyleProperty)>,
    unknown_fill_winners: BTreeSet<usize>,
    consumed: BTreeSet<usize>,
    palettes: BTreeSet<ThemeTarget>,
    bindings: BTreeSet<(ThemeTarget, String)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, ThemeRule, ThemeRuleSet,
        ThemeStylePatch,
    };

    #[test]
    fn text_surface_consumers_preserve_the_complete_family_request_set() {
        for targets in [
            vec![ThemeTarget::Text],
            vec![ThemeTarget::Title],
            vec![ThemeTarget::ClusterLabel],
            vec![ThemeTarget::Text, ThemeTarget::Title],
            vec![ThemeTarget::Text, ThemeTarget::ClusterLabel],
            vec![ThemeTarget::Title, ThemeTarget::ClusterLabel],
            vec![
                ThemeTarget::Text,
                ThemeTarget::Title,
                ThemeTarget::ClusterLabel,
            ],
        ] {
            let mut rules = ThemeRuleSet::default();
            for target in &targets {
                rules = rules.with_rule(ThemeRule::new(
                    *target,
                    ThemeStylePatch::default().with_fill(CanvasPaint::Transparent),
                ));
            }
            let theme = DiagramThemeCompiler::new()
                .compile(DiagramThemeSpec::new().with_styles(rules))
                .unwrap()
                .resolve(crate::DiagramFamilyId::FLOWCHART);
            let work = OperationWorkMeter::new(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            );
            let plan = FlowchartTextSurfacePaintPlan::resolve(
                Some(&theme),
                &MermaidConfig::default(),
                false,
                &work,
            )
            .unwrap();
            plan.begin_terminal_emission(std::iter::empty::<&str>(), &work)
                .unwrap();
            let mut css = String::new();
            plan.write_css(&mut css, "black", false).unwrap();
            plan.write_css(&mut css, "black", true).unwrap();
            plan.generic_text.record_stylesheet_emission();
            plan.generic_text
                .record_label(
                    super::super::FlowchartTextPaintChannel::Node,
                    super::super::FlowchartTextPaintFacts::from_visible(
                        &VisibleTextStyleFacts::plain_text("node"),
                    ),
                    super::super::FlowchartSourceFacetStatus::Absent,
                    &work,
                )
                .unwrap();
            let mut family = FamilyThemeEvidence::from_theme(Some(&theme));
            family.merge_accounted_from(plan.finish_evidence());
            assert_eq!(
                family.applied().len(),
                usize::from(targets.contains(&ThemeTarget::Text))
            );
            assert_eq!(
                family.not_applicable_mechanisms().len(),
                targets
                    .iter()
                    .filter(|target| CLUSTER_TEXT_ROLES.contains(target))
                    .count()
            );
        }
    }

    #[test]
    fn unknown_cluster_color_cannot_certify_a_rule_consumed_by_another_cluster() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::ClusterLabel,
                        ThemeStylePatch::default().with_fill(CanvasPaint::Transparent),
                    ),
                )),
            )
            .unwrap()
            .resolve(crate::DiagramFamilyId::FLOWCHART);
        let work = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let plan = FlowchartTextSurfacePaintPlan::resolve(
            Some(&theme),
            &MermaidConfig::default(),
            false,
            &work,
        )
        .unwrap();
        plan.begin_terminal_emission(["known", "unknown"], &work)
            .unwrap();
        let mut css = String::new();
        plan.write_css(&mut css, "black", false).unwrap();
        plan.write_css(&mut css, "black", true).unwrap();
        for (id, source) in [
            ("known", super::super::FlowchartSourceFacetStatus::Absent),
            (
                "unknown",
                super::super::FlowchartSourceFacetStatus::Unverified,
            ),
        ] {
            plan.record_label(
                id,
                &VisibleTextStyleFacts::plain_text("label"),
                source,
                &work,
            )
            .unwrap();
        }
        let evidence = plan.finish_evidence();
        assert!(evidence.applied().is_empty());
        assert_eq!(evidence.residuals().len(), 1);
    }

    #[test]
    fn missing_or_duplicate_cluster_receipts_do_not_certify_the_stylesheet() {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::ClusterLabel,
                        ThemeStylePatch::default().with_fill(CanvasPaint::Transparent),
                    ),
                )),
            )
            .unwrap()
            .resolve(crate::DiagramFamilyId::FLOWCHART);
        for duplicate in [false, true] {
            let work = OperationWorkMeter::new(
                crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
            );
            let plan = FlowchartTextSurfacePaintPlan::resolve(
                Some(&theme),
                &MermaidConfig::default(),
                false,
                &work,
            )
            .unwrap();
            plan.begin_terminal_emission(["first", "second"], &work)
                .unwrap();
            let mut css = String::new();
            plan.write_css(&mut css, "black", false).unwrap();
            plan.write_css(&mut css, "black", true).unwrap();
            for id in if duplicate {
                vec!["first", "first", "second"]
            } else {
                vec!["first"]
            } {
                plan.record_label(
                    id,
                    &VisibleTextStyleFacts::plain_text("label"),
                    super::super::FlowchartSourceFacetStatus::Absent,
                    &work,
                )
                .unwrap();
            }
            let evidence = plan.finish_evidence();
            assert!(evidence.applied().is_empty());
            assert!(evidence.not_applicable_mechanisms().is_empty());
        }
    }
}
