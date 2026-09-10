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
    resolve_direct_static_fill, resolve_direct_static_stroke, resolved_style_property_for_facet,
    unsupported_residual_for_facet,
};
use crate::model::GitGraphDiagramLayout;
use crate::resources::{OperationWorkError, OperationWorkMeter};

use super::{GITGRAPH_PALETTE_SLOT_COUNT, GitGraphCommitKind, palette_slot};

const GITGRAPH_NODE_PAINT_PATHS: [&str; 6] = [
    "themeVariables.primaryColor",
    "themeVariables.mainBkg",
    "themeVariables.tagLabelBackground",
    "themeVariables.primaryBorderColor",
    "themeVariables.nodeBorder",
    "themeVariables.tagLabelBorder",
];

pub(crate) const GITGRAPH_NODE_PAINT_DEFAULTS: crate::family::FamilyPaintDefaultPaths =
    crate::family::FamilyPaintDefaultPaths::new(
        crate::DiagramFamilyId::GIT_GRAPH,
        &[ThemeTarget::Node],
        &GITGRAPH_NODE_PAINT_PATHS,
    );

#[derive(Debug, Default)]
struct Observation {
    direct: u8,
    unsupported: u8,
    other: Option<FamilyThemeResidualReason>,
}

#[derive(Debug)]
pub(crate) struct GitGraphNodePaintPlan {
    fill: Option<DirectStaticPaint>,
    stroke: Option<DirectStaticPaint>,
    available_sources: u8,
    observations: BTreeMap<FamilyThemeMechanismKey, Observation>,
    terminal: OnceLock<TerminalFacts>,
}

#[derive(Debug, Default)]
struct TerminalFacts {
    sources: u8,
    nodes_visible: bool,
}

impl GitGraphNodePaintPlan {
    pub(crate) fn resolve(
        theme: &ResolvedDiagramTheme,
        config: &MermaidConfig,
        work: &OperationWorkMeter,
    ) -> Result<Option<Self>, OperationWorkError> {
        let requested = theme.family_mechanism_routes().iter().any(|route| {
            matches!(route.mechanism(), FamilyThemeMechanism::RuleFacet {
                target: ThemeTarget::Node, facet, ..
            } if matches!(resolved_style_property_for_facet(facet), ResolvedStyleProperty::Fill | ResolvedStyleProperty::Stroke))
        });
        if !requested {
            return Ok(None);
        }
        let style =
            theme.style_with_work_meter(ThemeTarget::Node, ThemeVariant::Default, None, work)?;
        // Capture is mandatory: a materialized/derived value cannot establish raw ownership.
        let blocked: [Option<bool>; 6] = std::array::from_fn(|i| {
            merman_core::__private::config_post_detection_default_blocked(
                config,
                GITGRAPH_NODE_PAINT_PATHS[i],
            )
        });
        let available_sources = blocked.iter().enumerate().fold(0, |mask, (i, ownership)| {
            mask | if *ownership == Some(false) { 1 << i } else { 0 }
        });
        let fill = (available_sources & 0b111 != 0)
            .then(|| {
                resolve_direct_static_fill(
                    theme,
                    &style,
                    &[ThemeTarget::Node],
                    DirectStaticSelectorDomain::Default,
                )
            })
            .flatten();
        let stroke = (available_sources & 0b111000 != 0)
            .then(|| {
                resolve_direct_static_stroke(
                    theme,
                    &style,
                    &[ThemeTarget::Node],
                    DirectStaticSelectorDomain::Default,
                )
            })
            .flatten();
        let mut plan = Self {
            fill,
            stroke,
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
            if !matches!(
                property,
                ResolvedStyleProperty::Fill | ResolvedStyleProperty::Stroke
            ) {
                observation
                    .other
                    .get_or_insert(unsupported_residual_for_facet(facet));
                continue;
            }
            let (sources, paint) = if property == ResolvedStyleProperty::Fill {
                (0..3, plan.fill.as_ref())
            } else {
                (3..6, plan.stroke.as_ref())
            };
            for i in sources {
                let ownership = blocked[i];
                if ownership == Some(true) {
                    continue;
                }
                if route.disposition() == FamilyThemeDisposition::TypedAdapter
                    && available_sources & (1 << i) != 0
                    && paint.is_some_and(|paint| paint.rule_index() == rule_index)
                {
                    observation.direct |= 1 << i;
                } else {
                    observation.unsupported |= 1 << i;
                }
            }
        }
        Ok(Some(plan))
    }

    pub(crate) fn css_values(&self) -> [Option<&str>; 6] {
        std::array::from_fn(|i| {
            (if i < 3 {
                self.fill.as_ref()
            } else {
                self.stroke.as_ref()
            })
            .filter(|_| self.available_sources & (1 << i) != 0)
            .map(DirectStaticPaint::css)
        })
    }

    pub(crate) fn begin_terminal_receipt<'a>(
        &'a self,
        layout: &'a GitGraphDiagramLayout,
        branches: &'a HashMap<&'a str, i64>,
        palette_fill_masked: [[bool; GITGRAPH_PALETTE_SLOT_COUNT]; 2],
        palette_stroke_masked: [[bool; GITGRAPH_PALETTE_SLOT_COUNT]; 2],
        css: &'a GitGraphNodePaintCss,
    ) -> GitGraphNodePaintReceipt<'a> {
        let expected = self.css_values();
        let valid = css.valid
            && css.complete
            && css.values.iter().enumerate().all(|(i, value)| {
                value
                    .as_deref()
                    .is_none_or(|value| expected[i].is_none_or(|expected| value == expected))
            });
        GitGraphNodePaintReceipt {
            plan: self,
            layout,
            branches,
            palette_fill_masked,
            palette_stroke_masked,
            css,
            next_branch: 0,
            next_commit: 0,
            next_arrow: 0,
            next_label: 0,
            next_state: 0,
            next_outer: 0,
            tag_commit: 0,
            next_tag: 0,
            valid,
            facts: TerminalFacts::default(),
        }
    }

    pub(crate) fn record_terminal(&self, mut receipt: GitGraphNodePaintReceipt<'_>) -> bool {
        let complete = std::ptr::eq(self, receipt.plan)
            && receipt.valid
            && receipt.next_branch
                == if receipt.layout.show_branches {
                    receipt.layout.branches.len()
                } else {
                    0
                }
            && receipt.next_commit == receipt.layout.commits.len()
            && receipt.next_arrow == receipt.layout.arrows.len()
            && receipt.next_label_commit().is_none()
            && receipt.next_state_commit(false).is_none()
            && receipt.next_state_commit(true).is_none()
            && receipt.next_tag_event().is_none();
        complete && self.terminal.set(receipt.facts).is_ok()
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
                    [self.fill.as_ref(), self.stroke.as_ref()]
                        .into_iter()
                        .enumerate()
                        .filter(|(i, _)| {
                            observation.direct & facts.sources & (0b111 << (i * 3)) != 0
                        })
                        .filter_map(|(_, paint)| paint.map(DirectStaticPaint::capability)),
                );
            } else {
                evidence.mark_not_applicable(key.clone());
            }
        }
    }
}

/// Bounded CSS facts are recorded alongside their declarations, never recovered from SVG.
#[derive(Debug)]
pub(crate) struct GitGraphNodePaintCss {
    values: [Option<String>; 6],
    valid: bool,
    pub(crate) complete: bool,
    pub(crate) state: u8,
    pub(crate) tag: u8,
    // Branch background fill, background stroke, and branch text fill.
    pub(crate) branches: [[u8; GITGRAPH_PALETTE_SLOT_COUNT]; 3],
    // Commit and highlight outer fill/stroke remain independent under palette overrides.
    pub(crate) commits: [[u8; GITGRAPH_PALETTE_SLOT_COUNT]; 2],
    pub(crate) outers: [[u8; GITGRAPH_PALETTE_SLOT_COUNT]; 2],
    pub(crate) arrows: [u8; GITGRAPH_PALETTE_SLOT_COUNT],
    pub(crate) cherry: u8,
    pub(crate) commit_label: u8,
}

impl Default for GitGraphNodePaintCss {
    fn default() -> Self {
        Self {
            values: std::array::from_fn(|_| None),
            valid: true,
            complete: false,
            state: 0,
            tag: 0,
            branches: [[0; GITGRAPH_PALETTE_SLOT_COUNT]; 3],
            commits: [[0; GITGRAPH_PALETTE_SLOT_COUNT]; 2],
            outers: [[0; GITGRAPH_PALETTE_SLOT_COUNT]; 2],
            arrows: [0; GITGRAPH_PALETTE_SLOT_COUNT],
            cherry: 0,
            commit_label: 0,
        }
    }
}

impl GitGraphNodePaintCss {
    pub(crate) fn source(&mut self, source: usize, value: &str) -> u8 {
        if let Some(expected) = self.values[source].as_ref() {
            self.valid &= expected == value;
        } else {
            self.values[source] = Some(value.to_owned());
        }
        1 << source
    }
}

/// Streaming checks borrow layout identities without retaining per-node state.
pub(crate) struct GitGraphNodePaintReceipt<'a> {
    plan: &'a GitGraphNodePaintPlan,
    layout: &'a GitGraphDiagramLayout,
    branches: &'a HashMap<&'a str, i64>,
    palette_fill_masked: [[bool; GITGRAPH_PALETTE_SLOT_COUNT]; 2],
    palette_stroke_masked: [[bool; GITGRAPH_PALETTE_SLOT_COUNT]; 2],
    css: &'a GitGraphNodePaintCss,
    next_branch: usize,
    next_commit: usize,
    next_arrow: usize,
    next_label: usize,
    next_state: usize,
    next_outer: usize,
    tag_commit: usize,
    next_tag: usize,
    valid: bool,
    facts: TerminalFacts,
}

impl GitGraphNodePaintReceipt<'_> {
    fn consume(&mut self, sources: u8) {
        // Every consumed bit must name a declaration whose exact value was captured.
        self.valid &=
            (0..6).all(|source| sources & (1 << source) == 0 || self.css.values[source].is_some());
        self.facts.sources |= sources;
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
        if slot < GITGRAPH_PALETTE_SLOT_COUNT {
            let fill = if self.palette_fill_masked[1][slot] {
                0
            } else {
                self.css.branches[0][slot]
            };
            self.consume(fill | self.css.branches[1][slot] | self.css.branches[2][slot]);
        }
        self.next_branch += 1;
    }

    fn commit_slot_matches(&self, index: usize, slot: usize) -> bool {
        self.layout.commits.get(index).is_some_and(|commit| {
            palette_slot(
                self.branches
                    .get(commit.branch.as_str())
                    .copied()
                    .unwrap_or(0),
            ) == slot
        })
    }

    pub(crate) fn record_commit(&mut self, index: usize, slot: usize) {
        self.valid &= index == self.next_commit && self.commit_slot_matches(index, slot);
        if let Some(commit) = self.layout.commits.get(index) {
            match GitGraphCommitKind::from_layout(commit) {
                GitGraphCommitKind::CherryPick => self.consume(self.css.cherry),
                GitGraphCommitKind::Highlight => self.facts.nodes_visible = true,
                _ if slot < GITGRAPH_PALETTE_SLOT_COUNT => {
                    let fill = if self.palette_fill_masked[0][slot] {
                        0
                    } else {
                        self.css.commits[0][slot]
                    };
                    let stroke = if self.palette_stroke_masked[0][slot] {
                        0
                    } else {
                        self.css.commits[1][slot]
                    };
                    self.consume(fill | stroke);
                }
                _ => {}
            }
        }
        self.next_commit += 1;
    }

    pub(crate) fn record_arrow(&mut self, index: usize, slot: usize) {
        self.valid &= index == self.next_arrow
            && self
                .layout
                .arrows
                .get(index)
                .is_some_and(|arrow| palette_slot(arrow.class_index) == slot);
        if slot < GITGRAPH_PALETTE_SLOT_COUNT && !self.palette_stroke_masked[1][slot] {
            self.consume(self.css.arrows[slot]);
        }
        self.next_arrow += 1;
    }

    fn next_label_commit(&mut self) -> Option<usize> {
        while let Some(commit) = self.layout.commits.get(self.next_label) {
            let index = self.next_label;
            self.next_label += 1;
            if super::gitgraph_commit_label_is_visible(self.layout, commit) {
                return Some(index);
            }
        }
        None
    }

    pub(crate) fn record_commit_label(&mut self, index: usize) {
        self.valid &= self.next_label_commit() == Some(index);
        self.consume(self.css.commit_label);
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
        self.valid &= self.commit_slot_matches(index, slot);
        if slot < GITGRAPH_PALETTE_SLOT_COUNT {
            if outer {
                self.consume(self.css.outers[0][slot] | self.css.outers[1][slot]);
            } else if !self.palette_fill_masked[0][slot] || !self.palette_stroke_masked[0][slot] {
                self.consume(self.css.state);
            }
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
        self.consume(self.css.tag);
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
    fn node_paint_receipt_requires_exact_css_and_complete_ordered_terminals() {
        let layout: GitGraphDiagramLayout = serde_json::from_value(serde_json::json!({
            "bounds": null, "direction": "LR", "rotate_commit_label": false,
            "show_branches": true, "show_commit_label": true, "parallel_commits": false,
            "diagram_padding": 0.0, "max_pos": 0.0,
            "branches": [{"name": "main", "index": 0, "pos": 0.0, "bbox_width": 5.0, "bbox_height": 5.0}],
            "arrows": [{"from": "A", "to": "A", "class_index": 0, "d": "M0 0L1 1"}],
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
                            .with_fill(CanvasPaint::solid("#123456").unwrap())
                            .with_stroke(CanvasPaint::solid("#654321").unwrap()),
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
            "wrong-stroke-css",
            "missing-source",
            "missing-branch",
            "wrong-branch",
            "missing-arrow",
            "wrong-arrow",
            "missing-commit",
            "wrong-commit",
            "missing-label",
            "wrong-label",
            "missing-state",
            "missing-outer",
            "duplicate-state",
            "wrong-slot",
            "missing-tag",
            "wrong-tag",
        ] {
            let plan = GitGraphNodePaintPlan {
                fill: resolve_direct_static_fill(
                    &theme,
                    &style,
                    &[ThemeTarget::Node],
                    DirectStaticSelectorDomain::Default,
                ),
                stroke: resolve_direct_static_stroke(
                    &theme,
                    &style,
                    &[ThemeTarget::Node],
                    DirectStaticSelectorDomain::Default,
                ),
                available_sources: 0b111111,
                observations: BTreeMap::new(),
                terminal: OnceLock::new(),
            };
            assert!(plan.fill.is_some());
            let mut css = GitGraphNodePaintCss::default();
            css.state = css.source(
                0,
                if case == "wrong-css" {
                    "#abcdef"
                } else {
                    "#123456"
                },
            );
            let stroke = css.source(
                4,
                if case == "wrong-stroke-css" {
                    "#abcdef"
                } else {
                    "#654321"
                },
            );
            css.tag = css.source(2, "#123456") | stroke;
            css.branches[2][0] = stroke;
            css.arrows[0] = stroke;
            css.commit_label = stroke;
            if case == "missing-source" {
                css.values[4] = None;
            }
            css.complete = case != "missing-css";
            let mut receipt = plan.begin_terminal_receipt(
                &layout,
                &branches,
                [[false; GITGRAPH_PALETTE_SLOT_COUNT]; 2],
                [[false; GITGRAPH_PALETTE_SLOT_COUNT]; 2],
                &css,
            );
            if case != "missing-branch" {
                receipt.record_branch(usize::from(case == "wrong-branch"), 0);
            }
            if case != "missing-arrow" {
                receipt.record_arrow(usize::from(case == "wrong-arrow"), 0);
            }
            if case != "missing-commit" {
                receipt.record_commit(usize::from(case == "wrong-commit"), 0);
            }
            if case != "missing-label" {
                receipt.record_commit_label(usize::from(case == "wrong-label"));
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
