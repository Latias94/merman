//! Current route invariants and workspace-private cutover observations.

use super::*;
pub(super) use crate::theme_route_cutover::{
    ThemeRouteCutoverDescriptor, ThemeRouteCutoverFacet, ThemeRouteCutoverId,
    ThemeRouteCutoverInventoryError, ThemeRouteCutoverProjectionSet, ThemeRouteCutoverSelector,
    ThemeRouteCutoverValue,
};

/// Stable identity used by the current matrix test to detect legacy compatibility routes.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) enum LegacyCompatibilityRouteKey {
    BaseTypography {
        family: DiagramFamilyId,
        property: ThemeTypographyProperty,
    },
    RuleFacet {
        family: DiagramFamilyId,
        target: ThemeTarget,
        selector: FamilyThemeSelectorShape,
        facet: FamilyThemeRuleFacet,
    },
    OrdinalPalette {
        family: DiagramFamilyId,
        target: ThemeTarget,
    },
    EffectBinding {
        family: DiagramFamilyId,
        target: ThemeTarget,
    },
}

/// Checks the current matrix domain independently of a concrete theme recipe.
#[cfg(test)]
pub(super) fn legacy_compatibility_route_inventory() -> Vec<LegacyCompatibilityRouteKey> {
    let mut routes = Vec::new();
    for &family in DiagramFamilyId::all() {
        for &property in ThemeTypographyProperty::ALL {
            if classify_base_typography(family, property)
                == FamilyThemeDisposition::LegacyCompatibility
            {
                routes.push(LegacyCompatibilityRouteKey::BaseTypography { family, property });
            }
        }

        for &target in ThemeTarget::ALL {
            if target == ThemeTarget::Canvas || !target.valid_for(family) {
                continue;
            }
            for_each_matrix_selector(|selector| {
                for_each_matrix_rule_facet(|facet| {
                    if classify_rule_facet(family, target, selector, facet)
                        == FamilyThemeDisposition::LegacyCompatibility
                    {
                        routes.push(LegacyCompatibilityRouteKey::RuleFacet {
                            family,
                            target,
                            selector,
                            facet,
                        });
                    }
                });
            });
            if compile_ordinal_palette_route(family, target).disposition()
                == FamilyThemeDisposition::LegacyCompatibility
            {
                routes.push(LegacyCompatibilityRouteKey::OrdinalPalette { family, target });
            }
            if compile_effect_binding_route(family, 0, target).disposition()
                == FamilyThemeDisposition::LegacyCompatibility
            {
                routes.push(LegacyCompatibilityRouteKey::EffectBinding { family, target });
            }
        }
    }
    routes.sort_unstable();
    routes.dedup();
    routes
}

/// Returns every currently typed route that changes ownership of legacy bridge projections.
///
/// Direct-only routes such as radius, dasharray, or Flowchart ordinal palettes are deliberately
/// absent because they never had an equivalent bridge projection to retire.
pub(crate) fn legacy_replacing_typed_routes()
-> Result<Vec<ThemeRouteCutoverDescriptor>, ThemeRouteCutoverInventoryError> {
    let mut routes = Vec::new();
    for &family in DiagramFamilyId::all() {
        for &target in ThemeTarget::ALL {
            // This direct consumer was added after KTD23 retired the unused Class CSS projection.
            // It does not replace an active bridge and must not acquire a new cutover authority.
            if family == DiagramFamilyId::CLASS && target == ThemeTarget::EdgeLabelBackground {
                continue;
            }
            for (facet, channel) in [
                (ThemeRouteCutoverFacet::Fill, PaintChannel::Fill),
                (ThemeRouteCutoverFacet::Stroke, PaintChannel::Stroke),
            ] {
                for selector in std::iter::once(ThemeRouteCutoverSelector::StaticUnqualified).chain(
                    ThemeVariant::ALL
                        .iter()
                        .copied()
                        .map(ThemeRouteCutoverSelector::StaticVariant),
                ) {
                    for (value, paint_kind) in [
                        (
                            ThemeRouteCutoverValue::Transparent,
                            FamilyThemePaintKind::Transparent,
                        ),
                        (ThemeRouteCutoverValue::Solid, FamilyThemePaintKind::Solid),
                    ] {
                        let rule_facet = match facet {
                            ThemeRouteCutoverFacet::Fill => FamilyThemeRuleFacet::Fill(paint_kind),
                            ThemeRouteCutoverFacet::Stroke => {
                                FamilyThemeRuleFacet::Stroke(paint_kind)
                            }
                        };
                        let selector_shape = FamilyThemeSelectorShape::Static {
                            variant: selector_variant(selector),
                        };
                        if classify_rule_facet(family, target, selector_shape, rule_facet)
                            != FamilyThemeDisposition::TypedAdapter
                            || !legacy_paint_supported(
                                family,
                                target,
                                selector_variant(selector),
                                channel,
                            )
                        {
                            continue;
                        }
                        let projections = legacy_bridge_projections(
                            family, target, facet, selector,
                        )
                        .ok_or_else(|| {
                            ThemeRouteCutoverInventoryError::missing_projections(
                                family, target, selector, facet,
                            )
                        })?;
                        routes.push(ThemeRouteCutoverDescriptor::new(
                            ThemeRouteCutoverId::new(family, target, selector, facet, value),
                            projections,
                        ));
                    }
                }
            }
        }
    }
    routes.sort_unstable();
    Ok(routes)
}

fn legacy_bridge_projections(
    family: DiagramFamilyId,
    target: ThemeTarget,
    facet: ThemeRouteCutoverFacet,
    selector: ThemeRouteCutoverSelector,
) -> Option<ThemeRouteCutoverProjectionSet> {
    match (family, target, facet) {
        (
            DiagramFamilyId::QUADRANT_CHART | DiagramFamilyId::XY_CHART,
            ThemeTarget::Text | ThemeTarget::Title,
            ThemeRouteCutoverFacet::Fill,
        ) => Some(ThemeRouteCutoverProjectionSet::REPLACE_CHART_TEXT_PAINT),
        (
            DiagramFamilyId::QUADRANT_CHART | DiagramFamilyId::XY_CHART,
            ThemeTarget::Axis,
            ThemeRouteCutoverFacet::Fill | ThemeRouteCutoverFacet::Stroke,
        ) => Some(ThemeRouteCutoverProjectionSet::REPLACE_CHART_TEXT_PAINT),
        (DiagramFamilyId::RADAR, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_RADAR_TEXT_PAINT)
        }
        (
            DiagramFamilyId::RADAR,
            ThemeTarget::Axis,
            ThemeRouteCutoverFacet::Fill | ThemeRouteCutoverFacet::Stroke,
        ) => Some(ThemeRouteCutoverProjectionSet::REPLACE_RADAR_AXIS_PAINT),
        (
            DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE | DiagramFamilyId::CLASS,
            ThemeTarget::Title,
            ThemeRouteCutoverFacet::Fill,
        ) => Some(ThemeRouteCutoverProjectionSet::REPLACE_TITLE_FILL),
        (
            DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE,
            ThemeTarget::ClusterLabel,
            ThemeRouteCutoverFacet::Fill,
        ) => Some(ThemeRouteCutoverProjectionSet::REPLACE_CLUSTER_LABEL_FILL),
        (
            DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE,
            ThemeTarget::Text,
            ThemeRouteCutoverFacet::Fill,
        ) => Some(ThemeRouteCutoverProjectionSet::REPLACE_FLOWCHART_TEXT_FILL),
        (DiagramFamilyId::CLASS, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_CLASS_TEXT_FILL)
        }

        (
            DiagramFamilyId::FLOWCHART
            | DiagramFamilyId::SWIMLANE
            | DiagramFamilyId::BLOCK
            | DiagramFamilyId::CLASS
            | DiagramFamilyId::MINDMAP,
            ThemeTarget::Node,
            ThemeRouteCutoverFacet::Fill,
        ) => Some(ThemeRouteCutoverProjectionSet::REPLACE_NODE_FILL),
        (
            DiagramFamilyId::FLOWCHART
            | DiagramFamilyId::SWIMLANE
            | DiagramFamilyId::BLOCK
            | DiagramFamilyId::CLASS
            | DiagramFamilyId::MINDMAP,
            ThemeTarget::Node,
            ThemeRouteCutoverFacet::Stroke,
        ) => Some(ThemeRouteCutoverProjectionSet::REPLACE_NODE_STROKE),
        (
            DiagramFamilyId::BLOCK
            | DiagramFamilyId::FLOWCHART
            | DiagramFamilyId::SWIMLANE
            | DiagramFamilyId::CLASS,
            ThemeTarget::NodeLabel,
            ThemeRouteCutoverFacet::Fill,
        ) => Some(ThemeRouteCutoverProjectionSet::REPLACE_NODE_LABEL_FILL),
        (DiagramFamilyId::BLOCK, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_NODE_LABEL_FILL)
        }
        (
            DiagramFamilyId::TREE_VIEW,
            ThemeTarget::NodeLabel | ThemeTarget::Text,
            ThemeRouteCutoverFacet::Fill,
        ) => Some(ThemeRouteCutoverProjectionSet::REPLACE_NODE_LABEL_FILL),
        (
            DiagramFamilyId::TREE_VIEW,
            ThemeTarget::Edge,
            ThemeRouteCutoverFacet::Fill | ThemeRouteCutoverFacet::Stroke,
        ) => Some(ThemeRouteCutoverProjectionSet::REPLACE_EDGE_STROKE),
        (
            DiagramFamilyId::TREE_VIEW,
            ThemeTarget::Marker,
            ThemeRouteCutoverFacet::Fill | ThemeRouteCutoverFacet::Stroke,
        ) => Some(ThemeRouteCutoverProjectionSet::REPLACE_TREE_VIEW_MARKER_PAINT),
        (
            DiagramFamilyId::BLOCK,
            ThemeTarget::Marker,
            ThemeRouteCutoverFacet::Fill | ThemeRouteCutoverFacet::Stroke,
        ) => Some(ThemeRouteCutoverProjectionSet::REPLACE_MARKER_PAINT),
        (
            DiagramFamilyId::FLOWCHART
            | DiagramFamilyId::SWIMLANE
            | DiagramFamilyId::CLASS
            | DiagramFamilyId::BLOCK,
            ThemeTarget::Edge,
            ThemeRouteCutoverFacet::Stroke,
        ) => Some(ThemeRouteCutoverProjectionSet::REPLACE_EDGE_STROKE_AND_RETIRE_MARKER_FALLBACK),
        (
            DiagramFamilyId::FLOWCHART
            | DiagramFamilyId::SWIMLANE
            | DiagramFamilyId::CLASS
            | DiagramFamilyId::BLOCK,
            ThemeTarget::Edge,
            ThemeRouteCutoverFacet::Fill,
        ) => Some(ThemeRouteCutoverProjectionSet::REPLACE_EDGE_STROKE_AND_RETIRE_MARKER_FALLBACK),
        (
            DiagramFamilyId::MINDMAP | DiagramFamilyId::GIT_GRAPH,
            ThemeTarget::Edge,
            ThemeRouteCutoverFacet::Stroke,
        ) => Some(ThemeRouteCutoverProjectionSet::REPLACE_EDGE_STROKE),
        (DiagramFamilyId::GIT_GRAPH, ThemeTarget::Edge, ThemeRouteCutoverFacet::Fill) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_EDGE_STROKE)
        }
        (DiagramFamilyId::GIT_GRAPH, ThemeTarget::Node, ThemeRouteCutoverFacet::Fill) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_NODE_FILL)
        }
        (DiagramFamilyId::GIT_GRAPH, ThemeTarget::Node, ThemeRouteCutoverFacet::Stroke) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_NODE_STROKE)
        }
        (
            DiagramFamilyId::BLOCK | DiagramFamilyId::FLOWCHART | DiagramFamilyId::SWIMLANE,
            ThemeTarget::EdgeLabelBackground,
            ThemeRouteCutoverFacet::Fill,
        ) => Some(ThemeRouteCutoverProjectionSet::REPLACE_EDGE_LABEL_BACKGROUND_FILL),
        (
            DiagramFamilyId::GIT_GRAPH,
            ThemeTarget::EdgeLabelBackground,
            ThemeRouteCutoverFacet::Fill,
        ) => Some(ThemeRouteCutoverProjectionSet::REPLACE_GITGRAPH_COMMIT_LABEL_BACKGROUND_FILL),
        (DiagramFamilyId::GIT_GRAPH, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_GITGRAPH_TEXT_FILL)
        }
        (DiagramFamilyId::GIT_GRAPH, ThemeTarget::NodeLabel, ThemeRouteCutoverFacet::Fill) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_NODE_LABEL_FILL)
        }
        (DiagramFamilyId::GIT_GRAPH, ThemeTarget::EdgeLabel, ThemeRouteCutoverFacet::Fill) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_EDGE_LABEL_FILL)
        }
        (
            DiagramFamilyId::ER,
            ThemeTarget::Relation,
            ThemeRouteCutoverFacet::Fill | ThemeRouteCutoverFacet::Stroke,
        ) => Some(ThemeRouteCutoverProjectionSet::REPLACE_EDGE_STROKE),
        (DiagramFamilyId::ER, ThemeTarget::Table, ThemeRouteCutoverFacet::Fill) => match selector {
            ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Odd) => {
                Some(ThemeRouteCutoverProjectionSet::REPLACE_ER_TABLE_ODD_FILL)
            }
            ThemeRouteCutoverSelector::StaticVariant(ThemeVariant::Even) => {
                Some(ThemeRouteCutoverProjectionSet::REPLACE_ER_TABLE_EVEN_FILL)
            }
            ThemeRouteCutoverSelector::StaticUnqualified => {
                Some(ThemeRouteCutoverProjectionSet::REPLACE_ER_TABLE_FILL)
            }
            ThemeRouteCutoverSelector::StaticVariant(_) => None,
        },
        (DiagramFamilyId::ER, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_TEXT_FILL)
        }
        (DiagramFamilyId::TREEMAP, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_TEXT_FILL)
        }
        (DiagramFamilyId::C4, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_C4_TEXT_FILL)
        }
        (DiagramFamilyId::INFO, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_TEXT_FILL)
        }
        (DiagramFamilyId::GANTT, ThemeTarget::Task, facet) => {
            gantt_task_projections(facet, selector)
        }
        (DiagramFamilyId::GANTT, ThemeTarget::Title, ThemeRouteCutoverFacet::Fill) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_TITLE_FILL)
        }
        (DiagramFamilyId::GANTT, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_TEXT_FILL)
        }
        (DiagramFamilyId::KANBAN, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_TEXT_FILL)
        }
        (DiagramFamilyId::KANBAN, ThemeTarget::Task, ThemeRouteCutoverFacet::Stroke) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_KANBAN_TASK_STROKE)
        }
        (DiagramFamilyId::TIMELINE, ThemeTarget::TimelineEvent, ThemeRouteCutoverFacet::Fill) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_TIMELINE_EVENT_FILL)
        }
        (DiagramFamilyId::TIMELINE, ThemeTarget::TimelineEvent, ThemeRouteCutoverFacet::Stroke) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_TIMELINE_EVENT_STROKE)
        }
        (DiagramFamilyId::TIMELINE, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_TIMELINE_TEXT_FILL)
        }
        (DiagramFamilyId::JOURNEY, ThemeTarget::JourneyTask, ThemeRouteCutoverFacet::Fill) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_JOURNEY_TASK_FILL)
        }
        (DiagramFamilyId::JOURNEY, ThemeTarget::JourneyTask, ThemeRouteCutoverFacet::Stroke) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_JOURNEY_TASK_STROKE)
        }
        (DiagramFamilyId::JOURNEY, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_TEXT_FILL)
        }
        (
            DiagramFamilyId::FLOWCHART
            | DiagramFamilyId::SWIMLANE
            | DiagramFamilyId::CLASS
            | DiagramFamilyId::BLOCK,
            ThemeTarget::Cluster,
            ThemeRouteCutoverFacet::Fill,
        ) => Some(ThemeRouteCutoverProjectionSet::REPLACE_CLUSTER_FILL),
        (
            DiagramFamilyId::FLOWCHART
            | DiagramFamilyId::SWIMLANE
            | DiagramFamilyId::CLASS
            | DiagramFamilyId::BLOCK,
            ThemeTarget::Cluster,
            ThemeRouteCutoverFacet::Stroke,
        ) => Some(ThemeRouteCutoverProjectionSet::REPLACE_CLUSTER_STROKE),
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Actor, ThemeRouteCutoverFacet::Fill) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_ACTOR_FILL)
        }
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Actor, ThemeRouteCutoverFacet::Stroke) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_ACTOR_STROKE)
        }
        (DiagramFamilyId::SEQUENCE, ThemeTarget::ActorLabel, ThemeRouteCutoverFacet::Fill) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_ACTOR_LABEL_FILL)
        }
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::Lifeline,
            ThemeRouteCutoverFacet::Fill | ThemeRouteCutoverFacet::Stroke,
        ) => Some(ThemeRouteCutoverProjectionSet::REPLACE_LIFELINE_STROKE),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::Message,
            ThemeRouteCutoverFacet::Fill | ThemeRouteCutoverFacet::Stroke,
        ) => Some(ThemeRouteCutoverProjectionSet::REPLACE_MESSAGE_STROKE),
        (DiagramFamilyId::SEQUENCE, ThemeTarget::MessageLabel, ThemeRouteCutoverFacet::Fill) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_MESSAGE_LABEL_FILL)
        }
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::LoopLabelBackground,
            ThemeRouteCutoverFacet::Fill,
        ) => Some(ThemeRouteCutoverProjectionSet::REPLACE_LOOP_FILL),
        (
            DiagramFamilyId::SEQUENCE,
            ThemeTarget::LoopLabelBackground,
            ThemeRouteCutoverFacet::Stroke,
        ) => Some(ThemeRouteCutoverProjectionSet::REPLACE_LOOP_STROKE),
        (DiagramFamilyId::SEQUENCE, ThemeTarget::LoopLabel, ThemeRouteCutoverFacet::Fill) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_LOOP_LABEL_FILL)
        }
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Note, ThemeRouteCutoverFacet::Fill) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_NOTE_FILL)
        }
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Note, ThemeRouteCutoverFacet::Stroke) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_NOTE_STROKE)
        }
        (DiagramFamilyId::SEQUENCE, ThemeTarget::NoteLabel, ThemeRouteCutoverFacet::Fill) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_NOTE_LABEL_FILL)
        }
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Activation, ThemeRouteCutoverFacet::Fill) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_ACTIVATION_FILL)
        }
        (DiagramFamilyId::SEQUENCE, ThemeTarget::Activation, ThemeRouteCutoverFacet::Stroke) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_ACTIVATION_STROKE)
        }
        (
            DiagramFamilyId::RAILROAD
            | DiagramFamilyId::TREEMAP
            | DiagramFamilyId::VENN
            | DiagramFamilyId::ZENUML
            | DiagramFamilyId::RADAR,
            ThemeTarget::Title,
            ThemeRouteCutoverFacet::Fill,
        ) => Some(ThemeRouteCutoverProjectionSet::REPLACE_TITLE_FILL),
        (
            DiagramFamilyId::ARCHITECTURE
            | DiagramFamilyId::CYNEFIN
            | DiagramFamilyId::EVENT_MODELING
            | DiagramFamilyId::ISHIKAWA
            | DiagramFamilyId::VENN,
            ThemeTarget::Text,
            ThemeRouteCutoverFacet::Fill,
        ) => Some(ThemeRouteCutoverProjectionSet::REPLACE_TEXT_FILL),
        (DiagramFamilyId::REQUIREMENT, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_REQUIREMENT_TEXT_FILL)
        }
        (DiagramFamilyId::REQUIREMENT, ThemeTarget::Requirement, ThemeRouteCutoverFacet::Fill) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_REQUIREMENT_FILL)
        }
        (
            DiagramFamilyId::REQUIREMENT,
            ThemeTarget::Requirement,
            ThemeRouteCutoverFacet::Stroke,
        ) => Some(ThemeRouteCutoverProjectionSet::REPLACE_REQUIREMENT_STROKE),
        (
            DiagramFamilyId::REQUIREMENT,
            ThemeTarget::Relation,
            ThemeRouteCutoverFacet::Fill | ThemeRouteCutoverFacet::Stroke,
        ) => Some(ThemeRouteCutoverProjectionSet::REPLACE_REQUIREMENT_RELATION_PAINT),
        (DiagramFamilyId::PIE, ThemeTarget::PieSlice, ThemeRouteCutoverFacet::Fill) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_PIE_SLICE_FILL)
        }
        (DiagramFamilyId::PIE, ThemeTarget::PieSlice, ThemeRouteCutoverFacet::Stroke) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_PIE_SLICE_STROKE)
        }
        (DiagramFamilyId::PIE, ThemeTarget::Title, ThemeRouteCutoverFacet::Fill) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_TITLE_FILL)
        }
        (DiagramFamilyId::PIE, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_TEXT_FILL)
        }
        (DiagramFamilyId::RAILROAD, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_TEXT_FILL)
        }
        (DiagramFamilyId::SANKEY, ThemeTarget::Text, ThemeRouteCutoverFacet::Fill) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_TEXT_FILL)
        }
        _ => None,
    }
}

const fn selector_variant(selector: ThemeRouteCutoverSelector) -> Option<ThemeVariant> {
    match selector {
        ThemeRouteCutoverSelector::StaticUnqualified => None,
        ThemeRouteCutoverSelector::StaticVariant(variant) => Some(variant),
    }
}

fn gantt_task_projections(
    facet: ThemeRouteCutoverFacet,
    selector: ThemeRouteCutoverSelector,
) -> Option<ThemeRouteCutoverProjectionSet> {
    use ThemeRouteCutoverSelector::{StaticUnqualified, StaticVariant};
    match (facet, selector) {
        (ThemeRouteCutoverFacet::Fill, StaticUnqualified) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_GANTT_TASK_FILLS)
        }
        (ThemeRouteCutoverFacet::Stroke, StaticUnqualified) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_GANTT_TASK_STROKES)
        }
        (ThemeRouteCutoverFacet::Fill, StaticVariant(ThemeVariant::Default)) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_GANTT_TASK_DEFAULT_FILL)
        }
        (ThemeRouteCutoverFacet::Fill, StaticVariant(ThemeVariant::Active)) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_GANTT_TASK_ACTIVE_FILL)
        }
        (ThemeRouteCutoverFacet::Fill, StaticVariant(ThemeVariant::Success)) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_GANTT_TASK_SUCCESS_FILL)
        }
        (ThemeRouteCutoverFacet::Fill, StaticVariant(ThemeVariant::Error)) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_GANTT_TASK_ERROR_FILL)
        }
        (ThemeRouteCutoverFacet::Stroke, StaticVariant(ThemeVariant::Default)) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_GANTT_TASK_DEFAULT_STROKE)
        }
        (ThemeRouteCutoverFacet::Stroke, StaticVariant(ThemeVariant::Active)) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_GANTT_TASK_ACTIVE_STROKE)
        }
        (ThemeRouteCutoverFacet::Stroke, StaticVariant(ThemeVariant::Success)) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_GANTT_TASK_SUCCESS_STROKE)
        }
        (ThemeRouteCutoverFacet::Stroke, StaticVariant(ThemeVariant::Error)) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_GANTT_TASK_ERROR_STROKE)
        }
        (ThemeRouteCutoverFacet::Stroke, StaticVariant(ThemeVariant::Warning)) => {
            Some(ThemeRouteCutoverProjectionSet::REPLACE_GANTT_TASK_WARNING_STROKE)
        }
        _ => None,
    }
}

#[cfg(test)]
pub(super) fn for_each_matrix_selector(mut visit: impl FnMut(FamilyThemeSelectorShape)) {
    for variant in std::iter::once(None).chain(ThemeVariant::ALL.iter().copied().map(Some)) {
        visit(FamilyThemeSelectorShape::Static { variant });
        visit(FamilyThemeSelectorShape::Ordinal {
            variant,
            selector: OrdinalSelector::Exact(1),
        });
        visit(FamilyThemeSelectorShape::Ordinal {
            variant,
            selector: OrdinalSelector::Cycle {
                period: 1,
                offset: 0,
            },
        });
    }
}

#[cfg(test)]
pub(super) fn for_each_matrix_rule_facet(mut visit: impl FnMut(FamilyThemeRuleFacet)) {
    for kind in FamilyThemePaintKind::ALL {
        visit(FamilyThemeRuleFacet::Fill(kind));
        visit(FamilyThemeRuleFacet::Stroke(kind));
    }
    for facet in [
        FamilyThemeRuleFacet::StrokeWidth,
        FamilyThemeRuleFacet::StrokeDasharray,
        FamilyThemeRuleFacet::StrokeLinecap,
        FamilyThemeRuleFacet::StrokeLinejoin,
        FamilyThemeRuleFacet::Opacity,
        FamilyThemeRuleFacet::FillOpacity,
        FamilyThemeRuleFacet::StrokeOpacity,
        FamilyThemeRuleFacet::Radius,
        FamilyThemeRuleFacet::Padding,
        FamilyThemeRuleFacet::Effect,
    ] {
        visit(facet);
    }
    for &property in ThemeTypographyProperty::ALL {
        visit(FamilyThemeRuleFacet::Typography(property));
    }
}
