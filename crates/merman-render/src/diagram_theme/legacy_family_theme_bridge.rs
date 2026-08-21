use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

use merman_core::__private::{
    ThemeFamilyCompatibilityOverlay, ThemeFamilyCompatibilityOverlayBuilder,
};
use merman_core::{MermaidConfig, OperationControl, OperationControlResult};
use serde_json::{Map, Value};

use crate::DiagramFamilyId;
use crate::theme_route_cutover::ThemeRouteCutoverProjection;

use super::canvas::CanvasPaint;
use super::family_mechanism_matrix::{
    FamilyThemeDisposition, FamilyThemeRuleFacet, MAX_LEGACY_ASSIGNMENT_STRING_BYTES,
};
use super::family_program::{FamilyThemeProgram, FamilyThemeProgramCache};
use super::resolved::{ResolvedProperty, ResolvedThemeStyle, ThemeTypographyProperty};
use super::semantic::{ThemeTarget, ThemeVariant};
use super::typography::{Specified, TextStyle};

#[cfg(any(test, feature = "internal-theme-acceptance"))]
use super::family_mechanism_matrix::{
    FamilyThemePaintKind, FamilyThemeSelectorShape, classify_rule_facet,
};
#[cfg(any(test, feature = "internal-theme-acceptance"))]
use super::legacy_projection_retirement::{
    ThemeLegacyProjectionDisposition, ThemeLegacyProjectionObservation,
    ThemeLegacyProjectionProbeError, ThemeLegacyProjectionProbeReceipt, ThemeLegacyRouteFacet,
    ThemeLegacyRouteSelector, ThemeLegacyRouteValue, value_digest,
};
#[cfg(any(test, feature = "internal-theme-acceptance"))]
use super::{DiagramThemeSpec, ThemeRule, ThemeRuleSet, ThemeStylePatch};

pub(super) const CONTRIBUTION_ID_PREFIX: &str = "merman.legacy-family-theme.v1.";
const EXPLICIT_MARKER_PAINT_CONTRIBUTION_ID: &str = "marker.paint";

#[cfg(any(test, feature = "internal-theme-acceptance"))]
type AcceptedLegacyProjections = BTreeSet<ThemeLegacyProjectionObservation>;
#[cfg(not(any(test, feature = "internal-theme-acceptance")))]
type AcceptedLegacyProjections = ();

/// Temporary, family-local compatibility inputs for Mermaid renderers that do not yet consume the
/// typed theme program directly.
///
/// This bridge is deliberately lossy. Its provenance must be reported as legacy compatibility,
/// never as evidence that a typed mechanism was applied.
#[derive(Debug, Clone)]
pub(super) struct LegacyFamilyThemeBridge {
    inner: Arc<LegacyFamilyThemeBridgeInner>,
}

#[derive(Debug)]
struct LegacyFamilyThemeBridgeInner {
    family_programs: Arc<FamilyThemeProgramCache>,
    artifacts: Mutex<HashMap<DiagramFamilyId, Arc<LegacyFamilyThemeArtifact>>>,
}

#[derive(Debug)]
struct LegacyFamilyThemeArtifact {
    overlay: ThemeFamilyCompatibilityOverlay,
    #[cfg(any(test, feature = "internal-theme-acceptance"))]
    contribution_ids: BTreeSet<String>,
    #[cfg(any(test, feature = "internal-theme-acceptance"))]
    accepted_projections: BTreeSet<ThemeLegacyProjectionObservation>,
}

impl LegacyFamilyThemeBridge {
    pub(super) fn new(family_programs: Arc<FamilyThemeProgramCache>) -> Self {
        Self {
            inner: Arc::new(LegacyFamilyThemeBridgeInner {
                family_programs,
                artifacts: Mutex::new(HashMap::new()),
            }),
        }
    }

    fn artifact_for_family(&self, family: DiagramFamilyId) -> Arc<LegacyFamilyThemeArtifact> {
        let mut artifacts = self
            .inner
            .artifacts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        artifacts
            .entry(family)
            .or_insert_with(|| {
                Arc::new(compile_selected_family(&self.inner.family_programs, family))
            })
            .clone()
    }

    #[cfg(test)]
    fn compile_for_family(&self, family: DiagramFamilyId) -> Arc<LegacyFamilyThemeArtifact> {
        self.artifact_for_family(family)
    }

    #[cfg(test)]
    fn cached_family_count(&self) -> usize {
        self.inner
            .artifacts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
    }

    #[cfg(test)]
    fn is_empty(&self) -> bool {
        self.cached_family_count() == 0
    }

    #[cfg(test)]
    pub(super) fn owns_contribution_id(&self, opaque_id: &str) -> bool {
        let Some(family) = contribution_family(opaque_id) else {
            return false;
        };
        self.artifact_for_family(family)
            .contribution_ids
            .contains(opaque_id)
    }

    pub(super) fn overlay_for_family(
        &self,
        family: &str,
        control: &OperationControl,
    ) -> OperationControlResult<Option<ThemeFamilyCompatibilityOverlay>> {
        control.checkpoint()?;
        let Some(family) = DiagramFamilyId::from_id(family) else {
            return Ok(None);
        };
        // State owns its compatibility path in the typed adapter and must never be projected
        // through the legacy Mermaid lane.
        if family == DiagramFamilyId::STATE {
            return Ok(None);
        }
        let artifact = self.artifact_for_family(family);
        control.checkpoint()?;
        Ok((!artifact.overlay.is_empty()).then(|| artifact.overlay.clone()))
    }
}

#[cfg(test)]
fn contribution_family(opaque_id: &str) -> Option<DiagramFamilyId> {
    let family = opaque_id
        .strip_prefix(CONTRIBUTION_ID_PREFIX)?
        .split_once('.')?
        .0;
    DiagramFamilyId::from_id(family)
}

fn compile_selected_family(
    family_programs: &FamilyThemeProgramCache,
    family: DiagramFamilyId,
) -> LegacyFamilyThemeArtifact {
    let mut builder = OverlayBuilder::new(family);
    match family {
        DiagramFamilyId::FLOWCHART
        | DiagramFamilyId::SWIMLANE
        | DiagramFamilyId::CLASS
        | DiagramFamilyId::MINDMAP
        | DiagramFamilyId::TREE_VIEW
        | DiagramFamilyId::BLOCK => {
            compile_node_family(&mut builder, family_programs, family);
        }
        DiagramFamilyId::GIT_GRAPH => {
            compile_gitgraph_family(&mut builder, family_programs);
        }
        DiagramFamilyId::SEQUENCE => {
            compile_sequence_family(&mut builder, family_programs);
        }
        DiagramFamilyId::GANTT | DiagramFamilyId::KANBAN => {
            compile_task_family(&mut builder, family_programs, family);
        }
        DiagramFamilyId::REQUIREMENT => {
            compile_requirement_family(&mut builder, family_programs);
        }
        DiagramFamilyId::ER => {
            compile_er_family(&mut builder, family_programs);
        }
        DiagramFamilyId::PIE => {
            compile_pie_family(&mut builder, family_programs);
        }
        DiagramFamilyId::XY_CHART | DiagramFamilyId::QUADRANT_CHART | DiagramFamilyId::RADAR => {
            compile_chart_family(&mut builder, family_programs, family);
        }
        DiagramFamilyId::TIMELINE => {
            compile_timeline_family(&mut builder, family_programs);
        }
        DiagramFamilyId::JOURNEY => {
            compile_journey_family(&mut builder, family_programs);
        }
        DiagramFamilyId::ERROR
        | DiagramFamilyId::ZENUML
        | DiagramFamilyId::ARCHITECTURE
        | DiagramFamilyId::C4
        | DiagramFamilyId::CYNEFIN
        | DiagramFamilyId::WARDLEY
        | DiagramFamilyId::RAILROAD
        | DiagramFamilyId::PACKET
        | DiagramFamilyId::SANKEY
        | DiagramFamilyId::INFO
        | DiagramFamilyId::TREEMAP
        | DiagramFamilyId::ISHIKAWA
        | DiagramFamilyId::EVENT_MODELING
        | DiagramFamilyId::VENN => {
            compile_text_family(&mut builder, family_programs, family);
        }
        DiagramFamilyId::STATE => {}
        _ => {}
    }
    let (overlay, contribution_ids, accepted_projections) = builder.finish();
    #[cfg(not(any(test, feature = "internal-theme-acceptance")))]
    let _ = (&contribution_ids, accepted_projections);
    LegacyFamilyThemeArtifact {
        overlay,
        #[cfg(any(test, feature = "internal-theme-acceptance"))]
        contribution_ids,
        #[cfg(any(test, feature = "internal-theme-acceptance"))]
        accepted_projections,
    }
}

/// Observes the current matrix disposition and exact compatibility assignments for one route.
///
/// Historical before/after policy deliberately lives in the independent acceptance crate. This
/// function reports current production facts only, so adding a historical manifest row cannot
/// change the runtime matrix or suppress a bridge projection.
#[cfg(any(test, feature = "internal-theme-acceptance"))]
pub(crate) fn legacy_projection_probe(
    id: super::legacy_projection_retirement::ThemeLegacyRouteId,
    value: ThemeLegacyRouteValue,
) -> Result<ThemeLegacyProjectionProbeReceipt, ThemeLegacyProjectionProbeError> {
    let selector = FamilyThemeSelectorShape::Static {
        variant: id.selector().variant(),
    };
    let paint_kind = match value {
        ThemeLegacyRouteValue::Transparent => FamilyThemePaintKind::Transparent,
        ThemeLegacyRouteValue::Solid => FamilyThemePaintKind::Solid,
    };
    let facet = match id.facet() {
        ThemeLegacyRouteFacet::Fill => FamilyThemeRuleFacet::Fill(paint_kind),
        ThemeLegacyRouteFacet::Stroke => FamilyThemeRuleFacet::Stroke(paint_kind),
    };

    let disposition = classify_rule_facet(id.family_id(), id.target(), selector, facet);
    let observed_disposition = match disposition {
        FamilyThemeDisposition::TypedAdapter => ThemeLegacyProjectionDisposition::TypedAdapter,
        FamilyThemeDisposition::LegacyCompatibility => {
            ThemeLegacyProjectionDisposition::LegacyCompatibility
        }
        FamilyThemeDisposition::Unsupported => ThemeLegacyProjectionDisposition::Unsupported,
    };

    let paint = match value {
        ThemeLegacyRouteValue::Transparent => CanvasPaint::Transparent,
        ThemeLegacyRouteValue::Solid => CanvasPaint::solid("#123456").map_err(|error| {
            ThemeLegacyProjectionProbeError::new(
                id,
                value,
                format!("solid probe construction failed: {error}"),
            )
        })?,
    };
    let style = match id.facet() {
        ThemeLegacyRouteFacet::Fill => ThemeStylePatch::default().with_fill(paint),
        ThemeLegacyRouteFacet::Stroke => ThemeStylePatch::default().with_stroke(paint),
    };
    let mut rule = ThemeRule::new(id.target(), style).for_family(id.family_id());
    if let ThemeLegacyRouteSelector::StaticVariant(variant) = id.selector() {
        rule = rule.with_variant(variant);
    }
    let spec = DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(rule));
    spec.validate().map_err(|error| {
        ThemeLegacyProjectionProbeError::new(
            id,
            value,
            format!("synthetic route is invalid: {error}"),
        )
    })?;

    let family_programs = FamilyThemeProgramCache::new(Arc::new(spec));
    let artifact = compile_selected_family(&family_programs, id.family_id());
    if artifact.overlay.is_empty() != artifact.accepted_projections.is_empty() {
        return Err(ThemeLegacyProjectionProbeError::new(
            id,
            value,
            "bridge overlay and exact projection observations disagree",
        ));
    }
    ThemeLegacyProjectionProbeReceipt::seal(
        id,
        value,
        observed_disposition,
        artifact.accepted_projections.into_iter().collect(),
    )
}

fn compile_node_family(
    builder: &mut OverlayBuilder,
    family_programs: &FamilyThemeProgramCache,
    family: DiagramFamilyId,
) {
    let reader = FamilyStyleReader::new(family_programs, family);
    let mut contributions = FamilyContributions::new(family);

    contributions.add_typography(&reader);
    contributions.add_theme_variables(
        ThemeRouteCutoverProjection::NodeFill.contribution_id(),
        [
            ("primaryColor", reader.fill(ThemeTarget::Node)),
            ("mainBkg", reader.fill(ThemeTarget::Node)),
        ],
    );
    contributions.add_theme_variables(
        ThemeRouteCutoverProjection::NodeStroke.contribution_id(),
        [
            ("primaryBorderColor", reader.stroke(ThemeTarget::Node)),
            ("nodeBorder", reader.stroke(ThemeTarget::Node)),
        ],
    );
    let node_label_fill = reader.text_fill(ThemeTarget::NodeLabel);
    if family == DiagramFamilyId::CLASS {
        contributions.add_theme_variables(
            ThemeRouteCutoverProjection::NodeLabelFill.contribution_id(),
            [
                ("primaryTextColor", node_label_fill.clone()),
                ("nodeTextColor", node_label_fill.clone()),
                ("textColor", node_label_fill.clone()),
                ("classText", node_label_fill.clone()),
                ("labelColor", node_label_fill),
            ],
        );
    } else {
        contributions.add_theme_variables(
            ThemeRouteCutoverProjection::NodeLabelFill.contribution_id(),
            [
                ("primaryTextColor", node_label_fill.clone()),
                ("nodeTextColor", node_label_fill.clone()),
                ("textColor", node_label_fill),
            ],
        );
    }
    contributions.add_theme_variables(
        "title.fill",
        [("titleColor", reader.text_fill(ThemeTarget::Title))],
    );
    let edge_paint = reader.stroke_or_fill(ThemeTarget::Edge);
    if family == DiagramFamilyId::GIT_GRAPH {
        contributions.add_theme_variables(
            ThemeRouteCutoverProjection::EdgeStroke.contribution_id(),
            [
                ("lineColor", edge_paint.clone()),
                ("commitLineColor", edge_paint),
            ],
        );
    } else {
        contributions.add_theme_variables(
            ThemeRouteCutoverProjection::EdgeStroke.contribution_id(),
            [("lineColor", edge_paint)],
        );
    }
    let marker_paint = reader.marker_paint_contribution();
    contributions.add_theme_variables(
        marker_paint.contribution_id,
        [("arrowheadColor", marker_paint.value)],
    );
    contributions.add_theme_variables(
        "edge-label-background.fill",
        [(
            "edgeLabelBackground",
            reader.fill(ThemeTarget::EdgeLabelBackground),
        )],
    );
    contributions.add_theme_variables(
        ThemeRouteCutoverProjection::ClusterFill.contribution_id(),
        [
            ("clusterBkg", reader.fill(ThemeTarget::Cluster)),
            ("secondaryColor", reader.fill(ThemeTarget::Cluster)),
        ],
    );
    contributions.add_theme_variables(
        ThemeRouteCutoverProjection::ClusterStroke.contribution_id(),
        [("clusterBorder", reader.stroke(ThemeTarget::Cluster))],
    );
    contributions.add_theme_variables(
        "cluster-label.fill",
        [
            (
                "secondaryTextColor",
                reader.text_fill(ThemeTarget::ClusterLabel),
            ),
            (
                "tertiaryTextColor",
                reader.text_fill(ThemeTarget::ClusterLabel),
            ),
        ],
    );

    match family {
        DiagramFamilyId::TREE_VIEW => {
            let mut tree_view = Map::new();
            if let Some(value) = reader.text_fill(ThemeTarget::NodeLabel) {
                tree_view.insert("labelColor".to_string(), Value::String(value));
            }
            if let Some(value) = reader.stroke_or_fill(ThemeTarget::Edge) {
                tree_view.insert("lineColor".to_string(), Value::String(value));
            }
            if let Some(value) = reader.stroke_or_fill(ThemeTarget::Marker) {
                tree_view.insert("iconColor".to_string(), Value::String(value));
            }
            contributions.add_theme_variable_object("tree-view", "treeView", tree_view);
        }
        DiagramFamilyId::GIT_GRAPH => {
            contributions.add_theme_variables(
                "git.commit",
                [
                    ("commitLabelColor", reader.text_fill(ThemeTarget::EdgeLabel)),
                    (
                        "commitLabelBackground",
                        reader.fill(ThemeTarget::EdgeLabelBackground),
                    ),
                    ("tagLabelColor", reader.text_fill(ThemeTarget::NodeLabel)),
                    ("tagLabelBackground", reader.fill(ThemeTarget::Node)),
                    ("tagLabelBorder", reader.stroke(ThemeTarget::Node)),
                ],
            );
        }
        _ => {}
    }

    contributions.finish_into(builder);
}

/// Projects only GitGraph variables that its terminal writer actually consumes.
///
/// GitGraph arrows are node-palette surfaces, not SVG markers, and its title inherits
/// `textColor`. Keeping this mapping family-local prevents the generic node bridge from
/// resurrecting terminal-less marker, cluster, and title projections.
fn compile_gitgraph_family(
    builder: &mut OverlayBuilder,
    family_programs: &FamilyThemeProgramCache,
) {
    let family = DiagramFamilyId::GIT_GRAPH;
    let reader = FamilyStyleReader::new(family_programs, family);
    let mut contributions = FamilyContributions::new(family);

    contributions.add_typography(&reader);
    let node_fill = reader.fill(ThemeTarget::Node);
    contributions.add_theme_variables(
        ThemeRouteCutoverProjection::NodeFill.contribution_id(),
        [
            ("primaryColor", node_fill.clone()),
            ("mainBkg", node_fill.clone()),
            ("tagLabelBackground", node_fill),
        ],
    );
    let node_stroke = reader.stroke(ThemeTarget::Node);
    contributions.add_theme_variables(
        ThemeRouteCutoverProjection::NodeStroke.contribution_id(),
        [
            ("primaryBorderColor", node_stroke.clone()),
            ("nodeBorder", node_stroke.clone()),
            ("tagLabelBorder", node_stroke),
        ],
    );
    contributions.add_theme_variables(
        ThemeRouteCutoverProjection::TextFill.contribution_id(),
        [("textColor", reader.text_fill(ThemeTarget::Text))],
    );
    contributions.add_theme_variables(
        ThemeRouteCutoverProjection::NodeLabelFill.contribution_id(),
        [("tagLabelColor", reader.text_fill(ThemeTarget::NodeLabel))],
    );
    contributions.add_theme_variables(
        ThemeRouteCutoverProjection::EdgeStroke.contribution_id(),
        [("commitLineColor", reader.stroke_or_fill(ThemeTarget::Edge))],
    );
    contributions.add_theme_variables(
        "edge-label.fill",
        [("commitLabelColor", reader.text_fill(ThemeTarget::EdgeLabel))],
    );
    contributions.add_theme_variables(
        "edge-label-background.fill",
        [(
            "commitLabelBackground",
            reader.fill(ThemeTarget::EdgeLabelBackground),
        )],
    );

    contributions.finish_into(builder);
}

fn compile_sequence_family(
    builder: &mut OverlayBuilder,
    family_programs: &FamilyThemeProgramCache,
) {
    let family = DiagramFamilyId::SEQUENCE;
    let reader = FamilyStyleReader::new(family_programs, family);
    let mut contributions = FamilyContributions::new(family);

    contributions.add_typography(&reader);
    contributions.add_theme_variables(
        ThemeRouteCutoverProjection::ActorFill.contribution_id(),
        [("actorBkg", reader.fill(ThemeTarget::Actor))],
    );
    contributions.add_theme_variables(
        ThemeRouteCutoverProjection::ActorStroke.contribution_id(),
        [("actorBorder", reader.stroke(ThemeTarget::Actor))],
    );
    contributions.add_theme_variables(
        ThemeRouteCutoverProjection::ActorLabelFill.contribution_id(),
        [("actorTextColor", reader.text_fill(ThemeTarget::ActorLabel))],
    );
    contributions.add_theme_variables(
        ThemeRouteCutoverProjection::LifelineStroke.contribution_id(),
        [(
            "actorLineColor",
            reader.stroke_or_fill(ThemeTarget::Lifeline),
        )],
    );
    contributions.add_theme_variables(
        ThemeRouteCutoverProjection::MessageStroke.contribution_id(),
        [("signalColor", reader.stroke_or_fill(ThemeTarget::Message))],
    );
    contributions.add_theme_variables(
        ThemeRouteCutoverProjection::MessageLabelFill.contribution_id(),
        [(
            "signalTextColor",
            reader.text_fill(ThemeTarget::MessageLabel),
        )],
    );
    contributions.add_theme_variables(
        ThemeRouteCutoverProjection::LoopFill.contribution_id(),
        [("labelBoxBkgColor", reader.fill(ThemeTarget::Loop))],
    );
    contributions.add_theme_variables(
        ThemeRouteCutoverProjection::LoopStroke.contribution_id(),
        [("labelBoxBorderColor", reader.stroke(ThemeTarget::Loop))],
    );
    contributions.add_theme_variables(
        ThemeRouteCutoverProjection::LoopLabelFill.contribution_id(),
        [
            ("labelTextColor", reader.text_fill(ThemeTarget::LoopLabel)),
            ("loopTextColor", reader.text_fill(ThemeTarget::LoopLabel)),
        ],
    );
    contributions.add_theme_variables(
        ThemeRouteCutoverProjection::ActivationFill.contribution_id(),
        [("activationBkgColor", reader.fill(ThemeTarget::Activation))],
    );
    contributions.add_theme_variables(
        ThemeRouteCutoverProjection::ActivationStroke.contribution_id(),
        [(
            "activationBorderColor",
            reader.stroke(ThemeTarget::Activation),
        )],
    );
    contributions.add_theme_variables(
        ThemeRouteCutoverProjection::NoteFill.contribution_id(),
        [("noteBkgColor", reader.fill(ThemeTarget::Note))],
    );
    contributions.add_theme_variables(
        ThemeRouteCutoverProjection::NoteStroke.contribution_id(),
        [("noteBorderColor", reader.stroke(ThemeTarget::Note))],
    );
    contributions.add_theme_variables(
        ThemeRouteCutoverProjection::NoteLabelFill.contribution_id(),
        [("noteTextColor", reader.text_fill(ThemeTarget::NoteLabel))],
    );
    contributions.add_theme_variables(
        "title.fill",
        [("titleColor", reader.text_fill(ThemeTarget::Title))],
    );

    contributions.finish_into(builder);
}

fn compile_task_family(
    builder: &mut OverlayBuilder,
    family_programs: &FamilyThemeProgramCache,
    family: DiagramFamilyId,
) {
    let reader = FamilyStyleReader::new(family_programs, family);
    let mut contributions = FamilyContributions::new(family);

    contributions.add_typography(&reader);
    contributions.add_theme_variables(
        "text.fill",
        [
            ("textColor", reader.text_fill(ThemeTarget::Text)),
            ("taskTextColor", reader.text_fill(ThemeTarget::Text)),
        ],
    );
    contributions.add_theme_variables(
        "title.fill",
        [("titleColor", reader.text_fill(ThemeTarget::Title))],
    );

    match family {
        DiagramFamilyId::GANTT => {
            for (stroke_mapping, fill_projection, variant, fill_key, stroke_key) in [
                (
                    "task.default",
                    ThemeRouteCutoverProjection::GanttTaskDefaultFill,
                    ThemeVariant::Default,
                    "taskBkgColor",
                    "taskBorderColor",
                ),
                (
                    "task.active",
                    ThemeRouteCutoverProjection::GanttTaskActiveFill,
                    ThemeVariant::Active,
                    "activeTaskBkgColor",
                    "activeTaskBorderColor",
                ),
                (
                    "task.success",
                    ThemeRouteCutoverProjection::GanttTaskSuccessFill,
                    ThemeVariant::Success,
                    "doneTaskBkgColor",
                    "doneTaskBorderColor",
                ),
                (
                    "task.error",
                    ThemeRouteCutoverProjection::GanttTaskErrorFill,
                    ThemeVariant::Error,
                    "critBkgColor",
                    "critBorderColor",
                ),
            ] {
                contributions.add_theme_variables(
                    fill_projection.contribution_id(),
                    [(fill_key, reader.fill_variant(ThemeTarget::Task, variant))],
                );
                contributions.add_theme_variables(
                    stroke_mapping,
                    [(
                        stroke_key,
                        reader.stroke_variant(ThemeTarget::Task, variant),
                    )],
                );
            }
            contributions.add_theme_variables(
                "task.warning",
                [
                    (
                        "todayLineColor",
                        reader.stroke_variant(ThemeTarget::Task, ThemeVariant::Warning),
                    ),
                    (
                        "vertLineColor",
                        reader.stroke_variant(ThemeTarget::Task, ThemeVariant::Warning),
                    ),
                ],
            );
        }
        DiagramFamilyId::KANBAN => {
            contributions.add_theme_variables(
                "task.default",
                [("nodeBorder", reader.stroke(ThemeTarget::Task))],
            );
        }
        _ => unreachable!("task compatibility is limited to Gantt and Kanban"),
    }

    contributions.finish_into(builder);
}

fn compile_requirement_family(
    builder: &mut OverlayBuilder,
    family_programs: &FamilyThemeProgramCache,
) {
    let family = DiagramFamilyId::REQUIREMENT;
    let reader = FamilyStyleReader::new(family_programs, family);
    let mut contributions = FamilyContributions::new(family);

    contributions.add_typography(&reader);
    contributions.add_theme_variables(
        ThemeRouteCutoverProjection::RequirementFill.contribution_id(),
        [(
            "requirementBackground",
            reader.fill(ThemeTarget::Requirement),
        )],
    );
    contributions.add_theme_variables(
        "requirement.paint",
        [
            (
                "requirementBorderColor",
                reader.stroke(ThemeTarget::Requirement),
            ),
            ("nodeBorder", reader.stroke(ThemeTarget::Requirement)),
        ],
    );
    contributions.add_theme_variables(
        "requirement.text",
        [
            ("requirementTextColor", reader.text_fill(ThemeTarget::Text)),
            ("nodeTextColor", reader.text_fill(ThemeTarget::Text)),
            ("relationLabelColor", reader.text_fill(ThemeTarget::Text)),
        ],
    );
    contributions.add_theme_variables(
        "relation.paint",
        [
            (
                "relationColor",
                reader.stroke_or_fill(ThemeTarget::Relation),
            ),
            ("lineColor", reader.stroke_or_fill(ThemeTarget::Relation)),
        ],
    );
    contributions.add_theme_variables(
        "table.odd.fill",
        [(
            "rowOdd",
            reader.fill_variant(ThemeTarget::Table, ThemeVariant::Odd),
        )],
    );
    contributions.add_theme_variables(
        "table.even.fill",
        [(
            "rowEven",
            reader.fill_variant(ThemeTarget::Table, ThemeVariant::Even),
        )],
    );
    contributions.finish_into(builder);
}

fn compile_er_family(builder: &mut OverlayBuilder, family_programs: &FamilyThemeProgramCache) {
    let family = DiagramFamilyId::ER;
    let reader = FamilyStyleReader::new(family_programs, family);
    let mut contributions = FamilyContributions::new(family);

    contributions.add_typography(&reader);
    let text_fill = reader.text_fill(ThemeTarget::Text);
    contributions.add_theme_variables(
        ThemeRouteCutoverProjection::TextFill.contribution_id(),
        [
            ("textColor", text_fill.clone()),
            ("nodeTextColor", text_fill),
        ],
    );
    contributions.add_theme_variables(
        ThemeRouteCutoverProjection::TitleFill.contribution_id(),
        [("titleColor", reader.text_fill(ThemeTarget::Title))],
    );
    contributions.add_theme_variables(
        ThemeRouteCutoverProjection::EdgeStroke.contribution_id(),
        [("lineColor", reader.stroke_or_fill(ThemeTarget::Relation))],
    );
    contributions.add_theme_variables(
        "table.odd.fill",
        [(
            "rowOdd",
            reader.fill_variant(ThemeTarget::Table, ThemeVariant::Odd),
        )],
    );
    contributions.add_theme_variables(
        "table.even.fill",
        [(
            "rowEven",
            reader.fill_variant(ThemeTarget::Table, ThemeVariant::Even),
        )],
    );
    contributions.finish_into(builder);
}

fn compile_pie_family(builder: &mut OverlayBuilder, family_programs: &FamilyThemeProgramCache) {
    let family = DiagramFamilyId::PIE;
    let reader = FamilyStyleReader::new(family_programs, family);
    let mut contributions = FamilyContributions::new(family);

    contributions.add_typography(&reader);
    contributions.add_theme_variables(
        "text.fill",
        [
            ("pieTitleTextColor", reader.text_fill(ThemeTarget::Title)),
            ("pieSectionTextColor", reader.text_fill(ThemeTarget::Text)),
        ],
    );
    contributions.add_theme_variables(
        "slice.stroke",
        [
            ("pieStrokeColor", reader.stroke(ThemeTarget::PieSlice)),
            ("pieOuterStrokeColor", reader.stroke(ThemeTarget::PieSlice)),
        ],
    );
    contributions.add_palette(
        "slice.fill",
        reader
            .fill(ThemeTarget::PieSlice)
            .map(|color| vec![color; 12]),
        PaletteProjection::Pie { limit: 12 },
    );
    if !reader.has_typed_ordinal_palette(ThemeTarget::PieSlice) {
        contributions.add_palette(
            "slice.palette",
            reader.palette(ThemeTarget::PieSlice),
            PaletteProjection::Pie { limit: 12 },
        );
    }
    contributions.finish_into(builder);
}

fn compile_chart_family(
    builder: &mut OverlayBuilder,
    family_programs: &FamilyThemeProgramCache,
    family: DiagramFamilyId,
) {
    let reader = FamilyStyleReader::new(family_programs, family);
    let mut contributions = FamilyContributions::new(family);
    let text = reader.text_fill(ThemeTarget::Text);
    let title = reader.text_fill(ThemeTarget::Title);
    let axis_text = reader.text_fill(ThemeTarget::Axis);
    let axis_line = reader.stroke_or_fill(ThemeTarget::Axis);
    contributions.add_typography(&reader);
    match family {
        DiagramFamilyId::XY_CHART => {
            let mut xy = Map::new();
            for (key, value) in [
                ("titleColor", title),
                ("dataLabelColor", text),
                ("xAxisTitleColor", axis_text.clone()),
                ("xAxisLabelColor", axis_text.clone()),
                ("xAxisTickColor", axis_line.clone()),
                ("xAxisLineColor", axis_line.clone()),
                ("yAxisTitleColor", axis_text.clone()),
                ("yAxisLabelColor", axis_text),
                ("yAxisTickColor", axis_line.clone()),
                ("yAxisLineColor", axis_line),
            ] {
                if let Some(value) = value {
                    xy.insert(key.to_string(), Value::String(value));
                }
            }
            contributions.add_theme_variable_object("chart.text-axis", "xyChart", xy);
        }
        DiagramFamilyId::QUADRANT_CHART => {
            contributions.add_theme_variables(
                "chart.text-axis",
                [
                    ("quadrantTitleFill", title),
                    ("quadrantPointTextFill", text),
                    ("quadrantXAxisTextFill", axis_text.clone()),
                    ("quadrantYAxisTextFill", axis_text),
                    ("quadrantExternalBorderStrokeFill", axis_line.clone()),
                    ("quadrantInternalBorderStrokeFill", axis_line),
                ],
            );
        }
        DiagramFamilyId::RADAR => {
            contributions.add_theme_variables(
                "chart.text",
                [
                    ("textColor", text),
                    ("titleColor", title),
                    ("lineColor", axis_line.clone()),
                ],
            );
            let mut radar = Map::new();
            if let Some(axis_line) = axis_line {
                radar.insert("axisColor".to_string(), Value::String(axis_line));
            }
            contributions.add_root_object("chart.axis", "radar", radar);
        }
        _ => unreachable!("chart compatibility is limited to XY, Quadrant, and Radar"),
    }
    contributions.finish_into(builder);
}

fn compile_timeline_family(
    builder: &mut OverlayBuilder,
    family_programs: &FamilyThemeProgramCache,
) {
    let family = DiagramFamilyId::TIMELINE;
    let reader = FamilyStyleReader::new(family_programs, family);
    let mut contributions = FamilyContributions::new(family);
    contributions.add_typography(&reader);
    contributions.add_theme_variables(
        "event.paint-text",
        [
            ("mainBkg", reader.fill(ThemeTarget::TimelineEvent)),
            ("nodeBorder", reader.stroke(ThemeTarget::TimelineEvent)),
            ("textColor", reader.text_fill(ThemeTarget::Text)),
            ("titleColor", reader.text_fill(ThemeTarget::Title)),
        ],
    );
    contributions.add_palette(
        "event.palette",
        reader.palette(ThemeTarget::TimelineEvent),
        PaletteProjection::ColorScale { limit: 12 },
    );
    contributions.finish_into(builder);
}

fn compile_journey_family(builder: &mut OverlayBuilder, family_programs: &FamilyThemeProgramCache) {
    let family = DiagramFamilyId::JOURNEY;
    let reader = FamilyStyleReader::new(family_programs, family);
    let mut contributions = FamilyContributions::new(family);
    contributions.add_typography(&reader);
    contributions.add_theme_variables(
        "task.paint-text",
        [
            ("mainBkg", reader.fill(ThemeTarget::JourneyTask)),
            ("nodeBorder", reader.stroke(ThemeTarget::JourneyTask)),
            ("textColor", reader.text_fill(ThemeTarget::Text)),
            ("titleColor", reader.text_fill(ThemeTarget::Title)),
        ],
    );
    contributions.add_palette(
        "task.palette",
        reader.palette(ThemeTarget::JourneyTask),
        PaletteProjection::Journey { task_limit: 8 },
    );
    contributions.finish_into(builder);
}

fn compile_text_family(
    builder: &mut OverlayBuilder,
    family_programs: &FamilyThemeProgramCache,
    family: DiagramFamilyId,
) {
    // Text and Title are only candidate compatibility mappings here. The family matrix filters
    // terminal-less routes, such as Info Title and Error Text/Title, before any contribution is
    // created. Renderer-specific roles remain unsupported until the public model types them.
    let reader = FamilyStyleReader::new(family_programs, family);
    let mut contributions = FamilyContributions::new(family);
    contributions.add_typography(&reader);
    // Packet owns its Text and Title terminal roles directly. Avoid reintroducing competing
    // global compatibility variables after that family-local ownership boundary.
    if family != DiagramFamilyId::PACKET {
        contributions.add_theme_variables(
            ThemeRouteCutoverProjection::TextFill.contribution_id(),
            [("textColor", reader.text_fill(ThemeTarget::Text))],
        );
        contributions.add_theme_variables(
            ThemeRouteCutoverProjection::TitleFill.contribution_id(),
            [("titleColor", reader.text_fill(ThemeTarget::Title))],
        );
    }
    contributions.finish_into(builder);
}

struct FamilyStyleReader {
    family: DiagramFamilyId,
    program: Arc<FamilyThemeProgram>,
}

impl FamilyStyleReader {
    fn new(family_programs: &FamilyThemeProgramCache, family: DiagramFamilyId) -> Self {
        Self {
            family,
            program: family_programs.get_or_compile(family),
        }
    }

    fn base_typography(&self) -> &TextStyle {
        self.program.base_typography()
    }

    fn has_legacy_base_typography(&self, property: ThemeTypographyProperty) -> bool {
        self.program.has_legacy_base_typography(property)
    }

    fn style(&self, target: ThemeTarget) -> ResolvedThemeStyle {
        self.style_variant(target, ThemeVariant::Default)
    }

    fn style_variant(&self, target: ThemeTarget, variant: ThemeVariant) -> ResolvedThemeStyle {
        self.program.resolve_style(target, variant, None)
    }

    fn text_style(&self, target: ThemeTarget) -> ResolvedThemeStyle {
        self.program
            .resolve_text_style(target, ThemeVariant::Default, None)
    }

    fn fill(&self, target: ThemeTarget) -> Option<String> {
        self.fill_resolution(target).into_value()
    }

    fn fill_variant(&self, target: ThemeTarget, variant: ThemeVariant) -> Option<String> {
        self.paint_resolution(
            self.style_variant(target, variant).fill_resolution(),
            FamilyThemeRuleFacet::fill,
        )
        .into_value()
    }

    fn stroke(&self, target: ThemeTarget) -> Option<String> {
        self.stroke_resolution(target).into_value()
    }

    fn stroke_variant(&self, target: ThemeTarget, variant: ThemeVariant) -> Option<String> {
        self.paint_resolution(
            self.style_variant(target, variant).stroke_resolution(),
            FamilyThemeRuleFacet::stroke,
        )
        .into_value()
    }

    fn stroke_or_fill(&self, target: ThemeTarget) -> Option<String> {
        self.stroke_or_fill_resolution(target).into_value()
    }

    fn marker_paint_contribution(&self) -> MarkerPaintContribution {
        match self.stroke_or_fill_resolution(ThemeTarget::Marker) {
            LegacyPaintResolution::Unspecified => MarkerPaintContribution {
                contribution_id: ThemeRouteCutoverProjection::MarkerPaintFromEdge.contribution_id(),
                value: self
                    .stroke_or_fill_resolution(ThemeTarget::Edge)
                    .into_value(),
            },
            LegacyPaintResolution::Unsupported if self.family == DiagramFamilyId::CLASS => {
                MarkerPaintContribution {
                    contribution_id: ThemeRouteCutoverProjection::MarkerPaintFromEdge
                        .contribution_id(),
                    value: self
                        .stroke_or_fill_resolution(ThemeTarget::Edge)
                        .into_value(),
                }
            }
            marker => MarkerPaintContribution {
                contribution_id: EXPLICIT_MARKER_PAINT_CONTRIBUTION_ID,
                value: marker.into_value(),
            },
        }
    }

    fn text_fill(&self, target: ThemeTarget) -> Option<String> {
        self.paint_resolution(
            self.text_style(target).fill_resolution(),
            FamilyThemeRuleFacet::fill,
        )
        .into_value()
    }

    fn fill_resolution(&self, target: ThemeTarget) -> LegacyPaintResolution {
        self.paint_resolution(
            self.style(target).fill_resolution(),
            FamilyThemeRuleFacet::fill,
        )
    }

    fn stroke_resolution(&self, target: ThemeTarget) -> LegacyPaintResolution {
        self.paint_resolution(
            self.style(target).stroke_resolution(),
            FamilyThemeRuleFacet::stroke,
        )
    }

    fn palette(&self, target: ThemeTarget) -> Option<Vec<String>> {
        if !self.program.has_legacy_ordinal_palette(target) {
            return None;
        }
        Some(
            self.program
                .ordinal_palette(target)?
                .colors()
                .iter()
                .map(super::canvas::ThemeColorValue::as_css)
                .collect(),
        )
    }

    fn has_typed_ordinal_palette(&self, target: ThemeTarget) -> bool {
        self.program.ordinal_palette_disposition(target)
            == Some(super::family_mechanism_matrix::FamilyThemeDisposition::TypedAdapter)
    }

    fn stroke_or_fill_resolution(&self, target: ThemeTarget) -> LegacyPaintResolution {
        let style = self.style(target);
        match self.paint_resolution(style.stroke_resolution(), FamilyThemeRuleFacet::stroke) {
            LegacyPaintResolution::Unspecified => {
                self.paint_resolution(style.fill_resolution(), FamilyThemeRuleFacet::fill)
            }
            stroke => stroke,
        }
    }

    fn paint_resolution(
        &self,
        property: &ResolvedProperty<CanvasPaint>,
        facet: fn(&Specified<CanvasPaint>) -> Option<FamilyThemeRuleFacet>,
    ) -> LegacyPaintResolution {
        let Some(origin) = property.winner() else {
            return LegacyPaintResolution::Unspecified;
        };
        let Some(facet) = facet(property.specified()) else {
            return LegacyPaintResolution::Unspecified;
        };
        match self
            .program
            .rule_facet_disposition(origin.rule_index(), facet)
        {
            Some(FamilyThemeDisposition::LegacyCompatibility) => {
                LegacyPaintResolution::from_property(property)
            }
            Some(FamilyThemeDisposition::Unsupported) => LegacyPaintResolution::Unsupported,
            Some(FamilyThemeDisposition::TypedAdapter) | None => LegacyPaintResolution::Suppressed,
        }
    }
}

struct MarkerPaintContribution {
    contribution_id: &'static str,
    value: Option<String>,
}

#[derive(Debug, Clone, Copy)]
enum PaletteProjection {
    ColorScale { limit: usize },
    Pie { limit: usize },
    Journey { task_limit: usize },
}

#[derive(Debug, PartialEq, Eq)]
enum LegacyPaintResolution {
    Unspecified,
    Unsupported,
    Suppressed,
    Value(String),
}

impl LegacyPaintResolution {
    fn from_property(property: &super::resolved::ResolvedProperty<CanvasPaint>) -> Self {
        match property.specified() {
            Specified::Unspecified => Self::Unspecified,
            Specified::Clear => Self::Suppressed,
            Specified::Value(paint) => solid_paint(paint).map_or(Self::Suppressed, Self::Value),
        }
    }

    fn into_value(self) -> Option<String> {
        match self {
            Self::Value(value) => Some(value),
            Self::Unspecified | Self::Unsupported | Self::Suppressed => None,
        }
    }
}

fn solid_paint(paint: &CanvasPaint) -> Option<String> {
    match paint {
        CanvasPaint::Transparent => Some("transparent".to_string()),
        CanvasPaint::Solid(color) => Some(color.as_css()),
        CanvasPaint::LinearGradient(_)
        | CanvasPaint::RadialGradient(_)
        | CanvasPaint::Pattern(_) => None,
    }
}

struct FamilyContributions {
    family: DiagramFamilyId,
    entries: Vec<PendingContribution>,
}

struct PendingContribution {
    mapping: &'static str,
    patch: Map<String, Value>,
}

impl FamilyContributions {
    fn new(family: DiagramFamilyId) -> Self {
        Self {
            family,
            entries: Vec::new(),
        }
    }

    fn add_typography(&mut self, reader: &FamilyStyleReader) {
        let typography = reader.base_typography();
        let mut variables = Map::new();
        let mut root = Map::new();
        if reader.has_legacy_base_typography(ThemeTypographyProperty::FontStack) {
            let font_family = typography.font_stack().as_css();
            if font_family.len() <= MAX_LEGACY_ASSIGNMENT_STRING_BYTES {
                variables.insert("fontFamily".to_string(), Value::String(font_family.clone()));
                root.insert("fontFamily".to_string(), Value::String(font_family));
            }
        }
        if reader.has_legacy_base_typography(ThemeTypographyProperty::FontSize) {
            variables.insert(
                "fontSize".to_string(),
                Value::String(format!("{}px", typography.font_size_px())),
            );
        }
        if variables.is_empty() {
            return;
        }
        root.insert("themeVariables".to_string(), Value::Object(variables));
        self.add_patch("typography", root);
    }

    fn add_theme_variables<const N: usize>(
        &mut self,
        mapping: &'static str,
        variables: [(&'static str, Option<String>); N],
    ) {
        let variables = variables
            .into_iter()
            .filter_map(|(key, value)| value.map(|value| (key.to_string(), Value::String(value))))
            .collect::<Map<_, _>>();
        if variables.is_empty() {
            return;
        }
        let mut root = Map::new();
        root.insert("themeVariables".to_string(), Value::Object(variables));
        self.add_patch(mapping, root);
    }

    fn add_theme_variable_object(
        &mut self,
        mapping: &'static str,
        key: &'static str,
        object: Map<String, Value>,
    ) {
        if object.is_empty() {
            return;
        }
        let mut variables = Map::new();
        variables.insert(key.to_string(), Value::Object(object));
        let mut root = Map::new();
        root.insert("themeVariables".to_string(), Value::Object(variables));
        self.add_patch(mapping, root);
    }

    fn add_root_object(
        &mut self,
        mapping: &'static str,
        key: &'static str,
        object: Map<String, Value>,
    ) {
        if object.is_empty() {
            return;
        }
        let mut root = Map::new();
        root.insert(key.to_string(), Value::Object(object));
        self.add_patch(mapping, root);
    }

    fn add_palette(
        &mut self,
        mapping: &'static str,
        palette: Option<Vec<String>>,
        projection: PaletteProjection,
    ) {
        let Some(palette) = palette.filter(|palette| !palette.is_empty()) else {
            return;
        };
        let mut variables = Map::new();
        match projection {
            PaletteProjection::ColorScale { limit } => {
                for (index, color) in palette.iter().take(limit).enumerate() {
                    variables.insert(format!("cScale{index}"), Value::String(color.clone()));
                }
            }
            PaletteProjection::Pie { limit } => {
                for (index, color) in palette.iter().take(limit).enumerate() {
                    variables.insert(format!("pie{}", index + 1), Value::String(color.clone()));
                }
            }
            PaletteProjection::Journey { task_limit } => {
                for (index, color) in palette.iter().take(task_limit).enumerate() {
                    variables.insert(format!("fillType{index}"), Value::String(color.clone()));
                }
            }
        }
        let mut root = Map::new();
        root.insert("themeVariables".to_string(), Value::Object(variables));
        self.add_patch(mapping, root);
    }

    fn add_patch(&mut self, mapping: &'static str, patch: Map<String, Value>) {
        self.entries.push(PendingContribution { mapping, patch });
    }

    fn finish_into(mut self, builder: &mut OverlayBuilder) {
        if self.entries.is_empty() {
            return;
        }
        for contribution in self.entries.drain(..) {
            builder.push(self.family, contribution.mapping, contribution.patch);
        }
    }
}

struct OverlayBuilder {
    family: DiagramFamilyId,
    overlay: ThemeFamilyCompatibilityOverlayBuilder,
    contribution_ids: BTreeSet<String>,
    claimed_paths: BTreeSet<String>,
    #[cfg(any(test, feature = "internal-theme-acceptance"))]
    accepted_projections: AcceptedLegacyProjections,
}

impl OverlayBuilder {
    fn new(family: DiagramFamilyId) -> Self {
        Self {
            family,
            overlay: ThemeFamilyCompatibilityOverlayBuilder::new(
                family.as_str(),
                CONTRIBUTION_ID_PREFIX,
            ),
            contribution_ids: BTreeSet::new(),
            claimed_paths: BTreeSet::new(),
            #[cfg(any(test, feature = "internal-theme-acceptance"))]
            accepted_projections: BTreeSet::new(),
        }
    }

    fn push(&mut self, family: DiagramFamilyId, mapping: &'static str, patch: Map<String, Value>) {
        debug_assert_eq!(family, self.family);
        let opaque_id = format!("{CONTRIBUTION_ID_PREFIX}{}.{}", family.as_str(), mapping);
        if self.contribution_ids.contains(&opaque_id) {
            return;
        }

        let mut accepted_paths = Vec::new();
        let patch = retain_unclaimed_assignments(
            patch,
            &self.claimed_paths,
            &mut Vec::new(),
            &mut accepted_paths,
        );
        if patch.is_empty() {
            return;
        }
        #[cfg(any(test, feature = "internal-theme-acceptance"))]
        let accepted_projections = projection_observations(&opaque_id, &patch);
        if self
            .overlay
            .try_push(mapping, MermaidConfig::from_value(Value::Object(patch)))
            .is_err()
        {
            return;
        }

        self.claimed_paths.extend(accepted_paths);
        self.contribution_ids.insert(opaque_id);
        #[cfg(any(test, feature = "internal-theme-acceptance"))]
        self.accepted_projections.extend(accepted_projections);
    }

    fn finish(
        self,
    ) -> (
        ThemeFamilyCompatibilityOverlay,
        BTreeSet<String>,
        AcceptedLegacyProjections,
    ) {
        #[cfg(any(test, feature = "internal-theme-acceptance"))]
        let accepted_projections = self.accepted_projections;
        #[cfg(not(any(test, feature = "internal-theme-acceptance")))]
        let accepted_projections = ();
        (
            self.overlay.finish(),
            self.contribution_ids,
            accepted_projections,
        )
    }
}

#[cfg(any(test, feature = "internal-theme-acceptance"))]
fn projection_observations(
    contribution_id: &str,
    patch: &Map<String, Value>,
) -> Vec<ThemeLegacyProjectionObservation> {
    fn visit(
        contribution_id: &str,
        value: &Value,
        path: &mut Vec<String>,
        observations: &mut Vec<ThemeLegacyProjectionObservation>,
    ) {
        match value {
            Value::Object(object) => {
                for (key, value) in object {
                    path.push(key.clone());
                    visit(contribution_id, value, path, observations);
                    path.pop();
                }
            }
            value => observations.push(ThemeLegacyProjectionObservation::new(
                contribution_id.to_string(),
                path.join("."),
                value_digest(value),
            )),
        }
    }

    let mut observations = Vec::new();
    visit(
        contribution_id,
        &Value::Object(patch.clone()),
        &mut Vec::new(),
        &mut observations,
    );
    observations
}

fn retain_unclaimed_assignments(
    patch: Map<String, Value>,
    claimed_paths: &BTreeSet<String>,
    path: &mut Vec<String>,
    accepted_paths: &mut Vec<String>,
) -> Map<String, Value> {
    patch
        .into_iter()
        .filter_map(|(key, value)| {
            path.push(key.clone());
            let retained = match value {
                Value::Object(object) => {
                    let object =
                        retain_unclaimed_assignments(object, claimed_paths, path, accepted_paths);
                    (!object.is_empty()).then_some(Value::Object(object))
                }
                value => {
                    let dotted_path = path.join(".");
                    let is_claimed = claimed_paths
                        .iter()
                        .any(|claimed| dotted_paths_overlap(claimed, &dotted_path));
                    (!is_claimed).then(|| {
                        accepted_paths.push(dotted_path);
                        value
                    })
                }
            };
            path.pop();
            retained.map(|value| (key, value))
        })
        .collect()
}

fn dotted_paths_overlap(left: &str, right: &str) -> bool {
    left == right
        || left
            .strip_prefix(right)
            .is_some_and(|suffix| suffix.starts_with('.'))
        || right
            .strip_prefix(left)
            .is_some_and(|suffix| suffix.starts_with('.'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagram_theme::{
        CanvasSpec, MermaidThemeCompatibility, Specified, ThemeRule, ThemeRuleSet, ThemeStylePatch,
        TypographySpec,
    };
    use merman_core::__private::{
        ThemeCompatibilityPlan, install_theme_compatibility, theme_parse_evidence,
    };

    fn solid(value: &str) -> CanvasPaint {
        CanvasPaint::solid(value).expect("valid test color")
    }

    const GANTT_FIXTURE: &str =
        include_str!("../../../../fixtures/gantt/task_tags_combinations.mmd");
    const REQUIREMENT_FIXTURE: &str =
        include_str!("../../../../fixtures/requirement/relations.mmd");
    const PIE_FIXTURE: &str = include_str!(
        "../../../../fixtures/pie/upstream_cypress_pie_spec_should_render_a_pie_diagram_with_showdata_005.mmd"
    );
    const JOURNEY_FIXTURE: &str =
        include_str!("../../../../fixtures/journey/upstream_tasks_and_people.mmd");

    fn bridge(spec: &DiagramThemeSpec) -> LegacyFamilyThemeBridge {
        let spec = Arc::new(spec.clone());
        LegacyFamilyThemeBridge::new(Arc::new(FamilyThemeProgramCache::new(spec)))
    }

    fn parse(spec: &DiagramThemeSpec, source: &str) -> merman_core::ParseMetadata {
        parse_with_compatibility(spec, source, MermaidConfig::empty_object())
    }

    fn parse_with_compatibility(
        spec: &DiagramThemeSpec,
        source: &str,
        compatibility_config: MermaidConfig,
    ) -> merman_core::ParseMetadata {
        let bridge = bridge(spec);
        let resolver = bridge.clone();
        let plan = ThemeCompatibilityPlan::try_new(
            [0x5a; 32],
            compatibility_config,
            move |family, control| resolver.overlay_for_family(family, control),
        )
        .expect("test compatibility plan");
        install_theme_compatibility(merman_core::Engine::new(), &plan)
            .parse_metadata_sync(source)
            .expect("test diagram should parse")
    }

    #[test]
    fn legacy_projection_probe_reports_current_facts_without_retirement_policy() {
        let unsupported = legacy_projection_probe(
            super::super::legacy_projection_retirement::ThemeLegacyRouteId::new(
                DiagramFamilyId::CLASS,
                ThemeTarget::Marker,
                ThemeLegacyRouteSelector::StaticUnqualified,
                ThemeLegacyRouteFacet::Fill,
            ),
            ThemeLegacyRouteValue::Solid,
        )
        .expect("observe current Class marker facts");
        assert_eq!(
            unsupported.disposition(),
            ThemeLegacyProjectionDisposition::Unsupported
        );
        assert!(unsupported.projections().is_empty());

        let partial = legacy_projection_probe(
            super::super::legacy_projection_retirement::ThemeLegacyRouteId::new(
                DiagramFamilyId::MINDMAP,
                ThemeTarget::Node,
                ThemeLegacyRouteSelector::StaticUnqualified,
                ThemeLegacyRouteFacet::Fill,
            ),
            ThemeLegacyRouteValue::Solid,
        )
        .expect("observe current Mindmap node facts");
        assert_eq!(
            partial.disposition(),
            ThemeLegacyProjectionDisposition::LegacyCompatibility
        );
        assert_eq!(
            partial
                .projections()
                .iter()
                .map(ThemeLegacyProjectionObservation::assignment_path)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["themeVariables.mainBkg", "themeVariables.primaryColor",])
        );
    }

    #[test]
    fn direct_mindmap_palette_does_not_project_legacy_color_scale_values() {
        let palette =
            super::super::OrdinalPalette::new([
                super::super::ThemeColorValue::parse("#abcdef").expect("valid palette color")
            ])
            .expect("non-empty palette");
        let spec = DiagramThemeSpec::new()
            .with_mermaid_compatibility(
                MermaidThemeCompatibility::default()
                    .with_theme("dark")
                    .expect("valid Mermaid theme"),
            )
            .with_styles(ThemeRuleSet::default().with_ordinal_palette(ThemeTarget::Node, palette));
        let compatibility = spec.mermaid().to_mermaid_config();
        let baseline = parse_with_compatibility(
            &DiagramThemeSpec::new().with_mermaid_compatibility(spec.mermaid().clone()),
            "mindmap\nroot(Root)\n Child(Child)\n",
            compatibility.clone(),
        );
        let parsed =
            parse_with_compatibility(&spec, "mindmap\nroot(Root)\n Child(Child)\n", compatibility);

        assert_eq!(
            parsed.effective_config.get_str("themeVariables.cScale0"),
            baseline.effective_config.get_str("themeVariables.cScale0")
        );
        assert_eq!(fallback_contribution_count(&parsed), 0);
    }

    #[test]
    fn direct_mindmap_and_gitgraph_edge_stroke_retire_only_the_edge_projection() {
        for family in [DiagramFamilyId::MINDMAP, DiagramFamilyId::GIT_GRAPH] {
            let spec = DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Edge,
                        ThemeStylePatch::default().with_stroke(solid("#22c55e")),
                    )
                    .for_family(family),
                ),
            );
            let bridge = bridge(&spec);
            let artifact = bridge.compile_for_family(family);

            assert!(artifact.overlay.is_empty(), "family={family}");
            assert!(artifact.contribution_ids.is_empty(), "family={family}");
            assert!(!bridge.owns_contribution_id(&format!(
                "merman.legacy-family-theme.v1.{}.edge.stroke",
                family.as_str()
            )));
        }
    }

    #[test]
    fn typed_treemap_title_fill_suppresses_only_the_title_projection() {
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default()
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Text,
                        ThemeStylePatch::default().with_fill(solid("#334155")),
                    )
                    .for_family(DiagramFamilyId::TREEMAP),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Title,
                        ThemeStylePatch::default().with_fill(solid("#f8fafc")),
                    )
                    .for_family(DiagramFamilyId::TREEMAP),
                ),
        );
        let bridge = bridge(&spec);
        let artifact = bridge.compile_for_family(DiagramFamilyId::TREEMAP);

        assert!(
            artifact
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.treemap.text.fill")
        );
        assert!(
            !artifact
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.treemap.title.fill")
        );
    }

    #[test]
    fn final_four_direct_slices_retire_only_real_legacy_paint_projections() {
        for (family, text_projection_survives) in [
            (DiagramFamilyId::EVENT_MODELING, false),
            (DiagramFamilyId::ISHIKAWA, false),
            (DiagramFamilyId::VENN, true),
            (DiagramFamilyId::ZENUML, false),
        ] {
            let spec = DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::Text,
                            ThemeStylePatch::default().with_fill(solid("#334155")),
                        )
                        .for_family(family),
                    )
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::Title,
                            ThemeStylePatch::default().with_fill(solid("#f8fafc")),
                        )
                        .for_family(family),
                    ),
            );
            let artifact = bridge(&spec).compile_for_family(family);
            let prefix = format!("merman.legacy-family-theme.v1.{}", family.as_str());

            assert_eq!(
                artifact
                    .contribution_ids
                    .contains(&format!("{prefix}.text.fill")),
                text_projection_survives,
                "family={family}"
            );
            assert!(
                !artifact
                    .contribution_ids
                    .contains(&format!("{prefix}.title.fill")),
                "family={family}"
            );
        }
    }

    #[test]
    fn mindmap_terminal_less_paint_rules_do_not_create_a_compatibility_overlay() {
        let color = "#123456";
        let mut styles = ThemeRuleSet::default();
        for target in [
            ThemeTarget::NodeLabel,
            ThemeTarget::Text,
            ThemeTarget::Title,
            ThemeTarget::EdgeLabelBackground,
            ThemeTarget::ClusterLabel,
        ] {
            styles = styles.with_rule(
                ThemeRule::new(target, ThemeStylePatch::default().with_fill(solid(color)))
                    .for_family(DiagramFamilyId::MINDMAP),
            );
        }
        for target in [ThemeTarget::Marker, ThemeTarget::Cluster] {
            styles = styles.with_rule(
                ThemeRule::new(
                    target,
                    ThemeStylePatch::default()
                        .with_fill(solid(color))
                        .with_stroke(solid(color)),
                )
                .for_family(DiagramFamilyId::MINDMAP),
            );
        }
        styles = styles
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::Edge,
                    ThemeStylePatch::default().with_fill(solid(color)),
                )
                .for_family(DiagramFamilyId::MINDMAP),
            )
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::Edge,
                    ThemeStylePatch::default().with_stroke(solid(color)),
                )
                .with_variant(ThemeVariant::Default)
                .for_family(DiagramFamilyId::MINDMAP),
            );

        let artifact = bridge(&DiagramThemeSpec::new().with_styles(styles))
            .compile_for_family(DiagramFamilyId::MINDMAP);

        assert!(artifact.overlay.is_empty());
        assert!(artifact.contribution_ids.is_empty());
    }

    #[test]
    fn mindmap_legacy_node_paint_keeps_its_historical_projection_shape() {
        let source = "mindmap\nroot(Root)\n Child(Child)\n";
        let fill = "#123456";
        let stroke = "#654321";
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::Node,
                    ThemeStylePatch::default()
                        .with_fill(solid(fill))
                        .with_stroke(solid(stroke)),
                )
                .for_family(DiagramFamilyId::MINDMAP),
            ),
        );

        let parsed = parse(&spec, source);

        for (path, value) in [
            ("themeVariables.primaryColor", fill),
            ("themeVariables.mainBkg", fill),
            ("themeVariables.primaryBorderColor", stroke),
            ("themeVariables.nodeBorder", stroke),
        ] {
            assert_eq!(
                parsed.effective_config.get_str(path),
                Some(value),
                "path={path}"
            );
        }
    }

    #[test]
    fn tree_view_terminal_less_paint_rules_do_not_create_a_compatibility_overlay() {
        let color = "#123456";
        let styles = ThemeRuleSet::default()
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::Node,
                    ThemeStylePatch::default()
                        .with_fill(solid(color))
                        .with_stroke(solid(color)),
                )
                .for_family(DiagramFamilyId::TREE_VIEW),
            )
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::Title,
                    ThemeStylePatch::default().with_fill(solid(color)),
                )
                .for_family(DiagramFamilyId::TREE_VIEW),
            )
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::EdgeLabelBackground,
                    ThemeStylePatch::default().with_fill(solid(color)),
                )
                .for_family(DiagramFamilyId::TREE_VIEW),
            )
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::Cluster,
                    ThemeStylePatch::default()
                        .with_fill(solid(color))
                        .with_stroke(solid(color)),
                )
                .for_family(DiagramFamilyId::TREE_VIEW),
            )
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::ClusterLabel,
                    ThemeStylePatch::default().with_fill(solid(color)),
                )
                .for_family(DiagramFamilyId::TREE_VIEW),
            );

        let artifact = bridge(&DiagramThemeSpec::new().with_styles(styles))
            .compile_for_family(DiagramFamilyId::TREE_VIEW);

        assert!(artifact.overlay.is_empty());
        assert!(artifact.contribution_ids.is_empty());
    }

    #[test]
    fn tree_view_legacy_paint_keeps_its_historical_projection_shape() {
        let source = "treeView-beta\nroot\n  child\n";
        let label = "#123456";
        let line = "#654321";
        let icon = "#abcdef";
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default()
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::NodeLabel,
                        ThemeStylePatch::default().with_fill(solid(label)),
                    )
                    .for_family(DiagramFamilyId::TREE_VIEW),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Edge,
                        ThemeStylePatch::default().with_fill(solid(line)),
                    )
                    .for_family(DiagramFamilyId::TREE_VIEW),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Marker,
                        ThemeStylePatch::default().with_stroke(solid(icon)),
                    )
                    .for_family(DiagramFamilyId::TREE_VIEW),
                ),
        );

        let parsed = parse(&spec, source);

        assert_eq!(
            parsed
                .effective_config
                .get_str("themeVariables.treeView.labelColor"),
            Some(label)
        );
        assert_eq!(
            parsed
                .effective_config
                .get_str("themeVariables.treeView.lineColor"),
            Some(line)
        );
        assert_eq!(
            parsed
                .effective_config
                .get_str("themeVariables.treeView.iconColor"),
            Some(icon)
        );

        let evidence = theme_parse_evidence(&parsed);
        let contributions = evidence.fallback_contributions().collect::<Vec<_>>();
        assert_eq!(
            contributions
                .iter()
                .flat_map(|contribution| contribution.surviving_assignment_paths())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                "themeVariables.arrowheadColor",
                "themeVariables.lineColor",
                "themeVariables.nodeTextColor",
                "themeVariables.primaryTextColor",
                "themeVariables.textColor",
                "themeVariables.treeView.iconColor",
                "themeVariables.treeView.labelColor",
                "themeVariables.treeView.lineColor",
            ])
        );
        for (path, value) in [
            ("themeVariables.primaryTextColor", label),
            ("themeVariables.nodeTextColor", label),
            ("themeVariables.textColor", label),
            ("themeVariables.lineColor", line),
            ("themeVariables.arrowheadColor", icon),
            ("themeVariables.treeView.labelColor", label),
            ("themeVariables.treeView.lineColor", line),
            ("themeVariables.treeView.iconColor", icon),
        ] {
            assert_eq!(
                contributions
                    .iter()
                    .find_map(|contribution| contribution.surviving_assignment_value(path))
                    .and_then(Value::as_str),
                Some(value),
                "path={path}"
            );
        }
    }

    #[test]
    fn gitgraph_terminal_less_paint_rules_do_not_create_a_compatibility_overlay() {
        let color = "#123456";
        let mut styles = ThemeRuleSet::default();
        for target in [ThemeTarget::Title, ThemeTarget::ClusterLabel] {
            styles = styles.with_rule(
                ThemeRule::new(target, ThemeStylePatch::default().with_fill(solid(color)))
                    .for_family(DiagramFamilyId::GIT_GRAPH),
            );
        }
        for target in [ThemeTarget::Marker, ThemeTarget::Cluster] {
            styles = styles.with_rule(
                ThemeRule::new(
                    target,
                    ThemeStylePatch::default()
                        .with_fill(solid(color))
                        .with_stroke(solid(color)),
                )
                .for_family(DiagramFamilyId::GIT_GRAPH),
            );
        }

        let artifact = bridge(&DiagramThemeSpec::new().with_styles(styles))
            .compile_for_family(DiagramFamilyId::GIT_GRAPH);

        assert!(artifact.overlay.is_empty());
        assert!(artifact.contribution_ids.is_empty());
    }

    #[test]
    fn gitgraph_legacy_paint_projects_only_writer_consumed_variables() {
        let source = "gitGraph\n  commit id: \"A\" tag: \"v1\"\n";
        let node_fill = "#101112";
        let node_stroke = "#131415";
        let node_label = "#161718";
        let text = "#191a1b";
        let edge = "#1c1d1e";
        let edge_label = "#1f2021";
        let edge_label_background = "#222324";
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default()
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default()
                            .with_fill(solid(node_fill))
                            .with_stroke(solid(node_stroke)),
                    )
                    .for_family(DiagramFamilyId::GIT_GRAPH),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Text,
                        ThemeStylePatch::default().with_fill(solid(text)),
                    )
                    .for_family(DiagramFamilyId::GIT_GRAPH),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::NodeLabel,
                        ThemeStylePatch::default().with_fill(solid(node_label)),
                    )
                    .for_family(DiagramFamilyId::GIT_GRAPH),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Edge,
                        ThemeStylePatch::default().with_fill(solid(edge)),
                    )
                    .for_family(DiagramFamilyId::GIT_GRAPH),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::EdgeLabel,
                        ThemeStylePatch::default().with_fill(solid(edge_label)),
                    )
                    .for_family(DiagramFamilyId::GIT_GRAPH),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::EdgeLabelBackground,
                        ThemeStylePatch::default().with_fill(solid(edge_label_background)),
                    )
                    .for_family(DiagramFamilyId::GIT_GRAPH),
                ),
        );

        let parsed = parse(&spec, source);

        for (path, value) in [
            ("themeVariables.primaryColor", node_fill),
            ("themeVariables.mainBkg", node_fill),
            ("themeVariables.tagLabelBackground", node_fill),
            ("themeVariables.primaryBorderColor", node_stroke),
            ("themeVariables.nodeBorder", node_stroke),
            ("themeVariables.tagLabelBorder", node_stroke),
            ("themeVariables.textColor", text),
            ("themeVariables.tagLabelColor", node_label),
            ("themeVariables.commitLineColor", edge),
            ("themeVariables.commitLabelColor", edge_label),
            (
                "themeVariables.commitLabelBackground",
                edge_label_background,
            ),
        ] {
            assert_eq!(
                parsed.effective_config.get_str(path),
                Some(value),
                "path={path}"
            );
        }

        let evidence = theme_parse_evidence(&parsed);
        let mut assignments = std::collections::BTreeMap::new();
        for contribution in evidence.fallback_contributions() {
            for path in contribution.surviving_assignment_paths() {
                let value = contribution
                    .surviving_assignment_value(path)
                    .and_then(Value::as_str)
                    .expect("paint compatibility assignments should be strings");
                assert_eq!(assignments.insert(path, value), None, "path={path}");
            }
        }
        assert_eq!(
            assignments,
            std::collections::BTreeMap::from([
                (
                    "themeVariables.commitLabelBackground",
                    edge_label_background
                ),
                ("themeVariables.commitLabelColor", edge_label),
                ("themeVariables.commitLineColor", edge),
                ("themeVariables.mainBkg", node_fill),
                ("themeVariables.nodeBorder", node_stroke),
                ("themeVariables.primaryBorderColor", node_stroke),
                ("themeVariables.primaryColor", node_fill),
                ("themeVariables.tagLabelBackground", node_fill),
                ("themeVariables.tagLabelBorder", node_stroke),
                ("themeVariables.tagLabelColor", node_label),
                ("themeVariables.textColor", text),
            ])
        );
    }

    #[test]
    fn generic_text_fill_reaches_only_gitgraph_visible_text_tokens() {
        let color = "#123456";
        let family = DiagramFamilyId::GIT_GRAPH;
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::Text,
                    ThemeStylePatch::default().with_fill(solid(color)),
                )
                .for_family(family),
            ),
        );

        let parsed = parse(&spec, "gitGraph\n  commit id: \"A\" tag: \"v1\"\n");
        assert_eq!(
            parsed.effective_config.get_str("themeVariables.textColor"),
            Some(color)
        );
        let evidence = theme_parse_evidence(&parsed);
        let paths = evidence
            .fallback_contributions()
            .flat_map(|contribution| contribution.surviving_assignment_paths())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            paths,
            BTreeSet::from([
                "themeVariables.commitLabelColor",
                "themeVariables.tagLabelColor",
                "themeVariables.textColor",
            ])
        );
    }

    #[test]
    fn direct_er_entity_paint_has_no_legacy_projection() {
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::Entity,
                    ThemeStylePatch::default()
                        .with_fill(solid("#f8fafc"))
                        .with_stroke(solid("#334155")),
                )
                .for_family(DiagramFamilyId::ER),
            ),
        );
        let bridge = bridge(&spec);
        let artifact = bridge.compile_for_family(DiagramFamilyId::ER);

        assert!(
            !artifact
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.er.entity.paint")
        );
    }

    #[test]
    fn direct_er_relation_stroke_has_no_legacy_projection() {
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::Relation,
                    ThemeStylePatch::default().with_stroke(solid("#334155")),
                )
                .for_family(DiagramFamilyId::ER),
            ),
        );
        let bridge = bridge(&spec);
        let artifact = bridge.compile_for_family(DiagramFamilyId::ER);

        assert!(artifact.overlay.is_empty());
        assert!(artifact.contribution_ids.is_empty());
        assert!(!bridge.owns_contribution_id("merman.legacy-family-theme.v1.er.edge.stroke"));
    }

    fn fallback_contribution_count(metadata: &merman_core::ParseMetadata) -> usize {
        theme_parse_evidence(metadata).fallback_contribution_count()
    }

    #[test]
    fn provider_compiles_only_the_selected_family_and_reuses_its_artifact() {
        let spec = Arc::new(
            DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Marker,
                        ThemeStylePatch::default().with_stroke(solid("#f8fafc")),
                    )
                    .for_family(DiagramFamilyId::FLOWCHART),
                ),
            ),
        );
        let family_programs = Arc::new(FamilyThemeProgramCache::new(Arc::clone(&spec)));
        let bridge = LegacyFamilyThemeBridge::new(Arc::clone(&family_programs));
        let control = OperationControl::new();

        assert_eq!(bridge.cached_family_count(), 0);
        assert_eq!(family_programs.len(), 0);

        let first = bridge
            .overlay_for_family("flowchart", &control)
            .expect("active control")
            .expect("flowchart compatibility overlay");
        assert!(!first.is_empty());
        assert_eq!(bridge.cached_family_count(), 1);
        assert_eq!(family_programs.len(), 1);
        assert!(family_programs.contains(DiagramFamilyId::FLOWCHART));
        assert!(!family_programs.contains(DiagramFamilyId::SEQUENCE));

        let second = bridge
            .overlay_for_family("flowchart", &control)
            .expect("active control")
            .expect("cached flowchart compatibility overlay");
        assert!(!second.is_empty());
        assert_eq!(bridge.cached_family_count(), 1);
        assert_eq!(family_programs.len(), 1);

        assert!(
            bridge
                .overlay_for_family("state", &control)
                .expect("active control")
                .is_none()
        );
        assert_eq!(bridge.cached_family_count(), 1);
        assert_eq!(family_programs.len(), 1);

        bridge
            .overlay_for_family("sequence", &control)
            .expect("active control");
        assert_eq!(bridge.cached_family_count(), 2);
        assert_eq!(family_programs.len(), 2);
        assert!(family_programs.contains(DiagramFamilyId::SEQUENCE));
    }

    #[test]
    fn direct_family_palettes_retire_git_graph_and_kanban_contributions() {
        let palette = super::super::OrdinalPalette::new([
            super::super::ThemeColorValue::parse("#ef4444").expect("valid palette color"),
            super::super::ThemeColorValue::parse("#22c55e").expect("valid palette color"),
        ])
        .expect("non-empty palette");
        let styles = ThemeRuleSet::default()
            .with_ordinal_palette(ThemeTarget::Node, palette.clone())
            .with_ordinal_palette(ThemeTarget::Task, palette);
        let spec = DiagramThemeSpec::new().with_styles(styles);
        let bridge = bridge(&spec);

        let git_graph = bridge.compile_for_family(DiagramFamilyId::GIT_GRAPH);
        assert!(
            !git_graph
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.gitGraph.node.palette")
        );

        let kanban = bridge.compile_for_family(DiagramFamilyId::KANBAN);
        for retired_id in [
            "merman.legacy-family-theme.v1.kanban.task.palette.color-scale",
            "merman.legacy-family-theme.v1.kanban.task.palette.git",
        ] {
            assert!(!kanban.contribution_ids.contains(retired_id));
        }
    }

    #[test]
    fn explicit_journey_task_fill_creates_a_direct_contribution() {
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::JourneyTask,
                    ThemeStylePatch::default().with_fill(solid("#ef4444")),
                )
                .for_family(DiagramFamilyId::JOURNEY),
            ),
        );
        let artifact = bridge(&spec).compile_for_family(DiagramFamilyId::JOURNEY);

        assert!(
            artifact
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.journey.task.paint-text")
        );
    }

    #[test]
    fn explicit_pie_slice_clear_does_not_create_a_direct_palette() {
        let mut clear = ThemeStylePatch::default();
        clear.paint.fill = Specified::Clear;
        let spec = DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
            ThemeRule::new(ThemeTarget::PieSlice, clear).for_family(DiagramFamilyId::PIE),
        ));
        let bridge = bridge(&spec);
        let artifact = bridge.compile_for_family(DiagramFamilyId::PIE);

        assert!(
            !artifact
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.pie.slice.palette")
        );
    }

    #[test]
    fn explicit_pie_slice_fill_survives_typed_palette_bridge_suppression() {
        let palette = super::super::OrdinalPalette::new([
            super::super::ThemeColorValue::parse("#ef4444").expect("valid palette color"),
            super::super::ThemeColorValue::parse("#22c55e").expect("valid palette color"),
        ])
        .expect("non-empty palette");
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default()
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::PieSlice,
                        ThemeStylePatch::default().with_fill(solid("#111827")),
                    )
                    .for_family(DiagramFamilyId::PIE),
                )
                .with_ordinal_palette(ThemeTarget::PieSlice, palette),
        );
        let artifact = bridge(&spec).compile_for_family(DiagramFamilyId::PIE);

        assert!(
            artifact
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.pie.slice.fill")
        );
        assert!(
            !artifact
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.pie.slice.palette")
        );
    }

    #[test]
    fn family_scoped_rules_only_contribute_to_their_detected_family() {
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default()
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Node,
                        ThemeStylePatch::default().with_fill(solid("#ef4444")),
                    )
                    .for_family(DiagramFamilyId::FLOWCHART),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Actor,
                        ThemeStylePatch::default().with_fill(solid("#22c55e")),
                    )
                    .for_family(DiagramFamilyId::SEQUENCE),
                ),
        );

        let flowchart = parse(&spec, "flowchart LR\nA --> B\n");
        let sequence = parse(&spec, "sequenceDiagram\nAlice->>Bob: Hello\n");

        assert_ne!(
            flowchart
                .effective_config
                .get_str("themeVariables.primaryColor"),
            Some("#ef4444")
        );
        assert_eq!(fallback_contribution_count(&flowchart), 0);
        assert_ne!(
            sequence.effective_config.get_str("themeVariables.actorBkg"),
            Some("#22c55e")
        );
        assert_eq!(fallback_contribution_count(&sequence), 0);
    }

    #[test]
    fn flowchart_roles_do_not_fall_back_into_sequence_roles() {
        let spec =
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::Node,
                ThemeStylePatch::default().with_fill(solid("#ef4444")),
            )));

        let sequence = parse(&spec, "sequenceDiagram\nAlice->>Bob: Hello\n");

        assert_eq!(fallback_contribution_count(&sequence), 0);
        assert_ne!(
            sequence.effective_config.get_str("themeVariables.actorBkg"),
            Some("#ef4444")
        );
    }

    #[test]
    fn swimlane_uses_its_own_program_and_stable_contribution_ids() {
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::Cluster,
                    ThemeStylePatch::default()
                        .with_fill(solid("#fef3c7"))
                        .with_stroke(solid("#a16207")),
                )
                .for_family(DiagramFamilyId::SWIMLANE),
            ),
        );

        let swimlane = parse(&spec, "swimlane-beta LR\nA --> B\n");
        let flowchart = parse(&spec, "flowchart LR\nA --> B\n");
        let baseline = parse(&DiagramThemeSpec::default(), "swimlane-beta LR\nA --> B\n");
        assert_eq!(fallback_contribution_count(&swimlane), 0);
        for path in [
            "themeVariables.clusterBkg",
            "themeVariables.secondaryColor",
            "themeVariables.clusterBorder",
        ] {
            assert_eq!(
                swimlane.effective_config.get_str(path),
                baseline.effective_config.get_str(path),
                "typed Swimlane Cluster paint must not mutate legacy `{path}`"
            );
        }
        assert_eq!(fallback_contribution_count(&flowchart), 0);
    }

    #[test]
    fn family_typography_is_local_and_preserves_the_patch_shape() {
        let class_typography = TextStyle::default()
            .with_font_stack(
                super::super::FontStack::new(["Inter", "sans-serif"]).expect("valid font stack"),
            )
            .with_font_size_px(18.0)
            .expect("valid font size");
        let spec = DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(DiagramFamilyId::CLASS, class_typography),
        );

        let class = parse(&spec, "classDiagram\nclass Alpha\n");
        let flowchart = parse(&spec, "flowchart LR\nA --> B\n");

        assert_eq!(fallback_contribution_count(&class), 1);
        assert_ne!(class.effective_config.get_str("theme"), Some("base"));
        assert_eq!(
            class.effective_config.get_str("fontFamily"),
            Some("Inter, sans-serif")
        );
        assert_eq!(
            class.effective_config.get_str("themeVariables.fontFamily"),
            Some("Inter, sans-serif")
        );
        assert_eq!(
            class.effective_config.get_str("themeVariables.fontSize"),
            Some("18px")
        );
        assert_eq!(fallback_contribution_count(&flowchart), 0);
    }

    #[test]
    fn packet_font_stack_retires_the_legacy_typography_projection() {
        let typography = TextStyle::default().with_font_stack(
            super::super::FontStack::new(["Inter", "sans-serif"]).expect("valid Packet font stack"),
        );
        let spec = DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(DiagramFamilyId::PACKET, typography),
        );
        let bridge = bridge(&spec);
        let artifact = bridge.compile_for_family(DiagramFamilyId::PACKET);

        assert!(artifact.overlay.is_empty());
        assert!(artifact.contribution_ids.is_empty());
        assert!(!bridge.owns_contribution_id("merman.legacy-family-theme.v1.packet.typography"));
    }

    #[test]
    fn railroad_font_stack_and_size_retire_only_the_legacy_typography_projection() {
        let typography = TextStyle::default()
            .with_font_stack(
                super::super::FontStack::new(["Inter", "sans-serif"])
                    .expect("valid Railroad font stack"),
            )
            .with_font_size_px(18.0)
            .expect("valid Railroad font size");
        let spec = DiagramThemeSpec::new()
            .with_typography(
                TypographySpec::default().with_family_style(DiagramFamilyId::RAILROAD, typography),
            )
            .with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Text,
                        ThemeStylePatch::default().with_fill(solid("#334155")),
                    )
                    .for_family(DiagramFamilyId::RAILROAD),
                ),
            );
        let bridge = bridge(&spec);
        let artifact = bridge.compile_for_family(DiagramFamilyId::RAILROAD);

        assert!(!bridge.owns_contribution_id("merman.legacy-family-theme.v1.railroad.typography"));
        assert!(
            artifact
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.railroad.text.fill")
        );
        let source = include_str!(
            "../../../../fixtures/railroad/upstream_cypress_railroad_spec_renders_a_simple_rule_001.mmd"
        );
        let parsed = parse(&spec, source);
        let baseline = parse(&DiagramThemeSpec::default(), source);
        for path in [
            "fontFamily",
            "themeVariables.fontFamily",
            "themeVariables.fontSize",
        ] {
            assert_eq!(
                parsed.effective_config.get_str(path),
                baseline.effective_config.get_str(path),
                "typed Railroad typography must not write legacy `{path}`"
            );
        }
        assert_eq!(
            parsed.effective_config.get_str("themeVariables.textColor"),
            Some("#334155")
        );
    }

    #[test]
    fn flowchart_and_swimlane_font_stack_and_size_retire_the_legacy_typography_projection() {
        let typography = TextStyle::default()
            .with_font_stack(
                super::super::FontStack::new(["Inter", "sans-serif"])
                    .expect("valid Flowchart-family font stack"),
            )
            .with_font_size_px(18.0)
            .expect("valid Flowchart-family font size");

        for (family, source) in [
            (DiagramFamilyId::FLOWCHART, "flowchart LR\nA --> B\n"),
            (
                DiagramFamilyId::SWIMLANE,
                "---\nconfig:\n  layout: swimlane\n---\nflowchart TD\nA --> B\n",
            ),
        ] {
            let spec = DiagramThemeSpec::new().with_typography(
                TypographySpec::default().with_family_style(family, typography.clone()),
            );
            let bridge = bridge(&spec);
            let artifact = bridge.compile_for_family(family);
            let contribution_id = format!(
                "merman.legacy-family-theme.v1.{}.typography",
                family.as_str()
            );

            assert!(!artifact.contribution_ids.contains(&contribution_id));
            assert!(!bridge.owns_contribution_id(&contribution_id));
            assert!(artifact.overlay.is_empty());

            let parsed = parse(&spec, source);
            let baseline = parse(&DiagramThemeSpec::default(), source);
            for path in [
                "fontFamily",
                "themeVariables.fontFamily",
                "themeVariables.fontSize",
            ] {
                assert_eq!(
                    parsed.effective_config.get_str(path),
                    baseline.effective_config.get_str(path),
                    "typed {family} base typography must not write legacy `{path}`"
                );
            }
        }
    }

    #[test]
    fn sequence_font_stack_and_size_retire_the_legacy_typography_projection() {
        let typography = TextStyle::default()
            .with_font_stack(
                super::super::FontStack::new(["Inter", "sans-serif"])
                    .expect("valid Sequence font stack"),
            )
            .with_font_size_px(18.0)
            .expect("valid Sequence font size");
        let spec = DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(DiagramFamilyId::SEQUENCE, typography),
        );
        let bridge = bridge(&spec);
        let artifact = bridge.compile_for_family(DiagramFamilyId::SEQUENCE);

        assert!(artifact.overlay.is_empty());
        assert!(artifact.contribution_ids.is_empty());
        assert!(!bridge.owns_contribution_id("merman.legacy-family-theme.v1.sequence.typography"));

        let source = "sequenceDiagram\nparticipant A\nparticipant B\nA->>B: Hello\n";
        let parsed = parse(&spec, source);
        let baseline = parse(&DiagramThemeSpec::default(), source);
        for path in [
            "fontFamily",
            "themeVariables.fontFamily",
            "themeVariables.fontSize",
        ] {
            assert_eq!(
                parsed.effective_config.get_str(path),
                baseline.effective_config.get_str(path),
                "typed Sequence base typography must not write legacy `{path}`"
            );
        }
    }

    #[test]
    fn info_and_error_font_stack_suppression_preserves_only_real_terminal_paint_routes() {
        for family in [DiagramFamilyId::INFO, DiagramFamilyId::ERROR] {
            let typography = TextStyle::default()
                .with_font_stack(
                    super::super::FontStack::new(["Inter", "sans-serif"])
                        .expect("valid Info/Error font stack"),
                )
                .with_font_size_px(18.0)
                .expect("valid fixed-terminal residual font size");
            let spec = DiagramThemeSpec::new()
                .with_typography(TypographySpec::default().with_family_style(family, typography))
                .with_styles(
                    ThemeRuleSet::default()
                        .with_rule(
                            ThemeRule::new(
                                ThemeTarget::Text,
                                ThemeStylePatch::default().with_fill(solid("#334155")),
                            )
                            .for_family(family),
                        )
                        .with_rule(
                            ThemeRule::new(
                                ThemeTarget::Title,
                                ThemeStylePatch::default().with_fill(solid("#f8fafc")),
                            )
                            .for_family(family),
                        ),
                );
            let bridge = bridge(&spec);
            let artifact = bridge.compile_for_family(family);
            let prefix = format!("merman.legacy-family-theme.v1.{}", family.as_str());

            assert!(
                !artifact
                    .contribution_ids
                    .contains(&format!("{prefix}.typography")),
                "{family} must not retain the migrated typography contribution"
            );
            assert!(
                !artifact
                    .contribution_ids
                    .contains(&format!("{prefix}.title.fill")),
                "{family} has no Title terminal and must not retain a Title bridge"
            );
            if family == DiagramFamilyId::INFO {
                assert!(
                    artifact
                        .contribution_ids
                        .contains(&format!("{prefix}.text.fill")),
                    "Info .version inherits the real textColor bridge"
                );
            } else {
                assert!(
                    !artifact
                        .contribution_ids
                        .contains(&format!("{prefix}.text.fill")),
                    "Error .error-text reads errorTextColor, not generic textColor"
                );
                assert!(artifact.overlay.is_empty());
                assert!(artifact.contribution_ids.is_empty());
            }
        }
    }

    #[test]
    fn cynefin_and_wardley_mixed_typography_never_recreates_the_legacy_projection() {
        for (family, source) in [
            (
                DiagramFamilyId::CYNEFIN,
                include_str!(
                    "../../../../fixtures/cynefin/upstream_cypress_cynefin_spec_should_render_a_simple_cynefin_diagram_with_all_five_domains_001.mmd"
                ),
            ),
            (
                DiagramFamilyId::WARDLEY,
                include_str!(
                    "../../../../fixtures/wardley/upstream_cypress_wardley_spec_1_should_render_tea_shop_001.mmd"
                ),
            ),
        ] {
            let typography = TextStyle::default()
                .with_font_stack(
                    super::super::FontStack::new(["Inter", "sans-serif"])
                        .expect("valid Cynefin/Wardley font stack"),
                )
                .with_font_size_px(18.0)
                .expect("valid unsupported Cynefin/Wardley font size");
            let spec = DiagramThemeSpec::new()
                .with_typography(TypographySpec::default().with_family_style(family, typography));
            let bridge = bridge(&spec);
            let artifact = bridge.compile_for_family(family);
            let contribution_id = format!(
                "merman.legacy-family-theme.v1.{}.typography",
                family.as_str()
            );

            assert!(!artifact.contribution_ids.contains(&contribution_id));
            assert!(!bridge.owns_contribution_id(&contribution_id));

            let parsed = parse(&spec, source);
            let baseline = parse(&DiagramThemeSpec::default(), source);
            for path in [
                "fontFamily",
                "themeVariables.fontFamily",
                "themeVariables.fontSize",
            ] {
                assert_eq!(
                    parsed.effective_config.get_str(path),
                    baseline.effective_config.get_str(path),
                    "typed/unsupported {family} typography must not write legacy `{path}`"
                );
            }
        }
    }

    #[test]
    fn pie_and_block_unqualified_stroke_retire_only_their_direct_projection() {
        for (family, target, contribution_id) in [
            (
                DiagramFamilyId::PIE,
                ThemeTarget::PieSlice,
                "merman.legacy-family-theme.v1.pie.slice.stroke",
            ),
            (
                DiagramFamilyId::BLOCK,
                ThemeTarget::Node,
                "merman.legacy-family-theme.v1.block.node.stroke",
            ),
        ] {
            let direct_spec = DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        target,
                        ThemeStylePatch::default().with_stroke(solid("#2563eb")),
                    )
                    .for_family(family),
                ),
            );
            let direct_bridge = bridge(&direct_spec);
            let direct = direct_bridge.compile_for_family(family);

            assert!(!direct.contribution_ids.contains(contribution_id));
            assert!(!direct_bridge.owns_contribution_id(contribution_id));

            let legacy_spec = DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        target,
                        ThemeStylePatch::default().with_stroke(solid("#2563eb")),
                    )
                    .with_variant(ThemeVariant::Default)
                    .for_family(family),
                ),
            );
            let legacy_bridge = bridge(&legacy_spec);
            let legacy = legacy_bridge.compile_for_family(family);

            assert!(legacy.contribution_ids.contains(contribution_id));
            assert!(legacy_bridge.owns_contribution_id(contribution_id));
        }
    }

    #[test]
    fn unsupported_base_typography_does_not_emit_unrelated_legacy_fields() {
        let sequence_typography = TextStyle::default()
            .with_font_weight(700)
            .expect("valid font weight");
        let spec = DiagramThemeSpec::new().with_typography(
            TypographySpec::default()
                .with_family_style(DiagramFamilyId::SEQUENCE, sequence_typography),
        );
        let source = "sequenceDiagram\nAlice->>Bob: Hello\n";

        let sequence = parse(&spec, source);
        let baseline = parse(&DiagramThemeSpec::default(), source);

        assert_eq!(fallback_contribution_count(&sequence), 0);
        for path in [
            "fontFamily",
            "themeVariables.fontFamily",
            "themeVariables.fontSize",
        ] {
            assert_eq!(
                sequence.effective_config.get_str(path),
                baseline.effective_config.get_str(path),
                "unsupported base typography must not mutate `{path}`"
            );
        }
    }

    #[test]
    fn oversized_font_stack_does_not_drop_independent_font_size() {
        let families = (0..32)
            .map(|index| format!("font-{index}-{}", "x".repeat(180)))
            .collect::<Vec<_>>();
        let typography = TextStyle::default()
            .with_font_stack(
                super::super::FontStack::new(families).expect("valid oversized font stack"),
            )
            .with_font_size_px(18.0)
            .expect("valid font size");
        let spec = DiagramThemeSpec::new().with_typography(
            TypographySpec::default().with_family_style(DiagramFamilyId::CLASS, typography),
        );
        let parsed = parse(&spec, "classDiagram\nclass Alpha\n");
        let baseline = parse(&DiagramThemeSpec::default(), "classDiagram\nclass Alpha\n");

        assert_eq!(fallback_contribution_count(&parsed), 1);
        assert_eq!(
            parsed.effective_config.get_str("themeVariables.fontSize"),
            Some("18px")
        );
        assert_eq!(
            parsed.effective_config.get_str("fontFamily"),
            baseline.effective_config.get_str("fontFamily")
        );
        assert_eq!(
            parsed.effective_config.get_str("themeVariables.fontFamily"),
            baseline
                .effective_config
                .get_str("themeVariables.fontFamily")
        );
    }

    #[test]
    fn state_and_root_owned_mechanisms_do_not_create_legacy_overlays() {
        let state_rule = ThemeRule::new(
            ThemeTarget::State,
            ThemeStylePatch::default().with_fill(solid("#2563eb")),
        )
        .for_family(DiagramFamilyId::STATE);
        let state_only = DiagramThemeSpec::new()
            .with_canvas(CanvasSpec::solid("#0f172a").expect("valid canvas"))
            .with_styles(ThemeRuleSet::default().with_rule(state_rule));
        let bridge = bridge(&state_only);

        assert!(bridge.is_empty());
        assert!(!bridge.owns_contribution_id("merman.legacy-family-theme.v1.flowchart.node.fill"));
        assert!(!bridge.owns_contribution_id("some-other-overlay"));
    }

    #[test]
    fn clear_blocks_stroke_to_fill_and_marker_to_edge_fallbacks() {
        let mut cleared_edge = ThemeStylePatch::default().with_fill(solid("#ef4444"));
        cleared_edge.stroke.paint = Specified::Clear;
        let cleared_edge_spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Edge, cleared_edge)),
        );
        let cleared_edge = parse(&cleared_edge_spec, "flowchart LR\nA --> B\n");

        assert_eq!(fallback_contribution_count(&cleared_edge), 0);
        assert_ne!(
            cleared_edge
                .effective_config
                .get_str("themeVariables.lineColor"),
            Some("#ef4444")
        );

        let edge = ThemeStylePatch::default().with_stroke(solid("#22c55e"));
        let mut marker = ThemeStylePatch::default();
        marker.stroke.paint = Specified::Clear;
        let cleared_marker_spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default()
                .with_rule(ThemeRule::new(ThemeTarget::Edge, edge))
                .with_rule(ThemeRule::new(ThemeTarget::Marker, marker)),
        );
        let cleared_marker = parse(&cleared_marker_spec, "flowchart LR\nA --> B\n");

        assert_eq!(fallback_contribution_count(&cleared_marker), 0);
        assert_ne!(
            cleared_marker
                .effective_config
                .get_str("themeVariables.arrowheadColor"),
            Some("#22c55e")
        );
    }

    #[test]
    fn unsupported_marker_paint_blocks_edge_fallback_outside_class() {
        for family in [
            DiagramFamilyId::FLOWCHART,
            DiagramFamilyId::SWIMLANE,
            DiagramFamilyId::BLOCK,
        ] {
            let edge = ThemeRule::new(
                ThemeTarget::Edge,
                ThemeStylePatch::default().with_stroke(solid("#22c55e")),
            )
            .with_variant(ThemeVariant::Default)
            .for_family(family);
            let mut marker_style = ThemeStylePatch::default();
            marker_style.stroke.paint = Specified::Clear;
            let marker = ThemeRule::new(ThemeTarget::Marker, marker_style).for_family(family);
            let spec = DiagramThemeSpec::new()
                .with_styles(ThemeRuleSet::default().with_rule(edge).with_rule(marker));
            let artifact = bridge(&spec).compile_for_family(family);

            assert!(artifact.contribution_ids.iter().any(|id| {
                id == &format!(
                    "merman.legacy-family-theme.v1.{}.edge.stroke",
                    family.as_str(),
                )
            }));
            assert!(!artifact.contribution_ids.iter().any(|id| {
                id == &format!(
                    "merman.legacy-family-theme.v1.{}.marker.paint-from-edge",
                    family.as_str(),
                )
            }));
        }
    }

    #[test]
    fn typed_flowchart_cluster_paint_does_not_create_legacy_assignments() {
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::Cluster,
                    ThemeStylePatch::default()
                        .with_fill(solid("#ef4444"))
                        .with_stroke(solid("#2563eb")),
                )
                .for_family(DiagramFamilyId::FLOWCHART),
            ),
        );
        let parsed = parse(&spec, "flowchart TD\nsubgraph Group\nA\nend\n");
        let baseline = parse(
            &DiagramThemeSpec::default(),
            "flowchart TD\nsubgraph Group\nA\nend\n",
        );

        assert_eq!(fallback_contribution_count(&parsed), 0);
        for path in [
            "themeVariables.clusterBkg",
            "themeVariables.secondaryColor",
            "themeVariables.clusterBorder",
        ] {
            assert_eq!(
                parsed.effective_config.get_str(path),
                baseline.effective_config.get_str(path),
                "typed Cluster paint must not mutate legacy `{path}`"
            );
        }
    }

    #[test]
    fn typed_node_family_edge_stroke_retires_legacy_marker_fallback() {
        for family in [
            DiagramFamilyId::FLOWCHART,
            DiagramFamilyId::SWIMLANE,
            DiagramFamilyId::CLASS,
        ] {
            let spec = DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(
                        ThemeTarget::Edge,
                        ThemeStylePatch::default().with_stroke(solid("#22c55e")),
                    )
                    .for_family(family),
                ),
            );
            let bridge = bridge(&spec);
            let artifact = bridge.compile_for_family(family);

            assert!(artifact.overlay.is_empty());
            assert!(artifact.contribution_ids.is_empty());
            assert!(!bridge.owns_contribution_id(&format!(
                "merman.legacy-family-theme.v1.{}.edge.stroke",
                family.as_str()
            )));
            assert!(!bridge.owns_contribution_id(&format!(
                "merman.legacy-family-theme.v1.{}.marker.paint",
                family.as_str()
            )));
            assert!(!bridge.owns_contribution_id(&format!(
                "merman.legacy-family-theme.v1.{}.marker.paint-from-edge",
                family.as_str()
            )));
        }
    }

    #[test]
    fn unsupported_class_marker_rule_does_not_block_legacy_edge_marker_fallback() {
        let edge = ThemeRule::new(
            ThemeTarget::Edge,
            ThemeStylePatch::default().with_stroke(solid("#22c55e")),
        )
        .with_variant(ThemeVariant::Default)
        .for_family(DiagramFamilyId::CLASS);
        let marker = ThemeRule::new(
            ThemeTarget::Marker,
            ThemeStylePatch::default().with_stroke(solid("#ef4444")),
        )
        .for_family(DiagramFamilyId::CLASS);
        let spec = DiagramThemeSpec::new()
            .with_styles(ThemeRuleSet::default().with_rule(edge).with_rule(marker));
        let parsed = parse(&spec, "classDiagram\nA *-- B\n");

        assert_eq!(
            parsed.effective_config.get_str("themeVariables.lineColor"),
            Some("#22c55e"),
        );
        assert_eq!(
            parsed
                .effective_config
                .get_str("themeVariables.arrowheadColor"),
            Some("#22c55e"),
            "an unsupported Marker rule must not consume the Edge fallback slot",
        );
    }

    #[test]
    fn class_title_fill_projects_to_the_visible_namespace_label_token() {
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::Title,
                    ThemeStylePatch::default().with_fill(solid("#e2e8f0")),
                )
                .for_family(DiagramFamilyId::CLASS),
            ),
        );
        let parsed = parse(
            &spec,
            "classDiagram\nnamespace Platform {\n  class Runtime\n}\n",
        );

        assert_eq!(
            parsed.effective_config.get_str("themeVariables.titleColor"),
            Some("#e2e8f0"),
        );
        assert_eq!(fallback_contribution_count(&parsed), 1);
    }

    #[test]
    fn class_terminal_less_paint_rules_do_not_create_legacy_assignments() {
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default()
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Marker,
                        ThemeStylePatch::default()
                            .with_fill(solid("#0f172a"))
                            .with_stroke(solid("#22c55e")),
                    )
                    .for_family(DiagramFamilyId::CLASS),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::ClusterLabel,
                        ThemeStylePatch::default().with_fill(solid("#f8fafc")),
                    )
                    .for_family(DiagramFamilyId::CLASS),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Table,
                        ThemeStylePatch::default().with_fill(solid("#334155")),
                    )
                    .with_variant(ThemeVariant::Odd)
                    .for_family(DiagramFamilyId::CLASS),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Table,
                        ThemeStylePatch::default().with_fill(solid("#475569")),
                    )
                    .with_variant(ThemeVariant::Even)
                    .for_family(DiagramFamilyId::CLASS),
                ),
        );
        let bridge = bridge(&spec);
        let artifact = bridge.compile_for_family(DiagramFamilyId::CLASS);

        assert!(artifact.overlay.is_empty());
        assert!(artifact.contribution_ids.is_empty());

        let parsed = parse(&spec, "classDiagram\nclass Alpha\n");
        let baseline = parse(&DiagramThemeSpec::default(), "classDiagram\nclass Alpha\n");
        for path in [
            "themeVariables.arrowheadColor",
            "themeVariables.secondaryTextColor",
            "themeVariables.tertiaryTextColor",
            "themeVariables.attributeBackgroundColorOdd",
            "themeVariables.attributeBackgroundColorEven",
            "themeVariables.rowOdd",
            "themeVariables.rowEven",
        ] {
            assert_eq!(
                parsed.effective_config.get_str(path),
                baseline.effective_config.get_str(path),
                "unsupported Class route must not mutate `{path}`"
            );
        }
        assert_eq!(fallback_contribution_count(&parsed), 0);
    }

    #[test]
    fn typed_sequence_message_stroke_suppresses_only_its_legacy_projection() {
        let stroke_spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::Message,
                    ThemeStylePatch::default().with_stroke(solid("#2563eb")),
                )
                .for_family(DiagramFamilyId::SEQUENCE),
            ),
        );
        let bridge = bridge(&stroke_spec);
        let artifact = bridge.compile_for_family(DiagramFamilyId::SEQUENCE);

        assert!(artifact.overlay.is_empty());
        assert!(artifact.contribution_ids.is_empty());
        assert!(
            !bridge.owns_contribution_id("merman.legacy-family-theme.v1.sequence.message.stroke")
        );

        let fill_spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::Message,
                    ThemeStylePatch::default().with_fill(solid("#ef4444")),
                )
                .for_family(DiagramFamilyId::SEQUENCE),
            ),
        );
        let parsed = parse(&fill_spec, "sequenceDiagram\nAlice->>Bob: Hello\n");

        assert_eq!(fallback_contribution_count(&parsed), 1);
        assert_eq!(
            parsed
                .effective_config
                .get_str("themeVariables.signalColor"),
            Some("#ef4444")
        );
    }

    #[test]
    fn typed_sequence_lifeline_paint_suppresses_its_shared_legacy_projection() {
        for style in [
            ThemeStylePatch::default().with_fill(solid("#ef4444")),
            ThemeStylePatch::default().with_stroke(solid("#2563eb")),
            ThemeStylePatch::default()
                .with_fill(solid("#ef4444"))
                .with_stroke(solid("#2563eb")),
        ] {
            let spec = DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                ThemeRule::new(ThemeTarget::Lifeline, style).for_family(DiagramFamilyId::SEQUENCE),
            ));
            let bridge = bridge(&spec);
            let artifact = bridge.compile_for_family(DiagramFamilyId::SEQUENCE);

            assert!(artifact.overlay.is_empty());
            assert!(artifact.contribution_ids.is_empty());
            assert!(
                !bridge
                    .owns_contribution_id("merman.legacy-family-theme.v1.sequence.lifeline.stroke")
            );
        }
    }

    #[test]
    fn typed_sequence_role_paints_suppress_only_their_unqualified_legacy_projections() {
        for (target, style, contribution_id) in [
            (
                ThemeTarget::ActorLabel,
                ThemeStylePatch::default().with_fill(solid("#dc2626")),
                "merman.legacy-family-theme.v1.sequence.actor-label.fill",
            ),
            (
                ThemeTarget::MessageLabel,
                ThemeStylePatch::default().with_fill(solid("#dc2626")),
                "merman.legacy-family-theme.v1.sequence.message-label.fill",
            ),
            (
                ThemeTarget::Loop,
                ThemeStylePatch::default().with_fill(solid("#dc2626")),
                "merman.legacy-family-theme.v1.sequence.loop.fill",
            ),
            (
                ThemeTarget::Loop,
                ThemeStylePatch::default().with_stroke(solid("#2563eb")),
                "merman.legacy-family-theme.v1.sequence.loop.stroke",
            ),
            (
                ThemeTarget::LoopLabel,
                ThemeStylePatch::default().with_fill(solid("#dc2626")),
                "merman.legacy-family-theme.v1.sequence.loop-label.fill",
            ),
            (
                ThemeTarget::NoteLabel,
                ThemeStylePatch::default().with_fill(solid("#dc2626")),
                "merman.legacy-family-theme.v1.sequence.note-label.fill",
            ),
        ] {
            let direct_spec =
                DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(
                    ThemeRule::new(target, style.clone()).for_family(DiagramFamilyId::SEQUENCE),
                ));
            let direct_bridge = bridge(&direct_spec);
            let direct = direct_bridge.compile_for_family(DiagramFamilyId::SEQUENCE);

            assert!(!direct.contribution_ids.contains(contribution_id));
            assert!(!direct_bridge.owns_contribution_id(contribution_id));

            let legacy_spec = DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default().with_rule(
                    ThemeRule::new(target, style)
                        .with_variant(ThemeVariant::Default)
                        .for_family(DiagramFamilyId::SEQUENCE),
                ),
            );
            let legacy_bridge = bridge(&legacy_spec);
            let legacy = legacy_bridge.compile_for_family(DiagramFamilyId::SEQUENCE);

            assert!(legacy.contribution_ids.contains(contribution_id));
            assert!(legacy_bridge.owns_contribution_id(contribution_id));
        }
    }

    #[test]
    fn explicit_marker_paint_remains_legacy_when_edge_stroke_is_typed() {
        for family in [DiagramFamilyId::FLOWCHART, DiagramFamilyId::SWIMLANE] {
            let spec = DiagramThemeSpec::new().with_styles(
                ThemeRuleSet::default()
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::Edge,
                            ThemeStylePatch::default().with_stroke(solid("#22c55e")),
                        )
                        .for_family(family),
                    )
                    .with_rule(
                        ThemeRule::new(
                            ThemeTarget::Marker,
                            ThemeStylePatch::default().with_stroke(solid("#d946ef")),
                        )
                        .for_family(family),
                    ),
            );
            let bridge = bridge(&spec);
            let artifact = bridge.compile_for_family(family);

            assert_eq!(artifact.contribution_ids.len(), 1);
            assert!(bridge.owns_contribution_id(&format!(
                "merman.legacy-family-theme.v1.{}.marker.paint",
                family.as_str()
            )));
            assert!(!bridge.owns_contribution_id(&format!(
                "merman.legacy-family-theme.v1.{}.marker.paint-from-edge",
                family.as_str()
            )));
            assert!(!bridge.owns_contribution_id(&format!(
                "merman.legacy-family-theme.v1.{}.edge.stroke",
                family.as_str()
            )));
        }
    }

    #[test]
    fn contribution_ownership_is_exact_and_recipe_deterministic() {
        let spec =
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::Node,
                ThemeStylePatch::default().with_fill(solid("#ef4444")),
            )));
        let bridge = bridge(&spec);
        assert_eq!(bridge.cached_family_count(), 0);

        assert!(!bridge.owns_contribution_id("merman.legacy-family-theme.v1.flowchart.node.fill"));
        assert_eq!(bridge.cached_family_count(), 1);
        assert!(
            !bridge
                .owns_contribution_id("merman.legacy-family-theme.v1.flowchart.node.fill.forged")
        );
        assert!(!bridge.owns_contribution_id("merman.legacy-family-theme.v1.sequence.node.fill"));
    }

    #[test]
    fn overlay_builder_keeps_the_first_assignment_without_panicking() {
        let mut builder = OverlayBuilder::new(DiagramFamilyId::FLOWCHART);
        let mut direct = FamilyContributions::new(DiagramFamilyId::FLOWCHART);
        direct.add_theme_variables("direct", [("primaryColor", Some("#ef4444".to_string()))]);
        direct.finish_into(&mut builder);

        let mut duplicate = FamilyContributions::new(DiagramFamilyId::FLOWCHART);
        duplicate.add_theme_variables("duplicate", [("primaryColor", Some("#22c55e".to_string()))]);
        duplicate.finish_into(&mut builder);

        let (_, contribution_ids, _) = builder.finish();
        assert!(contribution_ids.contains("merman.legacy-family-theme.v1.flowchart.direct"));
        assert!(!contribution_ids.contains("merman.legacy-family-theme.v1.flowchart.duplicate"));
    }

    #[test]
    fn bridge_does_not_inject_a_global_base_theme() {
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::Node,
                    ThemeStylePatch::default().with_fill(solid("#ef4444")),
                )
                .for_family(DiagramFamilyId::FLOWCHART),
            ),
        );
        let bridge = bridge(&spec);
        let artifact = bridge.compile_for_family(DiagramFamilyId::FLOWCHART);
        assert!(
            !artifact
                .contribution_ids
                .iter()
                .any(|id| id.ends_with(".theme-base"))
        );
        let parsed = parse(&spec, "flowchart LR\nA --> B\n");
        assert_ne!(parsed.effective_config.get_str("theme"), Some("base"));
    }

    #[test]
    fn unsupported_paint_does_not_create_a_direct_assignment() {
        let gradient = CanvasPaint::LinearGradient(
            super::super::LinearGradient::new(
                0.0,
                [
                    super::super::GradientStop::new(
                        0.0,
                        super::super::ThemeColorValue::parse("#ef4444").unwrap(),
                    )
                    .unwrap(),
                    super::super::GradientStop::new(
                        1.0,
                        super::super::ThemeColorValue::parse("#3b82f6").unwrap(),
                    )
                    .unwrap(),
                ],
            )
            .expect("valid test gradient"),
        );
        let mut edge = ThemeStylePatch::default().with_fill(solid("#22c55e"));
        edge.stroke.paint = Specified::Value(gradient);
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(ThemeRule::new(ThemeTarget::Edge, edge)),
        );
        let bridge = bridge(&spec);
        bridge.compile_for_family(DiagramFamilyId::FLOWCHART);
        assert!(
            !bridge.owns_contribution_id("merman.legacy-family-theme.v1.flowchart.edge.stroke")
        );
    }

    #[test]
    fn family_clear_blocks_an_inherited_public_fill() {
        let mut clear = ThemeStylePatch::default();
        clear.paint.fill = Specified::Clear;
        clear.stroke.paint = Specified::Clear;
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default()
                .with_rule(ThemeRule::new(
                    ThemeTarget::Node,
                    ThemeStylePatch::default().with_fill(solid("#f8fafc")),
                ))
                .with_rule(
                    ThemeRule::new(ThemeTarget::Node, clear).for_family(DiagramFamilyId::FLOWCHART),
                ),
        );

        let flowchart = parse(&spec, "flowchart LR\nA --> B\n");
        assert_ne!(
            flowchart
                .effective_config
                .get_str("themeVariables.primaryColor"),
            Some("#f8fafc")
        );
    }

    #[test]
    fn task_and_requirement_families_use_their_own_programs() {
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default()
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Task,
                        ThemeStylePatch::default()
                            .with_fill(solid("#f8fafc"))
                            .with_stroke(solid("#94a3b8")),
                    )
                    .for_family(DiagramFamilyId::GANTT),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Task,
                        ThemeStylePatch::default()
                            .with_fill(solid("#f1f5f9"))
                            .with_stroke(solid("#64748b")),
                    )
                    .for_family(DiagramFamilyId::GANTT)
                    .with_variant(ThemeVariant::Active),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Task,
                        ThemeStylePatch::default().with_stroke(solid("#059669")),
                    )
                    .for_family(DiagramFamilyId::GANTT)
                    .with_variant(ThemeVariant::Success),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Task,
                        ThemeStylePatch::default().with_stroke(solid("#dc2626")),
                    )
                    .for_family(DiagramFamilyId::GANTT)
                    .with_variant(ThemeVariant::Error),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Requirement,
                        ThemeStylePatch::default()
                            .with_fill(solid("#f8fafc"))
                            .with_stroke(solid("#94a3b8")),
                    )
                    .for_family(DiagramFamilyId::REQUIREMENT),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Relation,
                        ThemeStylePatch::default().with_stroke(solid("#64748b")),
                    )
                    .for_family(DiagramFamilyId::REQUIREMENT),
                ),
        );
        let gantt_baseline = parse(&DiagramThemeSpec::new(), GANTT_FIXTURE);
        let gantt = parse(&spec, GANTT_FIXTURE);
        assert_eq!(
            gantt
                .effective_config
                .get_str("themeVariables.taskBkgColor"),
            gantt_baseline
                .effective_config
                .get_str("themeVariables.taskBkgColor")
        );
        assert_eq!(
            gantt
                .effective_config
                .get_str("themeVariables.activeTaskBkgColor"),
            Some("#f1f5f9")
        );
        assert_eq!(
            gantt
                .effective_config
                .get_str("themeVariables.taskBorderColor"),
            Some("#94a3b8")
        );
        assert_eq!(
            gantt
                .effective_config
                .get_str("themeVariables.doneTaskBorderColor"),
            Some("#059669")
        );
        assert_eq!(
            gantt
                .effective_config
                .get_str("themeVariables.critBorderColor"),
            Some("#dc2626")
        );
        assert!(fallback_contribution_count(&gantt) > 0);
        let gantt_bridge = bridge(&spec).compile_for_family(DiagramFamilyId::GANTT);
        assert!(
            !gantt_bridge
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.gantt.task.default.fill")
        );
        assert!(
            gantt_bridge
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.gantt.task.active.fill")
        );
        for mapping in ["task.default", "task.active", "task.success", "task.error"] {
            assert!(
                gantt_bridge
                    .contribution_ids
                    .contains(&format!("merman.legacy-family-theme.v1.gantt.{mapping}"))
            );
        }

        let requirement_baseline = parse(&DiagramThemeSpec::new(), REQUIREMENT_FIXTURE);
        let requirement = parse(&spec, REQUIREMENT_FIXTURE);
        assert_eq!(
            requirement
                .effective_config
                .get_str("themeVariables.requirementBackground"),
            requirement_baseline
                .effective_config
                .get_str("themeVariables.requirementBackground")
        );
        assert_eq!(
            requirement
                .effective_config
                .get_str("themeVariables.requirementBorderColor"),
            Some("#94a3b8")
        );
        assert_eq!(
            requirement
                .effective_config
                .get_str("themeVariables.relationColor"),
            Some("#64748b")
        );
        assert!(fallback_contribution_count(&requirement) > 0);
        let requirement_bridge = bridge(&spec).compile_for_family(DiagramFamilyId::REQUIREMENT);
        assert!(
            !requirement_bridge
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.requirement.requirement.fill")
        );
        assert!(
            requirement_bridge
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.requirement.requirement.paint")
        );
        assert!(
            requirement_bridge
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.requirement.relation.paint")
        );
    }

    #[test]
    fn direct_pie_xy_and_radar_palettes_do_not_create_bridge_contributions() {
        let palette = super::super::OrdinalPalette::new([
            super::super::ThemeColorValue::parse("#2563eb").expect("valid palette color"),
            super::super::ThemeColorValue::parse("#16a34a").expect("valid palette color"),
            super::super::ThemeColorValue::parse("#d97706").expect("valid palette color"),
            super::super::ThemeColorValue::parse("#9333ea").expect("valid palette color"),
        ])
        .expect("non-empty palette");
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default()
                .with_ordinal_palette(ThemeTarget::PieSlice, palette.clone())
                .with_ordinal_palette(ThemeTarget::ChartSeries, palette.clone())
                .with_ordinal_palette(ThemeTarget::JourneyTask, palette),
        );

        let baseline = parse(&DiagramThemeSpec::new(), PIE_FIXTURE);
        let pie = parse(&spec, PIE_FIXTURE);
        assert_eq!(
            pie.effective_config.get_str("themeVariables.pie1"),
            baseline.effective_config.get_str("themeVariables.pie1")
        );
        assert_eq!(fallback_contribution_count(&pie), 0);

        let xy = bridge(&spec).compile_for_family(DiagramFamilyId::XY_CHART);
        assert!(
            !xy.contribution_ids
                .contains("merman.legacy-family-theme.v1.xychart.series.palette")
        );

        let radar = bridge(&spec).compile_for_family(DiagramFamilyId::RADAR);
        assert!(
            !radar
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.radar.series.palette")
        );

        let journey = parse(&spec, JOURNEY_FIXTURE);
        assert_eq!(
            journey.effective_config.get_str("themeVariables.fillType0"),
            Some("#2563eb")
        );
        assert!(fallback_contribution_count(&journey) > 0);
    }
}
