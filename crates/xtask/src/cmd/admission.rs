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
    diagram: &'static str,
    admission: AdmissionStatus,
    fixtures: FixtureCorpusStatus,
    normalized_fixture_dir: Option<&'static str>,
    deferred_fixture_dir: Option<&'static str>,
    semantic: CoverageStatus,
    layout: CoverageStatus,
    svg: CoverageStatus,
    root_viewport: CoverageStatus,
    compare_command: Option<&'static str>,
    owner_doc: &'static str,
    defer_reason: Option<&'static str>,
}

impl CoverageStatus {
    fn requires_fixture_evidence(self) -> bool {
        self == Self::Covered
    }
}

impl FixtureCorpusStatus {
    fn expects_normalized_dir(self) -> bool {
        matches!(self, Self::Normalized | Self::NormalizedWithDeferred)
    }

    fn expects_deferred_dir(self) -> bool {
        matches!(self, Self::NormalizedWithDeferred)
    }
}

impl DiagramAdmissionRecord {
    fn is_primary_svg_matrix(self) -> bool {
        self.admission == AdmissionStatus::PrimarySvgMatrix
    }

    fn is_root_viewport_deferred(self) -> bool {
        self.is_primary_svg_matrix() && self.root_viewport == CoverageStatus::Deferred
    }

    fn requires_compare_command(self) -> bool {
        self.is_primary_svg_matrix()
    }

    fn requires_defer_reason(self) -> bool {
        matches!(
            self.admission,
            AdmissionStatus::NotAdmitted | AdmissionStatus::NotInPinnedBaseline
        ) || self.is_root_viewport_deferred()
    }

    fn has_consistent_fixture_dirs(self) -> bool {
        self.normalized_fixture_dir.is_some() == self.fixtures.expects_normalized_dir()
            && self.deferred_fixture_dir.is_some() == self.fixtures.expects_deferred_dir()
    }

    fn semantic_requires_golden(self) -> bool {
        self.semantic.requires_fixture_evidence()
    }

    fn layout_requires_golden(self) -> bool {
        self.layout.requires_fixture_evidence()
    }

    fn svg_requires_upstream_baseline(self) -> bool {
        self.svg.requires_fixture_evidence()
    }
}

pub(crate) fn admission_inventory() -> &'static [DiagramAdmissionRecord] {
    ADMISSION_INVENTORY
}

pub(crate) fn primary_svg_matrix_diagrams() -> impl Iterator<Item = &'static str> {
    ADMISSION_INVENTORY
        .iter()
        .copied()
        .filter(|record| record.is_primary_svg_matrix())
        .map(|record| record.diagram)
}

pub(crate) fn root_viewport_deferred_diagrams() -> impl Iterator<Item = &'static str> {
    ADMISSION_INVENTORY
        .iter()
        .copied()
        .filter(|record| record.is_root_viewport_deferred())
        .map(|record| record.diagram)
}

pub(crate) fn admission_inventory_alignment_failures(fixtures_root: &Path) -> Vec<String> {
    let workspace_root = crate::cmd::workspace_root();
    let core_capabilities = merman_core::diagram_family_capabilities_for_profile(
        merman_core::baseline::BaselineRegistryProfile::Full,
    );
    let mut failures = Vec::new();

    for record in admission_inventory() {
        let core_capability = core_family_capability(core_capabilities, record.diagram);

        if !record.has_consistent_fixture_dirs() {
            failures.push(format!(
                "admission inventory: `{}` fixture status {:?} has inconsistent dirs",
                record.diagram, record.fixtures
            ));
        }

        if record.requires_compare_command() && record.compare_command.is_none() {
            failures.push(format!(
                "admission inventory: primary SVG diagram `{}` has no compare command",
                record.diagram
            ));
        }

        if record.requires_defer_reason() && record.defer_reason.is_none() {
            failures.push(format!(
                "admission inventory: diagram `{}` needs a defer reason",
                record.diagram
            ));
        }

        if record.semantic_requires_golden()
            && !core_capability.is_some_and(|capability| capability.has_semantic_parser)
        {
            failures.push(format!(
                "admission inventory: `{}` is semantic-covered but has no core semantic parser fact",
                record.diagram
            ));
        }

        if (record.layout_requires_golden() || record.svg_requires_upstream_baseline())
            && !core_capability.is_some_and(|capability| capability.has_render_parser)
        {
            failures.push(format!(
                "admission inventory: `{}` is layout/SVG-covered but has no core render parser fact",
                record.diagram
            ));
        }

        if record.admission == AdmissionStatus::NotInPinnedBaseline && core_capability.is_some() {
            failures.push(format!(
                "admission inventory: `{}` is marked outside pinned baseline but exists in core family facts",
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
                if record.semantic_requires_golden()
                    && count_files_with_suffix(&path, ".golden.json") == 0
                {
                    failures.push(format!(
                        "admission inventory: `{}` is marked semantic-covered but has no golden JSON under {}",
                        record.diagram,
                        path.display()
                    ));
                }
                if record.layout_requires_golden()
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

        if record.svg_requires_upstream_baseline() {
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

fn core_family_capability<'a>(
    capabilities: &'a [merman_core::DiagramFamilyCapability],
    diagram: &str,
) -> Option<&'a merman_core::DiagramFamilyCapability> {
    capabilities.iter().find(|capability| {
        capability.diagram_type == diagram || capability.metadata_id == Some(diagram)
    })
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
    primary!(
        "venn",
        FixtureCorpusStatus::Normalized,
        "compare-venn-svgs",
        "docs/alignment/VENN_BETA_ADMISSION_PLAN.md"
    ),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn diagram_config_key(diagram: &'static str) -> Option<&'static str> {
        match diagram {
            "gitgraph" => Some("gitGraph"),
            "quadrantchart" => Some("quadrantChart"),
            "treemap" => None,
            "xychart" => Some("xyChart"),
            other => Some(other),
        }
    }

    fn record(diagram: &str) -> DiagramAdmissionRecord {
        admission_inventory()
            .iter()
            .copied()
            .find(|record| record.diagram == diagram)
            .unwrap_or_else(|| panic!("missing admission record for {diagram}"))
    }

    #[test]
    fn primary_svg_matrix_projection_keeps_inventory_order() {
        let diagrams: Vec<_> = primary_svg_matrix_diagrams().collect();

        assert_eq!(diagrams.first().copied(), Some("er"));
        assert!(diagrams.contains(&"flowchart"));
        assert!(diagrams.contains(&"treeView"));
        assert!(diagrams.contains(&"venn"));
        assert!(!diagrams.contains(&"zenuml"));
        assert!(!diagrams.contains(&"error"));
    }

    #[test]
    fn default_config_overrides_do_not_remove_primary_svg_config_keys() {
        let overrides_path = crate::cmd::workspace_root()
            .join("crates")
            .join("xtask")
            .join("default_config_overrides.json");
        let overrides_text =
            fs::read_to_string(&overrides_path).expect("default config overrides should read");
        let overrides: Vec<serde_json::Value> =
            serde_json::from_str(&overrides_text).expect("default config overrides should parse");
        let removed_keys: BTreeSet<String> = overrides
            .iter()
            .filter(|entry| entry.get("op").and_then(serde_json::Value::as_str) == Some("remove"))
            .filter_map(|entry| {
                entry
                    .get("path")
                    .and_then(serde_json::Value::as_array)?
                    .first()
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string)
            })
            .collect();

        let schema_path = crate::cmd::default_config_schema_path();
        let schema_text =
            fs::read_to_string(&schema_path).expect("Mermaid config schema should read");
        let schema: serde_yaml::Value =
            serde_yaml::from_str(&schema_text).expect("Mermaid config schema should parse");
        let schema_properties = schema
            .get("properties")
            .and_then(serde_yaml::Value::as_mapping)
            .expect("Mermaid config schema should expose root properties");
        let schema_keys: BTreeSet<String> = schema_properties
            .keys()
            .filter_map(serde_yaml::Value::as_str)
            .map(str::to_string)
            .collect();

        let removed_admitted_keys: Vec<String> = admission_inventory()
            .iter()
            .copied()
            .filter(|record| record.is_primary_svg_matrix())
            .filter_map(|record| {
                diagram_config_key(record.diagram).map(|key| (record.diagram, key))
            })
            .filter(|(_, key)| schema_keys.contains(*key) && removed_keys.contains(*key))
            .map(|(diagram, key)| format!("{diagram} (`{key}`)"))
            .collect();

        assert!(
            removed_admitted_keys.is_empty(),
            "default config overrides remove admitted primary SVG config keys: {}",
            removed_admitted_keys.join(", ")
        );
    }

    #[test]
    fn default_config_overrides_keep_runtime_defaults_proven_by_local_tests() {
        let overrides_path = crate::cmd::workspace_root()
            .join("crates")
            .join("xtask")
            .join("default_config_overrides.json");
        let overrides_text =
            fs::read_to_string(&overrides_path).expect("default config overrides should read");
        let overrides: Vec<serde_json::Value> =
            serde_json::from_str(&overrides_text).expect("default config overrides should parse");

        let expected = [
            (["architecture", "seed"].as_slice(), serde_json::json!(1)),
            (["pie", "donutHole"].as_slice(), serde_json::json!(0)),
            (["pie", "highlightSlice"].as_slice(), serde_json::json!("")),
            (
                ["pie", "legendPosition"].as_slice(),
                serde_json::json!("right"),
            ),
            (
                ["sankey", "useMaxWidth"].as_slice(),
                serde_json::json!(true),
            ),
        ];

        for (path, value) in expected {
            let found = overrides.iter().any(|entry| {
                entry.get("op").and_then(serde_json::Value::as_str) == Some("set")
                    && entry
                        .get("path")
                        .and_then(serde_json::Value::as_array)
                        .is_some_and(|actual_path| {
                            actual_path
                                .iter()
                                .filter_map(serde_json::Value::as_str)
                                .eq(path.iter().copied())
                        })
                    && entry.get("value") == Some(&value)
            });
            assert!(
                found,
                "default config overrides should preserve tested runtime default {} = {}",
                path.join("."),
                value
            );
        }
    }

    #[test]
    fn root_deferred_projection_is_derived_from_inventory_records() {
        let diagrams: Vec<_> = root_viewport_deferred_diagrams().collect();

        assert_eq!(diagrams, ["treeView", "ishikawa", "eventmodeling"]);
        for diagram in diagrams {
            let record = record(diagram);
            assert!(record.is_primary_svg_matrix());
            assert!(record.is_root_viewport_deferred());
            assert!(record.requires_defer_reason());
        }
    }

    #[test]
    fn admission_rules_are_record_owned() {
        let primary = record("flowchart");
        assert!(primary.requires_compare_command());
        assert!(!primary.requires_defer_reason());
        assert!(primary.semantic_requires_golden());
        assert!(primary.layout_requires_golden());
        assert!(primary.svg_requires_upstream_baseline());

        let compatibility = record("zenuml");
        assert!(!compatibility.requires_compare_command());
        assert!(!compatibility.svg_requires_upstream_baseline());

        let venn = record("venn");
        assert!(venn.requires_compare_command());
        assert!(!venn.requires_defer_reason());
        assert!(venn.semantic_requires_golden());
        assert!(venn.layout_requires_golden());
        assert!(venn.svg_requires_upstream_baseline());

        let not_admitted = record("wardley");
        assert!(!not_admitted.requires_compare_command());
        assert!(not_admitted.requires_defer_reason());
        assert!(!not_admitted.semantic_requires_golden());
        assert!(!not_admitted.layout_requires_golden());
        assert!(!not_admitted.svg_requires_upstream_baseline());
    }

    #[test]
    fn admission_inventory_records_are_internally_consistent() {
        let mut seen = BTreeSet::new();

        for record in admission_inventory() {
            assert!(
                seen.insert(record.diagram),
                "duplicate admission record for {}",
                record.diagram
            );
            assert!(
                record.has_consistent_fixture_dirs(),
                "{} fixture dirs should match {:?}",
                record.diagram,
                record.fixtures
            );
            if record.requires_compare_command() {
                assert!(
                    record.compare_command.is_some(),
                    "{} should name its compare command",
                    record.diagram
                );
            }
            if record.requires_defer_reason() {
                assert!(
                    record.defer_reason.is_some(),
                    "{} should explain its defer reason",
                    record.diagram
                );
            }
        }
    }

    #[test]
    fn admission_inventory_covered_records_are_backed_by_core_family_facts() {
        let core_capabilities = merman_core::diagram_family_capabilities_for_profile(
            merman_core::baseline::BaselineRegistryProfile::Full,
        );

        for record in admission_inventory() {
            let capability = core_family_capability(core_capabilities, record.diagram);

            if record.semantic_requires_golden() {
                assert!(
                    capability.is_some_and(|capability| capability.has_semantic_parser),
                    "{} semantic coverage should be backed by a core semantic parser fact",
                    record.diagram
                );
            }

            if record.layout_requires_golden() || record.svg_requires_upstream_baseline() {
                assert!(
                    capability.is_some_and(|capability| capability.has_render_parser),
                    "{} layout/SVG coverage should be backed by a core render parser fact",
                    record.diagram
                );
            }

            if record.admission == AdmissionStatus::NotInPinnedBaseline {
                assert!(
                    capability.is_none(),
                    "{} should not exist in core family facts",
                    record.diagram
                );
            }
        }
    }
}
