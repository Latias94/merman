//! Shared root SVG viewport reporting helpers for compare commands.

use crate::XtaskError;
use std::collections::BTreeMap;
use std::fmt::Write as _;

pub(crate) const DEFAULT_ROOT_DELTA_REPORT_LIMIT: RootDeltaReportLimit =
    RootDeltaReportLimit::Top(25);
const ROOT_STYLE_VIEWBOX_ROUNDING_EPSILON_PX: f64 = 0.01;

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
    browser_math_diagnostics: Vec<RootDelta>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RootEvidencePolicy {
    pub(crate) parity_root_requested: bool,
    pub(crate) browser_math_dimensions_are_diagnostic: bool,
    pub(crate) report_delta: bool,
}

impl RootCoverageSummary {
    pub(crate) fn record_exact_root_gate(&mut self) {
        self.exact_root_gated += 1;
    }

    pub(crate) fn record_browser_math_diagnostic(&mut self, delta: RootDelta) {
        self.browser_math_diagnostics.push(delta);
    }

    pub(crate) fn write_report(&self, report: &mut String) {
        let _ = writeln!(
            report,
            "\n## Root Coverage\n\n- Exact root-gated rendered fixtures: `{}`\n- Browser-measured math roots with structural gates and diagnostic-only dimensions: `{}`\n",
            self.exact_root_gated,
            self.browser_math_diagnostics.len()
        );
        if self.browser_math_diagnostics.is_empty() {
            return;
        }

        let _ = writeln!(
            report,
            "Every row below passed required `viewBox`, positive finite dimensions, `max-width`, width/height, and non-numeric root-style checks. Numeric dimensions remain diagnostic because the host browser measures native MathML.\n"
        );
        write_root_delta_rows(report, &self.browser_math_diagnostics);
    }
}

#[derive(Debug, PartialEq)]
struct BrowserMathRootContract {
    viewbox: (f64, f64, f64, f64),
    width: Option<String>,
    height: Option<String>,
    style_without_max_width: BTreeMap<String, String>,
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

pub(crate) fn parse_root_attrs(svg: &str) -> Result<RootAttrs, String> {
    let svg = crate::svgdom::normalize_xml_entities(svg);
    let doc = roxmltree::Document::parse(svg.as_ref()).map_err(|e| e.to_string())?;
    let root = doc
        .descendants()
        .find(|n| n.has_tag_name("svg"))
        .ok_or_else(|| "missing <svg> root".to_string())?;
    let viewbox = root.attribute("viewBox").and_then(parse_viewbox);
    let max_width_px = root
        .attribute("style")
        .and_then(parse_style_max_width_px)
        .filter(|v| v.is_finite() && *v > 0.0);
    Ok(RootAttrs {
        viewbox,
        max_width_px,
    })
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
    let upstream = parse_root_attrs(upstream_svg).map_err(|e| format!("upstream {stem}: {e}"))?;
    let local = parse_root_attrs(local_svg).map_err(|e| format!("local {stem}: {e}"))?;
    let max_width_delta = match (upstream.max_width_px, local.max_width_px) {
        (Some(a), Some(b)) => Some(b - a),
        _ => None,
    };
    Ok(RootDelta {
        stem: stem.to_string(),
        upstream,
        local,
        max_width_delta,
    })
}

pub(crate) fn validate_browser_measured_math_root_contract(
    stem: &str,
    upstream_svg: &str,
    local_svg: &str,
) -> Result<RootDelta, String> {
    let upstream = parse_browser_math_root_contract(upstream_svg)
        .map_err(|error| format!("upstream {stem}: {error}"))?;
    let local = parse_browser_math_root_contract(local_svg)
        .map_err(|error| format!("local {stem}: {error}"))?;

    if upstream.width != local.width {
        return Err(format!(
            "{stem}: browser-measured math root width contract changed: upstream={:?} local={:?}",
            upstream.width, local.width
        ));
    }
    if upstream.height != local.height {
        return Err(format!(
            "{stem}: browser-measured math root height contract changed: upstream={:?} local={:?}",
            upstream.height, local.height
        ));
    }
    if upstream.style_without_max_width != local.style_without_max_width {
        return Err(format!(
            "{stem}: browser-measured math root non-numeric style contract changed: upstream={:?} local={:?}",
            upstream.style_without_max_width, local.style_without_max_width
        ));
    }
    if upstream.viewbox.0 != local.viewbox.0 || upstream.viewbox.1 != local.viewbox.1 {
        return Err(format!(
            "{stem}: browser-measured math root viewBox origin changed: upstream=({}, {}) local=({}, {})",
            upstream.viewbox.0, upstream.viewbox.1, local.viewbox.0, local.viewbox.1
        ));
    }

    collect_root_delta(stem, upstream_svg, local_svg)
}

pub(crate) fn record_fixture_root_evidence(
    coverage: &mut RootCoverageSummary,
    reported_deltas: &mut Vec<RootDelta>,
    stem: &str,
    upstream_svg: &str,
    local_svg: &str,
    policy: RootEvidencePolicy,
) -> Result<(), String> {
    if policy.browser_math_dimensions_are_diagnostic && !policy.parity_root_requested {
        return Err(format!(
            "browser-measured math root dimensions for {stem} can be diagnostic only under parity-root"
        ));
    }
    if !policy.parity_root_requested && !policy.report_delta {
        return Ok(());
    }

    let delta = if policy.browser_math_dimensions_are_diagnostic {
        Some(
            validate_browser_measured_math_root_contract(stem, upstream_svg, local_svg).map_err(
                |error| format!("browser-measured math root contract failed for {stem}: {error}"),
            )?,
        )
    } else if policy.report_delta {
        Some(
            collect_root_delta(stem, upstream_svg, local_svg)
                .map_err(|error| format!("root parse failed for {stem}: {error}"))?,
        )
    } else {
        None
    };

    if policy.parity_root_requested {
        if policy.browser_math_dimensions_are_diagnostic {
            coverage.record_browser_math_diagnostic(
                delta
                    .as_ref()
                    .expect("browser-math root validation produced a delta")
                    .clone(),
            );
        } else {
            coverage.record_exact_root_gate();
        }
    }
    if policy.report_delta {
        reported_deltas.push(delta.expect("reported root evidence produced a delta"));
    }
    Ok(())
}

fn parse_browser_math_root_contract(svg: &str) -> Result<BrowserMathRootContract, String> {
    let svg = crate::svgdom::normalize_xml_entities(svg);
    let doc = roxmltree::Document::parse(svg.as_ref()).map_err(|error| error.to_string())?;
    let root = doc
        .descendants()
        .find(|node| node.has_tag_name("svg"))
        .ok_or_else(|| "missing <svg> root".to_string())?;
    let viewbox_raw = root
        .attribute("viewBox")
        .ok_or_else(|| "missing required viewBox".to_string())?;
    let viewbox = parse_viewbox(viewbox_raw)
        .ok_or_else(|| format!("invalid required viewBox: {viewbox_raw:?}"))?;
    if viewbox.2 <= 0.0 || viewbox.3 <= 0.0 {
        return Err(format!(
            "viewBox dimensions must be positive and finite: {viewbox_raw:?}"
        ));
    }

    let style = root
        .attribute("style")
        .ok_or_else(|| "missing required root style".to_string())?;
    let (max_width_px, style_without_max_width) = parse_browser_math_root_style(style)?;
    if (max_width_px - viewbox.2).abs() > ROOT_STYLE_VIEWBOX_ROUNDING_EPSILON_PX {
        return Err(format!(
            "root max-width must describe its viewBox width: max-width={max_width_px} viewBox-width={}",
            viewbox.2
        ));
    }

    Ok(BrowserMathRootContract {
        viewbox,
        width: root.attribute("width").map(str::to_string),
        height: root.attribute("height").map(str::to_string),
        style_without_max_width,
    })
}

fn parse_browser_math_root_style(style: &str) -> Result<(f64, BTreeMap<String, String>), String> {
    let mut max_width_px = None;
    let mut remaining = BTreeMap::new();
    for declaration in style.split(';') {
        let declaration = declaration.trim();
        if declaration.is_empty() {
            continue;
        }
        let (name, value) = declaration
            .split_once(':')
            .ok_or_else(|| format!("invalid root style declaration: {declaration:?}"))?;
        let name = name.trim().to_ascii_lowercase();
        let value = value.trim();
        if name.is_empty() || value.is_empty() {
            return Err(format!("invalid root style declaration: {declaration:?}"));
        }
        if name == "max-width" {
            if max_width_px.is_some() {
                return Err("duplicate root max-width declaration".to_string());
            }
            let lower = value.to_ascii_lowercase();
            if !lower.ends_with("px") {
                return Err(format!("root max-width must use px: {value:?}"));
            }
            let numeric = value[..value.len() - 2].trim();
            let parsed = numeric
                .parse::<f64>()
                .map_err(|_| format!("invalid root max-width: {value:?}"))?;
            if !parsed.is_finite() || parsed <= 0.0 {
                return Err(format!(
                    "root max-width must be positive and finite: {value:?}"
                ));
            }
            max_width_px = Some(parsed);
        } else if remaining.insert(name.clone(), value.to_string()).is_some() {
            return Err(format!("duplicate root style declaration: {name}"));
        }
    }

    Ok((
        max_width_px.ok_or_else(|| "missing required root max-width".to_string())?,
        remaining,
    ))
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

    #[test]
    fn parses_svg_root_viewbox_and_max_width() {
        let svg = r#"<svg viewBox="-50 -10 1144 259" style="max-width: 1144px; background-color: white;"><g/></svg>"#;
        let attrs = parse_root_attrs(svg).expect("root attrs");

        assert_eq!(attrs.viewbox, Some((-50.0, -10.0, 1144.0, 259.0)));
        assert_eq!(attrs.max_width_px, Some(1144.0));
    }

    #[test]
    fn parses_root_attrs_after_dom_compare_xml_normalization() {
        let svg = r#"<svg viewBox="0 0 10 20" style="max-width: 10px;"><foreignObject><div><img src=x>&nbsp;</div></foreignObject></svg>"#;
        let attrs = parse_root_attrs(svg).expect("root attrs");

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
    fn browser_math_root_contract_allows_only_dimension_drift() {
        let upstream = r#"<svg width="100%" viewBox="-50 -10 550 273" style="max-width: 550px; background-color: white;"><g/></svg>"#;
        let local = r#"<svg style="background-color: white; max-width: 552.125px;" viewBox="-50 -10 552.125 281" width="100%"><g/></svg>"#;

        let delta = validate_browser_measured_math_root_contract("sequence-math", upstream, local)
            .expect("browser-dependent dimensions should remain diagnostic");

        assert_eq!(delta.max_width_delta, Some(2.125));
        assert_eq!(delta.local.viewbox, Some((-50.0, -10.0, 552.125, 281.0)));
    }

    #[test]
    fn browser_math_root_contract_rejects_missing_or_invalid_dimensions() {
        let upstream = r#"<svg width="100%" viewBox="0 0 100 50" style="max-width: 100px; background-color: white;"/>"#;
        let invalid = [
            (
                r#"<svg width="100%" style="max-width: 100px; background-color: white;"/>"#,
                "missing required viewBox",
            ),
            (
                r#"<svg width="100%" viewBox="0 bad 100 50" style="max-width: 100px; background-color: white;"/>"#,
                "invalid required viewBox",
            ),
            (
                r#"<svg width="100%" viewBox="0 0 100 0" style="max-width: 100px; background-color: white;"/>"#,
                "dimensions must be positive",
            ),
            (
                r#"<svg width="100%" viewBox="0 0 100 50" style="background-color: white;"/>"#,
                "missing required root max-width",
            ),
            (
                r#"<svg width="100%" viewBox="0 0 100 50" style="max-width: 100%; background-color: white;"/>"#,
                "must use px",
            ),
            (
                r#"<svg width="100%" viewBox="0 0 100 50" style="max-width: 80px; background-color: white;"/>"#,
                "must describe its viewBox width",
            ),
        ];

        for (local, expected) in invalid {
            let error =
                validate_browser_measured_math_root_contract("invalid-math-root", upstream, local)
                    .expect_err("invalid root contract must fail closed");
            assert!(error.contains(expected), "error={error}");
        }
    }

    #[test]
    fn browser_math_root_contract_keeps_deterministic_root_structure_exact() {
        let upstream = r#"<svg width="100%" viewBox="0 0 100 50" style="max-width: 100px; background-color: white;"/>"#;
        let invalid = [
            (
                r#"<svg width="800" viewBox="0 0 100 50" style="max-width: 100px; background-color: white;"/>"#,
                "width contract changed",
            ),
            (
                r#"<svg width="100%" height="50" viewBox="0 0 100 50" style="max-width: 100px; background-color: white;"/>"#,
                "height contract changed",
            ),
            (
                r#"<svg width="100%" viewBox="0 0 100 50" style="max-width: 100px; background-color: transparent;"/>"#,
                "non-numeric style contract changed",
            ),
            (
                r#"<svg width="100%" viewBox="1 0 100 50" style="max-width: 100px; background-color: white;"/>"#,
                "viewBox origin changed",
            ),
        ];

        for (local, expected) in invalid {
            let error =
                validate_browser_measured_math_root_contract("changed-math-root", upstream, local)
                    .expect_err("deterministic root structure must remain exact");
            assert!(error.contains(expected), "error={error}");
        }
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
    fn root_coverage_report_always_lists_every_browser_math_exception() {
        let upstream = r#"<svg width="100%" viewBox="0 0 100 100" style="max-width: 100px;"/>"#;
        let mut coverage = RootCoverageSummary::default();
        coverage.record_exact_root_gate();
        coverage.record_browser_math_diagnostic(
            collect_root_delta(
                "width-drift",
                upstream,
                r#"<svg width="100%" viewBox="0 0 101 100" style="max-width: 101px;"/>"#,
            )
            .unwrap(),
        );
        coverage.record_browser_math_diagnostic(
            collect_root_delta(
                "height-only-drift",
                upstream,
                r#"<svg width="100%" viewBox="0 0 100 104" style="max-width: 100px;"/>"#,
            )
            .unwrap(),
        );

        let mut report = String::new();
        coverage.write_report(&mut report);

        assert!(report.contains("Exact root-gated rendered fixtures: `1`"));
        assert!(report.contains("diagnostic-only dimensions: `2`"));
        assert!(report.contains("| `width-drift` |"));
        assert!(report.contains("| `height-only-drift` |"));
        assert!(report.contains("100.000×104.000"));
    }
}
