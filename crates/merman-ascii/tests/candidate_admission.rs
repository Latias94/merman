mod support;

use std::fs;
use std::path::Path;

use merman_ascii::{
    AsciiError, AsciiPrimaryProjection, AsciiRenderOptions, AsciiSupportLevel, ascii_capabilities,
    ascii_diagrammatic_diagram_types, ascii_supported_diagram_types,
};
use merman_core::{Engine, ParseOptions};
use support::render_model;

struct RejectedCandidate {
    capability_type: &'static str,
    render_kind: &'static str,
    fixture: &'static str,
}

const REJECTED_CANDIDATES: &[RejectedCandidate] = &[
    RejectedCandidate {
        capability_type: "railroad",
        render_kind: "railroad",
        fixture: "fixtures/railroad/upstream_cypress_railroad_spec_renders_sequences_and_choices_002.mmd",
    },
    RejectedCandidate {
        capability_type: "requirement",
        render_kind: "requirement",
        fixture: "fixtures/requirement/relations.mmd",
    },
    RejectedCandidate {
        capability_type: "ishikawa",
        render_kind: "ishikawa",
        fixture: "fixtures/ishikawa/upstream_cypress_ishikawa_spec_1_should_render_a_simple_ishikawa_diagram_001.mmd",
    },
    RejectedCandidate {
        capability_type: "quadrantchart",
        render_kind: "quadrantChart",
        fixture: "fixtures/quadrantchart/upstream_cypress_quadrantchart_spec_should_render_a_complete_quadrant_chart_002.mmd",
    },
];

#[test]
fn unsupported_candidates_match_capability_catalog_and_dispatch() {
    let workspace_root = workspace_root();
    let engine = Engine::new();
    let supported = ascii_supported_diagram_types();
    let diagrammatic = ascii_diagrammatic_diagram_types();

    for candidate in REJECTED_CANDIDATES {
        let capability = ascii_capabilities()
            .iter()
            .find(|capability| capability.diagram_type == candidate.capability_type)
            .unwrap_or_else(|| panic!("missing capability for {}", candidate.capability_type));
        assert_eq!(capability.semantic_coverage, None);
        assert_eq!(capability.primary_projection, AsciiPrimaryProjection::None);
        assert_eq!(capability.support_level, AsciiSupportLevel::Unsupported);
        assert!(!supported.contains(&candidate.capability_type));
        assert!(!diagrammatic.contains(&candidate.capability_type));

        let source = read_fixture(&workspace_root, candidate.fixture);
        let parsed = engine
            .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
            .unwrap_or_else(|error| panic!("failed to parse {}: {error}", candidate.fixture))
            .unwrap_or_else(|| panic!("failed to detect {}", candidate.fixture));
        assert_eq!(parsed.model().kind(), candidate.render_kind);

        let error = render_model(parsed.model(), &AsciiRenderOptions::ascii())
            .expect_err("rejected candidates must remain unsupported without admission");
        assert_eq!(
            error,
            AsciiError::UnsupportedDiagram {
                diagram_type: candidate.render_kind.to_string(),
            }
        );
    }
}

fn workspace_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read_fixture(workspace_root: &Path, fixture: &str) -> String {
    let path = workspace_root.join(fixture);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}
