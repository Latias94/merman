//! Root parity residual acceptance policy for compare-all sweeps.

use std::collections::BTreeSet;
use std::path::Path;

#[derive(Debug, Clone, Copy)]
struct AcceptedRootParityResidual {
    diagram: &'static str,
    stem: &'static str,
    detail: &'static str,
}

const ACCEPTED_ROOT_PARITY_RESIDUALS: &[AcceptedRootParityResidual] = &[
    AcceptedRootParityResidual {
        diagram: "class",
        stem: "upstream_cypress_classdiagram_elk_v3_spec_elk_should_render_classes_with_different_text_labels_037",
        detail: "scope=parity-normalized-descendants-match; svg: attr `style` mismatch upstream=`max-width: 2355.75px; background-color: white;` local=`max-width: 2345px; background-color: white;`; additional DOM differences (1): svg: attr `viewBox` mismatch upstream=`<n> <n> 2355.75 100` local=`<n> <n> 2345 100`",
    },
    AcceptedRootParityResidual {
        diagram: "class",
        stem: "upstream_cypress_classdiagram_handdrawn_v3_spec_hd_should_render_classes_with_different_text_labels_037",
        detail: "scope=parity-normalized-descendants-match; svg: attr `style` mismatch upstream=`max-width: 2355.75px; background-color: white;` local=`max-width: 2345px; background-color: white;`; additional DOM differences (1): svg: attr `viewBox` mismatch upstream=`<n> <n> 2355.75 100` local=`<n> <n> 2345 100`",
    },
    AcceptedRootParityResidual {
        diagram: "gitgraph",
        stem: "zed_pr_57644_gitgraph",
        detail: "scope=parity-normalized-descendants-match; svg: attr `style` mismatch upstream=`max-width: 845.25px; background-color: white;` local=`max-width: 845px; background-color: white;`; additional DOM differences (1): svg: attr `viewBox` mismatch upstream=`<n> <n> 845.25 370.5` local=`<n> <n> 845 370.25`",
    },
    AcceptedRootParityResidual {
        diagram: "mindmap",
        stem: "upstream_docs_example_icons_br",
        detail: "scope=parity-normalized-descendants-match; svg: attr `style` mismatch upstream=`max-width: 756.25px; background-color: white;` local=`max-width: 756.75px; background-color: white;`; additional DOM differences (1): svg: attr `viewBox` mismatch upstream=`<n> <n> 756.25 721` local=`<n> <n> 756.75 721`",
    },
    AcceptedRootParityResidual {
        diagram: "mindmap",
        stem: "upstream_examples_mindmap_basic_mindmap_001",
        detail: "scope=parity-normalized-descendants-match; svg: attr `style` mismatch upstream=`max-width: 756.25px; background-color: white;` local=`max-width: 756.75px; background-color: white;`; additional DOM differences (1): svg: attr `viewBox` mismatch upstream=`<n> <n> 756.25 721` local=`<n> <n> 756.75 721`",
    },
    AcceptedRootParityResidual {
        diagram: "railroad",
        stem: "basic_ir",
        detail: "scope=parity-normalized-descendants-match; svg: attr `viewBox` mismatch upstream=`<n> <n> 368 194.5` local=`<n> <n> 368 192.25`",
    },
    AcceptedRootParityResidual {
        diagram: "railroadEbnf",
        stem: "choice_optional_repetition",
        detail: "scope=parity-normalized-descendants-match; svg: attr `viewBox` mismatch upstream=`<n> <n> 397.25 174` local=`<n> <n> 397.25 171.25`",
    },
    AcceptedRootParityResidual {
        diagram: "railroadAbnf",
        stem: "repetition_optional_numval",
        detail: "scope=parity-normalized-descendants-match; svg: attr `viewBox` mismatch upstream=`<n> <n> 289.75 221` local=`<n> <n> 289.75 216.75`",
    },
    AcceptedRootParityResidual {
        diagram: "railroadPeg",
        stem: "prefix_suffix_any",
        detail: "scope=parity-normalized-descendants-match; svg: attr `viewBox` mismatch upstream=`<n> <n> 431.75 107` local=`<n> <n> 431.75 105.5`",
    },
];

#[derive(Debug, Clone, Copy)]
struct AcceptedDomParityResidual {
    diagram: &'static str,
    stem: &'static str,
    detail: &'static str,
}

const ACCEPTED_DOM_PARITY_RESIDUALS: &[AcceptedDomParityResidual] = &[
    AcceptedDomParityResidual {
        diagram: "sequence",
        stem: "upstream_cypress_sequencediagram_spec_should_render_long_notes_wrapped_inline_left_of_actor_026",
        detail: "svg/g[16]: child count mismatch upstream=9 local=8",
    },
    AcceptedDomParityResidual {
        diagram: "sequence",
        stem: "upstream_cypress_sequencediagram_v2_spec_should_render_wrapped_long_notes_left_of_control_019",
        detail: "svg/g[20]: child count mismatch upstream=9 local=8",
    },
    AcceptedDomParityResidual {
        diagram: "sequence",
        stem: "upstream_docs_diagrams_mermaid_api_sequence",
        detail: "svg/g[61]/text[9]: attr `class` mismatch upstream=`sectionTitle` local=`loopText`; additional DOM differences (2): svg/g[61]/text[9]: child count mismatch upstream=0 local=1 | svg/g[61]: child count mismatch upstream=10 local=11",
    },
];

fn has_exact_mismatch_detail(line: &str, stem: &str, expected_detail: &str) -> bool {
    let prefix = format!("dom mismatch for {stem}: upstream=");
    let suffix = format!(" ({expected_detail})");
    let Some(summary) = line.trim().strip_suffix(&suffix) else {
        return false;
    };
    let Some(summary) = summary.strip_prefix(&prefix) else {
        return false;
    };

    summary.contains(" local=")
}

#[derive(Debug)]
pub(crate) struct DomParityResidualPolicy {
    expected: Vec<&'static AcceptedDomParityResidual>,
    seen: BTreeSet<(&'static str, &'static str)>,
}

impl DomParityResidualPolicy {
    pub(crate) fn new(diagrams: &[&str]) -> Self {
        let expected = ACCEPTED_DOM_PARITY_RESIDUALS
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
            .map(|remaining| summarize_dom_parity_failure(diagram, &remaining, report_path))
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
                    "DOM parity residual policy expected {}/{} but it was not observed; update or remove the policy only with fresh closeout evidence",
                    residual.diagram, residual.stem
                )
            })
            .collect()
    }

    fn accept_or_return_remaining(&mut self, diagram: &str, msg: &str) -> Option<String> {
        let mut remaining = Vec::new();
        for line in msg.lines().filter(|line| !line.trim().is_empty()) {
            if let Some(residual) = self.matching_residual(diagram, line) {
                if !self.seen.insert((residual.diagram, residual.stem)) {
                    remaining.push(line.to_string());
                }
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
    ) -> Option<&'static AcceptedDomParityResidual> {
        self.expected.iter().copied().find(|residual| {
            residual.diagram == diagram
                && has_exact_mismatch_detail(line, residual.stem, residual.detail)
        })
    }
}

fn summarize_dom_parity_failure(diagram: &str, msg: &str, report_path: Option<&Path>) -> String {
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
    format!("{diagram}: {count} unaccepted parity DOM mismatch(es){report}; first: {first}")
}

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
                if !self.seen.insert((residual.diagram, residual.stem)) {
                    remaining.push(line.to_string());
                }
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
                && has_exact_mismatch_detail(line, residual.stem, residual.detail)
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

    const EXACT_SEQUENCE_DOM_RESIDUALS: &str = "dom mismatch for upstream_cypress_sequencediagram_spec_should_render_long_notes_wrapped_inline_left_of_actor_026: upstream=a local=b (svg/g[16]: child count mismatch upstream=9 local=8)\n\
dom mismatch for upstream_cypress_sequencediagram_v2_spec_should_render_wrapped_long_notes_left_of_control_019: upstream=a local=b (svg/g[20]: child count mismatch upstream=9 local=8)\n\
dom mismatch for upstream_docs_diagrams_mermaid_api_sequence: upstream=a local=b (svg/g[61]/text[9]: attr `class` mismatch upstream=`sectionTitle` local=`loopText`; additional DOM differences (2): svg/g[61]/text[9]: child count mismatch upstream=0 local=1 | svg/g[61]: child count mismatch upstream=10 local=11)";

    #[test]
    fn dom_parity_policy_accepts_only_the_three_documented_sequence_residuals() {
        let mut policy = DomParityResidualPolicy::new(&["sequence"]);

        assert!(
            policy
                .accept_or_summarize_failure("sequence", EXACT_SEQUENCE_DOM_RESIDUALS, None)
                .is_none()
        );
        assert_eq!(policy.accepted_summaries().len(), 3);
        assert!(policy.missing_failures().is_empty());
    }

    #[test]
    fn dom_parity_policy_preserves_unexpected_sequence_mismatches() {
        let mut policy = DomParityResidualPolicy::new(&["sequence"]);
        let msg = format!(
            "{EXACT_SEQUENCE_DOM_RESIDUALS}\ndom mismatch for unexpected_fixture: upstream=a local=b (svg/g[0]: child count mismatch upstream=1 local=0)"
        );

        let summary = policy
            .accept_or_summarize_failure("sequence", &msg, None)
            .expect("unexpected mismatch should remain");

        assert!(summary.contains("unexpected_fixture"));
        assert!(policy.missing_failures().is_empty());
    }

    #[test]
    fn dom_parity_policy_rejects_changed_sequence_residual_details() {
        let mut policy = DomParityResidualPolicy::new(&["sequence"]);
        let changed = EXACT_SEQUENCE_DOM_RESIDUALS.replace("local=8", "local=7");

        let summary = policy
            .accept_or_summarize_failure("sequence", &changed, None)
            .expect("changed residual should remain");

        assert!(summary.contains("local=7"));
        assert_eq!(policy.missing_failures().len(), 2);
    }

    #[test]
    fn dom_parity_policy_rejects_registered_residual_with_a_new_same_fixture_difference() {
        let mut policy = DomParityResidualPolicy::new(&["sequence"]);
        let msg = "dom mismatch for upstream_cypress_sequencediagram_spec_should_render_long_notes_wrapped_inline_left_of_actor_026: upstream=a local=b (svg/g[16]: child count mismatch upstream=9 local=8; additional DOM differences: svg/g[16]/text[0]: text mismatch upstream=`a` local=`b`)";

        let summary = policy
            .accept_or_summarize_failure("sequence", msg, None)
            .expect("a newly added same-fixture difference must remain actionable");

        assert!(summary.contains("additional DOM differences"));
        assert!(policy.accepted_summaries().is_empty());
        assert!(policy
            .missing_failures()
            .iter()
            .any(|failure| failure.contains("upstream_cypress_sequencediagram_spec_should_render_long_notes_wrapped_inline_left_of_actor_026")));
    }

    #[test]
    fn dom_parity_policy_requires_the_registered_detail_to_match_exactly() {
        let mut policy = DomParityResidualPolicy::new(&["sequence"]);
        let msg = "dom mismatch for upstream_cypress_sequencediagram_spec_should_render_long_notes_wrapped_inline_left_of_actor_026: upstream=changed-upstream local=changed-local (svg/g[16]: child count mismatch upstream=9 local=8 injected=true)";

        let summary = policy
            .accept_or_summarize_failure("sequence", msg, None)
            .expect("text injected into a registered detail must remain actionable");

        assert!(summary.contains("injected=true"));
        assert!(policy.accepted_summaries().is_empty());
    }

    #[test]
    fn root_parity_policy_accepts_exact_recorded_class_residuals() {
        let mut policy = RootParityResidualPolicy::new(&["class"]);
        let msg = "dom mismatch for upstream_cypress_classdiagram_elk_v3_spec_elk_should_render_classes_with_different_text_labels_037: upstream=a local=b (scope=parity-normalized-descendants-match; svg: attr `style` mismatch upstream=`max-width: 2355.75px; background-color: white;` local=`max-width: 2345px; background-color: white;`; additional DOM differences (1): svg: attr `viewBox` mismatch upstream=`<n> <n> 2355.75 100` local=`<n> <n> 2345 100`)\n\
dom mismatch for upstream_cypress_classdiagram_handdrawn_v3_spec_hd_should_render_classes_with_different_text_labels_037: upstream=a local=b (scope=parity-normalized-descendants-match; svg: attr `style` mismatch upstream=`max-width: 2355.75px; background-color: white;` local=`max-width: 2345px; background-color: white;`; additional DOM differences (1): svg: attr `viewBox` mismatch upstream=`<n> <n> 2355.75 100` local=`<n> <n> 2345 100`)";

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
        let msg = "dom mismatch for a: upstream=a local=b (scope=parity-normalized-descendants-match; svg: attr `style` mismatch)\n\
dom mismatch for b: upstream=a local=b (scope=parity-normalized-descendants-match; svg: attr `viewBox` mismatch)";

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
        let policy = RootParityResidualPolicy::new(&[
            "class",
            "gitgraph",
            "mindmap",
            "railroad",
            "railroadEbnf",
            "railroadAbnf",
            "railroadPeg",
        ]);

        let residual_lines = [
            (
                "class",
                "dom mismatch for upstream_cypress_classdiagram_elk_v3_spec_elk_should_render_classes_with_different_text_labels_037: upstream=a local=b (scope=parity-normalized-descendants-match; svg: attr `style` mismatch upstream=`max-width: 2355.75px; background-color: white;` local=`max-width: 2345px; background-color: white;`; additional DOM differences (1): svg: attr `viewBox` mismatch upstream=`<n> <n> 2355.75 100` local=`<n> <n> 2345 100`)",
            ),
            (
                "class",
                "dom mismatch for upstream_cypress_classdiagram_handdrawn_v3_spec_hd_should_render_classes_with_different_text_labels_037: upstream=a local=b (scope=parity-normalized-descendants-match; svg: attr `style` mismatch upstream=`max-width: 2355.75px; background-color: white;` local=`max-width: 2345px; background-color: white;`; additional DOM differences (1): svg: attr `viewBox` mismatch upstream=`<n> <n> 2355.75 100` local=`<n> <n> 2345 100`)",
            ),
            (
                "gitgraph",
                "dom mismatch for zed_pr_57644_gitgraph: upstream=a local=b (scope=parity-normalized-descendants-match; svg: attr `style` mismatch upstream=`max-width: 845.25px; background-color: white;` local=`max-width: 845px; background-color: white;`; additional DOM differences (1): svg: attr `viewBox` mismatch upstream=`<n> <n> 845.25 370.5` local=`<n> <n> 845 370.25`)",
            ),
            (
                "mindmap",
                "dom mismatch for upstream_docs_example_icons_br: upstream=a local=b (scope=parity-normalized-descendants-match; svg: attr `style` mismatch upstream=`max-width: 756.25px; background-color: white;` local=`max-width: 756.75px; background-color: white;`; additional DOM differences (1): svg: attr `viewBox` mismatch upstream=`<n> <n> 756.25 721` local=`<n> <n> 756.75 721`)",
            ),
            (
                "mindmap",
                "dom mismatch for upstream_examples_mindmap_basic_mindmap_001: upstream=a local=b (scope=parity-normalized-descendants-match; svg: attr `style` mismatch upstream=`max-width: 756.25px; background-color: white;` local=`max-width: 756.75px; background-color: white;`; additional DOM differences (1): svg: attr `viewBox` mismatch upstream=`<n> <n> 756.25 721` local=`<n> <n> 756.75 721`)",
            ),
            (
                "railroad",
                "dom mismatch for basic_ir: upstream=a local=b (scope=parity-normalized-descendants-match; svg: attr `viewBox` mismatch upstream=`<n> <n> 368 194.5` local=`<n> <n> 368 192.25`)",
            ),
            (
                "railroadEbnf",
                "dom mismatch for choice_optional_repetition: upstream=a local=b (scope=parity-normalized-descendants-match; svg: attr `viewBox` mismatch upstream=`<n> <n> 397.25 174` local=`<n> <n> 397.25 171.25`)",
            ),
            (
                "railroadAbnf",
                "dom mismatch for repetition_optional_numval: upstream=a local=b (scope=parity-normalized-descendants-match; svg: attr `viewBox` mismatch upstream=`<n> <n> 289.75 221` local=`<n> <n> 289.75 216.75`)",
            ),
            (
                "railroadPeg",
                "dom mismatch for prefix_suffix_any: upstream=a local=b (scope=parity-normalized-descendants-match; svg: attr `viewBox` mismatch upstream=`<n> <n> 431.75 107` local=`<n> <n> 431.75 105.5`)",
            ),
        ];

        assert_eq!(ACCEPTED_ROOT_PARITY_RESIDUALS.len(), 9);
        assert!(
            ACCEPTED_ROOT_PARITY_RESIDUALS
                .iter()
                .all(|residual| residual.stem != "upstream_docs_tidy_tree_example_usage_002")
        );

        for (diagram, line) in residual_lines {
            assert!(
                policy.matching_residual(diagram, line).is_some(),
                "residual should match for {diagram}: {line}",
            );
        }
    }

    #[test]
    fn root_parity_policy_keeps_railroad_residuals_narrow() {
        let policy = RootParityResidualPolicy::new(&["railroad"]);
        let exact = "dom mismatch for basic_ir: upstream=a local=b (scope=parity-normalized-descendants-match; svg: attr `viewBox` mismatch upstream=`<n> <n> 368 194.5` local=`<n> <n> 368 192.25`)";

        assert!(policy.matching_residual("railroad", exact).is_some());
        assert!(policy.matching_residual("railroadEbnf", exact).is_none());
        assert!(
            policy
                .matching_residual(
                    "railroad",
                    &exact.replace("descendants-match", "descendants-differ"),
                )
                .is_none()
        );
        assert!(
            policy
                .matching_residual("railroad", &exact.replace("192.25", "192.5"))
                .is_none()
        );
    }

    #[test]
    fn root_parity_policy_preserves_unexpected_mismatches() {
        let mut policy = RootParityResidualPolicy::new(&["class"]);
        let msg = "dom mismatch for upstream_cypress_classdiagram_elk_v3_spec_elk_should_render_classes_with_different_text_labels_037: upstream=a local=b (scope=parity-normalized-descendants-match; svg: attr `style` mismatch upstream=`max-width: 2355.75px; background-color: white;` local=`max-width: 2345px; background-color: white;`; additional DOM differences (1): svg: attr `viewBox` mismatch upstream=`<n> <n> 2355.75 100` local=`<n> <n> 2345 100`)\n\
dom mismatch for upstream_cypress_classdiagram_handdrawn_v3_spec_hd_should_render_classes_with_different_text_labels_037: upstream=a local=b (scope=parity-normalized-descendants-match; svg: attr `style` mismatch upstream=`max-width: 2355.75px; background-color: white;` local=`max-width: 2345px; background-color: white;`; additional DOM differences (1): svg: attr `viewBox` mismatch upstream=`<n> <n> 2355.75 100` local=`<n> <n> 2345 100`)\n\
dom mismatch for unexpected_fixture: upstream=a local=b (scope=parity-normalized-descendants-match; svg: attr `style` mismatch)";

        let summary = policy
            .accept_or_summarize_failure("class", msg, None)
            .expect("unexpected mismatch should remain");

        assert!(summary.contains("unexpected_fixture"));
        assert!(policy.missing_failures().is_empty());
    }

    #[test]
    fn root_parity_policy_rejects_changed_residual_values() {
        let mut policy = RootParityResidualPolicy::new(&["mindmap"]);
        let msg = "dom mismatch for upstream_docs_example_icons_br: upstream=a local=b (scope=parity-normalized-descendants-match; svg: attr `style` mismatch upstream=`max-width: 756.25px; background-color: white;` local=`max-width: 756.5px; background-color: white;`; additional DOM differences (1): svg: attr `viewBox` mismatch upstream=`<n> <n> 756.25 721` local=`<n> <n> 756.75 721`)";

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

    #[test]
    fn root_parity_policy_rejects_registered_residual_with_a_new_same_fixture_difference() {
        let mut policy = RootParityResidualPolicy::new(&["gitgraph"]);
        let msg = "dom mismatch for zed_pr_57644_gitgraph: upstream=a local=b (scope=parity-normalized-descendants-match; svg: attr `style` mismatch upstream=`max-width: 845.25px; background-color: white;` local=`max-width: 845px; background-color: white;`; additional DOM differences (2): svg: attr `viewBox` mismatch upstream=`<n> <n> 845.25 370.5` local=`<n> <n> 845 370.25` | svg/g[0]: text mismatch upstream=`a` local=`b`)";

        let summary = policy
            .accept_or_summarize_failure("gitgraph", msg, None)
            .expect("a newly added same-fixture difference must remain actionable");

        assert!(summary.contains("additional DOM differences"));
        assert!(policy.accepted_summaries().is_empty());
        assert!(
            policy
                .missing_failures()
                .iter()
                .any(|failure| failure.contains("zed_pr_57644_gitgraph"))
        );
    }

    #[test]
    fn root_parity_policy_requires_the_registered_detail_to_match_exactly() {
        let mut policy = RootParityResidualPolicy::new(&["gitgraph"]);
        let msg = "dom mismatch for zed_pr_57644_gitgraph: upstream=changed-upstream local=changed-local (scope=parity-normalized-descendants-match; svg: attr `style` mismatch upstream=`max-width: 845.25px; background-color: white;` local=`max-width: 845px; background-color: white;`; additional DOM differences (1): svg: attr `viewBox` mismatch upstream=`<n> <n> 845.25 370.5` local=`<n> <n> 845 370.25` injected=true)";

        let summary = policy
            .accept_or_summarize_failure("gitgraph", msg, None)
            .expect("text injected into a registered detail must remain actionable");

        assert!(summary.contains("injected=true"));
        assert!(policy.accepted_summaries().is_empty());
    }

    #[test]
    fn root_parity_policy_rejects_tidy_tree_layout_divergence() {
        let mut policy = RootParityResidualPolicy::new(&["mindmap"]);
        let msg = "dom mismatch for upstream_docs_tidy_tree_example_usage_002: upstream=a local=b (scope=parity-normalized-descendants-match; svg: attr `style` mismatch upstream=`max-width: 1479.5px; background-color: white;` local=`max-width: 796.5px; background-color: white;`)";

        let summary = policy
            .accept_or_summarize_failure("mindmap", msg, None)
            .expect("tidy-tree layout divergence must remain actionable");

        assert!(summary.contains("upstream_docs_tidy_tree_example_usage_002"));
    }

    #[test]
    fn root_parity_policy_rejects_sequence_zed_large_geometry_drift() {
        let mut policy = RootParityResidualPolicy::new(&["sequence"]);
        let msg = "dom mismatch for zed_pr_57644_sequence: upstream=a local=b (scope=parity-normalized-descendants-match; svg: attr `viewBox` mismatch upstream=`<n> <n> 796 1096` local=`<n> <n> 796 1126`)";

        let summary = policy
            .accept_or_summarize_failure("sequence", msg, None)
            .expect("large sequence geometry drift must remain actionable");

        assert!(summary.contains("zed_pr_57644_sequence"));
        assert!(policy.accepted_summaries().is_empty());
    }

    #[test]
    fn root_parity_policy_rejects_mindmap_zed_node_relayout() {
        let mut policy = RootParityResidualPolicy::new(&["mindmap"]);
        let msg = "dom mismatch for zed_pr_57644_mindmap: upstream=a local=b (scope=parity-normalized-descendants-match; svg: attr `style` mismatch upstream=`max-width: 1199.75px; background-color: white;` local=`max-width: 1161.75px; background-color: white;`)";

        let summary = policy
            .accept_or_summarize_failure("mindmap", msg, None)
            .expect("mindmap node relayout must remain actionable");

        assert!(summary.contains("zed_pr_57644_mindmap"));
        assert!(policy.accepted_summaries().is_empty());
    }

    #[test]
    fn root_parity_policy_rejects_hidden_beyond_root_regression() {
        let mut policy = RootParityResidualPolicy::new(&["mindmap"]);
        let msg = "dom mismatch for upstream_docs_example_icons_br: upstream=a local=b (scope=parity-normalized-descendants-differ; root-viewport-also-differs=true; svg/g[0]: attr `transform` mismatch; svg: attr `style` mismatch upstream=`max-width: 756.25px; background-color: white;` local=`max-width: 756.75px; background-color: white;`)";

        let summary = policy
            .accept_or_summarize_failure("mindmap", msg, None)
            .expect("a beyond-root regression must never be accepted as a root residual");

        assert!(summary.contains("upstream_docs_example_icons_br"));
        assert!(policy.accepted_summaries().is_empty());
    }
}
