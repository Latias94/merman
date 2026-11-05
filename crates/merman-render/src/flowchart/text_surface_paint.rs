use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

use merman_model::presentation::PresentationConfig;

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

pub(super) fn resolve_document_title_style(
    theme: &ResolvedDiagramTheme,
    ordinal: Option<usize>,
    work: &OperationWorkMeter,
) -> Result<ResolvedThemeStyle, OperationWorkError> {
    theme.text_style_with_role_override(
        ThemeTarget::Text,
        ThemeTarget::Title,
        ThemeVariant::Default,
        ordinal,
        work,
    )
}

pub(super) fn resolve_cluster_text_style(
    theme: &ResolvedDiagramTheme,
    ordinal: Option<usize>,
    work: &OperationWorkMeter,
) -> Result<ResolvedThemeStyle, OperationWorkError> {
    if theme.family_id() == crate::DiagramFamilyId::AGENTFLOW {
        let mut style = resolve_document_title_style(theme, ordinal, work)?;
        style.overlay_role(theme.style_with_work_meter(
            ThemeTarget::ClusterLabel,
            ThemeVariant::Default,
            ordinal,
            work,
        )?);
        return Ok(style);
    }
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

#[derive(Debug)]
struct DocumentTitlePaint {
    style: ResolvedThemeStyle,
    fill: Option<DirectStaticPaint>,
    config_owned: bool,
}

/// Shared CSS colors are resolved once; actual label writers own applicability and ordinals.
#[derive(Debug)]
pub(crate) struct FlowchartTextSurfacePaintPlan {
    pub(crate) background: crate::family::EdgeLabelBackgroundPlan,
    pub(crate) generic_text: super::FlowchartTextPaintPlan,
    theme: Option<ResolvedDiagramTheme>,
    style: Option<ResolvedThemeStyle>,
    fill: Option<DirectStaticPaint>,
    document_title: Option<DocumentTitlePaint>,
    config_owned: bool,
    ordinal_rules: bool,
    terminal: Mutex<Option<FlowchartTextSurfacePaintReceipt>>,
}

impl FlowchartTextSurfacePaintPlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        config: &PresentationConfig,
        node_config_owned: bool,
        work: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let background = crate::family::EdgeLabelBackgroundPlan::resolve(theme, config, work)?;
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
        let document_title = theme
            .filter(|theme| theme.family_id() == crate::DiagramFamilyId::AGENTFLOW)
            .map(|theme| {
                let style = resolve_document_title_style(theme, Some(1), work)?;
                let fill = resolve_direct_static_fill(
                    theme,
                    &style,
                    &[ThemeTarget::Title],
                    DirectStaticSelectorDomain::Default,
                );
                Ok::<_, OperationWorkError>(DocumentTitlePaint {
                    style,
                    fill,
                    config_owned: config
                        .config_paint_overrides_typed_default("themeVariables.textColor"),
                })
            })
            .transpose()?;
        Ok(Self {
            background,
            generic_text,
            theme: theme.cloned(),
            style,
            fill,
            document_title,
            config_owned: config.config_paint_overrides_typed_default("themeVariables.titleColor"),
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
        work: &OperationWorkMeter,
    ) -> Result<(), OperationWorkError> {
        if !self.requested() {
            return Ok(());
        }
        work.checkpoint(merman_model::OperationPhase::Emit)?;
        let mut terminal = self
            .terminal
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *terminal = Some(FlowchartTextSurfacePaintReceipt {
            css_emitted: [false; 2],
            document_title_css_emitted: false,
            ordinal: 0,
            winners: BTreeSet::new(),
            unknown_fill_winners: BTreeSet::new(),
            consumed: BTreeSet::new(),
            palettes: BTreeSet::new(),
            bindings: BTreeSet::new(),
        });
        Ok(())
    }

    pub(crate) fn document_title_color<'a>(&'a self, configured: &'a str) -> &'a str {
        if let Some(title) = &self.document_title {
            if title.config_owned {
                return configured;
            }
            if let Some(fill) = &title.fill {
                return fill.css();
            }
            if title
                .style
                .winner_rule_properties()
                .any(|(property, origin)| {
                    property == ResolvedStyleProperty::Fill && origin.target() == ThemeTarget::Title
                })
            {
                return configured;
            }
        }
        self.generic_text
            .color(super::FlowchartTextPaintChannel::DiagramTitle, configured)
    }

    pub(crate) fn record_document_title_css(&self, color: &str) {
        if let Some(receipt) = self
            .terminal
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_mut()
        {
            receipt.document_title_css_emitted |= self
                .document_title
                .as_ref()
                .and_then(|title| title.fill.as_ref())
                .is_some_and(|fill| fill.css() == color);
        }
    }

    pub(crate) fn record_document_title(
        &self,
        text: &str,
        work: &OperationWorkMeter,
    ) -> Result<(), OperationWorkError> {
        let facts = VisibleTextStyleFacts::plain_text(text);
        self.generic_text.record_label(
            super::FlowchartTextPaintChannel::DiagramTitle,
            super::FlowchartTextPaintFacts::from_visible(&facts),
            super::FlowchartSourceFacetStatus::Absent,
            work,
        )?;
        let Some((theme, title)) = self.theme.as_ref().zip(self.document_title.as_ref()) else {
            return Ok(());
        };
        if !facts.has_visible_runs() {
            return Ok(());
        }
        let mut guard = self
            .terminal
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(receipt) = guard.as_mut() else {
            return Ok(());
        };
        for (property, origin) in title.style.winner_rule_properties() {
            work.charge(1)?;
            if origin.target() != ThemeTarget::Title
                || property == ResolvedStyleProperty::Fill && title.config_owned
            {
                continue;
            }
            receipt.winners.insert((origin.rule_index(), property));
            if property == ResolvedStyleProperty::Fill
                && receipt.document_title_css_emitted
                && title
                    .fill
                    .as_ref()
                    .is_some_and(|fill| fill.rule_index() == origin.rule_index())
            {
                receipt.consumed.insert(origin.rule_index());
            }
        }
        if !title.config_owned
            && matches!(
                title.style.fill_resolution().specified(),
                Specified::Unspecified
            )
            && theme.series_color(ThemeTarget::Title, 1).is_some()
        {
            receipt.palettes.insert(ThemeTarget::Title);
        }
        if let Some(ResolvedThemeEffect::Binding { binding, .. }) =
            theme.resolve_effect(ThemeTarget::Title, title.style.effect_resolution())
        {
            receipt
                .bindings
                .insert((binding.target(), binding.effect_id().to_owned()));
        }
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
        let _ = id;
        work.charge(1 + facts.visible_run_count())?;
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
        let mut receipt = self
            .terminal
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(terminal) = receipt.as_mut() else {
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

    pub(crate) fn record_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        if self.background.requested() {
            self.background.record_evidence(evidence);
        }
        if self.generic_text.requested() {
            self.generic_text.record_evidence(evidence);
        }
        let receipt = self
            .terminal
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(terminal) = receipt.as_ref() else {
            return;
        };
        if terminal.css_emitted != [true, true] {
            return;
        }
        let Some(theme) = self.theme.as_ref() else {
            return;
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
                        && self.fill_for_rule(rule_index).is_some()
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
                        .fill_for_rule(index)
                        .expect("typed cluster text fill")
                        .capability()],
                );
            } else {
                evidence.mark_not_applicable(key);
            }
        }
    }

    fn fill_for_rule(&self, index: usize) -> Option<&DirectStaticPaint> {
        self.fill
            .iter()
            .chain(
                self.document_title
                    .as_ref()
                    .and_then(|title| title.fill.as_ref()),
            )
            .find(|fill| fill.rule_index() == index)
    }
}

#[derive(Debug)]
struct FlowchartTextSurfacePaintReceipt {
    css_emitted: [bool; 2],
    document_title_css_emitted: bool,
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
    fn text_surface_consumers_merge_only_their_own_outcomes() {
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
                &PresentationConfig::default(),
                false,
                &work,
            )
            .unwrap();
            plan.begin_terminal_emission(&work).unwrap();
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
            plan.record_evidence(&mut family);
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
            &PresentationConfig::default(),
            false,
            &work,
        )
        .unwrap();
        plan.begin_terminal_emission(&work).unwrap();
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
        let mut evidence = FamilyThemeEvidence::from_theme(Some(&theme));
        plan.record_evidence(&mut evidence);
        assert!(evidence.applied().is_empty());
        assert_eq!(evidence.residuals().len(), 1);
    }
}
