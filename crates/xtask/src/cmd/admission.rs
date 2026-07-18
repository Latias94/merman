//! Diagram admission inventory for alignment and compare tooling.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum AdmissionStatus {
    PrimarySvgMatrix,
    CompatibilityOnly,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum CoverageStatus {
    Covered,
    Deferred,
    NotApplicable,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum FixtureCorpusStatus {
    Normalized,
    NormalizedWithDeferred,
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

    fn requires_verification_fact(self) -> bool {
        self.is_primary_svg_matrix()
    }

    fn requires_defer_reason(self) -> bool {
        [self.semantic, self.layout, self.svg, self.root_viewport]
            .into_iter()
            .any(|coverage| coverage == CoverageStatus::Deferred)
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

pub(crate) fn admission_inventory_alignment_failures(fixtures_root: &Path) -> Vec<String> {
    let workspace_root = crate::cmd::workspace_root();
    let supported_diagrams = merman_core::supported_diagrams_for_profile(
        merman_core::baseline::BaselineRegistryProfile::Full,
    );
    let core_capabilities = merman_core::diagram_family_capabilities_for_profile(
        merman_core::baseline::BaselineRegistryProfile::Full,
    );
    let mut failures =
        public_metadata_admission_failures(supported_diagrams, admission_inventory());

    for record in admission_inventory() {
        let core_capability = core_family_capability(core_capabilities, record.diagram);

        if !record.has_consistent_fixture_dirs() {
            failures.push(format!(
                "admission inventory: `{}` fixture status {:?} has inconsistent dirs",
                record.diagram, record.fixtures
            ));
        }

        if record.requires_verification_fact() {
            match crate::cmd::compare::diagram_verification_fact(record.diagram) {
                Some(fact) if !fact.command.trim().is_empty() => {}
                Some(_) => failures.push(format!(
                    "admission inventory: primary SVG diagram `{}` has an empty verification command",
                    record.diagram
                )),
                None => failures.push(format!(
                    "admission inventory: primary SVG diagram `{}` has no executable verification fact",
                    record.diagram
                )),
            }
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

    for fact in crate::cmd::compare::DIAGRAM_VERIFICATION_FACTS {
        if !admission_inventory()
            .iter()
            .copied()
            .any(|record| record.diagram == fact.diagram && record.is_primary_svg_matrix())
        {
            failures.push(format!(
                "admission inventory: verification fact `{}` is not backed by a primary SVG admission record",
                fact.diagram
            ));
        }
    }

    failures
}

fn public_metadata_admission_failures(
    supported_diagrams: &[&str],
    inventory: &[DiagramAdmissionRecord],
) -> Vec<String> {
    let mut failures = Vec::new();
    let mut seen_supported = BTreeSet::new();
    for &diagram in supported_diagrams {
        if !seen_supported.insert(diagram) {
            failures.push(format!(
                "admission inventory: duplicate full-profile public metadata family `{diagram}`"
            ));
            continue;
        }

        let matching: Vec<_> = inventory
            .iter()
            .filter(|record| record.diagram == diagram)
            .collect();
        match matching.as_slice() {
            [] => failures.push(format!(
                "admission inventory: full-profile public metadata family `{diagram}` has no admission record"
            )),
            [record]
                if matches!(
                    record.admission,
                    AdmissionStatus::PrimarySvgMatrix | AdmissionStatus::CompatibilityOnly
                ) => {}
            [record] => failures.push(format!(
                "admission inventory: full-profile public metadata family `{diagram}` has unsupported admission status {:?}",
                record.admission
            )),
            records => failures.push(format!(
                "admission inventory: full-profile public metadata family `{diagram}` has {} admission records; expected exactly one",
                records.len()
            )),
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
    ($diagram:literal, $fixtures:expr, $owner:literal) => {
        DiagramAdmissionRecord {
            diagram: $diagram,
            admission: AdmissionStatus::PrimarySvgMatrix,
            fixtures: $fixtures,
            normalized_fixture_dir: Some($diagram),
            deferred_fixture_dir: match $fixtures {
                FixtureCorpusStatus::NormalizedWithDeferred => Some($diagram),
                FixtureCorpusStatus::Normalized => None,
            },
            semantic: CoverageStatus::Covered,
            layout: CoverageStatus::Covered,
            svg: CoverageStatus::Covered,
            root_viewport: CoverageStatus::Covered,
            owner_doc: $owner,
            defer_reason: None,
        }
    };
}

const ADMISSION_INVENTORY: &[DiagramAdmissionRecord] = &[
    primary!(
        "er",
        FixtureCorpusStatus::NormalizedWithDeferred,
        "docs/alignment/ER_MINIMUM.md"
    ),
    primary!(
        "flowchart",
        FixtureCorpusStatus::NormalizedWithDeferred,
        "docs/alignment/FLOWCHART_MINIMUM.md"
    ),
    primary!(
        "state",
        FixtureCorpusStatus::NormalizedWithDeferred,
        "docs/alignment/STATE_MINIMUM.md"
    ),
    primary!(
        "class",
        FixtureCorpusStatus::NormalizedWithDeferred,
        "docs/alignment/CLASS_MINIMUM.md"
    ),
    primary!(
        "sequence",
        FixtureCorpusStatus::NormalizedWithDeferred,
        "docs/alignment/SEQUENCE_MINIMUM.md"
    ),
    primary!(
        "info",
        FixtureCorpusStatus::NormalizedWithDeferred,
        "docs/alignment/INFO_MINIMUM.md"
    ),
    primary!(
        "pie",
        FixtureCorpusStatus::NormalizedWithDeferred,
        "docs/alignment/PIE_MINIMUM.md"
    ),
    primary!(
        "sankey",
        FixtureCorpusStatus::NormalizedWithDeferred,
        "docs/alignment/SANKEY_MINIMUM.md"
    ),
    primary!(
        "packet",
        FixtureCorpusStatus::NormalizedWithDeferred,
        "docs/alignment/PACKET_MINIMUM.md"
    ),
    primary!(
        "timeline",
        FixtureCorpusStatus::Normalized,
        "docs/alignment/TIMELINE_MINIMUM.md"
    ),
    primary!(
        "journey",
        FixtureCorpusStatus::Normalized,
        "docs/alignment/JOURNEY_MINIMUM.md"
    ),
    primary!(
        "kanban",
        FixtureCorpusStatus::Normalized,
        "docs/alignment/KANBAN_MINIMUM.md"
    ),
    primary!(
        "gitgraph",
        FixtureCorpusStatus::NormalizedWithDeferred,
        "docs/alignment/GITGRAPH_MINIMUM.md"
    ),
    primary!(
        "gantt",
        FixtureCorpusStatus::Normalized,
        "docs/alignment/GANTT_MINIMUM.md"
    ),
    primary!(
        "c4",
        FixtureCorpusStatus::NormalizedWithDeferred,
        "docs/alignment/C4_MINIMUM.md"
    ),
    primary!(
        "block",
        FixtureCorpusStatus::NormalizedWithDeferred,
        "docs/alignment/BLOCK_MINIMUM.md"
    ),
    primary!(
        "radar",
        FixtureCorpusStatus::NormalizedWithDeferred,
        "docs/alignment/RADAR_MINIMUM.md"
    ),
    primary!(
        "requirement",
        FixtureCorpusStatus::NormalizedWithDeferred,
        "docs/alignment/REQUIREMENT_MINIMUM.md"
    ),
    primary!(
        "mindmap",
        FixtureCorpusStatus::Normalized,
        "docs/alignment/MINDMAP_MINIMUM.md"
    ),
    primary!(
        "architecture",
        FixtureCorpusStatus::NormalizedWithDeferred,
        "docs/alignment/ARCHITECTURE_MINIMUM.md"
    ),
    primary!(
        "quadrantchart",
        FixtureCorpusStatus::Normalized,
        "docs/alignment/QUADRANTCHART_MINIMUM.md"
    ),
    primary!(
        "treemap",
        FixtureCorpusStatus::NormalizedWithDeferred,
        "docs/alignment/TREEMAP_MINIMUM.md"
    ),
    primary!(
        "xychart",
        FixtureCorpusStatus::NormalizedWithDeferred,
        "docs/alignment/XYCHART_MINIMUM.md"
    ),
    primary!(
        "treeView",
        FixtureCorpusStatus::Normalized,
        "docs/alignment/TREEVIEW_MINIMUM.md"
    ),
    primary!(
        "ishikawa",
        FixtureCorpusStatus::NormalizedWithDeferred,
        "docs/alignment/ISHIKAWA_MINIMUM.md"
    ),
    primary!(
        "eventmodeling",
        FixtureCorpusStatus::Normalized,
        "docs/alignment/EVENTMODELING_MINIMUM.md"
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
        owner_doc: "docs/alignment/ZENUML_MINIMUM.md",
        defer_reason: Some("upstream ZenUML renders through browser-only @zenuml/core"),
    },
    primary!(
        "error",
        FixtureCorpusStatus::Normalized,
        "docs/alignment/ERROR_MINIMUM.md"
    ),
    primary!(
        "venn",
        FixtureCorpusStatus::Normalized,
        "docs/alignment/VENN_BETA_ADMISSION_PLAN.md"
    ),
    primary!(
        "swimlane",
        FixtureCorpusStatus::Normalized,
        "docs/alignment/SWIMLANE_MINIMUM.md"
    ),
    primary!(
        "railroad",
        FixtureCorpusStatus::Normalized,
        "docs/alignment/RAILROAD_MINIMUM.md"
    ),
    primary!(
        "railroadEbnf",
        FixtureCorpusStatus::Normalized,
        "docs/alignment/RAILROAD_MINIMUM.md"
    ),
    primary!(
        "railroadAbnf",
        FixtureCorpusStatus::Normalized,
        "docs/alignment/RAILROAD_MINIMUM.md"
    ),
    primary!(
        "railroadPeg",
        FixtureCorpusStatus::Normalized,
        "docs/alignment/RAILROAD_MINIMUM.md"
    ),
    primary!(
        "wardley",
        FixtureCorpusStatus::Normalized,
        "docs/alignment/WARDLEY_MINIMUM.md"
    ),
    primary!(
        "cynefin",
        FixtureCorpusStatus::Normalized,
        "docs/alignment/CYNEFIN_MINIMUM.md"
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

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
        assert!(diagrams.contains(&"swimlane"));
        assert!(diagrams.contains(&"error"));
        assert!(diagrams.contains(&"wardley"));
        assert!(!diagrams.contains(&"zenuml"));
    }

    #[test]
    fn completed_root_viewport_families_are_not_deferred() {
        for diagram in ["treeView", "ishikawa", "eventmodeling"] {
            let record = record(diagram);
            assert!(record.is_primary_svg_matrix());
            assert_ne!(record.root_viewport, CoverageStatus::Deferred);
            assert!(!record.requires_defer_reason());
            assert_eq!(record.root_viewport, CoverageStatus::Covered);
            assert!(record.defer_reason.is_none());
        }
    }

    #[test]
    fn admission_rules_are_record_owned() {
        let primary = record("flowchart");
        assert!(primary.requires_verification_fact());
        assert!(!primary.requires_defer_reason());
        assert!(primary.semantic_requires_golden());
        assert!(primary.layout_requires_golden());
        assert!(primary.svg_requires_upstream_baseline());

        let compatibility = record("zenuml");
        assert!(!compatibility.requires_verification_fact());
        assert!(!compatibility.svg_requires_upstream_baseline());

        let venn = record("venn");
        assert!(venn.requires_verification_fact());
        assert!(!venn.requires_defer_reason());
        assert!(venn.semantic_requires_golden());
        assert!(venn.layout_requires_golden());
        assert!(venn.svg_requires_upstream_baseline());

        let wardley = record("wardley");
        assert!(wardley.requires_verification_fact());
        assert!(!wardley.requires_defer_reason());
        assert!(wardley.semantic_requires_golden());
        assert!(wardley.layout_requires_golden());
        assert!(wardley.svg_requires_upstream_baseline());

        let swimlane = record("swimlane");
        assert_eq!(swimlane.admission, AdmissionStatus::PrimarySvgMatrix);
        assert!(swimlane.requires_verification_fact());
        assert!(swimlane.semantic_requires_golden());
        assert!(swimlane.layout_requires_golden());
        assert!(swimlane.svg_requires_upstream_baseline());
    }

    #[test]
    fn primary_admission_and_verification_facts_are_bijective() {
        let primary_diagrams: BTreeSet<&str> = admission_inventory()
            .iter()
            .copied()
            .filter(|record| record.is_primary_svg_matrix())
            .map(|record| record.diagram)
            .collect();
        let verification_diagrams: BTreeSet<&str> = crate::cmd::compare::DIAGRAM_VERIFICATION_FACTS
            .iter()
            .map(|fact| fact.diagram)
            .collect();

        assert_eq!(
            primary_diagrams, verification_diagrams,
            "primary admission records and executable verification facts must stay one-to-one"
        );
        for fact in crate::cmd::compare::DIAGRAM_VERIFICATION_FACTS {
            assert!(
                !fact.command.trim().is_empty(),
                "verification fact {} should name its executable command",
                fact.diagram
            );
        }
    }

    #[test]
    fn full_profile_public_metadata_families_have_exactly_one_admission_record() {
        let supported_diagrams = merman_core::supported_diagrams_for_profile(
            merman_core::baseline::BaselineRegistryProfile::Full,
        );
        let failures =
            public_metadata_admission_failures(supported_diagrams, admission_inventory());

        assert!(
            failures.is_empty(),
            "full-profile public metadata must map one-to-one into admission records:\n{}",
            failures.join("\n")
        );
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
        }

        let wardley = record("wardley");
        let wardley_capability = core_family_capability(core_capabilities, "wardley")
            .expect("wardley should exist in core detector/parser facts");
        assert_eq!(wardley.admission, AdmissionStatus::PrimarySvgMatrix);
        assert!(wardley_capability.has_semantic_parser);
        assert!(wardley_capability.has_render_parser);

        let swimlane = record("swimlane");
        let swimlane_capability = core_family_capability(core_capabilities, "swimlane")
            .expect("swimlane should exist in core detector/parser facts");
        assert_eq!(swimlane.admission, AdmissionStatus::PrimarySvgMatrix);
        assert!(swimlane_capability.has_semantic_parser);
        assert!(swimlane_capability.has_render_parser);

        let cynefin = record("cynefin");
        let cynefin_capability = core_family_capability(core_capabilities, "cynefin")
            .expect("cynefin should exist in core detector/parser facts");
        assert_eq!(cynefin.admission, AdmissionStatus::PrimarySvgMatrix);
        assert!(cynefin_capability.has_semantic_parser);
        assert!(cynefin_capability.has_render_parser);

        let railroad = record("railroad");
        let railroad_capability = core_family_capability(core_capabilities, "railroad")
            .expect("railroad should exist in core detector/parser facts");
        assert_eq!(railroad.admission, AdmissionStatus::PrimarySvgMatrix);
        assert!(railroad_capability.has_semantic_parser);
        assert!(railroad_capability.has_render_parser);

        for diagram in ["railroadEbnf", "railroadAbnf", "railroadPeg"] {
            let record = record(diagram);
            let capability = core_family_capability(core_capabilities, diagram)
                .unwrap_or_else(|| panic!("{diagram} should exist in core detector/parser facts"));
            assert_eq!(record.admission, AdmissionStatus::PrimarySvgMatrix);
            assert!(capability.has_semantic_parser);
            assert!(capability.has_render_parser);
        }
    }
}
