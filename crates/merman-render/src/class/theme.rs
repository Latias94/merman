use std::collections::{BTreeMap, BTreeSet};

use crate::diagram_theme::{
    FamilyThemeDisposition, FamilyThemeMechanism, FamilyThemeRuleFacet, FamilyThemeSelectorShape,
    ResolvedDiagramTheme, ResolvedStyleProperty, ResolvedThemeStyle, Specified, ThemeTarget,
    ThemeVariant,
};
use crate::family::{
    DirectStaticSelectorDomain, resolve_direct_static_fill, resolve_direct_static_stroke,
};
use crate::resources::{OperationWorkError, OperationWorkMeter};

mod cluster;
mod evidence;
mod node;
mod terminal;
mod typography;

use cluster::ClassClusterThemePlan;
pub(crate) use evidence::ClassThemeEvidenceRecorder;
use node::ClassNodeThemePlan;
pub(crate) use terminal::ClassTerminalReceiptSummary;
use terminal::ExpectedStroke;
pub(crate) use terminal::{
    ClassMarkerTerminalExpectation, ClassNodePaintTerminalEmission, ClassNodeTerminalEmission,
    ClassNodeTerminalExpectation, ClassRelationTerminalExpectation, ClassRelationThemeReceipt,
};
pub(crate) use typography::{
    ClassNodeLabelStyleFacts, ClassTypographyCssEmission, ClassTypographyTerminalFacts,
    ClassTypographyThemePlan, ClassTypographyThemeReceipt,
};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
enum ClassRelationStrokeWidth {
    #[default]
    Unspecified,
    Clear {
        rule_index: usize,
    },
    // Keep Mermaid ownership even when its CSS value cannot be reduced to a finite paint width;
    // an unmeasurable source value must not accidentally fall back to the typed default.
    MermaidOwned {
        paint_width: Option<f32>,
    },
    Typed {
        rule_index: usize,
        value: f32,
    },
}

/// Prepared final Class relation paint winner shared by bounds, SVG emission, and evidence.
/// Node resolution stays behind the same crate-private handle for existing renderer callers.
#[derive(Debug, Clone, Default)]
pub(crate) struct ClassRelationThemePlan {
    stroke: Option<ExpectedStroke>,
    mermaid_owns_stroke: bool,
    note_attachment_indices: Vec<usize>,
    stroke_width: ClassRelationStrokeWidth,
    static_winner_rules: BTreeMap<ResolvedStyleProperty, usize>,
    ordinal_winner_rules: BTreeSet<(usize, ResolvedStyleProperty)>,
    node_plan: ClassNodeThemePlan,
    cluster_plan: ClassClusterThemePlan,
    track_edge_label_backgrounds: bool,
    mermaid_owns_edge_label_background: bool,
}

impl ClassRelationThemePlan {
    pub(crate) fn resolve(
        theme: Option<&ResolvedDiagramTheme>,
        effective_config: &merman_core::MermaidConfig,
        relation_count: usize,
        node_count: usize,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        let mermaid_owns_stroke_width = merman_core::__private::config_path_overrides_typed_default(
            effective_config,
            "themeVariables.strokeWidth",
        );
        let mermaid_stroke_width = if mermaid_owns_stroke_width {
            crate::class::config::ClassConfigView::new(effective_config.as_value())
                .relation_stroke_width_for_bounds()
        } else {
            None
        };
        let mermaid_owns_stroke = merman_core::__private::config_path_overrides_typed_default(
            effective_config,
            "themeVariables.lineColor",
        );
        let mut node_plan = ClassNodeThemePlan::from_config(effective_config);
        let Some(theme) = theme else {
            return Ok(Self {
                mermaid_owns_stroke,
                stroke_width: if mermaid_owns_stroke_width {
                    ClassRelationStrokeWidth::MermaidOwned {
                        paint_width: mermaid_stroke_width,
                    }
                } else {
                    ClassRelationStrokeWidth::Unspecified
                },
                node_plan,
                ..Self::default()
            });
        };
        let track_edge_label_backgrounds = theme.family_mechanism_routes().iter().any(|route| {
            matches!(
                route.mechanism(),
                FamilyThemeMechanism::RuleFacet {
                    target: ThemeTarget::EdgeLabelBackground,
                    ..
                } | FamilyThemeMechanism::OrdinalPalette {
                    target: ThemeTarget::EdgeLabelBackground
                } | FamilyThemeMechanism::EffectBinding {
                    target: ThemeTarget::EdgeLabelBackground,
                    ..
                }
            )
        });
        let mermaid_owns_edge_label_background =
            ["themeVariables.mainBkg", "themeVariables.primaryColor"]
                .into_iter()
                .any(|path| {
                    merman_core::__private::config_path_overrides_typed_default(
                        effective_config,
                        path,
                    )
                });
        let style = theme.style_with_work_meter(
            ThemeTarget::Edge,
            ThemeVariant::Default,
            None,
            work_meter,
        )?;
        let static_winner_rules = style
            .winner_rule_properties()
            .filter(|(property, _)| {
                *property != ResolvedStyleProperty::Fill
                    || matches!(
                        style.stroke_resolution().specified(),
                        Specified::Unspecified
                    )
            })
            .map(|(property, origin)| (property, origin.rule_index()))
            .collect::<BTreeMap<_, _>>();
        node_plan.resolve_static(theme, work_meter)?;
        let ordinal_rule_targets = theme
            .family_rules()
            .filter_map(|(_, rule)| rule.ordinal().is_some().then_some(rule.target()))
            .collect::<BTreeSet<_>>();
        let mut ordinal_winner_rules = BTreeSet::new();
        if ordinal_rule_targets.contains(&ThemeTarget::Edge) {
            for ordinal in 1..=relation_count {
                let ordinal_style = theme.style_with_work_meter(
                    ThemeTarget::Edge,
                    ThemeVariant::Default,
                    Some(ordinal),
                    work_meter,
                )?;
                for (property, origin) in ordinal_style.winner_rule_properties() {
                    if property != ResolvedStyleProperty::Fill
                        || matches!(
                            ordinal_style.stroke_resolution().specified(),
                            Specified::Unspecified
                        )
                    {
                        ordinal_winner_rules.insert((origin.rule_index(), property));
                    }
                }
            }
        }
        node_plan.resolve_ordinals(
            theme,
            node_count,
            ordinal_rule_targets.contains(&ThemeTarget::Node)
                || ordinal_rule_targets.contains(&ThemeTarget::NodeLabel),
            work_meter,
        )?;
        let stroke = typed_stroke_expectation(theme, &style, mermaid_owns_stroke);
        let typed_stroke_width = style
            .stroke_width_resolution()
            .winner()
            .and_then(|origin| {
                let rule_index = origin.rule_index();
                (theme.rule_facet_disposition(rule_index, FamilyThemeRuleFacet::StrokeWidth)
                    == Some(FamilyThemeDisposition::TypedAdapter))
                .then_some((rule_index, style.stroke_width_resolution().specified()))
            })
            .map_or(
                ClassRelationStrokeWidth::Unspecified,
                |(rule_index, specified)| match specified {
                    Specified::Unspecified => ClassRelationStrokeWidth::Unspecified,
                    Specified::Clear => ClassRelationStrokeWidth::Clear { rule_index },
                    Specified::Value(value) => ClassRelationStrokeWidth::Typed {
                        rule_index,
                        value: *value,
                    },
                },
            );
        let stroke_width = if mermaid_owns_stroke_width {
            ClassRelationStrokeWidth::MermaidOwned {
                paint_width: mermaid_stroke_width,
            }
        } else {
            typed_stroke_width
        };
        Ok(Self {
            stroke,
            mermaid_owns_stroke,
            note_attachment_indices: Vec::new(),
            stroke_width,
            static_winner_rules,
            ordinal_winner_rules,
            node_plan,
            cluster_plan: ClassClusterThemePlan::default(),
            track_edge_label_backgrounds,
            mermaid_owns_edge_label_background,
        })
    }

    pub(crate) fn with_cluster_domain(
        mut self,
        theme: Option<&ResolvedDiagramTheme>,
        config: &merman_core::MermaidConfig,
        cluster_count: usize,
        work_meter: &OperationWorkMeter,
    ) -> Result<Self, OperationWorkError> {
        self.cluster_plan =
            ClassClusterThemePlan::resolve(theme, config, cluster_count, work_meter)?;
        Ok(self)
    }

    pub(crate) fn cluster_terminal_style(&self) -> &str {
        self.cluster_plan.terminal_style()
    }

    pub(crate) fn cluster_paint_rule_indices(&self) -> (Option<usize>, Option<usize>) {
        (
            self.cluster_plan
                .fill
                .as_ref()
                .map(|paint| paint.rule_index),
            self.cluster_plan
                .stroke
                .as_ref()
                .map(|paint| paint.rule_index),
        )
    }

    pub(crate) fn with_note_attachments(mut self, indices: Vec<usize>) -> Self {
        // Note attachments share static edge paint, but do not enter the relation ordinal or
        // stroke-width domains. Retain declaration indices, including gaps from unattached notes.
        self.note_attachment_indices = indices;
        self
    }

    pub(crate) const fn paint_stroke_width(&self) -> Option<f32> {
        match self.stroke_width {
            ClassRelationStrokeWidth::MermaidOwned { paint_width } => paint_width,
            ClassRelationStrokeWidth::Typed { value, .. } => Some(value),
            ClassRelationStrokeWidth::Unspecified | ClassRelationStrokeWidth::Clear { .. } => None,
        }
    }

    pub(crate) const fn typed_stroke_width(&self) -> Option<f32> {
        match self.stroke_width {
            ClassRelationStrokeWidth::Typed { value, .. } => Some(value),
            ClassRelationStrokeWidth::Unspecified
            | ClassRelationStrokeWidth::Clear { .. }
            | ClassRelationStrokeWidth::MermaidOwned { .. } => None,
        }
    }

    pub(crate) fn typed_stroke(&self) -> Option<(usize, &str)> {
        self.stroke
            .as_ref()
            .map(|expected| (expected.rule_index, expected.css.as_str()))
    }

    pub(crate) fn begin_terminal_receipt(
        &self,
        relations: Vec<ClassRelationTerminalExpectation>,
        markers: Vec<ClassMarkerTerminalExpectation>,
        hand_drawn: bool,
    ) -> ClassRelationThemeReceipt {
        ClassRelationThemeReceipt::new(
            relations,
            if self.stroke.is_some() {
                markers
            } else {
                Vec::new()
            },
            self.stroke.clone(),
            hand_drawn,
        )
        .with_note_attachments(self.note_attachment_indices.clone())
        .with_edge_label_backgrounds(self.track_edge_label_backgrounds)
    }

    pub(crate) fn begin_terminal_receipt_with_nodes(
        &self,
        nodes: Vec<ClassNodeTerminalExpectation>,
        cluster_ids: Vec<String>,
        relations: Vec<ClassRelationTerminalExpectation>,
        markers: Vec<ClassMarkerTerminalExpectation>,
        hand_drawn: bool,
    ) -> ClassRelationThemeReceipt {
        self.begin_terminal_receipt(relations, markers, hand_drawn)
            .with_nodes(nodes)
            .with_clusters(
                cluster_ids,
                self.cluster_plan.fill.clone(),
                self.cluster_plan.stroke.clone(),
            )
    }

    pub(crate) fn resolve_node_expectations(
        &self,
        node_ids: impl IntoIterator<Item = String>,
        work_meter: &OperationWorkMeter,
    ) -> Result<Vec<ClassNodeTerminalExpectation>, OperationWorkError> {
        self.node_plan.resolve_expectations(node_ids, work_meter)
    }

    fn typed_stroke_width_emission(&self) -> Option<(usize, f32)> {
        match self.stroke_width {
            ClassRelationStrokeWidth::Typed { rule_index, value } => Some((rule_index, value)),
            ClassRelationStrokeWidth::Unspecified
            | ClassRelationStrokeWidth::Clear { .. }
            | ClassRelationStrokeWidth::MermaidOwned { .. } => None,
        }
    }

    fn stroke_width_is_clear_for(&self, rule_index: usize) -> bool {
        matches!(
            self.stroke_width,
            ClassRelationStrokeWidth::Clear {
                rule_index: winner
            } if winner == rule_index
        )
    }

    fn route_won(
        &self,
        rule_index: usize,
        target: ThemeTarget,
        selector: FamilyThemeSelectorShape,
        facet: FamilyThemeRuleFacet,
    ) -> bool {
        if target != ThemeTarget::Edge {
            return false;
        }
        if (facet == FamilyThemeRuleFacet::StrokeWidth
            && matches!(
                self.stroke_width,
                ClassRelationStrokeWidth::MermaidOwned { .. }
            ))
            || (matches!(
                facet,
                FamilyThemeRuleFacet::Fill(_) | FamilyThemeRuleFacet::Stroke(_)
            ) && self.mermaid_owns_stroke)
        {
            return false;
        }
        let property = crate::family::resolved_style_property_for_facet(facet);
        match selector {
            FamilyThemeSelectorShape::Static {
                variant: None | Some(ThemeVariant::Default),
            } => self.static_winner_rules.get(&property).copied() == Some(rule_index),
            FamilyThemeSelectorShape::Ordinal {
                variant: None | Some(ThemeVariant::Default),
                ..
            } => self.ordinal_winner_rules.contains(&(rule_index, property)),
            FamilyThemeSelectorShape::Static { .. } | FamilyThemeSelectorShape::Ordinal { .. } => {
                false
            }
        }
    }

    fn node_route_won(
        &self,
        rule_index: usize,
        target: ThemeTarget,
        selector: FamilyThemeSelectorShape,
        facet: FamilyThemeRuleFacet,
    ) -> bool {
        self.node_plan
            .route_won(rule_index, target, selector, facet)
    }
}

fn typed_stroke_expectation(
    theme: &ResolvedDiagramTheme,
    style: &ResolvedThemeStyle,
    mermaid_owns_stroke: bool,
) -> Option<ExpectedStroke> {
    if mermaid_owns_stroke {
        return None;
    }
    // Class uses fill as a relation paint fallback only when stroke was never specified.
    // Clear and unsupported stroke values still own the terminal and block that fallback.
    let (property, paint) = if matches!(
        style.stroke_resolution().specified(),
        Specified::Unspecified
    ) {
        (
            ResolvedStyleProperty::Fill,
            resolve_direct_static_fill(
                theme,
                style,
                &[ThemeTarget::Edge],
                DirectStaticSelectorDomain::Default,
            ),
        )
    } else {
        (
            ResolvedStyleProperty::Stroke,
            resolve_direct_static_stroke(
                theme,
                style,
                &[ThemeTarget::Edge],
                DirectStaticSelectorDomain::Default,
            ),
        )
    };
    let (css, rule_index, _) = paint?.into_parts();
    Some(ExpectedStroke {
        rule_index,
        property,
        css: css.into_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relation_width_receipt_requires_every_terminal_checkpoint() {
        let plan = ClassRelationThemePlan {
            stroke_width: ClassRelationStrokeWidth::Typed {
                rule_index: 3,
                value: 6.0,
            },
            ..ClassRelationThemePlan::default()
        };
        let relations = (0..2)
            .map(|index| ClassRelationTerminalExpectation::new(index, None, None))
            .collect();
        let mut receipt = plan.begin_terminal_receipt(relations, Vec::new(), false);
        receipt.record_relation(0, None, None, None, ";;;", None, true);
        assert!(!receipt.proves_typed_width());

        receipt.record_relation(1, None, None, None, ";;;", None, true);
        assert!(receipt.proves_typed_width());
    }
}
