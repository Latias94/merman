use crate::baseline::BaselineRegistryProfile;
use crate::{DetectorRegistry, DiagramRegistry, MermaidConfig, RenderDiagramRegistry};
use std::collections::BTreeSet;

const PINNED_SEMANTIC_WITHOUT_EDITOR: &[&str] = &["error"];
const PINNED_WITHOUT_SEMANTICS: &[&str] = &["wardley"];

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

const STANDARD_CAPABILITIES: CharacterizedCapabilities = CharacterizedCapabilities {
    semantic: true,
    editor: true,
    combined: false,
    typed: true,
};
const COMBINED_CAPABILITIES: CharacterizedCapabilities = CharacterizedCapabilities {
    semantic: true,
    editor: true,
    combined: true,
    typed: true,
};
const EDITOR_ONLY_RENDER_GAP_CAPABILITIES: CharacterizedCapabilities = CharacterizedCapabilities {
    semantic: true,
    editor: true,
    combined: true,
    typed: false,
};
const ERROR_CAPABILITIES: CharacterizedCapabilities = CharacterizedCapabilities {
    semantic: true,
    editor: false,
    combined: false,
    typed: false,
};
const UNSUPPORTED_CAPABILITIES: CharacterizedCapabilities = CharacterizedCapabilities {
    semantic: false,
    editor: false,
    combined: false,
    typed: false,
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
        capabilities: EDITOR_ONLY_RENDER_GAP_CAPABILITIES,
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
        capabilities: STANDARD_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "sequence",
        logical_family: "sequence",
        profile: CharacterizationProfile::All,
        representative_source: "sequenceDiagram\nAlice->>Bob: Hello\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: STANDARD_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "c4",
        logical_family: "c4",
        profile: CharacterizationProfile::All,
        representative_source: "C4Context\nPerson(user, \"User\")\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: STANDARD_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "kanban",
        logical_family: "kanban",
        profile: CharacterizationProfile::All,
        representative_source: "kanban\n  Todo\n    item1\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: STANDARD_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "classDiagram",
        logical_family: "class",
        profile: CharacterizationProfile::All,
        representative_source: "classDiagram\nclass Animal\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: STANDARD_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "class",
        logical_family: "class",
        profile: CharacterizationProfile::All,
        representative_source: "classDiagram\nclass Animal\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: STANDARD_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "er",
        logical_family: "er",
        profile: CharacterizationProfile::All,
        representative_source: "erDiagram\nCUSTOMER\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: STANDARD_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "erDiagram",
        logical_family: "er",
        profile: CharacterizationProfile::All,
        representative_source: "erDiagram\nCUSTOMER\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: STANDARD_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "gantt",
        logical_family: "gantt",
        profile: CharacterizationProfile::All,
        representative_source: "gantt\ndateFormat YYYY-MM-DD\nsection Work\nTask :a, 2024-01-01, 1d\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: STANDARD_CAPABILITIES,
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
        capabilities: STANDARD_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "timeline",
        logical_family: "timeline",
        profile: CharacterizationProfile::All,
        representative_source: "timeline\n2024 : Event\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: STANDARD_CAPABILITIES,
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
        capabilities: STANDARD_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "state",
        logical_family: "state",
        profile: CharacterizationProfile::All,
        representative_source: "stateDiagram\n[*] --> Idle\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: STANDARD_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "journey",
        logical_family: "journey",
        profile: CharacterizationProfile::All,
        representative_source: "journey\nsection Work\nTask: 5\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: STANDARD_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "quadrantChart",
        logical_family: "quadrantChart",
        profile: CharacterizationProfile::All,
        representative_source: "quadrantChart\nx-axis Low --> High\ny-axis Low --> High\nA: [0.5, 0.5]\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: STANDARD_CAPABILITIES,
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
        capabilities: STANDARD_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "block",
        logical_family: "block",
        profile: CharacterizationProfile::All,
        representative_source: "block\n  a b c\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: STANDARD_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "eventmodeling",
        logical_family: "eventmodeling",
        profile: CharacterizationProfile::All,
        representative_source: "eventmodeling\ntf 01 ui Shop.Cart\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: STANDARD_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "treeView",
        logical_family: "treeView",
        profile: CharacterizationProfile::All,
        representative_source: "treeView-beta\n  root\n    child\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: STANDARD_CAPABILITIES,
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
        capabilities: STANDARD_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "treemap",
        logical_family: "treemap",
        profile: CharacterizationProfile::All,
        representative_source: "treemap-beta\n\"Root\"\n  \"Child\": 1\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: STANDARD_CAPABILITIES,
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
        capabilities: STANDARD_CAPABILITIES,
        malformed_contract: MalformedContract::StrictRejectsEditorAvailable,
    },
    FamilyCharacterization {
        variant_id: "wardley",
        logical_family: "wardley",
        profile: CharacterizationProfile::All,
        representative_source: "wardley-beta\n",
        malformed_source: MALFORMED_SOURCE,
        capabilities: UNSUPPORTED_CAPABILITIES,
        malformed_contract: MalformedContract::Unsupported,
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
fn selected_supported_diagrams_follow_feature_profile() {
    assert_eq!(
        crate::supported_diagrams(),
        crate::supported_diagrams_for_profile(crate::selected_baseline_registry_profile())
    );

    #[cfg(feature = "full")]
    assert_eq!(
        crate::supported_diagrams(),
        crate::supported_diagrams_for_profile(BaselineRegistryProfile::Full)
    );

    #[cfg(not(feature = "full"))]
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
            "flowchart",
            "gantt",
            "gitgraph",
            "info",
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
            "timeline",
            "treemap",
            "venn",
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
            "flowchart",
            "gantt",
            "gitgraph",
            "info",
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
            "timeline",
            "treemap",
            "venn",
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
    assert_eq!(tree_view.metadata_id, None);
    assert!(tree_view.has_semantic_parser);
    assert!(tree_view.has_render_parser);

    let error = family_capability(full, "error");
    assert_eq!(error.metadata_id, None);
    assert!(error.has_semantic_parser);
    assert!(!error.has_render_parser);

    let swimlane = family_capability(full, "swimlane");
    assert_eq!(swimlane.metadata_id, None);
    assert!(swimlane.has_semantic_parser);
    assert!(!swimlane.has_render_parser);

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
    assert_eq!(wardley.metadata_id, None);
    assert!(!wardley.has_semantic_parser);
    assert!(!wardley.has_render_parser);

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
        let editor_ids = sorted_set(
            crate::family::editor_parser_facts(profile)
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
                editor_ids.contains(id),
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
                        crate::ParseOptions::strict(),
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
            assert_eq!(parsed.meta.diagram_type, row.variant_id);
        }

        if row.capabilities.combined {
            let parsed = engine
                .parse_diagram_with_editor_facts_sync(
                    row.representative_source,
                    crate::ParseOptions::strict(),
                )
                .unwrap_or_else(|err| {
                    panic!(
                        "{} representative combined parse failed: {err}",
                        row.variant_id
                    )
                })
                .unwrap_or_else(|| panic!("{} returned no combined model", row.variant_id));
            assert_eq!(
                crate::diagram_type_family_kind(&parsed.diagram.meta.diagram_type),
                Some(row.logical_family),
                "{} combined detection left its logical family",
                row.variant_id
            );
            assert!(matches!(
                parsed.editor_facts,
                crate::ParsedEditorFacts::Available(_)
            ));
        }

        let malformed_semantic = engine.parse_diagram_with_type_sync(
            row.variant_id,
            row.malformed_source,
            crate::ParseOptions::strict(),
        );
        let malformed_editor = engine.parse_editor_semantic_facts_with_type_sync(
            row.variant_id,
            row.malformed_source,
            crate::ParseOptions::strict(),
        );
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
}

#[test]
fn catalog_declares_alias_ownership_and_capability_gaps_without_inheritance() {
    let full = crate::diagram_family_capabilities_for_profile(BaselineRegistryProfile::Full);

    let zenuml = family_capability(full, "zenuml");
    assert_eq!(zenuml.logical_family_kind, "zenuml");
    assert_eq!(zenuml.render_model_kind, Some("sequence"));

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
    assert!(!swimlane.has_render_parser);
    assert_eq!(swimlane.render_model_kind, None);
    assert_eq!(swimlane.metadata_id, None);

    let er_alias = family_capability(full, "erDiagram");
    assert_eq!(er_alias.logical_family_kind, "er");
    assert!(!er_alias.has_detector);
    assert!(!er_alias.has_header);
    assert!(
        er_alias.has_semantic_parser && er_alias.has_editor_parser && er_alias.has_render_parser
    );

    let error = family_capability(full, "error");
    assert!(error.has_detector && error.has_semantic_parser);
    assert!(!error.has_editor_parser && !error.has_combined_parser && !error.has_render_parser);

    let wardley = family_capability(full, "wardley");
    assert!(wardley.has_detector && wardley.has_header);
    assert!(
        !wardley.has_semantic_parser
            && !wardley.has_editor_parser
            && !wardley.has_combined_parser
            && !wardley.has_render_parser
    );

    let combined = full
        .iter()
        .filter_map(|fact| fact.has_combined_parser.then_some(fact.diagram_type))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        combined,
        BTreeSet::from([
            "architecture",
            "flowchart",
            "flowchart-elk",
            "flowchart-v2",
            "gitGraph",
            "info",
            "mindmap",
            "packet",
            "pie",
            "radar",
            "railroad",
            "railroadAbnf",
            "railroadEbnf",
            "railroadPeg",
            "sankey",
            "swimlane",
            "cynefin",
        ])
    );
}

#[test]
fn langium_family_combined_parse_constructs_syntax_once() {
    for (family, source) in [
        ("info", "info\n"),
        ("pie", "pie\n\"A\": 1\n"),
        ("packet", "packet-beta\n0-7: \"A\"\n"),
        ("cynefin", "cynefin-beta\n  complex\n"),
        ("radar", "radar-beta\naxis A,B,C\ncurve sample{1,2,3}\n"),
    ] {
        crate::diagrams::langium_common::reset_family_syntax_construction_count(family);

        let parsed = crate::Engine::new()
            .parse_diagram_with_editor_facts_sync(source, crate::ParseOptions::strict())
            .unwrap_or_else(|error| panic!("{family} combined parse failed: {error}"))
            .unwrap_or_else(|| panic!("{family} combined parse returned no diagram"));

        assert!(matches!(
            parsed.editor_facts,
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
        .parse_diagram_with_editor_facts_sync("gitGraph\ncommit\n", crate::ParseOptions::strict())
        .expect("gitGraph combined parse succeeds")
        .expect("gitGraph combined parse returns a diagram");

    assert!(matches!(
        parsed.editor_facts,
        crate::ParsedEditorFacts::Available(_)
    ));
    assert_eq!(
        crate::diagrams::langium_common::family_syntax_construction_count(family),
        1,
        "one combined request must construct gitGraph syntax once"
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
        .parse_editor_semantic_facts_with_type_sync(family, source, crate::ParseOptions::strict())
        .unwrap_or_else(|error| panic!("{family} standalone editor failed: {error}"))
        .unwrap_or_else(|| panic!("{family} standalone editor returned no facts"));
    let combined = engine
        .parse_diagram_with_editor_facts_sync(source, crate::ParseOptions::strict())
        .unwrap_or_else(|error| panic!("{family} combined parse failed: {error}"))
        .unwrap_or_else(|| panic!("{family} combined parse returned no diagram"));

    assert_eq!(standalone.meta.diagram_type, family);
    assert_eq!(combined.diagram.meta.diagram_type, family);

    let mut standalone_model = standalone.model;
    let mut combined_model = combined.diagram.model;
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

    let crate::ParsedEditorFacts::Available(combined_editor) = combined.editor_facts else {
        panic!("{family} combined parse returned unavailable editor facts");
    };
    assert_eq!(
        standalone_editor, combined_editor,
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

#[cfg(feature = "full")]
#[test]
fn mindmap_combined_parse_constructs_family_syntax_once() {
    crate::diagrams::mindmap::reset_mindmap_syntax_construction_count();

    let parsed = crate::Engine::new()
        .parse_diagram_with_editor_facts_sync(
            "mindmap\n  root\n    child\n",
            crate::ParseOptions::strict(),
        )
        .expect("mindmap combined parse succeeds")
        .expect("mindmap combined parse returns a diagram");

    assert!(matches!(
        parsed.editor_facts,
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
            .parse_diagram_with_editor_facts_sync(source, crate::ParseOptions::strict())
            .expect("railroad combined parse succeeds")
            .expect("railroad combined parse returns a diagram");

        assert!(matches!(
            parsed.editor_facts,
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
        .parse_diagram_with_editor_facts_sync("sankey-beta\nA,B,1\n", crate::ParseOptions::strict())
        .expect("sankey combined parse succeeds")
        .expect("sankey combined parse returns a diagram");

    assert!(matches!(
        parsed.editor_facts,
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
    assert!(tiny_render.get("mindmap").is_none());
    assert!(tiny_render.get("architecture").is_none());
    assert!(tiny_render.get("flowchart-elk").is_none());
    assert!(tiny_render.get("flowchart-v2").is_some());
    assert!(tiny_render.get("flowchart").is_some());
}

#[cfg(not(feature = "full"))]
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
            .parse_editor_semantic_facts_with_type_sync(
                expected_type,
                source,
                crate::ParseOptions::strict(),
            )
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
            if permits_parser_only_semantic_fact(fact.id) {
                continue;
            }

            assert!(
                render_ids.contains(fact.id),
                "built-in semantic parser {} must not rely on JSON render fallback in {profile:?}",
                fact.id
            );
        }
    }
}

fn sorted_set(ids: impl IntoIterator<Item = &'static str>) -> BTreeSet<&'static str> {
    ids.into_iter().collect()
}

fn permits_parser_only_semantic_fact(id: &str) -> bool {
    matches!(
        id,
        "error"
            | "swimlane"
            | "cynefin"
            | "railroad"
            | "railroadEbnf"
            | "railroadAbnf"
            | "railroadPeg"
    )
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
