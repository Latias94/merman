//! Architecture debug utilities.

use crate::XtaskError;
use regex::Regex;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use super::super::svg_compare_layout_opts;

#[derive(Debug, Clone)]
struct ArchitectureFcoseProbeCli {
    fixture_filters: Vec<String>,
    out_dir: PathBuf,
    browser_exe: Option<PathBuf>,
}

#[derive(Debug, Clone)]
struct ArchitectureFcoseProbeRunSummary {
    stem: String,
    json_path: PathBuf,
    markdown_path: PathBuf,
    stage_count: usize,
    node_count: usize,
    edge_count: usize,
}

#[derive(Debug, Clone)]
struct ArchitectureDeltaRunSummary {
    stem: String,
    upstream_svg_path: PathBuf,
    local_svg_path: PathBuf,
    report_path: PathBuf,
    probe_json_path: Option<PathBuf>,
    viewbox_width_delta: Option<f64>,
    viewbox_height_delta: Option<f64>,
    max_width_delta: Option<f64>,
    root_residual_score: Option<f64>,
    service_count: usize,
    junction_count: usize,
    group_rect_count: usize,
    delta_row_count: usize,
}

#[derive(Debug, Clone)]
struct ArchitectureDeltaCli {
    fixture_filters: Vec<String>,
    out_dir: Option<PathBuf>,
    probe_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy)]
struct DebugPt {
    x: f64,
    y: f64,
}

#[derive(Debug, Clone, Copy)]
struct DebugRect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

impl DebugRect {
    fn from_model_bounds(bounds: &merman_render::model::Bounds) -> Self {
        Self {
            x: bounds.min_x,
            y: bounds.min_y,
            w: (bounds.max_x - bounds.min_x).max(0.0),
            h: (bounds.max_y - bounds.min_y).max(0.0),
        }
    }

    fn x2(self) -> f64 {
        self.x + self.w
    }

    fn y2(self) -> f64 {
        self.y + self.h
    }

    fn translated(self, dx: f64, dy: f64) -> Self {
        Self {
            x: self.x + dx,
            y: self.y + dy,
            w: self.w,
            h: self.h,
        }
    }

    fn expanded(self, by: f64) -> Self {
        let by = by.max(0.0);
        Self {
            x: self.x - by,
            y: self.y - by,
            w: self.w + by * 2.0,
            h: self.h + by * 2.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct DebugRectExpansion {
    left: f64,
    right: f64,
    top: f64,
    bottom: f64,
    dw: f64,
    dh: f64,
}

#[derive(Debug, Clone, Copy)]
enum DebugEdge {
    Left,
    Right,
    Top,
    Bottom,
}

impl DebugEdge {
    fn value(self, rect: DebugRect) -> f64 {
        match self {
            Self::Left => rect.x,
            Self::Right => rect.x2(),
            Self::Top => rect.y,
            Self::Bottom => rect.y2(),
        }
    }

    fn is_better(self, candidate: f64, current: f64) -> bool {
        match self {
            Self::Left | Self::Top => candidate < current,
            Self::Right | Self::Bottom => candidate > current,
        }
    }
}

#[derive(Debug, Clone)]
struct ArchitectureProbeNode {
    id: String,
    node_type: String,
    pos: Option<DebugPt>,
    bb: Option<DebugRect>,
    body: Option<DebugRect>,
    label: Option<DebugRect>,
    label_width: Option<f64>,
    label_height: Option<f64>,
    children_labels: Option<DebugRect>,
}

fn normalize_arch_svg_id_with_marker(id: &str, marker: &str) -> Option<String> {
    if id.starts_with(marker) {
        return Some(id.to_string());
    }

    let prefixed_marker = format!("-{marker}");
    id.find(&prefixed_marker)
        .map(|idx| id[idx + 1..].to_string())
}

fn normalize_arch_junction_svg_id(id: &str) -> Option<String> {
    if let Some(id) = normalize_arch_svg_id_with_marker(id, "junction-") {
        return Some(id);
    }

    normalize_arch_svg_id_with_marker(id, "node-").map(|id| {
        let junction = id.strip_prefix("node-").unwrap_or(&id);
        format!("junction-{junction}")
    })
}

fn architecture_delta_summary_sort_order(
    a_stem: &str,
    a_root_residual_score: Option<f64>,
    b_stem: &str,
    b_root_residual_score: Option<f64>,
) -> std::cmp::Ordering {
    let a_score = a_root_residual_score.unwrap_or(f64::NEG_INFINITY);
    let b_score = b_root_residual_score.unwrap_or(f64::NEG_INFINITY);
    b_score
        .partial_cmp(&a_score)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| a_stem.cmp(b_stem))
}

fn architecture_root_residual_score(
    max_width_delta: Option<f64>,
    viewbox_width_delta: Option<f64>,
    viewbox_height_delta: Option<f64>,
) -> Option<f64> {
    [max_width_delta, viewbox_width_delta, viewbox_height_delta]
        .into_iter()
        .flatten()
        .map(f64::abs)
        .reduce(f64::max)
}

fn parse_architecture_delta_args(args: &[String]) -> Result<ArchitectureDeltaCli, XtaskError> {
    let mut fixture_filters: Vec<String> = Vec::new();
    let mut out_dir: Option<PathBuf> = None;
    let mut probe_dir: Option<PathBuf> = None;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--fixture" => {
                i += 1;
                let Some(filter) = args.get(i).map(|s| s.trim().to_string()) else {
                    return Err(XtaskError::Usage);
                };
                fixture_filters.push(filter);
            }
            "--out" | "--out-dir" => {
                i += 1;
                out_dir = args.get(i).map(PathBuf::from);
            }
            "--probe-dir" => {
                i += 1;
                probe_dir = args.get(i).map(PathBuf::from);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    if fixture_filters.is_empty()
        || fixture_filters
            .iter()
            .any(|filter| filter.trim().is_empty())
    {
        return Err(XtaskError::Usage);
    }

    Ok(ArchitectureDeltaCli {
        fixture_filters,
        out_dir,
        probe_dir,
    })
}

fn parse_architecture_fcose_probe_args(
    args: &[String],
) -> Result<ArchitectureFcoseProbeCli, XtaskError> {
    let mut fixture_filters: Vec<String> = Vec::new();
    let mut out_dir: Option<PathBuf> = None;
    let mut browser_exe: Option<PathBuf> = None;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--fixture" => {
                i += 1;
                let Some(filter) = args.get(i).map(|s| s.trim().to_string()) else {
                    return Err(XtaskError::Usage);
                };
                fixture_filters.push(filter);
            }
            "--out" | "--out-dir" => {
                i += 1;
                out_dir = args.get(i).map(PathBuf::from);
            }
            "--browser-exe" => {
                i += 1;
                browser_exe = args.get(i).map(PathBuf::from);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    if fixture_filters.is_empty()
        || fixture_filters
            .iter()
            .any(|filter| filter.trim().is_empty())
    {
        return Err(XtaskError::Usage);
    }

    Ok(ArchitectureFcoseProbeCli {
        fixture_filters,
        out_dir: out_dir.unwrap_or_else(|| {
            crate::cmd::target_root()
                .join("debug")
                .join("architecture-fcose-probe")
        }),
        browser_exe,
    })
}

fn resolve_architecture_probe_fixture(filter: &str) -> Result<(PathBuf, String), XtaskError> {
    let fixtures_dir = crate::cmd::fixtures_root().join("architecture");
    let candidates = crate::cmd::list_mmd_fixtures_in_dir(&fixtures_dir, Some(filter), true);

    let mmd_path = match candidates.len() {
        0 => {
            return Err(XtaskError::DebugSvgFailed(format!(
                "no Architecture fixture matched {filter:?} under {}",
                fixtures_dir.display()
            )));
        }
        1 => candidates[0].clone(),
        _ => {
            let list = candidates
                .iter()
                .take(20)
                .map(|p| format!("- {}", p.display()))
                .collect::<Vec<_>>()
                .join("\n");
            return Err(XtaskError::DebugSvgFailed(format!(
                "multiple Architecture fixtures matched {filter:?}; please be more specific:\n{list}"
            )));
        }
    };

    let stem = mmd_path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| {
            XtaskError::DebugSvgFailed(format!("invalid fixture filename {}", mmd_path.display()))
        })?
        .to_string();

    Ok((mmd_path, stem))
}

fn architecture_fcose_probe_json_path(out_dir: &Path, stem: &str) -> PathBuf {
    out_dir.join(format!("{stem}.fcose-browser-probe.json"))
}

fn architecture_fcose_probe_markdown_path(out_dir: &Path, stem: &str) -> PathBuf {
    out_dir.join(format!("{stem}.fcose-browser-probe.md"))
}

fn architecture_fcose_probe_batch_markdown_path(out_dir: &Path) -> PathBuf {
    out_dir.join("architecture-fcose-probe-batch.md")
}

fn architecture_delta_batch_markdown_path(out_dir: &Path) -> PathBuf {
    out_dir.join("architecture-delta-batch.md")
}

fn json_f64(v: &serde_json::Value, key: &str) -> Option<f64> {
    v.get(key).and_then(|v| v.as_f64())
}

fn json_string<'a>(v: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(|v| v.as_str())
}

fn json_debug_point(v: Option<&serde_json::Value>) -> Option<DebugPt> {
    let v = v.filter(|v| !v.is_null())?;
    Some(DebugPt {
        x: json_f64(v, "x")?,
        y: json_f64(v, "y")?,
    })
}

fn json_debug_rect(v: Option<&serde_json::Value>) -> Option<DebugRect> {
    let v = v.filter(|v| !v.is_null())?;
    Some(DebugRect {
        x: json_f64(v, "x1")?,
        y: json_f64(v, "y1")?,
        w: json_f64(v, "w")?,
        h: json_f64(v, "h")?,
    })
}

fn architecture_probe_nodes_by_id(
    probe: &serde_json::Value,
) -> BTreeMap<String, ArchitectureProbeNode> {
    let mut out = BTreeMap::new();
    let Some(nodes) = probe
        .pointer("/finalElements/nodes")
        .and_then(|v| v.as_array())
    else {
        return out;
    };

    for node in nodes {
        let Some(id) = json_string(node, "id") else {
            continue;
        };
        let node_type = node
            .pointer("/data/type")
            .and_then(|v| v.as_str())
            .unwrap_or("<unknown>")
            .to_string();
        out.insert(
            id.to_string(),
            ArchitectureProbeNode {
                id: id.to_string(),
                node_type,
                pos: json_debug_point(node.get("pos")),
                bb: json_debug_rect(node.get("bb")),
                body: json_debug_rect(node.get("bodyBounds")),
                label: json_debug_rect(node.pointer("/labelBounds/all")),
                label_width: json_f64(node, "labelWidth")
                    .or_else(|| node.pointer("/metrics/labelWidth").and_then(|v| v.as_f64())),
                label_height: json_f64(node, "labelHeight").or_else(|| {
                    node.pointer("/metrics/labelHeight")
                        .and_then(|v| v.as_f64())
                }),
                children_labels: json_debug_rect(node.get("childrenBoundingBoxIncludeLabels")),
            },
        );
    }

    out
}

fn debug_rect_union(rects: impl IntoIterator<Item = DebugRect>) -> Option<DebugRect> {
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    let mut any = false;

    for rect in rects {
        any = true;
        min_x = min_x.min(rect.x);
        min_y = min_y.min(rect.y);
        max_x = max_x.max(rect.x2());
        max_y = max_y.max(rect.y2());
    }

    any.then_some(DebugRect {
        x: min_x,
        y: min_y,
        w: (max_x - min_x).max(0.0),
        h: (max_y - min_y).max(0.0),
    })
}

fn debug_rect_expansion(outer: DebugRect, inner: DebugRect) -> DebugRectExpansion {
    DebugRectExpansion {
        left: inner.x - outer.x,
        right: outer.x2() - inner.x2(),
        top: inner.y - outer.y,
        bottom: outer.y2() - inner.y2(),
        dw: outer.w - inner.w,
        dh: outer.h - inner.h,
    }
}

fn debug_edge_owner<'a>(
    rects: impl IntoIterator<Item = (&'a str, DebugRect)>,
    edge: DebugEdge,
) -> Option<(&'a str, f64)> {
    let mut best: Option<(&'a str, f64)> = None;
    for (id, rect) in rects {
        let value = edge.value(rect);
        if !value.is_finite() {
            continue;
        }
        if best
            .map(|(_, current)| edge.is_better(value, current))
            .unwrap_or(true)
        {
            best = Some((id, value));
        }
    }
    best
}

fn format_debug_f64(v: f64) -> String {
    format!("{v:.6}")
}

fn format_debug_optional_f64(v: Option<f64>) -> String {
    v.map(format_debug_f64)
        .unwrap_or_else(|| "<n/a>".to_string())
}

fn format_debug_point(v: Option<DebugPt>) -> String {
    v.map(|p| format!("x={:.6} y={:.6}", p.x, p.y))
        .unwrap_or_else(|| "<none>".to_string())
}

fn format_debug_rect(v: Option<DebugRect>) -> String {
    v.map(|r| format!("x={:.6} y={:.6} w={:.6} h={:.6}", r.x, r.y, r.w, r.h))
        .unwrap_or_else(|| "<none>".to_string())
}

fn format_debug_rect_expansion(v: Option<DebugRectExpansion>) -> String {
    v.map(|e| {
        format!(
            "l={:.6} r={:.6} t={:.6} b={:.6} dw={:.6} dh={:.6}",
            e.left, e.right, e.top, e.bottom, e.dw, e.dh
        )
    })
    .unwrap_or_else(|| "<none>".to_string())
}

fn format_debug_edge_owner(v: Option<(&str, f64)>) -> String {
    v.map(|(id, value)| format!("{id}@{value:.6}"))
        .unwrap_or_else(|| "<none>".to_string())
}

fn service_local_pos_key(service_id: &str) -> String {
    format!("service-{service_id}")
}

fn group_local_rect_key(group_id: &str) -> String {
    format!("group-{group_id}")
}

fn architecture_group_parent_map(semantic: &serde_json::Value) -> BTreeMap<String, Option<String>> {
    let mut out = BTreeMap::new();
    let Some(groups) = semantic.get("groups").and_then(|v| v.as_array()) else {
        return out;
    };

    for group in groups {
        let Some(id) = json_string(group, "id") else {
            continue;
        };
        out.insert(id.to_string(), json_string(group, "in").map(str::to_string));
    }

    out
}

fn format_browser_label_metrics(node: Option<&ArchitectureProbeNode>) -> String {
    match (
        node.and_then(|node| node.label_width),
        node.and_then(|node| node.label_height),
    ) {
        (Some(w), Some(h)) => format!("w={w:.6} h={h:.6}"),
        (Some(w), None) => format!("w={w:.6} h=<none>"),
        (None, Some(h)) => format!("w=<none> h={h:.6}"),
        (None, None) => "<none>".to_string(),
    }
}

fn format_local_label_metrics(
    metrics: Option<&merman_render::model::ArchitectureCytoscapeServiceLabelMetrics>,
) -> String {
    metrics
        .map(|metrics| {
            format!(
                "text_w={:.6} half={:.6} scale={:.6}",
                metrics.text_width, metrics.half_width, metrics.applied_scale
            )
        })
        .unwrap_or_else(|| "<none>".to_string())
}

fn format_probe_f64(v: f64) -> String {
    format!("{v:.3}")
}

fn format_probe_rect(v: Option<&serde_json::Value>) -> String {
    let Some(v) = v.filter(|v| !v.is_null()) else {
        return "<none>".to_string();
    };

    let mut parts = Vec::new();
    for key in ["x1", "y1", "w", "h"] {
        if let Some(n) = json_f64(v, key) {
            parts.push(format!("{key}={}", format_probe_f64(n)));
        }
    }
    if parts.is_empty() {
        "<none>".to_string()
    } else {
        parts.join(" ")
    }
}

fn format_probe_rect_expansion(
    outer: Option<&serde_json::Value>,
    inner: Option<&serde_json::Value>,
) -> String {
    let Some(outer) = outer.filter(|v| !v.is_null()) else {
        return "<none>".to_string();
    };
    let Some(inner) = inner.filter(|v| !v.is_null()) else {
        return "<none>".to_string();
    };
    let (Some(outer_x1), Some(outer_y1), Some(outer_w), Some(outer_h)) = (
        json_f64(outer, "x1"),
        json_f64(outer, "y1"),
        json_f64(outer, "w"),
        json_f64(outer, "h"),
    ) else {
        return "<none>".to_string();
    };
    let (Some(inner_x1), Some(inner_y1), Some(inner_w), Some(inner_h)) = (
        json_f64(inner, "x1"),
        json_f64(inner, "y1"),
        json_f64(inner, "w"),
        json_f64(inner, "h"),
    ) else {
        return "<none>".to_string();
    };

    let outer_x2 = outer_x1 + outer_w;
    let outer_y2 = outer_y1 + outer_h;
    let inner_x2 = inner_x1 + inner_w;
    let inner_y2 = inner_y1 + inner_h;

    format!(
        "l={} r={} t={} b={} dw={} dh={}",
        format_probe_f64(inner_x1 - outer_x1),
        format_probe_f64(outer_x2 - inner_x2),
        format_probe_f64(inner_y1 - outer_y1),
        format_probe_f64(outer_y2 - inner_y2),
        format_probe_f64(outer_w - inner_w),
        format_probe_f64(outer_h - inner_h)
    )
}

fn format_probe_point(v: Option<&serde_json::Value>) -> String {
    let Some(v) = v.filter(|v| !v.is_null()) else {
        return "<none>".to_string();
    };
    let Some(x) = json_f64(v, "x") else {
        return "<none>".to_string();
    };
    let Some(y) = json_f64(v, "y") else {
        return "<none>".to_string();
    };
    format!("x={} y={}", format_probe_f64(x), format_probe_f64(y))
}

fn format_probe_classes(v: Option<&serde_json::Value>) -> String {
    let Some(classes) = v.and_then(|v| v.as_array()) else {
        return "<none>".to_string();
    };
    let out: Vec<&str> = classes.iter().filter_map(|v| v.as_str()).collect();
    if out.is_empty() {
        "<none>".to_string()
    } else {
        out.join(" ")
    }
}

fn format_probe_config_value(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(v) => v.to_string(),
        serde_json::Value::Null => "<null>".to_string(),
        _ => serde_json::to_string(v).unwrap_or_else(|_| "<unprintable>".to_string()),
    }
}

fn render_architecture_fcose_probe_markdown(
    stem: &str,
    source_path: &Path,
    json_path: &Path,
    probe: &serde_json::Value,
) -> String {
    let mut md = String::new();
    let _ = writeln!(&mut md, "# Architecture FCoSE Browser Probe\n");
    let _ = writeln!(&mut md, "- Fixture: `{stem}`");
    let _ = writeln!(&mut md, "- Source: `{}`", source_path.display());
    let _ = writeln!(&mut md, "- JSON: `{}`\n", json_path.display());

    let _ = writeln!(&mut md, "## Config\n");
    let _ = writeln!(&mut md, "| key | value |\n|---|---|");
    if let Some(config) = probe.get("config").and_then(|v| v.as_object()) {
        for (key, value) in config {
            let _ = writeln!(
                &mut md,
                "| `{key}` | `{}` |",
                format_probe_config_value(value)
            );
        }
    }
    let _ = writeln!(&mut md);

    let _ = writeln!(&mut md, "## Layout BBox Stages\n");
    let _ = writeln!(&mut md, "| tag | bbox |\n|---|---|");
    if let Some(stages) = probe.get("stages").and_then(|v| v.as_array()) {
        for stage in stages {
            let Some(tag) = json_string(stage, "tag") else {
                continue;
            };
            let bbox = format_probe_rect(stage.get("bb"));
            let _ = writeln!(&mut md, "| `{tag}` | `{bbox}` |");
        }
    }
    let _ = writeln!(&mut md);

    let relocation_stages: Vec<&serde_json::Value> = probe
        .get("stages")
        .and_then(|v| v.as_array())
        .map(|stages| {
            stages
                .iter()
                .filter(|stage| json_string(stage, "tag") == Some("relocateComponent"))
                .collect()
        })
        .unwrap_or_default();
    if !relocation_stages.is_empty() {
        let _ = writeln!(&mut md, "## Relocation Stages\n");
        let _ = writeln!(
            &mut md,
            "| run | original center | rect bbox | rect center | delta |"
        );
        let _ = writeln!(&mut md, "|---:|---|---|---|---|");
        for stage in relocation_stages {
            let run = stage
                .get("runIndex")
                .and_then(|v| v.as_u64())
                .map(|v| v.to_string())
                .unwrap_or_else(|| "<none>".to_string());
            let original_center = format_probe_point(stage.get("originalCenter"));
            let rect_bbox = format_probe_rect(stage.get("rectBbox"));
            let rect_center = format_probe_point(stage.get("rectCenter"));
            let delta = format_probe_point(stage.get("delta"));
            let _ = writeln!(
                &mut md,
                "| {run} | `{original_center}` | `{rect_bbox}` | `{rect_center}` | `{delta}` |"
            );
        }
        let _ = writeln!(&mut md);
    }

    let _ = writeln!(&mut md, "## Final Node Bounds\n");
    let _ = writeln!(
        &mut md,
        "| id | type | classes | pos | bb | body | label | children labels | children body | children labels over body | bb over children labels | label text |"
    );
    let _ = writeln!(&mut md, "|---|---|---|---|---|---|---|---|---|---|---|---|");
    if let Some(nodes) = probe
        .pointer("/finalElements/nodes")
        .and_then(|v| v.as_array())
    {
        for node in nodes {
            let id = json_string(node, "id").unwrap_or("<missing>");
            let data = node.get("data").unwrap_or(&serde_json::Value::Null);
            let node_type = json_string(data, "type").unwrap_or("<missing>");
            let label = json_string(data, "label").unwrap_or("<none>");
            let classes = format_probe_classes(node.get("classes"));
            let pos = format_probe_point(node.get("pos"));
            let bb = format_probe_rect(node.get("bb"));
            let body = format_probe_rect(node.get("bodyBounds"));
            let label_bounds = format_probe_rect(node.pointer("/labelBounds/all"));
            let children_labels = format_probe_rect(node.get("childrenBoundingBoxIncludeLabels"));
            let children_body = format_probe_rect(node.get("childrenBoundingBoxBodyOnly"));
            let children_label_over_body = format_probe_rect_expansion(
                node.get("childrenBoundingBoxIncludeLabels"),
                node.get("childrenBoundingBoxBodyOnly"),
            );
            let bb_children_label_expansion = format_probe_rect_expansion(
                node.get("bb"),
                node.get("childrenBoundingBoxIncludeLabels"),
            );
            let _ = writeln!(
                &mut md,
                "| `{id}` | `{node_type}` | `{classes}` | `{pos}` | `{bb}` | `{body}` | `{label_bounds}` | `{children_labels}` | `{children_body}` | `{children_label_over_body}` | `{bb_children_label_expansion}` | `{label}` |"
            );
        }
    }
    let _ = writeln!(&mut md);

    let _ = writeln!(&mut md, "## Final Edge Bounds\n");
    let _ = writeln!(
        &mut md,
        "| id | endpoints | classes | dirs | bb | source endpoint | target endpoint | curve | weights | distances | edge distances |"
    );
    let _ = writeln!(&mut md, "|---|---|---|---|---|---|---|---|---|---|---|");
    if let Some(edges) = probe
        .pointer("/finalElements/edges")
        .and_then(|v| v.as_array())
    {
        for edge in edges {
            let id = json_string(edge, "id").unwrap_or("<missing>");
            let data = edge.get("data").unwrap_or(&serde_json::Value::Null);
            let style = edge.get("style").unwrap_or(&serde_json::Value::Null);
            let source = json_string(data, "source").unwrap_or("<missing>");
            let target = json_string(data, "target").unwrap_or("<missing>");
            let source_dir = json_string(data, "sourceDir").unwrap_or("<none>");
            let target_dir = json_string(data, "targetDir").unwrap_or("<none>");
            let classes = format_probe_classes(edge.get("classes"));
            let bb = format_probe_rect(edge.get("bb"));
            let source_endpoint = format_probe_point(edge.get("sourceEndpoint"));
            let target_endpoint = format_probe_point(edge.get("targetEndpoint"));
            let curve = json_string(style, "curveStyle").unwrap_or("<none>");
            let weights = json_string(style, "segmentWeights").unwrap_or("<none>");
            let distances = json_string(style, "segmentDistances").unwrap_or("<none>");
            let edge_distances = json_string(style, "edgeDistances").unwrap_or("<none>");
            let _ = writeln!(
                &mut md,
                "| `{id}` | `{source} -> {target}` | `{classes}` | `{source_dir} -> {target_dir}` | `{bb}` | `{source_endpoint}` | `{target_endpoint}` | `{curve}` | `{weights}` | `{distances}` | `{edge_distances}` |"
            );
        }
    }

    md
}

fn render_architecture_fcose_probe_batch_markdown(
    summaries: &[ArchitectureFcoseProbeRunSummary],
) -> String {
    let mut md = String::new();
    let _ = writeln!(&mut md, "# Architecture FCoSE Browser Probe Batch\n");
    let _ = writeln!(
        &mut md,
        "| fixture | json | summary | stages | nodes | edges |"
    );
    let _ = writeln!(&mut md, "|---|---|---|---:|---:|---:|");
    for summary in summaries {
        let _ = writeln!(
            &mut md,
            "| `{}` | `{}` | `{}` | {} | {} | {} |",
            summary.stem,
            summary.json_path.display(),
            summary.markdown_path.display(),
            summary.stage_count,
            summary.node_count,
            summary.edge_count,
        );
    }
    md
}

fn render_architecture_delta_batch_markdown(summaries: &[ArchitectureDeltaRunSummary]) -> String {
    fn format_optional_path(path: Option<&PathBuf>) -> String {
        path.map(|path| path.display().to_string())
            .unwrap_or_else(|| "<none>".to_string())
    }

    fn format_optional_delta(delta: Option<f64>) -> String {
        delta
            .map(|delta| format!("{delta:+.3}"))
            .unwrap_or_else(|| "<missing>".to_string())
    }

    fn format_optional_score(score: Option<f64>) -> String {
        score
            .map(|score| format!("{score:.3}"))
            .unwrap_or_else(|| "<missing>".to_string())
    }

    let mut summaries: Vec<&ArchitectureDeltaRunSummary> = summaries.iter().collect();
    summaries.sort_by(|a, b| {
        architecture_delta_summary_sort_order(
            &a.stem,
            a.root_residual_score,
            &b.stem,
            b.root_residual_score,
        )
    });

    let mut md = String::new();
    let _ = writeln!(&mut md, "# Architecture Delta Batch\n");
    let _ = writeln!(
        &mut md,
        "| fixture | report | upstream svg | local svg | probe json | viewBox width delta | viewBox height delta | max-width delta | root residual score | services | junctions | group rects | delta rows |"
    );
    let _ = writeln!(
        &mut md,
        "|---|---|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|"
    );
    for summary in summaries {
        let _ = writeln!(
            &mut md,
            "| `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | {} | {} | {} | {} |",
            summary.stem,
            summary.report_path.display(),
            summary.upstream_svg_path.display(),
            summary.local_svg_path.display(),
            format_optional_path(summary.probe_json_path.as_ref()),
            format_optional_delta(summary.viewbox_width_delta),
            format_optional_delta(summary.viewbox_height_delta),
            format_optional_delta(summary.max_width_delta),
            format_optional_score(summary.root_residual_score),
            summary.service_count,
            summary.junction_count,
            summary.group_rect_count,
            summary.delta_row_count,
        );
    }
    md
}

fn render_architecture_probe_join_markdown(
    report: &mut String,
    probe_json_path: &Path,
    probe: &serde_json::Value,
    layout: &merman_render::model::ArchitectureDiagramLayout,
    local_service_positions: &BTreeMap<String, DebugPt>,
    upstream_groups: &BTreeMap<String, DebugRect>,
    local_groups: &BTreeMap<String, DebugRect>,
    group_parents: &BTreeMap<String, Option<String>>,
) {
    let probe_nodes = architecture_probe_nodes_by_id(probe);
    let mut local_service_child_frame: BTreeMap<String, DebugRect> = BTreeMap::new();
    for service in &layout.cytoscape_service_bounds {
        let local_body = DebugRect::from_model_bounds(&service.body_bounds);
        let local_union = DebugRect::from_model_bounds(&service.union_bounds);
        local_service_child_frame.insert(
            service.id.clone(),
            local_union.translated(-local_body.w / 2.0, -local_body.h / 2.0),
        );
    }
    let mut browser_service_child_union: BTreeMap<String, DebugRect> = BTreeMap::new();
    for node in probe_nodes
        .values()
        .filter(|node| node.node_type == "service")
    {
        if let Some(union) = debug_rect_union(node.body.into_iter().chain(node.label)) {
            browser_service_child_union.insert(node.id.clone(), union);
        }
    }

    let mut group_ids: Vec<String> = probe_nodes
        .values()
        .filter(|node| node.node_type == "group")
        .map(|node| node.id.clone())
        .chain(
            layout
                .cytoscape_service_bounds
                .iter()
                .filter_map(|service| service.in_group.clone()),
        )
        .chain(group_parents.keys().cloned())
        .chain(group_parents.values().filter_map(|parent| parent.clone()))
        .collect();
    group_ids.sort();
    group_ids.dedup();

    let mut child_groups_by_parent: BTreeMap<String, Vec<(String, DebugRect)>> = BTreeMap::new();
    for (child_id, parent_id) in group_parents {
        let Some(parent_id) = parent_id else {
            continue;
        };
        let Some(rect) = local_groups.get(&group_local_rect_key(child_id)).copied() else {
            continue;
        };
        child_groups_by_parent
            .entry(parent_id.clone())
            .or_default()
            .push((child_id.clone(), rect));
    }

    let _ = writeln!(report, "## Browser probe phase join\n");
    let _ = writeln!(report, "Probe JSON: `{}`\n", probe_json_path.display());

    let _ = writeln!(report, "### Group content decomposition\n");
    let _ = writeln!(
        report,
        "This table compares browser `childrenBoundingBoxIncludeLabels`, local direct-service content, and final emitted group expansion. Local content is direct-service contribution only; nested group or junction content should be audited separately.\n"
    );
    let _ = writeln!(
        report,
        "| group | direct services | browser children labels | local service content | content dw | content dh | browser final expansion | local emitted expansion | expansion dw | expansion dh | emitted dw | emitted dh |\n|---|---:|---|---|---:|---:|---|---|---:|---:|---:|---:|"
    );

    if group_ids.is_empty() {
        let _ = writeln!(
            report,
            "| `<none>` | 0 | `<none>` | `<none>` | `<n/a>` | `<n/a>` | `<none>` | `<none>` | `<n/a>` | `<n/a>` | `<n/a>` | `<n/a>` |"
        );
    } else {
        for group_id in &group_ids {
            let direct_services: Vec<DebugRect> = layout
                .cytoscape_service_bounds
                .iter()
                .filter(|service| service.in_group.as_deref() == Some(group_id.as_str()))
                .map(|service| DebugRect::from_model_bounds(&service.union_bounds))
                .collect();
            let direct_service_count = direct_services.len();
            let local_content = debug_rect_union(direct_services);
            let browser_group = probe_nodes.get(group_id);
            let browser_children = browser_group.and_then(|node| node.children_labels);
            let browser_expansion = browser_group
                .and_then(|node| node.bb.zip(node.children_labels))
                .map(|(bb, children)| debug_rect_expansion(bb, children));
            let local_emitted = local_groups.get(&group_local_rect_key(group_id)).copied();
            let upstream_emitted = upstream_groups
                .get(&group_local_rect_key(group_id))
                .copied();
            let local_expansion = local_emitted
                .zip(local_content)
                .map(|(emitted, content)| debug_rect_expansion(emitted, content));

            let content_dw = local_content
                .zip(browser_children)
                .map(|(local, browser)| local.w - browser.w);
            let content_dh = local_content
                .zip(browser_children)
                .map(|(local, browser)| local.h - browser.h);
            let expansion_dw = local_expansion
                .zip(browser_expansion)
                .map(|(local, browser)| local.dw - browser.dw);
            let expansion_dh = local_expansion
                .zip(browser_expansion)
                .map(|(local, browser)| local.dh - browser.dh);
            let emitted_dw = local_emitted
                .zip(upstream_emitted)
                .map(|(local, upstream)| local.w - upstream.w);
            let emitted_dh = local_emitted
                .zip(upstream_emitted)
                .map(|(local, upstream)| local.h - upstream.h);

            let _ = writeln!(
                report,
                "| `{}` | {} | `{}` | `{}` | {} | {} | `{}` | `{}` | {} | {} | {} | {} |",
                group_id,
                direct_service_count,
                format_debug_rect(browser_children),
                format_debug_rect(local_content),
                format_debug_optional_f64(content_dw),
                format_debug_optional_f64(content_dh),
                format_debug_rect_expansion(browser_expansion),
                format_debug_rect_expansion(local_expansion),
                format_debug_optional_f64(expansion_dw),
                format_debug_optional_f64(expansion_dh),
                format_debug_optional_f64(emitted_dw),
                format_debug_optional_f64(emitted_dh)
            );
        }
    }
    let _ = writeln!(report);

    let _ = writeln!(report, "### Group aggregate child attribution\n");
    let _ = writeln!(
        report,
        "This table compares browser `childrenBoundingBoxIncludeLabels` with a local aggregate made from direct service contribution bounds plus direct child-group emitted rects. It is diagnostic evidence for nested groups; it does not replace the source-specific child service table above.\n"
    );
    let _ = writeln!(
        report,
        "| group | direct services | child groups | browser children labels | local aggregate content | content dw | content dh | browser final expansion | local emitted expansion | expansion dw | expansion dh | emitted dw | emitted dh |\n|---|---:|---|---|---|---:|---:|---|---|---:|---:|---:|---:|"
    );
    if group_ids.is_empty() {
        let _ = writeln!(
            report,
            "| `<none>` | 0 | `<none>` | `<none>` | `<none>` | `<n/a>` | `<n/a>` | `<none>` | `<none>` | `<n/a>` | `<n/a>` | `<n/a>` | `<n/a>` |"
        );
    } else {
        for group_id in &group_ids {
            let direct_services: Vec<DebugRect> = layout
                .cytoscape_service_bounds
                .iter()
                .filter(|service| service.in_group.as_deref() == Some(group_id.as_str()))
                .map(|service| DebugRect::from_model_bounds(&service.union_bounds))
                .collect();
            let direct_service_count = direct_services.len();
            let child_groups = child_groups_by_parent
                .get(group_id)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let child_group_names = if child_groups.is_empty() {
                "<none>".to_string()
            } else {
                child_groups
                    .iter()
                    .map(|(id, _)| id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            };

            let local_content = debug_rect_union(
                direct_services
                    .into_iter()
                    .chain(child_groups.iter().map(|(_, rect)| *rect)),
            );
            let browser_group = probe_nodes.get(group_id);
            let browser_children = browser_group.and_then(|node| node.children_labels);
            let browser_expansion = browser_group
                .and_then(|node| node.bb.zip(node.children_labels))
                .map(|(bb, children)| debug_rect_expansion(bb, children));
            let local_emitted = local_groups.get(&group_local_rect_key(group_id)).copied();
            let upstream_emitted = upstream_groups
                .get(&group_local_rect_key(group_id))
                .copied();
            let local_expansion = local_emitted
                .zip(local_content)
                .map(|(emitted, content)| debug_rect_expansion(emitted, content));

            let content_dw = local_content
                .zip(browser_children)
                .map(|(local, browser)| local.w - browser.w);
            let content_dh = local_content
                .zip(browser_children)
                .map(|(local, browser)| local.h - browser.h);
            let expansion_dw = local_expansion
                .zip(browser_expansion)
                .map(|(local, browser)| local.dw - browser.dw);
            let expansion_dh = local_expansion
                .zip(browser_expansion)
                .map(|(local, browser)| local.dh - browser.dh);
            let emitted_dw = local_emitted
                .zip(upstream_emitted)
                .map(|(local, upstream)| local.w - upstream.w);
            let emitted_dh = local_emitted
                .zip(upstream_emitted)
                .map(|(local, upstream)| local.h - upstream.h);

            let _ = writeln!(
                report,
                "| `{}` | {} | `{}` | `{}` | `{}` | {} | {} | `{}` | `{}` | {} | {} | {} | {} |",
                group_id,
                direct_service_count,
                child_group_names,
                format_debug_rect(browser_children),
                format_debug_rect(local_content),
                format_debug_optional_f64(content_dw),
                format_debug_optional_f64(content_dh),
                format_debug_rect_expansion(browser_expansion),
                format_debug_rect_expansion(local_expansion),
                format_debug_optional_f64(expansion_dw),
                format_debug_optional_f64(expansion_dh),
                format_debug_optional_f64(emitted_dw),
                format_debug_optional_f64(emitted_dh),
            );
        }
    }
    let _ = writeln!(report);

    let _ = writeln!(report, "### Group content edge attribution\n");
    let _ = writeln!(
        report,
        "This table attributes direct-service group content deltas to the browser child-union phase (`bodyBounds` union `labelBounds.all`) and the local service contribution shifted into the same final-frame coordinates. It only covers direct service children; nested groups and junctions still need their own phase audit.\n"
    );
    let _ = writeln!(
        report,
        "| group | direct services | browser left | local left | left dx | browser right | local right | right dx | edge dw | browser top | local top | top dy | browser bottom | local bottom | bottom dy | edge dh |\n|---|---:|---|---|---:|---|---|---:|---:|---|---|---:|---|---|---:|---:|"
    );
    if group_ids.is_empty() {
        let _ = writeln!(
            report,
            "| `<none>` | 0 | `<none>` | `<none>` | `<n/a>` | `<none>` | `<none>` | `<n/a>` | `<n/a>` | `<none>` | `<none>` | `<n/a>` | `<none>` | `<none>` | `<n/a>` | `<n/a>` |"
        );
    } else {
        for group_id in &group_ids {
            let direct_service_ids: Vec<&str> = layout
                .cytoscape_service_bounds
                .iter()
                .filter(|service| service.in_group.as_deref() == Some(group_id.as_str()))
                .map(|service| service.id.as_str())
                .collect();

            let browser_owner = |edge| {
                debug_edge_owner(
                    direct_service_ids.iter().filter_map(|id| {
                        browser_service_child_union
                            .get(*id)
                            .copied()
                            .map(|rect| (*id, rect))
                    }),
                    edge,
                )
            };
            let local_owner = |edge| {
                debug_edge_owner(
                    direct_service_ids.iter().filter_map(|id| {
                        local_service_child_frame
                            .get(*id)
                            .copied()
                            .map(|rect| (*id, rect))
                    }),
                    edge,
                )
            };

            let browser_left = browser_owner(DebugEdge::Left);
            let local_left = local_owner(DebugEdge::Left);
            let browser_right = browser_owner(DebugEdge::Right);
            let local_right = local_owner(DebugEdge::Right);
            let browser_top = browser_owner(DebugEdge::Top);
            let local_top = local_owner(DebugEdge::Top);
            let browser_bottom = browser_owner(DebugEdge::Bottom);
            let local_bottom = local_owner(DebugEdge::Bottom);
            let left_dx = local_left
                .zip(browser_left)
                .map(|(local, browser)| local.1 - browser.1);
            let right_dx = local_right
                .zip(browser_right)
                .map(|(local, browser)| local.1 - browser.1);
            let top_dy = local_top
                .zip(browser_top)
                .map(|(local, browser)| local.1 - browser.1);
            let bottom_dy = local_bottom
                .zip(browser_bottom)
                .map(|(local, browser)| local.1 - browser.1);
            let edge_dw = right_dx.zip(left_dx).map(|(right, left)| right - left);
            let edge_dh = bottom_dy.zip(top_dy).map(|(bottom, top)| bottom - top);

            let _ = writeln!(
                report,
                "| `{}` | {} | `{}` | `{}` | {} | `{}` | `{}` | {} | {} | `{}` | `{}` | {} | `{}` | `{}` | {} | {} |",
                group_id,
                direct_service_ids.len(),
                format_debug_edge_owner(browser_left),
                format_debug_edge_owner(local_left),
                format_debug_optional_f64(left_dx),
                format_debug_edge_owner(browser_right),
                format_debug_edge_owner(local_right),
                format_debug_optional_f64(right_dx),
                format_debug_optional_f64(edge_dw),
                format_debug_edge_owner(browser_top),
                format_debug_edge_owner(local_top),
                format_debug_optional_f64(top_dy),
                format_debug_edge_owner(browser_bottom),
                format_debug_edge_owner(local_bottom),
                format_debug_optional_f64(bottom_dy),
                format_debug_optional_f64(edge_dh),
            );
        }
    }
    let _ = writeln!(report);

    let _ = writeln!(report, "### Service bbox join\n");
    let _ = writeln!(
        report,
        "This table joins local service contribution phases with browser final service nodes. `label metric dw` compares local measured label text width with browser `labelWidth`. `local contribution label final-frame` shifts the local contribution-label rectangle by half the local body size so its x/y coordinates are comparable to browser `labelBounds.all`; it is an extended contribution rectangle, not browser text-label bounds. `browser child union` is `bodyBounds` union `labelBounds.all`, which matches the service contribution phase feeding browser `childrenBoundingBoxIncludeLabels`. `local union final-frame` applies the same frame shift to local child contribution. `local final bb final-frame` applies the source-shaped 1px final `node.boundingBox()` expansion to that local child union; it is diagnostic only and does not change renderer output.\n"
    );
    let _ = writeln!(
        report,
        "| id | group | browser pos | local svg pos | pos dx | pos dy | browser body | local body | body dw | body dh | browser label metrics | local label metrics | label metric dw | browser label | local contribution label | local contribution label final-frame | label dx | label dy | label dw | label dh | browser child union | local union final-frame | child dx | child dy | child dw | child dh | browser bb | local final bb final-frame | final dx | final dy | final dw | final dh | local union | union dw | union dh | bb frame dx | bb frame dy |\n|---|---|---|---|---:|---:|---|---|---:|---:|---|---|---:|---|---|---|---:|---:|---:|---:|---|---|---:|---:|---:|---:|---|---|---:|---:|---:|---:|---|---:|---:|---:|---:|"
    );
    if layout.cytoscape_service_bounds.is_empty() {
        let _ = writeln!(
            report,
            "| `<none>` | `<none>` | `<none>` | `<none>` | `<n/a>` | `<n/a>` | `<none>` | `<none>` | `<n/a>` | `<n/a>` | `<none>` | `<none>` | `<n/a>` | `<none>` | `<none>` | `<none>` | `<n/a>` | `<n/a>` | `<n/a>` | `<n/a>` | `<none>` | `<none>` | `<n/a>` | `<n/a>` | `<n/a>` | `<n/a>` | `<none>` | `<none>` | `<n/a>` | `<n/a>` | `<n/a>` | `<n/a>` | `<none>` | `<n/a>` | `<n/a>` | `<n/a>` | `<n/a>` |"
        );
    } else {
        for service in &layout.cytoscape_service_bounds {
            let browser = probe_nodes
                .get(&service.id)
                .filter(|node| node.node_type == "service");
            let browser_pos = browser.and_then(|node| node.pos);
            let local_pos = local_service_positions
                .get(&service_local_pos_key(&service.id))
                .copied();
            let pos_dx = local_pos
                .zip(browser_pos)
                .map(|(local, browser)| local.x - browser.x);
            let pos_dy = local_pos
                .zip(browser_pos)
                .map(|(local, browser)| local.y - browser.y);

            let browser_body = browser.and_then(|node| node.body);
            let local_body = DebugRect::from_model_bounds(&service.body_bounds);
            let body_dw = browser_body.map(|body| local_body.w - body.w);
            let body_dh = browser_body.map(|body| local_body.h - body.h);

            let browser_label = browser.and_then(|node| node.label);
            let local_label = service
                .label_bounds
                .as_ref()
                .map(DebugRect::from_model_bounds);
            let local_label_final_frame =
                local_label.map(|label| label.translated(-local_body.w / 2.0, -local_body.h / 2.0));
            let label_metric_dw = service
                .label_metrics
                .as_ref()
                .zip(browser.and_then(|node| node.label_width))
                .map(|(local, browser_width)| local.text_width - browser_width);
            let label_dx = local_label_final_frame
                .zip(browser_label)
                .map(|(local, browser)| local.x - browser.x);
            let label_dy = local_label_final_frame
                .zip(browser_label)
                .map(|(local, browser)| local.y - browser.y);
            let label_dw = local_label_final_frame
                .zip(browser_label)
                .map(|(local, browser)| local.w - browser.w);
            let label_dh = local_label_final_frame
                .zip(browser_label)
                .map(|(local, browser)| local.h - browser.h);

            let browser_bb = browser.and_then(|node| node.bb);
            let local_union = DebugRect::from_model_bounds(&service.union_bounds);
            let union_dw = browser_bb.map(|bb| local_union.w - bb.w);
            let union_dh = browser_bb.map(|bb| local_union.h - bb.h);
            let local_union_final_frame =
                local_union.translated(-local_body.w / 2.0, -local_body.h / 2.0);
            let browser_child_union =
                debug_rect_union(browser_body.into_iter().chain(browser_label));
            let child_dx = browser_child_union.map(|bb| local_union_final_frame.x - bb.x);
            let child_dy = browser_child_union.map(|bb| local_union_final_frame.y - bb.y);
            let child_dw = browser_child_union.map(|bb| local_union_final_frame.w - bb.w);
            let child_dh = browser_child_union.map(|bb| local_union_final_frame.h - bb.h);
            let bb_frame_dx = browser_bb.map(|bb| local_union_final_frame.x - bb.x);
            let bb_frame_dy = browser_bb.map(|bb| local_union_final_frame.y - bb.y);
            let local_final_bb_final_frame = local_union_final_frame.expanded(1.0);
            let final_dx = browser_bb.map(|bb| local_final_bb_final_frame.x - bb.x);
            let final_dy = browser_bb.map(|bb| local_final_bb_final_frame.y - bb.y);
            let final_dw = browser_bb.map(|bb| local_final_bb_final_frame.w - bb.w);
            let final_dh = browser_bb.map(|bb| local_final_bb_final_frame.h - bb.h);

            let _ = writeln!(
                report,
                "| `{}` | `{}` | `{}` | `{}` | {} | {} | `{}` | `{}` | {} | {} | `{}` | `{}` | {} | `{}` | `{}` | `{}` | {} | {} | {} | {} | `{}` | `{}` | {} | {} | {} | {} | `{}` | `{}` | {} | {} | {} | {} | `{}` | {} | {} | {} | {} |",
                service.id,
                service.in_group.as_deref().unwrap_or("<none>"),
                format_debug_point(browser_pos),
                format_debug_point(local_pos),
                format_debug_optional_f64(pos_dx),
                format_debug_optional_f64(pos_dy),
                format_debug_rect(browser_body),
                format_debug_rect(Some(local_body)),
                format_debug_optional_f64(body_dw),
                format_debug_optional_f64(body_dh),
                format_browser_label_metrics(browser),
                format_local_label_metrics(service.label_metrics.as_ref()),
                format_debug_optional_f64(label_metric_dw),
                format_debug_rect(browser_label),
                format_debug_rect(local_label),
                format_debug_rect(local_label_final_frame),
                format_debug_optional_f64(label_dx),
                format_debug_optional_f64(label_dy),
                format_debug_optional_f64(label_dw),
                format_debug_optional_f64(label_dh),
                format_debug_rect(browser_child_union),
                format_debug_rect(Some(local_union_final_frame)),
                format_debug_optional_f64(child_dx),
                format_debug_optional_f64(child_dy),
                format_debug_optional_f64(child_dw),
                format_debug_optional_f64(child_dh),
                format_debug_rect(browser_bb),
                format_debug_rect(Some(local_final_bb_final_frame)),
                format_debug_optional_f64(final_dx),
                format_debug_optional_f64(final_dy),
                format_debug_optional_f64(final_dw),
                format_debug_optional_f64(final_dh),
                format_debug_rect(Some(local_union)),
                format_debug_optional_f64(union_dw),
                format_debug_optional_f64(union_dh),
                format_debug_optional_f64(bb_frame_dx),
                format_debug_optional_f64(bb_frame_dy)
            );
        }
    }
    let _ = writeln!(report);
}

pub(crate) fn debug_architecture_fcose_probe(args: Vec<String>) -> Result<(), XtaskError> {
    let cli = parse_architecture_fcose_probe_args(&args)?;
    let fixtures: Vec<(PathBuf, String)> = cli
        .fixture_filters
        .iter()
        .map(|filter| resolve_architecture_probe_fixture(filter))
        .collect::<Result<_, _>>()?;
    let workspace_root = crate::cmd::workspace_root();
    let script_path = workspace_root
        .join("tools")
        .join("debug")
        .join("arch_fcose_browser_probe_fixture_025.js");
    if !script_path.is_file() {
        return Err(XtaskError::DebugSvgFailed(format!(
            "missing Architecture FCoSE probe script: {}",
            script_path.display()
        )));
    }

    fs::create_dir_all(&cli.out_dir).map_err(|source| XtaskError::WriteFile {
        path: cli.out_dir.display().to_string(),
        source,
    })?;

    let mut summaries = Vec::new();

    for (idx, (mmd_path, stem)) in fixtures.iter().enumerate() {
        let mut command = Command::new("node");
        command
            .arg(&script_path)
            .arg(stem)
            .current_dir(&workspace_root);
        if let Some(browser_exe) = &cli.browser_exe {
            command.env("PUPPETEER_EXECUTABLE_PATH", browser_exe);
        }
        let output = command
            .output()
            .map_err(|e| XtaskError::DebugSvgFailed(format!("failed to spawn node: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(XtaskError::DebugSvgFailed(format!(
                "Architecture FCoSE browser probe failed for {stem} (exit={}):\n{}",
                output.status.code().unwrap_or(-1),
                stderr.trim()
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let probe: serde_json::Value = serde_json::from_str(&stdout)?;
        let out_json = architecture_fcose_probe_json_path(&cli.out_dir, stem);
        fs::write(&out_json, serde_json::to_string_pretty(&probe)?).map_err(|source| {
            XtaskError::WriteFile {
                path: out_json.display().to_string(),
                source,
            }
        })?;
        let out_markdown = architecture_fcose_probe_markdown_path(&cli.out_dir, stem);
        fs::write(
            &out_markdown,
            render_architecture_fcose_probe_markdown(stem, mmd_path, &out_json, &probe),
        )
        .map_err(|source| XtaskError::WriteFile {
            path: out_markdown.display().to_string(),
            source,
        })?;

        let stage_count = probe
            .get("stages")
            .and_then(|v| v.as_array())
            .map_or(0, Vec::len);
        let node_count = probe
            .pointer("/finalElements/nodes")
            .and_then(|v| v.as_array())
            .map_or(0, Vec::len);
        let edge_count = probe
            .pointer("/finalElements/edges")
            .and_then(|v| v.as_array())
            .map_or(0, Vec::len);
        summaries.push(ArchitectureFcoseProbeRunSummary {
            stem: stem.clone(),
            json_path: out_json.clone(),
            markdown_path: out_markdown.clone(),
            stage_count,
            node_count,
            edge_count,
        });

        if idx > 0 {
            println!();
        }
        println!("fixture: {stem}");
        println!("source:  {}", mmd_path.display());
        println!("script:  {}", script_path.display());
        if let Some(browser_exe) = &cli.browser_exe {
            println!("browser: {}", browser_exe.display());
        }
        println!("json:    {}", out_json.display());
        println!("summary: {}", out_markdown.display());
        println!("stages:  {stage_count}");
        println!("final elements: nodes={node_count} edges={edge_count}");

        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.trim().is_empty() {
            println!("probe stderr: {}", stderr.trim());
        }
    }
    if summaries.len() > 1 {
        let out_batch = architecture_fcose_probe_batch_markdown_path(&cli.out_dir);
        fs::write(
            &out_batch,
            render_architecture_fcose_probe_batch_markdown(&summaries),
        )
        .map_err(|source| XtaskError::WriteFile {
            path: out_batch.display().to_string(),
            source,
        })?;
        println!();
        println!("batch:   {}", out_batch.display());
    }

    Ok(())
}

pub(crate) fn debug_architecture_delta(args: Vec<String>) -> Result<(), XtaskError> {
    let ArchitectureDeltaCli {
        fixture_filters,
        out_dir,
        probe_dir,
    } = parse_architecture_delta_args(&args)?;

    fn parse_viewbox(v: &str) -> Option<(f64, f64, f64, f64)> {
        let nums: Vec<f64> = v
            .split_whitespace()
            .filter_map(|s| s.trim().parse::<f64>().ok())
            .collect();
        if nums.len() != 4 {
            return None;
        }
        Some((nums[0], nums[1], nums[2], nums[3]))
    }

    fn parse_translate(transform: &str) -> Option<(f64, f64)> {
        // Mermaid emits `translate(x,y)` or `translate(x, y)` in Architecture outputs.
        let s = transform.trim();
        let s = s.strip_prefix("translate(")?;
        let s = s.strip_suffix(')')?;
        let parts: Vec<&str> = s
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|t: &&str| !t.trim().is_empty())
            .collect();
        let x = parts.first()?.trim().parse::<f64>().ok()?;
        let y = parts
            .get(1)
            .copied()
            .and_then(|v| v.trim().parse::<f64>().ok())?;
        Some((x, y))
    }

    fn parse_max_width_px(style: &str) -> Option<f64> {
        static RE: OnceLock<Regex> = OnceLock::new();
        let re = RE.get_or_init(|| Regex::new(r#"max-width:\s*([0-9.]+)px"#).unwrap());
        let cap = re.captures(style)?;
        cap.get(1)?.as_str().trim().parse::<f64>().ok()
    }

    fn has_class_token(class: &str, token: &str) -> bool {
        class.split_whitespace().any(|t| t == token)
    }

    fn sanitize_svg_id(stem: &str) -> String {
        stem.chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                    ch
                } else {
                    '_'
                }
            })
            .collect()
    }

    type ArchPositions = (
        Option<(f64, f64, f64, f64)>,
        Option<f64>,
        BTreeMap<String, DebugPt>,
        BTreeMap<String, DebugPt>,
        BTreeMap<String, DebugRect>,
    );

    fn extract_arch_positions(svg: &str) -> Result<ArchPositions, XtaskError> {
        let doc = roxmltree::Document::parse(svg)
            .map_err(|e| XtaskError::SvgCompareFailed(format!("failed to parse svg xml: {e}")))?;
        let root = doc.root_element();
        let viewbox = root.attribute("viewBox").and_then(parse_viewbox);
        let max_width = root.attribute("style").and_then(parse_max_width_px);

        let mut services: BTreeMap<String, DebugPt> = BTreeMap::new();
        let mut junctions: BTreeMap<String, DebugPt> = BTreeMap::new();
        let mut groups: BTreeMap<String, DebugRect> = BTreeMap::new();

        for n in doc.descendants().filter(|n| n.is_element()) {
            let tag = n.tag_name().name();
            let id = n.attribute("id");

            if tag == "g"
                && n.attribute("class")
                    .is_some_and(|c| has_class_token(c, "architecture-service"))
            {
                if let (Some(id), Some((x, y))) = (
                    id.and_then(|id| normalize_arch_svg_id_with_marker(id, "service-")),
                    n.attribute("transform").and_then(parse_translate),
                ) {
                    services.insert(id, DebugPt { x, y });
                }
            }

            if tag == "g"
                && n.attribute("class")
                    .is_some_and(|c| has_class_token(c, "architecture-junction"))
            {
                let junction_id = id.and_then(normalize_arch_junction_svg_id).or_else(|| {
                    n.descendants()
                        .filter(|child| child.is_element())
                        .find_map(|child| {
                            child
                                .attribute("id")
                                .and_then(normalize_arch_junction_svg_id)
                        })
                });
                if let (Some(id), Some((x, y))) = (
                    junction_id,
                    n.attribute("transform").and_then(parse_translate),
                ) {
                    junctions.insert(id, DebugPt { x, y });
                }
            }

            if tag == "rect" {
                let x = n
                    .attribute("x")
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(0.0);
                let y = n
                    .attribute("y")
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(0.0);
                let w = n
                    .attribute("width")
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(0.0);
                let h = n
                    .attribute("height")
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(0.0);
                if let Some(id) = id.and_then(|id| normalize_arch_svg_id_with_marker(id, "group-"))
                {
                    groups.insert(id, DebugRect { x, y, w, h });
                }
            }
        }

        Ok((viewbox, max_width, services, junctions, groups))
    }

    let fixtures_dir = crate::cmd::fixtures_root().join("architecture");
    let upstream_dir = crate::cmd::fixtures_root()
        .join("upstream-svgs")
        .join("architecture");
    let out_dir = out_dir.unwrap_or_else(|| {
        crate::cmd::target_root()
            .join("debug")
            .join("architecture-delta")
    });

    let mut summaries: Vec<ArchitectureDeltaRunSummary> = Vec::new();

    for (idx, fixture) in fixture_filters.iter().enumerate() {
        let candidates = crate::cmd::list_mmd_fixtures_in_dir(&fixtures_dir, Some(fixture), true);

        let mmd_path = match candidates.len() {
            0 => {
                return Err(XtaskError::SvgCompareFailed(format!(
                    "no Architecture fixture matched {fixture:?} under {}",
                    fixtures_dir.display()
                )));
            }
            1 => candidates[0].clone(),
            _ => {
                let list = candidates
                    .iter()
                    .take(20)
                    .map(|p| format!("- {}", p.display()))
                    .collect::<Vec<_>>()
                    .join("\n");
                return Err(XtaskError::SvgCompareFailed(format!(
                    "multiple Architecture fixtures matched {fixture:?}; please be more specific:\n{list}"
                )));
            }
        };

        let stem = mmd_path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| {
                XtaskError::SvgCompareFailed(format!(
                    "invalid fixture filename {}",
                    mmd_path.display()
                ))
            })?
            .to_string();

        let diagram_id = sanitize_svg_id(&stem);

        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        let upstream_svg =
            fs::read_to_string(&upstream_path).map_err(|source| XtaskError::ReadFile {
                path: upstream_path.display().to_string(),
                source,
            })?;

        let text = fs::read_to_string(&mmd_path).map_err(|source| XtaskError::ReadFile {
            path: mmd_path.display().to_string(),
            source,
        })?;

        let engine = merman::Engine::new();
        let parsed = futures::executor::block_on(
            engine.parse_diagram(&text, merman::ParseOptions::default()),
        )
        .map_err(|e| {
            XtaskError::SvgCompareFailed(format!("parse failed for {}: {e}", mmd_path.display()))
        })?
        .ok_or_else(|| {
            XtaskError::SvgCompareFailed(format!("no diagram detected in {}", mmd_path.display()))
        })?;

        let layout_opts = svg_compare_layout_opts();
        let layouted = merman_render::layout_parsed(&parsed, &layout_opts).map_err(|e| {
            XtaskError::SvgCompareFailed(format!("layout failed for {}: {e}", mmd_path.display()))
        })?;

        let merman_render::model::LayoutDiagram::ArchitectureDiagram(layout) = &layouted.layout
        else {
            return Err(XtaskError::SvgCompareFailed(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            )));
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(diagram_id),
            ..Default::default()
        };
        let local_svg = merman_render::svg::render_architecture_diagram_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            &svg_opts,
        )
        .map_err(|e| {
            XtaskError::SvgCompareFailed(format!("render failed for {}: {e}", mmd_path.display()))
        })?;

        fs::create_dir_all(&out_dir).map_err(|source| XtaskError::WriteFile {
            path: out_dir.display().to_string(),
            source,
        })?;

        let out_upstream_svg = out_dir.join(format!("{stem}.upstream.svg"));
        let out_local_svg = out_dir.join(format!("{stem}.local.svg"));
        let out_report = out_dir.join(format!("{stem}.md"));
        fs::write(&out_upstream_svg, &upstream_svg).map_err(|source| XtaskError::WriteFile {
            path: out_upstream_svg.display().to_string(),
            source,
        })?;
        fs::write(&out_local_svg, &local_svg).map_err(|source| XtaskError::WriteFile {
            path: out_local_svg.display().to_string(),
            source,
        })?;

        let (up_vb, up_mw, up_services, up_junctions, up_groups) =
            extract_arch_positions(&upstream_svg)?;
        let (lo_vb, lo_mw, lo_services, lo_junctions, lo_groups) =
            extract_arch_positions(&local_svg)?;
        let probe_json: Option<(PathBuf, serde_json::Value)> = if let Some(probe_dir) = &probe_dir {
            let probe_path = architecture_fcose_probe_json_path(&probe_dir, &stem);
            let probe_text =
                fs::read_to_string(&probe_path).map_err(|source| XtaskError::ReadFile {
                    path: probe_path.display().to_string(),
                    source,
                })?;
            let probe = serde_json::from_str(&probe_text)?;
            Some((probe_path, probe))
        } else {
            None
        };

        #[derive(Debug, Clone)]
        struct DeltaRow {
            id: String,
            kind: &'static str,
            up: String,
            lo: String,
            dx: f64,
            dy: f64,
            dw: Option<f64>,
            dh: Option<f64>,
            score: f64,
        }

        let mut deltas: Vec<DeltaRow> = Vec::new();

        fn delta_score(dx: f64, dy: f64, dw: Option<f64>, dh: Option<f64>) -> f64 {
            dx.abs()
                .max(dy.abs())
                .max(dw.unwrap_or(0.0).abs())
                .max(dh.unwrap_or(0.0).abs())
        }

        fn fmt_optional_delta(delta: Option<f64>) -> String {
            delta
                .map(|v| format!("{v:.6}"))
                .unwrap_or_else(|| "<n/a>".to_string())
        }

        fn fmt_model_bounds(bounds: Option<&merman_render::model::Bounds>) -> String {
            bounds
                .map(|b| {
                    format!(
                        "x={:.6} y={:.6} w={:.6} h={:.6}",
                        b.min_x,
                        b.min_y,
                        b.max_x - b.min_x,
                        b.max_y - b.min_y
                    )
                })
                .unwrap_or_else(|| "<none>".to_string())
        }

        fn split_missing<T>(
            upstream: &BTreeMap<String, T>,
            local: &BTreeMap<String, T>,
        ) -> (Vec<String>, Vec<String>) {
            let mut only_up: Vec<String> = upstream
                .keys()
                .filter(|id| !local.contains_key(*id))
                .cloned()
                .collect();
            let mut only_lo: Vec<String> = local
                .keys()
                .filter(|id| !upstream.contains_key(*id))
                .cloned()
                .collect();
            only_up.sort();
            only_lo.sort();
            (only_up, only_lo)
        }

        let (missing_services_in_local, missing_services_in_upstream) =
            split_missing(&up_services, &lo_services);
        let (missing_junctions_in_local, missing_junctions_in_upstream) =
            split_missing(&up_junctions, &lo_junctions);
        let (missing_groups_in_local, missing_groups_in_upstream) =
            split_missing(&up_groups, &lo_groups);

        for (id, up) in &up_services {
            let Some(lo) = lo_services.get(id).copied() else {
                continue;
            };
            let dx = lo.x - up.x;
            let dy = lo.y - up.y;
            deltas.push(DeltaRow {
                id: id.to_string(),
                kind: "service",
                up: format!("translate({:.6},{:.6})", up.x, up.y),
                lo: format!("translate({:.6},{:.6})", lo.x, lo.y),
                dx,
                dy,
                dw: None,
                dh: None,
                score: delta_score(dx, dy, None, None),
            });
        }

        for (id, up) in &up_junctions {
            let Some(lo) = lo_junctions.get(id).copied() else {
                continue;
            };
            let dx = lo.x - up.x;
            let dy = lo.y - up.y;
            deltas.push(DeltaRow {
                id: id.to_string(),
                kind: "junction",
                up: format!("translate({:.6},{:.6})", up.x, up.y),
                lo: format!("translate({:.6},{:.6})", lo.x, lo.y),
                dx,
                dy,
                dw: None,
                dh: None,
                score: delta_score(dx, dy, None, None),
            });
        }

        for (id, up) in &up_groups {
            let Some(lo) = lo_groups.get(id).copied() else {
                continue;
            };
            let dx = lo.x - up.x;
            let dy = lo.y - up.y;
            let dw = lo.w - up.w;
            let dh = lo.h - up.h;
            deltas.push(DeltaRow {
                id: id.to_string(),
                kind: "group-rect",
                up: format!("x={:.6} y={:.6} w={:.6} h={:.6}", up.x, up.y, up.w, up.h),
                lo: format!("x={:.6} y={:.6} w={:.6} h={:.6}", lo.x, lo.y, lo.w, lo.h),
                dx,
                dy,
                dw: Some(dw),
                dh: Some(dh),
                score: delta_score(dx, dy, Some(dw), Some(dh)),
            });
        }

        let fcose_compound_rows: Vec<(String, DebugRect, Option<DebugRect>)> = layout
            .fcose_compound_bounds
            .iter()
            .map(|compound| {
                let b = &compound.bounds;
                let fcose = DebugRect {
                    x: b.min_x,
                    y: b.min_y,
                    w: (b.max_x - b.min_x).max(0.0),
                    h: (b.max_y - b.min_y).max(0.0),
                };
                let local_key = format!("group-{}", compound.id);
                (
                    compound.id.clone(),
                    fcose,
                    lo_groups.get(&local_key).copied(),
                )
            })
            .collect();

        deltas.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let viewbox_width_delta = up_vb.zip(lo_vb).map(|(up, lo)| lo.2 - up.2);
        let viewbox_height_delta = up_vb.zip(lo_vb).map(|(up, lo)| lo.3 - up.3);
        let max_width_delta = up_mw.zip(lo_mw).map(|(up, lo)| lo - up);
        let root_residual_score = architecture_root_residual_score(
            max_width_delta,
            viewbox_width_delta,
            viewbox_height_delta,
        );

        let mut report = String::new();
        let _ = writeln!(&mut report, "# Architecture Delta Report\n");
        let _ = writeln!(
            &mut report,
            "- Fixture: `{}`\n- Upstream SVG: `{}`\n- Local SVG: `{}`\n",
            stem,
            out_upstream_svg.display(),
            out_local_svg.display()
        );

        let _ = writeln!(&mut report, "## Root viewport\n");
        let _ = writeln!(
            &mut report,
            "- upstream viewBox: `{}`",
            up_vb
                .map(|v| format!("{:.6} {:.6} {:.6} {:.6}", v.0, v.1, v.2, v.3))
                .unwrap_or_else(|| "<missing>".to_string())
        );
        let _ = writeln!(
            &mut report,
            "- local viewBox: `{}`",
            lo_vb
                .map(|v| format!("{:.6} {:.6} {:.6} {:.6}", v.0, v.1, v.2, v.3))
                .unwrap_or_else(|| "<missing>".to_string())
        );
        let _ = writeln!(
            &mut report,
            "- upstream max-width(px): `{}`",
            up_mw
                .map(|v| format!("{:.6}", v))
                .unwrap_or_else(|| "<missing>".to_string())
        );
        let _ = writeln!(
            &mut report,
            "- local max-width(px): `{}`",
            lo_mw
                .map(|v| format!("{:.6}", v))
                .unwrap_or_else(|| "<missing>".to_string())
        );
        let _ = writeln!(
            &mut report,
            "- viewBox width delta: `{}`",
            viewbox_width_delta
                .map(|v| format!("{v:+.6}"))
                .unwrap_or_else(|| "<missing>".to_string())
        );
        let _ = writeln!(
            &mut report,
            "- viewBox height delta: `{}`",
            viewbox_height_delta
                .map(|v| format!("{v:+.6}"))
                .unwrap_or_else(|| "<missing>".to_string())
        );
        let _ = writeln!(
            &mut report,
            "- max-width delta: `{}`",
            max_width_delta
                .map(|v| format!("{v:+.6}"))
                .unwrap_or_else(|| "<missing>".to_string())
        );
        let _ = writeln!(
            &mut report,
            "- root residual score: `{}`\n",
            root_residual_score
                .map(|v| format!("{v:.6}"))
                .unwrap_or_else(|| "<missing>".to_string())
        );

        let _ = writeln!(&mut report, "## Missing elements\n");
        let _ = writeln!(
            &mut report,
            "- services missing in local: `{}`",
            if missing_services_in_local.is_empty() {
                "<none>".to_string()
            } else {
                missing_services_in_local.join(", ")
            }
        );
        let _ = writeln!(
            &mut report,
            "- services missing in upstream: `{}`",
            if missing_services_in_upstream.is_empty() {
                "<none>".to_string()
            } else {
                missing_services_in_upstream.join(", ")
            }
        );
        let _ = writeln!(
            &mut report,
            "- junctions missing in local: `{}`",
            if missing_junctions_in_local.is_empty() {
                "<none>".to_string()
            } else {
                missing_junctions_in_local.join(", ")
            }
        );
        let _ = writeln!(
            &mut report,
            "- junctions missing in upstream: `{}`",
            if missing_junctions_in_upstream.is_empty() {
                "<none>".to_string()
            } else {
                missing_junctions_in_upstream.join(", ")
            }
        );
        let _ = writeln!(
            &mut report,
            "- group rects missing in local: `{}`",
            if missing_groups_in_local.is_empty() {
                "<none>".to_string()
            } else {
                missing_groups_in_local.join(", ")
            }
        );
        let _ = writeln!(
            &mut report,
            "- group rects missing in upstream: `{}`\n",
            if missing_groups_in_upstream.is_empty() {
                "<none>".to_string()
            } else {
                missing_groups_in_upstream.join(", ")
            }
        );

        let _ = writeln!(
            &mut report,
            "## Local FCoSE compound bounds vs emitted group rects\n"
        );
        let _ = writeln!(
            &mut report,
            "These FCoSE bounds are local layout-base compound rectangles, not browser `node.boundingBox()` values.\n"
        );
        let _ = writeln!(
            &mut report,
            "| id | fcose compound bounds | local emitted group rect | dx | dy | dw | dh |\n|---|---|---|---:|---:|---:|---:|"
        );
        if fcose_compound_rows.is_empty() {
            let _ = writeln!(
                &mut report,
                "| `<none>` | `<none>` | `<none>` | `<n/a>` | `<n/a>` | `<n/a>` | `<n/a>` |"
            );
        } else {
            for (id, fcose, emitted) in &fcose_compound_rows {
                if let Some(emitted) = emitted {
                    let _ = writeln!(
                        &mut report,
                        "| `{}` | `x={:.6} y={:.6} w={:.6} h={:.6}` | `x={:.6} y={:.6} w={:.6} h={:.6}` | {:.6} | {:.6} | {:.6} | {:.6} |",
                        id,
                        fcose.x,
                        fcose.y,
                        fcose.w,
                        fcose.h,
                        emitted.x,
                        emitted.y,
                        emitted.w,
                        emitted.h,
                        emitted.x - fcose.x,
                        emitted.y - fcose.y,
                        emitted.w - fcose.w,
                        emitted.h - fcose.h,
                    );
                } else {
                    let _ = writeln!(
                        &mut report,
                        "| `{}` | `x={:.6} y={:.6} w={:.6} h={:.6}` | `<missing>` | `<n/a>` | `<n/a>` | `<n/a>` | `<n/a>` |",
                        id, fcose.x, fcose.y, fcose.w, fcose.h
                    );
                }
            }
        }
        let _ = writeln!(&mut report);

        let _ = writeln!(&mut report, "## Local Cytoscape service child bounds\n");
        let _ = writeln!(
            &mut report,
            "These rows are the local body/label/union phases that feed Architecture group content bounds.\n"
        );
        let _ = writeln!(
            &mut report,
            "| id | group | body bounds | label bounds | label metrics | union bounds |\n|---|---|---|---|---|---|"
        );
        if layout.cytoscape_service_bounds.is_empty() {
            let _ = writeln!(
                &mut report,
                "| `<none>` | `<none>` | `<none>` | `<none>` | `<none>` | `<none>` |"
            );
        } else {
            for service in &layout.cytoscape_service_bounds {
                let _ = writeln!(
                    &mut report,
                    "| `{}` | `{}` | `{}` | `{}` | `{}` | `{}` |",
                    service.id,
                    service.in_group.as_deref().unwrap_or("<none>"),
                    fmt_model_bounds(Some(&service.body_bounds)),
                    fmt_model_bounds(service.label_bounds.as_ref()),
                    format_local_label_metrics(service.label_metrics.as_ref()),
                    fmt_model_bounds(Some(&service.union_bounds))
                );
            }
        }
        let _ = writeln!(&mut report);

        if let Some((probe_path, probe)) = &probe_json {
            let group_parents = architecture_group_parent_map(&layouted.semantic);
            render_architecture_probe_join_markdown(
                &mut report,
                probe_path,
                probe,
                layout,
                &lo_services,
                &up_groups,
                &lo_groups,
                &group_parents,
            );
        }

        let _ = writeln!(
            &mut report,
            "## Element deltas (top 50 by max(abs(dx), abs(dy), abs(dw), abs(dh)))\n"
        );
        let _ = writeln!(
            &mut report,
            "| kind | id | upstream | local | dx | dy | dw | dh | score |\n|---|---|---|---|---:|---:|---:|---:|---:|"
        );
        for row in deltas.iter().take(50) {
            let _ = writeln!(
                &mut report,
                "| {} | `{}` | `{}` | `{}` | {:.6} | {:.6} | {} | {} | {:.6} |",
                row.kind,
                row.id,
                row.up,
                row.lo,
                row.dx,
                row.dy,
                fmt_optional_delta(row.dw),
                fmt_optional_delta(row.dh),
                row.score
            );
        }

        fs::write(&out_report, &report).map_err(|source| XtaskError::WriteFile {
            path: out_report.display().to_string(),
            source,
        })?;

        summaries.push(ArchitectureDeltaRunSummary {
            stem: stem.clone(),
            upstream_svg_path: out_upstream_svg.clone(),
            local_svg_path: out_local_svg.clone(),
            report_path: out_report.clone(),
            probe_json_path: probe_json
                .as_ref()
                .map(|(probe_path, _)| probe_path.clone()),
            viewbox_width_delta,
            viewbox_height_delta,
            max_width_delta,
            root_residual_score,
            service_count: up_services.len().min(lo_services.len()),
            junction_count: up_junctions.len().min(lo_junctions.len()),
            group_rect_count: up_groups.len().min(lo_groups.len()),
            delta_row_count: deltas.len(),
        });

        if idx > 0 {
            println!();
        }
        println!("fixture: {stem}");
        println!("upstream: {}", upstream_path.display());
        println!("local:    {}", out_local_svg.display());
        println!("report:   {}", out_report.display());
        if let (Some(up), Some(lo)) = (up_vb, lo_vb) {
            println!(
                "root viewBox: upstream=({:.6},{:.6},{:.6},{:.6}) local=({:.6},{:.6},{:.6},{:.6})",
                up.0, up.1, up.2, up.3, lo.0, lo.1, lo.2, lo.3
            );
        }
        if let (Some(up), Some(lo)) = (up_mw, lo_mw) {
            println!("max-width(px): upstream={:.6} local={:.6}", up, lo);
        }
        println!(
            "elements: services={} junctions={} group_rects={}",
            up_services.len().min(lo_services.len()),
            up_junctions.len().min(lo_junctions.len()),
            up_groups.len().min(lo_groups.len())
        );
    }

    if summaries.len() > 1 {
        let out_batch = architecture_delta_batch_markdown_path(&out_dir);
        fs::write(
            &out_batch,
            render_architecture_delta_batch_markdown(&summaries),
        )
        .map_err(|source| XtaskError::WriteFile {
            path: out_batch.display().to_string(),
            source,
        })?;
        println!();
        println!("batch:   {}", out_batch.display());
    }

    Ok(())
}

pub(crate) fn summarize_architecture_deltas(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_dir: Option<PathBuf> = None;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_dir = args.get(i).map(PathBuf::from);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    fn parse_viewbox(v: &str) -> Option<(f64, f64, f64, f64)> {
        let nums: Vec<f64> = v
            .split_whitespace()
            .filter_map(|s| s.trim().parse::<f64>().ok())
            .collect();
        if nums.len() != 4 {
            return None;
        }
        Some((nums[0], nums[1], nums[2], nums[3]))
    }

    fn parse_translate(transform: &str) -> Option<(f64, f64)> {
        let s = transform.trim();
        let s = s.strip_prefix("translate(")?;
        let s = s.strip_suffix(')')?;
        let parts: Vec<&str> = s
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|t: &&str| !t.trim().is_empty())
            .collect();
        let x = parts.first()?.trim().parse::<f64>().ok()?;
        let y = parts
            .get(1)
            .copied()
            .and_then(|v| v.trim().parse::<f64>().ok())?;
        Some((x, y))
    }

    fn parse_max_width_px(style: &str) -> Option<f64> {
        static RE: OnceLock<Regex> = OnceLock::new();
        let re = RE.get_or_init(|| Regex::new(r#"max-width:\s*([0-9.]+)px"#).unwrap());
        let cap = re.captures(style)?;
        cap.get(1)?.as_str().trim().parse::<f64>().ok()
    }

    fn has_class_token(class: &str, token: &str) -> bool {
        class.split_whitespace().any(|t| t == token)
    }

    fn sanitize_svg_id(stem: &str) -> String {
        stem.chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                    ch
                } else {
                    '_'
                }
            })
            .collect()
    }

    #[derive(Debug, Clone, Copy)]
    struct Pt {
        x: f64,
        y: f64,
    }

    #[derive(Debug, Clone, Copy)]
    struct Rect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    }

    type ArchSummary = (
        Option<(f64, f64, f64, f64)>,
        Option<f64>,
        BTreeMap<String, Pt>,
        BTreeMap<String, Pt>,
        BTreeMap<String, Rect>,
    );

    fn extract_arch_summary(svg: &str) -> Result<ArchSummary, XtaskError> {
        let doc = roxmltree::Document::parse(svg)
            .map_err(|e| XtaskError::SvgCompareFailed(format!("failed to parse svg xml: {e}")))?;
        let root = doc.root_element();
        let viewbox = root.attribute("viewBox").and_then(parse_viewbox);
        let max_width = root.attribute("style").and_then(parse_max_width_px);

        let mut services: BTreeMap<String, Pt> = BTreeMap::new();
        let mut junctions: BTreeMap<String, Pt> = BTreeMap::new();
        let mut groups: BTreeMap<String, Rect> = BTreeMap::new();

        for n in doc.descendants().filter(|n| n.is_element()) {
            let tag = n.tag_name().name();
            let id = n.attribute("id");

            if tag == "g"
                && n.attribute("class")
                    .is_some_and(|c| has_class_token(c, "architecture-service"))
            {
                if let (Some(id), Some((x, y))) = (
                    id.and_then(|id| normalize_arch_svg_id_with_marker(id, "service-")),
                    n.attribute("transform").and_then(parse_translate),
                ) {
                    services.insert(id, Pt { x, y });
                }
            }

            if tag == "g"
                && n.attribute("class")
                    .is_some_and(|c| has_class_token(c, "architecture-junction"))
            {
                let junction_id = id.and_then(normalize_arch_junction_svg_id).or_else(|| {
                    n.descendants()
                        .filter(|child| child.is_element())
                        .find_map(|child| {
                            child
                                .attribute("id")
                                .and_then(normalize_arch_junction_svg_id)
                        })
                });
                if let (Some(id), Some((x, y))) = (
                    junction_id,
                    n.attribute("transform").and_then(parse_translate),
                ) {
                    junctions.insert(id, Pt { x, y });
                }
            }

            if tag == "rect" {
                let Some(id) = id.and_then(|id| normalize_arch_svg_id_with_marker(id, "group-"))
                else {
                    continue;
                };
                let x = n
                    .attribute("x")
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(0.0);
                let y = n
                    .attribute("y")
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(0.0);
                let w = n
                    .attribute("width")
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(0.0);
                let h = n
                    .attribute("height")
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(0.0);
                groups.insert(id, Rect { x, y, w, h });
            }
        }

        Ok((viewbox, max_width, services, junctions, groups))
    }

    fn bbox_center_from_top_left_pts(pts: impl Iterator<Item = Pt>, size: f64) -> Option<Pt> {
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;

        let mut any = false;
        for p in pts {
            any = true;
            min_x = min_x.min(p.x);
            min_y = min_y.min(p.y);
            max_x = max_x.max(p.x + size);
            max_y = max_y.max(p.y + size);
        }
        if !any {
            return None;
        }
        Some(Pt {
            x: (min_x + max_x) / 2.0,
            y: (min_y + max_y) / 2.0,
        })
    }

    fn mean_delta_by_id(up: &BTreeMap<String, Pt>, lo: &BTreeMap<String, Pt>) -> Option<Pt> {
        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        let mut n = 0usize;
        for (id, up_p) in up {
            let Some(lo_p) = lo.get(id) else {
                continue;
            };
            sum_x += lo_p.x - up_p.x;
            sum_y += lo_p.y - up_p.y;
            n += 1;
        }
        if n == 0 {
            return None;
        }
        Some(Pt {
            x: sum_x / (n as f64),
            y: sum_y / (n as f64),
        })
    }

    fn max_group_rect_delta_by_id(
        up: &BTreeMap<String, Rect>,
        lo: &BTreeMap<String, Rect>,
    ) -> Option<(f64, f64, f64, f64)> {
        let mut best: Option<(f64, f64, f64, f64, f64)> = None;
        for (id, up_rect) in up {
            let Some(lo_rect) = lo.get(id) else {
                continue;
            };
            let dx = lo_rect.x - up_rect.x;
            let dy = lo_rect.y - up_rect.y;
            let dw = lo_rect.w - up_rect.w;
            let dh = lo_rect.h - up_rect.h;
            let score = dx.abs().max(dy.abs()).max(dw.abs()).max(dh.abs());
            if best.map_or(true, |(_, _, _, _, best_score)| score > best_score) {
                best = Some((dx, dy, dw, dh, score));
            }
        }
        best.map(|(dx, dy, dw, dh, _)| (dx, dy, dw, dh))
    }

    let fixtures_dir = crate::cmd::fixtures_root().join("architecture");
    let upstream_dir = crate::cmd::fixtures_root()
        .join("upstream-svgs")
        .join("architecture");
    let out_dir = out_dir.unwrap_or_else(|| {
        crate::cmd::target_root()
            .join("debug")
            .join("architecture-delta")
    });

    fs::create_dir_all(&out_dir).map_err(|source| XtaskError::WriteFile {
        path: out_dir.display().to_string(),
        source,
    })?;

    let fixtures = crate::cmd::list_mmd_fixtures_in_dir(&fixtures_dir, None, true);

    let engine = merman::Engine::new();
    let layout_opts = svg_compare_layout_opts();

    #[derive(Debug, Clone)]
    struct Row {
        stem: String,
        up_vb: Option<(f64, f64, f64, f64)>,
        lo_vb: Option<(f64, f64, f64, f64)>,
        viewbox_width_delta: Option<f64>,
        viewbox_height_delta: Option<f64>,
        up_mw: Option<f64>,
        lo_mw: Option<f64>,
        max_width_delta: Option<f64>,
        root_residual_score: Option<f64>,
        service_center_dx: Option<f64>,
        service_center_dy: Option<f64>,
        service_mean_dx: Option<f64>,
        service_mean_dy: Option<f64>,
        junction_mean_dx: Option<f64>,
        junction_mean_dy: Option<f64>,
        group_max_dx: Option<f64>,
        group_max_dy: Option<f64>,
        group_max_dw: Option<f64>,
        group_max_dh: Option<f64>,
    }

    let mut rows: Vec<Row> = Vec::new();

    for mmd_path in fixtures {
        let Some(stem) = mmd_path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| s.to_string())
        else {
            continue;
        };

        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        if !upstream_path.is_file() {
            continue;
        }

        let upstream_svg =
            fs::read_to_string(&upstream_path).map_err(|source| XtaskError::ReadFile {
                path: upstream_path.display().to_string(),
                source,
            })?;

        let text = fs::read_to_string(&mmd_path).map_err(|source| XtaskError::ReadFile {
            path: mmd_path.display().to_string(),
            source,
        })?;

        let parsed = futures::executor::block_on(
            engine.parse_diagram(&text, merman::ParseOptions::default()),
        )
        .map_err(|e| {
            XtaskError::SvgCompareFailed(format!("parse failed for {}: {e}", mmd_path.display()))
        })?
        .ok_or_else(|| {
            XtaskError::SvgCompareFailed(format!("no diagram detected in {}", mmd_path.display()))
        })?;

        let layouted = merman_render::layout_parsed(&parsed, &layout_opts).map_err(|e| {
            XtaskError::SvgCompareFailed(format!("layout failed for {}: {e}", mmd_path.display()))
        })?;

        let merman_render::model::LayoutDiagram::ArchitectureDiagram(layout) = &layouted.layout
        else {
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(sanitize_svg_id(&stem)),
            ..Default::default()
        };
        let local_svg = merman_render::svg::render_architecture_diagram_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            &svg_opts,
        )
        .map_err(|e| {
            XtaskError::SvgCompareFailed(format!("render failed for {}: {e}", mmd_path.display()))
        })?;

        let (up_vb, up_mw, up_services, up_junctions, up_groups) =
            extract_arch_summary(&upstream_svg)?;
        let (lo_vb, lo_mw, lo_services, lo_junctions, lo_groups) =
            extract_arch_summary(&local_svg)?;

        let icon_size = 80.0;
        let up_center = bbox_center_from_top_left_pts(up_services.values().copied(), icon_size);
        let lo_center = bbox_center_from_top_left_pts(lo_services.values().copied(), icon_size);
        let (service_center_dx, service_center_dy) = match (up_center, lo_center) {
            (Some(up), Some(lo)) => (Some(lo.x - up.x), Some(lo.y - up.y)),
            _ => (None, None),
        };

        let svc_mean = mean_delta_by_id(&up_services, &lo_services);
        let junc_mean = mean_delta_by_id(&up_junctions, &lo_junctions);
        let group_max = max_group_rect_delta_by_id(&up_groups, &lo_groups);
        let viewbox_width_delta = up_vb.zip(lo_vb).map(|(up, lo)| lo.2 - up.2);
        let viewbox_height_delta = up_vb.zip(lo_vb).map(|(up, lo)| lo.3 - up.3);
        let max_width_delta = up_mw.zip(lo_mw).map(|(up, lo)| lo - up);

        rows.push(Row {
            stem,
            up_vb,
            lo_vb,
            viewbox_width_delta,
            viewbox_height_delta,
            up_mw,
            lo_mw,
            max_width_delta,
            root_residual_score: architecture_root_residual_score(
                max_width_delta,
                viewbox_width_delta,
                viewbox_height_delta,
            ),
            service_center_dx,
            service_center_dy,
            service_mean_dx: svc_mean.map(|p| p.x),
            service_mean_dy: svc_mean.map(|p| p.y),
            junction_mean_dx: junc_mean.map(|p| p.x),
            junction_mean_dy: junc_mean.map(|p| p.y),
            group_max_dx: group_max.map(|(dx, _, _, _)| dx),
            group_max_dy: group_max.map(|(_, dy, _, _)| dy),
            group_max_dw: group_max.map(|(_, _, dw, _)| dw),
            group_max_dh: group_max.map(|(_, _, _, dh)| dh),
        });
    }

    rows.sort_by(|a, b| {
        architecture_delta_summary_sort_order(
            &a.stem,
            a.root_residual_score,
            &b.stem,
            b.root_residual_score,
        )
    });

    let out_report = out_dir.join("architecture-delta-summary.md");
    let mut md = String::new();
    let _ = writeln!(&mut md, "# Architecture Delta Summary\n");
    let _ = writeln!(
        &mut md,
        "Generated by `xtask summarize-architecture-deltas`.\n"
    );
    let _ = writeln!(
        &mut md,
        "| fixture | up viewBox | lo viewBox | viewBox width delta | viewBox height delta | up max-width | lo max-width | max-width delta | root residual score | svc bbox center dx | svc bbox center dy | svc mean dx | svc mean dy | junc mean dx | junc mean dy | group max dx | group max dy | group max dw | group max dh |"
    );
    let _ = writeln!(
        &mut md,
        "|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|"
    );

    for r in rows {
        let vb_up = r
            .up_vb
            .map(|v| format!("{:.3} {:.3} {:.3} {:.3}", v.0, v.1, v.2, v.3))
            .unwrap_or_else(|| "<missing>".to_string());
        let vb_lo = r
            .lo_vb
            .map(|v| format!("{:.3} {:.3} {:.3} {:.3}", v.0, v.1, v.2, v.3))
            .unwrap_or_else(|| "<missing>".to_string());

        let _ = writeln!(
            &mut md,
            "| `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` |",
            r.stem,
            vb_up,
            vb_lo,
            r.viewbox_width_delta
                .map(|v| format!("{:+.3}", v))
                .unwrap_or_else(|| "<missing>".to_string()),
            r.viewbox_height_delta
                .map(|v| format!("{:+.3}", v))
                .unwrap_or_else(|| "<missing>".to_string()),
            r.up_mw
                .map(|v| format!("{:.3}", v))
                .unwrap_or_else(|| "<missing>".to_string()),
            r.lo_mw
                .map(|v| format!("{:.3}", v))
                .unwrap_or_else(|| "<missing>".to_string()),
            r.max_width_delta
                .map(|v| format!("{:+.3}", v))
                .unwrap_or_else(|| "<missing>".to_string()),
            r.root_residual_score
                .map(|v| format!("{:.3}", v))
                .unwrap_or_else(|| "<missing>".to_string()),
            r.service_center_dx
                .map(|v| format!("{:.3}", v))
                .unwrap_or_else(|| "<n/a>".to_string()),
            r.service_center_dy
                .map(|v| format!("{:.3}", v))
                .unwrap_or_else(|| "<n/a>".to_string()),
            r.service_mean_dx
                .map(|v| format!("{:.3}", v))
                .unwrap_or_else(|| "<n/a>".to_string()),
            r.service_mean_dy
                .map(|v| format!("{:.3}", v))
                .unwrap_or_else(|| "<n/a>".to_string()),
            r.junction_mean_dx
                .map(|v| format!("{:.3}", v))
                .unwrap_or_else(|| "<n/a>".to_string()),
            r.junction_mean_dy
                .map(|v| format!("{:.3}", v))
                .unwrap_or_else(|| "<n/a>".to_string()),
            r.group_max_dx
                .map(|v| format!("{:.3}", v))
                .unwrap_or_else(|| "<n/a>".to_string()),
            r.group_max_dy
                .map(|v| format!("{:.3}", v))
                .unwrap_or_else(|| "<n/a>".to_string()),
            r.group_max_dw
                .map(|v| format!("{:.3}", v))
                .unwrap_or_else(|| "<n/a>".to_string()),
            r.group_max_dh
                .map(|v| format!("{:.3}", v))
                .unwrap_or_else(|| "<n/a>".to_string()),
        );
    }

    fs::write(&out_report, &md).map_err(|source| XtaskError::WriteFile {
        path: out_report.display().to_string(),
        source,
    })?;

    println!("report: {}", out_report.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn architecture_svg_id_normalizer_accepts_prefixed_current_ids() {
        assert_eq!(
            normalize_arch_svg_id_with_marker(
                "stress_architecture_batch5_long_titles_and_punct_076-service-runner",
                "service-"
            )
            .as_deref(),
            Some("service-runner")
        );
        assert_eq!(
            normalize_arch_svg_id_with_marker(
                "stress_architecture_batch5_long_titles_and_punct_076-group-pipeline",
                "group-"
            )
            .as_deref(),
            Some("group-pipeline")
        );
        assert_eq!(
            normalize_arch_junction_svg_id("stress_architecture_junction_fork_join_026-node-fork")
                .as_deref(),
            Some("junction-fork")
        );
    }

    #[test]
    fn architecture_svg_id_normalizer_keeps_legacy_ids() {
        assert_eq!(
            normalize_arch_svg_id_with_marker("service-runner", "service-").as_deref(),
            Some("service-runner")
        );
        assert_eq!(
            normalize_arch_svg_id_with_marker("group-pipeline", "group-").as_deref(),
            Some("group-pipeline")
        );
        assert_eq!(
            normalize_arch_junction_svg_id("junction-fork").as_deref(),
            Some("junction-fork")
        );
    }

    #[test]
    fn architecture_delta_summary_order_sorts_by_root_residual_score_then_stem() {
        assert_eq!(
            architecture_root_residual_score(Some(2.0), Some(-3.0), Some(5.0)),
            Some(5.0)
        );
        assert_eq!(
            architecture_root_residual_score(None, Some(-6.0), Some(1.0)),
            Some(6.0)
        );
        assert_eq!(architecture_root_residual_score(None, None, None), None);
        assert_eq!(
            architecture_delta_summary_sort_order("small", Some(2.0), "large", Some(5.0)),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            architecture_delta_summary_sort_order("width", Some(5.0), "height", Some(6.0)),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            architecture_delta_summary_sort_order("large", Some(5.0), "small", Some(2.0)),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            architecture_delta_summary_sort_order("a", Some(1.0), "b", Some(1.0)),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            architecture_delta_summary_sort_order("has-delta", Some(0.0), "missing", None),
            std::cmp::Ordering::Less
        );
    }

    #[test]
    fn architecture_delta_args_accept_probe_dir() {
        let parsed = parse_architecture_delta_args(&args(&[
            "--fixture",
            "batch5_long_titles",
            "--fixture",
            "html_titles",
            "--out-dir",
            "target/custom-delta",
            "--probe-dir",
            "target/custom-probe",
        ]))
        .unwrap();

        assert_eq!(
            parsed.fixture_filters,
            vec!["batch5_long_titles", "html_titles"]
        );
        assert_eq!(parsed.out_dir, Some(PathBuf::from("target/custom-delta")));
        assert_eq!(parsed.probe_dir, Some(PathBuf::from("target/custom-probe")));
    }

    #[test]
    fn fcose_probe_args_require_fixture() {
        assert!(matches!(
            parse_architecture_fcose_probe_args(&[]),
            Err(XtaskError::Usage)
        ));
        assert!(matches!(
            parse_architecture_fcose_probe_args(&args(&["--fixture", ""])),
            Err(XtaskError::Usage)
        ));
    }

    #[test]
    fn fcose_probe_args_accept_out_dir_aliases() {
        let parsed = parse_architecture_fcose_probe_args(&args(&[
            "--fixture",
            "batch5_long_titles",
            "--out-dir",
            "target/custom-probe",
            "--browser-exe",
            "C:/Browser/chrome.exe",
        ]))
        .unwrap();
        assert_eq!(parsed.fixture_filters, vec!["batch5_long_titles"]);
        assert_eq!(parsed.out_dir, PathBuf::from("target/custom-probe"));
        assert_eq!(
            parsed.browser_exe,
            Some(PathBuf::from("C:/Browser/chrome.exe"))
        );

        let parsed = parse_architecture_fcose_probe_args(&args(&[
            "--fixture",
            "html_titles",
            "--out",
            "target/alt",
        ]))
        .unwrap();
        assert_eq!(parsed.fixture_filters, vec!["html_titles"]);
        assert_eq!(parsed.out_dir, PathBuf::from("target/alt"));
    }

    #[test]
    fn fcose_probe_args_accept_repeated_fixture_filters() {
        let parsed = parse_architecture_fcose_probe_args(&args(&[
            "--fixture",
            "batch5_long_titles",
            "--fixture",
            "group_port_edges",
        ]))
        .unwrap();

        assert_eq!(
            parsed.fixture_filters,
            vec!["batch5_long_titles", "group_port_edges"]
        );
    }

    #[test]
    fn fcose_probe_batch_markdown_links_per_fixture_artifacts() {
        let summaries = vec![
            ArchitectureFcoseProbeRunSummary {
                stem: "fixture_a".to_string(),
                json_path: PathBuf::from("target/probe/fixture_a.fcose-browser-probe.json"),
                markdown_path: PathBuf::from("target/probe/fixture_a.fcose-browser-probe.md"),
                stage_count: 4,
                node_count: 5,
                edge_count: 3,
            },
            ArchitectureFcoseProbeRunSummary {
                stem: "fixture_b".to_string(),
                json_path: PathBuf::from("target/probe/fixture_b.fcose-browser-probe.json"),
                markdown_path: PathBuf::from("target/probe/fixture_b.fcose-browser-probe.md"),
                stage_count: 4,
                node_count: 6,
                edge_count: 4,
            },
        ];

        let md = render_architecture_fcose_probe_batch_markdown(&summaries);

        assert!(md.contains("# Architecture FCoSE Browser Probe Batch"));
        assert!(md.contains("| `fixture_a` | `target/probe/fixture_a.fcose-browser-probe.json` | `target/probe/fixture_a.fcose-browser-probe.md` | 4 | 5 | 3 |"));
        assert!(md.contains("| `fixture_b` | `target/probe/fixture_b.fcose-browser-probe.json` | `target/probe/fixture_b.fcose-browser-probe.md` | 4 | 6 | 4 |"));
    }

    #[test]
    fn architecture_delta_batch_markdown_links_per_fixture_artifacts() {
        let summaries = vec![
            ArchitectureDeltaRunSummary {
                stem: "fixture_a".to_string(),
                upstream_svg_path: PathBuf::from("target/delta/fixture_a.upstream.svg"),
                local_svg_path: PathBuf::from("target/delta/fixture_a.local.svg"),
                report_path: PathBuf::from("target/delta/fixture_a.md"),
                probe_json_path: Some(PathBuf::from(
                    "target/probe/fixture_a.fcose-browser-probe.json",
                )),
                viewbox_width_delta: Some(5.0),
                viewbox_height_delta: Some(0.0),
                max_width_delta: Some(5.0),
                root_residual_score: Some(5.0),
                service_count: 2,
                junction_count: 1,
                group_rect_count: 1,
                delta_row_count: 4,
            },
            ArchitectureDeltaRunSummary {
                stem: "fixture_b".to_string(),
                upstream_svg_path: PathBuf::from("target/delta/fixture_b.upstream.svg"),
                local_svg_path: PathBuf::from("target/delta/fixture_b.local.svg"),
                report_path: PathBuf::from("target/delta/fixture_b.md"),
                probe_json_path: None,
                viewbox_width_delta: Some(0.0),
                viewbox_height_delta: Some(-6.0),
                max_width_delta: None,
                root_residual_score: Some(6.0),
                service_count: 3,
                junction_count: 0,
                group_rect_count: 2,
                delta_row_count: 5,
            },
        ];

        let md = render_architecture_delta_batch_markdown(&summaries);

        assert!(md.contains("# Architecture Delta Batch"));
        let fixture_b = "| `fixture_b` | `target/delta/fixture_b.md` | `target/delta/fixture_b.upstream.svg` | `target/delta/fixture_b.local.svg` | `<none>` | `+0.000` | `-6.000` | `<missing>` | `6.000` | 3 | 0 | 2 | 5 |";
        let fixture_a = "| `fixture_a` | `target/delta/fixture_a.md` | `target/delta/fixture_a.upstream.svg` | `target/delta/fixture_a.local.svg` | `target/probe/fixture_a.fcose-browser-probe.json` | `+5.000` | `+0.000` | `+5.000` | `5.000` | 2 | 1 | 1 | 4 |";
        assert!(md.contains(fixture_a));
        assert!(md.contains(fixture_b));
        assert!(md.find(fixture_b).unwrap() < md.find(fixture_a).unwrap());
    }

    #[test]
    fn fcose_probe_json_path_uses_fixture_stem() {
        assert_eq!(
            architecture_fcose_probe_json_path(Path::new("target/probe"), "fixture_001"),
            PathBuf::from("target/probe").join("fixture_001.fcose-browser-probe.json")
        );
    }

    #[test]
    fn fcose_probe_markdown_summarizes_stage_and_node_bounds() {
        let probe = serde_json::json!({
            "config": { "iconSize": 80, "fontSize": 16 },
            "stages": [
                { "tag": "probe-installed" },
                { "tag": "bbBeforeRun2", "bb": { "x1": 1.0, "y1": 2.0, "w": 30.0, "h": 40.0 } },
                {
                    "tag": "relocateComponent",
                    "runIndex": 1,
                    "originalCenter": { "x": 15.0, "y": 25.0 },
                    "rectBbox": { "x1": 4.0, "y1": 5.0, "w": 6.0, "h": 7.0 },
                    "rectCenter": { "x": 7.0, "y": 8.5 },
                    "delta": { "x": 8.0, "y": 16.5 }
                }
            ],
            "finalElements": {
                "nodes": [{
                    "id": "group",
                    "pos": { "x": 10.0, "y": 20.0 },
                    "bb": { "x1": 1.0, "y1": 2.0, "w": 20.0, "h": 30.0 },
                    "bodyBounds": { "x1": 1.0, "y1": 2.0, "w": 20.0, "h": 30.0 },
                    "labelBounds": { "all": { "x1": 3.0, "y1": 4.0, "w": 5.0, "h": 6.0 } },
                    "childrenBoundingBoxIncludeLabels": { "x1": 4.0, "y1": 5.0, "w": 10.0, "h": 12.0 },
                    "childrenBoundingBoxBodyOnly": { "x1": 5.0, "y1": 7.0, "w": 6.0, "h": 4.0 },
                    "classes": ["node-group"],
                    "data": { "type": "group", "label": "Group Label" }
                }],
                "edges": [{
                    "id": "svc-other",
                    "bb": { "x1": 7.0, "y1": 8.0, "w": 9.0, "h": 10.0 },
                    "sourceEndpoint": { "x": 11.0, "y": 12.0 },
                    "targetEndpoint": { "x": 13.0, "y": 14.0 },
                    "classes": ["straight"],
                    "data": { "source": "svc", "target": "other", "sourceDir": "R", "targetDir": "L" },
                    "style": {
                        "curveStyle": "straight",
                        "segmentWeights": "0.5",
                        "segmentDistances": "20px",
                        "edgeDistances": "intersection"
                    }
                }]
            }
        });

        let md = render_architecture_fcose_probe_markdown(
            "fixture_001",
            Path::new("fixtures/architecture/fixture_001.mmd"),
            Path::new("target/probe/fixture_001.fcose-browser-probe.json"),
            &probe,
        );

        assert!(md.contains("# Architecture FCoSE Browser Probe"));
        assert!(md.contains("| `bbBeforeRun2` | `x1=1.000 y1=2.000 w=30.000 h=40.000` |"));
        assert!(md.contains("## Relocation Stages"));
        assert!(md.contains("| 1 | `x=15.000 y=25.000` | `x1=4.000 y1=5.000 w=6.000 h=7.000` | `x=7.000 y=8.500` | `x=8.000 y=16.500` |"));
        assert!(md.contains("| `group` | `group` | `node-group` | `x=10.000 y=20.000` | `x1=1.000 y1=2.000 w=20.000 h=30.000` | `x1=1.000 y1=2.000 w=20.000 h=30.000` | `x1=3.000 y1=4.000 w=5.000 h=6.000` | `x1=4.000 y1=5.000 w=10.000 h=12.000` | `x1=5.000 y1=7.000 w=6.000 h=4.000` | `l=1.000 r=3.000 t=2.000 b=6.000 dw=4.000 dh=8.000` | `l=3.000 r=7.000 t=3.000 b=15.000 dw=10.000 dh=18.000` | `Group Label` |"));
        assert!(md.contains("## Final Edge Bounds"));
        assert!(md.contains("| `svc-other` | `svc -> other` | `straight` | `R -> L` | `x1=7.000 y1=8.000 w=9.000 h=10.000` | `x=11.000 y=12.000` | `x=13.000 y=14.000` | `straight` | `0.5` | `20px` | `intersection` |"));
    }

    #[test]
    fn architecture_probe_join_decomposes_group_and_service_bounds() {
        let probe = serde_json::json!({
            "finalElements": {
                "nodes": [
                    {
                        "id": "pipeline",
                        "pos": { "x": 0.0, "y": 0.0 },
                        "bb": { "x1": 0.0, "y1": 10.0, "w": 183.0, "h": 133.0 },
                        "childrenBoundingBoxIncludeLabels": {
                            "x1": 10.0,
                            "y1": 20.0,
                            "w": 100.0,
                            "h": 50.0
                        },
                        "data": { "type": "group" }
                    },
                    {
                        "id": "storage",
                        "pos": { "x": 20.0, "y": 30.0 },
                        "bb": { "x1": 20.0, "y1": 30.0, "w": 101.0, "h": 50.0 },
                        "bodyBounds": { "x1": 20.0, "y1": 30.0, "w": 82.0, "h": 42.0 },
                        "labelWidth": 99.0,
                        "labelHeight": 16.0,
                        "labelBounds": {
                            "all": { "x1": 20.0, "y1": 30.0, "w": 101.0, "h": 20.0 }
                        },
                        "data": { "type": "service" }
                    }
                ]
            }
        });
        let layout = merman_render::model::ArchitectureDiagramLayout {
            nodes: Vec::new(),
            edges: Vec::new(),
            cytoscape_service_bounds: vec![
                merman_render::model::ArchitectureCytoscapeServiceBounds {
                    id: "storage".to_string(),
                    in_group: Some("pipeline".to_string()),
                    body_bounds: merman_render::model::Bounds {
                        min_x: 20.0,
                        min_y: 30.0,
                        max_x: 100.0,
                        max_y: 70.0,
                    },
                    label_bounds: Some(merman_render::model::Bounds {
                        min_x: 20.0,
                        min_y: 30.0,
                        max_x: 123.0,
                        max_y: 78.0,
                    }),
                    label_metrics: Some(
                        merman_render::model::ArchitectureCytoscapeServiceLabelMetrics {
                            text_width: 103.0,
                            half_width: 51.5,
                            applied_scale: 1.055,
                        },
                    ),
                    union_bounds: merman_render::model::Bounds {
                        min_x: 20.0,
                        min_y: 30.0,
                        max_x: 123.0,
                        max_y: 78.0,
                    },
                },
            ],
            fcose_compound_bounds: Vec::new(),
            bounds: None,
        };
        let local_service_positions =
            BTreeMap::from([("service-storage".to_string(), DebugPt { x: 21.0, y: 31.0 })]);
        let upstream_groups = BTreeMap::from([(
            "group-pipeline".to_string(),
            DebugRect {
                x: 10.0,
                y: 20.0,
                w: 183.0,
                h: 133.0,
            },
        )]);
        let local_groups = BTreeMap::from([(
            "group-pipeline".to_string(),
            DebugRect {
                x: 10.0,
                y: 20.0,
                w: 188.0,
                h: 133.0,
            },
        )]);

        let mut md = String::new();
        render_architecture_probe_join_markdown(
            &mut md,
            Path::new("target/probe/sample.fcose-browser-probe.json"),
            &probe,
            &layout,
            &local_service_positions,
            &upstream_groups,
            &local_groups,
            &BTreeMap::new(),
        );

        assert!(md.contains("## Browser probe phase join"));
        assert!(md.contains("| `pipeline` | 1 | `x=10.000000 y=20.000000 w=100.000000 h=50.000000` | `x=20.000000 y=30.000000 w=103.000000 h=48.000000` | 3.000000 | -2.000000 |"));
        assert!(
            md.contains(
                "l=10.000000 r=73.000000 t=10.000000 b=73.000000 dw=83.000000 dh=83.000000"
            )
        );
        assert!(
            md.contains(
                "l=10.000000 r=75.000000 t=10.000000 b=75.000000 dw=85.000000 dh=85.000000"
            )
        );
        assert!(md.contains("| `storage` | `pipeline` | `x=20.000000 y=30.000000` | `x=21.000000 y=31.000000` | 1.000000 | 1.000000 |"));
        assert!(md.contains("| -2.000000 | -2.000000 | `w=99.000000 h=16.000000` | `text_w=103.000000 half=51.500000 scale=1.055000` | 4.000000 |"));
        assert!(md.contains("local contribution label final-frame"));
        assert!(md.contains("| `x=20.000000 y=30.000000 w=101.000000 h=20.000000` | `x=20.000000 y=30.000000 w=103.000000 h=48.000000` | `x=-20.000000 y=10.000000 w=103.000000 h=48.000000` | -40.000000 | -20.000000 | 2.000000 | 28.000000 |"));
        assert!(md.contains("browser child union"));
        assert!(md.contains("`x=20.000000 y=30.000000 w=101.000000 h=50.000000`"));
        assert!(md.contains(
            "| `x=-20.000000 y=10.000000 w=103.000000 h=48.000000` | -40.000000 | -20.000000 |"
        ));
        assert!(md.contains("local final bb final-frame"));
        assert!(md.contains(
            "| `x=20.000000 y=30.000000 w=101.000000 h=50.000000` | `x=-21.000000 y=9.000000 w=105.000000 h=50.000000` | -41.000000 | -21.000000 | 4.000000 | 0.000000 |"
        ));
        assert!(md.contains("### Group content edge attribution"));
        assert!(md.contains("| `pipeline` | 1 | `storage@20.000000` | `storage@-20.000000` | -40.000000 | `storage@121.000000` | `storage@83.000000` | -38.000000 | 2.000000 | `storage@30.000000` | `storage@10.000000` | -20.000000 | `storage@72.000000` | `storage@58.000000` | -14.000000 | 6.000000 |"));
    }

    #[test]
    fn architecture_probe_join_reports_nested_group_aggregate_content() {
        let probe = serde_json::json!({
            "finalElements": {
                "nodes": [
                    {
                        "id": "platform",
                        "bb": { "x1": 0.0, "y1": 0.0, "w": 200.0, "h": 200.0 },
                        "childrenBoundingBoxIncludeLabels": {
                            "x1": 10.0,
                            "y1": 10.0,
                            "w": 100.0,
                            "h": 100.0
                        },
                        "data": { "type": "group" }
                    },
                    {
                        "id": "runtime",
                        "bb": { "x1": 10.0, "y1": 10.0, "w": 100.0, "h": 100.0 },
                        "data": { "type": "group" }
                    }
                ]
            }
        });
        let layout = merman_render::model::ArchitectureDiagramLayout {
            nodes: Vec::new(),
            edges: Vec::new(),
            cytoscape_service_bounds: Vec::new(),
            fcose_compound_bounds: Vec::new(),
            bounds: None,
        };
        let upstream_groups = BTreeMap::from([(
            "group-platform".to_string(),
            DebugRect {
                x: 0.0,
                y: 0.0,
                w: 200.0,
                h: 200.0,
            },
        )]);
        let local_groups = BTreeMap::from([
            (
                "group-platform".to_string(),
                DebugRect {
                    x: 0.0,
                    y: 0.0,
                    w: 203.0,
                    h: 200.0,
                },
            ),
            (
                "group-runtime".to_string(),
                DebugRect {
                    x: 10.0,
                    y: 10.0,
                    w: 103.0,
                    h: 100.0,
                },
            ),
        ]);
        let group_parents = BTreeMap::from([("runtime".to_string(), Some("platform".to_string()))]);

        let mut md = String::new();
        render_architecture_probe_join_markdown(
            &mut md,
            Path::new("target/probe/nested.fcose-browser-probe.json"),
            &probe,
            &layout,
            &BTreeMap::new(),
            &upstream_groups,
            &local_groups,
            &group_parents,
        );

        assert!(md.contains("### Group aggregate child attribution"));
        assert!(md.contains("| `platform` | 0 | `runtime` | `x=10.000000 y=10.000000 w=100.000000 h=100.000000` | `x=10.000000 y=10.000000 w=103.000000 h=100.000000` | 3.000000 | 0.000000 |"));
    }
}
