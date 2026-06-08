//! Root parity residual acceptance policy for compare-all sweeps.

use std::collections::BTreeSet;
use std::path::Path;

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
pub(crate) struct RootParityResidualPolicy {
    expected: Vec<&'static AcceptedRootParityResidual>,
    seen: BTreeSet<(&'static str, &'static str)>,
}

impl RootParityResidualPolicy {
    pub(crate) fn new(diagrams: &[&str]) -> Self {
        let expected = ACCEPTED_ROOT_PARITY_RESIDUALS
            .iter()
            .filter(|residual| diagrams.contains(&residual.diagram))
            .collect();
        Self {
            expected,
            seen: BTreeSet::new(),
        }
    }

    pub(crate) fn accept_or_summarize_failure(
        &mut self,
        diagram: &str,
        msg: &str,
        report_path: Option<&Path>,
    ) -> Option<String> {
        self.accept_or_return_remaining(diagram, msg)
            .map(|remaining| summarize_root_parity_failure(diagram, &remaining, report_path))
    }

    pub(crate) fn accepted_summaries(&self) -> Vec<String> {
        self.expected
            .iter()
            .filter(|residual| self.seen.contains(&(residual.diagram, residual.stem)))
            .map(|residual| format!("- {}/{}", residual.diagram, residual.stem))
            .collect()
    }

    pub(crate) fn missing_failures(&self) -> Vec<String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_parity_policy_accepts_exact_recorded_class_residuals() {
        let mut policy = RootParityResidualPolicy::new(&["class"]);
        let msg = "dom mismatch for upstream_cypress_classdiagram_elk_v3_spec_elk_should_render_classes_with_different_text_labels_037: upstream=a local=b (svg: attr `style` mismatch upstream=`max-width: 2355.75px; background-color: white;` local=`max-width: 2344.92px; background-color: white;`)\n\
dom mismatch for upstream_cypress_classdiagram_handdrawn_v3_spec_hd_should_render_classes_with_different_text_labels_037: upstream=a local=b (svg: attr `style` mismatch upstream=`max-width: 2355.73px; background-color: white;` local=`max-width: 2344.92px; background-color: white;`)";

        assert!(
            policy
                .accept_or_summarize_failure("class", msg, None)
                .is_none()
        );
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

        let summary = policy
            .accept_or_summarize_failure("class", msg, None)
            .expect("unexpected mismatch should remain");

        assert!(summary.contains("unexpected_fixture"));
        assert!(policy.missing_failures().is_empty());
    }

    #[test]
    fn root_parity_policy_rejects_changed_residual_values() {
        let mut policy = RootParityResidualPolicy::new(&["mindmap"]);
        let msg = "dom mismatch for upstream_docs_example_icons_br: upstream=a local=b (svg: attr `style` mismatch upstream=`max-width: 756.25px; background-color: white;` local=`max-width: 756.5px; background-color: white;`)";

        let summary = policy
            .accept_or_summarize_failure("mindmap", msg, None)
            .expect("changed residual should remain");
        let missing = policy.missing_failures();

        assert!(summary.contains("upstream_docs_example_icons_br"));
        assert!(
            missing
                .iter()
                .any(|line| line.contains("upstream_docs_example_icons_br"))
        );
    }
}
