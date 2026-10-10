use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

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

const TARGET: ThemeTarget = ThemeTarget::EdgeLabelBackground;

/// The shared background CSS has its own semantic target, independent of label foreground.
#[derive(Debug)]
pub(crate) struct FlowchartLabelBackgroundPlan {
    theme: Option<ResolvedDiagramTheme>,
    style: Option<ResolvedThemeStyle>,
    fill: Option<DirectStaticPaint>,
    config_owned: bool,
    ordinal_rules: bool,
    terminal: Mutex<BackgroundReceipt>,
}

#[derive(Debug, Default)]
struct BackgroundReceipt {
    stylesheet_emitted: bool,
    ordinal: usize,
    winners: BTreeSet<(usize, ResolvedStyleProperty)>,
    complete_fills: BTreeMap<usize, bool>,
    palette_applies: bool,
    bindings: BTreeSet<String>,
}

impl FlowchartLabelBackgroundPlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        config: &merman_core::MermaidConfig,
        work: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let theme = theme.filter(|theme| {
            theme.family_mechanism_routes().iter().any(|route| {
                matches!(
                    route.mechanism(),
                    FamilyThemeMechanism::RuleFacet { target: TARGET, .. }
                        | FamilyThemeMechanism::OrdinalPalette { target: TARGET }
                        | FamilyThemeMechanism::EffectBinding { target: TARGET, .. }
                )
            })
        });
        let style = theme
            .map(|theme| {
                work.charge(theme.family_mechanism_routes().len())?;
                theme.style_with_work_meter(TARGET, ThemeVariant::Default, None, work)
            })
            .transpose()?;
        let fill = theme.zip(style.as_ref()).and_then(|(theme, style)| {
            resolve_direct_static_fill(theme, style, &[TARGET], DirectStaticSelectorDomain::Default)
        });
        Ok(Self {
            theme: theme.cloned(),
            style,
            fill,
            config_owned: merman_core::__private::config_path_overrides_typed_default(
                config,
                "themeVariables.edgeLabelBackground",
            ),
            ordinal_rules: theme.is_some_and(|theme| {
                theme
                    .family_rules()
                    .any(|(_, rule)| rule.target() == TARGET && rule.ordinal().is_some())
            }),
            terminal: Mutex::new(BackgroundReceipt::default()),
        })
    }

    pub(crate) fn requested(&self) -> bool {
        self.theme.is_some()
    }

    /// An explicit static background introduces a content box on Swimlane labelRect.
    /// The unthemed Mermaid layout sentinel remains unchanged.
    pub(crate) fn supplies_background(&self) -> bool {
        self.fill.is_some() && !self.config_owned
    }

    pub(crate) fn color<'a>(&'a self, configured: &'a str) -> &'a str {
        if self.config_owned {
            configured
        } else {
            self.fill
                .as_ref()
                .map_or(configured, DirectStaticPaint::css)
        }
    }

    pub(crate) fn record_stylesheet(&self) {
        self.terminal
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stylesheet_emitted = true;
    }

    /// Reconcile the semantic label surface separately from its emitted background paint.
    /// An unsupported request still targets a sized label even when it creates no rectangle.
    pub(crate) fn record_terminal(
        &self,
        has_area: bool,
        background_emitted: bool,
        work: &OperationWorkMeter,
    ) -> Result<(), OperationWorkError> {
        let Some(theme) = &self.theme else {
            return Ok(());
        };
        work.charge(1)?;
        if !has_area {
            return Ok(());
        }
        let mut receipt = self
            .terminal
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        receipt.ordinal += 1;
        let dynamic;
        let style = if self.ordinal_rules {
            dynamic = theme.style_with_work_meter(
                TARGET,
                ThemeVariant::Default,
                Some(receipt.ordinal),
                work,
            )?;
            &dynamic
        } else {
            self.style.as_ref().expect("requested background style")
        };
        for (property, origin) in style.winner_rule_properties() {
            work.charge(1)?;
            if property == ResolvedStyleProperty::Fill && self.config_owned {
                continue;
            }
            receipt.winners.insert((origin.rule_index(), property));
            if property == ResolvedStyleProperty::Fill {
                let consumed = background_emitted
                    && self
                        .fill
                        .as_ref()
                        .is_some_and(|fill| fill.rule_index() == origin.rule_index());
                // A successful sibling terminal cannot erase an unverified occurrence.
                receipt
                    .complete_fills
                    .entry(origin.rule_index())
                    .and_modify(|complete| *complete &= consumed)
                    .or_insert(consumed);
            }
        }
        receipt.palette_applies |= !self.config_owned
            && matches!(style.fill_resolution().specified(), Specified::Unspecified)
            && theme.series_color(TARGET, receipt.ordinal).is_some();
        if let Some(ResolvedThemeEffect::Binding { binding, .. }) =
            theme.resolve_effect(TARGET, style.effect_resolution())
            && !receipt.bindings.contains(binding.effect_id())
        {
            receipt.bindings.insert(binding.effect_id().to_owned());
        }
        Ok(())
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = FamilyThemeEvidence::from_theme(self.theme.as_ref());
        let Some(theme) = &self.theme else {
            return evidence;
        };
        let receipt = self
            .terminal
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !receipt.stylesheet_emitted {
            return evidence;
        }
        let mut rules = BTreeMap::<usize, (bool, Option<FamilyThemeResidualReason>)>::new();
        for route in theme.family_mechanism_routes().iter().copied() {
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    target: TARGET,
                    facet,
                    ..
                } => {
                    let outcome = rules.entry(rule_index).or_default();
                    if !receipt
                        .winners
                        .contains(&(rule_index, resolved_style_property_for_facet(facet)))
                    {
                        continue;
                    }
                    if route.disposition() == FamilyThemeDisposition::TypedAdapter
                        && matches!(facet, FamilyThemeRuleFacet::Fill(_))
                        && receipt.complete_fills.get(&rule_index) == Some(&true)
                    {
                        outcome.0 = true;
                    } else {
                        outcome
                            .1
                            .get_or_insert(unsupported_residual_for_facet(facet));
                    }
                }
                FamilyThemeMechanism::OrdinalPalette { target: TARGET } => {
                    let key = theme.family_mechanism_key(route);
                    if receipt.palette_applies {
                        evidence.mark_residual(
                            key,
                            FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                        );
                    } else {
                        evidence.mark_not_applicable(key);
                    }
                }
                FamilyThemeMechanism::EffectBinding { target: TARGET, .. } => {
                    let key = theme.family_mechanism_key(route);
                    if matches!(&key, FamilyThemeMechanismKey::EffectBinding { effect_id, .. } if receipt.bindings.contains(effect_id))
                    {
                        evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedEffect);
                    } else {
                        evidence.mark_not_applicable(key);
                    }
                }
                _ => {}
            }
        }
        for (index, (applied, residual)) in rules {
            let key = FamilyThemeMechanismKey::Rule {
                index,
                target: TARGET,
            };
            if let Some(reason) = residual {
                evidence.mark_residual(key, reason);
            } else if applied {
                evidence.mark_applied_with_capabilities(
                    key,
                    [self
                        .fill
                        .as_ref()
                        .expect("consumed background fill")
                        .capability()],
                );
            } else {
                evidence.mark_not_applicable(key);
            }
        }
        evidence
    }
}
