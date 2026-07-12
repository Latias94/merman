//! Compare all diagram SVGs under fixtures.

use crate::XtaskError;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::diagrams::compare_diagram_svgs;
use super::{
    DomParityResidualPolicy, RootDeltaReportLimit, RootParityResidualPolicy,
    diagram_supports_root_delta_report, parse_root_delta_report_limit,
};

pub(crate) fn compare_all_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let options = CompareAllOptions::parse(args)?;
    let diagram_selection = CompareAllDiagramSelection::from_options(&options)?;
    compare_selected_diagram_svgs(options, diagram_selection)
}

pub(crate) fn compare_all_svgs_with_family_lock(
    args: Vec<String>,
    family_lock: &crate::cmd::UpstreamSvgFamilyLock,
) -> Result<(), XtaskError> {
    let options = CompareAllOptions::parse(args)?;
    let diagram_selection = CompareAllDiagramSelection::from_options(&options)?;
    let [diagram] = diagram_selection.diagrams.as_slice() else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "compare-all with a borrowed upstream SVG family lock requires exactly one diagram, selected {}",
            diagram_selection.diagrams.len()
        )));
    };
    let diagram = *diagram;
    let upstream_dir =
        crate::cmd::compare_diagram_paths_with_roots(diagram, None, None, None).upstream_dir;

    family_lock.validate_target(&upstream_dir)?;
    super::with_borrowed_upstream_svg_family_lock(family_lock, &upstream_dir, || {
        compare_selected_diagram_svgs(options, diagram_selection)
    })
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
    diagram_selection.print_root_deferred_skip();
    let diagrams = diagram_selection.diagrams;

    let invocation_options = options.invocation_options();
    let mut failures = CompareAllFailures::new(&options, &diagrams);

    for diagram in diagrams {
        println!("\n== compare {diagram} ==");

        let DiagramCompareInvocation {
            args: cmd_args,
            report_path,
        } = invocation_options.for_diagram(diagram, &compare_dir);

        failures.record(
            diagram,
            compare_diagram_svgs(diagram, cmd_args),
            report_path.as_deref(),
        );
    }

    failures.finish()
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct CompareAllOptions {
    check_dom: bool,
    dom_mode: Option<String>,
    dom_decimals: Option<u32>,
    filter: Option<String>,
    flowchart_text_measurer: Option<String>,
    flowchart_elk_backend: Option<merman_render::FlowchartElkBackend>,
    include_elk_probes: bool,
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
                    options.dom_mode = args.get(i).map(|s| s.trim().to_string());
                }
                "--dom-decimals" => {
                    i += 1;
                    options.dom_decimals = args.get(i).and_then(|s| s.trim().parse::<u32>().ok());
                }
                "--filter" => {
                    i += 1;
                    options.filter = args.get(i).map(|s| s.to_string());
                }
                "--flowchart-text-measurer" => {
                    i += 1;
                    options.flowchart_text_measurer =
                        args.get(i).map(|s| s.trim().to_ascii_lowercase());
                }
                "--flowchart-elk-backend" => {
                    i += 1;
                    options.flowchart_elk_backend = Some(crate::cmd::parse_flowchart_elk_backend(
                        args.get(i).map(String::as_str),
                    )?);
                }
                "--include-elk-probes" => options.include_elk_probes = true,
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

        Ok(options)
    }

    fn root_parity_policy_enabled(&self) -> bool {
        self.check_dom
            && self.filter.is_none()
            && self
                .dom_mode
                .as_deref()
                .is_some_and(|mode| matches!(mode.trim(), "parity-root" | "parity_root"))
    }

    fn dom_parity_policy_enabled(&self) -> bool {
        self.check_dom
            && self.filter.is_none()
            && self
                .dom_mode
                .as_deref()
                .is_some_and(|mode| mode.trim() == "parity")
    }

    fn global_root_parity_sweep(&self) -> bool {
        self.root_parity_policy_enabled() && self.only_diagrams.is_empty()
    }

    fn invocation_options(&self) -> CompareAllInvocationOptions<'_> {
        CompareAllInvocationOptions {
            check_dom: self.check_dom,
            dom_mode: self.dom_mode.as_deref(),
            dom_decimals: self.dom_decimals,
            filter: self.filter.as_deref(),
            flowchart_text_measurer: self.flowchart_text_measurer.as_deref(),
            flowchart_elk_backend: self.flowchart_elk_backend,
            include_elk_probes: self.include_elk_probes,
            report_root: self.report_root,
            root_report_limit: self.root_report_limit,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CompareAllDiagramSelection {
    diagrams: Vec<&'static str>,
    skipped_root_deferred: Vec<&'static str>,
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

        let skipped_root_deferred = if options.global_root_parity_sweep() {
            let root_deferred: BTreeSet<&str> =
                crate::cmd::root_viewport_deferred_diagrams().collect();
            let skipped = diagrams
                .iter()
                .copied()
                .filter(|d| root_deferred.contains(d))
                .collect::<Vec<_>>();
            diagrams.retain(|d| !root_deferred.contains(d));
            skipped
        } else {
            Vec::new()
        };

        if diagrams.is_empty() {
            return Err(XtaskError::Usage);
        }

        Ok(Self {
            diagrams,
            skipped_root_deferred,
        })
    }

    fn print_root_deferred_skip(&self) {
        if !self.skipped_root_deferred.is_empty() {
            println!(
                "skipping root-viewport-deferred diagrams in global parity-root sweep: {}",
                self.skipped_root_deferred.join(", ")
            );
        }
    }
}

#[derive(Debug)]
struct CompareAllFailures {
    skip_unmatched_filter_messages: bool,
    dom_parity_policy: Option<DomParityResidualPolicy>,
    root_parity_policy: Option<RootParityResidualPolicy>,
    failures: Vec<String>,
}

impl CompareAllFailures {
    fn new(options: &CompareAllOptions, diagrams: &[&str]) -> Self {
        Self {
            skip_unmatched_filter_messages: options.filter.is_some()
                && options.only_diagrams.is_empty(),
            dom_parity_policy: options
                .dom_parity_policy_enabled()
                .then(|| DomParityResidualPolicy::new(diagrams)),
            root_parity_policy: options
                .root_parity_policy_enabled()
                .then(|| RootParityResidualPolicy::new(diagrams)),
            failures: Vec::new(),
        }
    }

    fn record(
        &mut self,
        diagram: &str,
        result: Result<(), XtaskError>,
        report_path: Option<&Path>,
    ) {
        match result {
            Ok(()) => {}
            Err(XtaskError::SvgCompareFailed(msg)) if self.should_skip_unmatched_filter(&msg) => {
                println!("(skipped: {msg})");
            }
            Err(XtaskError::SvgCompareFailed(msg)) => {
                self.record_svg_compare_failure(diagram, &msg, report_path);
            }
            Err(err) => self.failures.push(format!("{diagram}: {err}")),
        }
    }

    fn finish(mut self) -> Result<(), XtaskError> {
        if let Some(policy) = self.dom_parity_policy {
            let accepted = policy.accepted_summaries();
            if !accepted.is_empty() {
                println!("\n== accepted DOM parity residuals ==");
                for line in accepted {
                    println!("{line}");
                }
            }
            self.failures.extend(policy.missing_failures());
        }

        if let Some(policy) = self.root_parity_policy {
            let accepted = policy.accepted_summaries();
            if !accepted.is_empty() {
                println!("\n== accepted root parity residuals ==");
                for line in accepted {
                    println!("{line}");
                }
            }
            self.failures.extend(policy.missing_failures());
        }

        if self.failures.is_empty() {
            Ok(())
        } else {
            Err(XtaskError::SvgCompareFailed(self.failures.join("\n")))
        }
    }

    fn record_svg_compare_failure(&mut self, diagram: &str, msg: &str, report_path: Option<&Path>) {
        if let Some(policy) = self.dom_parity_policy.as_mut() {
            if let Some(failure) = policy.accept_or_summarize_failure(diagram, msg, report_path) {
                self.failures.push(failure);
            }
        } else if let Some(policy) = self.root_parity_policy.as_mut() {
            if let Some(failure) = policy.accept_or_summarize_failure(diagram, msg, report_path) {
                self.failures.push(failure);
            }
        } else {
            self.failures.push(format!(
                "{diagram}: {}",
                XtaskError::SvgCompareFailed(msg.to_string())
            ));
        }
    }

    fn should_skip_unmatched_filter(&self, msg: &str) -> bool {
        self.skip_unmatched_filter_messages && msg.contains("no .mmd fixtures matched under ")
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct CompareAllInvocationOptions<'a> {
    check_dom: bool,
    dom_mode: Option<&'a str>,
    dom_decimals: Option<u32>,
    filter: Option<&'a str>,
    flowchart_text_measurer: Option<&'a str>,
    flowchart_elk_backend: Option<merman_render::FlowchartElkBackend>,
    include_elk_probes: bool,
    report_root: bool,
    root_report_limit: Option<RootDeltaReportLimit>,
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
        if diagram != "flowchart" {
            return;
        }

        if let Some(tm) = self.flowchart_text_measurer {
            args.extend(["--text-measurer".to_string(), tm.to_string()]);
        }
        if let Some(backend) = self.flowchart_elk_backend {
            args.extend([
                "--flowchart-elk-backend".to_string(),
                crate::cmd::flowchart_elk_backend_name(backend).to_string(),
            ]);
        }
        if self.include_elk_probes {
            args.push("--include-elk-probes".to_string());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagram_filter_key_accepts_tree_view_aliases() {
        assert_eq!(diagram_filter_key("treeView"), "treeview");
        assert_eq!(diagram_filter_key("tree-view"), "treeview");
        assert_eq!(diagram_filter_key("eventmodeling"), "eventmodeling");
    }

    #[test]
    fn compare_all_options_parse_common_flags_without_tightening_legacy_inputs() {
        let options = CompareAllOptions::parse(vec![
            "--check-dom".to_string(),
            "--dom-mode".to_string(),
            " parity-root ".to_string(),
            "--dom-decimals".to_string(),
            "nope".to_string(),
            "--filter".to_string(),
            "upstream_info_spec".to_string(),
            "--flowchart-text-measurer".to_string(),
            " BROWSER ".to_string(),
            "--flowchart-elk-backend".to_string(),
            " source-ported ".to_string(),
            "--include-elk-probes".to_string(),
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
        assert_eq!(options.flowchart_text_measurer.as_deref(), Some("browser"));
        assert_eq!(
            options.flowchart_elk_backend,
            Some(merman_render::FlowchartElkBackend::SourcePorted)
        );
        assert!(options.include_elk_probes);
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
        assert!(CompareAllOptions::parse(vec!["--flowchart-elk-backend".to_string()]).is_err());
        assert!(
            CompareAllOptions::parse(vec![
                "--flowchart-elk-backend".to_string(),
                "unknown".to_string()
            ])
            .is_err()
        );
    }

    #[test]
    fn compare_all_invocation_passes_flowchart_elk_lane_only_to_flowchart() {
        let options = CompareAllOptions {
            filter: Some("elk_probe".to_string()),
            flowchart_text_measurer: Some("vendored".to_string()),
            flowchart_elk_backend: Some(merman_render::FlowchartElkBackend::SourcePorted),
            include_elk_probes: true,
            ..Default::default()
        };
        let invocation = options.invocation_options();
        let compare_dir = Path::new("target/compare");

        let flowchart = invocation.for_diagram("flowchart", compare_dir);
        assert!(flowchart.args.contains(&"--text-measurer".to_string()));
        assert!(flowchart.args.contains(&"vendored".to_string()));
        assert!(
            flowchart
                .args
                .contains(&"--flowchart-elk-backend".to_string())
        );
        assert!(flowchart.args.contains(&"source-ported".to_string()));
        assert!(flowchart.args.contains(&"--include-elk-probes".to_string()));

        let info = invocation.for_diagram("info", compare_dir);
        assert!(!info.args.contains(&"--text-measurer".to_string()));
        assert!(!info.args.contains(&"--flowchart-elk-backend".to_string()));
        assert!(!info.args.contains(&"--include-elk-probes".to_string()));
    }

    #[test]
    fn compare_all_options_detects_root_parity_policy_scope() {
        let global = CompareAllOptions {
            check_dom: true,
            dom_mode: Some("parity_root".to_string()),
            ..Default::default()
        };
        assert!(global.root_parity_policy_enabled());
        assert!(global.global_root_parity_sweep());

        let targeted = CompareAllOptions {
            only_diagrams: vec!["flowchart".to_string()],
            ..global.clone()
        };
        assert!(targeted.root_parity_policy_enabled());
        assert!(!targeted.global_root_parity_sweep());

        let filtered = CompareAllOptions {
            filter: Some("smoke".to_string()),
            ..global
        };
        assert!(!filtered.root_parity_policy_enabled());
    }

    #[test]
    fn compare_all_options_detects_dom_parity_policy_scope() {
        let global = CompareAllOptions {
            check_dom: true,
            dom_mode: Some("parity".to_string()),
            ..Default::default()
        };
        assert!(global.dom_parity_policy_enabled());

        let targeted = CompareAllOptions {
            only_diagrams: vec!["sequence".to_string()],
            ..global.clone()
        };
        assert!(targeted.dom_parity_policy_enabled());

        let filtered = CompareAllOptions {
            filter: Some("smoke".to_string()),
            ..global
        };
        assert!(!filtered.dom_parity_policy_enabled());
    }

    #[test]
    fn compare_all_failures_skip_unmatched_filter_only_for_global_filtered_runs() {
        let msg = "no .mmd fixtures matched under fixtures";
        let global_options = CompareAllOptions {
            filter: Some("missing".to_string()),
            ..Default::default()
        };
        let global = CompareAllFailures::new(&global_options, &["info"]);
        assert!(global.should_skip_unmatched_filter(msg));

        let targeted_options = CompareAllOptions {
            filter: Some("missing".to_string()),
            only_diagrams: vec!["info".to_string()],
            ..Default::default()
        };
        let targeted = CompareAllFailures::new(&targeted_options, &["info"]);
        assert!(!targeted.should_skip_unmatched_filter(msg));
    }

    #[test]
    fn compare_all_failures_records_plain_svg_compare_failures() {
        let options = CompareAllOptions::default();
        let mut failures = CompareAllFailures::new(&options, &["info"]);

        failures.record(
            "info",
            Err(XtaskError::SvgCompareFailed("dom mismatch".to_string())),
            None,
        );

        assert_eq!(
            failures.failures,
            ["info: svg compare failed:\ndom mismatch"]
        );
    }

    #[test]
    fn compare_all_failures_accepts_expected_root_residuals() {
        let options = CompareAllOptions {
            check_dom: true,
            dom_mode: Some("parity-root".to_string()),
            ..Default::default()
        };
        let mut failures = CompareAllFailures::new(&options, &["gitgraph"]);

        failures.record(
            "gitgraph",
            Err(XtaskError::SvgCompareFailed(
                "dom mismatch for zed_pr_57644_gitgraph: upstream=a local=b (scope=parity-normalized-descendants-match; svg: attr `style` mismatch upstream=`max-width: 845.25px; background-color: white;` local=`max-width: 845px; background-color: white;`)"
                    .to_string(),
            )),
            None,
        );

        assert!(failures.finish().is_ok());
    }

    #[test]
    fn compare_all_failures_accepts_exact_documented_sequence_dom_residuals() {
        let options = CompareAllOptions {
            check_dom: true,
            dom_mode: Some("parity".to_string()),
            ..Default::default()
        };
        let mut failures = CompareAllFailures::new(&options, &["sequence"]);
        let msg = "dom mismatch for upstream_cypress_sequencediagram_spec_should_render_long_notes_wrapped_inline_left_of_actor_026: upstream=a local=b (svg/g[16]: child count mismatch upstream=9 local=8)\n\
dom mismatch for upstream_cypress_sequencediagram_v2_spec_should_render_wrapped_long_notes_left_of_control_019: upstream=a local=b (svg/g[20]: child count mismatch upstream=9 local=8)\n\
dom mismatch for upstream_docs_diagrams_mermaid_api_sequence: upstream=a local=b (svg/g[61]/text[9]: attr `class` mismatch upstream=`sectionTitle` local=`loopText`)";

        failures.record(
            "sequence",
            Err(XtaskError::SvgCompareFailed(msg.to_string())),
            Some(Path::new("target/compare/sequence_report_parity.md")),
        );

        assert!(failures.finish().is_ok());
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
        assert!(selection.skipped_root_deferred.is_empty());
    }

    #[test]
    fn compare_all_diagram_selection_skips_root_deferred_only_for_global_root_sweep() {
        let options = CompareAllOptions {
            check_dom: true,
            dom_mode: Some("parity-root".to_string()),
            ..Default::default()
        };
        let selection = CompareAllDiagramSelection::from_options(&options).expect("selection");

        let root_deferred: Vec<&str> = crate::cmd::root_viewport_deferred_diagrams().collect();
        assert!(!root_deferred.is_empty());
        for diagram in root_deferred {
            assert!(!selection.diagrams.contains(&diagram));
            assert!(selection.skipped_root_deferred.contains(&diagram));
        }

        let targeted = CompareAllOptions {
            only_diagrams: vec!["flowchart".to_string()],
            ..options
        };
        let selection = CompareAllDiagramSelection::from_options(&targeted).expect("selection");
        assert_eq!(selection.diagrams, ["flowchart"]);
        assert!(selection.skipped_root_deferred.is_empty());
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
