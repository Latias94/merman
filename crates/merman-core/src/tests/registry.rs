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
fn supported_diagram_metadata_is_backed_by_typed_render_projection() {
    assert_eq!(
        crate::supported_diagrams_for_profile(BaselineRegistryProfile::Full),
        &[
            "architecture",
            "block",
            "c4",
            "class",
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
fn diagram_family_capabilities_follow_parser_fact_projection() {
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

    assert!(full.iter().any(|fact| fact.diagram_type == "mindmap"));
    assert!(!tiny.iter().any(|fact| fact.diagram_type == "mindmap"));
    assert!(!tiny.iter().any(|fact| fact.diagram_type == "architecture"));
    assert!(!tiny.iter().any(|fact| fact.diagram_type == "flowchart-elk"));
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
            if fact.id == "error" {
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

fn family_capability(
    capabilities: &'static [crate::DiagramFamilyCapability],
    diagram_type: &str,
) -> &'static crate::DiagramFamilyCapability {
    capabilities
        .iter()
        .find(|fact| fact.diagram_type == diagram_type)
        .unwrap_or_else(|| panic!("missing family capability for {diagram_type}"))
}
