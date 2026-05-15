//! Label-level SVG metric reporting helpers for compare commands.

use crate::XtaskError;
use std::fmt::Write as _;

pub(crate) const DEFAULT_LABEL_DELTA_REPORT_LIMIT: LabelDeltaReportLimit =
    LabelDeltaReportLimit::Top(80);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LabelDeltaReportLimit {
    Top(usize),
    All,
}

impl LabelDeltaReportLimit {
    fn take_count(self, total: usize) -> usize {
        match self {
            Self::Top(limit) => total.min(limit),
            Self::All => total,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct LabelMetricDelta {
    pub(crate) stem: String,
    pub(crate) index: usize,
    pub(crate) root_pinned: bool,
    pub(crate) label_class: String,
    pub(crate) text: String,
    pub(crate) markup: String,
    pub(crate) upstream_width: f64,
    pub(crate) local_width: f64,
    pub(crate) width_delta: f64,
    pub(crate) upstream_height: f64,
    pub(crate) local_height: f64,
    pub(crate) height_delta: f64,
}

#[derive(Debug, Clone)]
struct LabelMetricSample {
    label_class: String,
    text: String,
    markup: String,
    width: f64,
    height: f64,
}

pub(crate) fn parse_label_delta_report_limit(
    value: Option<&str>,
) -> Result<LabelDeltaReportLimit, XtaskError> {
    let value = value.ok_or(XtaskError::Usage)?.trim();
    if value.eq_ignore_ascii_case("all") {
        return Ok(LabelDeltaReportLimit::All);
    }
    let limit = value.parse::<usize>().map_err(|_| XtaskError::Usage)?;
    if limit == 0 {
        return Err(XtaskError::Usage);
    }
    Ok(LabelDeltaReportLimit::Top(limit))
}

pub(crate) fn collect_label_metric_deltas(
    stem: &str,
    upstream_svg: &str,
    local_svg: &str,
    root_pinned: bool,
) -> Result<Vec<LabelMetricDelta>, String> {
    let upstream =
        extract_label_metric_samples(upstream_svg).map_err(|e| format!("upstream {stem}: {e}"))?;
    let local =
        extract_label_metric_samples(local_svg).map_err(|e| format!("local {stem}: {e}"))?;

    let mut out = Vec::new();
    let len = upstream.len().min(local.len());
    for idx in 0..len {
        let up = &upstream[idx];
        let lo = &local[idx];
        let width_delta = lo.width - up.width;
        let height_delta = lo.height - up.height;
        if width_delta.abs() < 0.0005 && height_delta.abs() < 0.0005 {
            continue;
        }

        out.push(LabelMetricDelta {
            stem: stem.to_string(),
            index: idx,
            root_pinned,
            label_class: if !lo.label_class.is_empty() {
                lo.label_class.clone()
            } else {
                up.label_class.clone()
            },
            text: if !lo.text.is_empty() {
                lo.text.clone()
            } else {
                up.text.clone()
            },
            markup: if !lo.markup.is_empty() {
                lo.markup.clone()
            } else {
                up.markup.clone()
            },
            upstream_width: up.width,
            local_width: lo.width,
            width_delta,
            upstream_height: up.height,
            local_height: lo.height,
            height_delta,
        });
    }

    Ok(out)
}

pub(crate) fn write_label_deltas_report(
    report: &mut String,
    label_deltas: &mut [LabelMetricDelta],
    limit: LabelDeltaReportLimit,
) {
    if label_deltas.is_empty() {
        return;
    }

    label_deltas.sort_by(|a, b| {
        let aw = a.width_delta.abs().max(a.height_delta.abs());
        let bw = b.width_delta.abs().max(b.height_delta.abs());
        aw.partial_cmp(&bw)
            .unwrap_or(std::cmp::Ordering::Equal)
            .reverse()
    });

    let take = limit.take_count(label_deltas.len());
    let _ = writeln!(
        report,
        "\n## Label Metric Deltas\n\nForeignObject labels are paired by fixture-local DOM order. This report is intended to identify shared text metric drift before adding or deleting root viewport overrides.\n"
    );
    match limit {
        LabelDeltaReportLimit::All => {
            let _ = writeln!(
                report,
                "Showing all {} label delta rows.\n",
                label_deltas.len()
            );
        }
        LabelDeltaReportLimit::Top(_) => {
            let _ = writeln!(
                report,
                "Showing top {take} of {} label delta rows. Use `--report-label-all` or `--report-label-limit all` for a full audit table.\n",
                label_deltas.len()
            );
        }
    }

    let _ = writeln!(
        report,
        "| Fixture | root pin | # | class | upstream w×h | local w×h | Δw | Δh | text | markup |\n|---|---:|---:|---|---:|---:|---:|---:|---|---|"
    );
    for d in label_deltas.iter().take(take) {
        let _ = writeln!(
            report,
            "| `{}` | {} | {} | `{}` | {:.3}×{:.3} | {:.3}×{:.3} | {:+.3} | {:+.3} | {} | {} |",
            d.stem,
            if d.root_pinned { "yes" } else { "" },
            d.index,
            markdown_cell(&d.label_class),
            d.upstream_width,
            d.upstream_height,
            d.local_width,
            d.local_height,
            d.width_delta,
            d.height_delta,
            markdown_cell(&d.text),
            markdown_cell(&d.markup),
        );
    }
}

fn extract_label_metric_samples(svg: &str) -> Result<Vec<LabelMetricSample>, String> {
    let svg = crate::svgdom::normalize_xml_entities(svg);
    let doc = roxmltree::Document::parse(svg.as_ref()).map_err(|e| e.to_string())?;
    let mut out = Vec::new();

    for fo in doc
        .descendants()
        .filter(|n| n.has_tag_name("foreignObject"))
    {
        let width = fo
            .attribute("width")
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.0);
        let height = fo
            .attribute("height")
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(0.0);
        let label_class = fo
            .descendants()
            .find(|n| {
                n.has_tag_name("span")
                    && n.attribute("class")
                        .unwrap_or_default()
                        .split_whitespace()
                        .any(|t| t.ends_with("Label"))
            })
            .and_then(|n| n.attribute("class"))
            .unwrap_or_default()
            .to_string();

        out.push(LabelMetricSample {
            label_class,
            text: foreignobject_text(fo),
            markup: foreignobject_markup_summary(fo),
            width,
            height,
        });
    }

    Ok(out)
}

fn foreignobject_text(fo: roxmltree::Node<'_, '_>) -> String {
    let mut raw = String::new();
    for n in fo.descendants() {
        if n.is_element() {
            match n.tag_name().name() {
                "br" => raw.push('\n'),
                "p" => {
                    if !raw.is_empty() && !raw.ends_with('\n') {
                        raw.push('\n');
                    }
                }
                _ => {}
            }
        }
        if n.is_text() {
            if let Some(t) = n.text() {
                raw.push_str(t);
            }
        }
    }

    raw.split('\n')
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\\n")
}

fn foreignobject_markup_summary(fo: roxmltree::Node<'_, '_>) -> String {
    let mut parts = Vec::new();
    for n in fo.descendants().filter(|n| n.is_element()) {
        let name = n.tag_name().name();
        if !matches!(
            name,
            "i" | "img" | "strong" | "b" | "em" | "code" | "br" | "math" | "svg"
        ) {
            continue;
        }

        let class = n.attribute("class").unwrap_or_default();
        if class.is_empty() {
            parts.push(name.to_string());
        } else {
            parts.push(format!(
                "{}.{}",
                name,
                class.split_whitespace().collect::<Vec<_>>().join(".")
            ));
        }
    }
    parts.join(" ")
}

fn markdown_cell(value: &str) -> String {
    if value.is_empty() {
        return String::new();
    }
    value
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('\r', "")
        .replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_metric_deltas_extract_text_and_icon_markup() {
        let upstream = r#"<svg><foreignObject width="85.0625" height="24"><div><span class="nodeLabel"><p><i class="fa fa-twitter"></i> for peace</p></span></div></foreignObject></svg>"#;
        let local = r#"<svg><foreignObject width="89.0625" height="24"><div><span class="nodeLabel"><p><i class="fa fa-twitter"></i> for peace</p></span></div></foreignObject></svg>"#;

        let rows = collect_label_metric_deltas("fixture", upstream, local, true).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].label_class, "nodeLabel");
        assert_eq!(rows[0].text, "for peace");
        assert_eq!(rows[0].markup, "i.fa.fa-twitter");
        assert_eq!(rows[0].width_delta, 4.0);
        assert!(rows[0].root_pinned);
    }
}
