//! Per-diagram SVG compare commands.
use crate::XtaskError;
use crate::cmd::compare::{
    CompareRequest, CompareRunFailure, CompareRunResult, DiagnosticsPolicy, DiagramIdPolicy,
    DiagramVerificationFact, FixtureComparePolicy, FixtureReportPolicy, FixtureSkipPolicy,
    ParsePolicy, RenderProfile, SpecialistHook, run_canonical_svg_compare,
};

mod er;
mod flowchart;
mod gantt;

use er::{compare_er_args, compare_er_request};
use flowchart::{compare_flowchart_args, compare_flowchart_request};
use gantt::{compare_gantt_args, compare_gantt_request};
pub(crate) use gantt::{
    gantt_baseline_local_offset_minutes, gantt_calibrated_runtime_policy, gantt_compare_environment,
};

pub(crate) use flowchart::{audit_flowchart_elk_parity_coverage, check_flowchart_elk_parity};
macro_rules! verification_fact {
    (
        $diagram:literal, $command:literal, $title:literal, $mode:literal, $source:literal,
        $parse:ident, $profile:ident, $id:ident, $skip:ident, $compare:ident,
        $report:ident, $diagnostics:ident, $specialist:ident
    ) => {
        DiagramVerificationFact {
            diagram: $diagram,
            command: $command,
            report_title: $title,
            default_dom_mode: $mode,
            #[cfg(test)]
            representative_source: $source,
            parse_policy: ParsePolicy::$parse,
            render_profile: RenderProfile::$profile,
            diagram_id_policy: DiagramIdPolicy::$id,
            skip_policy: FixtureSkipPolicy::$skip,
            compare_policy: FixtureComparePolicy::$compare,
            report_policy: FixtureReportPolicy::$report,
            diagnostics: DiagnosticsPolicy::$diagnostics,
            specialist: SpecialistHook::$specialist,
        }
    };
}

pub(crate) const DIAGRAM_VERIFICATION_FACTS: &[DiagramVerificationFact] = &[
    verification_fact!(
        "er",
        "compare-er-svgs",
        "ER",
        "parity",
        "erDiagram\nCUSTOMER\n",
        SuppressErrors,
        Specialist,
        Specialist,
        None,
        Specialist,
        Specialist,
        Specialist,
        ErAdapter
    ),
    verification_fact!(
        "flowchart",
        "compare-flowchart-svgs",
        "Flowchart",
        "parity",
        "flowchart TD\nA-->B\n",
        Default,
        Specialist,
        Specialist,
        UpstreamBaseline,
        Specialist,
        Specialist,
        RootDelta,
        FlowchartAdapter
    ),
    verification_fact!(
        "state",
        "compare-state-svgs",
        "StateDiagram",
        "structure",
        "stateDiagram-v2\n[*] --> Idle\n",
        Default,
        Standard,
        SanitizedStem,
        UpstreamCompare,
        Dom,
        Summary,
        RootDelta,
        None
    ),
    verification_fact!(
        "class",
        "compare-class-svgs",
        "ClassDiagram",
        "parity",
        "classDiagram\nclass Animal\n",
        Default,
        HandDrawnSeed,
        SanitizedStem,
        UpstreamCompare,
        Dom,
        Summary,
        RootDelta,
        None
    ),
    verification_fact!(
        "sequence",
        "compare-sequence-svgs",
        "Sequence",
        "structure",
        "sequenceDiagram\nAlice->>Bob: Hello\n",
        SuppressErrors,
        SequenceMath,
        SanitizedStem,
        UpstreamBaseline,
        Dom,
        Summary,
        RootDelta,
        SequenceMath
    ),
    verification_fact!(
        "info",
        "compare-info-svgs",
        "Info",
        "parity",
        "info\n",
        Default,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        None,
        None
    ),
    verification_fact!(
        "error",
        "compare-error-svgs",
        "Error",
        "parity",
        "error\n",
        SuppressErrors,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        None,
        None
    ),
    verification_fact!(
        "pie",
        "compare-pie-svgs",
        "Pie",
        "structure",
        "pie\n\"A\": 1\n",
        Default,
        Standard,
        RawStem,
        None,
        Dom,
        Summary,
        None,
        None
    ),
    verification_fact!(
        "sankey",
        "compare-sankey-svgs",
        "Sankey",
        "parity-root",
        "sankey\nA,B,1\n",
        Default,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        None,
        None
    ),
    verification_fact!(
        "packet",
        "compare-packet-svgs",
        "Packet",
        "structure",
        "packet-beta\n0-7: \"A\"\n",
        Lenient,
        Standard,
        RawStem,
        None,
        Dom,
        Summary,
        None,
        None
    ),
    verification_fact!(
        "timeline",
        "compare-timeline-svgs",
        "Timeline",
        "structure",
        "timeline\n2024 : Event\n",
        Default,
        Standard,
        RawStem,
        None,
        Dom,
        Summary,
        RootDelta,
        None
    ),
    verification_fact!(
        "journey",
        "compare-journey-svgs",
        "Journey",
        "parity",
        "journey\nsection Work\nTask: 5\n",
        Default,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        RootDelta,
        None
    ),
    verification_fact!(
        "kanban",
        "compare-kanban-svgs",
        "Kanban",
        "structure",
        "kanban\n  Todo\n    item1\n",
        Default,
        Standard,
        RawStem,
        None,
        Dom,
        Summary,
        None,
        None
    ),
    verification_fact!(
        "gitgraph",
        "compare-gitgraph-svgs",
        "GitGraph",
        "parity",
        "gitGraph\ncommit\n",
        Default,
        GitGraphSeed,
        SanitizedStem,
        None,
        Dom,
        Summary,
        RootDelta,
        None
    ),
    verification_fact!(
        "gantt",
        "compare-gantt-svgs",
        "Gantt",
        "structure",
        "gantt\ndateFormat YYYY-MM-DD\nsection Work\nTask :a, 2024-01-01, 1d\n",
        Default,
        Specialist,
        Specialist,
        UpstreamBaseline,
        Specialist,
        Specialist,
        None,
        GanttAdapter
    ),
    verification_fact!(
        "c4",
        "compare-c4-svgs",
        "C4",
        "parity",
        "C4Context\nPerson(user, \"User\")\n",
        SuppressErrors,
        Standard,
        SanitizedStem,
        UpstreamBaseline,
        Dom,
        Summary,
        None,
        None
    ),
    verification_fact!(
        "block",
        "compare-block-svgs",
        "Block",
        "structure",
        "block\n  a b c\n",
        Default,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        None,
        None
    ),
    verification_fact!(
        "radar",
        "compare-radar-svgs",
        "Radar",
        "parity",
        "radar-beta\naxis A,B,C\ncurve sample{1,2,3}\n",
        SuppressErrors,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        None,
        None
    ),
    verification_fact!(
        "requirement",
        "compare-requirement-svgs",
        "Requirement",
        "parity",
        "requirementDiagram\nrequirement req1 {\n  id: 1\n  text: Test\n  risk: low\n  verifymethod: analysis\n}\n",
        Default,
        Standard,
        RawStem,
        None,
        DomAndRawSvgFallback,
        StatusLines,
        None,
        None
    ),
    verification_fact!(
        "mindmap",
        "compare-mindmap-svgs",
        "Mindmap",
        "parity",
        "mindmap\n  root\n    child\n",
        Default,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        RootDelta,
        None
    ),
    verification_fact!(
        "architecture",
        "compare-architecture-svgs",
        "Architecture",
        "parity",
        "architecture-beta\n  service api(server)[API]\n",
        Default,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        RootDelta,
        None
    ),
    verification_fact!(
        "quadrantchart",
        "compare-quadrantchart-svgs",
        "QuadrantChart",
        "parity",
        "quadrantChart\nx-axis Low --> High\ny-axis Low --> High\nA: [0.5, 0.5]\n",
        Default,
        Standard,
        RawStem,
        None,
        DomAndRawSvgFallback,
        StatusLines,
        None,
        None
    ),
    verification_fact!(
        "treemap",
        "compare-treemap-svgs",
        "Treemap",
        "parity",
        "treemap-beta\n\"Root\"\n  \"Child\": 1\n",
        SuppressErrors,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        None,
        None
    ),
    verification_fact!(
        "xychart",
        "compare-xychart-svgs",
        "XYChart",
        "parity",
        "xychart-beta\nline [10, 30, 20]\n",
        Default,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        None,
        None
    ),
    verification_fact!(
        "treeView",
        "compare-tree-view-svgs",
        "TreeView",
        "parity",
        "treeView-beta\n  root\n    child\n",
        SuppressErrors,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        None,
        None
    ),
    verification_fact!(
        "ishikawa",
        "compare-ishikawa-svgs",
        "Ishikawa",
        "parity",
        "ishikawa-beta\n  Effect\n    Cause\n",
        SuppressErrors,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        None,
        None
    ),
    verification_fact!(
        "eventmodeling",
        "compare-eventmodeling-svgs",
        "EventModeling",
        "parity",
        "eventmodeling\ntf 01 ui Shop.Cart\n",
        SuppressErrors,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        None,
        None
    ),
    verification_fact!(
        "venn",
        "compare-venn-svgs",
        "Venn",
        "parity",
        "venn-beta\nset Frontend\nset Backend\nunion Frontend,Backend[\"API\"]\n",
        SuppressErrors,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        None,
        None
    ),
    verification_fact!(
        "swimlane",
        "compare-swimlane-svgs",
        "Swimlane",
        "parity",
        "swimlane-beta LR\nsubgraph Team\nA[Start] --> B[Done]\nend\n",
        Default,
        Standard,
        SanitizedStem,
        UpstreamCompare,
        Dom,
        Summary,
        RootDelta,
        None
    ),
    verification_fact!(
        "cynefin",
        "compare-cynefin-svgs",
        "Cynefin",
        "parity",
        "cynefin-beta\n  complex\n",
        SuppressErrors,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        None,
        None
    ),
    verification_fact!(
        "wardley",
        "compare-wardley-svgs",
        "Wardley",
        "parity",
        "wardley-beta\ncomponent A [0.8, 0.2]\n",
        SuppressErrors,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        None,
        None
    ),
    verification_fact!(
        "railroad",
        "compare-railroad-svgs",
        "Railroad",
        "parity",
        "railroad-beta\nrule = terminal(\"a\") ;\n",
        SuppressErrors,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        None,
        None
    ),
    verification_fact!(
        "railroadEbnf",
        "compare-railroad-ebnf-svgs",
        "Railroad EBNF",
        "parity",
        "railroad-ebnf-beta\nrule = \"a\" ;\n",
        SuppressErrors,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        None,
        None
    ),
    verification_fact!(
        "railroadAbnf",
        "compare-railroad-abnf-svgs",
        "Railroad ABNF",
        "parity",
        "railroad-abnf-beta\nrule = \"a\" ;\n",
        SuppressErrors,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        None,
        None
    ),
    verification_fact!(
        "railroadPeg",
        "compare-railroad-peg-svgs",
        "Railroad PEG",
        "parity",
        "railroad-peg-beta\nrule <- \"a\" ;\n",
        SuppressErrors,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        None,
        None
    ),
];

pub(crate) fn compare_diagram_command(
    fact: DiagramVerificationFact,
    args: Vec<String>,
) -> Result<(), XtaskError> {
    match fact.specialist {
        SpecialistHook::FlowchartAdapter => compare_flowchart_args(fact, args),
        SpecialistHook::ErAdapter => compare_er_args(fact, args),
        SpecialistHook::GanttAdapter => compare_gantt_args(fact, args),
        SpecialistHook::None | SpecialistHook::SequenceMath => {
            let request = CompareRequest::parse_for_fact(args, fact)?;
            run_canonical_svg_compare(fact, request)
                .map(|_| ())
                .map_err(CompareRunFailure::into_error)
        }
    }
}

pub(crate) fn compare_diagram_request(diagram: &str, request: CompareRequest) -> CompareRunResult {
    let Some(fact) = diagram_verification_fact(diagram) else {
        return Err(CompareRunFailure::without_evidence(
            XtaskError::SvgCompareFailed(format!("unexpected diagram: {diagram}")),
        ));
    };

    match fact.specialist {
        SpecialistHook::FlowchartAdapter => compare_flowchart_request(*fact, request),
        SpecialistHook::ErAdapter => compare_er_request(*fact, request),
        SpecialistHook::GanttAdapter => compare_gantt_request(*fact, request),
        SpecialistHook::None | SpecialistHook::SequenceMath => {
            run_canonical_svg_compare(*fact, request)
        }
    }
}

pub(crate) fn diagram_verification_fact(diagram: &str) -> Option<&'static DiagramVerificationFact> {
    DIAGRAM_VERIFICATION_FACTS
        .iter()
        .find(|fact| fact.diagram == diagram)
}

pub(crate) fn diagram_verification_fact_for_command(
    command: &str,
) -> Option<&'static DiagramVerificationFact> {
    DIAGRAM_VERIFICATION_FACTS
        .iter()
        .find(|fact| fact.command == command)
}

pub(crate) fn diagram_supports_root_delta_report(diagram: &str) -> bool {
    diagram_verification_fact(diagram).is_some_and(|fact| fact.supports_root_report())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_and_specialist_commands_require_observed_operation_evidence() {
        let output_root = crate::cmd::target_root()
            .join("compare")
            .join("observed-operation-tests")
            .join(std::process::id().to_string());

        for (diagram, stem) in [
            ("info", "upstream_info_spec"),
            ("er", "basic"),
            ("flowchart", "basic"),
            ("gantt", "basic"),
        ] {
            let report_path = output_root.join(format!("{diagram}.md"));
            let request = CompareRequest {
                out_path: Some(report_path),
                filter: Some(stem.to_string()),
                check_dom: true,
                ..CompareRequest::default()
            };

            let evidence = compare_diagram_request(diagram, request)
                .unwrap_or_else(|error| panic!("{diagram}/{stem} compare failed: {error}"));

            let selected = evidence.selected_fixtures();
            assert!(selected > 0, "{diagram}/{stem}");
            assert_eq!(evidence.rendered_fixtures(), selected, "{diagram}/{stem}");
            assert_eq!(
                evidence.observed_operation_reports(),
                evidence.rendered_fixtures(),
                "{diagram}/{stem}"
            );
            assert_eq!(
                evidence.observed_measurement_routes(),
                evidence.observed_operation_reports() * 4,
                "{diagram}/{stem}"
            );
            assert_eq!(evidence.comparisons(), selected, "{diagram}/{stem}");
        }
    }

    #[test]
    fn filtered_canonical_compare_rejects_an_all_skipped_selection() {
        let report_path = crate::cmd::target_root()
            .join("compare")
            .join("evidence-gate-tests")
            .join(std::process::id().to_string())
            .join("sequence.md");
        let request = CompareRequest {
            out_path: Some(report_path.clone()),
            filter: Some("stress_end_keyword_016".to_string()),
            check_dom: true,
            ..CompareRequest::default()
        };

        let failure = compare_diagram_request("sequence", request)
            .expect_err("a filtered run with only an upstream skip has no comparison evidence");
        let message = failure.to_string();
        assert!(
            message.contains("no canonical typed render evidence for sequence"),
            "{message}"
        );
        assert!(
            message.contains(
                "--check-dom produced no raw/source SVG-DOM or SVG-byte comparison evidence for sequence"
            ),
            "{message}"
        );

        let report = std::fs::read_to_string(&report_path)
            .unwrap_or_else(|error| panic!("read {}: {error}", report_path.display()));
        assert!(report.contains("- Render operation: `not-observed`"));
        assert!(report.contains(
            "Evidence counts: selected=`1` rendered=`0` skipped=`1` operation-reports=`0` measurement-routes=`0` raw/source-SVG-DOM=`0` raw/source-SVG-bytes=`0`"
        ));
    }

    #[test]
    fn compare_adapter_registry_covers_primary_svg_matrix() {
        for diagram in crate::cmd::primary_svg_matrix_diagrams() {
            assert!(
                diagram_verification_fact(diagram).is_some(),
                "primary SVG matrix diagram {diagram} should have a verification fact"
            );
        }
    }

    #[test]
    fn compare_adapter_registry_is_one_to_one() {
        let mut diagrams = std::collections::BTreeSet::new();
        let mut commands = std::collections::BTreeSet::new();
        for fact in DIAGRAM_VERIFICATION_FACTS {
            assert!(
                diagrams.insert(fact.diagram),
                "duplicate verification fact for {}",
                fact.diagram
            );
            assert!(
                commands.insert(fact.command),
                "duplicate compare command {}",
                fact.command
            );
            assert_eq!(
                fact.render_path(),
                merman::render::RenderExecutionPath::HeadlessOperationTyped,
                "{} must verify the canonical typed operation",
                fact.diagram
            );
            assert_eq!(
                diagram_verification_fact_for_command(fact.command).map(|found| found.diagram),
                Some(fact.diagram),
                "{} must route through its verification fact",
                fact.command
            );
        }
        assert!(diagram_verification_fact_for_command("compare-unknown-svgs").is_none());
    }

    #[test]
    fn root_diagnostic_support_is_projected_from_verification_facts() {
        for fact in DIAGRAM_VERIFICATION_FACTS {
            assert_eq!(
                diagram_supports_root_delta_report(fact.diagram),
                fact.supports_root_report(),
                "root diagnostic support for {} must come from its verification fact",
                fact.diagram
            );
        }
        assert!(!diagram_supports_root_delta_report("unknown"));
    }

    #[test]
    fn specialist_facts_describe_their_effective_parse_and_diagnostic_policies() {
        let er = diagram_verification_fact("er").expect("ER verification fact");
        assert_eq!(er.parse_policy, ParsePolicy::SuppressErrors);

        let flowchart =
            diagram_verification_fact("flowchart").expect("Flowchart verification fact");
        assert_eq!(flowchart.diagnostics, DiagnosticsPolicy::RootDelta);

        let gantt = diagram_verification_fact("gantt").expect("Gantt verification fact");
        assert_eq!(gantt.diagnostics, DiagnosticsPolicy::None);
    }

    #[test]
    fn venn_adapter_is_available_for_primary_admission() {
        assert!(diagram_verification_fact("venn").is_some());
        assert!(
            crate::cmd::primary_svg_matrix_diagrams().any(|diagram| diagram == "venn"),
            "venn should be covered by compare-all after admission gates are green"
        );
    }

    #[test]
    fn error_adapter_is_available_for_typed_renderer_admission() {
        assert!(diagram_verification_fact("error").is_some());
        assert!(
            crate::cmd::primary_svg_matrix_diagrams().any(|diagram| diagram == "error"),
            "error already has typed semantic, layout, and SVG rendering and must not remain parse-only"
        );
    }

    #[test]
    fn mermaid_11_16_new_family_adapters_are_available_for_admission() {
        for diagram in [
            "cynefin",
            "wardley",
            "railroad",
            "railroadEbnf",
            "railroadAbnf",
            "railroadPeg",
        ] {
            assert!(
                diagram_verification_fact(diagram).is_some(),
                "{diagram} should have a verification fact before primary admission"
            );
        }
    }
}
