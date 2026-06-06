//! Diagram admission inventory for alignment and compare tooling.

use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum AdmissionStatus {
    PrimarySvgMatrix,
    CompatibilityOnly,
    ParseOnly,
    NotAdmitted,
    NotInPinnedBaseline,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum CoverageStatus {
    Covered,
    Deferred,
    NotApplicable,
    NotAdmitted,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FixtureCorpusStatus {
    Normalized,
    NormalizedWithDeferred,
    None,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct DiagramAdmissionRecord {
    pub(crate) diagram: &'static str,
    pub(crate) admission: AdmissionStatus,
    pub(crate) fixtures: FixtureCorpusStatus,
    pub(crate) normalized_fixture_dir: Option<&'static str>,
    pub(crate) deferred_fixture_dir: Option<&'static str>,
    pub(crate) semantic: CoverageStatus,
    pub(crate) layout: CoverageStatus,
    pub(crate) svg: CoverageStatus,
    pub(crate) root_viewport: CoverageStatus,
    pub(crate) compare_command: Option<&'static str>,
    pub(crate) owner_doc: &'static str,
    pub(crate) defer_reason: Option<&'static str>,
}

pub(crate) fn admission_inventory() -> &'static [DiagramAdmissionRecord] {
    ADMISSION_INVENTORY
}

pub(crate) fn primary_svg_matrix_diagrams() -> impl Iterator<Item = &'static str> {
    ADMISSION_INVENTORY
        .iter()
        .filter(|record| record.admission == AdmissionStatus::PrimarySvgMatrix)
        .map(|record| record.diagram)
}

pub(crate) fn root_viewport_deferred_diagrams() -> impl Iterator<Item = &'static str> {
    ADMISSION_INVENTORY
        .iter()
        .filter(|record| {
            record.admission == AdmissionStatus::PrimarySvgMatrix
                && record.root_viewport == CoverageStatus::Deferred
        })
        .map(|record| record.diagram)
}

pub(crate) fn admission_inventory_alignment_failures(fixtures_root: &Path) -> Vec<String> {
    let workspace_root = crate::cmd::workspace_root();
    let mut failures = Vec::new();

    for record in admission_inventory() {
        match record.fixtures {
            FixtureCorpusStatus::Normalized => {
                if record.normalized_fixture_dir.is_none() || record.deferred_fixture_dir.is_some()
                {
                    failures.push(format!(
                        "admission inventory: `{}` fixture status is Normalized but dirs are inconsistent",
                        record.diagram
                    ));
                }
            }
            FixtureCorpusStatus::NormalizedWithDeferred => {
                if record.normalized_fixture_dir.is_none() || record.deferred_fixture_dir.is_none()
                {
                    failures.push(format!(
                        "admission inventory: `{}` fixture status is NormalizedWithDeferred but dirs are incomplete",
                        record.diagram
                    ));
                }
            }
            FixtureCorpusStatus::None => {
                if record.normalized_fixture_dir.is_some() || record.deferred_fixture_dir.is_some()
                {
                    failures.push(format!(
                        "admission inventory: `{}` fixture status is None but fixture dirs are set",
                        record.diagram
                    ));
                }
            }
        }

        if record.admission == AdmissionStatus::PrimarySvgMatrix && record.compare_command.is_none()
        {
            failures.push(format!(
                "admission inventory: primary SVG diagram `{}` has no compare command",
                record.diagram
            ));
        }

        if matches!(
            record.admission,
            AdmissionStatus::NotAdmitted | AdmissionStatus::NotInPinnedBaseline
        ) && record.defer_reason.is_none()
        {
            failures.push(format!(
                "admission inventory: non-admitted diagram `{}` needs a defer reason",
                record.diagram
            ));
        }

        let owner = workspace_root.join(record.owner_doc);
        if !owner.exists() {
            failures.push(format!(
                "admission inventory: owner doc for `{}` does not exist: {}",
                record.diagram,
                owner.display()
            ));
        }

        if let Some(dir) = record.normalized_fixture_dir {
            let path = fixtures_root.join(dir);
            if !path.is_dir() {
                failures.push(format!(
                    "admission inventory: normalized fixture dir for `{}` does not exist: {}",
                    record.diagram,
                    path.display()
                ));
            } else {
                if record.semantic == CoverageStatus::Covered
                    && count_files_with_suffix(&path, ".golden.json") == 0
                {
                    failures.push(format!(
                        "admission inventory: `{}` is marked semantic-covered but has no golden JSON under {}",
                        record.diagram,
                        path.display()
                    ));
                }
                if record.layout == CoverageStatus::Covered
                    && count_files_with_suffix(&path, ".layout.golden.json") == 0
                {
                    failures.push(format!(
                        "admission inventory: `{}` is marked layout-covered but has no layout golden under {}",
                        record.diagram,
                        path.display()
                    ));
                }
            }
        }

        // `fixtures/_deferred` is intentionally ignored and used as a local investigation corpus.
        // Keep `NormalizedWithDeferred` as inventory metadata, but do not make the release
        // alignment gate depend on those local directories existing in every checkout.

        if record.svg == CoverageStatus::Covered {
            let upstream_dir = fixtures_root.join("upstream-svgs").join(record.diagram);
            if !upstream_dir.is_dir() {
                failures.push(format!(
                    "admission inventory: `{}` is marked SVG-covered but has no upstream SVG dir: {}",
                    record.diagram,
                    upstream_dir.display()
                ));
            }
        }
    }

    failures
}

fn count_files_with_suffix(dir: &Path, suffix: &str) -> usize {
    fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| {
                    entry
                        .file_name()
                        .to_str()
                        .is_some_and(|name| name.ends_with(suffix))
                })
                .count()
        })
        .unwrap_or(0)
}

macro_rules! primary {
    ($diagram:literal, $fixtures:expr, $compare:literal, $owner:literal) => {
        DiagramAdmissionRecord {
            diagram: $diagram,
            admission: AdmissionStatus::PrimarySvgMatrix,
            fixtures: $fixtures,
            normalized_fixture_dir: Some($diagram),
            deferred_fixture_dir: match $fixtures {
                FixtureCorpusStatus::NormalizedWithDeferred => Some($diagram),
                FixtureCorpusStatus::Normalized | FixtureCorpusStatus::None => None,
            },
            semantic: CoverageStatus::Covered,
            layout: CoverageStatus::Covered,
            svg: CoverageStatus::Covered,
            root_viewport: CoverageStatus::Covered,
            compare_command: Some($compare),
            owner_doc: $owner,
            defer_reason: None,
        }
    };
}

macro_rules! primary_root_deferred {
    ($diagram:literal, $fixtures:expr, $compare:literal, $owner:literal, $reason:literal) => {
        DiagramAdmissionRecord {
            root_viewport: CoverageStatus::Deferred,
            defer_reason: Some($reason),
            ..primary!($diagram, $fixtures, $compare, $owner)
        }
    };
}

const ADMISSION_INVENTORY: &[DiagramAdmissionRecord] = &[
    primary!(
        "er",
        FixtureCorpusStatus::NormalizedWithDeferred,
        "compare-er-svgs",
        "docs/alignment/ER_MINIMUM.md"
    ),
    primary!(
        "flowchart",
        FixtureCorpusStatus::NormalizedWithDeferred,
        "compare-flowchart-svgs",
        "docs/alignment/FLOWCHART_MINIMUM.md"
    ),
    primary!(
        "state",
        FixtureCorpusStatus::NormalizedWithDeferred,
        "compare-state-svgs",
        "docs/alignment/STATE_MINIMUM.md"
    ),
    primary!(
        "class",
        FixtureCorpusStatus::NormalizedWithDeferred,
        "compare-class-svgs",
        "docs/alignment/CLASS_MINIMUM.md"
    ),
    primary!(
        "sequence",
        FixtureCorpusStatus::NormalizedWithDeferred,
        "compare-sequence-svgs",
        "docs/alignment/SEQUENCE_MINIMUM.md"
    ),
    primary!(
        "info",
        FixtureCorpusStatus::NormalizedWithDeferred,
        "compare-info-svgs",
        "docs/alignment/INFO_MINIMUM.md"
    ),
    primary!(
        "pie",
        FixtureCorpusStatus::NormalizedWithDeferred,
        "compare-pie-svgs",
        "docs/alignment/PIE_MINIMUM.md"
    ),
    primary!(
        "sankey",
        FixtureCorpusStatus::NormalizedWithDeferred,
        "compare-sankey-svgs",
        "docs/alignment/SANKEY_MINIMUM.md"
    ),
    primary!(
        "packet",
        FixtureCorpusStatus::NormalizedWithDeferred,
        "compare-packet-svgs",
        "docs/alignment/PACKET_MINIMUM.md"
    ),
    primary!(
        "timeline",
        FixtureCorpusStatus::Normalized,
        "compare-timeline-svgs",
        "docs/alignment/TIMELINE_MINIMUM.md"
    ),
    primary!(
        "journey",
        FixtureCorpusStatus::Normalized,
        "compare-journey-svgs",
        "docs/alignment/JOURNEY_MINIMUM.md"
    ),
    primary!(
        "kanban",
        FixtureCorpusStatus::Normalized,
        "compare-kanban-svgs",
        "docs/alignment/KANBAN_MINIMUM.md"
    ),
    primary!(
        "gitgraph",
        FixtureCorpusStatus::NormalizedWithDeferred,
        "compare-gitgraph-svgs",
        "docs/alignment/GITGRAPH_MINIMUM.md"
    ),
    primary!(
        "gantt",
        FixtureCorpusStatus::Normalized,
        "compare-gantt-svgs",
        "docs/alignment/GANTT_MINIMUM.md"
    ),
    primary!(
        "c4",
        FixtureCorpusStatus::NormalizedWithDeferred,
        "compare-c4-svgs",
        "docs/alignment/C4_MINIMUM.md"
    ),
    primary!(
        "block",
        FixtureCorpusStatus::NormalizedWithDeferred,
        "compare-block-svgs",
        "docs/alignment/BLOCK_MINIMUM.md"
    ),
    primary!(
        "radar",
        FixtureCorpusStatus::NormalizedWithDeferred,
        "compare-radar-svgs",
        "docs/alignment/RADAR_MINIMUM.md"
    ),
    primary!(
        "requirement",
        FixtureCorpusStatus::NormalizedWithDeferred,
        "compare-requirement-svgs",
        "docs/alignment/REQUIREMENT_MINIMUM.md"
    ),
    primary!(
        "mindmap",
        FixtureCorpusStatus::Normalized,
        "compare-mindmap-svgs",
        "docs/alignment/MINDMAP_MINIMUM.md"
    ),
    primary!(
        "architecture",
        FixtureCorpusStatus::NormalizedWithDeferred,
        "compare-architecture-svgs",
        "docs/alignment/ARCHITECTURE_MINIMUM.md"
    ),
    primary!(
        "quadrantchart",
        FixtureCorpusStatus::Normalized,
        "compare-quadrantchart-svgs",
        "docs/alignment/QUADRANTCHART_MINIMUM.md"
    ),
    primary!(
        "treemap",
        FixtureCorpusStatus::NormalizedWithDeferred,
        "compare-treemap-svgs",
        "docs/alignment/TREEMAP_MINIMUM.md"
    ),
    primary!(
        "xychart",
        FixtureCorpusStatus::NormalizedWithDeferred,
        "compare-xychart-svgs",
        "docs/alignment/XYCHART_MINIMUM.md"
    ),
    primary_root_deferred!(
        "treeView",
        FixtureCorpusStatus::Normalized,
        "compare-tree-view-svgs",
        "docs/alignment/TREEVIEW_MINIMUM.md",
        "global parity-root sweep skips current browser text-metric root viewport residuals"
    ),
    primary_root_deferred!(
        "ishikawa",
        FixtureCorpusStatus::NormalizedWithDeferred,
        "compare-ishikawa-svgs",
        "docs/alignment/ISHIKAWA_MINIMUM.md",
        "global parity-root sweep skips current label/fish-head root viewport residuals"
    ),
    primary_root_deferred!(
        "eventmodeling",
        FixtureCorpusStatus::Normalized,
        "compare-eventmodeling-svgs",
        "docs/alignment/EVENTMODELING_MINIMUM.md",
        "global parity-root sweep skips current foreignObject/browser text-metric residuals"
    ),
    DiagramAdmissionRecord {
        diagram: "zenuml",
        admission: AdmissionStatus::CompatibilityOnly,
        fixtures: FixtureCorpusStatus::Normalized,
        normalized_fixture_dir: Some("zenuml"),
        deferred_fixture_dir: None,
        semantic: CoverageStatus::Covered,
        layout: CoverageStatus::Covered,
        svg: CoverageStatus::Deferred,
        root_viewport: CoverageStatus::NotApplicable,
        compare_command: None,
        owner_doc: "docs/alignment/ZENUML_MINIMUM.md",
        defer_reason: Some("upstream ZenUML renders through browser-only @zenuml/core"),
    },
    DiagramAdmissionRecord {
        diagram: "error",
        admission: AdmissionStatus::ParseOnly,
        fixtures: FixtureCorpusStatus::Normalized,
        normalized_fixture_dir: Some("error"),
        deferred_fixture_dir: None,
        semantic: CoverageStatus::Covered,
        layout: CoverageStatus::NotApplicable,
        svg: CoverageStatus::Deferred,
        root_viewport: CoverageStatus::NotApplicable,
        compare_command: None,
        owner_doc: "docs/alignment/ERROR_MINIMUM.md",
        defer_reason: Some("tracked as parse/snapshot-only; no upstream SVG baseline corpus"),
    },
    DiagramAdmissionRecord {
        diagram: "venn",
        admission: AdmissionStatus::NotAdmitted,
        fixtures: FixtureCorpusStatus::None,
        normalized_fixture_dir: None,
        deferred_fixture_dir: None,
        semantic: CoverageStatus::NotAdmitted,
        layout: CoverageStatus::NotAdmitted,
        svg: CoverageStatus::NotAdmitted,
        root_viewport: CoverageStatus::NotApplicable,
        compare_command: None,
        owner_doc: "docs/alignment/UNSUPPORTED_FAMILY_ADMISSION_RUBRIC.md",
        defer_reason: Some("needs source-backed @upsetjs/venn.js layout parity plan"),
    },
    DiagramAdmissionRecord {
        diagram: "wardley",
        admission: AdmissionStatus::NotAdmitted,
        fixtures: FixtureCorpusStatus::None,
        normalized_fixture_dir: None,
        deferred_fixture_dir: None,
        semantic: CoverageStatus::NotAdmitted,
        layout: CoverageStatus::NotAdmitted,
        svg: CoverageStatus::NotAdmitted,
        root_viewport: CoverageStatus::NotApplicable,
        compare_command: None,
        owner_doc: "docs/alignment/UNSUPPORTED_FAMILY_ADMISSION_RUBRIC.md",
        defer_reason: Some("large family lane deferred behind smaller source-backed work"),
    },
    DiagramAdmissionRecord {
        diagram: "railroad",
        admission: AdmissionStatus::NotInPinnedBaseline,
        fixtures: FixtureCorpusStatus::None,
        normalized_fixture_dir: None,
        deferred_fixture_dir: None,
        semantic: CoverageStatus::NotApplicable,
        layout: CoverageStatus::NotApplicable,
        svg: CoverageStatus::NotApplicable,
        root_viewport: CoverageStatus::NotApplicable,
        compare_command: None,
        owner_doc: "docs/alignment/UNSUPPORTED_FAMILY_ADMISSION_RUBRIC.md",
        defer_reason: Some("absent from pinned Mermaid 11.15 source"),
    },
    DiagramAdmissionRecord {
        diagram: "cynefin",
        admission: AdmissionStatus::NotInPinnedBaseline,
        fixtures: FixtureCorpusStatus::None,
        normalized_fixture_dir: None,
        deferred_fixture_dir: None,
        semantic: CoverageStatus::NotApplicable,
        layout: CoverageStatus::NotApplicable,
        svg: CoverageStatus::NotApplicable,
        root_viewport: CoverageStatus::NotApplicable,
        compare_command: None,
        owner_doc: "docs/alignment/UNSUPPORTED_FAMILY_ADMISSION_RUBRIC.md",
        defer_reason: Some("absent from pinned Mermaid 11.15 source"),
    },
];
