//! Flowchart debug utilities.

use crate::XtaskError;
use crate::util::*;
use regex::Regex;
use serde_json::Value as JsonValue;
use serde_yaml::Value as YamlValue;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use super::super::svg_compare_layout_opts;

pub(crate) fn debug_flowchart_svg_roots(args: Vec<String>) -> Result<(), XtaskError> {
    let mut fixture: Option<String> = None;
    let mut upstream: Option<PathBuf> = None;
    let mut local: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--fixture" => {
                i += 1;
                fixture = args.get(i).map(|s| s.to_string());
            }
            "--upstream" => {
                i += 1;
                upstream = args.get(i).map(PathBuf::from);
            }
            "--local" => {
                i += 1;
                local = args.get(i).map(PathBuf::from);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");

    if let Some(f) = fixture.as_deref() {
        let upstream_default = workspace_root
            .join("fixtures")
            .join("upstream-svgs")
            .join("flowchart")
            .join(format!("{f}.svg"));
        let local_default = workspace_root
            .join("target")
            .join("compare")
            .join("flowchart")
            .join(format!("{f}.svg"));
        upstream = upstream.or(Some(upstream_default));
        local = local.or(Some(local_default));
    }

    let Some(upstream_path) = upstream else {
        return Err(XtaskError::Usage);
    };
    let Some(local_path) = local else {
        return Err(XtaskError::Usage);
    };

    let upstream_svg =
        fs::read_to_string(&upstream_path).map_err(|source| XtaskError::ReadFile {
            path: upstream_path.display().to_string(),
            source,
        })?;
    let local_svg = fs::read_to_string(&local_path).map_err(|source| XtaskError::ReadFile {
        path: local_path.display().to_string(),
        source,
    })?;

    #[derive(Debug, Clone)]
    struct ClusterInfo {
        id: String,
        root_translate: Option<String>,
        rect_x: Option<String>,
        rect_y: Option<String>,
        rect_w: Option<String>,
        rect_h: Option<String>,
    }

    #[derive(Debug, Clone)]
    struct Summary {
        view_box: Option<String>,
        max_width: Option<String>,
        root_transforms: Vec<String>,
        clusters: Vec<ClusterInfo>,
    }

    fn parse_translate(transform: &str) -> Option<String> {
        // Keep the exact token payload inside `translate(...)` for readability.
        let t = transform.trim();
        let t = t.strip_prefix("translate(")?;
        let t = t.strip_suffix(')')?;
        Some(t.trim().to_string())
    }

    fn parse_summary(svg: &str) -> Result<Summary, String> {
        let doc = roxmltree::Document::parse(svg).map_err(|e| e.to_string())?;
        let root = doc.root_element();
        let view_box = root.attribute("viewBox").map(|s| s.to_string());
        let max_width = root.attribute("style").and_then(|s| {
            // Extract `max-width: <n>px` when present.
            static RE: OnceLock<Regex> = OnceLock::new();
            let re = RE.get_or_init(|| Regex::new(r#"max-width:\s*([0-9.]+)px"#).unwrap());
            re.captures(s)
                .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
        });

        let mut root_transforms: Vec<String> = Vec::new();
        let mut clusters: Vec<ClusterInfo> = Vec::new();

        for n in doc.descendants().filter(|n| n.is_element()) {
            if n.tag_name().name() == "g" {
                if let Some(class) = n.attribute("class") {
                    if class.split_whitespace().any(|t| t == "root") {
                        if let Some(transform) = n.attribute("transform") {
                            if let Some(t) = parse_translate(transform) {
                                root_transforms.push(t);
                            }
                        }
                    }
                    if class.split_whitespace().any(|t| t == "cluster") {
                        if let Some(id) = n.attribute("id") {
                            let mut root_translate: Option<String> = None;
                            for a in n.ancestors() {
                                if !a.is_element() || a.tag_name().name() != "g" {
                                    continue;
                                }
                                let Some(class) = a.attribute("class") else {
                                    continue;
                                };
                                if !class.split_whitespace().any(|t| t == "root") {
                                    continue;
                                }
                                let Some(transform) = a.attribute("transform") else {
                                    continue;
                                };
                                root_translate = parse_translate(transform);
                                break;
                            }

                            let rect = n
                                .children()
                                .find(|c| c.is_element() && c.tag_name().name() == "rect");
                            let rect_x = rect.and_then(|r| r.attribute("x")).map(|s| s.to_string());
                            let rect_y = rect.and_then(|r| r.attribute("y")).map(|s| s.to_string());
                            let rect_w = rect
                                .and_then(|r| r.attribute("width"))
                                .map(|s| s.to_string());
                            let rect_h = rect
                                .and_then(|r| r.attribute("height"))
                                .map(|s| s.to_string());

                            clusters.push(ClusterInfo {
                                id: id.to_string(),
                                root_translate,
                                rect_x,
                                rect_y,
                                rect_w,
                                rect_h,
                            });
                        }
                    }
                }
            }
        }

        root_transforms.sort();
        root_transforms.dedup();
        clusters.sort_by(|a, b| a.id.cmp(&b.id));

        Ok(Summary {
            view_box,
            max_width,
            root_transforms,
            clusters,
        })
    }

    let upstream_summary = parse_summary(&upstream_svg).map_err(XtaskError::DebugSvgFailed)?;
    let local_summary = parse_summary(&local_svg).map_err(XtaskError::DebugSvgFailed)?;

    println!("upstream: {}", upstream_path.display());
    println!("local:    {}", local_path.display());
    println!();

    println!("== Root SVG ==");
    println!(
        "upstream viewBox: {:?}",
        upstream_summary.view_box.as_deref().unwrap_or("<missing>")
    );
    println!(
        "local    viewBox: {:?}",
        local_summary.view_box.as_deref().unwrap_or("<missing>")
    );
    println!(
        "upstream max-width(px): {:?}",
        upstream_summary.max_width.as_deref().unwrap_or("<missing>")
    );
    println!(
        "local    max-width(px): {:?}",
        local_summary.max_width.as_deref().unwrap_or("<missing>")
    );
    println!();

    println!("== <g class=\"root\" transform=\"translate(...)\"> ==");
    println!("upstream:");
    for t in &upstream_summary.root_transforms {
        println!("- {t}");
    }
    println!("local:");
    for t in &local_summary.root_transforms {
        println!("- {t}");
    }
    println!();

    println!("== Clusters ==");
    fn fmt_cluster(c: &ClusterInfo) -> String {
        format!(
            "id={} root={:?} rect=({:?}, {:?}, {:?}, {:?})",
            c.id, c.root_translate, c.rect_x, c.rect_y, c.rect_w, c.rect_h
        )
    }
    let mut upstream_by_id: std::collections::HashMap<&str, &ClusterInfo> =
        std::collections::HashMap::new();
    for c in &upstream_summary.clusters {
        upstream_by_id.insert(c.id.as_str(), c);
    }
    for c in &local_summary.clusters {
        let up = upstream_by_id.get(c.id.as_str()).copied();
        if let Some(up) = up {
            if up.root_translate != c.root_translate
                || up.rect_w != c.rect_w
                || up.rect_h != c.rect_h
                || up.rect_x != c.rect_x
                || up.rect_y != c.rect_y
            {
                println!("upstream: {}", fmt_cluster(up));
                println!("local:    {}", fmt_cluster(c));
            }
        } else {
            println!("local-only: {}", fmt_cluster(c));
        }
    }
    for c in &upstream_summary.clusters {
        if !local_summary.clusters.iter().any(|l| l.id == c.id) {
            println!("upstream-only: {}", fmt_cluster(c));
        }
    }

    Ok(())
}

pub(crate) fn debug_flowchart_svg_positions(args: Vec<String>) -> Result<(), XtaskError> {
    let mut fixture: Option<String> = None;
    let mut upstream: Option<PathBuf> = None;
    let mut local: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--fixture" => {
                i += 1;
                fixture = args.get(i).map(|s| s.to_string());
            }
            "--upstream" => {
                i += 1;
                upstream = args.get(i).map(PathBuf::from);
            }
            "--local" => {
                i += 1;
                local = args.get(i).map(PathBuf::from);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");

    if let Some(f) = fixture.as_deref() {
        let upstream_default = workspace_root
            .join("fixtures")
            .join("upstream-svgs")
            .join("flowchart")
            .join(format!("{f}.svg"));
        let local_default = workspace_root
            .join("target")
            .join("compare")
            .join("flowchart")
            .join(format!("{f}.svg"));
        upstream = upstream.or(Some(upstream_default));
        local = local.or(Some(local_default));
    }

    let Some(upstream_path) = upstream else {
        return Err(XtaskError::Usage);
    };
    let Some(local_path) = local else {
        return Err(XtaskError::Usage);
    };

    let upstream_svg =
        fs::read_to_string(&upstream_path).map_err(|source| XtaskError::ReadFile {
            path: upstream_path.display().to_string(),
            source,
        })?;
    let local_svg = fs::read_to_string(&local_path).map_err(|source| XtaskError::ReadFile {
        path: local_path.display().to_string(),
        source,
    })?;

    #[derive(Debug, Clone, Copy)]
    struct Translate {
        x: f64,
        y: f64,
    }

    fn parse_translate(transform: &str) -> Option<Translate> {
        let t = transform.trim();
        let t = t.strip_prefix("translate(")?;
        let t = t.strip_suffix(')')?;
        let parts = t
            .split(|ch: char| ch == ',' || ch.is_whitespace())
            .filter(|s| !s.trim().is_empty())
            .filter_map(|s| s.trim().parse::<f64>().ok())
            .collect::<Vec<_>>();
        match parts.as_slice() {
            [x, y] => Some(Translate { x: *x, y: *y }),
            [x] => Some(Translate { x: *x, y: 0.0 }),
            _ => None,
        }
    }

    fn accumulated_translate(node: roxmltree::Node<'_, '_>) -> Translate {
        let mut x = 0.0;
        let mut y = 0.0;
        // `ancestors()` includes the node itself; we want the sum of parent transforms only.
        for n in node.ancestors().filter(|n| n.is_element()).skip(1) {
            if let Some(transform) = n.attribute("transform") {
                if let Some(t) = parse_translate(transform) {
                    x += t.x;
                    y += t.y;
                }
            }
        }
        Translate { x, y }
    }

    #[derive(Debug, Clone)]
    struct NodePos {
        kind: &'static str,
        x: f64,
        y: f64,
    }

    #[derive(Debug, Clone)]
    struct ClusterRect {
        left: f64,
        top: f64,
        w: f64,
        h: f64,
    }

    type PositionsAndClusters = (BTreeMap<String, NodePos>, BTreeMap<String, ClusterRect>);

    fn parse_positions(svg: &str) -> Result<PositionsAndClusters, String> {
        let doc = roxmltree::Document::parse(svg).map_err(|e| e.to_string())?;

        let mut nodes: BTreeMap<String, NodePos> = BTreeMap::new();
        let mut clusters: BTreeMap<String, ClusterRect> = BTreeMap::new();

        for n in doc.descendants().filter(|n| n.is_element()) {
            if n.tag_name().name() != "g" {
                continue;
            }
            let Some(id) = n.attribute("id") else {
                continue;
            };
            let class = n.attribute("class").unwrap_or_default();
            let class_tokens = class.split_whitespace().collect::<Vec<_>>();

            if class_tokens.contains(&"node") {
                let Some(transform) = n.attribute("transform") else {
                    continue;
                };
                let Some(local) = parse_translate(transform) else {
                    continue;
                };
                let abs = accumulated_translate(n);
                nodes.insert(
                    id.to_string(),
                    NodePos {
                        kind: "node",
                        x: local.x + abs.x,
                        y: local.y + abs.y,
                    },
                );
                continue;
            }

            // Mermaid self-loop helper nodes use `<g class="label edgeLabel" id="X---X---1" transform="translate(...)">`.
            if class_tokens.contains(&"edgeLabel") && class_tokens.contains(&"label") {
                let Some(transform) = n.attribute("transform") else {
                    continue;
                };
                let Some(local) = parse_translate(transform) else {
                    continue;
                };
                let abs = accumulated_translate(n);
                nodes.insert(
                    id.to_string(),
                    NodePos {
                        kind: "labelRect",
                        x: local.x + abs.x,
                        y: local.y + abs.y,
                    },
                );
                continue;
            }

            if class_tokens.contains(&"cluster") {
                let abs = accumulated_translate(n);
                let rect = n
                    .children()
                    .find(|c| c.is_element() && c.tag_name().name() == "rect");
                let Some(rect) = rect else {
                    continue;
                };
                let x = rect
                    .attribute("x")
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(0.0);
                let y = rect
                    .attribute("y")
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(0.0);
                let w = rect
                    .attribute("width")
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(0.0);
                let h = rect
                    .attribute("height")
                    .and_then(|v| v.parse::<f64>().ok())
                    .unwrap_or(0.0);
                clusters.insert(
                    id.to_string(),
                    ClusterRect {
                        left: abs.x + x,
                        top: abs.y + y,
                        w,
                        h,
                    },
                );
            }
        }

        Ok((nodes, clusters))
    }

    let (up_nodes, up_clusters) =
        parse_positions(&upstream_svg).map_err(XtaskError::DebugSvgFailed)?;
    let (lo_nodes, lo_clusters) =
        parse_positions(&local_svg).map_err(XtaskError::DebugSvgFailed)?;

    println!("upstream: {}", upstream_path.display());
    println!("local:    {}", local_path.display());
    println!();

    println!("== Nodes / LabelRects (abs translate) ==");
    let mut node_ids: Vec<&String> = up_nodes.keys().collect();
    node_ids.sort();
    for id in node_ids {
        let Some(a) = up_nodes.get(id) else { continue };
        let Some(b) = lo_nodes.get(id) else { continue };
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        if dx.abs() < 1e-6 && dy.abs() < 1e-6 {
            continue;
        }
        println!(
            "{id} kind={} upstream=({:.6},{:.6}) local=({:.6},{:.6}) Δ=({:.6},{:.6})",
            a.kind, a.x, a.y, b.x, b.y, dx, dy
        );
    }
    for (id, b) in &lo_nodes {
        if !up_nodes.contains_key(id) {
            println!("{id} local-only kind={} ({:.6},{:.6})", b.kind, b.x, b.y);
        }
    }
    for (id, a) in &up_nodes {
        if !lo_nodes.contains_key(id) {
            println!("{id} upstream-only kind={} ({:.6},{:.6})", a.kind, a.x, a.y);
        }
    }
    println!();

    println!("== Clusters (abs rect) ==");
    let mut cluster_ids: Vec<&String> = up_clusters.keys().collect();
    cluster_ids.sort();
    for id in cluster_ids {
        let Some(a) = up_clusters.get(id) else {
            continue;
        };
        let Some(b) = lo_clusters.get(id) else {
            continue;
        };
        let dx = b.left - a.left;
        let dy = b.top - a.top;
        let dw = b.w - a.w;
        let dh = b.h - a.h;
        if dx.abs() < 1e-6 && dy.abs() < 1e-6 && dw.abs() < 1e-6 && dh.abs() < 1e-6 {
            continue;
        }
        println!(
            "{id} upstream=({:.6},{:.6},{:.6},{:.6}) local=({:.6},{:.6},{:.6},{:.6}) Δ=({:.6},{:.6},{:.6},{:.6})",
            a.left, a.top, a.w, a.h, b.left, b.top, b.w, b.h, dx, dy, dw, dh
        );
    }
    for (id, b) in &lo_clusters {
        if !up_clusters.contains_key(id) {
            println!(
                "{id} local-only ({:.6},{:.6},{:.6},{:.6})",
                b.left, b.top, b.w, b.h
            );
        }
    }
    for (id, a) in &up_clusters {
        if !lo_clusters.contains_key(id) {
            println!(
                "{id} upstream-only ({:.6},{:.6},{:.6},{:.6})",
                a.left, a.top, a.w, a.h
            );
        }
    }

    Ok(())
}

pub(crate) fn debug_flowchart_svg_diff(args: Vec<String>) -> Result<(), XtaskError> {
    let mut fixture: Option<String> = None;
    let mut upstream: Option<PathBuf> = None;
    let mut local: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut min_abs_delta: f64 = 0.5;
    let mut max_rows: usize = 50;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--fixture" => {
                i += 1;
                fixture = args.get(i).map(|s| s.to_string());
            }
            "--upstream" => {
                i += 1;
                upstream = args.get(i).map(PathBuf::from);
            }
            "--local" => {
                i += 1;
                local = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--min-abs-delta" => {
                i += 1;
                min_abs_delta = args
                    .get(i)
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(0.5);
            }
            "--max" => {
                i += 1;
                max_rows = args
                    .get(i)
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(50);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");

    if let Some(f) = fixture.as_deref() {
        let upstream_default = workspace_root
            .join("fixtures")
            .join("upstream-svgs")
            .join("flowchart")
            .join(format!("{f}.svg"));
        let local_default = workspace_root
            .join("target")
            .join("compare")
            .join("flowchart")
            .join(format!("{f}.svg"));
        upstream = upstream.or(Some(upstream_default));
        local = local.or(Some(local_default));
    }

    let Some(upstream_path) = upstream else {
        return Err(XtaskError::Usage);
    };
    let Some(local_path) = local else {
        return Err(XtaskError::Usage);
    };

    let upstream_svg =
        fs::read_to_string(&upstream_path).map_err(|source| XtaskError::ReadFile {
            path: upstream_path.display().to_string(),
            source,
        })?;
    let local_svg = fs::read_to_string(&local_path).map_err(|source| XtaskError::ReadFile {
        path: local_path.display().to_string(),
        source,
    })?;

    #[derive(Debug, Clone, Copy)]
    struct Translate {
        x: f64,
        y: f64,
    }

    fn parse_translate(transform: &str) -> Option<Translate> {
        let t = transform.trim();
        let t = t.strip_prefix("translate(")?;
        let t = t.strip_suffix(')')?;
        let parts = t
            .split(|ch: char| ch == ',' || ch.is_whitespace())
            .filter(|s| !s.trim().is_empty())
            .filter_map(|s| s.trim().parse::<f64>().ok())
            .collect::<Vec<_>>();
        match parts.as_slice() {
            [x, y] => Some(Translate { x: *x, y: *y }),
            [x] => Some(Translate { x: *x, y: 0.0 }),
            _ => None,
        }
    }

    fn accumulated_translate_including_self(node: roxmltree::Node<'_, '_>) -> Translate {
        let mut x = 0.0;
        let mut y = 0.0;
        for n in node.ancestors().filter(|n| n.is_element()) {
            if let Some(transform) = n.attribute("transform") {
                if let Some(t) = parse_translate(transform) {
                    x += t.x;
                    y += t.y;
                }
            }
        }
        Translate { x, y }
    }

    #[derive(Debug, Clone)]
    struct NodePos {
        kind: &'static str,
        x: f64,
        y: f64,
    }

    #[derive(Debug, Clone)]
    struct ClusterRect {
        left: f64,
        top: f64,
        w: f64,
        h: f64,
    }

    #[derive(Debug, Clone, Copy)]
    struct BBox {
        min_x: f64,
        min_y: f64,
        max_x: f64,
        max_y: f64,
    }

    impl BBox {
        fn width(&self) -> f64 {
            self.max_x - self.min_x
        }
        fn height(&self) -> f64 {
            self.max_y - self.min_y
        }
    }

    #[derive(Debug, Clone)]
    struct EdgePoints {
        tx: f64,
        ty: f64,
        points: Vec<(f64, f64)>,
        bbox: Option<BBox>,
        abs_bbox: Option<BBox>,
    }

    fn decode_data_points(dp: &str) -> Option<Vec<(f64, f64)>> {
        use base64::Engine as _;
        let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(dp.as_bytes()) else {
            return None;
        };
        let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
            return None;
        };
        let arr = v.as_array()?;
        let mut out: Vec<(f64, f64)> = Vec::with_capacity(arr.len());
        for p in arr {
            let (Some(x), Some(y)) = (
                p.get("x").and_then(|v| v.as_f64()),
                p.get("y").and_then(|v| v.as_f64()),
            ) else {
                continue;
            };
            if !(x.is_finite() && y.is_finite()) {
                continue;
            }
            out.push((x, y));
        }
        Some(out)
    }

    fn bbox_of_points(points: &[(f64, f64)]) -> Option<BBox> {
        if points.is_empty() {
            return None;
        }
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for (x, y) in points {
            min_x = min_x.min(*x);
            min_y = min_y.min(*y);
            max_x = max_x.max(*x);
            max_y = max_y.max(*y);
        }
        if min_x.is_finite() && min_y.is_finite() && max_x.is_finite() && max_y.is_finite() {
            Some(BBox {
                min_x,
                min_y,
                max_x,
                max_y,
            })
        } else {
            None
        }
    }

    fn parse_root_viewport(svg: &str) -> Result<(Option<String>, Option<String>), String> {
        let doc = roxmltree::Document::parse(svg).map_err(|e| e.to_string())?;
        let root = doc.root_element();
        let view_box = root.attribute("viewBox").map(|s| s.to_string());
        let max_width = root.attribute("style").and_then(|s| {
            static RE: OnceLock<Regex> = OnceLock::new();
            let re = RE.get_or_init(|| Regex::new(r#"max-width:\s*([0-9.]+)px"#).unwrap());
            re.captures(s)
                .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
        });
        Ok((view_box, max_width))
    }

    type PositionsAndEdges = (
        BTreeMap<String, NodePos>,
        BTreeMap<String, ClusterRect>,
        BTreeMap<String, EdgePoints>,
        Vec<String>,
    );

    fn parse_positions_and_edges(svg: &str) -> Result<PositionsAndEdges, String> {
        let doc = roxmltree::Document::parse(svg).map_err(|e| e.to_string())?;

        let mut nodes: BTreeMap<String, NodePos> = BTreeMap::new();
        let mut clusters: BTreeMap<String, ClusterRect> = BTreeMap::new();
        let mut edges: BTreeMap<String, EdgePoints> = BTreeMap::new();
        let mut root_transforms: Vec<String> = Vec::new();

        for n in doc.descendants().filter(|n| n.is_element()) {
            if n.tag_name().name() == "g" {
                if let Some(class) = n.attribute("class") {
                    if class.split_whitespace().any(|t| t == "root") {
                        if let Some(transform) = n.attribute("transform") {
                            if let Some(t) = transform
                                .trim()
                                .strip_prefix("translate(")
                                .and_then(|s| s.strip_suffix(')'))
                            {
                                root_transforms.push(t.trim().to_string());
                            }
                        }
                    }
                }
            }

            if n.tag_name().name() == "g" {
                let Some(id) = n.attribute("id") else {
                    continue;
                };
                let class = n.attribute("class").unwrap_or_default();
                let class_tokens = class.split_whitespace().collect::<Vec<_>>();

                if class_tokens.contains(&"node") {
                    let abs = accumulated_translate_including_self(n);
                    nodes.insert(
                        id.to_string(),
                        NodePos {
                            kind: "node",
                            x: abs.x,
                            y: abs.y,
                        },
                    );
                    continue;
                }

                // Mermaid self-loop helper nodes use `<g class="label edgeLabel" id="X---X---1" transform="translate(...)">`.
                if class_tokens.contains(&"edgeLabel") && class_tokens.contains(&"label") {
                    let abs = accumulated_translate_including_self(n);
                    nodes.insert(
                        id.to_string(),
                        NodePos {
                            kind: "labelRect",
                            x: abs.x,
                            y: abs.y,
                        },
                    );
                    continue;
                }

                if class_tokens.contains(&"cluster") {
                    let abs = accumulated_translate_including_self(n);
                    let rect = n
                        .children()
                        .find(|c| c.is_element() && c.tag_name().name() == "rect");
                    let Some(rect) = rect else {
                        continue;
                    };
                    let x = rect
                        .attribute("x")
                        .and_then(|v| v.parse::<f64>().ok())
                        .unwrap_or(0.0);
                    let y = rect
                        .attribute("y")
                        .and_then(|v| v.parse::<f64>().ok())
                        .unwrap_or(0.0);
                    let w = rect
                        .attribute("width")
                        .and_then(|v| v.parse::<f64>().ok())
                        .unwrap_or(0.0);
                    let h = rect
                        .attribute("height")
                        .and_then(|v| v.parse::<f64>().ok())
                        .unwrap_or(0.0);
                    clusters.insert(
                        id.to_string(),
                        ClusterRect {
                            left: abs.x + x,
                            top: abs.y + y,
                            w,
                            h,
                        },
                    );
                }
            }

            if n.tag_name().name() == "path" {
                if n.attribute("data-edge").is_none_or(|v| v != "true") {
                    continue;
                }
                let Some(edge_id) = n.attribute("data-id") else {
                    continue;
                };
                let Some(dp) = n.attribute("data-points") else {
                    continue;
                };
                let Some(points) = decode_data_points(dp) else {
                    continue;
                };
                let abs = accumulated_translate_including_self(n);
                let bbox = bbox_of_points(&points);
                let abs_bbox = bbox.map(|b| BBox {
                    min_x: b.min_x + abs.x,
                    max_x: b.max_x + abs.x,
                    min_y: b.min_y + abs.y,
                    max_y: b.max_y + abs.y,
                });
                edges.insert(
                    edge_id.to_string(),
                    EdgePoints {
                        tx: abs.x,
                        ty: abs.y,
                        points,
                        bbox,
                        abs_bbox,
                    },
                );
            }
        }

        root_transforms.sort();
        root_transforms.dedup();
        Ok((nodes, clusters, edges, root_transforms))
    }

    let (up_viewbox, up_maxw) =
        parse_root_viewport(&upstream_svg).map_err(XtaskError::DebugSvgFailed)?;
    let (lo_viewbox, lo_maxw) =
        parse_root_viewport(&local_svg).map_err(XtaskError::DebugSvgFailed)?;

    let (up_nodes, up_clusters, up_edges, up_roots) =
        parse_positions_and_edges(&upstream_svg).map_err(XtaskError::DebugSvgFailed)?;
    let (lo_nodes, lo_clusters, lo_edges, lo_roots) =
        parse_positions_and_edges(&local_svg).map_err(XtaskError::DebugSvgFailed)?;

    println!("upstream: {}", upstream_path.display());
    println!("local:    {}", local_path.display());
    println!();

    println!("== Root SVG ==");
    println!(
        "upstream viewBox: {:?}",
        up_viewbox.as_deref().unwrap_or("<missing>")
    );
    println!(
        "local    viewBox: {:?}",
        lo_viewbox.as_deref().unwrap_or("<missing>")
    );
    println!(
        "upstream max-width(px): {:?}",
        up_maxw.as_deref().unwrap_or("<missing>")
    );
    println!(
        "local    max-width(px): {:?}",
        lo_maxw.as_deref().unwrap_or("<missing>")
    );
    println!(
        "counts: nodes={} clusters={} edges={}",
        up_nodes.len().min(lo_nodes.len()),
        up_clusters.len().min(lo_clusters.len()),
        up_edges.len().min(lo_edges.len())
    );
    println!();

    println!("== Root group transforms ==");
    println!("upstream:");
    for t in &up_roots {
        println!("- {t}");
    }
    println!("local:");
    for t in &lo_roots {
        println!("- {t}");
    }
    println!();

    fn keep_id(id: &str, filter: &Option<String>) -> bool {
        filter.as_deref().map(|f| id.contains(f)).unwrap_or(true)
    }

    println!("== Nodes / LabelRects (abs translate) ==");
    let mut node_rows: Vec<(f64, String)> = Vec::new();
    for (id, up) in &up_nodes {
        if !keep_id(id, &filter) {
            continue;
        }
        let Some(lo) = lo_nodes.get(id) else {
            continue;
        };
        let dx = lo.x - up.x;
        let dy = lo.y - up.y;
        let score = (dx * dx + dy * dy).sqrt();
        if score >= min_abs_delta {
            node_rows.push((
                score,
                format!(
                    "{id} kind={} upstream=({:.3},{:.3}) local=({:.3},{:.3}) Δ=({:.3},{:.3})",
                    up.kind, up.x, up.y, lo.x, lo.y, dx, dy
                ),
            ));
        }
    }
    node_rows.sort_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .reverse()
    });
    for (_, line) in node_rows.into_iter().take(max_rows) {
        println!("{line}");
    }
    println!();

    println!("== Clusters (abs rect) ==");
    let mut cluster_rows: Vec<(f64, String)> = Vec::new();
    for (id, up) in &up_clusters {
        if !keep_id(id, &filter) {
            continue;
        }
        let Some(lo) = lo_clusters.get(id) else {
            continue;
        };
        let dl = lo.left - up.left;
        let dt = lo.top - up.top;
        let dw = lo.w - up.w;
        let dh = lo.h - up.h;
        let score = dl.abs().max(dt.abs()).max(dw.abs()).max(dh.abs());
        if score >= min_abs_delta {
            cluster_rows.push((
                score,
                format!(
                    "{id} upstream=({:.3},{:.3},{:.3},{:.3}) local=({:.3},{:.3},{:.3},{:.3}) Δ=({:.3},{:.3},{:.3},{:.3})",
                    up.left, up.top, up.w, up.h,
                    lo.left, lo.top, lo.w, lo.h,
                    dl, dt, dw, dh
                ),
            ));
        }
    }
    cluster_rows.sort_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .reverse()
    });
    for (_, line) in cluster_rows.into_iter().take(max_rows) {
        println!("{line}");
    }
    println!();

    println!("== Edges (data-points bbox/translate) ==");
    let mut edge_rows: Vec<(f64, String)> = Vec::new();
    for (id, up) in &up_edges {
        if !keep_id(id, &filter) {
            continue;
        }
        let Some(lo) = lo_edges.get(id) else {
            continue;
        };
        let dtx = lo.tx - up.tx;
        let dty = lo.ty - up.ty;
        let mut score = dtx.abs().max(dty.abs());

        let mut detail = String::new();
        if up.points.len() != lo.points.len() {
            detail.push_str(&format!(
                " points_len upstream={} local={}",
                up.points.len(),
                lo.points.len()
            ));
        }

        if let (Some(ub), Some(lb), Some(uab), Some(lab)) =
            (up.bbox, lo.bbox, up.abs_bbox, lo.abs_bbox)
        {
            let dw = lb.width() - ub.width();
            let dh = lb.height() - ub.height();
            let dminx = lab.min_x - uab.min_x;
            let dmaxx = lab.max_x - uab.max_x;
            let dminy = lab.min_y - uab.min_y;
            let dmaxy = lab.max_y - uab.max_y;
            score = score
                .max(dw.abs())
                .max(dh.abs())
                .max(dminx.abs())
                .max(dmaxx.abs())
                .max(dminy.abs())
                .max(dmaxy.abs());
            detail.push_str(&format!(
                " abs_bbox upstream=({:.3},{:.3},{:.3},{:.3}) local=({:.3},{:.3},{:.3},{:.3}) Δ=({:.3},{:.3},{:.3},{:.3}) sizeΔ=({:.3},{:.3})",
                uab.min_x, uab.min_y, uab.max_x, uab.max_y,
                lab.min_x, lab.min_y, lab.max_x, lab.max_y,
                dminx, dminy, dmaxx, dmaxy,
                dw, dh
            ));
        }

        if score < min_abs_delta {
            continue;
        }

        edge_rows.push((score, format!("{id} Δt=({:.3},{:.3}){detail}", dtx, dty)));
    }
    edge_rows.sort_by(|a, b| {
        a.0.partial_cmp(&b.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .reverse()
    });
    for (_, line) in edge_rows.into_iter().take(max_rows) {
        println!("{line}");
    }

    Ok(())
}

pub(crate) fn debug_flowchart_data_points(args: Vec<String>) -> Result<(), XtaskError> {
    let mut fixture: Option<String> = None;
    let mut upstream: Option<PathBuf> = None;
    let mut local: Option<PathBuf> = None;
    let mut edge_id: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--fixture" => {
                i += 1;
                fixture = args.get(i).map(|s| s.to_string());
            }
            "--upstream" => {
                i += 1;
                upstream = args.get(i).map(PathBuf::from);
            }
            "--local" => {
                i += 1;
                local = args.get(i).map(PathBuf::from);
            }
            "--edge" => {
                i += 1;
                edge_id = args.get(i).map(|s| s.to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let Some(edge_id) = edge_id.as_deref() else {
        return Err(XtaskError::Usage);
    };

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");

    if let Some(f) = fixture.as_deref() {
        let upstream_default = workspace_root
            .join("fixtures")
            .join("upstream-svgs")
            .join("flowchart")
            .join(format!("{f}.svg"));
        let local_default = workspace_root
            .join("target")
            .join("compare")
            .join("flowchart")
            .join(format!("{f}.svg"));
        upstream = upstream.or(Some(upstream_default));
        local = local.or(Some(local_default));
    }

    let Some(upstream_path) = upstream else {
        return Err(XtaskError::Usage);
    };
    let Some(local_path) = local else {
        return Err(XtaskError::Usage);
    };

    let upstream_svg =
        fs::read_to_string(&upstream_path).map_err(|source| XtaskError::ReadFile {
            path: upstream_path.display().to_string(),
            source,
        })?;
    let local_svg = fs::read_to_string(&local_path).map_err(|source| XtaskError::ReadFile {
        path: local_path.display().to_string(),
        source,
    })?;

    fn find_data_points(doc: &roxmltree::Document<'_>, edge_id: &str) -> Option<String> {
        for n in doc.descendants().filter(|n| n.is_element()) {
            if n.tag_name().name() != "path" {
                continue;
            }
            let Some(id) = n.attribute("data-id") else {
                continue;
            };
            if id != edge_id {
                continue;
            }
            let Some(dp) = n.attribute("data-points") else {
                continue;
            };
            return Some(dp.to_string());
        }
        None
    }

    fn decode_data_points_json(dp: &str) -> Option<serde_json::Value> {
        use base64::Engine as _;
        let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(dp.as_bytes()) else {
            return None;
        };
        serde_json::from_slice::<serde_json::Value>(&bytes).ok()
    }

    fn to_points(v: &serde_json::Value) -> Vec<(f64, f64)> {
        let Some(arr) = v.as_array() else {
            return Vec::new();
        };
        let mut out = Vec::with_capacity(arr.len());
        for p in arr {
            let (Some(x), Some(y)) = (
                p.get("x").and_then(|v| v.as_f64()),
                p.get("y").and_then(|v| v.as_f64()),
            ) else {
                continue;
            };
            if x.is_finite() && y.is_finite() {
                out.push((x, y));
            }
        }
        out
    }

    let upstream_doc = roxmltree::Document::parse(&upstream_svg)
        .map_err(|e| XtaskError::DebugSvgFailed(e.to_string()))?;
    let local_doc = roxmltree::Document::parse(&local_svg)
        .map_err(|e| XtaskError::DebugSvgFailed(e.to_string()))?;

    let Some(up_dp) = find_data_points(&upstream_doc, edge_id) else {
        return Err(XtaskError::DebugSvgFailed(format!(
            "missing data-points for edge {edge_id:?} in {}",
            upstream_path.display()
        )));
    };
    let Some(lo_dp) = find_data_points(&local_doc, edge_id) else {
        return Err(XtaskError::DebugSvgFailed(format!(
            "missing data-points for edge {edge_id:?} in {}",
            local_path.display()
        )));
    };

    let up_json = decode_data_points_json(&up_dp).ok_or_else(|| {
        XtaskError::DebugSvgFailed("failed to decode upstream data-points".into())
    })?;
    let lo_json = decode_data_points_json(&lo_dp)
        .ok_or_else(|| XtaskError::DebugSvgFailed("failed to decode local data-points".into()))?;

    println!("upstream: {}", upstream_path.display());
    println!("local:    {}", local_path.display());
    println!("edge:     {edge_id}");
    println!();

    println!("== Upstream decoded JSON ==");
    println!(
        "{}",
        serde_json::to_string_pretty(&up_json).unwrap_or_else(|_| "<unprintable>".to_string())
    );
    println!();

    println!("== Local decoded JSON ==");
    println!(
        "{}",
        serde_json::to_string_pretty(&lo_json).unwrap_or_else(|_| "<unprintable>".to_string())
    );
    println!();

    let up_pts = to_points(&up_json);
    let lo_pts = to_points(&lo_json);
    if up_pts.is_empty() || lo_pts.is_empty() {
        return Ok(());
    }

    println!("== Point deltas (upstream -> local) ==");
    let n = up_pts.len().min(lo_pts.len());
    let mut max_abs = 0.0f64;
    for idx in 0..n {
        let (ux, uy) = up_pts[idx];
        let (lx, ly) = lo_pts[idx];
        let dx = lx - ux;
        let dy = ly - uy;
        max_abs = max_abs.max(dx.abs()).max(dy.abs());
        println!(
            "#{idx}: upstream=({ux:.17},{uy:.17}) local=({lx:.17},{ly:.17}) Δ=({dx:.17},{dy:.17})"
        );
    }
    if up_pts.len() != lo_pts.len() {
        println!(
            "length mismatch: upstream={} local={}",
            up_pts.len(),
            lo_pts.len()
        );
    }
    println!("max |Δ| = {max_abs:.17}");

    Ok(())
}

pub(crate) fn debug_flowchart_edge_trace(args: Vec<String>) -> Result<(), XtaskError> {
    let mut fixture: Option<String> = None;
    let mut edge_id: Option<String> = None;
    let mut out: Option<PathBuf> = None;
    let mut upstream: Option<PathBuf> = None;
    let mut local: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--fixture" => {
                i += 1;
                fixture = args.get(i).map(|s| s.to_string());
            }
            "--edge" => {
                i += 1;
                edge_id = args.get(i).map(|s| s.to_string());
            }
            "--out" => {
                i += 1;
                out = args.get(i).map(PathBuf::from);
            }
            "--upstream" => {
                i += 1;
                upstream = args.get(i).map(PathBuf::from);
            }
            "--local" => {
                i += 1;
                local = args.get(i).map(PathBuf::from);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let Some(edge_id) = edge_id.as_deref() else {
        return Err(XtaskError::Usage);
    };

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");

    let fixture_name = fixture
        .as_deref()
        .unwrap_or("upstream_flowchart_v2_self_loops_spec");
    let mmd_path = workspace_root
        .join("fixtures")
        .join("flowchart")
        .join(format!("{fixture_name}.mmd"));

    let text = fs::read_to_string(&mmd_path).map_err(|source| XtaskError::ReadFile {
        path: mmd_path.display().to_string(),
        source,
    })?;

    // Match compare-svg-xml defaults (handDrawnSeed ensures deterministic output).
    // Keep layout snapshots consistent with the in-repo `layout_snapshots_test` harness, which
    // uses the default engine configuration.
    let engine = merman::Engine::new();
    let measurer: std::sync::Arc<dyn merman_render::text::TextMeasurer + Send + Sync> =
        std::sync::Arc::new(merman_render::text::VendoredFontMetricsTextMeasurer::default());
    let layout_opts = merman_render::LayoutOptions {
        text_measurer: std::sync::Arc::clone(&measurer),
        ..Default::default()
    };

    let parsed =
        futures::executor::block_on(engine.parse_diagram(&text, merman::ParseOptions::default()))
            .map_err(|e| XtaskError::DebugSvgFailed(format!("parse failed: {e}")))?
            .ok_or_else(|| XtaskError::DebugSvgFailed("no diagram detected".to_string()))?;

    let layouted = merman_render::layout_parsed(&parsed, &layout_opts)
        .map_err(|e| XtaskError::DebugSvgFailed(format!("layout failed: {e}")))?;

    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = &layouted.layout else {
        return Err(XtaskError::DebugSvgFailed(format!(
            "expected flowchart-v2 layout, got {}",
            layouted.meta.diagram_type
        )));
    };

    let out = out.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("trace")
            .join("flowchart")
            .join(fixture_name)
            .join(format!("{edge_id}.json"))
    });
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }

    // Rust 1.85+ marks environment mutation as `unsafe` due to potential UB when other
    // threads concurrently read/modify the process environment. `xtask` sets these vars
    // up-front before invoking rendering code, so this is safe in our usage.
    unsafe {
        std::env::set_var("MERMAN_TRACE_FLOWCHART_EDGE", edge_id);
        std::env::set_var("MERMAN_TRACE_FLOWCHART_OUT", &out);
    }

    let svg_opts = merman_render::svg::SvgRenderOptions {
        diagram_id: Some(fixture_name.to_string()),
        ..Default::default()
    };

    // Render once to trigger the trace emission inside `merman-render`.
    let svg = merman_render::svg::render_flowchart_v2_svg(
        layout,
        &layouted.semantic,
        &layouted.meta.effective_config,
        layouted.meta.title.as_deref(),
        layout_opts.text_measurer.as_ref(),
        &svg_opts,
    )
    .map_err(|e| XtaskError::DebugSvgFailed(format!("render failed: {e}")))?;

    if let Ok(doc) = roxmltree::Document::parse(&svg) {
        if let Some(dp) = find_data_points(&doc, edge_id) {
            if let Some(json) = decode_data_points_json(&dp) {
                println!("== Rendered SVG data-points (decoded) ==");
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json)
                        .unwrap_or_else(|_| "<unprintable>".to_string())
                );
                println!();
            }
        }
    }

    let trace_json = fs::read_to_string(&out).map_err(|source| XtaskError::ReadFile {
        path: out.display().to_string(),
        source,
    })?;

    println!("trace:   {}", out.display());
    println!("fixture: {fixture_name}");
    println!("edge:    {edge_id}");
    println!();
    println!("== Local edge trace (JSON) ==");
    println!("{trace_json}");

    // Optional: also print upstream/local decoded `data-points` from the XML compare output if available.
    if upstream.is_none() && local.is_none() {
        let upstream_default = workspace_root
            .join("target")
            .join("compare")
            .join("xml")
            .join("flowchart")
            .join(format!("{fixture_name}.upstream.xml"));
        let local_default = workspace_root
            .join("target")
            .join("compare")
            .join("xml")
            .join("flowchart")
            .join(format!("{fixture_name}.local.xml"));
        upstream = Some(upstream_default);
        local = Some(local_default);
    }

    let (Some(upstream_path), Some(local_path)) = (upstream, local) else {
        return Ok(());
    };
    let upstream_svg =
        fs::read_to_string(&upstream_path).map_err(|source| XtaskError::ReadFile {
            path: upstream_path.display().to_string(),
            source,
        })?;
    let local_svg = fs::read_to_string(&local_path).map_err(|source| XtaskError::ReadFile {
        path: local_path.display().to_string(),
        source,
    })?;

    fn find_data_points(doc: &roxmltree::Document<'_>, edge_id: &str) -> Option<String> {
        for n in doc.descendants().filter(|n| n.is_element()) {
            if n.tag_name().name() != "path" {
                continue;
            }
            let Some(id) = n.attribute("data-id") else {
                continue;
            };
            if id != edge_id {
                continue;
            }
            let Some(dp) = n.attribute("data-points") else {
                continue;
            };
            return Some(dp.to_string());
        }
        None
    }

    fn decode_data_points_json(dp: &str) -> Option<serde_json::Value> {
        use base64::Engine as _;
        let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(dp.as_bytes()) else {
            return None;
        };
        serde_json::from_slice::<serde_json::Value>(&bytes).ok()
    }

    let upstream_doc = roxmltree::Document::parse(&upstream_svg)
        .map_err(|e| XtaskError::DebugSvgFailed(e.to_string()))?;
    let local_doc = roxmltree::Document::parse(&local_svg)
        .map_err(|e| XtaskError::DebugSvgFailed(e.to_string()))?;
    let Some(up_dp) = find_data_points(&upstream_doc, edge_id) else {
        println!();
        println!(
            "(no upstream data-points found for edge {edge_id} in {})",
            upstream_path.display()
        );
        return Ok(());
    };
    let Some(lo_dp) = find_data_points(&local_doc, edge_id) else {
        println!();
        println!(
            "(no local data-points found for edge {edge_id} in {})",
            local_path.display()
        );
        return Ok(());
    };

    let up_json = decode_data_points_json(&up_dp).ok_or_else(|| {
        XtaskError::DebugSvgFailed("failed to decode upstream data-points".into())
    })?;
    let lo_json = decode_data_points_json(&lo_dp)
        .ok_or_else(|| XtaskError::DebugSvgFailed("failed to decode local data-points".into()))?;

    println!();
    println!("== XML data-points (decoded) ==");
    println!("upstream: {}", upstream_path.display());
    println!("local:    {}", local_path.display());
    println!();
    println!("-- Upstream --");
    println!(
        "{}",
        serde_json::to_string_pretty(&up_json).unwrap_or_else(|_| "<unprintable>".to_string())
    );
    println!();
    println!("-- Local --");
    println!(
        "{}",
        serde_json::to_string_pretty(&lo_json).unwrap_or_else(|_| "<unprintable>".to_string())
    );

    Ok(())
}

pub(crate) fn debug_flowchart_layout(args: Vec<String>) -> Result<(), XtaskError> {
    let mut fixture: Option<PathBuf> = None;
    let mut edge_id: Option<String> = None;
    let mut text_measurer: String = "deterministic".to_string();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--fixture" => {
                i += 1;
                fixture = args.get(i).map(PathBuf::from);
            }
            "--edge" => {
                i += 1;
                edge_id = args.get(i).map(|s| s.to_string());
            }
            "--text-measurer" => {
                i += 1;
                text_measurer = args
                    .get(i)
                    .map(|s| s.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "deterministic".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let Some(fixture_path) = fixture else {
        return Err(XtaskError::Usage);
    };
    let text = std::fs::read_to_string(&fixture_path).map_err(|source| XtaskError::ReadFile {
        path: fixture_path.display().to_string(),
        source,
    })?;

    let engine = merman::Engine::new()
        .with_site_config(merman::MermaidConfig::from_value(
            serde_json::json!({ "handDrawnSeed": 1 }),
        ))
        .with_fixed_today(Some(
            chrono::NaiveDate::from_ymd_opt(2026, 2, 15).expect("valid date"),
        ));
    let parsed =
        futures::executor::block_on(engine.parse_diagram(&text, merman::ParseOptions::default()))
            .map_err(|e| XtaskError::DebugSvgFailed(e.to_string()))?
            .ok_or_else(|| {
                XtaskError::DebugSvgFailed(format!(
                    "no diagram detected in {}",
                    fixture_path.display()
                ))
            })?;

    let mut layout_opts = merman_render::LayoutOptions::default();
    if matches!(
        text_measurer.as_str(),
        "vendored" | "vendored-font" | "vendored-font-metrics"
    ) {
        layout_opts.text_measurer =
            std::sync::Arc::new(merman_render::text::VendoredFontMetricsTextMeasurer::default());
    }
    let layouted = merman_render::layout_parsed(&parsed, &layout_opts)
        .map_err(|e| XtaskError::DebugSvgFailed(e.to_string()))?;

    let merman_render::model::LayoutDiagram::FlowchartV2(layout) = &layouted.layout else {
        return Err(XtaskError::DebugSvgFailed(format!(
            "unexpected layout type: {}",
            layouted.meta.diagram_type
        )));
    };

    println!("fixture: {}", fixture_path.display());
    if let Some(title) = layouted.meta.title.as_deref() {
        println!("title: {}", title);
    }
    println!("diagram_type: {}", layouted.meta.diagram_type);
    println!("text_measurer: {}", text_measurer);
    println!();

    // Mirror `compute_layout_bounds` (private to `merman-render`) for debugging.
    #[derive(Debug, Clone, Copy)]
    struct Bounds {
        min_x: f64,
        min_y: f64,
        max_x: f64,
        max_y: f64,
    }

    fn compute_bounds(
        clusters: &[merman_render::model::LayoutCluster],
        nodes: &[merman_render::model::LayoutNode],
        edges: &[merman_render::model::LayoutEdge],
    ) -> Option<Bounds> {
        let mut b: Option<Bounds> = None;
        let mut include_rect = |min_x: f64, min_y: f64, max_x: f64, max_y: f64| {
            if let Some(ref mut cur) = b {
                cur.min_x = cur.min_x.min(min_x);
                cur.min_y = cur.min_y.min(min_y);
                cur.max_x = cur.max_x.max(max_x);
                cur.max_y = cur.max_y.max(max_y);
            } else {
                b = Some(Bounds {
                    min_x,
                    min_y,
                    max_x,
                    max_y,
                });
            }
        };

        for c in clusters {
            let hw = c.width / 2.0;
            let hh = c.height / 2.0;
            include_rect(c.x - hw, c.y - hh, c.x + hw, c.y + hh);
            let lhw = c.title_label.width / 2.0;
            let lhh = c.title_label.height / 2.0;
            include_rect(
                c.title_label.x - lhw,
                c.title_label.y - lhh,
                c.title_label.x + lhw,
                c.title_label.y + lhh,
            );
        }

        for n in nodes {
            let hw = n.width / 2.0;
            let hh = n.height / 2.0;
            include_rect(n.x - hw, n.y - hh, n.x + hw, n.y + hh);
        }

        for e in edges {
            for p in &e.points {
                include_rect(p.x, p.y, p.x, p.y);
            }
            for lbl in [
                e.label.as_ref(),
                e.start_label_left.as_ref(),
                e.start_label_right.as_ref(),
                e.end_label_left.as_ref(),
                e.end_label_right.as_ref(),
            ]
            .into_iter()
            .flatten()
            {
                let hw = lbl.width / 2.0;
                let hh = lbl.height / 2.0;
                include_rect(lbl.x - hw, lbl.y - hh, lbl.x + hw, lbl.y + hh);
            }
        }

        b
    }

    if let Some(b) = compute_bounds(&layout.clusters, &layout.nodes, &layout.edges) {
        println!(
            "bounds: min=({}, {}) max=({}, {}) size=({}, {})",
            b.min_x,
            b.min_y,
            b.max_x,
            b.max_y,
            b.max_x - b.min_x,
            b.max_y - b.min_y
        );
        println!();
    }

    println!("clusters: {}", layout.clusters.len());
    for c in &layout.clusters {
        println!(
            "- {} x={} y={} w={} h={} dir={}",
            c.id, c.x, c.y, c.width, c.height, c.effective_dir
        );
    }
    println!();

    println!("nodes: {}", layout.nodes.len());
    for n in &layout.nodes {
        println!(
            "- {} x={} y={} w={} h={}",
            n.id, n.x, n.y, n.width, n.height
        );
    }
    println!();

    println!("edges: {}", layout.edges.len());
    for e in &layout.edges {
        if edge_id.as_ref().is_some_and(|id| id != &e.id) {
            continue;
        }
        println!(
            "- {} {} -> {} from_cluster={:?} to_cluster={:?} points={}",
            e.id,
            e.from,
            e.to,
            e.from_cluster,
            e.to_cluster,
            e.points.len()
        );
        if let Some(lbl) = e.label.as_ref() {
            println!(
                "  label: x={} y={} w={} h={}",
                lbl.x, lbl.y, lbl.width, lbl.height
            );
        }
        for (idx, p) in e.points.iter().enumerate() {
            if idx >= 16 {
                println!("  ...");
                break;
            }
            println!("  - p{idx}: x={} y={}", p.x, p.y);
        }
    }

    Ok(())
}
