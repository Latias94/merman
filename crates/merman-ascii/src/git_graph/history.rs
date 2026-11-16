use crate::error::{AsciiError, Result};
use crate::resource::ResourceContext;
use crate::safe_text::{BudgetedTextLine, charge_text_layout};
use merman_model::families::git_graph::GitGraphCommit;
use std::collections::{HashMap, hash_map::Entry};

const MAX_LANES: usize = 8;

#[derive(Debug)]
pub(super) struct History<'a> {
    commits: &'a [GitGraphCommit],
    occurrences: HashMap<&'a str, Occurrence>,
    relation_reason: Option<&'static str>,
}

#[derive(Debug, Clone, Copy)]
enum Occurrence {
    Unique(usize),
    Ambiguous,
}

impl<'a> History<'a> {
    pub(super) fn plan(commits: &'a [GitGraphCommit], resources: &ResourceContext) -> Result<Self> {
        resources.charge_layout_work(commits.len())?;
        let mut occurrences = HashMap::new();
        occurrences
            .try_reserve(commits.len())
            .map_err(|_| AsciiError::allocation_failed("git_history"))?;
        let mut relation_reason = None;
        for (index, commit) in commits.iter().enumerate() {
            charge_text_layout(resources, &commit.id)?;
            match occurrences.entry(commit.id.as_str()) {
                Entry::Vacant(entry) => {
                    entry.insert(Occurrence::Unique(index));
                }
                Entry::Occupied(mut entry) => {
                    entry.insert(Occurrence::Ambiguous);
                    relation_reason =
                        Some("Repeated commit IDs are ambiguous; # numbers identify occurrences.");
                }
            }
            if index > 0 && commit.seq <= commits[index - 1].seq {
                relation_reason =
                    Some("Commit sequence is not chronological; model order is preserved.");
            }
        }
        let mut history = Self {
            commits,
            occurrences,
            relation_reason,
        };
        for (index, commit) in commits.iter().enumerate() {
            resources.charge_layout_work(1)?;
            for parent in &commit.parents {
                charge_text_layout(resources, parent)?;
                match history.occurrences.get(parent.as_str()) {
                    Some(Occurrence::Unique(parent_index)) if *parent_index < index => {}
                    Some(Occurrence::Ambiguous) => {
                        history.relation_reason = Some(
                            "Repeated commit IDs are ambiguous; # numbers identify occurrences.",
                        );
                    }
                    _ => {
                        history.relation_reason = Some(
                            "Some parents are external or out of order; explicit references are preserved.",
                        );
                    }
                }
            }
            if commit.parents.len() > 2
                || (commit.parents.len() == 2 && commit.parents[0] == commit.parents[1])
            {
                history.relation_reason = Some(
                    "Parent multiplicity is preserved using explicit references in model order.",
                );
            }
        }
        if history.relation_reason.is_none() {
            let mut lanes = HistoryLanes::default();
            for index in (0..commits.len()).rev() {
                let parents = history.parent_indices(index, resources)?;
                resources.charge_layout_work(MAX_LANES)?;
                if lanes.advance(index, parents).is_none() {
                    history.relation_reason = Some(
                        "Connections exceed the readable lane layout; explicit parent references follow.",
                    );
                    break;
                }
            }
        }
        Ok(history)
    }

    pub(super) fn relation_reason(&self) -> Option<&'static str> {
        self.relation_reason
    }

    fn parent_indices(
        &self,
        index: usize,
        resources: &ResourceContext,
    ) -> Result<[Option<usize>; 2]> {
        let mut parents = [None; 2];
        for (target, parent) in parents.iter_mut().zip(&self.commits[index].parents) {
            charge_text_layout(resources, parent)?;
            if let Some(Occurrence::Unique(index)) = self.occurrences.get(parent.as_str()) {
                *target = Some(*index);
            }
        }
        Ok(parents)
    }

    pub(super) fn advance(
        &self,
        lanes: &mut HistoryLanes,
        index: usize,
        resources: &ResourceContext,
    ) -> Result<LaneStep> {
        let parents = self.parent_indices(index, resources)?;
        resources.charge_layout_work(MAX_LANES)?;
        lanes
            .advance(index, parents)
            .ok_or(AsciiError::InvalidOption {
                field: "git_history",
                message: "history lanes changed after admission",
            })
    }

    pub(super) fn push_parents(
        &self,
        line: &mut BudgetedTextLine<'_>,
        commit: &GitGraphCommit,
        identify_occurrences: bool,
    ) -> Result<()> {
        if commit.parents.is_empty() {
            return line.push_str("No parents (root).");
        }
        let cherry_pick = commit.commit_type == 4 && commit.parents.len() == 2;
        line.push_str(if cherry_pick || commit.parents.len() == 1 {
            "Parent: "
        } else {
            "Parents: "
        })?;
        for (index, parent) in commit.parents.iter().enumerate() {
            if index > 0 {
                line.push_str(if cherry_pick {
                    "; cherry-picked from: "
                } else {
                    ", "
                })?;
            }
            line.push_quoted_text(parent)?;
            if identify_occurrences {
                match self.occurrences.get(parent.as_str()) {
                    Some(Occurrence::Unique(index)) => {
                        line.write_fmt(format_args!(" (#{})", index + 1))?;
                    }
                    Some(Occurrence::Ambiguous) => line.push_str(" (ambiguous ID)")?,
                    None => line.push_str(" (external)")?,
                }
            }
        }
        Ok(())
    }
}

#[derive(Default)]
pub(super) struct HistoryLanes {
    pending: [Option<usize>; MAX_LANES],
}

pub(super) struct LaneStep {
    before: [bool; MAX_LANES],
    after: [bool; MAX_LANES],
    commit_lane: usize,
    parents: [Option<usize>; 2],
    width: usize,
}

impl HistoryLanes {
    fn advance(&mut self, index: usize, parents: [Option<usize>; 2]) -> Option<LaneStep> {
        let commit_lane = self
            .pending
            .iter()
            .position(|value| *value == Some(index))
            .or_else(|| self.pending.iter().position(Option::is_none))?;
        let mut before = self.pending.map(|value| value.is_some());
        before[commit_lane] = true;
        self.pending[commit_lane] = None;
        let mut parent_lanes = [None; 2];
        for (target, parent) in parent_lanes
            .iter_mut()
            .zip(parents)
            .filter_map(|(target, parent)| parent.map(|parent| (target, parent)))
        {
            let lane = self
                .pending
                .iter()
                .position(|value| *value == Some(parent))
                .or_else(|| {
                    if self.pending[commit_lane].is_none() {
                        Some(commit_lane)
                    } else {
                        self.pending.iter().position(Option::is_none)
                    }
                })?;
            self.pending[lane] = Some(parent);
            *target = Some(lane);
        }
        let after = self.pending.map(|value| value.is_some());
        let width = (0..MAX_LANES).rfind(|lane| before[*lane] || after[*lane])? + 1;
        for parent_lane in parent_lanes.iter().flatten().copied() {
            let left = parent_lane.min(commit_lane);
            let right = parent_lane.max(commit_lane);
            if (left + 1..right).any(|lane| before[lane] || after[lane]) {
                return None;
            }
        }
        Some(LaneStep {
            before,
            after,
            commit_lane,
            parents: parent_lanes,
            width,
        })
    }
}

impl LaneStep {
    pub(super) fn push_commit_prefix(&self, line: &mut BudgetedTextLine<'_>) -> Result<()> {
        for lane in 0..self.width {
            line.push_str(if lane == self.commit_lane {
                "* "
            } else if self.before[lane] {
                "| "
            } else {
                "  "
            })?;
        }
        Ok(())
    }

    pub(super) fn push_parent_prefix(&self, line: &mut BudgetedTextLine<'_>) -> Result<()> {
        let left = self
            .parents
            .iter()
            .flatten()
            .copied()
            .min()
            .unwrap_or(self.commit_lane)
            .min(self.commit_lane);
        let right = self
            .parents
            .iter()
            .flatten()
            .copied()
            .max()
            .unwrap_or(self.commit_lane)
            .max(self.commit_lane);
        for lane in 0..self.width {
            if left < right && (left..=right).contains(&lane) {
                line.push_str(
                    if lane == self.commit_lane || self.parents.contains(&Some(lane)) {
                        "+"
                    } else {
                        "-"
                    },
                )?;
                line.push_str(if lane < right { "-" } else { " " })?;
            } else {
                line.push_str(if self.after[lane] { "| " } else { "  " })?;
            }
        }
        Ok(())
    }
}
