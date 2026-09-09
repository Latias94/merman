use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty, ResolvedThemeEffect,
    Specified, ThemeTarget, ThemeVariant,
};
use crate::family::{
    DirectStaticPaint, DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeResidualReason,
    resolve_direct_static_fill, resolved_style_property_for_facet, unsupported_residual_for_facet,
};
use crate::model::GitGraphDiagramLayout;
use crate::resources::{OperationWorkError, OperationWorkMeter};

use super::{GitGraphCommitKind, gitgraph_commit_label_is_visible, palette_slot};

// Text owns three distinct legacy sources. Later label rules and each configured source can
// independently suppress one of them without suppressing the other two.
const SOURCES: [&str; 3] = [
    "themeVariables.textColor",
    "themeVariables.tagLabelColor",
    "themeVariables.commitLabelColor",
];

#[derive(Debug, Default)]
struct TextObservation {
    direct: u8,
    unsupported_fill: u8,
    fill_reason: Option<FamilyThemeResidualReason>,
    typography: u8,
    other: [Option<FamilyThemeResidualReason>; 3],
}

#[derive(Debug)]
pub(crate) struct GitGraphTextPaintPlan {
    fills: [Option<DirectStaticPaint>; 3],
    observations: BTreeMap<FamilyThemeMechanismKey, TextObservation>,
    evidence: FamilyThemeEvidence,
    title: Option<Box<str>>,
    terminal: OnceLock<TextTerminalFacts>,
}

#[derive(Debug, Default)]
struct TextTerminalFacts {
    sources: [usize; 3],
    // Root title/branch text, tag text, and commit text, independent of fill ownership.
    text_visible: u8,
}

impl TextTerminalFacts {
    fn visible_mask(&self) -> u8 {
        self.sources
            .iter()
            .enumerate()
            .fold(0, |mask, (i, count)| mask | (u8::from(*count > 0) << i))
    }
}

impl GitGraphTextPaintPlan {
    pub(crate) fn baseline() -> Self {
        Self {
            fills: std::array::from_fn(|_| None),
            observations: BTreeMap::new(),
            evidence: FamilyThemeEvidence::default(),
            title: None,
            terminal: OnceLock::new(),
        }
    }

    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        config: &MermaidConfig,
        title: Option<&str>,
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
                plan.observations
                    .entry(theme.family_mechanism_key(*route))
                    .or_default();
            }
        }
        if !plan.is_requested() {
            return Ok(plan);
        }
        plan.title = title.map(Box::from);
        let styles = [
            theme.text_style_with_work_meter(
                ThemeTarget::Text,
                ThemeVariant::Default,
                None,
                work,
            )?,
            theme.text_style_with_work_meter(
                ThemeTarget::NodeLabel,
                ThemeVariant::Default,
                None,
                work,
            )?,
            theme.text_style_with_work_meter(
                ThemeTarget::EdgeLabel,
                ThemeVariant::Default,
                None,
                work,
            )?,
        ];
        let color_gen = config
            .get_str("theme")
            .is_some_and(crate::gitgraph::gitgraph_theme_uses_color_gen);
        let owned: [bool; 3] = std::array::from_fn(|i| {
            merman_core::__private::config_path_overrides_typed_default(config, SOURCES[i])
                || (i == 2 && color_gen)
        });
        plan.fills = std::array::from_fn(|i| {
            (!owned[i])
                .then(|| {
                    resolve_direct_static_fill(
                        theme,
                        &styles[i],
                        &[ThemeTarget::Text],
                        DirectStaticSelectorDomain::Default,
                    )
                })
                .flatten()
        });
        for route in theme.family_mechanism_routes() {
            let key = theme.family_mechanism_key(*route);
            let Some(observation) = plan.observations.get_mut(&key) else {
                continue;
            };
            work.charge(1)?;
            match route.mechanism() {
                FamilyThemeMechanism::RuleFacet {
                    rule_index,
                    facet,
                    selector,
                    ..
                } => {
                    let property = resolved_style_property_for_facet(facet);
                    if property == ResolvedStyleProperty::Fill {
                        for (i, style) in styles.iter().enumerate() {
                            if owned[i] {
                                continue;
                            }
                            let winner = style.fill_resolution().winner();
                            let won =
                                winner.is_some_and(|origin| origin.rule_index() == rule_index);
                            // Text has no semantic ordinal identity. Only a later static
                            // winner supersedes an ordinal, regardless of the winner's target.
                            let unproved_ordinal = matches!(
                                selector,
                                FamilyThemeSelectorShape::Ordinal {
                                    variant: None | Some(ThemeVariant::Default),
                                    ..
                                }
                            ) && winner
                                .is_none_or(|origin| origin.rule_index() <= rule_index);
                            if !won && !unproved_ordinal {
                                continue;
                            }
                            let mask = 1 << i;
                            if route.disposition() == FamilyThemeDisposition::TypedAdapter
                                && plan.fills[i]
                                    .as_ref()
                                    .is_some_and(|fill| fill.rule_index() == rule_index)
                            {
                                observation.direct |= mask;
                            } else {
                                observation.unsupported_fill |= mask;
                                observation.fill_reason =
                                    Some(unsupported_residual_for_facet(facet));
                            }
                        }
                    } else {
                        for (i, style) in styles.iter().enumerate() {
                            let won = style.winner_rule_properties().any(|(candidate, origin)| {
                                candidate == property && origin.rule_index() == rule_index
                            });
                            let unproved_ordinal =
                                matches!(
                                    selector,
                                    FamilyThemeSelectorShape::Ordinal {
                                        variant: None | Some(ThemeVariant::Default),
                                        ..
                                    }
                                ) && !style.winner_rule_properties().any(|(candidate, origin)| {
                                    candidate == property && origin.rule_index() > rule_index
                                });
                            if won || unproved_ordinal {
                                let reason = unsupported_residual_for_facet(facet);
                                if reason == FamilyThemeResidualReason::UnsupportedTypography {
                                    observation.typography |= 1 << i;
                                } else {
                                    observation.other[i].get_or_insert(reason);
                                }
                            }
                        }
                    }
                }
                FamilyThemeMechanism::OrdinalPalette { .. } => {
                    if theme.series_color(ThemeTarget::Text, 1).is_some() {
                        for (i, style) in styles.iter().enumerate() {
                            if !owned[i]
                                && matches!(
                                    style.fill_resolution().specified(),
                                    Specified::Unspecified
                                )
                            {
                                observation.unsupported_fill |= 1 << i;
                            }
                        }
                        observation.fill_reason =
                            Some(FamilyThemeResidualReason::UnsupportedOrdinalPalette);
                    }
                }
                FamilyThemeMechanism::EffectBinding { .. } => {
                    for (i, style) in styles.iter().enumerate() {
                        // A concrete role's effect winner can suppress the Text binding fallback.
                        if let Some(ResolvedThemeEffect::Binding { binding, .. }) =
                            theme.resolve_effect(ThemeTarget::Text, style.effect_resolution())
                            && key
                                == (FamilyThemeMechanismKey::EffectBinding {
                                    target: binding.target(),
                                    effect_id: binding.effect_id().to_string(),
                                })
                        {
                            observation.other[i] =
                                Some(FamilyThemeResidualReason::UnsupportedEffect);
                        }
                    }
                }
                FamilyThemeMechanism::BaseTypography(_) => unreachable!("Text routes only"),
            }
        }
        Ok(plan)
    }

    pub(crate) fn is_requested(&self) -> bool {
        !self.observations.is_empty()
    }

    pub(crate) fn css_colors(&self) -> [Option<&str>; 3] {
        std::array::from_fn(|i| self.fills[i].as_ref().map(DirectStaticPaint::css))
    }

    pub(crate) fn begin_terminal_receipt<'a>(
        &'a self,
        layout: &'a GitGraphDiagramLayout,
        branch_slots: &'a HashMap<&'a str, i64>,
        root_geometry: [[bool; 8]; 4],
        root_cherry_pick: bool,
    ) -> Option<GitGraphTextPaintReceipt<'a>> {
        self.is_requested().then(|| GitGraphTextPaintReceipt {
            plan: self,
            layout,
            branch_slots,
            root_geometry,
            root_cherry_pick,
            css_recorded: false,
            next_branch: 0,
            next_commit: 0,
            label_commit: 0,
            label_checked: false,
            next_tag: 0,
            title_recorded: false,
            valid: true,
            facts: TextTerminalFacts::default(),
        })
    }

    pub(crate) fn record_terminal(&self, mut receipt: GitGraphTextPaintReceipt<'_>) -> bool {
        let complete = std::ptr::eq(receipt.plan, self)
            && receipt.valid
            && receipt.css_recorded
            && receipt.title_recorded
            && receipt.next_branch
                == (if receipt.layout.show_branches {
                    receipt.layout.branches.len()
                } else {
                    0
                })
            && receipt.next_commit == receipt.layout.commits.len()
            && receipt.next_label().is_none();
        complete && self.terminal.set(receipt.facts).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        for (key, observation) in &self.observations {
            let Some(facts) = self.terminal.get() else {
                evidence.mark_residual(key.clone(), FamilyThemeResidualReason::UnsupportedPaint);
                continue;
            };
            let visible = facts.visible_mask();
            // Paint/effect/geometry can consume text or root-inheriting shapes. Typography
            // requires real text in the same role where this Text property remains the winner.
            let other_visible = visible | facts.text_visible;
            let other_reason = observation
                .other
                .iter()
                .enumerate()
                .find_map(|(i, reason)| {
                    (other_visible & (1 << i) != 0).then_some(*reason).flatten()
                });
            if let Some(reason) = other_reason {
                evidence.mark_residual(key.clone(), reason);
            } else if facts.text_visible & observation.typography != 0 {
                evidence.mark_residual(
                    key.clone(),
                    FamilyThemeResidualReason::UnsupportedTypography,
                );
            } else if observation.unsupported_fill & visible != 0 {
                evidence.mark_residual(
                    key.clone(),
                    observation
                        .fill_reason
                        .unwrap_or(FamilyThemeResidualReason::UnsupportedPaint),
                );
            } else if observation.direct & visible != 0 {
                let capabilities = self.fills.iter().enumerate().filter_map(|(i, fill)| {
                    (observation.direct & visible & (1 << i) != 0)
                        .then(|| fill.as_ref().map(DirectStaticPaint::capability))
                        .flatten()
                });
                evidence.mark_applied_with_capabilities(key.clone(), capabilities);
            } else {
                evidence.mark_not_applicable(key.clone());
            }
        }
        evidence
    }
}

#[derive(Debug, PartialEq, Eq)]
enum LabelEvent {
    Commit(usize),
    Tag(usize, usize),
}

/// Borrows existing layout identity and the writer's branch lookup; no occurrence vector is built.
#[derive(Debug)]
pub(crate) struct GitGraphTextPaintReceipt<'a> {
    plan: &'a GitGraphTextPaintPlan,
    layout: &'a GitGraphDiagramLayout,
    branch_slots: &'a HashMap<&'a str, i64>,
    // Branch backgrounds, ordinary commit outers, highlight outers, and branch text.
    root_geometry: [[bool; 8]; 4],
    root_cherry_pick: bool,
    css_recorded: bool,
    next_branch: usize,
    next_commit: usize,
    label_commit: usize,
    label_checked: bool,
    next_tag: usize,
    title_recorded: bool,
    valid: bool,
    facts: TextTerminalFacts,
}

impl GitGraphTextPaintReceipt<'_> {
    pub(crate) fn record_css(&mut self, colors: [&str; 5]) {
        self.valid &= !self.css_recorded;
        for (source, actual) in [0, 0, 0, 1, 2].into_iter().zip(colors) {
            self.valid &= self.plan.fills[source]
                .as_ref()
                .is_none_or(|paint| paint.css() == actual);
        }
        self.css_recorded = true;
    }

    pub(crate) fn record_branch(
        &mut self,
        index: usize,
        slot: usize,
        name: &str,
        root_inherited: bool,
        text_root_inherited: bool,
    ) {
        self.valid &= self.layout.show_branches && index == self.next_branch;
        self.valid &= self
            .layout
            .branches
            .get(index)
            .is_some_and(|branch| branch.name == name && palette_slot(branch.index) == slot);
        self.valid &= self.root_geometry[0].get(slot).copied() == Some(root_inherited);
        self.valid &= self.root_geometry[3].get(slot).copied() == Some(text_root_inherited);
        let text_visible = !name.trim().is_empty();
        self.facts.sources[0] +=
            usize::from(root_inherited) + usize::from(text_root_inherited && text_visible);
        self.facts.text_visible |= u8::from(text_visible);
        self.next_branch += 1;
    }

    pub(crate) fn record_commit(
        &mut self,
        index: usize,
        slot: usize,
        kind: GitGraphCommitKind,
        root_inherited: bool,
    ) {
        self.valid &= index == self.next_commit;
        self.valid &= self.layout.commits.get(index).is_some_and(|commit| {
            kind == GitGraphCommitKind::from_layout(commit)
                && slot
                    == palette_slot(
                        self.branch_slots
                            .get(commit.branch.as_str())
                            .copied()
                            .unwrap_or(0),
                    )
        });
        let expected = match kind {
            GitGraphCommitKind::CherryPick => self.root_cherry_pick,
            GitGraphCommitKind::Highlight => {
                self.root_geometry[2].get(slot).copied().unwrap_or(false)
            }
            _ => self.root_geometry[1].get(slot).copied().unwrap_or(false),
        };
        self.valid &= expected == root_inherited;
        self.facts.sources[0] += usize::from(root_inherited);
        self.next_commit += 1;
    }

    fn next_label(&mut self) -> Option<LabelEvent> {
        while let Some(commit) = self.layout.commits.get(self.label_commit) {
            if !self.label_checked {
                self.label_checked = true;
                if gitgraph_commit_label_is_visible(self.layout, commit) {
                    return Some(LabelEvent::Commit(self.label_commit));
                }
            }
            if self.next_tag < commit.tags.len() {
                let tag = self.next_tag;
                self.next_tag += 1;
                return Some(LabelEvent::Tag(self.label_commit, tag));
            }
            self.label_commit += 1;
            self.label_checked = false;
            self.next_tag = 0;
        }
        None
    }

    pub(crate) fn record_commit_label(&mut self, index: usize, text: &str) {
        let event = self.next_label();
        self.valid &= event == Some(LabelEvent::Commit(index));
        self.valid &= self
            .layout
            .commits
            .get(index)
            .is_some_and(|commit| commit.id == text);
        self.facts.sources[2] += usize::from(!text.trim().is_empty());
        self.facts.text_visible |= u8::from(!text.trim().is_empty()) << 2;
    }

    pub(crate) fn record_tag(&mut self, index: usize, output_index: usize, text: &str) {
        let event = self.next_label();
        self.valid &= event == Some(LabelEvent::Tag(index, output_index));
        self.valid &= self
            .layout
            .commits
            .get(index)
            .and_then(|commit| {
                commit
                    .tags
                    .len()
                    .checked_sub(output_index + 1)
                    .and_then(|i| commit.tags.get(i))
            })
            .is_some_and(|tag| tag == text);
        // Even an empty tag still emits a radius-1.5 colored hole.
        self.facts.sources[0] += 1;
        self.facts.sources[1] += usize::from(!text.trim().is_empty());
        self.facts.text_visible |= u8::from(!text.trim().is_empty()) << 1;
    }

    pub(crate) fn record_title(&mut self, title: Option<&str>) {
        self.valid &= !self.title_recorded && self.plan.title.as_deref() == title;
        self.title_recorded = true;
        self.facts.sources[0] += usize::from(title.is_some());
        self.facts.text_visible |= u8::from(title.is_some());
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
    fn text_receipt_rejects_missing_duplicate_and_wrong_terminal_facts() {
        let layout: GitGraphDiagramLayout = serde_json::from_value(serde_json::json!({
            "bounds": null, "direction": "LR", "rotate_commit_label": false,
            "show_branches": false, "show_commit_label": true, "parallel_commits": false,
            "diagram_padding": 0.0, "max_pos": 0.0, "branches": [], "arrows": [],
            "commits": [{"id": "A", "message": "", "seq": 0, "commit_type": 0,
                "custom_id": true, "tags": ["v1"], "parents": [], "branch": "main",
                "pos": 0.0, "pos_with_offset": 0.0, "x": 0.0, "y": 0.0}]
        }))
        .unwrap();
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(ThemeRule::new(
                        ThemeTarget::Text,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#123456").unwrap()),
                    )),
                ),
            )
            .unwrap()
            .resolve(crate::DiagramFamilyId::GIT_GRAPH);
        let branches = HashMap::new();
        for case in [
            "valid",
            "missing-css",
            "wrong-tag-color",
            "missing-commit",
            "duplicate-commit",
            "missing-tag",
            "wrong-tag",
            "wrong-title",
        ] {
            let plan = GitGraphTextPaintPlan::resolve(
                Some(&theme),
                &MermaidConfig::default(),
                Some("Title"),
                &OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input()),
            )
            .unwrap();
            let mut receipt = plan
                .begin_terminal_receipt(&layout, &branches, [[false; 8]; 4], false)
                .unwrap();
            if case != "missing-css" {
                let mut colors = ["#123456"; 5];
                if case == "wrong-tag-color" {
                    colors[3] = "#abcdef";
                }
                receipt.record_css(colors);
            }
            if case != "missing-commit" {
                receipt.record_commit(0, 0, GitGraphCommitKind::Normal, false);
            }
            if case == "duplicate-commit" {
                receipt.record_commit(0, 0, GitGraphCommitKind::Normal, false);
            }
            receipt.record_commit_label(0, "A");
            if case != "missing-tag" {
                receipt.record_tag(0, 0, if case == "wrong-tag" { "other" } else { "v1" });
            }
            receipt.record_title(Some(if case == "wrong-title" {
                "other"
            } else {
                "Title"
            }));
            assert_eq!(plan.record_terminal(receipt), case == "valid", "{case}");
            let evidence = plan.finish_evidence();
            assert_eq!(
                evidence.applied().len(),
                usize::from(case == "valid"),
                "{case}"
            );
            assert_eq!(
                evidence.residuals().len(),
                usize::from(case != "valid"),
                "{case}"
            );
        }
    }
}
