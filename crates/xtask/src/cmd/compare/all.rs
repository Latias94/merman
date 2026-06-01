//! Compare all diagram SVGs under fixtures.

use crate::XtaskError;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use super::diagrams::*;
use super::{RootDeltaReportLimit, parse_root_delta_report_limit};

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

    let mut diagrams: Vec<&str> = vec![
        "er",
        "flowchart",
        "state",
        "class",
        "sequence",
        "info",
        "pie",
        "sankey",
        "packet",
        "timeline",
        "journey",
        "kanban",
        "gitgraph",
        "gantt",
        "c4",
        "block",
        "radar",
        "requirement",
        "mindmap",
        "architecture",
        "quadrantchart",
        "treemap",
        "xychart",
    ];

    if !only_diagrams.is_empty() {
        let only: Vec<String> = only_diagrams
            .iter()
            .map(|s| s.trim().to_ascii_lowercase())
            .collect();
        diagrams.retain(|d| only.iter().any(|o| o == d));
    }

    if !skip_diagrams.is_empty() {
        let skip: Vec<String> = skip_diagrams
            .iter()
            .map(|s| s.trim().to_ascii_lowercase())
            .collect();
        diagrams.retain(|d| !skip.iter().any(|s| s == d));
    }

    if diagrams.is_empty() {
        return Err(XtaskError::Usage);
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
    let mut root_parity_policy =
        root_parity_policy_enabled.then(|| RootParityResidualPolicy::new(&diagrams));

    fn common_compare_args(
        check_dom: bool,
        dom_mode: Option<&str>,
        dom_decimals: Option<u32>,
        filter: Option<&str>,
    ) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        if check_dom {
            out.push("--check-dom".to_string());
        }
        if let Some(mode) = dom_mode {
            out.push("--dom-mode".to_string());
            out.push(mode.to_string());
        }
        if let Some(n) = dom_decimals {
            out.push("--dom-decimals".to_string());
            out.push(n.to_string());
        }
        if let Some(f) = filter {
            out.push("--filter".to_string());
            out.push(f.to_string());
        }
        out
    }

    let mut failures: Vec<String> = Vec::new();

    for diagram in diagrams {
        println!("\n== compare {diagram} ==");

        let mut cmd_args = common_compare_args(
            check_dom,
            dom_mode.as_deref(),
            dom_decimals,
            filter.as_deref(),
        );
        let mut report_path = None;

        // Avoid overwriting reports across multiple runs (e.g. `parity` then `parity-root`).
        // When a dom mode is specified, we emit mode-suffixed reports:
        // `target/compare/<diagram>_report_<mode>.md` (e.g. `state_report_parity_root.md`).
        if let Some(ref mode) = dom_mode {
            let mode = dom_mode_slug(mode);
            if !mode.is_empty() {
                let path = compare_dir.join(format!("{diagram}_report_{mode}.md"));
                cmd_args.push("--out".to_string());
                cmd_args.push(path.display().to_string());
                report_path = Some(path);
            }
        }

        if diagram == "flowchart" {
            if let Some(tm) = flowchart_text_measurer.as_deref() {
                cmd_args.push("--text-measurer".to_string());
                cmd_args.push(tm.to_string());
            }
        }

        if report_root
            && matches!(
                diagram,
                "architecture" | "flowchart" | "gitgraph" | "mindmap" | "sequence" | "state"
            )
        {
            cmd_args.push("--report-root".to_string());
            match root_report_limit {
                Some(RootDeltaReportLimit::All) => {
                    cmd_args.push("--report-root-all".to_string());
                }
                Some(RootDeltaReportLimit::Top(limit)) => {
                    cmd_args.push("--report-root-limit".to_string());
                    cmd_args.push(limit.to_string());
                }
                None => {}
            }
        }

        let res = match diagram {
            "er" => compare_er_svgs(cmd_args),
            "flowchart" => compare_flowchart_svgs(cmd_args),
            "state" => compare_state_svgs(cmd_args),
            "class" => compare_class_svgs(cmd_args),
            "sequence" => compare_sequence_svgs(cmd_args),
            "info" => compare_info_svgs(cmd_args),
            "pie" => compare_pie_svgs(cmd_args),
            "sankey" => compare_sankey_svgs(cmd_args),
            "packet" => compare_packet_svgs(cmd_args),
            "timeline" => compare_timeline_svgs(cmd_args),
            "journey" => compare_journey_svgs(cmd_args),
            "kanban" => compare_kanban_svgs(cmd_args),
            "gitgraph" => compare_gitgraph_svgs(cmd_args),
            "gantt" => compare_gantt_svgs(cmd_args),
            "c4" => compare_c4_svgs(cmd_args),
            "block" => compare_block_svgs(cmd_args),
            "radar" => compare_radar_svgs(cmd_args),
            "requirement" => compare_requirement_svgs(cmd_args),
            "mindmap" => compare_mindmap_svgs(cmd_args),
            "architecture" => compare_architecture_svgs(cmd_args),
            "quadrantchart" => compare_quadrantchart_svgs(cmd_args),
            "treemap" => compare_treemap_svgs(cmd_args),
            "xychart" => compare_xychart_svgs(cmd_args),
            other => Err(XtaskError::SvgCompareFailed(format!(
                "unexpected diagram: {other}"
            ))),
        };

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
            "local=`max-width: 2345px; background-color: white;`",
        ],
    },
    AcceptedRootParityResidual {
        diagram: "class",
        stem: "upstream_cypress_classdiagram_handdrawn_v3_spec_hd_should_render_classes_with_different_text_labels_037",
        fragments: &[
            "svg: attr `style` mismatch",
            "upstream=`max-width: 2355.75px; background-color: white;`",
            "local=`max-width: 2345px; background-color: white;`",
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
        let msg = "dom mismatch for upstream_cypress_classdiagram_elk_v3_spec_elk_should_render_classes_with_different_text_labels_037: upstream=a local=b (svg: attr `style` mismatch upstream=`max-width: 2355.75px; background-color: white;` local=`max-width: 2345px; background-color: white;`)\n\
dom mismatch for upstream_cypress_classdiagram_handdrawn_v3_spec_hd_should_render_classes_with_different_text_labels_037: upstream=a local=b (svg: attr `style` mismatch upstream=`max-width: 2355.75px; background-color: white;` local=`max-width: 2345px; background-color: white;`)";

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
        let msg = "dom mismatch for upstream_cypress_classdiagram_elk_v3_spec_elk_should_render_classes_with_different_text_labels_037: upstream=a local=b (svg: attr `style` mismatch upstream=`max-width: 2355.75px; background-color: white;` local=`max-width: 2345px; background-color: white;`)\n\
dom mismatch for upstream_cypress_classdiagram_handdrawn_v3_spec_hd_should_render_classes_with_different_text_labels_037: upstream=a local=b (svg: attr `style` mismatch upstream=`max-width: 2355.75px; background-color: white;` local=`max-width: 2345px; background-color: white;`)\n\
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
}
