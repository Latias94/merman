use merman_analysis::{
    AnalysisOptions, AnalysisRuleConfig, AnalysisRuleProfile, Analyzer, FenceTextIndexSource,
};

#[test]
fn representative_diagnostics_still_match_rich_generation_projection() {
    let cases = [
        (
            AnalysisOptions::default().with_rule_config(
                AnalysisRuleConfig::default().with_profile(AnalysisRuleProfile::Recommended),
            ),
            "flowchart\nA[Hello] --> B[World]\n",
        ),
        (AnalysisOptions::default(), "flowchart TD\nA[unterminated\n"),
        (
            AnalysisOptions::default(),
            "before\n```mermaid\ncynefin-beta\n  complex\n  complicated\n  complicated --> complicated : \"Self-loop\"\n```\nafter\n",
        ),
    ];

    for (options, source) in cases {
        let analyzer = Analyzer::with_options(options);
        let rich = analyzer
            .analyze_generation(source)
            .into_ready()
            .expect("characterization sources stay within the analysis limit");
        assert_eq!(
            analyzer.analyze(source),
            rich.project(analyzer.options().diagnostic_policy()),
            "diagnostic projection drifted for {source:?}"
        );
    }
}

#[test]
fn facts_schema2_keeps_generic_parser_semantics_without_flowchart_graph() {
    let facts = Analyzer::new().analyze_facts("flowchart TD\nA-->B\n");
    assert_eq!(facts.version, 2);

    let diagram = &facts.diagrams[0];
    assert_eq!(
        diagram.syntax.fact_source,
        FenceTextIndexSource::ParserComplete
    );
    assert!(diagram.syntax.node_ids.iter().any(|id| id == "A"));
    assert!(diagram.syntax.node_ids.iter().any(|id| id == "B"));
    let value = serde_json::to_value(facts).expect("facts should serialize");
    assert!(value["diagrams"][0]["syntax"].get("flowchart").is_none());
}
