//! Shared root SVG viewport reporting helpers for compare commands.

use crate::XtaskError;
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

pub(crate) fn parse_viewbox(v: &str) -> Option<(f64, f64, f64, f64)> {
    let parts = v
        .split_whitespace()
        .filter_map(|t| t.parse::<f64>().ok())
        .collect::<Vec<_>>();
    if parts.len() == 4 {
        Some((parts[0], parts[1], parts[2], parts[3]))
    } else {
        None
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
    let doc = roxmltree::Document::parse(svg).map_err(|e| e.to_string())?;
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

#[cfg(test)]
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

    let _ = writeln!(
        report,
        "| Fixture | upstream max-width(px) | local max-width(px) | Δ | upstream viewBox(w×h) | local viewBox(w×h) |\n|---|---:|---:|---:|---:|---:|"
    );
    for d in root_deltas.iter().take(take) {
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
    let _ = writeln!(
        report,
        "\nNote: These deltas are a symptom of numeric layout/text-metrics drift; matching them requires moving closer to upstream measurement behavior.\n"
    );
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
}
