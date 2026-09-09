use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey,
    FamilyThemeSelectorShape, ResolvedDiagramTheme, ResolvedStyleProperty, ThemeTarget,
    ThemeVariant,
};
use crate::family::{
    DirectStaticPaint, DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeResidualReason,
    resolve_direct_static_fill, resolved_style_property_for_facet, unsupported_residual_for_facet,
};
use crate::model::GitGraphDiagramLayout;
use crate::resources::{OperationWorkError, OperationWorkMeter};

use super::{GITGRAPH_PALETTE_SLOT_COUNT, GitGraphCommitKind, palette_slot};

pub(crate) const GITGRAPH_NODE_FILL_PATHS: [&str; 3] = [
    "themeVariables.primaryColor",
    "themeVariables.mainBkg",
    "themeVariables.tagLabelBackground",
];

#[derive(Debug, Default)]
struct Observation {
    direct: u8,
    unsupported: u8,
    other: Option<FamilyThemeResidualReason>,
}

#[derive(Debug)]
pub(crate) struct GitGraphNodeFillPlan {
    fill: Option<DirectStaticPaint>,
    available_sources: u8,
    observations: BTreeMap<FamilyThemeMechanismKey, Observation>,
    terminal: OnceLock<TerminalFacts>,
}

#[derive(Debug, Default)]
struct TerminalFacts {
    sources: u8,
    nodes_visible: bool,
}

impl GitGraphNodeFillPlan {
    pub(crate) fn resolve(
        theme: &ResolvedDiagramTheme,
        config: &MermaidConfig,
        work: &OperationWorkMeter,
    ) -> Result<Option<Self>, OperationWorkError> {
        let requested = theme.family_mechanism_routes().iter().any(|route| {
            matches!(route.mechanism(), FamilyThemeMechanism::RuleFacet {
                target: ThemeTarget::Node, facet, ..
            } if resolved_style_property_for_facet(facet) == ResolvedStyleProperty::Fill)
        });
        if !requested {
            return Ok(None);
        }
        let style =
            theme.style_with_work_meter(ThemeTarget::Node, ThemeVariant::Default, None, work)?;
        // Capture is mandatory: a materialized/derived value cannot establish raw ownership.
        let blocked: [Option<bool>; 3] = std::array::from_fn(|i| {
            merman_core::__private::config_post_detection_default_blocked(
                config,
                GITGRAPH_NODE_FILL_PATHS[i],
            )
        });
        let available_sources = blocked.iter().enumerate().fold(0, |mask, (i, ownership)| {
            mask | if *ownership == Some(false) { 1 << i } else { 0 }
        });
        let fill = (available_sources != 0)
            .then(|| {
                resolve_direct_static_fill(
                    theme,
                    &style,
                    &[ThemeTarget::Node],
                    DirectStaticSelectorDomain::Default,
                )
            })
            .flatten();
        let mut plan = Self {
            fill,
            available_sources,
            observations: BTreeMap::new(),
            terminal: OnceLock::new(),
        };
        for route in theme.family_mechanism_routes() {
            let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::Node,
                facet,
                selector,
            } = route.mechanism()
            else {
                continue;
            };
            work.charge(1)?;
            if route.disposition() == FamilyThemeDisposition::LegacyCompatibility {
                continue;
            }
            let property = resolved_style_property_for_facet(facet);
            let won = style.winner_rule_properties().any(|(candidate, origin)| {
                candidate == property && origin.rule_index() == rule_index
            });
            let unproved_ordinal = matches!(
                selector,
                FamilyThemeSelectorShape::Ordinal {
                    variant: None | Some(ThemeVariant::Default),
                    ..
                }
            ) && !style.winner_rule_properties().any(
                |(candidate, origin)| candidate == property && origin.rule_index() > rule_index,
            );
            let observation = plan
                .observations
                .entry(theme.family_mechanism_key(*route))
                .or_default();
            if !won && !unproved_ordinal {
                continue;
            }
            if property != ResolvedStyleProperty::Fill {
                observation
                    .other
                    .get_or_insert(unsupported_residual_for_facet(facet));
                continue;
            }
            for (i, ownership) in blocked.iter().enumerate() {
                if *ownership == Some(true) {
                    continue;
                }
                if route.disposition() == FamilyThemeDisposition::TypedAdapter
                    && available_sources & (1 << i) != 0
                    && plan
                        .fill
                        .as_ref()
                        .is_some_and(|fill| fill.rule_index() == rule_index)
                {
                    observation.direct |= 1 << i;
                } else {
                    observation.unsupported |= 1 << i;
                }
            }
        }
        Ok(Some(plan))
    }

    pub(crate) fn css_values(&self) -> [Option<&str>; 3] {
        std::array::from_fn(|i| {
            self.fill
                .as_ref()
                .filter(|_| self.available_sources & (1 << i) != 0)
                .map(DirectStaticPaint::css)
        })
    }

    pub(crate) fn begin_terminal_receipt<'a>(
        &'a self,
        layout: &'a GitGraphDiagramLayout,
        branches: &'a HashMap<&'a str, i64>,
        palette_fill_masked: [[bool; GITGRAPH_PALETTE_SLOT_COUNT]; 2],
    ) -> GitGraphNodeFillReceipt<'a> {
        GitGraphNodeFillReceipt {
            plan: self,
            layout,
            branches,
            palette_fill_masked,
            state_source: None,
            tag_source: None,
            branch_sources: [None; GITGRAPH_PALETTE_SLOT_COUNT],
            outer_sources: [None; GITGRAPH_PALETTE_SLOT_COUNT],
            next_branch: 0,
            next_state: 0,
            next_outer: 0,
            tag_commit: 0,
            next_tag: 0,
            css_recorded: false,
            valid: true,
            facts: TerminalFacts::default(),
        }
    }

    pub(crate) fn record_terminal(&self, mut receipt: GitGraphNodeFillReceipt<'_>) -> bool {
        let complete = std::ptr::eq(self, receipt.plan)
            && receipt.valid
            && receipt.css_recorded
            && receipt.next_branch
                == if receipt.layout.show_branches {
                    receipt.layout.branches.len()
                } else {
                    0
                }
            && receipt.next_state_commit(false).is_none()
            && receipt.next_state_commit(true).is_none()
            && receipt.next_tag_event().is_none();
        if complete {
            receipt.facts.nodes_visible |= !receipt.layout.commits.is_empty();
            self.terminal.set(receipt.facts).is_ok()
        } else {
            false
        }
    }

    pub(crate) fn finish_evidence(&self, evidence: &mut FamilyThemeEvidence) {
        for (key, observation) in &self.observations {
            let Some(facts) = self.terminal.get() else {
                evidence.mark_residual(key.clone(), FamilyThemeResidualReason::UnsupportedPaint);
                continue;
            };
            if facts.nodes_visible
                && let Some(reason) = observation.other
            {
                evidence.mark_residual(key.clone(), reason);
            } else if observation.unsupported & facts.sources != 0 {
                evidence.mark_residual(key.clone(), FamilyThemeResidualReason::UnsupportedPaint);
            } else if observation.direct & facts.sources != 0 {
                evidence.mark_applied_with_capabilities(
                    key.clone(),
                    self.fill.as_ref().map(DirectStaticPaint::capability),
                );
            } else {
                evidence.mark_not_applicable(key.clone());
            }
        }
    }
}

/// Streaming checks borrow layout identities; CSS facts are bounded by the eight visible slots.
pub(crate) struct GitGraphNodeFillReceipt<'a> {
    plan: &'a GitGraphNodeFillPlan,
    layout: &'a GitGraphDiagramLayout,
    branches: &'a HashMap<&'a str, i64>,
    palette_fill_masked: [[bool; GITGRAPH_PALETTE_SLOT_COUNT]; 2],
    state_source: Option<usize>,
    tag_source: Option<usize>,
    branch_sources: [Option<usize>; GITGRAPH_PALETTE_SLOT_COUNT],
    outer_sources: [Option<usize>; GITGRAPH_PALETTE_SLOT_COUNT],
    next_branch: usize,
    next_state: usize,
    next_outer: usize,
    tag_commit: usize,
    next_tag: usize,
    css_recorded: bool,
    valid: bool,
    facts: TerminalFacts,
}

impl GitGraphNodeFillReceipt<'_> {
    pub(crate) fn record_css(
        &mut self,
        state: (usize, &str),
        tag: (usize, &str),
        branches: [Option<(usize, &str)>; GITGRAPH_PALETTE_SLOT_COUNT],
        outers: [Option<(usize, &str)>; GITGRAPH_PALETTE_SLOT_COUNT],
    ) {
        self.valid &= !self.css_recorded;
        for (source, value) in [Some(state), Some(tag)]
            .into_iter()
            .chain(branches)
            .chain(outers)
            .flatten()
        {
            self.valid &= self
                .plan
                .css_values()
                .get(source)
                .is_some_and(|fill| fill.is_none_or(|fill| fill == value));
        }
        self.state_source = Some(state.0);
        self.tag_source = Some(tag.0);
        self.branch_sources = branches.map(|emission| emission.map(|(source, _)| source));
        self.outer_sources = outers.map(|emission| emission.map(|(source, _)| source));
        self.css_recorded = true;
    }

    fn consume(&mut self, source: Option<usize>) {
        if let Some(source) = source {
            self.facts.sources |= 1 << source;
        }
        self.facts.nodes_visible = true;
    }

    pub(crate) fn record_branch(&mut self, index: usize, slot: usize) {
        self.valid &= self.layout.show_branches
            && index == self.next_branch
            && self
                .layout
                .branches
                .get(index)
                .is_some_and(|branch| palette_slot(branch.index) == slot);
        self.facts.nodes_visible = true;
        if self.palette_fill_masked[1].get(slot).copied() == Some(false) {
            self.consume(self.branch_sources.get(slot).copied().flatten());
        }
        self.next_branch += 1;
    }

    fn next_state_commit(&mut self, outer: bool) -> Option<usize> {
        let cursor = if outer {
            &mut self.next_outer
        } else {
            &mut self.next_state
        };
        while let Some(commit) = self.layout.commits.get(*cursor) {
            let index = *cursor;
            *cursor += 1;
            let kind = GitGraphCommitKind::from_layout(commit);
            if kind == GitGraphCommitKind::Highlight
                || (!outer
                    && matches!(
                        kind,
                        GitGraphCommitKind::Merge | GitGraphCommitKind::Reverse
                    ))
            {
                return Some(index);
            }
        }
        None
    }

    pub(crate) fn record_state(&mut self, index: usize, slot: usize, outer: bool) {
        self.valid &= self.next_state_commit(outer) == Some(index);
        self.valid &= self.layout.commits.get(index).is_some_and(|commit| {
            palette_slot(
                self.branches
                    .get(commit.branch.as_str())
                    .copied()
                    .unwrap_or(0),
            ) == slot
        });
        if outer {
            self.consume(self.outer_sources.get(slot).copied().flatten());
        } else if self.palette_fill_masked[0].get(slot).copied() == Some(false) {
            self.consume(self.state_source);
        }
    }

    fn next_tag_event(&mut self) -> Option<(usize, usize)> {
        while let Some(commit) = self.layout.commits.get(self.tag_commit) {
            if self.next_tag < commit.tags.len() {
                let event = (self.tag_commit, self.next_tag);
                self.next_tag += 1;
                return Some(event);
            }
            self.tag_commit += 1;
            self.next_tag = 0;
        }
        None
    }

    pub(crate) fn record_tag(&mut self, index: usize, output_index: usize) {
        self.valid &= self.next_tag_event() == Some((index, output_index));
        self.consume(self.tag_source);
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
    fn node_fill_receipt_requires_exact_css_and_complete_ordered_terminals() {
        let layout: GitGraphDiagramLayout = serde_json::from_value(serde_json::json!({
            "bounds": null, "direction": "LR", "rotate_commit_label": false,
            "show_branches": false, "show_commit_label": false, "parallel_commits": false,
            "diagram_padding": 0.0, "max_pos": 0.0, "branches": [], "arrows": [],
            "commits": [{"id": "A", "message": "", "seq": 0, "commit_type": 2,
                "custom_id": true, "tags": ["v1"], "parents": [], "branch": "main",
                "pos": 0.0, "pos_with_offset": 0.0, "x": 0.0, "y": 0.0}]
        }))
        .unwrap();
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#123456").unwrap()),
                    )),
                ),
            )
            .unwrap()
            .resolve(crate::DiagramFamilyId::GIT_GRAPH);
        let work = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let style = theme
            .style_with_work_meter(ThemeTarget::Node, ThemeVariant::Default, None, &work)
            .unwrap();
        let branches = HashMap::new();
        for case in [
            "valid",
            "missing-css",
            "wrong-css",
            "missing-state",
            "missing-outer",
            "duplicate-state",
            "wrong-slot",
            "missing-tag",
            "wrong-tag",
        ] {
            let plan = GitGraphNodeFillPlan {
                fill: resolve_direct_static_fill(
                    &theme,
                    &style,
                    &[ThemeTarget::Node],
                    DirectStaticSelectorDomain::Default,
                ),
                available_sources: 0b111,
                observations: BTreeMap::new(),
                terminal: OnceLock::new(),
            };
            assert!(plan.fill.is_some());
            let mut receipt = plan.begin_terminal_receipt(
                &layout,
                &branches,
                [[false; GITGRAPH_PALETTE_SLOT_COUNT]; 2],
            );
            if case != "missing-css" {
                receipt.record_css(
                    (
                        0,
                        if case == "wrong-css" {
                            "#abcdef"
                        } else {
                            "#123456"
                        },
                    ),
                    (2, "#123456"),
                    [None; GITGRAPH_PALETTE_SLOT_COUNT],
                    [None; GITGRAPH_PALETTE_SLOT_COUNT],
                );
            }
            if case != "missing-outer" {
                receipt.record_state(0, 0, true);
            }
            if case != "missing-state" {
                receipt.record_state(0, usize::from(case == "wrong-slot"), false);
            }
            if case == "duplicate-state" {
                receipt.record_state(0, 0, false);
            }
            if case != "missing-tag" {
                receipt.record_tag(0, usize::from(case == "wrong-tag"));
            }
            assert_eq!(plan.record_terminal(receipt), case == "valid", "{case}");
        }
    }
}
