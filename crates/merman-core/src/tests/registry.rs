use crate::baseline::BaselineRegistryProfile;
use crate::{DetectorRegistry, DiagramRegistry, MermaidConfig, RenderDiagramRegistry};
use std::collections::BTreeSet;

const PINNED_SEMANTIC_WITHOUT_EDITOR: &[&str] = &["error"];
const PINNED_WITHOUT_SEMANTICS: &[&str] = &[];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CharacterizationProfile {
    All,
    FullOnly,
}

impl CharacterizationProfile {
    fn includes(self, profile: BaselineRegistryProfile) -> bool {
        matches!(self, Self::All) || profile == BaselineRegistryProfile::Full
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CharacterizedCapabilities {
    semantic: bool,
    editor: bool,
    combined: bool,
    typed: bool,
}

const COMBINED_CAPABILITIES: CharacterizedCapabilities = CharacterizedCapabilities {
    semantic: true,
    editor: true,
    combined: true,
    typed: true,
};
const ERROR_CAPABILITIES: CharacterizedCapabilities = CharacterizedCapabilities {
    semantic: true,
    editor: false,
    combined: false,
    typed: true,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MalformedContract {
    StrictAcceptsEditorAvailable,
    StrictAcceptsEditorUnavailable,
    StrictRejectsEditorAvailable,
    StrictRejectsEditorUnavailable,
    Unsupported,
}

#[derive(Debug, Clone, Copy)]
struct FamilyCharacterization {
    variant_id: &'static str,
    logical_family: &'static str,
    profile: CharacterizationProfile,
    representative_source: &'static str,
    malformed_source: &'static str,
    capabilities: CharacterizedCapabilities,
    malformed_contract: MalformedContract,
}

const MALFORMED_SOURCE: &str = "not-a-mermaid-diagram\n";

const FAMILY_CHARACTERIZATION_MATRIX: &[FamilyCharacterization] = &[
    FamilyCharacterization {
        variant_id: "error",
        logical_family: "error",
        profile: CharacterizationProfile::All,
        representative_source: "error\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: ERROR_CAPABILITIES,
        malformed_contract: MalformedContract::StrictAcceptsEditorUnavailable,
    },
    FamilyCharacterization {
        variant_id: "flowchart-elk",
        logical_family: "flowchart",
        profile: CharacterizationProfile::FullOnly,
        representative_source: "flowchart-elk TD\nA-->B\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "flowchart-v2",
        logical_family: "flowchart",
        profile: CharacterizationProfile::All,
        representative_source: "flowchart TD\nA-->B\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "flowchart",
        logical_family: "flowchart",
        profile: CharacterizationProfile::All,
        representative_source: "graph TD\nA-->B\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "swimlane",
        logical_family: "swimlane",
        profile: CharacterizationProfile::All,
        representative_source: "swimlane-beta LR\nA-->B\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "mindmap",
        logical_family: "mindmap",
        profile: CharacterizationProfile::FullOnly,
        representative_source: "mindmap\n  root\n    child\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "architecture",
        logical_family: "architecture",
        profile: CharacterizationProfile::FullOnly,
        representative_source: "architecture-beta\n  service api(server)[API]\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "zenuml",
        logical_family: "zenuml",
        profile: CharacterizationProfile::All,
        representative_source: "zenuml\n  Alice->Bob: Hello\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "sequence",
        logical_family: "sequence",
        profile: CharacterizationProfile::All,
        representative_source: "sequenceDiagram\nAlice->>Bob: Hello\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "c4",
        logical_family: "c4",
        profile: CharacterizationProfile::All,
        representative_source: "C4Context\nPerson(user, \"User\")\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "kanban",
        logical_family: "kanban",
        profile: CharacterizationProfile::All,
        representative_source: "kanban\n  Todo\n    item1\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "classDiagram",
        logical_family: "class",
        profile: CharacterizationProfile::All,
        representative_source: "classDiagram\nclass Animal\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "class",
        logical_family: "class",
        profile: CharacterizationProfile::All,
        representative_source: "classDiagram\nclass Animal\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "er",
        logical_family: "er",
        profile: CharacterizationProfile::All,
        representative_source: "erDiagram\nCUSTOMER\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "erDiagram",
        logical_family: "er",
        profile: CharacterizationProfile::All,
        representative_source: "erDiagram\nCUSTOMER\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "gantt",
        logical_family: "gantt",
        profile: CharacterizationProfile::All,
        representative_source: "gantt\ndateFormat YYYY-MM-DD\nsection Work\nTask :a, 2024-01-01, 1d\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "info",
        logical_family: "info",
        profile: CharacterizationProfile::All,
        representative_source: "info\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictAcceptsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "pie",
        logical_family: "pie",
        profile: CharacterizationProfile::All,
        representative_source: "pie\n\"A\": 1\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictAcceptsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "requirement",
        logical_family: "requirement",
        profile: CharacterizationProfile::All,
        representative_source: "requirementDiagram\nrequirement req1 {\n  id: 1\n  text: Test\n  risk: low\n  verifymethod: analysis\n}\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "timeline",
        logical_family: "timeline",
        profile: CharacterizationProfile::All,
        representative_source: "timeline\n2024 : Event\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "gitGraph",
        logical_family: "gitGraph",
        profile: CharacterizationProfile::All,
        representative_source: "gitGraph\ncommit\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "stateDiagram",
        logical_family: "state",
        profile: CharacterizationProfile::All,
        representative_source: "stateDiagram-v2\n[*] --> Idle\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "state",
        logical_family: "state",
        profile: CharacterizationProfile::All,
        representative_source: "stateDiagram\n[*] --> Idle\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "journey",
        logical_family: "journey",
        profile: CharacterizationProfile::All,
        representative_source: "journey\nsection Work\nTask: 5\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "quadrantChart",
        logical_family: "quadrantChart",
        profile: CharacterizationProfile::All,
        representative_source: "quadrantChart\nx-axis Low --> High\ny-axis Low --> High\nA: [0.5, 0.5]\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "sankey",
        logical_family: "sankey",
        profile: CharacterizationProfile::All,
        representative_source: "sankey\nA,B,1\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "packet",
        logical_family: "packet",
        profile: CharacterizationProfile::All,
        representative_source: "packet-beta\n0-7: \"A\"\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "xychart",
        logical_family: "xychart",
        profile: CharacterizationProfile::All,
        representative_source: "xychart-beta\nline [10, 30, 20]\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "block",
        logical_family: "block",
        profile: CharacterizationProfile::All,
        representative_source: "block\n  a b c\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "eventmodeling",
        logical_family: "eventmodeling",
        profile: CharacterizationProfile::All,
        representative_source: "eventmodeling\ntf 01 ui Shop.Cart\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "treeView",
        logical_family: "treeView",
        profile: CharacterizationProfile::All,
        representative_source: "treeView-beta\n  root\n    child\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "radar",
        logical_family: "radar",
        profile: CharacterizationProfile::All,
        representative_source: "radar-beta\naxis A,B,C\ncurve sample{1,2,3}\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "ishikawa",
        logical_family: "ishikawa",
        profile: CharacterizationProfile::All,
        representative_source: "ishikawa-beta\n  Effect\n    Cause\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "treemap",
        logical_family: "treemap",
        profile: CharacterizationProfile::All,
        representative_source: "treemap-beta\n\"Root\"\n  \"Child\": 1\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "railroad",
        logical_family: "railroad",
        profile: CharacterizationProfile::All,
        representative_source: "railroad-beta\nrule = terminal(\"a\") ;\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "railroadEbnf",
        logical_family: "railroad",
        profile: CharacterizationProfile::All,
        representative_source: "railroad-ebnf-beta\nrule = \"a\" ;\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "railroadAbnf",
        logical_family: "railroad",
        profile: CharacterizationProfile::All,
        representative_source: "railroad-abnf-beta\nrule = \"a\" ;\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "railroadPeg",
        logical_family: "railroad",
        profile: CharacterizationProfile::All,
        representative_source: "railroad-peg-beta\nrule <- \"a\" ;\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "venn",
        logical_family: "venn",
        profile: CharacterizationProfile::All,
        representative_source: "venn-beta\nset Frontend\nset Backend\nunion Frontend,Backend[\"API\"]\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "wardley",
        logical_family: "wardley",
        profile: CharacterizationProfile::All,
        representative_source: "wardley-beta\ncomponent API [0.6, 0.7]\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "cynefin",
        logical_family: "cynefin",
        profile: CharacterizationProfile::All,
        representative_source: "cynefin-beta\n  complex\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: COMBINED_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
];

#[test]
fn detector_registries_follow_family_fact_order() {
    let full = DetectorRegistry::pinned_mermaid_baseline_full();
    let full_actual: Vec<_> = full.detector_ids().collect();
    let full_expected: Vec<_> = crate::family::detector_facts(BaselineRegistryProfile::Full)
        .iter()
        .map(|fact| fact.id)
        .collect();
    assert_eq!(
        full_actual,
        with_frontmatter_guard_after_error(full_expected)
    );

    let tiny = DetectorRegistry::pinned_mermaid_baseline_tiny();
    let tiny_actual: Vec<_> = tiny.detector_ids().collect();
    let tiny_expected: Vec<_> = crate::family::detector_facts(BaselineRegistryProfile::Tiny)
        .iter()
        .map(|fact| fact.id)
        .collect();
    assert_eq!(
        tiny_actual,
        with_frontmatter_guard_after_error(tiny_expected)
    );
}

fn with_frontmatter_guard_after_error(mut family_ids: Vec<&'static str>) -> Vec<&'static str> {
    let insert_at = family_ids
        .iter()
        .position(|id| *id == "error")
        .expect("error detector")
        + 1;
    family_ids.insert(insert_at, "---");
    family_ids
}

#[test]
fn tiny_detector_projection_is_derived_from_full_detector_facts() {
    let full_only = ["architecture", "flowchart-elk", "mindmap"];
    let full_expected: Vec<_> = crate::family::detector_facts(BaselineRegistryProfile::Full)
        .iter()
        .filter_map(|fact| (!full_only.contains(&fact.id)).then_some(fact.id))
        .collect();
    let tiny_actual: Vec<_> = crate::family::detector_facts(BaselineRegistryProfile::Tiny)
        .iter()
        .map(|fact| fact.id)
        .collect();

    assert_eq!(tiny_actual, full_expected);
    for id in full_only {
        assert!(
            crate::family::detector_facts(BaselineRegistryProfile::Full)
                .iter()
                .any(|fact| fact.id == id),
            "{id} should stay registered in the full detector profile",
        );
        assert!(
            !tiny_actual.contains(&id),
            "{id} should stay excluded from the tiny detector profile",
        );
    }
}

#[test]
fn fast_detector_respects_family_feature_profile() {
    let mut config = MermaidConfig::empty_object();
    let full = DetectorRegistry::pinned_mermaid_baseline_full();
    assert_eq!(
        full.detect_type_precleaned("mindmap\n  root", &mut config)
            .unwrap(),
        "mindmap"
    );

    let tiny = DetectorRegistry::pinned_mermaid_baseline_tiny();
    let err = tiny
        .detect_type_precleaned("mindmap\n  root", &mut config)
        .unwrap_err();
    assert!(
        err.to_string()
            .contains("No diagram type detected matching given configuration")
    );
}

#[test]
fn fast_detector_keywords_respect_family_feature_profile() {
    assert_eq!(
        crate::family::fast_detect_by_leading_keyword(
            "sequenceDiagram\nA->>B: hi",
            BaselineRegistryProfile::Full,
        ),
        Some("sequence")
    );
    assert_eq!(
        crate::family::fast_detect_by_leading_keyword(
            "sequenceDiagram\nA->>B: hi",
            BaselineRegistryProfile::Tiny,
        ),
        Some("sequence")
    );
    assert_eq!(
        crate::family::fast_detect_by_leading_keyword(
            "mindmap\nroot",
            BaselineRegistryProfile::Full,
        ),
        Some("mindmap")
    );
    assert_eq!(
        crate::family::fast_detect_by_leading_keyword(
            "mindmap\nroot",
            BaselineRegistryProfile::Tiny,
        ),
        None
    );
}

#[test]
fn parser_registries_follow_family_fact_projection() {
    for (profile, semantic, render) in [
        (
            BaselineRegistryProfile::Full,
            DiagramRegistry::pinned_mermaid_baseline_full(),
            RenderDiagramRegistry::pinned_mermaid_baseline_full(),
        ),
        (
            BaselineRegistryProfile::Tiny,
            DiagramRegistry::pinned_mermaid_baseline_tiny(),
            RenderDiagramRegistry::pinned_mermaid_baseline_tiny(),
        ),
    ] {
        let semantic_actual = sorted_set(semantic.parser_ids());
        let semantic_expected = sorted_set(
            crate::family::semantic_parser_facts(profile)
                .iter()
                .map(|fact| fact.id),
        );
        assert_eq!(semantic_actual, semantic_expected, "{profile:?}");

        let render_actual = sorted_set(render.parser_ids());
        let render_expected = sorted_set(
            crate::family::render_parser_facts(profile)
                .iter()
                .map(|fact| fact.id),
        );
        assert_eq!(render_actual, render_expected, "{profile:?}");
    }
}

#[test]
fn family_catalog_projections_are_bidirectionally_aligned() {
    for profile in [BaselineRegistryProfile::Full, BaselineRegistryProfile::Tiny] {
        let detector_ids = sorted_set(
            crate::family::detector_facts(profile)
                .iter()
                .map(|fact| fact.id),
        );
        let semantic_ids = sorted_set(
            crate::family::semantic_parser_facts(profile)
                .iter()
                .map(|fact| fact.id),
        );
        let headers = crate::diagram_header_facts_for_profile(profile);
        let capabilities = crate::diagram_family_capabilities_for_profile(profile);

        for capability in capabilities {
            assert!(
                !capability.has_detector || detector_ids.contains(capability.diagram_type),
                "capability {} declares a detector missing from {profile:?}",
                capability.diagram_type,
            );
            assert!(
                !capability.has_semantic_parser || semantic_ids.contains(capability.diagram_type),
                "capability {} declares a semantic parser missing from {profile:?}",
                capability.diagram_type,
            );
            assert!(
                capability.has_header
                    == headers
                        .iter()
                        .any(|header| header.diagram_type == capability.diagram_type),
                "capability {} disagrees with its header projection in {profile:?}",
                capability.diagram_type,
            );
        }

        for header in headers {
            assert!(
                semantic_ids
                    .iter()
                    .any(|diagram_type| *diagram_type == header.diagram_type),
                "header {} points to missing semantic parser {} in {profile:?}",
                header.label,
                header.diagram_type,
            );
        }
    }

    let tiny_ids = sorted_set(
        crate::diagram_family_capabilities_for_profile(BaselineRegistryProfile::Tiny)
            .iter()
            .map(|capability| capability.diagram_type),
    );
    for header in crate::diagram_header_facts_for_profile(BaselineRegistryProfile::Full) {
        assert_eq!(
            header.full_only,
            !tiny_ids.contains(header.diagram_type),
            "header {} has a feature-profile flag that disagrees with its family",
            header.label,
        );
    }
}

#[test]
fn selected_supported_diagrams_follow_feature_profile() {
    assert_eq!(
        crate::supported_diagrams(),
        crate::supported_diagrams_for_profile(crate::selected_baseline_registry_profile())
    );

    #[cfg(feature = "full-registry")]
    assert_eq!(
        crate::supported_diagrams(),
        crate::supported_diagrams_for_profile(BaselineRegistryProfile::Full)
    );

    #[cfg(not(feature = "full-registry"))]
    assert_eq!(
        crate::supported_diagrams(),
        crate::supported_diagrams_for_profile(BaselineRegistryProfile::Tiny)
    );
}

#[test]
fn diagram_header_facts_follow_feature_profile() {
    let full_labels = crate::diagram_header_facts_for_profile(BaselineRegistryProfile::Full)
        .iter()
        .map(|fact| fact.label)
        .collect::<Vec<_>>();
    assert_eq!(
        full_labels,
        vec![
            "flowchart TD",
            "graph TD",
            "sequenceDiagram",
            "swimlane-beta",
            "classDiagram",
            "classDiagram-v2",
            "stateDiagram-v2",
            "stateDiagram",
            "erDiagram",
            "gantt",
            "mindmap",
            "info",
            "journey",
            "timeline",
            "gitGraph",
            "pie",
            "requirementDiagram",
            "sankey",
            "packet",
            "packet-beta",
            "xychart",
            "xychart-beta",
            "treeView-beta",
            "ishikawa-beta",
            "eventmodeling",
            "quadrantChart",
            "venn-beta",
            "zenuml",
            "C4Context",
            "C4Container",
            "C4Component",
            "C4Dynamic",
            "C4Deployment",
            "kanban",
            "architecture-beta",
            "block-beta",
            "radar-beta",
            "treemap-beta",
            "railroad-beta",
            "railroad-ebnf-beta",
            "railroad-abnf-beta",
            "railroad-peg-beta",
            "wardley-beta",
            "cynefin-beta",
            "flowchart-elk TD",
        ]
    );

    let full_only_labels = crate::diagram_header_facts_for_profile(BaselineRegistryProfile::Full)
        .iter()
        .filter(|fact| fact.full_only)
        .map(|fact| fact.label)
        .collect::<Vec<_>>();
    assert_eq!(
        full_only_labels,
        vec!["mindmap", "architecture-beta", "flowchart-elk TD"]
    );

    let tiny_labels = crate::diagram_header_facts_for_profile(BaselineRegistryProfile::Tiny)
        .iter()
        .map(|fact| fact.label)
        .collect::<Vec<_>>();
    assert_eq!(
        tiny_labels,
        vec![
            "flowchart TD",
            "graph TD",
            "sequenceDiagram",
            "swimlane-beta",
            "classDiagram",
            "classDiagram-v2",
            "stateDiagram-v2",
            "stateDiagram",
            "erDiagram",
            "gantt",
            "info",
            "journey",
            "timeline",
            "gitGraph",
            "pie",
            "requirementDiagram",
            "sankey",
            "packet",
            "packet-beta",
            "xychart",
            "xychart-beta",
            "treeView-beta",
            "ishikawa-beta",
            "eventmodeling",
            "quadrantChart",
            "venn-beta",
            "zenuml",
            "C4Context",
            "C4Container",
            "C4Component",
            "C4Dynamic",
            "C4Deployment",
            "kanban",
            "block-beta",
            "radar-beta",
            "treemap-beta",
            "railroad-beta",
            "railroad-ebnf-beta",
            "railroad-abnf-beta",
            "railroad-peg-beta",
            "wardley-beta",
            "cynefin-beta",
        ]
    );

    for profile in [BaselineRegistryProfile::Tiny, BaselineRegistryProfile::Full] {
        let capabilities = crate::diagram_family_capabilities_for_profile(profile);
        for header in crate::diagram_header_facts_for_profile(profile) {
            assert!(
                capabilities.iter().any(|capability| {
                    capability.diagram_type == header.diagram_type && capability.has_semantic_parser
                }),
                "header {} must be backed by a semantic parser",
                header.label
            );
        }
    }
}

#[test]
fn supported_diagram_metadata_is_backed_by_typed_render_projection() {
    assert_eq!(
        crate::supported_diagrams_for_profile(BaselineRegistryProfile::Full),
        &[
            "architecture",
            "block",
            "c4",
            "class",
            "cynefin",
            "er",
            "eventmodeling",
            "flowchart",
            "gantt",
            "gitgraph",
            "info",
            "ishikawa",
            "journey",
            "kanban",
            "mindmap",
            "packet",
            "pie",
            "quadrantchart",
            "radar",
            "railroad",
            "railroadAbnf",
            "railroadEbnf",
            "railroadPeg",
            "requirement",
            "sankey",
            "sequence",
            "state",
            "swimlane",
            "timeline",
            "treeView",
            "treemap",
            "venn",
            "wardley",
            "xychart",
            "zenuml",
        ]
    );

    assert_eq!(
        crate::supported_diagrams_for_profile(BaselineRegistryProfile::Tiny),
        &[
            "block",
            "c4",
            "class",
            "cynefin",
            "er",
            "eventmodeling",
            "flowchart",
            "gantt",
            "gitgraph",
            "info",
            "ishikawa",
            "journey",
            "kanban",
            "packet",
            "pie",
            "quadrantchart",
            "radar",
            "railroad",
            "railroadAbnf",
            "railroadEbnf",
            "railroadPeg",
            "requirement",
            "sankey",
            "sequence",
            "state",
            "swimlane",
            "timeline",
            "treeView",
            "treemap",
            "venn",
            "wardley",
            "xychart",
            "zenuml",
        ]
    );

    for profile in [BaselineRegistryProfile::Full, BaselineRegistryProfile::Tiny] {
        let render_ids = sorted_set(
            crate::family::render_parser_facts(profile)
                .iter()
                .map(|fact| fact.id),
        );
        for fact in crate::family::supported_diagram_facts(profile) {
            for parser_id in &fact.render_parser_ids {
                assert!(
                    render_ids.contains(parser_id),
                    "{} metadata points to missing render parser {parser_id}",
                    fact.metadata_id
                );
            }
        }
    }
}

#[test]
fn diagram_family_capabilities_follow_detector_and_parser_fact_projection() {
    let full = crate::diagram_family_capabilities_for_profile(BaselineRegistryProfile::Full);
    let tiny = crate::diagram_family_capabilities_for_profile(BaselineRegistryProfile::Tiny);

    let gitgraph = family_capability(full, "gitGraph");
    assert_eq!(gitgraph.metadata_id, Some("gitgraph"));
    assert!(gitgraph.has_semantic_parser);
    assert!(gitgraph.has_render_parser);

    let tree_view = family_capability(full, "treeView");
    assert_eq!(tree_view.metadata_id, Some("treeView"));
    assert!(tree_view.has_semantic_parser);
    assert!(tree_view.has_render_parser);

    let error = family_capability(full, "error");
    assert_eq!(error.metadata_id, None);
    assert_eq!(error.render_model_kind, Some("error"));
    assert!(error.has_semantic_parser);
    assert!(error.has_render_parser);

    let swimlane = family_capability(full, "swimlane");
    assert_eq!(swimlane.metadata_id, Some("swimlane"));
    assert_eq!(swimlane.render_model_kind, Some("flowchart"));
    assert!(swimlane.has_semantic_parser);
    assert!(swimlane.has_render_parser);

    let railroad = family_capability(full, "railroad");
    assert_eq!(railroad.metadata_id, Some("railroad"));
    assert!(railroad.has_semantic_parser);
    assert!(railroad.has_render_parser);

    for (diagram_type, metadata_id) in [
        ("railroadEbnf", "railroadEbnf"),
        ("railroadAbnf", "railroadAbnf"),
        ("railroadPeg", "railroadPeg"),
    ] {
        let capability = family_capability(full, diagram_type);
        assert_eq!(capability.metadata_id, Some(metadata_id));
        assert!(capability.has_semantic_parser);
        assert!(capability.has_render_parser);
    }

    let cynefin = family_capability(full, "cynefin");
    assert_eq!(cynefin.metadata_id, Some("cynefin"));
    assert!(cynefin.has_semantic_parser);
    assert!(cynefin.has_render_parser);

    let wardley = family_capability(full, "wardley");
    assert_eq!(wardley.metadata_id, Some("wardley"));
    assert_eq!(wardley.render_model_kind, Some("wardley"));
    assert!(wardley.has_semantic_parser);
    assert!(wardley.has_editor_parser);
    assert!(wardley.has_combined_parser);
    assert!(wardley.has_render_parser);
    assert_eq!(wardley.config_namespace, Some("wardley-beta"));

    assert!(!full.iter().any(|fact| fact.diagram_type == "---"));
    assert!(full.iter().any(|fact| fact.diagram_type == "mindmap"));
    assert!(!tiny.iter().any(|fact| fact.diagram_type == "mindmap"));
    assert!(!tiny.iter().any(|fact| fact.diagram_type == "architecture"));
    assert!(!tiny.iter().any(|fact| fact.diagram_type == "flowchart-elk"));
    assert!(tiny.iter().any(|fact| fact.diagram_type == "swimlane"));
    assert!(tiny.iter().any(|fact| fact.diagram_type == "cynefin"));
    assert!(tiny.iter().any(|fact| fact.diagram_type == "railroad"));
    assert!(tiny.iter().any(|fact| fact.diagram_type == "railroadEbnf"));
    assert!(tiny.iter().any(|fact| fact.diagram_type == "railroadAbnf"));
    assert!(tiny.iter().any(|fact| fact.diagram_type == "railroadPeg"));
}

#[test]
fn every_catalog_variant_projects_all_declared_capabilities_in_full_and_tiny_profiles() {
    for profile in [BaselineRegistryProfile::Full, BaselineRegistryProfile::Tiny] {
        let detector_ids = sorted_set(
            crate::family::detector_facts(profile)
                .iter()
                .map(|fact| fact.id),
        );
        let semantic_ids = sorted_set(
            crate::family::semantic_parser_facts(profile)
                .iter()
                .map(|fact| fact.id),
        );
        let combined_ids = sorted_set(
            crate::family::combined_parser_facts(profile)
                .iter()
                .map(|fact| fact.id),
        );
        let render_ids = sorted_set(
            crate::family::render_parser_facts(profile)
                .iter()
                .map(|fact| fact.id),
        );
        let header_ids = sorted_set(
            crate::diagram_header_facts_for_profile(profile)
                .iter()
                .map(|fact| fact.diagram_type),
        );

        for capability in crate::diagram_family_capabilities_for_profile(profile) {
            let id = capability.diagram_type;
            assert_eq!(
                capability.has_detector,
                detector_ids.contains(id),
                "{profile:?} {id}"
            );
            assert_eq!(
                capability.has_semantic_parser,
                semantic_ids.contains(id),
                "{profile:?} {id}"
            );
            assert_eq!(
                capability.has_editor_parser,
                combined_ids.contains(id),
                "{profile:?} {id}"
            );
            assert_eq!(
                capability.has_combined_parser,
                combined_ids.contains(id),
                "{profile:?} {id}"
            );
            assert_eq!(
                capability.has_render_parser,
                render_ids.contains(id),
                "{profile:?} {id}"
            );
            assert_eq!(
                capability.has_header,
                header_ids.contains(id),
                "{profile:?} {id}"
            );
            assert_eq!(
                crate::diagram_type_family_kind(id),
                Some(capability.logical_family_kind),
                "{profile:?} {id}"
            );
            assert_eq!(
                crate::diagram_type_render_model_kind(id),
                capability.render_model_kind,
                "{profile:?} {id}"
            );
            assert_eq!(
                crate::family::config_namespace_for_diagram_type(id),
                capability.config_namespace,
                "{profile:?} {id}"
            );

            let render_fact = crate::family::render_parser_facts(profile)
                .iter()
                .find(|fact| fact.id == id);
            assert_eq!(
                render_fact.and_then(|fact| fact.metadata_id),
                capability.metadata_id,
                "{profile:?} {id}"
            );
            assert_eq!(
                render_fact.map(|fact| fact.model_kind),
                capability.render_model_kind,
                "{profile:?} {id}"
            );
        }
    }

    let full_ids = sorted_set(
        crate::diagram_family_capabilities_for_profile(BaselineRegistryProfile::Full)
            .iter()
            .map(|fact| fact.diagram_type),
    );
    let tiny_ids = sorted_set(
        crate::diagram_family_capabilities_for_profile(BaselineRegistryProfile::Tiny)
            .iter()
            .map(|fact| fact.diagram_type),
    );
    assert_eq!(
        full_ids
            .difference(&tiny_ids)
            .copied()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["architecture", "flowchart-elk", "mindmap"])
    );
    assert!(!full_ids.contains("---"));
}

#[test]
fn registry_characterization_matrix_covers_every_variant_and_logical_family() {
    assert_eq!(FAMILY_CHARACTERIZATION_MATRIX.len(), 41);

    let variant_ids = FAMILY_CHARACTERIZATION_MATRIX
        .iter()
        .map(|row| row.variant_id)
        .collect::<BTreeSet<_>>();
    let logical_families = FAMILY_CHARACTERIZATION_MATRIX
        .iter()
        .map(|row| row.logical_family)
        .collect::<BTreeSet<_>>();
    assert_eq!(variant_ids.len(), 41, "variant ids must be unique");
    assert_eq!(logical_families.len(), 33);

    for profile in [BaselineRegistryProfile::Full, BaselineRegistryProfile::Tiny] {
        let actual = crate::diagram_family_capabilities_for_profile(profile);
        let actual_ids = actual
            .iter()
            .map(|fact| fact.diagram_type)
            .collect::<BTreeSet<_>>();
        let expected_ids = FAMILY_CHARACTERIZATION_MATRIX
            .iter()
            .filter_map(|row| row.profile.includes(profile).then_some(row.variant_id))
            .collect::<BTreeSet<_>>();
        assert_eq!(actual_ids, expected_ids, "{profile:?} admission drift");

        for row in FAMILY_CHARACTERIZATION_MATRIX
            .iter()
            .filter(|row| row.profile.includes(profile))
        {
            let fact = family_capability(actual, row.variant_id);
            assert_eq!(
                fact.logical_family_kind, row.logical_family,
                "{profile:?} {} logical family",
                row.variant_id
            );
            assert_eq!(
                CharacterizedCapabilities {
                    semantic: fact.has_semantic_parser,
                    editor: fact.has_editor_parser,
                    combined: fact.has_combined_parser,
                    typed: fact.has_render_parser,
                },
                row.capabilities,
                "{profile:?} {} capability contract",
                row.variant_id
            );
        }
    }
}

#[test]
fn registry_characterization_matrix_executes_representative_and_malformed_contracts() {
    let engine = crate::Engine::new();
    let selected_profile = crate::selected_baseline_registry_profile();
    let mut malformed_contract_mismatches = Vec::new();
    let mut recovery_contract_mismatches = Vec::new();

    for row in FAMILY_CHARACTERIZATION_MATRIX
        .iter()
        .filter(|row| row.profile.includes(selected_profile))
    {
        if row.capabilities.semantic {
            let parsed = engine
                .parse_diagram_with_type_sync(
                    row.variant_id,
                    row.representative_source,
                    crate::ParseOptions::strict(),
                )
                .unwrap_or_else(|err| {
                    panic!(
                        "{} representative semantic parse failed: {err}",
                        row.variant_id
                    )
                })
                .unwrap_or_else(|| panic!("{} returned no semantic model", row.variant_id));
            assert_eq!(parsed.meta.diagram_type, row.variant_id);
        }

        if row.capabilities.editor {
            assert!(
                engine
                    .parse_editor_semantic_facts_with_type_sync(
                        row.variant_id,
                        row.representative_source,
                    )
                    .unwrap_or_else(|err| {
                        panic!(
                            "{} representative editor parse failed: {err}",
                            row.variant_id
                        )
                    })
                    .is_some(),
                "{} returned no editor facts",
                row.variant_id
            );
        }

        if row.capabilities.typed {
            let parsed = engine
                .parse_diagram_for_render_model_with_type_sync(
                    row.variant_id,
                    row.representative_source,
                    crate::ParseOptions::strict(),
                )
                .unwrap_or_else(|err| {
                    panic!(
                        "{} representative typed parse failed: {err}",
                        row.variant_id
                    )
                })
                .unwrap_or_else(|| panic!("{} returned no typed model", row.variant_id));
            assert_eq!(parsed.metadata().diagram_type, row.variant_id);
        }

        if row.capabilities.combined {
            let parsed = engine
                .parse_diagram_snapshot_sync(row.representative_source)
                .unwrap_or_else(|err| {
                    panic!(
                        "{} representative combined parse failed: {err}",
                        row.variant_id
                    )
                })
                .unwrap_or_else(|| panic!("{} returned no combined model", row.variant_id));
            assert_eq!(
                crate::diagram_type_family_kind(&parsed.metadata().diagram_type),
                Some(row.logical_family),
                "{} combined detection left its logical family",
                row.variant_id
            );
            assert!(matches!(
                parsed.editor_facts(),
                crate::ParsedEditorFacts::Available(_)
            ));
        }

        let malformed_semantic = engine.parse_diagram_with_type_sync(
            row.variant_id,
            row.malformed_source,
            crate::ParseOptions::strict(),
        );
        let malformed_editor =
            engine.parse_editor_semantic_facts_with_type_sync(row.variant_id, row.malformed_source);
        if row.capabilities.combined {
            let snapshot = engine
                .parse_diagram_snapshot_with_type_sync(row.variant_id, row.malformed_source)
                .unwrap_or_else(|error| {
                    panic!(
                        "{} malformed snapshot operation failed: {error}",
                        row.variant_id
                    )
                })
                .unwrap_or_else(|| panic!("{} malformed snapshot was absent", row.variant_id));
            let outcome_matches_semantic = matches!(
                (&malformed_semantic, snapshot.outcome()),
                (Ok(Some(_)), crate::DiagramParseOutcome::Parsed(_))
                    | (Err(_), crate::DiagramParseOutcome::Failed(_))
            );
            if !outcome_matches_semantic {
                recovery_contract_mismatches.push(format!(
                    "{} malformed snapshot outcome drifted from strict semantics",
                    row.variant_id
                ));
            }
            if !matches!(
                snapshot.editor_facts(),
                crate::ParsedEditorFacts::Available(_)
            ) {
                recovery_contract_mismatches.push(format!(
                    "{} malformed snapshot omitted parser-backed editor facts",
                    row.variant_id
                ));
            }
        }
        if let (Err(_), Ok(Some(facts))) = (&malformed_semantic, &malformed_editor) {
            if facts.completeness != crate::EditorSemanticCompleteness::Recovered {
                recovery_contract_mismatches.push(format!(
                    "{} strict failure returned {:?} editor semantics",
                    row.variant_id, facts.completeness
                ));
            }
            if !facts.diagnostics.iter().any(|diagnostic| {
                diagnostic.kind == crate::EditorSemanticDiagnosticKind::ParserRecovery
                    && diagnostic.span.is_some()
            }) {
                recovery_contract_mismatches.push(format!(
                    "{} strict failure returned editor recovery without a source-backed parser diagnostic: {:?}",
                    row.variant_id, facts.diagnostics
                ));
            }
            if let Some(failure) = facts.lexeme_failure() {
                recovery_contract_mismatches.push(format!(
                    "{} malformed recovery returned invalid lexemes: {failure:?}",
                    row.variant_id
                ));
            }
            for lexeme in facts.lexemes() {
                let span = lexeme.span();
                if span.start >= span.end
                    || span.end > row.malformed_source.len()
                    || !row.malformed_source.is_char_boundary(span.start)
                    || !row.malformed_source.is_char_boundary(span.end)
                {
                    recovery_contract_mismatches.push(format!(
                        "{} malformed recovery returned invalid source span {span:?}",
                        row.variant_id
                    ));
                }
                match lexeme.producer().kind() {
                    crate::EditorLexemeProducerKind::GlobalPreprocess => {
                        if lexeme.producer().family().is_some() {
                            recovery_contract_mismatches.push(format!(
                                "{} global recovery lexeme unexpectedly names a family",
                                row.variant_id
                            ));
                        }
                    }
                    crate::EditorLexemeProducerKind::FamilyRecovery => {
                        if lexeme.producer().family().is_none() {
                            recovery_contract_mismatches.push(format!(
                                "{} family recovery lexeme has no family provenance",
                                row.variant_id
                            ));
                        }
                    }
                    producer => recovery_contract_mismatches.push(format!(
                        "{} malformed recovery retained non-recovery producer {producer:?}",
                        row.variant_id
                    )),
                }
            }
            if let Some(pair) = facts
                .lexemes()
                .windows(2)
                .find(|pair| pair[0].span().end > pair[1].span().start)
            {
                recovery_contract_mismatches.push(format!(
                    "{} malformed recovery returned overlapping lexemes {:?} and {:?}",
                    row.variant_id,
                    pair[0].span(),
                    pair[1].span()
                ));
            }
        }
        let observed_contract = match (&malformed_semantic, &malformed_editor) {
            (Ok(Some(_)), Ok(Some(_))) => MalformedContract::StrictAcceptsEditorAvailable,
            (Ok(Some(_)), Ok(None)) => MalformedContract::StrictAcceptsEditorUnavailable,
            (Err(crate::Error::UnsupportedDiagram { .. }), Ok(None))
            | (
                Err(crate::Error::UnsupportedDiagram { .. }),
                Err(crate::Error::UnsupportedDiagram { .. }),
            ) => MalformedContract::Unsupported,
            (Err(_), Ok(Some(_))) => MalformedContract::StrictRejectsEditorAvailable,
            (Err(_), Ok(None)) => MalformedContract::StrictRejectsEditorUnavailable,
            _ => panic!(
                "{} exposed an unclassified malformed contract: semantic={malformed_semantic:?}, editor={malformed_editor:?}",
                row.variant_id
            ),
        };
        if observed_contract != row.malformed_contract {
            malformed_contract_mismatches.push(format!(
                "{}: expected {:?}, observed {:?}",
                row.variant_id, row.malformed_contract, observed_contract
            ));
        }
    }

    assert!(
        malformed_contract_mismatches.is_empty(),
        "malformed contracts drifted:\n{}",
        malformed_contract_mismatches.join("\n")
    );
    assert!(
        recovery_contract_mismatches.is_empty(),
        "malformed recovery contracts drifted:\n{}",
        recovery_contract_mismatches.join("\n")
    );
}

#[test]
fn catalog_declares_alias_ownership_and_capability_gaps_without_inheritance() {
    let full = crate::diagram_family_capabilities_for_profile(BaselineRegistryProfile::Full);

    let zenuml = family_capability(full, "zenuml");
    assert_eq!(zenuml.logical_family_kind, "zenuml");
    assert_eq!(zenuml.render_model_kind, Some("zenuml"));

    for id in ["railroad", "railroadEbnf", "railroadAbnf", "railroadPeg"] {
        let fact = family_capability(full, id);
        assert_eq!(fact.logical_family_kind, "railroad", "{id}");
        assert_eq!(fact.render_model_kind, Some("railroad"), "{id}");
        assert!(fact.has_semantic_parser && fact.has_editor_parser && fact.has_render_parser);
    }

    let swimlane = family_capability(full, "swimlane");
    assert_eq!(swimlane.logical_family_kind, "swimlane");
    assert!(swimlane.has_detector && swimlane.has_semantic_parser && swimlane.has_editor_parser);
    assert!(swimlane.has_combined_parser);
    assert!(swimlane.has_render_parser);
    assert_eq!(swimlane.render_model_kind, Some("flowchart"));
    assert_eq!(swimlane.metadata_id, Some("swimlane"));

    let er_alias = family_capability(full, "erDiagram");
    assert_eq!(er_alias.logical_family_kind, "er");
    assert!(!er_alias.has_detector);
    assert!(!er_alias.has_header);
    assert!(
        er_alias.has_semantic_parser
            && er_alias.has_editor_parser
            && er_alias.has_combined_parser
            && er_alias.has_render_parser
    );

    let error = family_capability(full, "error");
    assert!(error.has_detector && error.has_semantic_parser && error.has_render_parser);
    assert!(!error.has_editor_parser && !error.has_combined_parser);
    assert_eq!(error.render_model_kind, Some("error"));

    let wardley = family_capability(full, "wardley");
    assert!(wardley.has_detector && wardley.has_header);
    assert!(
        wardley.has_semantic_parser
            && wardley.has_editor_parser
            && wardley.has_combined_parser
            && wardley.has_render_parser
    );
    assert_eq!(wardley.render_model_kind, Some("wardley"));
    assert_eq!(wardley.metadata_id, Some("wardley"));

    let combined = full
        .iter()
        .filter_map(|fact| fact.has_combined_parser.then_some(fact.diagram_type))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        combined,
        BTreeSet::from([
            "architecture",
            "block",
            "c4",
            "class",
            "classDiagram",
            "flowchart",
            "flowchart-elk",
            "flowchart-v2",
            "gantt",
            "gitGraph",
            "info",
            "journey",
            "kanban",
            "mindmap",
            "packet",
            "pie",
            "quadrantChart",
            "radar",
            "railroad",
            "railroadAbnf",
            "railroadEbnf",
            "railroadPeg",
            "requirement",
            "sankey",
            "sequence",
            "state",
            "stateDiagram",
            "swimlane",
            "timeline",
            "cynefin",
            "er",
            "erDiagram",
            "eventmodeling",
            "ishikawa",
            "treeView",
            "treemap",
            "venn",
            "wardley",
            "xychart",
            "zenuml",
        ])
    );
}

#[test]
fn builtin_editor_and_render_capabilities_require_combined_semantic_ownership() {
    for profile in [BaselineRegistryProfile::Full, BaselineRegistryProfile::Tiny] {
        for capability in crate::diagram_family_capabilities_for_profile(profile) {
            if capability.has_semantic_parser && capability.has_editor_parser {
                assert!(
                    capability.has_combined_parser,
                    "{} exposes semantic and editor parsers without one combined construction in {profile:?}",
                    capability.diagram_type
                );
            }

            if capability.has_render_parser {
                if capability.diagram_type == "error" {
                    assert!(capability.has_semantic_parser);
                    continue;
                }
                assert!(
                    capability.has_semantic_parser && capability.has_combined_parser,
                    "{} exposes a typed render parser without semantic + combined ownership in {profile:?}",
                    capability.diagram_type
                );
            }
        }
    }
}

#[test]
fn langium_family_combined_parse_constructs_syntax_once() {
    for (family, source) in [
        ("info", "info\n"),
        ("pie", "pie\n\"A\": 1\n"),
        ("packet", "packet-beta\n0-7: \"A\"\n"),
        ("cynefin", "cynefin-beta\n  complex\n"),
        ("radar", "radar-beta\naxis A,B,C\ncurve sample{1,2,3}\n"),
        ("wardley", "wardley-beta\ncomponent API [0.6, 0.7]\n"),
    ] {
        crate::diagrams::langium_common::reset_family_syntax_construction_count(family);

        let parsed = crate::Engine::new()
            .parse_diagram_snapshot_sync(source)
            .unwrap_or_else(|error| panic!("{family} combined parse failed: {error}"))
            .unwrap_or_else(|| panic!("{family} combined parse returned no diagram"));

        assert!(matches!(
            parsed.editor_facts(),
            crate::ParsedEditorFacts::Available(_)
        ));
        assert_eq!(
            crate::diagrams::langium_common::family_syntax_construction_count(family),
            1,
            "one combined request must construct {family} syntax once"
        );
    }
}

#[test]
fn git_graph_combined_parse_constructs_syntax_once() {
    let family = "gitGraph";
    crate::diagrams::langium_common::reset_family_syntax_construction_count(family);

    let parsed = crate::Engine::new()
        .parse_diagram_snapshot_sync("gitGraph\ncommit\n")
        .expect("gitGraph combined parse succeeds")
        .expect("gitGraph combined parse returns a diagram");

    assert!(matches!(
        parsed.editor_facts(),
        crate::ParsedEditorFacts::Available(_)
    ));
    assert_eq!(
        crate::diagrams::langium_common::family_syntax_construction_count(family),
        1,
        "one combined request must construct gitGraph syntax once"
    );
}

#[test]
fn er_combined_parse_constructs_family_syntax_once() {
    crate::diagrams::er::reset_er_syntax_construction_count();

    let parsed = crate::Engine::new()
        .parse_diagram_snapshot_sync("erDiagram\nCUSTOMER ||--o{ ORDER : places\n")
        .expect("ER combined parse succeeds")
        .expect("ER combined parse returns a diagram");

    assert!(matches!(
        parsed.editor_facts(),
        crate::ParsedEditorFacts::Available(_)
    ));
    assert_eq!(
        crate::diagrams::er::er_syntax_construction_count(),
        1,
        "one combined request must construct ER syntax once"
    );
}

#[test]
fn sequence_combined_parse_constructs_family_syntax_once() {
    crate::diagrams::sequence::reset_sequence_syntax_construction_count();

    let parsed = crate::Engine::new()
        .parse_diagram_snapshot_sync("sequenceDiagram\nAlice->>Bob: Hello\n")
        .expect("Sequence combined parse succeeds")
        .expect("Sequence combined parse returns a diagram");

    assert!(matches!(
        parsed.editor_facts(),
        crate::ParsedEditorFacts::Available(_)
    ));
    assert_eq!(
        crate::diagrams::sequence::sequence_syntax_construction_count(),
        1,
        "one combined request must construct Sequence syntax once"
    );
}

#[test]
fn class_combined_parse_constructs_family_syntax_once() {
    crate::diagrams::class::reset_class_syntax_construction_count();

    let parsed = crate::Engine::new()
        .parse_diagram_snapshot_sync(
            "classDiagram-v2\nclass Customer\nCustomer --> Order : places\n",
        )
        .expect("Class combined parse succeeds")
        .expect("Class combined parse returns a diagram");

    assert_eq!(parsed.metadata().diagram_type, "classDiagram");
    assert!(matches!(
        parsed.editor_facts(),
        crate::ParsedEditorFacts::Available(_)
    ));
    assert_eq!(
        crate::diagrams::class::class_syntax_construction_count(),
        1,
        "one combined request must construct Class syntax once"
    );
}

fn assert_combined_projections_match_standalone(
    engine: &crate::Engine,
    family: &str,
    source: &str,
    volatile_top_level_json_field: Option<&str>,
) -> serde_json::Value {
    let standalone = engine
        .parse_diagram_sync(source, crate::ParseOptions::strict())
        .unwrap_or_else(|error| panic!("{family} standalone JSON failed: {error}"))
        .unwrap_or_else(|| panic!("{family} standalone JSON returned no diagram"));
    let standalone_editor = engine
        .parse_editor_semantic_facts_with_type_sync(family, source)
        .unwrap_or_else(|error| panic!("{family} standalone editor failed: {error}"))
        .unwrap_or_else(|| panic!("{family} standalone editor returned no facts"));
    let combined = engine
        .parse_diagram_snapshot_sync(source)
        .unwrap_or_else(|error| panic!("{family} combined parse failed: {error}"))
        .unwrap_or_else(|| panic!("{family} combined parse returned no diagram"));

    assert_eq!(standalone.meta.diagram_type, family);
    assert_eq!(combined.metadata().diagram_type, family);

    let mut standalone_model = standalone.model;
    let mut combined_model = combined
        .outcome()
        .parsed_model()
        .expect("expected parsed snapshot")
        .clone();
    if let Some(field) = volatile_top_level_json_field {
        let standalone_value = standalone_model
            .as_object_mut()
            .and_then(|model| model.remove(field))
            .unwrap_or_else(|| panic!("{family} standalone JSON omitted volatile field {field}"));
        let combined_value = combined_model
            .as_object_mut()
            .and_then(|model| model.remove(field))
            .unwrap_or_else(|| panic!("{family} combined JSON omitted volatile field {field}"));
        assert!(
            standalone_value.is_string() && combined_value.is_string(),
            "{family} volatile JSON field {field} must remain a string"
        );
    }
    assert_eq!(
        standalone_model, combined_model,
        "{family} JSON projection drift"
    );

    let crate::ParsedEditorFacts::Available(combined_editor) = combined.editor_facts() else {
        panic!("{family} combined parse returned unavailable editor facts");
    };
    assert_eq!(
        &standalone_editor, combined_editor,
        "{family} editor projection drift"
    );

    standalone_model
}

#[test]
fn langium_combined_projections_match_standalone_public_entrypoints() {
    let engine = crate::Engine::new();
    for (family, source) in [
        ("info", "info showInfo\n"),
        ("pie", "pie showData\ntitle Breakdown\n\"A\": 1\n\"B\": 2\n"),
        (
            "packet",
            "packet-beta\ntitle Header\n0-7: \"A\"\n8-15: \"B\"\n",
        ),
        (
            "cynefin",
            "cynefin-beta\ntitle Frame\ncomplex \"Probe\"\ncomplex --> clear : \"Move\"\n",
        ),
        (
            "radar",
            "radar-beta\ntitle Scores\naxis A,B\ncurve sample{1,2}\nticks 4\n",
        ),
        (
            "gitGraph",
            concat!(
                "gitGraph TB:\n",
                "title History\n",
                "accTitle: Accessible history\n",
                "commit id:\"duplicate\"\n",
                "commit id:\"duplicate\"\n",
                "branch later order: 2\n",
                "branch first order: 1\n",
            ),
        ),
    ] {
        let standalone_model =
            assert_combined_projections_match_standalone(&engine, family, source, None);

        if family == "gitGraph" {
            let warnings = standalone_model["warningFacts"].as_array().unwrap();
            assert_eq!(warnings.len(), 1, "gitGraph warning projection fixture");
            let branches = standalone_model["branches"]
                .as_array()
                .unwrap()
                .iter()
                .map(|branch| branch["name"].as_str().unwrap())
                .collect::<Vec<_>>();
            assert_eq!(branches, ["main", "first", "later"]);
            assert_eq!(standalone_model["direction"], "TB");
            assert_eq!(standalone_model["title"], "History");
            assert_eq!(standalone_model["accTitle"], "Accessible history");
        }
    }
}

#[test]
fn er_combined_projections_match_standalone_and_typed_public_entrypoints() {
    let engine = crate::Engine::new();
    let source = concat!(
        "erDiagram\r\n",
        "accTitle: <script>bad()</script><b>Entity map</b>\r\n",
        "accDescr { <script>bad()</script>first line\r\n",
        "  second line }\r\n",
        "direction LR\r\n",
        "CUSTOMER[Customer] {\r\n",
        "  string id PK, FK \"primary key\"\r\n",
        "  string name UK\r\n",
        "}\r\n",
        "ORDER[Order] {\r\n",
        "  string id PK\r\n",
        "}\r\n",
        "CUSTOMER ||--o{ ORDER : places\r\n",
        "CUSTOMER ||--|| CUSTOMER : refers\r\n",
        "classDef emphasized fill:#fff,color:red\r\n",
        "class CUSTOMER emphasized\r\n",
        "style ORDER stroke:#000\r\n",
    );
    let mut compat = assert_combined_projections_match_standalone(&engine, "er", source, None);

    let acc_title = compat["accTitle"].as_str().unwrap();
    let acc_descr = compat["accDescr"].as_str().unwrap();
    assert!(!acc_title.contains("<script>"));
    assert!(!acc_descr.contains("<script>"));
    assert!(acc_title.contains("Entity map"));
    assert!(acc_descr.contains("first line"));
    assert!(acc_descr.contains("second line"));
    assert_eq!(compat["direction"], "LR");
    assert_eq!(compat["entities"]["CUSTOMER"]["alias"], "Customer");
    assert_eq!(
        compat["entities"]["CUSTOMER"]["attributes"][0]["keys"],
        serde_json::json!(["PK", "FK"])
    );
    assert_eq!(compat["relationships"].as_array().unwrap().len(), 2);
    assert_eq!(compat["relationships"][0]["roleA"], "places");
    assert_eq!(compat["relationships"][1]["roleA"], "refers");
    assert!(
        compat.get("warningFacts").is_none(),
        "ER has no warning projection"
    );

    let typed = engine
        .parse_diagram_for_render_model_sync(source, crate::ParseOptions::strict())
        .expect("ER typed parse succeeds")
        .expect("ER typed parse returns a diagram");
    let crate::RenderSemanticModel::Er(typed) = typed.model() else {
        panic!("ER typed parse returned a different family");
    };
    let typed = serde_json::to_value(typed).expect("ER typed model serializes");
    compat.as_object_mut().unwrap().remove("type");
    compat.as_object_mut().unwrap().remove("constants");
    assert_eq!(compat, typed, "ER JSON and typed projections drifted");
}

#[test]
fn class_combined_projections_match_standalone_and_typed_public_entrypoints() {
    let engine = crate::Engine::new().with_site_config({
        let mut config = crate::MermaidConfig::empty_object();
        config.set_value("securityLevel", serde_json::json!("loose"));
        config
    });
    let source = concat!(
        "---\r\n",
        "config:\r\n",
        "  securityLevel: loose\r\n",
        "  class:\r\n",
        "    hierarchicalNamespaces: true\r\n",
        "---\r\n",
        "%%{init: {\"theme\": \"default\"}}%%\r\n",
        "classDiagram-v2\r\n",
        "accTitle: <script>bad()</script><b>Class map</b>\r\n",
        "accDescr: <script>bad()</script><b>Class relationships</b>\r\n",
        "direction LR\r\n",
        "namespace 公司.平台[\"Platform Layer\"] {\r\n",
        "  class 顧客[\"Customer\"] {\r\n",
        "    +id: String\r\n",
        "    +find(value) Result~T~$\r\n",
        "  }\r\n",
        "  note for 顧客 \"Primary customer\"\r\n",
        "}\r\n",
        "class 訂單\r\n",
        "顧客 \"1\" *-- \"many\" 訂單 : owns\r\n",
        "<<service>> 顧客\r\n",
        "note \"Floating note\"\r\n",
        "classDef service fill:#fff,color:red\r\n",
        "class 訂單:::service\r\n",
        "cssClass \"顧客,訂單\" service\r\n",
        "style 顧客 stroke:#000\r\n",
        "click 顧客 call open(customerId) \"Open customer\"\r\n",
        "link 訂單 \"https://example.com/orders\" \"Orders\" _blank\r\n",
        "callback 訂單 \"refreshOrders\" \"Refresh orders\"\r\n",
    );
    let compat =
        assert_combined_projections_match_standalone(&engine, "classDiagram", source, None);

    let typed = engine
        .parse_diagram_for_render_model_sync(source, crate::ParseOptions::strict())
        .expect("Class typed parse succeeds")
        .expect("Class typed parse returns a diagram");
    let crate::RenderSemanticModel::Class(typed) = typed.model() else {
        panic!("Class typed parse returned a different family");
    };
    let typed = serde_json::to_value(typed).expect("Class typed model serializes");
    assert_eq!(compat, typed, "Class JSON and typed projections drifted");

    assert_eq!(typed["direction"], "LR");
    assert_eq!(typed["classes"]["顧客"]["label"], "Customer");
    assert_eq!(typed["classes"]["顧客"]["parent"], "公司.平台");
    assert_eq!(typed["relations"][0]["relationTitle1"], "1");
    assert_eq!(typed["relations"][0]["relationTitle2"], "many");
    assert_eq!(typed["relations"][0]["title"], "owns");
    assert_eq!(typed["notes"].as_array().unwrap().len(), 2);
    assert_eq!(typed["notes"][0]["parent"], "公司.平台");
    assert!(
        typed["notes"][1].get("parent").is_none(),
        "floating Class notes preserve Mermaid's absent parent field"
    );
    assert_eq!(typed["classes"]["顧客"]["callback"]["function"], "open");
    assert_eq!(typed["classes"]["顧客"]["callbackEffective"], true);
    assert_eq!(typed["classes"]["訂單"]["linkTarget"], "_blank");
    for field in ["accTitle", "accDescr"] {
        assert!(!typed[field].as_str().unwrap().contains("<script>"));
    }
}

#[test]
fn sequence_combined_projections_match_standalone_and_typed_public_entrypoints() {
    let engine = crate::Engine::new();
    let source = concat!(
        "---\r\n",
        "config:\r\n",
        "  sequence:\r\n",
        "    wrap: true\r\n",
        "---\r\n",
        "%%{init: {\"theme\": \"default\"}}%%\r\n",
        "sequenceDiagram\r\n",
        "title: <script>bad()</script><b>Unicode exchange</b>\r\n",
        "accTitle: <script>bad()</script><b>Accessible sequence</b>\r\n",
        "accDescr: <script>bad()</script><b>Ordered interactions</b>\r\n",
        "box rgb(34, 56, 0) Team; participant 顧客 as Customer; actor サーバー as API; end\r\n",
        "autonumber 3 2; 顧客->>+サーバー: 開始; Note over 顧客,サーバー: 確認; サーバー-->>-顧客: 完了\r\n",
        "links 顧客: { \"Portal\": \"https://example.com/\" }\r\n",
        "properties サーバー: { \"class\": \"internal\" }\r\n",
        "loop [again]; 顧客->>サーバー: 繰り返す; end\r\n",
    );
    let mut compat =
        assert_combined_projections_match_standalone(&engine, "sequence", source, None);

    let typed = engine
        .parse_diagram_for_render_model_sync(source, crate::ParseOptions::strict())
        .expect("Sequence typed parse succeeds")
        .expect("Sequence typed parse returns a diagram");
    let crate::RenderSemanticModel::Sequence(typed) = typed.model() else {
        panic!("Sequence typed parse returned a different family");
    };
    let typed = serde_json::to_value(typed).expect("Sequence typed model serializes");
    compat.as_object_mut().unwrap().remove("type");
    compat.as_object_mut().unwrap().remove("constants");
    assert_eq!(compat, typed, "Sequence JSON and typed projections drifted");

    for field in ["title", "accTitle", "accDescr"] {
        let value = typed[field].as_str().expect("sanitized common field");
        assert!(!value.contains("<script>"), "unsanitized Sequence {field}");
    }
    assert_eq!(typed["actorOrder"], serde_json::json!(["顧客", "サーバー"]));
    assert_eq!(
        typed["boxes"][0]["actorKeys"],
        serde_json::json!(["顧客", "サーバー"])
    );
    assert_eq!(typed["messages"][0]["type"], 26);
    assert_eq!(typed["messages"][1]["message"], "開始");
    assert_eq!(typed["messages"][2]["message"], "");
    assert_eq!(typed["messages"][3]["message"], "確認");
    assert_eq!(typed["messages"][4]["message"], "完了");

    let editor = engine
        .parse_editor_semantic_facts_with_type_sync("sequence", source)
        .expect("Sequence editor parse succeeds")
        .expect("Sequence editor parse returns facts");
    for (name, detail) in [
        ("Customer", "sequence participant label"),
        ("API", "sequence participant label"),
        ("Team", "sequence box"),
        ("開始", "sequence message"),
        ("確認", "sequence note"),
        ("[again]", "sequence fragment label"),
        ("繰り返す", "sequence message"),
    ] {
        assert!(
            editor.symbols.iter().any(|symbol| {
                symbol.name == name
                    && symbol.detail.as_deref() == Some(detail)
                    && symbol.role == crate::EditorSemanticRole::Payload
            }),
            "missing Sequence payload {name:?} ({detail})"
        );
    }
}

#[cfg(feature = "full")]
#[test]
fn mindmap_combined_parse_constructs_family_syntax_once() {
    crate::diagrams::mindmap::reset_mindmap_syntax_construction_count();

    let parsed = crate::Engine::new()
        .parse_diagram_snapshot_sync("mindmap\n  root\n    child\n")
        .expect("mindmap combined parse succeeds")
        .expect("mindmap combined parse returns a diagram");

    assert!(matches!(
        parsed.editor_facts(),
        crate::ParsedEditorFacts::Available(_)
    ));
    assert_eq!(
        crate::diagrams::mindmap::mindmap_syntax_construction_count(),
        1,
        "one combined request must construct Mindmap syntax once"
    );
}

#[cfg(feature = "full")]
#[test]
fn mindmap_combined_projections_match_standalone_public_entrypoints() {
    let source = concat!(
        "mindmap root(Root Node)\n",
        "  child1(Child 1)\n",
        "  :::hot\n",
        "  ::icon(bomb)\n",
        "  child2\n",
    );
    let model = assert_combined_projections_match_standalone(
        &crate::Engine::new(),
        "mindmap",
        source,
        Some("diagramId"),
    );

    assert_eq!(model["rootNode"]["descr"], "Root Node");
    assert_eq!(model["rootNode"]["children"].as_array().unwrap().len(), 2);
}

#[test]
fn railroad_combined_parse_constructs_family_syntax_once_for_every_dialect() {
    for source in [
        "railroad-beta\nrule = terminal(\"a\") ;\n",
        "railroad-ebnf-beta\nrule = \"a\" ;\n",
        "railroad-abnf-beta\nrule = \"a\" ;\n",
        "railroad-peg-beta\nrule <- \"a\" ;\n",
    ] {
        crate::diagrams::railroad::reset_railroad_syntax_construction_count();

        let parsed = crate::Engine::new()
            .parse_diagram_snapshot_sync(source)
            .expect("railroad combined parse succeeds")
            .expect("railroad combined parse returns a diagram");

        assert!(matches!(
            parsed.editor_facts(),
            crate::ParsedEditorFacts::Available(_)
        ));
        assert_eq!(
            crate::diagrams::railroad::railroad_syntax_construction_count(),
            1,
            "one combined request must construct Railroad syntax once for {source:?}"
        );
    }
}

#[test]
fn railroad_combined_projections_match_standalone_public_entrypoints() {
    let engine = crate::Engine::new();
    for (family, source) in [
        (
            "railroad",
            "railroad-beta\ntitle \"Grammar\"\nrule = sequence(terminal(\"a\"), nonterminal(\"next\")) ;\n",
        ),
        (
            "railroadEbnf",
            "railroad-ebnf-beta\nrule ::= \"a\" | [ next ] ;\n",
        ),
        (
            "railroadAbnf",
            "railroad-abnf-beta\nrule = 1*2\"a\" / [ next ] ;\n",
        ),
        (
            "railroadPeg",
            "railroad-peg-beta\nrule <- &\"a\" !\"b\" . next? ;\n",
        ),
    ] {
        let model = assert_combined_projections_match_standalone(&engine, family, source, None);
        assert_eq!(model["rules"][0]["name"], "rule");
    }
}

#[test]
fn sankey_combined_parse_constructs_family_syntax_once() {
    crate::diagrams::sankey::reset_sankey_syntax_construction_count();

    let parsed = crate::Engine::new()
        .parse_diagram_snapshot_sync("sankey-beta\nA,B,1\n")
        .expect("sankey combined parse succeeds")
        .expect("sankey combined parse returns a diagram");

    assert!(matches!(
        parsed.editor_facts(),
        crate::ParsedEditorFacts::Available(_)
    ));
    assert_eq!(
        crate::diagrams::sankey::sankey_syntax_construction_count(),
        1,
        "one combined request must construct Sankey syntax once"
    );
}

#[test]
fn sankey_combined_projections_match_standalone_public_entrypoints() {
    let source = concat!(
        "sankey-beta\n",
        "\"Source, Inc.\",\"Target \"\"quoted\"\"\",1.5\n",
        "Target,Done,2\n",
    );
    let model =
        assert_combined_projections_match_standalone(&crate::Engine::new(), "sankey", source, None);

    assert_eq!(model["graph"]["links"][0]["source"], "Source, Inc.");
    assert_eq!(model["graph"]["links"][0]["target"], "Target \"quoted\"");
}

#[test]
fn every_admitted_semantic_variant_has_editor_facts_except_pinned_exceptions() {
    let expected_semantic_without_editor = PINNED_SEMANTIC_WITHOUT_EDITOR
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let expected_without_semantics = PINNED_WITHOUT_SEMANTICS
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();

    for profile in [BaselineRegistryProfile::Full, BaselineRegistryProfile::Tiny] {
        let capabilities = crate::diagram_family_capabilities_for_profile(profile);
        let semantic_without_editor = capabilities
            .iter()
            .filter_map(|fact| {
                (fact.has_semantic_parser && !fact.has_editor_parser).then_some(fact.diagram_type)
            })
            .collect::<BTreeSet<_>>();
        let without_semantics = capabilities
            .iter()
            .filter_map(|fact| (!fact.has_semantic_parser).then_some(fact.diagram_type))
            .collect::<BTreeSet<_>>();

        assert_eq!(
            semantic_without_editor, expected_semantic_without_editor,
            "{profile:?} introduced an undeclared semantic/editor capability gap"
        );
        assert_eq!(
            without_semantics, expected_without_semantics,
            "{profile:?} introduced an undeclared semantic admission gap"
        );

        for fact in capabilities {
            if fact.has_semantic_parser
                && !expected_semantic_without_editor.contains(fact.diagram_type)
            {
                assert!(
                    fact.has_editor_parser,
                    "{} must declare parser-backed editor facts in {profile:?}",
                    fact.diagram_type
                );
            }
        }
    }
}

#[test]
fn diagram_type_family_kind_maps_parser_ids_to_shared_family_kind() {
    assert_eq!(
        crate::diagram_type_family_kind("flowchart-v2"),
        Some("flowchart")
    );
    assert_eq!(
        crate::diagram_type_family_kind("flowchart"),
        Some("flowchart")
    );
    assert_eq!(
        crate::diagram_type_family_kind("flowchart-elk"),
        Some("flowchart")
    );
    assert_eq!(
        crate::diagram_type_family_kind("classDiagram"),
        Some("class")
    );
    assert_eq!(crate::diagram_type_family_kind("unknown"), None);
}

#[test]
fn tiny_parser_projection_excludes_full_only_large_features() {
    let tiny_semantic = DiagramRegistry::pinned_mermaid_baseline_tiny();
    assert!(tiny_semantic.get("mindmap").is_none());
    assert!(tiny_semantic.get("architecture").is_none());
    assert!(tiny_semantic.get("flowchart-elk").is_none());
    assert!(tiny_semantic.get("flowchart-v2").is_some());
    assert!(tiny_semantic.get("flowchart").is_some());

    let tiny_render = RenderDiagramRegistry::pinned_mermaid_baseline_tiny();
    assert!(!tiny_render.contains("mindmap"));
    assert!(!tiny_render.contains("architecture"));
    assert!(!tiny_render.contains("flowchart-elk"));
    assert!(tiny_render.contains("flowchart-v2"));
    assert!(tiny_render.contains("flowchart"));
}

#[cfg(not(feature = "full-registry"))]
#[test]
fn tiny_engine_rejects_full_only_known_type_parsers() {
    let engine = crate::Engine::new();

    for (expected_type, source) in [
        ("mindmap", "mindmap\nroot\n"),
        (
            "architecture",
            "architecture-beta\n  service a(server)[A]\n",
        ),
        ("flowchart-elk", "flowchart-elk TD\nA-->B;\n"),
    ] {
        let err = engine
            .parse_diagram_with_type_sync(expected_type, source, crate::ParseOptions::strict())
            .unwrap_err();
        let crate::Error::UnsupportedDiagram { diagram_type } = &err else {
            panic!("unexpected error for {expected_type}: {err}");
        };
        assert_eq!(diagram_type, expected_type);

        let err = engine
            .parse_diagram_for_render_model_with_type_sync(
                expected_type,
                source,
                crate::ParseOptions::strict(),
            )
            .unwrap_err();
        let crate::Error::UnsupportedDiagram { diagram_type } = &err else {
            panic!("unexpected render error for {expected_type}: {err}");
        };
        assert_eq!(diagram_type, expected_type);

        let err = engine
            .parse_editor_semantic_facts_with_type_sync(expected_type, source)
            .unwrap_err();
        let crate::Error::UnsupportedDiagram { diagram_type } = &err else {
            panic!("unexpected editor facts error for {expected_type}: {err}");
        };
        assert_eq!(diagram_type, expected_type);
    }
}

#[test]
fn pinned_non_error_semantic_parsers_are_backed_by_typed_render_parsers() {
    for profile in [BaselineRegistryProfile::Full, BaselineRegistryProfile::Tiny] {
        let render_ids = sorted_set(
            crate::family::render_parser_facts(profile)
                .iter()
                .map(|fact| fact.id),
        );

        for fact in crate::family::semantic_parser_facts(profile) {
            assert!(
                render_ids.contains(fact.id),
                "built-in semantic parser {} must not rely on JSON render fallback in {profile:?}",
                fact.id
            );
        }
    }
}

#[cfg(feature = "full")]
#[test]
fn failed_editor_snapshot_runs_one_preprocess_and_one_family_construction() {
    let source = concat!(
        "%%{ initialize: {\"theme\": \"dark\"} }%%\n",
        "mindmap\n",
        " root\n",
        "  broken[unterminated\n",
        "  after\n",
    );
    crate::preprocess::reset_public_parse_preprocess_count();
    crate::diagrams::mindmap::reset_mindmap_syntax_construction_count();

    let snapshot = crate::Engine::new()
        .parse_diagram_snapshot_sync(source)
        .expect("snapshot operation")
        .expect("detected mindmap");

    assert_eq!(snapshot.metadata().diagram_type, "mindmap");
    assert!(matches!(
        snapshot.outcome(),
        crate::DiagramParseOutcome::Failed(_)
    ));
    let crate::ParsedEditorFacts::Available(facts) = snapshot.editor_facts() else {
        panic!("mindmap snapshot must retain parser-backed recovery facts");
    };
    assert_eq!(
        facts.completeness,
        crate::EditorSemanticCompleteness::Recovered
    );
    assert!(
        facts
            .directive_prefixes
            .iter()
            .any(|prefix| prefix == "initialize")
    );
    assert_eq!(crate::preprocess::public_parse_preprocess_count(), 1);
    assert_eq!(
        crate::diagrams::mindmap::mindmap_syntax_construction_count(),
        1
    );
}

fn sorted_set(ids: impl IntoIterator<Item = &'static str>) -> BTreeSet<&'static str> {
    ids.into_iter().collect()
}

fn family_capability(
    capabilities: &'static [crate::DiagramFamilyCapability],
    diagram_type: &str,
) -> &'static crate::DiagramFamilyCapability {
    capabilities
        .iter()
        .find(|fact| fact.diagram_type == diagram_type)
        .unwrap_or_else(|| panic!("missing family capability for {diagram_type}"))
}
