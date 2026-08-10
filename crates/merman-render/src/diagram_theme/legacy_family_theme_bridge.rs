use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

use merman_core::__private::{
    ThemeFamilyCompatibilityOverlay, ThemeFamilyCompatibilityOverlayBuilder,
};
use merman_core::theme_color::{ColorChannel, ThemeColor};
use merman_core::{MermaidConfig, ParseControl, ParseControlResult};
use serde_json::{Map, Value};

use crate::render_family::RenderFamilyKind;

use super::DiagramThemeSpec;
use super::canvas::CanvasPaint;
use super::family_program::{FamilyThemeProgram, FamilyThemeProgramCache};
use super::resolved::ResolvedThemeStyle;
use super::semantic::{ThemeTarget, ThemeVariant};
use super::tokens::FrozenLegacyThemeCompatibility;
use super::typography::{Specified, TextStyle};

pub(super) const CONTRIBUTION_ID_PREFIX: &str = "merman.legacy-family-theme.v1.";
const MAX_LEGACY_ASSIGNMENT_STRING_BYTES: usize = 4 * 1024;

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
    spec: Arc<DiagramThemeSpec>,
    family_programs: Arc<FamilyThemeProgramCache>,
    artifacts: Mutex<HashMap<RenderFamilyKind, Arc<LegacyFamilyThemeArtifact>>>,
}

#[derive(Debug)]
struct LegacyFamilyThemeArtifact {
    overlay: ThemeFamilyCompatibilityOverlay,
    #[cfg(test)]
    contribution_ids: BTreeSet<String>,
}

impl LegacyFamilyThemeBridge {
    pub(super) fn new(
        spec: Arc<DiagramThemeSpec>,
        family_programs: Arc<FamilyThemeProgramCache>,
    ) -> Self {
        Self {
            inner: Arc::new(LegacyFamilyThemeBridgeInner {
                spec,
                family_programs,
                artifacts: Mutex::new(HashMap::new()),
            }),
        }
    }

    fn artifact_for_family(&self, family: RenderFamilyKind) -> Arc<LegacyFamilyThemeArtifact> {
        let mut artifacts = self
            .inner
            .artifacts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        artifacts
            .entry(family)
            .or_insert_with(|| {
                Arc::new(compile_selected_family(
                    &self.inner.spec,
                    &self.inner.family_programs,
                    family,
                ))
            })
            .clone()
    }

    #[cfg(test)]
    fn compile_for_family(&self, family: RenderFamilyKind) -> Arc<LegacyFamilyThemeArtifact> {
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
        control: &ParseControl,
    ) -> ParseControlResult<Option<ThemeFamilyCompatibilityOverlay>> {
        control.checkpoint()?;
        let Some(family) = render_family_from_core_name(family) else {
            return Ok(None);
        };
        // State owns its compatibility path in the typed adapter and must never be projected
        // through the legacy Mermaid lane.
        if family == RenderFamilyKind::State {
            return Ok(None);
        }
        let artifact = self.artifact_for_family(family);
        control.checkpoint()?;
        Ok((!artifact.overlay.is_empty()).then(|| artifact.overlay.clone()))
    }
}

#[cfg(test)]
fn contribution_family(opaque_id: &str) -> Option<RenderFamilyKind> {
    let family = opaque_id
        .strip_prefix(CONTRIBUTION_ID_PREFIX)?
        .split_once('.')?
        .0;
    render_family_from_core_name(family)
}

fn render_family_from_core_name(name: &str) -> Option<RenderFamilyKind> {
    [
        RenderFamilyKind::Error,
        RenderFamilyKind::Mindmap,
        RenderFamilyKind::State,
        RenderFamilyKind::Sequence,
        RenderFamilyKind::Zenuml,
        RenderFamilyKind::Flowchart,
        RenderFamilyKind::Swimlane,
        RenderFamilyKind::Architecture,
        RenderFamilyKind::Class,
        RenderFamilyKind::C4,
        RenderFamilyKind::Cynefin,
        RenderFamilyKind::Wardley,
        RenderFamilyKind::Railroad,
        RenderFamilyKind::Kanban,
        RenderFamilyKind::Gantt,
        RenderFamilyKind::Pie,
        RenderFamilyKind::Packet,
        RenderFamilyKind::Timeline,
        RenderFamilyKind::Journey,
        RenderFamilyKind::Requirement,
        RenderFamilyKind::Sankey,
        RenderFamilyKind::Radar,
        RenderFamilyKind::Info,
        RenderFamilyKind::Treemap,
        RenderFamilyKind::Block,
        RenderFamilyKind::Er,
        RenderFamilyKind::QuadrantChart,
        RenderFamilyKind::XyChart,
        RenderFamilyKind::GitGraph,
        RenderFamilyKind::TreeView,
        RenderFamilyKind::Ishikawa,
        RenderFamilyKind::EventModeling,
        RenderFamilyKind::Venn,
    ]
    .into_iter()
    .find(|candidate| candidate.as_str() == name)
}

fn compile_selected_family(
    spec: &DiagramThemeSpec,
    family_programs: &FamilyThemeProgramCache,
    family: RenderFamilyKind,
) -> LegacyFamilyThemeArtifact {
    let mut builder = OverlayBuilder::new(family);
    match family {
        RenderFamilyKind::Flowchart
        | RenderFamilyKind::Swimlane
        | RenderFamilyKind::Class
        | RenderFamilyKind::Mindmap
        | RenderFamilyKind::TreeView
        | RenderFamilyKind::Block
        | RenderFamilyKind::GitGraph => {
            compile_node_family(&mut builder, spec, family_programs, family);
        }
        RenderFamilyKind::Sequence => {
            compile_sequence_family(&mut builder, spec, family_programs);
        }
        RenderFamilyKind::Gantt | RenderFamilyKind::Kanban => {
            compile_task_family(&mut builder, spec, family_programs, family);
        }
        RenderFamilyKind::Requirement => {
            compile_requirement_family(&mut builder, spec, family_programs);
        }
        RenderFamilyKind::Er => {
            compile_er_family(&mut builder, spec, family_programs);
        }
        RenderFamilyKind::Pie => {
            compile_pie_family(&mut builder, spec, family_programs);
        }
        RenderFamilyKind::XyChart | RenderFamilyKind::QuadrantChart | RenderFamilyKind::Radar => {
            compile_chart_family(&mut builder, spec, family_programs, family);
        }
        RenderFamilyKind::Timeline => {
            compile_timeline_family(&mut builder, spec, family_programs);
        }
        RenderFamilyKind::Journey => {
            compile_journey_family(&mut builder, spec, family_programs);
        }
        RenderFamilyKind::Error
        | RenderFamilyKind::Zenuml
        | RenderFamilyKind::Architecture
        | RenderFamilyKind::C4
        | RenderFamilyKind::Cynefin
        | RenderFamilyKind::Wardley
        | RenderFamilyKind::Railroad
        | RenderFamilyKind::Packet
        | RenderFamilyKind::Sankey
        | RenderFamilyKind::Info
        | RenderFamilyKind::Treemap
        | RenderFamilyKind::Ishikawa
        | RenderFamilyKind::EventModeling
        | RenderFamilyKind::Venn => {
            compile_text_family(&mut builder, spec, family_programs, family);
        }
        RenderFamilyKind::State => {}
    }
    // Legacy palette/config values are deliberately appended after direct family mappings. The
    // local builder admits each assignment path once, so direct mappings deterministically win
    // without passing duplicate assignments to the core overlay.
    compile_frozen_legacy_family(&mut builder, spec, family_programs, family);
    let (overlay, contribution_ids) = builder.finish();
    #[cfg(not(test))]
    let _ = &contribution_ids;
    LegacyFamilyThemeArtifact {
        overlay,
        #[cfg(test)]
        contribution_ids,
    }
}

fn compile_node_family(
    builder: &mut OverlayBuilder,
    spec: &DiagramThemeSpec,
    family_programs: &FamilyThemeProgramCache,
    family: RenderFamilyKind,
) {
    let reader = FamilyStyleReader::new(spec, family_programs, family);
    let mut contributions = FamilyContributions::new(family);

    contributions.add_typography(reader.base_typography());
    contributions.add_theme_variables(
        "node.fill",
        [
            ("primaryColor", reader.fill(ThemeTarget::Node)),
            ("mainBkg", reader.fill(ThemeTarget::Node)),
        ],
    );
    contributions.add_theme_variables(
        "node.stroke",
        [
            ("primaryBorderColor", reader.stroke(ThemeTarget::Node)),
            ("nodeBorder", reader.stroke(ThemeTarget::Node)),
        ],
    );
    contributions.add_theme_variables(
        "node-label.fill",
        [
            ("primaryTextColor", reader.text_fill(ThemeTarget::NodeLabel)),
            ("nodeTextColor", reader.text_fill(ThemeTarget::NodeLabel)),
            ("textColor", reader.text_fill(ThemeTarget::NodeLabel)),
        ],
    );
    contributions.add_theme_variables(
        "title.fill",
        [("titleColor", reader.text_fill(ThemeTarget::Title))],
    );
    contributions.add_theme_variables(
        "edge.stroke",
        [("lineColor", reader.stroke_or_fill(ThemeTarget::Edge))],
    );
    contributions.add_theme_variables(
        "marker.paint",
        [("arrowheadColor", reader.marker_or_edge_paint())],
    );
    contributions.add_theme_variables(
        "edge-label-background.fill",
        [(
            "edgeLabelBackground",
            reader.fill(ThemeTarget::EdgeLabelBackground),
        )],
    );
    contributions.add_theme_variables(
        "cluster.fill",
        [
            ("clusterBkg", reader.fill(ThemeTarget::Cluster)),
            ("secondaryColor", reader.fill(ThemeTarget::Cluster)),
        ],
    );
    contributions.add_theme_variables(
        "cluster.stroke",
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
        RenderFamilyKind::Class => {
            contributions.add_theme_variables(
                "class.text",
                [
                    ("classText", reader.text_fill(ThemeTarget::NodeLabel)),
                    ("labelColor", reader.text_fill(ThemeTarget::NodeLabel)),
                ],
            );
            contributions.add_theme_variables(
                "table.odd.fill",
                [
                    (
                        "attributeBackgroundColorOdd",
                        reader.fill_variant(ThemeTarget::Table, ThemeVariant::Odd),
                    ),
                    (
                        "rowOdd",
                        reader.fill_variant(ThemeTarget::Table, ThemeVariant::Odd),
                    ),
                ],
            );
            contributions.add_theme_variables(
                "table.even.fill",
                [
                    (
                        "attributeBackgroundColorEven",
                        reader.fill_variant(ThemeTarget::Table, ThemeVariant::Even),
                    ),
                    (
                        "rowEven",
                        reader.fill_variant(ThemeTarget::Table, ThemeVariant::Even),
                    ),
                ],
            );
        }
        RenderFamilyKind::TreeView => {
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
        RenderFamilyKind::GitGraph => {
            contributions.add_theme_variables(
                "git.commit",
                [
                    ("commitLineColor", reader.stroke_or_fill(ThemeTarget::Edge)),
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
            contributions.add_palette(
                "node.palette",
                reader.palette(ThemeTarget::Node),
                PaletteProjection::Git { limit: 64 },
                reader.canvas_color(),
            );
        }
        RenderFamilyKind::Mindmap => {
            contributions.add_palette(
                "node.palette",
                reader.palette(ThemeTarget::Node),
                PaletteProjection::ColorScale { limit: 64 },
                reader.canvas_color(),
            );
        }
        _ => {}
    }

    contributions.finish_into(builder);
}

fn compile_sequence_family(
    builder: &mut OverlayBuilder,
    spec: &DiagramThemeSpec,
    family_programs: &FamilyThemeProgramCache,
) {
    let family = RenderFamilyKind::Sequence;
    let reader = FamilyStyleReader::new(spec, family_programs, family);
    let mut contributions = FamilyContributions::new(family);

    contributions.add_typography(reader.base_typography());
    contributions.add_theme_variables(
        "actor.fill",
        [("actorBkg", reader.fill(ThemeTarget::Actor))],
    );
    contributions.add_theme_variables(
        "actor.stroke",
        [("actorBorder", reader.stroke(ThemeTarget::Actor))],
    );
    contributions.add_theme_variables(
        "actor-label.fill",
        [("actorTextColor", reader.text_fill(ThemeTarget::ActorLabel))],
    );
    contributions.add_theme_variables(
        "lifeline.stroke",
        [(
            "actorLineColor",
            reader.stroke_or_fill(ThemeTarget::Lifeline),
        )],
    );
    contributions.add_theme_variables(
        "message.stroke",
        [("signalColor", reader.stroke_or_fill(ThemeTarget::Message))],
    );
    contributions.add_theme_variables(
        "message-label.fill",
        [(
            "signalTextColor",
            reader.text_fill(ThemeTarget::MessageLabel),
        )],
    );
    contributions.add_theme_variables(
        "loop.fill",
        [("labelBoxBkgColor", reader.fill(ThemeTarget::Loop))],
    );
    contributions.add_theme_variables(
        "loop.stroke",
        [("labelBoxBorderColor", reader.stroke(ThemeTarget::Loop))],
    );
    contributions.add_theme_variables(
        "loop-label.fill",
        [
            ("labelTextColor", reader.text_fill(ThemeTarget::LoopLabel)),
            ("loopTextColor", reader.text_fill(ThemeTarget::LoopLabel)),
        ],
    );
    contributions.add_theme_variables(
        "activation.fill",
        [("activationBkgColor", reader.fill(ThemeTarget::Activation))],
    );
    contributions.add_theme_variables(
        "activation.stroke",
        [(
            "activationBorderColor",
            reader.stroke(ThemeTarget::Activation),
        )],
    );
    contributions.add_theme_variables(
        "note.fill",
        [("noteBkgColor", reader.fill(ThemeTarget::Note))],
    );
    contributions.add_theme_variables(
        "note.stroke",
        [("noteBorderColor", reader.stroke(ThemeTarget::Note))],
    );
    contributions.add_theme_variables(
        "note-label.fill",
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
    spec: &DiagramThemeSpec,
    family_programs: &FamilyThemeProgramCache,
    family: RenderFamilyKind,
) {
    let reader = FamilyStyleReader::new(spec, family_programs, family);
    let mut contributions = FamilyContributions::new(family);

    contributions.add_typography(reader.base_typography());
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
        RenderFamilyKind::Gantt => {
            for (mapping, variant, fill_key, stroke_key) in [
                (
                    "task.default",
                    ThemeVariant::Default,
                    "taskBkgColor",
                    "taskBorderColor",
                ),
                (
                    "task.active",
                    ThemeVariant::Active,
                    "activeTaskBkgColor",
                    "activeTaskBorderColor",
                ),
                (
                    "task.success",
                    ThemeVariant::Success,
                    "doneTaskBkgColor",
                    "doneTaskBorderColor",
                ),
                (
                    "task.error",
                    ThemeVariant::Error,
                    "critBkgColor",
                    "critBorderColor",
                ),
            ] {
                contributions.add_theme_variables(
                    mapping,
                    [
                        (fill_key, reader.fill_variant(ThemeTarget::Task, variant)),
                        (
                            stroke_key,
                            reader.stroke_variant(ThemeTarget::Task, variant),
                        ),
                    ],
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
        RenderFamilyKind::Kanban => {
            contributions.add_theme_variables(
                "task.default",
                [("nodeBorder", reader.stroke(ThemeTarget::Task))],
            );
            contributions.add_palette(
                "task.palette.color-scale",
                reader.palette(ThemeTarget::Task),
                PaletteProjection::ColorScale { limit: 12 },
                reader.canvas_color(),
            );
            contributions.add_palette(
                "task.palette.git",
                reader.palette(ThemeTarget::Task),
                PaletteProjection::Git { limit: 12 },
                reader.canvas_color(),
            );
        }
        _ => unreachable!("task compatibility is limited to Gantt and Kanban"),
    }

    contributions.finish_into(builder);
}

fn compile_requirement_family(
    builder: &mut OverlayBuilder,
    spec: &DiagramThemeSpec,
    family_programs: &FamilyThemeProgramCache,
) {
    let family = RenderFamilyKind::Requirement;
    let reader = FamilyStyleReader::new(spec, family_programs, family);
    let mut contributions = FamilyContributions::new(family);

    contributions.add_typography(reader.base_typography());
    contributions.add_theme_variables(
        "requirement.paint",
        [
            (
                "requirementBackground",
                reader.fill(ThemeTarget::Requirement),
            ),
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

fn compile_er_family(
    builder: &mut OverlayBuilder,
    spec: &DiagramThemeSpec,
    family_programs: &FamilyThemeProgramCache,
) {
    let family = RenderFamilyKind::Er;
    let reader = FamilyStyleReader::new(spec, family_programs, family);
    let mut contributions = FamilyContributions::new(family);

    contributions.add_typography(reader.base_typography());
    contributions.add_theme_variables(
        "entity.paint",
        [
            ("mainBkg", reader.fill(ThemeTarget::Requirement)),
            ("primaryColor", reader.fill(ThemeTarget::Requirement)),
            ("nodeBorder", reader.stroke(ThemeTarget::Requirement)),
        ],
    );
    contributions.add_theme_variables(
        "entity.text",
        [
            ("textColor", reader.text_fill(ThemeTarget::Text)),
            ("nodeTextColor", reader.text_fill(ThemeTarget::Text)),
            ("titleColor", reader.text_fill(ThemeTarget::Title)),
        ],
    );
    contributions.add_theme_variables(
        "relation.paint",
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

fn compile_pie_family(
    builder: &mut OverlayBuilder,
    spec: &DiagramThemeSpec,
    family_programs: &FamilyThemeProgramCache,
) {
    let family = RenderFamilyKind::Pie;
    let reader = FamilyStyleReader::new(spec, family_programs, family);
    let mut contributions = FamilyContributions::new(family);

    contributions.add_typography(reader.base_typography());
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
    let palette = reader.palette(ThemeTarget::PieSlice).or_else(|| {
        reader
            .fill(ThemeTarget::PieSlice)
            .map(|color| vec![color; 12])
    });
    contributions.add_palette(
        "slice.palette",
        palette,
        PaletteProjection::Pie { limit: 12 },
        None,
    );
    contributions.finish_into(builder);
}

fn compile_chart_family(
    builder: &mut OverlayBuilder,
    spec: &DiagramThemeSpec,
    family_programs: &FamilyThemeProgramCache,
    family: RenderFamilyKind,
) {
    let reader = FamilyStyleReader::new(spec, family_programs, family);
    let mut contributions = FamilyContributions::new(family);
    let text = reader.text_fill(ThemeTarget::Text);
    let title = reader.text_fill(ThemeTarget::Title);
    let axis_text = reader.text_fill(ThemeTarget::Axis);
    let axis_line = reader.stroke_or_fill(ThemeTarget::Axis);
    contributions.add_typography(reader.base_typography());
    match family {
        RenderFamilyKind::XyChart => {
            let palette = reader.palette(ThemeTarget::ChartSeries);
            contributions.add_xy_palette("series.palette", palette);
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
        RenderFamilyKind::QuadrantChart => {
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
        RenderFamilyKind::Radar => {
            let palette = reader.palette(ThemeTarget::ChartSeries);
            contributions.add_palette(
                "series.palette",
                palette,
                PaletteProjection::ColorScale { limit: 12 },
                reader.canvas_color(),
            );
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
    spec: &DiagramThemeSpec,
    family_programs: &FamilyThemeProgramCache,
) {
    let family = RenderFamilyKind::Timeline;
    let reader = FamilyStyleReader::new(spec, family_programs, family);
    let mut contributions = FamilyContributions::new(family);
    contributions.add_typography(reader.base_typography());
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
        reader.canvas_color(),
    );
    contributions.finish_into(builder);
}

fn compile_journey_family(
    builder: &mut OverlayBuilder,
    spec: &DiagramThemeSpec,
    family_programs: &FamilyThemeProgramCache,
) {
    let family = RenderFamilyKind::Journey;
    let reader = FamilyStyleReader::new(spec, family_programs, family);
    let mut contributions = FamilyContributions::new(family);
    contributions.add_typography(reader.base_typography());
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
        PaletteProjection::Journey {
            task_limit: 8,
            actor_limit: 0,
        },
        None,
    );
    contributions.finish_into(builder);
}

fn compile_text_family(
    builder: &mut OverlayBuilder,
    spec: &DiagramThemeSpec,
    family_programs: &FamilyThemeProgramCache,
    family: RenderFamilyKind,
) {
    // Only the family-neutral Text and Title targets are direct here. Renderer-specific keys for
    // Venn, Packet, Treemap, C4, and similar families remain frozen legacy compatibility until a
    // typed target exists for them.
    let reader = FamilyStyleReader::new(spec, family_programs, family);
    let mut contributions = FamilyContributions::new(family);
    contributions.add_typography(reader.base_typography());
    // Packet has no typed consumer for the family-neutral Text/Title compatibility variables.
    // Keep its renderer-specific frozen packet roles instead of writing unused global values.
    if family != RenderFamilyKind::Packet {
        contributions.add_theme_variables(
            "text.fill",
            [
                ("textColor", reader.text_fill(ThemeTarget::Text)),
                ("titleColor", reader.text_fill(ThemeTarget::Title)),
            ],
        );
    }
    contributions.finish_into(builder);
}

fn compile_frozen_legacy_family(
    builder: &mut OverlayBuilder,
    spec: &DiagramThemeSpec,
    family_programs: &FamilyThemeProgramCache,
    family: RenderFamilyKind,
) {
    let Some(frozen) = spec.frozen_legacy_compatibility() else {
        return;
    };
    let color = |value: &super::canvas::ThemeColorValue| value.as_css();
    let palette = || frozen.series.iter().map(color).collect::<Vec<_>>();
    let text = color(&frozen.text);

    match family {
        RenderFamilyKind::Gantt => {
            let mut contributions = FamilyContributions::new(family);
            contributions.add_theme_variables(
                "frozen.sections",
                [
                    ("sectionBkgColor", Some(color(&frozen.cluster_background))),
                    ("sectionBkgColor2", Some(color(&frozen.surface_muted))),
                    ("altSectionBkgColor", Some(color(&frozen.canvas))),
                    ("gridColor", Some(color(&frozen.border))),
                    ("excludeBkgColor", Some(color(&frozen.surface_alt))),
                ],
            );
            contributions.finish_into(builder);
        }
        RenderFamilyKind::Kanban => {
            let mut contributions = FamilyContributions::new(family);
            if !family_has_palette_or_resolved_fill(
                spec,
                family_programs,
                family,
                ThemeTarget::Task,
            ) {
                contributions.add_palette(
                    "frozen.series.color-scale",
                    Some(palette()),
                    PaletteProjection::ColorScale { limit: 12 },
                    Some(color(&frozen.canvas)),
                );
                contributions.add_palette(
                    "frozen.series.git",
                    Some(palette()),
                    PaletteProjection::Git { limit: 12 },
                    Some(color(&frozen.canvas)),
                );
            }
            contributions.finish_into(builder);
        }
        RenderFamilyKind::Requirement => {
            let mut contributions = FamilyContributions::new(family);
            contributions.add_theme_variables(
                "frozen.relation-label",
                [
                    (
                        "relationLabelBackground",
                        Some(color(&frozen.edge_label_background)),
                    ),
                    (
                        "requirementEdgeLabelBackground",
                        Some(color(&frozen.edge_label_background)),
                    ),
                    (
                        "edgeLabelBackground",
                        Some(color(&frozen.edge_label_background)),
                    ),
                ],
            );
            contributions.finish_into(builder);
        }
        RenderFamilyKind::Class => {
            let mut contributions = FamilyContributions::new(family);
            contributions.add_theme_variables(
                "frozen.note",
                [
                    ("noteBkgColor", Some(color(&frozen.note_background))),
                    ("noteBorderColor", Some(color(&frozen.note_border))),
                    ("noteTextColor", Some(color(&frozen.note_text))),
                ],
            );
            contributions.finish_into(builder);
        }
        RenderFamilyKind::Mindmap => {
            let mut contributions = FamilyContributions::new(family);
            if !family_has_palette_or_resolved_fill(
                spec,
                family_programs,
                family,
                ThemeTarget::Node,
            ) {
                contributions.add_palette(
                    "frozen.series",
                    Some(palette()),
                    PaletteProjection::ColorScale { limit: 12 },
                    Some(color(&frozen.canvas)),
                );
            }
            contributions.finish_into(builder);
        }
        RenderFamilyKind::Timeline
            if !family_has_palette_or_resolved_fill(
                spec,
                family_programs,
                family,
                ThemeTarget::TimelineEvent,
            ) =>
        {
            let mut contributions = FamilyContributions::new(family);
            contributions.add_palette(
                "frozen.series",
                Some(palette()),
                PaletteProjection::ColorScale { limit: 12 },
                Some(color(&frozen.canvas)),
            );
            contributions.finish_into(builder);
        }
        RenderFamilyKind::GitGraph => {
            let mut contributions = FamilyContributions::new(family);
            if !family_has_palette_or_resolved_fill(
                spec,
                family_programs,
                family,
                ThemeTarget::Node,
            ) {
                contributions.add_palette(
                    "frozen.series",
                    Some(palette()),
                    PaletteProjection::Git { limit: 64 },
                    Some(color(&frozen.canvas)),
                );
            }
            contributions.finish_into(builder);
        }
        RenderFamilyKind::Pie if !has_direct_pie_palette(spec, family_programs) => {
            let mut contributions = FamilyContributions::new(family);
            contributions.add_palette(
                "frozen.series",
                Some(palette()),
                PaletteProjection::Pie { limit: 12 },
                None,
            );
            contributions.finish_into(builder);
        }
        RenderFamilyKind::QuadrantChart => {
            let mut contributions = FamilyContributions::new(family);
            contributions.add_theme_variables(
                "frozen.quadrants",
                [
                    ("quadrant1Fill", Some(color(&frozen.surface))),
                    ("quadrant2Fill", Some(color(&frozen.surface_alt))),
                    ("quadrant3Fill", Some(color(&frozen.canvas))),
                    ("quadrant4Fill", Some(color(&frozen.surface_muted))),
                ],
            );
            contributions.finish_into(builder);
        }
        RenderFamilyKind::Radar => {
            let mut contributions = FamilyContributions::new(family);
            let mut radar_config = Map::new();
            radar_config.insert(
                "graticuleColor".to_string(),
                Value::String(color(&frozen.border)),
            );
            contributions.add_root_object("frozen.graticule", "radar", radar_config);
            if !family_has_palette_or_resolved_fill(
                spec,
                family_programs,
                family,
                ThemeTarget::ChartSeries,
            ) {
                contributions.add_palette(
                    "frozen.series",
                    Some(palette()),
                    PaletteProjection::ColorScale { limit: 12 },
                    Some(color(&frozen.canvas)),
                );
            }
            contributions.finish_into(builder);
        }
        RenderFamilyKind::Journey => {
            let mut contributions = FamilyContributions::new(family);
            if !family_has_palette_or_resolved_fill(
                spec,
                family_programs,
                family,
                ThemeTarget::JourneyTask,
            ) {
                contributions.add_palette(
                    "frozen.series.tasks",
                    Some(palette()),
                    PaletteProjection::Journey {
                        task_limit: 8,
                        actor_limit: 0,
                    },
                    None,
                );
            }
            contributions.add_palette(
                "frozen.series.actors",
                Some(palette()),
                PaletteProjection::Journey {
                    task_limit: 0,
                    actor_limit: 6,
                },
                None,
            );
            contributions.add_theme_variables(
                "frozen.roles",
                [
                    ("lineColor", Some(color(&frozen.line))),
                    ("arrowheadColor", Some(color(&frozen.accent))),
                    (
                        "edgeLabelBackground",
                        Some(color(&frozen.edge_label_background)),
                    ),
                    ("faceColor", Some(color(&frozen.surface))),
                    ("tertiaryColor", Some(color(&frozen.surface_alt))),
                    ("border2", Some(color(&frozen.cluster_border))),
                ],
            );
            contributions.finish_into(builder);
        }
        RenderFamilyKind::Venn => {
            let mut contributions = FamilyContributions::new(family);
            contributions.add_palette(
                "frozen.series",
                Some(palette()),
                PaletteProjection::Venn { limit: 8 },
                None,
            );
            contributions.add_theme_variables(
                "frozen.roles",
                [
                    ("background", Some(color(&frozen.canvas))),
                    ("primaryColor", Some(color(&frozen.surface))),
                    ("vennTitleTextColor", Some(text.clone())),
                    ("vennSetTextColor", Some(text.clone())),
                ],
            );
            contributions.finish_into(builder);
        }
        RenderFamilyKind::Sankey => {
            let mut contributions = FamilyContributions::new(family);
            contributions.add_theme_variables(
                "frozen.label-background",
                [("mainBkg", Some(color(&frozen.surface)))],
            );
            contributions.finish_into(builder);
        }
        RenderFamilyKind::Packet => compile_frozen_packet(builder, frozen),
        RenderFamilyKind::Treemap => compile_frozen_treemap(builder, frozen, &palette()),
        RenderFamilyKind::EventModeling => {
            compile_frozen_event_modeling(builder, frozen, &palette())
        }
        RenderFamilyKind::C4 => compile_frozen_c4(builder, frozen),
        RenderFamilyKind::Architecture => compile_frozen_architecture(builder, frozen),
        RenderFamilyKind::Ishikawa => compile_frozen_ishikawa(builder, frozen),
        _ => {}
    }
}

fn family_has_palette_or_resolved_fill(
    spec: &DiagramThemeSpec,
    family_programs: &FamilyThemeProgramCache,
    family: RenderFamilyKind,
    target: ThemeTarget,
) -> bool {
    let reader = FamilyStyleReader::new(spec, family_programs, family);
    reader.has_palette(target)
        || !matches!(
            reader.fill_resolution(target),
            LegacyPaintResolution::Unspecified
        )
}

fn has_direct_pie_palette(
    spec: &DiagramThemeSpec,
    family_programs: &FamilyThemeProgramCache,
) -> bool {
    let reader = FamilyStyleReader::new(spec, family_programs, RenderFamilyKind::Pie);
    reader.has_palette(ThemeTarget::PieSlice)
        || !matches!(
            reader.fill_resolution(ThemeTarget::PieSlice),
            LegacyPaintResolution::Unspecified
        )
}

fn compile_frozen_packet(builder: &mut OverlayBuilder, frozen: &FrozenLegacyThemeCompatibility) {
    let mut packet = Map::new();
    for (key, value) in [
        ("startByteColor", frozen.line.as_css()),
        ("endByteColor", frozen.border.as_css()),
        ("labelColor", frozen.text.as_css()),
        ("titleColor", frozen.text.as_css()),
        ("blockStrokeColor", frozen.border.as_css()),
        ("blockFillColor", frozen.surface.as_css()),
    ] {
        packet.insert(key.to_string(), Value::String(value));
    }
    let mut contributions = FamilyContributions::new(RenderFamilyKind::Packet);
    contributions.add_root_object("frozen.packet", "packet", packet);
    contributions.finish_into(builder);
}

fn compile_frozen_treemap(
    builder: &mut OverlayBuilder,
    frozen: &FrozenLegacyThemeCompatibility,
    palette: &[String],
) {
    let mut treemap = Map::new();
    for (key, value) in [
        ("titleColor", frozen.text.as_css()),
        ("labelColor", frozen.text.as_css()),
        ("valueColor", frozen.subtle_text.as_css()),
        ("sectionStrokeColor", frozen.border.as_css()),
        ("sectionFillColor", frozen.surface_alt.as_css()),
        ("leafStrokeColor", frozen.border.as_css()),
        ("leafFillColor", frozen.surface.as_css()),
    ] {
        treemap.insert(key.to_string(), Value::String(value));
    }
    let mut contributions = FamilyContributions::new(RenderFamilyKind::Treemap);
    contributions.add_root_object("frozen.treemap", "treemap", treemap);
    contributions.add_palette(
        "frozen.series",
        Some(palette.to_vec()),
        PaletteProjection::ColorScale { limit: 12 },
        Some(frozen.canvas.as_css()),
    );
    contributions.finish_into(builder);
}

fn compile_frozen_event_modeling(
    builder: &mut OverlayBuilder,
    frozen: &FrozenLegacyThemeCompatibility,
    palette: &[String],
) {
    let palette_color = |index: usize, fallback: &super::canvas::ThemeColorValue| {
        palette
            .get(index)
            .cloned()
            .unwrap_or_else(|| fallback.as_css())
    };
    let mut variables = Map::new();
    for (key, value) in [
        ("emProcessorFill", palette_color(3, &frozen.surface_alt)),
        ("emProcessorStroke", frozen.border.as_css()),
        ("emReadModelFill", palette_color(1, &frozen.success)),
        ("emReadModelStroke", frozen.success.as_css()),
        ("emCommandFill", palette_color(0, &frozen.surface_alt)),
        ("emCommandStroke", frozen.line.as_css()),
        ("emEventFill", palette_color(2, &frozen.warning)),
        ("emEventStroke", frozen.warning.as_css()),
        ("emUiFill", frozen.surface.as_css()),
        ("emUiStroke", frozen.border.as_css()),
        ("emRelationStroke", frozen.line.as_css()),
        ("emArrowhead", frozen.accent.as_css()),
        ("emSwimlaneBackground", frozen.cluster_background.as_css()),
        (
            "emSwimlaneBackgroundOdd",
            frozen.cluster_background.as_css(),
        ),
        ("emSwimlaneBackgroundStroke", frozen.cluster_border.as_css()),
    ] {
        variables.insert(key.to_string(), Value::String(value));
    }
    let mut contributions = FamilyContributions::new(RenderFamilyKind::EventModeling);
    contributions.add_theme_variable_map("frozen.event-modeling", variables);
    contributions.finish_into(builder);
}

fn compile_frozen_c4(builder: &mut OverlayBuilder, frozen: &FrozenLegacyThemeCompatibility) {
    let mut c4 = Map::new();
    for prefix in [
        "person",
        "system",
        "system_db",
        "system_queue",
        "container",
        "container_db",
        "container_queue",
        "component",
        "component_db",
        "component_queue",
        "external_person",
        "external_system",
        "external_system_db",
        "external_system_queue",
        "external_container",
        "external_container_db",
        "external_container_queue",
        "external_component",
        "external_component_db",
        "external_component_queue",
    ] {
        c4.insert(
            format!("{prefix}_bg_color"),
            Value::String(frozen.surface.as_css()),
        );
        c4.insert(
            format!("{prefix}_border_color"),
            Value::String(frozen.border.as_css()),
        );
    }
    let mut contributions = FamilyContributions::new(RenderFamilyKind::C4);
    contributions.add_root_object("frozen.c4", "c4", c4);
    contributions.finish_into(builder);
}

fn compile_frozen_architecture(
    builder: &mut OverlayBuilder,
    frozen: &FrozenLegacyThemeCompatibility,
) {
    let mut contributions = FamilyContributions::new(RenderFamilyKind::Architecture);
    contributions.add_theme_variables(
        "frozen.architecture",
        [
            ("archEdgeColor", Some(frozen.line.as_css())),
            ("archEdgeArrowColor", Some(frozen.accent.as_css())),
            ("archGroupBorderColor", Some(frozen.cluster_border.as_css())),
        ],
    );
    contributions.finish_into(builder);
}

fn compile_frozen_ishikawa(builder: &mut OverlayBuilder, frozen: &FrozenLegacyThemeCompatibility) {
    let mut contributions = FamilyContributions::new(RenderFamilyKind::Ishikawa);
    contributions.add_theme_variables(
        "frozen.ishikawa",
        [
            ("lineColor", Some(frozen.line.as_css())),
            ("mainBkg", Some(frozen.surface.as_css())),
            ("primaryColor", Some(frozen.surface.as_css())),
        ],
    );
    contributions.finish_into(builder);
}

struct FamilyStyleReader<'a> {
    spec: &'a DiagramThemeSpec,
    program: Arc<FamilyThemeProgram>,
}

impl<'a> FamilyStyleReader<'a> {
    fn new(
        spec: &'a DiagramThemeSpec,
        family_programs: &FamilyThemeProgramCache,
        family: RenderFamilyKind,
    ) -> Self {
        Self {
            spec,
            program: family_programs.get_or_compile(spec, family),
        }
    }

    fn base_typography(&self) -> &TextStyle {
        self.program.base_typography()
    }

    fn style(&self, target: ThemeTarget) -> ResolvedThemeStyle {
        self.style_variant(target, ThemeVariant::Default)
    }

    fn style_variant(&self, target: ThemeTarget, variant: ThemeVariant) -> ResolvedThemeStyle {
        self.program.resolve_style(self.spec, target, variant, None)
    }

    fn text_style(&self, target: ThemeTarget) -> ResolvedThemeStyle {
        self.program
            .resolve_text_style(self.spec, target, ThemeVariant::Default, None)
    }

    fn fill(&self, target: ThemeTarget) -> Option<String> {
        self.fill_resolution(target).into_value()
    }

    fn fill_variant(&self, target: ThemeTarget, variant: ThemeVariant) -> Option<String> {
        LegacyPaintResolution::from_property(self.style_variant(target, variant).fill_resolution())
            .into_value()
    }

    fn stroke(&self, target: ThemeTarget) -> Option<String> {
        self.stroke_resolution(target).into_value()
    }

    fn stroke_variant(&self, target: ThemeTarget, variant: ThemeVariant) -> Option<String> {
        LegacyPaintResolution::from_property(
            self.style_variant(target, variant).stroke_resolution(),
        )
        .into_value()
    }

    fn stroke_or_fill(&self, target: ThemeTarget) -> Option<String> {
        self.stroke_or_fill_resolution(target).into_value()
    }

    fn marker_or_edge_paint(&self) -> Option<String> {
        match self.stroke_or_fill_resolution(ThemeTarget::Marker) {
            LegacyPaintResolution::Unspecified => self.stroke_or_fill_resolution(ThemeTarget::Edge),
            marker => marker,
        }
        .into_value()
    }

    fn text_fill(&self, target: ThemeTarget) -> Option<String> {
        LegacyPaintResolution::from_property(self.text_style(target).fill_resolution()).into_value()
    }

    fn fill_resolution(&self, target: ThemeTarget) -> LegacyPaintResolution {
        LegacyPaintResolution::from_property(self.style(target).fill_resolution())
    }

    fn stroke_resolution(&self, target: ThemeTarget) -> LegacyPaintResolution {
        LegacyPaintResolution::from_property(self.style(target).stroke_resolution())
    }

    fn palette(&self, target: ThemeTarget) -> Option<Vec<String>> {
        let palette_index = self.program.ordinal_palette_index(target)?;
        Some(
            self.spec.styles().ordinal_palettes()[palette_index]
                .1
                .colors()
                .iter()
                .map(super::canvas::ThemeColorValue::as_css)
                .collect(),
        )
    }

    fn has_palette(&self, target: ThemeTarget) -> bool {
        self.program.ordinal_palette_index(target).is_some()
    }

    fn canvas_color(&self) -> Option<String> {
        solid_paint(self.spec.canvas().base())
    }

    fn stroke_or_fill_resolution(&self, target: ThemeTarget) -> LegacyPaintResolution {
        let style = self.style(target);
        match LegacyPaintResolution::from_property(style.stroke_resolution()) {
            LegacyPaintResolution::Unspecified => {
                LegacyPaintResolution::from_property(style.fill_resolution())
            }
            stroke => stroke,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum PaletteProjection {
    ColorScale {
        limit: usize,
    },
    Git {
        limit: usize,
    },
    Pie {
        limit: usize,
    },
    Journey {
        task_limit: usize,
        actor_limit: usize,
    },
    Venn {
        limit: usize,
    },
}

#[derive(Debug, PartialEq, Eq)]
enum LegacyPaintResolution {
    Unspecified,
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
            Self::Unspecified | Self::Suppressed => None,
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

fn readable_text_color(color: &str, canvas: Option<&str>) -> String {
    let Ok(color) = ThemeColor::parse(color.trim()) else {
        return "#ffffff".to_string();
    };
    let background = canvas
        .and_then(|canvas| ThemeColor::parse(canvas.trim()).ok())
        .map_or([1.0; 3], |canvas| composite_over(&canvas, [1.0; 3]));
    let [red, green, blue] = composite_over(&color, background);
    let luminance = relative_luminance(red, green, blue);
    let black_contrast = (luminance + 0.05) / 0.05;
    let white_contrast = 1.05 / (luminance + 0.05);
    if black_contrast >= white_contrast {
        "#000000".to_string()
    } else {
        "#ffffff".to_string()
    }
}

fn composite_over(color: &ThemeColor, background: [f64; 3]) -> [f64; 3] {
    let alpha = color.channel(ColorChannel::Alpha);
    let foreground = [
        color.channel(ColorChannel::Red) / 255.0,
        color.channel(ColorChannel::Green) / 255.0,
        color.channel(ColorChannel::Blue) / 255.0,
    ];
    std::array::from_fn(|index| foreground[index] * alpha + background[index] * (1.0 - alpha))
}

fn relative_luminance(red: f64, green: f64, blue: f64) -> f64 {
    fn linear(channel: f64) -> f64 {
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    }

    0.2126 * linear(red) + 0.7152 * linear(green) + 0.0722 * linear(blue)
}

struct FamilyContributions {
    family: RenderFamilyKind,
    entries: Vec<PendingContribution>,
}

struct PendingContribution {
    mapping: &'static str,
    patch: Map<String, Value>,
}

impl FamilyContributions {
    fn new(family: RenderFamilyKind) -> Self {
        Self {
            family,
            entries: Vec::new(),
        }
    }

    fn add_typography(&mut self, typography: &TextStyle) {
        if typography == &TextStyle::default() {
            return;
        }
        let font_family = typography.font_stack().as_css();
        if font_family.len() > 4 * 1024 {
            return;
        }
        let font_size = format!("{}px", typography.font_size_px());
        let mut variables = Map::new();
        variables.insert("fontFamily".to_string(), Value::String(font_family.clone()));
        variables.insert("fontSize".to_string(), Value::String(font_size));
        let mut root = Map::new();
        root.insert("fontFamily".to_string(), Value::String(font_family));
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

    fn add_theme_variable_map(&mut self, mapping: &'static str, variables: Map<String, Value>) {
        if variables.is_empty() {
            return;
        }
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
        canvas: Option<String>,
    ) {
        let Some(palette) = palette.filter(|palette| !palette.is_empty()) else {
            return;
        };
        let mut variables = Map::new();
        match projection {
            PaletteProjection::ColorScale { limit } => {
                for (index, color) in palette.iter().take(limit).enumerate() {
                    variables.insert(format!("cScale{index}"), Value::String(color.clone()));
                    variables.insert(format!("cScalePeer{index}"), Value::String(color.clone()));
                    let label_color = readable_text_color(color, canvas.as_deref());
                    variables.insert(
                        format!("cScaleLabel{index}"),
                        Value::String(label_color.clone()),
                    );
                    variables.insert(format!("cScaleInv{index}"), Value::String(label_color));
                }
            }
            PaletteProjection::Git { limit } => {
                for (index, color) in palette.iter().take(limit).enumerate() {
                    variables.insert(format!("git{index}"), Value::String(color.clone()));
                    variables.insert(
                        format!("gitBranchLabel{index}"),
                        Value::String(readable_text_color(color, canvas.as_deref())),
                    );
                }
            }
            PaletteProjection::Pie { limit } => {
                for (index, color) in palette.iter().take(limit).enumerate() {
                    variables.insert(format!("pie{}", index + 1), Value::String(color.clone()));
                }
            }
            PaletteProjection::Journey {
                task_limit,
                actor_limit,
            } => {
                for (index, color) in palette.iter().take(task_limit).enumerate() {
                    variables.insert(format!("fillType{index}"), Value::String(color.clone()));
                }
                for (index, color) in palette.iter().take(actor_limit).enumerate() {
                    variables.insert(format!("actor{index}"), Value::String(color.clone()));
                }
            }
            PaletteProjection::Venn { limit } => {
                for (index, color) in palette.iter().take(limit).enumerate() {
                    variables.insert(format!("venn{}", index + 1), Value::String(color.clone()));
                }
            }
        }
        let mut root = Map::new();
        root.insert("themeVariables".to_string(), Value::Object(variables));
        self.add_patch(mapping, root);
    }

    fn add_xy_palette(&mut self, mapping: &'static str, palette: Option<Vec<String>>) {
        let Some(palette) = palette.filter(|palette| !palette.is_empty()) else {
            return;
        };
        let csv = bounded_palette_csv(&palette);
        if csv.is_empty() {
            return;
        }
        let mut xy = Map::new();
        xy.insert("plotColorPalette".to_string(), Value::String(csv));
        xy.insert("accentColor".to_string(), Value::String(palette[0].clone()));
        self.add_theme_variable_object(mapping, "xyChart", xy);
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

fn bounded_palette_csv(palette: &[String]) -> String {
    let mut csv = String::new();
    for color in palette {
        let added_bytes = color.len() + usize::from(!csv.is_empty());
        if csv.len() + added_bytes > MAX_LEGACY_ASSIGNMENT_STRING_BYTES {
            break;
        }
        if !csv.is_empty() {
            csv.push(',');
        }
        csv.push_str(color);
    }
    csv
}

struct OverlayBuilder {
    family: RenderFamilyKind,
    overlay: ThemeFamilyCompatibilityOverlayBuilder,
    contribution_ids: BTreeSet<String>,
    claimed_paths: BTreeSet<String>,
}

impl OverlayBuilder {
    fn new(family: RenderFamilyKind) -> Self {
        Self {
            family,
            overlay: ThemeFamilyCompatibilityOverlayBuilder::new(
                family.as_str(),
                CONTRIBUTION_ID_PREFIX,
            ),
            contribution_ids: BTreeSet::new(),
            claimed_paths: BTreeSet::new(),
        }
    }

    fn push(&mut self, family: RenderFamilyKind, mapping: &'static str, patch: Map<String, Value>) {
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
        if self
            .overlay
            .try_push(mapping, MermaidConfig::from_value(Value::Object(patch)))
            .is_err()
        {
            return;
        }

        self.claimed_paths.extend(accepted_paths);
        self.contribution_ids.insert(opaque_id);
    }

    fn finish(self) -> (ThemeFamilyCompatibilityOverlay, BTreeSet<String>) {
        (self.overlay.finish(), self.contribution_ids)
    }
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
        CanvasSpec, Specified, ThemeRule, ThemeRuleSet, ThemeStylePatch, TypographySpec,
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
    const XY_FIXTURE: &str = include_str!(
        "../../../../fixtures/xychart/upstream_cypress_xychart_spec_render_all_the_theme_color_018.mmd"
    );
    const JOURNEY_FIXTURE: &str =
        include_str!("../../../../fixtures/journey/upstream_tasks_and_people.mmd");
    const VENN_FIXTURE: &str = include_str!(
        "../../../../fixtures/venn/upstream_cypress_venn_handdrawn_three_set_title_015.mmd"
    );
    const PACKET_FIXTURE: &str = include_str!(
        "../../../../fixtures/packet/upstream_cypress_packet_spec_should_render_a_complex_packet_diagram_004.mmd"
    );
    const C4_FIXTURE: &str =
        include_str!("../../../../fixtures/c4/upstream_docs_c4_c4_diagrams_001.mmd");
    const EVENT_MODELING_FIXTURE: &str =
        include_str!("../../../../fixtures/eventmodeling/upstream_docs_eventmodeling_minimum.mmd");

    fn bridge(spec: &DiagramThemeSpec) -> LegacyFamilyThemeBridge {
        LegacyFamilyThemeBridge::new(
            Arc::new(spec.clone()),
            Arc::new(FamilyThemeProgramCache::default()),
        )
    }

    fn parse(spec: &DiagramThemeSpec, source: &str) -> merman_core::ParseMetadata {
        let bridge = bridge(spec);
        let resolver = bridge.clone();
        let plan = ThemeCompatibilityPlan::try_new(
            [0x5a; 32],
            MermaidConfig::empty_object(),
            move |family, control| resolver.overlay_for_family(family, control),
        )
        .expect("test compatibility plan");
        install_theme_compatibility(merman_core::Engine::new(), &plan)
            .parse_metadata_sync(source)
            .expect("test diagram should parse")
    }

    fn fallback_contribution_count(metadata: &merman_core::ParseMetadata) -> usize {
        theme_parse_evidence(metadata).fallback_contribution_count()
    }

    #[test]
    fn provider_compiles_only_the_selected_family_and_reuses_its_artifact() {
        let spec = Arc::new(
            super::super::ThemeTokens::default()
                .with_text("#f8fafc")
                .expect("valid text color")
                .into_theme_spec(),
        );
        let family_programs = Arc::new(FamilyThemeProgramCache::default());
        let bridge = LegacyFamilyThemeBridge::new(Arc::clone(&spec), Arc::clone(&family_programs));
        let control = ParseControl::new();

        assert_eq!(bridge.cached_family_count(), 0);
        assert_eq!(family_programs.len(), 0);

        let first = bridge
            .overlay_for_family("flowchart", &control)
            .expect("active control")
            .expect("flowchart compatibility overlay");
        assert!(!first.is_empty());
        assert_eq!(bridge.cached_family_count(), 1);
        assert_eq!(family_programs.len(), 1);
        assert!(family_programs.contains(RenderFamilyKind::Flowchart));
        assert!(!family_programs.contains(RenderFamilyKind::Sequence));

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
        assert!(family_programs.contains(RenderFamilyKind::Sequence));
    }

    #[test]
    fn direct_family_palettes_suppress_overlapping_frozen_palette_fallbacks() {
        let base = super::super::ThemeTokens::default().into_theme_spec();
        let palette = super::super::OrdinalPalette::new([
            super::super::ThemeColorValue::parse("#ef4444").expect("valid palette color"),
            super::super::ThemeColorValue::parse("#22c55e").expect("valid palette color"),
        ])
        .expect("non-empty palette");
        let styles = base
            .styles()
            .clone()
            .with_ordinal_palette(ThemeTarget::Node, palette.clone())
            .with_ordinal_palette(ThemeTarget::Task, palette);
        let spec = base.with_styles(styles);
        let bridge = bridge(&spec);

        for (family, direct_id, frozen_id) in [
            (
                RenderFamilyKind::Mindmap,
                "merman.legacy-family-theme.v1.mindmap.node.palette",
                "merman.legacy-family-theme.v1.mindmap.frozen.series",
            ),
            (
                RenderFamilyKind::GitGraph,
                "merman.legacy-family-theme.v1.gitGraph.node.palette",
                "merman.legacy-family-theme.v1.gitGraph.frozen.series",
            ),
            (
                RenderFamilyKind::Kanban,
                "merman.legacy-family-theme.v1.kanban.task.palette.color-scale",
                "merman.legacy-family-theme.v1.kanban.frozen.series.color-scale",
            ),
        ] {
            let artifact = bridge.compile_for_family(family);
            assert!(artifact.contribution_ids.contains(direct_id));
            assert!(!artifact.contribution_ids.contains(frozen_id));
        }
    }

    #[test]
    fn explicit_fill_clear_blocks_frozen_palette_fallbacks() {
        let base = super::super::ThemeTokens::default().into_theme_spec();
        let rules_without_palettes = base
            .styles()
            .rules()
            .iter()
            .cloned()
            .fold(ThemeRuleSet::default(), ThemeRuleSet::with_rule);

        for (family, target, frozen_ids, retained_frozen_id) in [
            (
                RenderFamilyKind::Mindmap,
                ThemeTarget::Node,
                &["merman.legacy-family-theme.v1.mindmap.frozen.series"][..],
                None,
            ),
            (
                RenderFamilyKind::GitGraph,
                ThemeTarget::Node,
                &["merman.legacy-family-theme.v1.gitGraph.frozen.series"][..],
                None,
            ),
            (
                RenderFamilyKind::Kanban,
                ThemeTarget::Task,
                &[
                    "merman.legacy-family-theme.v1.kanban.frozen.series.color-scale",
                    "merman.legacy-family-theme.v1.kanban.frozen.series.git",
                ][..],
                None,
            ),
            (
                RenderFamilyKind::Timeline,
                ThemeTarget::TimelineEvent,
                &["merman.legacy-family-theme.v1.timeline.frozen.series"][..],
                None,
            ),
            (
                RenderFamilyKind::Radar,
                ThemeTarget::ChartSeries,
                &["merman.legacy-family-theme.v1.radar.frozen.series"][..],
                Some("merman.legacy-family-theme.v1.radar.frozen.graticule"),
            ),
            (
                RenderFamilyKind::Journey,
                ThemeTarget::JourneyTask,
                &["merman.legacy-family-theme.v1.journey.frozen.series.tasks"][..],
                Some("merman.legacy-family-theme.v1.journey.frozen.series.actors"),
            ),
        ] {
            let mut clear = ThemeStylePatch::default();
            clear.paint.fill = Specified::Clear;
            let spec = base.clone().with_styles(
                rules_without_palettes
                    .clone()
                    .with_rule(ThemeRule::new(target, clear).for_family(family)),
            );
            let artifact = bridge(&spec).compile_for_family(family);

            for frozen_id in frozen_ids {
                assert!(
                    !artifact.contribution_ids.contains(*frozen_id),
                    "{family} must not revive `{frozen_id}` after an explicit fill clear"
                );
            }
            if let Some(retained_frozen_id) = retained_frozen_id {
                assert!(artifact.contribution_ids.contains(retained_frozen_id));
            }
        }
    }

    #[test]
    fn unsupported_fill_blocks_frozen_palette_fallback() {
        let base = super::super::ThemeTokens::default().into_theme_spec();
        let gradient = CanvasPaint::LinearGradient(
            super::super::LinearGradient::new(
                0.0,
                [
                    super::super::GradientStop::new(
                        0.0,
                        super::super::ThemeColorValue::parse("#ef4444").expect("valid first stop"),
                    )
                    .expect("valid first stop"),
                    super::super::GradientStop::new(
                        1.0,
                        super::super::ThemeColorValue::parse("#3b82f6").expect("valid second stop"),
                    )
                    .expect("valid second stop"),
                ],
            )
            .expect("valid test gradient"),
        );
        let styles = base
            .styles()
            .rules()
            .iter()
            .cloned()
            .fold(ThemeRuleSet::default(), ThemeRuleSet::with_rule)
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::Node,
                    ThemeStylePatch::default().with_fill(gradient),
                )
                .for_family(RenderFamilyKind::Mindmap),
            );
        let spec = base.with_styles(styles);
        let artifact = bridge(&spec).compile_for_family(RenderFamilyKind::Mindmap);

        assert!(
            !artifact
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.mindmap.node.fill")
        );
        assert!(
            !artifact
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.mindmap.frozen.series")
        );
    }

    #[test]
    fn explicit_journey_task_solid_fill_blocks_frozen_palette_fallback() {
        let base = super::super::ThemeTokens::default().into_theme_spec();
        let styles = base
            .styles()
            .rules()
            .iter()
            .cloned()
            .fold(ThemeRuleSet::default(), ThemeRuleSet::with_rule)
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::JourneyTask,
                    ThemeStylePatch::default().with_fill(solid("#ef4444")),
                )
                .for_family(RenderFamilyKind::Journey),
            );
        let spec = base.with_styles(styles);
        let artifact = bridge(&spec).compile_for_family(RenderFamilyKind::Journey);

        assert!(
            artifact
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.journey.task.paint-text")
        );
        assert!(
            !artifact
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.journey.frozen.series.tasks")
        );
        assert!(
            artifact
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.journey.frozen.series.actors")
        );
    }

    #[test]
    fn explicit_journey_task_non_solid_fill_blocks_frozen_palette_fallback() {
        let base = super::super::ThemeTokens::default().into_theme_spec();
        let gradient = CanvasPaint::LinearGradient(
            super::super::LinearGradient::new(
                0.0,
                [
                    super::super::GradientStop::new(
                        0.0,
                        super::super::ThemeColorValue::parse("#ef4444").expect("valid first stop"),
                    )
                    .expect("valid first stop"),
                    super::super::GradientStop::new(
                        1.0,
                        super::super::ThemeColorValue::parse("#3b82f6").expect("valid second stop"),
                    )
                    .expect("valid second stop"),
                ],
            )
            .expect("valid test gradient"),
        );
        let styles = base
            .styles()
            .rules()
            .iter()
            .cloned()
            .fold(ThemeRuleSet::default(), ThemeRuleSet::with_rule)
            .with_rule(
                ThemeRule::new(
                    ThemeTarget::JourneyTask,
                    ThemeStylePatch::default().with_fill(gradient),
                )
                .for_family(RenderFamilyKind::Journey),
            );
        let spec = base.with_styles(styles);
        let artifact = bridge(&spec).compile_for_family(RenderFamilyKind::Journey);

        assert!(
            !artifact
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.journey.frozen.series.tasks")
        );
        assert!(
            artifact
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.journey.frozen.series.actors")
        );
    }

    #[test]
    fn explicit_pie_slice_clear_blocks_frozen_palette_fallback() {
        let base = super::super::ThemeTokens::default().into_theme_spec();
        let mut clear = ThemeStylePatch::default();
        clear.paint.fill = Specified::Clear;
        let styles = base
            .styles()
            .rules()
            .iter()
            .cloned()
            .fold(ThemeRuleSet::default(), ThemeRuleSet::with_rule)
            .with_rule(
                ThemeRule::new(ThemeTarget::PieSlice, clear).for_family(RenderFamilyKind::Pie),
            );
        let spec = base.with_styles(styles);
        let bridge = bridge(&spec);
        let artifact = bridge.compile_for_family(RenderFamilyKind::Pie);

        assert!(
            !artifact
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.pie.slice.palette")
        );
        assert!(
            !artifact
                .contribution_ids
                .contains("merman.legacy-family-theme.v1.pie.frozen.series")
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
                    .for_family(RenderFamilyKind::Flowchart),
                )
                .with_rule(
                    ThemeRule::new(
                        ThemeTarget::Actor,
                        ThemeStylePatch::default().with_fill(solid("#22c55e")),
                    )
                    .for_family(RenderFamilyKind::Sequence),
                ),
        );

        let flowchart = parse(&spec, "flowchart LR\nA --> B\n");
        let sequence = parse(&spec, "sequenceDiagram\nAlice->>Bob: Hello\n");

        assert_eq!(
            flowchart
                .effective_config
                .get_str("themeVariables.primaryColor"),
            Some("#ef4444")
        );
        assert_eq!(fallback_contribution_count(&flowchart), 1);
        assert_eq!(
            sequence.effective_config.get_str("themeVariables.actorBkg"),
            Some("#22c55e")
        );
        assert_eq!(fallback_contribution_count(&sequence), 1);
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
                .for_family(RenderFamilyKind::Swimlane),
            ),
        );

        let swimlane = parse(&spec, "swimlane-beta LR\nA --> B\n");
        let flowchart = parse(&spec, "flowchart LR\nA --> B\n");
        assert_eq!(fallback_contribution_count(&swimlane), 2);
        assert_ne!(swimlane.effective_config.get_str("theme"), Some("base"));
        assert_eq!(
            swimlane
                .effective_config
                .get_str("themeVariables.clusterBkg"),
            Some("#fef3c7")
        );
        assert_eq!(
            swimlane
                .effective_config
                .get_str("themeVariables.secondaryColor"),
            Some("#fef3c7")
        );
        assert_eq!(
            swimlane
                .effective_config
                .get_str("themeVariables.clusterBorder"),
            Some("#a16207")
        );
        assert_eq!(fallback_contribution_count(&flowchart), 0);
    }

    #[test]
    fn family_typography_is_local_and_preserves_the_frozen_patch_shape() {
        let sequence_typography = TextStyle::default()
            .with_font_stack(
                super::super::FontStack::new(["Inter", "sans-serif"]).expect("valid font stack"),
            )
            .with_font_size_px(18.0)
            .expect("valid font size");
        let spec = DiagramThemeSpec::new().with_typography(
            TypographySpec::default()
                .with_family_style(RenderFamilyKind::Sequence, sequence_typography),
        );

        let sequence = parse(&spec, "sequenceDiagram\nAlice->>Bob: Hello\n");
        let flowchart = parse(&spec, "flowchart LR\nA --> B\n");

        assert_eq!(fallback_contribution_count(&sequence), 1);
        assert_ne!(sequence.effective_config.get_str("theme"), Some("base"));
        assert_eq!(
            sequence.effective_config.get_str("fontFamily"),
            Some("Inter, sans-serif")
        );
        assert_eq!(
            sequence
                .effective_config
                .get_str("themeVariables.fontFamily"),
            Some("Inter, sans-serif")
        );
        assert_eq!(
            sequence.effective_config.get_str("themeVariables.fontSize"),
            Some("18px")
        );
        assert_eq!(fallback_contribution_count(&flowchart), 0);
    }

    #[test]
    fn state_and_root_owned_mechanisms_do_not_create_legacy_overlays() {
        let state_rule = ThemeRule::new(
            ThemeTarget::State,
            ThemeStylePatch::default().with_fill(solid("#2563eb")),
        )
        .for_family(RenderFamilyKind::State);
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

        assert_eq!(fallback_contribution_count(&cleared_marker), 1);
        assert_ne!(
            cleared_marker
                .effective_config
                .get_str("themeVariables.arrowheadColor"),
            Some("#22c55e")
        );
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

        assert!(bridge.owns_contribution_id("merman.legacy-family-theme.v1.flowchart.node.fill"));
        assert_eq!(bridge.cached_family_count(), 1);
        assert!(
            !bridge
                .owns_contribution_id("merman.legacy-family-theme.v1.flowchart.node.fill.forged")
        );
        assert!(!bridge.owns_contribution_id("merman.legacy-family-theme.v1.sequence.node.fill"));
    }

    #[test]
    fn overlay_builder_keeps_the_first_assignment_without_panicking() {
        let mut builder = OverlayBuilder::new(RenderFamilyKind::Flowchart);
        let mut direct = FamilyContributions::new(RenderFamilyKind::Flowchart);
        direct.add_theme_variables("direct", [("primaryColor", Some("#ef4444".to_string()))]);
        direct.finish_into(&mut builder);

        let mut frozen = FamilyContributions::new(RenderFamilyKind::Flowchart);
        frozen.add_theme_variables("frozen", [("primaryColor", Some("#22c55e".to_string()))]);
        frozen.finish_into(&mut builder);

        let (_, contribution_ids) = builder.finish();
        assert!(contribution_ids.contains("merman.legacy-family-theme.v1.flowchart.direct"));
        assert!(!contribution_ids.contains("merman.legacy-family-theme.v1.flowchart.frozen"));
    }

    #[test]
    fn bridge_does_not_inject_a_global_base_theme() {
        let spec = DiagramThemeSpec::new().with_styles(
            ThemeRuleSet::default().with_rule(
                ThemeRule::new(
                    ThemeTarget::Node,
                    ThemeStylePatch::default().with_fill(solid("#ef4444")),
                )
                .for_family(RenderFamilyKind::Flowchart),
            ),
        );
        let bridge = bridge(&spec);
        let artifact = bridge.compile_for_family(RenderFamilyKind::Flowchart);
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
    fn unsupported_paint_also_blocks_legacy_fallback() {
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
        bridge.compile_for_family(RenderFamilyKind::Flowchart);
        assert!(
            !bridge.owns_contribution_id("merman.legacy-family-theme.v1.flowchart.edge.stroke")
        );
    }

    #[test]
    fn frozen_legacy_corpus_never_reads_family_scoped_rules() {
        let base = super::super::ThemeTokens::default().into_theme_spec();
        let styles = base.styles().clone().with_rule(
            ThemeRule::new(
                ThemeTarget::Node,
                ThemeStylePatch::default().with_fill(solid("#ef4444")),
            )
            .for_family(RenderFamilyKind::Flowchart),
        );
        let spec = base.with_styles(styles);

        let packet = parse(&spec, PACKET_FIXTURE);
        assert_eq!(
            packet.effective_config.get_str("packet.blockFillColor"),
            Some("#f8fafc")
        );
        assert!(fallback_contribution_count(&packet) > 0);

        let manual =
            DiagramThemeSpec::new().with_styles(ThemeRuleSet::default().with_rule(ThemeRule::new(
                ThemeTarget::Node,
                ThemeStylePatch::default().with_fill(solid("#ef4444")),
            )));
        let packet = parse(&manual, PACKET_FIXTURE);
        assert_ne!(
            packet.effective_config.get_str("packet.blockFillColor"),
            Some("#ef4444")
        );
        assert_eq!(fallback_contribution_count(&packet), 0);
    }

    #[test]
    fn direct_clear_never_revives_the_frozen_surface() {
        let base = super::super::ThemeTokens::default().into_theme_spec();
        let mut clear = ThemeStylePatch::default();
        clear.paint.fill = Specified::Clear;
        clear.stroke.paint = Specified::Clear;
        let styles = base.styles().clone().with_rule(
            ThemeRule::new(ThemeTarget::Node, clear).for_family(RenderFamilyKind::Flowchart),
        );
        let spec = base.with_styles(styles);

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
        let spec = super::super::ThemeTokens::default().into_theme_spec();
        let gantt = parse(&spec, GANTT_FIXTURE);
        assert_eq!(
            gantt
                .effective_config
                .get_str("themeVariables.taskBkgColor"),
            Some("#f8fafc")
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

        let requirement = parse(&spec, REQUIREMENT_FIXTURE);
        assert_eq!(
            requirement
                .effective_config
                .get_str("themeVariables.requirementBackground"),
            Some("#f8fafc")
        );
        assert_eq!(
            requirement
                .effective_config
                .get_str("themeVariables.relationColor"),
            Some("#64748b")
        );
        assert!(fallback_contribution_count(&requirement) > 0);
    }

    #[test]
    fn direct_family_palettes_precede_frozen_palette_fallbacks() {
        let spec = super::super::ThemeTokens::default().into_theme_spec();

        let pie = parse(&spec, PIE_FIXTURE);
        assert_eq!(
            pie.effective_config.get_str("themeVariables.pie1"),
            Some("#2563eb")
        );
        assert!(fallback_contribution_count(&pie) > 0);

        let xy = parse(&spec, XY_FIXTURE);
        assert_eq!(
            xy.effective_config
                .get_str("themeVariables.xyChart.plotColorPalette"),
            Some("#2563eb,#16a34a,#d97706,#9333ea")
        );
        assert!(fallback_contribution_count(&xy) > 0);

        let journey = parse(&spec, JOURNEY_FIXTURE);
        assert_eq!(
            journey.effective_config.get_str("themeVariables.fillType0"),
            Some("#2563eb")
        );
        assert_eq!(
            journey.effective_config.get_str("themeVariables.actor0"),
            Some("#2563eb")
        );
        assert!(fallback_contribution_count(&journey) >= 2);
    }

    #[test]
    fn frozen_only_families_receive_named_local_contributions() {
        let spec = super::super::ThemeTokens::default().into_theme_spec();

        let venn = parse(&spec, VENN_FIXTURE);
        assert_eq!(
            venn.effective_config.get_str("themeVariables.venn1"),
            Some("#2563eb")
        );
        assert!(fallback_contribution_count(&venn) > 0);

        let sankey = parse(&spec, "sankey\nSource,Target,1\n");
        assert_eq!(
            sankey.effective_config.get_str("themeVariables.mainBkg"),
            Some("#f8fafc")
        );
        assert!(fallback_contribution_count(&sankey) > 0);

        let c4 = parse(&spec, C4_FIXTURE);
        assert_eq!(
            c4.effective_config.get_str("c4.person_bg_color"),
            Some("#f8fafc")
        );
        assert_eq!(
            c4.effective_config
                .get_str("c4.external_component_border_color"),
            Some("#94a3b8")
        );
        assert!(fallback_contribution_count(&c4) > 0);

        let event_modeling = parse(&spec, EVENT_MODELING_FIXTURE);
        assert_eq!(
            event_modeling
                .effective_config
                .get_str("themeVariables.emRelationStroke"),
            Some("#64748b")
        );
        assert_eq!(
            event_modeling
                .effective_config
                .get_str("themeVariables.emArrowhead"),
            Some("#2563eb")
        );
        assert_eq!(
            event_modeling
                .effective_config
                .get_str("themeVariables.emSwimlaneBackgroundOdd"),
            Some("#f1f5f9")
        );
    }

    #[test]
    fn xy_palette_csv_is_bounded_to_the_core_assignment_limit() {
        let colors = (0..256)
            .map(|index| format!("#{index:06x}{}", "a".repeat(96)))
            .collect::<Vec<_>>();
        let csv = bounded_palette_csv(&colors);
        assert!(csv.len() <= MAX_LEGACY_ASSIGNMENT_STRING_BYTES);
        assert!(!csv.ends_with(','));
        assert!(colors.iter().any(|color| !csv.contains(color)));
    }
}
