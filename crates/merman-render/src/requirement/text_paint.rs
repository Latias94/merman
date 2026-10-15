use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

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

/// Source declarations are classified by the same node-style parser that emits them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RequirementTextColorOwner {
    Inherited,
    Static,
    Unverified,
}

impl RequirementTextColorOwner {
    pub(crate) fn from_source_color(color: Option<&str>) -> Self {
        match color {
            None => Self::Inherited,
            Some(color) if crate::mermaid_style::is_supported_css_color_value(color) => {
                Self::Static
            }
            // These declarations are emitted on the label group and inner label terminal.
            // They bypass nodeTextColor, but their inherited source color is not a fixed paint.
            Some(_) => Self::Unverified,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum RequirementTextTerminalId {
    Node { id: String, line: usize },
    Edge(String),
}

impl RequirementTextTerminalId {
    pub(crate) fn node(id: &str, line: usize) -> Self {
        Self::Node {
            id: id.to_owned(),
            line,
        }
    }
    pub(crate) fn edge(id: &str) -> Self {
        Self::Edge(id.to_owned())
    }
}

/// A shared CSS color is resolved once; actual label writers own applicability and ordinals.
#[derive(Debug)]
pub(crate) struct RequirementTextPaintPlan {
    theme: Option<ResolvedDiagramTheme>,
    style: Option<ResolvedThemeStyle>,
    fill: Option<DirectStaticPaint>,
    node_owned: bool,
    relation_owned: bool,
    ordinal_rules: bool,
    terminal: OnceLock<FamilyThemeEvidence>,
}

impl RequirementTextPaintPlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        config: &MermaidConfig,
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
        let style = theme
            .map(|theme| {
                theme.style_with_work_meter(ThemeTarget::Text, ThemeVariant::Default, None, work)
            })
            .transpose()?;
        let fill = theme.zip(style.as_ref()).and_then(|(theme, style)| {
            resolve_direct_static_fill(
                theme,
                style,
                &[ThemeTarget::Text],
                DirectStaticSelectorDomain::Default,
            )
        });
        Ok(Self {
            theme: theme.cloned(),
            style,
            fill,
            node_owned: merman_core::__private::config_path_overrides_typed_default(
                config,
                "themeVariables.nodeTextColor",
            ),
            relation_owned: merman_core::__private::config_path_overrides_typed_default(
                config,
                "themeVariables.relationLabelColor",
            ),
            ordinal_rules: theme.is_some_and(|theme| {
                theme
                    .family_rules()
                    .any(|(_, rule)| rule.target() == ThemeTarget::Text && rule.ordinal().is_some())
            }),
            terminal: OnceLock::new(),
        })
    }

    pub(crate) fn requested(&self) -> bool {
        self.theme.is_some()
    }

    pub(crate) fn begin_terminal_receipt(
        &self,
        expected: impl IntoIterator<Item = RequirementTextTerminalId>,
        work: &OperationWorkMeter,
    ) -> Result<Option<RequirementTextPaintReceipt<'_>>, OperationWorkError> {
        if !self.requested() {
            return Ok(None);
        }
        work.charge(
            self.theme
                .as_ref()
                .expect("requested text plan")
                .family_mechanism_routes()
                .len(),
        )?;
        let mut terminals = BTreeMap::new();
        let mut valid = true;
        for id in expected {
            work.charge(1)?;
            valid &= terminals.insert(id, false).is_none();
        }
        Ok(Some(RequirementTextPaintReceipt {
            plan: self,
            terminals,
            valid,
            css_counts: [0; 3],
            ordinal: 0,
            winners: BTreeSet::new(),
            unknown_fill_winners: BTreeSet::new(),
            typed_fill_consumed: false,
            palette_applies: false,
            bindings: BTreeSet::new(),
        }))
    }

    pub(crate) fn record_terminal(&self, receipt: Option<RequirementTextPaintReceipt<'_>>) -> bool {
        match receipt {
            Some(receipt) => {
                std::ptr::eq(receipt.plan, self)
                    && receipt.complete()
                    && self.terminal.set(receipt.finish_evidence()).is_ok()
            }
            None => !self.requested(),
        }
    }

    pub(crate) fn finish_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        if let Some(terminal) = self.terminal.get() {
            evidence.merge_accounted_from(terminal.clone());
        }
    }
}

#[derive(Debug)]
pub(crate) struct RequirementTextPaintReceipt<'a> {
    plan: &'a RequirementTextPaintPlan,
    terminals: BTreeMap<RequirementTextTerminalId, bool>,
    valid: bool,
    css_counts: [usize; 3],
    ordinal: usize,
    winners: BTreeSet<(usize, ResolvedStyleProperty)>,
    unknown_fill_winners: BTreeSet<usize>,
    typed_fill_consumed: bool,
    palette_applies: bool,
    bindings: BTreeSet<String>,
}

impl RequirementTextPaintReceipt<'_> {
    /// Own the exact declarations inside the original .label and .label text,span rules.
    pub(crate) fn write_node_css(
        &mut self,
        out: &mut impl std::fmt::Write,
        configured: &str,
        include_fill: bool,
    ) -> std::fmt::Result {
        self.css_counts[usize::from(include_fill)] += 1;
        let color = if self.plan.node_owned {
            configured
        } else {
            self.plan
                .fill
                .as_ref()
                .map_or(configured, DirectStaticPaint::css)
        };
        let result = if include_fill {
            write!(out, "fill:{color};color:{color};")
        } else {
            write!(out, "color:{color};")
        };
        self.valid &= result.is_ok();
        result
    }

    /// Own the original SVG edge-label fill declaration.
    pub(crate) fn write_relation_css(
        &mut self,
        out: &mut impl std::fmt::Write,
        configured: &str,
    ) -> std::fmt::Result {
        self.css_counts[2] += 1;
        let color = if self.plan.relation_owned {
            configured
        } else {
            self.plan
                .fill
                .as_ref()
                .map_or(configured, DirectStaticPaint::css)
        };
        let result = write!(out, "fill:{color};");
        self.valid &= result.is_ok();
        result
    }

    pub(crate) fn record_label(
        &mut self,
        id: RequirementTextTerminalId,
        svg_edge: bool,
        facts: &VisibleTextStyleFacts,
        source: RequirementTextColorOwner,
        work: &OperationWorkMeter,
    ) -> Result<(), OperationWorkError> {
        work.charge(1 + facts.visible_run_count())?;
        let Some(seen) = self.terminals.get_mut(&id) else {
            self.valid = false;
            return Ok(());
        };
        self.valid &= !*seen;
        *seen = true;
        if facts.parse_valid() && !facts.has_visible_runs() {
            return Ok(());
        }
        self.ordinal += 1;
        let config_owned = if svg_edge {
            self.plan.relation_owned
        } else {
            self.plan.node_owned
        };
        let source_owned = config_owned || source == RequirementTextColorOwner::Static;
        let unverified = !facts.parse_valid()
            || source == RequirementTextColorOwner::Unverified
            || facts.unverified_portable_color_run_count() != 0;
        let inherited = !source_owned && facts.inherited_color_run_count() != 0;
        let fill_applies = !source_owned && (inherited || unverified);
        let theme = self.plan.theme.as_ref().expect("requested text plan");
        let ordinal_style;
        let style = if self.plan.ordinal_rules {
            ordinal_style = theme.style_with_work_meter(
                ThemeTarget::Text,
                ThemeVariant::Default,
                Some(self.ordinal),
                work,
            )?;
            &ordinal_style
        } else {
            self.plan
                .style
                .as_ref()
                .expect("requested static text style")
        };
        for (property, origin) in style.winner_rule_properties() {
            work.charge(1)?;
            if property == ResolvedStyleProperty::Fill && !fill_applies {
                continue;
            }
            self.winners.insert((origin.rule_index(), property));
            if property == ResolvedStyleProperty::Fill && unverified {
                self.unknown_fill_winners.insert(origin.rule_index());
            }
        }
        // Static CSS remains the emitted paint even where a later unsupported ordinal wins.
        // That ordinal is still residual; it does not erase consumption by other occurrences.
        self.typed_fill_consumed |= inherited && !unverified && self.plan.fill.is_some();
        self.palette_applies |= fill_applies
            && matches!(style.fill_resolution().specified(), Specified::Unspecified)
            && theme
                .series_color(ThemeTarget::Text, self.ordinal)
                .is_some();
        if let Some(ResolvedThemeEffect::Binding { binding, .. }) =
            theme.resolve_effect(ThemeTarget::Text, style.effect_resolution())
        {
            self.bindings.insert(binding.effect_id().to_owned());
        }
        Ok(())
    }

    fn complete(&self) -> bool {
        self.valid && self.css_counts == [1, 1, 1] && self.terminals.values().all(|seen| *seen)
    }

    fn finish_evidence(self) -> FamilyThemeEvidence {
        let theme = self.plan.theme.as_ref().expect("requested text plan");
        let mut evidence = FamilyThemeEvidence::from_theme(Some(theme));
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
                    let property = resolved_style_property_for_facet(facet);
                    if !self.winners.contains(&(rule_index, property)) {
                        continue;
                    }
                    if route.disposition() == FamilyThemeDisposition::TypedAdapter
                        && matches!(facet, FamilyThemeRuleFacet::Fill(_))
                        && self
                            .plan
                            .fill
                            .as_ref()
                            .is_some_and(|fill| fill.rule_index() == rule_index)
                        && self.typed_fill_consumed
                        && !self.unknown_fill_winners.contains(&rule_index)
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
                    if self.palette_applies {
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
                    if matches!(&key, FamilyThemeMechanismKey::EffectBinding { effect_id, .. } if self.bindings.contains(effect_id))
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
                    [self.plan.fill.as_ref().expect("typed fill").capability()],
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
    use crate::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, ThemeRule, ThemeRuleSet,
        ThemeStylePatch,
    };
    use crate::resources::RenderResourcePolicy;

    fn plan(work: &OperationWorkMeter) -> RequirementTextPaintPlan {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(ThemeRule::new(
                        ThemeTarget::Text,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#2468ac").unwrap()),
                    )),
                ),
            )
            .unwrap()
            .resolve(crate::DiagramFamilyId::REQUIREMENT);
        RequirementTextPaintPlan::resolve(Some(&theme), &MermaidConfig::empty_object(), work)
            .unwrap()
    }

    fn write_css(receipt: &mut RequirementTextPaintReceipt<'_>) {
        let mut css = String::new();
        receipt.write_node_css(&mut css, "#111111", false).unwrap();
        receipt.write_node_css(&mut css, "#111111", true).unwrap();
        receipt.write_relation_css(&mut css, "#222222").unwrap();
        assert_eq!(
            css,
            "color:#2468ac;fill:#2468ac;color:#2468ac;fill:#2468ac;"
        );
    }

    #[test]
    fn requirement_text_receipt_requires_each_emitted_identity_and_css_role() {
        let work = OperationWorkMeter::new(RenderResourcePolicy::default());
        let plan = plan(&work);
        for failure in [
            "none",
            "missing",
            "duplicate",
            "wrong",
            "missing-css",
            "duplicate-css-role",
        ] {
            let id = RequirementTextTerminalId::node("req1", 0);
            let mut receipt = plan
                .begin_terminal_receipt([id.clone()], &work)
                .unwrap()
                .unwrap();
            if failure == "duplicate-css-role" {
                let mut css = String::new();
                receipt.write_node_css(&mut css, "#111111", false).unwrap();
                receipt.write_node_css(&mut css, "#111111", false).unwrap();
                receipt.write_relation_css(&mut css, "#222222").unwrap();
            } else if failure != "missing-css" {
                write_css(&mut receipt);
            }
            if failure != "missing" {
                let actual = if failure == "wrong" {
                    RequirementTextTerminalId::edge("edge1")
                } else {
                    id.clone()
                };
                receipt
                    .record_label(
                        actual,
                        false,
                        &VisibleTextStyleFacts::plain_text("Label"),
                        RequirementTextColorOwner::Inherited,
                        &work,
                    )
                    .unwrap();
            }
            if failure == "duplicate" {
                receipt
                    .record_label(
                        id,
                        false,
                        &VisibleTextStyleFacts::plain_text("Label"),
                        RequirementTextColorOwner::Inherited,
                        &work,
                    )
                    .unwrap();
            }
            assert_eq!(receipt.complete(), failure == "none", "{failure}");
        }
    }

    #[test]
    fn requirement_text_receipt_keeps_css_write_failure() {
        struct Reject;
        impl std::fmt::Write for Reject {
            fn write_str(&mut self, _: &str) -> std::fmt::Result {
                Err(std::fmt::Error)
            }
        }
        let work = OperationWorkMeter::new(RenderResourcePolicy::default());
        let plan = plan(&work);
        let mut receipt = plan.begin_terminal_receipt([], &work).unwrap().unwrap();
        assert!(
            receipt
                .write_node_css(&mut Reject, "#111111", false)
                .is_err()
        );
        receipt
            .write_node_css(&mut String::new(), "#111111", true)
            .unwrap();
        receipt
            .write_relation_css(&mut String::new(), "#222222")
            .unwrap();
        assert!(!receipt.complete());
    }
}
