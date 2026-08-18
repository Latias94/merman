use std::collections::{BTreeSet, HashMap};
use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanismKey, ResolvedDiagramTheme, ThemeCapability,
    ThemeTarget, ThemeVariant,
};
use crate::family::{FamilyThemeEvidence, FamilyThemeResidualReason};
use crate::model::{GitGraphCommitLayout, GitGraphDiagramLayout};
use crate::resources::{OperationWorkError, OperationWorkMeter};

pub(crate) const GITGRAPH_PALETTE_SLOT_COUNT: usize = 8;

const GIT_COLOR_PATHS: [&str; GITGRAPH_PALETTE_SLOT_COUNT] = [
    "themeVariables.git0",
    "themeVariables.git1",
    "themeVariables.git2",
    "themeVariables.git3",
    "themeVariables.git4",
    "themeVariables.git5",
    "themeVariables.git6",
    "themeVariables.git7",
];

#[derive(Debug)]
struct GitGraphNodePaletteFill {
    css: String,
    capability: ThemeCapability,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExpectedCommitElement {
    commit_index: usize,
    element_index: usize,
    slot: usize,
}

/// Family-local projection of the direct GitGraph Node ordinal palette.
///
/// The plan owns only the eight visible `gitN` slots consumed by commit shapes, arrows, and branch
/// label backgrounds. Mermaid keeps ownership of `gitInvN`, `gitBranchLabelN`, branch lines, and
/// text colors. The terminal writer must provide a complete occurrence receipt before the palette
/// can become Applied evidence.
#[derive(Debug)]
pub(crate) struct GitGraphNodePalettePlan {
    fills_by_slot: [Option<GitGraphNodePaletteFill>; GITGRAPH_PALETTE_SLOT_COUNT],
    evidence: FamilyThemeEvidence,
    palette_key: Option<FamilyThemeMechanismKey>,
    expected_rule_slots: Vec<usize>,
    expected_branch_label_slots: Vec<usize>,
    expected_arrow_slots: Vec<usize>,
    expected_commit_elements: Vec<ExpectedCommitElement>,
    terminal_receipt: OnceLock<GitGraphNodePaletteReceipt>,
}

impl GitGraphNodePalettePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
        layout: &GitGraphDiagramLayout,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let mut plan = Self::baseline(layout);
        let Some(theme) = theme else {
            return Ok(plan);
        };

        plan.evidence = FamilyThemeEvidence::from_theme(Some(theme));
        let Some(disposition) = theme.ordinal_palette_disposition(ThemeTarget::Node) else {
            return Ok(plan);
        };
        let key = FamilyThemeMechanismKey::OrdinalPalette {
            target: ThemeTarget::Node,
        };
        if !plan.has_visible_surface() {
            plan.evidence.mark_not_applicable(key);
            return Ok(plan);
        }

        match disposition {
            FamilyThemeDisposition::TypedAdapter => {
                let visible_slots = plan.visible_slots();
                for (slot, visible) in visible_slots.into_iter().enumerate() {
                    if !visible
                        || merman_core::__private::config_path_overrides_typed_default(
                            effective_config,
                            GIT_COLOR_PATHS[slot],
                        )
                    {
                        continue;
                    }

                    let style = theme.style_with_work_meter(
                        ThemeTarget::Node,
                        ThemeVariant::Default,
                        Some(slot + 1),
                        work_meter,
                    )?;
                    if style.fill_resolution().winner().is_some()
                        || style.stroke_resolution().winner().is_some()
                    {
                        continue;
                    }

                    let Some(color) = theme.series_color(ThemeTarget::Node, slot + 1) else {
                        plan.evidence.mark_residual(
                            key,
                            FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                        );
                        return Ok(plan);
                    };
                    plan.fills_by_slot[slot] = Some(GitGraphNodePaletteFill {
                        css: color.as_css(),
                        capability: if color.is_transparent() {
                            ThemeCapability::TransparentPaint
                        } else {
                            ThemeCapability::SolidPaint
                        },
                    });
                }

                plan.expected_rule_slots = (0..GITGRAPH_PALETTE_SLOT_COUNT)
                    .filter(|slot| plan.fills_by_slot[*slot].is_some())
                    .collect();
                if plan.expected_rule_slots.is_empty() {
                    plan.evidence.mark_not_applicable(key);
                } else {
                    plan.palette_key = Some(key);
                }
            }
            FamilyThemeDisposition::Unsupported => plan
                .evidence
                .mark_residual(key, FamilyThemeResidualReason::UnsupportedOrdinalPalette),
            FamilyThemeDisposition::LegacyCompatibility => {}
        }

        Ok(plan)
    }

    pub(crate) fn baseline(layout: &GitGraphDiagramLayout) -> Self {
        let expected_branch_label_slots = if layout.show_branches {
            layout
                .branches
                .iter()
                .map(|branch| palette_slot(branch.index))
                .collect()
        } else {
            Vec::new()
        };
        let expected_arrow_slots = layout
            .arrows
            .iter()
            .map(|arrow| palette_slot(arrow.class_index))
            .collect();
        let branch_slots = layout
            .branches
            .iter()
            .map(|branch| (branch.name.as_str(), palette_slot(branch.index)))
            .collect::<HashMap<_, _>>();
        let mut expected_commit_elements = Vec::new();
        for (commit_index, commit) in layout.commits.iter().enumerate() {
            let slot = branch_slots
                .get(commit.branch.as_str())
                .copied()
                .unwrap_or(0);
            for element_index in 0..palette_element_count(commit) {
                expected_commit_elements.push(ExpectedCommitElement {
                    commit_index,
                    element_index,
                    slot,
                });
            }
        }

        Self {
            fills_by_slot: std::array::from_fn(|_| None),
            evidence: FamilyThemeEvidence::default(),
            palette_key: None,
            expected_rule_slots: Vec::new(),
            expected_branch_label_slots,
            expected_arrow_slots,
            expected_commit_elements,
            terminal_receipt: OnceLock::new(),
        }
    }

    fn has_visible_surface(&self) -> bool {
        !self.expected_branch_label_slots.is_empty()
            || !self.expected_arrow_slots.is_empty()
            || !self.expected_commit_elements.is_empty()
    }

    fn visible_slots(&self) -> [bool; GITGRAPH_PALETTE_SLOT_COUNT] {
        let mut visible = [false; GITGRAPH_PALETTE_SLOT_COUNT];
        for slot in self
            .expected_branch_label_slots
            .iter()
            .chain(self.expected_arrow_slots.iter())
            .chain(
                self.expected_commit_elements
                    .iter()
                    .map(|element| &element.slot),
            )
        {
            visible[*slot] = true;
        }
        visible
    }

    pub(crate) fn fill_css(&self, slot: usize) -> Option<&str> {
        self.fills_by_slot
            .get(slot)
            .and_then(Option::as_ref)
            .map(|fill| fill.css.as_str())
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<GitGraphNodePaletteReceipt> {
        self.palette_key
            .as_ref()
            .map(|_| GitGraphNodePaletteReceipt::default())
    }

    pub(crate) fn record_terminal(&self, receipt: GitGraphNodePaletteReceipt) -> bool {
        self.palette_key.is_some()
            && receipt.proves(self)
            && self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        let Some(key) = self.palette_key.clone() else {
            return evidence;
        };
        match self.terminal_receipt.get() {
            Some(receipt) if !receipt.applied_capabilities.is_empty() => {
                evidence.mark_applied_with_capabilities(
                    key,
                    receipt.applied_capabilities.iter().copied(),
                );
            }
            Some(_) => evidence.mark_not_applicable(key),
            None => {
                evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedOrdinalPalette)
            }
        }
        evidence
    }

    fn capability_for_slot(&self, slot: usize) -> Option<ThemeCapability> {
        self.fills_by_slot
            .get(slot)
            .and_then(Option::as_ref)
            .map(|fill| fill.capability)
    }
}

/// Writer-owned proof that all GitGraph palette rules and visible occurrences reached terminal
/// SVG in their canonical order.
#[derive(Debug, Default)]
pub(crate) struct GitGraphNodePaletteReceipt {
    next_rule: usize,
    next_branch_label: usize,
    next_arrow: usize,
    next_commit_element: usize,
    values_match: bool,
    initialized: bool,
    applied_capabilities: BTreeSet<ThemeCapability>,
}

impl GitGraphNodePaletteReceipt {
    fn initialize(&mut self) {
        if !self.initialized {
            self.initialized = true;
            self.values_match = true;
        }
    }

    pub(crate) fn record_stylesheet_rule(
        &mut self,
        plan: &GitGraphNodePalettePlan,
        slot: usize,
        emitted_fill: &str,
    ) {
        self.initialize();
        self.values_match &= plan.expected_rule_slots.get(self.next_rule).copied() == Some(slot)
            && plan.fill_css(slot) == Some(emitted_fill);
        self.next_rule += 1;
    }

    pub(crate) fn record_branch_label(&mut self, plan: &GitGraphNodePalettePlan, slot: usize) {
        self.initialize();
        self.values_match &= plan
            .expected_branch_label_slots
            .get(self.next_branch_label)
            .copied()
            == Some(slot);
        self.record_capability(plan, slot);
        self.next_branch_label += 1;
    }

    pub(crate) fn record_arrow(&mut self, plan: &GitGraphNodePalettePlan, slot: usize) {
        self.initialize();
        self.values_match &= plan.expected_arrow_slots.get(self.next_arrow).copied() == Some(slot);
        self.record_capability(plan, slot);
        self.next_arrow += 1;
    }

    pub(crate) fn record_commit_element(
        &mut self,
        plan: &GitGraphNodePalettePlan,
        commit_index: usize,
        element_index: usize,
        slot: usize,
    ) {
        self.initialize();
        self.values_match &= plan
            .expected_commit_elements
            .get(self.next_commit_element)
            .copied()
            == Some(ExpectedCommitElement {
                commit_index,
                element_index,
                slot,
            });
        self.record_capability(plan, slot);
        self.next_commit_element += 1;
    }

    fn record_capability(&mut self, plan: &GitGraphNodePalettePlan, slot: usize) {
        if let Some(capability) = plan.capability_for_slot(slot) {
            self.applied_capabilities.insert(capability);
        }
    }

    fn proves(&self, plan: &GitGraphNodePalettePlan) -> bool {
        self.initialized
            && self.values_match
            && self.next_rule == plan.expected_rule_slots.len()
            && self.next_branch_label == plan.expected_branch_label_slots.len()
            && self.next_arrow == plan.expected_arrow_slots.len()
            && self.next_commit_element == plan.expected_commit_elements.len()
    }
}

pub(crate) fn palette_slot(index: i64) -> usize {
    index.rem_euclid(GITGRAPH_PALETTE_SLOT_COUNT as i64) as usize
}

fn palette_element_count(commit: &GitGraphCommitLayout) -> usize {
    match commit.custom_type.unwrap_or(commit.commit_type) {
        1 | 3 => 2,
        2 => 1,
        4 => 0,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt_plan() -> GitGraphNodePalettePlan {
        let solid = GitGraphNodePaletteFill {
            css: "#123456".to_string(),
            capability: ThemeCapability::SolidPaint,
        };
        let transparent = GitGraphNodePaletteFill {
            css: "transparent".to_string(),
            capability: ThemeCapability::TransparentPaint,
        };
        GitGraphNodePalettePlan {
            fills_by_slot: std::array::from_fn(|slot| match slot {
                0 => Some(GitGraphNodePaletteFill {
                    css: solid.css.clone(),
                    capability: solid.capability,
                }),
                1 => Some(GitGraphNodePaletteFill {
                    css: transparent.css.clone(),
                    capability: transparent.capability,
                }),
                _ => None,
            }),
            evidence: FamilyThemeEvidence::default(),
            palette_key: Some(FamilyThemeMechanismKey::OrdinalPalette {
                target: ThemeTarget::Node,
            }),
            expected_rule_slots: vec![0, 1],
            expected_branch_label_slots: vec![0, 1],
            expected_arrow_slots: vec![1],
            expected_commit_elements: vec![
                ExpectedCommitElement {
                    commit_index: 0,
                    element_index: 0,
                    slot: 0,
                },
                ExpectedCommitElement {
                    commit_index: 1,
                    element_index: 0,
                    slot: 1,
                },
            ],
            terminal_receipt: OnceLock::new(),
        }
    }

    #[test]
    fn terminal_receipt_requires_every_rule_and_visible_occurrence_in_order() {
        let plan = receipt_plan();
        let mut receipt = GitGraphNodePaletteReceipt::default();
        receipt.record_stylesheet_rule(&plan, 0, "#123456");
        receipt.record_stylesheet_rule(&plan, 1, "transparent");
        receipt.record_branch_label(&plan, 0);
        receipt.record_branch_label(&plan, 1);
        receipt.record_arrow(&plan, 1);
        receipt.record_commit_element(&plan, 0, 0, 0);
        receipt.record_commit_element(&plan, 1, 0, 1);

        assert!(receipt.proves(&plan));
        assert_eq!(
            receipt.applied_capabilities,
            BTreeSet::from([
                ThemeCapability::SolidPaint,
                ThemeCapability::TransparentPaint,
            ])
        );
    }

    #[test]
    fn terminal_receipt_rejects_missing_duplicate_and_mismatched_checkpoints() {
        let plan = receipt_plan();

        let mut missing = GitGraphNodePaletteReceipt::default();
        missing.record_stylesheet_rule(&plan, 0, "#123456");
        assert!(!missing.proves(&plan));

        let mut duplicate = GitGraphNodePaletteReceipt::default();
        duplicate.record_stylesheet_rule(&plan, 0, "#123456");
        duplicate.record_stylesheet_rule(&plan, 0, "#123456");
        assert!(!duplicate.proves(&plan));

        let mut mismatch = GitGraphNodePaletteReceipt::default();
        mismatch.record_stylesheet_rule(&plan, 0, "#abcdef");
        mismatch.record_stylesheet_rule(&plan, 1, "transparent");
        mismatch.record_branch_label(&plan, 0);
        mismatch.record_branch_label(&plan, 1);
        mismatch.record_arrow(&plan, 1);
        mismatch.record_commit_element(&plan, 0, 0, 0);
        mismatch.record_commit_element(&plan, 1, 0, 1);
        assert!(!mismatch.proves(&plan));
    }

    #[test]
    fn palette_slots_follow_the_eight_slot_visible_ring() {
        assert_eq!(palette_slot(0), 0);
        assert_eq!(palette_slot(7), 7);
        assert_eq!(palette_slot(8), 0);
        assert_eq!(palette_slot(15), 7);
    }
}
