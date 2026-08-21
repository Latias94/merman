use crate::DiagramFamilyId;

use super::family_mechanism_matrix::{FamilyThemeRuleFacet, FamilyThemeSelectorShape};
use super::semantic::{ThemeTarget, ThemeVariant};

#[cfg(any(test, feature = "internal-theme-acceptance"))]
use sha2::{Digest as _, Sha256};

/// One static selector formerly admitted by the Mermaid compatibility bridge.
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

/// Paint facet formerly projected through Mermaid compatibility.
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

/// Atomic scalar paint value classes probed before a compatibility route is retired.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ThemeLegacyRouteValue {
    Transparent,
    Solid,
}

impl ThemeLegacyRouteValue {
    pub const ALL: [Self; 2] = [Self::Transparent, Self::Solid];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Transparent => "transparent",
            Self::Solid => "solid",
        }
    }
}

/// Canonical identity for one terminal-absence compatibility retirement.
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

/// One exact assignment that the retired bridge route formerly attempted to write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ThemeLegacyProjectionKey {
    contribution_suffix: &'static str,
    assignment_path: &'static str,
}

impl ThemeLegacyProjectionKey {
    pub const fn new(contribution_suffix: &'static str, assignment_path: &'static str) -> Self {
        Self {
            contribution_suffix,
            assignment_path,
        }
    }

    pub const fn contribution_suffix(self) -> &'static str {
        self.contribution_suffix
    }

    pub const fn assignment_path(self) -> &'static str {
        self.assignment_path
    }
}

/// Production-owned legacy projection retirement plus the exact historical bridge projection set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ThemeLegacyProjectionRetirementDescriptor {
    id: ThemeLegacyRouteId,
    former_projections: &'static [ThemeLegacyProjectionKey],
}

impl ThemeLegacyProjectionRetirementDescriptor {
    pub const fn new(
        id: ThemeLegacyRouteId,
        former_projections: &'static [ThemeLegacyProjectionKey],
    ) -> Self {
        Self {
            id,
            former_projections,
        }
    }

    pub const fn id(self) -> ThemeLegacyRouteId {
        self.id
    }

    pub const fn former_projections(self) -> &'static [ThemeLegacyProjectionKey] {
        self.former_projections
    }
}

/// Opaque production seal proving both atomic value classes leave the retired bridge empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeLegacyProjectionRetirementReceipt {
    descriptor: ThemeLegacyProjectionRetirementDescriptor,
    digest: [u8; 32],
}

impl ThemeLegacyProjectionRetirementReceipt {
    #[cfg(any(test, feature = "internal-theme-acceptance"))]
    pub(super) fn seal(
        descriptor: ThemeLegacyProjectionRetirementDescriptor,
        transparent_probe_digest: [u8; 32],
        solid_probe_digest: [u8; 32],
    ) -> Result<Self, ThemeLegacyProjectionRetirementInventoryError> {
        if transparent_probe_digest == [0; 32] || solid_probe_digest == [0; 32] {
            return Err(ThemeLegacyProjectionRetirementInventoryError::new(
                descriptor,
                None,
                "production bridge probe digest is zero",
            ));
        }
        let digest = receipt_digest(descriptor, transparent_probe_digest, solid_probe_digest);
        Ok(Self { descriptor, digest })
    }

    pub const fn descriptor(self) -> ThemeLegacyProjectionRetirementDescriptor {
        self.descriptor
    }

    pub const fn digest(self) -> [u8; 32] {
        self.digest
    }
}

/// Production inventory failure while sealing one retired compatibility projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeLegacyProjectionRetirementInventoryError {
    descriptor: ThemeLegacyProjectionRetirementDescriptor,
    value: Option<ThemeLegacyRouteValue>,
    detail: String,
}

impl ThemeLegacyProjectionRetirementInventoryError {
    pub(super) fn new(
        descriptor: ThemeLegacyProjectionRetirementDescriptor,
        value: Option<ThemeLegacyRouteValue>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            descriptor,
            value,
            detail: detail.into(),
        }
    }

    pub const fn descriptor(&self) -> ThemeLegacyProjectionRetirementDescriptor {
        self.descriptor
    }

    pub const fn value(&self) -> Option<ThemeLegacyRouteValue> {
        self.value
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl std::fmt::Display for ThemeLegacyProjectionRetirementInventoryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let id = self.descriptor.id();
        write!(
            formatter,
            "legacy projection retirement {}/{}/{}/{}",
            id.family_id().as_str(),
            id.target().id(),
            id.selector().id(),
            id.facet().id(),
        )?;
        if let Some(value) = self.value {
            write!(formatter, "/{}", value.id())?;
        }
        write!(formatter, ": {}", self.detail)
    }
}

impl std::error::Error for ThemeLegacyProjectionRetirementInventoryError {}

type PaintChannel = ThemeLegacyRouteFacet;

#[derive(Debug, Clone, Copy)]
struct LegacyProjectionRetirementPattern {
    family_id: DiagramFamilyId,
    target: ThemeTarget,
    channel: PaintChannel,
    selectors: &'static [ThemeLegacyRouteSelector],
    projections: &'static [ThemeLegacyProjectionKey],
}

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

const NODE_FILL: &[ThemeLegacyProjectionKey] = &[
    projection("node.fill", "themeVariables.primaryColor"),
    projection("node.fill", "themeVariables.mainBkg"),
];
const NODE_STROKE: &[ThemeLegacyProjectionKey] = &[
    projection("node.stroke", "themeVariables.primaryBorderColor"),
    projection("node.stroke", "themeVariables.nodeBorder"),
];
const NODE_LABEL_FILL: &[ThemeLegacyProjectionKey] = &[
    projection("node-label.fill", "themeVariables.primaryTextColor"),
    projection("node-label.fill", "themeVariables.nodeTextColor"),
    projection("node-label.fill", "themeVariables.textColor"),
];
const TITLE_FILL: &[ThemeLegacyProjectionKey] =
    &[projection("title.fill", "themeVariables.titleColor")];
const EDGE_STROKE: &[ThemeLegacyProjectionKey] =
    &[projection("edge.stroke", "themeVariables.lineColor")];
const MARKER_PAINT: &[ThemeLegacyProjectionKey] =
    &[projection("marker.paint", "themeVariables.arrowheadColor")];
const EDGE_LABEL_BACKGROUND_FILL: &[ThemeLegacyProjectionKey] = &[projection(
    "edge-label-background.fill",
    "themeVariables.edgeLabelBackground",
)];
const CLUSTER_FILL: &[ThemeLegacyProjectionKey] = &[
    projection("cluster.fill", "themeVariables.clusterBkg"),
    projection("cluster.fill", "themeVariables.secondaryColor"),
];
const CLUSTER_STROKE: &[ThemeLegacyProjectionKey] =
    &[projection("cluster.stroke", "themeVariables.clusterBorder")];
const CLUSTER_LABEL_FILL: &[ThemeLegacyProjectionKey] = &[
    projection("cluster-label.fill", "themeVariables.secondaryTextColor"),
    projection("cluster-label.fill", "themeVariables.tertiaryTextColor"),
];
const TABLE_ODD_FILL: &[ThemeLegacyProjectionKey] = &[
    projection(
        "table.odd.fill",
        "themeVariables.attributeBackgroundColorOdd",
    ),
    projection("table.odd.fill", "themeVariables.rowOdd"),
];
const TABLE_EVEN_FILL: &[ThemeLegacyProjectionKey] = &[
    projection(
        "table.even.fill",
        "themeVariables.attributeBackgroundColorEven",
    ),
    projection("table.even.fill", "themeVariables.rowEven"),
];
const TABLE_ALL_FILL: &[ThemeLegacyProjectionKey] = &[
    projection(
        "table.odd.fill",
        "themeVariables.attributeBackgroundColorOdd",
    ),
    projection("table.odd.fill", "themeVariables.rowOdd"),
    projection(
        "table.even.fill",
        "themeVariables.attributeBackgroundColorEven",
    ),
    projection("table.even.fill", "themeVariables.rowEven"),
];

const CLASS_RETIREMENT_PATTERNS: &[LegacyProjectionRetirementPattern] = &[
    // Class retains Title.fill for namespace labels. Marker, ClusterLabel, and table paint have no
    // independent terminal consumer in the Class writer.
    pattern(
        DiagramFamilyId::CLASS,
        ThemeTarget::Marker,
        PaintChannel::Fill,
        UNQUALIFIED_AND_DEFAULT,
        MARKER_PAINT,
    ),
    pattern(
        DiagramFamilyId::CLASS,
        ThemeTarget::Marker,
        PaintChannel::Stroke,
        UNQUALIFIED_AND_DEFAULT,
        MARKER_PAINT,
    ),
    pattern(
        DiagramFamilyId::CLASS,
        ThemeTarget::ClusterLabel,
        PaintChannel::Fill,
        UNQUALIFIED_AND_DEFAULT,
        CLUSTER_LABEL_FILL,
    ),
    pattern(
        DiagramFamilyId::CLASS,
        ThemeTarget::Table,
        PaintChannel::Fill,
        UNQUALIFIED_ONLY,
        TABLE_ALL_FILL,
    ),
    pattern(
        DiagramFamilyId::CLASS,
        ThemeTarget::Table,
        PaintChannel::Fill,
        ODD_ONLY,
        TABLE_ODD_FILL,
    ),
    pattern(
        DiagramFamilyId::CLASS,
        ThemeTarget::Table,
        PaintChannel::Fill,
        EVEN_ONLY,
        TABLE_EVEN_FILL,
    ),
];

const MINDMAP_RETIREMENT_PATTERNS: &[LegacyProjectionRetirementPattern] = &[
    // Mindmap directly owns Node palette and unqualified Edge.stroke only.
    pattern(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::NodeLabel,
        PaintChannel::Fill,
        UNQUALIFIED_AND_DEFAULT,
        NODE_LABEL_FILL,
    ),
    pattern(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::Text,
        PaintChannel::Fill,
        UNQUALIFIED_AND_DEFAULT,
        NODE_LABEL_FILL,
    ),
    pattern(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::Title,
        PaintChannel::Fill,
        UNQUALIFIED_AND_DEFAULT,
        TITLE_FILL,
    ),
    pattern(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::Edge,
        PaintChannel::Fill,
        UNQUALIFIED_AND_DEFAULT,
        EDGE_STROKE,
    ),
    pattern(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::Edge,
        PaintChannel::Stroke,
        DEFAULT_ONLY,
        EDGE_STROKE,
    ),
    pattern(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::Marker,
        PaintChannel::Fill,
        UNQUALIFIED_AND_DEFAULT,
        MARKER_PAINT,
    ),
    pattern(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::Marker,
        PaintChannel::Stroke,
        UNQUALIFIED_AND_DEFAULT,
        MARKER_PAINT,
    ),
    pattern(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::EdgeLabelBackground,
        PaintChannel::Fill,
        UNQUALIFIED_AND_DEFAULT,
        EDGE_LABEL_BACKGROUND_FILL,
    ),
    pattern(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::Cluster,
        PaintChannel::Fill,
        UNQUALIFIED_AND_DEFAULT,
        CLUSTER_FILL,
    ),
    pattern(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::Cluster,
        PaintChannel::Stroke,
        UNQUALIFIED_AND_DEFAULT,
        CLUSTER_STROKE,
    ),
    pattern(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::ClusterLabel,
        PaintChannel::Fill,
        UNQUALIFIED_AND_DEFAULT,
        CLUSTER_LABEL_FILL,
    ),
];

const TREE_VIEW_RETIREMENT_PATTERNS: &[LegacyProjectionRetirementPattern] = &[
    // TreeView has text, edge, and icon tokens, but no painted node shell, title, or cluster.
    pattern(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::Node,
        PaintChannel::Fill,
        UNQUALIFIED_AND_DEFAULT,
        NODE_FILL,
    ),
    pattern(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::Node,
        PaintChannel::Stroke,
        UNQUALIFIED_AND_DEFAULT,
        NODE_STROKE,
    ),
    pattern(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::Title,
        PaintChannel::Fill,
        UNQUALIFIED_AND_DEFAULT,
        TITLE_FILL,
    ),
    pattern(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::EdgeLabelBackground,
        PaintChannel::Fill,
        UNQUALIFIED_AND_DEFAULT,
        EDGE_LABEL_BACKGROUND_FILL,
    ),
    pattern(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::Cluster,
        PaintChannel::Fill,
        UNQUALIFIED_AND_DEFAULT,
        CLUSTER_FILL,
    ),
    pattern(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::Cluster,
        PaintChannel::Stroke,
        UNQUALIFIED_AND_DEFAULT,
        CLUSTER_STROKE,
    ),
    pattern(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::ClusterLabel,
        PaintChannel::Fill,
        UNQUALIFIED_AND_DEFAULT,
        CLUSTER_LABEL_FILL,
    ),
];

const GIT_GRAPH_RETIREMENT_PATTERNS: &[LegacyProjectionRetirementPattern] = &[
    // GitGraph arrows are Node palette surfaces, not Marker terminals; it has no cluster surface.
    pattern(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::Title,
        PaintChannel::Fill,
        UNQUALIFIED_AND_DEFAULT,
        TITLE_FILL,
    ),
    pattern(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::Marker,
        PaintChannel::Fill,
        UNQUALIFIED_AND_DEFAULT,
        MARKER_PAINT,
    ),
    pattern(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::Marker,
        PaintChannel::Stroke,
        UNQUALIFIED_AND_DEFAULT,
        MARKER_PAINT,
    ),
    pattern(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::Cluster,
        PaintChannel::Fill,
        UNQUALIFIED_AND_DEFAULT,
        CLUSTER_FILL,
    ),
    pattern(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::Cluster,
        PaintChannel::Stroke,
        UNQUALIFIED_AND_DEFAULT,
        CLUSTER_STROKE,
    ),
    pattern(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::ClusterLabel,
        PaintChannel::Fill,
        UNQUALIFIED_AND_DEFAULT,
        CLUSTER_LABEL_FILL,
    ),
];

const LEGACY_PROJECTION_RETIREMENT_PATTERN_GROUPS: &[&[LegacyProjectionRetirementPattern]] = &[
    CLASS_RETIREMENT_PATTERNS,
    MINDMAP_RETIREMENT_PATTERNS,
    TREE_VIEW_RETIREMENT_PATTERNS,
    GIT_GRAPH_RETIREMENT_PATTERNS,
];

const fn projection(
    contribution_suffix: &'static str,
    assignment_path: &'static str,
) -> ThemeLegacyProjectionKey {
    ThemeLegacyProjectionKey::new(contribution_suffix, assignment_path)
}

const fn pattern(
    family_id: DiagramFamilyId,
    target: ThemeTarget,
    channel: PaintChannel,
    selectors: &'static [ThemeLegacyRouteSelector],
    projections: &'static [ThemeLegacyProjectionKey],
) -> LegacyProjectionRetirementPattern {
    LegacyProjectionRetirementPattern {
        family_id,
        target,
        channel,
        selectors,
        projections,
    }
}

/// Returns true when the selected route has no family-owned terminal after direct routes have had
/// an opportunity to claim their exact selector and facet.
pub(super) fn route_has_retired_legacy_projection(
    family_id: DiagramFamilyId,
    target: ThemeTarget,
    selector: FamilyThemeSelectorShape,
    facet: FamilyThemeRuleFacet,
) -> bool {
    let Some(channel) = facet_channel(facet) else {
        return false;
    };
    retirement_patterns_for_family(family_id)
        .iter()
        .any(|pattern| {
            pattern.target == target
                && pattern.channel == channel
                && pattern
                    .selectors
                    .iter()
                    .any(|expected| selector_matches(*expected, selector))
        })
}

fn retirement_patterns_for_family(
    family_id: DiagramFamilyId,
) -> &'static [LegacyProjectionRetirementPattern] {
    match family_id {
        DiagramFamilyId::CLASS => CLASS_RETIREMENT_PATTERNS,
        DiagramFamilyId::MINDMAP => MINDMAP_RETIREMENT_PATTERNS,
        DiagramFamilyId::TREE_VIEW => TREE_VIEW_RETIREMENT_PATTERNS,
        DiagramFamilyId::GIT_GRAPH => GIT_GRAPH_RETIREMENT_PATTERNS,
        _ => &[],
    }
}

fn selector_matches(expected: ThemeLegacyRouteSelector, actual: FamilyThemeSelectorShape) -> bool {
    match (expected, actual) {
        (
            ThemeLegacyRouteSelector::StaticUnqualified,
            FamilyThemeSelectorShape::Static { variant: None },
        ) => true,
        (
            ThemeLegacyRouteSelector::StaticVariant(expected),
            FamilyThemeSelectorShape::Static {
                variant: Some(actual),
            },
        ) => expected == actual,
        (
            ThemeLegacyRouteSelector::StaticUnqualified
            | ThemeLegacyRouteSelector::StaticVariant(_),
            _,
        ) => false,
    }
}

fn facet_channel(facet: FamilyThemeRuleFacet) -> Option<PaintChannel> {
    match facet {
        FamilyThemeRuleFacet::Fill(_) => Some(PaintChannel::Fill),
        FamilyThemeRuleFacet::Stroke(_) => Some(PaintChannel::Stroke),
        FamilyThemeRuleFacet::StrokeWidth
        | FamilyThemeRuleFacet::StrokeDasharray
        | FamilyThemeRuleFacet::StrokeLinecap
        | FamilyThemeRuleFacet::StrokeLinejoin
        | FamilyThemeRuleFacet::Opacity
        | FamilyThemeRuleFacet::FillOpacity
        | FamilyThemeRuleFacet::StrokeOpacity
        | FamilyThemeRuleFacet::Radius
        | FamilyThemeRuleFacet::Padding
        | FamilyThemeRuleFacet::Typography(_)
        | FamilyThemeRuleFacet::Effect => None,
    }
}

#[cfg(any(test, feature = "internal-theme-acceptance"))]
pub(super) fn legacy_projection_retirement_descriptors()
-> Vec<ThemeLegacyProjectionRetirementDescriptor> {
    let mut descriptors = Vec::new();
    for pattern in LEGACY_PROJECTION_RETIREMENT_PATTERN_GROUPS
        .iter()
        .flat_map(|patterns| patterns.iter())
    {
        let facet = pattern.channel;
        for selector in pattern.selectors {
            descriptors.push(ThemeLegacyProjectionRetirementDescriptor::new(
                ThemeLegacyRouteId::new(pattern.family_id, pattern.target, *selector, facet),
                pattern.projections,
            ));
        }
    }
    descriptors.sort_unstable();
    descriptors
}

#[cfg(any(test, feature = "internal-theme-acceptance"))]
pub(super) fn descriptor_digest(descriptor: ThemeLegacyProjectionRetirementDescriptor) -> [u8; 32] {
    let mut hasher = Sha256::new();
    update_len_prefixed(&mut hasher, b"merman.theme-legacy-projection-retirement.v1");
    update_len_prefixed(&mut hasher, descriptor.id().family_id().as_str().as_bytes());
    update_len_prefixed(&mut hasher, descriptor.id().target().id().as_bytes());
    update_len_prefixed(&mut hasher, descriptor.id().selector().id().as_bytes());
    if let Some(variant) = descriptor.id().selector().variant() {
        update_len_prefixed(&mut hasher, variant.id().as_bytes());
    }
    update_len_prefixed(&mut hasher, descriptor.id().facet().id().as_bytes());
    for value in ThemeLegacyRouteValue::ALL {
        update_len_prefixed(&mut hasher, value.id().as_bytes());
    }
    for projection in descriptor.former_projections() {
        update_len_prefixed(&mut hasher, projection.contribution_suffix().as_bytes());
        update_len_prefixed(&mut hasher, projection.assignment_path().as_bytes());
    }
    hasher.finalize().into()
}

#[cfg(any(test, feature = "internal-theme-acceptance"))]
fn receipt_digest(
    descriptor: ThemeLegacyProjectionRetirementDescriptor,
    transparent_probe_digest: [u8; 32],
    solid_probe_digest: [u8; 32],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    update_len_prefixed(
        &mut hasher,
        b"merman.theme-legacy-projection-retirement-receipt.v1",
    );
    hasher.update(descriptor_digest(descriptor));
    hasher.update(transparent_probe_digest);
    hasher.update(solid_probe_digest);
    hasher.finalize().into()
}

#[cfg(any(test, feature = "internal-theme-acceptance"))]
pub(super) fn update_len_prefixed(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retirement_inventory_distinguishes_selector_shapes() {
        let descriptors = legacy_projection_retirement_descriptors();
        assert_eq!(descriptors.len(), 56);

        let mindmap_edge_stroke = descriptors
            .iter()
            .filter(|descriptor| {
                descriptor.id().family_id() == DiagramFamilyId::MINDMAP
                    && descriptor.id().target() == ThemeTarget::Edge
                    && descriptor.id().facet() == ThemeLegacyRouteFacet::Stroke
            })
            .collect::<Vec<_>>();
        assert_eq!(mindmap_edge_stroke.len(), 1);
        assert_eq!(
            mindmap_edge_stroke[0].id().selector(),
            ThemeLegacyRouteSelector::StaticVariant(ThemeVariant::Default)
        );
    }

    #[test]
    fn class_title_is_not_a_retired_projection_route() {
        for selector in [
            FamilyThemeSelectorShape::Static { variant: None },
            FamilyThemeSelectorShape::Static {
                variant: Some(ThemeVariant::Default),
            },
        ] {
            assert!(!route_has_retired_legacy_projection(
                DiagramFamilyId::CLASS,
                ThemeTarget::Title,
                selector,
                FamilyThemeRuleFacet::Fill(
                    super::super::family_mechanism_matrix::FamilyThemePaintKind::Solid,
                ),
            ));
        }
    }

    #[test]
    fn descriptor_digest_binds_selector_and_projection_set() {
        const DIFFERENT_PROJECTION: &[ThemeLegacyProjectionKey] = &[ThemeLegacyProjectionKey::new(
            "marker.paint",
            "themeVariables.lineColor",
        )];
        let descriptors = legacy_projection_retirement_descriptors();
        let marker = descriptors
            .iter()
            .copied()
            .find(|descriptor| {
                descriptor.id().family_id() == DiagramFamilyId::CLASS
                    && descriptor.id().target() == ThemeTarget::Marker
                    && descriptor.id().facet() == ThemeLegacyRouteFacet::Fill
                    && descriptor.id().selector() == ThemeLegacyRouteSelector::StaticUnqualified
            })
            .expect("Class Marker.fill descriptor");
        let qualified = ThemeLegacyProjectionRetirementDescriptor::new(
            ThemeLegacyRouteId::new(
                DiagramFamilyId::CLASS,
                ThemeTarget::Marker,
                ThemeLegacyRouteSelector::StaticVariant(ThemeVariant::Default),
                ThemeLegacyRouteFacet::Fill,
            ),
            marker.former_projections(),
        );
        let drifted =
            ThemeLegacyProjectionRetirementDescriptor::new(marker.id(), DIFFERENT_PROJECTION);

        assert_ne!(descriptor_digest(marker), descriptor_digest(qualified));
        assert_ne!(descriptor_digest(marker), descriptor_digest(drifted));
    }
}
