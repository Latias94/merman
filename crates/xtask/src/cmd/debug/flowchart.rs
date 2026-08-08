//! Flowchart debug utilities.

use crate::XtaskError;
use merman_core::ParsedDiagramRender;
use merman_core::RenderSemanticModel;
use merman_core::diagrams::flowchart::FlowchartModel;
use merman_render::environment::RenderSession;
use merman_render::model::FlowchartLayout;
use regex::Regex;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

fn flowchart_model(parsed: &ParsedDiagramRender) -> Result<&FlowchartModel, XtaskError> {
    let RenderSemanticModel::Flowchart(model) = parsed.model() else {
        return Err(XtaskError::DebugSvgFailed(format!(
            "expected Flowchart render model, got {}",
            parsed.model().kind()
        )));
    };
    Ok(model)
}

fn layout_flowchart_render_model(
    parsed: &ParsedDiagramRender,
    session: RenderSession,
) -> Result<FlowchartLayout, XtaskError> {
    let artifact = merman_render::family::prepare(
        parsed.clone(),
        &merman_render::LayoutOptions::headless_svg_defaults(),
        session,
    )
    .map_err(|error| XtaskError::DebugSvgFailed(error.to_string()))?;
    let projection = artifact
        .layout_json()
        .map_err(|error| XtaskError::DebugSvgFailed(error.to_string()))?;
    let layout = projection
        .get("layout")
        .and_then(|layout| layout.get("FlowchartV2"))
        .cloned()
        .ok_or_else(|| {
            XtaskError::DebugSvgFailed(
                "prepared Flowchart artifact did not expose a FlowchartV2 layout projection"
                    .to_string(),
            )
        })?;
    serde_json::from_value(layout).map_err(|error| {
        XtaskError::DebugSvgFailed(format!(
            "failed to decode prepared Flowchart layout projection: {error}"
        ))
    })
}

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

    let workspace_root = crate::cmd::workspace_root();

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
            if n.tag_name().name() == "g"
                && let Some(class) = n.attribute("class")
            {
                if class.split_whitespace().any(|t| t == "root")
                    && let Some(transform) = n.attribute("transform")
                    && let Some(t) = parse_translate(transform)
                {
                    root_transforms.push(t);
                }
                if class.split_whitespace().any(|t| t == "cluster")
                    && let Some(id) = n.attribute("id")
                {
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

    let workspace_root = crate::cmd::workspace_root();

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
            if let Some(transform) = n.attribute("transform")
                && let Some(t) = parse_translate(transform)
            {
                x += t.x;
                y += t.y;
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

    let workspace_root = crate::cmd::workspace_root();

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
            if let Some(transform) = n.attribute("transform")
                && let Some(t) = parse_translate(transform)
            {
                x += t.x;
                y += t.y;
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
            if n.tag_name().name() == "g"
                && let Some(class) = n.attribute("class")
                && class.split_whitespace().any(|t| t == "root")
                && let Some(transform) = n.attribute("transform")
                && let Some(t) = transform
                    .trim()
                    .strip_prefix("translate(")
                    .and_then(|s| s.strip_suffix(')'))
            {
                root_transforms.push(t.trim().to_string());
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

    let workspace_root = crate::cmd::workspace_root();

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

    let workspace_root = crate::cmd::workspace_root();

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
    let layout_opts = merman_render::LayoutOptions::default();
    let session = merman::svg::RenderEnvironment::deterministic()
        .begin_session()
        .map_err(|e| XtaskError::DebugSvgFailed(e.to_string()))?;

    let parsed = futures::executor::block_on(
        engine.parse_diagram_for_render_model(&text, merman::ParseOptions::default()),
    )
    .map_err(|e| XtaskError::DebugSvgFailed(format!("parse failed: {e}")))?
    .ok_or_else(|| XtaskError::DebugSvgFailed("no diagram detected".to_string()))?;
    flowchart_model(&parsed)?;

    let artifact = merman_render::family::prepare(parsed, &layout_opts, session)
        .map_err(|e| XtaskError::DebugSvgFailed(format!("layout failed: {e}")))?;

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

    let svg_opts = merman_render::svg::SvgRenderOptions {
        diagram_id: Some(fixture_name.to_string()),
        ..Default::default()
    };
    let trace_collector = merman_render::svg::FlowchartEdgeTraceCollector::default();
    let debug = merman_render::svg::SvgDebugOptions::default()
        .with_flowchart_edge_trace(edge_id.to_string(), trace_collector.clone());

    // Render once to collect the trace in caller-owned memory. This command owns the checked
    // serialization and filesystem boundary below.
    let rendered = artifact
        .render_svg(&svg_opts, &debug)
        .map_err(|e| XtaskError::DebugSvgFailed(format!("render failed: {e}")))?;
    let svg = rendered.svg();

    if let Ok(doc) = roxmltree::Document::parse(svg)
        && let Some(dp) = find_data_points(&doc, edge_id)
        && let Some(json) = decode_data_points_json(&dp)
    {
        println!("== Rendered SVG data-points (decoded) ==");
        println!(
            "{}",
            serde_json::to_string_pretty(&json).unwrap_or_else(|_| "<unprintable>".to_string())
        );
        println!();
    }

    let trace = trace_collector
        .drain()
        .into_iter()
        .find(|trace| trace.edge_id == edge_id)
        .ok_or_else(|| {
            XtaskError::DebugSvgFailed(format!("render did not collect trace for edge {edge_id:?}"))
        })?;
    let trace_json = serde_json::to_string_pretty(&trace)?;
    fs::write(&out, &trace_json).map_err(|source| XtaskError::WriteFile {
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
            merman_core::time::CivilDate::new(2026, 2, 15).expect("valid date"),
        ));
    let parsed = futures::executor::block_on(
        engine.parse_diagram_for_render_model(&text, merman::ParseOptions::default()),
    )
    .map_err(|e| XtaskError::DebugSvgFailed(e.to_string()))?
    .ok_or_else(|| {
        XtaskError::DebugSvgFailed(format!("no diagram detected in {}", fixture_path.display()))
    })?;

    let measurement_policy = if matches!(
        text_measurer.as_str(),
        "vendored" | "vendored-font" | "vendored-font-metrics"
    ) {
        merman::svg::TextMeasurementPolicy::parity()
    } else {
        merman::svg::TextMeasurementPolicy::deterministic()
    };
    let session = merman::svg::RenderEnvironment::deterministic()
        .with_text_measurement_policy(measurement_policy)
        .begin_session()
        .map_err(|e| XtaskError::DebugSvgFailed(e.to_string()))?;
    let layout = layout_flowchart_render_model(&parsed, session)?;

    println!("fixture: {}", fixture_path.display());
    if let Some(title) = parsed.metadata().title.as_deref() {
        println!("title: {}", title);
    }
    println!("diagram_type: {}", parsed.metadata().diagram_type);
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

pub(crate) fn debug_flowchart_elk_source_phase(args: Vec<String>) -> Result<(), XtaskError> {
    let mut fixture: Option<PathBuf> = None;
    let mut phase = Some(merman_layout_elk::LayeredPhase::P3NodeOrdering);
    let mut processor: Option<merman_layout_elk::ProcessorKind> = None;
    let mut p3_trace = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--fixture" => {
                i += 1;
                fixture = args.get(i).map(PathBuf::from);
            }
            "--phase" => {
                i += 1;
                phase = match args.get(i).map(|s| s.trim().to_ascii_lowercase()) {
                    Some(value) if value == "full" => None,
                    Some(value) if matches!(value.as_str(), "p1" | "p1-cycle" | "cycle") => {
                        Some(merman_layout_elk::LayeredPhase::P1CycleBreaking)
                    }
                    Some(value) if matches!(value.as_str(), "p2" | "p2-layer" | "layer") => {
                        Some(merman_layout_elk::LayeredPhase::P2Layering)
                    }
                    Some(value) if matches!(value.as_str(), "p3" | "p3-order" | "order") => {
                        Some(merman_layout_elk::LayeredPhase::P3NodeOrdering)
                    }
                    Some(value) if matches!(value.as_str(), "p4" | "p4-place" | "place") => {
                        Some(merman_layout_elk::LayeredPhase::P4NodePlacement)
                    }
                    Some(value) if matches!(value.as_str(), "p5" | "p5-route" | "route") => {
                        Some(merman_layout_elk::LayeredPhase::P5EdgeRouting)
                    }
                    _ => return Err(XtaskError::Usage),
                };
            }
            "--processor" => {
                i += 1;
                let Some(value) = args.get(i) else {
                    return Err(XtaskError::Usage);
                };
                processor = Some(parse_source_processor_kind(value)?);
            }
            "--p3-trace" => {
                p3_trace = true;
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
            merman_core::time::CivilDate::new(2026, 2, 15).expect("valid date"),
        ));
    let parsed = futures::executor::block_on(
        engine.parse_diagram_for_render_model(&text, merman::ParseOptions::default()),
    )
    .map_err(|e| XtaskError::DebugSvgFailed(e.to_string()))?
    .ok_or_else(|| {
        XtaskError::DebugSvgFailed(format!("no diagram detected in {}", fixture_path.display()))
    })?;
    let measurer = merman_render::text::VendoredFontMetricsTextMeasurer::default();
    let elk_graph =
        merman_render::flowchart::elk::build_flowchart_elk_graph(&parsed, &measurer, None)
            .map_err(|e| XtaskError::DebugSvgFailed(e.to_string()))?;
    let mut source_diagnostics = merman_layout_elk::SourcePhaseDiagnostics::from_graph(&elk_graph)
        .map_err(|e| XtaskError::DebugSvgFailed(e.to_string()))?;

    let has_parent_nodes = elk_graph.nodes.iter().any(|node| node.parent.is_some());
    let mut trace = None;
    let executed = if p3_trace {
        if !has_parent_nodes {
            return Err(XtaskError::DebugSvgFailed(
                "--p3-trace currently expects a compound flowchart fixture".to_string(),
            ));
        }
        let (executions, crossing_trace) = source_diagnostics
            .inspect_compound_crossings_after_processor(
                merman_layout_elk::ProcessorKind::SortByInputModelProcessor,
            )
            .map_err(|e| XtaskError::DebugSvgFailed(e.to_string()))?;
        trace = crossing_trace;
        executions
            .into_iter()
            .map(|execution| {
                format!(
                    "{}({:?})={:?}",
                    execution.graph_id, execution.parent_node_id, execution.processors
                )
            })
            .collect::<Vec<_>>()
    } else if has_parent_nodes {
        let executions = if let Some(processor) = processor {
            source_diagnostics
                .execute_compound_until_processor(processor)
                .map_err(|e| XtaskError::DebugSvgFailed(e.to_string()))?
        } else if let Some(phase) = phase {
            source_diagnostics
                .execute_compound_until(phase)
                .map_err(|e| XtaskError::DebugSvgFailed(e.to_string()))?
        } else {
            source_diagnostics
                .execute_compound_all()
                .map_err(|e| XtaskError::DebugSvgFailed(e.to_string()))?
        };
        executions
            .into_iter()
            .map(|execution| {
                format!(
                    "{}({:?})={:?}",
                    execution.graph_id, execution.parent_node_id, execution.processors
                )
            })
            .collect::<Vec<_>>()
    } else {
        let processors = if let Some(processor) = processor {
            source_diagnostics
                .execute_until_processor(processor)
                .map_err(|e| XtaskError::DebugSvgFailed(e.to_string()))?
        } else if let Some(phase) = phase {
            source_diagnostics
                .execute_until(phase)
                .map_err(|e| XtaskError::DebugSvgFailed(e.to_string()))?
        } else {
            source_diagnostics
                .execute_all()
                .map_err(|e| XtaskError::DebugSvgFailed(e.to_string()))?
        };
        vec![format!("{}={processors:?}", elk_graph.id)]
    };

    println!("fixture: {}", fixture_path.display());
    println!("diagram_type: {}", parsed.metadata().diagram_type);
    println!(
        "phase: {:?}",
        phase.unwrap_or(merman_layout_elk::LayeredPhase::P5EdgeRouting)
    );
    if let Some(processor) = processor {
        println!("processor_stop: {processor:?}");
    }
    if p3_trace {
        println!("p3_trace: true");
    }
    println!("executed:");
    for item in executed {
        println!("- {item}");
    }
    println!();
    if let Some(trace) = trace {
        dump_hierarchy_sweep_debug_trace(&trace);
    }

    print!("{}", source_diagnostics.graph_dump());

    Ok(())
}

fn parse_source_processor_kind(
    value: &str,
) -> Result<merman_layout_elk::ProcessorKind, XtaskError> {
    let normalized = value
        .trim()
        .chars()
        .filter(|ch| *ch != '-' && *ch != '_' && !ch.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect::<String>();

    let processor = match normalized.as_str() {
        "directionpreprocessor" => merman_layout_elk::ProcessorKind::DirectionPreprocessor,
        "edgeandlayerconstraintedgereverser" => {
            merman_layout_elk::ProcessorKind::EdgeAndLayerConstraintEdgeReverser
        }
        "greedycyclebreaker" => merman_layout_elk::ProcessorKind::GreedyCycleBreaker,
        "layerconstraintpreprocessor" => {
            merman_layout_elk::ProcessorKind::LayerConstraintPreprocessor
        }
        "networksimplexlayerer" => merman_layout_elk::ProcessorKind::NetworkSimplexLayerer,
        "layerconstraintpostprocessor" => {
            merman_layout_elk::ProcessorKind::LayerConstraintPostprocessor
        }
        "hierarchicalportconstraintprocessor" => {
            merman_layout_elk::ProcessorKind::HierarchicalPortConstraintProcessor
        }
        "longedgesplitter" => merman_layout_elk::ProcessorKind::LongEdgeSplitter,
        "portsideprocessor" => merman_layout_elk::ProcessorKind::PortSideProcessor,
        "invertedportprocessor" => merman_layout_elk::ProcessorKind::InvertedPortProcessor,
        "portlistsorter" => merman_layout_elk::ProcessorKind::PortListSorter,
        "sortbyinputmodelprocessor" | "sortbyinputmodel" => {
            merman_layout_elk::ProcessorKind::SortByInputModelProcessor
        }
        "layersweepcrossingminimizerbarycenter" | "barycenter" => {
            merman_layout_elk::ProcessorKind::LayerSweepCrossingMinimizerBarycenter
        }
        "inlayerconstraintprocessor" => {
            merman_layout_elk::ProcessorKind::InLayerConstraintProcessor
        }
        "labelandnodesizeprocessor" => merman_layout_elk::ProcessorKind::LabelAndNodeSizeProcessor,
        "innermostnodemargincalculator" => {
            merman_layout_elk::ProcessorKind::InnermostNodeMarginCalculator
        }
        "bknodeplacer" => merman_layout_elk::ProcessorKind::BKNodePlacer,
        "layersizeandgraphheightcalculator" => {
            merman_layout_elk::ProcessorKind::LayerSizeAndGraphHeightCalculator
        }
        "orthogonaledgerouter" => merman_layout_elk::ProcessorKind::OrthogonalEdgeRouter,
        "hierarchicalportdummysizeprocessor" => {
            merman_layout_elk::ProcessorKind::HierarchicalPortDummySizeProcessor
        }
        "hierarchicalportpositionprocessor" => {
            merman_layout_elk::ProcessorKind::HierarchicalPortPositionProcessor
        }
        "hierarchicalportorthogonaledgerouter" => {
            merman_layout_elk::ProcessorKind::HierarchicalPortOrthogonalEdgeRouter
        }
        "longedgejoiner" => merman_layout_elk::ProcessorKind::LongEdgeJoiner,
        "endlabelsorter" => merman_layout_elk::ProcessorKind::EndLabelSorter,
        "reversededgerestorer" => merman_layout_elk::ProcessorKind::ReversedEdgeRestorer,
        "hierarchicalnoderesizer" => merman_layout_elk::ProcessorKind::HierarchicalNodeResizer,
        "directionpostprocessor" => merman_layout_elk::ProcessorKind::DirectionPostprocessor,
        _ => return Err(XtaskError::Usage),
    };

    Ok(processor)
}

fn dump_hierarchy_sweep_debug_trace(trace: &merman_layout_elk::HierarchySweepDebugTrace) {
    println!("p3_trace_graphs:");
    for graph in &trace.graphs {
        let child_paths = graph
            .child_paths
            .iter()
            .map(|path| {
                path.iter()
                    .map(usize::to_string)
                    .collect::<Vec<_>>()
                    .join("/")
            })
            .collect::<Vec<_>>();
        println!(
            "- graph={} parent={:?} path={} children=[{}] distributor={} use_bottom_up={} paths_random={} paths_hierarchical={} normalized={}",
            graph.graph_id,
            graph.parent_node_id,
            graph
                .path
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join("/"),
            child_paths.join(","),
            graph.port_distributor,
            graph.use_bottom_up,
            graph.layer_sweep_paths_to_random,
            graph.layer_sweep_paths_to_hierarchical,
            graph.layer_sweep_normalized
        );
    }
    println!("p3_trace_runs:");
    for run in &trace.runs {
        println!(
            "- graph={} run={} first_initial={} second_initial={} initial_crossings={} early_returned={} crossings={} best_before={} improved={}",
            run.graph_id,
            run.run_index,
            run.first_try_with_initial_order,
            run.second_try_with_initial_order,
            run.initial_crossings,
            run.early_returned,
            run.crossings,
            run.best_crossings_before,
            run.improved_best
        );
    }
    println!("p3_trace_layer_sweeps:");
    for layer in &trace.layer_sweeps {
        println!(
            "- graph={} layer={} forward={} first_sweep={} first_sweep_for_heuristic={} pre_ordered={}",
            layer.graph_id,
            layer.layer_index,
            layer.forward,
            layer.is_first_sweep,
            layer.first_sweep_for_heuristic,
            layer.pre_ordered
        );
        println!("  before={}", format_hierarchy_sweep_nodes(&layer.before));
        println!("  after={}", format_hierarchy_sweep_nodes(&layer.after));
    }
    println!();
}

fn format_hierarchy_sweep_nodes(nodes: &[merman_layout_elk::HierarchySweepNodeDebug]) -> String {
    nodes
        .iter()
        .map(|node| {
            format!(
                "{}#{}(b={:?},sum={:.6},deg={})",
                node.node_id, node.node_index, node.barycenter, node.summed_weight, node.degree
            )
        })
        .collect::<Vec<_>>()
        .join(" -> ")
}
