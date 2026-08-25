//! Shared root SVG viewport reporting helpers for compare commands.

use crate::XtaskError;
use crate::svgdom::ParsedSvgDom;
use std::fmt::Write as _;

pub(crate) const DEFAULT_ROOT_DELTA_REPORT_LIMIT: RootDeltaReportLimit =
    RootDeltaReportLimit::Top(25);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RootDeltaReportLimit {
    Top(usize),
    All,
}

impl RootDeltaReportLimit {
    fn take_count(self, total: usize) -> usize {
        match self {
            Self::Top(limit) => total.min(limit),
            Self::All => total,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RootAttrs {
    pub(crate) viewbox: Option<(f64, f64, f64, f64)>,
    pub(crate) max_width_px: Option<f64>,
}

#[derive(Debug, Clone)]
pub(crate) struct RootDelta {
    pub(crate) stem: String,
    pub(crate) upstream: RootAttrs,
    pub(crate) local: RootAttrs,
    pub(crate) max_width_delta: Option<f64>,
}

#[derive(Debug, Default)]
pub(crate) struct RootCoverageSummary {
    exact_root_gated: usize,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RootEvidencePolicy {
    pub(crate) parity_root_requested: bool,
    pub(crate) report_delta: bool,
}

impl RootCoverageSummary {
    pub(crate) fn record_exact_root_gate(&mut self) {
        self.exact_root_gated += 1;
    }

    pub(crate) fn write_report(&self, report: &mut String) {
        let _ = writeln!(
            report,
            "\n## Root Coverage\n\n- Exact root-gated rendered fixtures: `{}`\n",
            self.exact_root_gated
        );
    }
}

pub(crate) fn parse_viewbox(v: &str) -> Option<(f64, f64, f64, f64)> {
    let parts = v.split_whitespace().collect::<Vec<_>>();
    if parts.len() != 4 {
        None
    } else {
        let x = parts[0].parse::<f64>().ok()?;
        let y = parts[1].parse::<f64>().ok()?;
        let width = parts[2].parse::<f64>().ok()?;
        let height = parts[3].parse::<f64>().ok()?;
        [x, y, width, height]
            .iter()
            .all(|value| value.is_finite())
            .then_some((x, y, width, height))
    }
}

pub(crate) fn parse_style_max_width_px(style: &str) -> Option<f64> {
    let style = style.to_ascii_lowercase();
    let key = "max-width:";
    let i = style.find(key)?;
    let rest = &style[i + key.len()..];
    let rest = rest.trim_start();
    let mut num = String::new();
    for ch in rest.chars() {
        if ch.is_ascii_digit() || matches!(ch, '.' | '-' | '+' | 'e' | 'E') {
            num.push(ch);
        } else {
            break;
        }
    }
    num.trim().parse::<f64>().ok()
}

fn parse_root_attrs_from_dom(document: &ParsedSvgDom<'_>) -> RootAttrs {
    let root = document.svg_root();
    let viewbox = root.attribute("viewBox").and_then(parse_viewbox);
    let max_width_px = root
        .attribute("style")
        .and_then(parse_style_max_width_px)
        .filter(|v| v.is_finite() && *v > 0.0);
    RootAttrs {
        viewbox,
        max_width_px,
    }
}

pub(crate) fn parse_root_delta_report_limit(
    value: Option<&str>,
) -> Result<RootDeltaReportLimit, XtaskError> {
    let value = value.ok_or(XtaskError::Usage)?.trim();
    if value.eq_ignore_ascii_case("all") {
        return Ok(RootDeltaReportLimit::All);
    }
    let limit = value.parse::<usize>().map_err(|_| XtaskError::Usage)?;
    if limit == 0 {
        return Err(XtaskError::Usage);
    }
    Ok(RootDeltaReportLimit::Top(limit))
}

pub(crate) fn collect_root_delta(
    stem: &str,
    upstream_svg: &str,
    local_svg: &str,
) -> Result<RootDelta, String> {
    with_parsed_svg_pair(stem, upstream_svg, local_svg, |upstream, local| {
        Ok(collect_root_delta_from_dom(stem, upstream, local))
    })
}

pub(crate) fn collect_root_delta_from_dom(
    stem: &str,
    upstream_document: &ParsedSvgDom<'_>,
    local_document: &ParsedSvgDom<'_>,
) -> RootDelta {
    let upstream = parse_root_attrs_from_dom(upstream_document);
    let local = parse_root_attrs_from_dom(local_document);
    let max_width_delta = match (upstream.max_width_px, local.max_width_px) {
        (Some(a), Some(b)) => Some(b - a),
        _ => None,
    };
    RootDelta {
        stem: stem.to_string(),
        upstream,
        local,
        max_width_delta,
    }
}

pub(crate) fn record_fixture_root_evidence(
    coverage: &mut RootCoverageSummary,
    reported_deltas: &mut Vec<RootDelta>,
    stem: &str,
    upstream_svg: &str,
    local_svg: &str,
    policy: RootEvidencePolicy,
) -> Result<(), String> {
    record_fixture_root_evidence_with(coverage, reported_deltas, stem, policy, || {
        collect_root_delta(stem, upstream_svg, local_svg)
    })
}

pub(crate) fn record_fixture_root_evidence_from_dom(
    coverage: &mut RootCoverageSummary,
    reported_deltas: &mut Vec<RootDelta>,
    stem: &str,
    upstream_document: &ParsedSvgDom<'_>,
    local_document: &ParsedSvgDom<'_>,
    policy: RootEvidencePolicy,
) -> Result<(), String> {
    record_fixture_root_evidence_with(coverage, reported_deltas, stem, policy, || {
        Ok(collect_root_delta_from_dom(
            stem,
            upstream_document,
            local_document,
        ))
    })
}

fn record_fixture_root_evidence_with(
    coverage: &mut RootCoverageSummary,
    reported_deltas: &mut Vec<RootDelta>,
    stem: &str,
    policy: RootEvidencePolicy,
    collect_delta: impl FnOnce() -> Result<RootDelta, String>,
) -> Result<(), String> {
    if !policy.parity_root_requested && !policy.report_delta {
        return Ok(());
    }

    let delta = if policy.report_delta {
        Some(collect_delta().map_err(|error| format!("root parse failed for {stem}: {error}"))?)
    } else {
        None
    };

    if policy.parity_root_requested {
        coverage.record_exact_root_gate();
    }
    if policy.report_delta {
        reported_deltas.push(delta.expect("reported root evidence produced a delta"));
    }
    Ok(())
}

fn with_parsed_svg_pair<T>(
    stem: &str,
    upstream_svg: &str,
    local_svg: &str,
    inspect: impl FnOnce(&ParsedSvgDom<'_>, &ParsedSvgDom<'_>) -> Result<T, String>,
) -> Result<T, String> {
    let upstream_svg = crate::svgdom::normalize_xml_entities(upstream_svg);
    let upstream_document = ParsedSvgDom::parse_normalized(upstream_svg.as_ref())
        .map_err(|error| format!("upstream {stem}: {error}"))?;
    let local_svg = crate::svgdom::normalize_xml_entities(local_svg);
    let local_document = ParsedSvgDom::parse_normalized(local_svg.as_ref())
        .map_err(|error| format!("local {stem}: {error}"))?;
    inspect(&upstream_document, &local_document)
}

pub(crate) fn write_root_deltas_report(
    report: &mut String,
    root_deltas: &mut [RootDelta],
    limit: RootDeltaReportLimit,
) {
    if root_deltas.is_empty() {
        return;
    }

    let _ = writeln!(
        report,
        "\n## Root Viewport Deltas (max-width/viewBox)\n\nThis section is mainly useful when `--dom-mode parity-root` is enabled.\n"
    );

    root_deltas.sort_by(|a, b| {
        a.max_width_delta
            .unwrap_or(0.0)
            .abs()
            .partial_cmp(&b.max_width_delta.unwrap_or(0.0).abs())
            .unwrap_or(std::cmp::Ordering::Equal)
            .reverse()
    });

    let take = limit.take_count(root_deltas.len());
    match limit {
        RootDeltaReportLimit::All => {
            let _ = writeln!(
                report,
                "Showing all {} root delta rows.\n",
                root_deltas.len()
            );
        }
        RootDeltaReportLimit::Top(_) => {
            let _ = writeln!(
                report,
                "Showing top {take} of {} root delta rows. Use `--report-root-all` or `--report-root-limit all` for a full audit table.\n",
                root_deltas.len()
            );
        }
    }

    write_root_delta_rows(report, &root_deltas[..take]);
    let _ = writeln!(
        report,
        "\nNote: These deltas are a symptom of numeric layout/text-metrics drift; matching them requires moving closer to upstream measurement behavior.\n"
    );
}

fn write_root_delta_rows(report: &mut String, root_deltas: &[RootDelta]) {
    if root_deltas.is_empty() {
        return;
    }
    let _ = writeln!(
        report,
        "| Fixture | upstream max-width(px) | local max-width(px) | Δ | upstream viewBox(w×h) | local viewBox(w×h) |\n|---|---:|---:|---:|---:|---:|"
    );
    for d in root_deltas {
        let (up_mw, lo_mw, mw_delta) = match (d.upstream.max_width_px, d.local.max_width_px) {
            (Some(a), Some(b)) => (
                format!("{a:.3}"),
                format!("{b:.3}"),
                format!("{:+.3}", b - a),
            ),
            _ => ("".to_string(), "".to_string(), "".to_string()),
        };
        let (up_vb, lo_vb) = match (d.upstream.viewbox, d.local.viewbox) {
            (Some((_, _, w, h)), Some((_, _, w2, h2))) => {
                (format!("{w:.3}×{h:.3}"), format!("{w2:.3}×{h2:.3}"))
            }
            (Some((_, _, w, h)), None) => (format!("{w:.3}×{h:.3}"), "".to_string()),
            (None, Some((_, _, w, h))) => ("".to_string(), format!("{w:.3}×{h:.3}")),
            _ => ("".to_string(), "".to_string()),
        };
        let _ = writeln!(
            report,
            "| `{}` | {} | {} | {} | {} | {} |",
            d.stem, up_mw, lo_mw, mw_delta, up_vb, lo_vb
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_test_root_attrs(svg: &str) -> Result<RootAttrs, String> {
        let svg = crate::svgdom::normalize_xml_entities(svg);
        let document = ParsedSvgDom::parse_normalized(svg.as_ref())?;
        Ok(parse_root_attrs_from_dom(&document))
    }

    #[test]
    fn parses_svg_root_viewbox_and_max_width() {
        let svg = r#"<svg viewBox="-50 -10 1144 259" style="max-width: 1144px; background-color: white;"><g/></svg>"#;
        let attrs = parse_test_root_attrs(svg).expect("root attrs");

        assert_eq!(attrs.viewbox, Some((-50.0, -10.0, 1144.0, 259.0)));
        assert_eq!(attrs.max_width_px, Some(1144.0));
    }

    #[test]
    fn parses_root_attrs_after_dom_compare_xml_normalization() {
        let svg = r#"<svg viewBox="0 0 10 20" style="max-width: 10px;"><foreignObject><div><img src=x>&nbsp;</div></foreignObject></svg>"#;
        let attrs = parse_test_root_attrs(svg).expect("root attrs");

        assert_eq!(attrs.viewbox, Some((0.0, 0.0, 10.0, 20.0)));
        assert_eq!(attrs.max_width_px, Some(10.0));
    }

    #[test]
    fn viewbox_parser_rejects_partial_or_non_finite_values() {
        assert_eq!(parse_viewbox("0 nope 10 20"), None);
        assert_eq!(parse_viewbox("0 0 10 20 trailing"), None);
        assert_eq!(parse_viewbox("0 0 inf 20"), None);
    }

    #[test]
    fn renders_root_deltas_in_descending_width_delta_order() {
        let upstream = r#"<svg viewBox="-50 -10 100 100" style="max-width: 100px;"><g/></svg>"#;
        let local_small = r#"<svg viewBox="-50 -10 101 100" style="max-width: 101px;"><g/></svg>"#;
        let local_large = r#"<svg viewBox="-50 -10 105 100" style="max-width: 105px;"><g/></svg>"#;
        let mut deltas = vec![
            collect_root_delta("small", upstream, local_small).unwrap(),
            collect_root_delta("large", upstream, local_large).unwrap(),
        ];

        let mut report = String::new();
        write_root_deltas_report(&mut report, &mut deltas, DEFAULT_ROOT_DELTA_REPORT_LIMIT);

        let large_pos = report.find("`large`").expect("large row");
        let small_pos = report.find("`small`").expect("small row");
        assert!(large_pos < small_pos);
        assert!(report.contains("| `large` | 100.000 | 105.000 | +5.000 |"));
    }

    #[test]
    fn parses_root_report_limits() {
        assert_eq!(
            parse_root_delta_report_limit(Some("all")).unwrap(),
            RootDeltaReportLimit::All
        );
        assert_eq!(
            parse_root_delta_report_limit(Some("3")).unwrap(),
            RootDeltaReportLimit::Top(3)
        );
        assert!(parse_root_delta_report_limit(Some("0")).is_err());
        assert!(parse_root_delta_report_limit(Some("nope")).is_err());
        assert!(parse_root_delta_report_limit(None).is_err());
    }

    #[test]
    fn report_limit_can_show_all_rows() {
        let upstream = r#"<svg viewBox="-50 -10 100 100" style="max-width: 100px;"><g/></svg>"#;
        let mut deltas = vec![
            collect_root_delta(
                "one",
                upstream,
                r#"<svg viewBox="-50 -10 101 100" style="max-width: 101px;"><g/></svg>"#,
            )
            .unwrap(),
            collect_root_delta(
                "two",
                upstream,
                r#"<svg viewBox="-50 -10 102 100" style="max-width: 102px;"><g/></svg>"#,
            )
            .unwrap(),
            collect_root_delta(
                "three",
                upstream,
                r#"<svg viewBox="-50 -10 103 100" style="max-width: 103px;"><g/></svg>"#,
            )
            .unwrap(),
        ];

        let mut report = String::new();
        write_root_deltas_report(&mut report, &mut deltas, RootDeltaReportLimit::All);

        assert!(report.contains("Showing all 3 root delta rows."));
        assert!(report.contains("| `one` |"));
        assert!(report.contains("| `two` |"));
        assert!(report.contains("| `three` |"));
    }

    #[test]
    fn root_coverage_report_counts_exact_root_gates() {
        let mut coverage = RootCoverageSummary::default();
        coverage.record_exact_root_gate();
        coverage.record_exact_root_gate();

        let mut report = String::new();
        coverage.write_report(&mut report);

        assert!(report.contains("Exact root-gated rendered fixtures: `2`"));
        assert!(!report.contains("diagnostic-only"));
    }
}
