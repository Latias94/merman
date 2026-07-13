//! Per-diagram SVG compare commands.
use crate::XtaskError;
#[cfg(test)]
use crate::cmd::compare::VerificationRenderPath;
use crate::cmd::compare::{
    CompareRequest, DiagnosticsPolicy, DiagramIdPolicy, DiagramVerificationFact,
    FixtureComparePolicy, FixtureReportPolicy, FixtureSkipPolicy, ParsePolicy, RenderProfile,
    SpecialistHook, run_canonical_svg_compare,
};

mod er;
mod flowchart;
mod gantt;

use er::{compare_er_args, compare_er_request};
use flowchart::{compare_flowchart_args, compare_flowchart_request};
use gantt::{compare_gantt_args, compare_gantt_request};

pub(crate) use er::compare_er_svgs;
pub(crate) use flowchart::{
    audit_flowchart_elk_source_backed_coverage, check_flowchart_elk_source_backed_probes,
    compare_flowchart_svgs,
};
pub(crate) use gantt::compare_gantt_svgs;
macro_rules! verification_fact {
    (
        $diagram:literal, $command:literal, $title:literal, $mode:literal,
        $parse:ident, $profile:ident, $id:ident, $skip:ident, $compare:ident,
        $report:ident, $diagnostics:ident, $specialist:ident, $root_overrides:literal
    ) => {
        DiagramVerificationFact {
            diagram: $diagram,
            command: $command,
            report_title: $title,
            default_dom_mode: $mode,
            parse_policy: ParsePolicy::$parse,
            render_profile: RenderProfile::$profile,
            diagram_id_policy: DiagramIdPolicy::$id,
            skip_policy: FixtureSkipPolicy::$skip,
            compare_policy: FixtureComparePolicy::$compare,
            report_policy: FixtureReportPolicy::$report,
            diagnostics: DiagnosticsPolicy::$diagnostics,
            specialist: SpecialistHook::$specialist,
            supports_root_overrides: $root_overrides,
        }
    };
}

pub(crate) const DIAGRAM_VERIFICATION_FACTS: &[DiagramVerificationFact] = &[
    verification_fact!(
        "er",
        "compare-er-svgs",
        "ER",
        "parity",
        SuppressErrors,
        Specialist,
        Specialist,
        None,
        Specialist,
        Specialist,
        Specialist,
        ErAdapter,
        false
    ),
    verification_fact!(
        "flowchart",
        "compare-flowchart-svgs",
        "Flowchart",
        "parity",
        Default,
        Specialist,
        Specialist,
        UpstreamBaseline,
        Specialist,
        Specialist,
        RootDelta,
        FlowchartAdapter,
        true
    ),
    verification_fact!(
        "state",
        "compare-state-svgs",
        "StateDiagram",
        "structure",
        Default,
        Standard,
        SanitizedStem,
        UpstreamCompare,
        Dom,
        Summary,
        RootDelta,
        None,
        false
    ),
    verification_fact!(
        "class",
        "compare-class-svgs",
        "ClassDiagram",
        "parity",
        Default,
        HandDrawnSeed,
        SanitizedStem,
        UpstreamCompare,
        Dom,
        Summary,
        RootDelta,
        ClassV2Role,
        false
    ),
    verification_fact!(
        "sequence",
        "compare-sequence-svgs",
        "Sequence",
        "structure",
        SuppressErrors,
        SequenceMath,
        SanitizedStem,
        UpstreamBaseline,
        Dom,
        Summary,
        RootDelta,
        SequenceMath,
        true
    ),
    verification_fact!(
        "info",
        "compare-info-svgs",
        "Info",
        "parity",
        Default,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        None,
        None,
        false
    ),
    verification_fact!(
        "pie",
        "compare-pie-svgs",
        "Pie",
        "structure",
        Default,
        Standard,
        RawStem,
        None,
        Dom,
        Summary,
        None,
        None,
        false
    ),
    verification_fact!(
        "sankey",
        "compare-sankey-svgs",
        "Sankey",
        "parity-root",
        Default,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        None,
        None,
        false
    ),
    verification_fact!(
        "packet",
        "compare-packet-svgs",
        "Packet",
        "structure",
        Lenient,
        Standard,
        RawStem,
        None,
        Dom,
        Summary,
        None,
        None,
        false
    ),
    verification_fact!(
        "timeline",
        "compare-timeline-svgs",
        "Timeline",
        "structure",
        Default,
        Standard,
        RawStem,
        None,
        Dom,
        Summary,
        RootDelta,
        None,
        false
    ),
    verification_fact!(
        "journey",
        "compare-journey-svgs",
        "Journey",
        "parity",
        Default,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        RootDelta,
        None,
        false
    ),
    verification_fact!(
        "kanban",
        "compare-kanban-svgs",
        "Kanban",
        "structure",
        Default,
        Standard,
        RawStem,
        None,
        Dom,
        Summary,
        None,
        None,
        false
    ),
    verification_fact!(
        "gitgraph",
        "compare-gitgraph-svgs",
        "GitGraph",
        "parity",
        Default,
        GitGraphSeed,
        SanitizedStem,
        None,
        Dom,
        Summary,
        RootDelta,
        None,
        false
    ),
    verification_fact!(
        "gantt",
        "compare-gantt-svgs",
        "Gantt",
        "structure",
        Default,
        Specialist,
        Specialist,
        UpstreamBaseline,
        Specialist,
        Specialist,
        None,
        GanttAdapter,
        false
    ),
    verification_fact!(
        "c4",
        "compare-c4-svgs",
        "C4",
        "parity",
        SuppressErrors,
        Standard,
        SanitizedStem,
        UpstreamBaseline,
        Dom,
        Summary,
        None,
        None,
        false
    ),
    verification_fact!(
        "block",
        "compare-block-svgs",
        "Block",
        "structure",
        Default,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        None,
        None,
        false
    ),
    verification_fact!(
        "radar",
        "compare-radar-svgs",
        "Radar",
        "parity",
        SuppressErrors,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        None,
        None,
        false
    ),
    verification_fact!(
        "requirement",
        "compare-requirement-svgs",
        "Requirement",
        "parity",
        Default,
        Standard,
        RawStem,
        None,
        DomAndRawSvgFallback,
        StatusLines,
        None,
        None,
        false
    ),
    verification_fact!(
        "mindmap",
        "compare-mindmap-svgs",
        "Mindmap",
        "parity",
        Default,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        RootDelta,
        None,
        false
    ),
    verification_fact!(
        "architecture",
        "compare-architecture-svgs",
        "Architecture",
        "parity",
        Default,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        RootDelta,
        None,
        true
    ),
    verification_fact!(
        "quadrantchart",
        "compare-quadrantchart-svgs",
        "QuadrantChart",
        "parity",
        Default,
        Standard,
        RawStem,
        None,
        DomAndRawSvgFallback,
        StatusLines,
        None,
        None,
        false
    ),
    verification_fact!(
        "treemap",
        "compare-treemap-svgs",
        "Treemap",
        "parity",
        SuppressErrors,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        None,
        None,
        false
    ),
    verification_fact!(
        "xychart",
        "compare-xychart-svgs",
        "XYChart",
        "parity",
        Default,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        None,
        None,
        false
    ),
    verification_fact!(
        "treeView",
        "compare-tree-view-svgs",
        "TreeView",
        "parity",
        SuppressErrors,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        None,
        None,
        false
    ),
    verification_fact!(
        "ishikawa",
        "compare-ishikawa-svgs",
        "Ishikawa",
        "parity",
        SuppressErrors,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        None,
        None,
        false
    ),
    verification_fact!(
        "eventmodeling",
        "compare-eventmodeling-svgs",
        "EventModeling",
        "parity",
        SuppressErrors,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        None,
        None,
        false
    ),
    verification_fact!(
        "venn",
        "compare-venn-svgs",
        "Venn",
        "parity",
        SuppressErrors,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        None,
        None,
        false
    ),
    verification_fact!(
        "cynefin",
        "compare-cynefin-svgs",
        "Cynefin",
        "parity",
        SuppressErrors,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        None,
        None,
        false
    ),
    verification_fact!(
        "railroad",
        "compare-railroad-svgs",
        "Railroad",
        "parity",
        SuppressErrors,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        None,
        None,
        false
    ),
    verification_fact!(
        "railroadEbnf",
        "compare-railroad-ebnf-svgs",
        "Railroad EBNF",
        "parity",
        SuppressErrors,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        None,
        None,
        false
    ),
    verification_fact!(
        "railroadAbnf",
        "compare-railroad-abnf-svgs",
        "Railroad ABNF",
        "parity",
        SuppressErrors,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        None,
        None,
        false
    ),
    verification_fact!(
        "railroadPeg",
        "compare-railroad-peg-svgs",
        "Railroad PEG",
        "parity",
        SuppressErrors,
        Standard,
        SanitizedStem,
        None,
        Dom,
        Summary,
        None,
        None,
        false
    ),
];

pub(crate) fn compare_diagram_svgs(diagram: &str, args: Vec<String>) -> Result<(), XtaskError> {
    let Some(fact) = diagram_verification_fact(diagram) else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "unexpected diagram: {diagram}"
        )));
    };

    match fact.specialist {
        SpecialistHook::FlowchartAdapter => compare_flowchart_args(*fact, args),
        SpecialistHook::ErAdapter => compare_er_args(*fact, args),
        SpecialistHook::GanttAdapter => compare_gantt_args(*fact, args),
        SpecialistHook::None | SpecialistHook::ClassV2Role | SpecialistHook::SequenceMath => {
            let request = CompareRequest::parse_for_fact(args, *fact)?;
            run_canonical_svg_compare(*fact, request)
        }
    }
}

pub(crate) fn compare_diagram_request(
    diagram: &str,
    request: CompareRequest,
) -> Result<(), XtaskError> {
    let Some(fact) = diagram_verification_fact(diagram) else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "unexpected diagram: {diagram}"
        )));
    };

    match fact.specialist {
        SpecialistHook::FlowchartAdapter => compare_flowchart_request(*fact, request),
        SpecialistHook::ErAdapter => compare_er_request(*fact, request),
        SpecialistHook::GanttAdapter => compare_gantt_request(*fact, request),
        SpecialistHook::None | SpecialistHook::ClassV2Role | SpecialistHook::SequenceMath => {
            run_canonical_svg_compare(*fact, request)
        }
    }
}

pub(crate) fn diagram_verification_fact(diagram: &str) -> Option<&'static DiagramVerificationFact> {
    DIAGRAM_VERIFICATION_FACTS
        .iter()
        .find(|fact| fact.diagram == diagram)
}

pub(crate) fn diagram_supports_root_delta_report(diagram: &str) -> bool {
    diagram_verification_fact(diagram).is_some_and(|fact| fact.supports_root_report())
}

macro_rules! generic_compare_entrypoints {
    ($(($name:ident, $diagram:literal)),+ $(,)?) => {
        $(
            pub(crate) fn $name(args: Vec<String>) -> Result<(), XtaskError> {
                compare_diagram_svgs($diagram, args)
            }
        )+
    };
}

generic_compare_entrypoints!(
    (compare_state_svgs, "state"),
    (compare_class_svgs, "class"),
    (compare_sequence_svgs, "sequence"),
    (compare_info_svgs, "info"),
    (compare_pie_svgs, "pie"),
    (compare_sankey_svgs, "sankey"),
    (compare_packet_svgs, "packet"),
    (compare_timeline_svgs, "timeline"),
    (compare_journey_svgs, "journey"),
    (compare_kanban_svgs, "kanban"),
    (compare_gitgraph_svgs, "gitgraph"),
    (compare_c4_svgs, "c4"),
    (compare_block_svgs, "block"),
    (compare_radar_svgs, "radar"),
    (compare_requirement_svgs, "requirement"),
    (compare_mindmap_svgs, "mindmap"),
    (compare_architecture_svgs, "architecture"),
    (compare_quadrantchart_svgs, "quadrantchart"),
    (compare_treemap_svgs, "treemap"),
    (compare_xychart_svgs, "xychart"),
    (compare_tree_view_svgs, "treeView"),
    (compare_ishikawa_svgs, "ishikawa"),
    (compare_eventmodeling_svgs, "eventmodeling"),
    (compare_venn_svgs, "venn"),
    (compare_cynefin_svgs, "cynefin"),
    (compare_railroad_svgs, "railroad"),
    (compare_railroad_ebnf_svgs, "railroadEbnf"),
    (compare_railroad_abnf_svgs, "railroadAbnf"),
    (compare_railroad_peg_svgs, "railroadPeg"),
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare_adapters_do_not_rebuild_the_legacy_render_pipeline() {
        let adapters_dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/cmd/compare/diagrams");
        let forbidden = [
            ".parse_diagram(",
            "merman_render::layout_parsed(",
            "merman_render::svg::render_",
        ];
        let mut violations = Vec::new();

        for entry in std::fs::read_dir(&adapters_dir).expect("compare adapter directory") {
            let entry = entry.expect("compare adapter entry");
            let path = entry.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
                continue;
            }
            let source = std::fs::read_to_string(&path).expect("compare adapter source");
            for symbol in forbidden {
                if source.contains(symbol) {
                    violations.push(format!(
                        "{} still contains `{symbol}`",
                        path.file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("<unknown>")
                    ));
                }
            }
        }

        assert!(
            violations.is_empty(),
            "per-family compare adapters must use the canonical prepared operation:\n{}",
            violations.join("\n")
        );
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
                VerificationRenderPath::HeadlessOperationTyped,
                "{} must verify the canonical typed operation",
                fact.diagram
            );
        }
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
    fn mermaid_11_16_new_family_adapters_are_available_for_admission() {
        for diagram in [
            "cynefin",
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
