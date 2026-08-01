use merman_analysis::{
    AnalysisOptions, AnalysisRuleConfig, AnalysisRuleProfile, Analyzer, DiagnosticSeverity,
    SourceDescriptor, analyze_document, analyze_document_facts,
    source_descriptor_for_markdown_path,
};
use serde_json::{Value, json};

const BASELINE_COMMIT: &str = "0be1a409286f044f40954aec686bd316ff78cb16";
// This fixture was emitted by the public analysis APIs at BASELINE_COMMIT before U1 replaced
// the retained evidence path. Never regenerate it from the current implementation.
const ORACLE: &str = include_str!("fixtures/pre_generation_analysis_oracle.json");

struct CaseSpec {
    name: &'static str,
    source: &'static str,
    document: bool,
}

const CASES: &[CaseSpec] = &[
    CaseSpec {
        name: "recommended_flowchart_fix",
        source: "flowchart\nA[Hello] --> B[World]\n",
        document: false,
    },
    CaseSpec {
        name: "recovered_flowchart_default",
        source: "flowchart TD\nA[unterminated\n",
        document: false,
    },
    CaseSpec {
        name: "recovered_flowchart_warning_override",
        source: "flowchart TD\nA[unterminated\n",
        document: false,
    },
    CaseSpec {
        name: "markdown_recovered_editor_facts",
        source: concat!(
            "before\n",
            "```mermaid\n",
            "cynefin-beta\n",
            "  complex\n",
            "  complicated\n",
            "  complicated --> complicated : \"Self-loop\"\n",
            "```\n",
            "after\n",
        ),
        document: true,
    },
];

fn analyzer_for(case_name: &str) -> Analyzer {
    match case_name {
        "recommended_flowchart_fix" => {
            Analyzer::with_options(AnalysisOptions::default().with_rule_config(
                AnalysisRuleConfig::default().with_profile(AnalysisRuleProfile::Recommended),
            ))
        }
        "recovered_flowchart_warning_override" => Analyzer::with_options(
            AnalysisOptions::default().with_rule_config(
                AnalysisRuleConfig::default()
                    .with_rule_severity("merman.parse.diagram_parse", DiagnosticSeverity::Warning)
                    .expect("parse rule severity override"),
            ),
        ),
        "recovered_flowchart_default" | "markdown_recovered_editor_facts" => Analyzer::new(),
        unknown => panic!("unknown compatibility oracle case {unknown}"),
    }
}

fn diagram_case(name: &str, source: &str, analyzer: &Analyzer) -> Value {
    json!({
        "name": name,
        "source": source,
        "diagnostics": analyzer.analyze(source),
        "facts": analyzer.analyze_facts(source),
    })
}

fn document_case(
    name: &str,
    source: &str,
    analyzer: &Analyzer,
    descriptor: SourceDescriptor,
) -> Value {
    json!({
        "name": name,
        "source": source,
        "diagnostics": analyze_document(source, analyzer, descriptor.clone()),
        "facts": analyze_document_facts(source, analyzer, descriptor),
    })
}

#[test]
fn generation_refactor_preserves_pre_u1_analysis_json() {
    let oracle: Value = serde_json::from_str(ORACLE).expect("valid compatibility oracle JSON");
    assert_eq!(oracle["oracle_version"], 1);
    assert_eq!(oracle["baseline_commit"], BASELINE_COMMIT);

    let expected_cases = oracle["cases"]
        .as_array()
        .expect("compatibility oracle cases");
    assert_eq!(
        expected_cases.len(),
        CASES.len(),
        "the baseline fixture must contain every fixed compatibility case"
    );
    let actual_cases = CASES
        .iter()
        .map(|case| {
            let analyzer = analyzer_for(case.name);
            if case.document {
                document_case(
                    case.name,
                    case.source,
                    &analyzer,
                    source_descriptor_for_markdown_path(Some("compatibility-oracle.md")),
                )
            } else {
                diagram_case(case.name, case.source, &analyzer)
            }
        })
        .collect::<Vec<_>>();

    assert_eq!(actual_cases, *expected_cases);
}
