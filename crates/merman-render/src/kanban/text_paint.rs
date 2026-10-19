use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, FamilyThemeRuleFacet,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty, ThemeTarget,
    ThemeVariant,
};
use crate::family::{
    DirectStaticPaint, DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeResidualReason,
    UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains, resolve_direct_static_fill,
    resolved_style_property_for_facet, unsupported_residual_for_facet,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

use super::{KanbanPreparedItem, KanbanPreparedMarkdownLabel, KanbanTaskTheme};

/// Inherited Text paint is less specific than task-label paint and source XHTML color.
#[derive(Debug)]
pub(crate) struct KanbanTextPaintPlan {
    fill: Option<DirectStaticPaint>,
    evidence: FamilyThemeEvidence,
    pending: Vec<(FamilyThemeMechanismKey, Option<FamilyThemeResidualReason>)>,
    text_keys: BTreeSet<FamilyThemeMechanismKey>,
    terminal: OnceLock<KanbanTextPaintOutcome>,
}

/// Aggregate facts reuse preparation's XHTML classification, without retaining terminal IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct KanbanTextPaintFacts {
    visible_runs: usize,
    inherited_runs: usize,
    partial_color_ownership: bool,
    unverified_color_runs: usize,
    parse_valid: bool,
}

impl Default for KanbanTextPaintFacts {
    fn default() -> Self {
        Self {
            visible_runs: 0,
            inherited_runs: 0,
            partial_color_ownership: false,
            unverified_color_runs: 0,
            parse_valid: true,
        }
    }
}

impl KanbanTextPaintFacts {
    fn record(
        &mut self,
        parse_valid: bool,
        visible_runs: usize,
        inherited_runs: usize,
        unverified_color_runs: usize,
        task_label_owned: bool,
    ) {
        self.visible_runs += visible_runs;
        if !task_label_owned {
            self.inherited_runs += inherited_runs;
            self.partial_color_ownership |= inherited_runs > 0 && inherited_runs < visible_runs;
            self.unverified_color_runs += unverified_color_runs;
        }
        self.parse_valid &= parse_valid;
    }
}

#[derive(Debug, Default)]
pub(crate) struct KanbanTextPaintOutcome {
    emitted: KanbanTextPaintFacts,
    css_written: bool,
}

impl KanbanTextPaintPlan {
    pub(crate) fn baseline() -> Self {
        Self {
            fill: None,
            evidence: FamilyThemeEvidence::default(),
            pending: Vec::new(),
            text_keys: BTreeSet::new(),
            terminal: OnceLock::new(),
        }
    }

    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        config: &MermaidConfig,
        sections: &[KanbanPreparedMarkdownLabel],
        items: &[KanbanPreparedItem],
        tasks: &KanbanTaskTheme,
        work: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let Some(theme) = theme else {
            return Ok(Self::baseline());
        };
        let mut plan = Self {
            evidence: FamilyThemeEvidence::from_theme(Some(theme)),
            ..Self::baseline()
        };
        for route in theme.family_mechanism_routes() {
            work.charge(1)?;
            if matches!(
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
            ) {
                let key = theme.family_mechanism_key(*route);
                plan.text_keys.insert(key);
            }
        }
        if plan.text_keys.is_empty() {
            return Ok(plan);
        }
        let mut ownership = KanbanTextPaintFacts::default();
        for section in sections {
            let facts = &section.visible_style_facts;
            work.charge(1usize.saturating_add(facts.visible_run_count()))?;
            ownership.record(
                facts.parse_valid(),
                facts.visible_run_count(),
                facts.inherited_color_run_count(),
                facts.unverified_color_run_count(),
                false,
            );
        }
        for (index, item) in items.iter().enumerate() {
            let task_label_owned = tasks.owns_label_foreground(index);
            for facts in [
                &item.title.visible_style_facts,
                &item.ticket.visible_style_facts,
                &item.assigned.visible_style_facts,
            ] {
                work.charge(1usize.saturating_add(facts.visible_run_count()))?;
                ownership.record(
                    facts.parse_valid(),
                    facts.visible_run_count(),
                    facts.inherited_color_run_count(),
                    facts.unverified_color_run_count(),
                    task_label_owned,
                );
            }
        }
        let config_owned = merman_core::__private::config_path_overrides_typed_default(
            config,
            "themeVariables.textColor",
        );
        let style =
            theme.style_with_work_meter(ThemeTarget::Text, ThemeVariant::Default, None, work)?;
        let winners = style
            .winner_rule_properties()
            .map(|(property, origin)| (origin.rule_index(), property))
            .collect::<BTreeSet<_>>();
        if !config_owned {
            plan.fill = resolve_direct_static_fill(
                theme,
                &style,
                &[ThemeTarget::Text],
                DirectStaticSelectorDomain::Default,
            );
        }
        // An empty visible domain has no effect or palette terminal. The writer records
        // its N/A result after successfully emitting the document.
        if ownership.parse_valid && ownership.visible_runs == 0 {
            return Ok(plan);
        }
        let source_owned_fill = config_owned
            || (ownership.parse_valid
                && ownership.unverified_color_runs == 0
                && ownership.inherited_runs == 0);
        let mut observations = BTreeMap::new();
        for route in theme.family_mechanism_routes() {
            let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::Text,
                facet,
                selector,
            } = route.mechanism()
            else {
                continue;
            };
            work.charge(1)?;
            let entry = observations
                .entry(theme.family_mechanism_key(*route))
                .or_insert((false, None));
            let property = resolved_style_property_for_facet(facet);
            // Text has no semantic ordinal numbering. A later static winner can supersede it.
            let unproved_ordinal = matches!(
                selector,
                FamilyThemeSelectorShape::Ordinal {
                    variant: None | Some(ThemeVariant::Default),
                    ..
                }
            ) && !winners.iter().any(|(winner, winner_property)| {
                *winner_property == property && *winner > rule_index
            });
            if (!winners.contains(&(rule_index, property)) && !unproved_ordinal)
                || (property == ResolvedStyleProperty::Fill && source_owned_fill)
            {
                continue;
            }
            if route.disposition() == FamilyThemeDisposition::TypedAdapter
                && matches!(facet, FamilyThemeRuleFacet::Fill(_))
                && plan
                    .fill
                    .as_ref()
                    .is_some_and(|fill| fill.rule_index() == rule_index)
            {
                entry.0 = true;
            } else {
                entry.1.get_or_insert(unsupported_residual_for_facet(facet));
            }
        }
        for (key, (fill, residual)) in observations {
            if fill || residual.is_some() {
                plan.pending.push((key, residual));
            } else {
                plan.evidence.mark_not_applicable(key);
            }
        }
        reconcile_unsupported_terminal_domains(
            theme,
            &mut plan.evidence,
            &[UnsupportedTerminalDomain::static_fallbacks_only(
                ThemeTarget::Text,
                ThemeVariant::Default,
            )
            .with_source_owned_fill(&[source_owned_fill])],
            work,
        )?;
        Ok(plan)
    }

    pub(crate) fn fill_css(&self) -> Option<&str> {
        self.fill.as_ref().map(DirectStaticPaint::css)
    }

    pub(crate) fn begin_terminal_outcome(&self) -> Option<KanbanTextPaintOutcome> {
        (!self.text_keys.is_empty()).then(KanbanTextPaintOutcome::default)
    }

    pub(crate) fn record_terminal(&self, outcome: KanbanTextPaintOutcome) {
        let _ = self.terminal.set(outcome);
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        if self
            .terminal
            .get()
            .is_some_and(|outcome| outcome.emitted.parse_valid && outcome.emitted.visible_runs == 0)
        {
            for key in &self.text_keys {
                evidence.mark_not_applicable(key.clone());
            }
            return evidence;
        }
        for (key, residual) in &self.pending {
            if let Some(reason) = residual {
                evidence.mark_residual(key.clone(), *reason);
            } else {
                if self.terminal.get().is_some_and(|outcome| {
                    outcome.css_written
                        && outcome.emitted.parse_valid
                        && outcome.emitted.inherited_runs > 0
                        && !outcome.emitted.partial_color_ownership
                        && outcome.emitted.unverified_color_runs == 0
                }) {
                    if let Some(fill) = &self.fill {
                        evidence.mark_applied_with_capabilities(key.clone(), [fill.capability()]);
                    }
                } else {
                    evidence
                        .mark_residual(key.clone(), FamilyThemeResidualReason::UnsupportedPaint);
                }
            }
        }
        evidence
    }
}

impl KanbanTextPaintOutcome {
    pub(crate) fn record_css(&mut self) {
        self.css_written = true;
    }

    pub(crate) fn record_label(
        &mut self,
        parse_valid: bool,
        visible_runs: usize,
        inherited_runs: usize,
        unverified_color_runs: usize,
        task_label_owned: bool,
    ) {
        self.emitted.record(
            parse_valid,
            visible_runs,
            inherited_runs,
            unverified_color_runs,
            task_label_owned,
        );
    }
}
