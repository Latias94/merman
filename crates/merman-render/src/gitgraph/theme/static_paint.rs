use std::sync::OnceLock;

use merman_core::MermaidConfig;

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeMechanismKey, ResolvedDiagramTheme,
    ThemeTarget, ThemeVariant,
};
use crate::family::{
    DirectStaticPaint, DirectStaticSelectorDomain, FamilyThemeEvidence, FamilyThemeResidualReason,
    resolve_direct_static_fill, unsupported_residual_for_facet,
};
use crate::model::GitGraphDiagramLayout;
use crate::resources::{OperationWorkError, OperationWorkMeter};

const COMMIT_LABEL_BACKGROUND_PATH: &str = "themeVariables.commitLabelBackground";

#[derive(Debug, Clone)]
struct StaticPaintAssignment {
    route_key: FamilyThemeMechanismKey,
    paint: DirectStaticPaint,
}

#[derive(Debug)]
pub(crate) struct GitGraphStaticPaintPlan {
    assignment: Option<StaticPaintAssignment>,
    pending_key: Option<FamilyThemeMechanismKey>,
    evidence: FamilyThemeEvidence,
    terminal_receipt: OnceLock<GitGraphStaticPaintReceipt>,
}

impl GitGraphStaticPaintPlan {
    pub(crate) fn baseline() -> Self {
        Self {
            assignment: None,
            pending_key: None,
            evidence: FamilyThemeEvidence::default(),
            terminal_receipt: OnceLock::new(),
        }
    }

    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &MermaidConfig,
        layout: &GitGraphDiagramLayout,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let mut plan = Self {
            evidence: FamilyThemeEvidence::from_theme(theme),
            ..Self::baseline()
        };
        let Some(theme) = theme else {
            return Ok(plan);
        };

        let style = theme.style_with_work_meter(
            ThemeTarget::EdgeLabelBackground,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let winner_key =
            style
                .fill_resolution()
                .winner()
                .map(|origin| FamilyThemeMechanismKey::Rule {
                    index: origin.rule_index(),
                    target: origin.target(),
                });
        let mermaid_owns_commit_label_background = effective_config
            .get_str("theme")
            .is_some_and(crate::gitgraph::gitgraph_theme_uses_color_gen);
        let assignment = (!mermaid_owns_commit_label_background
            && !merman_core::__private::config_path_overrides_typed_default(
                effective_config,
                COMMIT_LABEL_BACKGROUND_PATH,
            ))
        .then(|| {
            resolve_direct_static_fill(
                theme,
                &style,
                &[ThemeTarget::EdgeLabelBackground],
                DirectStaticSelectorDomain::Default,
            )
        })
        .flatten()
        .map(|paint| StaticPaintAssignment {
            route_key: FamilyThemeMechanismKey::Rule {
                index: paint.rule_index(),
                target: ThemeTarget::EdgeLabelBackground,
            },
            paint,
        });

        for route in theme.family_mechanism_routes().iter().copied() {
            let FamilyThemeMechanism::RuleFacet {
                rule_index,
                target: ThemeTarget::EdgeLabelBackground,
                facet,
                ..
            } = route.mechanism()
            else {
                continue;
            };
            let key = FamilyThemeMechanismKey::Rule {
                index: rule_index,
                target: ThemeTarget::EdgeLabelBackground,
            };
            let route_won = winner_key.as_ref() == Some(&key);
            if !route_won {
                plan.evidence.mark_not_applicable(key);
                continue;
            }

            match route.disposition() {
                FamilyThemeDisposition::TypedAdapter if assignment.is_some() => {
                    plan.pending_key = Some(key);
                }
                FamilyThemeDisposition::TypedAdapter => plan.evidence.mark_not_applicable(key),
                FamilyThemeDisposition::Unsupported => {
                    plan.evidence
                        .mark_residual(key, unsupported_residual_for_facet(facet));
                }
                FamilyThemeDisposition::LegacyCompatibility => {
                    plan.evidence
                        .mark_residual(key, FamilyThemeResidualReason::UnsupportedPaint);
                }
            }
        }

        plan.assignment = assignment;
        if plan.pending_key.is_none() && plan.assignment.is_some() {
            plan.evidence.mark_not_applicable(
                plan.assignment
                    .as_ref()
                    .expect("assignment exists")
                    .route_key
                    .clone(),
            );
        }

        // A commit label background cannot reach a terminal without a visible commit label. The
        // receipt performs the final occurrence check, but this early guard keeps an empty
        // GitGraph document from claiming an applied paint route.
        if layout
            .commits
            .iter()
            .all(|commit| !crate::gitgraph::gitgraph_commit_label_is_visible(layout, commit))
        {
            if let Some(key) = plan.pending_key.take() {
                plan.evidence.mark_not_applicable(key);
            }
        }

        Ok(plan)
    }

    pub(crate) fn commit_label_background_css(&self) -> Option<&str> {
        self.assignment
            .as_ref()
            .map(|assignment| assignment.paint.css())
    }

    pub(crate) fn begin_terminal_receipt(&self) -> Option<GitGraphStaticPaintReceipt> {
        self.assignment
            .as_ref()
            .map(|assignment| GitGraphStaticPaintReceipt {
                expected_route: assignment.route_key.clone(),
                expected_value: assignment.paint.css().into(),
                css_emitted: false,
                values_match: true,
                terminal_count: 0,
            })
    }

    pub(crate) fn record_terminal_receipt(&self, receipt: GitGraphStaticPaintReceipt) -> bool {
        self.assignment.is_some()
            && receipt.proves(self)
            && self.terminal_receipt.set(receipt).is_ok()
    }

    pub(crate) fn record_css_emission(
        &self,
        receipt: &mut Option<GitGraphStaticPaintReceipt>,
        emitted_value: Option<&str>,
    ) {
        if let Some(receipt) = receipt.as_mut() {
            receipt.record_css_emission(self, emitted_value);
        }
    }

    pub(crate) fn record_commit_label_background(
        &self,
        receipt: &mut Option<GitGraphStaticPaintReceipt>,
    ) {
        if let Some(receipt) = receipt.as_mut() {
            receipt.record_commit_label_background();
        }
    }

    pub(crate) fn finish_evidence(&self) -> FamilyThemeEvidence {
        let mut evidence = self.evidence.clone();
        if let Some(key) = self.pending_key.clone() {
            match self.terminal_receipt.get() {
                Some(receipt) if receipt.proves(self) => {
                    evidence.mark_applied_with_capabilities(
                        key,
                        self.assignment
                            .as_ref()
                            .map(|assignment| [assignment.paint.capability()])
                            .into_iter()
                            .flatten(),
                    );
                }
                Some(_) => evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedPaint),
                None => evidence.mark_residual(key, FamilyThemeResidualReason::UnsupportedPaint),
            }
        }
        evidence
    }
}

#[derive(Debug)]
pub(crate) struct GitGraphStaticPaintReceipt {
    expected_route: FamilyThemeMechanismKey,
    expected_value: Box<str>,
    css_emitted: bool,
    values_match: bool,
    terminal_count: usize,
}

impl GitGraphStaticPaintReceipt {
    fn record_css_emission(&mut self, plan: &GitGraphStaticPaintPlan, emitted_value: Option<&str>) {
        self.values_match &= emitted_value == plan.commit_label_background_css()
            && emitted_value == Some(self.expected_value.as_ref());
        self.css_emitted = true;
    }

    fn record_commit_label_background(&mut self) {
        self.terminal_count = self.terminal_count.saturating_add(1);
    }

    fn proves(&self, plan: &GitGraphStaticPaintPlan) -> bool {
        self.css_emitted
            && self.values_match
            && self.expected_route
                == plan
                    .assignment
                    .as_ref()
                    .map(|assignment| assignment.route_key.clone())
                    .expect("receipt requires assignment")
            && self.terminal_count > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, ThemeRule, ThemeRuleSet,
        ThemeStylePatch,
    };
    use crate::model::GitGraphCommitLayout;
    use crate::resources::RenderResourcePolicy;

    fn layout_with_label() -> GitGraphDiagramLayout {
        GitGraphDiagramLayout {
            bounds: None,
            direction: "LR".to_string(),
            rotate_commit_label: false,
            show_branches: false,
            show_commit_label: true,
            parallel_commits: false,
            diagram_padding: 0.0,
            max_pos: 0.0,
            branches: Vec::new(),
            commits: vec![GitGraphCommitLayout {
                id: "1".to_string(),
                message: "one".to_string(),
                seq: 0,
                commit_type: 0,
                parents: Vec::new(),
                branch: "main".to_string(),
                custom_type: None,
                custom_id: Some(true),
                tags: Vec::new(),
                x: 0.0,
                y: 0.0,
                pos: 0.0,
                pos_with_offset: 0.0,
            }],
            arrows: Vec::new(),
        }
    }

    fn plan() -> GitGraphStaticPaintPlan {
        let paint = CanvasPaint::solid("#123456").expect("valid paint");
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(
                        ThemeRule::new(
                            ThemeTarget::EdgeLabelBackground,
                            ThemeStylePatch::default().with_fill(paint),
                        )
                        .for_family(crate::DiagramFamilyId::GIT_GRAPH),
                    ),
                ),
            )
            .expect("compile theme")
            .resolve(crate::DiagramFamilyId::GIT_GRAPH);
        GitGraphStaticPaintPlan::resolve(
            Some(&theme),
            &MermaidConfig::default(),
            &layout_with_label(),
            &OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input()),
        )
        .expect("resolve static paint")
    }

    #[test]
    fn receipt_requires_css_and_commit_label_background_terminal() {
        let plan = plan();
        let mut receipt = plan.begin_terminal_receipt().expect("typed receipt");
        receipt.record_css_emission(&plan, Some("#123456"));
        // The production writer records this occurrence after emitting the rect.
        receipt.record_commit_label_background();
        assert!(receipt.proves(&plan));
    }
}
