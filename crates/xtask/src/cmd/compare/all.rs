//! Compare all diagram SVGs under fixtures.

use crate::XtaskError;
use std::fs;
use std::path::{Path, PathBuf};

use super::diagrams::compare_diagram_request;
use super::{
    AcceptedResidualPolicy, CompareEvidence, CompareRequest, CompareRunResult,
    RootDeltaReportLimit, diagram_supports_root_delta_report, dom_mode_label,
    parse_root_delta_report_limit,
};

pub(crate) fn compare_all_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let options = CompareAllOptions::parse(args)?;
    let diagram_selection = CompareAllDiagramSelection::from_options(&options)?;
    compare_selected_diagram_svgs(options, diagram_selection)
}

fn compare_selected_diagram_svgs(
    options: CompareAllOptions,
    diagram_selection: CompareAllDiagramSelection,
) -> Result<(), XtaskError> {
    let compare_dir = crate::cmd::target_root().join("compare");
    fs::create_dir_all(&compare_dir).map_err(|source| XtaskError::WriteFile {
        path: compare_dir.display().to_string(),
        source,
    })?;
    let diagrams = diagram_selection.diagrams;

    let invocation_options = options.invocation_options();
    let mut failures = CompareAllFailures::new(&options);

    for diagram in diagrams {
        println!("\n== compare {diagram} ==");

        let DiagramCompareInvocation {
            request,
            report_path,
        } = invocation_options.for_diagram(diagram, &compare_dir);

        failures.record(
            diagram,
            compare_diagram_request(diagram, request),
            report_path.as_deref(),
        );
    }

    failures.finish()
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct CompareAllOptions {
    check_dom: bool,
    dom_mode: Option<String>,
    dom_modes: Vec<crate::svgdom::DomMode>,
    dom_decimals: Option<u32>,
    filter: Option<String>,
    report_root: bool,
    root_report_limit: Option<RootDeltaReportLimit>,
    only_diagrams: Vec<String>,
    skip_diagrams: Vec<String>,
}

impl CompareAllOptions {
    fn parse(args: Vec<String>) -> Result<Self, XtaskError> {
        let mut options = Self::default();

        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--check-dom" => options.check_dom = true,
                "--dom-mode" => {
                    i += 1;
                    let mode = args
                        .get(i)
                        .ok_or(XtaskError::Usage)?
                        .parse::<crate::svgdom::DomMode>()
                        .map_err(|_| XtaskError::Usage)?;
                    options.dom_mode = Some(mode.to_string());
                }
                "--dom-modes" => {
                    if !options.dom_modes.is_empty() {
                        return Err(XtaskError::Usage);
                    }
                    i += 1;
                    options.dom_modes = parse_dom_modes(args.get(i).map(String::as_str))?;
                }
                "--dom-decimals" => {
                    i += 1;
                    options.dom_decimals = args.get(i).and_then(|s| s.trim().parse::<u32>().ok());
                }
                "--filter" => {
                    i += 1;
                    options.filter = args.get(i).map(|s| s.to_string());
                }
                "--report-root" => options.report_root = true,
                "--report-root-all" => {
                    options.report_root = true;
                    options.root_report_limit = Some(RootDeltaReportLimit::All);
                }
                "--report-root-limit" => {
                    i += 1;
                    options.report_root = true;
                    options.root_report_limit = Some(parse_root_delta_report_limit(
                        args.get(i).map(String::as_str),
                    )?);
                }
                "--diagram" => {
                    i += 1;
                    let diagram = args.get(i).ok_or(XtaskError::Usage)?.trim().to_string();
                    if !diagram.is_empty() {
                        options.only_diagrams.push(diagram);
                    }
                }
                "--skip" => {
                    i += 1;
                    let diagram = args.get(i).ok_or(XtaskError::Usage)?.trim().to_string();
                    if !diagram.is_empty() {
                        options.skip_diagrams.push(diagram);
                    }
                }
                "--help" | "-h" => return Err(XtaskError::Usage),
                _ => return Err(XtaskError::Usage),
            }
            i += 1;
        }
        if options.dom_mode.is_some() && !options.dom_modes.is_empty() {
            return Err(XtaskError::Usage);
        }
        Ok(options)
    }

    fn invocation_options(&self) -> CompareAllInvocationOptions<'_> {
        CompareAllInvocationOptions {
            check_dom: self.check_dom,
            dom_mode: self.dom_mode.as_deref(),
            dom_modes: &self.dom_modes,
            dom_decimals: self.dom_decimals,
            filter: self.filter.as_deref(),
            report_root: self.report_root,
            root_report_limit: self.root_report_limit,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CompareAllDiagramSelection {
    diagrams: Vec<&'static str>,
}

impl CompareAllDiagramSelection {
    fn from_options(options: &CompareAllOptions) -> Result<Self, XtaskError> {
        let mut diagrams: Vec<&'static str> = crate::cmd::primary_svg_matrix_diagrams().collect();

        if !options.only_diagrams.is_empty() {
            let only: Vec<String> = options
                .only_diagrams
                .iter()
                .map(|s| diagram_filter_key(s))
                .collect();
            diagrams.retain(|d| only.iter().any(|o| o == &diagram_filter_key(d)));
        }

        if !options.skip_diagrams.is_empty() {
            let skip: Vec<String> = options
                .skip_diagrams
                .iter()
                .map(|s| diagram_filter_key(s))
                .collect();
            diagrams.retain(|d| !skip.iter().any(|s| s == &diagram_filter_key(d)));
        }

        if diagrams.is_empty() {
            return Err(XtaskError::Usage);
        }

        Ok(Self { diagrams })
    }
}

#[derive(Debug)]
struct CompareAllFailures {
    skip_unmatched_filter_messages: bool,
    check_dom: bool,
    evidence: CompareEvidence,
    failures: Vec<String>,
}

impl CompareAllFailures {
    fn new(options: &CompareAllOptions) -> Self {
        Self {
            skip_unmatched_filter_messages: options.filter.is_some()
                && options.only_diagrams.is_empty(),
            check_dom: options.check_dom,
            evidence: CompareEvidence::default(),
            failures: Vec::new(),
        }
    }

    fn record(&mut self, diagram: &str, result: CompareRunResult, report_path: Option<&Path>) {
        let error = match result {
            Ok(evidence) => {
                self.evidence += evidence;
                return;
            }
            Err(failure) => {
                self.evidence += failure.evidence();
                failure.into_error()
            }
        };
        match error {
            XtaskError::SvgCompareFailed(msg) if self.should_skip_unmatched_filter(&msg) => {
                println!("(skipped: {msg})");
            }
            XtaskError::SvgCompareFailed(msg) => {
                self.record_svg_compare_failure(diagram, &msg, report_path);
            }
            err => self.failures.push(format!("{diagram}: {err}")),
        }
    }

    fn finish(mut self) -> Result<(), XtaskError> {
        self.failures.extend(
            self.evidence
                .gate_failures("compare-all selected families", self.check_dom),
        );

        if self.failures.is_empty() {
            Ok(())
        } else {
            Err(XtaskError::SvgCompareFailed(self.failures.join("\n")))
        }
    }

    fn record_svg_compare_failure(&mut self, diagram: &str, msg: &str, report_path: Option<&Path>) {
        let report = report_path
            .map(|path| format!("\nreport={}", path.display()))
            .unwrap_or_default();
        self.failures.push(format!(
            "{diagram}: {}{report}",
            XtaskError::SvgCompareFailed(msg.to_string())
        ));
    }

    fn should_skip_unmatched_filter(&self, msg: &str) -> bool {
        self.skip_unmatched_filter_messages && msg.contains("no .mmd fixtures matched under ")
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct CompareAllInvocationOptions<'a> {
    check_dom: bool,
    dom_mode: Option<&'a str>,
    dom_modes: &'a [crate::svgdom::DomMode],
    dom_decimals: Option<u32>,
    filter: Option<&'a str>,
    report_root: bool,
    root_report_limit: Option<RootDeltaReportLimit>,
}

impl CompareAllInvocationOptions<'_> {
    fn for_diagram(&self, diagram: &str, compare_dir: &Path) -> DiagramCompareInvocation {
        let report_mode = if self.dom_modes.is_empty() {
            self.dom_mode.map(dom_mode_slug)
        } else if self.dom_modes.contains(&crate::svgdom::DomMode::ParityRoot) {
            Some("parity_root".to_string())
        } else {
            Some(
                self.dom_modes
                    .iter()
                    .map(|mode| dom_mode_slug(dom_mode_label(*mode)))
                    .collect::<Vec<_>>()
                    .join("_"),
            )
        };
        let report_path = report_mode.and_then(|mode| {
            (!mode.is_empty()).then(|| compare_dir.join(format!("{diagram}_report_{mode}.md")))
        });
        let supports_root_report = diagram_supports_root_delta_report(diagram);
        let request = CompareRequest {
            out_path: report_path.clone(),
            filter: self.filter.map(str::to_string),
            check_dom: self.check_dom,
            dom_mode: self.dom_mode.map(str::to_string),
            dom_modes: self.dom_modes.to_vec(),
            dom_decimals: self.dom_decimals,
            report_root: self.report_root && supports_root_report,
            root_report_limit: supports_root_report
                .then_some(self.root_report_limit)
                .flatten(),
            accepted_residual_policy: if matches!(diagram, "c4" | "class" | "ishikawa" | "venn") {
                AcceptedResidualPolicy::ScopedDomEvidenceCatalog
            } else {
                AcceptedResidualPolicy::None
            },
        };

        DiagramCompareInvocation {
            request,
            report_path,
        }
    }
}

fn parse_dom_modes(value: Option<&str>) -> Result<Vec<crate::svgdom::DomMode>, XtaskError> {
    let value = value.ok_or(XtaskError::Usage)?;
    let mut modes = Vec::new();
    for raw in value.split(',') {
        let mode = raw
            .parse::<crate::svgdom::DomMode>()
            .map_err(|_| XtaskError::Usage)?;
        if modes.contains(&mode) {
            return Err(XtaskError::Usage);
        }
        modes.push(mode);
    }
    if modes.is_empty() {
        Err(XtaskError::Usage)
    } else {
        Ok(modes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DiagramCompareInvocation {
    request: CompareRequest,
    report_path: Option<PathBuf>,
}

fn dom_mode_slug(mode: &str) -> String {
    let mut out = String::with_capacity(mode.len());
    let mut prev_underscore = false;
    for ch in mode.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            prev_underscore = false;
            out.push(ch.to_ascii_lowercase());
        } else {
            if prev_underscore {
                continue;
            }
            prev_underscore = true;
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}

fn diagram_filter_key(diagram: &str) -> String {
    match diagram.trim().to_ascii_lowercase().as_str() {
        "tree-view" | "treeview" => "treeview".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::compare::CompareRunFailure;

    #[test]
    fn diagram_filter_key_accepts_tree_view_aliases() {
        assert_eq!(diagram_filter_key("treeView"), "treeview");
        assert_eq!(diagram_filter_key("tree-view"), "treeview");
        assert_eq!(diagram_filter_key("eventmodeling"), "eventmodeling");
    }

    #[test]
    fn compare_all_options_parse_common_flags_with_deterministic_measurement() {
        let options = CompareAllOptions::parse(vec![
            "--check-dom".to_string(),
            "--dom-mode".to_string(),
            " parity-root ".to_string(),
            "--dom-decimals".to_string(),
            "nope".to_string(),
            "--filter".to_string(),
            "upstream_info_spec".to_string(),
            "--report-root-limit".to_string(),
            "7".to_string(),
            "--diagram".to_string(),
            "tree-view".to_string(),
            "--skip".to_string(),
            "er".to_string(),
        ])
        .expect("options should parse");

        assert!(options.check_dom);
        assert_eq!(options.dom_mode.as_deref(), Some("parity-root"));
        assert_eq!(options.dom_decimals, None);
        assert_eq!(options.filter.as_deref(), Some("upstream_info_spec"));
        assert!(options.report_root);
        assert_eq!(
            options.root_report_limit,
            Some(RootDeltaReportLimit::Top(7))
        );
        assert_eq!(options.only_diagrams, ["tree-view"]);
        assert_eq!(options.skip_diagrams, ["er"]);
    }

    #[test]
    fn compare_all_options_reject_missing_required_values_for_strict_flags() {
        assert!(CompareAllOptions::parse(vec!["--diagram".to_string()]).is_err());
        assert!(CompareAllOptions::parse(vec!["--skip".to_string()]).is_err());
        assert!(CompareAllOptions::parse(vec!["--report-root-limit".to_string()]).is_err());
        assert!(CompareAllOptions::parse(vec!["--dom-modes".to_string()]).is_err());
    }

    #[test]
    fn compare_all_options_parse_one_render_dom_suite() {
        let options = CompareAllOptions::parse(vec![
            "--check-dom".to_string(),
            "--dom-modes".to_string(),
            "structure, parity, parity-root".to_string(),
        ])
        .expect("DOM suite should parse");

        assert_eq!(
            options.dom_modes,
            [
                crate::svgdom::DomMode::Structure,
                crate::svgdom::DomMode::Parity,
                crate::svgdom::DomMode::ParityRoot,
            ]
        );
        assert!(options.dom_mode.is_none());
    }

    #[test]
    fn compare_all_options_reject_conflicting_or_ambiguous_dom_suites() {
        assert!(
            CompareAllOptions::parse(vec![
                "--dom-mode".to_string(),
                "parity".to_string(),
                "--dom-modes".to_string(),
                "structure,parity".to_string(),
            ])
            .is_err()
        );
        assert!(
            CompareAllOptions::parse(vec!["--dom-modes".to_string(), "parity,parity".to_string(),])
                .is_err()
        );
        assert!(
            CompareAllOptions::parse(vec![
                "--dom-modes".to_string(),
                "structure,unknown".to_string(),
            ])
            .is_err()
        );
        assert!(
            CompareAllOptions::parse(vec!["--dom-mode".to_string(), "unknown".to_string(),])
                .is_err()
        );
    }

    #[test]
    fn compare_all_root_mode_uses_the_blocking_contract_without_an_acceptance_policy() {
        let global = CompareAllOptions {
            check_dom: true,
            dom_mode: Some("parity_root".to_string()),
            ..Default::default()
        };
        assert_eq!(
            global
                .invocation_options()
                .for_diagram("flowchart", Path::new("target/compare"))
                .request
                .accepted_residual_policy,
            AcceptedResidualPolicy::None
        );

        let filtered = CompareAllOptions {
            filter: Some("smoke".to_string()),
            ..global
        };
        assert_eq!(
            filtered
                .invocation_options()
                .for_diagram("flowchart", Path::new("target/compare"))
                .request
                .accepted_residual_policy,
            AcceptedResidualPolicy::None
        );
    }

    #[test]
    fn structure_policy_metadata_is_family_specific() {
        let options = CompareAllOptions {
            check_dom: true,
            dom_mode: Some("structure".to_string()),
            ..Default::default()
        };
        let invocation = options.invocation_options();
        let compare_dir = Path::new("target/compare");

        assert_eq!(
            invocation
                .for_diagram("sequence", compare_dir)
                .request
                .accepted_residual_policy,
            AcceptedResidualPolicy::None
        );
        assert_eq!(
            invocation
                .for_diagram("quadrantchart", compare_dir)
                .request
                .accepted_residual_policy,
            AcceptedResidualPolicy::None
        );
        assert_eq!(
            invocation
                .for_diagram("info", compare_dir)
                .request
                .accepted_residual_policy,
            AcceptedResidualPolicy::None
        );
        assert_eq!(
            invocation
                .for_diagram("ishikawa", compare_dir)
                .request
                .accepted_residual_policy,
            AcceptedResidualPolicy::ScopedDomEvidenceCatalog
        );
        assert_eq!(
            invocation
                .for_diagram("c4", compare_dir)
                .request
                .accepted_residual_policy,
            AcceptedResidualPolicy::ScopedDomEvidenceCatalog
        );
        assert_eq!(
            invocation
                .for_diagram("class", compare_dir)
                .request
                .accepted_residual_policy,
            AcceptedResidualPolicy::ScopedDomEvidenceCatalog
        );
    }

    #[test]
    fn compare_all_failures_skip_unmatched_filter_only_for_global_filtered_runs() {
        let msg = "no .mmd fixtures matched under fixtures";
        let global_options = CompareAllOptions {
            filter: Some("missing".to_string()),
            ..Default::default()
        };
        let global = CompareAllFailures::new(&global_options);
        assert!(global.should_skip_unmatched_filter(msg));

        let targeted_options = CompareAllOptions {
            filter: Some("missing".to_string()),
            only_diagrams: vec!["info".to_string()],
            ..Default::default()
        };
        let targeted = CompareAllFailures::new(&targeted_options);
        assert!(!targeted.should_skip_unmatched_filter(msg));
    }

    #[test]
    fn compare_all_failures_records_plain_svg_compare_failures() {
        let options = CompareAllOptions::default();
        let mut failures = CompareAllFailures::new(&options);

        failures.record(
            "info",
            Err(CompareRunFailure::without_evidence(
                XtaskError::SvgCompareFailed("dom mismatch".to_string()),
            )),
            None,
        );

        assert_eq!(
            failures.failures,
            ["info: svg compare failed:\ndom mismatch"]
        );

        let mut failures = CompareAllFailures::new(&options);
        failures.record(
            "info",
            Err(CompareRunFailure::without_evidence(
                XtaskError::SvgCompareFailed("dom mismatch".to_string()),
            )),
            Some(Path::new("target/compare/info_structure.md")),
        );

        assert_eq!(
            failures.failures,
            ["info: svg compare failed:\ndom mismatch\nreport=target/compare/info_structure.md"]
        );
    }

    #[test]
    fn compare_all_failures_rejects_sequence_dom_mismatches() {
        let former_residuals = [
            (
                "upstream_cypress_sequencediagram_spec_should_render_long_notes_wrapped_inline_left_of_actor_026",
                "dom mismatch for upstream_cypress_sequencediagram_spec_should_render_long_notes_wrapped_inline_left_of_actor_026: upstream=a local=b (svg/g[16]: child count mismatch upstream=9 local=8)",
            ),
            (
                "upstream_cypress_sequencediagram_v2_spec_should_render_wrapped_long_notes_left_of_control_019",
                "dom mismatch for upstream_cypress_sequencediagram_v2_spec_should_render_wrapped_long_notes_left_of_control_019: upstream=a local=b (svg/g[20]: child count mismatch upstream=9 local=8)",
            ),
            (
                "upstream_docs_diagrams_mermaid_api_sequence",
                "dom mismatch for upstream_docs_diagrams_mermaid_api_sequence: upstream=a local=b (svg/g[61]/text[9]: attr `class` mismatch upstream=`sectionTitle` local=`loopText`; additional DOM differences (2): svg/g[61]/text[9]: child count mismatch upstream=0 local=1 | svg/g[61]: child count mismatch upstream=10 local=11)",
            ),
        ];

        for mode in ["structure", "parity"] {
            for (stem, msg) in former_residuals {
                let options = CompareAllOptions {
                    check_dom: true,
                    dom_mode: Some(mode.to_string()),
                    ..Default::default()
                };
                let mut failures = CompareAllFailures::new(&options);
                failures.record(
                    "sequence",
                    Err(CompareRunFailure::without_evidence(
                        XtaskError::SvgCompareFailed(msg.to_string()),
                    )),
                    Some(Path::new("target/compare/sequence_report.md")),
                );

                let error = failures
                    .finish()
                    .expect_err("former Sequence DOM residuals must remain actionable");
                assert!(error.to_string().contains(stem), "mode={mode}, stem={stem}");
            }
        }
    }

    #[test]
    fn compare_all_rejects_a_filter_whose_only_match_is_skipped() {
        let error = compare_all_svgs(vec![
            "--filter".to_string(),
            "stress_end_keyword_016".to_string(),
            "--check-dom".to_string(),
        ])
        .expect_err("compare-all must not turn unmatched families plus one skip into success");
        let message = error.to_string();

        assert!(
            message.contains("no canonical typed render evidence for sequence"),
            "{message}"
        );
        assert!(
            message
                .contains("no canonical typed render evidence for compare-all selected families"),
            "{message}"
        );
        assert!(
            message.contains(
                "--check-dom produced no raw/source SVG-DOM or SVG-byte comparison evidence for compare-all selected families"
            ),
            "{message}"
        );
    }

    #[test]
    fn compare_all_diagram_selection_applies_only_and_skip_aliases() {
        let options = CompareAllOptions {
            only_diagrams: vec!["tree-view".to_string(), "info".to_string()],
            skip_diagrams: vec!["info".to_string()],
            ..Default::default()
        };

        let selection = CompareAllDiagramSelection::from_options(&options).expect("selection");

        assert_eq!(selection.diagrams, ["treeView"]);
    }

    #[test]
    fn compare_all_global_root_sweep_includes_every_primary_family() {
        let options = CompareAllOptions {
            check_dom: true,
            dom_mode: Some("parity-root".to_string()),
            ..Default::default()
        };
        let selection = CompareAllDiagramSelection::from_options(&options).expect("selection");

        let primary: Vec<&str> = crate::cmd::primary_svg_matrix_diagrams().collect();
        assert_eq!(selection.diagrams, primary);
        for diagram in ["treeView", "ishikawa", "eventmodeling"] {
            assert!(
                selection.diagrams.contains(&diagram),
                "global parity-root sweep must include {diagram}"
            );
        }
    }

    #[test]
    fn compare_all_diagram_selection_rejects_empty_result() {
        let options = CompareAllOptions {
            only_diagrams: vec!["not-a-diagram".to_string()],
            ..Default::default()
        };

        assert!(CompareAllDiagramSelection::from_options(&options).is_err());
    }

    #[test]
    fn root_report_support_covers_active_diagnostic_families() {
        for diagram in ["class", "timeline", "journey"] {
            assert!(
                diagram_supports_root_delta_report(diagram),
                "{diagram} should emit root delta reports through compare-all"
            );
        }
    }

    #[test]
    fn compare_invocation_builds_typed_request_and_mode_report_path() {
        let compare_dir = Path::new("target/compare");
        let expected_report = compare_dir.join("info_report_parity_root.md");
        let invocation = CompareAllInvocationOptions {
            check_dom: true,
            dom_mode: Some("parity-root"),
            dom_decimals: Some(3),
            filter: Some("upstream_info_spec"),
            ..Default::default()
        }
        .for_diagram("info", compare_dir);

        assert_eq!(
            invocation.report_path.as_deref(),
            Some(expected_report.as_path())
        );
        assert!(invocation.request.check_dom);
        assert_eq!(invocation.request.dom_mode.as_deref(), Some("parity-root"));
        assert_eq!(invocation.request.dom_decimals, Some(3));
        assert_eq!(
            invocation.request.filter.as_deref(),
            Some("upstream_info_spec")
        );
        assert_eq!(
            invocation.request.out_path.as_deref(),
            Some(expected_report.as_path())
        );
    }

    #[test]
    fn compare_invocation_projects_dom_suite_into_one_report_and_request() {
        let compare_dir = Path::new("target/compare");
        let modes = [
            crate::svgdom::DomMode::Structure,
            crate::svgdom::DomMode::Parity,
            crate::svgdom::DomMode::ParityRoot,
        ];
        let expected_report = compare_dir.join("info_report_parity_root.md");
        let invocation = CompareAllInvocationOptions {
            check_dom: true,
            dom_modes: &modes,
            ..Default::default()
        }
        .for_diagram("info", compare_dir);

        assert_eq!(invocation.request.dom_modes, modes);
        assert!(invocation.request.dom_mode.is_none());
        assert_eq!(
            invocation.report_path.as_deref(),
            Some(expected_report.as_path())
        );
    }

    #[test]
    fn compare_invocation_adds_root_report_args_only_for_supported_diagrams() {
        let compare_dir = Path::new("target/compare");
        let options = CompareAllInvocationOptions {
            report_root: true,
            root_report_limit: Some(RootDeltaReportLimit::Top(7)),
            ..Default::default()
        };

        let class = options.for_diagram("class", compare_dir).request;
        assert!(class.report_root);
        assert_eq!(class.root_report_limit, Some(RootDeltaReportLimit::Top(7)));
        let er = options.for_diagram("er", compare_dir).request;
        assert!(!er.report_root);
        assert_eq!(er.root_report_limit, None);
    }

    #[test]
    fn compare_invocation_propagates_all_root_report_limit() {
        let compare_dir = Path::new("target/compare");
        let invocation = CompareAllInvocationOptions {
            report_root: true,
            root_report_limit: Some(RootDeltaReportLimit::All),
            ..Default::default()
        }
        .for_diagram("timeline", compare_dir);

        assert!(invocation.request.report_root);
        assert_eq!(
            invocation.request.root_report_limit,
            Some(RootDeltaReportLimit::All)
        );
    }
}
