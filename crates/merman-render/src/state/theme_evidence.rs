use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use crate::diagram_theme::{FamilyThemeMechanismKey, ThemeCapability};
use crate::family::{FamilyThemeEvidence, FamilyThemeResidualReason};

/// The visibility switches relevant to an actual State styling decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum StateThemeSurface {
    Title,
    Node,
    Edge,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct StateAppliedThemeOutcome {
    pub(crate) capabilities: BTreeSet<ThemeCapability>,
    pub(crate) occurrences: BTreeSet<StateThemeSurface>,
}

/// Selected styles are shared by layout and the writer. Only output visibility remains deferred.
///
/// There is no independent identity or signature ledger: successful emission uses those same
/// prepared styles, and debug filtering is the only writer choice that can hide their surfaces.
#[derive(Debug, Clone)]
pub(crate) struct StateThemeOutcomes {
    selected: BTreeMap<FamilyThemeMechanismKey, StateAppliedThemeOutcome>,
    visibility: OnceLock<(bool, bool)>,
}

impl StateThemeOutcomes {
    pub(crate) fn new(
        selected: BTreeMap<FamilyThemeMechanismKey, StateAppliedThemeOutcome>,
    ) -> Self {
        Self {
            selected,
            visibility: OnceLock::new(),
        }
    }

    pub(crate) fn record_output_visibility(&self, include_nodes: bool, include_edges: bool) {
        if !self.selected.is_empty() {
            let _ = self.visibility.set((include_nodes, include_edges));
        }
    }

    pub(crate) fn record_theme_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        let Some(&(include_nodes, include_edges)) = self.visibility.get() else {
            return;
        };
        for (key, outcome) in &self.selected {
            let filtered = outcome.occurrences.iter().any(|surface| match surface {
                StateThemeSurface::Title => false,
                StateThemeSurface::Node => !include_nodes,
                StateThemeSurface::Edge => !include_edges,
            });
            if filtered {
                evidence.mark_residual(
                    key.clone(),
                    FamilyThemeResidualReason::OutputVisibilityFiltered,
                );
            } else {
                evidence.mark_applied_with_capabilities(
                    key.clone(),
                    outcome.capabilities.iter().copied(),
                );
            }
        }
    }
}
