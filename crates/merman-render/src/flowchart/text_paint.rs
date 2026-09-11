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

/// The four existing stylesheet color channels have independent role and config ownership.
#[derive(Debug, Clone, Copy)]
pub(crate) enum FlowchartTextPaintChannel {
    Node,
    Edge,
    Cluster,
    DiagramTitle,
}

impl FlowchartTextPaintChannel {
    fn target(self) -> ThemeTarget {
        match self {
            Self::Node => ThemeTarget::NodeLabel,
            Self::Edge => ThemeTarget::EdgeLabel,
            Self::Cluster => ThemeTarget::Title,
            Self::DiagramTitle => ThemeTarget::Text,
        }
    }
}

/// A bounded summary of the already emitted label, carried with its existing terminal receipt.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FlowchartTextPaintFacts {
    visible: bool,
    inherited: bool,
    unknown: bool,
    work: usize,
}

impl FlowchartTextPaintFacts {
    pub(crate) fn from_visible(facts: &VisibleTextStyleFacts) -> Self {
        Self {
            visible: facts.has_visible_runs() || !facts.parse_valid(),
            inherited: facts.inherited_color_run_count() != 0,
            unknown: !facts.parse_valid() || facts.unverified_portable_color_run_count() != 0,
            work: 1 + facts.visible_run_count(),
        }
    }

    pub(crate) fn unknown() -> Self {
        Self {
            visible: true,
            inherited: false,
            unknown: true,
            work: 1,
        }
    }
}

#[derive(Debug)]
struct ChannelPaint {
    style: ResolvedThemeStyle,
    fill: Option<DirectStaticPaint>,
    config_owned: bool,
    ordinal_rules: bool,
    cluster_override: bool,
    cluster_ordinal_rules: bool,
}

#[derive(Debug, Default)]
struct TextPaintReceipt {
    css_emitted: bool,
    ordinals: [usize; 4],
    winners: BTreeSet<(usize, ResolvedStyleProperty)>,
    unknown: BTreeSet<usize>,
    consumed: BTreeSet<usize>,
    palette_applies: bool,
    bindings: BTreeSet<String>,
}

#[derive(Debug)]
pub(crate) struct FlowchartTextPaintPlan {
    theme: Option<ResolvedDiagramTheme>,
    channels: Vec<ChannelPaint>,
    terminal: Mutex<TextPaintReceipt>,
}

impl FlowchartTextPaintPlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        config: &MermaidConfig,
        node_config_owned: bool,
        work: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let theme = theme.filter(|theme| {
            theme.family_mechanism_routes().iter().any(|route| {
                matches!(
                    route.mechanism(),
                    FamilyThemeMechanism::RuleFacet {
                        target: ThemeTarget::Text,
                        ..
                    } | FamilyThemeMechanism::OrdinalPalette {
                        target: ThemeTarget::Text
                    } | FamilyThemeMechanism::EffectBinding {
                        target: ThemeTarget::Text,
                        ..
                    }
                )
            })
        });
        let mut channels = Vec::new();
        if let Some(theme) = theme {
            work.charge(theme.family_mechanism_routes().len())?;
            for channel in [
                FlowchartTextPaintChannel::Node,
                FlowchartTextPaintChannel::Edge,
                FlowchartTextPaintChannel::Cluster,
                FlowchartTextPaintChannel::DiagramTitle,
            ] {
                let target = channel.target();
                let style =
                    theme.text_style_with_work_meter(target, ThemeVariant::Default, None, work)?;
                work.charge(theme.family_mechanism_routes().len())?;
                let cluster_role = matches!(channel, FlowchartTextPaintChannel::Cluster)
                    && theme
                        .family_rules()
                        .any(|(_, rule)| rule.target() == ThemeTarget::ClusterLabel);
                let cluster_override = cluster_role
                    && theme
                        .text_style_with_work_meter(
                            ThemeTarget::ClusterLabel,
                            ThemeVariant::Default,
                            None,
                            work,
                        )?
                        .fill_resolution()
                        .winner()
                        .is_some_and(|origin| origin.target() == ThemeTarget::ClusterLabel);
                work.charge(theme.family_mechanism_routes().len())?;
                let cluster_ordinal_rules = cluster_role
                    && theme.family_rules().any(|(_, rule)| {
                        matches!(rule.target(), ThemeTarget::Text | ThemeTarget::ClusterLabel)
                            && rule.ordinal().is_some()
                    });
                let fill = (!cluster_override)
                    .then(|| {
                        resolve_direct_static_fill(
                            theme,
                            &style,
                            &[ThemeTarget::Text],
                            DirectStaticSelectorDomain::Default,
                        )
                    })
                    .flatten();
                let config_owned = match channel {
                    FlowchartTextPaintChannel::Node | FlowchartTextPaintChannel::Edge => {
                        node_config_owned
                    }
                    FlowchartTextPaintChannel::Cluster => {
                        merman_core::__private::config_path_overrides_typed_default(
                            config,
                            "themeVariables.titleColor",
                        )
                    }
                    FlowchartTextPaintChannel::DiagramTitle => {
                        merman_core::__private::config_path_overrides_typed_default(
                            config,
                            "themeVariables.textColor",
                        )
                    }
                };
                work.charge(theme.family_mechanism_routes().len())?;
                channels.push(ChannelPaint {
                    style,
                    fill,
                    config_owned,
                    cluster_override,
                    cluster_ordinal_rules,
                    ordinal_rules: theme.family_rules().any(|(_, rule)| {
                        (rule.target() == ThemeTarget::Text || rule.target() == target)
                            && rule.ordinal().is_some()
                    }),
                });
            }
        }
        Ok(Self {
            theme: theme.cloned(),
            channels,
            terminal: Mutex::new(TextPaintReceipt::default()),
        })
    }

    pub(crate) fn requested(&self) -> bool {
        self.theme.is_some()
    }

    pub(crate) fn color<'a>(
        &'a self,
        channel: FlowchartTextPaintChannel,
        configured: &'a str,
    ) -> &'a str {
        self.channels
            .get(channel as usize)
            .filter(|paint| !paint.config_owned)
            .and_then(|paint| paint.fill.as_ref())
            .map_or(configured, DirectStaticPaint::css)
    }

    pub(crate) fn record_stylesheet_emission(&self) {
        self.terminal
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .css_emitted = true;
    }

    pub(crate) fn record_label(
        &self,
        channel: FlowchartTextPaintChannel,
        facts: FlowchartTextPaintFacts,
        source: super::FlowchartSourceFacetStatus,
        work: &OperationWorkMeter,
    ) -> Result<(), OperationWorkError> {
        let Some(theme) = &self.theme else {
            return Ok(());
        };
        work.charge(facts.work)?;
        if !facts.visible {
            return Ok(());
        }
        let mut terminal = self
            .terminal
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let paint = &self.channels[channel as usize];
        terminal.ordinals[channel as usize] += 1;
        let ordinal = terminal.ordinals[channel as usize];
        let ordinal_style;
        let style = if paint.ordinal_rules {
            ordinal_style = theme.text_style_with_work_meter(
                channel.target(),
                ThemeVariant::Default,
                Some(ordinal),
                work,
            )?;
            &ordinal_style
        } else {
            &paint.style
        };
        let cluster_override = if paint.cluster_ordinal_rules {
            theme
                .text_style_with_work_meter(
                    ThemeTarget::ClusterLabel,
                    ThemeVariant::Default,
                    Some(ordinal),
                    work,
                )?
                .fill_resolution()
                .winner()
                .is_some_and(|origin| origin.target() == ThemeTarget::ClusterLabel)
        } else {
            paint.cluster_override
        };
        let source_owned =
            paint.config_owned || source == super::FlowchartSourceFacetStatus::Admitted;
        let unknown = facts.unknown || source == super::FlowchartSourceFacetStatus::Unverified;
        let fill_applies = !source_owned && !cluster_override && (facts.inherited || unknown);
        for (property, origin) in style.winner_rule_properties() {
            work.charge(1)?;
            if origin.target() != ThemeTarget::Text
                || (property == ResolvedStyleProperty::Fill && !fill_applies)
            {
                continue;
            }
            terminal.winners.insert((origin.rule_index(), property));
            if property == ResolvedStyleProperty::Fill {
                if unknown {
                    terminal.unknown.insert(origin.rule_index());
                }
                if facts.inherited
                    && !unknown
                    && paint
                        .fill
                        .as_ref()
                        .is_some_and(|fill| fill.rule_index() == origin.rule_index())
                {
                    terminal.consumed.insert(origin.rule_index());
                }
            }
        }
        terminal.palette_applies |= fill_applies
            && matches!(style.fill_resolution().specified(), Specified::Unspecified)
            && theme.series_color(ThemeTarget::Text, ordinal).is_some();
        if let Some(ResolvedThemeEffect::Binding { binding, .. }) =
            theme.resolve_effect(channel.target(), style.effect_resolution())
        {
            terminal.bindings.insert(binding.effect_id().to_owned());
        }
        Ok(())
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = FamilyThemeEvidence::from_theme(self.theme.as_ref());
        let Some(theme) = &self.theme else {
            return evidence;
        };
        let terminal = self
            .terminal
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !terminal.css_emitted {
            return evidence;
        }
        let mut observations = BTreeMap::<usize, (bool, Option<FamilyThemeResidualReason>)>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: ThemeTarget::Text,
                    facet,
                    ..
                } => {
                    let observation = observations.entry(rule_index).or_default();
                    if !terminal
                        .winners
                        .contains(&(rule_index, resolved_style_property_for_facet(facet)))
                    {
                        continue;
                    }
                    if route.disposition() == FamilyThemeDisposition::TypedAdapter
                        && matches!(facet, FamilyThemeRuleFacet::Fill(_))
                        && terminal.consumed.contains(&rule_index)
                        && !terminal.unknown.contains(&rule_index)
                    {
                        observation.0 = true;
                    } else {
                        observation
                            .1
                            .get_or_insert(unsupported_residual_for_facet(facet));
                    }
                }
                FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::Text,
                } => {
                    let key = theme.family_mechanism_key(route);
                    if terminal.palette_applies {
                        evidence.mark_residual(
                            key,
                            FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                        );
                    } else {
                        evidence.mark_not_applicable(key);
                    }
                }
                FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::Text,
                    ..
                } => {
                    let key = theme.family_mechanism_key(route);
                    if matches!(&key, FamilyThemeMechanismKey::EffectBinding { effect_id, .. } if terminal.bindings.contains(effect_id))
                    {
                        evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedEffect);
                    } else {
                        evidence.mark_not_applicable(key);
                    }
                }
                _ => {}
            }
        }
        for (index, (applied, residual)) in observations {
            let key = FamilyThemeMechanismKey::Rule {
                index,
                target: ThemeTarget::Text,
            };
            if let Some(reason) = residual {
                evidence.mark_residual(key, reason);
            } else if applied {
                evidence.mark_applied_with_capabilities(
                    key,
                    self.channels
                        .iter()
                        .filter_map(|channel| channel.fill.as_ref())
                        .filter(|fill| fill.rule_index() == index)
                        .map(DirectStaticPaint::capability),
                );
            } else {
                evidence.mark_not_applicable(key);
            }
        }
        evidence
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, OrdinalPalette, OrdinalSelector,
        ThemeColorValue, ThemeRule, ThemeRuleSet, ThemeStylePatch,
    };

    fn resolve(rules: ThemeRuleSet) -> (FlowchartTextPaintPlan, OperationWorkMeter) {
        let theme = DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(rules))
            .unwrap()
            .resolve(DiagramFamilyId::FLOWCHART);
        let meter = OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let plan =
            FlowchartTextPaintPlan::resolve(Some(&theme), &MermaidConfig::default(), false, &meter)
                .unwrap();
        plan.record_stylesheet_emission();
        (plan, meter)
    }

    fn fill(target: ThemeTarget) -> ThemeRule {
        ThemeRule::new(
            target,
            ThemeStylePatch::default().with_fill(CanvasPaint::Transparent),
        )
    }

    fn record(
        plan: &FlowchartTextPaintPlan,
        channel: FlowchartTextPaintChannel,
        fragment: &str,
        meter: &OperationWorkMeter,
    ) {
        plan.record_label(
            channel,
            FlowchartTextPaintFacts::from_visible(&VisibleTextStyleFacts::from_xhtml_fragment(
                fragment,
            )),
            super::super::FlowchartSourceFacetStatus::Absent,
            meter,
        )
        .unwrap();
    }

    #[test]
    fn unknown_owner_on_any_channel_prevents_generic_rule_certification() {
        for channel in [
            FlowchartTextPaintChannel::Node,
            FlowchartTextPaintChannel::Edge,
            FlowchartTextPaintChannel::Cluster,
            FlowchartTextPaintChannel::DiagramTitle,
        ] {
            let (plan, meter) = resolve(ThemeRuleSet::default().with_rule(fill(ThemeTarget::Text)));
            record(
                &plan,
                FlowchartTextPaintChannel::Node,
                "<p>known</p>",
                &meter,
            );
            record(
                &plan,
                channel,
                "<span class=\"unknown\">unverified</span>",
                &meter,
            );
            let evidence = plan.finish_evidence();
            assert!(evidence.applied().is_empty());
            assert_eq!(evidence.residuals().len(), 1);
        }
    }

    #[test]
    fn invisible_or_role_owned_labels_do_not_consume_a_generic_fallback() {
        let (plan, meter) = resolve(
            ThemeRuleSet::default()
                .with_rule(fill(ThemeTarget::Text))
                .with_rule(fill(ThemeTarget::NodeLabel)),
        );
        record(
            &plan,
            FlowchartTextPaintChannel::Node,
            "<p>role winner</p>",
            &meter,
        );
        record(
            &plan,
            FlowchartTextPaintChannel::Edge,
            "<img src=\"asset\"/>",
            &meter,
        );
        let evidence = plan.finish_evidence();
        assert!(evidence.applied().is_empty());
        assert!(
            evidence
                .not_applicable_mechanisms()
                .contains(&FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::Text,
                })
        );
    }

    #[test]
    fn a_winning_sibling_facet_stays_residual_after_fill_consumption() {
        let rule = ThemeRule::new(
            ThemeTarget::Text,
            ThemeStylePatch::default()
                .with_fill(CanvasPaint::Transparent)
                .with_stroke(CanvasPaint::Transparent),
        );
        let (plan, meter) = resolve(ThemeRuleSet::default().with_rule(rule));
        record(&plan, FlowchartTextPaintChannel::Node, "text", &meter);
        assert!(plan.finish_evidence().applied().is_empty());
        assert_eq!(plan.finish_evidence().residuals().len(), 1);
    }

    #[test]
    fn an_ordinal_winner_does_not_certify_the_static_css_fallback() {
        let (plan, meter) = resolve(
            ThemeRuleSet::default()
                .with_rule(fill(ThemeTarget::Text))
                .with_rule(
                    fill(ThemeTarget::Text).with_ordinal(OrdinalSelector::exact(1).unwrap()),
                ),
        );
        record(&plan, FlowchartTextPaintChannel::Node, "text", &meter);
        let evidence = plan.finish_evidence();
        assert!(evidence.applied().is_empty());
        assert_eq!(evidence.residuals().len(), 1);
        assert!(
            evidence
                .not_applicable_mechanisms()
                .contains(&FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::Text,
                })
        );
    }

    #[test]
    fn an_unconsumed_palette_is_not_applicable_but_a_visible_palette_is_residual() {
        let palette = OrdinalPalette::new([ThemeColorValue::parse("#123456").unwrap()]).unwrap();
        let (plan, meter) =
            resolve(ThemeRuleSet::default().with_ordinal_palette(ThemeTarget::Text, palette));
        assert!(plan.finish_evidence().residuals().is_empty());
        record(&plan, FlowchartTextPaintChannel::Edge, "label", &meter);
        assert_eq!(plan.finish_evidence().residuals().len(), 1);
        assert!(plan.finish_evidence().applied().is_empty());
    }

    #[test]
    fn static_cluster_override_is_reused_for_every_visible_cluster() {
        let (plan, meter) = resolve(
            ThemeRuleSet::default()
                .with_rule(fill(ThemeTarget::Text))
                .with_rule(fill(ThemeTarget::ClusterLabel)),
        );
        let cluster = &plan.channels[FlowchartTextPaintChannel::Cluster as usize];
        assert!(cluster.cluster_override);
        assert!(!cluster.cluster_ordinal_rules);
        for _ in 0..3 {
            let before = meter.used();
            record(&plan, FlowchartTextPaintChannel::Cluster, "cluster", &meter);
            // One visible run, the receipt, and one winning property need work per label.
            assert_eq!(meter.used() - before, 3);
        }
        assert!(plan.finish_evidence().applied().is_empty());
    }

    #[test]
    fn ordinal_cluster_override_only_shadows_its_selected_label() {
        let (plan, meter) = resolve(
            ThemeRuleSet::default()
                .with_rule(fill(ThemeTarget::Text))
                .with_rule(
                    fill(ThemeTarget::ClusterLabel)
                        .with_ordinal(OrdinalSelector::exact(1).unwrap()),
                ),
        );
        let cluster = &plan.channels[FlowchartTextPaintChannel::Cluster as usize];
        assert!(!cluster.cluster_override);
        assert!(cluster.cluster_ordinal_rules);
        let before = meter.used();
        record(&plan, FlowchartTextPaintChannel::Cluster, "first", &meter);
        assert!(
            meter.used() - before > 3,
            "ordinal candidate resolution must be metered"
        );
        assert!(plan.finish_evidence().applied().is_empty());
        record(&plan, FlowchartTextPaintChannel::Cluster, "second", &meter);
        assert_eq!(plan.finish_evidence().applied().len(), 1);
    }

    #[test]
    fn text_ordinals_preserve_a_static_cluster_role_override() {
        let (plan, meter) = resolve(
            ThemeRuleSet::default()
                .with_rule(fill(ThemeTarget::Text))
                .with_rule(fill(ThemeTarget::Text).with_ordinal(OrdinalSelector::exact(1).unwrap()))
                .with_rule(fill(ThemeTarget::ClusterLabel)),
        );
        let cluster = &plan.channels[FlowchartTextPaintChannel::Cluster as usize];
        assert!(cluster.cluster_override);
        assert!(cluster.cluster_ordinal_rules);
        record(&plan, FlowchartTextPaintChannel::Cluster, "first", &meter);
        record(&plan, FlowchartTextPaintChannel::Cluster, "second", &meter);
        let evidence = plan.finish_evidence();
        assert!(evidence.applied().is_empty());
        assert!(
            evidence
                .not_applicable_mechanisms()
                .contains(&FamilyThemeMechanismKey::Rule {
                    index: 0,
                    target: ThemeTarget::Text,
                })
        );
        assert!(
            evidence
                .not_applicable_mechanisms()
                .contains(&FamilyThemeMechanismKey::Rule {
                    index: 1,
                    target: ThemeTarget::Text,
                })
        );
    }

    #[test]
    fn cluster_label_role_keeps_its_indirect_title_color_consumer() {
        let (plan, meter) = resolve(
            ThemeRuleSet::default()
                .with_rule(fill(ThemeTarget::Text))
                .with_rule(fill(ThemeTarget::ClusterLabel)),
        );
        assert_eq!(
            plan.color(FlowchartTextPaintChannel::Cluster, "configured"),
            "configured"
        );
        record(&plan, FlowchartTextPaintChannel::Cluster, "cluster", &meter);
        assert!(plan.finish_evidence().applied().is_empty());
        assert!(plan.finish_evidence().not_applicable_mechanisms().contains(
            &FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Text,
            }
        ));
    }
}
