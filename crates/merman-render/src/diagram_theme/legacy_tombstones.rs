//! Renderer-owned identities for permanently retired legacy theme routes.
//!
//! This module deliberately contains no compatibility projection logic and does not observe
//! finalized SVG/CSS. Its inventory is an immutable route list used to keep historical KTD23
//! tombstones independent from the transitional compatibility bridge.

use crate::DiagramFamilyId;

use super::semantic::{ThemeTarget, ThemeVariant};

/// One static selector in the historical Mermaid compatibility projection domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ThemeLegacyRouteSelector {
    StaticUnqualified,
    StaticVariant(ThemeVariant),
}

impl ThemeLegacyRouteSelector {
    pub const fn variant(self) -> Option<ThemeVariant> {
        match self {
            Self::StaticUnqualified => None,
            Self::StaticVariant(variant) => Some(variant),
        }
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::StaticUnqualified => "static-unqualified",
            Self::StaticVariant(ThemeVariant::Default) => "static-default",
            Self::StaticVariant(ThemeVariant::Odd) => "static-odd",
            Self::StaticVariant(ThemeVariant::Even) => "static-even",
            Self::StaticVariant(_) => "static-qualified",
        }
    }
}

/// Paint facet of one retired legacy route.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ThemeLegacyRouteFacet {
    Fill,
    Stroke,
}

impl ThemeLegacyRouteFacet {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Fill => "fill",
            Self::Stroke => "stroke",
        }
    }
}

/// Stable identity for one renderer-owned KTD23 tombstone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ThemeLegacyRouteId {
    family_id: DiagramFamilyId,
    target: ThemeTarget,
    selector: ThemeLegacyRouteSelector,
    facet: ThemeLegacyRouteFacet,
}

impl ThemeLegacyRouteId {
    pub const fn new(
        family_id: DiagramFamilyId,
        target: ThemeTarget,
        selector: ThemeLegacyRouteSelector,
        facet: ThemeLegacyRouteFacet,
    ) -> Self {
        Self {
            family_id,
            target,
            selector,
            facet,
        }
    }

    pub const fn family_id(self) -> DiagramFamilyId {
        self.family_id
    }

    pub const fn target(self) -> ThemeTarget {
        self.target
    }

    pub const fn selector(self) -> ThemeLegacyRouteSelector {
        self.selector
    }

    pub const fn facet(self) -> ThemeLegacyRouteFacet {
        self.facet
    }
}

/// Failure while validating the renderer-owned KTD23 tombstone inventory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeLegacyTombstoneInventoryError {
    detail: String,
}

impl ThemeLegacyTombstoneInventoryError {
    pub(crate) fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
        }
    }
}

impl std::fmt::Display for ThemeLegacyTombstoneInventoryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "legacy theme tombstone inventory: {}",
            self.detail
        )
    }
}

impl std::error::Error for ThemeLegacyTombstoneInventoryError {}

#[derive(Debug, Clone, Copy)]
struct TombstonePattern {
    family_id: DiagramFamilyId,
    target: ThemeTarget,
    facet: ThemeLegacyRouteFacet,
    selectors: &'static [ThemeLegacyRouteSelector],
}

const EXPECTED_KTD23_TOMBSTONE_COUNT: usize = 56;

const UNQUALIFIED_AND_DEFAULT: &[ThemeLegacyRouteSelector] = &[
    ThemeLegacyRouteSelector::StaticUnqualified,
    ThemeLegacyRouteSelector::StaticVariant(ThemeVariant::Default),
];
const DEFAULT_ONLY: &[ThemeLegacyRouteSelector] = &[ThemeLegacyRouteSelector::StaticVariant(
    ThemeVariant::Default,
)];
const UNQUALIFIED_ONLY: &[ThemeLegacyRouteSelector] =
    &[ThemeLegacyRouteSelector::StaticUnqualified];
const ODD_ONLY: &[ThemeLegacyRouteSelector] =
    &[ThemeLegacyRouteSelector::StaticVariant(ThemeVariant::Odd)];
const EVEN_ONLY: &[ThemeLegacyRouteSelector] =
    &[ThemeLegacyRouteSelector::StaticVariant(ThemeVariant::Even)];

const CLASS_TOMBSTONES: &[TombstonePattern] = &[
    pattern(
        DiagramFamilyId::CLASS,
        ThemeTarget::Marker,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
    ),
    pattern(
        DiagramFamilyId::CLASS,
        ThemeTarget::Marker,
        ThemeLegacyRouteFacet::Stroke,
        UNQUALIFIED_AND_DEFAULT,
    ),
    pattern(
        DiagramFamilyId::CLASS,
        ThemeTarget::ClusterLabel,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
    ),
    pattern(
        DiagramFamilyId::CLASS,
        ThemeTarget::Table,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_ONLY,
    ),
    pattern(
        DiagramFamilyId::CLASS,
        ThemeTarget::Table,
        ThemeLegacyRouteFacet::Fill,
        ODD_ONLY,
    ),
    pattern(
        DiagramFamilyId::CLASS,
        ThemeTarget::Table,
        ThemeLegacyRouteFacet::Fill,
        EVEN_ONLY,
    ),
];

const MINDMAP_TOMBSTONES: &[TombstonePattern] = &[
    pattern(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::NodeLabel,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
    ),
    pattern(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::Text,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
    ),
    pattern(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::Title,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
    ),
    pattern(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::Edge,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
    ),
    pattern(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::Edge,
        ThemeLegacyRouteFacet::Stroke,
        DEFAULT_ONLY,
    ),
    pattern(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::Marker,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
    ),
    pattern(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::Marker,
        ThemeLegacyRouteFacet::Stroke,
        UNQUALIFIED_AND_DEFAULT,
    ),
    pattern(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::EdgeLabelBackground,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
    ),
    pattern(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::Cluster,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
    ),
    pattern(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::Cluster,
        ThemeLegacyRouteFacet::Stroke,
        UNQUALIFIED_AND_DEFAULT,
    ),
    pattern(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::ClusterLabel,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
    ),
];

const TREE_VIEW_TOMBSTONES: &[TombstonePattern] = &[
    pattern(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::Node,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
    ),
    pattern(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::Node,
        ThemeLegacyRouteFacet::Stroke,
        UNQUALIFIED_AND_DEFAULT,
    ),
    pattern(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::Title,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
    ),
    pattern(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::EdgeLabelBackground,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
    ),
    pattern(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::Cluster,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
    ),
    pattern(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::Cluster,
        ThemeLegacyRouteFacet::Stroke,
        UNQUALIFIED_AND_DEFAULT,
    ),
    pattern(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::ClusterLabel,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
    ),
];

const GIT_GRAPH_TOMBSTONES: &[TombstonePattern] = &[
    pattern(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::Title,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
    ),
    pattern(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::Marker,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
    ),
    pattern(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::Marker,
        ThemeLegacyRouteFacet::Stroke,
        UNQUALIFIED_AND_DEFAULT,
    ),
    pattern(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::Cluster,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
    ),
    pattern(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::Cluster,
        ThemeLegacyRouteFacet::Stroke,
        UNQUALIFIED_AND_DEFAULT,
    ),
    pattern(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::ClusterLabel,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
    ),
];

const TOMBSTONE_GROUPS: &[&[TombstonePattern]] = &[
    CLASS_TOMBSTONES,
    MINDMAP_TOMBSTONES,
    TREE_VIEW_TOMBSTONES,
    GIT_GRAPH_TOMBSTONES,
];

const fn pattern(
    family_id: DiagramFamilyId,
    target: ThemeTarget,
    facet: ThemeLegacyRouteFacet,
    selectors: &'static [ThemeLegacyRouteSelector],
) -> TombstonePattern {
    TombstonePattern {
        family_id,
        target,
        facet,
        selectors,
    }
}

/// Returns the fixed renderer-owned KTD23 tombstone identities.
///
/// The returned list is sorted and duplicate-free. Its route count is intentionally fixed so a
/// missing or duplicated tombstone fails closed before acceptance can compare any receipt.
pub fn ktd23_tombstone_inventory()
-> Result<Vec<ThemeLegacyRouteId>, ThemeLegacyTombstoneInventoryError> {
    let mut routes = Vec::with_capacity(EXPECTED_KTD23_TOMBSTONE_COUNT);
    for pattern in TOMBSTONE_GROUPS.iter().flat_map(|patterns| patterns.iter()) {
        for selector in pattern.selectors {
            routes.push(ThemeLegacyRouteId::new(
                pattern.family_id,
                pattern.target,
                *selector,
                pattern.facet,
            ));
        }
    }
    routes.sort_unstable();
    if routes.len() != EXPECTED_KTD23_TOMBSTONE_COUNT {
        return Err(ThemeLegacyTombstoneInventoryError::new(format!(
            "expected {EXPECTED_KTD23_TOMBSTONE_COUNT} routes, observed {}",
            routes.len()
        )));
    }
    if routes.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(ThemeLegacyTombstoneInventoryError::new(
            "duplicate route identity",
        ));
    }
    Ok(routes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ktd23_tombstone_inventory_has_stable_route_boundary() {
        let routes = ktd23_tombstone_inventory().expect("valid KTD23 tombstones");
        assert_eq!(routes.len(), EXPECTED_KTD23_TOMBSTONE_COUNT);
        assert!(routes.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(routes.iter().any(|route| {
            route.family_id() == DiagramFamilyId::MINDMAP
                && route.target() == ThemeTarget::Edge
                && route.selector()
                    == ThemeLegacyRouteSelector::StaticVariant(ThemeVariant::Default)
                && route.facet() == ThemeLegacyRouteFacet::Stroke
        }));
    }

    #[test]
    fn ktd23_tombstone_inventory_excludes_live_family_routes() {
        let routes = ktd23_tombstone_inventory().expect("valid KTD23 tombstones");
        assert!(routes.iter().all(|route| {
            !matches!(
                (route.family_id(), route.target()),
                (DiagramFamilyId::CLASS, ThemeTarget::Title)
                    | (DiagramFamilyId::MINDMAP, ThemeTarget::Node)
                    | (DiagramFamilyId::GIT_GRAPH, ThemeTarget::Node)
            )
        }));
    }
}
