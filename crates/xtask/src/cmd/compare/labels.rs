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
        "\n## Label Metric Deltas\n\nHTML `<foreignObject>` labels and SVG `<text>` labels are paired by fixture-local DOM order. SVG text rows use emitted label-container geometry when no browser `getBBox()` dimensions are present. This report is intended to identify shared text metric drift before adding or deleting root viewport overrides.\n"
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

    for node in doc.descendants().filter(|n| n.is_element()) {
        if node.has_tag_name("foreignObject") {
            out.push(foreignobject_label_metric_sample(node));
        } else if let Some(sample) = svg_text_label_metric_sample(node) {
            out.push(sample);
        }
    }

    Ok(out)
}

fn foreignobject_label_metric_sample(fo: roxmltree::Node<'_, '_>) -> LabelMetricSample {
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

    LabelMetricSample {
        label_class,
        text: foreignobject_text(fo),
        markup: foreignobject_markup_summary(fo),
        width,
        height,
    }
}

fn svg_text_label_metric_sample(label_group: roxmltree::Node<'_, '_>) -> Option<LabelMetricSample> {
    if !label_group.has_tag_name("g")
        || !(has_class_token(label_group, "label") || has_class_token(label_group, "cluster-label"))
        || label_group
            .descendants()
            .any(|n| n.has_tag_name("foreignObject"))
    {
        return None;
    }

    let text_node = label_group.descendants().find(|n| n.has_tag_name("text"))?;
    let text = svg_text_content(text_node);
    if text.is_empty() {
        return None;
    }

    let (width, height) = svg_text_label_container_size(label_group)?;
    Some(LabelMetricSample {
        label_class: svg_text_label_class(label_group),
        text,
        markup: svg_text_markup_summary(text_node),
        width,
        height,
    })
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

fn svg_text_content(text: roxmltree::Node<'_, '_>) -> String {
    let mut lines = Vec::new();
    for outer in text
        .children()
        .filter(|n| n.has_tag_name("tspan") && has_class_token(*n, "text-outer-tspan"))
    {
        let line = normalize_text_line(&collect_text(outer));
        if !line.is_empty() {
            lines.push(line);
        }
    }

    if lines.is_empty() {
        let line = normalize_text_line(&collect_text(text));
        if !line.is_empty() {
            lines.push(line);
        }
    }

    lines.join("\\n")
}

fn svg_text_markup_summary(text: roxmltree::Node<'_, '_>) -> String {
    let mut parts = vec![format!("svgText:{}lines", svg_text_line_count(text).max(1))];
    for node in text.descendants().filter(|n| n.has_tag_name("tspan")) {
        if let Some(weight) = node.attribute("font-weight") {
            if weight != "normal" {
                parts.push(format!("weight:{weight}"));
            }
        }
        if let Some(style) = node.attribute("font-style") {
            if style != "normal" {
                parts.push(format!("style:{style}"));
            }
        }
    }
    parts.sort();
    parts.dedup();
    parts.join(" ")
}

fn svg_text_line_count(text: roxmltree::Node<'_, '_>) -> usize {
    text.children()
        .filter(|n| n.has_tag_name("tspan") && has_class_token(*n, "text-outer-tspan"))
        .count()
}

fn svg_text_label_class(label_group: roxmltree::Node<'_, '_>) -> String {
    if self_or_ancestor_has_class(label_group, "edgeLabel") {
        "edgeLabel".to_string()
    } else if self_or_ancestor_has_class(label_group, "cluster-label") {
        "clusterLabel".to_string()
    } else if self_or_ancestor_has_class(label_group, "node") {
        "nodeLabel".to_string()
    } else {
        label_group
            .attribute("class")
            .unwrap_or_default()
            .to_string()
    }
}

fn svg_text_label_container_size(label_group: roxmltree::Node<'_, '_>) -> Option<(f64, f64)> {
    let owner = nearest_label_owner(label_group)?;
    owner
        .descendants()
        .filter(|n| n.is_element() && has_class_token(*n, "label-container"))
        .filter(|n| !is_descendant_of(*n, label_group))
        .find_map(element_bbox_size)
}

fn nearest_label_owner<'a, 'input>(
    label_group: roxmltree::Node<'a, 'input>,
) -> Option<roxmltree::Node<'a, 'input>> {
    let mut current = Some(label_group);
    while let Some(node) = current {
        if has_class_token(node, "node")
            || has_class_token(node, "edgeLabel")
            || has_class_token(node, "cluster")
        {
            return Some(node);
        }
        current = node.parent();
    }
    None
}

fn element_bbox_size(node: roxmltree::Node<'_, '_>) -> Option<(f64, f64)> {
    match node.tag_name().name() {
        "rect" | "image" | "foreignObject" => {
            Some((attr_f64(node, "width")?, attr_f64(node, "height")?))
        }
        "circle" => {
            let r = attr_f64(node, "r")?;
            Some((r * 2.0, r * 2.0))
        }
        "ellipse" => Some((attr_f64(node, "rx")? * 2.0, attr_f64(node, "ry")? * 2.0)),
        "polygon" | "polyline" => bbox_size_from_points(node.attribute("points")?),
        _ => None,
    }
}

fn bbox_size_from_points(points: &str) -> Option<(f64, f64)> {
    let nums = points
        .split(|ch: char| ch.is_ascii_whitespace() || ch == ',')
        .filter(|part| !part.is_empty())
        .map(str::parse::<f64>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    if nums.len() < 4 || nums.len() % 2 != 0 {
        return None;
    }

    let mut min_x = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for pair in nums.chunks_exact(2) {
        min_x = min_x.min(pair[0]);
        max_x = max_x.max(pair[0]);
        min_y = min_y.min(pair[1]);
        max_y = max_y.max(pair[1]);
    }
    Some((max_x - min_x, max_y - min_y))
}

fn attr_f64(node: roxmltree::Node<'_, '_>, name: &str) -> Option<f64> {
    node.attribute(name)?.parse().ok()
}

fn collect_text(node: roxmltree::Node<'_, '_>) -> String {
    let mut raw = String::new();
    for n in node.descendants() {
        if n.is_text() {
            if let Some(text) = n.text() {
                raw.push_str(text);
            }
        }
    }
    raw
}

fn normalize_text_line(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn has_class_token(node: roxmltree::Node<'_, '_>, token: &str) -> bool {
    node.attribute("class")
        .unwrap_or_default()
        .split_whitespace()
        .any(|part| part == token)
}

fn self_or_ancestor_has_class(node: roxmltree::Node<'_, '_>, token: &str) -> bool {
    let mut current = Some(node);
    while let Some(n) = current {
        if has_class_token(n, token) {
            return true;
        }
        current = n.parent();
    }
    false
}

fn is_descendant_of(node: roxmltree::Node<'_, '_>, ancestor: roxmltree::Node<'_, '_>) -> bool {
    let mut current = node.parent();
    while let Some(n) = current {
        if n == ancestor {
            return true;
        }
        current = n.parent();
    }
    false
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

    #[test]
    fn label_metric_deltas_extract_svg_text_container_geometry() {
        let upstream = r#"<svg><g class="node default"><polygon class="label-container" points="-10,0 110,0 100,-40 0,-40"/><g class="label"><text><tspan class="text-outer-tspan"><tspan font-weight="bold">Hello</tspan></tspan><tspan class="text-outer-tspan"><tspan>World</tspan></tspan></text></g></g></svg>"#;
        let local = r#"<svg><g class="node default"><polygon class="label-container" points="-10,0 108,0 98,-40 0,-40"/><g class="label"><text><tspan class="text-outer-tspan"><tspan font-weight="bold">Hello</tspan></tspan><tspan class="text-outer-tspan"><tspan>World</tspan></tspan></text></g></g></svg>"#;

        let rows = collect_label_metric_deltas("fixture", upstream, local, true).unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].label_class, "nodeLabel");
        assert_eq!(rows[0].text, "Hello\\nWorld");
        assert!(rows[0].markup.contains("svgText:2lines"));
        assert!(rows[0].markup.contains("weight:bold"));
        assert_eq!(rows[0].upstream_width, 120.0);
        assert_eq!(rows[0].local_width, 118.0);
        assert_eq!(rows[0].width_delta, -2.0);
        assert_eq!(rows[0].height_delta, 0.0);
    }
}
