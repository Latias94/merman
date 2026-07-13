use crate::baseline::BaselineRegistryProfile;
use crate::{DetectorRegistry, DiagramRegistry, MermaidConfig, RenderDiagramRegistry};
use std::collections::BTreeSet;

#[test]
fn detector_registries_follow_family_fact_order() {
    let full = DetectorRegistry::pinned_mermaid_baseline_full();
    let full_actual: Vec<_> = full.detector_ids().collect();
    let full_expected: Vec<_> = crate::family::detector_facts(BaselineRegistryProfile::Full)
        .iter()
        .map(|fact| fact.id)
        .collect();
    assert_eq!(full_actual, full_expected);

    let tiny = DetectorRegistry::pinned_mermaid_baseline_tiny();
    let tiny_actual: Vec<_> = tiny.detector_ids().collect();
    let tiny_expected: Vec<_> = crate::family::detector_facts(BaselineRegistryProfile::Tiny)
        .iter()
        .map(|fact| fact.id)
        .collect();
    assert_eq!(tiny_actual, tiny_expected);
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
fn family_header_and_fast_keyword_projections_are_bidirectionally_aligned() {
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

        for keyword in crate::family::fast_detect_keyword_facts(profile) {
            assert!(
                detector_ids.contains(keyword.id),
                "fast keyword {} points to missing detector {} in {profile:?}",
                keyword.keyword,
                keyword.id,
            );
            assert!(
                semantic_ids.contains(keyword.id),
                "fast keyword {} points to missing semantic parser {} in {profile:?}",
                keyword.keyword,
                keyword.id,
            );
        }

        for header in headers {
            assert!(
                semantic_ids.contains(header.diagram_type),
                "header {} points to missing semantic parser {} in {profile:?}",
                header.label,
                header.diagram_type,
            );
        }

        for semantic in crate::family::semantic_parser_facts(profile) {
            let header_id = match semantic.header_policy {
                crate::family::HeaderPolicy::Required => semantic.id,
                crate::family::HeaderPolicy::Alias(canonical_id) => {
                    let canonical = crate::family::semantic_parser_facts(profile)
                        .iter()
                        .find(|candidate| candidate.id == canonical_id)
                        .unwrap_or_else(|| {
                            panic!(
                                "semantic parser alias {} points to missing canonical parser {canonical_id} in {profile:?}",
                                semantic.id,
                            )
                        });
                    assert_eq!(
                        canonical.header_policy,
                        crate::family::HeaderPolicy::Required,
                        "semantic parser alias {} must point to a required-header parser",
                        semantic.id,
                    );
                    assert!(
                        std::ptr::fn_addr_eq(semantic.parser, canonical.parser),
                        "semantic parser alias {} must reuse the canonical parser {canonical_id}",
                        semantic.id,
                    );
                    canonical_id
                }
                crate::family::HeaderPolicy::Internal => continue,
            };
            assert!(
                headers
                    .iter()
                    .any(|header| header.diagram_type == header_id),
                "semantic parser {} has no header declaration for {header_id} in {profile:?}",
                semantic.id,
            );
        }
    }

    for header in crate::family::declared_diagram_header_facts() {
        assert_eq!(
            header.full_only,
            !crate::family::diagram_type_supported_in_profile(
                BaselineRegistryProfile::Tiny,
                header.diagram_type,
            ),
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
