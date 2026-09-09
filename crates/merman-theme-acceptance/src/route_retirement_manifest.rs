use std::collections::{BTreeMap, BTreeSet};

use merman_render::__private::{
    ThemeLegacyProjectionRetirementDescriptor, ThemeLegacyProjectionRetirementReceipt,
    ThemeLegacyRouteFacet, ThemeLegacyRouteId, ThemeLegacyRouteSelector,
    retired_legacy_theme_projection_inventory, retired_legacy_theme_projection_receipts,
};
use merman_render::DiagramFamilyId;
use merman_render::diagram_theme::{ThemeTarget, ThemeVariant};
use sha2::{Digest as _, Sha256};

const RETIREMENT_MANIFEST_VERSION: u16 = 3;
const V1_RETIREMENT_BASELINE_REVISION: &str = "a4b1db26d315f044140a33378723ca8834452a75";
const V2_RETIREMENT_BASELINE_REVISION: &str = "4c3a2d839584555d506792aa76c4ee3a7332a9c8";
const V3_RETIREMENT_BASELINE_REVISION: &str = "dab18e2e5351be8605fd8d696075bca87953d29b";
const V1_EXPECTED_RETIREMENT_COUNT: usize = 56;
const V2_EXPECTED_RETIREMENT_COUNT: usize = 2;
const V3_EXPECTED_RETIREMENT_COUNT: usize = 2;
const EXPECTED_RETIREMENT_COUNT: usize =
    V1_EXPECTED_RETIREMENT_COUNT + V2_EXPECTED_RETIREMENT_COUNT + V3_EXPECTED_RETIREMENT_COUNT;
const EXPECTED_VALUE_PROBE_COUNT: usize = EXPECTED_RETIREMENT_COUNT * 2;

// Acceptance-owned authority. Update only after reviewing the independent historical witness,
// production inventory, and exact production receipt report respectively.
const EXPECTED_V1_HISTORICAL_WITNESS_DIGEST: [u8; 32] = [
    0xd7, 0xd6, 0x52, 0xcd, 0x59, 0x5f, 0x05, 0xd8, 0xbd, 0x9d, 0x79, 0x94, 0x31, 0xf3, 0x2c, 0x38,
    0xb4, 0x82, 0x7d, 0x8c, 0x9c, 0x0f, 0xd6, 0x5f, 0x91, 0xa9, 0xaa, 0xdf, 0x23, 0x67, 0xa9, 0xb3,
];
const EXPECTED_V1_PRODUCTION_INVENTORY_DIGEST: [u8; 32] = [
    0x36, 0x6c, 0xed, 0xc4, 0x10, 0xa0, 0x1c, 0x60, 0x5e, 0x9e, 0x0c, 0x63, 0xea, 0xf2, 0xc8, 0x7c,
    0x4f, 0x90, 0xef, 0x9d, 0xac, 0xcc, 0xd0, 0x05, 0xaf, 0xfa, 0xff, 0x44, 0xc6, 0x73, 0x86, 0xfa,
];
const EXPECTED_V1_RECEIPT_REPORT_DIGEST: [u8; 32] = [
    0x8a, 0x9b, 0x76, 0x5c, 0x16, 0x38, 0x8e, 0xe5, 0x54, 0x68, 0x50, 0x8e, 0x1f, 0x25, 0x19, 0x55,
    0xe3, 0xdd, 0xfd, 0xb1, 0x4a, 0x02, 0x41, 0x1b, 0x7b, 0xee, 0x0d, 0x16, 0xd0, 0x46, 0x1b, 0xee,
];
const EXPECTED_V2_HISTORICAL_WITNESS_DIGEST: [u8; 32] = [
    0x87, 0xd1, 0x80, 0xe6, 0xf9, 0x05, 0x49, 0x46, 0x4a, 0x7e, 0x80, 0x78, 0x3c, 0xd5, 0x89, 0xf7,
    0xc2, 0x66, 0x91, 0x92, 0x6b, 0x95, 0xd9, 0x50, 0x9e, 0xc4, 0xdc, 0x63, 0x32, 0x1a, 0xaf, 0xcd,
];
const EXPECTED_V3_HISTORICAL_WITNESS_DIGEST: [u8; 32] = [
    77, 14, 211, 72, 42, 177, 86, 75, 205, 18, 73, 188, 163, 105, 200, 220, 180, 214, 208, 139,
    213, 84, 151, 23, 58, 153, 18, 251, 31, 132, 175, 5,
];
const EXPECTED_HISTORICAL_WITNESS_DIGEST: [u8; 32] = [
    237, 19, 91, 44, 153, 97, 209, 33, 188, 18, 101, 113, 63, 49, 156, 133, 117, 32, 144, 121, 152,
    143, 104, 126, 60, 97, 24, 16, 212, 189, 216, 214,
];
const EXPECTED_PRODUCTION_INVENTORY_DIGEST: [u8; 32] = [
    123, 200, 145, 162, 227, 44, 217, 200, 225, 192, 200, 171, 248, 7, 216, 221, 36, 142, 27, 126,
    76, 50, 249, 42, 155, 3, 170, 18, 121, 26, 251, 17,
];
const EXPECTED_RECEIPT_REPORT_DIGEST: [u8; 32] = [
    226, 19, 52, 137, 33, 190, 125, 175, 188, 146, 213, 23, 163, 160, 6, 212, 222, 30, 133, 93,
    171, 79, 70, 1, 76, 180, 104, 109, 123, 149, 146, 84,
];

/// Successful authorization of the independently frozen KTD23 retirement boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegacyProjectionRetirementAuthorization {
    manifest_version: u16,
    manifest_digest: [u8; 32],
    historical_witness_digest: [u8; 32],
    production_inventory_digest: [u8; 32],
    receipt_report_digest: [u8; 32],
    retirement_count: usize,
    value_probe_count: usize,
}

impl LegacyProjectionRetirementAuthorization {
    pub const fn manifest_version(&self) -> u16 {
        self.manifest_version
    }

    pub const fn manifest_digest(&self) -> &[u8; 32] {
        &self.manifest_digest
    }

    pub const fn historical_witness_digest(&self) -> &[u8; 32] {
        &self.historical_witness_digest
    }

    pub const fn production_inventory_digest(&self) -> &[u8; 32] {
        &self.production_inventory_digest
    }

    pub const fn receipt_report_digest(&self) -> &[u8; 32] {
        &self.receipt_report_digest
    }

    pub const fn retirement_count(&self) -> usize {
        self.retirement_count
    }

    pub const fn value_probe_count(&self) -> usize {
        self.value_probe_count
    }
}

/// Feature-neutral KTD23 verification failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyProjectionVerificationError {
    stage: &'static str,
    detail: String,
}

impl LegacyProjectionVerificationError {
    fn new(stage: &'static str, detail: impl Into<String>) -> Self {
        Self {
            stage,
            detail: detail.into(),
        }
    }

    pub const fn stage(&self) -> &'static str {
        self.stage
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }
}

impl std::fmt::Display for LegacyProjectionVerificationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.stage, self.detail)
    }
}

impl std::error::Error for LegacyProjectionVerificationError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct HistoricalProjectionKey {
    contribution_suffix: &'static str,
    assignment_path: &'static str,
}

#[derive(Debug, Clone, Copy)]
struct HistoricalRetirementPattern {
    family_id: DiagramFamilyId,
    target: ThemeTarget,
    facet: ThemeLegacyRouteFacet,
    selectors: &'static [ThemeLegacyRouteSelector],
    former_projections: &'static [HistoricalProjectionKey],
}

#[derive(Debug, Clone, Copy)]
struct HistoricalRetirementBatchSpec {
    version: u16,
    baseline_revision: &'static str,
    expected_count: usize,
    groups: &'static [&'static [HistoricalRetirementPattern]],
}

#[derive(Debug)]
struct HistoricalRetirementBatch {
    version: u16,
    baseline_revision: &'static str,
    retirements: Vec<HistoricalRetirement>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct HistoricalRetirement {
    family_id: DiagramFamilyId,
    target: ThemeTarget,
    selector: ThemeLegacyRouteSelector,
    facet: ThemeLegacyRouteFacet,
    former_projections: &'static [HistoricalProjectionKey],
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

const NODE_FILL: &[HistoricalProjectionKey] = &[
    projection("node.fill", "themeVariables.primaryColor"),
    projection("node.fill", "themeVariables.mainBkg"),
];
const NODE_STROKE: &[HistoricalProjectionKey] = &[
    projection("node.stroke", "themeVariables.primaryBorderColor"),
    projection("node.stroke", "themeVariables.nodeBorder"),
];
const NODE_LABEL_FILL: &[HistoricalProjectionKey] = &[
    projection("node-label.fill", "themeVariables.primaryTextColor"),
    projection("node-label.fill", "themeVariables.nodeTextColor"),
    projection("node-label.fill", "themeVariables.textColor"),
];
const TITLE_FILL: &[HistoricalProjectionKey] =
    &[projection("title.fill", "themeVariables.titleColor")];
const EDGE_WITH_MARKER_FALLBACK: &[HistoricalProjectionKey] = &[
    projection("edge.stroke", "themeVariables.lineColor"),
    projection("marker.paint-from-edge", "themeVariables.arrowheadColor"),
];
const MARKER_PAINT: &[HistoricalProjectionKey] =
    &[projection("marker.paint", "themeVariables.arrowheadColor")];
const EDGE_LABEL_BACKGROUND_FILL: &[HistoricalProjectionKey] = &[projection(
    "edge-label-background.fill",
    "themeVariables.edgeLabelBackground",
)];
const CLUSTER_FILL: &[HistoricalProjectionKey] = &[
    projection("cluster.fill", "themeVariables.clusterBkg"),
    projection("cluster.fill", "themeVariables.secondaryColor"),
];
const CLUSTER_STROKE: &[HistoricalProjectionKey] =
    &[projection("cluster.stroke", "themeVariables.clusterBorder")];
const CLUSTER_LABEL_FILL: &[HistoricalProjectionKey] = &[
    projection("cluster-label.fill", "themeVariables.secondaryTextColor"),
    projection("cluster-label.fill", "themeVariables.tertiaryTextColor"),
];
const TABLE_ODD_FILL: &[HistoricalProjectionKey] = &[
    projection(
        "table.odd.fill",
        "themeVariables.attributeBackgroundColorOdd",
    ),
    projection("table.odd.fill", "themeVariables.rowOdd"),
];
const TABLE_EVEN_FILL: &[HistoricalProjectionKey] = &[
    projection(
        "table.even.fill",
        "themeVariables.attributeBackgroundColorEven",
    ),
    projection("table.even.fill", "themeVariables.rowEven"),
];
const TABLE_ALL_FILL: &[HistoricalProjectionKey] = &[
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
const MINDMAP_TEXT_FILL: &[HistoricalProjectionKey] = &[
    projection("node-label.fill", "themeVariables.primaryTextColor"),
    projection("node-label.fill", "themeVariables.nodeTextColor"),
    projection("node-label.fill", "themeVariables.textColor"),
    projection("title.fill", "themeVariables.titleColor"),
    projection("cluster-label.fill", "themeVariables.secondaryTextColor"),
    projection("cluster-label.fill", "themeVariables.tertiaryTextColor"),
];

const CLASS_RETIREMENTS: &[HistoricalRetirementPattern] = &[
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

const MINDMAP_RETIREMENTS: &[HistoricalRetirementPattern] = &[
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

const TREE_VIEW_RETIREMENTS: &[HistoricalRetirementPattern] = &[
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

const GIT_GRAPH_RETIREMENTS: &[HistoricalRetirementPattern] = &[
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

const V1_RETIREMENT_GROUPS: &[&[HistoricalRetirementPattern]] = &[
    CLASS_RETIREMENTS,
    MINDMAP_RETIREMENTS,
    TREE_VIEW_RETIREMENTS,
    GIT_GRAPH_RETIREMENTS,
];

const ER_RETIREMENTS: &[HistoricalRetirementPattern] = &[pattern(
    DiagramFamilyId::ER,
    ThemeTarget::Title,
    ThemeLegacyRouteFacet::Fill,
    UNQUALIFIED_AND_DEFAULT,
    TITLE_FILL,
)];

const V2_RETIREMENT_GROUPS: &[&[HistoricalRetirementPattern]] = &[ER_RETIREMENTS];

const JOURNEY_RETIREMENTS: &[HistoricalRetirementPattern] = &[pattern(
    DiagramFamilyId::JOURNEY,
    ThemeTarget::Title,
    ThemeLegacyRouteFacet::Fill,
    UNQUALIFIED_AND_DEFAULT,
    TITLE_FILL,
)];

const V3_RETIREMENT_GROUPS: &[&[HistoricalRetirementPattern]] = &[JOURNEY_RETIREMENTS];

const RETIREMENT_BATCHES: &[HistoricalRetirementBatchSpec] = &[
    HistoricalRetirementBatchSpec {
        version: 1,
        baseline_revision: V1_RETIREMENT_BASELINE_REVISION,
        expected_count: V1_EXPECTED_RETIREMENT_COUNT,
        groups: V1_RETIREMENT_GROUPS,
    },
    HistoricalRetirementBatchSpec {
        version: 2,
        baseline_revision: V2_RETIREMENT_BASELINE_REVISION,
        expected_count: V2_EXPECTED_RETIREMENT_COUNT,
        groups: V2_RETIREMENT_GROUPS,
    },
    HistoricalRetirementBatchSpec {
        version: 3,
        baseline_revision: V3_RETIREMENT_BASELINE_REVISION,
        expected_count: V3_EXPECTED_RETIREMENT_COUNT,
        groups: V3_RETIREMENT_GROUPS,
    },
];

const fn projection(
    contribution_suffix: &'static str,
    assignment_path: &'static str,
) -> HistoricalProjectionKey {
    HistoricalProjectionKey {
        contribution_suffix,
        assignment_path,
    }
}

const fn pattern(
    family_id: DiagramFamilyId,
    target: ThemeTarget,
    facet: ThemeLegacyRouteFacet,
    selectors: &'static [ThemeLegacyRouteSelector],
    former_projections: &'static [HistoricalProjectionKey],
) -> HistoricalRetirementPattern {
    HistoricalRetirementPattern {
        family_id,
        target,
        facet,
        selectors,
        former_projections,
    }
}

/// Verifies current renderer receipts against the independent KTD23 historical witness.
pub fn authorize_legacy_projection_retirements()
-> Result<LegacyProjectionRetirementAuthorization, LegacyProjectionVerificationError> {
    let batches = historical_retirement_batches()?;
    let historical = flatten_historical_retirements(&batches)?;
    let production_inventory = retired_legacy_theme_projection_inventory().map_err(|error| {
        LegacyProjectionVerificationError::new(
            "legacy-projection-production-inventory",
            error.to_string(),
        )
    })?;
    verify_production_inventory(&historical, &production_inventory)?;

    let v1_batch = historical_retirement_batch(&batches, 1)?;
    let v2_batch = historical_retirement_batch(&batches, 2)?;
    let v3_batch = historical_retirement_batch(&batches, 3)?;
    let historical_witness_digest = historical_witness_digest(&batches);
    let v1_historical_witness_digest = historical_batch_digest(v1_batch);
    let v2_historical_witness_digest = historical_batch_digest(v2_batch);
    let v3_historical_witness_digest = historical_batch_digest(v3_batch);
    let cumulative_production_inventory_digest = production_inventory_digest(&production_inventory);
    let v1_production_inventory = production_inventory_for_batch(v1_batch, &production_inventory);
    let v1_production_inventory_digest = production_inventory_digest(&v1_production_inventory);
    if v1_historical_witness_digest != EXPECTED_V1_HISTORICAL_WITNESS_DIGEST
        || v2_historical_witness_digest != EXPECTED_V2_HISTORICAL_WITNESS_DIGEST
        || v3_historical_witness_digest != EXPECTED_V3_HISTORICAL_WITNESS_DIGEST
        || historical_witness_digest != EXPECTED_HISTORICAL_WITNESS_DIGEST
        || v1_production_inventory_digest != EXPECTED_V1_PRODUCTION_INVENTORY_DIGEST
        || cumulative_production_inventory_digest != EXPECTED_PRODUCTION_INVENTORY_DIGEST
    {
        return Err(LegacyProjectionVerificationError::new(
            "legacy-projection-authority",
            format!(
                "frozen inventory digest mismatch: v1 historical expected {}, observed {}; v2 historical expected {}, observed {}; v3 historical expected {}, observed {}; cumulative historical expected {}, observed {}; v1 production expected {}, observed {}; cumulative production expected {}, observed {}",
                hex_digest(EXPECTED_V1_HISTORICAL_WITNESS_DIGEST),
                hex_digest(v1_historical_witness_digest),
                hex_digest(EXPECTED_V2_HISTORICAL_WITNESS_DIGEST),
                hex_digest(v2_historical_witness_digest),
                hex_digest(EXPECTED_V3_HISTORICAL_WITNESS_DIGEST),
                hex_digest(v3_historical_witness_digest),
                hex_digest(EXPECTED_HISTORICAL_WITNESS_DIGEST),
                hex_digest(historical_witness_digest),
                hex_digest(EXPECTED_V1_PRODUCTION_INVENTORY_DIGEST),
                hex_digest(v1_production_inventory_digest),
                hex_digest(EXPECTED_PRODUCTION_INVENTORY_DIGEST),
                hex_digest(cumulative_production_inventory_digest),
            ),
        ));
    }

    let receipts = retired_legacy_theme_projection_receipts().map_err(|error| {
        LegacyProjectionVerificationError::new(
            "legacy-projection-production-receipt",
            error.to_string(),
        )
    })?;
    let receipt_map = verify_receipts(&production_inventory, receipts)?;
    let receipt_report_digest = receipt_report_digest(&receipt_map);
    let v1_receipt_report_digest =
        receipt_report_digest_for_inventory(&v1_production_inventory, &receipt_map)?;
    if v1_receipt_report_digest != EXPECTED_V1_RECEIPT_REPORT_DIGEST
        || receipt_report_digest != EXPECTED_RECEIPT_REPORT_DIGEST
    {
        return Err(LegacyProjectionVerificationError::new(
            "legacy-projection-authority",
            format!(
                "frozen receipt report digest mismatch: v1 expected {}, observed {}; cumulative expected {}, observed {}",
                hex_digest(EXPECTED_V1_RECEIPT_REPORT_DIGEST),
                hex_digest(v1_receipt_report_digest),
                hex_digest(EXPECTED_RECEIPT_REPORT_DIGEST),
                hex_digest(receipt_report_digest),
            ),
        ));
    }

    Ok(LegacyProjectionRetirementAuthorization {
        manifest_version: RETIREMENT_MANIFEST_VERSION,
        manifest_digest: manifest_digest(
            &batches,
            historical_witness_digest,
            cumulative_production_inventory_digest,
            receipt_report_digest,
        ),
        historical_witness_digest,
        production_inventory_digest: cumulative_production_inventory_digest,
        receipt_report_digest,
        retirement_count: historical.len(),
        value_probe_count: EXPECTED_VALUE_PROBE_COUNT,
    })
}

fn historical_retirement_batches()
-> Result<Vec<HistoricalRetirementBatch>, LegacyProjectionVerificationError> {
    let mut batches = Vec::with_capacity(RETIREMENT_BATCHES.len());
    for spec in RETIREMENT_BATCHES {
        let mut retirements = Vec::with_capacity(spec.expected_count);
        for pattern in spec.groups.iter().flat_map(|patterns| patterns.iter()) {
            if pattern.former_projections.is_empty() {
                return Err(LegacyProjectionVerificationError::new(
                    "legacy-projection-historical-witness",
                    format!(
                        "batch {} contains a route without a former projection set",
                        spec.version
                    ),
                ));
            }
            for selector in pattern.selectors {
                retirements.push(HistoricalRetirement {
                    family_id: pattern.family_id,
                    target: pattern.target,
                    selector: *selector,
                    facet: pattern.facet,
                    former_projections: pattern.former_projections,
                });
            }
        }
        retirements.sort_unstable();
        if retirements.len() != spec.expected_count {
            return Err(LegacyProjectionVerificationError::new(
                "legacy-projection-historical-witness",
                format!(
                    "batch {} expected {} routes, observed {}",
                    spec.version,
                    spec.expected_count,
                    retirements.len()
                ),
            ));
        }
        if retirements
            .windows(2)
            .any(|pair| same_route(pair[0], pair[1]))
        {
            return Err(LegacyProjectionVerificationError::new(
                "legacy-projection-historical-witness",
                format!("batch {} contains a duplicate route identity", spec.version),
            ));
        }
        batches.push(HistoricalRetirementBatch {
            version: spec.version,
            baseline_revision: spec.baseline_revision,
            retirements,
        });
    }
    if batches.len() != usize::from(RETIREMENT_MANIFEST_VERSION)
        || batches
            .iter()
            .enumerate()
            .any(|(index, batch)| u16::try_from(index + 1).ok() != Some(batch.version))
    {
        return Err(LegacyProjectionVerificationError::new(
            "legacy-projection-historical-witness",
            format!(
                "retirement batches must cover every version from 1 through {RETIREMENT_MANIFEST_VERSION}",
            ),
        ));
    }
    Ok(batches)
}

fn historical_retirement_batch(
    batches: &[HistoricalRetirementBatch],
    version: u16,
) -> Result<&HistoricalRetirementBatch, LegacyProjectionVerificationError> {
    batches
        .iter()
        .find(|batch| batch.version == version)
        .ok_or_else(|| {
            LegacyProjectionVerificationError::new(
                "legacy-projection-historical-witness",
                format!("retirement batch version {version} is missing"),
            )
        })
}

fn flatten_historical_retirements(
    batches: &[HistoricalRetirementBatch],
) -> Result<Vec<HistoricalRetirement>, LegacyProjectionVerificationError> {
    let mut retirements = batches
        .iter()
        .flat_map(|batch| batch.retirements.iter().copied())
        .collect::<Vec<_>>();
    retirements.sort_unstable();
    if retirements.len() != EXPECTED_RETIREMENT_COUNT {
        return Err(LegacyProjectionVerificationError::new(
            "legacy-projection-historical-witness",
            format!(
                "expected {EXPECTED_RETIREMENT_COUNT} routes, observed {}",
                retirements.len()
            ),
        ));
    }
    if retirements
        .windows(2)
        .any(|pair| same_route(pair[0], pair[1]))
    {
        return Err(LegacyProjectionVerificationError::new(
            "legacy-projection-historical-witness",
            "retirement batches contain a duplicate route identity",
        ));
    }
    Ok(retirements)
}

#[cfg(test)]
fn historical_retirements() -> Result<Vec<HistoricalRetirement>, LegacyProjectionVerificationError>
{
    let batches = historical_retirement_batches()?;
    flatten_historical_retirements(&batches)
}

fn production_inventory_for_batch(
    batch: &HistoricalRetirementBatch,
    production: &[ThemeLegacyProjectionRetirementDescriptor],
) -> Vec<ThemeLegacyProjectionRetirementDescriptor> {
    let route_ids = batch
        .retirements
        .iter()
        .map(|retirement| {
            ThemeLegacyRouteId::new(
                retirement.family_id,
                retirement.target,
                retirement.selector,
                retirement.facet,
            )
        })
        .collect::<BTreeSet<_>>();
    production
        .iter()
        .copied()
        .filter(|descriptor| route_ids.contains(&descriptor.id()))
        .collect()
}

fn receipt_report_digest_for_inventory(
    inventory: &[ThemeLegacyProjectionRetirementDescriptor],
    receipts: &BTreeMap<ThemeLegacyProjectionRetirementDescriptor, [u8; 32]>,
) -> Result<[u8; 32], LegacyProjectionVerificationError> {
    let mut selected = BTreeMap::new();
    for descriptor in inventory {
        let Some(digest) = receipts.get(descriptor) else {
            return Err(LegacyProjectionVerificationError::new(
                "legacy-projection-production-receipt",
                format!(
                    "batch receipt is missing from the cumulative report: {}",
                    descriptor_label(*descriptor)
                ),
            ));
        };
        selected.insert(*descriptor, *digest);
    }
    Ok(receipt_report_digest(&selected))
}

fn verify_production_inventory(
    historical: &[HistoricalRetirement],
    production: &[ThemeLegacyProjectionRetirementDescriptor],
) -> Result<(), LegacyProjectionVerificationError> {
    if production.len() != historical.len() {
        return Err(LegacyProjectionVerificationError::new(
            "legacy-projection-production-inventory",
            format!(
                "expected {} production routes, observed {}",
                historical.len(),
                production.len()
            ),
        ));
    }
    for (historical, production) in historical.iter().zip(production) {
        let id = production.id();
        if id.family_id() != historical.family_id
            || id.target() != historical.target
            || id.selector() != historical.selector
            || id.facet() != historical.facet
        {
            return Err(LegacyProjectionVerificationError::new(
                "legacy-projection-production-inventory",
                format!(
                    "route identity drift: expected {}, observed {}",
                    historical_label(*historical),
                    descriptor_label(*production),
                ),
            ));
        }

        let expected = historical
            .former_projections
            .iter()
            .map(|projection| (projection.contribution_suffix, projection.assignment_path))
            .collect::<BTreeSet<_>>();
        let observed = production
            .former_projections()
            .iter()
            .map(|projection| {
                (
                    projection.contribution_suffix(),
                    projection.assignment_path(),
                )
            })
            .collect::<BTreeSet<_>>();
        if expected.len() != historical.former_projections.len()
            || observed.len() != production.former_projections().len()
            || expected != observed
        {
            return Err(LegacyProjectionVerificationError::new(
                "legacy-projection-production-inventory",
                format!(
                    "former projection set drift for {}: expected {expected:?}, observed {observed:?}",
                    historical_label(*historical),
                ),
            ));
        }
    }
    Ok(())
}

fn verify_receipts(
    production_inventory: &[ThemeLegacyProjectionRetirementDescriptor],
    receipts: Vec<ThemeLegacyProjectionRetirementReceipt>,
) -> Result<
    BTreeMap<ThemeLegacyProjectionRetirementDescriptor, [u8; 32]>,
    LegacyProjectionVerificationError,
> {
    let inventory = production_inventory
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut by_descriptor = BTreeMap::new();
    for receipt in receipts {
        if receipt.digest() == [0; 32] {
            return Err(LegacyProjectionVerificationError::new(
                "legacy-projection-production-receipt",
                format!(
                    "zero production receipt digest for {}",
                    descriptor_label(receipt.descriptor())
                ),
            ));
        }
        if !inventory.contains(&receipt.descriptor()) {
            return Err(LegacyProjectionVerificationError::new(
                "legacy-projection-production-receipt",
                format!(
                    "receipt is outside the production inventory: {}",
                    descriptor_label(receipt.descriptor())
                ),
            ));
        }
        if by_descriptor
            .insert(receipt.descriptor(), receipt.digest())
            .is_some()
        {
            return Err(LegacyProjectionVerificationError::new(
                "legacy-projection-production-receipt",
                format!(
                    "duplicate receipt for {}",
                    descriptor_label(receipt.descriptor())
                ),
            ));
        }
    }
    if by_descriptor.len() != inventory.len() {
        return Err(LegacyProjectionVerificationError::new(
            "legacy-projection-production-receipt",
            format!(
                "expected {} receipts, observed {}",
                inventory.len(),
                by_descriptor.len()
            ),
        ));
    }
    Ok(by_descriptor)
}

fn historical_batch_digest(batch: &HistoricalRetirementBatch) -> [u8; 32] {
    let mut hasher = Sha256::new();
    update_len_prefixed(
        &mut hasher,
        b"merman.theme-legacy-projection-historical-witness.v1",
    );
    hasher.update(batch.version.to_be_bytes());
    update_len_prefixed(&mut hasher, batch.baseline_revision.as_bytes());
    hasher.update(usize_to_u64(batch.retirements.len()).to_be_bytes());
    for retirement in &batch.retirements {
        append_historical_retirement(&mut hasher, *retirement);
    }
    hasher.finalize().into()
}

fn historical_witness_digest(batches: &[HistoricalRetirementBatch]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    update_len_prefixed(
        &mut hasher,
        b"merman.theme-legacy-projection-historical-witness.v2",
    );
    hasher.update(RETIREMENT_MANIFEST_VERSION.to_be_bytes());
    hasher.update(usize_to_u64(batches.len()).to_be_bytes());
    for batch in batches {
        hasher.update(batch.version.to_be_bytes());
        update_len_prefixed(&mut hasher, batch.baseline_revision.as_bytes());
        hasher.update(usize_to_u64(batch.retirements.len()).to_be_bytes());
        hasher.update(historical_batch_digest(batch));
    }
    hasher.finalize().into()
}

fn production_inventory_digest(
    descriptors: &[ThemeLegacyProjectionRetirementDescriptor],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    update_len_prefixed(
        &mut hasher,
        b"merman.theme-legacy-projection-production-inventory.v1",
    );
    hasher.update(usize_to_u64(descriptors.len()).to_be_bytes());
    for descriptor in descriptors {
        append_descriptor(&mut hasher, *descriptor);
    }
    hasher.finalize().into()
}

fn receipt_report_digest(
    receipts: &BTreeMap<ThemeLegacyProjectionRetirementDescriptor, [u8; 32]>,
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    update_len_prefixed(
        &mut hasher,
        b"merman.theme-legacy-projection-retirement-report.v1",
    );
    hasher.update(usize_to_u64(receipts.len()).to_be_bytes());
    for (descriptor, digest) in receipts {
        append_descriptor(&mut hasher, *descriptor);
        hasher.update(digest);
    }
    hasher.finalize().into()
}

fn manifest_digest(
    batches: &[HistoricalRetirementBatch],
    historical_witness_digest: [u8; 32],
    production_inventory_digest: [u8; 32],
    receipt_report_digest: [u8; 32],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    update_len_prefixed(
        &mut hasher,
        b"merman.theme-legacy-projection-retirement-manifest.v2",
    );
    hasher.update(RETIREMENT_MANIFEST_VERSION.to_be_bytes());
    hasher.update(usize_to_u64(batches.len()).to_be_bytes());
    for batch in batches {
        hasher.update(batch.version.to_be_bytes());
        update_len_prefixed(&mut hasher, batch.baseline_revision.as_bytes());
        hasher.update(usize_to_u64(batch.retirements.len()).to_be_bytes());
        hasher.update(historical_batch_digest(batch));
    }
    hasher.update(usize_to_u64(EXPECTED_RETIREMENT_COUNT).to_be_bytes());
    hasher.update(usize_to_u64(EXPECTED_VALUE_PROBE_COUNT).to_be_bytes());
    hasher.update(historical_witness_digest);
    hasher.update(production_inventory_digest);
    hasher.update(receipt_report_digest);
    hasher.finalize().into()
}

fn append_historical_retirement(hasher: &mut Sha256, retirement: HistoricalRetirement) {
    append_route(
        hasher,
        retirement.family_id,
        retirement.target,
        retirement.selector,
        retirement.facet,
    );
    append_historical_projection_set(hasher, retirement.former_projections);
}

fn append_descriptor(hasher: &mut Sha256, descriptor: ThemeLegacyProjectionRetirementDescriptor) {
    let id = descriptor.id();
    append_route(
        hasher,
        id.family_id(),
        id.target(),
        id.selector(),
        id.facet(),
    );
    let mut projections = descriptor.former_projections().to_vec();
    projections.sort_unstable();
    hasher.update(usize_to_u64(projections.len()).to_be_bytes());
    for projection in projections {
        update_len_prefixed(hasher, projection.contribution_suffix().as_bytes());
        update_len_prefixed(hasher, projection.assignment_path().as_bytes());
    }
}

fn append_historical_projection_set(hasher: &mut Sha256, projections: &[HistoricalProjectionKey]) {
    let mut projections = projections.to_vec();
    projections.sort_unstable();
    hasher.update(usize_to_u64(projections.len()).to_be_bytes());
    for projection in projections {
        update_len_prefixed(hasher, projection.contribution_suffix.as_bytes());
        update_len_prefixed(hasher, projection.assignment_path.as_bytes());
    }
}

fn append_route(
    hasher: &mut Sha256,
    family_id: DiagramFamilyId,
    target: ThemeTarget,
    selector: ThemeLegacyRouteSelector,
    facet: ThemeLegacyRouteFacet,
) {
    update_len_prefixed(hasher, family_id.as_str().as_bytes());
    update_len_prefixed(hasher, target.id().as_bytes());
    update_len_prefixed(hasher, selector.id().as_bytes());
    if let Some(variant) = selector.variant() {
        update_len_prefixed(hasher, variant.id().as_bytes());
    }
    update_len_prefixed(hasher, facet.id().as_bytes());
}

fn same_route(left: HistoricalRetirement, right: HistoricalRetirement) -> bool {
    left.family_id == right.family_id
        && left.target == right.target
        && left.selector == right.selector
        && left.facet == right.facet
}

fn historical_label(retirement: HistoricalRetirement) -> String {
    format!(
        "{}/{}/{}/{}",
        retirement.family_id.as_str(),
        retirement.target.id(),
        retirement.selector.id(),
        retirement.facet.id(),
    )
}

fn descriptor_label(descriptor: ThemeLegacyProjectionRetirementDescriptor) -> String {
    let id = descriptor.id();
    format!(
        "{}/{}/{}/{}",
        id.family_id().as_str(),
        id.target().id(),
        id.selector().id(),
        id.facet().id(),
    )
}

fn update_len_prefixed(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update(usize_to_u64(bytes.len()).to_be_bytes());
    hasher.update(bytes);
}

fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).expect("acceptance manifest sizes fit in u64")
}

fn hex_digest(digest: [u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_render::__private::ThemeLegacyProjectionKey;

    #[test]
    fn independent_historical_witness_has_exact_ktd23_boundary() {
        let batches = historical_retirement_batches().expect("valid versioned KTD23 batches");
        let v1 = historical_retirement_batch(&batches, 1).expect("v1 batch exists");
        let v2 = historical_retirement_batch(&batches, 2).expect("v2 batch exists");

        assert_eq!(v1.retirements.len(), V1_EXPECTED_RETIREMENT_COUNT);
        assert_eq!(v2.retirements.len(), V2_EXPECTED_RETIREMENT_COUNT);
        assert!(v2.retirements.iter().all(|retirement| {
            retirement.family_id == DiagramFamilyId::ER
                && retirement.target == ThemeTarget::Title
                && retirement.facet == ThemeLegacyRouteFacet::Fill
        }));

        let v3 = historical_retirement_batch(&batches, 3).expect("v3 batch exists");
        assert_eq!(v3.baseline_revision, V3_RETIREMENT_BASELINE_REVISION);
        assert_eq!(v3.retirements.len(), V3_EXPECTED_RETIREMENT_COUNT);
        assert!(v3.retirements.iter().all(|retirement| {
            retirement.family_id == DiagramFamilyId::JOURNEY
                && retirement.target == ThemeTarget::Title
                && retirement.facet == ThemeLegacyRouteFacet::Fill
                && retirement.former_projections == TITLE_FILL
        }));
        assert_eq!(
            v3.retirements
                .iter()
                .map(|retirement| retirement.selector)
                .collect::<Vec<_>>(),
            UNQUALIFIED_AND_DEFAULT,
        );

        let retirements = historical_retirements().expect("valid independent KTD23 witness");
        assert_eq!(retirements.len(), EXPECTED_RETIREMENT_COUNT);
        assert!(retirements.iter().all(|retirement| {
            retirement.family_id != DiagramFamilyId::CLASS
                || retirement.target != ThemeTarget::Title
        }));
    }

    #[test]
    fn exact_production_inventory_and_receipts_are_authorized() {
        let authorization = authorize_legacy_projection_retirements()
            .expect("authorize production inventory and receipts against independent history");
        assert_eq!(
            authorization.manifest_version(),
            RETIREMENT_MANIFEST_VERSION
        );
        assert_eq!(authorization.retirement_count(), EXPECTED_RETIREMENT_COUNT);
        assert_eq!(
            authorization.value_probe_count(),
            EXPECTED_VALUE_PROBE_COUNT
        );
        assert_ne!(authorization.manifest_digest(), &[0; 32]);
    }

    #[test]
    fn production_inventory_drift_fails_closed() {
        let historical = historical_retirements().expect("valid independent KTD23 witness");
        let inventory =
            retired_legacy_theme_projection_inventory().expect("production KTD23 inventory");

        let mut missing = inventory.clone();
        missing.pop();
        assert!(verify_production_inventory(&historical, &missing).is_err());

        let mut reordered = inventory.clone();
        reordered.swap(0, 1);
        assert!(verify_production_inventory(&historical, &reordered).is_err());

        const DRIFTED_PROJECTION: &[ThemeLegacyProjectionKey] = &[ThemeLegacyProjectionKey::new(
            "drifted.projection",
            "themeVariables.__drifted",
        )];
        let mut projection_drift = inventory;
        projection_drift[0] = ThemeLegacyProjectionRetirementDescriptor::new(
            projection_drift[0].id(),
            DRIFTED_PROJECTION,
        );
        assert!(verify_production_inventory(&historical, &projection_drift).is_err());
    }

    #[test]
    fn missing_or_duplicate_production_receipt_fails_closed() {
        let inventory =
            retired_legacy_theme_projection_inventory().expect("production KTD23 inventory");
        let receipts =
            retired_legacy_theme_projection_receipts().expect("production KTD23 receipts");
        let mut missing = receipts.clone();
        missing.pop();
        assert!(verify_receipts(&inventory, missing).is_err());

        let mut duplicate = receipts;
        duplicate.push(duplicate[0]);
        assert!(verify_receipts(&inventory, duplicate).is_err());
    }
}
