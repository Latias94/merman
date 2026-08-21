use std::collections::{BTreeSet, HashMap};
use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanismKey, ResolvedDiagramTheme, ThemeCapability,
    ThemeTarget, ThemeVariant,
};
use crate::family::{
    FamilyThemeEvidence, FamilyThemeResidualReason, TerminalVariantDomain,
    UnsupportedTerminalDomain, reconcile_unsupported_terminal_domains,
};
use crate::model::{GitGraphCommitLayout, GitGraphDiagramLayout};
use crate::resources::{OperationWorkError, OperationWorkMeter};

mod branch_stroke;

use branch_stroke::GitGraphBranchStrokePlan;
pub(crate) use branch_stroke::GitGraphBranchStrokeReceipt;

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

const NODE_BORDER_PATH: &str = "themeVariables.nodeBorder";
const MAIN_BACKGROUND_PATH: &str = "themeVariables.mainBkg";
const PRIMARY_COLOR_PATH: &str = "themeVariables.primaryColor";
const BORDER_COLOR_ARRAY_PATH: &str = "themeVariables.borderColorArray";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GitGraphPaletteSurface {
    Commit,
    Arrow,
    BranchLabelBackground,
}

impl GitGraphPaletteSurface {
    pub(crate) const ALL: [Self; 3] = [Self::Commit, Self::Arrow, Self::BranchLabelBackground];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GitGraphPaletteSource {
    Git(usize),
    NodeBorder,
    MainBackground,
    PrimaryColor,
    BorderColorArray,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GitGraphCommitPaletteRole {
    BranchSlot,
    State,
}

const NORMAL_COMMIT_PALETTE_ROLES: [GitGraphCommitPaletteRole; 1] =
    [GitGraphCommitPaletteRole::BranchSlot];
const REVERSE_COMMIT_PALETTE_ROLES: [GitGraphCommitPaletteRole; 2] = [
    GitGraphCommitPaletteRole::BranchSlot,
    GitGraphCommitPaletteRole::State,
];
const HIGHLIGHT_COMMIT_PALETTE_ROLES: [GitGraphCommitPaletteRole; 1] =
    [GitGraphCommitPaletteRole::State];
const MERGE_COMMIT_PALETTE_ROLES: [GitGraphCommitPaletteRole; 2] = [
    GitGraphCommitPaletteRole::BranchSlot,
    GitGraphCommitPaletteRole::State,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GitGraphCommitKind {
    Normal,
    Reverse,
    Highlight,
    Merge,
    CherryPick,
}

impl GitGraphCommitKind {
    pub(crate) fn from_layout(commit: &GitGraphCommitLayout) -> Self {
        match commit.custom_type.unwrap_or(commit.commit_type) {
            1 => Self::Reverse,
            2 => Self::Highlight,
            3 => Self::Merge,
            4 => Self::CherryPick,
            _ => Self::Normal,
        }
    }

    pub(crate) const fn class_name(self) -> &'static str {
        match self {
            Self::Normal => "commit-normal",
            Self::Reverse => "commit-reverse",
            Self::Highlight => "commit-highlight",
            Self::Merge => "commit-merge",
            Self::CherryPick => "commit-cherry-pick",
        }
    }

    pub(crate) const fn palette_element_roles(self) -> &'static [GitGraphCommitPaletteRole] {
        match self {
            Self::Normal => &NORMAL_COMMIT_PALETTE_ROLES,
            Self::Reverse => &REVERSE_COMMIT_PALETTE_ROLES,
            Self::Highlight => &HIGHLIGHT_COMMIT_PALETTE_ROLES,
            Self::Merge => &MERGE_COMMIT_PALETTE_ROLES,
            Self::CherryPick => &[],
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct GitGraphPaletteSurfaceSet {
    commit: bool,
    arrow: bool,
    branch_label_background: bool,
}

impl GitGraphPaletteSurfaceSet {
    fn insert(&mut self, surface: GitGraphPaletteSurface) {
        match surface {
            GitGraphPaletteSurface::Commit => self.commit = true,
            GitGraphPaletteSurface::Arrow => self.arrow = true,
            GitGraphPaletteSurface::BranchLabelBackground => self.branch_label_background = true,
        }
    }

    fn contains(self, surface: GitGraphPaletteSurface) -> bool {
        match surface {
            GitGraphPaletteSurface::Commit => self.commit,
            GitGraphPaletteSurface::Arrow => self.arrow,
            GitGraphPaletteSurface::BranchLabelBackground => self.branch_label_background,
        }
    }

    fn any(self) -> bool {
        self.commit || self.arrow || self.branch_label_background
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct GitGraphPaletteSurfaceOwnership {
    fill_owned_by_slot: [GitGraphPaletteSurfaceSet; GITGRAPH_PALETTE_SLOT_COUNT],
    stroke_owned_by_slot: [GitGraphPaletteSurfaceSet; GITGRAPH_PALETTE_SLOT_COUNT],
}

impl Default for GitGraphPaletteSurfaceOwnership {
    fn default() -> Self {
        Self {
            fill_owned_by_slot: [GitGraphPaletteSurfaceSet::default(); GITGRAPH_PALETTE_SLOT_COUNT],
            stroke_owned_by_slot: [GitGraphPaletteSurfaceSet::default();
                GITGRAPH_PALETTE_SLOT_COUNT],
        }
    }
}

impl GitGraphPaletteSurfaceOwnership {
    pub(crate) fn mark_fill_owned(&mut self, surface: GitGraphPaletteSurface, slot: usize) {
        if let Some(owned) = self.fill_owned_by_slot.get_mut(slot) {
            owned.insert(surface);
        }
    }

    pub(crate) fn mark_stroke_owned(&mut self, surface: GitGraphPaletteSurface, slot: usize) {
        if let Some(owned) = self.stroke_owned_by_slot.get_mut(slot) {
            owned.insert(surface);
        }
    }

    fn owns_fill(self, surface: GitGraphPaletteSurface, slot: usize) -> bool {
        self.fill_owned_by_slot
            .get(slot)
            .is_some_and(|owned| owned.contains(surface))
    }

    fn owns_stroke(self, surface: GitGraphPaletteSurface, slot: usize) -> bool {
        self.stroke_owned_by_slot
            .get(slot)
            .is_some_and(|owned| owned.contains(surface))
    }
}

#[derive(Debug, Clone, Copy)]
struct GitGraphPaletteSourceOwnership {
    git: [bool; GITGRAPH_PALETTE_SLOT_COUNT],
    node_border: bool,
    main_background: bool,
    primary_color: bool,
    border_color_array: bool,
}

impl Default for GitGraphPaletteSourceOwnership {
    fn default() -> Self {
        Self {
            git: [false; GITGRAPH_PALETTE_SLOT_COUNT],
            node_border: false,
            main_background: false,
            primary_color: false,
            border_color_array: false,
        }
    }
}

impl GitGraphPaletteSourceOwnership {
    fn from_config(config: &MermaidConfig) -> Self {
        Self {
            git: std::array::from_fn(|slot| {
                merman_core::__private::config_path_overrides_typed_default(
                    config,
                    GIT_COLOR_PATHS[slot],
                )
            }),
            node_border: merman_core::__private::config_path_overrides_typed_default(
                config,
                NODE_BORDER_PATH,
            ),
            main_background: merman_core::__private::config_path_overrides_typed_default(
                config,
                MAIN_BACKGROUND_PATH,
            ),
            primary_color: merman_core::__private::config_path_overrides_typed_default(
                config,
                PRIMARY_COLOR_PATH,
            ),
            border_color_array: merman_core::__private::config_path_overrides_typed_default(
                config,
                BORDER_COLOR_ARRAY_PATH,
            ),
        }
    }

    fn owns(self, source: GitGraphPaletteSource) -> bool {
        match source {
            GitGraphPaletteSource::Git(slot) => self.git.get(slot).copied().unwrap_or(false),
            GitGraphPaletteSource::NodeBorder => self.node_border,
            GitGraphPaletteSource::MainBackground => self.main_background,
            GitGraphPaletteSource::PrimaryColor => self.primary_color,
            GitGraphPaletteSource::BorderColorArray => self.border_color_array,
        }
    }
}

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
    role: GitGraphCommitPaletteRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExpectedStylesheetRule {
    surface: GitGraphPaletteSurface,
    slot: usize,
}

/// Family-local projection of the direct GitGraph theme routes.
///
/// The plan owns only the eight visible palette slots consumed by commit shapes, arrows, and branch
/// label backgrounds plus the static Edge stroke consumed by visible branch lines. The terminal
/// writer resolves the actual Mermaid source for each surface and suppresses a direct value when
/// explicit source or site configuration owns that source. Mermaid keeps ownership of `gitInvN`,
/// `gitBranchLabelN`, text colors, and every unsupported Edge selector or paint kind. The terminal
/// writer must provide a complete occurrence receipt before either route can become Applied
/// evidence. The historical type name remains the central artifact seam while the family-local
/// implementation deepens behind it.
#[derive(Debug)]
pub(crate) struct GitGraphNodePalettePlan {
    fills_by_slot: [Option<GitGraphNodePaletteFill>; GITGRAPH_PALETTE_SLOT_COUNT],
    fill_winner_by_slot: [bool; GITGRAPH_PALETTE_SLOT_COUNT],
    branch_stroke: GitGraphBranchStrokePlan,
    evidence: FamilyThemeEvidence,
    palette_key: Option<FamilyThemeMechanismKey>,
    visible_surfaces: [GitGraphPaletteSurfaceSet; GITGRAPH_PALETTE_SLOT_COUNT],
    mermaid_source_ownership: GitGraphPaletteSourceOwnership,
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
        has_title: bool,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let mut plan = Self::baseline(layout);
        plan.mermaid_source_ownership =
            GitGraphPaletteSourceOwnership::from_config(effective_config);
        let Some(theme) = theme else {
            return Ok(plan);
        };

        plan.evidence = FamilyThemeEvidence::from_theme(Some(theme));
        let absent = TerminalVariantDomain::uniform(0, ThemeVariant::Default);
        reconcile_unsupported_terminal_domains(
            theme,
            &mut plan.evidence,
            &[
                UnsupportedTerminalDomain::direct(
                    ThemeTarget::Title,
                    TerminalVariantDomain::uniform(usize::from(has_title), ThemeVariant::Default),
                ),
                UnsupportedTerminalDomain::direct(ThemeTarget::Marker, absent),
                UnsupportedTerminalDomain::direct(ThemeTarget::Cluster, absent),
                UnsupportedTerminalDomain::direct(ThemeTarget::ClusterLabel, absent),
            ],
            work_meter,
        )?;
        plan.resolve_node_palette(theme, work_meter)?;
        plan.branch_stroke = GitGraphBranchStrokePlan::resolve(
            theme,
            effective_config,
            if layout.show_branches {
                layout.branches.len()
            } else {
                0
            },
            work_meter,
            &mut plan.evidence,
        )?;
        Ok(plan)
    }

    fn resolve_node_palette(
        &mut self,
        theme: &ResolvedDiagramTheme,
        work_meter: &OperationWorkMeter,
    ) -> Result<(), OperationWorkError> {
        let Some(disposition) = theme.ordinal_palette_disposition(ThemeTarget::Node) else {
            return Ok(());
        };
        let key = FamilyThemeMechanismKey::OrdinalPalette {
            target: ThemeTarget::Node,
        };
        if !self.has_visible_surface() {
            self.evidence.mark_not_applicable(key);
            return Ok(());
        }

        match disposition {
            FamilyThemeDisposition::TypedAdapter => {
                for slot in 0..GITGRAPH_PALETTE_SLOT_COUNT {
                    if !self.visible_surfaces[slot].any() {
                        continue;
                    }

                    let style = theme.style_with_work_meter(
                        ThemeTarget::Node,
                        ThemeVariant::Default,
                        Some(slot + 1),
                        work_meter,
                    )?;
                    self.fill_winner_by_slot[slot] = style.fill_resolution().winner().is_some();
                    if !self.slot_has_palette_terminal(slot) {
                        continue;
                    }

                    let Some(color) = theme.series_color(ThemeTarget::Node, slot + 1) else {
                        self.evidence.mark_residual(
                            key,
                            FamilyThemeResidualReason::UnsupportedOrdinalPalette,
                        );
                        return Ok(());
                    };
                    self.fills_by_slot[slot] = Some(GitGraphNodePaletteFill {
                        css: color.as_css(),
                        capability: if color.is_transparent() {
                            ThemeCapability::TransparentPaint
                        } else {
                            ThemeCapability::SolidPaint
                        },
                    });
                }

                if self.fills_by_slot.iter().all(Option::is_none) {
                    self.evidence.mark_not_applicable(key);
                } else {
                    self.palette_key = Some(key);
                }
            }
            FamilyThemeDisposition::Unsupported => self
                .evidence
                .mark_residual(key, FamilyThemeResidualReason::UnsupportedOrdinalPalette),
            FamilyThemeDisposition::LegacyCompatibility => {}
        }

        Ok(())
    }

    pub(crate) fn baseline(layout: &GitGraphDiagramLayout) -> Self {
        let mut visible_surfaces =
            [GitGraphPaletteSurfaceSet::default(); GITGRAPH_PALETTE_SLOT_COUNT];
        let expected_branch_label_slots = if layout.show_branches {
            layout
                .branches
                .iter()
                .map(|branch| {
                    let slot = palette_slot(branch.index);
                    visible_surfaces[slot].insert(GitGraphPaletteSurface::BranchLabelBackground);
                    slot
                })
                .collect()
        } else {
            Vec::new()
        };
        let expected_arrow_slots = layout
            .arrows
            .iter()
            .map(|arrow| {
                let slot = palette_slot(arrow.class_index);
                visible_surfaces[slot].insert(GitGraphPaletteSurface::Arrow);
                slot
            })
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
            for (element_index, role) in GitGraphCommitKind::from_layout(commit)
                .palette_element_roles()
                .iter()
                .copied()
                .enumerate()
            {
                visible_surfaces[slot].insert(GitGraphPaletteSurface::Commit);
                expected_commit_elements.push(ExpectedCommitElement {
                    commit_index,
                    element_index,
                    slot,
                    role,
                });
            }
        }

        Self {
            fills_by_slot: std::array::from_fn(|_| None),
            fill_winner_by_slot: [false; GITGRAPH_PALETTE_SLOT_COUNT],
            branch_stroke: GitGraphBranchStrokePlan::baseline(if layout.show_branches {
                layout.branches.len()
            } else {
                0
            }),
            evidence: FamilyThemeEvidence::default(),
            palette_key: None,
            visible_surfaces,
            mermaid_source_ownership: GitGraphPaletteSourceOwnership::default(),
            expected_branch_label_slots,
            expected_arrow_slots,
            expected_commit_elements,
            terminal_receipt: OnceLock::new(),
        }
    }

    fn has_visible_surface(&self) -> bool {
        self.visible_surfaces
            .iter()
            .copied()
            .any(GitGraphPaletteSurfaceSet::any)
    }

    pub(crate) fn surface_is_visible(&self, surface: GitGraphPaletteSurface, slot: usize) -> bool {
        self.visible_surfaces
            .get(slot)
            .is_some_and(|visible| visible.contains(surface))
    }

    pub(crate) fn terminal_fill_css(
        &self,
        surface: GitGraphPaletteSurface,
        slot: usize,
        ownership: GitGraphPaletteSurfaceOwnership,
    ) -> Option<&str> {
        if !self.surface_is_visible(surface, slot)
            || ownership.owns_fill(surface, slot)
            || surface == GitGraphPaletteSurface::Arrow
            || self.fill_winner_by_slot.get(slot).copied().unwrap_or(false)
        {
            return None;
        }
        self.fills_by_slot
            .get(slot)
            .and_then(Option::as_ref)
            .map(|fill| fill.css.as_str())
    }

    pub(crate) fn terminal_stroke_css(
        &self,
        surface: GitGraphPaletteSurface,
        slot: usize,
        ownership: GitGraphPaletteSurfaceOwnership,
    ) -> Option<&str> {
        if !self.surface_is_visible(surface, slot)
            || ownership.owns_stroke(surface, slot)
            || surface == GitGraphPaletteSurface::BranchLabelBackground
        {
            return None;
        }
        self.fills_by_slot
            .get(slot)
            .and_then(Option::as_ref)
            .map(|fill| fill.css.as_str())
    }

    pub(crate) fn terminal_branch_stroke_css(&self) -> Option<&str> {
        self.branch_stroke.terminal_css()
    }

    pub(crate) fn mermaid_source_is_owned(&self, source: GitGraphPaletteSource) -> bool {
        self.mermaid_source_ownership.owns(source)
    }

    pub(crate) fn commit_palette_elements(
        &self,
    ) -> impl Iterator<Item = (usize, GitGraphCommitPaletteRole)> + '_ {
        self.expected_commit_elements
            .iter()
            .map(|element| (element.slot, element.role))
    }

    pub(crate) fn begin_terminal_receipt(
        &self,
        ownership: GitGraphPaletteSurfaceOwnership,
    ) -> Option<GitGraphNodePaletteReceipt> {
        self.palette_key
            .as_ref()
            .map(|_| GitGraphNodePaletteReceipt::new(self, ownership))
    }

    pub(crate) fn record_terminal(&self, receipt: GitGraphNodePaletteReceipt) -> bool {
        self.palette_key.is_some()
            && receipt.proves(self)
            && self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn begin_branch_stroke_receipt(&self) -> Option<GitGraphBranchStrokeReceipt> {
        self.branch_stroke.begin_terminal_receipt()
    }

    pub(crate) fn record_branch_line(
        &self,
        receipt: &mut Option<GitGraphBranchStrokeReceipt>,
        emitted_class: &str,
        emitted_style: Option<&str>,
    ) {
        if let Some(receipt) = receipt.as_mut() {
            receipt.record_branch_line(emitted_class, emitted_style);
        }
    }

    pub(crate) fn record_branch_stroke_terminal(
        &self,
        receipt: GitGraphBranchStrokeReceipt,
    ) -> bool {
        self.branch_stroke.record_terminal(receipt)
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        if let Some(key) = self.palette_key.clone() {
            match self.terminal_receipt.get() {
                Some(receipt) if !receipt.applied_capabilities.is_empty() => {
                    evidence.mark_applied_with_capabilities(
                        key,
                        receipt.applied_capabilities.iter().copied(),
                    );
                }
                Some(_) => evidence.mark_not_applicable(key),
                None => evidence
                    .mark_residual(key, FamilyThemeResidualReason::UnsupportedOrdinalPalette),
            }
        }
        self.branch_stroke.finish_evidence(&mut evidence);
        evidence
    }

    fn capability_for_surface(
        &self,
        surface: GitGraphPaletteSurface,
        slot: usize,
        ownership: GitGraphPaletteSurfaceOwnership,
    ) -> Option<ThemeCapability> {
        if self.terminal_fill_css(surface, slot, ownership).is_none()
            && self.terminal_stroke_css(surface, slot, ownership).is_none()
        {
            return None;
        }
        self.fills_by_slot
            .get(slot)
            .and_then(Option::as_ref)
            .map(|fill| fill.capability)
    }

    fn slot_has_palette_terminal(&self, slot: usize) -> bool {
        let Some(visible) = self.visible_surfaces.get(slot).copied() else {
            return false;
        };
        let fill_available = !self.fill_winner_by_slot.get(slot).copied().unwrap_or(false)
            && (visible.contains(GitGraphPaletteSurface::Commit)
                || visible.contains(GitGraphPaletteSurface::BranchLabelBackground));
        let stroke_available = visible.contains(GitGraphPaletteSurface::Commit)
            || visible.contains(GitGraphPaletteSurface::Arrow);
        fill_available || stroke_available
    }
}

/// Writer-owned proof that all GitGraph palette rules and visible occurrences reached terminal
/// SVG in their canonical order.
#[derive(Debug)]
pub(crate) struct GitGraphNodePaletteReceipt {
    ownership: GitGraphPaletteSurfaceOwnership,
    expected_rules: Vec<ExpectedStylesheetRule>,
    next_rule: usize,
    next_branch_label: usize,
    next_arrow: usize,
    next_commit_element: usize,
    values_match: bool,
    initialized: bool,
    applied_capabilities: BTreeSet<ThemeCapability>,
}

impl GitGraphNodePaletteReceipt {
    fn new(plan: &GitGraphNodePalettePlan, ownership: GitGraphPaletteSurfaceOwnership) -> Self {
        let expected_rules = (0..GITGRAPH_PALETTE_SLOT_COUNT)
            .flat_map(|slot| {
                GitGraphPaletteSurface::ALL
                    .into_iter()
                    .filter_map(move |surface| {
                        (plan.terminal_fill_css(surface, slot, ownership).is_some()
                            || plan.terminal_stroke_css(surface, slot, ownership).is_some())
                        .then_some(ExpectedStylesheetRule { surface, slot })
                    })
            })
            .collect();
        Self {
            ownership,
            expected_rules,
            next_rule: 0,
            next_branch_label: 0,
            next_arrow: 0,
            next_commit_element: 0,
            values_match: false,
            initialized: false,
            applied_capabilities: BTreeSet::new(),
        }
    }

    fn initialize(&mut self) {
        if !self.initialized {
            self.initialized = true;
            self.values_match = true;
        }
    }

    pub(crate) fn record_stylesheet_rule(
        &mut self,
        plan: &GitGraphNodePalettePlan,
        surface: GitGraphPaletteSurface,
        slot: usize,
        emitted_fill: Option<&str>,
        emitted_stroke: Option<&str>,
    ) {
        self.initialize();
        self.values_match &= self.expected_rules.get(self.next_rule).copied()
            == Some(ExpectedStylesheetRule { surface, slot })
            && plan.terminal_fill_css(surface, slot, self.ownership) == emitted_fill
            && plan.terminal_stroke_css(surface, slot, self.ownership) == emitted_stroke;
        self.next_rule += 1;
    }

    pub(crate) fn record_branch_label(&mut self, plan: &GitGraphNodePalettePlan, slot: usize) {
        self.initialize();
        self.values_match &= plan
            .expected_branch_label_slots
            .get(self.next_branch_label)
            .copied()
            == Some(slot);
        self.record_capability(plan, GitGraphPaletteSurface::BranchLabelBackground, slot);
        self.next_branch_label += 1;
    }

    pub(crate) fn record_arrow(&mut self, plan: &GitGraphNodePalettePlan, slot: usize) {
        self.initialize();
        self.values_match &= plan.expected_arrow_slots.get(self.next_arrow).copied() == Some(slot);
        self.record_capability(plan, GitGraphPaletteSurface::Arrow, slot);
        self.next_arrow += 1;
    }

    pub(crate) fn record_commit_element(
        &mut self,
        plan: &GitGraphNodePalettePlan,
        commit_index: usize,
        element_index: usize,
        slot: usize,
        role: GitGraphCommitPaletteRole,
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
                role,
            });
        self.record_capability(plan, GitGraphPaletteSurface::Commit, slot);
        self.next_commit_element += 1;
    }

    fn record_capability(
        &mut self,
        plan: &GitGraphNodePalettePlan,
        surface: GitGraphPaletteSurface,
        slot: usize,
    ) {
        if let Some(capability) = plan.capability_for_surface(surface, slot, self.ownership) {
            self.applied_capabilities.insert(capability);
        }
    }

    fn proves(&self, plan: &GitGraphNodePalettePlan) -> bool {
        self.initialized
            && self.values_match
            && self.next_rule == self.expected_rules.len()
            && self.next_branch_label == plan.expected_branch_label_slots.len()
            && self.next_arrow == plan.expected_arrow_slots.len()
            && self.next_commit_element == plan.expected_commit_elements.len()
    }
}

pub(crate) fn palette_slot(index: i64) -> usize {
    index.rem_euclid(GITGRAPH_PALETTE_SLOT_COUNT as i64) as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DiagramFamilyId;
    use crate::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, ThemeRule, ThemeRuleSet,
        ThemeStylePatch,
    };
    use crate::resources::RenderResourcePolicy;

    fn empty_layout() -> GitGraphDiagramLayout {
        GitGraphDiagramLayout {
            bounds: None,
            direction: "LR".to_string(),
            rotate_commit_label: false,
            show_branches: false,
            show_commit_label: false,
            parallel_commits: false,
            diagram_padding: 0.0,
            max_pos: 0.0,
            branches: Vec::new(),
            commits: Vec::new(),
            arrows: Vec::new(),
        }
    }

    fn unsupported_fill_theme(target: ThemeTarget) -> ResolvedDiagramTheme {
        let styles = ThemeRuleSet::default().with_rule(
            ThemeRule::new(
                target,
                ThemeStylePatch::default().with_fill(CanvasPaint::solid("#123456").unwrap()),
            )
            .for_family(DiagramFamilyId::GIT_GRAPH),
        );
        DiagramThemeCompiler::new()
            .compile(DiagramThemeSpec::new().with_styles(styles))
            .expect("compile GitGraph terminal fixture")
            .resolve(DiagramFamilyId::GIT_GRAPH)
    }

    fn work_meter() -> OperationWorkMeter {
        OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input())
    }

    fn receipt_plan() -> GitGraphNodePalettePlan {
        let solid = GitGraphNodePaletteFill {
            css: "#123456".to_string(),
            capability: ThemeCapability::SolidPaint,
        };
        let transparent = GitGraphNodePaletteFill {
            css: "transparent".to_string(),
            capability: ThemeCapability::TransparentPaint,
        };
        let mut visible_surfaces =
            [GitGraphPaletteSurfaceSet::default(); GITGRAPH_PALETTE_SLOT_COUNT];
        visible_surfaces[0].insert(GitGraphPaletteSurface::Commit);
        visible_surfaces[0].insert(GitGraphPaletteSurface::BranchLabelBackground);
        visible_surfaces[1].insert(GitGraphPaletteSurface::Commit);
        visible_surfaces[1].insert(GitGraphPaletteSurface::Arrow);
        visible_surfaces[1].insert(GitGraphPaletteSurface::BranchLabelBackground);
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
            fill_winner_by_slot: [false; GITGRAPH_PALETTE_SLOT_COUNT],
            branch_stroke: GitGraphBranchStrokePlan::baseline(0),
            evidence: FamilyThemeEvidence::default(),
            palette_key: Some(FamilyThemeMechanismKey::OrdinalPalette {
                target: ThemeTarget::Node,
            }),
            visible_surfaces,
            mermaid_source_ownership: GitGraphPaletteSourceOwnership::default(),
            expected_branch_label_slots: vec![0, 1],
            expected_arrow_slots: vec![1],
            expected_commit_elements: vec![
                ExpectedCommitElement {
                    commit_index: 0,
                    element_index: 0,
                    slot: 0,
                    role: GitGraphCommitPaletteRole::BranchSlot,
                },
                ExpectedCommitElement {
                    commit_index: 1,
                    element_index: 0,
                    slot: 1,
                    role: GitGraphCommitPaletteRole::BranchSlot,
                },
            ],
            terminal_receipt: OnceLock::new(),
        }
    }

    #[test]
    fn terminal_receipt_requires_every_rule_and_visible_occurrence_in_order() {
        let plan = receipt_plan();
        let mut receipt =
            GitGraphNodePaletteReceipt::new(&plan, GitGraphPaletteSurfaceOwnership::default());
        receipt.record_stylesheet_rule(
            &plan,
            GitGraphPaletteSurface::Commit,
            0,
            Some("#123456"),
            Some("#123456"),
        );
        receipt.record_stylesheet_rule(
            &plan,
            GitGraphPaletteSurface::BranchLabelBackground,
            0,
            Some("#123456"),
            None,
        );
        receipt.record_stylesheet_rule(
            &plan,
            GitGraphPaletteSurface::Commit,
            1,
            Some("transparent"),
            Some("transparent"),
        );
        receipt.record_stylesheet_rule(
            &plan,
            GitGraphPaletteSurface::Arrow,
            1,
            None,
            Some("transparent"),
        );
        receipt.record_stylesheet_rule(
            &plan,
            GitGraphPaletteSurface::BranchLabelBackground,
            1,
            Some("transparent"),
            None,
        );
        receipt.record_branch_label(&plan, 0);
        receipt.record_branch_label(&plan, 1);
        receipt.record_arrow(&plan, 1);
        receipt.record_commit_element(&plan, 0, 0, 0, GitGraphCommitPaletteRole::BranchSlot);
        receipt.record_commit_element(&plan, 1, 0, 1, GitGraphCommitPaletteRole::BranchSlot);

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
    fn absent_gitgraph_marker_is_not_applicable() {
        let theme = unsupported_fill_theme(ThemeTarget::Marker);
        let evidence = GitGraphNodePalettePlan::resolve(
            Some(&theme),
            &MermaidConfig::default(),
            &empty_layout(),
            false,
            &work_meter(),
        )
        .expect("resolve absent GitGraph marker evidence")
        .finish_evidence();

        assert_eq!(
            evidence.not_applicable_mechanisms(),
            &[FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Marker,
            }]
        );
        assert!(evidence.residuals().is_empty());
    }

    #[test]
    fn visible_gitgraph_title_keeps_unsupported_fill_as_a_residual() {
        let theme = unsupported_fill_theme(ThemeTarget::Title);
        let evidence = GitGraphNodePalettePlan::resolve(
            Some(&theme),
            &MermaidConfig::default(),
            &empty_layout(),
            true,
            &work_meter(),
        )
        .expect("resolve visible GitGraph title evidence")
        .finish_evidence();

        assert_eq!(evidence.residuals().len(), 1);
        assert_eq!(
            evidence.residuals()[0].key(),
            &FamilyThemeMechanismKey::Rule {
                index: 0,
                target: ThemeTarget::Title,
            }
        );
        assert!(evidence.not_applicable_mechanisms().is_empty());
    }

    #[test]
    fn terminal_receipt_rejects_missing_duplicate_and_mismatched_checkpoints() {
        let plan = receipt_plan();

        let mut missing =
            GitGraphNodePaletteReceipt::new(&plan, GitGraphPaletteSurfaceOwnership::default());
        missing.record_stylesheet_rule(
            &plan,
            GitGraphPaletteSurface::Commit,
            0,
            Some("#123456"),
            Some("#123456"),
        );
        assert!(!missing.proves(&plan));

        let mut duplicate =
            GitGraphNodePaletteReceipt::new(&plan, GitGraphPaletteSurfaceOwnership::default());
        duplicate.record_stylesheet_rule(
            &plan,
            GitGraphPaletteSurface::Commit,
            0,
            Some("#123456"),
            Some("#123456"),
        );
        duplicate.record_stylesheet_rule(
            &plan,
            GitGraphPaletteSurface::Commit,
            0,
            Some("#123456"),
            Some("#123456"),
        );
        assert!(!duplicate.proves(&plan));

        let mut mismatch =
            GitGraphNodePaletteReceipt::new(&plan, GitGraphPaletteSurfaceOwnership::default());
        mismatch.record_stylesheet_rule(
            &plan,
            GitGraphPaletteSurface::Commit,
            0,
            Some("#abcdef"),
            Some("#123456"),
        );
        mismatch.record_stylesheet_rule(
            &plan,
            GitGraphPaletteSurface::BranchLabelBackground,
            0,
            Some("#123456"),
            None,
        );
        mismatch.record_stylesheet_rule(
            &plan,
            GitGraphPaletteSurface::Commit,
            1,
            Some("transparent"),
            Some("transparent"),
        );
        mismatch.record_stylesheet_rule(
            &plan,
            GitGraphPaletteSurface::Arrow,
            1,
            None,
            Some("transparent"),
        );
        mismatch.record_stylesheet_rule(
            &plan,
            GitGraphPaletteSurface::BranchLabelBackground,
            1,
            Some("transparent"),
            None,
        );
        mismatch.record_branch_label(&plan, 0);
        mismatch.record_branch_label(&plan, 1);
        mismatch.record_arrow(&plan, 1);
        mismatch.record_commit_element(&plan, 0, 0, 0, GitGraphCommitPaletteRole::BranchSlot);
        mismatch.record_commit_element(&plan, 1, 0, 1, GitGraphCommitPaletteRole::BranchSlot);
        assert!(!mismatch.proves(&plan));
    }

    #[test]
    fn terminal_receipt_rejects_wrong_fill_or_stroke_for_a_partially_owned_surface() {
        let mut plan = receipt_plan();
        plan.fills_by_slot[1] = None;
        plan.visible_surfaces[1] = GitGraphPaletteSurfaceSet::default();
        plan.visible_surfaces[0].insert(GitGraphPaletteSurface::Arrow);
        plan.expected_branch_label_slots = vec![0];
        plan.expected_arrow_slots = vec![0];
        plan.expected_commit_elements.truncate(1);

        let mut ownership = GitGraphPaletteSurfaceOwnership::default();
        ownership.mark_stroke_owned(GitGraphPaletteSurface::Commit, 0);
        ownership.mark_stroke_owned(GitGraphPaletteSurface::Arrow, 0);

        let record_occurrences = |receipt: &mut GitGraphNodePaletteReceipt| {
            receipt.record_branch_label(&plan, 0);
            receipt.record_arrow(&plan, 0);
            receipt.record_commit_element(&plan, 0, 0, 0, GitGraphCommitPaletteRole::BranchSlot);
        };

        let mut correct = GitGraphNodePaletteReceipt::new(&plan, ownership);
        correct.record_stylesheet_rule(
            &plan,
            GitGraphPaletteSurface::Commit,
            0,
            Some("#123456"),
            None,
        );
        correct.record_stylesheet_rule(
            &plan,
            GitGraphPaletteSurface::BranchLabelBackground,
            0,
            Some("#123456"),
            None,
        );
        record_occurrences(&mut correct);
        assert!(correct.proves(&plan));

        let mut wrong_fill = GitGraphNodePaletteReceipt::new(&plan, ownership);
        wrong_fill.record_stylesheet_rule(
            &plan,
            GitGraphPaletteSurface::Commit,
            0,
            Some("#abcdef"),
            None,
        );
        wrong_fill.record_stylesheet_rule(
            &plan,
            GitGraphPaletteSurface::BranchLabelBackground,
            0,
            Some("#123456"),
            None,
        );
        record_occurrences(&mut wrong_fill);
        assert!(!wrong_fill.proves(&plan));

        let mut wrong_stroke = GitGraphNodePaletteReceipt::new(&plan, ownership);
        wrong_stroke.record_stylesheet_rule(
            &plan,
            GitGraphPaletteSurface::Commit,
            0,
            Some("#123456"),
            Some("#123456"),
        );
        wrong_stroke.record_stylesheet_rule(
            &plan,
            GitGraphPaletteSurface::BranchLabelBackground,
            0,
            Some("#123456"),
            None,
        );
        record_occurrences(&mut wrong_stroke);
        assert!(!wrong_stroke.proves(&plan));
    }

    #[test]
    fn palette_slots_follow_the_eight_slot_visible_ring() {
        assert_eq!(palette_slot(0), 0);
        assert_eq!(palette_slot(7), 7);
        assert_eq!(palette_slot(8), 0);
        assert_eq!(palette_slot(15), 7);
    }
}
