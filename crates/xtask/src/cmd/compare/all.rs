//! Compare all diagram SVGs under fixtures.

use crate::XtaskError;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::diagrams::compare_diagram_svgs;
use super::{
    RootDeltaReportLimit, diagram_supports_root_delta_report, parse_root_delta_report_limit,
};

pub(crate) fn compare_all_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut check_dom: bool = false;
    let mut dom_mode: Option<String> = None;
    let mut dom_decimals: Option<u32> = None;
    let mut filter: Option<String> = None;
    let mut flowchart_text_measurer: Option<String> = None;
    let mut report_root: bool = false;
    let mut root_report_limit: Option<RootDeltaReportLimit> = None;

    let mut only_diagrams: Vec<String> = Vec::new();
    let mut skip_diagrams: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--check-dom" => check_dom = true,
            "--dom-mode" => {
                i += 1;
                dom_mode = args.get(i).map(|s| s.trim().to_string());
            }
            "--dom-decimals" => {
                i += 1;
                dom_decimals = args.get(i).and_then(|s| s.trim().parse::<u32>().ok());
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--flowchart-text-measurer" => {
                i += 1;
                flowchart_text_measurer = args.get(i).map(|s| s.trim().to_ascii_lowercase());
            }
            "--report-root" => report_root = true,
            "--report-root-all" => {
                report_root = true;
                root_report_limit = Some(RootDeltaReportLimit::All);
            }
            "--report-root-limit" => {
                i += 1;
                report_root = true;
                root_report_limit = Some(parse_root_delta_report_limit(
                    args.get(i).map(String::as_str),
                )?);
            }
            "--diagram" => {
                i += 1;
                let d = args.get(i).ok_or(XtaskError::Usage)?.trim().to_string();
                if !d.is_empty() {
                    only_diagrams.push(d);
                }
            }
            "--skip" => {
                i += 1;
                let d = args.get(i).ok_or(XtaskError::Usage)?.trim().to_string();
                if !d.is_empty() {
                    skip_diagrams.push(d);
                }
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let mut diagrams: Vec<&str> = crate::cmd::primary_svg_matrix_diagrams().collect();

    if !only_diagrams.is_empty() {
        let only: Vec<String> = only_diagrams
            .iter()
            .map(|s| diagram_filter_key(s))
            .collect();
        diagrams.retain(|d| only.iter().any(|o| o == &diagram_filter_key(d)));
    }

    if !skip_diagrams.is_empty() {
        let skip: Vec<String> = skip_diagrams
            .iter()
            .map(|s| diagram_filter_key(s))
            .collect();
        diagrams.retain(|d| !skip.iter().any(|s| s == &diagram_filter_key(d)));
    }

    if diagrams.is_empty() {
        return Err(XtaskError::Usage);
    }

    let compare_dir = crate::cmd::target_root().join("compare");
    fs::create_dir_all(&compare_dir).map_err(|source| XtaskError::WriteFile {
        path: compare_dir.display().to_string(),
        source,
    })?;

    let root_parity_policy_enabled = check_dom
        && filter.is_none()
        && dom_mode
            .as_deref()
            .is_some_and(|mode| matches!(mode.trim(), "parity-root" | "parity_root"));
    let global_root_parity_sweep = root_parity_policy_enabled && only_diagrams.is_empty();

    if global_root_parity_sweep {
        let root_deferred: BTreeSet<&str> = crate::cmd::root_viewport_deferred_diagrams().collect();
        let skipped: Vec<&str> = diagrams
            .iter()
            .copied()
            .filter(|d| root_deferred.contains(d))
            .collect();
        diagrams.retain(|d| !root_deferred.contains(d));
        if !skipped.is_empty() {
            println!(
                "skipping root-viewport-deferred diagrams in global parity-root sweep: {}",
                skipped.join(", ")
            );
        }
    }

    let mut root_parity_policy =
        root_parity_policy_enabled.then(|| RootParityResidualPolicy::new(&diagrams));

    let invocation_options = CompareAllInvocationOptions {
        check_dom,
        dom_mode: dom_mode.as_deref(),
        dom_decimals,
        filter: filter.as_deref(),
        flowchart_text_measurer: flowchart_text_measurer.as_deref(),
        report_root,
        root_report_limit,
    };

    let mut failures: Vec<String> = Vec::new();

    for diagram in diagrams {
        println!("\n== compare {diagram} ==");

        let DiagramCompareInvocation {
            args: cmd_args,
            report_path,
        } = invocation_options.for_diagram(diagram, &compare_dir);

        let res = compare_diagram_svgs(diagram, cmd_args);

        match res {
            Ok(()) => {}
            Err(XtaskError::SvgCompareFailed(msg))
                if filter.is_some()
                    && only_diagrams.is_empty()
                    && msg.contains("no .mmd fixtures matched under ") =>
            {
                println!("(skipped: {msg})");
            }
            Err(XtaskError::SvgCompareFailed(msg)) => {
                if let Some(policy) = root_parity_policy.as_mut() {
                    if let Some(remaining) = policy.accept_or_return_remaining(diagram, &msg) {
                        failures.push(summarize_root_parity_failure(
                            diagram,
                            &remaining,
                            report_path.as_deref(),
                        ));
                    }
                } else {
                    failures.push(format!("{diagram}: {}", XtaskError::SvgCompareFailed(msg)));
                }
            }
            Err(err) => failures.push(format!("{diagram}: {err}")),
        }
    }

    if let Some(policy) = root_parity_policy {
        let accepted = policy.accepted_summaries();
        if !accepted.is_empty() {
            println!("\n== accepted root parity residuals ==");
            for line in accepted {
                println!("{line}");
            }
        }
        failures.extend(policy.missing_failures());
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::SvgCompareFailed(failures.join("\n")))
    }
}

#[derive(Debug, Clone, Copy)]
struct CompareAllInvocationOptions<'a> {
    check_dom: bool,
    dom_mode: Option<&'a str>,
    dom_decimals: Option<u32>,
    filter: Option<&'a str>,
    flowchart_text_measurer: Option<&'a str>,
    report_root: bool,
    root_report_limit: Option<RootDeltaReportLimit>,
}

impl<'a> Default for CompareAllInvocationOptions<'a> {
    fn default() -> Self {
        Self {
            check_dom: false,
            dom_mode: None,
            dom_decimals: None,
            filter: None,
            flowchart_text_measurer: None,
            report_root: false,
            root_report_limit: None,
        }
    }
}

impl CompareAllInvocationOptions<'_> {
    fn for_diagram(&self, diagram: &str, compare_dir: &Path) -> DiagramCompareInvocation {
        let mut args = self.common_compare_args();
        let report_path = self.push_report_path_args(diagram, compare_dir, &mut args);
        self.push_diagram_args(diagram, &mut args);
        self.push_root_report_args(diagram, &mut args);
        DiagramCompareInvocation { args, report_path }
    }

    fn common_compare_args(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        if self.check_dom {
            out.push("--check-dom".to_string());
        }
        if let Some(mode) = self.dom_mode {
            out.push("--dom-mode".to_string());
            out.push(mode.to_string());
        }
        if let Some(n) = self.dom_decimals {
            out.push("--dom-decimals".to_string());
            out.push(n.to_string());
        }
        if let Some(f) = self.filter {
            out.push("--filter".to_string());
            out.push(f.to_string());
        }
        out
    }

    fn push_report_path_args(
        &self,
        diagram: &str,
        compare_dir: &Path,
        args: &mut Vec<String>,
    ) -> Option<PathBuf> {
        let mode = self.dom_mode.map(dom_mode_slug)?;
        if mode.is_empty() {
            return None;
        }

        let path = compare_dir.join(format!("{diagram}_report_{mode}.md"));
        args.push("--out".to_string());
        args.push(path.display().to_string());
        Some(path)
    }

    fn push_diagram_args(&self, diagram: &str, args: &mut Vec<String>) {
        if diagram == "flowchart" {
            if let Some(tm) = self.flowchart_text_measurer {
                args.push("--text-measurer".to_string());
                args.push(tm.to_string());
            }
        }
    }

    fn push_root_report_args(&self, diagram: &str, args: &mut Vec<String>) {
        if !self.report_root || !diagram_supports_root_delta_report(diagram) {
            return;
        }

        args.push("--report-root".to_string());
        match self.root_report_limit {
            Some(RootDeltaReportLimit::All) => {
                args.push("--report-root-all".to_string());
            }
            Some(RootDeltaReportLimit::Top(limit)) => {
                args.push("--report-root-limit".to_string());
                args.push(limit.to_string());
            }
            None => {}
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DiagramCompareInvocation {
    args: Vec<String>,
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

fn summarize_root_parity_failure(diagram: &str, msg: &str, report_path: Option<&Path>) -> String {
    let lines: Vec<&str> = msg.lines().filter(|line| !line.trim().is_empty()).collect();
    let mismatch_count = lines
        .iter()
        .filter(|line| line.trim_start().starts_with("dom mismatch for "))
        .count();
    let count = if mismatch_count > 0 {
        mismatch_count
    } else {
        lines.len()
    };
    let first = lines.first().copied().unwrap_or("no mismatch details");
    let report = report_path
        .map(|path| format!("; report={}", path.display()))
        .unwrap_or_default();
    format!("{diagram}: {count} unaccepted parity-root DOM mismatch(es){report}; first: {first}")
}

#[derive(Debug, Clone, Copy)]
struct AcceptedRootParityResidual {
    diagram: &'static str,
    stem: &'static str,
    fragments: &'static [&'static str],
}

const ACCEPTED_ROOT_PARITY_RESIDUALS: &[AcceptedRootParityResidual] = &[
    AcceptedRootParityResidual {
        diagram: "class",
        stem: "upstream_cypress_classdiagram_elk_v3_spec_elk_should_render_classes_with_different_text_labels_037",
        fragments: &[
            "svg: attr `style` mismatch",
            "upstream=`max-width: 2355.75px; background-color: white;`",
            "local=`max-width: 2344.92px; background-color: white;`",
        ],
    },
    AcceptedRootParityResidual {
        diagram: "class",
        stem: "upstream_cypress_classdiagram_handdrawn_v3_spec_hd_should_render_classes_with_different_text_labels_037",
        fragments: &[
            "svg: attr `style` mismatch",
            "upstream=`max-width: 2355.73px; background-color: white;`",
            "local=`max-width: 2344.92px; background-color: white;`",
        ],
    },
    AcceptedRootParityResidual {
        diagram: "sequence",
        stem: "zed_pr_57644_sequence",
        fragments: &[
            "svg: attr `viewBox` mismatch",
            "upstream=`<n> <n> 796 1096`",
            "local=`<n> <n> 796 1126`",
        ],
    },
    AcceptedRootParityResidual {
        diagram: "gitgraph",
        stem: "zed_pr_57644_gitgraph",
        fragments: &[
            "svg: attr `style` mismatch",
            "upstream=`max-width: 845.25px; background-color: white;`",
            "local=`max-width: 845px; background-color: white;`",
        ],
    },
    AcceptedRootParityResidual {
        diagram: "mindmap",
        stem: "upstream_docs_example_icons_br",
        fragments: &[
            "svg: attr `style` mismatch",
            "upstream=`max-width: 756.25px; background-color: white;`",
            "local=`max-width: 756.75px; background-color: white;`",
        ],
    },
    AcceptedRootParityResidual {
        diagram: "mindmap",
        stem: "upstream_docs_tidy_tree_example_usage_002",
        fragments: &[
            "svg: attr `viewBox` mismatch",
            "upstream=`<n> <n> 796.5 671.5`",
            "local=`<n> <n> 796.5 671.75`",
        ],
    },
    AcceptedRootParityResidual {
        diagram: "mindmap",
        stem: "upstream_examples_mindmap_basic_mindmap_001",
        fragments: &[
            "svg: attr `style` mismatch",
            "upstream=`max-width: 756.25px; background-color: white;`",
            "local=`max-width: 756.75px; background-color: white;`",
        ],
    },
    AcceptedRootParityResidual {
        diagram: "mindmap",
        stem: "zed_pr_57644_mindmap",
        fragments: &[
            "svg: attr `style` mismatch",
            "upstream=`max-width: 1199.75px; background-color: white;`",
            "local=`max-width: 1161.75px; background-color: white;`",
        ],
    },
];

#[derive(Debug)]
struct RootParityResidualPolicy {
    expected: Vec<&'static AcceptedRootParityResidual>,
    seen: BTreeSet<(&'static str, &'static str)>,
}

impl RootParityResidualPolicy {
    fn new(diagrams: &[&str]) -> Self {
        let expected = ACCEPTED_ROOT_PARITY_RESIDUALS
            .iter()
            .filter(|residual| diagrams.contains(&residual.diagram))
            .collect();
        Self {
            expected,
            seen: BTreeSet::new(),
        }
    }

    fn accept_or_return_remaining(&mut self, diagram: &str, msg: &str) -> Option<String> {
        let mut remaining = Vec::new();
        for line in msg.lines().filter(|line| !line.trim().is_empty()) {
            if let Some(residual) = self.matching_residual(diagram, line) {
                self.seen.insert((residual.diagram, residual.stem));
            } else {
                remaining.push(line.to_string());
            }
        }

        if remaining.is_empty() {
            None
        } else {
            Some(remaining.join("\n"))
        }
    }

    fn matching_residual(
        &self,
        diagram: &str,
        line: &str,
    ) -> Option<&'static AcceptedRootParityResidual> {
        self.expected.iter().copied().find(|residual| {
            residual.diagram == diagram
                && line.contains(&format!("dom mismatch for {}:", residual.stem))
                && residual
                    .fragments
                    .iter()
                    .all(|fragment| line.contains(fragment))
        })
    }

    fn accepted_summaries(&self) -> Vec<String> {
        self.expected
            .iter()
            .filter(|residual| self.seen.contains(&(residual.diagram, residual.stem)))
            .map(|residual| format!("- {}/{}", residual.diagram, residual.stem))
            .collect()
    }

    fn missing_failures(&self) -> Vec<String> {
        self.expected
            .iter()
            .filter(|residual| !self.seen.contains(&(residual.diagram, residual.stem)))
            .map(|residual| {
                format!(
                    "root parity residual policy expected {}/{} but it was not observed; update or remove the policy only with fresh closeout evidence",
                    residual.diagram, residual.stem
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_parity_policy_accepts_exact_recorded_class_residuals() {
        let mut policy = RootParityResidualPolicy::new(&["class"]);
        let msg = "dom mismatch for upstream_cypress_classdiagram_elk_v3_spec_elk_should_render_classes_with_different_text_labels_037: upstream=a local=b (svg: attr `style` mismatch upstream=`max-width: 2355.75px; background-color: white;` local=`max-width: 2344.92px; background-color: white;`)\n\
dom mismatch for upstream_cypress_classdiagram_handdrawn_v3_spec_hd_should_render_classes_with_different_text_labels_037: upstream=a local=b (svg: attr `style` mismatch upstream=`max-width: 2355.73px; background-color: white;` local=`max-width: 2344.92px; background-color: white;`)";

        assert!(policy.accept_or_return_remaining("class", msg).is_none());
        assert_eq!(policy.accepted_summaries().len(), 2);
        assert!(policy.missing_failures().is_empty());
    }

    #[test]
    fn root_parity_failure_summary_keeps_final_error_bounded() {
        let msg = "dom mismatch for a: upstream=a local=b (svg: attr `style` mismatch)\n\
dom mismatch for b: upstream=a local=b (svg: attr `viewBox` mismatch)";

        let summary = summarize_root_parity_failure(
            "flowchart",
            msg,
            Some(std::path::Path::new(
                "target/compare/flowchart_report_parity_root.md",
            )),
        );

        assert!(summary.contains("flowchart: 2 unaccepted parity-root DOM mismatch"));
        assert!(summary.contains("target/compare/flowchart_report_parity_root.md"));
        assert!(summary.contains("first: dom mismatch for a:"));
        assert!(!summary.contains("dom mismatch for b:"));
    }

    #[test]
    fn root_parity_policy_matches_current_strict_root_residuals() {
        let policy = RootParityResidualPolicy::new(&["sequence", "gitgraph", "mindmap"]);

        let residual_lines = [
            (
                "sequence",
                "dom mismatch for zed_pr_57644_sequence: upstream=a local=b (svg: attr `viewBox` mismatch upstream=`<n> <n> 796 1096` local=`<n> <n> 796 1126`)",
            ),
            (
                "gitgraph",
                "dom mismatch for zed_pr_57644_gitgraph: upstream=a local=b (svg: attr `style` mismatch upstream=`max-width: 845.25px; background-color: white;` local=`max-width: 845px; background-color: white;`)",
            ),
            (
                "mindmap",
                "dom mismatch for zed_pr_57644_mindmap: upstream=a local=b (svg: attr `style` mismatch upstream=`max-width: 1199.75px; background-color: white;` local=`max-width: 1161.75px; background-color: white;`)",
            ),
        ];

        for (diagram, line) in residual_lines {
            assert!(
                policy.matching_residual(diagram, line).is_some(),
                "residual should match for {diagram}: {line}",
            );
        }
    }

    #[test]
    fn root_parity_policy_preserves_unexpected_mismatches() {
        let mut policy = RootParityResidualPolicy::new(&["class"]);
        let msg = "dom mismatch for upstream_cypress_classdiagram_elk_v3_spec_elk_should_render_classes_with_different_text_labels_037: upstream=a local=b (svg: attr `style` mismatch upstream=`max-width: 2355.75px; background-color: white;` local=`max-width: 2344.92px; background-color: white;`)\n\
dom mismatch for upstream_cypress_classdiagram_handdrawn_v3_spec_hd_should_render_classes_with_different_text_labels_037: upstream=a local=b (svg: attr `style` mismatch upstream=`max-width: 2355.73px; background-color: white;` local=`max-width: 2344.92px; background-color: white;`)\n\
dom mismatch for unexpected_fixture: upstream=a local=b (svg: attr `style` mismatch)";

        let remaining = policy
            .accept_or_return_remaining("class", msg)
            .expect("unexpected mismatch should remain");

        assert!(remaining.contains("unexpected_fixture"));
        assert!(policy.missing_failures().is_empty());
    }

    #[test]
    fn root_parity_policy_rejects_changed_residual_values() {
        let mut policy = RootParityResidualPolicy::new(&["mindmap"]);
        let msg = "dom mismatch for upstream_docs_example_icons_br: upstream=a local=b (svg: attr `style` mismatch upstream=`max-width: 756.25px; background-color: white;` local=`max-width: 756.5px; background-color: white;`)";

        let remaining = policy
            .accept_or_return_remaining("mindmap", msg)
            .expect("changed residual should remain");
        let missing = policy.missing_failures();

        assert!(remaining.contains("upstream_docs_example_icons_br"));
        assert!(
            missing
                .iter()
                .any(|line| line.contains("upstream_docs_example_icons_br"))
        );
    }

    #[test]
    fn diagram_filter_key_accepts_tree_view_aliases() {
        assert_eq!(diagram_filter_key("treeView"), "treeview");
        assert_eq!(diagram_filter_key("tree-view"), "treeview");
        assert_eq!(diagram_filter_key("eventmodeling"), "eventmodeling");
    }

    #[test]
    fn root_report_support_covers_active_residual_families() {
        for diagram in ["class", "timeline", "journey"] {
            assert!(
                diagram_supports_root_delta_report(diagram),
                "{diagram} should emit root delta reports through compare-all"
            );
        }
    }

    #[test]
    fn compare_invocation_builds_common_dom_args_and_mode_report_path() {
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
        assert_eq!(
            invocation.args,
            vec![
                "--check-dom".to_string(),
                "--dom-mode".to_string(),
                "parity-root".to_string(),
                "--dom-decimals".to_string(),
                "3".to_string(),
                "--filter".to_string(),
                "upstream_info_spec".to_string(),
                "--out".to_string(),
                expected_report.display().to_string(),
            ]
        );
    }

    #[test]
    fn compare_invocation_adds_flowchart_text_measurer_only_for_flowchart() {
        let compare_dir = Path::new("target/compare");
        let options = CompareAllInvocationOptions {
            flowchart_text_measurer: Some("browser"),
            ..Default::default()
        };

        assert_eq!(
            options.for_diagram("flowchart", compare_dir).args,
            ["--text-measurer", "browser"]
        );
        assert!(options.for_diagram("state", compare_dir).args.is_empty());
    }

    #[test]
    fn compare_invocation_adds_root_report_args_only_for_supported_diagrams() {
        let compare_dir = Path::new("target/compare");
        let options = CompareAllInvocationOptions {
            report_root: true,
            root_report_limit: Some(RootDeltaReportLimit::Top(7)),
            ..Default::default()
        };

        assert_eq!(
            options.for_diagram("class", compare_dir).args,
            ["--report-root", "--report-root-limit", "7"]
        );
        assert!(options.for_diagram("er", compare_dir).args.is_empty());
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

        assert_eq!(invocation.args, ["--report-root", "--report-root-all"]);
    }
}
