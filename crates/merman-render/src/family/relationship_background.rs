use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, ResolvedDiagramTheme,
    ResolvedStyleProperty, ThemeCapability, ThemeTarget, ThemeVariant,
};
use crate::family::{
    DirectPaintExpectation, DirectPaintTerminalLedger, DirectStaticSelectorDomain,
    FamilyThemeEvidence, FamilyThemeOutcomes, TerminalVariantDomain, UnsupportedTerminalDomain,
    reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

const TARGET: ThemeTarget = ThemeTarget::EdgeLabelBackground;

/// The HTML wrapper and its nested span have independent Mermaid paint owners.
#[derive(Debug)]
pub(crate) struct RelationshipLabelBackgroundPlan {
    theme: Option<ResolvedDiagramTheme>,
    outer_owned: bool,
    inner_owned: bool,
    terminal: OnceLock<FamilyThemeOutcomes>,
}

impl RelationshipLabelBackgroundPlan {
    pub(crate) fn new(
        theme: Option<&ResolvedDiagramTheme>,
        outer_owned: bool,
        inner_owned: bool,
    ) -> Self {
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
        Self {
            theme: theme.cloned(),
            outer_owned,
            inner_owned,
            terminal: OnceLock::new(),
        }
    }

    pub(crate) fn requested(&self) -> bool {
        self.theme.is_some()
    }

    pub(crate) fn begin_terminal_receipt(
        &self,
        occurrences: impl IntoIterator<Item = (String, usize)>,
        html: bool,
        work: &OperationWorkMeter,
    ) -> Result<Option<RelationshipLabelBackgroundReceipt>, OperationWorkError> {
        let Some(theme) = &self.theme else {
            return Ok(None);
        };
        let mut expected = BTreeMap::new();
        let mut ordinals = Vec::new();
        let mut valid = true;
        let fill_owned = self.inner_owned && (!html || self.outer_owned);
        for (id, ordinal) in occurrences {
            work.charge(1usize.saturating_add(id.len()))?;
            ordinals.push((ordinal, ThemeVariant::Default));
            let style =
                theme.style_with_work_meter(TARGET, ThemeVariant::Default, Some(ordinal), work)?;
            let paint = (!fill_owned)
                .then(|| {
                    resolve_direct_static_fill(
                        theme,
                        &style,
                        &[TARGET],
                        DirectStaticSelectorDomain::Default,
                    )
                    .map(DirectPaintExpectation::from_paint)
                })
                .flatten();
            valid &= expected.insert(id, paint).is_none();
        }
        let mut outcomes = FamilyThemeOutcomes::default();
        reconcile_unsupported_terminal_domains(
            theme,
            &mut outcomes,
            &[
                UnsupportedTerminalDomain::direct(
                    TARGET,
                    TerminalVariantDomain::indexed(&ordinals),
                )
                .with_fill_fully_overridden(fill_owned),
            ],
            work,
        )?;
        let mut rules = BTreeMap::new();
        for route in theme.family_mechanism_routes() {
            work.charge(1)?;
            if let FamilyThemeMechanism::RuleFacet {
                target: TARGET,
                rule_index,
                ..
            } = route.mechanism()
                && route.disposition() == FamilyThemeDisposition::TypedAdapter
            {
                rules.entry(rule_index).or_insert(None);
            }
        }
        for paint in expected.values().flatten() {
            work.charge(1)?;
            rules.insert(paint.rule_index(), Some(paint.capability()));
        }
        Ok(Some(RelationshipLabelBackgroundReceipt {
            expected,
            seen: BTreeSet::new(),
            rules,
            outcomes,
            outer_owned: self.outer_owned,
            inner_owned: self.inner_owned,
            html,
            valid,
            ledger: DirectPaintTerminalLedger::default(),
        }))
    }

    pub(crate) fn record_terminal(
        &self,
        receipt: Option<RelationshipLabelBackgroundReceipt>,
    ) -> bool {
        match receipt {
            None => self.theme.is_none(),
            Some(receipt) => receipt.complete() && self.terminal.set(receipt.finish()).is_ok(),
        }
    }

    pub(crate) fn record_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        if let Some(outcomes) = self.terminal.get() {
            evidence.merge_borrowed(outcomes);
        }
    }
}

#[derive(Debug)]
pub(crate) struct RelationshipLabelBackgroundReceipt {
    expected: BTreeMap<String, Option<DirectPaintExpectation>>,
    seen: BTreeSet<String>,
    rules: BTreeMap<usize, Option<ThemeCapability>>,
    outcomes: FamilyThemeOutcomes,
    outer_owned: bool,
    inner_owned: bool,
    html: bool,
    valid: bool,
    ledger: DirectPaintTerminalLedger,
}

impl RelationshipLabelBackgroundReceipt {
    /// Cheap Arc-backed values keep the emitted attributes independent of the receipt borrow.
    pub(crate) fn paints(&self, id: &str) -> [Option<DirectPaintExpectation>; 2] {
        let paint = self.expected.get(id).and_then(Option::as_ref);
        [
            paint.filter(|_| self.html && !self.outer_owned).cloned(),
            paint.filter(|_| !self.inner_owned).cloned(),
        ]
    }

    /// Called only after the actual div/span or rect write has passed its checkpoint.
    pub(crate) fn record(&mut self, id: &str, emitted: [Option<&DirectPaintExpectation>; 2]) {
        let Some(expected) = self.expected.get(id) else {
            self.valid = false;
            return;
        };
        self.valid &= self.seen.insert(id.to_owned());
        if self.html {
            self.valid &= self.ledger.record(
                expected.as_ref(),
                self.outer_owned,
                emitted[0].map(|paint| (paint.rule_index(), paint.css())),
                ResolvedStyleProperty::Fill,
            );
        } else {
            self.valid &= emitted[0].is_none();
        }
        self.valid &= self.ledger.record(
            expected.as_ref(),
            self.inner_owned,
            emitted[1].map(|paint| (paint.rule_index(), paint.css())),
            ResolvedStyleProperty::Fill,
        );
    }

    fn complete(&self) -> bool {
        self.valid && self.seen.len() == self.expected.len()
    }

    fn finish(mut self) -> FamilyThemeOutcomes {
        for (rule_index, capability) in self.rules {
            let key = FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target: TARGET,
            };
            if let Some(capability) = capability {
                if self
                    .ledger
                    .proves_property(self.valid, rule_index, ResolvedStyleProperty::Fill)
                {
                    self.outcomes
                        .mark_applied_with_capabilities(key, [capability]);
                }
            } else {
                self.outcomes.mark_not_applicable(key);
            }
        }
        self.outcomes
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

    #[test]
    fn receipt_rejects_missing_nested_backing_wrong_paint_duplicate_and_missing_labels() {
        for family in [
            #[cfg(feature = "diagram-er")]
            crate::DiagramFamilyId::ER,
            #[cfg(feature = "diagram-requirement")]
            crate::DiagramFamilyId::REQUIREMENT,
        ] {
            for failure in 0..7 {
                let theme = DiagramThemeCompiler::new()
                    .compile(
                        DiagramThemeSpec::new().with_styles(
                            ThemeRuleSet::default().with_rule(ThemeRule::new(
                                TARGET,
                                ThemeStylePatch::default()
                                    .with_fill(CanvasPaint::solid("#102537").unwrap()),
                            )),
                        ),
                    )
                    .unwrap()
                    .resolve(family);
                let work =
                    OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
                let plan = RelationshipLabelBackgroundPlan::new(Some(&theme), false, false);
                let mut receipt = plan
                    .begin_terminal_receipt(
                        [("first".to_owned(), 2), ("second".to_owned(), 4)],
                        true,
                        &work,
                    )
                    .unwrap()
                    .unwrap();
                let mut paints = receipt.paints("first");
                match failure {
                    0 => paints[0] = None,
                    1 => paints[1] = None,
                    2 => {
                        let expected = paints[1].as_ref().unwrap();
                        paints[1] = Some(DirectPaintExpectation::new(
                            expected.rule_index(),
                            "#abcdef",
                            expected.capability(),
                        ));
                    }
                    _ => {}
                }
                receipt.record(
                    if failure == 3 { "unknown" } else { "first" },
                    paints.each_ref().map(Option::as_ref),
                );
                if failure == 4 {
                    receipt.record("first", paints.each_ref().map(Option::as_ref));
                }
                if failure != 5 {
                    let paints = receipt.paints("second");
                    receipt.record("second", paints.each_ref().map(Option::as_ref));
                }
                assert_eq!(
                    plan.record_terminal(Some(receipt)),
                    failure == 6,
                    "terminal failure {failure}"
                );
            }
        }
    }
}
