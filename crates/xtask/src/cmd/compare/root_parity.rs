//! Fail-closed residual acceptance policies for compare-all sweeps.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

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

const ACCEPTED_QUADRANT_STRUCTURE_RENDERABILITY_STEMS: &[&str] = &[
    "stress_quadrantchart_batch1_axis_missing_sides_011",
    "stress_quadrantchart_batch1_boundaries_001",
    "stress_quadrantchart_batch1_br_in_axes_and_long_quadrants_010",
    "stress_quadrantchart_batch1_config_positions_padding_006",
    "stress_quadrantchart_batch1_dense_points_overlap_003",
    "stress_quadrantchart_batch1_large_chart_many_points_012",
    "stress_quadrantchart_batch1_style_whitespace_and_order_005",
    "stress_quadrantchart_batch1_unicode_quotes_punct_009",
    "stress_quadrantchart_batch1_usemaxwidth_false_fixed_size_007",
    "upstream_cypress_quadrantchart_spec_it_should_render_data_points_with_styles_013",
    "upstream_cypress_quadrantchart_spec_it_should_render_data_points_with_styles_classes_014",
    "upstream_cypress_quadrantchart_spec_should_render_a_complete_quadrant_chart_002",
    "upstream_cypress_quadrantchart_spec_should_render_both_axes_labels_on_the_left_and_bottom_if_both_ax_012",
    "upstream_cypress_quadrantchart_spec_should_render_x_axis_labels_in_the_center_if_x_axis_has_two_labe_010",
    "upstream_cypress_quadrantchart_spec_should_render_y_axis_labels_in_the_center_if_y_axis_has_two_labe_011",
    "upstream_cypress_quadrantchart_spec_should_use_all_the_config_008",
    "upstream_examples_quadrant_chart_product_positioning_001",
    "upstream_html_demos_quadrantchart_quadrant_chart_demos_002",
    "upstream_pkgtests_quadrant_jison_spec_001",
    "upstream_pkgtests_quadrant_jison_spec_002",
    "upstream_pkgtests_quadrant_jison_spec_003",
    "upstream_pkgtests_quadrant_jison_spec_105",
    "upstream_quadrant_docs_example_reach_engagement",
    "upstream_quadrant_docs_styling_example",
    "upstream_quadrant_whole_chart_jison_spec",
    "upstream_quadrant_whole_chart_point_styles_jison_spec",
    "upstream_quadrant_whole_chart_random_style_order_and_class_jison_spec",
    "zed_pr_57644_quadrant",
];

fn dom_mismatch_paths<'a>(line: &'a str, stem: &str) -> Option<(&'a str, &'a str)> {
    let prefix = format!("dom mismatch for {stem}: upstream=");
    let body = line.trim().strip_prefix(&prefix)?;
    let (upstream_path, local_and_detail) = body.split_once(" local=")?;
    let (local_path, _) = local_and_detail.split_once(" (")?;
    (!upstream_path.is_empty() && !local_path.is_empty()).then_some((upstream_path, local_path))
}

#[derive(Debug)]
pub(crate) struct QuadrantStructureResidualPolicy {
    expected: Vec<&'static str>,
    seen: BTreeSet<&'static str>,
    decimals: u32,
}

impl QuadrantStructureResidualPolicy {
    pub(crate) fn new(diagrams: &[&str], decimals: u32) -> Self {
        let expected = if diagrams.contains(&"quadrantchart") {
            ACCEPTED_QUADRANT_STRUCTURE_RENDERABILITY_STEMS.to_vec()
        } else {
            Vec::new()
        };
        Self {
            expected,
            seen: BTreeSet::new(),
            decimals,
        }
    }

    pub(crate) fn accept_or_summarize_failure(
        &mut self,
        diagram: &str,
        msg: &str,
        report_path: Option<&Path>,
    ) -> Option<String> {
        let mut remaining = Vec::new();
        for line in msg.lines().filter(|line| !line.trim().is_empty()) {
            if let Some(stem) = self.matching_residual(diagram, line) {
                if !self.seen.insert(stem) {
                    remaining.push(line.to_string());
                }
            } else {
                remaining.push(line.to_string());
            }
        }

        (!remaining.is_empty()).then(|| {
            summarize_quadrant_structure_failure(diagram, &remaining.join("\n"), report_path)
        })
    }

    pub(crate) fn accepted_summaries(&self) -> Vec<String> {
        self.expected
            .iter()
            .filter(|stem| self.seen.contains(**stem))
            .map(|stem| format!("- quadrantchart/{stem}"))
            .collect()
    }

    pub(crate) fn missing_failures(&self) -> Vec<String> {
        self.expected
            .iter()
            .filter(|stem| !self.seen.contains(**stem))
            .map(|stem| {
                format!(
                    "QuadrantChart structure renderability policy expected quadrantchart/{stem} but it was not observed; update or remove the policy only with fresh pinned-source evidence"
                )
            })
            .collect()
    }

    fn matching_residual(&self, diagram: &str, line: &str) -> Option<&'static str> {
        if diagram != "quadrantchart" {
            return None;
        }

        self.expected.iter().copied().find(|stem| {
            let Some((upstream_path, local_path)) = dom_mismatch_paths(line, stem) else {
                return false;
            };
            let (Ok(upstream_svg), Ok(local_svg)) = (
                fs::read_to_string(upstream_path),
                fs::read_to_string(local_path),
            ) else {
                return false;
            };
            let Ok(true) = crate::svgdom::is_quadrant_structure_renderability_divergence(
                &upstream_svg,
                &local_svg,
                self.decimals,
            ) else {
                return false;
            };
            let (Ok(upstream), Ok(local)) = (
                crate::svgdom::dom_signature(
                    &upstream_svg,
                    crate::svgdom::DomMode::Structure,
                    self.decimals,
                ),
                crate::svgdom::dom_signature(
                    &local_svg,
                    crate::svgdom::DomMode::Structure,
                    self.decimals,
                ),
            ) else {
                return false;
            };
            crate::svgdom::format_dom_diffs(&crate::svgdom::dom_diffs(&upstream, &local))
                .is_some_and(|detail| has_exact_mismatch_detail(line, stem, &detail))
        })
    }
}

fn summarize_quadrant_structure_failure(
    diagram: &str,
    msg: &str,
    report_path: Option<&Path>,
) -> String {
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
    format!(
        "{diagram}: {count} unaccepted structure renderability mismatch(es){report}; first: {first}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn quadrant_svg(color: &str, extra: &str) -> String {
        format!(
            r#"<svg aria-roledescription="quadrantChart"><g class="data-points"><g class="data-point"><circle fill="{color}" stroke="{color}"/>{extra}</g></g></svg>"#
        )
    }

    fn write_quadrant_policy_pair(
        stem: &str,
        upstream: &str,
        local: &str,
    ) -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should follow Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "merman-quadrant-structure-policy-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("temporary policy directory should be created");
        let upstream_path = root.join(format!("{stem}-upstream.svg"));
        let local_path = root.join(format!("{stem}-local.svg"));
        fs::write(&upstream_path, upstream).expect("upstream probe should be written");
        fs::write(&local_path, local).expect("local probe should be written");
        (root, upstream_path, local_path)
    }

    fn structure_mismatch_line(
        stem: &str,
        upstream_path: &Path,
        local_path: &Path,
        upstream_svg: &str,
        local_svg: &str,
    ) -> String {
        let upstream =
            crate::svgdom::dom_signature(upstream_svg, crate::svgdom::DomMode::Structure, 3)
                .expect("upstream probe should parse");
        let local = crate::svgdom::dom_signature(local_svg, crate::svgdom::DomMode::Structure, 3)
            .expect("local probe should parse");
        let detail = crate::svgdom::format_dom_diffs(&crate::svgdom::dom_diffs(&upstream, &local))
            .expect("probe signatures should differ");
        format!(
            "dom mismatch for {stem}: upstream={} local={} ({detail})",
            upstream_path.display(),
            local_path.display()
        )
    }

    #[test]
    fn quadrant_structure_policy_accepts_only_a_proven_directional_divergence() {
        let stem = ACCEPTED_QUADRANT_STRUCTURE_RENDERABILITY_STEMS[0];
        let upstream = quadrant_svg("hsl(240, 100%, NaN%)", "");
        let local = quadrant_svg("rgb(185, 185, 255)", "");
        let (root, upstream_path, local_path) = write_quadrant_policy_pair(stem, &upstream, &local);
        let line = structure_mismatch_line(stem, &upstream_path, &local_path, &upstream, &local);
        let mut policy = QuadrantStructureResidualPolicy::new(&["quadrantchart"], 3);

        assert!(
            policy
                .accept_or_summarize_failure("quadrantchart", &line, None)
                .is_none()
        );
        assert_eq!(
            policy.accepted_summaries(),
            [format!("- quadrantchart/{stem}")]
        );
        assert_eq!(policy.missing_failures().len(), 27);

        fs::remove_dir_all(root).expect("temporary policy directory should be removed");
    }

    #[test]
    fn quadrant_structure_policy_rejects_an_extra_dom_difference() {
        let stem = ACCEPTED_QUADRANT_STRUCTURE_RENDERABILITY_STEMS[0];
        let upstream = quadrant_svg("hsl(240, 100%, NaN%)", "");
        let local = quadrant_svg("rgb(185, 185, 255)", "<text>extra</text>");
        let (root, upstream_path, local_path) = write_quadrant_policy_pair(stem, &upstream, &local);
        let line = structure_mismatch_line(stem, &upstream_path, &local_path, &upstream, &local);
        let mut policy = QuadrantStructureResidualPolicy::new(&["quadrantchart"], 3);

        let failure = policy
            .accept_or_summarize_failure("quadrantchart", &line, None)
            .expect("an extra DOM difference must remain actionable");
        assert!(failure.contains(stem));
        assert!(policy.accepted_summaries().is_empty());

        fs::remove_dir_all(root).expect("temporary policy directory should be removed");
    }

    #[test]
    fn quadrant_structure_registry_matches_the_pinned_nan_corpus() {
        let upstream_dir = crate::cmd::fixtures_root()
            .join("upstream-svgs")
            .join("quadrantchart");
        let mut actual = BTreeSet::new();
        for entry in fs::read_dir(&upstream_dir).expect("pinned QuadrantChart corpus should exist")
        {
            let entry = entry.expect("pinned corpus entry should be readable");
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("svg") {
                continue;
            }
            let svg = fs::read_to_string(&path).expect("pinned SVG should be readable");
            if svg.contains("hsl(240, 100%, NaN%)") {
                actual.insert(
                    path.file_stem()
                        .and_then(|value| value.to_str())
                        .expect("pinned SVG stem should be UTF-8")
                        .to_string(),
                );
            }
        }
        let expected = ACCEPTED_QUADRANT_STRUCTURE_RENDERABILITY_STEMS
            .iter()
            .map(|stem| (*stem).to_string())
            .collect::<BTreeSet<_>>();

        assert_eq!(actual, expected);
    }
}
