#[cfg(any(test, feature = "internal-theme-acceptance"))]
use std::fmt;

#[cfg(any(test, feature = "internal-theme-acceptance"))]
use crate::DiagramFamilyId;
#[cfg(any(test, feature = "internal-theme-acceptance"))]
use crate::diagram_theme::ThemeTarget;

/// Canonical selector class for one legacy bridge cutover witness.
#[cfg(any(test, feature = "internal-theme-acceptance"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ThemeRouteCutoverSelector {
    /// A static rule with neither an ordinal selector nor an explicit variant.
    StaticUnqualified,
}

/// Theme facet proven by one legacy bridge cutover witness.
#[cfg(any(test, feature = "internal-theme-acceptance"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ThemeRouteCutoverFacet {
    Fill,
    Stroke,
}

/// Admitted value class proven by one legacy bridge cutover witness.
#[cfg(any(test, feature = "internal-theme-acceptance"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ThemeRouteCutoverValue {
    Transparent,
    Solid,
}

/// Canonical identity for a typed route whose bridge ownership is being cut over.
///
/// Projection obligations deliberately do not participate in this identity. They are versioned
/// authorization facts about the route and can change only through an explicit cutover migration.
#[cfg(any(test, feature = "internal-theme-acceptance"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ThemeRouteCutoverId {
    family_id: DiagramFamilyId,
    target: ThemeTarget,
    selector: ThemeRouteCutoverSelector,
    facet: ThemeRouteCutoverFacet,
    value: ThemeRouteCutoverValue,
}

#[cfg(any(test, feature = "internal-theme-acceptance"))]
impl ThemeRouteCutoverId {
    pub const fn new(
        family_id: DiagramFamilyId,
        target: ThemeTarget,
        selector: ThemeRouteCutoverSelector,
        facet: ThemeRouteCutoverFacet,
        value: ThemeRouteCutoverValue,
    ) -> Self {
        Self {
            family_id,
            target,
            selector,
            facet,
            value,
        }
    }

    pub const fn family_id(self) -> DiagramFamilyId {
        self.family_id
    }

    pub const fn target(self) -> ThemeTarget {
        self.target
    }

    pub const fn selector(self) -> ThemeRouteCutoverSelector {
        self.selector
    }

    pub const fn facet(self) -> ThemeRouteCutoverFacet {
        self.facet
    }

    pub const fn value(self) -> ThemeRouteCutoverValue {
        self.value
    }
}

/// One legacy projection affected by typed route ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum ThemeRouteCutoverProjection {
    NodeFill = 0,
    NodeStroke = 1,
    EdgeStroke = 2,
    MarkerPaintFromEdge = 3,
    ActorFill = 4,
    ActorStroke = 5,
    NoteFill = 6,
    NoteStroke = 7,
    ActivationFill = 8,
    ActivationStroke = 9,
    MessageStroke = 10,
    ClusterFill = 11,
    ClusterStroke = 12,
    LifelineStroke = 13,
    TitleFill = 14,
    RequirementFill = 15,
    PieSliceStroke = 16,
}

impl ThemeRouteCutoverProjection {
    #[cfg(any(test, feature = "internal-theme-acceptance"))]
    const ALL: [Self; 17] = [
        Self::NodeFill,
        Self::NodeStroke,
        Self::EdgeStroke,
        Self::MarkerPaintFromEdge,
        Self::ActorFill,
        Self::ActorStroke,
        Self::NoteFill,
        Self::NoteStroke,
        Self::ActivationFill,
        Self::ActivationStroke,
        Self::MessageStroke,
        Self::ClusterFill,
        Self::ClusterStroke,
        Self::LifelineStroke,
        Self::TitleFill,
        Self::RequirementFill,
        Self::PieSliceStroke,
    ];

    pub const fn contribution_id(self) -> &'static str {
        match self {
            Self::NodeFill => "node.fill",
            Self::NodeStroke => "node.stroke",
            Self::EdgeStroke => "edge.stroke",
            Self::MarkerPaintFromEdge => "marker.paint-from-edge",
            Self::ActorFill => "actor.fill",
            Self::ActorStroke => "actor.stroke",
            Self::NoteFill => "note.fill",
            Self::NoteStroke => "note.stroke",
            Self::ActivationFill => "activation.fill",
            Self::ActivationStroke => "activation.stroke",
            Self::MessageStroke => "message.stroke",
            Self::ClusterFill => "cluster.fill",
            Self::ClusterStroke => "cluster.stroke",
            Self::LifelineStroke => "lifeline.stroke",
            Self::TitleFill => "title.fill",
            Self::RequirementFill => "requirement.fill",
            Self::PieSliceStroke => "slice.stroke",
        }
    }

    #[cfg(any(test, feature = "internal-theme-acceptance"))]
    pub const fn action(self) -> ThemeRouteCutoverProjectionAction {
        match self {
            Self::MarkerPaintFromEdge => ThemeRouteCutoverProjectionAction::RetireFallback,
            Self::NodeFill
            | Self::NodeStroke
            | Self::EdgeStroke
            | Self::ActorFill
            | Self::ActorStroke
            | Self::NoteFill
            | Self::NoteStroke
            | Self::ActivationFill
            | Self::ActivationStroke
            | Self::MessageStroke
            | Self::ClusterFill
            | Self::ClusterStroke
            | Self::LifelineStroke
            | Self::TitleFill
            | Self::RequirementFill
            | Self::PieSliceStroke => ThemeRouteCutoverProjectionAction::Replace,
        }
    }

    #[cfg(any(test, feature = "internal-theme-acceptance"))]
    const fn bit(self) -> u32 {
        1 << self as u8
    }
}

/// What typed ownership must prove about a legacy projection.
#[cfg(any(test, feature = "internal-theme-acceptance"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ThemeRouteCutoverProjectionAction {
    Replace,
    RetireFallback,
}

#[cfg(any(test, feature = "internal-theme-acceptance"))]
impl ThemeRouteCutoverProjectionAction {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Replace => "replace",
            Self::RetireFallback => "retire-fallback",
        }
    }
}

/// Fixed, canonical set of legacy projection obligations for one route.
#[cfg(any(test, feature = "internal-theme-acceptance"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ThemeRouteCutoverProjectionSet(u32);

#[cfg(any(test, feature = "internal-theme-acceptance"))]
impl ThemeRouteCutoverProjectionSet {
    pub const REPLACE_NODE_FILL: Self = Self::replacing(ThemeRouteCutoverProjection::NodeFill);
    pub const REPLACE_NODE_STROKE: Self = Self::replacing(ThemeRouteCutoverProjection::NodeStroke);
    pub const REPLACE_EDGE_STROKE_AND_RETIRE_MARKER_FALLBACK: Self = Self::edge_stroke();
    pub const REPLACE_ACTOR_FILL: Self = Self::replacing(ThemeRouteCutoverProjection::ActorFill);
    pub const REPLACE_ACTOR_STROKE: Self =
        Self::replacing(ThemeRouteCutoverProjection::ActorStroke);
    pub const REPLACE_NOTE_FILL: Self = Self::replacing(ThemeRouteCutoverProjection::NoteFill);
    pub const REPLACE_NOTE_STROKE: Self = Self::replacing(ThemeRouteCutoverProjection::NoteStroke);
    pub const REPLACE_ACTIVATION_FILL: Self =
        Self::replacing(ThemeRouteCutoverProjection::ActivationFill);
    pub const REPLACE_ACTIVATION_STROKE: Self =
        Self::replacing(ThemeRouteCutoverProjection::ActivationStroke);
    pub const REPLACE_MESSAGE_STROKE: Self =
        Self::replacing(ThemeRouteCutoverProjection::MessageStroke);
    pub const REPLACE_CLUSTER_FILL: Self =
        Self::replacing(ThemeRouteCutoverProjection::ClusterFill);
    pub const REPLACE_CLUSTER_STROKE: Self =
        Self::replacing(ThemeRouteCutoverProjection::ClusterStroke);
    pub const REPLACE_LIFELINE_STROKE: Self =
        Self::replacing(ThemeRouteCutoverProjection::LifelineStroke);
    pub const REPLACE_TITLE_FILL: Self = Self::replacing(ThemeRouteCutoverProjection::TitleFill);
    pub const REPLACE_REQUIREMENT_FILL: Self =
        Self::replacing(ThemeRouteCutoverProjection::RequirementFill);
    pub const REPLACE_PIE_SLICE_STROKE: Self =
        Self::replacing(ThemeRouteCutoverProjection::PieSliceStroke);

    pub(crate) const fn replacing(projection: ThemeRouteCutoverProjection) -> Self {
        Self(projection.bit())
    }

    pub(crate) const fn edge_stroke() -> Self {
        Self(
            ThemeRouteCutoverProjection::EdgeStroke.bit()
                | ThemeRouteCutoverProjection::MarkerPaintFromEdge.bit(),
        )
    }

    pub const fn contains(self, projection: ThemeRouteCutoverProjection) -> bool {
        self.0 & projection.bit() != 0
    }

    pub fn iter(self) -> impl Iterator<Item = ThemeRouteCutoverProjection> {
        ThemeRouteCutoverProjection::ALL
            .into_iter()
            .filter(move |projection| self.contains(*projection))
    }

    pub const fn len(self) -> usize {
        self.0.count_ones() as usize
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// Workspace-private route plus the legacy projections that typed ownership replaces or retires.
///
/// This is an inventory projection, not terminal evidence. Only the non-published C6 harness can
/// combine it with finalized SVG and PNG receipts to authorize a cutover.
#[cfg(any(test, feature = "internal-theme-acceptance"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ThemeRouteCutoverDescriptor {
    id: ThemeRouteCutoverId,
    projections: ThemeRouteCutoverProjectionSet,
}

#[cfg(any(test, feature = "internal-theme-acceptance"))]
impl ThemeRouteCutoverDescriptor {
    pub(crate) const fn new(
        id: ThemeRouteCutoverId,
        projections: ThemeRouteCutoverProjectionSet,
    ) -> Self {
        Self { id, projections }
    }

    pub const fn id(self) -> ThemeRouteCutoverId {
        self.id
    }

    pub const fn family_id(self) -> DiagramFamilyId {
        self.id.family_id()
    }

    pub const fn target(self) -> ThemeTarget {
        self.id.target()
    }

    pub const fn selector(self) -> ThemeRouteCutoverSelector {
        self.id.selector()
    }

    pub const fn facet(self) -> ThemeRouteCutoverFacet {
        self.id.facet()
    }

    pub const fn value(self) -> ThemeRouteCutoverValue {
        self.id.value()
    }

    pub const fn projections(self) -> ThemeRouteCutoverProjectionSet {
        self.projections
    }
}

/// Failure to bind a typed legacy-replacing route to every bridge projection it owns.
#[cfg(any(test, feature = "internal-theme-acceptance"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeRouteCutoverInventoryError {
    family_id: DiagramFamilyId,
    target: ThemeTarget,
    facet: ThemeRouteCutoverFacet,
}

#[cfg(any(test, feature = "internal-theme-acceptance"))]
impl ThemeRouteCutoverInventoryError {
    pub(crate) const fn missing_projections(
        family_id: DiagramFamilyId,
        target: ThemeTarget,
        facet: ThemeRouteCutoverFacet,
    ) -> Self {
        Self {
            family_id,
            target,
            facet,
        }
    }
}

#[cfg(any(test, feature = "internal-theme-acceptance"))]
impl fmt::Display for ThemeRouteCutoverInventoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "typed legacy-replacing route {}/{}/{:?} lacks projection obligations",
            self.family_id,
            self.target.id(),
            self.facet
        )
    }
}

#[cfg(any(test, feature = "internal-theme-acceptance"))]
impl std::error::Error for ThemeRouteCutoverInventoryError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_message_stroke_projection_is_an_exact_replacement() {
        let projection = ThemeRouteCutoverProjection::MessageStroke;
        let projections = ThemeRouteCutoverProjectionSet::REPLACE_MESSAGE_STROKE
            .iter()
            .collect::<Vec<_>>();

        assert_eq!(projection.contribution_id(), "message.stroke");
        assert_eq!(
            projection.action(),
            ThemeRouteCutoverProjectionAction::Replace
        );
        assert_eq!(projections, vec![projection]);
    }

    #[test]
    fn sequence_lifeline_paint_projection_is_one_exact_replacement() {
        let projection = ThemeRouteCutoverProjection::LifelineStroke;
        let projections = ThemeRouteCutoverProjectionSet::REPLACE_LIFELINE_STROKE
            .iter()
            .collect::<Vec<_>>();

        assert_eq!(projection.contribution_id(), "lifeline.stroke");
        assert_eq!(
            projection.action(),
            ThemeRouteCutoverProjectionAction::Replace
        );
        assert_eq!(projections, vec![projection]);
    }

    #[test]
    fn cluster_paint_projections_are_exact_replacements() {
        for (projection, set, contribution_id) in [
            (
                ThemeRouteCutoverProjection::ClusterFill,
                ThemeRouteCutoverProjectionSet::REPLACE_CLUSTER_FILL,
                "cluster.fill",
            ),
            (
                ThemeRouteCutoverProjection::ClusterStroke,
                ThemeRouteCutoverProjectionSet::REPLACE_CLUSTER_STROKE,
                "cluster.stroke",
            ),
        ] {
            assert_eq!(projection.contribution_id(), contribution_id);
            assert_eq!(
                projection.action(),
                ThemeRouteCutoverProjectionAction::Replace
            );
            assert_eq!(set.iter().collect::<Vec<_>>(), vec![projection]);
        }
    }

    #[test]
    fn title_fill_projection_is_an_exact_replacement() {
        let projection = ThemeRouteCutoverProjection::TitleFill;
        let projections = ThemeRouteCutoverProjectionSet::REPLACE_TITLE_FILL
            .iter()
            .collect::<Vec<_>>();

        assert_eq!(projection.contribution_id(), "title.fill");
        assert_eq!(
            projection.action(),
            ThemeRouteCutoverProjectionAction::Replace
        );
        assert_eq!(projections, vec![projection]);
    }

    #[test]
    fn requirement_fill_projection_is_an_exact_replacement() {
        let projection = ThemeRouteCutoverProjection::RequirementFill;
        let projections = ThemeRouteCutoverProjectionSet::REPLACE_REQUIREMENT_FILL
            .iter()
            .collect::<Vec<_>>();

        assert_eq!(projection.contribution_id(), "requirement.fill");
        assert_eq!(
            projection.action(),
            ThemeRouteCutoverProjectionAction::Replace
        );
        assert_eq!(projections, vec![projection]);
    }

    #[test]
    fn pie_slice_stroke_projection_is_an_exact_replacement() {
        let projection = ThemeRouteCutoverProjection::PieSliceStroke;
        let projections = ThemeRouteCutoverProjectionSet::REPLACE_PIE_SLICE_STROKE
            .iter()
            .collect::<Vec<_>>();

        assert_eq!(projection.contribution_id(), "slice.stroke");
        assert_eq!(
            projection.action(),
            ThemeRouteCutoverProjectionAction::Replace
        );
        assert_eq!(projections, vec![projection]);
    }
}
