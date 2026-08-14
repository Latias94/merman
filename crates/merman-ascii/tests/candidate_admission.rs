#[path = "candidate_admission/evaluator.rs"]
mod evaluator;
mod support;

use std::fs;
use std::path::Path;

use evaluator::{CandidateKind, PrototypeObservation, Scenario, evaluate};
use merman_ascii::{
    AsciiError, AsciiPrimaryProjection, AsciiRenderOptions, AsciiSupportLevel, ascii_capabilities,
    ascii_diagrammatic_diagram_types, ascii_supported_diagram_types,
};
use merman_core::{Engine, ParseOptions};
use support::render_model;

#[derive(Clone, Copy)]
struct CandidateFixture {
    scenario: Scenario,
    path: &'static str,
}

struct RejectedCandidate {
    kind: CandidateKind,
    capability_type: &'static str,
    render_kind: &'static str,
    report_anchor: &'static str,
    fixtures: [CandidateFixture; 3],
}

const REJECTED_CANDIDATES: &[RejectedCandidate] = &[
    RejectedCandidate {
        kind: CandidateKind::Railroad,
        capability_type: "railroad",
        render_kind: "railroad",
        report_anchor: "railroad-r34",
        fixtures: [
            CandidateFixture {
                scenario: Scenario::Small,
                path: "fixtures/railroad/upstream_cypress_railroad_spec_renders_a_simple_rule_001.mmd",
            },
            CandidateFixture {
                scenario: Scenario::Typical,
                path: "fixtures/railroad/upstream_cypress_railroad_spec_renders_sequences_and_choices_002.mmd",
            },
            CandidateFixture {
                scenario: Scenario::Dense,
                path: "fixtures/railroad/upstream_cypress_railroad_spec_renders_multiple_rules_in_one_diagram_004.mmd",
            },
        ],
    },
    RejectedCandidate {
        kind: CandidateKind::Requirement,
        capability_type: "requirement",
        render_kind: "requirement",
        report_anchor: "requirement-r34",
        fixtures: [
            CandidateFixture {
                scenario: Scenario::Small,
                path: "fixtures/requirement/basic.mmd",
            },
            CandidateFixture {
                scenario: Scenario::Typical,
                path: "fixtures/requirement/relations.mmd",
            },
            CandidateFixture {
                scenario: Scenario::Dense,
                path: "fixtures/requirement/upstream_docs_requirementdiagram_larger_example_010.mmd",
            },
        ],
    },
    RejectedCandidate {
        kind: CandidateKind::Ishikawa,
        capability_type: "ishikawa",
        render_kind: "ishikawa",
        report_anchor: "ishikawa-r34",
        fixtures: [
            CandidateFixture {
                scenario: Scenario::Small,
                path: "fixtures/ishikawa/upstream_cypress_ishikawa_spec_4_should_render_with_a_single_cause_004.mmd",
            },
            CandidateFixture {
                scenario: Scenario::Typical,
                path: "fixtures/ishikawa/upstream_cypress_ishikawa_spec_1_should_render_a_simple_ishikawa_diagram_001.mmd",
            },
            CandidateFixture {
                scenario: Scenario::Dense,
                path: "fixtures/ishikawa/upstream_cypress_ishikawa_spec_3_should_render_with_deeply_nested_causes_003.mmd",
            },
        ],
    },
    RejectedCandidate {
        kind: CandidateKind::Quadrant,
        capability_type: "quadrantchart",
        render_kind: "quadrantChart",
        report_anchor: "quadrant-r34",
        fixtures: [
            CandidateFixture {
                scenario: Scenario::Small,
                path: "fixtures/quadrantchart/stress_quadrantchart_batch1_boundaries_001.mmd",
            },
            CandidateFixture {
                scenario: Scenario::Typical,
                path: "fixtures/quadrantchart/upstream_cypress_quadrantchart_spec_should_render_a_complete_quadrant_chart_002.mmd",
            },
            CandidateFixture {
                scenario: Scenario::Dense,
                path: "fixtures/quadrantchart/stress_quadrantchart_batch1_dense_points_overlap_003.mmd",
            },
        ],
    },
];

#[test]
fn rejected_candidate_report_matches_runtime_capabilities_and_dispatch() {
    let workspace_root = workspace_root();
    let report = read_report(&workspace_root);
    let engine = Engine::new();

    for candidate in REJECTED_CANDIDATES {
        let capability = ascii_capabilities()
            .iter()
            .find(|capability| capability.diagram_type == candidate.capability_type)
            .unwrap_or_else(|| panic!("missing capability for {}", candidate.capability_type));
        assert_eq!(capability.semantic_coverage, None);
        assert_eq!(capability.primary_projection, AsciiPrimaryProjection::None);
        assert_eq!(capability.support_level, AsciiSupportLevel::Unsupported);
        assert!(!ascii_supported_diagram_types().contains(&candidate.capability_type));
        assert!(!ascii_diagrammatic_diagram_types().contains(&candidate.capability_type));
        assert!(
            report.contains(&format!(r#"id="{}""#, candidate.report_anchor)),
            "gate report must own the {} disposition",
            candidate.capability_type
        );

        for fixture in candidate.fixtures {
            let source = read_fixture(&workspace_root, fixture.path);
            assert!(
                report.contains(&format!("`{}`", fixture.path)),
                "gate report must cite representative fixture {}",
                fixture.path
            );
            let parsed = engine
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap_or_else(|error| panic!("failed to parse {}: {error}", fixture.path))
                .unwrap_or_else(|| panic!("failed to detect {}", fixture.path));
            assert_eq!(parsed.model().kind(), candidate.render_kind);

            let error = match render_model(parsed.model(), &AsciiRenderOptions::ascii()) {
                Ok(_) => panic!("{} must remain unsupported without admission", fixture.path),
                Err(error) => error,
            };
            assert_eq!(
                error,
                AsciiError::UnsupportedDiagram {
                    diagram_type: candidate.render_kind.to_string(),
                }
            );
        }
    }
}

#[test]
fn candidate_prototypes_supply_real_width_and_information_gain_evidence() {
    let workspace_root = workspace_root();
    let report = read_report(&workspace_root);
    let engine = Engine::new();

    for candidate in REJECTED_CANDIDATES {
        let output_fragment = match candidate.kind {
            CandidateKind::Railroad => "loop[0..*]",
            CandidateKind::Requirement => "(repeat)",
            CandidateKind::Ishikawa => "[prototype omits 4 deeper edge(s)]",
            CandidateKind::Quadrant => "|-------------+**------------|",
        };
        assert!(
            report.contains(output_fragment),
            "gate report must retain an actual {} prototype output marker",
            candidate.capability_type
        );
        for scenario in Scenario::ALL {
            assert_eq!(
                candidate
                    .fixtures
                    .iter()
                    .filter(|fixture| fixture.scenario == scenario)
                    .count(),
                1,
                "each candidate must own one {} fixture",
                scenario.label()
            );
        }

        for fixture in candidate.fixtures {
            let source = read_fixture(&workspace_root, fixture.path);
            let parsed = engine
                .parse_diagram_for_render_model_sync(&source, ParseOptions::strict())
                .unwrap_or_else(|error| panic!("failed to parse {}: {error}", fixture.path))
                .unwrap_or_else(|| panic!("failed to detect {}", fixture.path));

            for width in [80, 100, 120] {
                let observation = evaluate(candidate.kind, parsed.model(), width);
                assert!(!observation.output.is_empty());
                assert!(
                    observation.output.is_ascii(),
                    "the gate prototype must be inspectable in a 7-bit terminal"
                );
                assert!(
                    observation.max_line_width() <= width,
                    "{} {} output exceeds its {width}-column viewport",
                    candidate.capability_type,
                    fixture.scenario.label()
                );
                assert_eq!(
                    observation.structured_text_facts, observation.expected_spatial_facts,
                    "the comparison baseline must retain every evaluated fact"
                );
                assert_expected_observation(candidate.kind, fixture.scenario, &observation);
                assert!(
                    report.contains(&evidence_row(fixture.scenario, width, &observation)),
                    "gate report is missing derived evidence for {} {} at {width} columns",
                    candidate.capability_type,
                    fixture.scenario.label()
                );
            }
        }
    }
}

fn assert_expected_observation(
    kind: CandidateKind,
    scenario: Scenario,
    observation: &PrototypeObservation,
) {
    let (expected, recovered, topology, gain) = match (kind, scenario) {
        (CandidateKind::Railroad, Scenario::Small) => (0, 0, true, false),
        (CandidateKind::Railroad, Scenario::Typical) => (5, 3, false, false),
        (CandidateKind::Railroad, Scenario::Dense) => (7, 4, false, false),
        (CandidateKind::Requirement, Scenario::Small) => (0, 0, true, false),
        (CandidateKind::Requirement, Scenario::Typical) => (1, 1, true, false),
        (CandidateKind::Requirement, Scenario::Dense) => (8, 8, false, false),
        (CandidateKind::Ishikawa, Scenario::Small) => (1, 1, true, false),
        (CandidateKind::Ishikawa, Scenario::Typical) => (4, 4, true, true),
        (CandidateKind::Ishikawa, Scenario::Dense) => (9, 5, false, false),
        (CandidateKind::Quadrant, Scenario::Small) => (5, 5, true, true),
        (CandidateKind::Quadrant, Scenario::Typical) => (6, 6, true, true),
        (CandidateKind::Quadrant, Scenario::Dense) => (9, 2, false, false),
    };
    assert_eq!(observation.expected_spatial_facts, expected);
    assert_eq!(observation.recovered_spatial_facts, recovered);
    assert_eq!(observation.topology_recoverable, topology);
    assert_eq!(observation.information_gain, gain);
    assert_eq!(observation.clipped_lines, 0);
}

fn evidence_row(scenario: Scenario, width: usize, observation: &PrototypeObservation) -> String {
    format!(
        "| {} | {width} | {}/{} | {} | {} | {} |",
        scenario.label(),
        observation.recovered_spatial_facts,
        observation.expected_spatial_facts,
        yes_no(observation.topology_recoverable),
        yes_no(observation.information_gain),
        observation.diagnostic
    )
}

fn yes_no(value: bool) -> &'static str {
    if value { "Yes" } else { "No" }
}

fn workspace_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read_report(workspace_root: &Path) -> String {
    let report_path = workspace_root.join("docs/rendering/ASCII_PHASE_GATE_REPORT.md");
    fs::read_to_string(&report_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", report_path.display()))
}

fn read_fixture(workspace_root: &Path, fixture: &str) -> String {
    let path = workspace_root.join(fixture);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}
