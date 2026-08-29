use crate::DiagramFamilyId;

use super::legacy_tombstones::{
    ThemeLegacyRouteFacet, ThemeLegacyRouteId, ThemeLegacyRouteSelector,
};
use super::semantic::{ThemeTarget, ThemeVariant};

#[cfg(any(test, feature = "internal-theme-acceptance"))]
use sha2::{Digest as _, Sha256};

/// Atomic scalar paint value classes used by the historical transition witness.
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

/// One exact assignment that a retired bridge route formerly attempted to write.
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

/// Historical projection witness paired with one renderer-owned KTD23 tombstone identity.
///
/// The route identity lives in [`super::legacy_tombstones`]. The projection set is retained only
/// for historical acceptance reconciliation and is not consumed by the compatibility bridge.
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

/// Current matrix disposition observed independently of historical transition policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ThemeLegacyProjectionDisposition {
    TypedAdapter,
    LegacyCompatibility,
    Unsupported,
}

impl ThemeLegacyProjectionDisposition {
    pub const fn id(self) -> &'static str {
        match self {
            Self::TypedAdapter => "typed-adapter",
            Self::LegacyCompatibility => "legacy-compatibility",
            Self::Unsupported => "unsupported",
        }
    }
}

/// One exact assignment emitted by the current bridge for a probe value.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ThemeLegacyProjectionObservation {
    contribution_id: String,
    assignment_path: String,
    value_digest: [u8; 32],
}

impl ThemeLegacyProjectionObservation {
    #[cfg(any(test, feature = "internal-theme-acceptance"))]
    pub(super) fn new(
        contribution_id: String,
        assignment_path: String,
        value_digest: [u8; 32],
    ) -> Self {
        Self {
            contribution_id,
            assignment_path,
            value_digest,
        }
    }

    pub fn assignment_path(&self) -> &str {
        &self.assignment_path
    }
}

/// Production observation of one current matrix-and-bridge route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeLegacyProjectionProbeReceipt {
    id: ThemeLegacyRouteId,
    value: ThemeLegacyRouteValue,
    disposition: ThemeLegacyProjectionDisposition,
    projections: Vec<ThemeLegacyProjectionObservation>,
    digest: [u8; 32],
}

/// Opaque production seal proving both KTD23 probe values leave one retired route empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeLegacyProjectionRetirementReceipt {
    descriptor: ThemeLegacyProjectionRetirementDescriptor,
    digest: [u8; 32],
}

impl ThemeLegacyProjectionRetirementReceipt {
    #[cfg(any(test, feature = "internal-theme-acceptance"))]
    fn seal(
        descriptor: ThemeLegacyProjectionRetirementDescriptor,
        transparent_probe_digest: [u8; 32],
        solid_probe_digest: [u8; 32],
    ) -> Result<Self, ThemeLegacyProjectionRetirementInventoryError> {
        if transparent_probe_digest == [0; 32] || solid_probe_digest == [0; 32] {
            return Err(
                ThemeLegacyProjectionRetirementInventoryError::for_descriptor(
                    descriptor,
                    None,
                    "production bridge probe digest is zero",
                ),
            );
        }
        let digest =
            retirement_receipt_digest(descriptor, transparent_probe_digest, solid_probe_digest);
        Ok(Self { descriptor, digest })
    }

    pub const fn descriptor(self) -> ThemeLegacyProjectionRetirementDescriptor {
        self.descriptor
    }

    pub const fn digest(self) -> [u8; 32] {
        self.digest
    }
}

impl ThemeLegacyProjectionProbeReceipt {
    #[cfg(any(test, feature = "internal-theme-acceptance"))]
    pub(super) fn seal(
        id: ThemeLegacyRouteId,
        value: ThemeLegacyRouteValue,
        disposition: ThemeLegacyProjectionDisposition,
        mut projections: Vec<ThemeLegacyProjectionObservation>,
    ) -> Result<Self, ThemeLegacyProjectionProbeError> {
        projections.sort_unstable();
        if projections.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(ThemeLegacyProjectionProbeError::new(
                id,
                value,
                "current bridge emitted a duplicate projection observation",
            ));
        }
        if projections
            .iter()
            .any(|projection| projection.value_digest == [0; 32])
        {
            return Err(ThemeLegacyProjectionProbeError::new(
                id,
                value,
                "current bridge emitted a zero value digest",
            ));
        }
        let digest = probe_digest(id, value, disposition, &projections);
        Ok(Self {
            id,
            value,
            disposition,
            projections,
            digest,
        })
    }

    pub const fn id(&self) -> ThemeLegacyRouteId {
        self.id
    }

    pub const fn value(&self) -> ThemeLegacyRouteValue {
        self.value
    }

    pub const fn disposition(&self) -> ThemeLegacyProjectionDisposition {
        self.disposition
    }

    pub fn projections(&self) -> &[ThemeLegacyProjectionObservation] {
        &self.projections
    }

    pub const fn digest(&self) -> [u8; 32] {
        self.digest
    }
}

/// Failure to observe the current matrix-and-bridge route under a synthetic atomic value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeLegacyProjectionProbeError {
    id: ThemeLegacyRouteId,
    value: ThemeLegacyRouteValue,
    detail: String,
}

impl ThemeLegacyProjectionProbeError {
    pub(super) fn new(
        id: ThemeLegacyRouteId,
        value: ThemeLegacyRouteValue,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            id,
            value,
            detail: detail.into(),
        }
    }
}

impl std::fmt::Display for ThemeLegacyProjectionProbeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let variant = self
            .id
            .selector()
            .variant()
            .map_or("unqualified", ThemeVariant::id);
        write!(
            formatter,
            "legacy projection probe {}/{}/{}/{}/{}: {}",
            self.id.family_id().as_str(),
            self.id.target().id(),
            variant,
            self.id.facet().id(),
            self.value.id(),
            self.detail,
        )
    }
}

impl std::error::Error for ThemeLegacyProjectionProbeError {}

/// Failure while validating or sealing the production-owned KTD23 inventory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeLegacyProjectionRetirementInventoryError {
    descriptor: Option<ThemeLegacyProjectionRetirementDescriptor>,
    value: Option<ThemeLegacyRouteValue>,
    detail: String,
}

impl ThemeLegacyProjectionRetirementInventoryError {
    fn inventory(detail: impl Into<String>) -> Self {
        Self {
            descriptor: None,
            value: None,
            detail: detail.into(),
        }
    }

    fn for_descriptor(
        descriptor: ThemeLegacyProjectionRetirementDescriptor,
        value: Option<ThemeLegacyRouteValue>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            descriptor: Some(descriptor),
            value,
            detail: detail.into(),
        }
    }
}

impl std::fmt::Display for ThemeLegacyProjectionRetirementInventoryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Some(descriptor) = self.descriptor else {
            return write!(
                formatter,
                "legacy projection retirement inventory: {}",
                self.detail
            );
        };
        let id = descriptor.id();
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

#[derive(Debug, Clone, Copy)]
struct LegacyProjectionRetirementPattern {
    family_id: DiagramFamilyId,
    target: ThemeTarget,
    facet: ThemeLegacyRouteFacet,
    selectors: &'static [ThemeLegacyRouteSelector],
    former_projections: &'static [ThemeLegacyProjectionKey],
}

const EXPECTED_RETIREMENT_COUNT: usize = 56;

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
const EDGE_WITH_MARKER_FALLBACK: &[ThemeLegacyProjectionKey] = &[
    projection("edge.stroke", "themeVariables.lineColor"),
    projection("marker.paint-from-edge", "themeVariables.arrowheadColor"),
];
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
const MINDMAP_TEXT_FILL: &[ThemeLegacyProjectionKey] = &[
    projection("node-label.fill", "themeVariables.primaryTextColor"),
    projection("node-label.fill", "themeVariables.nodeTextColor"),
    projection("node-label.fill", "themeVariables.textColor"),
    projection("title.fill", "themeVariables.titleColor"),
    projection("cluster-label.fill", "themeVariables.secondaryTextColor"),
    projection("cluster-label.fill", "themeVariables.tertiaryTextColor"),
];

const CLASS_RETIREMENT_PATTERNS: &[LegacyProjectionRetirementPattern] = &[
    pattern(
        DiagramFamilyId::CLASS,
        ThemeTarget::Marker,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
        MARKER_PAINT,
    ),
    pattern(
        DiagramFamilyId::CLASS,
        ThemeTarget::Marker,
        ThemeLegacyRouteFacet::Stroke,
        UNQUALIFIED_AND_DEFAULT,
        MARKER_PAINT,
    ),
    pattern(
        DiagramFamilyId::CLASS,
        ThemeTarget::ClusterLabel,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
        CLUSTER_LABEL_FILL,
    ),
    pattern(
        DiagramFamilyId::CLASS,
        ThemeTarget::Table,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_ONLY,
        TABLE_ALL_FILL,
    ),
    pattern(
        DiagramFamilyId::CLASS,
        ThemeTarget::Table,
        ThemeLegacyRouteFacet::Fill,
        ODD_ONLY,
        TABLE_ODD_FILL,
    ),
    pattern(
        DiagramFamilyId::CLASS,
        ThemeTarget::Table,
        ThemeLegacyRouteFacet::Fill,
        EVEN_ONLY,
        TABLE_EVEN_FILL,
    ),
];

const MINDMAP_RETIREMENT_PATTERNS: &[LegacyProjectionRetirementPattern] = &[
    pattern(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::NodeLabel,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
        NODE_LABEL_FILL,
    ),
    pattern(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::Text,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
        MINDMAP_TEXT_FILL,
    ),
    pattern(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::Title,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
        TITLE_FILL,
    ),
    pattern(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::Edge,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
        EDGE_WITH_MARKER_FALLBACK,
    ),
    pattern(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::Edge,
        ThemeLegacyRouteFacet::Stroke,
        DEFAULT_ONLY,
        EDGE_WITH_MARKER_FALLBACK,
    ),
    pattern(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::Marker,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
        MARKER_PAINT,
    ),
    pattern(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::Marker,
        ThemeLegacyRouteFacet::Stroke,
        UNQUALIFIED_AND_DEFAULT,
        MARKER_PAINT,
    ),
    pattern(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::EdgeLabelBackground,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
        EDGE_LABEL_BACKGROUND_FILL,
    ),
    pattern(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::Cluster,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
        CLUSTER_FILL,
    ),
    pattern(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::Cluster,
        ThemeLegacyRouteFacet::Stroke,
        UNQUALIFIED_AND_DEFAULT,
        CLUSTER_STROKE,
    ),
    pattern(
        DiagramFamilyId::MINDMAP,
        ThemeTarget::ClusterLabel,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
        CLUSTER_LABEL_FILL,
    ),
];

const TREE_VIEW_RETIREMENT_PATTERNS: &[LegacyProjectionRetirementPattern] = &[
    pattern(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::Node,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
        NODE_FILL,
    ),
    pattern(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::Node,
        ThemeLegacyRouteFacet::Stroke,
        UNQUALIFIED_AND_DEFAULT,
        NODE_STROKE,
    ),
    pattern(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::Title,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
        TITLE_FILL,
    ),
    pattern(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::EdgeLabelBackground,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
        EDGE_LABEL_BACKGROUND_FILL,
    ),
    pattern(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::Cluster,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
        CLUSTER_FILL,
    ),
    pattern(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::Cluster,
        ThemeLegacyRouteFacet::Stroke,
        UNQUALIFIED_AND_DEFAULT,
        CLUSTER_STROKE,
    ),
    pattern(
        DiagramFamilyId::TREE_VIEW,
        ThemeTarget::ClusterLabel,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
        CLUSTER_LABEL_FILL,
    ),
];

const GIT_GRAPH_RETIREMENT_PATTERNS: &[LegacyProjectionRetirementPattern] = &[
    pattern(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::Title,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
        TITLE_FILL,
    ),
    pattern(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::Marker,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
        MARKER_PAINT,
    ),
    pattern(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::Marker,
        ThemeLegacyRouteFacet::Stroke,
        UNQUALIFIED_AND_DEFAULT,
        MARKER_PAINT,
    ),
    pattern(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::Cluster,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
        CLUSTER_FILL,
    ),
    pattern(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::Cluster,
        ThemeLegacyRouteFacet::Stroke,
        UNQUALIFIED_AND_DEFAULT,
        CLUSTER_STROKE,
    ),
    pattern(
        DiagramFamilyId::GIT_GRAPH,
        ThemeTarget::ClusterLabel,
        ThemeLegacyRouteFacet::Fill,
        UNQUALIFIED_AND_DEFAULT,
        CLUSTER_LABEL_FILL,
    ),
];

const RETIREMENT_PATTERN_GROUPS: &[&[LegacyProjectionRetirementPattern]] = &[
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
    facet: ThemeLegacyRouteFacet,
    selectors: &'static [ThemeLegacyRouteSelector],
    former_projections: &'static [ThemeLegacyProjectionKey],
) -> LegacyProjectionRetirementPattern {
    LegacyProjectionRetirementPattern {
        family_id,
        target,
        facet,
        selectors,
        former_projections,
    }
}

/// Returns historical projection rows aligned to the renderer-owned KTD23 tombstone identities.
#[cfg(any(test, feature = "internal-theme-acceptance"))]
pub(crate) fn legacy_projection_retirement_inventory() -> Result<
    Vec<ThemeLegacyProjectionRetirementDescriptor>,
    ThemeLegacyProjectionRetirementInventoryError,
> {
    let mut descriptors = Vec::with_capacity(EXPECTED_RETIREMENT_COUNT);
    for pattern in RETIREMENT_PATTERN_GROUPS
        .iter()
        .flat_map(|patterns| patterns.iter())
    {
        for selector in pattern.selectors {
            descriptors.push(ThemeLegacyProjectionRetirementDescriptor::new(
                ThemeLegacyRouteId::new(
                    pattern.family_id,
                    pattern.target,
                    *selector,
                    pattern.facet,
                ),
                pattern.former_projections,
            ));
        }
    }
    descriptors.sort_unstable();
    if descriptors.len() != EXPECTED_RETIREMENT_COUNT {
        return Err(ThemeLegacyProjectionRetirementInventoryError::inventory(
            format!(
                "expected {EXPECTED_RETIREMENT_COUNT} routes, observed {}",
                descriptors.len()
            ),
        ));
    }
    if descriptors
        .windows(2)
        .any(|pair| pair[0].id() == pair[1].id())
    {
        return Err(ThemeLegacyProjectionRetirementInventoryError::inventory(
            "duplicate route identity",
        ));
    }
    if descriptors
        .iter()
        .any(|descriptor| descriptor.former_projections().is_empty())
    {
        return Err(ThemeLegacyProjectionRetirementInventoryError::inventory(
            "retired routes must bind a non-empty historical projection set",
        ));
    }
    let tombstones = super::legacy_tombstones::ktd23_tombstone_inventory().map_err(|error| {
        ThemeLegacyProjectionRetirementInventoryError::inventory(error.to_string())
    })?;
    let descriptor_ids = descriptors
        .iter()
        .map(|descriptor| descriptor.id())
        .collect::<Vec<_>>();
    if descriptor_ids != tombstones {
        return Err(ThemeLegacyProjectionRetirementInventoryError::inventory(
            "historical projection rows drifted from renderer-owned tombstone identities",
        ));
    }
    Ok(descriptors)
}

/// Seals one opaque receipt per production-owned KTD23 route.
#[cfg(any(test, feature = "internal-theme-acceptance"))]
pub(crate) fn legacy_projection_retirement_receipts() -> Result<
    Vec<ThemeLegacyProjectionRetirementReceipt>,
    ThemeLegacyProjectionRetirementInventoryError,
> {
    let descriptors = legacy_projection_retirement_inventory()?;
    let mut receipts = Vec::with_capacity(descriptors.len());
    for descriptor in descriptors {
        let mut probe_digests = [[0; 32]; 2];
        for (index, value) in ThemeLegacyRouteValue::ALL.into_iter().enumerate() {
            let probe =
                super::legacy_family_theme_bridge::legacy_projection_probe(descriptor.id(), value)
                    .map_err(|error| {
                        ThemeLegacyProjectionRetirementInventoryError::for_descriptor(
                            descriptor,
                            Some(value),
                            error.to_string(),
                        )
                    })?;
            if probe.id() != descriptor.id() || probe.value() != value {
                return Err(
                    ThemeLegacyProjectionRetirementInventoryError::for_descriptor(
                        descriptor,
                        Some(value),
                        "production probe identity drifted",
                    ),
                );
            }
            if probe.disposition() != ThemeLegacyProjectionDisposition::Unsupported {
                return Err(
                    ThemeLegacyProjectionRetirementInventoryError::for_descriptor(
                        descriptor,
                        Some(value),
                        format!(
                            "expected unsupported disposition, observed {}",
                            probe.disposition().id()
                        ),
                    ),
                );
            }
            if !probe.projections().is_empty() {
                return Err(
                    ThemeLegacyProjectionRetirementInventoryError::for_descriptor(
                        descriptor,
                        Some(value),
                        "retired route still emits compatibility projections",
                    ),
                );
            }
            probe_digests[index] = probe.digest();
        }
        receipts.push(ThemeLegacyProjectionRetirementReceipt::seal(
            descriptor,
            probe_digests[0],
            probe_digests[1],
        )?);
    }
    Ok(receipts)
}

#[cfg(any(test, feature = "internal-theme-acceptance"))]
fn retirement_descriptor_digest(descriptor: ThemeLegacyProjectionRetirementDescriptor) -> [u8; 32] {
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
    // Acceptance canonicalizes projection sets before hashing. Keep the renderer
    // seal independent from declaration order as well, so reordering a static
    // inventory row cannot invalidate an otherwise identical retirement receipt.
    let mut projections = descriptor.former_projections().to_vec();
    projections.sort_unstable();
    for projection in projections {
        update_len_prefixed(&mut hasher, projection.contribution_suffix().as_bytes());
        update_len_prefixed(&mut hasher, projection.assignment_path().as_bytes());
    }
    hasher.finalize().into()
}

#[cfg(any(test, feature = "internal-theme-acceptance"))]
fn retirement_receipt_digest(
    descriptor: ThemeLegacyProjectionRetirementDescriptor,
    transparent_probe_digest: [u8; 32],
    solid_probe_digest: [u8; 32],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    update_len_prefixed(
        &mut hasher,
        b"merman.theme-legacy-projection-retirement-receipt.v1",
    );
    hasher.update(retirement_descriptor_digest(descriptor));
    hasher.update(transparent_probe_digest);
    hasher.update(solid_probe_digest);
    hasher.finalize().into()
}

#[cfg(any(test, feature = "internal-theme-acceptance"))]
fn probe_digest(
    id: ThemeLegacyRouteId,
    value: ThemeLegacyRouteValue,
    disposition: ThemeLegacyProjectionDisposition,
    projections: &[ThemeLegacyProjectionObservation],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    update_len_prefixed(
        &mut hasher,
        b"merman.theme-legacy-projection-current-probe.v2",
    );
    update_len_prefixed(&mut hasher, id.family_id().as_str().as_bytes());
    update_len_prefixed(&mut hasher, id.target().id().as_bytes());
    update_len_prefixed(&mut hasher, id.selector().id().as_bytes());
    if let Some(variant) = id.selector().variant() {
        update_len_prefixed(&mut hasher, variant.id().as_bytes());
    }
    update_len_prefixed(&mut hasher, id.facet().id().as_bytes());
    update_len_prefixed(&mut hasher, value.id().as_bytes());
    update_len_prefixed(&mut hasher, disposition.id().as_bytes());
    hasher.update((projections.len() as u64).to_be_bytes());
    for projection in projections {
        update_len_prefixed(&mut hasher, projection.contribution_id.as_bytes());
        update_len_prefixed(&mut hasher, projection.assignment_path.as_bytes());
        hasher.update(projection.value_digest);
    }
    hasher.finalize().into()
}

#[cfg(any(test, feature = "internal-theme-acceptance"))]
pub(super) fn value_digest(value: &serde_json::Value) -> [u8; 32] {
    let mut hasher = Sha256::new();
    update_len_prefixed(&mut hasher, b"merman.theme-legacy-projection-value.v1");
    let encoded = serde_json::to_vec(value).expect("JSON values always serialize");
    update_len_prefixed(&mut hasher, &encoded);
    hasher.finalize().into()
}

#[cfg(any(test, feature = "internal-theme-acceptance"))]
fn update_len_prefixed(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_be_bytes());
    hasher.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_inventory_has_exact_ktd23_boundary() {
        let descriptors = legacy_projection_retirement_inventory().expect("valid KTD23 inventory");
        assert_eq!(descriptors.len(), EXPECTED_RETIREMENT_COUNT);
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
    fn class_title_is_not_in_the_ktd23_inventory() {
        let descriptors = legacy_projection_retirement_inventory().expect("valid KTD23 inventory");
        assert!(descriptors.iter().all(|descriptor| {
            descriptor.id().family_id() != DiagramFamilyId::CLASS
                || descriptor.id().target() != ThemeTarget::Title
        }));
    }

    #[test]
    fn retirement_descriptor_digest_is_independent_of_projection_declaration_order() {
        const DECLARED_ORDER: &[ThemeLegacyProjectionKey] = &[
            projection("node.fill", "themeVariables.primaryColor"),
            projection("node.fill", "themeVariables.mainBkg"),
        ];
        const REVERSED_ORDER: &[ThemeLegacyProjectionKey] = &[
            projection("node.fill", "themeVariables.mainBkg"),
            projection("node.fill", "themeVariables.primaryColor"),
        ];
        let id = ThemeLegacyRouteId::new(
            DiagramFamilyId::TREE_VIEW,
            ThemeTarget::Node,
            ThemeLegacyRouteSelector::StaticUnqualified,
            ThemeLegacyRouteFacet::Fill,
        );
        let declared = ThemeLegacyProjectionRetirementDescriptor::new(id, DECLARED_ORDER);
        let reversed = ThemeLegacyProjectionRetirementDescriptor::new(id, REVERSED_ORDER);

        assert_eq!(
            retirement_descriptor_digest(declared),
            retirement_descriptor_digest(reversed)
        );
    }
}
