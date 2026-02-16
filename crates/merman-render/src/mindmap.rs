use crate::json::from_value_ref;
use crate::model::{Bounds, LayoutEdge, LayoutNode, LayoutPoint, MindmapDiagramLayout};
use crate::text::WrapMode;
use crate::text::{TextMeasurer, TextStyle};
use crate::{Error, Result};
use serde_json::Value;

fn config_f64(cfg: &Value, path: &[&str]) -> Option<f64> {
    let mut v = cfg;
    for p in path {
        v = v.get(*p)?;
    }
    v.as_f64()
}

fn config_string(cfg: &Value, path: &[&str]) -> Option<String> {
    let mut v = cfg;
    for p in path {
        v = v.get(*p)?;
    }
    v.as_str().map(|s| s.to_string())
}

type MindmapModel = merman_core::diagrams::mindmap::MindmapDiagramRenderModel;
type MindmapNodeModel = merman_core::diagrams::mindmap::MindmapDiagramRenderNode;

fn mindmap_text_style(effective_config: &Value) -> TextStyle {
    // Mermaid mindmap labels are rendered via HTML `<foreignObject>` and inherit the global font.
    let font_family = config_string(effective_config, &["fontFamily"])
        .or_else(|| config_string(effective_config, &["themeVariables", "fontFamily"]))
        .or_else(|| Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()));
    let font_size = config_f64(effective_config, &["fontSize"])
        .unwrap_or(16.0)
        .max(1.0);
    TextStyle {
        font_family,
        font_size,
        font_weight: None,
    }
}

fn is_simple_markdown_label(text: &str) -> bool {
    // Conservative: only fast-path labels that would render as plain text inside a `<p>...</p>`
    // when passed through Mermaid's Markdown + sanitizer pipeline.
    if text.contains('\n') || text.contains('\r') {
        return false;
    }
    let trimmed = text.trim_start();
    let bytes = trimmed.as_bytes();
    // Line-leading markdown constructs that can change the HTML shape even without newlines.
    if bytes.first().is_some_and(|b| matches!(b, b'#' | b'>')) {
        return false;
    }
    if bytes.starts_with(b"- ") || bytes.starts_with(b"+ ") || bytes.starts_with(b"---") {
        return false;
    }
    // Ordered list: `1. item` / `1) item`
    let mut i = 0usize;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i > 0
        && i + 1 < bytes.len()
        && (bytes[i] == b'.' || bytes[i] == b')')
        && bytes[i + 1] == b' '
    {
        return false;
    }
    // Block/inline markdown triggers we don't want to replicate here.
    if text.contains('*')
        || text.contains('_')
        || text.contains('`')
        || text.contains('~')
        || text.contains('[')
        || text.contains(']')
        || text.contains('!')
        || text.contains('\\')
    {
        return false;
    }
    // HTML passthrough / entity patterns: keep the full markdown path.
    if text.contains('<') || text.contains('>') || text.contains('&') {
        return false;
    }
    true
}

fn mindmap_label_bbox_px(
    text: &str,
    label_type: &str,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    max_node_width_px: f64,
) -> (f64, f64) {
    // Mermaid mindmap labels are rendered via HTML `<foreignObject>` and respect
    // `mindmap.maxNodeWidth` (default 200px). When the raw label is wider than that, Mermaid
    // switches the label container to a fixed 200px width and allows HTML-like wrapping (e.g.
    // `white-space: break-spaces` in upstream SVG baselines).
    //
    // Mirror that by measuring with an explicit max width in HTML-like mode.
    let max_node_width_px = max_node_width_px.max(1.0);

    // Complex Markdown labels require the full DOM-like measurement path (bold/em deltas, inline
    // HTML, sanitizer edge cases). Keep the existing two-pass approach for those.
    if label_type == "markdown" && !is_simple_markdown_label(text) {
        let wrapped = crate::text::measure_markdown_with_flowchart_bold_deltas(
            measurer,
            text,
            style,
            Some(max_node_width_px),
            WrapMode::HtmlLike,
        );
        let unwrapped = crate::text::measure_markdown_with_flowchart_bold_deltas(
            measurer,
            text,
            style,
            None,
            WrapMode::HtmlLike,
        );
        return (
            wrapped.width.max(unwrapped.width).max(0.0),
            wrapped.height.max(0.0),
        );
    }

    let (wrapped, raw_width_px) = measurer.measure_wrapped_with_raw_width(
        text,
        style,
        Some(max_node_width_px),
        WrapMode::HtmlLike,
    );

    // Mermaid mindmap labels can overflow the configured `maxNodeWidth` when they contain long
    // unbreakable tokens. Upstream measures these via DOM in a way that resembles `scrollWidth`,
    // so keep the larger of:
    // - the wrapped layout width (clamped by `max-width`), and
    // - the unwrapped overflow width (ignores `max-width`).
    let overflow_width_px = raw_width_px.unwrap_or_else(|| {
        measurer
            .measure_wrapped(text, style, None, WrapMode::HtmlLike)
            .width
    });

    (
        wrapped.width.max(overflow_width_px).max(0.0),
        wrapped.height.max(0.0),
    )
}

fn mindmap_node_dimensions_px(
    node: &MindmapNodeModel,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    max_node_width_px: f64,
) -> (f64, f64) {
    let (bbox_w, bbox_h) = mindmap_label_bbox_px(
        &node.label,
        &node.label_type,
        measurer,
        style,
        max_node_width_px,
    );
    let padding = node.padding.max(0.0);
    let half_padding = padding / 2.0;

    // Align with Mermaid shape sizing rules for mindmap nodes (via `labelHelper(...)` + shape
    // handlers in `rendering-elements/shapes/*`).
    match node.shape.as_str() {
        // `defaultMindmapNode.ts`: w = bbox.width + 8 * halfPadding; h = bbox.height + 2 * halfPadding
        "" | "defaultMindmapNode" => (bbox_w + 8.0 * half_padding, bbox_h + 2.0 * half_padding),
        // Mindmap node shapes use the standard `labelHelper(...)` label bbox, but mindmap DB
        // adjusts `node.padding` depending on the delimiter type (e.g. `[` / `(` / `{{`).
        //
        // Upstream Mermaid@11.12.2 mindmap SVG baselines show:
        // - rect (`[text]`): w = bbox.width + 2*padding, h = bbox.height + padding
        // - rounded (`(text)`): w = bbox.width + 1.5*padding, h = bbox.height + 1.5*padding
        "rect" => (bbox_w + 2.0 * padding, bbox_h + padding),
        "rounded" => (bbox_w + 1.5 * padding, bbox_h + 1.5 * padding),
        // `mindmapCircle.ts` -> `circle.ts`: radius = bbox.width/2 + padding (mindmap passes full padding)
        "mindmapCircle" => {
            let d = bbox_w + 2.0 * padding;
            (d, d)
        }
        // `cloud.ts`: w = bbox.width + 2*halfPadding; h = bbox.height + 2*halfPadding
        "cloud" => (bbox_w + 2.0 * half_padding, bbox_h + 2.0 * half_padding),
        // `bang.ts`: effectiveWidth = bbox.width + 10*halfPadding (min bbox+20 is always smaller here)
        //           effectiveHeight = bbox.height + 8*halfPadding (min bbox+20 is always smaller here)
        "bang" => (bbox_w + 10.0 * half_padding, bbox_h + 8.0 * half_padding),
        // `hexagon.ts`: h = bbox.height + padding; w = bbox.width + 2.5*padding; then expands by +w/6
        // due to `halfWidth = w/2 + m` where `m = (w/2)/6`.
        "hexagon" => {
            let w = bbox_w + 2.5 * padding;
            let h = bbox_h + padding;
            (w * (7.0 / 6.0), h)
        }
        _ => (bbox_w + 8.0 * half_padding, bbox_h + 2.0 * half_padding),
    }
}

fn compute_bounds(nodes: &[LayoutNode], edges: &[LayoutEdge]) -> Option<Bounds> {
    let mut pts: Vec<(f64, f64)> = Vec::new();
    for n in nodes {
        let x0 = n.x - n.width / 2.0;
        let y0 = n.y - n.height / 2.0;
        let x1 = n.x + n.width / 2.0;
        let y1 = n.y + n.height / 2.0;
        pts.push((x0, y0));
        pts.push((x1, y1));
    }
    for e in edges {
        for p in &e.points {
            pts.push((p.x, p.y));
        }
    }
    Bounds::from_points(pts)
}

fn shift_nodes_to_positive_bounds(nodes: &mut [LayoutNode], content_min: f64) {
    if nodes.is_empty() {
        return;
    }
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    for n in nodes.iter() {
        min_x = min_x.min(n.x - n.width / 2.0);
        min_y = min_y.min(n.y - n.height / 2.0);
    }
    if !(min_x.is_finite() && min_y.is_finite()) {
        return;
    }
    let dx = content_min - min_x;
    let dy = content_min - min_y;
    for n in nodes.iter_mut() {
        n.x += dx;
        n.y += dy;
    }
}

pub fn layout_mindmap_diagram(
    model: &Value,
    effective_config: &Value,
    text_measurer: &dyn TextMeasurer,
    use_manatee_layout: bool,
) -> Result<MindmapDiagramLayout> {
    let model: MindmapModel = from_value_ref(model)?;
    layout_mindmap_diagram_model(&model, effective_config, text_measurer, use_manatee_layout)
}

pub fn layout_mindmap_diagram_typed(
    model: &MindmapModel,
    effective_config: &Value,
    text_measurer: &dyn TextMeasurer,
    use_manatee_layout: bool,
) -> Result<MindmapDiagramLayout> {
    layout_mindmap_diagram_model(model, effective_config, text_measurer, use_manatee_layout)
}

fn layout_mindmap_diagram_model(
    model: &MindmapModel,
    effective_config: &Value,
    text_measurer: &dyn TextMeasurer,
    use_manatee_layout: bool,
) -> Result<MindmapDiagramLayout> {
    let timing_enabled = std::env::var("MERMAN_MINDMAP_LAYOUT_TIMING")
        .ok()
        .as_deref()
        == Some("1");
    #[derive(Debug, Default, Clone)]
    struct MindmapLayoutTimings {
        total: std::time::Duration,
        measure_nodes: std::time::Duration,
        manatee: std::time::Duration,
        build_edges: std::time::Duration,
        bounds: std::time::Duration,
    }
    let mut timings = MindmapLayoutTimings::default();
    let total_start = timing_enabled.then(std::time::Instant::now);

    let text_style = mindmap_text_style(effective_config);
    let max_node_width_px = config_f64(effective_config, &["mindmap", "maxNodeWidth"])
        .unwrap_or(200.0)
        .max(1.0);

    let measure_nodes_start = timing_enabled.then(std::time::Instant::now);
    let mut nodes_sorted: Vec<(i64, &MindmapNodeModel)> = model
        .nodes
        .iter()
        .map(|n| (n.id.parse::<i64>().unwrap_or(i64::MAX), n))
        .collect();
    nodes_sorted.sort_by(|(na, a), (nb, b)| na.cmp(nb).then_with(|| a.id.cmp(&b.id)));

    let mut nodes: Vec<LayoutNode> = Vec::with_capacity(model.nodes.len());
    for (_id_num, n) in nodes_sorted {
        let (width, height) =
            mindmap_node_dimensions_px(n, text_measurer, &text_style, max_node_width_px);

        nodes.push(LayoutNode {
            id: n.id.clone(),
            // Mermaid mindmap uses Cytoscape COSE-Bilkent and initializes node positions at (0,0).
            // We keep that behavior so `manatee` can reproduce upstream placements deterministically.
            x: 0.0,
            y: 0.0,
            width: width.max(1.0),
            height: height.max(1.0),
            is_cluster: false,
            label_width: None,
            label_height: None,
        });
    }
    if let Some(s) = measure_nodes_start {
        timings.measure_nodes = s.elapsed();
    }

    if use_manatee_layout {
        let manatee_start = timing_enabled.then(std::time::Instant::now);
        let mut id_to_idx: rustc_hash::FxHashMap<&str, usize> =
            rustc_hash::FxHashMap::with_capacity_and_hasher(nodes.len(), Default::default());
        for (idx, n) in nodes.iter().enumerate() {
            id_to_idx.insert(n.id.as_str(), idx);
        }

        let indexed_nodes: Vec<manatee::algo::cose_bilkent::IndexedNode> = nodes
            .iter()
            .map(|n| manatee::algo::cose_bilkent::IndexedNode {
                width: n.width,
                height: n.height,
                x: n.x,
                y: n.y,
            })
            .collect();
        let mut indexed_edges: Vec<manatee::algo::cose_bilkent::IndexedEdge> =
            Vec::with_capacity(model.edges.len());
        for (edge_idx, e) in model.edges.iter().enumerate() {
            let Some(&a) = id_to_idx.get(e.start.as_str()) else {
                return Err(Error::InvalidModel {
                    message: format!("edge start node not found: {}", e.start),
                });
            };
            let Some(&b) = id_to_idx.get(e.end.as_str()) else {
                return Err(Error::InvalidModel {
                    message: format!("edge end node not found: {}", e.end),
                });
            };
            if a == b {
                continue;
            }
            indexed_edges.push(manatee::algo::cose_bilkent::IndexedEdge { a, b });

            // Keep `edge_idx` referenced so unused warnings don't obscure failures if we ever
            // enhance indexed validation error messages.
            let _ = edge_idx;
        }

        let positions = manatee::algo::cose_bilkent::layout_indexed(
            &indexed_nodes,
            &indexed_edges,
            &Default::default(),
        )
        .map_err(|e| Error::InvalidModel {
            message: format!("manatee layout failed: {e}"),
        })?;

        for (n, p) in nodes.iter_mut().zip(positions) {
            n.x = p.x;
            n.y = p.y;
        }
        if let Some(s) = manatee_start {
            timings.manatee = s.elapsed();
        }
    }

    // Mermaid's COSE-Bilkent post-layout normalizes to a positive coordinate space via
    // `transform(0,0)` (layout-base), yielding a content bbox that starts around (15,15) before
    // the 10px viewport padding is applied (viewBox starts at 5,5).
    //
    // When we do NOT use the manatee COSE port, keep a compatibility translation so parity-root
    // viewport comparisons remain stable.
    if !use_manatee_layout {
        shift_nodes_to_positive_bounds(&mut nodes, 15.0);
    }

    let build_edges_start = timing_enabled.then(std::time::Instant::now);
    let mut id_to_idx: rustc_hash::FxHashMap<&str, usize> =
        rustc_hash::FxHashMap::with_capacity_and_hasher(nodes.len(), Default::default());
    for (idx, n) in nodes.iter().enumerate() {
        id_to_idx.insert(n.id.as_str(), idx);
    }

    let mut edges: Vec<LayoutEdge> = Vec::new();
    for e in &model.edges {
        let Some(&sidx) = id_to_idx.get(e.start.as_str()) else {
            return Err(Error::InvalidModel {
                message: format!("edge start node not found: {}", e.start),
            });
        };
        let Some(&tidx) = id_to_idx.get(e.end.as_str()) else {
            return Err(Error::InvalidModel {
                message: format!("edge end node not found: {}", e.end),
            });
        };
        let (sx, sy) = (nodes[sidx].x, nodes[sidx].y);
        let (tx, ty) = (nodes[tidx].x, nodes[tidx].y);
        let points = vec![LayoutPoint { x: sx, y: sy }, LayoutPoint { x: tx, y: ty }];
        edges.push(LayoutEdge {
            id: e.id.clone(),
            from: e.start.clone(),
            to: e.end.clone(),
            from_cluster: None,
            to_cluster: None,
            points,
            label: None,
            start_label_left: None,
            start_label_right: None,
            end_label_left: None,
            end_label_right: None,
            start_marker: None,
            end_marker: None,
            stroke_dasharray: None,
        });
    }
    if let Some(s) = build_edges_start {
        timings.build_edges = s.elapsed();
    }

    let bounds_start = timing_enabled.then(std::time::Instant::now);
    let bounds = compute_bounds(&nodes, &edges);
    if let Some(s) = bounds_start {
        timings.bounds = s.elapsed();
    }
    if let Some(s) = total_start {
        timings.total = s.elapsed();
        eprintln!(
            "[layout-timing] diagram=mindmap total={:?} measure_nodes={:?} manatee={:?} build_edges={:?} bounds={:?} nodes={} edges={}",
            timings.total,
            timings.measure_nodes,
            timings.manatee,
            timings.build_edges,
            timings.bounds,
            nodes.len(),
            edges.len(),
        );
    }
    Ok(MindmapDiagramLayout {
        nodes,
        edges,
        bounds,
    })
}
